//! WorkBuddy 的 models.json 适配，兼容顶层数组和包含 models 的对象。
//! 通过既有路径函数保留设置与环境覆盖，更新托管模型时保留无关字段及 availableModels 语义。
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::config::{get_home_dir, write_json_file_private};
use crate::error::AppError;

const WORKBUDDY_CONFIG_DIR_ENV: &str = "WORKBUDDY_CONFIG_DIR";

#[derive(Clone, Debug)]
pub struct NativeProviderGroup {
    pub id: String,
    pub config: Value,
}

pub fn get_workbuddy_home() -> PathBuf {
    let default_dir = get_home_dir().join(".workbuddy");
    resolve_workbuddy_home(
        crate::settings::get_workbuddy_override_dir(),
        std::env::var(WORKBUDDY_CONFIG_DIR_ENV).ok(),
        default_dir,
    )
}

fn resolve_workbuddy_home(
    override_dir: Option<PathBuf>,
    env_dir: Option<String>,
    default_dir: PathBuf,
) -> PathBuf {
    override_dir
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| {
            env_dir
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or(default_dir)
}

pub fn get_workbuddy_models_path() -> PathBuf {
    get_workbuddy_home().join("models.json")
}

fn read_document_from(path: &Path) -> Result<Value, AppError> {
    if !path.exists() {
        return Ok(Value::Array(Vec::new()));
    }

    let content = fs::read_to_string(path).map_err(|error| AppError::io(path, error))?;
    if content.trim().is_empty() {
        return Ok(Value::Array(Vec::new()));
    }
    let value: Value =
        serde_json::from_str(&content).map_err(|error| AppError::json(path, error))?;
    match value {
        Value::Array(_) | Value::Object(_) => Ok(value),
        _ => Err(AppError::localized(
            "provider.workbuddy.models.invalid_root",
            "WorkBuddy models.json 必须是模型数组或包含 models 数组的对象",
            "WorkBuddy models.json must be a model array or an object containing a models array",
        )),
    }
}

fn extract_models(document: &Value) -> Result<Vec<Value>, AppError> {
    match document {
        Value::Array(models) => Ok(models.clone()),
        Value::Object(object) => object
            .get("models")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| {
                AppError::localized(
                    "provider.workbuddy.models.missing",
                    "WorkBuddy models.json 的对象格式缺少 models 数组",
                    "The WorkBuddy models.json object format is missing the models array",
                )
            }),
        _ => Err(AppError::localized(
            "provider.workbuddy.models.invalid_root",
            "WorkBuddy models.json 格式无效",
            "Invalid WorkBuddy models.json format",
        )),
    }
}

pub fn read_models() -> Result<Vec<Value>, AppError> {
    let document = read_document_from(&get_workbuddy_models_path())?;
    extract_models(&document)
}

pub fn read_models_snapshot() -> Result<Value, AppError> {
    let document = read_document_from(&get_workbuddy_models_path())?;
    let models = extract_models(&document)?;
    let mut snapshot = Map::new();
    snapshot.insert("models".into(), Value::Array(models));
    if let Some(available) = document.get("availableModels") {
        snapshot.insert("availableModels".into(), available.clone());
    }
    Ok(Value::Object(snapshot))
}

fn model_id(model: &Value) -> Option<&str> {
    model
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

fn normalized_url(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
}

fn api_key(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

pub fn credentials(config: &Value) -> Option<(String, String)> {
    Some((
        normalized_url(config.get("url"))?,
        api_key(config.get("apiKey")),
    ))
}

pub fn stable_provider_id(url: &str, api_key: &str) -> String {
    let digest = Sha256::digest(format!("{url}\0{api_key}").as_bytes());
    format!("workbuddy-{}", &format!("{digest:x}")[..12])
}

fn model_matches_credentials(model: &Value, url: &str, key: &str) -> bool {
    normalized_url(model.get("url")).as_deref() == Some(url) && api_key(model.get("apiKey")) == key
}

fn model_entry(provider: &Value, model: &Value) -> Result<Value, AppError> {
    let (url, key) = credentials(provider).ok_or_else(missing_url)?;
    let mut entry = model.as_object().cloned().ok_or_else(|| {
        AppError::localized(
            "provider.workbuddy.model.not_object",
            "WorkBuddy 模型配置必须是 JSON 对象",
            "WorkBuddy model configuration must be a JSON object",
        )
    })?;
    let id = entry
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::localized(
                "provider.workbuddy.id.required",
                "WorkBuddy 模型 ID 不能为空",
                "WorkBuddy model ID is required",
            )
        })?
        .to_string();
    if entry
        .get("name")
        .and_then(Value::as_str)
        .is_none_or(|name| name.trim().is_empty())
    {
        entry.insert("name".into(), Value::String(id));
    }
    entry.insert("url".into(), Value::String(url));
    entry.insert("apiKey".into(), Value::String(key));
    if let Some(vendor) = provider
        .get("vendor")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|vendor| !vendor.is_empty())
    {
        entry.insert("vendor".into(), Value::String(vendor.to_string()));
    }
    Ok(Value::Object(entry))
}

