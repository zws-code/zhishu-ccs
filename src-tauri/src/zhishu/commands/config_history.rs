//! 配置历史的 IPC 入口：列表、显式敏感详情、清空记录、历史备份定位与分组还原。
//! 还原沿用平台操作锁和工具切换锁，避免与供应商同步、切换或 pending 恢复交错。
use crate::config_history::{self, ConfigChangeDetail, ConfigChangeGroup, ConfigRestoreSummary};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn open_config_sync_backup(handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let path = config_history::sync_backup_location(&id).map_err(|error| error.to_string())?;
    handle
        .opener()
        .open_path(path.to_string_lossy().to_string(), None::<String>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_config_change_history(app: Option<String>) -> Result<Vec<ConfigChangeGroup>, String> {
    config_history::list(app).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_config_change_history(
    id: String,
    reveal_sensitive: bool,
) -> Result<Option<ConfigChangeDetail>, String> {
    config_history::detail(id, reveal_sensitive).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clear_config_change_history() -> Result<usize, String> {
    config_history::clear().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn restore_config_change_history(
    ids: Vec<String>,
    state: tauri::State<'_, crate::store::AppState>,
) -> Result<ConfigRestoreSummary, String> {
    let state = state.inner().clone();
    let summary = tauri::async_runtime::spawn_blocking(move || restore_group(&state, &ids))
        .await
        .map_err(|_| "Configuration restore task failed".to_string())?
        .map_err(|error: crate::error::AppError| error.to_string())?;
    Ok(summary)
}

/// 按平台操作锁、工具切换锁的顺序获取锁，先完成 pending 恢复，再还原该组文件。
/// 直连模式的供应商指针仅在仍指向本次历史切换目标时回退，保留用户后续切换的选择；
/// 切换前没有供应商时清除选择，切换前的供应商已被删除时保留当前选择。
fn restore_group(
    state: &crate::store::AppState,
    ids: &[String],
) -> Result<ConfigRestoreSummary, crate::error::AppError> {
    let _apply =
        futures::executor::block_on(crate::zhishu::services::platform_models::APPLY_LOCK.lock());
    let plan = config_history::restore_plan(ids)?;
    let _switch = futures::executor::block_on(state.proxy_service.lock_switch_for_app(&plan.app));
    crate::mode::operation::settle(&state.db, &plan.app)?;
    let store = crate::live::engine::DeviceStore::for_device();
    let files = config_history::restore_records(&plan, &store)?;

    let mut pointer_restored = false;
    if let Some(pointer) = plan.pointer.as_ref() {
        let app_type: crate::app_config::AppType = plan.app.parse()?;
        let current = crate::mode::current::provider_for(
            &state.db,
            &app_type,
            crate::mode::current::Purpose::Direct,
        )?;
        // Only move the selection back while it still points at the provider
        // this switch selected; a later switch keeps its choice.
        if let Some(target) = pointer_restore_target(pointer, current.as_deref()) {
            match target {
                Some(from) => {
                    // The provider the switch started from may have been
                    // deleted since (e.g. 一键删除平台供应商); selecting a
                    // missing row would leave files and selection inconsistent.
                    if state.db.get_provider_by_id(from, &plan.app)?.is_some() {
                        crate::settings::set_current_provider(&app_type, Some(from))?;
                        state.db.set_current_provider(&plan.app, from)?;
                        pointer_restored = true;
                    }
                }
                None => {
                    // The switch started with no direct provider, so restoring
                    // it clears the selection. `set_current_provider` has no
                    // clear form, and an empty id matches no provider row, so
                    // this only zeroes `is_current` for the app.
                    crate::settings::set_current_provider(&app_type, None)?;
                    state.db.set_current_provider(&plan.app, "")?;
                    pointer_restored = true;
                }
            }
        }
    }
    Ok(summary(files, pointer_restored))
}

/// The direct selection a restored switch should move back to while the app
/// still points at the provider that switch chose. `Some(None)` means the
/// switch started with no current provider, so restoring clears the selection.
fn pointer_restore_target<'a>(
    pointer: &'a config_history::SwitchPointer,
    current: Option<&str>,
) -> Option<Option<&'a str>> {
    (current == Some(pointer.to.as_str())).then_some(pointer.from.as_deref())
}

fn summary(
    files: Vec<config_history::ConfigRestoreFile>,
    pointer_restored: bool,
) -> ConfigRestoreSummary {
    let changed = files.iter().any(|file| file.changed);
    ConfigRestoreSummary {
        files,
        changed,
        pointer_restored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_history::SwitchPointer;

    fn switch(from: Option<&str>, to: &str) -> SwitchPointer {
        SwitchPointer {
            from: from.map(str::to_owned),
            to: to.to_owned(),
        }
    }

    #[test]
    fn pointer_restore_only_moves_back_from_the_switched_provider() {
        let pointer = switch(Some("provider-a"), "provider-b");
        assert_eq!(
            pointer_restore_target(&pointer, Some("provider-b")),
            Some(Some("provider-a"))
        );
        // A later switch (or a missing selection) keeps its own choice.
        assert_eq!(pointer_restore_target(&pointer, Some("provider-c")), None);
        assert_eq!(pointer_restore_target(&pointer, None), None);
    }

    #[test]
    fn first_switch_restores_to_no_selection() {
        // The switch started with no current provider; restoring must clear
        // it instead of leaving the switched-to provider selected.
        let pointer = switch(None, "provider-b");
        assert_eq!(
            pointer_restore_target(&pointer, Some("provider-b")),
            Some(None)
        );
        assert_eq!(pointer_restore_target(&pointer, Some("provider-c")), None);
    }
}
