//! 认领旧版智枢（0.0.x 二开 WorkBuddy 适配器）写入的平台条目。
//!
//! 旧适配器把平台模型直接写进 `models.json`，条目上没有归属标记，只能按条目自带的
//! `url` + `apiKey` 识别。改用上游 WorkBuddy 后，归属标记是
//! [`crate::workbuddy_config::OWNER_FIELD`]（智枢补丁为 `zhishuCcsProviderId`），
//! 没有标记的条目会被当作用户条目永远保留，平台同步再写入同名模型时还会被上游拒绝。
//!
//! 平台同步时调用 [`claim_legacy_platform_entries`]：未标记、且 `url`/`apiKey` 与当前账号的
//! 平台供应商一致的条目补上归属标记，随后上游写入会按归属整体替换这些条目。
//! 用户自建条目（凭据不匹配）和原版 CC Switch 的 `ccSwitchProviderId` 条目一律不动。
use crate::error::AppError;
use serde_json::Value;
use std::path::Path;

/// 原版 CC Switch 的归属字段；带这个字段的条目属于原版，不能被智枢认领。
const UPSTREAM_OWNER_FIELD: &str = "ccSwitchProviderId";

/// 一个可以认领旧条目的平台供应商：供应商 id 和它的端点、Key。
#[derive(Debug, Clone)]
pub(crate) struct LegacyClaim {
    pub id: String,
    pub url: String,
    pub api_key: String,
}

impl LegacyClaim {
    /// 从供应商配置（新旧格式都把 `url` / `apiKey` 放在顶层）取认领条件；缺任一项时不认领。
    pub(crate) fn from_settings(id: &str, settings: &Value) -> Option<Self> {
        let text = |field: &str| {
            settings
                .get(field)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        };
        Some(Self {
            id: id.to_string(),
            url: text("url")?,
            api_key: text("apiKey")?,
        })
    }
}

/// 旧适配器写 `<根>/v1`，上游格式写 `<根>/v1/chat/completions`；比较时统一去掉后缀。
fn endpoint_key(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    url.strip_suffix("/chat/completions")
        .unwrap_or(url)
        .trim_end_matches('/')
        .to_string()
}

fn models_mut(document: &mut Value) -> Option<&mut Vec<Value>> {
    match document {
        Value::Array(models) => Some(models),
        Value::Object(map) => map.get_mut("models").and_then(Value::as_array_mut),
        _ => None,
    }
}

/// 在内存文档中认领旧条目，返回认领数量。只改条目的归属标记，不增删条目、不改其他字段。
pub(crate) fn claim_in_document(document: &mut Value, claims: &[LegacyClaim]) -> usize {
    let owner_field = crate::workbuddy_config::OWNER_FIELD;
    let Some(models) = models_mut(document) else {
        return 0;
    };
    let mut claimed = 0;
    for entry in models.iter_mut() {
        let Some(object) = entry.as_object_mut() else {
            continue;
        };
        if object.contains_key(owner_field) || object.contains_key(UPSTREAM_OWNER_FIELD) {
            continue;
        }
        let (Some(url), Some(key)) = (
            object.get("url").and_then(Value::as_str),
            object.get("apiKey").and_then(Value::as_str),
        ) else {
            continue;
        };
        let url = endpoint_key(url);
        let key = key.trim();
        if url.is_empty() || key.is_empty() {
            continue;
        }
        if let Some(claim) = claims
            .iter()
            .find(|claim| claim.api_key == key && endpoint_key(&claim.url) == url)
        {
            object.insert(owner_field.into(), Value::String(claim.id.clone()));
            claimed += 1;
        }
    }
    claimed
}

fn read_document(path: &Path) -> Result<Option<Value>, AppError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AppError::io(path, error)),
    };
    let text = text.trim_start_matches('\u{feff}').trim();
    if text.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(text)
        .map(Some)
        .map_err(|error| AppError::json(path, error))
}

