//! 平台本机文件的统一路径入口，供会话、登录记录和模型目录复用。
//! 开发隔离优先；正式运行使用设备本地目录，不随供应商数据库导出或云同步。

use std::path::PathBuf;
use tauri::Manager;

pub(crate) fn data_file(handle: &tauri::AppHandle, name: &str) -> tauri::Result<PathBuf> {
    let directory = if crate::config::isolated_dev_home().is_some() {
        crate::config::get_app_config_dir()
    } else {
        handle.path().app_local_data_dir()?
    };
    Ok(directory.join(name))
}
