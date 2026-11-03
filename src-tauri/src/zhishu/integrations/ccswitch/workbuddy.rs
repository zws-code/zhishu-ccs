//! WorkBuddy 供应商业务：按原生模型分组同步数据库，并区分保存供应商与加入实际配置。
//! 增删改保留应用切换锁和局部回滚路径，liveConfigManaged 标记用于判断已加入配置的条目。
use crate::app_config::AppType;
use crate::error::AppError;
use crate::provider::{Provider, ProviderMeta};
use crate::services::provider::{ProviderService, SwitchResult};
use crate::store::AppState;
use indexmap::IndexMap;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::collections::HashSet;

const WORKBUDDY_APP: &str = "workbuddy";

pub(crate) fn list(state: &AppState) -> Result<IndexMap<String, Provider>, AppError> {
    let _guard =
        futures::executor::block_on(state.proxy_service.lock_switch_for_app(WORKBUDDY_APP));
    migrate_legacy_providers(state)?;
    match crate::workbuddy_config::read_provider_groups() {
        Ok(native) => {
            if let Err(error) = sync_native_locked(state, &native) {
                log::warn!("Failed to sync WorkBuddy providers from models.json: {error}");
            }
        }
        Err(error) => {
            log::warn!("Failed to read WorkBuddy models.json; showing saved catalog: {error}");
        }
    }
    state.db.get_all_providers(WORKBUDDY_APP)
}

pub(crate) fn import_from_live(state: &AppState) -> Result<usize, AppError> {
    let _guard =
        futures::executor::block_on(state.proxy_service.lock_switch_for_app(WORKBUDDY_APP));
    migrate_legacy_providers(state)?;
    let native = crate::workbuddy_config::read_provider_groups()?;
    sync_native_locked(state, &native)
}

pub(crate) fn add(
    state: &AppState,
    mut provider: Provider,
    add_to_live: bool,
) -> Result<bool, AppError> {
    let app_type = AppType::WorkBuddy;
    let _guard =
        futures::executor::block_on(state.proxy_service.lock_switch_for_app(app_type.as_str()));
    normalize_provider(&mut provider)?;
    ProviderService::validate_provider_settings(&app_type, &provider)?;
    strip_unsupported_metadata(&mut provider);
    ProviderService::normalize_usage_script_credential_overrides(&app_type, &mut provider);

    if state
        .db
        .get_provider_by_id(&provider.id, app_type.as_str())?
        .is_some()
    {
        return Err(AppError::InvalidInput(format!(
            "WorkBuddy provider '{}' already exists",
            provider.id
        )));
    }

    set_live_managed(&mut provider, add_to_live);
    let inserted = if add_to_live {
        crate::workbuddy_config::upsert_provider(&provider.settings_config, None)?;
        true
    } else {
        false
    };

    if let Err(error) = state.db.save_provider(app_type.as_str(), &provider) {
        if inserted {
            if let Err(rollback) =
                crate::workbuddy_config::remove_provider(&provider.settings_config)
            {
                return Err(AppError::Config(format!(
                    "failed to save WorkBuddy provider: {error}; native rollback failed: {rollback}"
                )));
            }
        }
        return Err(error);
    }
    Ok(true)
}

