//! Product-owned names in shared CLI directories and remote stores.
//! Legacy CC Switch names are intentionally not ownership evidence.
pub const CODEX_CATALOG: &str = "zhishu-ccs-model-catalog.json";
pub const CODEX_ROUTE: &str = "zhishu";
pub const CODEX_OFFICIAL_PROXY: &str = "zhishu-ccs-official";
pub const PROXY_TOKEN: &str = "ZHISHU_CCS_PROXY_MANAGED";
pub const LEGACY_PROXY_TOKEN: &str = "PROXY_MANAGED";
pub const SYNC_ROOT: &str = "zhishu-ccs-sync";
pub const SYNC_FORMAT: &str = "zhishu-ccs-sync";

pub fn is_proxy_token(value: &str) -> bool {
    matches!(value, PROXY_TOKEN | LEGACY_PROXY_TOKEN)
}

/// Old remote settings are preserved, but must not write to the upstream default.
pub fn validate_sync_root(root: &str) -> Result<(), crate::error::AppError> {
    if root.split('/').any(|part| part == "cc-switch-sync") {
        return Err(crate::error::AppError::localized(
            "sync.shared_namespace",
            "此同步目录与原版 CC Switch 共用。请在同步设置中选择智枢独立目录（例如 zhishu-ccs-sync）；原远端数据保持不变",
            "This sync directory is shared with CC Switch. Choose a separate directory (for example zhishu-ccs-sync) in sync settings. Existing remote data is unchanged.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_remote_root_is_rejected_without_changing_it() {
        for root in ["cc-switch-sync", "/cc-switch-sync/", "team/cc-switch-sync"] {
            assert!(validate_sync_root(root).is_err());
        }
        assert!(validate_sync_root(SYNC_ROOT).is_ok());
        assert!(validate_sync_root("team/zhishu").is_ok());
        let old_webdav: crate::settings::WebDavSyncSettings =
            serde_json::from_value(serde_json::json!({
                "baseUrl":"https://example.test", "username":"fixture"
            }))
            .unwrap();
        assert_eq!(old_webdav.remote_root, "cc-switch-sync");
        assert!(old_webdav.validate().is_err());
        assert_eq!(
            crate::settings::WebDavSyncSettings::default().remote_root,
            SYNC_ROOT
        );
        let old_s3: crate::settings::S3SyncSettings =
            serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(old_s3.remote_root, "cc-switch-sync");
    }

    #[tokio::test]
    async fn coexistence_both_uploaders_reject_upstream_before_any_put() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let writes = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let (put_count, get_count) = (writes.clone(), reads.clone());
        let app = axum::Router::new().fallback(move |request: axum::extract::Request| {
            let (put_count, get_count) = (put_count.clone(), get_count.clone());
            async move {
                if request.method() == axum::http::Method::PUT {
                    put_count.fetch_add(1, Ordering::SeqCst);
                }
                if request.method().as_str() == "MKCOL" {
                    return (axum::http::StatusCode::CREATED, String::new());
                }
                if request.method() == axum::http::Method::GET && request.uri().path().ends_with("manifest.json") {
                    get_count.fetch_add(1, Ordering::SeqCst);
                    return (axum::http::StatusCode::OK, serde_json::json!({
                        "format":"cc-switch-webdav-sync", "version":2, "dbCompatVersion":6,
                        "deviceName":"upstream", "createdAt":"2026-11-03", "artifacts":{}, "snapshotId":"old"
                    }).to_string());
                }
                (axum::http::StatusCode::NOT_FOUND, String::new())
            }
        });
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let db = crate::database::Database::memory().unwrap();
        let mut webdav = crate::settings::WebDavSyncSettings {
            base_url: endpoint.clone(),
            username: "test".into(),
            password: "test".into(),
            ..Default::default()
        };
        assert!(crate::services::webdav_sync::upload(&db, &mut webdav)
            .await
            .is_err());
        let mut s3 = crate::settings::S3SyncSettings {
            endpoint,
            region: "test".into(),
            bucket: "test".into(),
            access_key_id: "test".into(),
            secret_access_key: "test".into(),
            ..Default::default()
        };
        assert!(crate::services::s3_sync::upload(&db, &mut s3)
            .await
            .is_err());
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        server.abort();
    }
}
