//! 显式清理当前账号的本机数据后退出。只撤销本次刷新会话，不管理共享模型 Key。
use super::platform_auth::{error, PlatformAuthError, PlatformAuthState};
use super::platform_models::{base_data::CatalogStore, delete, APPLY_LOCK};
use crate::store::AppState;
use serde::Serialize;
use std::future::Future;
use tauri::{Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupStep {
    pub completed: bool,
    pub error: Option<PlatformAuthError>,
}
impl CleanupStep {
    fn from_result(result: Result<(), PlatformAuthError>) -> Self {
        Self {
            completed: result.is_ok(),
            error: result.err(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCleanupReport {
    pub base_url: String,
    pub user_id: i64,
    pub email: String,
    pub providers: Option<delete::PlatformDeleteSummary>,
    pub provider_error: Option<PlatformAuthError>,
    pub catalog: CleanupStep,
    pub session: CleanupStep,
    pub server_logout: CleanupStep,
}

struct LocalCleanup {
    providers: Result<delete::PlatformDeleteSummary, PlatformAuthError>,
    catalog: Result<(), PlatformAuthError>,
}

/// 锁覆盖身份检查、全部本机操作和退出；身份不匹配时无副作用。
/// 一旦验证通过，各项失败都进入报告，仍然尝试其余清理和退出。
async fn clear_and_logout<F, Fut>(
    auth: &PlatformAuthState,
    base_url: &str,
    user_id: i64,
    clear: F,
) -> Result<PlatformCleanupReport, PlatformAuthError>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = LocalCleanup>,
{
    let _operation = APPLY_LOCK.lock().await;
    let email = auth.cleanup_email(base_url, user_id).await?;
    let local = clear(email.clone()).await;
    let (session, server_logout) = auth.logout_after_cleanup().await;
    let (providers, provider_error) = match local.providers {
        Ok(result) => (Some(result), None),
        Err(err) => (None, Some(err)),
    };
    Ok(PlatformCleanupReport {
        base_url: base_url.into(),
        user_id,
        email,
        providers,
        provider_error,
        catalog: CleanupStep::from_result(local.catalog),
        session: CleanupStep::from_result(session),
        server_logout: CleanupStep::from_result(server_logout),
    })
}

#[tauri::command]
pub async fn platform_logout_and_clear_account(
    handle: tauri::AppHandle,
    auth: State<'_, PlatformAuthState>,
    base_url: String,
    user_id: i64,
) -> Result<PlatformCleanupReport, PlatformAuthError> {
    let origin = base_url.clone();
    clear_and_logout(auth.inner(), &base_url, user_id, move |_| async move {
        let worker_handle = handle.clone();
        let worker_origin = origin.clone();
        let results = tauri::async_runtime::spawn_blocking(move || {
            let state = worker_handle.state::<AppState>();
            let providers =
                delete::delete_all_account_providers(state.inner(), &worker_origin, user_id);
            let catalog = CatalogStore::for_app(&worker_handle)
                .and_then(|store| store.remove_account(&worker_origin, user_id));
            (providers, catalog)
        })
        .await;
        let (providers, catalog) = match results {
            Ok(results) => results,
            Err(_) => (
                Err(error("CLEANUP", "供应商清理未完成，请重新登录后检查并重试")),
                Err(error(
                    "CLEANUP",
                    "基础数据清理未完成，请重新登录后检查并重试",
                )),
            ),
        };
        crate::tray::refresh_tray_menu(&handle);
        LocalCleanup { providers, catalog }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stale_identity_does_not_clean_or_logout_new_account() {
        let auth = PlatformAuthState::for_test("https://platform.test".into(), 2);
        let result = clear_and_logout(&auth, "https://platform.test", 1, |_| async {
            panic!("must not start cleanup for a stale identity")
        })
        .await;
        assert_eq!(result.err().unwrap().code, "UNAUTHORIZED");
        assert_eq!(auth.with_local_identity(|_, id| Ok(id)).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn partial_cleanup_still_logs_out_and_reports_each_failure() {
        let auth = PlatformAuthState::for_test("https://platform.test".into(), 1);
        let result = clear_and_logout(&auth, "https://platform.test", 1, |email| async move {
            assert_eq!(email, "test@example.com");
            LocalCleanup {
                providers: Err(error("CONFIG", "provider failure")),
                catalog: Err(error("CATALOG_STORE", "catalog failure")),
            }
        })
        .await
        .unwrap();
        assert!(result.providers.is_none());
        assert_eq!(result.provider_error.unwrap().code, "CONFIG");
        assert!(!result.catalog.completed);
        assert!(result.session.completed);
        assert!(result.server_logout.completed);
        assert!(auth.with_local_identity(|_, _| Ok(())).await.is_err());
    }

    #[tokio::test]
    async fn success_preserves_per_app_skips_in_report() {
        let auth = PlatformAuthState::for_test("https://platform.test".into(), 1);
        let result = clear_and_logout(&auth, "https://platform.test", 1, |_| async {
            LocalCleanup {
                providers: Ok(delete::PlatformDeleteSummary {
                    removed: 2,
                    skipped: 1,
                    warnings: vec!["no safe fallback".into()],
                    ..Default::default()
                }),
                catalog: Ok(()),
            }
        })
        .await
        .unwrap();
        assert_eq!(result.providers.unwrap().skipped, 1);
        assert!(result.catalog.completed && result.session.completed);
        assert!(auth.with_local_identity(|_, _| Ok(())).await.is_err());
    }
}
