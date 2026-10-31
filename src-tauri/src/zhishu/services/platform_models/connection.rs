//! 读取用户现有模型选择，构建或增量刷新托管供应商的模型目录。
//! 保留未知字段、未报告参数和可沿用的用户选择；最终发布仍由 ProviderService 负责。
use super::{adapter, fail, AgentModel, PlatformAuthError};
use crate::zhishu::integrations::ccswitch::providers::Providers as ProviderService;
use crate::{app_config::AppType, error::AppError, provider::Provider, store::AppState};
use serde_json::{json, Value};

pub(crate) fn read_settings(state: &AppState, app: &AppType) -> Result<Value, AppError> {
    let path = match app {
        AppType::Pi => {
            return serde_json::to_value(crate::pi_config::read_pi_native_defaults()?)
                .map_err(|source| AppError::JsonSerialize { source })
        }
        AppType::Mcode => {
            let path = crate::mcode_config::config_path();
            if !path.try_exists().map_err(|e| AppError::io(&path, e))? {
                return Ok(json!({}));
            }
            let text = std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
            return serde_yaml::from_str(&text)
                .map_err(|_| AppError::Config("Invalid MCode config".into()));
        }
        AppType::Kimi => return adapter::kimi_selection(),
        AppType::Dsh => return Ok(json!({})),
        AppType::Claude => crate::config::get_claude_settings_path(),
        AppType::ClaudeDesktop => {
            let current = ProviderService::current(state, AppType::ClaudeDesktop)?;
            if current.is_empty() {
                return Ok(json!({}));
            }
            let provider = state
                .db
                .get_provider_by_id(&current, AppType::ClaudeDesktop.as_str())?
                .ok_or_else(|| AppError::Message(format!("供应商 {current} 不存在")))?;
            let model = provider
                .meta
                .as_ref()
                .and_then(|meta| meta.platform_managed.as_ref())
                .map(|managed| managed.model.clone())
                .or_else(|| {
                    provider
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.claude_desktop_model_routes.values().next())
                        .map(|route| route.model.clone())
                })
                .or_else(|| {
                    provider
                        .settings_config
                        .pointer("/env/ANTHROPIC_MODEL")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                });
            return Ok(json!({
                "model": model,
                "providerId": current,
            }));
        }
        AppType::Codex => crate::codex_config::get_codex_config_path(),
        AppType::Gemini => {
            // settings.json may exist even when no .env has been created yet.
            let config_path = crate::gemini_config::get_gemini_settings_path();
            let config: Value = if config_path
                .try_exists()
                .map_err(|e| AppError::io(&config_path, e))?
            {
                crate::config::read_json_file(&config_path)?
            } else {
                json!({})
            };
            return Ok(json!({"env": crate::gemini_config::read_gemini_env()?, "config": config}));
        }
        AppType::GrokBuild => crate::grok_config::get_grok_config_path(),
        AppType::OpenCode => crate::opencode_config::get_opencode_config_path()?,
        AppType::OpenClaw => crate::openclaw_config::get_openclaw_config_path(),
        AppType::Hermes => crate::hermes_config::get_hermes_config_path(),
        AppType::WorkBuddy => {
            return crate::workbuddy_config::read_models_snapshot();
        }
        _ => return Err(AppError::InvalidInput("Unsupported platform Agent".into())),
    };
    if !path.try_exists().map_err(|e| AppError::io(&path, e))? {
        return Ok(json!({}));
    }
    ProviderService::read_live_settings(app.clone())
}

fn toml_config(live: &Value) -> Result<toml::Value, PlatformAuthError> {
    live.get("config")
        .and_then(Value::as_str)
        .unwrap_or("")
        .parse()
        .map_err(|_| fail("CONFIG", "现有工具配置无法解析，请先在原配置页面检查"))
}

fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
}

fn reference_model(reference: Option<String>) -> Option<String> {
    reference.map(|s| {
        s.split_once('/')
            .map(|(_, model)| model.to_string())
            .unwrap_or(s)
    })
}

