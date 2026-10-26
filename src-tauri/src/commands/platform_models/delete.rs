//! 显式删除当前账户、当前应用的托管供应商；不读取目录或访问平台 HTTP。
use super::{delete_managed_provider, local_error, sync_report, PlatformAuthError};
use crate::{app_config::AppType, store::AppState};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformDeleteSummary {
    pub removed: usize,
    pub skipped: usize,
    pub warnings: Vec<String>,
    pub native_files_changed: usize,
}

pub(super) fn delete_for_identity(
    auth: &super::PlatformAuthState,
    state: &AppState,
    app: &AppType,
    base_url: &str,
    user_id: i64,
) -> Result<PlatformDeleteSummary, PlatformAuthError> {
    auth.with_local_identity_blocking(|origin, current_user_id| {
        if origin != base_url || current_user_id != user_id {
            return Err(super::fail("UNAUTHORIZED", "登录状态已改变，请重新操作"));
        }
        delete_account_providers(state, app, origin, current_user_id)
    })
}

pub(super) fn delete_account_providers(
    state: &AppState,
    app: &AppType,
    origin: &str,
    user_id: i64,
) -> Result<PlatformDeleteSummary, PlatformAuthError> {
    let providers = local_error(state.db.get_all_providers(app.as_str()))?;
    let targets: Vec<_> = providers
        .values()
        .filter(|provider| {
            provider
                .meta
                .as_ref()
                .and_then(|meta| meta.platform_managed.as_ref())
                .is_some_and(|managed| managed.origin == origin && managed.user_id == user_id)
        })
        .collect();
    let mut summary = PlatformDeleteSummary::default();
    if targets.is_empty() {
        return Ok(summary);
    }
    let apps = std::slice::from_ref(app);
    let before = sync_report::observe(state, apps, origin, user_id)?;
    let deleting: HashSet<_> = targets.iter().map(|provider| provider.id.clone()).collect();
    let all_ids: Vec<_> = providers.keys().cloned().collect();
    crate::config_history::with_audit_context(app.as_str(), "platform_delete", || {
        for provider in targets {
            match delete_managed_provider(state, app, provider, &all_ids, &deleting) {
                Ok(()) => summary.removed += 1,
                Err(err) => {
                    summary.skipped += 1;
                    summary
                        .warnings
                        .push(format!("未删除平台供应商 {}：{}", provider.name, err));
                }
            }
        }
    });
    match sync_report::observe(state, apps, origin, user_id) {
        Ok(after) => {
            for (agent, report) in sync_report::reports(&before, &after, &summary.warnings) {
                summary.native_files_changed += report.changed_files.len();
                if crate::config_history::record_provider_change(&agent, "platform_delete", report)
                    .is_err()
                {
                    log::warn!("Failed to persist platform provider deletion for {agent}");
                    summary
                        .warnings
                        .push("删除操作记录未能保存，请检查应用数据目录权限".into());
                }
            }
        }
        Err(err) => summary.warnings.push(err.message),
    }
    Ok(summary)
}
