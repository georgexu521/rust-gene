//! Provider-bound message protocol normalization.
//!
//! This layer keeps OpenAI-compatible tool-call turns in a shape strict
//! providers accept before the request is serialized.

use crate::services::api::{normalize_tool_message_sequence, Message};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocolFamily {
    OpenAiCompatible,
    MiniMax,
    Kimi,
    AnthropicLike,
    ReasoningCapable,
}

impl ProviderProtocolFamily {
    pub fn label(self) -> &'static str {
        match self {
            ProviderProtocolFamily::OpenAiCompatible => "openai_compatible",
            ProviderProtocolFamily::MiniMax => "minimax",
            ProviderProtocolFamily::Kimi => "kimi",
            ProviderProtocolFamily::AnthropicLike => "anthropic_like",
            ProviderProtocolFamily::ReasoningCapable => "reasoning_capable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub protocol_family: ProviderProtocolFamily,
    pub supports_tool_calls: bool,
    pub supports_streaming_tool_calls: bool,
    pub supports_streaming_usage: bool,
    pub supports_reasoning_tokens: bool,
    pub requires_nonstreaming_tool_calls: bool,
    pub requires_merged_system_messages: bool,
    pub requires_tool_result_adjacency: bool,
}

impl ProviderCapabilities {
    pub const fn for_family(protocol_family: ProviderProtocolFamily) -> Self {
        match protocol_family {
            ProviderProtocolFamily::MiniMax => Self {
                protocol_family,
                supports_tool_calls: true,
                supports_streaming_tool_calls: false,
                supports_streaming_usage: false,
                supports_reasoning_tokens: true,
                requires_nonstreaming_tool_calls: true,
                requires_merged_system_messages: true,
                requires_tool_result_adjacency: true,
            },
            ProviderProtocolFamily::Kimi => Self {
                protocol_family,
                supports_tool_calls: true,
                supports_streaming_tool_calls: true,
                supports_streaming_usage: true,
                supports_reasoning_tokens: true,
                requires_nonstreaming_tool_calls: false,
                requires_merged_system_messages: false,
                requires_tool_result_adjacency: true,
            },
            ProviderProtocolFamily::ReasoningCapable => Self {
                protocol_family,
                supports_tool_calls: true,
                supports_streaming_tool_calls: true,
                supports_streaming_usage: true,
                supports_reasoning_tokens: true,
                requires_nonstreaming_tool_calls: false,
                requires_merged_system_messages: false,
                requires_tool_result_adjacency: true,
            },
            ProviderProtocolFamily::AnthropicLike => Self {
                protocol_family,
                supports_tool_calls: true,
                supports_streaming_tool_calls: true,
                supports_streaming_usage: true,
                supports_reasoning_tokens: false,
                requires_nonstreaming_tool_calls: false,
                requires_merged_system_messages: false,
                requires_tool_result_adjacency: true,
            },
            ProviderProtocolFamily::OpenAiCompatible => Self {
                protocol_family,
                supports_tool_calls: true,
                supports_streaming_tool_calls: true,
                supports_streaming_usage: true,
                supports_reasoning_tokens: false,
                requires_nonstreaming_tool_calls: false,
                requires_merged_system_messages: false,
                requires_tool_result_adjacency: true,
            },
        }
    }

