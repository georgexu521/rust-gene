use super::tool_execution::truncate_tool_result;
use super::tool_metadata::{
    build_tool_execution_summary, merge_tool_result_metadata, provider_tool_result_content,
    tool_error_code_label,
};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::services::api::{Message, ToolCall};
use crate::tools::{ToolErrorCode, ToolResult};
use serde::{Deserialize, Serialize};

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
    pub(super) observation: ToolObservation,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct ToolObservation {
    pub(super) schema: String,
    pub(super) tool: String,
    pub(super) call_id: String,
    pub(super) status: String,
    pub(super) summary: String,
    pub(super) files_read: Vec<String>,
    pub(super) files_changed: Vec<String>,
    pub(super) command_run: Option<String>,
    pub(super) validation_result: Option<String>,
    pub(super) permission_decision: Option<String>,
    pub(super) checkpoint_id: Option<String>,
    pub(super) artifact_path: Option<String>,
    pub(super) state_updates: Vec<String>,
    pub(super) recommended_next_action: Option<String>,
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
        let observation = ToolObservation::from_result(tool_call, result, &model_content);
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
            object.insert("tool_observation".to_string(), observation.as_metadata());
        }
        NormalizedToolResult {
            ui_content: model_content.clone(),
            model_content,
            structured_metadata,
            observation,
            evidence_facts,
            context_policy,
        }
    }

    pub(super) async fn normalize_after_execution(
        tool_call: &ToolCall,
        result: &mut ToolResult,
    ) -> NormalizedToolResult {
        truncate_tool_result(result, &tool_call.name, &tool_call.id).await;
        Self::attach_observation_metadata(tool_call, result);
        Self::normalize(tool_call, result)
    }

    pub(super) fn attach_observation_metadata(tool_call: &ToolCall, result: &mut ToolResult) {
        let model_content = provider_tool_result_content(tool_call, result);
        let observation = ToolObservation::from_result(tool_call, result, &model_content);
        merge_tool_result_metadata(result, "tool_observation", observation.as_metadata());
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

impl ToolObservation {
    fn from_result(tool_call: &ToolCall, result: &ToolResult, model_content: &str) -> Self {
        let data = result.data.as_ref();
        let status = observation_status(data, result);
        let files_read = observation_files_read(tool_call, data);
        let files_changed = observation_files_changed(tool_call, data, result.success);
        let command_run = observation_command(tool_call, data);
        let validation_result = observation_validation_result(tool_call, result, data);
        let permission_decision = observation_permission_decision(data);
        let checkpoint_id = observation_checkpoint_id(data);
        let artifact_path = observation_artifact_path(data);
        let recommended_next_action = observation_recommended_next_action(data);
        let mut state_updates = Vec::new();
        if !files_read.is_empty() {
            state_updates.push("files_read".to_string());
        }
        if !files_changed.is_empty() {
            state_updates.push("files_changed".to_string());
        }
        if validation_result.is_some() {
            state_updates.push("validation_result".to_string());
        }
        if permission_decision.is_some() {
            state_updates.push("permission_decision".to_string());
        }
        if checkpoint_id.is_some() {
            state_updates.push("checkpoint".to_string());
        }
        if artifact_path.is_some() {
            state_updates.push("artifact".to_string());
        }

        Self {
            schema: "tool_observation.v1".to_string(),
            tool: tool_call.name.clone(),
            call_id: tool_call.id.clone(),
            status,
            summary: observation_summary(tool_call, result, model_content),
            files_read,
            files_changed,
            command_run,
            validation_result,
            permission_decision,
            checkpoint_id,
            artifact_path,
            state_updates,
            recommended_next_action,
        }
    }

    fn as_metadata(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| {
            serde_json::json!({
                "schema": "tool_observation.v1",
                "tool": self.tool,
                "call_id": self.call_id,
                "status": self.status,
                "summary": self.summary,
                "serialization_error": true,
            })
        })
    }
}

fn observation_status(data: Option<&serde_json::Value>, result: &ToolResult) -> String {
    if result.success {
        return "success".to_string();
    }
    if let Some(decision) = data
        .and_then(|value| value.get("action_review"))
        .and_then(|value| value.get("decision"))
        .and_then(serde_json::Value::as_str)
    {
        return match decision {
            "revise" => "revised",
            "deny" => "denied",
            _ => "failed",
        }
        .to_string();
    }
    match result.error_code {
        Some(ToolErrorCode::PermissionDenied | ToolErrorCode::DangerousBlocked) => "denied",
        Some(ToolErrorCode::Timeout) => "timed_out",
        Some(ToolErrorCode::Cancelled) => "cancelled",
        _ => "failed",
    }
    .to_string()
}

