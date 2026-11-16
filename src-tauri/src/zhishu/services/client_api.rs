//! 客户端专用协议 DTO。只接受服务端授权范围，不查询或管理后台 Key。
use super::platform_auth::{error, PlatformAuthError, PlatformUser};
use serde::{Deserialize, Serialize};
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

pub(crate) fn invalid_response() -> PlatformAuthError {
    error(
        "INVALID_RESPONSE",
        "客户端接口响应格式无效，请更新平台后重试",
    )
}

#[derive(Deserialize, Serialize)]
pub struct ClientConfig {
    pub schema_version: u32,
    pub site_name: String,
    pub api_base_url: String,
    pub login_methods: Vec<String>,
    pub subscriptions_url: Option<String>,
    // 旧平台缺少 OAuth2 字段时保持未配置状态，由登录页提示。
    #[serde(default)]
    pub oauth_authorize_url: Option<String>,
    #[serde(default)]
    pub oauth_client_id: String,
    #[serde(default)]
    pub oauth_scopes: Vec<String>,
    /// 回环回调成功后浏览器要返回的平台成功页；旧平台缺省为空，保持原有 HTML 结果页。
    #[serde(default)]
    pub oauth_success_url: Option<String>,
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
        self.subscriptions_url = self
            .subscriptions_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .and_then(|value| origin.join(value).ok())
            .filter(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.username().is_empty()
                    && url.password().is_none()
            })
            .map(|url| url.to_string());
        self.oauth_authorize_url = self
            .oauth_authorize_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .and_then(|value| origin.join(value).ok())
            .filter(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.username().is_empty()
                    && url.password().is_none()
            })
            .map(|url| url.to_string());
        self.oauth_success_url = self
            .oauth_success_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .and_then(|value| origin.join(value).ok())
            .filter(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.username().is_empty()
                    && url.password().is_none()
            })
            .map(|url| url.to_string());
        self.oauth_client_id = self.oauth_client_id.trim().to_string();
        self.oauth_scopes = self
            .oauth_scopes
            .into_iter()
            .map(|scope| scope.trim().to_string())
            .filter(|scope| !scope.is_empty())
            .collect();
        Ok(self)
    }
}
