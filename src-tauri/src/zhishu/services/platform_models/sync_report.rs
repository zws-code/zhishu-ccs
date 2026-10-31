//! 对比同步前后的托管供应商与受监测文件，为配置历史生成同步报告。
//! 观察结果只保留哈希，不保存配置正文；数据库行变化与原生文件变化分别统计。

use super::{local_error, PlatformAuthError};
use crate::{
    app_config::AppType,
    error::AppError,
    live::engine::{digest, lock_app, DeviceStore},
    store::AppState,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 复用各适配器的路径解析，覆盖其可能写入的配置、凭据和派生文件，保留目录覆盖规则。
pub(crate) fn paths(app: &AppType) -> Result<Vec<PathBuf>, AppError> {
    Ok(match app {
        AppType::Claude => vec![
            crate::config::get_claude_settings_path(),
            crate::config::get_claude_mcp_path(),
        ],
        AppType::Codex => vec![
            crate::codex_config::get_codex_config_path(),
            crate::codex_config::get_codex_auth_path(),
            crate::codex_config::get_codex_model_catalog_path(),
            crate::codex_config::get_codex_managed_oauth_live_auth_marker_path(),
            DeviceStore::for_device().file("codex-login-stash.json"),
        ],
        AppType::Gemini => vec![
            crate::gemini_config::get_gemini_env_path(),
            crate::gemini_config::get_gemini_settings_path(),
        ],
        AppType::GrokBuild => vec![crate::grok_config::get_grok_config_path()],
        AppType::OpenCode => vec![crate::opencode_config::get_opencode_config_path()?],
        AppType::OpenClaw => vec![crate::openclaw_config::get_openclaw_config_path()],
        AppType::Hermes => vec![crate::hermes_config::get_hermes_config_path()],
        AppType::Pi => vec![
            crate::pi_config::get_pi_models_path()?,
            crate::pi_config::get_pi_settings_path()?,
        ],
        AppType::Mcode => vec![crate::mcode_config::config_path()],
        AppType::Kimi => vec![crate::kimi_config::config_path()],
        AppType::ClaudeDesktop => crate::claude_desktop_config::live_file_paths()?,
        AppType::WorkBuddy => vec![crate::workbuddy_config::get_workbuddy_models_path()],
        _ => return Err(AppError::InvalidInput("Unsupported platform Agent".into())),
    })
}

#[derive(Clone, Default)]
pub(crate) struct Observation {
    files: BTreeMap<PathBuf, Option<String>>,
    providers: BTreeMap<String, String>,
}

pub(crate) fn observe(
    state: &AppState,
    apps: &[AppType],
    origin: &str,
    user_id: i64,
) -> Result<BTreeMap<String, Observation>, PlatformAuthError> {
    apps.iter()
        .map(|app| {
            capture(state, app, origin, user_id)
                .map(|observation| (app.as_str().to_string(), observation))
        })
        .collect()
}

fn capture(
    state: &AppState,
    app: &AppType,
    origin: &str,
    user_id: i64,
) -> Result<Observation, PlatformAuthError> {
    let _switch =
        futures::executor::block_on(state.proxy_service.lock_switch_for_app(app.as_str()));
    let _guard = lock_app(app.as_str());
    let mut files = BTreeMap::new();
    for path in local_error(paths(app))? {
        let bytes = local_error(crate::live::engine::read_current(&path))?;
        files.insert(path, digest(bytes.as_deref()));
    }
    let mut providers = BTreeMap::new();
    for provider in local_error(state.db.get_all_providers(app.as_str()))?.into_values() {
        if !provider
            .meta
            .as_ref()
            .and_then(|meta| meta.platform_managed.as_ref())
            .is_some_and(|managed| managed.origin == origin && managed.user_id == user_id)
        {
            continue;
        }
        let value = json!([provider.name, provider.settings_config, provider.meta]);
        providers.insert(
            provider.id,
            digest(Some(&local_error(serde_json::to_vec(&value))?)).expect("hash"),
        );
    }
    Ok(Observation { files, providers })
}

/// 按工具计算增删改与文件消失情况；部分失败同样留痕，报告不能作为文件级还原快照。
pub(crate) fn reports(
    before: &BTreeMap<String, Observation>,
    after: &BTreeMap<String, Observation>,
    warnings: &[String],
) -> Vec<(String, crate::config_history::ProviderSyncReport)> {
    after
        .iter()
        .filter_map(|(agent, after)| {
            let original = before.get(agent)?;
            let added = after
                .providers
                .keys()
                .filter(|id| !original.providers.contains_key(*id))
                .count();
            let removed = original
                .providers
                .keys()
                .filter(|id| !after.providers.contains_key(*id))
                .count();
            let updated = after
                .providers
                .iter()
                .filter(|(id, hash)| {
                    original
                        .providers
                        .get(*id)
                        .is_some_and(|before| before != *hash)
                })
                .count();
            let mut changed_files: Vec<_> = after
                .files
                .iter()
                .filter(|(path, hash)| original.files.get(*path) != Some(*hash))
                .map(|(path, _)| path.to_string_lossy().to_string())
                .collect();
            changed_files.extend(
                original
                    .files
                    .keys()
                    .filter(|path| !after.files.contains_key(*path))
                    .map(|path| path.to_string_lossy().to_string()),
            );
            Some((
                agent.clone(),
                crate::config_history::ProviderSyncReport {
                    added,
                    updated,
                    removed,
                    unchanged: after.providers.len().saturating_sub(added + updated),
                    changed_files,
                    backup_path: None,
                    warnings: warnings.to_vec(),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn observation(files: &[(&str, Option<&str>)], providers: &[(&str, &str)]) -> Observation {
        Observation {
            files: files
                .iter()
                .map(|(path, hash)| (PathBuf::from(path), hash.map(str::to_owned)))
                .collect(),
            providers: providers
                .iter()
                .map(|(id, hash)| (id.to_string(), hash.to_string()))
                .collect(),
        }
    }

    #[test]
    fn reports_diff_added_removed_updated_and_file_deletions() {
        let before = BTreeMap::from([(
            "claude".to_string(),
            observation(
                &[
                    ("/isolated/settings.json", Some("old")),
                    ("/gone.json", Some("x")),
                ],
                &[("kept", "same"), ("edited", "before"), ("deleted", "x")],
            ),
        )]);
        let after = BTreeMap::from([(
            "claude".to_string(),
            observation(
                &[
                    ("/isolated/settings.json", Some("new")),
                    ("/added.json", None),
                ],
                &[("kept", "same"), ("edited", "after"), ("added", "x")],
            ),
        )]);
        let reports = reports(&before, &after, &["warn".to_string()]);
        let (_, report) = &reports[0];
        assert_eq!(report.added, 1);
        assert_eq!(report.updated, 1);
        assert_eq!(report.removed, 1);
        assert_eq!(report.unchanged, 1);
        assert_eq!(report.changed_files.len(), 3);
        assert!(report
            .changed_files
            .contains(&"/isolated/settings.json".into()));
        assert!(report.changed_files.contains(&"/gone.json".into()));
        assert!(report.changed_files.contains(&"/added.json".into()));
        assert_eq!(report.warnings, vec!["warn".to_string()]);
        assert_eq!(report.backup_path, None);
    }

    #[test]
    fn reports_skip_agents_missing_from_the_before_observation() {
        let before = BTreeMap::new();
        let after = BTreeMap::from([("claude".to_string(), observation(&[], &[("kept", "same")]))]);
        assert!(reports(&before, &after, &[]).is_empty());
    }
}