    pub fn detect(base_url: &str, model: &str) -> Self {
        let base = base_url.to_ascii_lowercase();
        let model = model.to_ascii_lowercase();
        let family = if base.contains("minimax") || model.contains("minimax") {
            ProviderProtocolFamily::MiniMax
        } else if base.contains("moonshot") || model.contains("kimi") {
            ProviderProtocolFamily::Kimi
        } else if base.contains("anthropic") || model.contains("claude") {
            ProviderProtocolFamily::AnthropicLike
        } else if model.contains("reasoning") || model.starts_with("o1") || model.starts_with("o3")
        {
            ProviderProtocolFamily::ReasoningCapable
        } else {
            ProviderProtocolFamily::OpenAiCompatible
        };
        Self::for_family(family)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRuntimeFacts {
    pub model: String,
    pub protocol_family: ProviderProtocolFamily,
    pub supports_tool_calls: bool,
    pub supports_streaming_tool_calls: bool,
    pub supports_streaming_usage: bool,
    pub supports_reasoning_tokens: bool,
    pub requires_nonstreaming_tool_calls: bool,
    pub requires_merged_system_messages: bool,
    pub requires_tool_result_adjacency: bool,
    pub normalization: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl ProviderRuntimeFacts {
    pub fn detect(base_url: &str, model: &str) -> Self {
        Self::from_capabilities(model, ProviderCapabilities::detect(base_url, model))
    }

    pub fn from_capabilities(model: &str, capabilities: ProviderCapabilities) -> Self {
        let mut normalization = vec!["tool_result_sequence:sanitize".to_string()];
        if capabilities.requires_merged_system_messages {
            normalization.push("system_messages:merge".to_string());
        }
        if capabilities.requires_tool_result_adjacency {
            normalization.push("tool_results:adjacent_to_tool_calls".to_string());
        }

        let mut diagnostics = Vec::new();
        if capabilities.requires_nonstreaming_tool_calls {
            diagnostics.push("tool calls require non-streaming request path".to_string());
        }
        if !capabilities.supports_streaming_usage {
            diagnostics.push("streaming usage deltas unavailable".to_string());
        }
        if capabilities.supports_reasoning_tokens {
            diagnostics.push("reasoning token accounting supported".to_string());
        }
        if diagnostics.is_empty() {
            diagnostics.push("provider uses standard OpenAI-compatible streaming path".to_string());
        }

        Self {
            model: model.to_string(),
            protocol_family: capabilities.protocol_family,
            supports_tool_calls: capabilities.supports_tool_calls,
            supports_streaming_tool_calls: capabilities.supports_streaming_tool_calls,
            supports_streaming_usage: capabilities.supports_streaming_usage,
            supports_reasoning_tokens: capabilities.supports_reasoning_tokens,
            requires_nonstreaming_tool_calls: capabilities.requires_nonstreaming_tool_calls,
            requires_merged_system_messages: capabilities.requires_merged_system_messages,
            requires_tool_result_adjacency: capabilities.requires_tool_result_adjacency,
            normalization,
            diagnostics,
        }
    }
}

pub fn normalize_messages_for_provider(
    family: ProviderProtocolFamily,
    messages: Vec<Message>,
) -> Vec<Message> {
    let messages = match family {
        ProviderProtocolFamily::MiniMax => merge_system_messages(messages),
        ProviderProtocolFamily::OpenAiCompatible
        | ProviderProtocolFamily::Kimi
        | ProviderProtocolFamily::AnthropicLike
        | ProviderProtocolFamily::ReasoningCapable => messages,
    };

    normalize_tool_message_sequence(messages)
}

pub fn normalize_messages_for_capabilities(
    capabilities: ProviderCapabilities,
    messages: Vec<Message>,
) -> Vec<Message> {
    normalize_messages_for_provider(capabilities.protocol_family, messages)
}

fn merge_system_messages(messages: Vec<Message>) -> Vec<Message> {
    let mut system_parts: Vec<String> = Vec::new();
    let mut others: Vec<Message> = Vec::new();

    for msg in messages {
        match msg {
            Message::System { content } => system_parts.push(content),
            other => others.push(other),
        }
    }

    if system_parts.is_empty() {
        return others;
    }

    let mut normalized = Vec::with_capacity(others.len() + 1);
    normalized.push(Message::system(system_parts.join("\n\n")));
    normalized.extend(others);
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::ToolCall;

    fn tool_call(id: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "Cargo.toml"}),
        }
    }

    fn all_families() -> [ProviderProtocolFamily; 5] {
        [
            ProviderProtocolFamily::OpenAiCompatible,
            ProviderProtocolFamily::MiniMax,
            ProviderProtocolFamily::Kimi,
            ProviderProtocolFamily::AnthropicLike,
            ProviderProtocolFamily::ReasoningCapable,
        ]
    }

    #[test]
    fn provider_capabilities_capture_minimax_streaming_constraints() {
        let capabilities =
            ProviderCapabilities::detect("https://api.minimaxi.com/v1", "MiniMax-M2.7");

        assert_eq!(
            capabilities.protocol_family,
            ProviderProtocolFamily::MiniMax
        );
        assert!(capabilities.supports_tool_calls);
        assert!(!capabilities.supports_streaming_tool_calls);
        assert!(capabilities.requires_nonstreaming_tool_calls);
        assert!(capabilities.requires_merged_system_messages);
    }

    #[test]
    fn provider_runtime_facts_explain_minimax_constraints() {
        let facts = ProviderRuntimeFacts::detect("https://api.minimaxi.com/v1", "MiniMax-M2.7");

        assert_eq!(facts.protocol_family, ProviderProtocolFamily::MiniMax);
        assert!(facts.requires_nonstreaming_tool_calls);
        assert!(facts.requires_merged_system_messages);
        assert!(facts
            .normalization
            .contains(&"system_messages:merge".to_string()));
        assert!(facts
            .diagnostics
            .iter()
            .any(|line| line.contains("non-streaming")));
    }