fn rebuild_document(mut document: Value, models: Vec<Value>) -> Value {
    if document.is_array() {
        return Value::Array(models);
    }

    if let Some(object) = document.as_object_mut() {
        object.insert("models".to_string(), Value::Array(models));
    }
    document
}

fn sync_available_models(document: &mut Value, models: &[Value], removed_ids: &HashSet<String>) {
    let Some(object) = document.as_object_mut() else {
        return;
    };
    let Some(available) = object
        .get_mut("availableModels")
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    let current: Vec<String> = models
        .iter()
        .filter_map(model_id)
        .map(str::to_string)
        .collect();
    let current_set: HashSet<&str> = current.iter().map(String::as_str).collect();
    available.retain(|value| {
        value
            .as_str()
            .is_none_or(|id| !removed_ids.contains(id) || current_set.contains(id))
    });
    for id in current {
        if !available.iter().any(|value| value.as_str() == Some(&id)) {
            available.push(Value::String(id));
        }
    }
}

fn write_document(path: &Path, document: &Value) -> Result<(), AppError> {
    write_json_file_private(path, document)
}

pub fn normalize_provider_config(mut config: Value) -> Result<Value, AppError> {
    let object = config.as_object_mut().ok_or_else(|| {
        AppError::localized(
            "provider.workbuddy.settings.not_object",
            "WorkBuddy 供应商配置必须是 JSON 对象",
            "WorkBuddy provider configuration must be a JSON object",
        )
    })?;

    if !object.get("models").is_some_and(Value::is_array) && object.contains_key("id") {
        let legacy_model = Value::Object(object.clone());
        object.insert("models".into(), Value::Array(vec![legacy_model]));
    }

    let models = object
        .get_mut("models")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            AppError::localized(
                "provider.workbuddy.models.required",
                "WorkBuddy 供应商必须包含模型列表",
                "WorkBuddy provider configuration must contain a model list",
            )
        })?;
    for model in models.iter_mut() {
        normalize_model_object(model)?;
    }
    normalize_provider_fields(object);
    Ok(config)
}

fn normalize_model_object(model: &mut Value) -> Result<(), AppError> {
    let object = model.as_object_mut().ok_or_else(|| {
        AppError::localized(
            "provider.workbuddy.model.not_object",
            "WorkBuddy 模型配置必须是 JSON 对象",
            "WorkBuddy model configuration must be a JSON object",
        )
    })?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::localized(
                "provider.workbuddy.id.required",
                "WorkBuddy 模型 ID 不能为空",
                "WorkBuddy model ID is required",
            )
        })?
        .to_string();
    object.insert("id".into(), Value::String(id.clone()));
    if object
        .get("name")
        .and_then(Value::as_str)
        .is_none_or(|name| name.trim().is_empty())
    {
        object.insert("name".into(), Value::String(id));
    }
    remove_empty_optional_fields(object);
    Ok(())
}

fn normalize_provider_fields(object: &mut Map<String, Value>) {
    for field in ["url", "apiKey", "vendor"] {
        if let Some(value) = object.get_mut(field) {
            if let Some(text) = value.as_str() {
                *value = Value::String(text.trim().to_string());
            }
        }
    }
}

fn remove_empty_optional_fields(object: &mut Map<String, Value>) {
    for field in ["vendor", "maxInputTokens", "maxOutputTokens", "temperature"] {
        if object
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|value| value.trim().is_empty())
        {
            object.remove(field);
        }
    }
}