fn observation_files_read(tool_call: &ToolCall, data: Option<&serde_json::Value>) -> Vec<String> {
    let mut files = Vec::new();
    if tool_call.name == "file_read" {
        collect_string_field(&mut files, data, "path");
        collect_string_field(&mut files, Some(&tool_call.arguments), "path");
    }
    if matches!(tool_call.name.as_str(), "grep" | "glob") {
        collect_string_field(&mut files, data, "path");
        collect_string_field(&mut files, Some(&tool_call.arguments), "path");
        collect_string_field(&mut files, Some(&tool_call.arguments), "include");
    }
    if matches!(tool_call.name.as_str(), "git_status" | "git_diff") {
        collect_string_field(&mut files, Some(&tool_call.arguments), "path");
    }
    dedup_strings(&mut files);
    files
}

fn observation_files_changed(
    tool_call: &ToolCall,
    data: Option<&serde_json::Value>,
    success: bool,
) -> Vec<String> {
    if !success
        || !matches!(
            tool_call.name.as_str(),
            "file_write" | "file_edit" | "file_patch"
        )
    {
        return Vec::new();
    }
    let mut files = Vec::new();
    collect_string_field(&mut files, data, "path");
    collect_string_field(&mut files, data, "resolved_path");
    collect_string_array_field(&mut files, data, "written_paths");
    if let Some(file_items) = data
        .and_then(|value| value.get("files"))
        .and_then(serde_json::Value::as_array)
    {
        for file in file_items {
            collect_string_field(&mut files, Some(file), "path");
            collect_string_field(&mut files, Some(file), "resolved_path");
        }
    }
    if let Some(file_changes) = data
        .and_then(|value| value.get("file_changes"))
        .and_then(serde_json::Value::as_array)
    {
        for change in file_changes {
            collect_string_field(&mut files, Some(change), "path");
            collect_string_field(&mut files, Some(change), "resolved_path");
        }
    }
    collect_string_field(&mut files, Some(&tool_call.arguments), "path");
    if let Some(operations) = tool_call
        .arguments
        .get("operations")
        .and_then(serde_json::Value::as_array)
    {
        for operation in operations {
            collect_string_field(&mut files, Some(operation), "path");
        }
    }
    dedup_strings(&mut files);
    files
}

fn observation_command(tool_call: &ToolCall, data: Option<&serde_json::Value>) -> Option<String> {
    let command = tool_call
        .arguments
        .get("command")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            data.and_then(|value| value.get("shell_result"))
                .and_then(|value| value.get("command"))
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            data.and_then(|value| value.get("dependency_install"))
                .and_then(|value| value.get("command"))
                .and_then(serde_json::Value::as_str)
        })
        .map(|command| safe_observation_text(command, 240));
    if command.is_some() {
        return command;
    }

    match tool_call.name.as_str() {
        "git" => {
            let action = tool_call.arguments["action"].as_str().unwrap_or("unknown");
            Some(format!("git {action}"))
        }
        "git_status" => Some(git_status_observation_command(tool_call)),
        "git_diff" => Some(git_diff_observation_command(tool_call)),
        _ => None,
    }
}

fn git_status_observation_command(tool_call: &ToolCall) -> String {
    let mut parts = vec!["git status --short".to_string()];
    if let Some(path) = tool_call.arguments["path"]
        .as_str()
        .filter(|path| !path.trim().is_empty())
    {
        parts.push("--".to_string());
        parts.push(path.trim().to_string());
    }
    parts.join(" ")
}

fn git_diff_observation_command(tool_call: &ToolCall) -> String {
    let mut parts = vec!["git diff".to_string()];
    if tool_call.arguments["cached"].as_bool().unwrap_or(false) {
        parts.push("--cached".to_string());
    }
    if tool_call.arguments["stat"].as_bool().unwrap_or(false) {
        parts.push("--stat".to_string());
    }
    if let Some(range) = tool_call.arguments["range"]
        .as_str()
        .filter(|range| !range.trim().is_empty())
    {
        parts.push(range.trim().to_string());
    }
    if let Some(path) = tool_call.arguments["path"]
        .as_str()
        .filter(|path| !path.trim().is_empty())
    {
        parts.push("--".to_string());
        parts.push(path.trim().to_string());
    }
    parts.join(" ")
}

fn observation_validation_result(
    tool_call: &ToolCall,
    result: &ToolResult,
    data: Option<&serde_json::Value>,
) -> Option<String> {
    let command = observation_command(tool_call, data)?;
    if !matches!(tool_call.name.as_str(), "bash" | "run_tests") {
        return None;
    }
    let classification = crate::tools::bash_tool::command_classifier::classify_command(&command);
    if !classification.is_safe_validation() {
        return None;
    }
    Some(if result.success { "passed" } else { "failed" }.to_string())
}

