//! Provider-bound message protocol normalization.
//!
//! This layer keeps OpenAI-compatible tool-call turns in a shape strict
//! providers accept before the request is serialized.

use crate::services::api::{normalize_tool_message_sequence, Message};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
