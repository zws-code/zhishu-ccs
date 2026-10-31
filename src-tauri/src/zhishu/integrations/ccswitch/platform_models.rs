//! 将平台模型与分组凭据适配为各工具的供应商数据，保留稳定 ID 和平台托管来源标记。
//! 本模块只构造配置；原生文件发布和失败恢复由各工具的 ProviderService 路径负责。
use crate::app_config::AppType;
use crate::provider::{
    ClaudeDesktopMode, ClaudeDesktopModelRoute, OpenCodeConfigFormat, PlatformManagedProvider,
    Provider, ProviderMeta,
};
use crate::zhishu::services::platform_models::{fail, AgentModel, PlatformAuthError};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

// 与前端 src/config/platformConfig.ts 的 PLATFORM_SYNC_APPS 保持一致。
pub(crate) const AGENTS: &[&str] = &[
    "claude",
    "claude-desktop",
    "codex",
    "gemini",
    "grokbuild",
    "opencode",
    "openclaw",
    "hermes",
    "pi",
    "deepseek-harness",
    "workbuddy",
    "mcode",
    "kimi",
];

pub(crate) fn app(agent: &str) -> Result<AppType, PlatformAuthError> {
    if !AGENTS.contains(&agent) {
        return Err(fail(
            "UNSUPPORTED_AGENT",
            "这个工具还未完成平台模型适配，请在供应商页面手动配置",
        ));
    }
    AppType::from_str(agent).map_err(|_| fail("UNSUPPORTED_AGENT", "暂不支持这个工具"))
}

pub(crate) fn needs_routing(m: &AgentModel) -> bool {
    match m.agent.as_str() {
        "claude" => m.protocol != "anthropic",
        "claude-desktop" => {
            m.protocol != "anthropic"
                || !crate::claude_desktop_config::is_claude_safe_model_id(&m.model)
        }
        "codex" | "grokbuild" => m.protocol != "responses",
        _ => false,
    }
}

