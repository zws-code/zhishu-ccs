//! 平台登录、二次验证、账户恢复以及带认证的分组与额度请求。
//! access token 和二次验证临时令牌仅驻内存，refresh token 交给系统凭据库保存；
//! 「记住密码」由独立的 platform_login_history 管理，不参与会话恢复。
use reqwest::{Client, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
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
    pub(crate) role: String,
    pub(crate) balance: f64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformAccount {
    pub(crate) base_url: String,
    pub(crate) user: PlatformUser,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformUsageWindow {
    pub limit_usd: f64,
    pub used_usd: f64,
    pub remaining_usd: f64,
    pub percentage: f64,
    pub resets_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformSubscriptionUsage {
    pub subscription_id: i64,
    pub group_id: i64,
    pub group_name: String,
    pub group_platform: String,
    pub expires_at: String,
    pub expires_in_days: i64,
    pub unlimited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily: Option<PlatformUsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly: Option<PlatformUsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly: Option<PlatformUsageWindow>,
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
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct GroupDiscovery {
    pub account: PlatformAccount,
    pub groups: Vec<AvailableGroup>,
    pub catalogs: Vec<GroupCatalog>,
    pub failed_group_ids: HashSet<i64>,
    #[serde(skip)]
    pub obsolete_keys: Vec<ApiKeyRecord>,
    pub subscription_group_ids: HashSet<i64>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct ApiKeyRecord {
    pub id: i64,
    pub key: String,
    name: String,
    pub group_id: Option<i64>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    quota: f64,
    #[serde(default)]
    quota_used: f64,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Deserialize)]
struct Paginated<T> {
    items: Vec<T>,
    #[serde(default)]
    total: i64,
    #[serde(default)]
    pages: i64,
}

#[derive(Deserialize)]
struct SubscriptionRecord {
    id: i64,
    group_id: i64,
    expires_at: String,
    status: String,
    #[serde(default)]
    group: Option<SubscriptionUsageGroup>,
}

#[derive(Deserialize)]
struct SubscriptionUsageItem {
    subscription: SubscriptionUsageIdentity,
    progress: SubscriptionUsageProgress,
}

#[derive(Deserialize)]
struct SubscriptionUsageIdentity {
    group_id: i64,
    #[serde(default)]
    group: Option<SubscriptionUsageGroup>,
}

#[derive(Deserialize)]
struct SubscriptionUsageGroup {
    #[serde(default)]
    name: String,
    #[serde(default)]
    platform: String,
    #[serde(default)]
    daily_limit_usd: Option<f64>,
    #[serde(default)]
    weekly_limit_usd: Option<f64>,
    #[serde(default)]
    monthly_limit_usd: Option<f64>,
}

#[derive(Deserialize)]
struct SubscriptionUsageProgress {
    id: i64,
    #[serde(default)]
    group_name: String,
    expires_at: String,
    #[serde(default)]
    expires_in_days: i64,
    #[serde(default)]
    daily: Option<UsageWindowProgress>,
    #[serde(default)]
    weekly: Option<UsageWindowProgress>,
    #[serde(default)]
    monthly: Option<UsageWindowProgress>,
}

#[derive(Deserialize)]
struct UsageWindowProgress {
    limit_usd: f64,
    used_usd: f64,
    remaining_usd: f64,
    percentage: f64,
    #[serde(default)]
    resets_at: Option<String>,
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

fn normalize_base_url_with_policy(
    input: &str,
    allow_remote_http: bool,
) -> Result<String, PlatformAuthError> {
    let mut url =
        Url::parse(input.trim()).map_err(|_| error("INVALID_URL", "请输入有效的平台地址"))?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    let http_allowed = url.scheme() == "http" && (local || allow_remote_http);
    if (url.scheme() != "https" && !http_allowed)
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(error(
            "INVALID_URL",
            "平台地址必须使用 HTTPS；本机开发可使用 HTTP，且地址不能包含账号、查询参数或片段",
        ));
    }
    let path = url.path().trim_end_matches('/');
    let path = path.strip_suffix("/api/v1").unwrap_or(path).to_string();
    url.set_path(&path);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn normalize_base_url(input: &str) -> Result<String, PlatformAuthError> {
    normalize_base_url_with_policy(
        input,
        matches!(
            option_env!("ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP"),
            Some("true")
        ),
    )
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
    let mut builder = client.request(method, format!("{base}/api/v1{path}"));
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
        if path.starts_with("/keys") && status != StatusCode::UNAUTHORIZED {
            let (code, message) = match status {
                StatusCode::FORBIDDEN => {
                    ("KEY_FORBIDDEN", "平台拒绝管理该分组的 CC Switch 专用 Key")
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    ("RATE_LIMIT", "专用 Key 操作过于频繁，请稍后重试")
                }
                StatusCode::BAD_REQUEST => {
                    ("KEY_REQUEST", "专用 Key 参数无效，请检查分组权限后重试")
                }
                _ => ("KEY_REQUEST", "无法完成分组专用 Key 操作，请稍后重试"),
            };
            return Err(error(code, message));
        }
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
    let user = request(
        client,
        &base_url,
        Method::GET,
        "/auth/me",
        None,
        Some(&tokens.access_token),
    )
    .await?;
    let account = PlatformAccount {
        base_url: base_url.clone(),
        user,
    };
    // 发布新会话前先失效旧账户；新凭据持久化失败时保留本次登录，并返回明确警告。
    let (persistence, persistence_warning) = if let Some(store) = &state.store {
        store.clear().await?;
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
    } else {
        (None, None)
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
    let settings: Value =
        request(&client, &base, Method::GET, "/settings/public", None, None).await?;
    if [
        "turnstile_enabled",
        "tencent_captcha_enabled",
        "aliyun_captcha_enabled",
    ]
    .iter()
    .any(|key| settings.get(key).and_then(Value::as_bool) == Some(true))
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

fn usage_window(progress: UsageWindowProgress) -> PlatformUsageWindow {
    PlatformUsageWindow {
        limit_usd: progress.limit_usd.max(0.0),
        used_usd: progress.used_usd.max(0.0),
        remaining_usd: progress.remaining_usd.max(0.0),
        percentage: progress.percentage.clamp(0.0, 100.0),
        resets_at: progress.resets_at,
    }
}

fn window_from_limit(limit: Option<f64>) -> Option<PlatformUsageWindow> {
    limit
        .filter(|limit| *limit > 0.0)
        .map(|limit| PlatformUsageWindow {
            limit_usd: limit,
            used_usd: 0.0,
            remaining_usd: limit,
            percentage: 0.0,
            resets_at: None,
        })
}

// 首次使用前可能没有窗口进度，此时沿用分组已报告的限额；不猜测未报告的额度。
fn map_subscription_usage(item: SubscriptionUsageItem) -> PlatformSubscriptionUsage {
    let group = item.subscription.group;
    let fallback_name = group
        .as_ref()
        .map(|group| group.name.clone())
        .unwrap_or_default();
    let group_platform = group
        .as_ref()
        .map(|group| group.platform.clone())
        .unwrap_or_default();
    let daily_limit = group.as_ref().and_then(|group| group.daily_limit_usd);
    let weekly_limit = group.as_ref().and_then(|group| group.weekly_limit_usd);
    let monthly_limit = group.as_ref().and_then(|group| group.monthly_limit_usd);
    let group_name = if item.progress.group_name.trim().is_empty() {
        fallback_name
    } else {
        item.progress.group_name
    };
    let daily = item
        .progress
        .daily
        .map(usage_window)
        .or_else(|| window_from_limit(daily_limit));
    let weekly = item
        .progress
        .weekly
        .map(usage_window)
        .or_else(|| window_from_limit(weekly_limit));
    let monthly = item
        .progress
        .monthly
        .map(usage_window)
        .or_else(|| window_from_limit(monthly_limit));
    PlatformSubscriptionUsage {
        subscription_id: item.progress.id,
        group_id: item.subscription.group_id,
        group_name,
        group_platform,
        expires_at: item.progress.expires_at,
        expires_in_days: item.progress.expires_in_days.max(0),
        unlimited: daily.is_none() && weekly.is_none() && monthly.is_none(),
        status: None,
        daily,
        weekly,
        monthly,
    }
}

/// 在 Rust 内完成订阅额度请求与认证，避免网页 access token 随供应商配置进入前端或落盘。
#[tauri::command]
pub async fn platform_get_subscription_usage(
    state: State<'_, PlatformAuthState>,
) -> Result<Vec<PlatformSubscriptionUsage>, PlatformAuthError> {
    subscription_usage(&state.0).await
}

async fn subscription_usage(
    state: &Mutex<AuthState>,
) -> Result<Vec<PlatformSubscriptionUsage>, PlatformAuthError> {
    let mut auth = state.lock().await;
    let session = auth
        .session
        .as_mut()
        .ok_or_else(|| error("UNAUTHORIZED", "请先登录平台"))?;
    let client = client()?;
    let items: Vec<SubscriptionUsageItem> =
        match get_authenticated(&client, session, "/subscriptions/progress").await {
            Ok(items) => items,
            Err(err) if err.code == "UNAUTHORIZED" => {
                invalidate_session(&mut auth).await;
                return Err(err);
            }
            Err(err) => return Err(err),
        };
    // 进度接口只包含活跃订阅，因此还需合并完整记录，保留过期和暂停状态。
    // 缺少进度不能显示为不限量，也不能将订阅误当成钱包计费。
    let records: Vec<SubscriptionRecord> =
        match get_authenticated(&client, session, "/subscriptions").await {
            Ok(records) => records,
            Err(err) if err.code == "UNAUTHORIZED" => {
                invalidate_session(&mut auth).await;
                return Err(err);
            }
            Err(err) => return Err(err),
        };
    let mut usage: Vec<_> = items.into_iter().map(map_subscription_usage).collect();
    for record in records {
        if let Some(item) = usage
            .iter_mut()
            .find(|item| item.group_id == record.group_id)
        {
            item.status = Some(record.status);
            item.expires_at = record.expires_at;
            continue;
        }
        let group = record.group;
        usage.push(PlatformSubscriptionUsage {
            subscription_id: record.id,
            group_id: record.group_id,
            group_name: group
                .as_ref()
                .map(|group| group.name.clone())
                .unwrap_or_default(),
            group_platform: group
                .as_ref()
                .map(|group| group.platform.clone())
                .unwrap_or_default(),
            expires_at: record.expires_at,
            expires_in_days: 0,
            unlimited: false,
            status: Some(if record.status == "active" {
                "unavailable".into()
            } else {
                record.status
            }),
            daily: None,
            weekly: None,
            monthly: None,
        });
    }
    Ok(usage)
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
    let user = match get_authenticated::<PlatformUser>(&client, session, "/auth/me").await {
        Ok(user) => user,
        Err(err) if matches!(err.code, "UNAUTHORIZED" | "FORBIDDEN") => {
            invalidate_session(&mut state).await;
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    if user.id != session.user_id {
        invalidate_session(&mut state).await;
        return Ok(None);
    }
    Ok(Some(PlatformAccount {
        base_url: session.base_url.clone(),
        user,
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

async fn key_models(
    client: &Client,
    base: &str,
    key: &str,
) -> Result<Vec<Value>, PlatformAuthError> {
    let response = send_json(client.get(format!("{base}/v1/models")).bearer_auth(key))
        .await
        .map_err(|_| error("NETWORK", "无法获取模型列表，请检查网络后重试"))?;
    let status = response.status;
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => error("MODEL_AUTH", "模型授权已失效，请重新登录"),
            402 => error(
                "INSUFFICIENT_BALANCE",
                "账户余额或模型额度不足，请充值后重试",
            ),
            403 => error(
                "MODEL_FORBIDDEN",
                "专用 Key 无权访问模型，请检查平台分组权限或账户额度",
            ),
            429 => error("RATE_LIMIT", "请求过于频繁，请稍后重试"),
            _ => error("MODEL_LIST", "无法获取平台模型列表，请刷新后重试"),
        });
    }
    let response: Value = response
        .body
        .map_err(|_| error("INVALID_RESPONSE", "模型列表响应格式无效"))?;
    response
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| error("INVALID_RESPONSE", "模型列表响应缺少数据"))
}

const SYNC_GROUP_PREFIX: &str = "智枢router-ccs-";

fn is_obsolete_group_key(key: &ApiKeyRecord, allowed_group_ids: &HashSet<i64>) -> bool {
    let Some(group_id) = key.group_id.filter(|id| *id > 0) else {
        return false;
    };
    // Only the exact per-group naming convention belongs to CC Switch.
    // Ordinary user keys, including similarly named ones, are never revoked.
    let dedicated = key.name.strip_prefix("zhishu-ccs-").is_some_and(|name| {
        name.strip_suffix(&format!("-{group_id}"))
            .is_some_and(|platform| !platform.is_empty() && !platform.contains(char::is_whitespace))
    });
    dedicated && !allowed_group_ids.contains(&group_id)
}

fn group_key_name(group: &AvailableGroup) -> String {
    format!("zhishu-ccs-{}-{}", group.platform.trim(), group.id)
}

async fn group_key_models(
    client: &Client,
    base: &str,
    key: &str,
) -> Result<Vec<Value>, PlatformAuthError> {
    let mut models = key_models(client, base, key).await?;
    if models.is_empty() {
        return Ok(models);
    }
    // The ordinary list controls membership; the Codex manifest supplies group-
    // specific capabilities, including metadata for public model aliases.
    let manifest = async {
        let response = send_json(
            client
                .get(format!("{base}/v1/models"))
                .query(&[(
                    "client_version",
                    crate::proxy::providers::codex_oauth_auth::CODEX_OAUTH_CLIENT_VERSION,
                )])
                .bearer_auth(key)
                .timeout(Duration::from_secs(10)),
        )
        .await
        .map_err(|_| error("NETWORK", "无法获取模型能力清单"))?;
        if !response.status.is_success() {
            return Err(error("MODEL_LIST", "无法获取模型能力清单"));
        }
        response
            .body
            .map_err(|_| error("INVALID_RESPONSE", "模型能力清单响应格式无效"))
    }
    .await;
    if let Ok(manifest) = manifest {
        if let Some(entries) = manifest
            .get("models")
            .or_else(|| manifest.get("data"))
            .and_then(Value::as_array)
        {
            for model in &mut models {
                let Some(id) = model.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(entry) = entries.iter().find(|entry| {
                    entry
                        .get("slug")
                        .or_else(|| entry.get("id"))
                        .and_then(Value::as_str)
                        == Some(id)
                }) else {
                    continue;
                };
                for field in [
                    "display_name",
                    "description",
                    "context_window",
                    "max_output_tokens",
                    "supported_reasoning_levels",
                    "default_reasoning_level",
                    "reasoning",
                    "input_modalities",
                    "supports_parallel_tool_calls",
                ] {
                    if let Some(value) = entry.get(field).filter(|value| !value.is_null()) {
                        model[field] = value.clone();
                    }
                }
            }
        }
    }
    Ok(models)
}

fn key_is_exhausted(key: &ApiKeyRecord) -> bool {
    key.status == "quota_exhausted" || (key.quota > 0.0 && key.quota_used >= key.quota)
}

fn key_is_usable(key: &ApiKeyRecord) -> bool {
    let expired = key.expires_at.as_deref().is_some_and(|value| {
        chrono::DateTime::parse_from_rfc3339(value)
            .is_ok_and(|expires_at| expires_at < chrono::Utc::now())
    });
    key.status == "active" && !key_is_exhausted(key) && !expired
}

async fn list_user_keys(
    client: &Client,
    base: &str,
    access_token: &str,
) -> Result<Vec<ApiKeyRecord>, PlatformAuthError> {
    let mut keys = Vec::new();
    for page in 1..=100 {
        let path = format!("/keys?page={page}&page_size=100&sort_by=created_at&sort_order=desc");
        let result: Paginated<ApiKeyRecord> =
            request(client, base, Method::GET, &path, None, Some(access_token)).await?;
        let count = result.items.len();
        keys.extend(result.items);
        if count == 0
            || (result.pages > 0 && page >= result.pages)
            || (result.pages == 0 && count < 100)
        {
            break;
        }
        if result.total > 0 && keys.len() as i64 >= result.total {
            break;
        }
    }
    Ok(keys)
}

async fn create_group_key(
    client: &Client,
    base: &str,
    access_token: &str,
    group: &AvailableGroup,
) -> Result<ApiKeyRecord, PlatformAuthError> {
    request(
        client,
        base,
        Method::POST,
        "/keys",
        Some(json!({"name": group_key_name(group), "group_id": group.id})),
        Some(access_token),
    )
    .await
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

    /// 持有会话锁校验本机身份；不刷新令牌、不请求 /auth/me，使已登录用户断网时仍能本地同步。
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

    /// 平台阶段获取分组、订阅、分页 Key 和模型目录，复用专用 Key 并保留失败分组信息。
    pub(crate) async fn discover_group_catalogs(
        &self,
    ) -> Result<GroupDiscovery, PlatformAuthError> {
        let account = get_account(&self.0)
            .await?
            .ok_or_else(|| error("UNAUTHORIZED", "请先登录平台"))?;
        let (base_url, access_token) = {
            let state = self.0.lock().await;
            let session = state
                .session
                .as_ref()
                .filter(|s| s.base_url == account.base_url && s.user_id == account.user.id)
                .ok_or_else(|| error("UNAUTHORIZED", "登录状态已改变，请重试"))?;
            (
                session.base_url.clone(),
                session.tokens.access_token.clone(),
            )
        };

        let client = client()?;
        let groups: Vec<AvailableGroup> = request(
            &client,
            &base_url,
            Method::GET,
            "/groups/available",
            None,
            Some(&access_token),
        )
        .await?;
        // The available-group list omits expired subscriptions. Fetch the
        // account's complete subscription history before making any key changes.
        // A failed read aborts discovery so an outage cannot trigger cleanup.
        let subscriptions: Vec<SubscriptionRecord> = request(
            &client,
            &base_url,
            Method::GET,
            "/subscriptions",
            None,
            Some(&access_token),
        )
        .await?;
        let subscription_group_ids: HashSet<_> = subscriptions
            .iter()
            .map(|sub| sub.group_id)
            .chain(
                groups
                    .iter()
                    .filter(|group| group.subscription_type == "subscription")
                    .map(|group| group.id),
            )
            .collect();
        let allowed_group_ids: HashSet<_> = groups
            .iter()
            .filter(|group| {
                group.subscription_type == "subscription"
                    || group.name.starts_with(SYNC_GROUP_PREFIX)
            })
            .map(|group| group.id)
            .chain(subscription_group_ids.iter().copied())
            .collect();
        let groups: Vec<_> = groups
            .into_iter()
            .filter(|group| {
                group.subscription_type == "subscription"
                    || group.name.starts_with(SYNC_GROUP_PREFIX)
            })
            .filter(|g| g.status.is_empty() || g.status == "active")
            .collect();
        let keys = list_user_keys(&client, &base_url, &access_token).await?;
        let obsolete_keys = keys
            .iter()
            .filter(|key| is_obsolete_group_key(key, &allowed_group_ids))
            .cloned()
            .collect();

        let mut catalogs = Vec::new();
        let mut failed_group_ids = HashSet::new();
        let mut warnings = Vec::new();
        for group in &groups {
            let key_name = group_key_name(group);
            if group.platform.trim().is_empty() {
                failed_group_ids.insert(group.id);
                warnings.push(format!("分组 {} 缺少平台类型，已跳过", group.name));
                continue;
            }
            let group_keys: Vec<_> = keys
                .iter()
                .filter(|key| key.name == key_name && key.group_id == Some(group.id))
                .collect();
            let mut key = group_keys
                .iter()
                .copied()
                .find(|key| key_is_usable(key))
                .cloned();
            if key.is_none() && group_keys.iter().any(|key| key_is_exhausted(key)) {
                failed_group_ids.insert(group.id);
                warnings.push(format!(
                    "分组 {} 的 CC Switch 专用 Key 额度已耗尽，已保留本地供应商",
                    group.name
                ));
                continue;
            }
            if key.is_none() {
                match create_group_key(&client, &base_url, &access_token, group).await {
                    Ok(created) => key = Some(created),
                    Err(err) if err.code == "UNAUTHORIZED" => return Err(err),
                    Err(err) => {
                        failed_group_ids.insert(group.id);
                        warnings.push(format!(
                            "无法为分组 {} 创建 CC Switch 专用 Key：{}",
                            group.name, err.message
                        ));
                        continue;
                    }
                }
            }
            let Some(mut key) = key else {
                failed_group_ids.insert(group.id);
                continue;
            };
            let (models, catalog_error) = match group_key_models(&client, &base_url, &key.key).await
            {
                Ok(models) => (models, None),
                Err(err) if err.code == "MODEL_AUTH" => {
                    // A revoked key can still be listed briefly. Recreate it once and retry.
                    match create_group_key(&client, &base_url, &access_token, group).await {
                        Ok(created) => {
                            // Keep the replacement even if model discovery is temporarily down;
                            // fallback providers must never receive the already rejected key.
                            key = created;
                            match group_key_models(&client, &base_url, &key.key).await {
                                Ok(models) => (models, None),
                                Err(err) if err.code == "UNAUTHORIZED" => return Err(err),
                                Err(err) if err.code == "MODEL_AUTH" => {
                                    failed_group_ids.insert(group.id);
                                    warnings.push(format!(
                                        "分组 {} 的新 Key 授权失败，已保留本地供应商：{}",
                                        group.name, err.message
                                    ));
                                    continue;
                                }
                                Err(err) => {
                                    failed_group_ids.insert(group.id);
                                    (Vec::new(), Some(err.message))
                                }
                            }
                        }
                        Err(err) if err.code == "UNAUTHORIZED" => return Err(err),
                        Err(err) => {
                            failed_group_ids.insert(group.id);
                            warnings.push(format!(
                                "无法替换分组 {} 的失效 Key，已保留本地供应商：{}",
                                group.name, err.message
                            ));
                            continue;
                        }
                    }
                }
                Err(err) if err.code == "UNAUTHORIZED" => return Err(err),
                Err(err) => {
                    failed_group_ids.insert(group.id);
                    (Vec::new(), Some(err.message))
                }
            };
            catalogs.push(GroupCatalog {
                group: group.clone(),
                credential: GroupCredential {
                    key: key.key,
                    key_id: key.id,
                    platform: group.platform.clone(),
                    key_name,
                },
                models,
                catalog_error,
            });
        }

        Ok(GroupDiscovery {
            account,
            groups,
            catalogs,
            failed_group_ids,
            obsolete_keys,
            subscription_group_ids,
            warnings,
        })
    }

    pub(crate) async fn delete_obsolete_group_keys(
        &self,
        account: &PlatformAccount,
        key_ids: &[i64],
    ) -> Result<Vec<String>, PlatformAuthError> {
        if key_ids.is_empty() {
            return Ok(Vec::new());
        }
        // Hold the session lock through revocation so an account change cannot
        // send a previous account's deletion list with a different user's JWT.
        let mut state = self.0.lock().await;
        let session = state
            .session
            .as_mut()
            .filter(|session| {
                session.base_url == account.base_url && session.user_id == account.user.id
            })
            .ok_or_else(|| {
                error(
                    "UNAUTHORIZED",
                    "登录状态已改变，旧分组专用 Key 未清理，请重新同步",
                )
            })?;
        let client = client()?;
        let mut warnings = Vec::new();
        for id in key_ids {
            let result: Result<Value, _> =
                request_authenticated(&client, session, Method::DELETE, &format!("/keys/{id}"))
                    .await;
            if let Err(err) = result {
                warnings.push(format!(
                    "旧分组专用 Key {id} 未能删除，请重新同步：{}",
                    err.message
                ));
                if err.code == "UNAUTHORIZED" {
                    break;
                }
            }
        }
        Ok(warnings)
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
            normalize_base_url("https://example.com/sub/api/v1/").unwrap(),
            "https://example.com/sub"
        );
        assert_eq!(
            normalize_base_url("http://[::1]:3000").unwrap(),
            "http://[::1]:3000"
        );
    }

    #[test]
    fn rejects_insecure_remote_and_ambiguous_urls() {
        for url in [
            "http://example.com",
            "file:///tmp/auth",
            "https://user:pass@example.com",
            "https://example.com/?token=secret",
            "https://example.com/#login",
            "localhost:3000",
        ] {
            assert!(normalize_base_url(url).is_err(), "accepted {url}");
        }
    }

    #[test]
    fn allows_plain_http_only_when_release_configuration_opts_in() {
        assert_eq!(
            normalize_base_url_with_policy("http://192.168.10.20:3000", true).unwrap(),
            "http://192.168.10.20:3000"
        );
        assert!(normalize_base_url_with_policy("http://192.168.10.20:3000", false).is_err());
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
                let body = body.to_string();
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        (base, handle)
    }

    fn profile() -> Value {
        json!({"code":0,"data":{"id":1,"email":"test@example.com","username":"test","role":"user","balance":2.0}})
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

    #[test]
    fn subscription_group_limits_fill_windows_before_first_usage() {
        let usage = map_subscription_usage(SubscriptionUsageItem {
            subscription: SubscriptionUsageIdentity {
                group_id: 42,
                group: Some(SubscriptionUsageGroup {
                    name: "lite".into(),
                    platform: "composite".into(),
                    daily_limit_usd: Some(10.0),
                    weekly_limit_usd: Some(50.0),
                    monthly_limit_usd: None,
                }),
            },
            progress: SubscriptionUsageProgress {
                id: 7,
                group_name: "lite订阅".into(),
                expires_at: "2026-11-01T12:00:00Z".into(),
                expires_in_days: 9,
                daily: None,
                weekly: None,
                monthly: None,
            },
        });
        assert!(!usage.unlimited);
        let daily = usage.daily.unwrap();
        assert_eq!(daily.remaining_usd, 10.0);
        assert_eq!(daily.percentage, 0.0);
        assert!(daily.resets_at.is_none());
        assert_eq!(usage.weekly.unwrap().limit_usd, 50.0);
        assert!(usage.monthly.is_none());
    }

    #[tokio::test]
    async fn subscription_usage_returns_group_windows_without_tokens() {
        let (base, server) = mock_server(vec![
            (
                "GET /api/v1/subscriptions/progress |access-secret",
                200,
                json!({"code":0,"data":[{
                    "subscription":{
                        "group_id":42,
                        "group":{"name":"fallback","platform":"composite"}
                    },
                    "progress":{
                        "id":7,
                        "group_name":"lite订阅",
                        "expires_at":"2026-11-01T12:00:00Z",
                        "expires_in_days":9,
                        "daily":{
                            "limit_usd":10.0,"used_usd":2.5,"remaining_usd":7.5,
                            "percentage":25.0,"resets_at":"2026-10-24T00:00:00Z"
                        },
                        "weekly":null,
                        "monthly":{
                            "limit_usd":100.0,"used_usd":20.0,"remaining_usd":80.0,
                            "percentage":20.0,"resets_at":"2026-11-01T00:00:00Z"
                        }
                    }
                }]}),
            ),
            (
                "GET /api/v1/subscriptions |access-secret",
                200,
                json!({"code":0,"data":[]}),
            ),
        ]);
        let state = model_session(base);
        let usage = subscription_usage(&state.0).await.unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].group_id, 42);
        assert_eq!(usage[0].group_name, "lite订阅");
        assert_eq!(usage[0].group_platform, "composite");
        assert!(!usage[0].unlimited);
        assert_eq!(usage[0].daily.as_ref().unwrap().remaining_usd, 7.5);
        assert!(usage[0].weekly.is_none());
        assert_eq!(usage[0].monthly.as_ref().unwrap().percentage, 20.0);
        let serialized = serde_json::to_string(&usage).unwrap();
        assert!(!serialized.contains("access-secret"));
        assert!(!serialized.contains("refresh-secret"));
        server.join().unwrap();
    }

    #[tokio::test]
    async fn subscription_usage_refreshes_an_expired_access_token_once() {
        let (base, server) = mock_server(vec![
            (
                "GET /api/v1/subscriptions/progress |access-secret",
                401,
                json!({}),
            ),
            (
                "POST /api/v1/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"new-access","refresh_token":"new-refresh"}}),
            ),
            (
                "GET /api/v1/subscriptions/progress |new-access",
                200,
                json!({"code":0,"data":[{
                    "subscription":{"group_id":43,"group":{"name":"unlimited","platform":"composite"}},
                    "progress":{
                        "id":8,"group_name":"unlimited","expires_at":"2026-11-01T12:00:00Z",
                        "expires_in_days":9,"daily":null,"weekly":null,"monthly":null
                    }
                }]}),
            ),
            (
                "GET /api/v1/subscriptions |new-access",
                200,
                json!({"code":0,"data":[]}),
            ),
        ]);
        let state = model_session(base);
        let usage = subscription_usage(&state.0).await.unwrap();
        assert_eq!(usage.len(), 1);
        assert!(usage[0].unlimited);
        let auth = state.0.lock().await;
        let tokens = &auth.session.as_ref().unwrap().tokens;
        assert_eq!(tokens.access_token, "new-access");
        assert_eq!(tokens.refresh_token, "new-refresh");
        drop(auth);
        server.join().unwrap();
    }

    #[tokio::test]
    async fn group_catalog_enriches_only_authorized_ids_with_the_group_key() {
        let (base, server) = mock_server(vec![
            (
                "GET /v1/models |group-secret",
                200,
                json!({"data":[
                    {"id":"public-alias","supported_endpoint_types":["responses"]},
                    {"id":"basic-only"}
                ]}),
            ),
            (
                "GET /v1/models?client_version=|group-secret",
                200,
                json!({"models":[
                    {"slug":"public-alias","context_window":1000000,
                     "supported_reasoning_levels":[{"effort":"low"},{"effort":"high"},{"effort":"max"}],
                     "default_reasoning_level":"high","input_modalities":["text","image"]},
                    {"slug":"not-in-basic-list","context_window":32000}
                ]}),
            ),
        ]);
        let models = group_key_models(&client().unwrap(), &base, "group-secret")
            .await
            .unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["id"], "public-alias");
        assert_eq!(models[0]["context_window"], 1000000);
        assert_eq!(models[0]["default_reasoning_level"], "high");
        assert_eq!(models[0]["supported_endpoint_types"], json!(["responses"]));
        assert_eq!(models[1], json!({"id":"basic-only"}));
        server.join().unwrap();
    }

    #[test]
    fn obsolete_group_keys_exclude_manual_mismatched_and_allowed_keys() {
        for (name, group_id, obsolete) in [
            ("zhishu-ccs-anthropic-43", Some(43), true),
            ("zhishu-ccs-composite-43", Some(43), true),
            ("zhishu-ccs-anthropic-42", Some(42), false),
            ("manual-key", Some(43), false),
            ("zhishu-ccs-anthropic-43-copy", Some(43), false),
            ("zhishu-ccs-anthropic-44", Some(43), false),
            ("zhishu-ccs--43", Some(43), false),
            ("zhishu-ccs-anthropic-43", None, false),
        ] {
            let key: ApiKeyRecord = serde_json::from_value(json!({
                "id": 90, "key": "secret", "name": name, "group_id": group_id
            }))
            .unwrap();
            assert_eq!(
                is_obsolete_group_key(&key, &HashSet::from([42])),
                obsolete,
                "{name}"
            );
        }
    }

    #[tokio::test]
    async fn obsolete_key_cleanup_refreshes_auth_and_reports_partial_failure() {
        let (base, server) = mock_server(vec![
            (
                "DELETE /api/v1/keys/90 |access-secret",
                401,
                json!({"error":"expired"}),
            ),
            (
                "POST /api/v1/auth/refresh ",
                200,
                json!({"code":0,"data":{
                    "access_token":"rotated-access", "refresh_token":"rotated-refresh"
                }}),
            ),
            (
                "DELETE /api/v1/keys/90 |rotated-access",
                200,
                json!({"code":0,"data":{}}),
            ),
            (
                "DELETE /api/v1/keys/91 |rotated-access",
                403,
                json!({"error":"private-server-detail"}),
            ),
            (
                "DELETE /api/v1/keys/92 |rotated-access",
                200,
                json!({"code":0,"data":{}}),
            ),
        ]);
        let account = PlatformAccount {
            base_url: base.clone(),
            user: serde_json::from_value(profile()["data"].clone()).unwrap(),
        };
        let state = model_session(base);
        let warnings = state
            .delete_obsolete_group_keys(&account, &[90, 91, 92])
            .await
            .unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("91"));
        assert!(!warnings[0].contains("private-server-detail"));
        assert_eq!(
            state
                .0
                .lock()
                .await
                .session
                .as_ref()
                .unwrap()
                .tokens
                .refresh_token,
            "rotated-refresh"
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn obsolete_key_cleanup_rejects_a_different_account_before_sending_requests() {
        let base = "http://127.0.0.1:1".to_string();
        let mut account = PlatformAccount {
            base_url: base.clone(),
            user: serde_json::from_value(profile()["data"].clone()).unwrap(),
        };
        let state = model_session(base);
        account.user.id = 2;
        assert!(
            matches!(state.delete_obsolete_group_keys(&account, &[90]).await,
            Err(err) if err.code == "UNAUTHORIZED")
        );
        account.user.id = 1;
        account.base_url = "http://127.0.0.1:2".into();
        assert!(
            matches!(state.delete_obsolete_group_keys(&account, &[90]).await,
            Err(err) if err.code == "UNAUTHORIZED")
        );
    }

    #[tokio::test]
    async fn discovers_available_groups_and_creates_one_named_key_per_group() {
        let (base, server) = mock_server(vec![
            ("GET /api/v1/auth/me ", 200, profile()),
            (
                "GET /api/v1/groups/available |access-secret",
                200,
                json!({"code":0,"data":[
                    {"id":42,"name":"智枢router-ccs-Claude 主力","platform":"anthropic","status":"active","subscription_type":"standard"},
                    {"id":43,"name":"原有 Claude 分组","platform":"anthropic","status":"active"},
                    {"id":44,"name":"其他-智枢router-ccs-Claude","platform":"anthropic","status":"active"},
                    {"id":45,"name":"智枢router-ccs-暂停","platform":"anthropic","status":"inactive"}
                ]}),
            ),
            ("GET /api/v1/subscriptions |access-secret", 200, json!({"code":0,"data":[]})),
            (
                "GET /api/v1/keys?page=1&page_size=100&sort_by=created_at&sort_order=desc |access-secret",
                200,
                json!({"code":0,"data":{"items":[
                    {"id":90,"key":"old-secret","name":"zhishu-ccs-anthropic-43","group_id":43,"status":"active"},
                    {"id":91,"key":"manual-secret","name":"manual-key","group_id":43,"status":"active"},
                    {"id":92,"key":"inactive-secret","name":"zhishu-ccs-anthropic-45","group_id":45,"status":"active"}
                ],"total":3,"page":1,"page_size":100,"pages":1}}),
            ),
            (
                "POST /api/v1/keys |access-secret",
                200,
                json!({"code":0,"data":{
                    "id":77,
                    "key":"group-secret",
                    "name":"zhishu-ccs-anthropic-42",
                    "group_id":42,
                    "status":"active",
                    "quota":0,
                    "quota_used":0
                }}),
            ),
            (
                "GET /v1/models |group-secret",
                200,
                json!({"data":[{"id":"claude-sonnet-5","supported_endpoint_types":["anthropic"]}]}),
            ),
            ("GET /v1/models?client_version=|group-secret", 200, json!({"models":[
                {"slug":"claude-sonnet-5","context_window":1000000,
                 "supported_reasoning_levels":[{"effort":"high"}],"default_reasoning_level":"high"}
            ]})),
        ]);
        let state = model_session(base);
        let discovery = state.discover_group_catalogs().await.unwrap();
        assert_eq!(discovery.groups.len(), 1);
        assert_eq!(discovery.groups[0].id, 42);
        assert_eq!(
            discovery
                .obsolete_keys
                .iter()
                .map(|key| key.id)
                .collect::<Vec<_>>(),
            vec![90]
        );
        assert_eq!(discovery.catalogs.len(), 1);
        assert_eq!(
            discovery.catalogs[0].credential.key_name,
            "zhishu-ccs-anthropic-42"
        );
        assert_eq!(discovery.catalogs[0].credential.key, "group-secret");
        assert_eq!(discovery.catalogs[0].models[0]["context_window"], 1000000);
        server.join().unwrap();
    }

    #[tokio::test]
    async fn revoked_group_key_never_survives_a_failed_replacement() {
        for (create_status, model_status) in [(200, 500), (200, 401), (500, 0)] {
            let mut replies = vec![
                ("GET /api/v1/auth/me ", 200, profile()),
                ("GET /api/v1/groups/available |access-secret", 200, json!({"code":0,"data":[
                    {"id":42,"name":"智枢router-ccs-Claude","platform":"anthropic","status":"active"}
                ]})),
                ("GET /api/v1/subscriptions |access-secret", 200, json!({"code":0,"data":[]})),
                ("GET /api/v1/keys?page=1&page_size=100&sort_by=created_at&sort_order=desc |access-secret", 200,
                    json!({"code":0,"data":{"items":[
                        {"id":70,"key":"revoked-key","name":"zhishu-ccs-anthropic-42","group_id":42,"status":"active"}
                    ],"pages":1}})),
                ("GET /v1/models |revoked-key", 401, json!({})),
                ("POST /api/v1/keys |access-secret", create_status, json!({"code":0,"data":{
                    "id":71,"key":"replacement-key","name":"zhishu-ccs-anthropic-42","group_id":42,"status":"active"
                }})),
            ];
            if create_status == 200 {
                replies.push(("GET /v1/models |replacement-key", model_status, json!({})));
            }
            let (base, server) = mock_server(replies);
            let discovery = model_session(base).discover_group_catalogs().await.unwrap();
            server.join().unwrap();
            assert!(discovery.failed_group_ids.contains(&42));
            assert!(!serde_json::to_string(&discovery)
                .unwrap()
                .contains("revoked-key"));
            if create_status == 200 && model_status == 500 {
                assert_eq!(discovery.catalogs.len(), 1);
                assert_eq!(discovery.catalogs[0].credential.key, "replacement-key");
                assert_eq!(discovery.catalogs[0].credential.key_id, 71);
                assert!(discovery.catalogs[0].catalog_error.is_some());
            } else {
                assert!(discovery.catalogs.is_empty());
                assert!(!discovery.warnings.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn subscription_groups_ignore_prefix_and_expired_keys_are_not_obsolete() {
        let (base, server) = mock_server(vec![
            ("GET /api/v1/auth/me ", 200, profile()),
            ("GET /api/v1/groups/available |access-secret", 200, json!({"code":0,"data":[
                {"id":14,"name":"月度订阅","platform":"anthropic","status":"active","subscription_type":"subscription"},
                {"id":43,"name":"普通分组","platform":"anthropic","status":"active","subscription_type":"standard"},
                {"id":16,"name":"已改为按量的历史分组","platform":"anthropic","status":"active","subscription_type":"standard"}
            ]})),
            ("GET /api/v1/subscriptions |access-secret", 200, json!({"code":0,"data":[
                {"id":7,"group_id":14,"expires_at":"2099-01-01T00:00:00Z","status":"active"},
                {"id":8,"group_id":15,"expires_at":"2000-01-01T00:00:00Z","status":"expired"},
                {"id":9,"group_id":16,"expires_at":"2000-01-01T00:00:00Z","status":"expired"}
            ]})),
            ("GET /api/v1/keys?page=1&page_size=100&sort_by=created_at&sort_order=desc |access-secret", 200,
                json!({"code":0,"data":{"items":[
                    {"id":70,"key":"subscription-key","name":"zhishu-ccs-anthropic-14","group_id":14,"status":"active"},
                    {"id":80,"key":"expired-subscription-key","name":"zhishu-ccs-anthropic-15","group_id":15,"status":"active"},
                    {"id":90,"key":"old-key","name":"zhishu-ccs-anthropic-43","group_id":43,"status":"active"}
                ],"pages":1}})),
            ("GET /v1/models |subscription-key", 200, json!({"data":[{"id":"claude-sonnet-5"}]})),
            ("GET /v1/models?client_version=|subscription-key", 404, json!({})),
        ]);
        let discovery = model_session(base).discover_group_catalogs().await.unwrap();
        assert_eq!(discovery.groups.len(), 1);
        assert_eq!(discovery.groups[0].id, 14);
        assert_eq!(discovery.groups[0].subscription_type, "subscription");
        assert_eq!(discovery.catalogs.len(), 1);
        assert_eq!(
            discovery.subscription_group_ids,
            HashSet::from([14, 15, 16])
        );
        assert_eq!(
            discovery
                .obsolete_keys
                .iter()
                .map(|key| key.id)
                .collect::<Vec<_>>(),
            vec![90]
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn exhausted_subscription_key_is_preserved_without_creating_a_replacement() {
        let (base, server) = mock_server(vec![
            ("GET /api/v1/auth/me ", 200, profile()),
            ("GET /api/v1/groups/available |access-secret", 200, json!({"code":0,"data":[
                {"id":14,"name":"月度订阅","platform":"anthropic","status":"active","subscription_type":"subscription"}
            ]})),
            ("GET /api/v1/subscriptions |access-secret", 200, json!({"code":0,"data":[]})),
            ("GET /api/v1/keys?page=1&page_size=100&sort_by=created_at&sort_order=desc |access-secret", 200,
                json!({"code":0,"data":{"items":[
                    {"id":70,"key":"subscription-key","name":"zhishu-ccs-anthropic-14","group_id":14,"status":"quota_exhausted","quota":10,"quota_used":10}
                ],"pages":1}})),
        ]);
        let discovery = model_session(base).discover_group_catalogs().await.unwrap();
        assert!(discovery.obsolete_keys.is_empty());
        assert!(discovery.catalogs.is_empty());
        assert!(discovery.failed_group_ids.contains(&14));
        assert_eq!(discovery.warnings.len(), 1);
        server.join().unwrap();
    }

    #[tokio::test]
    async fn failed_subscription_history_aborts_discovery_before_any_key_changes() {
        let (base, server) = mock_server(vec![
            ("GET /api/v1/auth/me ", 200, profile()),
            (
                "GET /api/v1/groups/available |access-secret",
                200,
                json!({"code":0,"data":[]}),
            ),
            (
                "GET /api/v1/subscriptions |access-secret",
                500,
                json!({"error":"private"}),
            ),
        ]);
        assert!(
            matches!(model_session(base).discover_group_catalogs().await,
            Err(err) if err.code == "SERVER")
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn subscription_usage_includes_expired_and_suspended_records_without_inventing_quota() {
        let (base, server) = mock_server(vec![
            (
                "GET /api/v1/subscriptions/progress |access-secret",
                200,
                json!({"code":0,"data":[]}),
            ),
            (
                "GET /api/v1/subscriptions |access-secret",
                200,
                json!({"code":0,"data":[
                    {"id":7,"group_id":14,"expires_at":"2000-01-01T00:00:00Z","status":"expired", "group":{"name":"月度订阅","platform":"anthropic"}},
                    {"id":8,"group_id":15,"expires_at":"2099-01-01T00:00:00Z","status":"suspended"},
                    {"id":9,"group_id":16,"expires_at":"2099-01-01T00:00:00Z","status":"active"}
                ]}),
            ),
        ]);
        let usage = subscription_usage(&model_session(base).0).await.unwrap();
        assert_eq!(usage.len(), 3);
        assert_eq!(usage[0].status.as_deref(), Some("expired"));
        assert_eq!(usage[0].group_name, "月度订阅");
        assert_eq!(usage[1].status.as_deref(), Some("suspended"));
        assert_eq!(usage[2].status.as_deref(), Some("unavailable"));
        assert!(usage.iter().all(|item| !item.unlimited
            && item.daily.is_none()
            && item.weekly.is_none()
            && item.monthly.is_none()));
        server.join().unwrap();
    }

    #[tokio::test]
    async fn reuses_an_existing_active_group_key_without_creating_another() {
        let (base, server) = mock_server(vec![
            ("GET /api/v1/auth/me ", 200, profile()),
            (
                "GET /api/v1/groups/available |access-secret",
                200,
                json!({"code":0,"data":[
                    {"id":42,"name":"智枢router-ccs-Claude 主力","platform":"anthropic","status":"active","subscription_type":"standard"}
                ]}),
            ),
            ("GET /api/v1/subscriptions |access-secret", 200, json!({"code":0,"data":[]})),
            (
                "GET /api/v1/keys?page=1&page_size=100&sort_by=created_at&sort_order=desc |access-secret",
                200,
                json!({"code":0,"data":{"items":[{
                    "id":77,
                    "key":"existing-group-secret",
                    "name":"zhishu-ccs-anthropic-42",
                    "group_id":42,
                    "status":"active",
                    "quota":0,
                    "quota_used":0
                }],"total":1,"page":1,"page_size":100,"pages":1}}),
            ),
            (
                "GET /v1/models |existing-group-secret",
                200,
                json!({"data":[{"id":"claude-sonnet-5","supported_endpoint_types":["anthropic"]}]}),
            ),
            ("GET /v1/models?client_version=|existing-group-secret", 404, json!({"error":"unsupported"})),
        ]);
        let state = model_session(base);
        let discovery = state.discover_group_catalogs().await.unwrap();
        assert_eq!(
            discovery.catalogs[0].credential.key,
            "existing-group-secret"
        );
        assert_eq!(discovery.catalogs[0].models[0]["id"], "claude-sonnet-5");
        assert!(discovery.catalogs[0].catalog_error.is_none());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn model_forbidden_keeps_the_group_for_a_local_fallback_and_does_not_create_another_key()
    {
        let (base, server) = mock_server(vec![
            ("GET /api/v1/auth/me ", 200, profile()),
            (
                "GET /api/v1/groups/available |access-secret",
                200,
                json!({"code":0,"data":[
                    {"id":42,"name":"智枢router-ccs-混合分组","platform":"composite","status":"active","subscription_type":"standard"}
                ]}),
            ),
            ("GET /api/v1/subscriptions |access-secret", 200, json!({"code":0,"data":[]})),
            (
                "GET /api/v1/keys?page=1&page_size=100&sort_by=created_at&sort_order=desc |access-secret",
                200,
                json!({"code":0,"data":{"items":[{
                    "id":77,
                    "key":"existing-group-secret",
                    "name":"zhishu-ccs-composite-42",
                    "group_id":42,
                    "status":"active",
                    "quota":0,
                    "quota_used":0
                }],"total":1,"page":1,"page_size":100,"pages":1}}),
            ),
            (
                "GET /v1/models |existing-group-secret",
                403,
                json!({"error":"forbidden"}),
            ),
        ]);
        let state = model_session(base);
        let discovery = state.discover_group_catalogs().await.unwrap();
        assert_eq!(discovery.catalogs.len(), 1);
        assert!(discovery.catalogs[0].models.is_empty());
        assert!(discovery.catalogs[0].catalog_error.is_some());
        assert!(discovery.failed_group_ids.contains(&42));
        server.join().unwrap();
    }

    #[tokio::test]
    async fn login_keeps_pending_2fa_out_of_the_renderer() {
        let (base, server) = mock_server(vec![
            (
                "GET /api/v1/settings/public ",
                200,
                json!({"code":0,"data":{}}),
            ),
            (
                "POST /api/v1/auth/login ",
                200,
                json!({"code":0,"data":{"requires_2fa":true,"temp_token":"temporary-secret"}}),
            ),
            (
                "POST /api/v1/auth/login/2fa ",
                200,
                json!({"code":0,"data":{"access_token":"access-secret","refresh_token":"refresh-secret"}}),
            ),
            ("GET /api/v1/auth/me ", 200, profile()),
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
            let (base, server) = mock_server(vec![(
                "GET /api/v1/settings/public ",
                200,
                json!({"code":0,"data":{key:true}}),
            )]);
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
            ("GET /api/v1/auth/me ", 401, json!({"message":"secret"})),
            (
                "POST /api/v1/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"new-access","refresh_token":"new-refresh"}}),
            ),
            ("GET /api/v1/auth/me ", 503, json!({})),
            ("GET /api/v1/auth/me ", 200, profile()),
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
            ("GET /api/v1/auth/me ", 401, json!({})),
            ("POST /api/v1/auth/refresh ", 401, json!({})),
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
            "POST /api/v1/auth/logout ",
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
                "POST /api/v1/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"access-one","refresh_token":"refresh-one"}}),
            ),
            ("GET /api/v1/auth/me |access-one", 200, profile()),
            (
                "POST /api/v1/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"access-two","refresh_token":"refresh-two"}}),
            ),
            ("GET /api/v1/auth/me |access-two", 200, profile()),
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
            ("POST /api/v1/auth/refresh ", 503, json!({})),
            (
                "POST /api/v1/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"new-access","refresh_token":"new-refresh"}}),
            ),
            ("GET /api/v1/auth/me ", 503, json!({})),
            ("GET /api/v1/auth/me ", 200, profile()),
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
                "POST /api/v1/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"access","refresh_token":"refresh"}}),
            ),
            ("GET /api/v1/auth/me ", 200, profile()),
            ("GET /api/v1/auth/me ", 200, profile()),
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
        let (base, server) = mock_server(vec![("POST /api/v1/auth/refresh ", 401, json!({}))]);
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
                    "POST /api/v1/auth/refresh ",
                    200,
                    json!({"code":0,"data":{"access_token":"access","refresh_token":"refresh"}}),
                ),
                ("GET /api/v1/auth/me ", 200, profile()),
                (
                    "POST /api/v1/auth/logout ",
                    status,
                    json!({"code":0,"data":{}}),
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
                "POST /api/v1/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"access","refresh_token":"refresh"}}),
            ),
            (
                "GET /api/v1/auth/me ",
                200,
                json!({"code":0,"data":{"id":2,"email":"other@example.com","role":"user","balance":0}}),
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
            ("GET /api/v1/auth/me ", 401, json!({})),
            (
                "POST /api/v1/auth/refresh ",
                200,
                json!({"code":0,"data":{"access_token":"rotated-access","refresh_token":"rotated-refresh"}}),
            ),
            ("GET /api/v1/auth/me |rotated-access", 200, profile()),
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
        let (base, server) = mock_server(vec![("GET /api/v1/auth/me ", 200, profile())]);
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
            "GET /api/v1/auth/me ",
            200,
            json!({"code":0,"data":{"id":2,"email":"other@example.com","role":"user","balance":0}}),
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
}
