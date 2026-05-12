use super::tool_execution::truncate_tool_result;
use super::tool_metadata::{build_tool_execution_summary, provider_tool_result_content};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::services::api::{Message, ToolCall};
use crate::tools::ToolResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NormalizedEvidenceFact {
    Command,
    Validation,
    File,
    ChangedFile,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct NormalizedToolResult {
    pub(super) model_content: String,
    pub(super) ui_content: String,
    pub(super) structured_metadata: serde_json::Value,
    pub(super) evidence_facts: Vec<NormalizedEvidenceFact>,
}

impl NormalizedToolResult {
    fn record_evidence(
        &self,
        evidence_ledger: &mut EvidenceLedger,
        tool_call: &ToolCall,
        result: &ToolResult,
    ) {
        if self.evidence_facts.is_empty() {
            return;
        }
        if self.structured_metadata.get("tool_summary").is_none() {
            return;
        }
        evidence_ledger.record_tool_result(tool_call, result);
    }
}

pub(super) struct ToolResultNormalizer;

impl ToolResultNormalizer {
    pub(super) fn normalize(tool_call: &ToolCall, result: &ToolResult) -> NormalizedToolResult {
        let model_content = provider_tool_result_content(tool_call, result);
        NormalizedToolResult {
            ui_content: model_content.clone(),
            model_content,
            structured_metadata: structured_metadata(tool_call, result),
            evidence_facts: evidence_facts(tool_call, result),
        }
    }

    pub(super) async fn normalize_after_execution(
        tool_call: &ToolCall,
        result: &mut ToolResult,
    ) -> NormalizedToolResult {
        truncate_tool_result(result, &tool_call.name, &tool_call.id).await;
        Self::normalize(tool_call, result)
    }
}

pub(super) async fn append_provider_tool_result(
    tool_call: &ToolCall,
    result: &mut ToolResult,
    evidence_ledger: &mut EvidenceLedger,
    tool_results_text: &mut String,
    messages: &mut Vec<Message>,
) {
    let normalized = ToolResultNormalizer::normalize_after_execution(tool_call, result).await;
    normalized.record_evidence(evidence_ledger, tool_call, result);
    tool_results_text.push_str(&normalized.ui_content);
    tool_results_text.push('\n');
    messages.push(Message::tool(
        tool_call.id.clone(),
        normalized.model_content,
    ));
}

fn structured_metadata(tool_call: &ToolCall, result: &ToolResult) -> serde_json::Value {
    let tool_summary = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_summary"))
        .cloned()
        .unwrap_or_else(|| build_tool_execution_summary(tool_call, result));
    serde_json::json!({
        "tool": tool_call.name.clone(),
        "call_id": tool_call.id.clone(),
        "success": result.success,
        "duration_ms": result.duration_ms,
        "error": result.error.clone(),
        "tool_summary": tool_summary,
        "tool_result_data": result.data.clone().unwrap_or(serde_json::Value::Null),
    })
}

fn evidence_facts(tool_call: &ToolCall, result: &ToolResult) -> Vec<NormalizedEvidenceFact> {
    match tool_call.name.as_str() {
        "bash" => bash_evidence_facts(tool_call),
        "file_read" | "glob" | "grep" => vec![NormalizedEvidenceFact::File],
        "file_write" | "file_edit" => {
            let mut facts = vec![NormalizedEvidenceFact::File];
            if result.success {
                facts.push(NormalizedEvidenceFact::ChangedFile);
            }
            facts
        }
        _ => Vec::new(),
    }
}

fn bash_evidence_facts(tool_call: &ToolCall) -> Vec<NormalizedEvidenceFact> {
    let Some(command) = tool_call.arguments["command"].as_str() else {
        return Vec::new();
    };
    let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
    let mut facts = vec![NormalizedEvidenceFact::Command];
    if classification.is_safe_validation() {
        facts.push(NormalizedEvidenceFact::Validation);
    }
    facts
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

    #[tokio::test]
    async fn appends_provider_tool_result_and_records_evidence() {
        let mut ledger = EvidenceLedger::new();
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let mut result = ToolResult::success("ok");

        append_provider_tool_result(
            &tool_call("bash"),
            &mut result,
            &mut ledger,
            &mut tool_results_text,
            &mut messages,
        )
        .await;

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

    #[tokio::test]
    async fn normalize_after_execution_truncates_large_output_with_metadata() {
        let mut result = ToolResult::success("A".repeat(40_000));
        let normalized = ToolResultNormalizer::normalize_after_execution(
            &ToolCall {
                id: "call_large".to_string(),
                name: "grep".to_string(),
                arguments: serde_json::json!({"pattern": "A", "path": "src"}),
            },
            &mut result,
        )
        .await;

        assert!(normalized.model_content.contains("Output truncated"));
        assert_eq!(
            normalized.structured_metadata["tool_result_data"]["output_truncation"]
                ["original_bytes"],
            40_000
        );
        assert!(
            normalized.structured_metadata["tool_result_data"]["output_truncation"]["stored_path"]
                .as_str()
                .unwrap_or_default()
                .contains("tool-results")
        );
    }

    #[test]
    fn normalizes_provider_tool_result_content() {
        let normalized =
            ToolResultNormalizer::normalize(&tool_call("bash"), &ToolResult::success("ok"));

        assert_eq!(normalized.model_content, "Result: OK\nok");
        assert_eq!(normalized.ui_content, "Result: OK\nok");
        assert_eq!(
            normalized.evidence_facts,
            vec![
                NormalizedEvidenceFact::Command,
                NormalizedEvidenceFact::Validation
            ]
        );
        assert_eq!(normalized.structured_metadata["tool"], "bash");
        assert_eq!(normalized.structured_metadata["call_id"], "call_1");
        assert_eq!(normalized.structured_metadata["success"], true);
        assert!(normalized.structured_metadata.get("tool_summary").is_some());
    }

    #[test]
    fn normalizes_file_write_evidence_categories() {
        let normalized = ToolResultNormalizer::normalize(
            &ToolCall {
                id: "call_2".to_string(),
                name: "file_write".to_string(),
                arguments: serde_json::json!({"path": "src/app.rs"}),
            },
            &ToolResult::success("Wrote file"),
        );

        assert_eq!(
            normalized.evidence_facts,
            vec![
                NormalizedEvidenceFact::File,
                NormalizedEvidenceFact::ChangedFile
            ]
        );
    }
}