pub(crate) fn current_model(
    agent: &str,
    live: &Value,
) -> Result<Option<String>, PlatformAuthError> {
    Ok(match agent {
        "claude" => text(live.get("model")).or_else(|| text(live.pointer("/env/ANTHROPIC_MODEL"))),
        "claude-desktop" => {
            text(live.get("model")).or_else(|| text(live.pointer("/env/ANTHROPIC_MODEL")))
        }
        "codex" => toml_config(live)?
            .get("model")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        "gemini" => text(live.pointer("/env/GEMINI_MODEL"))
            .or_else(|| text(live.pointer("/config/model/name"))),
        "grokbuild" => {
            let config = toml_config(live)?;
            let profile = config
                .get("models")
                .and_then(|m| m.get("default"))
                .and_then(toml::Value::as_str);
            profile
                .and_then(|p| config.get("model").and_then(|m| m.get(p)))
                .and_then(|m| m.get("model"))
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        }
        "opencode" => reference_model(text(live.get("model"))),
        "openclaw" => reference_model(
            text(live.pointer("/agents/defaults/model/primary"))
                .or_else(|| text(live.pointer("/agents/defaults/model"))),
        ),
        "hermes" => text(live.pointer("/model/default")),
        "pi" => text(live.get("defaultModel")),
        "kimi" => text(live.get("defaultModel")),
        "mcode" => reference_model(text(live.get("defaultModel"))),
        "workbuddy" => live
            .get("availableModels")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find_map(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                live.get("models")
                    .and_then(Value::as_array)
                    .and_then(|models| models.first())
                    .and_then(|model| model.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            }),
        _ => None,
    })
}

pub(crate) fn select_model<'a>(
    agent: &str,
    models: &'a [AgentModel],
    live: &Value,
) -> Result<&'a AgentModel, PlatformAuthError> {
    let current = current_model(agent, live)?;
    let native = match agent {
        "claude" | "claude-desktop" => "anthropic",
        "gemini" => "gemini",
        "workbuddy" => "chat",
        "kimi" => "responses",
        _ => "responses",
    };
    let available = || models.iter().filter(|m| m.agent == agent && m.available);
    available()
        .find(|m| current.as_deref() == Some(m.model.as_str()))
        .or_else(|| available().find(|m| m.protocol == native))
        .or_else(|| available().next())
        .ok_or_else(|| {
            fail(
                "MODEL_UNAVAILABLE",
                "当前账号没有适配此工具的可用模型，请检查平台分组和权限",
            )
        })
}

pub(crate) fn populate_catalog(
    provider: &mut Provider,
    model: &AgentModel,
    models: &[AgentModel],
) -> Result<(), PlatformAuthError> {
    let compatible: Vec<_> = models
        .iter()
        .filter(|m| m.agent == model.agent && m.protocol == model.protocol && m.available)
        .collect();
    let provider_id = provider.id.clone();
    let settings = &mut provider.settings_config;
    if model.agent == "claude-desktop" {
        let routes = adapter::desktop_routes(compatible.iter().copied());
        let direct = model.protocol == "anthropic"
            && compatible
                .iter()
                .all(|item| crate::claude_desktop_config::is_claude_safe_model_id(&item.model));
        if let Some(meta) = provider.meta.as_mut() {
            meta.claude_desktop_mode = Some(if direct {
                crate::provider::ClaudeDesktopMode::Direct
            } else {
                crate::provider::ClaudeDesktopMode::Proxy
            });
            meta.claude_desktop_model_routes = routes;
        }
        return Ok(());
    }
    if model.agent == "workbuddy" {
        adapter::publish_workbuddy_models(provider, compatible.iter().copied())?;
        return Ok(());
    }
    for item in &compatible {
        let single = adapter::provider(
            item,
            provider.website_url.as_deref().unwrap_or(""),
            0,
            "",
            0,
            None,
        )?;
        match model.agent.as_str() {
            "codex" => merge_array(
                &mut settings["modelCatalog"]["models"],
                &single.settings_config["modelCatalog"]["models"],
                "model",
            ),
            "opencode" | "mcode" | "dsh" => {
                if let (Some(target), Some(source)) = (
                    settings["models"].as_object_mut(),
                    single.settings_config["models"].as_object(),
                ) {
                    target.extend(source.clone());
                }
            }
            "kimi" => {
                if let (Some(target), Some(source)) = (
                    settings["models"].as_object_mut(),
                    single.settings_config["models"].as_object(),
                ) {
                    for entry in source.values() {
                        let Some(model_id) = entry.get("model").and_then(Value::as_str) else {
                            continue;
                        };
                        let mut entry = entry.clone();
                        entry["provider"] = json!(provider_id);
                        target.insert(format!("{provider_id}/{model_id}"), entry);
                    }
                }
            }
            "pi" | "openclaw" | "hermes" => merge_array(
                &mut settings["models"],
                &single.settings_config["models"],
                "id",
            ),
            _ => (),
        }
    }
    Ok(())
}