    #[test]
    fn provider_runtime_facts_describe_standard_openai_path() {
        let facts = ProviderRuntimeFacts::detect("https://api.openai.com/v1", "gpt-4.1");

        assert_eq!(
            facts.protocol_family,
            ProviderProtocolFamily::OpenAiCompatible
        );
        assert!(facts.supports_streaming_tool_calls);
        assert!(facts
            .normalization
            .contains(&"tool_result_sequence:sanitize".to_string()));
        assert!(facts
            .diagnostics
            .iter()
            .any(|line| line.contains("standard OpenAI-compatible")));
    }

    #[test]
    fn provider_capabilities_drive_same_normalization_path() {
        let capabilities =
            ProviderCapabilities::for_family(ProviderProtocolFamily::OpenAiCompatible);
        let normalized = normalize_messages_for_capabilities(
            capabilities,
            vec![
                Message::user("inspect"),
                Message::assistant_with_tools("", vec![tool_call("call_1")]),
                Message::tool("call_1", "Result: OK\ncontent"),
            ],
        );

        assert_eq!(normalized.len(), 3);
        assert!(normalized[1].tool_calls().is_some());
    }

    #[test]
    fn provider_matrix_keeps_valid_tool_roundtrips() {
        for family in all_families() {
            let normalized = normalize_messages_for_provider(
                family,
                vec![
                    Message::user("inspect"),
                    Message::assistant_with_tools("", vec![tool_call("call_1")]),
                    Message::tool("call_1", "Result: OK\ncontent"),
                    Message::assistant("done"),
                ],
            );

            assert!(
                matches!(normalized[1], Message::Assistant { .. }),
                "assistant call kept for {}",
                family.label()
            );
            assert!(
                normalized[1].tool_calls().is_some(),
                "tool calls kept for {}",
                family.label()
            );
            assert!(
                matches!(normalized[2], Message::Tool { .. }),
                "tool result kept for {}",
                family.label()
            );
        }
    }

    #[test]
    fn provider_matrix_drops_orphan_tool_result_after_abort() {
        for family in all_families() {
            let normalized = normalize_messages_for_provider(
                family,
                vec![
                    Message::user("continue"),
                    Message::tool("call_aborted", "Tool aborted"),
                    Message::assistant("I can continue without it."),
                ],
            );

            assert_eq!(normalized.len(), 2, "{}", family.label());
            assert!(
                normalized
                    .iter()
                    .all(|msg| !matches!(msg, Message::Tool { .. })),
                "orphan tool result removed for {}",
                family.label()
            );
        }
    }

    #[test]
    fn provider_matrix_downgrades_incomplete_multiple_tool_call_turns() {
        for family in all_families() {
            let normalized = normalize_messages_for_provider(
                family,
                vec![
                    Message::assistant_with_tools(
                        "I need two reads.",
                        vec![tool_call("call_1"), tool_call("call_2")],
                    ),
                    Message::tool("call_1", "Result: OK\nfirst"),
                    Message::assistant("partial"),
                ],
            );

            assert_eq!(normalized.len(), 2, "{}", family.label());
            assert!(
                normalized[0].tool_calls().is_none(),
                "incomplete assistant tool calls downgraded for {}",
                family.label()
            );
        }
    }

    #[test]
    fn provider_matrix_preserves_multiple_complete_tool_results() {
        for family in all_families() {
            let normalized = normalize_messages_for_provider(
                family,
                vec![
                    Message::assistant_with_tools(
                        "I need two reads.",
                        vec![tool_call("call_1"), tool_call("call_2")],
                    ),
                    Message::tool("call_1", "Result: OK\nfirst"),
                    Message::tool("call_2", "Error: missing file"),
                ],
            );

            assert_eq!(normalized.len(), 3, "{}", family.label());
            assert!(normalized[0].tool_calls().is_some(), "{}", family.label());
            assert!(
                matches!(normalized[1], Message::Tool { .. }),
                "{}",
                family.label()
            );
            assert!(
                matches!(normalized[2], Message::Tool { .. }),
                "{}",
                family.label()
            );
        }
    }

    #[test]
    fn minimax_merges_system_messages_without_breaking_tool_pairs() {
        let normalized = normalize_messages_for_provider(
            ProviderProtocolFamily::MiniMax,
            vec![
                Message::system("system one"),
                Message::user("inspect"),
                Message::system("system two"),
                Message::assistant_with_tools("", vec![tool_call("call_1")]),
                Message::tool("call_1", "Result: OK\ncontent"),
            ],
        );

        match &normalized[0] {
            Message::System { content } => assert_eq!(content, "system one\n\nsystem two"),
            other => panic!("expected merged system message, got {other:?}"),
        }
        assert!(normalized[2].tool_calls().is_some());
        assert!(matches!(normalized[3], Message::Tool { .. }));
    }
}