pub fn provider_model_ids(config: &Value) -> Vec<String> {
    config
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(model_id)
        .map(str::to_string)
        .collect()
}

pub fn provider_is_live(config: &Value) -> Result<bool, AppError> {
    let Some((url, key)) = credentials(config) else {
        return Ok(false);
    };
    let expected = provider_model_ids(config);
    if expected.is_empty() {
        return Ok(false);
    }
    let native = read_models()?;
    Ok(expected.iter().all(|id| {
        native.iter().any(|model| {
            model_id(model) == Some(id.as_str()) && model_matches_credentials(model, &url, &key)
        })
    }))
}

pub fn read_provider_groups() -> Result<Vec<NativeProviderGroup>, AppError> {
    let models = read_models()?;
    let mut groups = HashMap::<(String, String), Vec<(Option<String>, Value)>>::new();
    let mut order = Vec::<(String, String)>::new();

    for model in models {
        let Some(url) = normalized_url(model.get("url")) else {
            continue;
        };
        let key = api_key(model.get("apiKey"));
        let group_key = (url, key);
        if !groups.contains_key(&group_key) {
            order.push(group_key.clone());
        }
        let vendor = model
            .get("vendor")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut model = model;
        if let Some(object) = model.as_object_mut() {
            object.remove("url");
            object.remove("apiKey");
            object.remove("vendor");
        }
        groups.entry(group_key).or_default().push((vendor, model));
    }

    Ok(order
        .into_iter()
        .filter_map(|(url, key)| {
            let entries = groups.remove(&(url.clone(), key.clone()))?;
            let vendor = entries
                .iter()
                .find_map(|(vendor, _)| vendor.as_deref())
                .unwrap_or("custom")
                .to_string();
            let models: Vec<Value> = entries.into_iter().map(|(_, model)| model).collect();
            Some(NativeProviderGroup {
                id: stable_provider_id(&url, &key),
                config: serde_json::json!({
                    "url": url,
                    "apiKey": key,
                    "vendor": vendor,
                    "models": models,
                }),
            })
        })
        .collect())
}

pub fn upsert_provider(config: &Value, previous: Option<&Value>) -> Result<(), AppError> {
    let config = normalize_provider_config(config.clone())?;
    validate_provider_node(&config)?;
    let (next_url, next_key) = credentials(&config).ok_or_else(missing_url)?;
    let previous_credentials = previous.and_then(credentials);
    let path = get_workbuddy_models_path();
    let mut document = read_document_from(&path)?;
    let mut models = extract_models(&document)?;
    let mut removed_ids = HashSet::new();
    for model in &models {
        let matches_next = model_matches_credentials(model, &next_url, &next_key);
        let matches_previous = previous_credentials
            .as_ref()
            .is_some_and(|(url, key)| model_matches_credentials(model, url, key));
        if matches_next || matches_previous {
            if let Some(id) = model_id(model) {
                removed_ids.insert(id.to_string());
            }
        }
    }
    models.retain(|model| {
        let matches_next = model_matches_credentials(model, &next_url, &next_key);
        let matches_previous = previous_credentials
            .as_ref()
            .is_some_and(|(url, key)| model_matches_credentials(model, url, key));
        !matches_next && !matches_previous
    });
    for model in config
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        models.push(model_entry(&config, model)?);
    }

    sync_available_models(&mut document, &models, &removed_ids);
    let document = rebuild_document(document, models);
    write_document(&path, &document)
}

pub fn remove_provider(config: &Value) -> Result<bool, AppError> {
    let Some((url, key)) = credentials(config) else {
        return Ok(false);
    };
    let path = get_workbuddy_models_path();
    if !path.exists() {
        return Ok(false);
    }

    let mut document = read_document_from(&path)?;
    let mut models = extract_models(&document)?;
    let before = models.len();
    let removed_ids: HashSet<String> = models
        .iter()
        .filter(|model| model_matches_credentials(model, &url, &key))
        .filter_map(model_id)
        .map(str::to_string)
        .collect();
    models.retain(|model| !model_matches_credentials(model, &url, &key));
    if models.len() == before {
        return Ok(false);
    }
    sync_available_models(&mut document, &models, &removed_ids);
    let document = rebuild_document(document, models);
    write_document(&path, &document)?;
    Ok(true)
}