/// The successful platform catalog owns model membership. Retained models keep
/// their unreported/custom fields; removed models must not survive a merge.
/// The user's selected model, endpoint and unrelated settings stay unchanged.
pub(crate) fn refresh_catalog(provider: &mut Provider, desired: &Provider, app: &AppType) -> bool {
    let previous = provider.settings_config.clone();
    let mut format_changed = false;
    if app == &AppType::OpenCode {
        adapter::upgrade_opencode_settings(&mut provider.settings_config);
        if let Some(meta) = provider.meta.as_mut() {
            let native = Some(crate::provider::OpenCodeConfigFormat::V2);
            format_changed = meta.opencode_config_format != native;
            meta.opencode_config_format = native;
        }
    }
    let fields: &[&str] = match app {
        AppType::Codex => &["modelCatalog"],
        AppType::OpenCode
        | AppType::Mcode
        | AppType::Dsh
        | AppType::Kimi
        | AppType::Pi
        | AppType::OpenClaw
        | AppType::Hermes => &["models"],
        AppType::WorkBuddy => &[],
        _ => &[],
    };
    for &field in fields {
        if let Some(source) = desired.settings_config.get(field) {
            if app == &AppType::Codex {
                if let Some(models) = source.get("models") {
                    refresh_model_membership(
                        &mut provider.settings_config[field]["models"],
                        models,
                        "model",
                    );
                }
                // Preserve catalog-level options such as autoReviewModel.
                if let Some(options) = source.as_object() {
                    for (key, value) in options.iter().filter(|(key, _)| key.as_str() != "models") {
                        merge_catalog_value(&mut provider.settings_config[field][key], value);
                    }
                }
            } else {
                refresh_model_membership(&mut provider.settings_config[field], source, "id");
            }
        }
    }
    if app == &AppType::Dsh {
        if let Some(models) = provider.settings_config["models"].as_object_mut() {
            for model in models.values_mut() {
                if let Some(efforts) = model
                    .get_mut("reasoningEfforts")
                    .and_then(Value::as_object_mut)
                {
                    efforts.retain(|level, _| adapter::dsh_reasoning_effort_supported(level));
                    if !efforts.keys().any(|level| level != "off") {
                        model["reasoningEfforts"] = json!(false);
                        model["reasoning"] = json!(false);
                    }
                }
            }
        }
    }
    let mut changed = format_changed || previous != provider.settings_config;
    if app == &AppType::ClaudeDesktop {
        if let (Some(meta), Some(desired_meta)) = (provider.meta.as_mut(), desired.meta.as_ref()) {
            let before_mode = meta.claude_desktop_mode.clone();
            let before_routes = meta.claude_desktop_model_routes.clone();
            meta.claude_desktop_mode = desired_meta.claude_desktop_mode.clone();
            meta.claude_desktop_model_routes = desired_meta.claude_desktop_model_routes.clone();
            changed |= before_mode != meta.claude_desktop_mode
                || before_routes != meta.claude_desktop_model_routes;
        }
    }
    if app == &AppType::WorkBuddy {
        let existing: std::collections::HashMap<String, Value> = provider
            .settings_config
            .get("models")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|model| {
                model
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|id| (id.to_string(), model.clone()))
            })
            .collect();
        let refreshed_models = desired
            .settings_config
            .get("models")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|model| {
                let Some(id) = model.get("id").and_then(Value::as_str) else {
                    return model.clone();
                };
                let mut merged = existing.get(id).cloned().unwrap_or(Value::Null);
                merge_catalog_value(&mut merged, model);
                merged
            })
            .collect::<Vec<_>>();
        if provider.settings_config.get("url") != desired.settings_config.get("url") {
            provider.settings_config["url"] = desired.settings_config["url"].clone();
            changed = true;
        }
        if provider.settings_config.get("vendor") != desired.settings_config.get("vendor") {
            provider.settings_config["vendor"] = desired.settings_config["vendor"].clone();
            changed = true;
        }
        let next_models = Value::Array(refreshed_models);
        if provider.settings_config.get("models") != Some(&next_models) {
            provider.settings_config["models"] = next_models;
            changed = true;
        }
    }
    if let (Some(meta), Some(desired_meta)) = (provider.meta.as_mut(), desired.meta.as_ref()) {
        if meta.api_format != desired_meta.api_format {
            meta.api_format = desired_meta.api_format.clone();
            // Native SDK/API selectors must agree with the published format.
            for field in ["npm", "package", "api", "api_mode"] {
                if let Some(value) = desired.settings_config.get(field) {
                    provider.settings_config[field] = value.clone();
                }
            }
            if app == &AppType::Kimi {
                if let Some(provider_type) = desired.settings_config.pointer("/provider/type") {
                    provider.settings_config["provider"]["type"] = provider_type.clone();
                }
            }
            changed = true;
        }
    }
    changed
}