fn desktop_route_id(model: &AgentModel, used: &mut HashSet<String>) -> String {
    if crate::claude_desktop_config::is_claude_safe_model_id(&model.model)
        && used.insert(model.model.clone())
    {
        return model.model.clone();
    }

    let normalized = model.model.to_ascii_lowercase();
    let base = if normalized.contains("opus") {
        "claude-opus-5"
    } else if normalized.contains("haiku") {
        "claude-haiku-4-5"
    } else if normalized.contains("fable") {
        "claude-fable-5"
    } else {
        "claude-sonnet-5"
    };
    if used.insert(base.to_string()) {
        return base.to_string();
    }

    let mut index = 2usize;
    loop {
        let candidate = format!("{base}-r{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

pub(crate) fn desktop_routes<'a>(
    models: impl IntoIterator<Item = &'a AgentModel>,
) -> HashMap<String, ClaudeDesktopModelRoute> {
    let mut used = HashSet::new();
    let mut routes = HashMap::new();
    for model in models {
        let route_id = desktop_route_id(model, &mut used);
        routes.insert(
            route_id.clone(),
            ClaudeDesktopModelRoute {
                model: if model.model == route_id {
                    route_id
                } else {
                    model.model.clone()
                },
                label_override: (!model.name.trim().is_empty()).then(|| model.name.clone()),
                supports_1m: (model.context_window >= 1_000_000).then_some(true),
            },
        );
    }
    routes
}

fn desktop_mode(m: &AgentModel) -> ClaudeDesktopMode {
    if needs_routing(m) {
        ClaudeDesktopMode::Proxy
    } else {
        ClaudeDesktopMode::Direct
    }
}

fn deepseek_harness_credential_ref(id: &str) -> String {
    let mut reference = String::with_capacity(id.len() + 9);
    reference.push_str("PLATFORM_");
    for character in id.chars() {
        if character.is_ascii_alphanumeric() {
            reference.push(character.to_ascii_uppercase());
        } else {
            reference.push('_');
        }
    }
    reference.push_str("_API_KEY");
    reference
}

pub(crate) fn service_provider_id(
    agent: &str,
    group_id: i64,
    platform: &str,
    origin: &str,
    user_id: i64,
) -> String {
    let digest = Sha256::digest(format!("{origin}\0{user_id}").as_bytes());
    let scope = format!("{digest:x}")[..12].to_string();
    format!("platform-managed-{agent}-{scope}-{platform}-{group_id}")
}

pub(crate) fn provider(
    m: &AgentModel,
    origin: &str,
    user: i64,
    key: &str,
    key_id: i64,
    previous: Option<&Provider>,
) -> Result<Provider, PlatformAuthError> {
    build_provider(
        m,
        origin,
        user,
        key,
        key_id,
        previous,
        format!("platform-managed-{}", m.agent),
        "智枢平台托管".into(),
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn service_provider(
    m: &AgentModel,
    origin: &str,
    user: i64,
    key: &str,
    key_id: i64,
    previous: Option<&Provider>,
    group_id: i64,
    group_platform: &str,
    group_name: &str,
    key_name: &str,
) -> Result<Provider, PlatformAuthError> {
    build_provider(
        m,
        origin,
        user,
        key,
        key_id,
        previous,
        service_provider_id(&m.agent, group_id, group_platform, origin, user),
        format!("智枢平台 · {group_name}"),
        Some((group_id, group_platform, group_name, key_name)),
        None,
    )
}

/// Keep the account identity separate from the service-configured model gateway.
#[allow(clippy::too_many_arguments)]
pub(crate) fn service_provider_with_connection(
    m: &AgentModel,
    origin: &str,
    user: i64,
    key: &str,
    key_id: i64,
    previous: Option<&Provider>,
    group_id: i64,
    group_platform: &str,
    group_name: &str,
    key_name: &str,
    connection: Option<&crate::zhishu::services::client_api::ClientConnection>,
) -> Result<Provider, PlatformAuthError> {
    let Some(connection) = connection else {
        return service_provider(
            m,
            origin,
            user,
            key,
            key_id,
            previous,
            group_id,
            group_platform,
            group_name,
            key_name,
        );
    };
    let (endpoint, suffix) = match m.protocol.as_str() {
        "anthropic" => ("messages", "/v1/messages"),
        "responses" => ("responses", "/v1/responses"),
        "chat" => ("chat_completions", "/v1/chat/completions"),
        "gemini" => ("gemini", "/v1beta/models"),
        _ => return Err(fail("INCOMPATIBLE", "平台尚未适配这个模型的协议")),
    };
    let path = connection
        .endpoints
        .get(endpoint)
        .ok_or_else(|| fail("INCOMPATIBLE", "平台未提供该协议的连接路径"))?;
    let prefix = path
        .strip_suffix(suffix)
        .filter(|prefix| {
            prefix.is_empty()
                || (prefix.starts_with('/')
                    && !prefix.starts_with("//")
                    && !prefix.contains(['?', '#']))
        })
        .ok_or_else(|| fail("INCOMPATIBLE", "工具暂不支持平台配置的协议路径"))?;
    let root = format!("{}{}", connection.base_url.trim_end_matches('/'), prefix);
    build_provider(
        m,
        origin,
        user,
        key,
        key_id,
        previous,
        service_provider_id(&m.agent, group_id, group_platform, origin, user),
        format!("智枢平台 · {group_name}"),
        Some((group_id, group_platform, group_name, key_name)),
        Some(&root),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_provider(
    m: &AgentModel,
    origin: &str,
    user: i64,
    key: &str,
    key_id: i64,
    previous: Option<&Provider>,
    id: String,
    name: String,
    group: Option<(i64, &str, &str, &str)>,
    connection_root: Option<&str>,
) -> Result<Provider, PlatformAuthError> {
    app(&m.agent)?;
    let protocol_api = match m.protocol.as_str() {
        "responses" => "openai-responses",
        "anthropic" => "anthropic-messages",
        "chat" => "openai-completions",
        "gemini" => "google-generative-ai",
        _ => return Err(fail("INCOMPATIBLE", "平台尚未适配这个模型的协议")),
    };
    if (m.agent == "gemini" && m.protocol != "gemini")
        || (m.protocol == "gemini"
            && !["gemini", "openclaw", "pi", "claude-desktop", "kimi"].contains(&m.agent.as_str()))
    {
        return Err(fail("INCOMPATIBLE", "这个模型与所选工具不兼容"));
    }
    if m.agent == "workbuddy" && m.protocol != "chat" {
        return Err(fail(
            "INCOMPATIBLE",
            "WorkBuddy 仅支持 OpenAI Chat Completions 模型",
        ));
    }
    let root = connection_root.unwrap_or(origin).trim_end_matches('/');
    let base = format!("{root}/v1");
    // Native Anthropic clients append /v1/messages. OpenCode's AI SDK
    // expects /v1 in its prefix, so its options continue using `base`.
    let native_base = if m.protocol == "anthropic" {
        root
    } else {
        &base
    };
    let model = &m.model;
    // These native schemas need concrete input/reasoning defaults. The defaults
    // below are local CLI policy; neither AgentModel nor the catalog is modified.
    let input = if m.input_modalities.is_empty() {
        vec!["text".to_string()]
    } else {
        m.input_modalities.clone()
    };
    let mut settings = match m.agent.as_str() {
        "claude" => json!({"env": {
            "ANTHROPIC_BASE_URL": if m.protocol == "anthropic" { root } else { &base },
            "ANTHROPIC_AUTH_TOKEN": key, "ANTHROPIC_MODEL": model,
            "ANTHROPIC_DEFAULT_SONNET_MODEL": model, "ANTHROPIC_DEFAULT_OPUS_MODEL": model,
            "ANTHROPIC_DEFAULT_HAIKU_MODEL": model, "CLAUDE_CODE_SUBAGENT_MODEL":model,
            "ANTHROPIC_SMALL_FAST_MODEL":model
        }}),
        "claude-desktop" => json!({"env": {
            "ANTHROPIC_BASE_URL": if m.protocol == "anthropic" { root } else { &base },
            "ANTHROPIC_AUTH_TOKEN": key,
            "ANTHROPIC_MODEL": model
        }}),
        "codex" => {
            // Use TOML serialization, never interpolate model IDs into TOML code.
            let config = json!({"model_provider":"platform", "model":model,
                "model_providers":{"platform":{"name":"智枢平台", "base_url":base, "wire_api":"responses", "requires_openai_auth":true}}});
            let config =
                toml::to_string(&config).map_err(|_| fail("CONFIG", "无法生成工具配置"))?;
            let mut entry = json!({"model":model,"displayName":m.name});
            if !m.input_modalities.is_empty() {
                entry["inputModalities"] = json!(m.input_modalities);
            }
            if m.context_window > 0 {
                entry["contextWindow"] = json!(m.context_window);
            }
            if !m.reasoning_levels.is_empty() {
                entry["reasoningLevels"] = json!(m.reasoning_levels);
            }
            if let Some(level) = &m.default_reasoning_level {
                entry["defaultReasoningLevel"] = json!(level);
            }
            if let Some(parallel) = m.supports_parallel_tool_calls {
                entry["supportsParallelToolCalls"] = json!(parallel);
            }
            json!({"auth":{"OPENAI_API_KEY":key}, "config":config,
                "modelCatalog":{"models":[entry]}})
        }
        "grokbuild" => {
            // GrokBuild requires a local context budget even when the platform is silent.
            let context_window = if m.context_window == 0 {
                crate::grok_config::DEFAULT_CONTEXT_WINDOW as u64
            } else {
                m.context_window
            };
            let config = json!({"models":{"default":"platform"},"model":{"platform":{"name":m.name,"model":model,"base_url":base,"api_key":key,"api_backend":"responses","context_window":context_window}}});
            json!({"config":toml::to_string(&config).map_err(|_| fail("CONFIG", "无法生成工具配置"))?})
        }
        "gemini" => {
            json!({"env":{"GOOGLE_GEMINI_BASE_URL":root, "GEMINI_API_KEY":key, "GEMINI_MODEL":model}, "config":{"model":{"name":model}}})
        }
        "opencode" => {
            json!({"npm":match m.protocol.as_str() {"anthropic" => "@ai-sdk/anthropic", "responses" => "@ai-sdk/openai", _ => "@ai-sdk/openai-compatible"},
            "name":"智枢平台", "options":{"baseURL":base,"apiKey":key}, "models":{(model):{"name":m.name}}})
        }
        "openclaw" | "pi" => {
            let mut entry = json!({
                "id": model,
                "name": m.name,
                "input": input,
                "reasoning": m.reasoning.unwrap_or(false)
            });
            if m.context_window > 0 {
                entry["contextWindow"] = json!(m.context_window);
            }
            if m.max_output_tokens > 0 {
                entry["maxTokens"] = json!(m.max_output_tokens);
            }
            json!({"name":"智枢平台", "baseUrl": if m.protocol == "gemini" { root } else { native_base }, "apiKey":key, "api":protocol_api,
                "models":[entry]})
        }
        "hermes" => json!({"name":id,"base_url":native_base,"api_key":key,
            "api_mode":match m.protocol.as_str() {"anthropic"=>"anthropic_messages", "responses"=>"codex_responses", _=>"chat_completions"},
            "models":[{"id":model,"name":m.name}]}),
        "deepseek-harness" => {
            let api = match m.protocol.as_str() {
                "anthropic" => "anthropic-messages",
                "responses" => "openai-responses",
                "chat" => "openai-completions",
                _ => return Err(fail("INCOMPATIBLE", "DeepSeek Harness 不支持这个模型协议")),
            };
            let mut model_entry = json!({"id":model,"name":m.name});
            if m.context_window > 0 {
                model_entry["contextWindow"] = json!(m.context_window);
            }
            json!({
                "displayName":name,
                "api":api,
                "baseURL":native_base,
                "apiKeyEnv":deepseek_harness_credential_ref(&id),
                "apiKey":key,
                "models":[model_entry]
            })
        }
        "workbuddy" => json!({
            "url": base,
            "apiKey": key,
            "vendor": "智枢平台",
            "models": [workbuddy_model_entry(m, &base, key)?]
        }),
        "mcode" => json!({"name":"智枢平台", "kind":"custom", "enabled":true,"api":protocol_api,
            "options":{"baseURL":native_base,"apiKey":key}, "models":{(model):{"name":m.name}}}),
        "kimi" => {
            let provider_type = match m.protocol.as_str() {
                "anthropic" => "anthropic",
                "responses" => "openai_responses",
                "chat" => "openai",
                "gemini" => "google-genai",
                _ => return Err(fail("INCOMPATIBLE", "Kimi Code 不支持这个模型协议")),
            };
            let kimi_base = if matches!(m.protocol.as_str(), "anthropic" | "gemini") {
                root
            } else {
                &base
            };
            let supports_reasoning = m.reasoning.unwrap_or(false) || !m.reasoning_levels.is_empty();
            let mut capabilities = vec!["tool_use"];
            if m.input_modalities.iter().any(|item| item == "image") {
                capabilities.push("image_in");
            }
            if supports_reasoning {
                capabilities.push("thinking");
            }
            let model_key = format!("{id}/{model}");
            // Kimi requires a concrete context size; this is only a native fallback.
            let context = if m.context_window > 0 {
                m.context_window
            } else {
                32_768
            };
            let mut entry = json!({
                "provider": id,
                "model": model,
                "display_name": m.name,
                "max_context_size": context,
                "capabilities": capabilities
            });
            if m.protocol == "anthropic" && m.max_output_tokens > 0 {
                entry["max_output_size"] = json!(m.max_output_tokens);
            }
            json!({
                "provider": {
                    "type": provider_type,
                    "base_url": kimi_base,
                    "api_key": key
                },
                "models": {(model_key): entry}
            })
        }
        _ => return Err(fail("UNSUPPORTED_AGENT", "暂不支持这个工具")),
    };
    if matches!(m.agent.as_str(), "opencode" | "mcode") {
        let entry = &mut settings["models"][model];
        if m.context_window > 0 {
            entry["limit"] = json!({"context":m.context_window});
        }
        if m.max_output_tokens > 0 {
            if !entry["limit"].is_object() {
                entry["limit"] = json!({});
            }
            entry["limit"]["output"] = json!(m.max_output_tokens);
        }
        if let Some(reasoning) = m.reasoning {
            entry["reasoning"] = json!(reasoning);
        }
        if !m.input_modalities.is_empty() {
            entry["modalities"] = json!({"input":m.input_modalities});
        }
        if !m.reasoning_levels.is_empty() && m.protocol != "gemini" {
            let effort_field = if m.protocol == "anthropic" {
                "effort"
            } else {
                "reasoningEffort"
            };
            let variants: serde_json::Map<String, serde_json::Value> = m
                .reasoning_levels
                .iter()
                .map(|level| (level.clone(), json!({(effort_field):level})))
                .collect();
            entry["variants"] = json!(variants);
        }
    }
    if m.agent == "opencode" {
        upgrade_opencode_settings(&mut settings);
    }
    if m.agent == "hermes" && m.context_window > 0 {
        settings["models"][0]["context_length"] = json!(m.context_window);
    }
    // Pi/MCode own model selection and refuse removing their active model. Keep
    // previous declarations while selecting the new default through the adapter.
    if let Some(old) = previous {
        if matches!(m.agent.as_str(), "mcode" | "kimi") {
            if let (Some(new), Some(old)) = (
                settings["models"].as_object_mut(),
                old.settings_config["models"].as_object(),
            ) {
                for (id, value) in old {
                    new.entry(id.clone()).or_insert_with(|| value.clone());
                }
            }
        } else if m.agent == "pi" {
            if let (Some(new), Some(old)) = (
                settings["models"].as_array_mut(),
                old.settings_config["models"].as_array(),
            ) {
                for value in old {
                    if value["id"].as_str() != Some(model) {
                        new.push(value.clone());
                    }
                }
            }
        }
    }
    let mut p = Provider::with_id(id, name, settings, Some(origin.into()));
    p.category = Some("aggregator".into());
    let desktop_meta = (m.agent == "claude-desktop")
        .then(|| (desktop_mode(m), desktop_routes(std::iter::once(m))));
    let previous_dsh_model = previous
        .and_then(|provider| provider.meta.as_ref())
        .and_then(|meta| meta.dsh_current_model.clone())
        .filter(|previous| previous == model.as_str());
    p.meta = Some(ProviderMeta {
        platform_managed: Some(PlatformManagedProvider {
            origin: origin.into(),
            user_id: user,
            entry_id: m.id.clone(),
            model: model.clone(),
            key_id,
            group_id: group.map(|(id, _, _, _)| id),
            group_platform: group.map(|(_, platform, _, _)| platform.to_string()),
            group_name: group.map(|(_, _, name, _)| name.to_string()),
            key_name: group.map(|(_, _, _, key_name)| key_name.to_string()),
            subscription_type: None,
            connection_base_url: connection_root.map(str::to_owned),
        }),
        api_format: Some(
            match m.protocol.as_str() {
                "anthropic" => "anthropic",
                "responses" => "openai_responses",
                "chat" => "openai_chat",
                _ => "gemini_native",
            }
            .into(),
        ),
        opencode_config_format: (m.agent == "opencode").then_some(OpenCodeConfigFormat::V2),
        common_config_enabled: Some(false),
        provider_type: (m.agent == "deepseek-harness").then(|| "dsh_pi_ai".into()),
        dsh_current_model: (m.agent == "deepseek-harness")
            .then(|| previous_dsh_model.unwrap_or_else(|| model.clone())),
        claude_desktop_mode: desktop_meta.as_ref().map(|(mode, _)| mode.clone()),
        claude_desktop_model_routes: desktop_meta.map(|(_, routes)| routes).unwrap_or_default(),
        ..Default::default()
    });
    p.created_at = previous
        .and_then(|p| p.created_at)
        .or(Some(chrono::Utc::now().timestamp_millis()));
    Ok(p)
}

/// Upgrade only platform-owned legacy declarations. Preserve custom options and
/// model metadata while translating fields whose V2 representation changed.
pub(crate) fn upgrade_opencode_settings(settings: &mut Value) -> bool {
    let previous = settings.clone();
    let Some(object) = settings.as_object_mut() else {
        return false;
    };
    if let Some(npm) = object.remove("npm") {
        let package = match npm.as_str() {
            Some("@ai-sdk/anthropic") => "@opencode/ai/providers/anthropic",
            Some("@ai-sdk/openai") => "@opencode/ai/providers/openai",
            Some("@ai-sdk/openai-compatible") => "@opencode/ai/providers/openai-compatible",
            _ => {
                object.insert("npm".into(), npm);
                return false;
            }
        };
        object.entry("package").or_insert(json!(package));
        if let Some(Value::Object(options)) = object.remove("options") {
            let native = object.entry("settings").or_insert(json!({}));
            if let Some(native) = native.as_object_mut() {
                for (key, value) in options {
                    native.entry(key).or_insert(value);
                }
            }
        }
        if let Some(models) = object.get_mut("models").and_then(Value::as_object_mut) {
            for model in models.values_mut().filter_map(Value::as_object_mut) {
                if let Some(modalities) = model.remove("modalities") {
                    model.entry("capabilities").or_insert(json!({
                        "tools": true,
                        "input": modalities.get("input").cloned().unwrap_or(json!(["text"])),
                        "output": modalities.get("output").cloned().unwrap_or(json!(["text"]))
                    }));
                }
                if let Some(Value::Object(variants)) = model.get("variants").cloned() {
                    model.insert(
                        "variants".into(),
                        Value::Array(
                            variants
                                .into_iter()
                                .map(|(id, settings)| json!({"id":id,"settings":settings}))
                                .collect(),
                        ),
                    );
                }
            }
        }
    }
    *settings != previous
}

/// Called only after sync verifies ownership by the current platform account.
/// Repair the exact former generated URL, preserving customized endpoints.
pub(crate) fn repair_legacy_anthropic_base_url(
    settings: &mut Value,
    desired: &Value,
    app: &AppType,
    origin: &str,
) -> bool {
    let (protocol_path, protocol, base_path) = match app {
        AppType::Pi | AppType::OpenClaw => ("/api", "anthropic-messages", "/baseUrl"),
        AppType::Hermes => ("/api_mode", "anthropic_messages", "/base_url"),
        AppType::DeepSeekHarness => ("/api", "anthropic-messages", "/baseURL"),
        AppType::Mcode => ("/api", "anthropic-messages", "/options/baseURL"),
        _ => return false,
    };
    let root = origin.trim_end_matches('/');
    let legacy = format!("{root}/v1");
    if settings.pointer(protocol_path).and_then(Value::as_str) != Some(protocol)
        || desired.pointer(protocol_path).and_then(Value::as_str) != Some(protocol)
        || settings.pointer(base_path).and_then(Value::as_str) != Some(legacy.as_str())
        || desired.pointer(base_path).and_then(Value::as_str) != Some(root)
    {
        return false;
    }
    if let Some(base) = settings.pointer_mut(base_path) {
        *base = json!(root);
        true
    } else {
        false
    }
}

fn api_format_protocol(format: &str) -> Option<&'static str> {
    match format {
        "anthropic" => Some("anthropic"),
        "openai_responses" => Some("responses"),
        "openai_chat" => Some("chat"),
        "gemini_native" => Some("gemini"),
        _ => None,
    }
}

fn settings_protocol(app: &AppType, settings: &Value) -> Option<&'static str> {
    if app == &AppType::Kimi {
        return match settings.pointer("/provider/type").and_then(Value::as_str)? {
            "anthropic" => Some("anthropic"),
            "openai_responses" => Some("responses"),
            "openai" => Some("chat"),
            "google-genai" => Some("gemini"),
            _ => None,
        };
    }
    let (path, anthropic, responses, chat) = match app {
        AppType::Pi | AppType::OpenClaw | AppType::DeepSeekHarness | AppType::Mcode => (
            "/api",
            "anthropic-messages",
            "openai-responses",
            "openai-completions",
        ),
        AppType::Hermes => (
            "/api_mode",
            "anthropic_messages",
            "codex_responses",
            "chat_completions",
        ),
        _ => return None,
    };
    match settings.pointer(path).and_then(Value::as_str)? {
        value if value == anthropic => Some("anthropic"),
        value if value == responses => Some("responses"),
        value if value == chat => Some("chat"),
        "google-generative-ai" => Some("gemini"),
        _ => None,
    }
}

fn generated_base_url(
    app: &AppType,
    protocol: &str,
    origin: &str,
) -> Option<(&'static str, String)> {
    let root = origin.trim_end_matches('/');
    let base = format!("{root}/v1");
    let (path, url) = match app {
        AppType::Claude | AppType::ClaudeDesktop => (
            "/env/ANTHROPIC_BASE_URL",
            if protocol == "anthropic" { root } else { &base },
        ),
        AppType::Pi | AppType::OpenClaw => (
            "/baseUrl",
            if matches!(protocol, "anthropic" | "gemini") {
                root
            } else {
                &base
            },
        ),
        AppType::Hermes => (
            "/base_url",
            if protocol == "anthropic" { root } else { &base },
        ),
        AppType::DeepSeekHarness => (
            "/baseURL",
            if protocol == "anthropic" { root } else { &base },
        ),
        AppType::Mcode => (
            "/options/baseURL",
            if protocol == "anthropic" { root } else { &base },
        ),
        AppType::Kimi => (
            "/provider/base_url",
            if matches!(protocol, "anthropic" | "gemini") {
                root
            } else {
                &base
            },
        ),
        _ => return None,
    };
    Some((path, url.to_string()))
}

/// Repair an endpoint generated for the previous protocol after synchronized
/// model metadata changes that protocol. Customized endpoints are preserved.
pub(crate) fn repair_protocol_base_url(
    settings: &mut Value,
    app: &AppType,
    origin: &str,
    previous_api_format: Option<&str>,
    next_api_format: Option<&str>,
) -> bool {
    let previous_protocol = previous_api_format
        .and_then(api_format_protocol)
        .or_else(|| settings_protocol(app, settings));
    let next_protocol = next_api_format.and_then(api_format_protocol);
    let (Some(previous_protocol), Some(next_protocol)) = (previous_protocol, next_protocol) else {
        return false;
    };
    if previous_protocol == next_protocol {
        return false;
    }
    let Some((path, previous_url)) = generated_base_url(app, previous_protocol, origin) else {
        return false;
    };
    let Some((_, next_url)) = generated_base_url(app, next_protocol, origin) else {
        return false;
    };
    if previous_url == next_url
        || settings.pointer(path).and_then(Value::as_str) != Some(previous_url.as_str())
    {
        return false;
    }
    let Some(endpoint) = settings.pointer_mut(path) else {
        return false;
    };
    *endpoint = json!(next_url);
    true
}

pub(crate) fn refresh_connection_base(settings: &mut Value, previous: &str, next: &str) -> bool {
    if previous == next {
        return false;
    }
    fn replace(value: &mut Value, previous: &str, next: &str) -> bool {
        match value {
            Value::Object(fields) => {
                let mut changed = false;
                for (key, value) in fields {
                    if key == "config" {
                        if let Some(text) = value.as_str() {
                            if let Ok(mut doc) = text.parse::<toml_edit::DocumentMut>() {
                                let mut edited = false;
                                fn visit(
                                    table: &mut dyn toml_edit::TableLike,
                                    previous: &str,
                                    next: &str,
                                    edited: &mut bool,
                                ) {
                                    for (key, item) in table.iter_mut() {
                                        if key == "base_url" {
                                            if let Some(url) = item.as_str() {
                                                if url == previous
                                                    || url == format!("{previous}/v1")
                                                {
                                                    let replacement =
                                                        format!("{next}{}", &url[previous.len()..]);
                                                    let decor = item
                                                        .as_value()
                                                        .map(|value| value.decor().clone());
                                                    *item = toml_edit::value(replacement);
                                                    if let (Some(decor), Some(value)) =
                                                        (decor, item.as_value_mut())
                                                    {
                                                        *value.decor_mut() = decor;
                                                    }
                                                    *edited = true;
                                                }
                                            }
                                        } else if let Some(child) = item.as_table_like_mut() {
                                            visit(child, previous, next, edited);
                                        }
                                    }
                                }
                                visit(doc.as_table_mut(), previous, next, &mut edited);
                                if edited {
                                    *value = Value::String(doc.to_string());
                                    changed = true;
                                }
                                continue;
                            }
                        }
                    }
                    if matches!(
                        key.as_str(),
                        "base_url"
                            | "baseUrl"
                            | "baseURL"
                            | "url"
                            | "ANTHROPIC_BASE_URL"
                            | "GOOGLE_GEMINI_BASE_URL"
                    ) {
                        if let Some(url) = value.as_str() {
                            if url == previous || url == format!("{previous}/v1") {
                                *value = json!(format!("{next}{}", &url[previous.len()..]));
                                changed = true;
                            }
                        }
                    } else {
                        changed |= replace(value, previous, next);
                    }
                }
                changed
            }
            Value::Array(items) => {
                let mut changed = false;
                for item in items {
                    changed |= replace(item, previous, next);
                }
                changed
            }
            _ => false,
        }
    }
    replace(
        settings,
        previous.trim_end_matches('/'),
        next.trim_end_matches('/'),
    )
}

fn workbuddy_model_entry(m: &AgentModel, url: &str, key: &str) -> Result<Value, PlatformAuthError> {
    // WorkBuddy requires concrete booleans. Unknown tool support is a local
    // optimistic adapter policy; AgentModel and the platform snapshot retain None.
    let supports_reasoning = m.reasoning.unwrap_or(false) || !m.reasoning_levels.is_empty();
    let mut entry = json!({
        "id": m.model,
        "name": m.name,
        "url": url,
        "apiKey": key,
        "supportsToolCall": m.supports_tools.unwrap_or(true),
        "supportsImages": m.input_modalities.iter().any(|item| item == "image"),
        "supportsReasoning": supports_reasoning,
        "useCustomProtocol": false
    });
    if m.context_window > 0 {
        entry["maxInputTokens"] = json!(m.context_window);
    }
    if m.max_output_tokens > 0 {
        entry["maxOutputTokens"] = json!(m.max_output_tokens);
    }
    if supports_reasoning && !m.reasoning_levels.is_empty() {
        let mut reasoning = serde_json::Map::new();
        if let Some(level) = &m.default_reasoning_level {
            reasoning.insert("defaultEffort".into(), json!(level));
        }
        reasoning.insert("supportedEfforts".into(), json!(m.reasoning_levels));
        entry["reasoning"] = Value::Object(reasoning);
    }
    Ok(entry)
}

pub(crate) fn publish_workbuddy_models<'a>(
    provider: &mut Provider,
    models: impl IntoIterator<Item = &'a AgentModel>,
) -> Result<(), PlatformAuthError> {
    let url = provider
        .settings_config
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let key = provider
        .settings_config
        .get("apiKey")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut entries = Vec::new();
    for model in models {
        entries.push(workbuddy_model_entry(model, &url, &key)?);
    }
    provider.settings_config["models"] = Value::Array(entries);
    Ok(())
}

// Helpers for exercising native selection in provider-sync and logout tests.
#[cfg(test)]
pub(crate) fn select_default(
    app: &AppType,
    id: &str,
    model: &str,
) -> Result<(), crate::error::AppError> {
    match app {
        AppType::OpenCode => crate::opencode_config::set_platform_default(id, model),
        AppType::OpenClaw => {
            let reference = format!("{id}/{model}");
            let mut catalog = crate::openclaw_config::get_model_catalog()?.unwrap_or_default();
            catalog.entry(reference.clone()).or_insert(
                crate::openclaw_config::OpenClawModelCatalogEntry {
                    alias: None,
                    extra: Default::default(),
                },
            );
            crate::openclaw_config::set_model_catalog(&catalog)?;
            crate::openclaw_config::set_default_model(
                &crate::openclaw_config::OpenClawDefaultModel {
                    primary: reference,
                    fallbacks: Vec::new(),
                    extra: Default::default(),
                },
            )
            .map(|_| ())
        }
        AppType::Pi => crate::pi_config::set_platform_default(id, model),
        AppType::Mcode => crate::mcode_config::set_platform_default(id, model),
        AppType::Kimi => crate::kimi_config::set_platform_default(id, model),
        AppType::Hermes => {
            let mut config = crate::hermes_config::get_model_config()?.unwrap_or_default();
            config.default = Some(model.into());
            config.provider = Some(id.into());
            // Old model-level endpoint/limits must not override the new custom provider.
            config.base_url = None;
            config.context_length = None;
            config.max_tokens = None;
            crate::hermes_config::set_model_config(&config).map(|_| ())
        }
        AppType::DeepSeekHarness => crate::deepseek_harness_config::set_current_model(id, model),
        AppType::WorkBuddy => Ok(()),
        _ => Ok(()),
    }
}

#[cfg(test)]
pub(crate) fn is_selected(
    state: &crate::store::AppState,
    app: &AppType,
    id: &str,
    model: &str,
) -> Result<bool, crate::error::AppError> {
    Ok(match app {
        AppType::OpenCode => {
            crate::opencode_config::get_default_model()?.as_deref()
                == Some(format!("{id}/{model}").as_str())
        }
        AppType::OpenClaw => crate::openclaw_config::get_default_model()?
            .is_some_and(|m| m.primary == format!("{id}/{model}")),
        AppType::Pi => {
            let selected = crate::pi_config::read_pi_native_defaults()?;
            selected.default_provider.as_deref() == Some(id)
                && selected.default_model.as_deref() == Some(model)
        }
        AppType::Hermes => crate::hermes_config::get_model_config()?.is_some_and(|m| {
            m.default.as_deref() == Some(model) && m.provider.as_deref() == Some(id)
        }),
        AppType::Mcode => crate::mcode_config::platform_default_matches(id, model)?,
        AppType::Kimi => crate::kimi_config::platform_default_matches(id, model)?,
        AppType::DeepSeekHarness => {
            let native = crate::deepseek_harness_config::read_native_state()?;
            native.current_provider.as_deref() == Some(id)
                && native.current_model.as_deref() == Some(model)
        }
        AppType::WorkBuddy => state
            .db
            .get_provider_by_id(id, app.as_str())?
            .and_then(|provider| {
                crate::workbuddy_config::provider_is_live(&provider.settings_config).ok()
            })
            .unwrap_or(false),
        _ => crate::services::ProviderService::current(state, app.clone())? == id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(agent: &str, protocol: &str) -> AgentModel {
        serde_json::from_value(json!({"id":"entry", "agent":agent,"model":"model/with-\"quotes", "name":"Example", "protocol":protocol,"available":true,"category":"recommended","supportsTools":true,"supportsStreaming":true,"contextWindow":32000,"maxOutputTokens":4096})).unwrap()
    }
    #[test]
    fn opencode_platform_providers_use_valid_native_packages_and_model_metadata() {
        for (protocol, package) in [
            ("anthropic", "anthropic"),
            ("responses", "openai"),
            ("chat", "openai-compatible"),
        ] {
            let mut m = model("opencode", protocol);
            m.input_modalities = vec!["text".into(), "image".into()];
            m.reasoning_levels = vec!["low".into(), "high".into()];
            let p = provider(&m, "https://example.test", 1, "key", 2, None).unwrap();
            assert_eq!(p.opencode_config_format(), Some(OpenCodeConfigFormat::V2));
            assert_eq!(
                p.settings_config["package"],
                format!("@opencode/ai/providers/{package}")
            );
            assert!(p.settings_config.get("npm").is_none());
            assert!(p.settings_config.get("options").is_none());
            crate::opencode_config::validate_native_provider(&p.id, &p.settings_config).unwrap();
            assert_eq!(p.resolve_usage_credentials(&AppType::OpenCode).1, "key");
            let row = &p.settings_config["models"][&m.model];
            assert_eq!(row["capabilities"]["tools"], true);
            assert_eq!(row["capabilities"]["input"], json!(["text", "image"]));
            assert_eq!(row["limit"], json!({"context":32000,"output":4096}));
        }
    }

    #[test]
    fn builds_all_registered_agents_and_keeps_one_stable_provider() {
        for agent in AGENTS {
            let protocol = match *agent {
                "claude" => "anthropic",
                "gemini" => "gemini",
                "workbuddy" => "chat",
                _ => "responses",
            };
            let m = model(agent, protocol);
            let p = provider(&m, "https://example.test", 1, "model-key", 2, None).unwrap();
            assert_eq!(p.id, format!("platform-managed-{agent}"));
            if *agent == "workbuddy" {
                assert_eq!(p.settings_config["models"][0]["id"], m.model);
                assert_eq!(p.settings_config["apiKey"], "model-key");
            }
            assert_eq!(p.meta.unwrap().platform_managed.unwrap().model, m.model);
            if *agent == "codex" {
                let config: toml::Value = p.settings_config["config"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(config["model"].as_str(), Some(m.model.as_str()));
            }
            if *agent == "grokbuild" {
                crate::grok_config::validate_config_toml(
                    p.settings_config["config"].as_str().unwrap(),
                )
                .unwrap();
            }
        }
    }
    #[test]
    fn publishes_reported_codex_catalog_fields_for_each_upstream_format() {
        for (protocol, format) in [
            ("responses", "openai_responses"),
            ("chat", "openai_chat"),
            ("anthropic", "anthropic"),
        ] {
            let mut m = model("codex", protocol);
            m.reasoning_levels = vec!["low".into(), "high".into(), "max".into()];
            m.default_reasoning_level = Some("high".into());
            m.input_modalities = vec!["text".into(), "image".into()];
            let p = provider(&m, "https://example.test", 1, "key", 2, None).unwrap();
            assert_eq!(p.meta.as_ref().unwrap().api_format.as_deref(), Some(format));
            let row = &p.settings_config["modelCatalog"]["models"][0];
            assert_eq!(row["contextWindow"], 32000);
            assert_eq!(row["reasoningLevels"], json!(["low", "high", "max"]));
            assert_eq!(row["defaultReasoningLevel"], "high");
            assert_eq!(row["inputModalities"], json!(["text", "image"]));
        }
        let mut unknown = model("codex", "responses");
        unknown.context_window = 0;
        let p = provider(&unknown, "https://example.test", 1, "key", 2, None).unwrap();
        let row = &p.settings_config["modelCatalog"]["models"][0];
        assert!(row.get("contextWindow").is_none());
        assert!(row.get("reasoningLevels").is_none());
    }

    #[test]
    fn maps_every_claude_role_to_the_selected_model() {
        let m = model("claude", "anthropic");
        let p = provider(&m, "https://example.test", 1, "key", 2, None).unwrap();
        for role in [
            "ANTHROPIC_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "CLAUDE_CODE_SUBAGENT_MODEL",
        ] {
            assert_eq!(p.settings_config["env"][role], m.model);
        }
    }

    #[test]
    fn builds_claude_desktop_routing_and_deepseek_harness_native_configs() {
        let mut direct = model("claude-desktop", "anthropic");
        direct.model = "claude-sonnet-5".into();
        direct.name = "Claude Sonnet 5".into();
        let desktop = provider(&direct, "https://example.test", 1, "key", 2, None).unwrap();
        let desktop_meta = desktop.meta.as_ref().unwrap();
        assert_eq!(
            desktop_meta.claude_desktop_mode,
            Some(ClaudeDesktopMode::Direct)
        );
        assert_eq!(
            desktop.settings_config["env"]["ANTHROPIC_AUTH_TOKEN"],
            "key"
        );
        assert_eq!(
            desktop_meta
                .claude_desktop_model_routes
                .get("claude-sonnet-5")
                .map(|route| route.model.as_str()),
            Some("claude-sonnet-5")
        );
        assert!(crate::claude_desktop_config::validate_provider(&desktop).is_ok());

        let mut routed = model("claude-desktop", "responses");
        routed.name = "Platform Model".into();
        let routed = provider(&routed, "https://example.test", 1, "key", 2, None).unwrap();
        let routed_meta = routed.meta.as_ref().unwrap();
        assert_eq!(
            routed_meta.claude_desktop_mode,
            Some(ClaudeDesktopMode::Proxy)
        );
        assert_eq!(
            routed.settings_config["env"]["ANTHROPIC_BASE_URL"],
            "https://example.test/v1"
        );
        assert!(routed_meta
            .claude_desktop_model_routes
            .iter()
            .any(|(route, value)| {
                route.starts_with("claude-")
                    && value.model == "model/with-\"quotes"
                    && value.label_override.as_deref() == Some("Platform Model")
            }));
        assert!(crate::claude_desktop_config::validate_provider(&routed).is_ok());

        let dsh = provider(
            &model("deepseek-harness", "responses"),
            "https://example.test",
            1,
            "key",
            2,
            None,
        )
        .unwrap();
        let dsh_meta = dsh.meta.as_ref().unwrap();
        assert_eq!(dsh_meta.provider_type.as_deref(), Some("dsh_pi_ai"));
        assert_eq!(
            dsh_meta.dsh_current_model.as_deref(),
            Some("model/with-\"quotes")
        );
        assert_eq!(dsh.settings_config["api"], "openai-responses");
        assert_eq!(dsh.settings_config["baseURL"], "https://example.test/v1");
        assert_eq!(
            dsh.settings_config["models"][0]["id"],
            "model/with-\"quotes"
        );
        assert!(dsh.settings_config["apiKeyEnv"]
            .as_str()
            .is_some_and(|value| value
                .bytes()
                .all(|byte| byte == b'_' || byte.is_ascii_uppercase() || byte.is_ascii_digit())
                && !value.as_bytes()[0].is_ascii_digit()));
    }

    #[test]
    fn native_clients_use_protocol_specific_base_urls() {
        for (agent, base_path) in [
            ("pi", "/baseUrl"),
            ("openclaw", "/baseUrl"),
            ("hermes", "/base_url"),
            ("deepseek-harness", "/baseURL"),
            ("mcode", "/options/baseURL"),
        ] {
            for origin in [
                "https://example.test/gateway",
                "https://example.test/gateway/",
            ] {
                for (protocol, expected) in [
                    ("anthropic", "https://example.test/gateway"),
                    ("responses", "https://example.test/gateway/v1"),
                    ("chat", "https://example.test/gateway/v1"),
                ] {
                    let p = provider(&model(agent, protocol), origin, 1, "key", 2, None).unwrap();
                    assert_eq!(
                        p.settings_config.pointer(base_path).unwrap(),
                        expected,
                        "{agent}/{protocol}"
                    );
                }
            }
        }
    }

    #[test]
    fn other_clients_keep_their_native_url_conventions() {
        for (agent, protocol, base_path, expected) in [
            (
                "claude",
                "anthropic",
                "/env/ANTHROPIC_BASE_URL",
                "https://example.test/gateway",
            ),
            (
                "claude-desktop",
                "anthropic",
                "/env/ANTHROPIC_BASE_URL",
                "https://example.test/gateway",
            ),
            (
                "claude",
                "responses",
                "/env/ANTHROPIC_BASE_URL",
                "https://example.test/gateway/v1",
            ),
            (
                "claude-desktop",
                "responses",
                "/env/ANTHROPIC_BASE_URL",
                "https://example.test/gateway/v1",
            ),
            (
                "opencode",
                "anthropic",
                "/settings/baseURL",
                "https://example.test/gateway/v1",
            ),
            (
                "opencode",
                "responses",
                "/settings/baseURL",
                "https://example.test/gateway/v1",
            ),
            (
                "opencode",
                "chat",
                "/settings/baseURL",
                "https://example.test/gateway/v1",
            ),
            (
                "workbuddy",
                "chat",
                "/url",
                "https://example.test/gateway/v1",
            ),
            (
                "gemini",
                "gemini",
                "/env/GOOGLE_GEMINI_BASE_URL",
                "https://example.test/gateway",
            ),
            ("pi", "gemini", "/baseUrl", "https://example.test/gateway"),
            (
                "openclaw",
                "gemini",
                "/baseUrl",
                "https://example.test/gateway",
            ),
        ] {
            let p = provider(
                &model(agent, protocol),
                "https://example.test/gateway",
                1,
                "key",
                2,
                None,
            )
            .unwrap();
            assert_eq!(
                p.settings_config.pointer(base_path).unwrap(),
                expected,
                "{agent}/{protocol}"
            );
        }
        for (agent, base_path) in [
            ("codex", "/model_providers/platform/base_url"),
            ("grokbuild", "/model/platform/base_url"),
        ] {
            for protocol in ["anthropic", "responses", "chat"] {
                let p = provider(
                    &model(agent, protocol),
                    "https://example.test/gateway/",
                    1,
                    "key",
                    2,
                    None,
                )
                .unwrap();
                let config: toml::Value = p.settings_config["config"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap();
                let config = serde_json::to_value(config).unwrap();
                assert_eq!(
                    config.pointer(base_path).unwrap(),
                    "https://example.test/gateway/v1",
                    "{agent}/{protocol}"
                );
            }
        }
    }

    #[test]
    fn endpoint_repair_preserves_non_anthropic_protocols() {
        for (agent, base_path, protocol_path) in [
            ("pi", "/baseUrl", "/api"),
            ("openclaw", "/baseUrl", "/api"),
            ("hermes", "/base_url", "/api_mode"),
            ("deepseek-harness", "/baseURL", "/api"),
            ("mcode", "/options/baseURL", "/api"),
        ] {
            let desired = provider(
                &model(agent, "anthropic"),
                "https://example.test",
                1,
                "key",
                2,
                None,
            )
            .unwrap();
            let mut current = desired.settings_config.clone();
            *current.pointer_mut(base_path).unwrap() = json!("https://example.test/v1");
            *current.pointer_mut(protocol_path).unwrap() = json!("openai-responses");
            let before = current.clone();
            assert!(!repair_legacy_anthropic_base_url(
                &mut current,
                &desired.settings_config,
                &app(agent).unwrap(),
                "https://example.test"
            ));
            assert_eq!(current, before);
        }
    }

    #[test]
    fn protocol_changes_update_only_generated_base_urls() {
        for (agent, base_path) in [
            ("claude", "/env/ANTHROPIC_BASE_URL"),
            ("claude-desktop", "/env/ANTHROPIC_BASE_URL"),
            ("pi", "/baseUrl"),
            ("openclaw", "/baseUrl"),
            ("hermes", "/base_url"),
            ("deepseek-harness", "/baseURL"),
            ("mcode", "/options/baseURL"),
        ] {
            let app = app(agent).unwrap();
            for (from, from_format, to, to_format, expected) in [
                (
                    "responses",
                    "openai_responses",
                    "anthropic",
                    "anthropic",
                    "https://example.test/gateway",
                ),
                (
                    "anthropic",
                    "anthropic",
                    "responses",
                    "openai_responses",
                    "https://example.test/gateway/v1",
                ),
            ] {
                let mut current = provider(
                    &model(agent, from),
                    "https://example.test/gateway",
                    1,
                    "key",
                    2,
                    None,
                )
                .unwrap();
                assert!(repair_protocol_base_url(
                    &mut current.settings_config,
                    &app,
                    "https://example.test/gateway",
                    Some(from_format),
                    Some(to_format),
                ));
                assert_eq!(
                    current.settings_config.pointer(base_path).unwrap(),
                    expected,
                    "{agent}: {from} -> {to}"
                );
            }

            let mut customized = provider(
                &model(agent, "responses"),
                "https://example.test",
                1,
                "key",
                2,
                None,
            )
            .unwrap();
            *customized.settings_config.pointer_mut(base_path).unwrap() =
                json!("https://custom.test/gateway");
            assert!(!repair_protocol_base_url(
                &mut customized.settings_config,
                &app,
                "https://example.test",
                Some("openai_responses"),
                Some("anthropic"),
            ));
            assert_eq!(
                customized.settings_config.pointer(base_path).unwrap(),
                "https://custom.test/gateway",
                "{agent}"
            );
        }
    }

    #[test]
    fn rejects_unknown_agents_and_incompatible_protocols() {
        assert!(provider(
            &model("dsh", "responses"),
            "https://example.test",
            1,
            "key",
            2,
            None
        )
        .is_err());
        assert!(provider(
            &model("gemini", "chat"),
            "https://example.test",
            1,
            "key",
            2,
            None
        )
        .is_err());
    }
    #[test]
    fn service_connection_uses_gateway_without_changing_account_identity() {
        let connection = crate::zhishu::services::client_api::ClientConnection {
            base_url: "https://gateway.test/tenant".into(),
            endpoints: [("responses".into(), "/v1/responses".into())].into(),
        };
        let provider = service_provider_with_connection(
            &model("codex", "responses"),
            "https://login.test",
            1,
            "test-key",
            123,
            None,
            42,
            "openai",
            "Group",
            "shared",
            Some(&connection),
        )
        .unwrap();
        let managed = provider
            .meta
            .as_ref()
            .unwrap()
            .platform_managed
            .as_ref()
            .unwrap();
        assert_eq!(managed.origin, "https://login.test");
        assert_eq!(
            provider.id,
            service_provider_id("codex", 42, "openai", "https://login.test", 1)
        );
        assert_eq!(
            managed.connection_base_url.as_deref(),
            Some("https://gateway.test/tenant")
        );
        let config: toml::Value = provider.settings_config["config"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            config["model_providers"]["platform"]["base_url"].as_str(),
            Some("https://gateway.test/tenant/v1")
        );
    }

    #[test]
    fn changed_gateway_updates_generated_urls_and_preserves_custom_urls_and_toml_comments() {
        let mut settings = json!({"baseUrl":"https://old.test/v1", "nested":{"url":"https://custom.test/v1"},
            "config":"# keep comment\n[model_providers.platform]\nbase_url = \"https://old.test/v1\" # keep inline\n"});
        assert!(refresh_connection_base(
            &mut settings,
            "https://old.test",
            "https://new.test"
        ));
        assert_eq!(settings["baseUrl"], "https://new.test/v1");
        assert_eq!(settings["nested"]["url"], "https://custom.test/v1");
        let text = settings["config"].as_str().unwrap();
        assert!(text.contains("# keep comment"));
        assert!(text.contains("https://new.test/v1"));
        assert!(text.contains("# keep inline"));
    }
}