fn observation_permission_decision(data: Option<&serde_json::Value>) -> Option<String> {
    data.and_then(|value| value.get("permission_request"))
        .and_then(|request| {
            request
                .get("approved")
                .and_then(serde_json::Value::as_bool)
                .map(|approved| if approved { "approved" } else { "denied" }.to_string())
                .or_else(|| {
                    request
                        .get("rejection_feedback")
                        .and_then(serde_json::Value::as_str)
                        .map(|_| "denied".to_string())
                })
        })
        .or_else(|| {
            data.and_then(|value| value.get("action_review"))
                .and_then(|review| review.get("permission"))
                .and_then(|permission| {
                    permission
                        .get("decision")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
        })
}

fn observation_checkpoint_id(data: Option<&serde_json::Value>) -> Option<String> {
    data.and_then(|value| value.get("checkpoint"))
        .and_then(|checkpoint| checkpoint.get("id"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            data.and_then(|value| value.get("edit_preview"))
                .and_then(|preview| preview.get("checkpoint_id"))
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            data.and_then(|value| value.get("action_review"))
                .and_then(|review| review.get("checkpoint"))
                .and_then(|checkpoint| checkpoint.get("checkpoint_id"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::to_string)
}

fn observation_artifact_path(data: Option<&serde_json::Value>) -> Option<String> {
    data.and_then(|value| value.get("output_truncation"))
        .and_then(|truncation| truncation.get("stored_path"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn observation_recommended_next_action(data: Option<&serde_json::Value>) -> Option<String> {
    data.and_then(|value| value.get("recovery"))
        .and_then(|recovery| {
            recovery
                .get("recommended_action")
                .or_else(|| recovery.get("action"))
        })
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            data.and_then(|value| value.get("action_review"))
                .and_then(|review| review.get("model_recovery"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
}

fn observation_summary(tool_call: &ToolCall, result: &ToolResult, model_content: &str) -> String {
    let status = if result.success {
        "succeeded"
    } else {
        "failed"
    };
    let detail = if let Some(error) = result.error.as_deref() {
        safe_observation_text(error, 180)
    } else {
        safe_observation_text(
            model_content
                .lines()
                .find(|line| !line.trim().is_empty() && !line.starts_with("Result:"))
                .unwrap_or(status),
            180,
        )
    };
    format!("{} {}: {}", tool_call.name, status, detail)
}

fn collect_string_field(out: &mut Vec<String>, data: Option<&serde_json::Value>, key: &str) {
    if let Some(value) = data
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        out.push(value.to_string());
    }
}

fn collect_string_array_field(out: &mut Vec<String>, data: Option<&serde_json::Value>, key: &str) {
    if let Some(values) = data
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_array)
    {
        out.extend(
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string),
        );
    }
}

fn dedup_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn safe_observation_text(value: &str, max_chars: usize) -> String {
    let mut text = value.trim().chars().take(max_chars).collect::<String>();
    if value.trim().chars().count() > max_chars {
        text.push_str("...");
    }
    text
}

fn evidence_facts(tool_call: &ToolCall, result: &ToolResult) -> Vec<NormalizedEvidenceFact> {
    let mut facts = match tool_call.name.as_str() {
        "bash" | "run_tests" => bash_evidence_facts(tool_call, result),
        "start_dev_server" => vec![NormalizedEvidenceFact::Command],
        "install_dependencies" => vec![NormalizedEvidenceFact::Command],
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
        assert_eq!(normalized.observation.status, "success");
        assert_eq!(
            normalized.observation.command_run.as_deref(),
            Some("cargo test -q")
        );
        assert_eq!(
            normalized.observation.validation_result.as_deref(),
            Some("passed")
        );
        assert_eq!(
            normalized.structured_metadata["tool_observation"]["status"],
            "success"
        );
        assert!(normalized.context_policy.ledger_fact_eligible);
        assert!(!normalized.context_policy.protected_recent_tail);
    }

    #[test]
    fn attach_observation_metadata_writes_compact_result_state() {
        let mut result = ToolResult::success_with_data(
            "Edited src/app.rs",
            serde_json::json!({
                "checkpoint": {"id": "cp_1"},
                "diff": {"additions": 1, "deletions": 0}
            }),
        );
        let tool_call = ToolCall {
            id: "call_edit".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({"path": "src/app.rs"}),
        };

        ToolResultNormalizer::attach_observation_metadata(&tool_call, &mut result);

        let observation = &result.data.as_ref().unwrap()["tool_observation"];
        assert_eq!(observation["status"], "success");
        assert_eq!(observation["files_changed"][0], "src/app.rs");
        assert_eq!(observation["checkpoint_id"], "cp_1");
        assert_eq!(observation["state_updates"][0], "files_changed");
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