pub(crate) fn update(
    state: &AppState,
    original_id: Option<&str>,
    mut provider: Provider,
) -> Result<bool, AppError> {
    let app_type = AppType::WorkBuddy;
    let _guard =
        futures::executor::block_on(state.proxy_service.lock_switch_for_app(app_type.as_str()));
    let original_id = original_id.unwrap_or(&provider.id).to_string();
    if original_id != provider.id {
        return Err(AppError::InvalidInput(
            "WorkBuddy provider IDs cannot be renamed after creation".to_string(),
        ));
    }

    let existing = state
        .db
        .get_provider_by_id(&original_id, app_type.as_str())?
        .ok_or_else(|| {
            AppError::InvalidInput(format!("WorkBuddy provider '{original_id}' not found"))
        })?;
    normalize_provider(&mut provider)?;
    ProviderService::validate_provider_settings(&app_type, &provider)?;
    strip_unsupported_metadata(&mut provider);
    ProviderService::normalize_usage_script_credential_overrides(&app_type, &mut provider);

    let was_live = crate::workbuddy_config::provider_is_live(&existing.settings_config)?;
    set_live_managed(&mut provider, was_live);
    if was_live {
        crate::workbuddy_config::upsert_provider(
            &provider.settings_config,
            Some(&existing.settings_config),
        )?;
    }

    if let Err(error) = state.db.save_provider(app_type.as_str(), &provider) {
        if was_live {
            if let Err(rollback) = crate::workbuddy_config::upsert_provider(
                &existing.settings_config,
                Some(&provider.settings_config),
            ) {
                return Err(AppError::Config(format!(
                    "failed to save WorkBuddy provider: {error}; native rollback failed: {rollback}"
                )));
            }
        }
        return Err(error);
    }
    Ok(true)
}

pub(crate) fn delete(state: &AppState, id: &str) -> Result<(), AppError> {
    let app_type = AppType::WorkBuddy;
    let _guard =
        futures::executor::block_on(state.proxy_service.lock_switch_for_app(app_type.as_str()));
    let Some(provider) = state.db.get_provider_by_id(id, app_type.as_str())? else {
        return Ok(());
    };
    let removed = crate::workbuddy_config::remove_provider(&provider.settings_config)?;

    if let Err(error) = state.db.delete_provider(app_type.as_str(), id) {
        if removed {
            if let Err(rollback) =
                crate::workbuddy_config::upsert_provider(&provider.settings_config, None)
            {
                return Err(AppError::Config(format!(
                    "failed to delete WorkBuddy provider: {error}; native rollback failed: {rollback}"
                )));
            }
        }
        return Err(error);
    }
    Ok(())
}

pub(crate) fn remove(state: &AppState, id: &str) -> Result<(), AppError> {
    let app_type = AppType::WorkBuddy;
    let _guard =
        futures::executor::block_on(state.proxy_service.lock_switch_for_app(app_type.as_str()));
    let mut provider = state
        .db
        .get_provider_by_id(id, app_type.as_str())?
        .ok_or_else(|| AppError::InvalidInput(format!("WorkBuddy provider '{id}' not found")))?;
    let removed = crate::workbuddy_config::remove_provider(&provider.settings_config)?;
    set_live_managed(&mut provider, false);
    if let Err(error) = state.db.save_provider(app_type.as_str(), &provider) {
        if removed {
            if let Err(rollback) =
                crate::workbuddy_config::upsert_provider(&provider.settings_config, None)
            {
                return Err(AppError::Config(format!(
                    "failed to preserve WorkBuddy provider before removal: {error}; native rollback failed: {rollback}"
                )));
            }
        }
        return Err(error);
    }
    Ok(())
}

pub(crate) fn enable(state: &AppState, id: &str) -> Result<SwitchResult, AppError> {
    let app_type = AppType::WorkBuddy;
    let _guard =
        futures::executor::block_on(state.proxy_service.lock_switch_for_app(app_type.as_str()));
    let mut provider = state
        .db
        .get_provider_by_id(id, app_type.as_str())?
        .ok_or_else(|| AppError::InvalidInput(format!("WorkBuddy provider '{id}' not found")))?;

    normalize_provider(&mut provider)?;
    ProviderService::validate_provider_settings(&app_type, &provider)?;
    let previous = provider.settings_config.clone();
    crate::workbuddy_config::upsert_provider(&previous, None)?;
    set_live_managed(&mut provider, true);
    if let Err(error) = state.db.save_provider(app_type.as_str(), &provider) {
        if let Err(rollback) = crate::workbuddy_config::remove_provider(&previous) {
            return Err(AppError::Config(format!(
                "failed to enable WorkBuddy provider: {error}; native rollback failed: {rollback}"
            )));
        }
        return Err(error);
    }
    Ok(SwitchResult::default())
}