fn refresh_model_membership(target: &mut Value, source: &Value, id_field: &str) {
    let previous = std::mem::take(target);
    *target = match source {
        Value::Array(models) => Value::Array(
            models
                .iter()
                .map(|model| {
                    let mut entry = previous
                        .as_array()
                        .and_then(|old| {
                            old.iter().find(|entry| {
                                model.get(id_field).is_some()
                                    && entry.get(id_field) == model.get(id_field)
                            })
                        })
                        .cloned()
                        .unwrap_or(Value::Null);
                    merge_catalog_value(&mut entry, model);
                    entry
                })
                .collect(),
        ),
        Value::Object(models) => Value::Object(
            models
                .iter()
                .map(|(id, model)| {
                    let mut entry = previous.get(id).cloned().unwrap_or(Value::Null);
                    merge_catalog_value(&mut entry, model);
                    (id.clone(), entry)
                })
                .collect(),
        ),
        _ => source.clone(),
    };
}

fn merge_catalog_value(target: &mut Value, source: &Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                if key == "variants" {
                    target.insert(key.clone(), value.clone());
                } else {
                    merge_catalog_value(target.entry(key.clone()).or_insert(Value::Null), value);
                }
            }
        }
        (Value::Array(target), Value::Array(source))
            if source.iter().all(|entry| entry.is_object()) =>
        {
            for entry in source {
                let id_field = if entry.get("model").is_some() {
                    "model"
                } else {
                    "id"
                };
                if let Some(existing) = target.iter_mut().find(|old| {
                    entry.get(id_field).is_some() && old.get(id_field) == entry.get(id_field)
                }) {
                    merge_catalog_value(existing, entry);
                } else {
                    target.push(entry.clone());
                }
            }
        }
        (target, source) => *target = source.clone(),
    }
}

