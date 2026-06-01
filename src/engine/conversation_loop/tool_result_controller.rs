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
    pub(super) model_visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct ObservationEvidence {
    pub(super) kind: String,
    pub(super) source: Option<String>,
    pub(super) text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct HypothesisUpdate {
    pub(super) hypothesis: String,
    pub(super) confidence_delta: Option<i8>,
    pub(super) confidence: Option<u8>,
    pub(super) evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct ToolObservation {
    pub(super) schema: String,
    pub(super) tool: String,
    pub(super) call_id: String,
    pub(super) status: String,
    pub(super) result_kind: String,
    pub(super) summary: String,
    pub(super) key_findings: Vec<String>,
    pub(super) evidence: Vec<ObservationEvidence>,
    pub(super) impact_on_goal: Option<String>,
    pub(super) next_attention: Vec<String>,
    pub(super) files_read: Vec<String>,
    pub(super) files_changed: Vec<String>,
    pub(super) command_run: Option<String>,
    pub(super) validation_result: Option<String>,
    pub(super) permission_decision: Option<String>,
    pub(super) permission_source: Option<String>,
    pub(super) checkpoint_id: Option<String>,
    pub(super) artifact_path: Option<String>,
    pub(super) state_updates: Vec<String>,
    pub(super) recommended_next_action: Option<String>,
    pub(super) include_in_next_context: bool,
    pub(super) store_in_state: bool,
    pub(super) confidence: Option<u8>,
    pub(super) raw_result_ref: Option<String>,
    pub(super) hypothesis_updates: Vec<HypothesisUpdate>,
    pub(super) candidate_focus: Vec<String>,
    pub(super) reduced_uncertainty: bool,
    pub(super) risk_note: Option<String>,
    pub(super) failure_type: Option<String>,
    pub(super) recovery_plan_id: Option<String>,
    pub(super) recovery_kind: Option<String>,
    #[serde(default)]
    pub(super) quality_warnings: Vec<String>,
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
        let raw_model_content = provider_tool_result_content(tool_call, result);
        let observation = ToolObservation::from_result(tool_call, result, &raw_model_content);
        let mut structured_metadata = structured_metadata(tool_call, result);
        let evidence_facts = evidence_facts(tool_call, result);
        let model_visibility =
            model_visibility_for(tool_call, result, &observation, &raw_model_content);
        let model_content = model_content_for_visibility(
            result,
            &observation,
            &raw_model_content,
            model_visibility,
        );
        let context_policy = ToolResultContextPolicy::from_normalized(
            tool_call,
            result,
            &model_content,
            &structured_metadata,
            &evidence_facts,
            model_visibility,
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
        let evidence_facts = evidence_facts(tool_call, result);
        let model_visibility =
            model_visibility_for(tool_call, result, &observation, &model_content);
        let selected_model_content =
            model_content_for_visibility(result, &observation, &model_content, model_visibility);
        let context_policy = ToolResultContextPolicy::from_normalized(
            tool_call,
            result,
            &selected_model_content,
            &structured_metadata(tool_call, result),
            &evidence_facts,
            model_visibility,
        );
        merge_tool_result_metadata(result, "tool_observation", observation.as_metadata());
        merge_tool_result_metadata(
            result,
            "tool_result_context_policy",
            context_policy.as_metadata(),
        );
    }
}

impl ToolResultContextPolicy {
    fn from_normalized(
        tool_call: &ToolCall,
        result: &ToolResult,
        model_content: &str,
        structured_metadata: &serde_json::Value,
        evidence_facts: &[NormalizedEvidenceFact],
        model_visibility: &'static str,
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
            model_visibility: model_visibility.to_string(),
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
            "model_visibility": self.model_visibility,
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
        let permission_source = observation_permission_source(data);
        let checkpoint_id = observation_checkpoint_id(data);
        let artifact_path = observation_artifact_path(data);
        let raw_result_ref = artifact_path.clone();
        let recommended_next_action = observation_recommended_next_action(data);
        let result_kind = observation_result_kind(
            tool_call,
            data,
            command_run.as_deref(),
            validation_result.as_deref(),
            permission_decision.as_deref(),
        );
        let finding_context = ObservationFindingContext {
            result_kind: &result_kind,
            tool_call,
            result,
            data,
            command_run: command_run.as_deref(),
            validation_result: validation_result.as_deref(),
            files_read: &files_read,
            files_changed: &files_changed,
        };
        let key_findings = observation_key_findings(&finding_context);
        let evidence = observation_evidence(&result_kind, tool_call, result, data, model_content);
        let next_attention = observation_next_attention(
            &result_kind,
            result,
            data,
            command_run.as_deref(),
            validation_result.as_deref(),
            &files_changed,
        );
        let candidate_focus =
            observation_candidate_focus(&result_kind, data, &files_read, &files_changed);
        let hypothesis_updates = observation_hypothesis_updates(
            &result_kind,
            result,
            validation_result.as_deref(),
            &evidence,
        );
        let impact_on_goal = observation_impact_on_goal(
            &result_kind,
            result,
            validation_result.as_deref(),
            &candidate_focus,
        );
        let risk_note = observation_risk_note(&result_kind, result, command_run.as_deref());
        let (failure_type, recovery_plan_id, recovery_kind) =
            observation_recovery_metadata(data, result);
        let confidence = observation_confidence(&result_kind, result, validation_result.as_deref());
        let include_in_next_context =
            observation_include_in_next_context(&result_kind, result, &key_findings, &evidence);
        let store_in_state = !matches!(result_kind.as_str(), "generic")
            || !key_findings.is_empty()
            || !evidence.is_empty()
            || !result.success;
        let reduced_uncertainty =
            observation_reduced_uncertainty(&result_kind, result, &key_findings, &candidate_focus);
        let quality_warnings = observation_quality_warnings(ObservationQualityInput {
            result_kind: &result_kind,
            result,
            command_run: command_run.as_deref(),
            validation_result: validation_result.as_deref(),
            permission_decision: permission_decision.as_deref(),
            permission_source: permission_source.as_deref(),
            files_changed: &files_changed,
            checkpoint_id: checkpoint_id.as_deref(),
            artifact_path: artifact_path.as_deref(),
            evidence: &evidence,
            next_attention: &next_attention,
            data,
        });
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
        if permission_source.is_some() {
            state_updates.push("permission_source".to_string());
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
            result_kind,
            summary: observation_summary(tool_call, result, model_content),
            key_findings,
            evidence,
            impact_on_goal,
            next_attention,
            files_read,
            files_changed,
            command_run,
            validation_result,
            permission_decision,
            permission_source,
            checkpoint_id,
            artifact_path,
            state_updates,
            recommended_next_action,
            include_in_next_context,
            store_in_state,
            confidence,
            raw_result_ref,
            hypothesis_updates,
            candidate_focus,
            reduced_uncertainty,
            risk_note,
            failure_type,
            recovery_plan_id,
            recovery_kind,
            quality_warnings,
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

fn observation_result_kind(
    tool_call: &ToolCall,
    data: Option<&serde_json::Value>,
    command_run: Option<&str>,
    validation_result: Option<&str>,
    permission_decision: Option<&str>,
) -> String {
    match tool_call.name.as_str() {
        "file_read" => "file_read",
        "grep" | "glob" => "search",
        "run_tests" => "validation",
        "file_write" | "file_edit" | "file_patch" => "edit",
        "git_diff" => "diff",
        "install_dependencies" => "install",
        "start_dev_server" => "dev_server",
        "permission_request" => "permission",
        "bash" => {
            if validation_result.is_some() {
                "validation"
            } else if command_run.map(looks_like_diff_command).unwrap_or(false) {
                "diff"
            } else {
                "unknown_command"
            }
        }
        "git_status" => "generic",
        _ if permission_decision.is_some() => "permission",
        _ => data
            .and_then(|value| value.get("kind"))
            .and_then(serde_json::Value::as_str)
            .filter(|kind| !kind.trim().is_empty())
            .unwrap_or("generic"),
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

fn observation_permission_source(data: Option<&serde_json::Value>) -> Option<String> {
    let request = data.and_then(|value| value.get("permission_request"))?;
    request
        .get("permission_source")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            request
                .get("metadata")
                .and_then(|metadata| {
                    metadata
                        .get("resolved_permission_source")
                        .or_else(|| metadata.get("permission_source"))
                })
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
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

struct ObservationQualityInput<'a> {
    result_kind: &'a str,
    result: &'a ToolResult,
    command_run: Option<&'a str>,
    validation_result: Option<&'a str>,
    permission_decision: Option<&'a str>,
    permission_source: Option<&'a str>,
    files_changed: &'a [String],
    checkpoint_id: Option<&'a str>,
    artifact_path: Option<&'a str>,
    evidence: &'a [ObservationEvidence],
    next_attention: &'a [String],
    data: Option<&'a serde_json::Value>,
}

fn observation_quality_warnings(input: ObservationQualityInput<'_>) -> Vec<String> {
    let mut warnings = Vec::new();
    match input.result_kind {
        "validation" => {
            if input.command_run.is_none() {
                warnings.push("missing_validation_command".to_string());
            }
            if input.validation_result.is_none() {
                warnings.push("missing_validation_result".to_string());
            }
            if !input.result.success && input.evidence.is_empty() {
                warnings.push("missing_validation_failure_evidence".to_string());
            }
            if !input.result.success && input.next_attention.is_empty() {
                warnings.push("missing_validation_repair_attention".to_string());
            }
        }
        "edit" if input.result.success => {
            if input.files_changed.is_empty() {
                warnings.push("missing_changed_files".to_string());
            }
            if input.checkpoint_id.is_none() {
                warnings.push("missing_edit_checkpoint".to_string());
            }
        }
        "permission" => {
            if input.permission_decision.is_none() {
                warnings.push("missing_permission_decision".to_string());
            }
            if input.permission_source.is_none() {
                warnings.push("missing_permission_source".to_string());
            }
            if input.next_attention.is_empty() {
                warnings.push("missing_permission_recovery_attention".to_string());
            }
        }
        _ => {}
    }
    if input
        .data
        .and_then(|value| value.get("output_truncation"))
        .is_some()
        && input.artifact_path.is_none()
    {
        warnings.push("missing_truncation_artifact".to_string());
    }
    dedup_strings(&mut warnings);
    warnings
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

struct ObservationFindingContext<'a> {
    result_kind: &'a str,
    tool_call: &'a ToolCall,
    result: &'a ToolResult,
    data: Option<&'a serde_json::Value>,
    command_run: Option<&'a str>,
    validation_result: Option<&'a str>,
    files_read: &'a [String],
    files_changed: &'a [String],
}

fn observation_key_findings(context: &ObservationFindingContext<'_>) -> Vec<String> {
    let mut findings = Vec::new();
    match context.result_kind {
        "file_read" => {
            let path = context
                .files_read
                .first()
                .cloned()
                .or_else(|| {
                    context.tool_call.arguments["path"]
                        .as_str()
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "file".to_string());
            let total = context
                .data
                .and_then(|value| value.get("total_lines"))
                .and_then(serde_json::Value::as_u64);
            let displayed = context
                .data
                .and_then(|value| value.get("displayed_lines"))
                .and_then(serde_json::Value::as_u64);
            let coverage = context
                .data
                .and_then(|value| value.get("read_coverage"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("read");
            let truncated = context
                .data
                .and_then(|value| value.get("truncated"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            findings.push(match (displayed, total) {
                (Some(displayed), Some(total)) => format!(
                    "Read {path} with {displayed}/{total} line(s) visible ({coverage}{}).",
                    if truncated { ", truncated" } else { "" }
                ),
                _ => format!("Read {path}."),
            });
            if context
                .data
                .and_then(|value| value.get("unchanged"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                findings.push(format!(
                    "{path} was unchanged since the previous full read."
                ));
            }
        }
        "search" => {
            let total = context
                .data
                .and_then(|value| value.get("total_matches"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let top_files = top_search_files(context.data, 4);
            if total == 0 {
                findings.push("Search returned no matches.".to_string());
            } else {
                findings.push(format!(
                    "Search found {total} match(es) across {} file(s).",
                    top_files.len().max(1)
                ));
                if !top_files.is_empty() {
                    findings.push(format!("Top matching files: {}.", top_files.join(", ")));
                }
            }
        }
        "validation" => {
            let command = context.command_run.unwrap_or("validation command");
            let status = context
                .validation_result
                .unwrap_or(if context.result.success {
                    "passed"
                } else {
                    "failed"
                });
            findings.push(format!("Validation `{command}` {status}."));
            let failed_tests = extract_failed_tests_from_text(&result_output_body(context.result));
            if !failed_tests.is_empty() {
                findings.push(format!(
                    "Failed tests: {}.",
                    failed_tests
                        .into_iter()
                        .take(4)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if let Some(line) = first_diagnostic_line(&result_output_body(context.result)) {
                findings.push(format!("First diagnostic: {line}"));
            }
        }
        "edit" => {
            let target = if context.files_changed.is_empty() {
                context
                    .tool_call
                    .arguments
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("target file")
                    .to_string()
            } else {
                context.files_changed.join(", ")
            };
            if context.result.success {
                findings.push(format!("Changed {target}."));
            } else {
                findings.push(format!(
                    "Attempted to change {target}, but the edit failed."
                ));
            }
            if let Some(diff) = context.data.and_then(|value| value.get("diff")) {
                let additions = diff.get("additions").and_then(serde_json::Value::as_u64);
                let deletions = diff.get("deletions").and_then(serde_json::Value::as_u64);
                if additions.is_some() || deletions.is_some() {
                    findings.push(format!(
                        "Diff summary: +{} -{}.",
                        additions.unwrap_or(0),
                        deletions.unwrap_or(0)
                    ));
                }
            }
            if let Some(replacements) = context
                .data
                .and_then(|value| value.get("replacements"))
                .and_then(serde_json::Value::as_u64)
            {
                findings.push(format!("Applied {replacements} replacement(s)."));
            }
        }
        "diff" => {
            let changed = !context.result.content.trim().is_empty() && context.result.success;
            findings.push(format!(
                "Diff inspection {} changes.",
                if changed { "found" } else { "found no" }
            ));
        }
        "install" | "dev_server" => {
            let command = context
                .command_run
                .unwrap_or(context.tool_call.name.as_str());
            findings.push(format!(
                "{} `{command}` {}.",
                if context.result_kind == "install" {
                    "Install command"
                } else {
                    "Dev-server command"
                },
                if context.result.success {
                    "succeeded"
                } else {
                    "failed"
                }
            ));
        }
        "unknown_command" => {
            let command = context.command_run.unwrap_or("bash command");
            findings.push(format!(
                "Command `{command}` {} but was not classified as validation.",
                if context.result.success {
                    "completed"
                } else {
                    "failed"
                }
            ));
        }
        _ => {
            if !context.result.success {
                findings.push(format!("{} failed.", context.tool_call.name));
            }
        }
    }
    dedup_strings(&mut findings);
    findings
}

fn observation_evidence(
    result_kind: &str,
    tool_call: &ToolCall,
    result: &ToolResult,
    data: Option<&serde_json::Value>,
    model_content: &str,
) -> Vec<ObservationEvidence> {
    let mut evidence = Vec::new();
    match result_kind {
        "search" => {
            if let Some(matches) = data
                .and_then(|value| value.get("matches"))
                .and_then(serde_json::Value::as_array)
            {
                for item in matches.iter().take(3) {
                    let file = item
                        .get("file")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("match");
                    let line = item
                        .get("line")
                        .and_then(serde_json::Value::as_u64)
                        .map(|line| line.to_string())
                        .unwrap_or_else(|| "?".to_string());
                    let content = item
                        .get("content")
                        .or_else(|| item.get("raw_line"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    push_evidence(
                        &mut evidence,
                        "match",
                        Some(format!("{file}:{line}")),
                        content,
                    );
                }
            }
        }
        "validation" => {
            let body = result_output_body(result);
            for line in diagnostic_lines(&body).into_iter().take(4) {
                push_evidence(&mut evidence, "diagnostic", None, &line);
            }
        }
        "edit" | "diff" => {
            if let Some(diff) = data
                .and_then(|value| value.get("diff"))
                .and_then(|diff| diff.get("unified_diff"))
                .and_then(serde_json::Value::as_str)
            {
                push_evidence(&mut evidence, "diff", None, diff);
            } else if result_kind == "diff" {
                push_evidence(&mut evidence, "diff", None, &result_output_body(result));
            }
        }
        "unknown_command" => {
            push_evidence(
                &mut evidence,
                "output_excerpt",
                None,
                &result_output_body(result),
            );
        }
        _ if !result.success => {
            let source = Some(format!("{}:{}", tool_call.name, tool_call.id));
            push_evidence(&mut evidence, "error", source, &result_output_body(result));
        }
        _ => {}
    }
    if evidence.is_empty() && result.content.chars().count() <= 800 {
        let body = model_content
            .lines()
            .filter(|line| !line.starts_with("Result:"))
            .collect::<Vec<_>>()
            .join("\n");
        if !body.trim().is_empty() && !matches!(result_kind, "file_read") {
            push_evidence(&mut evidence, "output_excerpt", None, &body);
        }
    }
    evidence.truncate(5);
    evidence
}

fn observation_next_attention(
    result_kind: &str,
    result: &ToolResult,
    data: Option<&serde_json::Value>,
    command_run: Option<&str>,
    validation_result: Option<&str>,
    files_changed: &[String],
) -> Vec<String> {
    let mut next = Vec::new();
    match result_kind {
        "search" => {
            for file in top_search_files(data, 3) {
                next.push(format!(
                    "Inspect {file} if it is relevant to the current goal."
                ));
            }
            if data
                .and_then(|value| value.get("total_matches"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                next.push(
                    "Try a simpler query or inspect the nearest known file manually.".to_string(),
                );
            }
        }
        "validation" if validation_result == Some("failed") || !result.success => {
            if let Some(command) = command_run {
                next.push(format!(
                    "Use the failed `{command}` evidence to choose the next repair."
                ));
                next.push(format!("Rerun `{command}` after the next patch."));
            } else {
                next.push(
                    "Use the validation failure evidence to choose the next repair.".to_string(),
                );
            }
        }
        "edit" if result.success => {
            let target = if files_changed.is_empty() {
                "the changed file".to_string()
            } else {
                files_changed.join(", ")
            };
            next.push(format!("Verify the change affecting {target}."));
        }
        "edit" => {
            next.push("Repair the edit inputs before retrying the mutation.".to_string());
        }
        "permission" => {
            if let Some(recovery) = data
                .and_then(|value| value.get("permission_request"))
                .and_then(|request| request.get("recovery_feedback"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                next.push(safe_observation_text(recovery, 240));
            } else {
                next.push(
                    "Ask for approval or choose a lower-risk alternative before retrying."
                        .to_string(),
                );
            }
        }
        "unknown_command" => {
            next.push("Do not treat this command as validation evidence unless a runtime classifier proves it.".to_string());
            next.push("Inspect possible side effects before relying on the output.".to_string());
        }
        _ => {}
    }
    dedup_strings(&mut next);
    next
}

fn observation_candidate_focus(
    result_kind: &str,
    data: Option<&serde_json::Value>,
    files_read: &[String],
    files_changed: &[String],
) -> Vec<String> {
    let mut focus = Vec::new();
    focus.extend(files_read.iter().cloned());
    focus.extend(files_changed.iter().cloned());
    if result_kind == "search" {
        focus.extend(top_search_files(data, 5));
    }
    dedup_strings(&mut focus);
    focus
}

fn observation_hypothesis_updates(
    result_kind: &str,
    result: &ToolResult,
    validation_result: Option<&str>,
    evidence: &[ObservationEvidence],
) -> Vec<HypothesisUpdate> {
    if result_kind != "validation" || validation_result != Some("failed") || result.success {
        return Vec::new();
    }
    let evidence_text = evidence
        .iter()
        .take(3)
        .map(|item| item.text.clone())
        .collect::<Vec<_>>();
    vec![HypothesisUpdate {
        hypothesis: "current implementation does not satisfy the latest validation".to_string(),
        confidence_delta: Some(25),
        confidence: Some(80),
        evidence: evidence_text,
    }]
}

fn observation_impact_on_goal(
    result_kind: &str,
    result: &ToolResult,
    validation_result: Option<&str>,
    candidate_focus: &[String],
) -> Option<String> {
    match result_kind {
        "validation" if validation_result == Some("failed") || !result.success => {
            Some("Narrows the next step to repairing the reported validation failure.".to_string())
        }
        "validation" => {
            Some("Provides runtime evidence that the current goal may be satisfied.".to_string())
        }
        "search" if !candidate_focus.is_empty() => Some(format!(
            "Reduces search space to {} candidate file(s).",
            candidate_focus.len()
        )),
        "edit" if result.success => {
            Some("Moves the task from editing toward verification.".to_string())
        }
        "file_read" => Some("Adds source context for deciding the next action.".to_string()),
        "unknown_command" => Some(
            "Provides raw terminal output but weak goal evidence until classified.".to_string(),
        ),
        _ => None,
    }
}

fn observation_risk_note(
    result_kind: &str,
    result: &ToolResult,
    command_run: Option<&str>,
) -> Option<String> {
    if result_kind != "unknown_command" {
        return None;
    }
    Some(format!(
        "Command `{}` is not classified as validation; check for side effects before using it as proof.",
        command_run.unwrap_or(if result.success { "completed command" } else { "failed command" })
    ))
}

fn observation_recovery_metadata(
    data: Option<&serde_json::Value>,
    result: &ToolResult,
) -> (Option<String>, Option<String>, Option<String>) {
    let recovery = data.and_then(|value| value.get("recovery"));
    let failure_type = recovery
        .and_then(|value| value.get("failure_type"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            result.error_code.as_ref().map(|code| match code {
                ToolErrorCode::InvalidParams => "invalid_params".to_string(),
                ToolErrorCode::PermissionDenied | ToolErrorCode::DangerousBlocked => {
                    "permission_block".to_string()
                }
                ToolErrorCode::Timeout => "timeout".to_string(),
                ToolErrorCode::NotFound => "target_not_found".to_string(),
                ToolErrorCode::Unavailable => "unavailable".to_string(),
                ToolErrorCode::ExecutionFailed => "execution_failed".to_string(),
                ToolErrorCode::Cancelled => "cancelled".to_string(),
                ToolErrorCode::Success => "success".to_string(),
                ToolErrorCode::Unknown => "unknown".to_string(),
            })
        });
    let recovery_plan_id = recovery
        .and_then(|value| value.get("plan_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let recovery_kind = recovery
        .and_then(|value| value.get("recovery_kind"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    (failure_type, recovery_plan_id, recovery_kind)
}

fn observation_confidence(
    result_kind: &str,
    result: &ToolResult,
    validation_result: Option<&str>,
) -> Option<u8> {
    Some(match result_kind {
        "validation" if validation_result.is_some() => {
            if result.success {
                95
            } else {
                90
            }
        }
        "edit" => {
            if result.success {
                90
            } else {
                75
            }
        }
        "search" => 85,
        "file_read" => 85,
        "diff" => 80,
        "unknown_command" => 45,
        "permission" => 90,
        _ => 60,
    })
}

fn observation_include_in_next_context(
    result_kind: &str,
    result: &ToolResult,
    key_findings: &[String],
    evidence: &[ObservationEvidence],
) -> bool {
    !result.success
        || !key_findings.is_empty()
        || !evidence.is_empty()
        || matches!(
            result_kind,
            "search" | "validation" | "edit" | "diff" | "unknown_command"
        )
}

fn observation_reduced_uncertainty(
    result_kind: &str,
    result: &ToolResult,
    key_findings: &[String],
    candidate_focus: &[String],
) -> bool {
    !key_findings.is_empty()
        && (result.success || !candidate_focus.is_empty())
        && matches!(
            result_kind,
            "file_read" | "search" | "validation" | "edit" | "diff"
        )
}

fn model_visibility_for(
    tool_call: &ToolCall,
    result: &ToolResult,
    observation: &ToolObservation,
    raw_model_content: &str,
) -> &'static str {
    let raw_chars = raw_model_content.chars().count();
    if observation.raw_result_ref.is_some() && raw_chars > 8_000 {
        return "artifact_only";
    }
    match observation.result_kind.as_str() {
        "search" => {
            let total_matches = result
                .data
                .as_ref()
                .and_then(|data| data.get("total_matches"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let truncated = result
                .data
                .as_ref()
                .and_then(|data| data.get("truncated"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if total_matches > 8 || truncated || raw_chars > 1_200 {
                "observation"
            } else {
                "full_raw"
            }
        }
        "validation" if !result.success || raw_chars > 2_000 => "raw_excerpt",
        "install" | "dev_server" => "observation",
        "unknown_command" if raw_chars > 1_200 || !result.success => "raw_excerpt",
        "diff" if raw_chars > 2_000 => "raw_excerpt",
        "edit" if !result.success => "raw_excerpt",
        _ if tool_call.name == "run_tests" && raw_chars > 1_200 => "raw_excerpt",
        _ => "full_raw",
    }
}

fn model_content_for_visibility(
    result: &ToolResult,
    observation: &ToolObservation,
    raw_model_content: &str,
    model_visibility: &str,
) -> String {
    if model_visibility == "full_raw" {
        return raw_model_content.to_string();
    }
    let label = if result.success { "OK" } else { "ERROR" };
    let mut out = format!(
        "Result: {label}\nObservation ({kind}): {summary}\nStatus: {status}",
        kind = observation.result_kind,
        summary = observation.summary,
        status = observation.status
    );
    append_lines(&mut out, "Key findings", &observation.key_findings);
    if !observation.evidence.is_empty() {
        out.push_str("\nEvidence:");
        for item in observation.evidence.iter().take(5) {
            let source = item
                .source
                .as_deref()
                .map(|source| format!(" {source}:"))
                .unwrap_or_default();
            out.push_str(&format!("\n- [{}]{} {}", item.kind, source, item.text));
        }
    }
    append_lines(&mut out, "Next attention", &observation.next_attention);
    if let Some(impact) = observation.impact_on_goal.as_deref() {
        out.push_str("\nImpact on goal: ");
        out.push_str(impact);
    }
    if let Some(risk_note) = observation.risk_note.as_deref() {
        out.push_str("\nRisk note: ");
        out.push_str(risk_note);
    }
    if let Some(permission_source) = observation.permission_source.as_deref() {
        out.push_str("\nPermission source: ");
        out.push_str(permission_source);
    }
    if let Some(failure_type) = observation.failure_type.as_deref() {
        out.push_str("\nFailure type: ");
        out.push_str(failure_type);
    }
    if let Some(recovery_kind) = observation.recovery_kind.as_deref() {
        out.push_str("\nRecovery kind: ");
        out.push_str(recovery_kind);
    }
    if observation.raw_result_ref.is_some() {
        out.push_str("\nRaw result stored outside provider-visible context; use targeted follow-up tools if more detail is needed.");
    }
    append_lines(&mut out, "Observer warnings", &observation.quality_warnings);
    if model_visibility == "raw_excerpt" {
        let excerpt = safe_observation_text(&result_output_from_provider(raw_model_content), 1_600);
        if !excerpt.trim().is_empty() {
            out.push_str("\nRaw excerpt:\n");
            out.push_str(&excerpt);
        }
    }
    out
}

fn append_lines(out: &mut String, title: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    out.push('\n');
    out.push_str(title);
    out.push(':');
    for line in lines.iter().take(5) {
        out.push_str("\n- ");
        out.push_str(line);
    }
}

fn top_search_files(data: Option<&serde_json::Value>, limit: usize) -> Vec<String> {
    let mut files = Vec::new();
    if let Some(matches) = data
        .and_then(|value| value.get("matches"))
        .and_then(serde_json::Value::as_array)
    {
        for item in matches {
            if let Some(file) = item.get("file").and_then(serde_json::Value::as_str) {
                files.push(file.to_string());
            } else if let Some(file) = item
                .get("resolved_file")
                .and_then(serde_json::Value::as_str)
            {
                files.push(file.to_string());
            }
        }
    }
    dedup_strings(&mut files);
    files.truncate(limit);
    files
}

fn push_evidence(
    evidence: &mut Vec<ObservationEvidence>,
    kind: &str,
    source: Option<String>,
    text: &str,
) {
    let text = safe_observation_text(text, 360);
    if text.trim().is_empty() {
        return;
    }
    if evidence
        .iter()
        .any(|item| item.kind == kind && item.source == source && item.text == text)
    {
        return;
    }
    evidence.push(ObservationEvidence {
        kind: kind.to_string(),
        source,
        text,
    });
}

fn result_output_body(result: &ToolResult) -> String {
    if !result.content.trim().is_empty() {
        result.content.clone()
    } else {
        result.error.clone().unwrap_or_default()
    }
}

fn result_output_from_provider(model_content: &str) -> String {
    model_content
        .lines()
        .filter(|line| !line.starts_with("Result:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn diagnostic_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if trimmed.ends_with("--- FAILED")
            || trimmed.starts_with("---- ")
            || trimmed.starts_with("FAIL")
            || trimmed.starts_with("FAILED")
            || trimmed.starts_with("error:")
            || trimmed.starts_with("error[")
            || lower.contains("panicked at")
            || lower.contains("expected")
            || lower.contains("received")
            || lower.contains("assertion")
        {
            push_unique(&mut lines, safe_observation_text(trimmed, 280));
        }
        if lines.len() >= 8 {
            break;
        }
    }
    lines
}

fn first_diagnostic_line(text: &str) -> Option<String> {
    diagnostic_lines(text).into_iter().next()
}

fn extract_failed_tests_from_text(text: &str) -> Vec<String> {
    let mut tests = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with("--- FAILED") {
            let name = trimmed
                .trim_end_matches("--- FAILED")
                .trim()
                .strip_prefix("test ")
                .unwrap_or_else(|| trimmed.trim_end_matches("--- FAILED").trim())
                .to_string();
            push_unique(&mut tests, name);
        } else if let Some(rest) = trimmed.strip_prefix("---- ") {
            if let Some((name, _)) = rest.split_once(" stdout ----") {
                push_unique(&mut tests, name.trim().to_string());
            }
        } else if let Some((_, name)) = trimmed.split_once("test ") {
            if let Some((test_name, status)) = name.rsplit_once(" ... ") {
                if status.trim() == "FAILED" {
                    push_unique(&mut tests, test_name.trim().to_string());
                }
            }
        }
    }
    tests
}

fn looks_like_diff_command(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    lower.starts_with("git diff")
        || lower.starts_with("git --no-pager diff")
        || lower.contains(" git diff ")
        || lower.contains(" git --no-pager diff ")
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

fn push_unique(values: &mut Vec<String>, value: String) {
    if value.trim().is_empty() || values.contains(&value) {
        return;
    }
    values.push(value);
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
        assert_eq!(
            result.data.as_ref().unwrap()["tool_result_context_policy"]["model_visibility"],
            "full_raw"
        );
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
    fn observes_failed_validation_with_findings_evidence_and_repair_attention() {
        let result = ToolResult::error_with_content(
            "cargo test failed",
            "running 2 tests\n\
             test auth::login --- FAILED\n\
             ---- auth::session stdout ----\n\
             thread 'auth::session' panicked at src/auth/session.rs:42: token missing\n\
             error[E0425]: cannot find value `token` in this scope",
        );
        let normalized = ToolResultNormalizer::normalize(&tool_call("bash"), &result);

        assert_eq!(normalized.observation.result_kind, "validation");
        assert_eq!(
            normalized.observation.validation_result.as_deref(),
            Some("failed")
        );
        assert!(normalized
            .observation
            .key_findings
            .iter()
            .any(|finding| finding.contains("Failed tests: auth::login")));
        assert!(normalized
            .observation
            .evidence
            .iter()
            .any(|evidence| evidence.text.contains("error[E0425]")));
        assert!(normalized
            .observation
            .next_attention
            .iter()
            .any(|item| item.contains("Rerun `cargo test -q`")));
        assert!(!normalized.observation.hypothesis_updates.is_empty());
        assert_eq!(normalized.context_policy.model_visibility, "raw_excerpt");
        assert!(normalized
            .model_content
            .contains("Observation (validation)"));
        assert!(normalized.model_content.contains("Raw excerpt:"));
    }

    #[test]
    fn observes_noisy_search_as_observation_first_with_top_matches() {
        let matches = (1..=12)
            .map(|line| {
                serde_json::json!({
                    "file": if line <= 6 { "src/auth/login.rs" } else { "src/auth/session.rs" },
                    "line": line,
                    "content": format!("fn match_{line}() {{}}"),
                })
            })
            .collect::<Vec<_>>();
        let result = ToolResult::success_with_data(
            (1..=12)
                .map(|line| format!("{line:4}: fn match_{line}() {{}}"))
                .collect::<Vec<_>>()
                .join("\n"),
            serde_json::json!({
                "pattern": "match_",
                "path": "src",
                "kind": "search",
                "total_matches": 12,
                "truncated": false,
                "matches": matches,
            }),
        );
        let normalized = ToolResultNormalizer::normalize(
            &ToolCall {
                id: "call_search".to_string(),
                name: "grep".to_string(),
                arguments: serde_json::json!({"pattern": "match_", "path": "src"}),
            },
            &result,
        );

        assert_eq!(normalized.observation.result_kind, "search");
        assert!(normalized
            .observation
            .key_findings
            .iter()
            .any(|finding| finding.contains("Search found 12 match")));
        assert!(normalized
            .observation
            .candidate_focus
            .contains(&"src/auth/login.rs".to_string()));
        assert_eq!(normalized.context_policy.model_visibility, "observation");
        assert!(normalized.model_content.contains("Observation (search)"));
        assert!(!normalized.model_content.contains("match_12"));
    }

    #[test]
    fn observes_successful_edit_with_diff_summary_and_validation_attention() {
        let result = ToolResult::success_with_data(
            "File edited successfully: src/app.rs (1 replacement(s))",
            serde_json::json!({
                "path": "src/app.rs",
                "checkpoint": {"id": "cp_1"},
                "replacements": 1,
                "diff": {
                    "additions": 2,
                    "deletions": 1,
                    "changed_line_start": 10,
                    "changed_line_end": 12,
                    "unified_diff": "--- a/src/app.rs\n+++ b/src/app.rs\n-old\n+new"
                }
            }),
        );
        let normalized = ToolResultNormalizer::normalize(
            &ToolCall {
                id: "call_edit".to_string(),
                name: "file_edit".to_string(),
                arguments: serde_json::json!({"path": "src/app.rs"}),
            },
            &result,
        );

        assert_eq!(normalized.observation.result_kind, "edit");
        assert_eq!(normalized.observation.files_changed, vec!["src/app.rs"]);
        assert!(normalized
            .observation
            .key_findings
            .iter()
            .any(|finding| finding.contains("Diff summary: +2 -1")));
        assert!(normalized
            .observation
            .next_attention
            .iter()
            .any(|item| item.contains("Verify the change")));
        assert_eq!(
            normalized.observation.checkpoint_id.as_deref(),
            Some("cp_1")
        );
    }

    #[test]
    fn observes_unknown_bash_without_validation_evidence() {
        let result = ToolResult::error_with_content(
            "custom command failed",
            "custom tool wrote partial output before failing",
        );
        let normalized = ToolResultNormalizer::normalize(
            &ToolCall {
                id: "call_unknown".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "custom-tool --maybe-mutates"}),
            },
            &result,
        );

        assert_eq!(normalized.observation.result_kind, "unknown_command");
        assert_eq!(normalized.observation.validation_result, None);
        assert!(normalized.observation.risk_note.is_some());
        assert_eq!(
            normalized.evidence_facts,
            vec![NormalizedEvidenceFact::Command]
        );
        assert_eq!(normalized.context_policy.model_visibility, "raw_excerpt");
        assert!(normalized
            .model_content
            .contains("not classified as validation"));
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
                "permission_source": "hook_deny",
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
        assert_eq!(
            normalized.observation.permission_source.as_deref(),
            Some("hook_deny")
        );
        assert!(normalized
            .observation
            .next_attention
            .iter()
            .any(|item| item.contains("Ask for approval")));
        assert!(normalized.observation.quality_warnings.is_empty());
    }
}
