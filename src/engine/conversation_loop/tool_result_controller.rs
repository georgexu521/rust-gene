use super::tool_execution::truncate_tool_result;
use super::tool_metadata::{
    build_tool_execution_summary, provider_tool_result_content, tool_error_code_label,
};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::services::api::{Message, ToolCall};
use crate::tools::{ToolErrorCode, ToolResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NormalizedEvidenceFact {
    Command,
    Validation,
    File,
    ChangedFile,
    Permission,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct NormalizedToolResult {
    pub(super) model_content: String,
    pub(super) ui_content: String,
    pub(super) structured_metadata: serde_json::Value,
    pub(super) evidence_facts: Vec<NormalizedEvidenceFact>,
    pub(super) context_policy: ToolResultContextPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ToolResultContextPolicy {
    pub(super) provider_visible_chars: usize,
    pub(super) desktop_visible_chars: usize,
    pub(super) trace_payload_available: bool,
    pub(super) durable_artifact_path: Option<String>,
    pub(super) ledger_fact_eligible: bool,
    pub(super) compaction_eligible: bool,
    pub(super) protected_recent_tail: bool,
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
        let mut structured_metadata = structured_metadata(tool_call, result);
        let evidence_facts = evidence_facts(tool_call, result);
        let context_policy = ToolResultContextPolicy::from_normalized(
            tool_call,
            result,
            &model_content,
            &structured_metadata,
            &evidence_facts,
        );
        if let Some(object) = structured_metadata.as_object_mut() {
            object.insert(
                "tool_result_context_policy".to_string(),
                context_policy.as_metadata(),
            );
        }
        NormalizedToolResult {
            ui_content: model_content.clone(),
            model_content,
            structured_metadata,
            evidence_facts,
            context_policy,
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

impl ToolResultContextPolicy {
    fn from_normalized(
        tool_call: &ToolCall,
        result: &ToolResult,
        model_content: &str,
        structured_metadata: &serde_json::Value,
        evidence_facts: &[NormalizedEvidenceFact],
    ) -> Self {
        let durable_artifact_path = structured_metadata
            .get("tool_result_data")
            .and_then(|data| data.get("output_truncation"))
            .and_then(|truncation| truncation.get("stored_path"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let is_high_value = matches!(
            tool_call.name.as_str(),
            "file_edit" | "file_write" | "file_patch" | "permission_request"
        ) || !result.success;
        let ledger_fact_eligible = evidence_facts.iter().any(|fact| {
            matches!(
                fact,
                NormalizedEvidenceFact::Command
                    | NormalizedEvidenceFact::Validation
                    | NormalizedEvidenceFact::File
                    | NormalizedEvidenceFact::ChangedFile
            )
        });
        Self {
            provider_visible_chars: model_content.chars().count(),
            desktop_visible_chars: model_content.chars().count(),
            trace_payload_available: structured_metadata.get("tool_result_data").is_some(),
            durable_artifact_path,
            ledger_fact_eligible,
            compaction_eligible: !is_high_value || model_content.chars().count() > 8_000,
            protected_recent_tail: is_high_value,
        }
    }

    fn as_metadata(&self) -> serde_json::Value {
        serde_json::json!({
            "provider_visible_chars": self.provider_visible_chars,
            "desktop_visible_chars": self.desktop_visible_chars,
            "trace_payload_available": self.trace_payload_available,
            "durable_artifact_path": self.durable_artifact_path,
            "ledger_fact_eligible": self.ledger_fact_eligible,
            "compaction_eligible": self.compaction_eligible,
            "protected_recent_tail": self.protected_recent_tail,
        })
    }
}

pub(super) async fn append_provider_tool_result(
    tool_call: &ToolCall,
    result: &mut ToolResult,
    evidence_ledger: &mut EvidenceLedger,
    tool_results_text: &mut String,
    messages: &mut Vec<Message>,
) -> NormalizedToolResult {
    let normalized = ToolResultNormalizer::normalize_after_execution(tool_call, result).await;
    normalized.record_evidence(evidence_ledger, tool_call, result);
    tool_results_text.push_str(&normalized.ui_content);
    tool_results_text.push('\n');
    messages.push(Message::tool(
        tool_call.id.clone(),
        normalized.model_content.clone(),
    ));
    normalized
}

pub(super) fn invalid_tool_params_result(
    tool_call: &ToolCall,
    error: impl Into<String>,
) -> ToolResult {
    let error = error.into();
    let mut result = ToolResult::error(format!(
        "Invalid params for '{}': {}",
        tool_call.name, error
    ));
    result.error_code = Some(ToolErrorCode::InvalidParams);
    result.data = Some(serde_json::json!({
        "schema_validation": {
            "valid": false,
            "error": error,
        }
    }));
    result
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
        "error_code": tool_error_code_label(result),
        "tool_summary": tool_summary,
        "tool_result_data": result.data.clone().unwrap_or(serde_json::Value::Null),
    })
}

fn evidence_facts(tool_call: &ToolCall, result: &ToolResult) -> Vec<NormalizedEvidenceFact> {
    let mut facts = match tool_call.name.as_str() {
        "bash" => bash_evidence_facts(tool_call, result),
        "file_read" | "glob" | "grep" => vec![NormalizedEvidenceFact::File],
        "file_write" | "file_edit" | "file_patch" => {
            let mut facts = vec![NormalizedEvidenceFact::File];
            if result.success {
                facts.push(NormalizedEvidenceFact::ChangedFile);
            }
            facts
        }
        _ => Vec::new(),
    };
    if result
        .data
        .as_ref()
        .and_then(|data| data.get("permission_request"))
        .is_some()
    {
        facts.push(NormalizedEvidenceFact::Permission);
    }
    facts
}

fn bash_evidence_facts(tool_call: &ToolCall, result: &ToolResult) -> Vec<NormalizedEvidenceFact> {
    let Some(command) = tool_call.arguments["command"]
        .as_str()
        .or_else(|| bash_result_command(result))
    else {
        return Vec::new();
    };
    let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
    let mut facts = vec![NormalizedEvidenceFact::Command];
    if classification.is_safe_validation() {
        facts.push(NormalizedEvidenceFact::Validation);
    }
    facts
}

fn bash_result_command(result: &ToolResult) -> Option<&str> {
    result
        .data
        .as_ref()
        .and_then(|data| data.get("shell_result"))
        .and_then(|shell| shell.get("command"))
        .and_then(serde_json::Value::as_str)
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
        assert!(normalized.context_policy.compaction_eligible);
        assert!(normalized
            .context_policy
            .durable_artifact_path
            .as_deref()
            .unwrap_or_default()
            .contains("tool-results"));
        assert_eq!(
            normalized.structured_metadata["tool_result_context_policy"]["compaction_eligible"],
            true
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
        assert_eq!(normalized.structured_metadata["error_code"], "success");
        assert!(normalized.structured_metadata.get("tool_summary").is_some());
        assert!(normalized.context_policy.ledger_fact_eligible);
        assert!(!normalized.context_policy.protected_recent_tail);
    }

    #[test]
    fn context_policy_protects_failed_and_mutating_tool_results() {
        let failed = ToolResultNormalizer::normalize(
            &ToolCall {
                id: "call_fail".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "cargo test -q"}),
            },
            &ToolResult::error("tests failed"),
        );
        assert!(failed.context_policy.protected_recent_tail);

        let edit = ToolResultNormalizer::normalize(
            &ToolCall {
                id: "call_edit".to_string(),
                name: "file_edit".to_string(),
                arguments: serde_json::json!({"path": "src/lib.rs"}),
            },
            &ToolResult::success("edited src/lib.rs"),
        );
        assert!(edit.context_policy.protected_recent_tail);
        assert!(edit.context_policy.ledger_fact_eligible);
    }

    #[test]
    fn normalizes_bash_validation_from_result_metadata_when_arguments_are_missing() {
        let result = ToolResult::success_with_data(
            "PASS: directory exists",
            serde_json::json!({
                "shell_result": {
                    "command": "if test -d fixtures/core_quality/inspection_target/gex; then echo PASS; else echo FAIL; fi"
                }
            }),
        );
        let normalized = ToolResultNormalizer::normalize(
            &ToolCall {
                id: "call_from_result".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({}),
            },
            &result,
        );

        assert_eq!(
            normalized.evidence_facts,
            vec![
                NormalizedEvidenceFact::Command,
                NormalizedEvidenceFact::Validation
            ]
        );
    }

    #[test]
    fn invalid_params_result_carries_schema_validation_metadata() {
        let result =
            invalid_tool_params_result(&tool_call("bash"), "Missing required parameter: command");
        let normalized = ToolResultNormalizer::normalize(&tool_call("bash"), &result);

        assert!(!result.success);
        assert_eq!(
            normalized.structured_metadata["error_code"],
            "invalid_params"
        );
        assert_eq!(
            normalized.structured_metadata["tool_result_data"]["schema_validation"]["valid"],
            false
        );
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

    #[test]
    fn normalizes_permission_denied_evidence_category() {
        let mut result = ToolResult::error("Permission denied: 'git' requires user confirmation.");
        result.data = Some(serde_json::json!({
            "permission_request": {
                "kind": "runtime_rule",
                "rejection_feedback": "Permission denied: 'git' requires user confirmation."
            }
        }));
        let normalized = ToolResultNormalizer::normalize(
            &ToolCall {
                id: "call_permission".to_string(),
                name: "git".to_string(),
                arguments: serde_json::json!({"action": "push"}),
            },
            &result,
        );

        assert_eq!(
            normalized.evidence_facts,
            vec![NormalizedEvidenceFact::Permission]
        );
        assert_eq!(
            normalized.structured_metadata["tool_result_data"]["permission_request"]["kind"],
            "runtime_rule"
        );
    }
}
