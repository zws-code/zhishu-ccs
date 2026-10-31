//! 将客户端 /access 授权的模型目录转换为各工具可用的模型，保留未报告的能力和参数。
use super::AgentModel;
use serde_json::Value;
use std::collections::HashSet;

fn protocol(value: &str) -> Option<&'static str> {
    match value {
        "responses" | "openai-responses" | "openai_responses" | "openai_response"
        | "/v1/responses" => Some("responses"),
        "anthropic" | "anthropic-messages" | "anthropic_messages" | "messages" | "/v1/messages" => {
            Some("anthropic")
        }
        "chat"
        | "chat_completions"
        | "chat/completions"
        | "openai"
        | "openai-chat"
        | "openai_completions"
        | "openai_chat"
        | "openai_compatible"
        | "/v1/chat/completions" => Some("chat"),
        "gemini"
        | "google-generative-ai"
        | "google_generative_ai"
        | "generateContent"
        | "generate_content"
        | "gemini-native"
        | "gemini_native"
        | "/v1beta"
        | "/v1beta/models" => Some("gemini"),
        _ => None,
    }
}

pub(crate) fn strings(value: Option<&Value>) -> Vec<String> {
    let mut result = Vec::new();
    for entry in value.and_then(Value::as_array).into_iter().flatten() {
        if let Some(text) = entry
            .as_str()
            .or_else(|| entry.get("effort").and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            if !result.iter().any(|old| old == text) {
                result.push(text.to_owned());
            }
        }
    }
    result
}

pub(crate) fn positive(model: &Value, field: &str) -> u64 {
    model
        .get(field)
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        })
        .filter(|value| *value > 0)
        .unwrap_or(0)
}

fn group_protocol(platform: &str) -> Option<&'static str> {
    match platform {
        "anthropic" | "antigravity" => Some("anthropic"),
        "gemini" => Some("gemini"),
        // These platforms use the gateway's Responses bridge, independently of
        // the model name or the upstream account's native protocol.
        "openai" | "composite" | "grok" | "kimi" | "zhipu" | "deepseek" | "minimax"
        | "opencode-go" | "commandcode" | "cline" => Some("responses"),
        _ => None,
    }
}

fn is_composite_gemini_model(id: &str) -> bool {
    id.starts_with("gemini-") || id.starts_with("google/gemini-")
}

