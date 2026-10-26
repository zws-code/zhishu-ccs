//! 平台 HTTP 脱敏诊断：仅记录可解析的安全摘要，先脱敏再截断。
//! 不记录认证头或未解析的原始响应，错误页与凭据回显也不能绕过脱敏。
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const BODY_LOG_LIMIT: usize = 32 * 1024;
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub(super) struct RequestLog {
    id: u64,
    method: Method,
    url: String,
    started: Instant,
    secrets: Vec<String>,
}

fn sensitive_field(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect();
    normalized.ends_with("key")
        || normalized.ends_with("token")
        || normalized == "tokens"
        || normalized.contains("password")
        || normalized.contains("secret")
        || normalized.contains("authorization")
        || normalized.contains("cookie")
        || matches!(
            normalized.as_str(),
            "keys" | "apikeys" | "totpcode" | "otp" | "credentials"
        )
}

fn collect_strings(value: &Value, secrets: &mut Vec<String>) {
    match value {
        Value::String(text) if !text.is_empty() => secrets.push(text.clone()),
        Value::Object(fields) => {
            for value in fields.values() {
                collect_strings(value, secrets);
            }
        }
        Value::Array(items) => {
            for value in items {
                collect_strings(value, secrets);
            }
        }
        _ => {}
    }
}

fn collect_secrets(value: &Value, secrets: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (name, value) in fields {
                if sensitive_field(name) {
                    collect_strings(value, secrets);
                } else {
                    collect_secrets(value, secrets);
                }
            }
        }
        Value::Array(items) => {
            for value in items {
                collect_secrets(value, secrets);
            }
        }
        _ => {}
    }
}

fn redact(value: &Value, secrets: &[String]) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(name, value)| {
                    (
                        crate::redact_known_secrets_strict(name, secrets),
                        if sensitive_field(name) {
                            Value::String("[REDACTED]".into())
                        } else {
                            redact(value, secrets)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.iter().map(|value| redact(value, secrets)).collect())
        }
        Value::String(text) => Value::String(crate::redact_known_secrets_strict(text, secrets)),
        _ => value.clone(),
    }
}

fn body_for_log(value: &Value, known_secrets: &[String]) -> String {
    // Collect first so credentials echoed in another field are also hidden,
    // regardless of JSON field order. Redact before truncating or escaping.
    let mut secrets = known_secrets.to_vec();
    collect_secrets(value, &mut secrets);
    let mut output = redact(value, &secrets).to_string();
    if output.len() > BODY_LOG_LIMIT {
        let mut end = BODY_LOG_LIMIT;
        while !output.is_char_boundary(end) {
            end -= 1;
        }
        output.truncate(end);
        output.push_str("…[truncated]");
    }
    output
}

fn request_parameters(request: &reqwest::Request) -> Value {
    let mut query = serde_json::Map::new();
    for (name, value) in request.url().query_pairs() {
        let value = Value::String(value.into_owned());
        match query.get_mut(name.as_ref()) {
            Some(Value::Array(values)) => values.push(value),
            Some(previous) => {
                let first = previous.take();
                *previous = Value::Array(vec![first, value]);
            }
            None => {
                query.insert(name.into_owned(), value);
            }
        }
    }
    // Inspect buffered JSON without consuming or changing the outgoing body.
    let body = match request.body() {
        None => Value::Null,
        Some(body) => match body.as_bytes() {
            Some(bytes) => serde_json::from_slice(bytes).unwrap_or_else(|_| {
                Value::String(format!(
                    "[non-JSON request body omitted; {} bytes]",
                    bytes.len()
                ))
            }),
            None => Value::String("[streaming request body omitted]".into()),
        },
    };
    serde_json::json!({"query": query, "body": body})
}

