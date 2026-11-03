//! Evidence for background writes to CLI files shared with other managers.
//! Only hashes and paths are stored. Explicit user operations remain the handoff.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::live::engine::{digest, read_current, DeviceStore};

#[derive(Default, Serialize, Deserialize)]
struct Ownership {
    files: BTreeMap<PathBuf, Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    route: Option<String>,
}

/// Hash routing/authentication only. Edits to MCP, theme, model choice and other
/// user fields must not prevent safe detach and leave the CLI on a stopped proxy.
fn routing_signature(app: &str) -> Option<String> {
    use serde_json::{json, Value};
    let select = |value: &Value, keys: &[&str]| -> Value {
        Value::Array(
            keys.iter()
                .map(|key| value.get(*key).cloned().unwrap_or(Value::Null))
                .collect(),
        )
    };
    let route = match app {
        "claude" => {
            let value: Value =
                crate::config::read_json_file(&crate::config::get_claude_settings_path()).ok()?;
            select(
                &value["env"],
                &[
                    "ANTHROPIC_BASE_URL",
                    "ANTHROPIC_AUTH_TOKEN",
                    "ANTHROPIC_API_KEY",
                ],
            )
        }
        "codex" => {
            let text =
                std::fs::read_to_string(crate::codex_config::get_codex_config_path()).ok()?;
            let value: toml::Value = text.parse().ok()?;
            let value = serde_json::to_value(value).ok()?;
            let id = value["model_provider"].as_str().unwrap_or("openai");
            let profile = value["profile"].as_str().unwrap_or_default();
            let mut profile_route = value["profiles"].get(profile).cloned();
            if let Some(Value::Object(fields)) = profile_route.as_mut() {
                // Match the profile overrides checked by CodexConfigPatch. Model,
                // reasoning and other user preferences do not transfer ownership.
                fields.retain(|key, _| {
                    matches!(
                        key.as_str(),
                        "model_provider" | "openai_base_url" | "experimental_bearer_token"
                    )
                });
            }
            json!([
                id,
                value.get("openai_base_url"),
                profile,
                profile_route,
                select(
                    &value["model_providers"][id],
                    &[
                        "base_url",
                        "experimental_bearer_token",
                        "env_key",
                        "requires_openai_auth",
                        "wire_api"
                    ]
                )
            ])
        }
        "gemini" => {
            let text = std::fs::read_to_string(crate::gemini_config::get_gemini_env_path()).ok()?;
            let env = crate::gemini_config::parse_env_file_strict(&text).ok()?;
            let settings: Value =
                crate::config::read_json_file(&crate::gemini_config::get_gemini_settings_path())
                    .ok()?;
            json!([
                env.get("GOOGLE_GEMINI_BASE_URL"),
                env.get("GEMINI_API_KEY"),
                settings.pointer("/security/auth/selectedType")
            ])
        }
        "grokbuild" => {
            let text = std::fs::read_to_string(crate::grok_config::get_grok_config_path()).ok()?;
            crate::grok_config::validate_config_toml_syntax(&text).ok()?;
            if crate::grok_config::is_official_live_config(&text) {
                json!("official")
            } else {
                let model = crate::grok_config::extract_model_config(&text)?;
                json!([model.base_url, model.api_key, model.api_backend])
            }
        }
        "claude-desktop" => {
            let paths = crate::claude_desktop_config::live_file_paths().ok()?;
            let read = |index: usize| -> Option<Value> {
                let bytes = read_current(paths.get(index)?).ok()?;
                match bytes {
                    Some(bytes) => serde_json::from_slice(&bytes).ok(),
                    None => Some(Value::Null),
                }
            };
            let normal = read(0)?;
            let third_party = read(1)?;
            let profile = read(2)?;
            let meta = read(3)?;
            json!([
                normal.get("deploymentMode"),
                third_party.get("deploymentMode"),
                meta.get("appliedId"),
                select(
                    &profile,
                    &[
                        "inferenceProvider",
                        "inferenceGatewayBaseUrl",
                        "inferenceGatewayApiKey",
                        "inferenceGatewayAuthScheme"
                    ]
                )
            ])
        }
        _ => return None,
    };
    Some(crate::live::engine::sha256_hex(
        &serde_json::to_vec(&route).ok()?,
    ))
}

#[derive(Serialize, Deserialize)]
struct Deployment {
    source: PathBuf,
    hash: String,
}

fn deployment_receipt(destination: &Path) -> PathBuf {
    crate::config::get_device_data_dir()
        .join("skill-deployments")
        .join(format!(
            "{}.json",
            crate::live::engine::sha256_hex(destination.to_string_lossy().as_bytes())
        ))
}

pub fn remember_skill_copy(
    source: &Path,
    destination: &Path,
    hash: String,
) -> Result<(), AppError> {
    crate::config::write_json_file_private(
        &deployment_receipt(destination),
        &Deployment {
            source: source.to_path_buf(),
            hash,
        },
    )
}

