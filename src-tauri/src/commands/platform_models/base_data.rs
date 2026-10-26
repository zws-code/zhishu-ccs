//! 平台基础数据的设备本地快照：保存分组、模型及分组 Key，供离线的本地同步使用。
//! 返回前端的摘要只包含白名单字段；单个 Key 必须经显式查看入口和账户归属校验读取。
use super::super::platform_auth::{error, GroupDiscovery, PlatformAuthError};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Deserialize, Serialize)]
pub(super) struct CachedCatalog {
    version: u32,
    synced_at: i64,
    #[serde(default)]
    credentials_cleared: bool,
    pub discovery: GroupDiscovery,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSummary {
    base_url: String,
    user_id: i64,
    synced_at: i64,
    credentials_cleared: bool,
    groups: Vec<GroupSummary>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupSummary {
    id: i64,
    name: String,
    platform: String,
    subscription_type: String,
    status: String,
    key_id: Option<i64>,
    key_name: Option<String>,
    key_available: bool,
    failed: bool,
    error: Option<String>,
    models: Vec<ModelSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelSummary {
    id: String,
    name: String,
    description: Option<String>,
    context_window: Option<u64>,
    max_output_tokens: Option<u64>,
    endpoints: Vec<String>,
    supports_tools: Option<bool>,
    supports_streaming: Option<bool>,
    input_modalities: Vec<String>,
    reasoning_levels: Vec<String>,
    default_reasoning_level: Option<String>,
}

impl ModelSummary {
    fn from_value(model: &serde_json::Value) -> Option<Self> {
        let text = |field: &str| {
            model
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let id = text("id")?;
        let context = super::catalog::positive(
            model,
            &[
                "/context_window",
                "/contextWindow",
                "/max_input_tokens",
                "/limit/context",
            ],
        );
        let output = super::catalog::positive(
            model,
            &["/max_output_tokens", "/maxOutputTokens", "/limit/output"],
        );
        // 只投影展示字段，避免任意服务端字段或嵌套凭据进入前端查询缓存。
        Some(Self {
            name: text("display_name")
                .or_else(|| text("name"))
                .unwrap_or_else(|| id.clone()),
            id,
            description: text("description"),
            context_window: (context > 0).then_some(context),
            max_output_tokens: (output > 0).then_some(output),
            endpoints: model
                .get("supported_endpoint_types")
                .or_else(|| model.get("supported_endpoints"))
                .map(|endpoints| super::catalog::strings(Some(endpoints)))
                .unwrap_or_else(|| text("api_format").into_iter().collect()),
            supports_tools: model
                .get("supports_tools")
                .or_else(|| model.get("supports_function_calling"))
                .and_then(serde_json::Value::as_bool),
            supports_streaming: model
                .get("supports_streaming")
                .and_then(serde_json::Value::as_bool),
            input_modalities: super::catalog::strings(
                model
                    .get("input_modalities")
                    .or_else(|| model.pointer("/modalities/input")),
            ),
            reasoning_levels: super::catalog::strings(
                model
                    .get("supported_reasoning_levels")
                    .or_else(|| model.get("reasoningLevels")),
            ),
            default_reasoning_level: text("default_reasoning_level")
                .or_else(|| text("defaultReasoningLevel")),
        })
    }
}

impl CachedCatalog {
    pub fn credentials_cleared(&self) -> bool {
        self.credentials_cleared
    }

    pub fn summary(&self) -> CatalogSummary {
        // 一次建立分组索引，避免为每个分组重复扫描整份目录；保持分组原顺序和首项优先。
        let mut catalogs = HashMap::with_capacity(self.discovery.catalogs.len());
        for catalog in &self.discovery.catalogs {
            catalogs.entry(catalog.group.id).or_insert(catalog);
        }
        CatalogSummary {
            base_url: self.discovery.account.base_url.clone(),
            user_id: self.discovery.account.user.id,
            synced_at: self.synced_at,
            credentials_cleared: self.credentials_cleared,
            warnings: self.discovery.warnings.clone(),
            groups: self
                .discovery
                .groups
                .iter()
                .map(|group| {
                    let catalog = catalogs.get(&group.id).copied();
                    GroupSummary {
                        id: group.id,
                        name: group.name.clone(),
                        platform: group.platform.clone(),
                        subscription_type: group.subscription_type.clone(),
                        status: group.status.clone(),
                        key_id: catalog.map(|c| c.credential.key_id),
                        key_name: catalog.map(|c| c.credential.key_name.clone()),
                        key_available: !self.credentials_cleared
                            && catalog.is_some_and(|c| {
                                !c.credential.key.is_empty()
                                    && c.credential.key != "PLATFORM_LOGIN_REQUIRED"
                            }),
                        failed: self.discovery.failed_group_ids.contains(&group.id),
                        error: catalog.and_then(|c| c.catalog_error.clone()),
                        models: catalog
                            .map(|c| {
                                c.models
                                    .iter()
                                    .filter_map(ModelSummary::from_value)
                                    .collect()
                            })
                            .unwrap_or_default(),
                    }
                })
                .collect(),
        }
    }
}

pub(super) struct CatalogStore {
    path: PathBuf,
}

fn storage_error() -> PlatformAuthError {
    error(
        "CATALOG_STORE",
        "无法读取或保存平台基础数据，请检查应用数据目录权限后重试",
    )
}

impl CatalogStore {
    pub fn for_app(handle: &tauri::AppHandle) -> Result<Self, PlatformAuthError> {
        Ok(Self {
            path: super::super::platform_storage::data_file(handle, "platform-catalog.json")
                .map_err(|_| storage_error())?,
        })
    }

    fn load(&self) -> Result<Option<CachedCatalog>, PlatformAuthError> {
        let cached: Option<CachedCatalog> = match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| storage_error())?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(storage_error()),
        };
        if cached.as_ref().is_some_and(|c| c.version != 1) {
            return Err(storage_error());
        }
        Ok(cached)
    }

    pub fn read(
        &self,
        origin: &str,
        user_id: i64,
    ) -> Result<Option<CachedCatalog>, PlatformAuthError> {
        Ok(self.load()?.filter(|cached| {
            cached.discovery.account.base_url == origin
                && cached.discovery.account.user.id == user_id
        }))
    }

    pub fn save(&self, discovery: GroupDiscovery) -> Result<CachedCatalog, PlatformAuthError> {
        let cached = CachedCatalog {
            version: 1,
            synced_at: chrono::Utc::now().timestamp_millis(),
            credentials_cleared: false,
            discovery,
        };
        let bytes = serde_json::to_vec(&cached).map_err(|_| storage_error())?;
        // Atomic replacement leaves the previous usable snapshot intact on failure.
        crate::config::atomic_write_private(&self.path, &bytes).map_err(|_| storage_error())?;
        Ok(cached)
    }

    pub fn group_key(
        &self,
        origin: &str,
        user_id: i64,
        group_id: i64,
    ) -> Result<Option<String>, PlatformAuthError> {
        let cached = self
            .read(origin, user_id)?
            .ok_or_else(|| error("CATALOG_REQUIRED", "请先同步当前账号的平台基础数据"))?;
        if cached.credentials_cleared {
            return Ok(None);
        }
        if !cached
            .discovery
            .groups
            .iter()
            .any(|group| group.id == group_id)
        {
            return Ok(None);
        }
        Ok(cached
            .discovery
            .catalogs
            .iter()
            .find(|catalog| catalog.group.id == group_id)
            .map(|catalog| catalog.credential.key.clone())
            .filter(|key| !key.is_empty() && key != "PLATFORM_LOGIN_REQUIRED"))
    }

    pub fn clear_credentials(&self) -> Result<(), PlatformAuthError> {
        let cached = match self.load() {
            Ok(Some(cached)) => cached,
            Ok(None) => return Ok(()),
            Err(_) => {
                // Corrupt data cannot be safely retained with unknown secrets.
                return crate::config::atomic_write_private(&self.path, b"null")
                    .map_err(|_| storage_error());
            }
        };
        let mut value = serde_json::to_value(&cached).map_err(|_| storage_error())?;
        // Also scrub any echoed credentials in model metadata or warnings.
        for catalog in &cached.discovery.catalogs {
            if !catalog.credential.key.is_empty() {
                super::redact_credential(&mut value, &catalog.credential.key);
            }
        }
        let mut retained: CachedCatalog =
            serde_json::from_value(value).map_err(|_| storage_error())?;
        for catalog in &mut retained.discovery.catalogs {
            catalog.credential.key.clear();
        }
        retained.credentials_cleared = true;
        // Explicit credential cleanup does not refresh platform data.
        let bytes = serde_json::to_vec(&retained).map_err(|_| storage_error())?;
        crate::config::atomic_write_private(&self.path, &bytes).map_err(|_| storage_error())
    }

    #[cfg(test)]
    pub fn for_test(path: PathBuf) -> Self {
        Self { path }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn summary_keeps_group_order_missing_catalogs_and_first_matching_catalog() {
        let group = |id| json!({"id":id,"name":format!("group-{id}"),"platform":"openai"});
        let catalog = |id, model| {
            json!({
                "group":group(id),
                "credential":{"key":"test-only-secret","key_id":id,"platform":"openai","key_name":"dedicated"},
                "models":[{"id":model}],"catalog_error":null
            })
        };
        let cached: CachedCatalog = serde_json::from_value(json!({
            "version":1,"synced_at":123,
            "discovery":{
                "account":{"baseUrl":"https://platform.test","user":{"id":1,"email":"test@example.com","role":"user","balance":0}},
                "groups":[group(2),group(1),group(3)],
                "catalogs":[catalog(1,"first"),catalog(2,"second"),catalog(1,"duplicate")],
                "failed_group_ids":[3],"subscription_group_ids":[],"warnings":[]
            }
        })).unwrap();
        let summary = cached.summary();
        assert_eq!(
            summary
                .groups
                .iter()
                .map(|group| group.id)
                .collect::<Vec<_>>(),
            [2, 1, 3]
        );
        assert_eq!(summary.groups[0].models[0].id, "second");
        assert_eq!(summary.groups[1].models[0].id, "first");
        assert!(summary.groups[2].failed);
        assert!(!summary.groups[2].key_available);
        assert!(summary.groups[2].models.is_empty());
        assert!(!serde_json::to_string(&summary)
            .unwrap()
            .contains("test-only-secret"));
    }

    #[test]
    fn model_details_expose_reported_parameters_without_raw_credentials() {
        let model = json!({
            "id": "test-model", "display_name": "Test Model", "context_window": "128000",
            "max_output_tokens": 4096, "supported_endpoint_types": ["anthropic"],
            "supports_tools": true, "supports_streaming": false,
            "supported_reasoning_levels": [{"effort": "low"}, {"effort": "high"}],
            "default_reasoning_level": "high", "input_modalities": ["text", "image"],
            "api_key": "private-model-key", "auth": {"token": "private-model-token"}
        });
        let summary = serde_json::to_value(ModelSummary::from_value(&model).unwrap()).unwrap();
        assert_eq!(summary["contextWindow"], 128000);
        assert_eq!(summary["maxOutputTokens"], 4096);
        assert_eq!(summary["supportsTools"], true);
        assert_eq!(summary["supportsStreaming"], false);
        assert_eq!(summary["endpoints"], json!(["anthropic"]));
        assert_eq!(summary["reasoningLevels"], json!(["low", "high"]));
        assert_eq!(summary["defaultReasoningLevel"], "high");
        assert_eq!(summary["inputModalities"], json!(["text", "image"]));
        assert!(!summary.to_string().contains("private-model"));
        assert!(summary.get("api_key").is_none());
        assert!(summary.get("auth").is_none());
    }

    #[test]
    fn model_details_keep_unknown_capabilities_and_missing_limits_unknown() {
        let summary =
            serde_json::to_value(ModelSummary::from_value(&json!({"id":"id-only"})).unwrap())
                .unwrap();
        for field in [
            "contextWindow",
            "maxOutputTokens",
            "supportsTools",
            "supportsStreaming",
            "defaultReasoningLevel",
        ] {
            assert!(summary[field].is_null(), "{field}");
        }
        assert_eq!(summary["endpoints"], json!([]));
        assert_eq!(summary["reasoningLevels"], json!([]));
        assert!(ModelSummary::from_value(&json!({"name":"no id"})).is_none());
    }
}
