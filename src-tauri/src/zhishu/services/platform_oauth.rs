//! 平台 OAuth 2.0 授权码登录：RFC 8252 回环重定向 + RFC 7636 PKCE(S256)。
//! 授权码、verifier 与令牌只停留在 Rust 侧；本地回调只接受 state 匹配的 GET /callback。
use super::client_api::ClientConfig;
use super::platform_auth::{client, client_config, error, normalize_base_url, PlatformAuthError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use reqwest::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use url::Url;

const OAUTH_PREFIX: &str = "/api/zhishu/v1/oauth";
const CALLBACK_PATH: &str = "/callback";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REQUEST_BYTES: usize = 8 * 1024;
const DEFAULT_SCOPE: &str = "client";

const SUCCESS_PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>登录成功</title><p>授权已完成，请返回智枢客户端。</p>";
const DENIED_PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>授权已取消</title><p>你已拒绝本次授权，可以关闭此页面并返回客户端重试。</p>";
const INVALID_PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>授权回调无效</title><p>授权回调无效或已过期，请返回客户端重新登录。</p>";

pub(crate) struct OAuthAttempt {
    pub(crate) authorize_url: String,
    redirect_uri: String,
    state: String,
    verifier: String,
    /// 平台成功页；下发时回环回调改为 302 顶层跳转，避免 Edge LNA 拦截 iframe。
    success_url: Option<String>,
    listener: TcpListener,
}

impl OAuthAttempt {
    /// 校验平台配置并准备本地回环地址与 PKCE 参数；浏览器此时尚未打开。
    pub(crate) async fn prepare(config: &ClientConfig) -> Result<Self, PlatformAuthError> {
        if !config.login_methods.iter().any(|method| method == "oauth") {
            return Err(error("LOGIN_UNSUPPORTED", "平台未启用浏览器授权登录"));
        }
        let authorize_url = config
            .oauth_authorize_url
            .as_deref()
            .ok_or_else(|| error("LOGIN_UNSUPPORTED", "平台未提供授权地址"))?;
        if config.oauth_client_id.is_empty() {
            return Err(error("LOGIN_UNSUPPORTED", "平台未提供授权客户端标识"));
        }
        let mut url = Url::parse(authorize_url)
            .map_err(|_| error("LOGIN_UNSUPPORTED", "平台授权地址无效"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(error("LOGIN_UNSUPPORTED", "平台授权地址无效"));
        }
        let success_url = config.oauth_success_url.clone();
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|_| error("OAUTH_UNAVAILABLE", "无法启动本地授权回调端口"))?;
        let port = listener
            .local_addr()
            .map_err(|_| error("OAUTH_UNAVAILABLE", "无法读取本地授权回调端口"))?
            .port();
        // 服务端只接受带显式端口的 http 回环地址（RFC 8252）。
        let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
        let verifier = random_verifier();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = random_state();
        let scope = if config.oauth_scopes.is_empty() {
            DEFAULT_SCOPE.to_string()
        } else {
            config.oauth_scopes.join(" ")
        };
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &config.oauth_client_id)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &state)
            .append_pair("scope", &scope);
        Ok(Self {
            authorize_url: url.to_string(),
            redirect_uri,
            state,
            verifier,
            success_url,
            listener,
        })
    }

    /// 等待浏览器回跳；非回调请求和 state 不匹配的请求会被忽略并继续等待。
    pub(crate) async fn wait_for_callback(
        &mut self,
        cancel: oneshot::Receiver<()>,
    ) -> Result<String, PlatformAuthError> {
        let deadline = tokio::time::Instant::now() + CALLBACK_TIMEOUT;
        tokio::pin!(cancel);
        loop {
            tokio::select! {
                _ = &mut cancel => {
                    return Err(error("OAUTH_CANCELLED", "已取消授权登录"));
                }
                _ = tokio::time::sleep_until(deadline) => {
                    return Err(error("OAUTH_TIMEOUT", "授权等待超时，请重新登录"));
                }
                accepted = self.listener.accept() => {
                    let (mut stream, _) = accepted
                        .map_err(|_| error("OAUTH_UNAVAILABLE", "无法接收授权回调"))?;
                    if let Some(result) =
                        read_callback(&mut stream, &self.state, self.success_url.as_deref()).await
                    {
                        return result;
                    }
                }
            }
        }
    }
}

