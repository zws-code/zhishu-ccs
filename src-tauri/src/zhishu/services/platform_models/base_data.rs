//! 平台基础数据的设备本地快照：保存分组、模型及分组 Key，供离线的本地同步使用。
//! 返回前端的摘要只包含白名单字段；单个 Key 必须经显式查看入口和账户归属校验读取。
use crate::zhishu::services::platform_auth::{error, GroupDiscovery, PlatformAuthError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize)]
pub(crate) struct CachedCatalog {
    version: u32,
    synced_at: i64,
    pub discovery: GroupDiscovery,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSummary {
    base_url: String,
    user_id: i64,
    synced_at: i64,
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
    quota: Option<super::super::client_api::ClientQuota>,
    subscriptions: Vec<super::super::client_api::ClientSubscription>,
    credential: Option<super::super::client_api::CredentialMetadata>,
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
        let context = super::catalog::positive(model, "context_window");
        let output = super::catalog::positive(model, "max_output_tokens");
        // 只投影展示字段，避免任意服务端字段或嵌套凭据进入前端查询缓存。
        Some(Self {
            name: text("display_name").unwrap_or_else(|| id.clone()),
            id,
            description: text("description"),
            context_window: (context > 0).then_some(context),
            max_output_tokens: (output > 0).then_some(output),
            endpoints: model
                .get("supported_endpoint_types")
                .or_else(|| model.get("supported_endpoints"))
                .map(|endpoints| super::catalog::strings(Some(endpoints)))
                .unwrap_or_default(),
            supports_tools: model
                .get("supports_tools")
                .or_else(|| model.get("supports_function_calling"))
                .and_then(serde_json::Value::as_bool),
            supports_streaming: model
                .get("supports_streaming")
                .and_then(serde_json::Value::as_bool),
            input_modalities: super::catalog::strings(model.get("input_modalities")),
            reasoning_levels: super::catalog::strings(model.get("supported_reasoning_levels")),
            default_reasoning_level: text("default_reasoning_level"),
        })
    }
}