fn merge_array(target: &mut Value, source: &Value, id: &str) {
    if let (Some(target), Some(source)) = (target.as_array_mut(), source.as_array()) {
        for entry in source {
            if let Some(index) = target.iter().position(|old| old[id] == entry[id]) {
                target[index] = entry.clone();
            } else {
                target.push(entry.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsh_refresh_removes_unsupported_reasoning_effort_from_existing_provider() {
        let mut model = models("dsh").remove(0);
        model.reasoning_levels = vec!["high".into(), "ultra".into()];
        model.reasoning = Some(true);
        let desired =
            adapter::provider(&model, "https://platform.test", 1, "key", 7, None).unwrap();
        let mut provider = desired.clone();
        provider.settings_config["models"][&model.model]["reasoningEfforts"] =
            json!({"high":"high","ultra":"ultra"});

        assert!(refresh_catalog(&mut provider, &desired, &AppType::Dsh));
        assert_eq!(
            provider.settings_config["models"][&model.model]["reasoningEfforts"],
            json!({"high":"high"})
        );
        assert!(!refresh_catalog(&mut provider, &desired, &AppType::Dsh));

        let mut only_unsupported_model = model.clone();
        only_unsupported_model.reasoning_levels = vec!["ultra".into()];
        let desired_only = adapter::provider(
            &only_unsupported_model,
            "https://platform.test",
            1,
            "key",
            7,
            None,
        )
        .unwrap();
        let mut only_unsupported = desired_only.clone();
        only_unsupported.settings_config["models"][&model.model]["reasoningEfforts"] =
            json!({"ultra":"ultra"});
        assert!(refresh_catalog(
            &mut only_unsupported,
            &desired_only,
            &AppType::Dsh
        ));
        assert_eq!(
            only_unsupported.settings_config["models"][&model.model]["reasoningEfforts"],
            false
        );
    }

    fn models(agent: &str) -> Vec<AgentModel> {
        ["first-model", "kept-model"].into_iter().map(|id| serde_json::from_value(json!({
            "id": id, "agent":agent, "model":id, "name":id,
            "protocol": match agent { "claude" => "anthropic", "gemini" => "gemini", "workbuddy" => "chat", _ => "responses" },
            "category":"available", "available":true, "supportsTools":true, "supportsStreaming":true,
            "contextWindow":32000, "maxOutputTokens":4096,
        })).unwrap()).collect()
    }

    #[test]
    fn registers_full_catalogs_and_uses_existing_additive_model_ids() {
        for (agent, live) in [
            ("opencode", json!({"model":"old/kept-model"})),
            (
                "openclaw",
                json!({"agents":{"defaults":{"model":{"primary":"old/kept-model"}}}}),
            ),
            ("pi", json!({"defaultModel":"kept-model"})),
            (
                "mcode",
                json!({"defaultModel":"custom_provider:old/kept-model"}),
            ),
            (
                "hermes",
                json!({"model":{"default":"kept-model","provider":"old"}}),
            ),
        ] {
            let models = models(agent);
            let model = select_model(agent, &models, &live).unwrap();
            assert_eq!(model.model, "kept-model", "{agent}");
            let mut provider =
                adapter::provider(model, "https://platform.test", 1, "account-key", 7, None)
                    .unwrap();
            populate_catalog(&mut provider, model, &models).unwrap();
            let catalog = &provider.settings_config["models"];
            assert_eq!(
                catalog
                    .as_array()
                    .map(Vec::len)
                    .or_else(|| catalog.as_object().map(|m| m.len())),
                Some(2),
                "{agent}"
            );
        }
    }

    #[test]
    fn successful_catalog_replaces_membership_and_preserves_retained_model_details() {
        for app in [
            AppType::Codex,
            AppType::OpenCode,
            AppType::Pi,
            AppType::OpenClaw,
            AppType::Hermes,
            AppType::Mcode,
            AppType::Dsh,
            AppType::Kimi,
            AppType::WorkBuddy,
        ] {
            let object_models = matches!(
                app,
                AppType::OpenCode | AppType::Mcode | AppType::Dsh | AppType::Kimi
            );
            let field = if app == AppType::Codex { "model" } else { "id" };
            let previous = if object_models {
                json!({"removed":{"name":"Removed"}, "kept":{"custom":"keep", "context":100}})
            } else {
                json!([{(field):"removed"}, {(field):"kept", "custom":"keep", "context":100}])
            };
            let next = if object_models {
                json!({"kept":{"name":"Kept", "context":200}, "new":{"name":"New"}})
            } else {
                json!([{(field):"kept", "name":"Kept", "context":200}, {(field):"new", "name":"New"}])
            };
            let settings = |models: Value| {
                if app == AppType::Codex {
                    json!({"modelCatalog":{"models":models,"autoReviewModel":"kept"}, "config":"model = 'user-selected'\n", "custom":"keep"})
                } else {
                    json!({"models":models, "custom":"keep"})
                }
            };
            let mut provider =
                Provider::with_id("managed".into(), "Managed".into(), settings(previous), None);
            let desired =
                Provider::with_id("managed".into(), "Managed".into(), settings(next), None);
            assert!(refresh_catalog(&mut provider, &desired, &app), "{app:?}");
            let result = if app == AppType::Codex {
                &provider.settings_config["modelCatalog"]["models"]
            } else {
                &provider.settings_config["models"]
            };
            let kept = if object_models {
                assert!(result.get("removed").is_none(), "{app:?}");
                assert_eq!(result.as_object().unwrap().len(), 2);
                &result["kept"]
            } else {
                assert_eq!(result.as_array().unwrap().len(), 2, "{app:?}");
                assert_eq!(result[0][field], "kept");
                assert_eq!(result[1][field], "new");
                &result[0]
            };
            assert_eq!(kept["custom"], "keep", "{app:?}");
            assert_eq!(kept["context"], 200);
            assert_eq!(provider.settings_config["custom"], "keep");
            if app == AppType::Codex {
                assert_eq!(
                    provider.settings_config["config"],
                    "model = 'user-selected'\n"
                );
                assert_eq!(
                    provider.settings_config["modelCatalog"]["autoReviewModel"],
                    "kept"
                );
            }
            assert!(!refresh_catalog(&mut provider, &desired, &app), "{app:?}");
        }
        for empty in [json!([]), json!({})] {
            let mut previous = json!([{"id":"removed"}]);
            refresh_model_membership(&mut previous, &empty, "id");
            assert_eq!(previous, empty);
        }
    }

    #[test]
    fn desktop_refresh_replaces_removed_routes() {
        let model: AgentModel = serde_json::from_value(json!({
            "id":"entry",
            "agent":"claude-desktop",
            "model":"claude-sonnet-5",
            "name":"Claude Sonnet 5",
            "protocol":"anthropic",
            "category":"available",
            "available":true,
            "supportsTools":true,
            "supportsStreaming":true
        }))
        .unwrap();
        let desired =
            adapter::provider(&model, "https://platform.test", 1, "key", 2, None).unwrap();
        let mut existing = desired.clone();
        existing
            .meta
            .as_mut()
            .unwrap()
            .claude_desktop_model_routes
            .insert(
                "claude-opus-5".into(),
                crate::provider::ClaudeDesktopModelRoute {
                    model: "removed-model".into(),
                    label_override: None,
                    supports_1m: None,
                },
            );

        assert!(refresh_catalog(
            &mut existing,
            &desired,
            &AppType::ClaudeDesktop
        ));
        assert_eq!(
            existing.meta.as_ref().unwrap().claude_desktop_model_routes,
            desired.meta.as_ref().unwrap().claude_desktop_model_routes
        );
    }

    #[test]
    fn rejects_empty_catalogs_and_malformed_existing_toml_before_writing() {
        assert!(select_model("codex", &[], &json!({})).is_err());
        assert!(select_model("codex", &models("codex"), &json!({"config":"[broken"})).is_err());
    }
}