/// 认领 `models.json` 中的旧平台条目；没有可认领的条目时不写文件。文件损坏时返回错误且不改写。
pub(crate) fn claim_legacy_platform_entries(claims: &[LegacyClaim]) -> Result<usize, AppError> {
    if claims.is_empty() {
        return Ok(0);
    }
    let path = crate::workbuddy_config::config_path();
    let Some(mut document) = read_document(&path)? else {
        return Ok(0);
    };
    if !matches!(document, Value::Array(_) | Value::Object(_)) {
        return Err(AppError::Config(
            "WorkBuddy models.json must be a JSON object".into(),
        ));
    }
    let claimed = claim_in_document(&mut document, claims);
    if claimed > 0 {
        let json = serde_json::to_string_pretty(&document)
            .map_err(|source| AppError::JsonSerialize { source })?;
        crate::config::atomic_write_private(&path, format!("{json}\n").as_bytes())?;
        log::info!("Claimed {claimed} legacy Zhishu WorkBuddy model entries");
    }
    Ok(claimed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn claim() -> LegacyClaim {
        LegacyClaim {
            id: "platform-row".into(),
            url: "https://platform.test/v1/chat/completions".into(),
            api_key: "key-42".into(),
        }
    }

    fn owner(entry: &Value) -> Option<&str> {
        entry
            .get(crate::workbuddy_config::OWNER_FIELD)
            .and_then(Value::as_str)
    }

    #[test]
    fn claims_untagged_legacy_platform_entries_by_endpoint_and_key() {
        let mut document = json!({"models": [
            {"id": "model-a", "url": "https://platform.test/v1", "apiKey": "key-42"},
            {"id": "model-b", "url": "https://platform.test/v1/", "apiKey": "key-42"}
        ], "availableModels": ["model-a"]});
        assert_eq!(claim_in_document(&mut document, &[claim()]), 2);
        for entry in document["models"].as_array().unwrap() {
            assert_eq!(owner(entry), Some("platform-row"));
        }
        assert_eq!(document["availableModels"], json!(["model-a"]));
    }

    #[test]
    fn leaves_user_and_upstream_cc_switch_entries_untouched() {
        let mut document = json!([
            {"id": "mine", "url": "https://platform.test/v1", "apiKey": "user-key"},
            {"id": "other", "url": "https://other.test/v1", "apiKey": "key-42"},
            {"id": "upstream", "url": "https://platform.test/v1", "apiKey": "key-42",
             "ccSwitchProviderId": "cc-card"},
            {"id": "no-key", "url": "https://platform.test/v1"}
        ]);
        let before = document.clone();
        assert_eq!(claim_in_document(&mut document, &[claim()]), 0);
        assert_eq!(document, before);
    }

    #[test]
    fn does_not_reassign_entries_already_owned_by_zhishu() {
        let mut document = json!({"models": [
            {"id": "model-a", "url": "https://platform.test/v1/chat/completions",
             "apiKey": "key-42", "zhishuCcsProviderId": "someone-else"}
        ]});
        assert_eq!(claim_in_document(&mut document, &[claim()]), 0);
        assert_eq!(owner(&document["models"][0]), Some("someone-else"));
    }

    #[test]
    fn claims_duplicate_model_ids_without_adding_entries() {
        let mut document = json!({"models": [
            {"id": "model-a", "url": "https://platform.test/v1", "apiKey": "key-42"},
            {"id": "model-a", "url": "https://platform.test/v1", "apiKey": "key-42"},
            {"id": "model-a", "url": "https://platform.test/v1", "apiKey": "user-key"}
        ]});
        assert_eq!(claim_in_document(&mut document, &[claim()]), 2);
        let models = document["models"].as_array().unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(owner(&models[2]), None);
    }

    #[test]
    fn settings_without_endpoint_or_key_never_claim() {
        assert!(LegacyClaim::from_settings("x", &json!({"url": "https://a.test"})).is_none());
        assert!(LegacyClaim::from_settings("x", &json!({"apiKey": "k"})).is_none());
        let claim = LegacyClaim::from_settings(
            "x",
            &json!({"url": " https://a.test/v1 ", "apiKey": " k "}),
        )
        .unwrap();
        assert_eq!(
            (claim.url.as_str(), claim.api_key.as_str()),
            ("https://a.test/v1", "k")
        );
    }
}