impl CachedCatalog {
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
            warnings: self.discovery.warnings.clone(),
            groups: self
                .discovery
                .groups
                .iter()
                .map(|group| {
                    let catalog = catalogs.get(&group.id).copied();
                    let details = self.discovery.details.get(&group.id);
                    GroupSummary {
                        quota: details.map(|details| details.quota.clone()),
                        subscriptions: details
                            .map(|details| details.subscriptions.clone())
                            .unwrap_or_default(),
                        credential: details.and_then(|details| details.credential.clone()),
                        id: group.id,
                        name: group.name.clone(),
                        platform: group.platform.clone(),
                        subscription_type: group.subscription_type.clone(),
                        status: group.status.clone(),
                        key_id: catalog.map(|c| c.credential.key_id),
                        key_name: catalog.map(|c| c.credential.key_name.clone()),
                        key_available: catalog.is_some_and(|c| !c.credential.key.is_empty()),
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

pub(crate) struct CatalogStore {
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
            path: crate::zhishu::storage::platform_storage::data_file(
                handle,
                "platform-catalog.json",
            )
            .map_err(|_| storage_error())?,
        })
    }

    pub(crate) fn account_path(&self, origin: &str, user_id: i64) -> PathBuf {
        let identity = serde_json::to_vec(&(origin, user_id)).expect("account identity");
        let name = format!("{:x}.json", Sha256::digest(identity));
        self.path.with_file_name("platform-catalogs").join(name)
    }

    fn value(path: &Path) -> Result<Option<serde_json::Value>, PlatformAuthError> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| storage_error()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(storage_error()),
        }
    }

    fn belongs_to(value: &serde_json::Value, origin: &str, user_id: i64) -> bool {
        value
            .pointer("/discovery/account/baseUrl")
            .and_then(serde_json::Value::as_str)
            == Some(origin)
            && value
                .pointer("/discovery/account/user/id")
                .and_then(serde_json::Value::as_i64)
                == Some(user_id)
    }

    fn decode(value: serde_json::Value) -> Result<Option<CachedCatalog>, PlatformAuthError> {
        // v1 不迁移、不使用旧 Key；当前 v2 只改变存储位置，不改变凭证契约。
        if value.is_null() || value.get("version").and_then(serde_json::Value::as_u64) != Some(2) {
            return Ok(None);
        }
        let cached: CachedCatalog = serde_json::from_value(value).map_err(|_| storage_error())?;
        if cached
            .discovery
            .catalogs
            .iter()
            .any(|catalog| catalog.credential.key.trim().is_empty())
        {
            return Err(storage_error());
        }
        Ok(Some(cached))
    }

    pub fn read(
        &self,
        origin: &str,
        user_id: i64,
    ) -> Result<Option<CachedCatalog>, PlatformAuthError> {
        let path = self.account_path(origin, user_id);
        if let Some(value) = Self::value(&path)? {
            if !Self::belongs_to(&value, origin, user_id) {
                return Err(storage_error());
            }
            return Self::decode(value);
        }
        let Some(value) = Self::value(&self.path)? else {
            return Ok(None);
        };
        if !Self::belongs_to(&value, origin, user_id) {
            return Ok(None);
        }
        let Some(cached) = Self::decode(value)? else {
            return Ok(None);
        };
        let bytes = serde_json::to_vec(&cached).map_err(|_| storage_error())?;
        // 先成功写入私密文件再移除旧快照；失败保留原件，下次可重试或显式清理。
        crate::config::atomic_write_private(&path, &bytes).map_err(|_| storage_error())?;
        Self::remove_file(&self.path)?;
        Ok(Some(cached))
    }

    pub fn save(&self, discovery: GroupDiscovery) -> Result<CachedCatalog, PlatformAuthError> {
        let cached = CachedCatalog {
            version: 2,
            synced_at: discovery.synced_at,
            discovery,
        };
        let account = &cached.discovery.account;
        let bytes = serde_json::to_vec(&cached).map_err(|_| storage_error())?;
        crate::config::atomic_write_private(
            &self.account_path(&account.base_url, account.user.id),
            &bytes,
        )
        .map_err(|_| storage_error())?;
        // 刷新可以修复损坏快照。旧文件归属不明时保留，不阻塞新的有效快照保存。
        if self
            .legacy_matches(&account.base_url, account.user.id)
            .unwrap_or(false)
        {
            Self::remove_file(&self.path)?;
        }
        Ok(cached)
    }

    fn legacy_matches(&self, origin: &str, user_id: i64) -> Result<bool, PlatformAuthError> {
        let Some(value) = Self::value(&self.path)? else {
            return Ok(false);
        };
        if value.is_null() {
            return Ok(false);
        }
        if value
            .pointer("/discovery/account/baseUrl")
            .and_then(serde_json::Value::as_str)
            .is_none()
            || value
                .pointer("/discovery/account/user/id")
                .and_then(serde_json::Value::as_i64)
                .is_none()
        {
            return Err(storage_error());
        }
        Ok(Self::belongs_to(&value, origin, user_id))
    }

    fn remove_file(path: &Path) -> Result<(), PlatformAuthError> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(storage_error()),
        }
    }

    pub(crate) fn remove_account(
        &self,
        origin: &str,
        user_id: i64,
    ) -> Result<(), PlatformAuthError> {
        // 先删归属明确的旧快照，防止成功清理后再次迁移；损坏的未知归属文件保留并报告。
        let legacy = self.legacy_matches(origin, user_id).and_then(|matches| {
            if matches {
                Self::remove_file(&self.path)
            } else {
                Ok(())
            }
        });
        // 账号路径由已验证身份生成，允许删除损坏的当前账号文件。
        let scoped = Self::remove_file(&self.account_path(origin, user_id));
        legacy.and(scoped)
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
            .filter(|key| !key.is_empty()))
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

    fn discovery(origin: &str, user_id: i64) -> GroupDiscovery {
        serde_json::from_value(json!({
            "account":{"baseUrl":origin,"user":{"id":user_id,"email":"test@example.com","balance":0}},
            "groups":[],"catalogs":[],"failed_group_ids":[],"subscription_group_ids":[],"warnings":[]
        })).unwrap()
    }

    #[test]
    fn account_files_survive_switching_and_cleanup_leaves_others_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let store = CatalogStore::for_test(dir.path().join("platform-catalog.json"));
        let identities = [
            ("https://platform.test", 1),
            ("https://platform.test", 2),
            ("https://other.test", 1),
        ];
        for (origin, id) in identities {
            store.save(discovery(origin, id)).unwrap();
        }
        let others: Vec<_> = identities[1..]
            .iter()
            .map(|(origin, id)| {
                let path = store.account_path(origin, *id);
                (path.clone(), std::fs::read(path).unwrap())
            })
            .collect();
        for (origin, id) in identities {
            assert_eq!(
                store
                    .read(origin, id)
                    .unwrap()
                    .unwrap()
                    .discovery
                    .account
                    .user
                    .id,
                id
            );
        }
        // Explicit cleanup can remove a corrupt scoped file without reading credentials.
        std::fs::write(store.account_path(identities[0].0, 1), b"broken").unwrap();
        store.remove_account(identities[0].0, 1).unwrap();
        store.remove_account(identities[0].0, 1).unwrap();
        assert!(store.read(identities[0].0, 1).unwrap().is_none());
        for (path, bytes) in others {
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }

    #[test]
    fn matching_v2_legacy_migrates_privately_and_other_account_cannot_consume_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("platform-catalog.json");
        let store = CatalogStore::for_test(path.clone());
        let cached = CachedCatalog {
            version: 2,
            synced_at: 123,
            discovery: discovery("https://platform.test", 1),
        };
        let bytes = serde_json::to_vec(&cached).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert!(store.read("https://platform.test", 2).unwrap().is_none());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            store
                .read("https://platform.test", 1)
                .unwrap()
                .unwrap()
                .synced_at,
            123
        );
        assert!(!path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(store.account_path("https://platform.test", 1))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        store.remove_account("https://platform.test", 1).unwrap();
        assert!(store.read("https://platform.test", 1).unwrap().is_none());
    }

    #[test]
    fn failed_migration_keeps_legacy_and_retry_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("platform-catalog.json");
        let store = CatalogStore::for_test(path.clone());
        let bytes = serde_json::to_vec(&CachedCatalog {
            version: 2,
            synced_at: 1,
            discovery: discovery("https://platform.test", 1),
        })
        .unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let blocker = dir.path().join("platform-catalogs");
        std::fs::write(&blocker, b"block directory").unwrap();
        assert!(store.read("https://platform.test", 1).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(blocker).unwrap();
        assert!(store.read("https://platform.test", 1).unwrap().is_some());
    }

    #[test]
    fn cleanup_removes_matching_legacy_but_reports_unknown_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("platform-catalog.json");
        let store = CatalogStore::for_test(path.clone());
        store.save(discovery("https://platform.test", 1)).unwrap();
        let legacy = serde_json::to_vec(&CachedCatalog {
            version: 1,
            synced_at: 1,
            discovery: discovery("https://platform.test", 1),
        })
        .unwrap();
        std::fs::write(&path, legacy).unwrap();
        store.remove_account("https://platform.test", 1).unwrap();
        assert!(!path.exists());
        assert!(store.read("https://platform.test", 1).unwrap().is_none());
        std::fs::write(&path, b"unknown owner").unwrap();
        store.save(discovery("https://platform.test", 1)).unwrap();
        assert!(store.remove_account("https://platform.test", 1).is_err());
        assert!(!store.account_path("https://platform.test", 1).exists());
        assert_eq!(std::fs::read(&path).unwrap(), b"unknown owner");
    }

    #[test]
    fn embedded_wrong_owner_is_rejected_without_harming_other_account() {
        let dir = tempfile::tempdir().unwrap();
        let store = CatalogStore::for_test(dir.path().join("platform-catalog.json"));
        store.save(discovery("https://platform.test", 2)).unwrap();
        std::fs::copy(
            store.account_path("https://platform.test", 2),
            store.account_path("https://platform.test", 1),
        )
        .unwrap();
        assert!(store.read("https://platform.test", 1).is_err());
        assert!(store.read("https://platform.test", 2).unwrap().is_some());
    }

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
            "version":2,"synced_at":123,
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
    #[test]
    fn legacy_snapshot_is_not_migrated_or_used_and_remains_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.json");
        let bytes = br#"{"version":1,"legacy_key":"old-secret","discovery":{}}"#;
        std::fs::write(&path, bytes).unwrap();
        let store = CatalogStore::for_test(path.clone());
        assert!(store.read("https://platform.test", 1).unwrap().is_none());
        assert_eq!(
            store
                .group_key("https://platform.test", 1, 42)
                .err()
                .unwrap()
                .code,
            "CATALOG_REQUIRED"
        );
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