/// 只生成调用方需要的工具模型，借用目录，避免单工具同步时克隆原始 JSON 并构建全部工具结果。
pub(crate) fn agent_models(platform: &str, models: &[Value], agents: &[&str]) -> Vec<AgentModel> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for model in models {
        let Some(id) = model.get("id").and_then(Value::as_str) else {
            continue;
        };
        if id.trim().is_empty() || id.contains(['\r', '\n', '\0']) {
            continue;
        }
        if model
            .get("supports_function_calling")
            .and_then(Value::as_bool)
            == Some(false)
            || model.get("supports_tools").and_then(Value::as_bool) == Some(false)
            || model.get("supports_streaming").and_then(Value::as_bool) == Some(false)
        {
            continue;
        }
        let mut protocols: Vec<_> = if let Some(endpoints) = model
            .get("supported_endpoint_types")
            .or_else(|| model.get("supported_endpoints"))
            .and_then(Value::as_array)
        {
            endpoints
                .iter()
                .filter_map(Value::as_str)
                .filter_map(protocol)
                .collect()
        } else {
            group_protocol(platform).into_iter().collect()
        };
        if model
            .get("supported_endpoint_types")
            .or_else(|| model.get("supported_endpoints"))
            .is_none()
            && platform == "composite"
            && is_composite_gemini_model(id)
            && !protocols.contains(&"gemini")
        {
            // Local CLI routing policy for composite groups with no explicit endpoints;
            // it does not add model IDs or change the saved server metadata.
            protocols.push("gemini");
        }
        let context = positive(model, "context_window");
        let output = positive(model, "max_output_tokens");
        let reasoning_levels = strings(model.get("supported_reasoning_levels"));
        let default_reasoning_level = model
            .get("default_reasoning_level")
            .and_then(Value::as_str)
            .filter(|level| {
                reasoning_levels.is_empty()
                    || reasoning_levels.iter().any(|supported| supported == level)
            })
            .map(str::to_owned);
        let input_modalities = strings(model.get("input_modalities"));
        for &agent in agents {
            let preferred = match agent {
                "claude" | "claude-desktop" => "anthropic",
                "gemini" => "gemini",
                _ => "responses",
            };
            let supported: Vec<_> = protocols
                .iter()
                .copied()
                .filter(|p| {
                    if agent == "gemini" {
                        *p == "gemini"
                    } else if agent == "deepseek-harness" {
                        *p != "gemini"
                    } else {
                        *p != "gemini"
                            || matches!(agent, "pi" | "openclaw" | "claude-desktop" | "kimi")
                    }
                })
                .collect();
            let selected = if agent == "workbuddy" {
                if supported.contains(&"chat") {
                    Some("chat")
                } else if !matches!(platform, "antigravity" | "gemini")
                    && supported
                        .iter()
                        .any(|protocol| matches!(*protocol, "anthropic" | "responses"))
                {
                    // Bridged platform groups expose native Anthropic and
                    // Responses models through the OpenAI-compatible endpoint
                    // WorkBuddy appends to its configured base URL.
                    Some("chat")
                } else {
                    None
                }
            } else {
                supported
                    .iter()
                    .copied()
                    .find(|p| *p == preferred)
                    .or_else(|| supported.first().copied())
            };
            let Some(protocol) = selected else { continue };
            if !seen.insert((agent.to_string(), id.to_string())) {
                continue;
            }
            result.push(AgentModel {
                id: id.into(),
                agent: agent.into(),
                model: id.into(),
                name: model
                    .get("display_name")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(id)
                    .into(),
                description: model
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                protocol: protocol.into(),
                category: "available".into(),
                available: true,
                supports_tools: model
                    .get("supports_tools")
                    .or_else(|| model.get("supports_function_calling"))
                    .and_then(Value::as_bool),
                supports_streaming: model.get("supports_streaming").and_then(Value::as_bool),
                context_window: context,
                max_output_tokens: output,
                reasoning_levels: reasoning_levels.clone(),
                default_reasoning_level: default_reasoning_level.clone(),
                reasoning: model.get("reasoning").and_then(Value::as_bool).or_else(|| {
                    (!reasoning_levels.is_empty())
                        .then(|| reasoning_levels.iter().any(|level| level != "none"))
                }),
                input_modalities: input_modalities.clone(),
                supports_parallel_tool_calls: model
                    .get("supports_parallel_tool_calls")
                    .and_then(Value::as_bool),
                pricing: None,
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::super::adapter;
    use super::*;
    use serde_json::json;

    fn agent_models(platform: &str, models: Vec<Value>) -> Vec<AgentModel> {
        super::agent_models(platform, &models, adapter::AGENTS)
    }

    #[test]
    fn selecting_one_agent_preserves_its_models_and_reported_parameters() {
        let catalog = vec![
            json!({"id":"shared","supported_endpoint_types":["responses","messages","chat"],
                "context_window":128000,"supported_reasoning_levels":["low","high"]}),
            json!({"id":"gemini-native","supported_endpoint_types":["gemini"]}),
            json!({"id":"shared","supported_endpoints":["responses"]}),
            json!({"id":"embedding","supported_endpoint_types":["embeddings"]}),
        ];
        let all = super::agent_models("composite", &catalog, adapter::AGENTS);
        for &agent in adapter::AGENTS {
            let selected = super::agent_models("composite", &catalog, &[agent]);
            let expected: Vec<_> = all.iter().filter(|model| model.agent == agent).collect();
            assert_eq!(
                serde_json::to_value(&selected).unwrap(),
                serde_json::to_value(expected).unwrap(),
                "{agent} 的单独同步应与全量同步使用相同模型和参数"
            );
        }
        assert!(super::agent_models("composite", &catalog, &[]).is_empty());
    }

    #[test]
    fn reads_reported_model_limits_efforts_and_protocol() {
        let models = agent_models(
            "composite",
            vec![json!({
                "id":"public-alias", "supported_endpoints":["openai_chat"],
                "context_window":"1000000", "max_output_tokens":64000,
                "supported_reasoning_levels":[{"effort":"low"},{"effort":"high"},{"effort":"max"},{"effort":"high"}],
                "default_reasoning_level":"high", "input_modalities":["text","image"],
                "supports_parallel_tool_calls":false
            })],
        );
        let model = models.iter().find(|model| model.agent == "codex").unwrap();
        assert_eq!(model.protocol, "chat");
        assert_eq!(model.context_window, 1000000);
        assert_eq!(model.reasoning_levels, ["low", "high", "max"]);
        assert_eq!(model.default_reasoning_level.as_deref(), Some("high"));
        assert_eq!(model.input_modalities, ["text", "image"]);
        assert_eq!(model.supports_parallel_tool_calls, Some(false));
        assert_eq!(model.reasoning, Some(true));

        let models = agent_models(
            "openai",
            vec![json!({
                "id":"partial-limits", "context_window":null,"max_output_tokens":4096,
                "supported_reasoning_levels":["none"],"default_reasoning_level":"high"
            })],
        );
        let model = models.iter().find(|model| model.agent == "codex").unwrap();
        assert_eq!(model.context_window, 0);
        assert_eq!(model.max_output_tokens, 4096);
        assert_eq!(model.default_reasoning_level, None);
        assert_eq!(model.reasoning, Some(false));
    }

    #[test]
    fn uses_access_authorized_ids_without_a_separate_catalog() {
        let models = agent_models(
            "openai",
            vec![json!({"id":"custom-model"}), json!({"id":"custom-model"})],
        );
        assert_eq!(models.len(), 12);
        for agent in [
            "claude",
            "claude-desktop",
            "codex",
            "deepseek-harness",
            "kimi",
        ] {
            assert!(models
                .iter()
                .any(|m| m.agent == agent && m.id == "custom-model" && m.protocol == "responses"));
        }
        assert!(models
            .iter()
            .any(|m| m.agent == "workbuddy" && m.id == "custom-model" && m.protocol == "chat"));
        assert!(agent_models("openai", vec![]).is_empty());
        assert!(agent_models("unknown", vec![json!({"id":"custom-model"})]).is_empty());
    }

    #[test]
    fn includes_desktop_and_harness_with_their_preferred_protocols() {
        let models = agent_models(
            "openai",
            vec![json!({
                "id":"shared-model",
                "supported_endpoint_types":["messages","responses"]
            })],
        );
        assert_eq!(
            models
                .iter()
                .find(|model| model.agent == "claude-desktop")
                .map(|model| model.protocol.as_str()),
            Some("anthropic")
        );
        assert_eq!(
            models
                .iter()
                .find(|model| model.agent == "deepseek-harness")
                .map(|model| model.protocol.as_str()),
            Some("responses")
        );

        assert!(agent_models(
            "gemini",
            vec![json!({"id":"gemini-only","supported_endpoint_types":["gemini"]})]
        )
        .iter()
        .all(|model| model.agent != "deepseek-harness"));
    }

    #[test]
    fn maps_anthropic_models_to_chat_for_bridged_workbuddy_groups() {
        let models = agent_models(
            "composite",
            vec![json!({
                "id":"claude-sonnet-5",
                "display_name":"Claude Sonnet 5",
                "supported_endpoint_types":["anthropic"]
            })],
        );
        let workbuddy = models
            .iter()
            .find(|model| model.agent == "workbuddy")
            .expect("WorkBuddy should receive bridged Claude models");
        assert_eq!(workbuddy.protocol, "chat");
        assert_eq!(workbuddy.model, "claude-sonnet-5");

        let anthropic_group = agent_models(
            "anthropic",
            vec![json!({
                "id":"claude-sonnet-5",
                "supported_endpoint_types":["anthropic"]
            })],
        );
        let workbuddy = anthropic_group
            .iter()
            .find(|model| model.agent == "workbuddy")
            .expect("Anthropic groups expose Chat Completions for WorkBuddy");
        assert_eq!(workbuddy.protocol, "chat");
    }

    #[test]
    fn keeps_limitless_models_and_infers_composite_gemini_models() {
        let models = agent_models(
            "composite",
            vec![
                json!({"id":"gemini-3.1-pro"}),
                json!({"id":"google/gemini-3.1-pro"}),
                json!({"id":"gpt-5.6-sol"}),
            ],
        );
        assert!(models
            .iter()
            .any(|model| model.agent == "gemini" && model.model == "gemini-3.1-pro"));
        assert!(models
            .iter()
            .any(|model| model.agent == "gemini" && model.model == "google/gemini-3.1-pro"));
        assert!(!models
            .iter()
            .any(|model| model.agent == "gemini" && model.model == "gpt-5.6-sol"));
        for agent in ["grokbuild", "openclaw", "pi", "kimi"] {
            assert!(
                models
                    .iter()
                    .any(|model| model.agent == agent && model.model == "gpt-5.6-sol"),
                "{agent}"
            );
        }
    }

    #[test]
    fn filters_protocols_and_capabilities() {
        let models = agent_models(
            "composite",
            vec![
                json!({"id":"native-gemini","supported_endpoint_types":["gemini"],"context_window":32000,"max_output_tokens":4096}),
                json!({"id":"embedding","supported_endpoint_types":["embeddings"]}),
                json!({"id":"no-tools","supports_function_calling":false}),
                json!({"id":"non-streaming","supports_streaming":false}),
                json!({"id":"image","supported_endpoint_types":["images"]}),
            ],
        );
        assert_eq!(models.len(), 5);
        assert!(models
            .iter()
            .all(|m| m.model == "native-gemini" && m.protocol == "gemini"));
        assert!(models.iter().any(|m| m.agent == "gemini"));
        let models = agent_models(
            "composite",
            vec![json!({"id":"native","supported_endpoint_types":["responses","messages"]})],
        );
        assert_eq!(
            models
                .iter()
                .find(|m| m.agent == "claude")
                .unwrap()
                .protocol,
            "anthropic"
        );
        assert_eq!(
            models.iter().find(|m| m.agent == "codex").unwrap().protocol,
            "responses"
        );
    }
    #[test]
    fn unknown_capabilities_remain_unknown_in_adapter_inputs() {
        let models = agent_models(
            "openai",
            vec![json!({"id":"alias","default_reasoning_level":"high"})],
        );
        let model = models.iter().find(|model| model.agent == "codex").unwrap();
        assert_eq!(model.supports_tools, None);
        assert_eq!(model.supports_streaming, None);
        assert_eq!(model.context_window, 0);
        assert!(model.reasoning_levels.is_empty());
        assert_eq!(model.default_reasoning_level.as_deref(), Some("high"));
    }
}