pub fn validate_provider_node(config: &Value) -> Result<(), AppError> {
    let object = config.as_object().ok_or_else(|| {
        AppError::localized(
            "provider.workbuddy.settings.not_object",
            "WorkBuddy 供应商配置必须是 JSON 对象",
            "WorkBuddy provider configuration must be a JSON object",
        )
    })?;
    let url = normalized_url(object.get("url")).ok_or_else(missing_url)?;
    let parsed = url::Url::parse(&url).map_err(|_| invalid_url())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(invalid_url());
    }
    let key = api_key(object.get("apiKey"));
    if key.is_empty() && !is_loopback_url(&parsed) {
        return Err(AppError::localized(
            "provider.workbuddy.api_key.required",
            "WorkBuddy 非本机接口地址必须填写 API Key",
            "WorkBuddy requires an API key for non-loopback endpoints",
        ));
    }
    if let Some(vendor) = object.get("vendor") {
        if !vendor.is_string() {
            return Err(invalid_field("vendor", "string"));
        }
    }

    let models = object
        .get("models")
        .and_then(Value::as_array)
        .filter(|models| !models.is_empty())
        .ok_or_else(|| {
            AppError::localized(
                "provider.workbuddy.models.required",
                "WorkBuddy 供应商至少需要一个模型",
                "WorkBuddy provider requires at least one model",
            )
        })?;
    let mut ids = HashSet::new();
    for model in models {
        let model = model.as_object().ok_or_else(|| {
            AppError::localized(
                "provider.workbuddy.model.not_object",
                "WorkBuddy 模型配置必须是 JSON 对象",
                "WorkBuddy model configuration must be a JSON object",
            )
        })?;
        let id = model
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                AppError::localized(
                    "provider.workbuddy.id.required",
                    "WorkBuddy 模型 ID 不能为空",
                    "WorkBuddy model ID is required",
                )
            })?
            .to_string();
        if !ids.insert(id.clone()) {
            return Err(AppError::localized(
                "provider.workbuddy.id.duplicate",
                format!("WorkBuddy 模型 ID {id} 重复"),
                format!("WorkBuddy model ID {id} is duplicated"),
            ));
        }
        for field in ["name", "vendor"] {
            if let Some(value) = model.get(field) {
                if !value.is_string() {
                    return Err(invalid_field(field, "string"));
                }
            }
        }
        for field in ["maxInputTokens", "maxOutputTokens", "temperature"] {
            if let Some(value) = model.get(field) {
                let numeric = value.as_f64().is_some()
                    || value
                        .as_str()
                        .is_some_and(|value| value.trim().parse::<f64>().is_ok());
                if !numeric {
                    return Err(invalid_field(field, "number"));
                }
            }
        }
        for field in [
            "supportsToolCall",
            "supportsImages",
            "supportsReasoning",
            "onlyReasoning",
            "useCustomProtocol",
        ] {
            if let Some(value) = model.get(field) {
                if !value.is_boolean() {
                    return Err(invalid_field(field, "boolean"));
                }
            }
        }
        if let Some(reasoning) = model.get("reasoning") {
            if !reasoning.is_object() {
                return Err(invalid_field("reasoning", "object"));
            }
        }
    }
    Ok(())
}

fn missing_url() -> AppError {
    AppError::localized(
        "provider.workbuddy.url.required",
        "WorkBuddy 接口地址不能为空",
        "WorkBuddy endpoint URL is required",
    )
}

fn invalid_url() -> AppError {
    AppError::localized(
        "provider.workbuddy.url.invalid",
        "WorkBuddy 接口地址必须是完整的 HTTP(S) URL",
        "WorkBuddy endpoint URL must be an absolute HTTP(S) URL",
    )
}

fn invalid_field(field: &str, expected: &str) -> AppError {
    AppError::localized(
        "provider.workbuddy.field.invalid",
        format!("WorkBuddy 字段 {field} 必须是 {expected}"),
        format!("WorkBuddy field {field} must be a {expected}"),
    )
}

