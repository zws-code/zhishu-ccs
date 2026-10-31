//! 客户端专用协议 DTO。只接受服务端授权范围，不查询或管理后台 Key。
use super::platform_auth::{
    error, AvailableGroup, GroupCatalog, GroupCredential, GroupDiscovery, PlatformAccount,
    PlatformAuthError, PlatformUser,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use url::Url;

pub(crate) const PREFIX: &str = "/api/zhishu/v1/client";

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct ClientAccount {
    pub id: i64,
    pub email: String,
    #[serde(default)]
    pub username: String,
    pub status: String,
    pub balance_usd: f64,
}

impl ClientAccount {
    pub fn into_user(self) -> Result<PlatformUser, PlatformAuthError> {
        if self.id <= 0 || self.status != "active" {
            return Err(error("UNAUTHORIZED", "账户暂不可用，请重新登录"));
        }
        Ok(PlatformUser {
            id: self.id,
            email: self.email,
            username: self.username,
            role: String::new(),
            status: self.status,
            balance: self.balance_usd,
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ClientQuota {
    pub available: bool,
    pub reasons: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ClientWindow {
    pub limit_usd: Option<f64>,
    pub used_usd: f64,
    pub remaining_usd: Option<f64>,
    pub resets_at: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ClientSubscription {
    pub id: i64,
    pub group_id: i64,
    pub status: String,
    pub starts_at: String,
    pub expires_at: String,
    pub active: bool,
    pub daily: ClientWindow,
    pub weekly: ClientWindow,
    pub monthly: ClientWindow,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct CredentialMetadata {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub client_managed: bool,
    pub expires_at: Option<String>,
    pub quota_usd: f64,
    pub used_usd: f64,
}

#[derive(Deserialize)]
pub(crate) struct AccessCredential {
    #[serde(flatten)]
    pub metadata: CredentialMetadata,
    pub key: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct ClientConnection {
    pub base_url: String,
    pub endpoints: HashMap<String, String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct GroupDetails {
    pub quota: ClientQuota,
    pub subscriptions: Vec<ClientSubscription>,
    pub credential: Option<CredentialMetadata>,
    pub is_exclusive: bool,
}

#[derive(Deserialize)]
struct AccessGroup {
    #[serde(flatten)]
    group: AvailableGroup,
    is_exclusive: bool,
    quota: ClientQuota,
    subscriptions: Vec<ClientSubscription>,
    credential: Option<AccessCredential>,
    connection: ClientConnection,
    models: Option<Vec<Value>>,
    catalog_status: String,
    catalog_error: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct ClientAccess {
    schema_version: u32,
    synced_at: String,
    account: ClientAccount,
    groups: Vec<AccessGroup>,
}

pub(crate) fn invalid_response() -> PlatformAuthError {
    error(
        "INVALID_RESPONSE",
        "客户端接口响应格式无效，请更新平台后重试",
    )
}

pub(crate) fn snapshot_time(version: u32, time: &str) -> Result<i64, PlatformAuthError> {
    if version != 1 {
        return Err(invalid_response());
    }
    chrono::DateTime::parse_from_rfc3339(time)
        .map(|time| time.timestamp_millis())
        .map_err(|_| invalid_response())
}

/// 连接地址只用于模型配置；绝不改变登录 JWT 请求的目的地址。
pub(crate) fn connection_base(base: &str, platform: &str) -> Result<String, PlatformAuthError> {
    let url = if base.trim().is_empty() {
        let platform = Url::parse(platform).map_err(|_| invalid_response())?;
        Url::parse(&platform.origin().ascii_serialization()).map_err(|_| invalid_response())?
    } else {
        Url::parse(base).map_err(|_| invalid_response())?
    };
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_response());
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

impl ClientAccess {
    pub fn into_discovery(
        self,
        base_url: &str,
        user_id: i64,
        login_secrets: &[String],
    ) -> Result<GroupDiscovery, PlatformAuthError> {
        let synced_at = snapshot_time(self.schema_version, &self.synced_at)?;
        let user = self.account.into_user()?;
        if user.id != user_id {
            return Err(error("UNAUTHORIZED", "登录状态已改变，请重新登录"));
        }
        let mut secrets: Vec<String> = self
            .groups
            .iter()
            .filter_map(|group| group.credential.as_ref())
            .map(|credential| credential.key.clone())
            .collect();
        secrets.extend(
            login_secrets
                .iter()
                .filter(|value| !value.is_empty())
                .cloned(),
        );
        let mut result = GroupDiscovery {
            account: PlatformAccount {
                base_url: base_url.into(),
                user,
            },
            synced_at,
            groups: Vec::new(),
            catalogs: Vec::new(),
            details: HashMap::new(),
            failed_group_ids: HashSet::new(),
            subscription_group_ids: HashSet::new(),
            warnings: Vec::new(),
        };
        for mut entry in self.groups {
            let id = entry.group.id;
            if id <= 0
                || result.details.contains_key(&id)
                || entry.group.platform.trim().is_empty()
                || entry
                    .subscriptions
                    .iter()
                    .any(|subscription| subscription.group_id != id)
            {
                return Err(invalid_response());
            }
            entry.connection.base_url = connection_base(&entry.connection.base_url, base_url)?;
            let failed = match (entry.catalog_status.as_str(), entry.models.as_ref()) {
                ("ok", Some(_)) => false,
                ("failed", None)
                    if entry.catalog_error.as_deref() == Some("MODEL_CATALOG_UNAVAILABLE") =>
                {
                    true
                }
                _ => return Err(invalid_response()),
            };
            let mut models = Vec::new();
            for model in entry.models.unwrap_or_default() {
                let model_id = model
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.trim().is_empty() && !id.contains(['\r', '\n', '\0']))
                    .ok_or_else(invalid_response)?;
                let mut safe = serde_json::Map::new();
                safe.insert("id".into(), Value::String(model_id.into()));
                // 白名单也用于私密快照，避免任意嵌套响应进入后续 Provider 和前端摘要。
                for field in [
                    "display_name",
                    "description",
                    "context_window",
                    "max_output_tokens",
                    "supported_endpoint_types",
                    "supported_endpoints",
                    "supports_tools",
                    "supports_function_calling",
                    "supports_streaming",
                    "input_modalities",
                    "supported_reasoning_levels",
                    "default_reasoning_level",
                    "supports_parallel_tool_calls",
                    "reasoning",
                ] {
                    if let Some(value) = model.get(field) {
                        safe.insert(field.into(), value.clone());
                    }
                }
                models.push(Value::Object(safe));
            }
            if failed {
                result.failed_group_ids.insert(id);
                result
                    .warnings
                    .push(format!("分组 {id} 的模型目录暂不可用，已保留本地供应商"));
            }
            if !entry.subscriptions.is_empty() || entry.group.subscription_type == "subscription" {
                result.subscription_group_ids.insert(id);
            }
            let credential_metadata = entry
                .credential
                .as_ref()
                .map(|credential| credential.metadata.clone());
            if let Some(credential) = entry.credential {
                if credential.metadata.id <= 0
                    || !credential.metadata.client_managed
                    || credential.key.trim().is_empty()
                {
                    return Err(invalid_response());
                }
                result.catalogs.push(GroupCatalog {
                    group: entry.group.clone(),
                    credential: GroupCredential {
                        key: credential.key,
                        key_id: credential.metadata.id,
                        key_name: credential.metadata.name,
                        platform: entry.group.platform.clone(),
                    },
                    connection: Some(entry.connection),
                    models,
                    catalog_error: failed.then(|| "MODEL_CATALOG_UNAVAILABLE".into()),
                });
            }
            result.details.insert(
                id,
                GroupDetails {
                    quota: entry.quota,
                    subscriptions: entry.subscriptions,
                    credential: credential_metadata,
                    is_exclusive: entry.is_exclusive,
                },
            );
            result.groups.push(entry.group);
        }
        // Redact credentials echoed in otherwise allowlisted display fields.
        let keys: Vec<_> = result
            .catalogs
            .iter()
            .map(|catalog| catalog.credential.key.clone())
            .collect();
        let mut value = serde_json::to_value(&result).map_err(|_| invalid_response())?;
        redact_display(&mut value, &secrets);
        let mut result: GroupDiscovery =
            serde_json::from_value(value).map_err(|_| invalid_response())?;
        for (catalog, key) in result.catalogs.iter_mut().zip(keys) {
            catalog.credential.key = key;
        }
        Ok(result)
    }
}

fn redact_display(value: &mut Value, secrets: &[String]) {
    match value {
        Value::String(text) => *text = crate::redact_known_secrets_strict(text, secrets),
        Value::Array(items) => {
            for item in items {
                redact_display(item, secrets);
            }
        }
        Value::Object(fields) => {
            for item in fields.values_mut() {
                redact_display(item, secrets);
            }
        }
        _ => (),
    }
}

/// 单独的只读 DTO 根本没有 Key、连接或模型字段，不可能覆盖 /access 的本机快照。
#[derive(Clone, Deserialize, Serialize)]
pub struct UsageGroup {
    pub id: i64,
    pub name: String,
    pub platform: String,
    pub status: String,
    pub subscription_type: String,
    pub quota: ClientQuota,
    pub subscriptions: Vec<ClientSubscription>,
    pub credential: Option<CredentialMetadata>,
}

#[derive(Deserialize)]
pub(crate) struct ClientUsage {
    pub schema_version: u32,
    pub synced_at: String,
    pub account: ClientAccount,
    pub groups: Vec<UsageGroup>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformUsageSummary {
    pub base_url: String,
    pub user_id: i64,
    pub synced_at: i64,
    pub balance: f64,
    pub groups: Vec<UsageGroup>,
}

#[derive(Deserialize, Serialize)]
pub struct ClientConfig {
    pub schema_version: u32,
    pub site_name: String,
    pub api_base_url: String,
    pub login_methods: Vec<String>,
    pub totp_enabled: bool,
    pub registration_enabled: bool,
    pub password_reset_enabled: bool,
    pub register_url: Option<String>,
    pub password_reset_url: Option<String>,
    pub subscriptions_url: Option<String>,
    pub turnstile_enabled: bool,
    #[serde(default)]
    pub turnstile_site_key: String,
    pub tencent_captcha_enabled: bool,
    #[serde(default)]
    pub tencent_captcha_app_id: String,
    pub aliyun_captcha_enabled: bool,
}

impl ClientConfig {
    pub fn validate(mut self, base: &str) -> Result<Self, PlatformAuthError> {
        if self.schema_version != 1 {
            return Err(invalid_response());
        }
        let origin = Url::parse(base)
            .map_err(|_| invalid_response())?
            .origin()
            .ascii_serialization();
        let origin = Url::parse(&format!("{origin}/")).map_err(|_| invalid_response())?;
        for field in [
            &mut self.register_url,
            &mut self.password_reset_url,
            &mut self.subscriptions_url,
        ] {
            *field = field
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .and_then(|value| origin.join(value).ok())
                .filter(|url| {
                    matches!(url.scheme(), "http" | "https")
                        && url.username().is_empty()
                        && url.password().is_none()
                })
                .map(|url| url.to_string());
        }
        if !self.registration_enabled {
            self.register_url = None;
        }
        if !self.password_reset_enabled {
            self.password_reset_url = None;
        }
        Ok(self)
    }
}