fn sync_native_locked(
    state: &AppState,
    native: &[crate::workbuddy_config::NativeProviderGroup],
) -> Result<usize, AppError> {
    let saved = state.db.get_all_providers(WORKBUDDY_APP)?;
    let mut claimed = HashSet::new();
    let mut changed = 0;

    for group in native {
        let provider_id = saved
            .values()
            .filter(|provider| !claimed.contains(&provider.id))
            .filter(|provider| same_credentials(provider, &group.config))
            .min_by_key(|provider| {
                let managed = provider
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.live_config_managed)
                    == Some(true);
                let platform = provider
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.platform_managed.as_ref())
                    .is_some();
                (!managed, !platform, provider.id.clone())
            })
            .map(|provider| provider.id.clone())
            .unwrap_or_else(|| group.id.clone());

        let previous = saved.get(&provider_id);
        let mut provider = previous.cloned().unwrap_or_else(|| {
            let name = crate::workbuddy_config::provider_name(&group.config, &provider_id);
            Provider::with_id(
                provider_id.clone(),
                name,
                group.config.clone(),
                group
                    .config
                    .get("url")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        });
        let was_new = previous.is_none();
        let previous_config = provider.settings_config.clone();
        let previous_name = provider.name.clone();
        let previous_managed = provider
            .meta
            .as_ref()
            .and_then(|meta| meta.live_config_managed);
        provider.settings_config = group.config.clone();
        provider
            .category
            .get_or_insert_with(|| "custom".to_string());
        set_live_managed(&mut provider, true);

        if !was_new
            && provider.name == previous_name
            && provider.settings_config == previous_config
            && previous_managed == Some(true)
        {
            claimed.insert(provider_id);
            continue;
        }
        state.db.save_provider(WORKBUDDY_APP, &provider)?;
        claimed.insert(provider_id);
        changed += 1;
    }

    for (id, mut provider) in saved {
        if claimed.contains(&id)
            || provider
                .meta
                .as_ref()
                .and_then(|meta| meta.live_config_managed)
                != Some(true)
        {
            continue;
        }
        set_live_managed(&mut provider, false);
        state.db.save_provider(WORKBUDDY_APP, &provider)?;
        changed += 1;
    }

    Ok(changed)
}

fn migrate_legacy_providers(state: &AppState) -> Result<(), AppError> {
    let saved = state.db.get_all_providers(WORKBUDDY_APP)?;
    let mut groups = HashMap::<(String, String), Vec<Provider>>::new();
    for provider in saved.into_values() {
        let Some(credentials) = crate::workbuddy_config::credentials(&provider.settings_config)
        else {
            continue;
        };
        groups.entry(credentials).or_default().push(provider);
    }

    for group in groups.into_values() {
        if !group.iter().any(|provider| {
            !provider
                .settings_config
                .get("models")
                .is_some_and(Value::is_array)
        }) {
            continue;
        }
        let platform = group.iter().find_map(|provider| {
            provider
                .meta
                .as_ref()
                .and_then(|meta| meta.platform_managed.as_ref())
        });
        let (url, key) = crate::workbuddy_config::credentials(&group[0].settings_config)
            .ok_or_else(|| AppError::Config("WorkBuddy provider URL is missing".into()))?;
        let target_id = platform
            .map(platform_provider_id)
            .unwrap_or_else(|| crate::workbuddy_config::stable_provider_id(&url, &key));
        let mut merged = platform
            .and_then(|managed| {
                group.iter().find(|provider| {
                    provider
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.platform_managed.as_ref())
                        .is_some_and(|candidate| std::ptr::eq(candidate, managed))
                })
            })
            .cloned()
            .unwrap_or_else(|| group[0].clone());
        let mut base_config = merged.settings_config.clone();
        let mut models = Vec::new();
        let mut seen = HashSet::new();
        for provider in &group {
            let Ok(config) = crate::workbuddy_config::normalize_provider_config(
                provider.settings_config.clone(),
            ) else {
                continue;
            };
            for model in config
                .get("models")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let id = model
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if seen.insert(id) {
                    models.push(model.clone());
                }
            }
        }
        if models.is_empty() {
            continue;
        }
        base_config["models"] = Value::Array(models);
        merged.id = target_id.clone();
        merged.settings_config = base_config;
        if let Some(group_name) = platform.and_then(|managed| managed.group_name.as_deref()) {
            merged.name = format!("智枢平台 · {group_name}");
        }
        let live = group.iter().any(|provider| {
            provider
                .meta
                .as_ref()
                .and_then(|meta| meta.live_config_managed)
                == Some(true)
        }) || crate::workbuddy_config::provider_is_live(&merged.settings_config)
            .unwrap_or(false);
        set_live_managed(&mut merged, live);
        state.db.save_provider(WORKBUDDY_APP, &merged)?;
        for provider in group {
            if provider.id != target_id {
                state.db.delete_provider(WORKBUDDY_APP, &provider.id)?;
            }
        }
    }
    Ok(())
}