fn is_loopback_url(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(host)) => {
            host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

pub fn provider_name(config: &Value, fallback: &str) -> String {
    config
        .get("vendor")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "custom")
        .map(str::to_string)
        .or_else(|| {
            config
                .get("url")
                .and_then(Value::as_str)
                .and_then(|url| url::Url::parse(url).ok())
                .and_then(|url| url.host_str().map(str::to_string))
        })
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    fn with_config_dir<T>(test: impl FnOnce(&Path) -> T) -> T {
        let temp = TempDir::new().expect("temp dir");
        let config_dir = temp.path().join(".workbuddy");
        let previous = std::env::var_os(WORKBUDDY_CONFIG_DIR_ENV);
        std::env::set_var(WORKBUDDY_CONFIG_DIR_ENV, &config_dir);
        let result = test(&config_dir);
        match previous {
            Some(value) => std::env::set_var(WORKBUDDY_CONFIG_DIR_ENV, value),
            None => std::env::remove_var(WORKBUDDY_CONFIG_DIR_ENV),
        }
        result
    }

    fn provider_config() -> Value {
        json!({
            "url": "https://example.com/v1",
            "apiKey": "secret",
            "vendor": "智枢平台",
            "models": [
                {"id": "model-a", "name": "Model A"},
                {"id": "model-b", "name": "Model B"}
            ]
        })
    }

    #[test]
    #[serial]
    fn upsert_writes_each_model_under_one_provider_and_preserves_wrapper() {
        with_config_dir(|config_dir| {
            let path = config_dir.join("models.json");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(
                &path,
                r#"{"future":true,"models":[],"availableModels":["builtin"]}"#,
            )
            .unwrap();

            upsert_provider(&provider_config(), None).unwrap();

            let value: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(value["future"], true);
            assert_eq!(value["models"].as_array().unwrap().len(), 2);
            assert_eq!(value["models"][0]["id"], "model-a");
            assert_eq!(value["models"][1]["id"], "model-b");
            assert_eq!(value["models"][0]["url"], "https://example.com/v1");
            assert_eq!(value["models"][0]["apiKey"], "secret");
            assert_eq!(
                value["availableModels"],
                json!(["builtin", "model-a", "model-b"])
            );
        });
    }

    #[test]
    #[serial]
    fn groups_native_models_by_endpoint_and_key() {
        with_config_dir(|config_dir| {
            let path = config_dir.join("models.json");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(
                &path,
                r#"[
                    {"id":"a","name":"A","url":"https://example.com/v1","apiKey":"k"},
                    {"id":"b","name":"B","url":"https://example.com/v1/","apiKey":"k"},
                    {"id":"c","name":"C","url":"https://other.example/v1","apiKey":"k2"}
                ]"#,
            )
            .unwrap();

            let groups = read_provider_groups().unwrap();
            assert_eq!(groups.len(), 2);
            assert_eq!(groups[0].config["models"].as_array().unwrap().len(), 2);
            assert_eq!(groups[1].config["models"].as_array().unwrap().len(), 1);
        });
    }

    #[test]
    #[serial]
    fn upsert_replaces_previous_provider_models_and_remove_updates_visibility() {
        with_config_dir(|config_dir| {
            let path = config_dir.join("models.json");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, r#"{"models":[],"availableModels":[]}"#).unwrap();
            let config = provider_config();
            upsert_provider(&config, None).unwrap();
            let mut next = config.clone();
            next["models"] = json!([{"id":"model-a","name":"Updated A"}]);
            upsert_provider(&next, Some(&config)).unwrap();

            let models = read_models().unwrap();
            assert_eq!(models.len(), 1);
            assert_eq!(models[0]["id"], "model-a");
            assert_eq!(models[0]["name"], "Updated A");

            assert!(remove_provider(&next).unwrap());
            let value: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(value["availableModels"], json!([]));
        });
    }

    #[test]
    #[serial]
    fn normalizes_legacy_single_model_provider() {
        let config = normalize_provider_config(json!({
            "id": "legacy-model",
            "name": "Legacy",
            "url": "http://127.0.0.1:11434/v1",
            "apiKey": ""
        }))
        .unwrap();
        assert_eq!(config["models"][0]["id"], "legacy-model");
        validate_provider_node(&config).unwrap();
    }

    #[test]
    fn rejects_non_loopback_provider_without_api_key() {
        let error = validate_provider_node(&json!({
            "url": "https://example.com/v1",
            "apiKey": "",
            "models": [{"id": "a"}]
        }))
        .unwrap_err();
        assert!(error.to_string().contains("API Key"));
    }
}