pub fn owns_skill_copy(source: &Path, destination: &Path, hash: &str) -> bool {
    crate::config::read_json_file::<Deployment>(&deployment_receipt(destination))
        .is_ok_and(|receipt| receipt.source == source && receipt.hash == hash)
}

fn receipt(store: &DeviceStore, app: &str) -> PathBuf {
    store.file(&format!(
        "zhishu-live-owner-{}.json",
        crate::live::engine::sha256_hex(app.as_bytes())
    ))
}

pub fn has_receipt(store: &DeviceStore, app: &str) -> bool {
    receipt(store, app).exists()
}

fn tracked(path: &Path) -> bool {
    // Native refresh-token rotation must not revoke configuration ownership.
    path.file_name().and_then(|s| s.to_str()) != Some("auth.json")
}

fn current_paths(app: &str) -> Option<Vec<PathBuf>> {
    Some(match app {
        "claude" => vec![crate::config::get_claude_settings_path()],
        "codex" => vec![
            crate::codex_config::get_codex_config_path(),
            crate::codex_config::get_codex_model_catalog_path(),
        ],
        "gemini" => vec![
            crate::gemini_config::get_gemini_env_path(),
            crate::gemini_config::get_gemini_settings_path(),
        ],
        "grokbuild" => vec![crate::grok_config::get_grok_config_path()],
        "claude-desktop" => crate::claude_desktop_config::live_file_paths().ok()?,
        _ => return None,
    })
}

/// Shared Codex history is never rebucketed automatically on app startup.
pub fn private_codex_home() -> bool {
    let home = crate::codex_config::get_codex_config_dir();
    home.starts_with(crate::config::get_app_config_dir())
        || home.starts_with(crate::config::get_device_data_dir())
}

/// Called only after the existing operation has committed all files and state.
pub fn record(store: &DeviceStore, app: &str, paths: &[PathBuf]) -> Result<(), AppError> {
    let path = receipt(store, app);
    let mut owner: Ownership = if path.exists() {
        // A successful explicit write can establish fresh evidence even when
        // the old receipt was corrupt; corrupt evidence never authorizes startup.
        crate::config::read_json_file(&path).unwrap_or_default()
    } else {
        Ownership::default()
    };
    let scope = current_paths(app);
    let in_scope = |path: &Path| {
        scope
            .as_ref()
            .is_none_or(|scope| scope.iter().any(|current| current == path))
    };
    owner.files.retain(|path, _| in_scope(path));
    for file in paths.iter().filter(|path| tracked(path) && in_scope(path)) {
        owner
            .files
            .insert(file.clone(), digest(read_current(file)?.as_deref()));
    }
    owner.route = routing_signature(app);
    if !owner.files.is_empty() {
        crate::config::write_json_file_private(&path, &owner)?;
    }
    Ok(())
}

/// Unknown, corrupt, moved or externally changed routes are never reclaimed in background.
pub fn unchanged(store: &DeviceStore, app: &str) -> bool {
    let Ok(owner) = crate::config::read_json_file::<Ownership>(&receipt(store, app)) else {
        return false;
    };
    let scope = current_paths(app);
    !owner.files.is_empty()
        && owner
            .files
            .keys()
            .all(|path| scope.as_ref().is_none_or(|scope| scope.contains(path)))
        && match owner.route {
            Some(expected) => routing_signature(app).as_deref() == Some(expected.as_str()),
            None => owner.files.iter().all(|(path, expected)| {
                read_current(path).is_ok_and(|bytes| digest(bytes.as_deref()) == *expected)
            }),
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_changes_and_missing_receipt_revoke_background_ownership() {
        let tmp = tempfile::tempdir().unwrap();
        let store = DeviceStore::at(tmp.path().join("device"));
        let native = tmp.path().join("config.toml");
        std::fs::write(&native, "model = 'ours'").unwrap();
        assert!(!unchanged(&store, "fixture"));
        record(&store, "fixture", &[native.clone()]).unwrap();
        assert!(unchanged(&store, "fixture"));
        std::fs::write(&native, "model = 'external'").unwrap();
        assert!(!unchanged(&store, "fixture"));
        assert_eq!(
            std::fs::read_to_string(&native).unwrap(),
            "model = 'external'"
        );
        std::fs::remove_file(&native).unwrap();
        assert!(!unchanged(&store, "fixture"));
        std::fs::write(receipt(&store, "fixture"), b"{broken").unwrap();
        assert!(!unchanged(&store, "fixture"));
        std::fs::write(&native, "model = 'explicit-handoff'").unwrap();
        record(&store, "fixture", &[native]).unwrap();
        assert!(unchanged(&store, "fixture"));
    }
}
