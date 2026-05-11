use super::tool_metadata::provider_tool_result_content;
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::services::api::{Message, ToolCall};
use crate::tools::ToolResult;

pub(super) struct NormalizedToolResult {
    pub(super) model_content: String,
}

pub(super) struct ToolResultNormalizer;

impl ToolResultNormalizer {
    pub(super) fn normalize_for_provider(
        tool_call: &ToolCall,
        result: &ToolResult,
    ) -> NormalizedToolResult {
        NormalizedToolResult {
            model_content: provider_tool_result_content(tool_call, result),
        }
    }
}

pub(super) fn append_provider_tool_result(
    tool_call: &ToolCall,
    result: &ToolResult,
    evidence_ledger: &mut EvidenceLedger,
    tool_results_text: &mut String,
    messages: &mut Vec<Message>,
) {
    evidence_ledger.record_tool_result(tool_call, result);
    let normalized = ToolResultNormalizer::normalize_for_provider(tool_call, result);
    tool_results_text.push_str(&normalized.model_content);
    tool_results_text.push('\n');
    messages.push(Message::tool(
        tool_call.id.clone(),
        normalized.model_content,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(name: &str) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: name.to_string(),
            arguments: serde_json::json!({"command": "cargo test -q"}),
        }
    }

    #[test]
    fn appends_provider_tool_result_and_records_evidence() {
        let mut ledger = EvidenceLedger::new();
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();

        append_provider_tool_result(
            &tool_call("bash"),
            &ToolResult::success("ok"),
            &mut ledger,
            &mut tool_results_text,
            &mut messages,
        );

        assert_eq!(tool_results_text, "Result: OK\nok\n");
        assert_eq!(ledger.snapshot().command_facts, 1);
        assert_eq!(ledger.snapshot().validation_facts, 1);
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            &messages[0],
            Message::Tool {
                tool_call_id,
                content
            } if tool_call_id == "call_1" && content == "Result: OK\nok"
        ));
    }

    #[test]
    fn normalizes_provider_tool_result_content() {
        let normalized = ToolResultNormalizer::normalize_for_provider(
            &tool_call("bash"),
            &ToolResult::success("ok"),
        );

        assert_eq!(normalized.model_content, "Result: OK\nok");
    }
}
