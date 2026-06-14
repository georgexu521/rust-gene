use crate::services::api::ToolCall;
use crate::session_store::{LearningEventRecord, SessionStore};
use crate::tools::ToolResult;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

pub const CONTEXT_LEDGER_FILE_READ_KIND: &str = "context_ledger.file_read";
pub const CONTEXT_LEDGER_BASH_READ_KIND: &str = "context_ledger.bash_read";
pub const CONTEXT_LEDGER_FILE_EDIT_KIND: &str = "context_ledger.file_edit";
pub const CONTEXT_LEDGER_DIFF_KIND: &str = "context_ledger.diff";
pub const CONTEXT_LEDGER_VALIDATION_KIND: &str = "context_ledger.validation";
pub const CONTEXT_LEDGER_USER_CONFIRMATION_KIND: &str = "context_ledger.user_confirmation";
pub const CONTEXT_LEDGER_TOOL_OBSERVATION_KIND: &str = "context_ledger.tool_observation";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileReadLedgerEntry {
    pub path: String,
    pub resolved_path: String,
    pub content_hash: String,
    #[serde(default)]
    pub content_preview: Option<String>,
    pub size_bytes: u64,
    pub total_lines: usize,
    pub displayed_lines: usize,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub targeted_read: bool,
    pub truncated: bool,
    pub mtime_unix_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEditLedgerEntry {
    pub tool: String,
    pub paths: Vec<String>,
    pub resolved_paths: Vec<String>,
    pub success: bool,
    pub file_count: usize,
    pub bytes_written: u64,
    pub replacements: Option<u64>,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub changed_line_start: Option<u64>,
    pub changed_line_end: Option<u64>,
    pub diff_hash: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffLedgerEntry {
    pub tool: String,
    pub action: Option<String>,
    pub command: Option<String>,
    pub path: Option<String>,
    pub success: bool,
    pub changed: bool,
    pub output_hash: String,
    pub output_chars: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationLedgerEntry {
    pub tool: String,
    pub command: String,
    pub cwd: Option<String>,
    pub success: bool,
    pub exit_code: Option<i64>,
    pub command_kind: String,
    pub category: String,
    pub validation_family: Option<String>,
    pub safe_for_closeout: bool,
    pub output_hash: String,
    pub output_chars: usize,
    pub timed_out: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserConfirmationLedgerEntry {
    pub tool: String,
    pub approved: bool,
    pub kind: Option<String>,
    pub request_id: Option<String>,
    pub patterns: Vec<String>,
    pub allowed_always_rules: Vec<String>,
    pub risk_level: Option<String>,
    pub decision: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolObservationLedgerEntry {
    pub tool: String,
    pub call_id: String,
    pub status: String,
    #[serde(default)]
    pub result_kind: String,
    pub summary: String,
    #[serde(default)]
    pub key_findings: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub impact_on_goal: Option<String>,
    #[serde(default)]
    pub next_attention: Vec<String>,
    pub files_read: Vec<String>,
    pub files_changed: Vec<String>,
    pub command_run: Option<String>,
    pub validation_result: Option<String>,
    pub permission_decision: Option<String>,
    #[serde(default)]
    pub permission_source: Option<String>,
    pub checkpoint_id: Option<String>,
    pub artifact_path: Option<String>,
    #[serde(default)]
    pub quality_warnings: Vec<String>,
    pub state_updates: Vec<String>,
    pub recommended_next_action: Option<String>,
    #[serde(default = "default_true")]
    pub include_in_next_context: bool,
    #[serde(default = "default_true")]
    pub store_in_state: bool,
    #[serde(default)]
    pub confidence: Option<u8>,
    #[serde(default)]
    pub raw_result_ref: Option<String>,
    #[serde(default)]
    pub hypothesis_updates: Vec<String>,
    #[serde(default)]
    pub candidate_focus: Vec<String>,
    #[serde(default)]
    pub reduced_uncertainty: bool,
    #[serde(default)]
    pub risk_note: Option<String>,
    #[serde(default)]
    pub failure_type: Option<String>,
    #[serde(default)]
    pub recovery_plan_id: Option<String>,
    #[serde(default)]
    pub recovery_kind: Option<String>,
    #[serde(default)]
    pub action_stage: Option<String>,
    #[serde(default)]
    pub action_value: Option<u8>,
    #[serde(default)]
    pub action_risk: Option<u8>,
    #[serde(default)]
    pub action_uncertainty_reduction: Option<u8>,
    #[serde(default)]
    pub action_cost: Option<u8>,
    #[serde(default)]
    pub action_reversibility: Option<u8>,
    #[serde(default)]
    pub action_scope_fit: Option<u8>,
    #[serde(default)]
    pub action_score: Option<i16>,
    #[serde(default)]
    pub action_formula_stage: Option<String>,
    #[serde(default)]
    pub action_formula_version: Option<String>,
    #[serde(default)]
    pub action_review_decision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextLedgerEntry {
    FileEdit(FileEditLedgerEntry),
    Diff(DiffLedgerEntry),
    Validation(ValidationLedgerEntry),
    UserConfirmation(UserConfirmationLedgerEntry),
    ToolObservation(Box<ToolObservationLedgerEntry>),
}

#[derive(Debug, Clone)]
pub struct FileReadLedgerInput<'a> {
    pub session_id: &'a str,
    pub path: &'a str,
    pub resolved_path: &'a str,
    pub content_hash: &'a str,
    pub content_preview: Option<&'a str>,
    pub size_bytes: u64,
    pub total_lines: usize,
    pub displayed_lines: usize,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub targeted_read: bool,
    pub truncated: bool,
    pub mtime: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub struct BashReadLedgerInput<'a> {
    pub session_id: &'a str,
    pub command: &'a str,
    pub cwd: &'a str,
    pub category: &'a str,
    pub exit_code: i32,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub output_hash: &'a str,
    pub timed_out: bool,
}

pub fn record_file_read(store: &SessionStore, input: &FileReadLedgerInput<'_>) {
    let entry = FileReadLedgerEntry {
        path: input.path.to_string(),
        resolved_path: input.resolved_path.to_string(),
        content_hash: input.content_hash.to_string(),
        content_preview: input
            .content_preview
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        size_bytes: input.size_bytes,
        total_lines: input.total_lines,
        displayed_lines: input.displayed_lines,
        line_start: input.line_start,
        line_end: input.line_end,
        targeted_read: input.targeted_read,
        truncated: input.truncated,
        mtime_unix_secs: input
            .mtime
            .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs()),
    };

    let summary = if input.targeted_read {
        format!(
            "Read {} lines {}-{} from {}",
            input.displayed_lines,
            input.line_start.unwrap_or(0),
            input.line_end.unwrap_or(0),
            display_path(input.path, input.resolved_path)
        )
    } else {
        format!(
            "Read {} lines from {}",
            input.total_lines,
            display_path(input.path, input.resolved_path)
        )
    };

    let payload = json!({
        "path": entry.path,
        "resolved_path": entry.resolved_path,
        "content_hash": entry.content_hash,
        "content_preview": entry.content_preview,
        "size_bytes": entry.size_bytes,
        "total_lines": entry.total_lines,
        "displayed_lines": entry.displayed_lines,
        "line_start": entry.line_start,
        "line_end": entry.line_end,
        "targeted_read": entry.targeted_read,
        "truncated": entry.truncated,
        "mtime_unix_secs": entry.mtime_unix_secs,
    });

    if let Err(err) = store.add_learning_event(
        input.session_id,
        CONTEXT_LEDGER_FILE_READ_KIND,
        "file_read",
        &summary,
        1.0,
        &payload,
    ) {
        warn!(
            session_id = input.session_id,
            path = input.resolved_path,
            error = %err,
            "failed to persist file read context ledger entry"
        );
    }
}

pub fn record_bash_read(store: &SessionStore, input: &BashReadLedgerInput<'_>) {
    let summary = format!(
        "Ran read-only bash {} in {} with exit {}",
        compact_command(input.command),
        input.cwd,
        input.exit_code
    );
    let payload = json!({
        "command": input.command,
        "cwd": input.cwd,
        "category": input.category,
        "exit_code": input.exit_code,
        "stdout_bytes": input.stdout_bytes,
        "stderr_bytes": input.stderr_bytes,
        "output_hash": input.output_hash,
        "timed_out": input.timed_out,
    });

    if let Err(err) = store.add_learning_event(
        input.session_id,
        CONTEXT_LEDGER_BASH_READ_KIND,
        "bash",
        &summary,
        if input.exit_code == 0 { 1.0 } else { 0.7 },
        &payload,
    ) {
        warn!(
            session_id = input.session_id,
            command = input.command,
            error = %err,
            "failed to persist bash read context ledger entry"
        );
    }
}

pub fn record_tool_context_evidence(
    store: &SessionStore,
    session_id: &str,
    tool_call: &ToolCall,
    result: &ToolResult,
) -> usize {
    let mut recorded = 0;

    for entry in tool_context_evidence_entries(tool_call, result) {
        recorded += persist_context_ledger_entry(ContextLedgerPersistRequest {
            store,
            session_id,
            kind: entry.kind(),
            source: entry.source(),
            summary: entry.summary(),
            confidence: entry.confidence(),
            entry: &entry.payload(),
            label: entry.label(),
        });
    }

    recorded
}

pub fn tool_context_evidence_entries(
    tool_call: &ToolCall,
    result: &ToolResult,
) -> Vec<ContextLedgerEntry> {
    let mut entries = Vec::new();
    if let Some(entry) = file_edit_entry_from_tool_result(tool_call, result) {
        entries.push(ContextLedgerEntry::FileEdit(entry));
    }
    if let Some(entry) = diff_entry_from_tool_result(tool_call, result) {
        entries.push(ContextLedgerEntry::Diff(entry));
    }
    if let Some(entry) = validation_entry_from_tool_result(tool_call, result) {
        entries.push(ContextLedgerEntry::Validation(entry));
    }
    if let Some(entry) = user_confirmation_entry_from_tool_result(tool_call, result) {
        entries.push(ContextLedgerEntry::UserConfirmation(entry));
    }
    if let Some(entry) = tool_observation_entry_from_tool_result(tool_call, result) {
        entries.push(ContextLedgerEntry::ToolObservation(Box::new(entry)));
    }
    entries
}

impl ContextLedgerEntry {
    fn kind(&self) -> &'static str {
        match self {
            Self::FileEdit(_) => CONTEXT_LEDGER_FILE_EDIT_KIND,
            Self::Diff(_) => CONTEXT_LEDGER_DIFF_KIND,
            Self::Validation(_) => CONTEXT_LEDGER_VALIDATION_KIND,
            Self::UserConfirmation(_) => CONTEXT_LEDGER_USER_CONFIRMATION_KIND,
            Self::ToolObservation(_) => CONTEXT_LEDGER_TOOL_OBSERVATION_KIND,
        }
    }

    fn source(&self) -> &str {
        match self {
            Self::FileEdit(entry) => &entry.tool,
            Self::Diff(entry) => &entry.tool,
            Self::Validation(entry) => &entry.tool,
            Self::UserConfirmation(entry) => &entry.tool,
            Self::ToolObservation(entry) => &entry.tool,
        }
    }

    fn summary(&self) -> &str {
        match self {
            Self::FileEdit(entry) => &entry.summary,
            Self::Diff(entry) => &entry.summary,
            Self::Validation(entry) => &entry.summary,
            Self::UserConfirmation(entry) => &entry.summary,
            Self::ToolObservation(entry) => &entry.summary,
        }
    }

    fn confidence(&self) -> f64 {
        match self {
            Self::FileEdit(entry) => {
                if entry.success {
                    1.0
                } else {
                    0.75
                }
            }
            Self::Diff(entry) => {
                if entry.success {
                    1.0
                } else {
                    0.75
                }
            }
            Self::Validation(entry) => {
                if entry.success {
                    1.0
                } else {
                    0.85
                }
            }
            Self::UserConfirmation(entry) => {
                if entry.approved {
                    1.0
                } else {
                    0.9
                }
            }
            Self::ToolObservation(entry) => match entry.status.as_str() {
                _ if entry.confidence.is_some() => {
                    f64::from(entry.confidence.unwrap_or_default()) / 100.0
                }
                "success" => 1.0,
                "failed" => 0.8,
                "denied" | "revised" => 0.9,
                _ => 0.75,
            },
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::FileEdit(_) => "file edit",
            Self::Diff(_) => "diff",
            Self::Validation(_) => "validation",
            Self::UserConfirmation(_) => "user confirmation",
            Self::ToolObservation(_) => "tool observation",
        }
    }

    fn payload(&self) -> serde_json::Value {
        match self {
            Self::FileEdit(entry) => serde_json::to_value(entry),
            Self::Diff(entry) => serde_json::to_value(entry),
            Self::Validation(entry) => serde_json::to_value(entry),
            Self::UserConfirmation(entry) => serde_json::to_value(entry),
            Self::ToolObservation(entry) => serde_json::to_value(entry),
        }
        .unwrap_or_else(|_| json!({}))
    }
}

pub fn file_read_entry_from_event(event: &LearningEventRecord) -> Option<FileReadLedgerEntry> {
    ledger_entry_from_event(event, CONTEXT_LEDGER_FILE_READ_KIND)
}

pub fn file_edit_entry_from_event(event: &LearningEventRecord) -> Option<FileEditLedgerEntry> {
    ledger_entry_from_event(event, CONTEXT_LEDGER_FILE_EDIT_KIND)
}

pub fn diff_entry_from_event(event: &LearningEventRecord) -> Option<DiffLedgerEntry> {
    ledger_entry_from_event(event, CONTEXT_LEDGER_DIFF_KIND)
}

pub fn validation_entry_from_event(event: &LearningEventRecord) -> Option<ValidationLedgerEntry> {
    ledger_entry_from_event(event, CONTEXT_LEDGER_VALIDATION_KIND)
}

pub fn user_confirmation_entry_from_event(
    event: &LearningEventRecord,
) -> Option<UserConfirmationLedgerEntry> {
    ledger_entry_from_event(event, CONTEXT_LEDGER_USER_CONFIRMATION_KIND)
}

pub fn tool_observation_entry_from_event(
    event: &LearningEventRecord,
) -> Option<ToolObservationLedgerEntry> {
    ledger_entry_from_event(event, CONTEXT_LEDGER_TOOL_OBSERVATION_KIND)
}

fn ledger_entry_from_event<T>(event: &LearningEventRecord, kind: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    if event.kind != kind {
        return None;
    }
    serde_json::from_value(event.payload.clone()).ok()
}

fn file_edit_entry_from_tool_result(
    tool_call: &ToolCall,
    result: &ToolResult,
) -> Option<FileEditLedgerEntry> {
    if !matches!(
        tool_call.name.as_str(),
        "file_edit" | "file_write" | "file_patch"
    ) {
        return None;
    }

    let data = result.data.as_ref();
    let mut paths = collect_file_paths(data, &tool_call.arguments, "path");
    dedup_strings(&mut paths);
    let mut resolved_paths = collect_file_paths(data, &tool_call.arguments, "resolved_path");
    dedup_strings(&mut resolved_paths);

    if paths.is_empty() && resolved_paths.is_empty() && !result.success {
        return None;
    }

    let bytes_written = summed_number(data, "bytes_written").unwrap_or(0);
    let replacements = summed_number(data, "replacements");
    let additions = summed_diff_number(data, "additions");
    let deletions = summed_diff_number(data, "deletions");
    let changed_line_start = top_level_diff_number(data, "changed_line_start");
    let changed_line_end = top_level_diff_number(data, "changed_line_end");
    let diff_hash = combined_unified_diff(data)
        .filter(|diff| !diff.trim().is_empty())
        .map(|diff| stable_hash(&diff));
    let file_count = paths.len().max(resolved_paths.len()).max(1);
    let display = paths
        .first()
        .or_else(|| resolved_paths.first())
        .cloned()
        .unwrap_or_else(|| "unknown path".to_string());
    let summary = if file_count > 1 {
        format!(
            "{} changed {} file(s), first {}",
            tool_call.name, file_count, display
        )
    } else if result.success {
        format!("{} changed {}", tool_call.name, display)
    } else {
        format!("{} attempted {}", tool_call.name, display)
    };

    Some(FileEditLedgerEntry {
        tool: tool_call.name.clone(),
        paths,
        resolved_paths,
        success: result.success,
        file_count,
        bytes_written,
        replacements,
        additions,
        deletions,
        changed_line_start,
        changed_line_end,
        diff_hash,
        summary,
    })
}

fn diff_entry_from_tool_result(
    tool_call: &ToolCall,
    result: &ToolResult,
) -> Option<DiffLedgerEntry> {
    let (action, command, path) = match tool_call.name.as_str() {
        "diff" => (
            json_string(&tool_call.arguments, "action"),
            None,
            json_string(&tool_call.arguments, "path")
                .or_else(|| json_string(&tool_call.arguments, "old_path"))
                .or_else(|| json_string(&tool_call.arguments, "new_path")),
        ),
        "git" if tool_call.arguments.get("action").and_then(|v| v.as_str()) == Some("diff") => (
            Some("diff".to_string()),
            None,
            json_string(&tool_call.arguments, "path"),
        ),
        "git_diff" => (
            Some("diff".to_string()),
            None,
            json_string(&tool_call.arguments, "path"),
        ),
        "bash" => {
            let command = tool_call.arguments.get("command")?.as_str()?.to_string();
            if !looks_like_diff_command(&command) {
                return None;
            }
            (
                Some("shell_diff".to_string()),
                Some(command.clone()),
                first_path_pattern(&command),
            )
        }
        _ => return None,
    };

    let output_hash = stable_hash(&result_output_material(result));
    let output_chars = result.content.chars().count();
    let changed = diff_output_has_changes(result);
    let target = command
        .as_deref()
        .or(path.as_deref())
        .or(action.as_deref())
        .unwrap_or("diff");
    let summary = format!(
        "{} observed {} ({})",
        tool_call.name,
        compact_command(target),
        if changed { "changes" } else { "no changes" }
    );

    Some(DiffLedgerEntry {
        tool: tool_call.name.clone(),
        action,
        command,
        path,
        success: result.success,
        changed,
        output_hash,
        output_chars,
        summary,
    })
}

fn validation_entry_from_tool_result(
    tool_call: &ToolCall,
    result: &ToolResult,
) -> Option<ValidationLedgerEntry> {
    if !matches!(tool_call.name.as_str(), "bash" | "run_tests") {
        return None;
    }
    let command = tool_call
        .arguments
        .get("command")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            result
                .data
                .as_ref()
                .and_then(|data| data.get("shell_result"))
                .and_then(|shell| shell.get("command"))
                .and_then(serde_json::Value::as_str)
        })?
        .trim();
    if command.is_empty() {
        return None;
    }
    let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
    if !classification.is_safe_validation() {
        return None;
    }

    let shell_result = result
        .data
        .as_ref()
        .and_then(|data| data.get("shell_result"));
    let exit_code = shell_result
        .and_then(|value| value.get("exit_code"))
        .and_then(serde_json::Value::as_i64);
    let cwd = shell_result
        .and_then(|value| value.get("cwd"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let timed_out = shell_result
        .and_then(|value| value.get("timed_out"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let output_hash = stable_hash(&result_output_material(result));
    let output_chars = result.content.chars().count();
    let command_kind = enum_json_string(classification.command_kind);
    let category = enum_json_string(classification.category);
    let validation_family = classification.validation_family.map(enum_json_string);
    let summary = format!(
        "Validation {} {}",
        compact_command(command),
        if result.success { "passed" } else { "failed" }
    );

    Some(ValidationLedgerEntry {
        tool: tool_call.name.clone(),
        command: command.to_string(),
        cwd,
        success: result.success,
        exit_code,
        command_kind,
        category,
        validation_family,
        safe_for_closeout: classification.safe_for_closeout,
        output_hash,
        output_chars,
        timed_out,
        summary,
    })
}

fn user_confirmation_entry_from_tool_result(
    tool_call: &ToolCall,
    result: &ToolResult,
) -> Option<UserConfirmationLedgerEntry> {
    let permission_request = result
        .data
        .as_ref()
        .and_then(|data| data.get("permission_request"))?;
    let approved = permission_request
        .get("approved")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(result.success);
    let kind = json_string(permission_request, "kind");
    let request_id = json_string(permission_request, "id");
    let patterns = json_string_array(permission_request, "patterns");
    let allowed_always_rules = json_string_array(permission_request, "allowed_always_rules");
    let metadata = permission_request.get("metadata");
    let risk_level = metadata.and_then(|value| json_string(value, "risk_level"));
    let decision = metadata.and_then(|value| json_string(value, "permission_decision"));
    let source = json_string(permission_request, "permission_source")
        .or_else(|| metadata.and_then(|value| json_string(value, "resolved_permission_source")))
        .or_else(|| metadata.and_then(|value| json_string(value, "permission_source")));
    let label = kind.as_deref().unwrap_or(tool_call.name.as_str());
    let summary = format!(
        "User {} {} for {}",
        if approved { "approved" } else { "denied" },
        label,
        tool_call.name
    );

    Some(UserConfirmationLedgerEntry {
        tool: tool_call.name.clone(),
        approved,
        kind,
        request_id,
        patterns,
        allowed_always_rules,
        risk_level,
        decision,
        source,
        summary,
    })
}

fn tool_observation_entry_from_tool_result(
    tool_call: &ToolCall,
    result: &ToolResult,
) -> Option<ToolObservationLedgerEntry> {
    let data = result.data.as_ref()?;
    let observation = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_observation"))?;
    let action_decision = data.get("action_decision");
    let action_scores = action_decision.and_then(|decision| decision.get("scores"));
    let action_computation = action_decision.and_then(|decision| decision.get("score_computation"));
    let action_review = data.get("action_review");
    let status = json_string(observation, "status").unwrap_or_else(|| {
        if result.success {
            "success".to_string()
        } else {
            "failed".to_string()
        }
    });
    let summary = json_string(observation, "summary")
        .unwrap_or_else(|| format!("{} result: {}", tool_call.name, status));

    Some(ToolObservationLedgerEntry {
        tool: json_string(observation, "tool").unwrap_or_else(|| tool_call.name.clone()),
        call_id: json_string(observation, "call_id").unwrap_or_else(|| tool_call.id.clone()),
        status,
        result_kind: json_string(observation, "result_kind")
            .unwrap_or_else(|| "generic".to_string()),
        summary,
        key_findings: json_string_array(observation, "key_findings"),
        evidence: json_observation_evidence_array(observation),
        impact_on_goal: json_string(observation, "impact_on_goal"),
        next_attention: json_string_array(observation, "next_attention"),
        files_read: json_string_array(observation, "files_read"),
        files_changed: json_string_array(observation, "files_changed"),
        command_run: json_string(observation, "command_run"),
        validation_result: json_string(observation, "validation_result"),
        permission_decision: json_string(observation, "permission_decision"),
        permission_source: json_string(observation, "permission_source"),
        checkpoint_id: json_string(observation, "checkpoint_id"),
        artifact_path: json_string(observation, "artifact_path"),
        quality_warnings: json_string_array(observation, "quality_warnings"),
        state_updates: json_string_array(observation, "state_updates"),
        recommended_next_action: json_string(observation, "recommended_next_action"),
        include_in_next_context: json_bool_default(observation, "include_in_next_context", true),
        store_in_state: json_bool_default(observation, "store_in_state", true),
        confidence: json_u8(observation, "confidence"),
        raw_result_ref: json_string(observation, "raw_result_ref"),
        hypothesis_updates: json_hypothesis_update_array(observation),
        candidate_focus: json_string_array(observation, "candidate_focus"),
        reduced_uncertainty: json_bool_default(observation, "reduced_uncertainty", false),
        risk_note: json_string(observation, "risk_note"),
        failure_type: json_string(observation, "failure_type"),
        recovery_plan_id: json_string(observation, "recovery_plan_id"),
        recovery_kind: json_string(observation, "recovery_kind"),
        action_stage: action_decision
            .and_then(|decision| decision.get("action"))
            .and_then(|action| json_string(action, "stage")),
        action_value: action_scores.and_then(|scores| json_u8(scores, "value")),
        action_risk: action_scores.and_then(|scores| json_u8(scores, "risk")),
        action_uncertainty_reduction: action_scores
            .and_then(|scores| json_u8(scores, "uncertainty_reduction")),
        action_cost: action_scores.and_then(|scores| json_u8(scores, "cost")),
        action_reversibility: action_scores.and_then(|scores| json_u8(scores, "reversibility")),
        action_scope_fit: action_scores.and_then(|scores| json_u8(scores, "scope_fit")),
        action_score: action_scores.and_then(|scores| json_i16(scores, "action_score")),
        action_formula_stage: action_computation
            .and_then(|computation| json_string(computation, "formula_stage")),
        action_formula_version: action_computation
            .and_then(|computation| json_string(computation, "formula_version")),
        action_review_decision: action_review.and_then(|review| json_string(review, "decision")),
    })
}

struct ContextLedgerPersistRequest<'a, T> {
    store: &'a SessionStore,
    session_id: &'a str,
    kind: &'a str,
    source: &'a str,
    summary: &'a str,
    confidence: f64,
    entry: &'a T,
    label: &'a str,
}

fn persist_context_ledger_entry<T>(request: ContextLedgerPersistRequest<'_, T>) -> usize
where
    T: Serialize,
{
    let payload = serde_json::to_value(request.entry).unwrap_or_else(|_| json!({}));
    match request.store.add_learning_event(
        request.session_id,
        request.kind,
        request.source,
        request.summary,
        request.confidence,
        &payload,
    ) {
        Ok(_) => 1,
        Err(err) => {
            warn!(
                session_id = request.session_id,
                kind = request.kind,
                source = request.source,
                error = %err,
                "failed to persist {} context ledger entry",
                request.label
            );
            0
        }
    }
}

fn collect_file_paths(
    data: Option<&serde_json::Value>,
    arguments: &serde_json::Value,
    key: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(value) = data
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
    {
        out.push(value.to_string());
    }
    if let Some(files) = data
        .and_then(|value| value.get("files"))
        .and_then(serde_json::Value::as_array)
    {
        out.extend(files.iter().filter_map(|file| {
            file.get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        }));
    }
    if let Some(value) = arguments.get(key).and_then(serde_json::Value::as_str) {
        out.push(value.to_string());
    }
    if key == "path" {
        if let Some(operations) = arguments
            .get("operations")
            .and_then(serde_json::Value::as_array)
        {
            out.extend(operations.iter().filter_map(|operation| {
                operation
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            }));
        }
    }
    out
}

fn summed_number(data: Option<&serde_json::Value>, key: &str) -> Option<u64> {
    let mut total = number_value(data.and_then(|value| value.get(key))).unwrap_or(0);
    let mut found = total > 0;
    if let Some(files) = data
        .and_then(|value| value.get("files"))
        .and_then(serde_json::Value::as_array)
    {
        for file in files {
            if let Some(value) = number_value(file.get(key)) {
                total += value;
                found = true;
            }
        }
    }
    found.then_some(total)
}

fn summed_diff_number(data: Option<&serde_json::Value>, key: &str) -> Option<u64> {
    let mut total = top_level_diff_number(data, key).unwrap_or(0);
    let mut found = total > 0;
    if let Some(files) = data
        .and_then(|value| value.get("files"))
        .and_then(serde_json::Value::as_array)
    {
        for file in files {
            if let Some(value) = file
                .get("diff")
                .and_then(|diff| number_value(diff.get(key)))
            {
                total += value;
                found = true;
            }
        }
    }
    found.then_some(total)
}

fn top_level_diff_number(data: Option<&serde_json::Value>, key: &str) -> Option<u64> {
    data.and_then(|value| value.get("diff"))
        .and_then(|diff| number_value(diff.get(key)))
}

fn number_value(value: Option<&serde_json::Value>) -> Option<u64> {
    value.and_then(serde_json::Value::as_u64).or_else(|| {
        value
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| u64::try_from(value).ok())
    })
}

fn combined_unified_diff(data: Option<&serde_json::Value>) -> Option<String> {
    let mut diffs = Vec::new();
    if let Some(diff) = data
        .and_then(|value| value.get("diff"))
        .and_then(|diff| diff.get("unified_diff"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        diffs.push(diff.to_string());
    }
    if let Some(files) = data
        .and_then(|value| value.get("files"))
        .and_then(serde_json::Value::as_array)
    {
        diffs.extend(files.iter().filter_map(|file| {
            file.get("diff")
                .and_then(|diff| diff.get("unified_diff"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
        }));
    }
    (!diffs.is_empty()).then(|| diffs.join("\n\n"))
}

fn looks_like_diff_command(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    lower.starts_with("git diff")
        || lower.starts_with("git --no-pager diff")
        || (lower.starts_with("git -c ") && lower.contains(" diff"))
        || lower.contains(" git diff ")
        || lower.contains(" git --no-pager diff ")
}

fn first_path_pattern(command: &str) -> Option<String> {
    let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
    classification.path_patterns.first().cloned()
}

fn diff_output_has_changes(result: &ToolResult) -> bool {
    if !result.success {
        return false;
    }
    let output = result.content.trim();
    if output.is_empty() {
        return false;
    }
    let lower = output.to_ascii_lowercase();
    !(lower.starts_with("no differences")
        || lower.starts_with("no changes")
        || lower.contains("no differences found"))
}

fn result_output_material(result: &ToolResult) -> String {
    let mut material = result.content.clone();
    if let Some(error) = result.error.as_deref() {
        material.push_str("\nerror:");
        material.push_str(error);
    }
    material
}

fn stable_hash(value: &str) -> String {
    format!("{:x}", md5::compute(value))
        .chars()
        .take(16)
        .collect()
}

fn enum_json_string<T>(value: T) -> String
where
    T: Serialize,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn json_string_array(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn json_bool_default(value: &serde_json::Value, key: &str, default: bool) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(default)
}

fn default_true() -> bool {
    true
}

fn json_u8(value: &serde_json::Value, key: &str) -> Option<u8> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
}

fn json_i16(value: &serde_json::Value, key: &str) -> Option<i16> {
    value
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i16::try_from(value).ok())
}

fn json_observation_evidence_array(value: &serde_json::Value) -> Vec<String> {
    value
        .get("evidence")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(str::to_string).or_else(|| {
                        item.get("text")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                })
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn json_hypothesis_update_array(value: &serde_json::Value) -> Vec<String> {
    value
        .get("hypothesis_updates")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(str::to_string).or_else(|| {
                        item.get("hypothesis")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                })
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn dedup_strings(values: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn display_path(path: &str, resolved_path: &str) -> String {
    if path.is_empty() {
        return resolved_path.to_string();
    }
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| path.to_string())
}

fn compact_command(command: &str) -> String {
    const MAX_CHARS: usize = 80;
    let mut out = String::new();
    for (idx, ch) in command.chars().enumerate() {
        if idx >= MAX_CHARS {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(name: &str, arguments: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: name.to_string(),
            arguments,
        }
    }

    #[test]
    fn file_read_entry_round_trips_from_learning_event() {
        let event = LearningEventRecord {
            id: 1,
            session_id: "s1".to_string(),
            kind: CONTEXT_LEDGER_FILE_READ_KIND.to_string(),
            source: "file_read".to_string(),
            summary: "Read file".to_string(),
            confidence: 1.0,
            payload: json!({
                "path": "README.md",
                "resolved_path": "/tmp/README.md",
                "content_hash": "abc",
                "size_bytes": 12,
                "total_lines": 2,
                "displayed_lines": 2,
                "line_start": 1,
                "line_end": 2,
                "targeted_read": false,
                "truncated": false,
                "mtime_unix_secs": 42
            }),
            created_at: "now".to_string(),
        };

        let entry = file_read_entry_from_event(&event).expect("entry");
        assert_eq!(entry.path, "README.md");
        assert_eq!(entry.total_lines, 2);
        assert_eq!(entry.mtime_unix_secs, Some(42));
    }

    #[test]
    fn records_validation_and_user_confirmation_from_tool_result() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "test", "model", None).unwrap();
        let call = tool_call("bash", json!({"command": "cargo test -q"}));
        let result = ToolResult::success_with_data(
            "test result",
            json!({
                "shell_result": {
                    "command": "cargo test -q",
                    "cwd": "/tmp/project",
                    "exit_code": 0,
                    "stdout_bytes": 11,
                    "stderr_bytes": 0,
                    "timed_out": false
                },
                "permission_request": {
                    "id": "perm_1",
                    "kind": "bash",
                    "approved": true,
                    "patterns": ["cargo test -q"],
                    "allowed_always_rules": [],
                    "metadata": {
                        "risk_level": "low",
                        "permission_decision": "allow_once"
                    }
                }
            }),
        );

        let count = record_tool_context_evidence(&store, "s1", &call, &result);

        assert_eq!(count, 2);
        let events = store.recent_context_ledger_events("s1", 10).unwrap();
        let validation = events
            .iter()
            .find_map(validation_entry_from_event)
            .expect("validation entry");
        assert_eq!(validation.command, "cargo test -q");
        assert!(validation.success);
        assert_eq!(validation.exit_code, Some(0));
        assert_eq!(validation.validation_family.as_deref(), Some("cargo_test"));

        let confirmation = events
            .iter()
            .find_map(user_confirmation_entry_from_event)
            .expect("confirmation entry");
        assert!(confirmation.approved);
        assert_eq!(confirmation.kind.as_deref(), Some("bash"));
        assert_eq!(confirmation.risk_level.as_deref(), Some("low"));
    }

    #[test]
    fn records_file_edit_and_diff_from_tool_result() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "test", "model", None).unwrap();
        let edit_call = tool_call("file_edit", json!({"path": "src/lib.rs"}));
        let edit_result = ToolResult::success_with_data(
            "edited",
            json!({
                "path": "src/lib.rs",
                "resolved_path": "/tmp/project/src/lib.rs",
                "replacements": 1,
                "bytes_written": 42,
                "diff": {
                    "additions": 2,
                    "deletions": 1,
                    "changed_line_start": 10,
                    "changed_line_end": 12,
                    "unified_diff": "@@ -10 +10 @@\n-old\n+new\n"
                }
            }),
        );
        let diff_call = tool_call("diff", json!({"action": "file", "path": "src/lib.rs"}));
        let diff_result = ToolResult::success("diff --git a/src/lib.rs b/src/lib.rs\n+new\n");

        let edit_count = record_tool_context_evidence(&store, "s1", &edit_call, &edit_result);
        let diff_count = record_tool_context_evidence(&store, "s1", &diff_call, &diff_result);

        assert_eq!(edit_count, 1);
        assert_eq!(diff_count, 1);
        let events = store.recent_context_ledger_events("s1", 10).unwrap();
        let edit = events
            .iter()
            .find_map(file_edit_entry_from_event)
            .expect("edit entry");
        assert_eq!(edit.paths, vec!["src/lib.rs"]);
        assert_eq!(edit.bytes_written, 42);
        assert_eq!(edit.replacements, Some(1));
        assert_eq!(edit.additions, Some(2));
        assert_eq!(edit.changed_line_start, Some(10));
        assert!(edit.diff_hash.is_some());

        let diff = events
            .iter()
            .find_map(diff_entry_from_event)
            .expect("diff entry");
        assert_eq!(diff.tool, "diff");
        assert_eq!(diff.action.as_deref(), Some("file"));
        assert!(diff.changed);
    }

    #[test]
    fn records_tool_observation_from_result_metadata() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "test", "model", None).unwrap();
        let call = tool_call("file_edit", json!({"path": "src/lib.rs"}));
        let result = ToolResult::success_with_data(
            "edited",
            json!({
                "tool_observation": {
                    "schema": "tool_observation.v1",
                    "tool": "file_edit",
                    "call_id": "call_1",
                    "status": "success",
                    "summary": "file_edit succeeded: edited src/lib.rs",
                    "files_read": [],
                    "files_changed": ["src/lib.rs"],
                    "command_run": null,
                    "validation_result": null,
                    "permission_decision": null,
                    "checkpoint_id": "cp_1",
                    "artifact_path": null,
                    "state_updates": ["files_changed", "checkpoint"],
                    "recommended_next_action": null
                },
                "action_decision": {
                    "action": {
                        "stage": "Edit"
                    },
                    "scores": {
                        "value": 8,
                        "risk": 5,
                        "uncertainty_reduction": 3,
                        "cost": 4,
                        "reversibility": 6,
                        "scope_fit": 9,
                        "action_score": 12
                    },
                    "score_computation": {
                        "formula_stage": "implementation",
                        "formula_version": "action_score.v1"
                    }
                },
                "action_review": {
                    "decision": "allow"
                }
            }),
        );

        let count = record_tool_context_evidence(&store, "s1", &call, &result);

        assert_eq!(count, 2);
        let events = store.recent_context_ledger_events("s1", 10).unwrap();
        let observation = events
            .iter()
            .find_map(tool_observation_entry_from_event)
            .expect("tool observation entry");
        assert_eq!(observation.status, "success");
        assert_eq!(observation.files_changed, vec!["src/lib.rs"]);
        assert_eq!(observation.checkpoint_id.as_deref(), Some("cp_1"));
        assert_eq!(observation.action_score, Some(12));
        assert_eq!(observation.action_scope_fit, Some(9));
        assert_eq!(observation.action_review_decision.as_deref(), Some("allow"));
    }
}