fn platform_provider_id(managed: &crate::provider::PlatformManagedProvider) -> String {
    let digest = Sha256::digest(format!("{}\0{}", managed.origin, managed.user_id).as_bytes());
    let scope = format!("{digest:x}")[..12].to_string();
    format!(
        "platform-managed-workbuddy-{scope}-{}-{}",
        managed.group_platform.as_deref().unwrap_or("unknown"),
        managed.group_id.unwrap_or_default()
    )
}

fn same_credentials(provider: &Provider, config: &Value) -> bool {
    let Some((url, key)) = crate::workbuddy_config::credentials(config) else {
        return false;
    };
    let Some((provider_url, provider_key)) =
        crate::workbuddy_config::credentials(&provider.settings_config)
    else {
        return false;
    };
    provider_url == url && provider_key == key
}

fn normalize_provider(provider: &mut Provider) -> Result<(), AppError> {
    provider.settings_config =
        crate::workbuddy_config::normalize_provider_config(provider.settings_config.clone())?;
    Ok(())
}

fn set_live_managed(provider: &mut Provider, managed: bool) {
    provider
        .meta
        .get_or_insert_with(ProviderMeta::default)
        .live_config_managed = Some(managed);
}

fn strip_unsupported_metadata(provider: &mut Provider) {
    provider.in_failover_queue = false;
    let Some(meta) = provider.meta.take() else {
        return;
    };
    provider.meta = Some(ProviderMeta {
        platform_managed: meta.platform_managed,
        usage_script: meta.usage_script,
        is_partner: meta.is_partner,
        partner_promotion_key: meta.partner_promotion_key,
        live_config_managed: meta.live_config_managed,
        ..ProviderMeta::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use serde_json::json;
    use serial_test::serial;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn state() -> AppState {
        AppState::new(Arc::new(
            Database::memory().expect("create in-memory database"),
        ))
    }

    fn provider() -> Provider {
        let mut provider = Provider::with_id(
            "workbuddy-provider".to_string(),
            "DeepSeek".to_string(),
            json!({
                "url": "https://api.deepseek.com/v1",
                "apiKey": "sk-test",
                "vendor": "DeepSeek",
                "models": [
                    {
                        "id": "deepseek-chat",
                        "name": "DeepSeek Chat",
                        "supportsToolCall": true,
                        "supportsImages": false,
                        "supportsReasoning": false
                    },
                    {
                        "id": "deepseek-reasoner",
                        "name": "DeepSeek Reasoner",
                        "supportsToolCall": true,
                        "supportsImages": false,
                        "supportsReasoning": true
                    }
                ]
            }),
            None,
        );
        provider.category = Some("custom".to_string());
        provider
    }

    #[test]
    #[serial]
    fn provider_models_round_trip_through_models_json() {
        let temp = TempDir::new().expect("temp dir");
        let config_dir = temp.path().join(".workbuddy");
        let previous = std::env::var_os("WORKBUDDY_CONFIG_DIR");
        std::env::set_var("WORKBUDDY_CONFIG_DIR", &config_dir);
        let state = state();

        ProviderService::add(&state, AppType::WorkBuddy, provider(), false)
            .expect("save disabled provider");
        assert!(!crate::workbuddy_config::provider_is_live(&provider().settings_config).unwrap());
        let saved = state
            .db
            .get_provider_by_id("workbuddy-provider", "workbuddy")
            .unwrap()
            .expect("saved provider");
        assert_eq!(
            saved
                .meta
                .as_ref()
                .and_then(|meta| meta.live_config_managed),
            Some(false)
        );

        ProviderService::switch(&state, AppType::WorkBuddy, "workbuddy-provider")
            .expect("enable provider");
        assert!(crate::workbuddy_config::provider_is_live(&provider().settings_config).unwrap());
        assert_eq!(crate::workbuddy_config::read_models().unwrap().len(), 2);

        let mut edited = state
            .db
            .get_provider_by_id("workbuddy-provider", "workbuddy")
            .unwrap()
            .expect("enabled provider");
        edited.settings_config["models"] = json!([{
            "id": "deepseek-chat",
            "name": "DeepSeek Chat Updated",
            "supportsToolCall": true,
            "supportsImages": false,
            "supportsReasoning": false
        }]);
        ProviderService::update(&state, AppType::WorkBuddy, None, edited)
            .expect("update live provider");
        let native = crate::workbuddy_config::read_models().unwrap();
        assert_eq!(native.len(), 1);
        assert_eq!(native[0]["name"], "DeepSeek Chat Updated");

        ProviderService::remove_from_live_config(&state, AppType::WorkBuddy, "workbuddy-provider")
            .expect("remove from live");
        assert!(!crate::workbuddy_config::provider_is_live(&provider().settings_config).unwrap());
        assert!(state
            .db
            .get_provider_by_id("workbuddy-provider", "workbuddy")
            .unwrap()
            .is_some());

        ProviderService::delete(&state, AppType::WorkBuddy, "workbuddy-provider")
            .expect("delete provider");
        assert!(state
            .db
            .get_provider_by_id("workbuddy-provider", "workbuddy")
            .unwrap()
            .is_none());

        match previous {
            Some(value) => std::env::set_var("WORKBUDDY_CONFIG_DIR", value),
            None => std::env::remove_var("WORKBUDDY_CONFIG_DIR"),
        }
    }

    #[test]
    #[serial]
    fn migrates_legacy_model_cards_into_one_provider() {
        let state = state();
        for (id, name) in [("model-a", "Model A"), ("model-b", "Model B")] {
            let mut provider = Provider::with_id(
                id.into(),
                name.into(),
                json!({
                    "id": id,
                    "name": name,
                    "url": "https://example.com/v1",
                    "apiKey": "sk-test"
                }),
                None,
            );
            provider.category = Some("custom".into());
            state.db.save_provider("workbuddy", &provider).unwrap();
        }

        migrate_legacy_providers(&state).unwrap();

        let providers = state.db.get_all_providers("workbuddy").unwrap();
        assert_eq!(providers.len(), 1);
        let provider = providers.values().next().unwrap();
        assert_eq!(
            provider.id,
            crate::workbuddy_config::stable_provider_id("https://example.com/v1", "sk-test")
        );
        assert_eq!(
            provider.settings_config["models"].as_array().unwrap().len(),
            2
        );
    }
}