impl RequestLog {
    pub(super) async fn send(builder: RequestBuilder) -> Result<(Self, Response), reqwest::Error> {
        let (client, request) = builder.build_split();
        let request = request.inspect_err(|_| {
            log::debug!("[platform-http] request build failed");
        })?;
        let mut secrets = Vec::new();
        // Headers are inspected only to scrub credentials echoed by the server.
        for name in ["authorization", "x-api-key", "x-goog-api-key"] {
            if let Some(value) = request
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
            {
                secrets.push(value.strip_prefix("Bearer ").unwrap_or(value).to_string());
            }
        }
        let parameters = request_parameters(&request);
        collect_secrets(&parameters, &mut secrets);
        let log = Self {
            id: NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
            method: request.method().clone(),
            url: crate::url_for_log_with_secrets(request.url().as_str(), &secrets).to_string(),
            started: Instant::now(),
            secrets,
        };
        log::debug!(
            "[platform-http #{}] --> {} {} query={} body={}",
            log.id,
            log.method,
            log.url,
            body_for_log(&parameters["query"], &log.secrets),
            body_for_log(&parameters["body"], &log.secrets)
        );
        let response = client.execute(request).await.inspect_err(|err| {
            // reqwest's Display may include URLs and credentials.
            log.failure(if err.is_timeout() {
                "timeout"
            } else if err.is_connect() {
                "connect"
            } else {
                "send"
            });
        })?;
        Ok((log, response))
    }

    pub(super) fn failure(&self, stage: &str) {
        log::debug!(
            "[platform-http #{}] {} {} failed stage={} elapsed_ms={}",
            self.id,
            self.method,
            self.url,
            stage,
            self.started.elapsed().as_millis()
        );
    }

    fn response(&self, status: StatusCode, body: &str) {
        log::debug!(
            "[platform-http #{}] <-- {} {} status={} elapsed_ms={} body={}",
            self.id,
            self.method,
            self.url,
            status.as_u16(),
            self.started.elapsed().as_millis(),
            body
        );
    }

    fn json_result(&self, status: StatusCode, value: &Value) {
        if log::log_enabled!(log::Level::Debug) {
            self.response(status, &body_for_log(value, &self.secrets));
        }
    }
}

pub(super) struct JsonResponse {
    pub status: StatusCode,
    pub body: Result<Value, ()>,
}

