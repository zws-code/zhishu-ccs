//! 平台登录、二次验证、账户恢复以及带认证的分组与额度请求。
//! access token 和二次验证临时令牌仅驻内存，refresh token 交给系统凭据库保存；
//! 「记住密码」由独立的 platform_login_history 管理，不参与会话恢复。
use super::client_api::{
    self, ClientAccess, ClientAccount, ClientConfig, ClientConnection, ClientUsage, GroupDetails,
    PlatformUsageSummary,
};
use reqwest::{Client, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tauri::{Manager, State};
use tokio::sync::Mutex;
use url::Url;

use crate::zhishu::services::platform_http_log::send_json;
use crate::zhishu::storage::platform_session::{idle_expired, now, SessionRecord, SessionStore};

#[derive(Serialize, Debug)]
pub struct PlatformAuthError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

pub(crate) fn error(code: &'static str, message: &str) -> PlatformAuthError {
    PlatformAuthError {
        code,
        message: message.into(),
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PlatformUser {
    pub(crate) id: i64,
    pub(crate) email: String,
    #[serde(default)]
    pub(crate) username: String,
    #[serde(default)]
    pub(crate) role: String,
    #[serde(default)]
    pub(crate) status: String,
    pub(crate) balance: f64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformAccount {
    pub(crate) base_url: String,
    pub(crate) user: PlatformUser,
}

#[derive(Deserialize)]
struct TokenPair {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct AvailableGroup {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub subscription_type: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct GroupCredential {
    pub key: String,
    pub key_id: i64,
    pub platform: String,
    pub key_name: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct GroupCatalog {
    pub group: AvailableGroup,
    pub credential: GroupCredential,
    pub models: Vec<Value>,
    pub catalog_error: Option<String>,
    #[serde(default)]
    pub connection: Option<ClientConnection>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct GroupDiscovery {
    pub account: PlatformAccount,
    pub groups: Vec<AvailableGroup>,
    pub catalogs: Vec<GroupCatalog>,
    pub failed_group_ids: HashSet<i64>,
    #[serde(default)]
    pub synced_at: i64,
    #[serde(default)]
    pub details: HashMap<i64, GroupDetails>,
    pub subscription_group_ids: HashSet<i64>,
    pub warnings: Vec<String>,
}

struct Session {
    base_url: String,
    user_id: i64,
    tokens: TokenPair,
    persistence: Option<(SessionStore, SessionRecord)>,
    // 令牌已轮换但尚未可靠保存时保持为 true，下次认证请求先重试持久化。
    persistence_dirty: bool,
    last_used_at: i64,
}

struct PendingLogin {
    base_url: String,
    temp_token: String,
    remembered: Option<(String, bool)>,
}

#[derive(Default)]
pub struct PlatformAuthState(Mutex<AuthState>);

#[derive(Default)]
struct AuthState {
    session: Option<Session>,
    pending: Option<PendingLogin>,
    store: Option<SessionStore>,
    restoration_checked: bool,
}

#[derive(Deserialize)]
struct Envelope<T> {
    code: i64,
    data: Option<T>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginResult {
    Authenticated {
        account: PlatformAccount,
        #[serde(rename = "persistenceWarning", skip_serializing_if = "Option::is_none")]
        persistence_warning: Option<String>,
    },
    #[serde(rename = "requires_2fa")]
    Requires2fa,
}

fn normalize_base_url(input: &str) -> Result<String, PlatformAuthError> {
    let mut url =
        Url::parse(input.trim()).map_err(|_| error("INVALID_URL", "请输入有效的平台地址"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(error("INVALID_URL", "请输入有效的平台地址"));
    }
    // 接受完整地址，但账号、查询参数和片段不属于平台 API 基址，也不应进入会话记录。
    url.set_username("")
        .map_err(|_| error("INVALID_URL", "请输入有效的平台地址"))?;
    url.set_password(None)
        .map_err(|_| error("INVALID_URL", "请输入有效的平台地址"))?;
    url.set_query(None);
    url.set_fragment(None);
    let path = url.path().trim_end_matches('/');
    let path = path
        .strip_suffix(client_api::PREFIX)
        .unwrap_or(path)
        .to_string();
    url.set_path(&path);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

pub(crate) fn client() -> Result<Client, PlatformAuthError> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        // Never forward credentials to a redirected origin.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| error("NETWORK", "无法初始化平台连接"))
}

async fn request<T: DeserializeOwned>(
    client: &Client,
    base: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> Result<T, PlatformAuthError> {
    // Closed allowlist: new code cannot accidentally resurrect panel/model endpoints.
    let allowed = matches!(
        (method.as_str(), path),
        ("GET", "/config" | "/account" | "/usage/summary")
            | (
                "POST",
                "/auth/login" | "/auth/login/2fa" | "/auth/refresh" | "/auth/logout" | "/access"
            )
    );
    if !allowed {
        return Err(error("CLIENT_API_ONLY", "仅允许客户端专用接口"));
    }
    let mut builder = client.request(method, format!("{base}{}{path}", client_api::PREFIX));
    if let Some(body) = body {
        builder = builder.json(&body);
    }
    if let Some(token) = token {
        builder = builder.bearer_auth(token);
    }
    let response = send_json(builder)
        .await
        .map_err(|_| error("NETWORK", "无法连接平台，请检查地址、网络或证书后重试"))?;
    let status = response.status;
    // 平台错误只返回受控文案，禁止回显可能包含凭据的服务端原文或 reqwest 异常。
    if !status.is_success() {
        let (code, message) = match status {
            StatusCode::UNAUTHORIZED => ("UNAUTHORIZED", "登录凭证无效或已过期，请重新登录"),
            StatusCode::FORBIDDEN => ("FORBIDDEN", "账户暂不可用或平台禁止本次登录"),
            StatusCode::TOO_MANY_REQUESTS => ("RATE_LIMITED", "操作过于频繁，请稍后重试"),
            StatusCode::BAD_REQUEST => ("BAD_REQUEST", "登录信息或验证码无效，请检查后重试"),
            _ => ("SERVER", "平台请求失败，请稍后重试"),
        };
        return Err(error(code, message));
    }
    let envelope: Envelope<T> = response
        .body
        .and_then(|body| serde_json::from_value(body).map_err(|_| ()))
        .map_err(|_| {
            error(
                "INVALID_RESPONSE",
                "平台响应格式无效，请确认填写的是后端地址",
            )
        })?;
    if envelope.code != 0 {
        return Err(error("SERVER", "平台未能完成请求，请检查登录信息后重试"));
    }
    envelope
        .data
        .ok_or_else(|| error("INVALID_RESPONSE", "平台响应缺少数据"))
}

async fn finish_login(
    state: &mut AuthState,
    client: &Client,
    base_url: String,
    data: Value,
) -> Result<LoginResult, PlatformAuthError> {
    let tokens: TokenPair =
        serde_json::from_value(data).map_err(|_| error("INVALID_RESPONSE", "登录响应缺少凭证"))?;
    if tokens.access_token.is_empty() {
        return Err(error("INVALID_RESPONSE", "登录响应缺少凭证"));
    }
    let user: ClientAccount = request(
        client,
        &base_url,
        Method::GET,
        "/account",
        None,
        Some(&tokens.access_token),
    )
    .await?;
    let account = PlatformAccount {
        base_url: base_url.clone(),
        user: user.into_user()?,
    };
    // 发布新会话前先失效旧账户；新凭据持久化失败时保留本次登录，并返回明确警告。
    let (persistence, persistence_warning) = if let Some(store) = &state.store {
        store.clear().await?;
        if tokens.refresh_token.is_empty() {
            (
                None,
                Some("平台未提供刷新令牌，本次登录无法自动续期".into()),
            )
        } else {
            match store
                .create(
                    base_url.clone(),
                    account.user.id,
                    tokens.refresh_token.clone(),
                )
                .await
            {
                Ok(record) => (Some((store.clone(), record)), None),
                Err(err) => (None, Some(err.message)),
            }
        }
    } else {
        (
            None,
            tokens
                .refresh_token
                .is_empty()
                .then(|| "平台未提供刷新令牌，本次登录无法自动续期".into()),
        )
    };
    let session = Session {
        persistence,
        persistence_dirty: false,
        last_used_at: now(),
        base_url,
        user_id: account.user.id,
        tokens,
    };
    state.session = Some(session);
    state.restoration_checked = true;
    state.pending = None;
    Ok(LoginResult::Authenticated {
        account,
        persistence_warning,
    })
}

async fn initialize_session_store(
    state: &Mutex<AuthState>,
    handle: &tauri::AppHandle,
) -> Result<(), PlatformAuthError> {
    let mut auth = state.lock().await;
    if auth.store.is_none() {
        auth.store = Some(SessionStore::for_app(handle)?);
    }
    Ok(())
}

#[tauri::command]
pub async fn platform_login(
    handle: tauri::AppHandle,
    state: State<'_, PlatformAuthState>,
    base_url: String,
    email: String,
    password: String,
    remember_password: Option<bool>,
) -> Result<LoginResult, PlatformAuthError> {
    let _operation = crate::zhishu::services::platform_models::APPLY_LOCK
        .lock()
        .await;
    let store_result = initialize_session_store(&state.0, &handle).await;
    let remember = remember_password.unwrap_or(false);
    let mut result = login(&state.0, base_url, email, password.clone(), Some(remember)).await?;
    if let Err(err) = store_result {
        if let LoginResult::Authenticated {
            persistence_warning,
            ..
        } = &mut result
        {
            *persistence_warning = Some(err.message);
        }
    }
    persist_login(handle, &mut result, password, remember).await;
    Ok(result)
}

async fn persist_login(
    handle: tauri::AppHandle,
    result: &mut LoginResult,
    password: String,
    remember: bool,
) {
    if let LoginResult::Authenticated {
        account,
        persistence_warning,
    } = result
    {
        if let Err(err) = crate::zhishu::storage::platform_login_history::remember_login(
            handle,
            account.base_url.clone(),
            account.user.email.clone(),
            password,
            remember,
        )
        .await
        {
            *persistence_warning = Some(match persistence_warning.take() {
                Some(previous) => format!("{previous}；{}", err.message),
                None => err.message,
            });
        }
    }
}

async fn login(
    state: &Mutex<AuthState>,
    base_url: String,
    email: String,
    password: String,
    remember_password: Option<bool>,
) -> Result<LoginResult, PlatformAuthError> {
    let base = normalize_base_url(&base_url)?;
    if email.trim().is_empty() || password.is_empty() {
        return Err(error("BAD_REQUEST", "请输入邮箱和密码"));
    }
    let client = client()?;
    let mut state = state.lock().await;
    state.pending = None;
    let settings = client_config(&client, &base).await?;
    if !settings
        .login_methods
        .iter()
        .any(|method| method == "password")
    {
        return Err(error("LOGIN_UNSUPPORTED", "平台未启用邮箱密码登录"));
    }
    if settings.turnstile_enabled
        || settings.tencent_captcha_enabled
        || settings.aliyun_captcha_enabled
    {
        return Err(error(
            "CAPTCHA_REQUIRED",
            "平台已启用人机验证，当前客户端暂不支持此登录方式。",
        ));
    }
    let data: Value = request(
        &client,
        &base,
        Method::POST,
        "/auth/login",
        Some(json!({ "email": email.trim(), "password": password })),
        None,
    )
    .await?;
    if data.get("requires_2fa").and_then(Value::as_bool) == Some(true) {
        let temp_token = data
            .get("temp_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| error("INVALID_RESPONSE", "平台未返回二次验证会话"))?;
        state.pending = Some(PendingLogin {
            base_url: base,
            temp_token: temp_token.into(),
            remembered: remember_password
                .map(|remember| (if remember { password } else { String::new() }, remember)),
        });
        return Ok(LoginResult::Requires2fa);
    }
    finish_login(&mut state, &client, base, data).await
}

#[tauri::command]
pub async fn platform_login_2fa(
    handle: tauri::AppHandle,
    state: State<'_, PlatformAuthState>,
    code: String,
) -> Result<LoginResult, PlatformAuthError> {
    let _operation = crate::zhishu::services::platform_models::APPLY_LOCK
        .lock()
        .await;
    let (mut result, remembered) = login_2fa_with_preferences(&state.0, code).await?;
    if let Some((password, remember)) = remembered {
        persist_login(handle, &mut result, password, remember).await;
    }
    Ok(result)
}

#[cfg(test)]
async fn login_2fa(
    state: &Mutex<AuthState>,
    code: String,
) -> Result<LoginResult, PlatformAuthError> {
    login_2fa_with_preferences(state, code)
        .await
        .map(|(result, _)| result)
}

async fn login_2fa_with_preferences(
    state: &Mutex<AuthState>,
    code: String,
) -> Result<(LoginResult, Option<(String, bool)>), PlatformAuthError> {
    if code.len() != 6 || !code.bytes().all(|c| c.is_ascii_digit()) {
        return Err(error("BAD_REQUEST", "请输入六位数字验证码"));
    }
    let mut state = state.lock().await;
    let pending = state
        .pending
        .as_ref()
        .ok_or_else(|| error("UNAUTHORIZED", "二次验证会话已失效，请返回重新登录"))?;
    let base = pending.base_url.clone();
    let remembered = pending.remembered.clone();
    let client = client()?;
    let data = request(
        &client,
        &base,
        Method::POST,
        "/auth/login/2fa",
        Some(json!({"temp_token": pending.temp_token, "totp_code": code})),
        None,
    )
    .await?;
    let result = finish_login(&mut state, &client, base, data).await?;
    Ok((result, remembered))
}

#[tauri::command]
pub async fn platform_cancel_login(
    state: State<'_, PlatformAuthState>,
) -> Result<(), PlatformAuthError> {
    state.0.lock().await.pending = None;
    Ok(())
}

#[tauri::command]
pub async fn platform_get_account(
    handle: tauri::AppHandle,
    state: State<'_, PlatformAuthState>,
    active: Option<bool>,
) -> Result<Option<PlatformAccount>, PlatformAuthError> {
    initialize_session_store(&state.0, &handle).await?;
    let active = active.unwrap_or(false)
        && handle.get_webview_window("main").is_some_and(|window| {
            window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(true)
        });
    get_account_with_activity(&state.0, active).await
}

/// 只读额度快照；不保存目录，不申请 Key，不请求模型接口。
#[tauri::command]
pub async fn platform_get_subscription_usage(
    state: State<'_, PlatformAuthState>,
) -> Result<PlatformUsageSummary, PlatformAuthError> {
    subscription_usage(&state.0).await
}

async fn subscription_usage(
    state: &Mutex<AuthState>,
) -> Result<PlatformUsageSummary, PlatformAuthError> {
    let mut auth = state.lock().await;
    let session = auth
        .session
        .as_mut()
        .filter(|s| !idle_expired(s.last_used_at, now()))
        .ok_or_else(|| error("UNAUTHORIZED", "请先登录平台"))?;
    let data: ClientUsage = match get_authenticated(&client()?, session, "/usage/summary").await {
        Ok(data) => data,
        Err(err) if matches!(err.code, "UNAUTHORIZED" | "FORBIDDEN") => {
            invalidate_session(&mut auth).await;
            return Err(err);
        }
        Err(err) => return Err(err),
    };
    let synced_at = client_api::snapshot_time(data.schema_version, &data.synced_at)?;
    if data.account.id != session.user_id || data.account.status != "active" {
        invalidate_session(&mut auth).await;
        return Err(error("UNAUTHORIZED", "登录状态已改变，请重新登录"));
    }
    let mut group_ids = HashSet::new();
    if data.groups.iter().any(|group| {
        group.id <= 0
            || !group_ids.insert(group.id)
            || group
                .subscriptions
                .iter()
                .any(|sub| sub.group_id != group.id)
    }) {
        return Err(client_api::invalid_response());
    }
    Ok(PlatformUsageSummary {
        base_url: session.base_url.clone(),
        user_id: session.user_id,
        synced_at,
        balance: data.account.balance_usd,
        groups: data.groups,
    })
}

async fn client_config(client: &Client, base: &str) -> Result<ClientConfig, PlatformAuthError> {
    request::<ClientConfig>(client, base, Method::GET, "/config", None, None)
        .await?
        .validate(base)
}

#[tauri::command]
pub async fn platform_get_client_config(
    base_url: String,
) -> Result<ClientConfig, PlatformAuthError> {
    client_config(&client()?, &normalize_base_url(&base_url)?).await
}

async fn refresh_session_tokens(
    client: &Client,
    session: &mut Session,
) -> Result<(), PlatformAuthError> {
    if session.tokens.refresh_token.is_empty() {
        return Err(error("UNAUTHORIZED", "登录凭证无效或已过期，请重新登录"));
    }
    let refreshed: TokenPair = request(
        client,
        &session.base_url,
        Method::POST,
        "/auth/refresh",
        Some(json!({ "refresh_token": session.tokens.refresh_token })),
        None,
    )
    .await
    .map_err(|err| {
        if matches!(err.code, "UNAUTHORIZED" | "FORBIDDEN" | "BAD_REQUEST") {
            error("UNAUTHORIZED", "登录凭证无效或已过期，请重新登录")
        } else {
            err
        }
    })?;
    if refreshed.access_token.is_empty() || refreshed.refresh_token.is_empty() {
        return Err(error("INVALID_RESPONSE", "平台刷新凭证无效"));
    }
    // 先替换内存令牌并保存轮换结果，再重试请求；后续断网不能使已消费的旧令牌重新生效。
    session.tokens = refreshed;
    session.persistence_dirty = true;
    persist_rotated_tokens(session).await
}

async fn persist_rotated_tokens(session: &mut Session) -> Result<(), PlatformAuthError> {
    if session.persistence_dirty {
        if let Some((store, record)) = &session.persistence {
            store
                .save_token(record, session.tokens.refresh_token.clone())
                .await?;
        }
        session.persistence_dirty = false;
    }
    Ok(())
}

async fn get_authenticated<T: DeserializeOwned>(
    client: &Client,
    session: &mut Session,
    path: &str,
) -> Result<T, PlatformAuthError> {
    request_authenticated(client, session, Method::GET, path).await
}

async fn request_authenticated<T: DeserializeOwned>(
    client: &Client,
    session: &mut Session,
    method: Method,
    path: &str,
) -> Result<T, PlatformAuthError> {
    persist_rotated_tokens(session).await?;
    if session.tokens.access_token.is_empty() {
        refresh_session_tokens(client, session).await?;
    }
    let base = session.base_url.clone();
    let token = session.tokens.access_token.clone();
    match request(client, &base, method.clone(), path, None, Some(&token)).await {
        Err(err) if err.code == "UNAUTHORIZED" => {
            refresh_session_tokens(client, session).await?;
            request(
                client,
                &base,
                method,
                path,
                None,
                Some(&session.tokens.access_token),
            )
            .await
        }
        result => result,
    }
}

async fn invalidate_session(state: &mut AuthState) {
    state.session = None;
    state.restoration_checked = true;
    if let Some(store) = &state.store {
        if let Err(err) = store.clear().await {
            log::warn!("Unable to clear persisted platform session: {}", err.code);
        }
    }
}

#[cfg(test)]
async fn get_account(
    state: &Mutex<AuthState>,
) -> Result<Option<PlatformAccount>, PlatformAuthError> {
    get_account_with_activity(state, false).await
}

async fn get_account_with_activity(
    state: &Mutex<AuthState>,
    active: bool,
) -> Result<Option<PlatformAccount>, PlatformAuthError> {
    let mut state = state.lock().await;
    if !state.restoration_checked && state.session.is_none() {
        if let Some(store) = &state.store {
            if let Some((record, refresh_token)) = store.load().await? {
                let base_url = normalize_base_url(&record.base_url)?;
                state.session = Some(Session {
                    base_url,
                    user_id: record.user_id,
                    last_used_at: record.last_used_at,
                    tokens: TokenPair {
                        access_token: String::new(),
                        refresh_token,
                    },
                    persistence: Some((store.clone(), record)),
                    persistence_dirty: false,
                });
            }
        }
        state.restoration_checked = true;
    }
    let Some(session) = state.session.as_mut() else {
        return Ok(None);
    };
    if idle_expired(session.last_used_at, now()) {
        invalidate_session(&mut state).await;
        return Ok(None);
    }
    // 可见窗口中的实际使用才续期，即使暂时断网也记录；后台检查不延长七天闲置期限。
    if active {
        if let Some((store, record)) = &mut session.persistence {
            store.touch(record).await?;
        }
        session.last_used_at = now();
    }
    let client = client()?;
    let user = match get_authenticated::<ClientAccount>(&client, session, "/account").await {
        Ok(user) => user,
        Err(err) if matches!(err.code, "UNAUTHORIZED" | "FORBIDDEN") => {
            invalidate_session(&mut state).await;
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    if user.id != session.user_id || user.status != "active" {
        invalidate_session(&mut state).await;
        return Ok(None);
    }
    Ok(Some(PlatformAccount {
        base_url: session.base_url.clone(),
        user: user.into_user()?,
    }))
}

#[tauri::command]
pub async fn platform_logout(state: State<'_, PlatformAuthState>) -> Result<(), PlatformAuthError> {
    state.logout().await
}

async fn logout(state: &Mutex<AuthState>) -> Result<(), PlatformAuthError> {
    let mut state = state.lock().await;
    state.pending = None;
    state.restoration_checked = true;
    let cleared = if let Some(store) = &state.store {
        store.clear().await
    } else {
        Ok(())
    };
    // 先清本机会话再尽力撤销服务端令牌，网络错误不能恢复已经退出的本地账户。
    let session = state.session.take();
    if let Some(session) = session {
        if !session.tokens.refresh_token.is_empty() {
            let _: Value = request(
                &client()?,
                &session.base_url,
                Method::POST,
                "/auth/logout",
                Some(json!({"refresh_token": session.tokens.refresh_token})),
                None,
            )
            .await?;
        }
    }
    cleared
}

impl PlatformAuthState {
    pub(crate) async fn logout(&self) -> Result<(), PlatformAuthError> {
        let _operation = crate::zhishu::services::platform_models::APPLY_LOCK
            .lock()
            .await;
        // 退出只清理登录会话；保留本机目录、供应商和工具配置中的 API Key。
        logout(&self.0).await
    }

    #[cfg(test)]
    pub(crate) fn for_test(base_url: String, user_id: i64) -> Self {
        Self(Mutex::new(AuthState {
            session: Some(Session {
                base_url,
                user_id,
                tokens: TokenPair {
                    access_token: "test-access-token".into(),
                    refresh_token: String::new(),
                },
                persistence: None,
                persistence_dirty: false,
                last_used_at: now(),
            }),
            ..AuthState::default()
        }))
    }

    /// 持有会话锁校验本机身份；不刷新令牌、不请求 /account，使已登录用户断网时仍能本地同步。
    pub(crate) async fn with_local_identity<T>(
        &self,
        action: impl FnOnce(&str, i64) -> Result<T, PlatformAuthError>,
    ) -> Result<T, PlatformAuthError> {
        let state = self.0.lock().await;
        Self::with_local_identity_locked(&state, action)
    }

    /// 同步配置适配器使用的身份校验，只能在阻塞线程调用，避免阻塞异步运行时。
    pub(crate) fn with_local_identity_blocking<T>(
        &self,
        action: impl FnOnce(&str, i64) -> Result<T, PlatformAuthError>,
    ) -> Result<T, PlatformAuthError> {
        let state = self.0.blocking_lock();
        Self::with_local_identity_locked(&state, action)
    }

    fn with_local_identity_locked<T>(
        state: &AuthState,
        action: impl FnOnce(&str, i64) -> Result<T, PlatformAuthError>,
    ) -> Result<T, PlatformAuthError> {
        let session = state
            .session
            .as_ref()
            .filter(|session| !idle_expired(session.last_used_at, now()))
            .ok_or_else(|| error("UNAUTHORIZED", "请先登录平台"))?;
        action(&session.base_url, session.user_id)
    }

    /// 唯一凭证入口。会话锁覆盖请求和身份验证，APPLY_LOCK 覆盖保存阶段。
    pub(crate) async fn discover_group_catalogs(
        &self,
    ) -> Result<GroupDiscovery, PlatformAuthError> {
        let mut auth = self.0.lock().await;
        let session = auth
            .session
            .as_mut()
            .filter(|s| !idle_expired(s.last_used_at, now()))
            .ok_or_else(|| error("UNAUTHORIZED", "请先登录平台"))?;
        let data: ClientAccess =
            match request_authenticated(&client()?, session, Method::POST, "/access").await {
                Ok(data) => data,
                Err(err) if matches!(err.code, "UNAUTHORIZED" | "FORBIDDEN") => {
                    invalidate_session(&mut auth).await;
                    return Err(err);
                }
                Err(err) => return Err(err),
            };
        let result = data.into_discovery(
            &session.base_url,
            session.user_id,
            &[
                session.tokens.access_token.clone(),
                session.tokens.refresh_token.clone(),
            ],
        );
        if result.as_ref().is_err_and(|err| err.code == "UNAUTHORIZED") {
            invalidate_session(&mut auth).await;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zhishu::storage::platform_session::IDLE_SECONDS;

    #[test]
    fn normalizes_platform_urls_and_api_suffix() {
        assert_eq!(
            normalize_base_url(" http://127.0.0.1:3000/ ").unwrap(),
            "http://127.0.0.1:3000"
        );
        assert_eq!(
            normalize_base_url("https://example.com/sub/api/zhishu/v1/client/").unwrap(),
            "https://example.com/sub"
        );
        assert_eq!(
            normalize_base_url("http://[::1]:3000").unwrap(),
            "http://[::1]:3000"
        );
    }

    #[test]
    fn allows_remote_http_without_release_configuration() {
        for url in ["http://example.com", "http://192.168.10.20:3000"] {
            assert_eq!(normalize_base_url(url).unwrap(), url);
        }
    }

    #[test]
    fn normalizes_urls_with_credentials_query_or_fragment() {
        for url in [
            "https://user:pass@example.com",
            "https://user@example.com",
            "https://:pass@example.com",
            "https://example.com/?token=secret",
            "https://example.com/#login",
            "https://user:pass@example.com/api/zhishu/v1/client/?token=secret#login",
        ] {
            assert_eq!(normalize_base_url(url).unwrap(), "https://example.com");
        }
        assert_eq!(
            normalize_base_url(
                "http://user:pass@example.com/sub/api/zhishu/v1/client/?token=secret#login"
            )
            .unwrap(),
            "http://example.com/sub"
        );
    }

    #[test]
    fn rejects_invalid_platform_addresses() {
        for url in [
            "",
            "not a url",
            "http://",
            "file:///tmp/auth",
            "ftp://example.com",
            "localhost:3000",
        ] {
            assert_eq!(normalize_base_url(url).unwrap_err().code, "INVALID_URL");
        }
    }

    #[test]
    fn public_account_never_serializes_tokens() {
        let account = PlatformAccount {
            base_url: "https://example.com".into(),
            user: PlatformUser {
                id: 1,
                email: "user@example.com".into(),
                username: "user".into(),
                role: "user".into(),
                status: "active".into(),
                balance: 1.5,
            },
        };
        let serialized = serde_json::to_value(account).unwrap();
        assert_eq!(serialized.as_object().unwrap().len(), 2);
        assert!(serialized.get("access_token").is_none());
        assert!(serialized.get("refresh_token").is_none());
    }

    // A real local HTTP fixture exercises wire format without a live platform account.
    fn mock_server(
        replies: Vec<(&'static str, u16, Value)>,
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            for (expected, status, body) in replies {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(e) => panic!("missing request {expected}: {e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buf = [0; 4096];
                loop {
                    let count = stream.read(&mut buf).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buf[..count]);
                    let text = String::from_utf8_lossy(&request);
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let length: usize = head
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(str::trim)
                                    .map(str::to_owned)
                            })
                            .map(|n| n.parse().unwrap())
                            .unwrap_or(0);
                        if body.len() >= length {
                            break;
                        }
                    }
                }
                let request = String::from_utf8(request).unwrap();
                let (expected, auth) = expected.split_once('|').unwrap_or((expected, ""));
                assert!(request.starts_with(expected), "unexpected request path");
                if !auth.is_empty() {
                    assert!(request
                        .to_ascii_lowercase()
                        .contains(&format!("authorization: bearer {auth}")));
                }
                if expected.starts_with("POST /api/zhishu/v1/client/access ") {
                    let sent = request.split_once("\r\n\r\n").unwrap().1.trim();
                    assert!(
                        sent.is_empty() || sent == "{}",
                        "access must not specify users, groups or key settings"
                    );
                }
                let body = body.to_string();
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        (base, handle)
    }

    fn config_response() -> Value {
        json!({"code":0,"data":{
            "schema_version":1,"site_name":"Test","api_base_url":"https://gateway.test",
            "login_methods":["password"],"totp_enabled":true,"registration_enabled":true,
            "password_reset_enabled":true,"register_url":"/register","password_reset_url":"/forgot-password",
            "subscriptions_url":"/subscriptions","turnstile_enabled":false,"tencent_captcha_enabled":false,"aliyun_captcha_enabled":false
        }})
    }

    fn profile() -> Value {
        json!({"code":0,"data":{"id":1,"email":"test@example.com","username":"test","status":"active","balance_usd":2.0}})
    }

    #[tokio::test]
    async fn local_identity_never_fetches_account_or_refreshes_tokens() {
        // No server is listening. Local operations still work even if the next
        // authenticated HTTP request would require a token refresh.
        let state = model_session("http://127.0.0.1:0".into());
        state
            .0
            .lock()
            .await
            .session
            .as_mut()
            .unwrap()
            .tokens
            .access_token
            .clear();
        let identity = state
            .with_local_identity(|origin, user_id| Ok((origin.to_string(), user_id)))
            .await
            .unwrap();
        assert_eq!(identity, ("http://127.0.0.1:0".into(), 1));
        state.0.lock().await.session.as_mut().unwrap().last_used_at = now() - IDLE_SECONDS - 1;
        assert!(
            matches!(state.with_local_identity(|_, _| Ok(())).await, Err(err) if err.code == "UNAUTHORIZED")
        );
        assert!(
            matches!(PlatformAuthState::default().with_local_identity(|_, _| Ok(())).await, Err(err) if err.code == "UNAUTHORIZED")
        );
    }

    #[tokio::test]
    async fn blocking_local_identity_preserves_validation_without_async_poll_budget() {
        let state = model_session("http://127.0.0.1:0".into());
        state
            .0
            .lock()
            .await
            .session
            .as_mut()
            .unwrap()
            .tokens
            .access_token
            .clear();
        tauri::async_runtime::spawn_blocking(move || {
            let locks = crate::proxy::switch_lock::SwitchLockManager::new();
            state.with_local_identity_blocking(|origin, user_id| {
                assert_eq!(origin, "http://127.0.0.1:0");
                assert_eq!(user_id, 1);
                for _ in 0..256 {
                    let _guard = futures::executor::block_on(locks.lock_for_app("workbuddy"));
                }
                Ok(())
            }).unwrap();
            state.0.blocking_lock().session.as_mut().unwrap().last_used_at = now() - IDLE_SECONDS - 1;
            assert!(matches!(state.with_local_identity_blocking(|_, _| Ok(())), Err(err) if err.code == "UNAUTHORIZED"));
            assert!(matches!(PlatformAuthState::default().with_local_identity_blocking(|_, _| Ok(())), Err(err) if err.code == "UNAUTHORIZED"));
        }).await.unwrap();
    }

    fn model_session(base: String) -> PlatformAuthState {
        PlatformAuthState(Mutex::new(AuthState {
            session: Some(Session {
                base_url: base,
                user_id: 1,
                persistence: None,
                persistence_dirty: false,
                last_used_at: now(),
                tokens: TokenPair {
                    access_token: "access-secret".into(),
                    refresh_token: "refresh-secret".into(),
                },
            }),
            pending: None,
            ..AuthState::default()
        }))
    }

    #[tokio::test]
    async fn login_uses_and_persists_only_the_normalized_platform_base_url() {
        let (base, server) = mock_server(vec![
            (
                "GET /sub/api/zhishu/v1/client/config ",
                200,
                config_response(),
            ),
            (
                "POST /sub/api/zhishu/v1/client/auth/login ",
                200,
                json!({"code":0,"data":{"access_token":"access-secret","refresh_token":"refresh-secret"}}),
            ),
            (
                "GET /sub/api/zhishu/v1/client/account |access-secret",
                200,
                profile(),
            ),
        ]);
        let expected_base = format!("{base}/sub");
        let (_dir, store, _vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&expected_base, now())
                .await;
        store.clear().await.unwrap();
        let state = Mutex::new(AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        });
        let mut input = Url::parse(&format!(
            "{base}/sub/api/zhishu/v1/client/?token=url-secret#login"
        ))
        .unwrap();
        input.set_username("url-user").unwrap();
        input.set_password(Some("url-password")).unwrap();
        let result = login(
            &state,
            input.to_string(),
            "test@example.com".into(),
            "password".into(),
            None,
        )
        .await
        .unwrap();
        let LoginResult::Authenticated { account, .. } = result else {
            panic!("expected authenticated login");
        };
        assert_eq!(account.base_url, expected_base);
        assert_eq!(
            store.load().await.unwrap().unwrap().0.base_url,
            expected_base
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn login_keeps_pending_2fa_out_of_the_renderer() {
        let (base, server) = mock_server(vec![
            ("GET /api/zhishu/v1/client/config ", 200, config_response()),
            (
                "POST /api/zhishu/v1/client/auth/login ",
                200,
                json!({"code":0,"data":{"requires_2fa":true,"temp_token":"temporary-secret"}}),
            ),
            (
                "POST /api/zhishu/v1/client/auth/login/2fa ",
                200,
                json!({"code":0,"data":{"access_token":"access-secret","refresh_token":"refresh-secret"}}),
            ),
            ("GET /api/zhishu/v1/client/account ", 200, profile()),
        ]);
        let (_dir, store, _vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, now()).await;
        store.clear().await.unwrap();
        let state = Mutex::new(AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        });
        let result = login(
            &state,
            base,
            "test@example.com".into(),
            "password".into(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            json!({"status":"requires_2fa"})
        );
        assert!(state.lock().await.session.is_none());
        assert!(store.load().await.unwrap().is_none());
        let result = login_2fa(&state, "123456".into()).await.unwrap();
        let public = serde_json::to_string(&result).unwrap();
        assert!(public.contains("test@example.com"));
        assert!(!public.contains("secret"));
        assert!(state.lock().await.pending.is_none());
        assert_eq!(store.load().await.unwrap().unwrap().1, "refresh-secret");
        server.join().unwrap();
    }

    #[tokio::test]
    async fn captcha_blocks_password_submission() {
        for key in [
            "turnstile_enabled",
            "tencent_captcha_enabled",
            "aliyun_captcha_enabled",
        ] {
            let (base, server) = mock_server(vec![("GET /api/zhishu/v1/client/config ", 200, {
                let mut value = config_response();
                value["data"][key] = json!(true);
                value
            })]);
            let state = Mutex::new(AuthState::default());
            let result = login(
                &state,
                base,
                "test@example.com".into(),
                "password".into(),
                None,
            )
            .await;
            assert!(matches!(result, Err(err) if err.code == "CAPTCHA_REQUIRED"));
            assert!(state.lock().await.session.is_none());
            server.join().unwrap();
        }
    }

    #[tokio::test]
    async fn keeps_rotated_refresh_token_when_profile_request_fails() {
        let (base, server) = mock_server(vec![
            (
                "GET /api/zhishu/v1/client/account ",
                401,
                json!({"message":"secret"}),
            ),
            (
                "POST /api/zhishu/v1/client/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"new-access","refresh_token":"new-refresh"}}),
            ),
            ("GET /api/zhishu/v1/client/account ", 503, json!({})),
            ("GET /api/zhishu/v1/client/account ", 200, profile()),
        ]);
        let state = Mutex::new(AuthState {
            session: Some(Session {
                base_url: base,
                user_id: 1,
                persistence: None,
                persistence_dirty: false,
                last_used_at: now(),
                tokens: TokenPair {
                    access_token: "old-access".into(),
                    refresh_token: "old-refresh".into(),
                },
            }),
            pending: None,
            ..AuthState::default()
        });
        assert!(get_account(&state).await.is_err());
        assert_eq!(
            state
                .lock()
                .await
                .session
                .as_ref()
                .unwrap()
                .tokens
                .refresh_token,
            "new-refresh"
        );
        assert!(get_account(&state).await.unwrap().is_some());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn invalid_refresh_returns_to_logged_out_state() {
        let (base, server) = mock_server(vec![
            ("GET /api/zhishu/v1/client/account ", 401, json!({})),
            ("POST /api/zhishu/v1/client/auth/refresh ", 401, json!({})),
        ]);
        let state = Mutex::new(AuthState {
            session: Some(Session {
                base_url: base,
                user_id: 1,
                persistence: None,
                persistence_dirty: false,
                last_used_at: now(),
                tokens: TokenPair {
                    access_token: "access".into(),
                    refresh_token: "expired".into(),
                },
            }),
            pending: None,
            ..AuthState::default()
        });
        assert!(get_account(&state).await.unwrap().is_none());
        assert!(state.lock().await.session.is_none());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn logout_clears_local_tokens_even_if_revocation_fails() {
        let (base, server) = mock_server(vec![(
            "POST /api/zhishu/v1/client/auth/logout ",
            503,
            json!({"message":"secret"}),
        )]);
        let state = Mutex::new(AuthState {
            session: Some(Session {
                base_url: base,
                user_id: 1,
                persistence: None,
                persistence_dirty: false,
                last_used_at: now(),
                tokens: TokenPair {
                    access_token: "access".into(),
                    refresh_token: "refresh".into(),
                },
            }),
            pending: None,
            ..AuthState::default()
        });
        let err = logout(&state).await.unwrap_err();
        assert!(!err.message.contains("secret"));
        assert!(state.lock().await.session.is_none());
        server.join().unwrap();
    }
    #[tokio::test]
    async fn saved_refresh_restores_after_each_process_restart_and_rotates_in_the_vault() {
        let (base, server) = mock_server(vec![
            (
                "POST /api/zhishu/v1/client/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"access-one","refresh_token":"refresh-one"}}),
            ),
            (
                "GET /api/zhishu/v1/client/account |access-one",
                200,
                profile(),
            ),
            (
                "POST /api/zhishu/v1/client/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"access-two","refresh_token":"refresh-two"}}),
            ),
            (
                "GET /api/zhishu/v1/client/account |access-two",
                200,
                profile(),
            ),
        ]);
        let (_dir, store, _vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, now() - 100).await;
        for token in ["refresh-one", "refresh-two"] {
            let state = Mutex::new(AuthState {
                store: Some(store.clone()),
                ..AuthState::default()
            });
            let account = get_account_with_activity(&state, true)
                .await
                .unwrap()
                .unwrap();
            let public = serde_json::to_string(&account).unwrap();
            assert!(!public.contains("refresh"));
            assert!(!public.contains("access-one"));
            let (record, saved_token) = store.load().await.unwrap().unwrap();
            assert_eq!(saved_token, token);
            assert!(record.last_used_at >= now() - 2);
        }
        server.join().unwrap();
    }

    #[tokio::test]
    async fn restore_retries_transient_failures_without_losing_rotated_tokens() {
        let (base, server) = mock_server(vec![
            ("POST /api/zhishu/v1/client/auth/refresh ", 503, json!({})),
            (
                "POST /api/zhishu/v1/client/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"new-access","refresh_token":"new-refresh"}}),
            ),
            ("GET /api/zhishu/v1/client/account ", 503, json!({})),
            ("GET /api/zhishu/v1/client/account ", 200, profile()),
        ]);
        let (_dir, store, _vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, now()).await;
        let state = Mutex::new(AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        });
        assert!(get_account(&state).await.is_err());
        assert_eq!(store.load().await.unwrap().unwrap().1, "saved-refresh");
        assert!(get_account(&state).await.is_err());
        assert_eq!(store.load().await.unwrap().unwrap().1, "new-refresh");
        assert!(get_account(&state).await.unwrap().is_some());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn background_account_checks_do_not_extend_inactivity_but_visible_use_does() {
        let (base, server) = mock_server(vec![
            (
                "POST /api/zhishu/v1/client/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"access","refresh_token":"refresh"}}),
            ),
            ("GET /api/zhishu/v1/client/account ", 200, profile()),
            ("GET /api/zhishu/v1/client/account ", 200, profile()),
        ]);
        let previous = now() - 3600;
        let (_dir, store, _vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, previous).await;
        let state = Mutex::new(AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        });
        assert!(get_account(&state).await.unwrap().is_some());
        assert_eq!(
            store.load().await.unwrap().unwrap().0.last_used_at,
            previous
        );
        assert!(get_account_with_activity(&state, true)
            .await
            .unwrap()
            .is_some());
        assert!(store.load().await.unwrap().unwrap().0.last_used_at > previous);
        server.join().unwrap();
    }

    #[tokio::test]
    async fn expired_saved_session_never_calls_the_server_even_when_opened_again() {
        let (_dir, store, _vault) = crate::zhishu::storage::platform_session::tests::saved_store(
            "https://example.com",
            now() - IDLE_SECONDS - 1,
        )
        .await;
        let state = Mutex::new(AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        });
        assert!(get_account_with_activity(&state, true)
            .await
            .unwrap()
            .is_none());
        assert!(store.load().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn revoked_saved_session_is_not_restored_again() {
        let (base, server) = mock_server(vec![(
            "POST /api/zhishu/v1/client/auth/refresh ",
            401,
            json!({}),
        )]);
        let (_dir, store, _vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, now()).await;
        let state = Mutex::new(AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        });
        assert!(get_account(&state).await.unwrap().is_none());
        assert!(store.load().await.unwrap().is_none());
        assert!(get_account(&state).await.unwrap().is_none());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn logout_clears_saved_session_for_successful_and_failed_revocation() {
        for status in [200, 503] {
            let (base, server) = mock_server(vec![
                (
                    "POST /api/zhishu/v1/client/auth/refresh ",
                    200,
                    json!({"code":0,"data":{"access_token":"access","refresh_token":"refresh"}}),
                ),
                ("GET /api/zhishu/v1/client/account ", 200, profile()),
                (
                    "POST /api/zhishu/v1/client/auth/logout ",
                    status,
                    config_response(),
                ),
            ]);
            let (_dir, store, _vault) =
                crate::zhishu::storage::platform_session::tests::saved_store(&base, now()).await;
            let state = PlatformAuthState(Mutex::new(AuthState {
                store: Some(store.clone()),
                ..AuthState::default()
            }));
            assert!(get_account(&state.0).await.unwrap().is_some());
            state.0.lock().await.pending = Some(PendingLogin {
                base_url: base,
                temp_token: "pending-2fa-token".into(),
                remembered: None,
            });
            assert_eq!(state.logout().await.is_ok(), status == 200);
            assert!(state.0.lock().await.pending.is_none());
            assert!(store.load().await.unwrap().is_none());
            assert!(get_account(&state.0).await.unwrap().is_none());
            let restarted = Mutex::new(AuthState {
                store: Some(store),
                ..AuthState::default()
            });
            assert!(get_account(&restarted).await.unwrap().is_none());
            server.join().unwrap();
        }
    }

    #[tokio::test]
    async fn account_identity_mismatch_clears_saved_credentials() {
        let (base, server) = mock_server(vec![
            (
                "POST /api/zhishu/v1/client/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"access","refresh_token":"refresh"}}),
            ),
            (
                "GET /api/zhishu/v1/client/account ",
                200,
                json!({"code":0,"data":{"id":2,"email":"other@example.com","status":"active","balance_usd":0}}),
            ),
        ]);
        let (_dir, store, _vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, now()).await;
        let state = Mutex::new(AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        });
        assert!(get_account(&state).await.unwrap().is_none());
        assert!(store.load().await.unwrap().is_none());
        server.join().unwrap();
    }
    #[tokio::test]
    async fn locked_vault_after_rotation_retries_saving_the_new_token() {
        use std::sync::atomic::Ordering;
        let (base, server) = mock_server(vec![
            ("GET /api/zhishu/v1/client/account ", 401, json!({})),
            (
                "POST /api/zhishu/v1/client/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"rotated-access","refresh_token":"rotated-refresh"}}),
            ),
            (
                "GET /api/zhishu/v1/client/account |rotated-access",
                200,
                profile(),
            ),
        ]);
        let (_dir, store, vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, now()).await;
        let (record, refresh_token) = store.load().await.unwrap().unwrap();
        let state = Mutex::new(AuthState {
            store: Some(store.clone()),
            restoration_checked: true,
            session: Some(Session {
                base_url: base,
                user_id: 1,
                last_used_at: now(),
                tokens: TokenPair {
                    access_token: "old-access".into(),
                    refresh_token,
                },
                persistence: Some((store.clone(), record)),
                persistence_dirty: false,
            }),
            ..AuthState::default()
        });
        vault.unavailable.store(true, Ordering::SeqCst);
        assert!(matches!(get_account(&state).await, Err(err) if err.code == "SESSION_STORE"));
        assert_eq!(
            state
                .lock()
                .await
                .session
                .as_ref()
                .unwrap()
                .tokens
                .refresh_token,
            "rotated-refresh"
        );
        vault.unavailable.store(false, Ordering::SeqCst);
        assert!(get_account(&state).await.unwrap().is_some());
        assert_eq!(store.load().await.unwrap().unwrap().1, "rotated-refresh");
        server.join().unwrap();
    }

    #[tokio::test]
    async fn login_succeeds_with_a_warning_when_the_session_vault_is_unavailable() {
        use std::sync::atomic::Ordering;
        let (base, server) =
            mock_server(vec![("GET /api/zhishu/v1/client/account ", 200, profile())]);
        let (_dir, store, vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, now()).await;
        store.clear().await.unwrap();
        vault.unavailable.store(true, Ordering::SeqCst);
        let mut state = AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        };
        let result = finish_login(
            &mut state,
            &client().unwrap(),
            base,
            json!({"access_token":"new-access","refresh_token":"new-refresh"}),
        )
        .await
        .unwrap();
        assert!(matches!(
            result,
            LoginResult::Authenticated {
                persistence_warning: Some(_),
                ..
            }
        ));
        assert!(state.session.is_some());
        assert!(store.load().await.unwrap().is_none());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn successful_new_login_replaces_the_saved_account_without_a_password_preference() {
        let (base, server) = mock_server(vec![(
            "GET /api/zhishu/v1/client/account ",
            200,
            json!({"code":0,"data":{"id":2,"email":"other@example.com","status":"active","balance_usd":0}}),
        )]);
        let (_dir, store, _vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, now()).await;
        let mut state = AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        };
        let result = finish_login(
            &mut state,
            &client().unwrap(),
            base,
            json!({"access_token":"other-access","refresh_token":"other-refresh"}),
        )
        .await
        .unwrap();
        assert!(matches!(
            result,
            LoginResult::Authenticated {
                persistence_warning: None,
                ..
            }
        ));
        let (record, token) = store.load().await.unwrap().unwrap();
        assert_eq!(record.user_id, 2);
        assert_eq!(token, "other-refresh");
        server.join().unwrap();
    }
    fn access_response() -> Value {
        json!({"code":0,"data":{
            "schema_version":1,"synced_at":"2026-10-31T08:00:00Z",
            "account":profile()["data"],"groups":[{
                "id":42,"name":"Exclusive without prefix","platform":"openai","status":"active",
                "subscription_type":"standard","is_exclusive":true,
                "credential":{"id":123,"name":"zhishu-client-42","key":"shared-model-secret","status":"active",
                    "client_managed":true,"expires_at":null,"quota_usd":0,"used_usd":0},
                "quota":{"available":true,"reasons":[]},"subscriptions":[],
                "connection":{"base_url":"https://gateway.test","endpoints":{"responses":"/v1/responses"}},
                "models":[{"id":"only-authorized","supports_streaming":false,"description":"echo shared-model-secret","unknown_token":"must-not-save"}],
                "catalog_status":"ok"
            }]
        }})
    }

    #[tokio::test]
    async fn access_is_the_only_key_source_and_keeps_server_authorized_groups() {
        let (base, server) = mock_server(vec![(
            "POST /api/zhishu/v1/client/access |access-secret",
            200,
            access_response(),
        )]);
        let discovery = model_session(base.clone())
            .discover_group_catalogs()
            .await
            .unwrap();
        assert_eq!(discovery.account.base_url, base);
        assert_eq!(discovery.synced_at, 1793433600000);
        assert_eq!(discovery.groups[0].name, "Exclusive without prefix");
        let catalog = &discovery.catalogs[0];
        assert_eq!(catalog.credential.key, "shared-model-secret");
        assert_eq!(catalog.credential.key_name, "zhishu-client-42");
        assert_eq!(
            catalog.connection.as_ref().unwrap().base_url,
            "https://gateway.test"
        );
        assert_eq!(catalog.models[0]["supports_streaming"], false);
        assert!(catalog.models[0].get("supports_tools").is_none());
        assert!(!serde_json::to_string(&catalog.models)
            .unwrap()
            .contains("must-not-save"));
        assert!(!serde_json::to_string(&catalog.models)
            .unwrap()
            .contains("shared-model-secret"));
        server.join().unwrap();
    }

    #[tokio::test]
    async fn access_refreshes_once_without_falling_back_to_panel_endpoints() {
        let (base, server) = mock_server(vec![
            (
                "POST /api/zhishu/v1/client/access |access-secret",
                401,
                json!({}),
            ),
            (
                "POST /api/zhishu/v1/client/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"new-access","refresh_token":"new-refresh"}}),
            ),
            (
                "POST /api/zhishu/v1/client/access |new-access",
                200,
                access_response(),
            ),
        ]);
        assert_eq!(
            model_session(base)
                .discover_group_catalogs()
                .await
                .unwrap()
                .catalogs
                .len(),
            1
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn access_errors_never_create_empty_snapshots_or_request_old_endpoints() {
        for status in [403, 404, 429, 503] {
            let (base, server) = mock_server(vec![(
                "POST /api/zhishu/v1/client/access ",
                status,
                json!({"message":"shared-model-secret"}),
            )]);
            let result = model_session(base).discover_group_catalogs().await;
            assert!(result.is_err());
            assert!(!serde_json::to_string(&result.err().unwrap())
                .unwrap()
                .contains("shared-model-secret"));
            server.join().unwrap();
        }
    }

    #[tokio::test]
    async fn access_distinguishes_failed_catalog_empty_success_and_unavailable_quota() {
        for failed in [false, true] {
            let mut response = access_response();
            response["data"]["groups"][0]["models"] = if failed { Value::Null } else { json!([]) };
            response["data"]["groups"][0]["catalog_status"] =
                json!(if failed { "failed" } else { "ok" });
            response["data"]["groups"][0]["catalog_error"] = json!("MODEL_CATALOG_UNAVAILABLE");
            response["data"]["groups"][0]["quota"] =
                json!({"available":false,"reasons":["key_quota_exhausted"]});
            let (base, server) =
                mock_server(vec![("POST /api/zhishu/v1/client/access ", 200, response)]);
            let data = model_session(base).discover_group_catalogs().await.unwrap();
            assert_eq!(data.failed_group_ids.contains(&42), failed);
            assert_eq!(data.catalogs[0].credential.key, "shared-model-secret");
            assert!(!data.details[&42].quota.available);
            server.join().unwrap();
        }
    }

    #[tokio::test]
    async fn access_rejects_wrong_identity_schema_and_invalid_model_ids() {
        for (path, value) in [
            ("identity", json!(2)),
            ("schema", json!(2)),
            ("model", json!({"name":"no id"})),
            ("duplicate", Value::Null),
        ] {
            let mut response = access_response();
            match path {
                "identity" => response["data"]["account"]["id"] = value,
                "schema" => response["data"]["schema_version"] = value,
                "duplicate" => {
                    let group = response["data"]["groups"][0].clone();
                    response["data"]["groups"]
                        .as_array_mut()
                        .unwrap()
                        .push(group);
                }
                _ => response["data"]["groups"][0]["models"][0] = value,
            }
            let (base, server) =
                mock_server(vec![("POST /api/zhishu/v1/client/access ", 200, response)]);
            let state = model_session(base);
            assert!(state.discover_group_catalogs().await.is_err());
            if path == "identity" {
                assert!(state.0.lock().await.session.is_none());
            }
            server.join().unwrap();
        }
    }

    #[tokio::test]
    async fn usage_is_read_only_and_cannot_expose_keys_connection_or_models() {
        let mut response = access_response();
        response["data"]["groups"][0]["models"] = Value::Null;
        let (base, server) = mock_server(vec![(
            "GET /api/zhishu/v1/client/usage/summary |access-secret",
            200,
            response,
        )]);
        let result = subscription_usage(&model_session(base).0).await.unwrap();
        let public = serde_json::to_value(result).unwrap();
        assert_eq!(public["groups"][0]["quota"]["available"], true);
        for secret in [
            "shared-model-secret",
            "connection",
            "models",
            "access-secret",
        ] {
            assert!(!public.to_string().contains(secret));
        }
        server.join().unwrap();
    }

    #[tokio::test]
    async fn login_without_refresh_token_clears_previous_account_persistence() {
        let (base, server) =
            mock_server(vec![("GET /api/zhishu/v1/client/account ", 200, profile())]);
        let (_dir, store, _vault) =
            crate::zhishu::storage::platform_session::tests::saved_store(&base, now()).await;
        let mut state = AuthState {
            store: Some(store.clone()),
            ..AuthState::default()
        };
        let result = finish_login(
            &mut state,
            &client().unwrap(),
            base,
            json!({"access_token":"access-only"}),
        )
        .await
        .unwrap();
        assert!(matches!(
            result,
            LoginResult::Authenticated {
                persistence_warning: Some(_),
                ..
            }
        ));
        assert!(state
            .session
            .as_ref()
            .unwrap()
            .tokens
            .refresh_token
            .is_empty());
        assert!(store.load().await.unwrap().is_none());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn request_allowlist_rejects_arbitrary_paths_before_network() {
        for (method, path) in [
            (Method::GET, "/keys"),
            (Method::POST, "/credentials"),
            (Method::DELETE, "/access"),
            (Method::GET, "/models"),
        ] {
            let result: Result<Value, _> = request(
                &client().unwrap(),
                "http://127.0.0.1:0",
                method,
                path,
                None,
                None,
            )
            .await;
            assert_eq!(result.err().unwrap().code, "CLIENT_API_ONLY");
        }
    }

    #[test]
    fn config_resolves_web_links_from_origin_and_honors_disabled_entries() {
        let mut value = config_response()["data"].clone();
        value["registration_enabled"] = json!(false);
        value["subscriptions_url"] = json!("javascript:alert(1)");
        let config: ClientConfig = serde_json::from_value(value).unwrap();
        let config = config.validate("https://platform.test/sub").unwrap();
        assert!(config.register_url.is_none());
        assert!(config.subscriptions_url.is_none());
        assert_eq!(
            config.password_reset_url.as_deref(),
            Some("https://platform.test/forgot-password")
        );
    }
    #[tokio::test]
    async fn successful_empty_access_and_usage_are_valid_snapshots() {
        let mut response = access_response();
        response["data"]["groups"] = json!([]);
        let (base, server) = mock_server(vec![
            ("POST /api/zhishu/v1/client/access ", 200, response.clone()),
            ("GET /api/zhishu/v1/client/usage/summary ", 200, response),
        ]);
        let state = model_session(base);
        assert!(state
            .discover_group_catalogs()
            .await
            .unwrap()
            .groups
            .is_empty());
        assert!(subscription_usage(&state.0)
            .await
            .unwrap()
            .groups
            .is_empty());
        server.join().unwrap();
    }
}
