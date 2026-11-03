//! Native selection fixtures for serialized platform tests only.
//! Runtime publishing remains in upstream ProviderService; these simulate user selection.
use crate::{config::atomic_write_private, error::AppError};
use serde_json::{json, Value};
use std::path::Path;

fn write_json(path: &Path, value: &Value) -> Result<(), AppError> {
    atomic_write_private(
        path,
        &serde_json::to_vec_pretty(value).map_err(|source| AppError::JsonSerialize { source })?,
    )
}

pub(super) fn select_opencode(id: &str, model: &str) -> Result<(), AppError> {
    let mut config = crate::opencode_config::read_opencode_config()?;
    if ["providers", "provider"]
        .iter()
        .all(|key| config.get(*key).and_then(|p| p.get(id)).is_none())
    {
        return Err(AppError::Config(
            "The platform provider has not been configured".into(),
        ));
    }
    config["model"] = json!(format!("{id}/{model}"));
    config["small_model"] = json!(format!("{id}/{model}"));
    if let Some(disabled) = config
        .get_mut("disabled_providers")
        .and_then(Value::as_array_mut)
    {
        disabled.retain(|value| value.as_str() != Some(id));
    }
    if let Some(enabled) = config
        .get_mut("enabled_providers")
        .and_then(Value::as_array_mut)
    {
        if !enabled.iter().any(|value| value.as_str() == Some(id)) {
            enabled.push(json!(id));
        }
    }
    write_json(
        &crate::opencode_config::get_opencode_config_path()?,
        &config,
    )
}

pub(super) fn select_pi(id: &str, model: &str) -> Result<(), AppError> {
    let path = crate::pi_config::get_pi_settings_path()?;
    let mut document: Value = if path.exists() {
        json5::from_str(&std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?)
            .map_err(|e| AppError::Config(e.to_string()))?
    } else {
        json!({})
    };
    let object = document
        .as_object_mut()
        .ok_or_else(|| AppError::Config("Pi settings must be an object".into()))?;
    object.insert("defaultProvider".into(), json!(id));
    object.insert("defaultModel".into(), json!(model));
    write_json(&path, &document)
}

fn read_mcode() -> Result<serde_yaml::Value, AppError> {
    let path = crate::mcode_config::config_path();
    let text = std::fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    serde_yaml::from_str(&text).map_err(|e| AppError::Config(e.to_string()))
}

pub(super) fn select_mcode(id: &str, model: &str) -> Result<(), AppError> {
    let mut document = read_mcode()?;
    if document
        .get("custom_provider")
        .and_then(|v| v.get(id))
        .and_then(|v| v.get("models"))
        .and_then(|v| v.get(model))
        .is_none()
    {
        return Err(AppError::Config(
            "The selected MCode model has not been configured".into(),
        ));
    }
    document["defaultModel"] = format!("custom_provider:{id}/{model}").into();
    document["defaultLightModel"] = format!("custom_provider:{id}/{model}").into();
    atomic_write_private(
        &crate::mcode_config::config_path(),
        serde_yaml::to_string(&document)
            .map_err(|e| AppError::Config(e.to_string()))?
            .as_bytes(),
    )
}

pub(super) fn mcode_selected(id: &str, model: &str) -> Result<bool, AppError> {
    Ok(read_mcode()?
        .get("defaultModel")
        .and_then(serde_yaml::Value::as_str)
        == Some(format!("custom_provider:{id}/{model}").as_str()))
}

pub(super) fn select_kimi(id: &str, model: &str) -> Result<(), AppError> {
    let path = crate::kimi_config::config_path();
    let mut document = crate::kimi_config::read(&path)?;
    if document.get("providers").and_then(|p| p.get(id)).is_none() {
        return Err(AppError::InvalidInput(format!(
            "Kimi Code provider '{id}' is not enabled"
        )));
    }
    let key = document
        .get("models")
        .and_then(Value::as_object)
        .and_then(|models| {
            models.iter().find_map(|(key, entry)| {
                (entry.get("provider").and_then(Value::as_str) == Some(id)
                    && entry.get("model").and_then(Value::as_str) == Some(model))
                .then(|| key.clone())
            })
        })
        .ok_or_else(|| {
            AppError::InvalidInput(format!("Kimi Code provider '{id}' has no model '{model}'"))
        })?;
    document["default_model"] = json!(key);
    atomic_write_private(
        &path,
        toml::to_string_pretty(&document)
            .map_err(|e| AppError::Config(e.to_string()))?
            .as_bytes(),
    )
}

pub(super) fn kimi_selected(id: &str, model: &str) -> Result<bool, AppError> {
    let document = crate::kimi_config::read(&crate::kimi_config::config_path())?;
    Ok(document
        .get("default_model")
        .and_then(Value::as_str)
        .and_then(|key| document.get("models").and_then(|models| models.get(key)))
        .is_some_and(|entry| {
            entry.get("provider").and_then(Value::as_str) == Some(id)
                && entry.get("model").and_then(Value::as_str) == Some(model)
        }))
}