pub(super) async fn send_json(builder: RequestBuilder) -> Result<JsonResponse, reqwest::Error> {
    let (log, response) = RequestLog::send(builder).await?;
    let status = response.status();
    // Keep HTTP status even if an error response has a broken/invalid body.
    let body = match response.bytes().await {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) => {
                log.json_result(status, &value);
                Ok(value)
            }
            Err(err) => {
                log.response(
                    status,
                    &format!("[non-JSON body omitted; {} bytes]", bytes.len()),
                );
                log::debug!(
                    "[platform-http #{}] invalid JSON line={} column={}",
                    log.id,
                    err.line(),
                    err.column()
                );
                Err(())
            }
        },
        Err(_) => {
            log.response(status, "[body read failed]");
            log.failure("read_body");
            Err(())
        }
    };
    Ok(JsonResponse { status, body })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_parameters_preserve_queries_and_json_without_leaking_credentials() {
        let body = json!({"email": "test@example.com", "password": "login-password", "group_id": 42,
            "note": "query-secret header-secret", "totp_code": "123456"});
        let request = reqwest::Client::new()
            .post("https://example.com/api/v1/keys")
            .query(&[
                ("page", "2"),
                ("tag", "first"),
                ("tag", "中文"),
                ("access_token", "query-secret"),
            ])
            .bearer_auth("header-secret")
            .json(&body)
            .build()
            .unwrap();
        let parameters = request_parameters(&request);
        let output = body_for_log(&parameters, &["header-secret".into()]);
        for secret in ["login-password", "query-secret", "header-secret", "123456"] {
            assert!(!output.contains(secret));
        }
        let logged: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(logged["query"]["page"], "2");
        assert_eq!(logged["query"]["tag"], json!(["first", "中文"]));
        assert_eq!(logged["body"]["group_id"], 42);
        assert_eq!(logged["body"]["email"], "test@example.com");
        assert_eq!(
            serde_json::from_slice::<Value>(request.body().unwrap().as_bytes().unwrap()).unwrap(),
            body
        );
    }

    #[test]
    fn request_parameters_handle_empty_and_non_json_bodies() {
        let client = reqwest::Client::new();
        let empty = client
            .get("https://example.com/api/v1/auth/me")
            .build()
            .unwrap();
        assert_eq!(
            request_parameters(&empty),
            json!({"query": {}, "body": null})
        );
        let invalid = client
            .post("https://example.com/api/v1/auth/login")
            .body("password=raw-secret")
            .build()
            .unwrap();
        let output = body_for_log(&request_parameters(&invalid), &[]);
        assert!(!output.contains("raw-secret"));
        assert!(output.contains("non-JSON request body omitted"));
    }

    #[test]
    fn preserves_business_data_and_redacts_nested_credentials_and_echoes() {
        let value = json!({
            "code": 0,
            "message": "use new-access or short",
            "data": {
                "access_token": "new-access", "refreshToken": "new-refresh",
                "temp_token": "temporary", "password": "short", "totp_code": "123456",
                "items": [{"key": "group-key", "key_id": 42, "group_id": 3, "name": "test"}],
                "nested": {"apiKey": "another-key", "Authorization": "Bearer hidden"},
                "balance": 1.5, "models": [{"id": "gpt-test", "max_output_tokens": 512}]
            }
        });
        let output = body_for_log(&value, &[]);
        for secret in [
            "new-access",
            "new-refresh",
            "temporary",
            "short",
            "123456",
            "group-key",
            "another-key",
            "Bearer hidden",
        ] {
            assert!(!output.contains(secret));
        }
        let logged: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(logged["code"], 0);
        assert_eq!(logged["data"]["items"][0]["key_id"], 42);
        assert_eq!(logged["data"]["balance"], 1.5);
        assert_eq!(logged["data"]["models"][0]["id"], "gpt-test");
        assert_eq!(logged["data"]["models"][0]["max_output_tokens"], 512);
        assert_eq!(value["data"]["access_token"], "new-access");
    }

    #[test]
    fn scrubs_known_request_credentials_from_errors_and_field_names() {
        let output = body_for_log(
            &json!({"message": "invalid pw\nBearer login-token", "login-token": "echo"}),
            &["pw".into(), "login-token".into()],
        );
        assert!(!output.contains("pw"));
        assert!(!output.contains("login-token"));
        assert!(!output.contains('\n'));
    }

    #[test]
    fn truncates_utf8_after_redaction() {
        let output = body_for_log(
            &json!({"data": "中".repeat(BODY_LOG_LIMIT), "key": "hidden-key"}),
            &[],
        );
        assert!(output.ends_with("…[truncated]"));
        assert!(output.len() < BODY_LOG_LIMIT + 32);
        assert!(!output.contains("hidden-key"));
    }

    #[tokio::test]
    async fn logs_without_changing_success_error_or_invalid_responses() {
        use axum::{routing::get, Json, Router};
        let app = Router::new()
            .route(
                "/success",
                get(|| async {
                    Json(json!({"code": 0, "data": {"key": "model-secret", "id": 42}}))
                }),
            )
            .route(
                "/forbidden",
                get(|| async {
                    (
                        StatusCode::FORBIDDEN,
                        Json(json!({"code": 403, "message": "denied"})),
                    )
                }),
            )
            .route(
                "/invalid",
                get(|| async { "<html>secret: do not log</html>" }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::new();
        let response = send_json(client.get(format!("{base}/success")))
            .await
            .unwrap();
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.body.unwrap()["data"]["key"], "model-secret");
        let response = send_json(client.get(format!("{base}/forbidden")))
            .await
            .unwrap();
        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert_eq!(response.body.unwrap()["code"], 403);
        let response = send_json(client.get(format!("{base}/invalid")))
            .await
            .unwrap();
        assert_eq!(response.status, StatusCode::OK);
        assert!(response.body.is_err());
        server.abort();
    }
}
