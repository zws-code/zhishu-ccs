//! 平台二阶段同步：联网更新设备目录，再使用本机目录协调各工具的托管供应商。
//! 认证、分组 Key 和原生配置写入留在 Rust；本地同步只处理当前账户拥有的供应商。
use crate::zhishu::integrations::ccswitch::platform_models as adapter;
use crate::zhishu::integrations::ccswitch::providers::Providers as ProviderService;
pub(crate) mod base_data;
mod catalog;
mod connection;
pub(crate) mod delete;
mod sync_report;

pub(crate) use crate::zhishu::services::platform_auth::{error as fail, PlatformAuthError};
use crate::zhishu::services::platform_auth::{GroupDiscovery, PlatformAuthState};
use crate::{provider::Provider, store::AppState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use tauri::{Manager, State};
use tokio::sync::Mutex;

// 同步、退出登录和历史还原共用操作锁；涉及供应商时按操作锁、切换锁的顺序获取。
pub(crate) static APPLY_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentModel {
    pub id: String,
    pub agent: String,
    pub model: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub protocol: String,
    pub category: String,
    #[serde(default)]
    pub available: bool,
    pub supports_tools: Option<bool>,
    pub supports_streaming: Option<bool>,
    #[serde(default)]
    pub context_window: u64,
    #[serde(default)]
    pub max_output_tokens: u64,
    #[serde(default)]
    pub reasoning_levels: Vec<String>,
    #[serde(default)]
    pub default_reasoning_level: Option<String>,
    #[serde(default)]
    pub reasoning: Option<bool>,
    #[serde(default)]
    pub input_modalities: Vec<String>,
    #[serde(default)]
    pub supports_parallel_tool_calls: Option<bool>,
    #[serde(default)]
    pub pricing: Option<Value>,
}

fn local_error<T>(result: Result<T, impl std::fmt::Display>) -> Result<T, PlatformAuthError> {
    result.map_err(|error| {
        log::warn!("Platform tool configuration failed: {error}");
        fail("CONFIG", "工具配置未能完成，请检查配置文件权限后重试")
    })
}

fn replace_credential(value: &mut Value, from: &str, to: &str) -> bool {
    match value {
        Value::String(text) if text.contains(from) => {
            *text = text.replace(from, to);
            true
        }
        Value::Array(values) => {
            let mut changed = false;
            for item in values {
                changed |= replace_credential(item, from, to);
            }
            changed
        }
        Value::Object(values) => {
            let mut changed = false;
            for item in values.values_mut() {
                changed |= replace_credential(item, from, to);
            }
            changed
        }
        _ => false,
    }
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformSyncSummary {
    added: usize,
    removed: usize,
    skipped: usize,
    warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    native_files_changed: Option<usize>,
}

struct DesiredProvider {
    group_id: i64,
    provider: Provider,
}

/// 第一阶段：联网拉取平台基础数据并保存设备快照，不修改工具供应商。
#[tauri::command]
pub async fn platform_sync_catalog(
    handle: tauri::AppHandle,
    auth: State<'_, PlatformAuthState>,
) -> Result<base_data::CatalogSummary, PlatformAuthError> {
    let _operation = APPLY_LOCK.lock().await;
    let discovery = auth.discover_group_catalogs().await?;
    let store = base_data::CatalogStore::for_app(&handle)?;
    let cached = auth
        .with_local_identity(|origin, user_id| {
            if discovery.account.base_url != origin || discovery.account.user.id != user_id {
                return Err(fail("UNAUTHORIZED", "登录状态已改变，请重新同步"));
            }
            store.save(discovery.clone())
        })
        .await?;
    Ok(cached.summary())
}

/// 读取当前账户本机摘要，按需迁移旧单文件 v2；不发 HTTP，也不泄露目录 Key。
#[tauri::command]
pub async fn platform_get_catalog(
    handle: tauri::AppHandle,
    auth: State<'_, PlatformAuthState>,
) -> Result<Option<base_data::CatalogSummary>, PlatformAuthError> {
    let _operation = APPLY_LOCK.lock().await;
    let store = base_data::CatalogStore::for_app(&handle)?;
    auth.with_local_identity(|origin, user_id| {
        Ok(store.read(origin, user_id)?.map(|cached| cached.summary()))
    })
    .await
}

/// 显式查看单个分组 Key：先校验当前身份和快照归属，再从设备目录读取。
#[tauri::command]
pub async fn platform_get_catalog_key(
    handle: tauri::AppHandle,
    auth: State<'_, PlatformAuthState>,
    base_url: String,
    user_id: i64,
    group_id: i64,
) -> Result<Option<String>, PlatformAuthError> {
    let _operation = APPLY_LOCK.lock().await;
    let store = base_data::CatalogStore::for_app(&handle)?;
    auth.with_local_identity(|origin, current_user_id| {
        if origin != base_url || current_user_id != user_id {
            return Err(fail("UNAUTHORIZED", "登录状态已改变，请重新打开详情"));
        }
        store.group_key(origin, current_user_id, group_id)
    })
    .await
}

/// 第二阶段：只用本机目录同步工具供应商，身份校验也不联网、不创建或撤销 Key。
/// agent 为空表示全部已支持工具，保留既有 IPC 含义。
#[tauri::command]
pub async fn platform_sync_providers(
    handle: tauri::AppHandle,
    agent: Option<String>,
) -> Result<PlatformSyncSummary, PlatformAuthError> {
    let _operation = APPLY_LOCK.lock().await;
    let apps = match agent.as_deref() {
        Some(agent) => vec![adapter::app(agent)?],
        None => adapter::AGENTS
            .iter()
            .map(|agent| adapter::app(agent))
            .collect::<Result<Vec<_>, _>>()?,
    };
    let store = base_data::CatalogStore::for_app(&handle)?;
    let worker_handle = handle.clone();
    // 适配器含同步 I/O 和切换锁的 block_on，必须放在阻塞线程；否则可能耗尽 Tokio
    // 协作预算并一直持有 APPLY_LOCK，连带阻止后续同步和退出。
    let summary = run_provider_sync(move || {
        let state = worker_handle.state::<AppState>();
        let auth = worker_handle.state::<PlatformAuthState>();
        auth.with_local_identity_blocking(|origin, user_id| {
            sync_account_providers(state.inner(), &apps, &store, origin, user_id)
        })
    })
    .await?;
    crate::tray::refresh_tray_menu(&handle);
    Ok(summary)
}

/// 显式删除平台供应商；agent 为空时遍历全部已支持应用，不受应用可见性限制。
/// 按请求时的身份校验，避免迟到操作删除新账户。
#[tauri::command]
pub async fn platform_delete_providers(
    handle: tauri::AppHandle,
    agent: Option<String>,
    base_url: String,
    user_id: i64,
) -> Result<delete::PlatformDeleteSummary, PlatformAuthError> {
    let _operation = APPLY_LOCK.lock().await;
    let app = agent.as_deref().map(adapter::app).transpose()?;
    let worker_handle = handle.clone();
    let summary = tauri::async_runtime::spawn_blocking(move || {
        let state = worker_handle.state::<AppState>();
        let auth = worker_handle.state::<PlatformAuthState>();
        match app {
            Some(app) => {
                delete::delete_for_identity(auth.inner(), state.inner(), &app, &base_url, user_id)
            }
            None => {
                delete::delete_all_for_identity(auth.inner(), state.inner(), &base_url, user_id)
            }
        }
    })
    .await
    .map_err(|_| fail("CONFIG", "平台供应商删除任务未能完成，请重试"))??;
    crate::tray::refresh_tray_menu(&handle);
    Ok(summary)
}

async fn run_provider_sync(
    action: impl FnOnce() -> Result<PlatformSyncSummary, PlatformAuthError> + Send + 'static,
) -> Result<PlatformSyncSummary, PlatformAuthError> {
    tauri::async_runtime::spawn_blocking(action)
        .await
        .map_err(|_| fail("CONFIG", "工具供应商同步任务未能完成，请重试"))?
}

/// 同步本机目录并记录操作前后差异，不做外部编辑检查、覆盖确认或专用同步备份。
/// 未变化的托管行不重写原生文件；实际写入仍通过 ProviderService 保留各工具既有恢复路径。
fn sync_account_providers(
    state: &AppState,
    apps: &[crate::app_config::AppType],
    store: &base_data::CatalogStore,
    origin: &str,
    user_id: i64,
) -> Result<PlatformSyncSummary, PlatformAuthError> {
    let cached = store
        .read(origin, user_id)?
        .ok_or_else(|| fail("CATALOG_REQUIRED", "请先同步当前账号的平台基础数据"))?;
    let before = sync_report::observe(state, apps, origin, user_id)?;
    let mut summary = sync_cached_providers(state, apps, &cached.discovery);
    let mut updated = 0;
    let mut native_files_changed = 0;
    match sync_report::observe(state, apps, origin, user_id) {
        Ok(after) => {
            for (agent, report) in sync_report::reports(&before, &after, &summary.warnings) {
                updated += report.updated;
                native_files_changed += report.changed_files.len();
                if crate::config_history::record_provider_sync(&agent, report).is_err() {
                    log::warn!("Failed to persist platform sync result for {agent}");
                    summary
                        .warnings
                        .push("同步操作记录未能保存，请检查应用数据目录权限".into());
                }
            }
        }
        Err(error) => summary.warnings.push(error.message),
    }
    summary.updated = Some(updated);
    summary.native_files_changed = Some(native_files_changed);
    Ok(summary)
}

fn sync_cached_providers(
    state: &AppState,
    apps: &[crate::app_config::AppType],
    discovery: &GroupDiscovery,
) -> PlatformSyncSummary {
    let mut summary = PlatformSyncSummary {
        warnings: discovery.warnings.clone(),
        ..Default::default()
    };
    for app in apps {
        let result =
            crate::config_history::with_audit_context(app.as_str(), "platform_sync", || {
                sync_agent_providers(state, app, discovery)
            });
        match result {
            Ok((added, removed, skipped, warnings)) => {
                summary.added += added;
                summary.removed += removed;
                summary.skipped += skipped;
                summary.warnings.extend(warnings);
            }
            Err(err) => summary.warnings.push(format!(
                "{} 的服务供应商列表未能同步：{}",
                app.as_str(),
                err.message
            )),
        }
    }
    summary
}

fn sync_agent_providers(
    state: &AppState,
    app: &crate::app_config::AppType,
    discovery: &GroupDiscovery,
) -> Result<(usize, usize, usize, Vec<String>), PlatformAuthError> {
    let agent = app.as_str();
    let live = local_error(connection::read_settings(state, app))?;
    // Expired subscriptions may have no catalog. Update only their marker through the adapter.
    let ids = local_error(state.db.get_all_providers(agent))?;
    for provider in ids.values().filter(|provider| {
        provider
            .meta
            .as_ref()
            .and_then(|meta| meta.platform_managed.as_ref())
            .is_some_and(|managed| {
                managed.origin == discovery.account.base_url
                    && managed.user_id == discovery.account.user.id
                    && managed
                        .group_id
                        .is_some_and(|id| discovery.subscription_group_ids.contains(&id))
                    && managed.subscription_type.as_deref() != Some("subscription")
            })
    }) {
        local_error(ProviderService::mark_subscription(
            state,
            app.clone(),
            &provider.id,
            &discovery.account.base_url,
            discovery.account.user.id,
            &discovery.subscription_group_ids,
        ))?;
    }
    let mut existing = local_error(state.db.get_all_providers(agent))?;
    let mut failed_group_ids = discovery.failed_group_ids.clone();
    let mut warnings = Vec::new();
    // 无凭证不等于目录失败，更不能将未知 Key 写入工具配置。
    for group in &discovery.groups {
        if !discovery
            .catalogs
            .iter()
            .any(|catalog| catalog.group.id == group.id)
            && failed_group_ids.insert(group.id)
        {
            warnings.push(format!(
                "分组 {} 未提供专用凭证，已保留本地供应商，请重新同步平台基础数据",
                group.id
            ));
        }
    }
    let mut desired = HashMap::<String, DesiredProvider>::new();

    for catalog in &discovery.catalogs {
        let models = catalog::agent_models(&catalog.credential.platform, &catalog.models, &[agent]);
        if agent == "workbuddy" && models.is_empty() && !catalog.models.is_empty() {
            failed_group_ids.insert(catalog.group.id);
            warnings.push(format!(
                "分组 {} 没有可用于 WorkBuddy 的 OpenAI Chat Completions 模型",
                catalog.group.name
            ));
            continue;
        }
        if agent == "gemini" && models.is_empty() && !catalog.models.is_empty() {
            failed_group_ids.insert(catalog.group.id);
            warnings.push(format!(
                "分组 {} 没有 Gemini 原生模型；Gemini CLI 不能使用该分组的 GPT/Claude 模型",
                catalog.group.name
            ));
            continue;
        }
        if catalog.catalog_error.is_some() {
            failed_group_ids.insert(catalog.group.id);
            warnings.push(format!(
                "分组 {} 的模型目录暂不可用，已保留本地供应商",
                catalog.group.id
            ));
            continue;
        }
        if models.is_empty() {
            continue;
        }
        let model = match connection::select_model(agent, &models, &live) {
            Ok(model) => model,
            Err(err) => {
                failed_group_ids.insert(catalog.group.id);
                warnings.push(format!(
                    "分组 {} 的模型配置无法解析，已保留本地供应商：{}",
                    catalog.group.name, err.message
                ));
                continue;
            }
        };
        let mut provider = match adapter::service_provider_with_connection(
            model,
            &discovery.account.base_url,
            discovery.account.user.id,
            &catalog.credential.key,
            catalog.credential.key_id,
            None,
            catalog.group.id,
            &catalog.group.platform,
            &catalog.group.name,
            &catalog.credential.key_name,
            catalog.connection.as_ref(),
        ) {
            Ok(provider) => provider,
            Err(err) => {
                failed_group_ids.insert(catalog.group.id);
                warnings.push(format!(
                    "分组 {} 无法适配 {}：{}",
                    catalog.group.name, agent, err.message
                ));
                continue;
            }
        };
        if let Some(managed) = provider
            .meta
            .as_mut()
            .and_then(|meta| meta.platform_managed.as_mut())
        {
            managed.subscription_type =
                if discovery.subscription_group_ids.contains(&catalog.group.id) {
                    Some("subscription".into())
                } else {
                    (!catalog.group.subscription_type.is_empty())
                        .then(|| catalog.group.subscription_type.clone())
                };
        }
        if let Err(err) = connection::populate_catalog(&mut provider, model, &models) {
            failed_group_ids.insert(catalog.group.id);
            warnings.push(format!(
                "分组 {} 的模型目录无法写入：{}",
                catalog.group.name, err.message
            ));
            continue;
        }
        let id = provider.id.clone();
        desired.insert(
            id,
            DesiredProvider {
                group_id: catalog.group.id,
                provider,
            },
        );
    }

    // 先添加再删除，让删除当前旧供应商时有机会选择刚同步的安全备用项。
    let mut added = 0;
    let mut attempted_adds = HashSet::new();
    for (id, desired) in &desired {
        if existing.contains_key(id) {
            continue;
        }
        attempted_adds.insert(id.clone());
        let add = if *app == crate::app_config::AppType::Dsh {
            ProviderService::add_to_live(state, app.clone(), desired.provider.clone())
        } else {
            ProviderService::add_without_switching(state, app.clone(), desired.provider.clone())
        };
        match add {
            Ok(_) => {
                added += 1;
                existing.insert(id.clone(), desired.provider.clone());
            }
            Err(err) => warnings.push(format!("无法添加服务供应商 {id}：{err}")),
        }
    }

    // 只协调当前平台账户的托管行；保留其他账户、自建项及失败或额度不可用的分组。
    let to_delete: Vec<_> = existing
        .values()
        .filter_map(|provider| {
            let managed = provider
                .meta
                .as_ref()
                .and_then(|meta| meta.platform_managed.as_ref())?;
            let same_account = managed.origin == discovery.account.base_url
                && managed.user_id == discovery.account.user.id;
            if !same_account {
                return None;
            }
            let group_failed = managed
                .group_id
                .is_some_and(|group_id| failed_group_ids.contains(&group_id));
            let quota_unavailable = managed
                .group_id
                .and_then(|id| discovery.details.get(&id))
                .is_some_and(|details| !details.quota.available);
            if quota_unavailable || group_failed || desired.contains_key(&provider.id) {
                return None;
            }
            // 成功且可用的空模型目录/撤销授权可以协调本地项；失败或额度不足不能冒充下线。
            Some(provider.clone())
        })
        .collect();
    let deleting: HashSet<_> = to_delete
        .iter()
        .map(|provider| provider.id.clone())
        .collect();
    let all_ids: Vec<_> = existing.keys().cloned().collect();
    let mut removed = 0;
    for provider in to_delete {
        match delete_managed_provider(state, app, &provider, &all_ids, &deleting) {
            Ok(()) => {
                existing.shift_remove(&provider.id);
                removed += 1;
            }
            Err(err) => warnings.push(format!(
                "无法删除已下线的服务供应商 {}：{}",
                provider.name, err
            )),
        }
    }

    let mut skipped = 0;
    for (id, desired) in &desired {
        if attempted_adds.contains(id) {
            continue;
        }
        if let Some(provider) = existing.get(id) {
            let same_account = provider
                .meta
                .as_ref()
                .and_then(|meta| meta.platform_managed.as_ref())
                .is_some_and(|managed| {
                    managed.origin == discovery.account.base_url
                        && managed.user_id == discovery.account.user.id
                        && managed.group_id == Some(desired.group_id)
                });
            if !same_account {
                warnings.push(format!("供应商 ID {} 已被其他配置占用，已跳过", id));
            } else {
                let (_, current_key) = provider.resolve_usage_credentials(app);
                let (_, desired_key) = desired.provider.resolve_usage_credentials(app);
                let mut refreshed = provider.clone();
                let mut changed = current_key != desired_key;
                if current_key != desired_key {
                    let replaced = !current_key.is_empty()
                        && replace_credential(
                            &mut refreshed.settings_config,
                            &current_key,
                            &desired_key,
                        );
                    if !replaced {
                        refreshed = desired.provider.clone();
                        refreshed.sort_index = provider.sort_index;
                        refreshed.created_at = provider.created_at;
                    }
                }
                let previous_base = provider
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.platform_managed.as_ref())
                    .and_then(|managed| managed.connection_base_url.as_deref())
                    .unwrap_or(&discovery.account.base_url);
                let next_base = desired
                    .provider
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.platform_managed.as_ref())
                    .and_then(|managed| managed.connection_base_url.as_deref())
                    .unwrap_or(&discovery.account.base_url);
                changed |= adapter::refresh_connection_base(
                    &mut refreshed.settings_config,
                    previous_base,
                    next_base,
                );
                if refreshed.name != desired.provider.name {
                    if app == &crate::app_config::AppType::Dsh
                        && refreshed.settings_config["name"] == provider.name
                    {
                        refreshed.settings_config["name"] =
                            Value::String(desired.provider.name.clone());
                    }
                    refreshed.name = desired.provider.name.clone();
                    changed = true;
                }
                changed |= adapter::repair_legacy_anthropic_base_url(
                    &mut refreshed.settings_config,
                    &desired.provider.settings_config,
                    app,
                    next_base,
                );
                changed |= adapter::repair_protocol_base_url(
                    &mut refreshed.settings_config,
                    app,
                    next_base,
                    provider
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.api_format.as_deref()),
                    desired
                        .provider
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.api_format.as_deref()),
                );
                changed |= connection::refresh_catalog(&mut refreshed, &desired.provider, app);
                if let (Some(managed), Some(desired)) = (
                    refreshed
                        .meta
                        .as_mut()
                        .and_then(|meta| meta.platform_managed.as_mut()),
                    desired
                        .provider
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.platform_managed.as_ref()),
                ) {
                    changed |= managed.connection_base_url != desired.connection_base_url
                        || managed.key_id != desired.key_id
                        || managed.key_name != desired.key_name
                        || managed.group_id != desired.group_id
                        || managed.group_platform != desired.group_platform
                        || managed.group_name != desired.group_name
                        || managed.subscription_type != desired.subscription_type;
                    managed.connection_base_url = desired.connection_base_url.clone();
                    managed.key_id = desired.key_id;
                    managed.key_name = desired.key_name.clone();
                    managed.group_id = desired.group_id;
                    managed.group_platform = desired.group_platform.clone();
                    managed.group_name = desired.group_name.clone();
                    managed.subscription_type = desired.subscription_type.clone();
                }
                let mut refresh_succeeded = true;
                if changed {
                    if let Err(err) = ProviderService::update(state, app.clone(), None, refreshed) {
                        warnings.push(format!("无法刷新服务供应商 {id} 的配置：{err}"));
                        refresh_succeeded = false;
                    }
                }
                if refresh_succeeded && *app == crate::app_config::AppType::Dsh {
                    match ProviderService::dsh_native_exists(id) {
                        Ok(false) => {
                            if let Err(err) = ProviderService::switch(state, app.clone(), id) {
                                warnings.push(format!("无法将服务供应商 {id} 写入 DSH：{err}"));
                            }
                        }
                        Ok(true) => {}
                        Err(err) => warnings.push(format!("无法检查 DSH 供应商 {id}：{err}")),
                    }
                }
            }
            skipped += 1;
            continue;
        }
        warnings.push(format!("服务供应商 {id} 未能加入列表"));
    }
    Ok((added, removed, skipped, warnings))
}