/// 完整授权：读取平台配置、打开浏览器、等待回调并兑换令牌对。
pub(crate) async fn login(
    handle: &tauri::AppHandle,
    base_url: String,
    cancel: oneshot::Receiver<()>,
) -> Result<(String, Value), PlatformAuthError> {
    let base = normalize_base_url(&base_url)?;
    let http = client()?;
    let config = client_config(&http, &base).await?;
    let mut attempt = OAuthAttempt::prepare(&config).await?;
    handle
        .opener()
        .open_url(&attempt.authorize_url, None::<String>)
        .map_err(|_| error("OAUTH_BROWSER", "无法打开浏览器，请检查系统默认浏览器设置"))?;
    let code = attempt.wait_for_callback(cancel).await?;
    let tokens = exchange_code(
        &http,
        &base,
        &config,
        &code,
        &attempt.redirect_uri,
        &attempt.verifier,
    )
    .await?;
    Ok((base, tokens))
}

async fn exchange_code(
    http: &Client,
    base: &str,
    config: &ClientConfig,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<Value, PlatformAuthError> {
    let builder = http
        .post(format!("{base}{OAUTH_PREFIX}/token"))
        .json(&json!({
            "grant_type": "authorization_code",
            "client_id": config.oauth_client_id,
            "code": code,
            "redirect_uri": redirect_uri,
            "code_verifier": verifier,
        }));
    let response = super::platform_http_log::send_json(builder)
        .await
        .map_err(|_| error("NETWORK", "无法连接平台，请检查地址、网络或证书后重试"))?;
    let status = response.status;
    if !status.is_success() {
        let (code, message) = match status {
            reqwest::StatusCode::BAD_REQUEST => {
                ("OAUTH_INVALID_GRANT", "授权码无效或已过期，请重新授权")
            }
            reqwest::StatusCode::FORBIDDEN => ("FORBIDDEN", "账户暂不可用或平台禁止本次登录"),
            reqwest::StatusCode::TOO_MANY_REQUESTS => ("RATE_LIMITED", "操作过于频繁，请稍后重试"),
            _ => ("SERVER", "平台请求失败，请稍后重试"),
        };
        return Err(error(code, message));
    }
    let envelope: Envelope = response
        .body
        .and_then(|body| serde_json::from_value(body).map_err(|_| ()))
        .map_err(|_| {
            error(
                "INVALID_RESPONSE",
                "平台响应格式无效，请确认填写的是后端地址",
            )
        })?;
    if envelope.code != 0 {
        return Err(error("OAUTH_FAILED", "平台未能完成授权，请重试"));
    }
    envelope
        .data
        .ok_or_else(|| error("INVALID_RESPONSE", "平台响应缺少数据"))
}

#[derive(serde::Deserialize)]
struct Envelope {
    code: i64,
    data: Option<Value>,
}

async fn read_callback(
    stream: &mut TcpStream,
    expected_state: &str,
    success_url: Option<&str>,
) -> Option<Result<String, PlatformAuthError>> {
    let mut buffer = vec![0u8; MAX_REQUEST_BYTES];
    let read = tokio::time::timeout(CONNECTION_TIMEOUT, stream.read(&mut buffer)).await;
    let count = match read {
        Ok(Ok(count)) if count > 0 => count,
        _ => {
            respond(stream, "400 Bad Request", INVALID_PAGE).await;
            return None;
        }
    };
    let request = String::from_utf8_lossy(&buffer[..count]);
    let mut parts = request.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let url = match Url::parse(&format!("http://127.0.0.1{target}")) {
        Ok(url) if method == "GET" && url.path() == CALLBACK_PATH => url,
        _ => {
            respond(stream, "404 Not Found", INVALID_PAGE).await;
            return None;
        }
    };
    let params: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();
    if params.get("state").map(String::as_str) != Some(expected_state) {
        // 伪造或串线的回调不应结束本次授权；继续等待正确回调直到超时。
        respond(stream, "400 Bad Request", INVALID_PAGE).await;
        return None;
    }
    if let Some(reason) = params.get("error") {
        respond(stream, "400 Bad Request", DENIED_PAGE).await;
        return Some(Err(match reason.as_str() {
            "access_denied" => error("OAUTH_DENIED", "已拒绝授权登录"),
            _ => error("OAUTH_FAILED", "平台未能完成授权，请重试"),
        }));
    }
    match params.get("code").filter(|code| !code.is_empty()) {
        Some(code) => {
            match success_url {
                // 主框架导航不受 LNA 限制；客户端消费 code 后把浏览器带回平台成功页。
                Some(location) => respond_redirect(stream, location).await,
                None => respond(stream, "200 OK", SUCCESS_PAGE).await,
            }
            Some(Ok(code.clone()))
        }
        None => {
            respond(stream, "400 Bad Request", INVALID_PAGE).await;
            None
        }
    }
}

async fn respond_redirect(stream: &mut TcpStream, location: &str) {
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nCache-Control: no-store\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// 64 位十六进制字符（256 bit 熵），满足 RFC 7636 的 43–128 字符限制。
fn random_verifier() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn random_state() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream as TokioTcpStream;

    fn config(authorize: &str) -> ClientConfig {
        ClientConfig {
            schema_version: 1,
            site_name: "Test".into(),
            api_base_url: "https://gateway.test".into(),
            login_methods: vec!["oauth".into()],
            subscriptions_url: None,
            oauth_authorize_url: Some(authorize.into()),
            oauth_client_id: "zhishu-ccs".into(),
            oauth_scopes: vec!["client".into()],
            oauth_success_url: None,
        }
    }

    #[test]
    fn pkce_challenge_matches_rfc_7636_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[tokio::test]
    async fn prepare_builds_authorize_url_with_loopback_and_pkce() {
        let attempt = OAuthAttempt::prepare(&config("https://portal.test/oauth/authorize"))
            .await
            .unwrap();
        let url = Url::parse(&attempt.authorize_url).unwrap();
        assert_eq!(url.path(), "/oauth/authorize");
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["response_type"], "code");
        assert_eq!(params["client_id"], "zhishu-ccs");
        assert_eq!(params["code_challenge_method"], "S256");
        assert_eq!(params["scope"], "client");
        assert_eq!(params["state"], attempt.state);
        assert!(params["redirect_uri"].starts_with("http://127.0.0.1:"));
        assert!(params["redirect_uri"].ends_with("/callback"));
        assert_eq!(params["code_challenge"].len(), 43);
        assert!((43..=128).contains(&attempt.verifier.len()));
    }

    #[tokio::test]
    async fn prepare_rejects_disabled_oauth() {
        let mut cfg = config("https://portal.test/oauth/authorize");
        cfg.login_methods.clear();
        assert!(OAuthAttempt::prepare(&cfg).await.is_err());
        cfg.login_methods = vec!["oauth".into()];
        cfg.oauth_authorize_url = None;
        assert!(OAuthAttempt::prepare(&cfg).await.is_err());
    }

    #[tokio::test]
    async fn callback_ignores_wrong_state_then_returns_code() {
        let mut attempt = OAuthAttempt::prepare(&config("https://portal.test/oauth/authorize"))
            .await
            .unwrap();
        let port = attempt.listener.local_addr().unwrap().port();
        let state = attempt.state.clone();
        let (_cancel, receiver) = oneshot::channel();
        let browser = tokio::spawn(async move {
            for target in [
                format!("http://127.0.0.1:{port}/callback?code=bad&state=wrong"),
                format!("http://127.0.0.1:{port}/callback?code=good&state={state}"),
            ] {
                let mut stream = TokioTcpStream::connect(("127.0.0.1", port)).await.unwrap();
                let url = Url::parse(&target).unwrap();
                let request = format!(
                    "GET {}?{} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                    url.path(),
                    url.query().unwrap_or("")
                );
                stream.write_all(request.as_bytes()).await.unwrap();
                let mut response = String::new();
                let _ = stream.read_to_string(&mut response).await;
            }
        });
        let code = attempt.wait_for_callback(receiver).await.unwrap();
        assert_eq!(code, "good");
        browser.await.unwrap();
    }

    #[tokio::test]
    async fn callback_redirects_to_platform_success_page_when_configured() {
        let mut cfg = config("https://portal.test/oauth/authorize");
        cfg.oauth_success_url = Some("https://portal.test/oauth/authorized".into());
        let mut attempt = OAuthAttempt::prepare(&cfg).await.unwrap();
        let port = attempt.listener.local_addr().unwrap().port();
        let state = attempt.state.clone();
        let (_cancel, receiver) = oneshot::channel();
        let browser = tokio::spawn(async move {
            let mut stream = TokioTcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let request = format!(
                "GET /callback?code=good&state={state} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"
            );
            stream.write_all(request.as_bytes()).await.unwrap();
            let mut response = String::new();
            let _ = stream.read_to_string(&mut response).await;
            response
        });
        let code = attempt.wait_for_callback(receiver).await.unwrap();
        assert_eq!(code, "good");
        let response = browser.await.unwrap();
        assert!(response.starts_with("HTTP/1.1 302 Found"), "{response}");
        assert!(
            response.contains("Location: https://portal.test/oauth/authorized"),
            "{response}"
        );
    }

    #[tokio::test]
    async fn callback_reports_denied_authorization() {
        let mut attempt = OAuthAttempt::prepare(&config("https://portal.test/oauth/authorize"))
            .await
            .unwrap();
        let port = attempt.listener.local_addr().unwrap().port();
        let state = attempt.state.clone();
        let (_cancel, receiver) = oneshot::channel();
        tokio::spawn(async move {
            let mut stream = TokioTcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let request =
                format!("GET /callback?error=access_denied&state={state} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
            stream.write_all(request.as_bytes()).await.unwrap();
            let mut response = String::new();
            let _ = stream.read_to_string(&mut response).await;
        });
        let failure = attempt.wait_for_callback(receiver).await.unwrap_err();
        assert_eq!(failure.code, "OAUTH_DENIED");
    }

    fn token_server(status_code: u16, body: Value) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
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
                        .map(|value| value.parse().unwrap())
                        .unwrap_or(0);
                    if body.len() >= length {
                        break;
                    }
                }
            }
            let response_body = body.to_string();
            write!(stream, "HTTP/1.1 {status_code} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}", response_body.len()).unwrap();
            String::from_utf8(request).unwrap()
        });
        (base, handle)
    }

    #[tokio::test]
    async fn exchanges_code_at_oauth_token_endpoint() {
        let (base, server) = token_server(
            200,
            json!({"code":0,"data":{"access_token":"access","refresh_token":"refresh","expires_in":3600,"token_type":"Bearer","scope":"client"}}),
        );
        let tokens = exchange_code(
            &client().unwrap(),
            &base,
            &config("https://portal.test/oauth/authorize"),
            "auth-code",
            "http://127.0.0.1:12345/callback",
            "verifier",
        )
        .await
        .unwrap();
        assert_eq!(tokens["refresh_token"], "refresh");
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /api/zhishu/v1/oauth/token "));
        assert!(request.contains("\"grant_type\":\"authorization_code\""));
        assert!(request.contains("\"code_verifier\":\"verifier\""));
    }

    #[tokio::test]
    async fn invalid_grant_is_reported_without_leaking_details() {
        let (base, server) = token_server(
            400,
            json!({"code":400,"message":"authorization code is invalid or expired"}),
        );
        let failure = exchange_code(
            &client().unwrap(),
            &base,
            &config("https://portal.test/oauth/authorize"),
            "bad-code",
            "http://127.0.0.1:12345/callback",
            "verifier",
        )
        .await
        .unwrap_err();
        assert_eq!(failure.code, "OAUTH_INVALID_GRANT");
        assert!(!failure.message.contains("invalid or expired"));
        server.join().unwrap();
    }
}
