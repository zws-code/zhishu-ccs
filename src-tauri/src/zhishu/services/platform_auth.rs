//! 平台 OAuth2 登录与账户恢复。
//! access token 仅驻内存，refresh token 交给系统凭据库保存。
use super::client_api::{self, ClientAccount, ClientConfig};
use reqwest::{Client, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{Manager, State};
use tokio::sync::{oneshot, Mutex};
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

struct Session {
    base_url: String,
    user_id: i64,
    tokens: TokenPair,
    persistence: Option<(SessionStore, SessionRecord)>,
    // 令牌已轮换但尚未可靠保存时保持为 true，下次认证请求先重试持久化。
    persistence_dirty: bool,
    last_used_at: i64,
}

#[derive(Default)]
pub struct PlatformAuthState(Mutex<AuthState>);

#[derive(Default)]
struct AuthState {
    session: Option<Session>,
    store: Option<SessionStore>,
    restoration_checked: bool,
    // 浏览器授权等待期间持有；取消登录或流程结束时清除。
    oauth_cancel: Option<oneshot::Sender<()>>,
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
}

pub(crate) fn normalize_base_url(input: &str) -> Result<String, PlatformAuthError> {
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
        ("GET", "/config" | "/account") | ("POST", "/auth/refresh" | "/auth/logout")
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
pub async fn platform_cancel_login(
    state: State<'_, PlatformAuthState>,
) -> Result<(), PlatformAuthError> {
    let mut auth = state.0.lock().await;
    // 浏览器授权等待由独立通道取消；没有进行中的授权时为空操作。
    if let Some(cancel) = auth.oauth_cancel.take() {
        let _ = cancel.send(());
    }
    Ok(())
}

/// 浏览器授权登录：配置校验、打开系统浏览器、等待回环回调并兑换令牌对。
#[tauri::command]
pub async fn platform_login_oauth(
    handle: tauri::AppHandle,
    state: State<'_, PlatformAuthState>,
    base_url: String,
) -> Result<LoginResult, PlatformAuthError> {
    let store_result = initialize_session_store(&state.0, &handle).await;
    let (cancel, receiver) = oneshot::channel();
    state.0.lock().await.oauth_cancel = Some(cancel);
    let outcome = super::platform_oauth::login(&handle, base_url, receiver).await;
    let mut auth = state.0.lock().await;
    auth.oauth_cancel = None;
    let (base, tokens) = outcome?;
    let client = client()?;
    let mut result = finish_login(&mut auth, &client, base, tokens).await?;
    if let Err(err) = store_result {
        let LoginResult::Authenticated {
            persistence_warning,
            ..
        } = &mut result;
        *persistence_warning = Some(err.message);
    }
    Ok(result)
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

pub(crate) async fn client_config(
    client: &Client,
    base: &str,
) -> Result<ClientConfig, PlatformAuthError> {
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
    let user = user.into_user()?;
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
    let (local, remote) = logout_results(state, false).await;
    local.and(remote)
}

async fn logout_results(
    state: &Mutex<AuthState>,
    complete: bool,
) -> (Result<(), PlatformAuthError>, Result<(), PlatformAuthError>) {
    let mut state = state.lock().await;
    state.restoration_checked = true;
    let local = if let Some(store) = &state.store {
        if complete {
            store.clear_completely().await
        } else {
            store.clear().await
        }
    } else {
        Ok(())
    };
    // 无论本机凭据清理或服务端撤销是否失败，都丢弃内存会话；分别报告两种失败。
    let session = state.session.take();
    let remote = async {
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
        Ok(())
    }
    .await;
    (local, remote)
}

impl PlatformAuthState {
    pub(crate) async fn logout(&self) -> Result<(), PlatformAuthError> {
        // 退出只清理登录会话；保留本机目录、供应商和工具配置中的 API Key。
        logout(&self.0).await
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
            "login_methods":["oauth"],"subscriptions_url":"/subscriptions"
        }})
    }

    fn profile() -> Value {
        json!({"code":0,"data":{"id":1,"email":"test@example.com","username":"test","status":"active","balance_usd":2.0}})
    }

    #[tokio::test]
    async fn login_uses_and_persists_only_the_normalized_platform_base_url() {
        let (base, server) = mock_server(vec![(
            "GET /sub/api/zhishu/v1/client/account |access-secret",
            200,
            profile(),
        )]);
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
        let result = finish_login(
            &mut *state.lock().await,
            &client().unwrap(),
            normalize_base_url(input.as_str()).unwrap(),
            json!({"access_token":"access-secret","refresh_token":"refresh-secret"}),
        )
        .await
        .unwrap();
        let LoginResult::Authenticated { account, .. } = result;
        assert_eq!(account.base_url, expected_base);
        assert_eq!(
            store.load().await.unwrap().unwrap().0.base_url,
            expected_base
        );
        server.join().unwrap();
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
            assert_eq!(state.logout().await.is_ok(), status == 200);
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
    async fn successful_oauth_login_replaces_the_saved_account() {
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
            (Method::POST, "/auth/login"),
            (Method::POST, "/auth/login/2fa"),
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
    fn config_validates_subscription_and_oauth_links() {
        let mut value = config_response()["data"].clone();
        value["subscriptions_url"] = json!("javascript:alert(1)");
        value["oauth_authorize_url"] = json!("https://portal.test/oauth/authorize");
        value["oauth_client_id"] = json!(" zhishu-ccs ");
        value["oauth_scopes"] = json!([" client ", ""]);
        let config: ClientConfig = serde_json::from_value(value).unwrap();
        let config = config.validate("https://platform.test/sub").unwrap();
        assert!(config.subscriptions_url.is_none());
        assert_eq!(
            config.oauth_authorize_url.as_deref(),
            Some("https://portal.test/oauth/authorize")
        );
        assert_eq!(config.oauth_client_id, "zhishu-ccs");
        assert_eq!(config.oauth_scopes, vec!["client"]);
    }

    #[test]
    fn legacy_config_without_oauth_fields_still_parses() {
        let config: ClientConfig =
            serde_json::from_value(config_response()["data"].clone()).unwrap();
        let config = config.validate("https://platform.test").unwrap();
        assert!(config.oauth_authorize_url.is_none());
        assert!(config.oauth_client_id.is_empty());
        assert!(config.oauth_scopes.is_empty());
    }
}