fn delete_managed_provider(
    state: &AppState,
    app: &crate::app_config::AppType,
    provider: &Provider,
    all_ids: &[String],
    deleting: &HashSet<String>,
) -> Result<(), crate::error::AppError> {
    let is_current = ProviderService::current(state, app.clone())? == provider.id.as_str();
    if is_current && !app.is_additive_mode() {
        let mut switched = false;
        for candidate in all_ids
            .iter()
            .filter(|id| *id != &provider.id && !deleting.contains(*id))
        {
            if ProviderService::switch(state, app.clone(), candidate).is_ok() {
                switched = true;
                break;
            }
        }
        if !switched {
            return Err(crate::error::AppError::Message(
                "当前正在使用这个供应商，且没有可切换的备用供应商".into(),
            ));
        }
    }
    ProviderService::delete(state, app.clone(), &provider.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test(flavor = "current_thread")]
    async fn provider_sync_keeps_runtime_responsive_and_releases_logout_lock() {
        let operation = std::sync::Arc::new(tokio::sync::Mutex::new(()));
        let sync_lock = operation.clone();
        let app_locks = crate::proxy::switch_lock::SwitchLockManager::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let sync = tokio::spawn(async move {
            let _guard = sync_lock.lock().await;
            run_provider_sync(move || {
                started_tx.send(()).unwrap();
                // More lock acquisitions than one async task's cooperative budget.
                // The real all-app sync repeatedly enters these adapter locks.
                for _ in 0..256 {
                    let _app_guard =
                        futures::executor::block_on(app_locks.lock_for_app("workbuddy"));
                }
                Ok(PlatformSyncSummary::default())
            })
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            started_rx.await.unwrap();
            let _logout_guard = operation.lock().await;
            sync.await.unwrap().unwrap();
        })
        .await
        .expect("provider sync must finish and let logout acquire its lock");
    }

    struct SyncTestHome {
        _temp: tempfile::TempDir,
        env: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl SyncTestHome {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let mut env = Vec::new();
            for (key, dir) in [
                ("CC_SWITCH_TEST_HOME", temp.path().to_path_buf()),
                ("PI_CODING_AGENT_DIR", temp.path().join("pi")),
                ("HERMES_HOME", temp.path().join("hermes")),
                ("MINIMAX_DATA_DIR", temp.path().join("mcode")),
                ("WORKBUDDY_CONFIG_DIR", temp.path().join("workbuddy")),
                ("KIMI_CODE_HOME", temp.path().join("kimi")),
                ("DSH_HOME", temp.path().join("dsh")),
            ] {
                env.push((key, std::env::var_os(key)));
                std::env::set_var(key, dir);
            }
            crate::settings::reload_settings().unwrap();
            Self { _temp: temp, env }
        }
    }

    impl Drop for SyncTestHome {
        fn drop(&mut self) {
            for (key, value) in &self.env {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
            crate::settings::reload_settings().unwrap();
        }
    }

    #[test]
    #[serial_test::serial]
    fn platform_claude_slots_are_assembled_before_native_switch() {
        let _home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let model: AgentModel = serde_json::from_value(json!({
            "id":"model", "agent":"claude", "model":"platform-model", "name":"Platform",
            "protocol":"anthropic", "category":"recommended", "available":true
        }))
        .unwrap();
        let provider =
            adapter::provider(&model, "https://platform.test", 1, "fixture-key", 7, None).unwrap();
        let id = provider.id.clone();
        ProviderService::add_without_switching(
            &state,
            crate::app_config::AppType::Claude,
            provider.clone(),
        )
        .unwrap();
        ProviderService::switch(&state, crate::app_config::AppType::Claude, &id).unwrap();
        let live =
            crate::config::read_json_file::<Value>(&crate::config::get_claude_settings_path())
                .unwrap();
        for key in [
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
        ] {
            assert_eq!(provider.settings_config["env"][key], "platform-model");
            assert_eq!(live["env"][key], "platform-model");
        }
    }

    #[test]
    #[serial_test::serial]
    fn subscription_marker_updates_latest_metadata_without_native_writes_or_resurrection() {
        use crate::app_config::AppType;
        use crate::database::Database;
        use crate::services::ProviderService as UpstreamProviders;
        let _home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(Database::memory().unwrap()));
        let provider = deletion_provider("workbuddy", "subscription", "https://platform.test", 1);
        state.db.save_provider("workbuddy", &provider).unwrap();
        // Unknown future fields must survive; a fresh edit made after discovery must survive too.
        let mut raw = serde_json::to_value(provider.meta.as_ref().unwrap()).unwrap();
        raw["future"] = json!({"keep":true});
        raw["platformManaged"]["groupId"] = json!(42);
        raw["platformManaged"]["future"] = json!("keep");
        raw["platformManaged"]
            .as_object_mut()
            .unwrap()
            .remove("subscriptionType");
        state.db.conn.lock().unwrap().execute(
            "UPDATE providers SET meta = ?1, name = 'edited', notes = 'latest', settings_config = ?2 WHERE id = ?3 AND app_type = 'workbuddy'",
            rusqlite::params![raw.to_string(), json!({"userSetting":"keep"}).to_string(), provider.id],
        ).unwrap();
        let path = crate::workbuddy_config::get_workbuddy_home().join("models.json");
        crate::config::atomic_write_private(&path, b"{\"models\":[],\"userSetting\":true}")
            .unwrap();
        let before = std::fs::read(&path).unwrap();
        let groups = HashSet::from([42]);
        for (origin, user, group) in [
            ("https://other.test", 1, 42),
            ("https://platform.test", 2, 42),
            ("https://platform.test", 1, 43),
        ] {
            assert!(!ProviderService::mark_subscription(
                &state,
                AppType::WorkBuddy,
                &provider.id,
                origin,
                user,
                &HashSet::from([group])
            )
            .unwrap());
        }
        assert!(ProviderService::mark_subscription(
            &state,
            AppType::WorkBuddy,
            &provider.id,
            "https://platform.test",
            1,
            &groups
        )
        .unwrap());
        assert!(!ProviderService::mark_subscription(
            &state,
            AppType::WorkBuddy,
            &provider.id,
            "https://platform.test",
            1,
            &groups
        )
        .unwrap());
        let saved = state
            .db
            .get_provider_by_id(&provider.id, "workbuddy")
            .unwrap()
            .unwrap();
        assert_eq!(saved.name, "edited");
        assert_eq!(saved.notes.as_deref(), Some("latest"));
        assert_eq!(saved.settings_config, json!({"userSetting":"keep"}));
        let saved_raw: String = state
            .db
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT meta FROM providers WHERE app_type = 'workbuddy' AND id = ?1",
                [&provider.id],
                |row| row.get(0),
            )
            .unwrap();
        let mut expected = raw;
        expected["platformManaged"]["subscriptionType"] = json!("subscription");
        assert_eq!(serde_json::from_str::<Value>(&saved_raw).unwrap(), expected);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(UpstreamProviders::update_metadata(
            &state,
            AppType::WorkBuddy,
            &provider.id,
            |meta| {
                meta["future"] = json!("discard");
                Err(crate::error::AppError::Config("abort".into()))
            }
        )
        .is_err());
        let after_failure: String = state
            .db
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT meta FROM providers WHERE app_type = 'workbuddy' AND id = ?1",
                [&provider.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(after_failure, saved_raw);
        state.db.delete_provider("workbuddy", &provider.id).unwrap();
        assert!(!ProviderService::mark_subscription(
            &state,
            AppType::WorkBuddy,
            &provider.id,
            "https://platform.test",
            1,
            &groups
        )
        .unwrap());
        assert!(state
            .db
            .get_provider_by_id(&provider.id, "workbuddy")
            .unwrap()
            .is_none());
    }

    #[test]
    #[serial_test::serial]
    fn metadata_updates_wait_for_application_lock_and_preserve_concurrent_fields() {
        use crate::{
            app_config::AppType, database::Database, services::ProviderService as UpstreamProviders,
        };
        let _home = SyncTestHome::new();
        let state = std::sync::Arc::new(AppState::new(std::sync::Arc::new(
            Database::memory().unwrap(),
        )));
        state
            .db
            .save_provider(
                "workbuddy",
                &Provider::with_id("concurrent".into(), "Original".into(), json!({}), None),
            )
            .unwrap();
        let guard =
            futures::executor::block_on(state.proxy_service.lock_switch_for_app("workbuddy"));
        let worker_state = state.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result = UpstreamProviders::update_metadata(
                &worker_state,
                AppType::WorkBuddy,
                "concurrent",
                |meta| {
                    meta["counter"] = json!(
                        meta.get("counter")
                            .and_then(Value::as_u64)
                            .unwrap_or_default()
                            + 1
                    );
                    Ok(())
                },
            );
            done_tx.send(result).unwrap();
        });
        started_rx.recv().unwrap();
        assert!(matches!(
            done_rx.recv_timeout(std::time::Duration::from_millis(50)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        state
            .db
            .update_provider_metadata("workbuddy", "concurrent", |meta| {
                meta["counter"] = json!(40);
                meta["unknown"] = json!({"keep":true});
                Ok(())
            })
            .unwrap();
        drop(guard);
        assert!(done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap());
        worker.join().unwrap();
        let raw: String = state
            .db
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT meta FROM providers WHERE id = 'concurrent' AND app_type = 'workbuddy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let meta: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(meta["counter"], 41);
        assert_eq!(meta["unknown"], json!({"keep":true}));
    }

    #[test]
    #[serial_test::serial]
    fn logout_preserves_all_agent_keys_provider_rows_and_native_files() {
        let _home = SyncTestHome::new();
        use std::sync::Arc;
        fn model(agent: &str, name: &str) -> AgentModel {
            serde_json::from_value(json!({"id":"entry","agent":agent,"model":name,"name":"Verified","protocol":match agent {"claude"|"claude-desktop"=>"anthropic","gemini"=>"gemini","workbuddy"=>"chat",_=>"responses"},"category":"recommended","available":true,"supportsTools":true,"supportsStreaming":true,"contextWindow":32000,"maxOutputTokens":4096})).unwrap()
        }
        let store = AppState::new(Arc::new(crate::database::Database::memory().unwrap()));
        for agent in adapter::AGENTS {
            let app = adapter::app(agent).unwrap();
            let m = model(
                agent,
                if *agent == "claude-desktop" {
                    "claude-sonnet-5"
                } else {
                    "selected-model"
                },
            );
            let user = if *agent == "pi" { 2 } else { 1 };
            let p = adapter::provider(
                &m,
                "https://example.test",
                user,
                "secret-model-key",
                7,
                None,
            )
            .unwrap();
            let id = p.id.clone();
            ProviderService::add_without_switching(&store, app.clone(), p).unwrap();
            ProviderService::switch(&store, app.clone(), &id).unwrap();
            adapter::select_default(&app, &id, &m.model).unwrap();
        }
        let providers_before: HashMap<_, _> = adapter::AGENTS
            .iter()
            .map(|agent| {
                (
                    *agent,
                    serde_json::to_value(store.db.get_all_providers(agent).unwrap()).unwrap(),
                )
            })
            .collect();
        let files_before: HashMap<_, _> = adapter::AGENTS
            .iter()
            .flat_map(|agent| sync_report::paths(&adapter::app(agent).unwrap()).unwrap())
            .map(|path| {
                let bytes = crate::live::engine::read_current(&path).unwrap();
                (path, bytes)
            })
            .collect();
        let auth = PlatformAuthState::for_test("https://example.test".into(), 1);
        tauri::async_runtime::block_on(async {
            auth.logout().await.unwrap();
            assert!(matches!(
                auth.with_local_identity(|_, _| Ok(())).await,
                Err(err) if err.code == "UNAUTHORIZED"
            ));
            // A repeated logout must also leave provider rows and native files untouched.
            auth.logout().await.unwrap();
        });
        for agent in adapter::AGENTS {
            assert_eq!(
                serde_json::to_value(store.db.get_all_providers(agent).unwrap()).unwrap(),
                providers_before[agent],
                "{agent}"
            );
        }
        for (path, before) in files_before {
            assert_eq!(
                crate::live::engine::read_current(&path).unwrap(),
                before,
                "{}",
                path.display()
            );
        }
    }

    fn deletion_provider(agent: &str, id: &str, origin: &str, user_id: i64) -> Provider {
        let model: AgentModel = serde_json::from_value(json!({
            "id": "model", "agent": agent, "model": "claude-sonnet-5", "name": "Test",
            "protocol": match agent { "claude" | "claude-desktop" => "anthropic", "gemini" => "gemini", "workbuddy" => "chat", _ => "responses" },
            "category": "available", "available": true, "supportsTools": true, "supportsStreaming": true
        })).unwrap();
        let mut provider =
            adapter::provider(&model, origin, user_id, "test-only-key", 42, None).unwrap();
        provider.id = id.into();
        provider.name = id.into();
        provider
    }

    #[test]
    #[serial_test::serial]
    fn platform_deletion_is_local_account_scoped_and_supported_for_every_agent() {
        let home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let origin = "https://platform.test";
        let auth = PlatformAuthState::for_test(origin.into(), 1);
        let catalog_path = home._temp.path().join("catalog.json");
        base_data::CatalogStore::for_test(catalog_path.clone())
            .save(test_discovery(Vec::new()))
            .unwrap();
        let catalog_path = base_data::CatalogStore::for_test(catalog_path).account_path(origin, 1);
        let catalog_before = std::fs::read(&catalog_path).unwrap();
        for agent in adapter::AGENTS {
            for (id, base, user) in [
                ("target", origin, 1),
                ("other-user", origin, 2),
                ("other-platform", "https://other.test", 1),
            ] {
                state
                    .db
                    .save_provider(agent, &deletion_provider(agent, id, base, user))
                    .unwrap();
            }
            let mut manual = deletion_provider(agent, "manual", origin, 1);
            manual.meta.as_mut().unwrap().platform_managed = None;
            state.db.save_provider(agent, &manual).unwrap();
        }
        for agent in adapter::AGENTS {
            let app = adapter::app(agent).unwrap();
            let before = state.db.get_all_providers(agent).unwrap();
            let result = delete::delete_for_identity(&auth, &state, &app, origin, 1).unwrap();
            assert_eq!(
                (result.removed, result.skipped),
                (1, 0),
                "{agent}: {:?}",
                result.warnings
            );
            let after = state.db.get_all_providers(agent).unwrap();
            assert!(!after.contains_key("target"), "{agent}");
            for id in ["manual", "other-user", "other-platform"] {
                assert_eq!(
                    serde_json::to_value(&before[id]).unwrap(),
                    serde_json::to_value(&after[id]).unwrap(),
                    "{agent}: {id}"
                );
            }
            let record = crate::config_history::list(Some(agent.to_string()))
                .unwrap()
                .into_iter()
                .find(|record| record.action == "platform_delete" && record.sync.is_some())
                .unwrap();
            assert_eq!(record.sync.unwrap().removed, 1);
            let repeated = delete::delete_for_identity(&auth, &state, &app, origin, 1).unwrap();
            assert_eq!((repeated.removed, repeated.skipped), (0, 0));
        }
        assert_eq!(std::fs::read(catalog_path).unwrap(), catalog_before);
    }

    #[test]
    #[serial_test::serial]
    fn platform_deletion_all_agents_works_without_catalog_and_preserves_unmanaged_rows() {
        let _home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let origin = "https://platform.test";
        let auth = PlatformAuthState::for_test(origin.into(), 1);
        for agent in adapter::AGENTS {
            for (id, base, user) in [
                ("target", origin, 1),
                ("other-user", origin, 2),
                ("other-platform", "https://other.test", 1),
            ] {
                state
                    .db
                    .save_provider(agent, &deletion_provider(agent, id, base, user))
                    .unwrap();
            }
            let mut manual = deletion_provider(agent, "manual", origin, 1);
            manual.meta.as_mut().unwrap().platform_managed = None;
            state.db.save_provider(agent, &manual).unwrap();
        }
        let result = delete::delete_all_for_identity(&auth, &state, origin, 1).unwrap();
        assert_eq!(
            result.removed,
            adapter::AGENTS.len(),
            "{:?}",
            result.warnings
        );
        assert_eq!(result.skipped, 0);
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        assert_eq!(result.apps.len(), adapter::AGENTS.len());
        for (app, agent) in result.apps.iter().zip(adapter::AGENTS) {
            assert_eq!(app.agent, *agent);
            assert_eq!((app.removed, app.skipped), (1, 0));
            assert!(app.error.is_none());
            let after = state.db.get_all_providers(agent).unwrap();
            assert!(!after.contains_key("target"), "{agent}");
            for id in ["manual", "other-user", "other-platform"] {
                assert!(after.contains_key(id), "{agent}: {id}");
            }
            let report = crate::config_history::list(Some((*agent).into()))
                .unwrap()
                .into_iter()
                .find(|record| record.action == "platform_delete" && record.sync.is_some())
                .unwrap()
                .sync
                .unwrap();
            assert_eq!(report.removed, 1);
        }
        let serialized = serde_json::to_value(&result).unwrap();
        assert_eq!(serialized["apps"][0]["agent"], "claude");
        assert!(serialized["apps"][0]["nativeFilesChanged"].is_number());
        assert!(serialized["apps"][0].get("error").is_none());
        let repeated = delete::delete_all_for_identity(&auth, &state, origin, 1).unwrap();
        assert_eq!((repeated.removed, repeated.skipped), (0, 0));
        assert_eq!(repeated.apps.len(), adapter::AGENTS.len());
    }

    #[test]
    #[serial_test::serial]
    fn platform_deletion_all_agents_continues_after_app_errors_and_unsafe_current() {
        let home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let origin = "https://platform.test";
        let auth = PlatformAuthState::for_test(origin.into(), 1);
        for agent in adapter::AGENTS {
            state
                .db
                .save_provider(agent, &deletion_provider(agent, "target", origin, 1))
                .unwrap();
        }
        ProviderService::switch(&state, adapter::app("claude").unwrap(), "target").unwrap();
        let claude_path = crate::config::get_claude_settings_path();
        let claude_before = std::fs::read(&claude_path).unwrap();
        // Directory in place of a file fails before deletion; later apps must still run.
        let codex_path = crate::codex_config::get_codex_config_path();
        assert!(codex_path.starts_with(home._temp.path()));
        std::fs::create_dir_all(&codex_path).unwrap();
        let workbuddy_path = crate::workbuddy_config::get_workbuddy_models_path();
        std::fs::create_dir_all(workbuddy_path.parent().unwrap()).unwrap();
        std::fs::write(&workbuddy_path, b"{broken JSON").unwrap();
        let catalog_path = home._temp.path().join("catalog.json");
        base_data::CatalogStore::for_test(catalog_path.clone())
            .save(test_discovery(Vec::new()))
            .unwrap();
        let catalog_path = base_data::CatalogStore::for_test(catalog_path).account_path(origin, 1);
        let catalog_before = std::fs::read(&catalog_path).unwrap();

        let result = delete::delete_all_for_identity(&auth, &state, origin, 1).unwrap();
        assert_eq!(
            result.removed,
            adapter::AGENTS.len() - 3,
            "{:?}",
            result.warnings
        );
        assert_eq!(result.skipped, 2);
        assert_eq!(result.apps.len(), adapter::AGENTS.len());
        assert_eq!(
            result.native_files_changed,
            result
                .apps
                .iter()
                .map(|app| app.native_files_changed)
                .sum::<usize>()
        );
        for app in &result.apps {
            let row = state.db.get_provider_by_id("target", &app.agent).unwrap();
            match app.agent.as_str() {
                "codex" => {
                    assert!(row.is_some());
                    assert_eq!(app.error.as_ref().unwrap().code, "CONFIG");
                }
                "claude" | "workbuddy" => {
                    assert!(row.is_some());
                    assert_eq!(app.skipped, 1);
                    assert!(!app.warnings.is_empty());
                    assert!(app.error.is_none());
                    let report = crate::config_history::list(Some(app.agent.clone()))
                        .unwrap()
                        .into_iter()
                        .find(|record| record.action == "platform_delete" && record.sync.is_some())
                        .unwrap()
                        .sync
                        .unwrap();
                    assert_eq!(report.removed, 0);
                    assert!(!report.warnings.is_empty());
                }
                _ => {
                    assert!(row.is_none(), "{}", app.agent);
                    assert_eq!(app.removed, 1);
                    assert!(app.error.is_none());
                }
            }
        }
        assert_eq!(std::fs::read(claude_path).unwrap(), claude_before);
        assert_eq!(std::fs::read(workbuddy_path).unwrap(), b"{broken JSON");
        assert!(codex_path.is_dir());
        assert_eq!(std::fs::read(catalog_path).unwrap(), catalog_before);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.starts_with("codex：")));
        let serialized = serde_json::to_value(result).unwrap();
        let codex = serialized["apps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|app| app["agent"] == "codex")
            .unwrap();
        assert_eq!(codex["error"]["code"], "CONFIG");
    }

    #[test]
    #[serial_test::serial]
    fn platform_deletion_rejects_stale_and_missing_identity_without_writes() {
        let _home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let app = adapter::app("claude").unwrap();
        state
            .db
            .save_provider(
                "claude",
                &deletion_provider("claude", "target", "https://platform.test", 1),
            )
            .unwrap();
        for auth in [
            PlatformAuthState::default(),
            PlatformAuthState::for_test("https://platform.test".into(), 2),
            PlatformAuthState::for_test("https://other.test".into(), 1),
        ] {
            assert!(
                matches!(delete::delete_for_identity(&auth, &state, &app, "https://platform.test", 1), Err(err) if err.code == "UNAUTHORIZED")
            );
            assert!(
                matches!(delete::delete_all_for_identity(&auth, &state, "https://platform.test", 1), Err(err) if err.code == "UNAUTHORIZED")
            );
        }
        assert!(state
            .db
            .get_provider_by_id("target", "claude")
            .unwrap()
            .is_some());
        assert!(crate::config_history::list(Some("claude".into()))
            .unwrap()
            .is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn platform_deletion_switches_to_manual_fallback_and_keeps_backups() {
        let _home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let app = adapter::app("claude").unwrap();
        let target = deletion_provider("claude", "target", "https://platform.test", 1);
        ProviderService::add_without_switching(&state, app.clone(), target).unwrap();
        ProviderService::switch(&state, app.clone(), "target").unwrap();
        let mut manual = deletion_provider("claude", "manual", "https://platform.test", 1);
        manual.meta.as_mut().unwrap().platform_managed = None;
        manual.settings_config["env"]["ANTHROPIC_AUTH_TOKEN"] = json!("manual-key");
        ProviderService::add_without_switching(&state, app.clone(), manual).unwrap();
        let result =
            delete::delete_account_providers(&state, &app, "https://platform.test", 1).unwrap();
        assert_eq!(
            (result.removed, result.skipped),
            (1, 0),
            "{:?}",
            result.warnings
        );
        assert_eq!(ProviderService::current(&state, app).unwrap(), "manual");
        assert!(state
            .db
            .get_provider_by_id("target", "claude")
            .unwrap()
            .is_none());
        let live: Value = serde_json::from_slice(
            &std::fs::read(crate::config::get_claude_settings_path()).unwrap(),
        )
        .unwrap();
        assert_eq!(live["env"]["ANTHROPIC_AUTH_TOKEN"], "manual-key");
        assert!(crate::live::engine::DeviceStore::for_device()
            .first_write_backup_dir()
            .is_dir());
        assert!(result.native_files_changed > 0);
    }

    #[test]
    #[serial_test::serial]
    fn platform_deletion_retains_current_without_fallback_and_reports_partial_success() {
        let _home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let app = adapter::app("claude").unwrap();
        for id in ["current", "unused"] {
            ProviderService::add_without_switching(
                &state,
                app.clone(),
                deletion_provider("claude", id, "https://platform.test", 1),
            )
            .unwrap();
        }
        ProviderService::switch(&state, app.clone(), "current").unwrap();
        let before = std::fs::read(crate::config::get_claude_settings_path()).unwrap();
        let result =
            delete::delete_account_providers(&state, &app, "https://platform.test", 1).unwrap();
        assert_eq!((result.removed, result.skipped), (1, 1));
        assert!(result.warnings[0].contains("备用供应商"));
        assert_eq!(
            std::fs::read(crate::config::get_claude_settings_path()).unwrap(),
            before
        );
        assert!(state
            .db
            .get_provider_by_id("current", "claude")
            .unwrap()
            .is_some());
        assert!(state
            .db
            .get_provider_by_id("unused", "claude")
            .unwrap()
            .is_none());
        let report = crate::config_history::list(Some("claude".into()))
            .unwrap()
            .into_iter()
            .find(|record| record.action == "platform_delete" && record.sync.is_some())
            .unwrap()
            .sync
            .unwrap();
        assert_eq!(report.removed, 1);
        assert_eq!(report.warnings, result.warnings);
        assert!(report.changed_files.is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn platform_deletion_removes_live_workbuddy_models_and_preserves_other_settings() {
        let _home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let app = adapter::app("workbuddy").unwrap();
        ProviderService::add_without_switching(
            &state,
            app.clone(),
            deletion_provider("workbuddy", "target", "https://platform.test", 1),
        )
        .unwrap();
        ProviderService::switch(&state, app.clone(), "target").unwrap();
        let path = crate::workbuddy_config::get_workbuddy_models_path();
        let native: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let models = if native.is_array() {
            native
        } else {
            native["models"].clone()
        };
        assert!(!models.as_array().unwrap().is_empty());
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({ "models": models, "userPreference": "keep" })).unwrap(),
        )
        .unwrap();
        let result =
            delete::delete_account_providers(&state, &app, "https://platform.test", 1).unwrap();
        assert_eq!(
            (result.removed, result.skipped),
            (1, 0),
            "{:?}",
            result.warnings
        );
        let after: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(after["models"], json!([]));
        assert_eq!(after["userPreference"], "keep");
        assert!(state
            .db
            .get_provider_by_id("target", "workbuddy")
            .unwrap()
            .is_none());
        let result = serde_json::to_value(result).unwrap();
        assert_eq!(result["nativeFilesChanged"], 1);
    }

    #[test]
    #[serial_test::serial]
    fn platform_deletion_preserves_malformed_native_config_and_provider_row() {
        let _home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let app = adapter::app("workbuddy").unwrap();
        let provider = deletion_provider("workbuddy", "target", "https://platform.test", 1);
        state.db.save_provider("workbuddy", &provider).unwrap();
        let path = crate::workbuddy_config::get_workbuddy_models_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{broken JSON").unwrap();
        let result =
            delete::delete_account_providers(&state, &app, "https://platform.test", 1).unwrap();
        assert_eq!((result.removed, result.skipped), (0, 1));
        assert!(!result.warnings.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), b"{broken JSON");
        assert_eq!(
            serde_json::to_value(
                state
                    .db
                    .get_provider_by_id("target", "workbuddy")
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_value(provider).unwrap()
        );
    }

    fn test_discovery(
        groups: Vec<crate::zhishu::services::platform_auth::AvailableGroup>,
    ) -> GroupDiscovery {
        let account = crate::zhishu::services::platform_auth::PlatformAccount {
            base_url: "https://platform.test".into(),
            user: crate::zhishu::services::platform_auth::PlatformUser {
                id: 1,
                email: "test@example.com".into(),
                username: "test".into(),
                role: "user".into(),
                status: "active".into(),
                balance: 1.0,
            },
        };
        let models = vec![json!({
            "id": "claude-sonnet-5",
            "display_name": "Claude Sonnet 5",
            "supported_endpoint_types": ["anthropic"],
            "context_window": 32000,
            "max_output_tokens": 4096,
            "supports_tools": true,
            "supports_streaming": true
        })];
        let catalogs = groups
            .iter()
            .map(
                |group| crate::zhishu::services::platform_auth::GroupCatalog {
                    group: group.clone(),
                    credential: crate::zhishu::services::platform_auth::GroupCredential {
                        key: format!("key-{}", group.id),
                        key_id: group.id,
                        platform: group.platform.clone(),
                        key_name: format!("zhishu-client-{}", group.id),
                    },
                    models: models.clone(),
                    catalog_error: None,
                    connection: None,
                },
            )
            .collect();
        let subscription_group_ids = groups
            .iter()
            .filter(|group| group.subscription_type == "subscription")
            .map(|group| group.id)
            .collect();
        let details = groups
            .iter()
            .map(|group| {
                (
                    group.id,
                    super::super::client_api::GroupDetails {
                        quota: super::super::client_api::ClientQuota {
                            available: true,
                            reasons: Vec::new(),
                        },
                        subscriptions: Vec::new(),
                        credential: None,
                        is_exclusive: false,
                    },
                )
            })
            .collect();
        GroupDiscovery {
            account,
            groups,
            catalogs,
            failed_group_ids: HashSet::new(),
            synced_at: 1793433600000,
            details,
            subscription_group_ids,
            warnings: Vec::new(),
        }
    }

    #[test]
    #[serial_test::serial]
    fn sync_adds_providers_without_touching_existing_config_files() {
        use crate::{app_config::AppType, database::Database};
        use std::sync::Arc;
        let home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let store = base_data::CatalogStore::for_test(home._temp.path().join("catalog.json"));
        store
            .save(test_discovery(vec![
                crate::zhishu::services::platform_auth::AvailableGroup {
                    id: 42,
                    name: "Test group".into(),
                    platform: "anthropic".into(),
                    status: "active".into(),
                    subscription_type: "standard".into(),
                },
            ]))
            .unwrap();
        let path = crate::config::get_claude_settings_path();
        let manual = r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"manual-first"},"custom":"keep"}"#;
        crate::config::atomic_write_private(&path, manual.as_bytes()).unwrap();
        let run = || {
            sync_account_providers(
                &state,
                &[AppType::Claude],
                &store,
                "https://platform.test",
                1,
            )
            .unwrap()
        };
        // There is no pre-sync detection or consent: the provider is added
        // directly, and adding never switches, so the manual file survives.
        let first = run();
        assert_eq!(first.added, 1);
        assert!(first.warnings.is_empty(), "{:?}", first.warnings);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), manual);
        let second = run();
        assert_eq!(
            (second.added, second.updated, second.skipped),
            (0, Some(0), 1)
        );
        assert!(second.warnings.is_empty(), "{:?}", second.warnings);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), manual);
    }

    #[test]
    #[serial_test::serial]
    fn sync_rotates_keys_over_external_edits_and_keeps_unknown_settings() {
        use crate::{app_config::AppType, database::Database};
        use std::sync::Arc;
        let home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let store = base_data::CatalogStore::for_test(home._temp.path().join("catalog.json"));
        let discovery = test_discovery(vec![
            crate::zhishu::services::platform_auth::AvailableGroup {
                id: 42,
                name: "Test group".into(),
                platform: "anthropic".into(),
                status: "active".into(),
                subscription_type: "standard".into(),
            },
        ]);
        store.save(discovery.clone()).unwrap();
        let run = || {
            sync_account_providers(
                &state,
                &[AppType::Claude],
                &store,
                "https://platform.test",
                1,
            )
            .unwrap()
        };
        assert_eq!(run().added, 1);
        let id = state
            .db
            .get_all_providers("claude")
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        ProviderService::switch(&state, AppType::Claude, &id).unwrap();
        let path = crate::config::get_claude_settings_path();
        let mut changed: Value = crate::config::read_json_file(&path).unwrap();
        changed["env"]["ANTHROPIC_AUTH_TOKEN"] = json!("external-secret");
        changed["externalCustom"] = json!({"keep":true});
        crate::config::write_json_file(&path, &changed).unwrap();
        // Unchanged managed rows are no longer force-rewritten, so the
        // external edit survives an otherwise idempotent sync.
        let unchanged = run();
        assert!(unchanged.warnings.is_empty(), "{:?}", unchanged.warnings);
        assert_eq!(unchanged.updated, Some(0));
        assert_eq!(
            crate::config::read_json_file::<Value>(&path).unwrap(),
            changed
        );
        // A rotated key updates the managed row and republishes the file,
        // preserving settings the platform does not manage.
        let mut rotated_discovery = discovery;
        rotated_discovery.catalogs[0].credential.key = "rotated-key".into();
        store.save(rotated_discovery).unwrap();
        let rotated = run();
        assert!(rotated.warnings.is_empty(), "{:?}", rotated.warnings);
        assert_eq!(rotated.updated, Some(1));
        let applied: Value = crate::config::read_json_file(&path).unwrap();
        assert_eq!(applied["env"]["ANTHROPIC_AUTH_TOKEN"], "rotated-key");
        assert_eq!(applied["externalCustom"], changed["externalCustom"]);
    }

    #[test]
    fn cached_catalog_is_persistent_account_scoped_and_returns_no_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("platform-catalog.json");
        let store = base_data::CatalogStore::for_test(path.clone());
        let discovery = test_discovery(vec![
            crate::zhishu::services::platform_auth::AvailableGroup {
                id: 42,
                name: "Test group".into(),
                platform: "anthropic".into(),
                status: "active".into(),
                subscription_type: "standard".into(),
            },
        ]);
        store.save(discovery).unwrap();
        let restarted = base_data::CatalogStore::for_test(path.clone());
        let path = store.account_path("https://platform.test", 1);
        let cached = restarted.read("https://platform.test", 1).unwrap().unwrap();
        assert_eq!(cached.discovery.catalogs[0].credential.key, "key-42");
        let summary = serde_json::to_value(cached.summary()).unwrap();
        assert_eq!(summary["baseUrl"], "https://platform.test");
        assert_eq!(summary["userId"], 1);
        assert_eq!(summary["groups"][0]["models"][0]["id"], "claude-sonnet-5");
        assert_eq!(summary["groups"][0]["keyId"], 42);
        assert_eq!(summary["groups"][0]["keyName"], "zhishu-client-42");
        assert!(!summary.to_string().contains("key-42"));
        assert_eq!(
            restarted.group_key("https://platform.test", 1, 42).unwrap(),
            Some("key-42".into())
        );
        assert!(restarted
            .group_key("https://platform.test", 1, 999)
            .unwrap()
            .is_none());
        assert!(restarted.group_key("https://platform.test", 2, 42).is_err());
        assert!(restarted.group_key("https://other.test", 1, 42).is_err());
        assert!(restarted.read("https://other.test", 1).unwrap().is_none());
        assert!(restarted
            .read("https://platform.test", 2)
            .unwrap()
            .is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        // Missing credential content is corruption, not a separate lifecycle state.
        let mut incomplete: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        incomplete["discovery"]["catalogs"][0]["credential"]["key"] = json!(" ");
        for corrupted in [
            serde_json::to_vec(&incomplete).unwrap(),
            b"broken snapshot with key-42".to_vec(),
        ] {
            std::fs::write(&path, &corrupted).unwrap();
            assert!(restarted.read("https://platform.test", 1).is_err());
            assert!(restarted.group_key("https://platform.test", 1, 42).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), corrupted);
        }
    }

    #[tokio::test]
    async fn logout_preserves_catalog_and_relogin_requires_the_same_account() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("catalog.json");
        let store = base_data::CatalogStore::for_test(path.clone());
        let discovery = test_discovery(vec![
            crate::zhishu::services::platform_auth::AvailableGroup {
                id: 42,
                name: "Retained group".into(),
                platform: "anthropic".into(),
                status: "active".into(),
                subscription_type: "standard".into(),
            },
        ]);
        store.save(discovery).unwrap();
        let path = store.account_path("https://platform.test", 1);
        let before_logout = std::fs::read(&path).unwrap();
        let auth = PlatformAuthState::for_test("https://platform.test".into(), 1);
        auth.logout().await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before_logout);
        assert!(matches!(
            auth.with_local_identity(|origin, user_id| store.group_key(origin, user_id, 42)).await,
            Err(err) if err.code == "UNAUTHORIZED"
        ));
        let relogged = PlatformAuthState::for_test("https://platform.test".into(), 1);
        assert_eq!(
            relogged
                .with_local_identity(|origin, user_id| store.group_key(origin, user_id, 42))
                .await
                .unwrap(),
            Some("key-42".into())
        );
        let other = PlatformAuthState::for_test("https://platform.test".into(), 2);
        assert!(matches!(
            other.with_local_identity(|origin, user_id| store.group_key(origin, user_id, 42)).await,
            Err(err) if err.code == "CATALOG_REQUIRED"
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before_logout);
    }

    #[test]
    #[serial_test::serial]
    fn cached_provider_sync_requires_current_accounts_valid_base_data() {
        use crate::{app_config::AppType, database::Database};
        use std::sync::Arc;
        let home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let path = home._temp.path().join("catalog.json");
        let store = base_data::CatalogStore::for_test(path.clone());
        let sync = || {
            sync_account_providers(
                &state,
                &[AppType::Claude],
                &store,
                "https://platform.test",
                1,
            )
        };
        assert!(matches!(sync(), Err(err) if err.code == "CATALOG_REQUIRED"));
        std::fs::write(&path, b"invalid json").unwrap();
        assert!(matches!(sync(), Err(err) if err.code == "CATALOG_STORE"));
        std::fs::remove_file(&path).unwrap();
        let mut discovery = test_discovery(Vec::new());
        discovery.account.user.id = 2;
        store.save(discovery).unwrap();
        assert!(matches!(sync(), Err(err) if err.code == "CATALOG_REQUIRED"));
        assert!(state.db.get_all_providers("claude").unwrap().is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn cached_provider_sync_is_local_idempotent_and_preserves_failed_groups() {
        use crate::{app_config::AppType, database::Database};
        use std::sync::Arc;
        let home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let store = base_data::CatalogStore::for_test(home._temp.path().join("catalog.json"));
        let discovery = test_discovery(vec![
            crate::zhishu::services::platform_auth::AvailableGroup {
                id: 42,
                name: "Test group".into(),
                platform: "anthropic".into(),
                status: "active".into(),
                subscription_type: "standard".into(),
            },
        ]);
        store.save(discovery).unwrap();
        // Layer 1 saves data without touching any Agent list.
        assert!(state.db.get_all_providers("claude").unwrap().is_empty());
        let sync = || {
            sync_account_providers(
                &state,
                &[AppType::Claude],
                &store,
                "https://platform.test",
                1,
            )
            .unwrap()
        };
        let first = sync();
        assert_eq!((first.added, first.removed), (1, 0));
        assert!(first.warnings.is_empty());
        let second = sync();
        assert_eq!((second.added, second.removed, second.skipped), (0, 0, 1));
        assert!(state.db.get_all_providers("codex").unwrap().is_empty());
        let mut partial = store
            .read("https://platform.test", 1)
            .unwrap()
            .unwrap()
            .discovery;
        partial.catalogs.clear();
        partial.failed_group_ids.insert(42);
        partial.warnings.push("Temporary group failure".into());
        store.save(partial).unwrap();
        let third = sync();
        assert_eq!((third.added, third.removed), (0, 0));
        assert_eq!(third.warnings, vec!["Temporary group failure"]);
        assert_eq!(state.db.get_all_providers("claude").unwrap().len(), 1);
    }

    #[test]
    #[serial_test::serial]
    fn subscription_sync_keeps_expired_and_exhausted_rows_and_migrates_legacy_metadata() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let app = crate::app_config::AppType::Claude;
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 14,
            name: "月度订阅".into(),
            platform: "anthropic".into(),
            status: "active".into(),
            subscription_type: "subscription".into(),
        };
        let active = test_discovery(vec![group]);
        let (added, _, _, warnings) = sync_agent_providers(&state, &app, &active).unwrap();
        assert_eq!(added, 1);
        assert!(warnings.is_empty());
        let id =
            adapter::service_provider_id("claude", 14, "anthropic", "https://platform.test", 1);
        let mut provider = state.db.get_provider_by_id(&id, "claude").unwrap().unwrap();
        assert_eq!(
            provider
                .meta
                .as_ref()
                .unwrap()
                .platform_managed
                .as_ref()
                .unwrap()
                .subscription_type
                .as_deref(),
            Some("subscription")
        );
        provider.settings_config["customSetting"] = json!("keep");
        // Simulate an old client row with no persisted subscription type.
        provider
            .meta
            .as_mut()
            .unwrap()
            .platform_managed
            .as_mut()
            .unwrap()
            .subscription_type = None;
        state.db.save_provider("claude", &provider).unwrap();
        let original = provider.settings_config.clone();
        let manual = Provider::with_id(
            "manual-subscription".into(),
            "Manual".into(),
            json!({"env":{"ANTHROPIC_API_KEY":"manual-key"}}),
            None,
        );
        state.db.save_provider("claude", &manual).unwrap();
        let mut other_account = provider.clone();
        other_account.id = "other-subscription-account".into();
        other_account
            .meta
            .as_mut()
            .unwrap()
            .platform_managed
            .as_mut()
            .unwrap()
            .user_id = 2;
        state.db.save_provider("claude", &other_account).unwrap();
        let mut expired = active.clone();
        expired.catalogs[0].models.clear();
        expired.details.get_mut(&14).unwrap().quota = super::super::client_api::ClientQuota {
            available: false,
            reasons: vec!["subscription_expired".into()],
        };
        let (_, removed, _, warnings) = sync_agent_providers(&state, &app, &expired).unwrap();
        assert_eq!(removed, 0);
        assert!(warnings.is_empty());
        let retained = state.db.get_provider_by_id(&id, "claude").unwrap().unwrap();
        assert_eq!(retained.settings_config, original);
        let managed = retained.meta.unwrap().platform_managed.unwrap();
        assert_eq!(managed.subscription_type.as_deref(), Some("subscription"));
        assert_eq!(
            serde_json::to_value(&managed).unwrap()["subscriptionType"],
            "subscription"
        );
        let untouched = state
            .db
            .get_provider_by_id("other-subscription-account", "claude")
            .unwrap()
            .unwrap();
        assert_eq!(untouched.settings_config, original);
        assert!(untouched
            .meta
            .unwrap()
            .platform_managed
            .unwrap()
            .subscription_type
            .is_none());
        assert_eq!(
            state
                .db
                .get_provider_by_id("manual-subscription", "claude")
                .unwrap()
                .unwrap()
                .settings_config,
            manual.settings_config
        );

        let mut exhausted = active;
        exhausted.catalogs.clear();
        exhausted.failed_group_ids.insert(14);
        let (_, removed, _, _) = sync_agent_providers(&state, &app, &exhausted).unwrap();
        assert_eq!(removed, 0);
        assert_eq!(
            state
                .db
                .get_provider_by_id(&id, "claude")
                .unwrap()
                .unwrap()
                .settings_config,
            original
        );
        // A successful access snapshot with no group means authorization was withdrawn.
        let (_, removed, _, _) =
            sync_agent_providers(&state, &app, &test_discovery(Vec::new())).unwrap();
        assert_eq!(removed, 1);
        assert!(state
            .db
            .get_provider_by_id(&id, "claude")
            .unwrap()
            .is_none());
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_adds_skips_and_removes_only_matching_managed_rows() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let app = crate::app_config::AppType::Claude;
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 42,
            name: "Claude 主力".into(),
            platform: "anthropic".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let discovery = test_discovery(vec![group.clone()]);
        let (added, removed, skipped, warnings) =
            sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (1, 0, 0));
        assert!(warnings.is_empty());
        let id =
            adapter::service_provider_id("claude", 42, "anthropic", "https://platform.test", 1);
        let provider = state.db.get_provider_by_id(&id, "claude").unwrap().unwrap();
        let managed = provider.meta.unwrap().platform_managed.unwrap();
        assert_eq!(managed.group_id, Some(42));
        assert_eq!(managed.group_platform.as_deref(), Some("anthropic"));
        let mut customized = state.db.get_provider_by_id(&id, "claude").unwrap().unwrap();
        customized.settings_config["customSetting"] = json!("keep");
        state.db.save_provider("claude", &customized).unwrap();

        let (added, removed, skipped, _) = sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (0, 0, 1));
        let restored = state.db.get_provider_by_id(&id, "claude").unwrap().unwrap();
        let (_, restored_key) = restored.resolve_usage_credentials(&app);
        assert_eq!(restored_key, "key-42");
        assert_eq!(restored.settings_config["customSetting"], "keep");

        let (added, removed, skipped, _) =
            sync_agent_providers(&state, &app, &test_discovery(Vec::new())).unwrap();
        assert_eq!((added, removed, skipped), (0, 1, 0));
        assert!(state
            .db
            .get_provider_by_id(&id, "claude")
            .unwrap()
            .is_none());
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_builds_one_working_provider_for_every_supported_agent() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 42,
            name: "全协议分组".into(),
            platform: "composite".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let mut discovery = test_discovery(vec![group]);
        discovery.catalogs[0].models = vec![json!({
            "id": "claude-sonnet-5",
            "display_name": "Claude Sonnet 5",
            "supported_endpoint_types": ["anthropic", "responses", "chat", "gemini"],
            "context_window": 32000,
            "max_output_tokens": 4096,
            "input_modalities": ["text", "image"],
            "supports_tools": true,
            "supports_streaming": true
        })];

        for agent in adapter::AGENTS {
            let app = adapter::app(agent).unwrap();
            let (added, removed, skipped, warnings) =
                sync_agent_providers(&state, &app, &discovery).unwrap();
            assert_eq!(
                (added, removed, skipped),
                (1, 0, 0),
                "{agent}: {warnings:?}"
            );
            assert!(warnings.is_empty(), "{agent}: {warnings:?}");

            let providers = state.db.get_all_providers(agent).unwrap();
            assert_eq!(providers.len(), 1, "{agent}");
            let id =
                adapter::service_provider_id(agent, 42, "composite", "https://platform.test", 1);
            let provider = providers.get(&id).expect(agent);
            assert_eq!(provider.name, "智枢平台 · 全协议分组", "{agent}");
            assert_eq!(provider.resolve_usage_credentials(&app).1, "key-42");
            let managed = provider
                .meta
                .as_ref()
                .and_then(|meta| meta.platform_managed.as_ref())
                .unwrap_or_else(|| panic!("{agent} lost platform ownership"));
            assert_eq!(managed.group_id, Some(42), "{agent}");
            assert_eq!(managed.origin, "https://platform.test", "{agent}");
            assert_eq!(managed.user_id, 1, "{agent}");
            assert_eq!(managed.model, "claude-sonnet-5", "{agent}");

            ProviderService::switch(&state, app.clone(), &id).unwrap();
            adapter::select_default(&app, &id, "claude-sonnet-5").unwrap();
            assert!(
                adapter::is_selected(&state, &app, &id, "claude-sonnet-5").unwrap(),
                "{agent}"
            );
        }
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_supports_gemini_grok_openclaw_pi_and_kimi_without_limits() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 88,
            name: "平台兼容分组".into(),
            platform: "composite".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let mut discovery = test_discovery(vec![group]);
        discovery.catalogs[0].models = vec![
            json!({"id": "gemini-3.1-pro"}),
            json!({"id": "gpt-5.6-sol"}),
        ];

        for app in [
            crate::app_config::AppType::Gemini,
            crate::app_config::AppType::GrokBuild,
            crate::app_config::AppType::OpenClaw,
            crate::app_config::AppType::Pi,
            crate::app_config::AppType::Kimi,
        ] {
            let (added, removed, skipped, warnings) =
                sync_agent_providers(&state, &app, &discovery).unwrap();
            assert_eq!(
                (added, removed, skipped),
                (1, 0, 0),
                "{app:?}: {warnings:?}"
            );
            assert!(warnings.is_empty(), "{app:?}: {warnings:?}");

            let id = adapter::service_provider_id(
                app.as_str(),
                88,
                "composite",
                "https://platform.test",
                1,
            );
            let provider = state
                .db
                .get_provider_by_id(&id, app.as_str())
                .unwrap()
                .unwrap_or_else(|| panic!("{app:?} provider"));
            assert!(
                provider
                    .settings_config
                    .to_string()
                    .contains("gemini-3.1-pro")
                    || provider.settings_config.to_string().contains("gpt-5.6-sol"),
                "{app:?}"
            );
        }
    }

    #[test]
    #[serial_test::serial]
    fn gemini_sync_reports_groups_without_native_models() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 89,
            name: "智枢router-ccs-gpt".into(),
            platform: "composite".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let mut discovery = test_discovery(vec![group]);
        discovery.catalogs[0].models = vec![json!({"id": "gpt-5.6-sol"})];

        let (added, removed, skipped, warnings) =
            sync_agent_providers(&state, &crate::app_config::AppType::Gemini, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (0, 0, 0));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("没有 Gemini 原生模型"), "{warnings:?}");
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_adds_claude_desktop_row() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 42,
            name: "Claude 主力".into(),
            platform: "anthropic".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let app = crate::app_config::AppType::ClaudeDesktop;
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let discovery = test_discovery(vec![group.clone()]);
        let (added, removed, skipped, warnings) =
            sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (1, 0, 0), "{app:?}");
        assert!(warnings.is_empty(), "{app:?}: {warnings:?}");
        let id = adapter::service_provider_id(
            app.as_str(),
            group.id,
            &group.platform,
            "https://platform.test",
            1,
        );
        let provider = state
            .db
            .get_provider_by_id(&id, app.as_str())
            .unwrap()
            .unwrap();
        assert_eq!(
            provider
                .meta
                .as_ref()
                .and_then(|meta| meta.claude_desktop_mode.as_ref()),
            Some(&crate::provider::ClaudeDesktopMode::Direct)
        );
        assert_eq!(
            provider.settings_config["env"]["ANTHROPIC_BASE_URL"],
            "https://platform.test"
        );
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_repairs_generated_endpoint_when_a_group_changes_protocol() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        for (app, base_path) in [
            (
                crate::app_config::AppType::Claude,
                "/env/ANTHROPIC_BASE_URL",
            ),
            (
                crate::app_config::AppType::ClaudeDesktop,
                "/env/ANTHROPIC_BASE_URL",
            ),
            (crate::app_config::AppType::Pi, "/baseUrl"),
            (crate::app_config::AppType::OpenClaw, "/baseUrl"),
            (crate::app_config::AppType::Hermes, "/base_url"),
            (crate::app_config::AppType::Mcode, "/options/baseURL"),
        ] {
            let state = AppState::new(Arc::new(Database::memory().unwrap()));
            let group = crate::zhishu::services::platform_auth::AvailableGroup {
                id: 42,
                name: "混合分组".into(),
                platform: "composite".into(),
                status: "active".into(),
                subscription_type: "standard".into(),
            };
            let mut discovery = test_discovery(vec![group]);
            discovery.catalogs[0].models = vec![json!({
                "id": "model-a",
                "display_name": "Model A",
                "supported_endpoint_types": ["responses"],
                "context_window": 32000,
                "max_output_tokens": 4096,
                "supports_tools": true,
                "supports_streaming": true
            })];
            let (added, removed, skipped, warnings) =
                sync_agent_providers(&state, &app, &discovery).unwrap();
            assert_eq!((added, removed, skipped), (1, 0, 0), "{app:?}");
            assert!(warnings.is_empty(), "{app:?}: {warnings:?}");
            let id = adapter::service_provider_id(
                app.as_str(),
                42,
                "composite",
                "https://platform.test",
                1,
            );
            let before = state
                .db
                .get_provider_by_id(&id, app.as_str())
                .unwrap()
                .unwrap();
            assert_eq!(
                before
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.api_format.as_deref()),
                if app == crate::app_config::AppType::Pi {
                    None
                } else {
                    Some("openai_responses")
                },
                "{app:?}"
            );
            assert_eq!(
                before.settings_config.pointer(base_path).unwrap(),
                "https://platform.test/v1",
                "{app:?}"
            );

            discovery.catalogs[0].models[0]["supported_endpoint_types"] = json!(["anthropic"]);
            let (added, removed, skipped, warnings) =
                sync_agent_providers(&state, &app, &discovery).unwrap();
            assert_eq!((added, removed, skipped), (0, 0, 1), "{app:?}");
            assert!(warnings.is_empty(), "{app:?}: {warnings:?}");
            let after = state
                .db
                .get_provider_by_id(&id, app.as_str())
                .unwrap()
                .unwrap();
            assert_eq!(
                after
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.api_format.as_deref()),
                if app == crate::app_config::AppType::Pi {
                    None
                } else {
                    Some("anthropic")
                },
                "{app:?}"
            );
            assert_eq!(
                after.settings_config.pointer(base_path).unwrap(),
                "https://platform.test",
                "{app:?}"
            );
            assert_eq!(after.resolve_usage_credentials(&app).1, "key-42", "{app:?}");
        }
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_repairs_only_legacy_native_anthropic_base_urls() {
        use crate::app_config::AppType;
        use crate::database::Database;
        use std::sync::Arc;

        for (app, base_path) in [
            (AppType::Pi, "/baseUrl"),
            (AppType::OpenClaw, "/baseUrl"),
            (AppType::Hermes, "/base_url"),
            (AppType::Mcode, "/options/baseURL"),
        ] {
            for (before, expected) in [
                ("https://platform.test/v1", "https://platform.test"),
                ("https://platform.test", "https://platform.test"),
                (
                    "https://custom.test/anthropic",
                    "https://custom.test/anthropic",
                ),
            ] {
                let _home = SyncTestHome::new();
                let state = AppState::new(Arc::new(Database::memory().unwrap()));
                let discovery = test_discovery(vec![
                    crate::zhishu::services::platform_auth::AvailableGroup {
                        id: 42,
                        name: "Claude 主力".into(),
                        platform: "anthropic".into(),
                        status: "active".into(),
                        subscription_type: "standard".into(),
                    },
                ]);
                let (_, _, _, warnings) = sync_agent_providers(&state, &app, &discovery).unwrap();
                assert!(warnings.is_empty(), "{warnings:?}");
                let id = adapter::service_provider_id(
                    app.as_str(),
                    42,
                    "anthropic",
                    "https://platform.test",
                    1,
                );
                let mut provider = state
                    .db
                    .get_provider_by_id(&id, app.as_str())
                    .unwrap()
                    .unwrap();
                *provider.settings_config.pointer_mut(base_path).unwrap() = json!(before);
                provider.sort_index = Some(7);
                state.db.save_provider(app.as_str(), &provider).unwrap();
                ProviderService::switch(&state, app.clone(), &id).unwrap();
                adapter::select_default(&app, &id, "claude-sonnet-5").unwrap();

                // Repeated syncs repair an enabled route without rotating its key,
                // changing the selection, or overwriting a customized endpoint.
                for _ in 0..2 {
                    let (added, removed, skipped, warnings) =
                        sync_agent_providers(&state, &app, &discovery).unwrap();
                    assert_eq!((added, removed, skipped), (0, 0, 1));
                    assert!(warnings.is_empty(), "{warnings:?}");
                    let refreshed = state
                        .db
                        .get_provider_by_id(&id, app.as_str())
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        refreshed.settings_config.pointer(base_path).unwrap(),
                        expected,
                        "{app:?}"
                    );
                    assert_eq!(refreshed.sort_index, Some(7));
                    assert_eq!(refreshed.created_at, provider.created_at);
                    assert_eq!(refreshed.resolve_usage_credentials(&app).1, "key-42");
                    let native = match app {
                        AppType::Pi => crate::pi_config::read_pi_native_provider(&id)
                            .unwrap()
                            .unwrap(),
                        AppType::OpenClaw => {
                            crate::openclaw_config::get_providers().unwrap()[&id].clone()
                        }
                        AppType::Hermes => {
                            crate::hermes_config::get_providers().unwrap()[&id].clone()
                        }
                        AppType::Mcode => {
                            crate::mcode_config::get_providers().unwrap()[&id].clone()
                        }
                        _ => unreachable!(),
                    };
                    assert_eq!(native.pointer(base_path).unwrap(), expected, "{app:?}");
                    assert!(
                        adapter::is_selected(&state, &app, &id, "claude-sonnet-5").unwrap(),
                        "{app:?}"
                    );
                }
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_adds_one_workbuddy_provider_with_all_models() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let app = crate::app_config::AppType::WorkBuddy;
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 42,
            name: "OpenAI 主力".into(),
            platform: "openai".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let mut discovery = test_discovery(vec![group]);
        discovery.catalogs[0].models = vec![
            json!({
                "id": "model-a",
                "display_name": "Model A",
                "supported_endpoint_types": ["chat"],
                "context_window": 128000,
                "max_output_tokens": 16384,
                "input_modalities": ["text", "image"],
                "supports_tools": true,
                "supports_streaming": true
            }),
            json!({
                "id": "model-b",
                "display_name": "Model B",
                "supported_endpoint_types": ["openai_chat"],
                "supports_tools": true,
                "supports_streaming": true
            }),
            json!({
                "id": "claude-sonnet-5",
                "display_name": "Claude Sonnet 5",
                "supported_endpoint_types": ["anthropic"],
                "supports_tools": true,
                "supports_streaming": true
            }),
        ];

        let (added, removed, skipped, warnings) =
            sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (1, 0, 0));
        assert!(warnings.is_empty(), "{warnings:?}");

        let id =
            adapter::service_provider_id("workbuddy", 42, "openai", "https://platform.test", 1);
        let provider = state
            .db
            .get_provider_by_id(&id, "workbuddy")
            .unwrap()
            .expect("WorkBuddy provider");
        assert_eq!(provider.name, "智枢平台 · OpenAI 主力");
        assert_eq!(
            provider.settings_config["models"].as_array().unwrap().len(),
            3
        );
        assert_eq!(provider.settings_config["models"][0]["id"], "model-a");
        assert_eq!(
            provider.settings_config["models"][2]["id"],
            "claude-sonnet-5"
        );
        assert_eq!(provider.settings_config["url"], "https://platform.test/v1");
        assert_eq!(provider.settings_config["apiKey"], "key-42");
        assert_eq!(
            provider.settings_config["models"][0]["supportsToolCall"],
            true
        );
        assert_eq!(
            provider.settings_config["models"][0]["supportsImages"],
            true
        );
        assert_eq!(
            provider.settings_config["models"][0]["maxInputTokens"],
            128000
        );
        assert_eq!(
            provider.settings_config["models"][0]["maxOutputTokens"],
            16384
        );
        assert_eq!(
            provider
                .meta
                .as_ref()
                .and_then(|meta| meta.platform_managed.as_ref())
                .and_then(|managed| managed.group_id),
            Some(42)
        );

        discovery.catalogs[0].models.clear();
        discovery.catalogs[0].catalog_error = Some("temporary outage".into());
        let (added, removed, skipped, warnings) =
            sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (0, 0, 0));
        assert_eq!(warnings.len(), 1);
        assert_eq!(state.db.get_all_providers("workbuddy").unwrap().len(), 1);

        ProviderService::switch(&state, app.clone(), &id).unwrap();
        let live_provider = state
            .db
            .get_provider_by_id(&id, "workbuddy")
            .unwrap()
            .unwrap();
        assert!(crate::workbuddy_config::provider_is_live(&live_provider.settings_config).unwrap());
        assert_eq!(crate::workbuddy_config::read_models().unwrap().len(), 3);

        discovery.catalogs[0].catalog_error = None;
        discovery.catalogs[0].models = vec![json!({
            "id": "model-a",
            "display_name": "Model A",
            "supported_endpoint_types": ["chat"],
            "context_window": 200000,
            "max_output_tokens": 32768,
            "supports_tools": true,
            "supports_streaming": true
        })];
        let (added, removed, skipped, warnings) =
            sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (0, 0, 1));
        assert!(warnings.is_empty(), "{warnings:?}");
        let refreshed = state
            .db
            .get_provider_by_id(&id, "workbuddy")
            .unwrap()
            .unwrap();
        assert_eq!(
            refreshed.settings_config["models"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            refreshed.settings_config["models"][0]["maxInputTokens"],
            200000
        );
        assert_eq!(
            refreshed.settings_config["models"][0]["maxOutputTokens"],
            32768
        );
        assert_eq!(crate::workbuddy_config::read_models().unwrap().len(), 1);
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_adds_workbuddy_provider_for_anthropic_platform() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let app = crate::app_config::AppType::WorkBuddy;
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 77,
            name: "Claude 主力".into(),
            platform: "anthropic".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let discovery = test_discovery(vec![group]);

        let (added, removed, skipped, warnings) =
            sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (1, 0, 0));
        assert!(warnings.is_empty(), "{warnings:?}");

        let id =
            adapter::service_provider_id("workbuddy", 77, "anthropic", "https://platform.test", 1);
        let provider = state
            .db
            .get_provider_by_id(&id, "workbuddy")
            .unwrap()
            .expect("WorkBuddy provider for Anthropic platform");
        assert_eq!(provider.name, "智枢平台 · Claude 主力");
        assert_eq!(provider.settings_config["url"], "https://platform.test/v1");
        assert_eq!(
            provider.settings_config["models"][0]["id"],
            "claude-sonnet-5"
        );
        assert_eq!(
            provider.settings_config["models"][0]["useCustomProtocol"],
            false
        );
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_refreshes_names_with_unchanged_and_rotated_keys() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let app = crate::app_config::AppType::Claude;
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 42,
            name: "Original group".into(),
            platform: "anthropic".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let mut discovery = test_discovery(vec![group]);
        sync_agent_providers(&state, &app, &discovery).unwrap();
        let id =
            adapter::service_provider_id("claude", 42, "anthropic", "https://platform.test", 1);
        let mut provider = state.db.get_provider_by_id(&id, "claude").unwrap().unwrap();
        provider.settings_config["customSetting"] = json!("keep");
        provider.sort_index = Some(7);
        state.db.save_provider("claude", &provider).unwrap();

        for (name, key) in [
            ("Renamed group", "key-42"),
            ("Renamed again", "rotated-key"),
        ] {
            discovery.groups[0].name = name.into();
            discovery.catalogs[0].group.name = name.into();
            discovery.catalogs[0].credential.key = key.into();
            let (added, removed, skipped, warnings) =
                sync_agent_providers(&state, &app, &discovery).unwrap();
            assert_eq!((added, removed, skipped), (0, 0, 1));
            assert!(warnings.is_empty());
            let refreshed = state.db.get_provider_by_id(&id, "claude").unwrap().unwrap();
            assert_eq!(refreshed.name, format!("智枢平台 · {name}"));
            assert_eq!(refreshed.settings_config["customSetting"], "keep");
            assert_eq!(refreshed.sort_index, provider.sort_index);
            assert_eq!(refreshed.created_at, provider.created_at);
            assert_eq!(refreshed.resolve_usage_credentials(&app).1, key);
            let managed = refreshed.meta.unwrap().platform_managed.unwrap();
            assert_eq!(managed.group_name.as_deref(), Some(name));
        }
    }

    #[test]
    #[serial_test::serial]
    fn dsh_sync_writes_native_profiles_and_deletes_only_managed_provider() {
        use crate::{app_config::AppType, database::Database};
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let app = AppType::Dsh;
        let manual = Provider::with_id(
            "shoudongtianjia".into(),
            "shoudongtianjia".into(),
            json!({"api":"openai-responses","options":{"baseURL":"https://manual.test/v1","apiKey":"manual-key"},
                "models":{"model-one":{"name":"model-one"}}}),
            None,
        );
        crate::services::ProviderService::add(&state, app.clone(), manual, true).unwrap();
        let manual_before = crate::dsh_config::get_providers().unwrap()["shoudongtianjia"].clone();
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 42,
            name: "DSH models".into(),
            platform: "composite".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let mut discovery = test_discovery(vec![group]);
        discovery.catalogs[0].models = vec![
            json!({"id":"model-one","supported_endpoint_types":["responses"],"context_window":64000,
                "supported_reasoning_levels":["high","ultra"]}),
            json!({"id":"model-two","supported_endpoint_types":["responses"],"max_output_tokens":4096}),
        ];
        let id = adapter::service_provider_id("dsh", 42, "composite", "https://platform.test", 1);
        let result = sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((result.0, result.1, result.2), (1, 0, 0));
        assert!(result.3.is_empty(), "{:?}", result.3);
        let provider = state.db.get_provider_by_id(&id, "dsh").unwrap().unwrap();
        assert_eq!(
            provider.settings_config["models"]
                .as_object()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            crate::dsh_config::get_providers().unwrap()["shoudongtianjia"],
            manual_before
        );
        let profile = crate::dsh_config::profile_path("web");
        let credentials = crate::dsh_config::credentials_path();
        let native: serde_yaml::Value =
            serde_yaml::from_slice(&std::fs::read(&profile).unwrap()).unwrap();
        let models = native[0]["config"]["providers"][&id]["models"]
            .as_sequence()
            .unwrap();
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|model| model["id"] == "model-one"));
        let first = models
            .iter()
            .find(|model| model["id"] == "model-one")
            .unwrap();
        assert_eq!(first["reasoningEfforts"]["high"], "high");
        assert!(first["reasoningEfforts"].get("ultra").is_none());
        assert!(!std::fs::read_to_string(&profile)
            .unwrap()
            .contains("key-42"));
        assert!(std::fs::read_to_string(&credentials)
            .unwrap()
            .contains("key-42"));
        let files_before = (
            std::fs::read(&profile).unwrap(),
            std::fs::read(&credentials).unwrap(),
        );
        let result = sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((result.0, result.1, result.2), (0, 0, 1));
        assert!(result.3.is_empty(), "{:?}", result.3);
        assert_eq!(std::fs::read(&profile).unwrap(), files_before.0);
        assert_eq!(std::fs::read(&credentials).unwrap(), files_before.1);

        crate::services::ProviderService::remove_from_live_config(&state, app.clone(), &id)
            .unwrap();
        assert!(!crate::dsh_config::get_providers()
            .unwrap()
            .contains_key(&id));
        let result = sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((result.0, result.1, result.2), (0, 0, 1));
        assert!(result.3.is_empty(), "{:?}", result.3);
        assert!(crate::dsh_config::get_providers()
            .unwrap()
            .contains_key(&id));

        discovery.catalogs[0].credential.key = "rotated-key".into();
        discovery.groups[0].name = "Renamed DSH group".into();
        discovery.catalogs[0].group.name = "Renamed DSH group".into();
        discovery.catalogs[0].models.remove(1);
        let result = sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((result.0, result.1, result.2), (0, 0, 1));
        assert!(result.3.is_empty(), "{:?}", result.3);
        let refreshed = crate::dsh_config::get_providers().unwrap();
        assert_eq!(refreshed[&id]["name"], "智枢平台 · Renamed DSH group");
        assert_eq!(refreshed[&id]["options"]["apiKey"], "rotated-key");
        assert_eq!(refreshed[&id]["models"].as_object().unwrap().len(), 1);

        let result =
            delete::delete_account_providers(&state, &app, "https://platform.test", 1).unwrap();
        assert_eq!((result.removed, result.skipped), (1, 0));
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        assert!(!crate::dsh_config::get_providers()
            .unwrap()
            .contains_key(&id));
        assert!(state.db.get_provider_by_id(&id, "dsh").unwrap().is_none());
        assert_eq!(
            crate::dsh_config::get_providers().unwrap()["shoudongtianjia"],
            manual_before
        );
    }

    #[test]
    #[serial_test::serial]
    fn opencode_sync_upgrades_live_legacy_provider_and_preserves_custom_settings() {
        use crate::{app_config::AppType, database::Database, provider::OpenCodeConfigFormat};
        use std::sync::Arc;
        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let app = AppType::OpenCode;
        let discovery = test_discovery(vec![
            crate::zhishu::services::platform_auth::AvailableGroup {
                id: 42,
                name: "Mixed".into(),
                platform: "composite".into(),
                status: "active".into(),
                subscription_type: "standard".into(),
            },
        ]);
        sync_agent_providers(&state, &app, &discovery).unwrap();
        let id =
            adapter::service_provider_id("opencode", 42, "composite", "https://platform.test", 1);
        let mut legacy = state
            .db
            .get_provider_by_id(&id, "opencode")
            .unwrap()
            .unwrap();
        legacy.settings_config = json!({
            "npm":"@ai-sdk/openai", "name":"Custom display", "custom":"keep",
            "options":{"baseURL":"https://custom.test/v1","apiKey":"key-42","timeout":30000},
            "models":{"local-alias":{"name":"Keep alias", "modalities":{"input":["text","image"]},
                "variants":{"high":{"reasoningEffort":"high"}}, "limit":{"context":123456}}}
        });
        legacy.meta.as_mut().unwrap().opencode_config_format = None;
        ProviderService::update(&state, app.clone(), None, legacy.clone()).unwrap();
        ProviderService::switch(&state, app.clone(), &id).unwrap();
        adapter::select_default(&app, &id, "local-alias").unwrap();
        let (added, removed, skipped, warnings) =
            sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (0, 0, 1));
        assert!(warnings.is_empty(), "{warnings:?}");
        let upgraded = state
            .db
            .get_provider_by_id(&id, "opencode")
            .unwrap()
            .unwrap();
        assert_eq!(
            upgraded.opencode_config_format(),
            Some(OpenCodeConfigFormat::V2)
        );
        assert_eq!(
            upgraded.resolve_usage_credentials(&app),
            ("https://custom.test/v1".into(), "key-42".into())
        );
        assert_eq!(upgraded.settings_config["custom"], "keep");
        assert_eq!(upgraded.settings_config["settings"]["timeout"], 30000);
        crate::opencode_config::validate_native_provider(&id, &upgraded.settings_config).unwrap();
        let live = crate::opencode_config::read_opencode_config().unwrap();
        assert_eq!(live["providers"][&id], upgraded.settings_config);
        assert_eq!(live["model"], format!("{id}/local-alias"));
        assert_eq!(live["small_model"], format!("{id}/local-alias"));
        let (_, _, _, warnings) = sync_agent_providers(&state, &app, &discovery).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            state
                .db
                .get_provider_by_id(&id, "opencode")
                .unwrap()
                .unwrap()
                .settings_config,
            upgraded.settings_config
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial_test::serial]
    async fn cached_provider_sync_removes_unlisted_models_and_keeps_catalog_on_fetch_failure() {
        use crate::{app_config::AppType, database::Database};
        use std::sync::Arc;
        let home = SyncTestHome::new();
        for app in [
            AppType::Codex,
            AppType::OpenCode,
            AppType::Pi,
            AppType::OpenClaw,
            AppType::Hermes,
            AppType::Mcode,
            AppType::Kimi,
            AppType::WorkBuddy,
        ] {
            let state = AppState::new(Arc::new(Database::memory().unwrap()));
            let store = base_data::CatalogStore::for_test(
                home._temp
                    .path()
                    .join(format!("catalog-{}.json", app.as_str())),
            );
            let mut discovery = test_discovery(vec![
                crate::zhishu::services::platform_auth::AvailableGroup {
                    id: 42,
                    name: "智枢router-ccs-gpt".into(),
                    platform: "composite".into(),
                    status: "active".into(),
                    subscription_type: "standard".into(),
                },
            ]);
            let model = |id: &str| {
                json!({"id":id,"context_window":32000,"max_output_tokens":4096,
                "supports_tools":true,"supports_streaming":true})
            };
            discovery.catalogs[0].models = vec![model("kept"), model("removed")];
            store.save(discovery.clone()).unwrap();
            let run = || async {
                // Each run represents a separate IPC request, with a fresh task budget.
                tokio::task::yield_now().await;
                sync_account_providers(
                    &state,
                    std::slice::from_ref(&app),
                    &store,
                    "https://platform.test",
                    1,
                )
                .unwrap()
            };
            let first = run().await;
            assert_eq!(first.added, 1, "{}: {:?}", app.as_str(), first.warnings);
            assert!(first.warnings.is_empty(), "{:?}", first.warnings);
            let id = state
                .db
                .get_all_providers(app.as_str())
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .clone();
            discovery.catalogs[0].models = vec![model("kept"), model("new")];
            store.save(discovery.clone()).unwrap();
            let next = run().await;
            assert!(
                next.warnings.is_empty(),
                "{}: {:?}",
                app.as_str(),
                next.warnings
            );
            assert_eq!(next.updated, Some(1), "{}", app.as_str());
            let mut updated = state
                .db
                .get_provider_by_id(&id, app.as_str())
                .unwrap()
                .unwrap();
            let models = if app == AppType::Codex {
                &updated.settings_config["modelCatalog"]["models"]
            } else {
                &updated.settings_config["models"]
            };
            let ids: Vec<_> = if let Some(models) = models.as_array() {
                models
                    .iter()
                    .map(|m| {
                        m.get("model")
                            .or_else(|| m.get("id"))
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string()
                    })
                    .collect()
            } else {
                models.as_object().unwrap().keys().cloned().collect()
            };
            assert_eq!(ids.len(), 2, "{}: {ids:?}", app.as_str());
            assert!(ids.iter().all(|id| !id.ends_with("removed")), "{ids:?}");
            assert!(ids.iter().any(|id| id.ends_with("new")), "{ids:?}");
            assert_eq!(run().await.updated, Some(0));

            if app == AppType::Codex {
                state
                    .db
                    .update_proxy_config(crate::proxy::types::ProxyConfig {
                        listen_port: 0,
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                ProviderService::switch(&state, app.clone(), &id).unwrap();
                run().await;
                for (phase, mode) in [None, Some(false), Some(true)].into_iter().enumerate() {
                    if let Some(stack) = mode {
                        crate::mode::controller::enter(&state, &app, stack)
                            .await
                            .unwrap();
                        run().await;
                    }
                    let replacement = format!("new-{phase}");
                    discovery.catalogs[0].models = vec![model("kept"), model(&replacement)];
                    store.save(discovery.clone()).unwrap();
                    let result = run().await;
                    assert!(
                        result.warnings.is_empty(),
                        "mode={mode:?}: {:?}",
                        result.warnings
                    );
                    assert_eq!(result.updated, Some(1));
                    assert!(result.native_files_changed.unwrap() > 0);
                    let native: Value = crate::config::read_json_file(
                        &crate::codex_config::get_codex_model_catalog_path(),
                    )
                    .unwrap();
                    let native_models: Vec<_> = native["models"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|m| m["slug"].as_str().unwrap())
                        .collect();
                    assert_eq!(
                        native_models,
                        vec!["kept", replacement.as_str()],
                        "mode={mode:?}"
                    );
                    updated = state
                        .db
                        .get_provider_by_id(&id, app.as_str())
                        .unwrap()
                        .unwrap();
                }
                crate::mode::controller::exit(&state, &app).await.unwrap();
                run().await;
            }

            // A failed fetch is not a successful empty/reduced platform list.
            discovery.catalogs[0].models.clear();
            discovery.catalogs[0].catalog_error = Some("temporary network failure".into());
            discovery.failed_group_ids.insert(42);
            store.save(discovery.clone()).unwrap();
            let _ = run().await;
            assert_eq!(
                state
                    .db
                    .get_provider_by_id(&id, app.as_str())
                    .unwrap()
                    .unwrap()
                    .settings_config,
                updated.settings_config,
                "{}",
                app.as_str()
            );
        }
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_refreshes_existing_model_capabilities_without_rotating_the_key() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        for app in [
            crate::app_config::AppType::Codex,
            crate::app_config::AppType::OpenCode,
        ] {
            let state = AppState::new(Arc::new(Database::memory().unwrap()));
            let group = crate::zhishu::services::platform_auth::AvailableGroup {
                id: 42,
                name: "Mixed".into(),
                platform: "composite".into(),
                status: "active".into(),
                subscription_type: "standard".into(),
            };
            let mut discovery = test_discovery(vec![group]);
            discovery.catalogs[0].models = vec![json!({"id":"public-alias"})];
            let (_, _, _, warnings) = sync_agent_providers(&state, &app, &discovery).unwrap();
            assert!(warnings.is_empty(), "{warnings:?}");
            let id = adapter::service_provider_id(
                app.as_str(),
                42,
                "composite",
                "https://platform.test",
                1,
            );
            let mut provider = state
                .db
                .get_provider_by_id(&id, app.as_str())
                .unwrap()
                .unwrap();
            provider.meta.as_mut().unwrap().api_format = None;
            provider.settings_config["customSetting"] = json!("keep");
            if app == crate::app_config::AppType::Codex {
                provider.settings_config["config"] =
                    json!("model = 'local-selection'\nmodel_reasoning_effort = 'low'\n");
                provider.settings_config["modelCatalog"]["models"][0]["customField"] =
                    json!("keep");
            }
            state.db.save_provider(app.as_str(), &provider).unwrap();

            discovery.catalogs[0].models = vec![json!({
                "id":"public-alias", "context_window":1000000,
                "max_output_tokens":64000,
                "supported_reasoning_levels":[{"effort":"low"},{"effort":"high"},{"effort":"max"}],
                "default_reasoning_level":"high", "input_modalities":["text","image"]
            })];
            let (added, removed, skipped, warnings) =
                sync_agent_providers(&state, &app, &discovery).unwrap();
            assert_eq!((added, removed, skipped), (0, 0, 1));
            assert!(warnings.is_empty(), "{warnings:?}");
            let refreshed = state
                .db
                .get_provider_by_id(&id, app.as_str())
                .unwrap()
                .unwrap();
            assert_eq!(refreshed.resolve_usage_credentials(&app).1, "key-42");
            assert_eq!(refreshed.settings_config["customSetting"], "keep");
            assert_eq!(
                refreshed.meta.as_ref().unwrap().api_format.as_deref(),
                Some("openai_responses")
            );
            if app == crate::app_config::AppType::Codex {
                assert_eq!(
                    refreshed.settings_config["config"],
                    provider.settings_config["config"]
                );
                let row = &refreshed.settings_config["modelCatalog"]["models"][0];
                assert_eq!(row["contextWindow"], 1000000);
                assert_eq!(row["reasoningLevels"], json!(["low", "high", "max"]));
                assert_eq!(row["defaultReasoningLevel"], "high");
                assert_eq!(row["inputModalities"], json!(["text", "image"]));
                assert_eq!(row["customField"], "keep");
            } else {
                let row = &refreshed.settings_config["models"]["public-alias"];
                assert_eq!(row["limit"], json!({"context":1000000,"output":64000}));
                assert!(row["variants"].as_array().unwrap().iter().any(|variant| {
                    variant["id"] == "max" && variant["settings"]["reasoningEffort"] == "max"
                }));
                assert_eq!(row["reasoning"], true);
                assert_eq!(row["capabilities"]["input"], json!(["text", "image"]));
            }
            assert!(!connection::refresh_catalog(
                &mut refreshed.clone(),
                &refreshed,
                &app
            ));
            // An older platform may return only IDs on a later synchronization.
            discovery.catalogs[0].models = vec![json!({"id":"public-alias"})];
            let (_, _, _, warnings) = sync_agent_providers(&state, &app, &discovery).unwrap();
            assert!(warnings.is_empty(), "{warnings:?}");
            let retained = state
                .db
                .get_provider_by_id(&id, app.as_str())
                .unwrap()
                .unwrap();
            assert_eq!(retained.settings_config, refreshed.settings_config);
        }
    }

    #[test]
    #[serial_test::serial]
    fn provider_sync_never_deletes_another_accounts_managed_provider() {
        use crate::database::Database;
        use std::sync::Arc;

        let _home = SyncTestHome::new();
        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let model = catalog::agent_models(
            "anthropic",
            &[json!({"id":"claude-sonnet-5","supported_endpoint_types":["anthropic"]})],
            &["claude"],
        )
        .into_iter()
        .find(|model| model.agent == "claude")
        .unwrap();
        let provider = adapter::service_provider(
            &model,
            "https://platform.test",
            2,
            "other-account-key",
            99,
            None,
            77,
            "anthropic",
            "Other account",
            "zhishu-client-77",
        )
        .unwrap();
        let id = provider.id.clone();
        state.db.save_provider("claude", &provider).unwrap();

        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 77,
            name: "Shared group id".into(),
            platform: "anthropic".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let (added, removed, skipped, warnings) = sync_agent_providers(
            &state,
            &crate::app_config::AppType::Claude,
            &test_discovery(vec![group]),
        )
        .unwrap();
        assert_eq!((added, removed, skipped), (1, 0, 0));
        assert!(warnings.is_empty());
        assert!(state
            .db
            .get_provider_by_id(&id, "claude")
            .unwrap()
            .is_some());
        let current_id =
            adapter::service_provider_id("claude", 77, "anthropic", "https://platform.test", 1);
        assert_ne!(id, current_id);
        assert!(state
            .db
            .get_provider_by_id(&current_id, "claude")
            .unwrap()
            .is_some());
    }

    #[test]
    #[serial_test::serial]
    fn failed_catalog_never_borrows_an_unauthorized_existing_model() {
        use crate::database::Database;
        use std::sync::Arc;

        let home = SyncTestHome::new();
        let codex_dir = home._temp.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();
        std::fs::write(codex_dir.join("config.toml"), "model = \"gpt-5.6-sol\"\n").unwrap();

        let state = AppState::new(Arc::new(Database::memory().unwrap()));
        let app = crate::app_config::AppType::Codex;
        let group = crate::zhishu::services::platform_auth::AvailableGroup {
            id: 42,
            name: "混合分组".into(),
            platform: "composite".into(),
            status: "active".into(),
            subscription_type: "standard".into(),
        };
        let mut discovery = test_discovery(vec![group]);
        discovery.catalogs[0].models.clear();
        discovery.catalogs[0].catalog_error = Some("HTTP 403".into());
        discovery.failed_group_ids.insert(42);

        let (added, removed, skipped, warnings) =
            sync_agent_providers(&state, &app, &discovery).unwrap();
        assert_eq!((added, removed, skipped), (0, 0, 0));
        assert!(!warnings.is_empty());
        assert!(state.db.get_all_providers("codex").unwrap().is_empty());
        assert_eq!(
            std::fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
            "model = \"gpt-5.6-sol\"\n"
        );
    }
    #[test]
    #[serial_test::serial]
    fn available_empty_model_catalog_removes_subscription_row_but_quota_failure_retains_it() {
        let _home = SyncTestHome::new();
        let state = AppState::new(std::sync::Arc::new(
            crate::database::Database::memory().unwrap(),
        ));
        let app = crate::app_config::AppType::Claude;
        let mut discovery = test_discovery(vec![
            crate::zhishu::services::platform_auth::AvailableGroup {
                id: 42,
                name: "subscription".into(),
                platform: "anthropic".into(),
                status: "active".into(),
                subscription_type: "subscription".into(),
            },
        ]);
        assert_eq!(sync_agent_providers(&state, &app, &discovery).unwrap().0, 1);
        discovery.catalogs[0].models.clear();
        discovery.details.get_mut(&42).unwrap().quota.available = false;
        assert_eq!(sync_agent_providers(&state, &app, &discovery).unwrap().1, 0);
        discovery.details.get_mut(&42).unwrap().quota.available = true;
        assert_eq!(sync_agent_providers(&state, &app, &discovery).unwrap().1, 1);
    }
}
