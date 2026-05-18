//! Runtime-owned evidence ledger for facts the model must not invent.
//!
//! The ledger keeps file, command, and validation facts as structured runtime
//! data. User-facing summaries and workflow closeout can then cite evidence
//! without mixing raw tool metadata into model-visible text.

use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

const PREVIEW_CHARS: usize = 240;
const HASH_PREVIEW_CHARS: usize = 16;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceLedger {
    changed_files: BTreeSet<String>,
    tool_execution_records: Vec<ToolExecutionRecord>,
    file_facts: Vec<FileEvidence>,
    command_facts: Vec<CommandEvidence>,
    validation_facts: Vec<ValidationEvidence>,
    permission_facts: Vec<PermissionEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionRecord {
    pub call_id: String,
    pub tool: String,
    pub status: ToolExecutionStatus,
    pub arguments_hash: String,
    pub duration_ms: Option<u64>,
    pub output_chars: usize,
    pub error_code: Option<String>,
    pub error_preview: Option<String>,
    pub permission: Option<ToolPermissionRecord>,
    pub command: Option<String>,
    pub command_kind: Option<String>,
    pub command_category: Option<String>,
    pub validation_family: Option<String>,
    pub safe_for_closeout: Option<bool>,
    pub terminal_task: Option<TerminalTaskRecord>,
    pub changed_paths: Vec<String>,
    pub relevance: ToolExecutionRelevance,
    pub execution: ToolExecutionContextRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Completed,
    Failed,
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolPermissionRecord {
    pub kind: Option<String>,
    pub approved: bool,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub patterns: Vec<String>,
    pub allowed_always_rules: Vec<String>,
    pub source: ToolPermissionSourceRecord,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolPermissionSourceRecord {
    pub permission_requires: Option<bool>,
    pub tool_requires: Option<bool>,
    pub raw_tool_requires: Option<bool>,
    pub drift_requires_approval: Option<bool>,
    pub permission_family: Option<String>,
    pub permission_decision: Option<String>,
    pub risk_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalTaskRecord {
    pub task_id: Option<String>,
    pub status: Option<String>,
    pub terminal_kind: Option<String>,
    pub handle: Option<String>,
    pub output_path: Option<String>,
    pub duration_ms: Option<u64>,
    pub exit_code: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionRelevance {
    pub validation: bool,
    pub closeout: bool,
    pub repair: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionContextRecord {
    pub route: Option<ToolRouteRecord>,
    pub policy: Option<ToolExecutionPolicyRecord>,
    pub parallel: bool,
    pub pre_executed: bool,
    pub action_checkpoint_active: bool,
    pub has_changes_before_tools: bool,
    pub exposed_tools_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolRouteRecord {
    pub intent: Option<String>,
    pub workflow: Option<String>,
    pub retrieval: Option<String>,
    pub reasoning: Option<String>,
    pub risk: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionPolicyRecord {
    pub latency: Option<String>,
    pub parallelism_limit: Option<usize>,
    pub max_tool_calls: Option<usize>,
    pub context_budget_tokens: Option<usize>,
    pub allow_fallback_model: Option<bool>,
    pub cost_ceiling_usd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEvidence {
    pub tool: String,
    pub path: Option<String>,
    pub success: bool,
    pub kind: Option<String>,
    pub line_start: Option<u64>,
    pub line_end: Option<u64>,
    pub total_lines: Option<u64>,
    pub displayed_lines: Option<u64>,
    pub truncated: Option<bool>,
    pub content_hash: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandEvidence {
    pub command: String,
    pub success: bool,
    pub command_kind: Option<String>,
    pub command_category: Option<String>,
    pub validation_family: Option<String>,
    pub safe_for_closeout: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationEvidence {
    pub source: String,
    pub command: Option<String>,
    pub passed: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionEvidence {
    pub tool: String,
    pub kind: Option<String>,
    pub approved: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceSnapshot {
    pub changed_files: Vec<String>,
    pub tool_execution_records: usize,
    pub file_facts: usize,
    pub command_facts: usize,
    pub validation_facts: usize,
    pub passed_validation_facts: usize,
    pub failed_validation_facts: usize,
    pub permission_facts: usize,
    pub denied_permission_facts: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidationRollup {
    current_total: usize,
    current_passed: usize,
    current_failed: usize,
    recovered_failed: usize,
}

pub async fn changed_files_diff_evidence(
    working_dir: &Path,
    changed_files: &[PathBuf],
) -> Option<String> {
    let mut args = vec![
        "diff".to_string(),
        "--no-color".to_string(),
        "--".to_string(),
    ];
    let mut seen = HashSet::new();
    for path in changed_files {
        let display_path = path
            .strip_prefix(working_dir)
            .ok()
            .unwrap_or(path.as_path())
            .display()
            .to_string();
        if display_path.trim().is_empty() || !seen.insert(display_path.clone()) {
            continue;
        }
        args.push(display_path);
    }

    if args.len() <= 3 {
        return None;
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(working_dir)
        .output()
        .await
        .ok()?;

    if !output.status.success() && output.stdout.is_empty() {
        return None;
    }

    let diff = String::from_utf8_lossy(&output.stdout);
    let trimmed = diff.trim();
    if trimmed.is_empty() {
        return None;
    }

    let max_chars = 12_000usize;
    let mut excerpt = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        excerpt.push_str("\n[diff excerpt truncated]");
    }

    Some(format!(
        "[Changed-file diff evidence]\n{}\nUse this diff as direct acceptance evidence for the modified files.",
        excerpt
    ))
}

impl EvidenceLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_tool_result(&mut self, tool_call: &ToolCall, result: &ToolResult) {
        self.record_tool_execution_record(tool_call, result);
        self.record_permission_tool_result(tool_call, result);
        match tool_call.name.as_str() {
            "bash" => self.record_bash_tool_result(tool_call, result),
            "file_patch" => self.record_file_patch_tool_result(result),
            "file_read" | "file_write" | "file_edit" | "glob" | "grep" => {
                self.record_file_tool_result(tool_call, result)
            }
            _ => {}
        }
    }

    pub fn record_changed_files(&mut self, changed_files: &[PathBuf]) {
        for path in changed_files {
            self.changed_files.insert(path.display().to_string());
        }
    }

    pub fn record_validation_result(
        &mut self,
        source: impl Into<String>,
        command: Option<&str>,
        passed: bool,
        summary: impl AsRef<str>,
    ) {
        self.validation_facts.push(ValidationEvidence {
            source: source.into(),
            command: command.map(str::to_string),
            passed,
            summary: preview(summary.as_ref()),
        });
    }

    pub fn snapshot(&self) -> EvidenceSnapshot {
        let passed_validation_facts = self
            .validation_facts
            .iter()
            .filter(|fact| fact.passed)
            .count();
        let failed_validation_facts = self.validation_facts.len() - passed_validation_facts;
        let denied_permission_facts = self
            .permission_facts
            .iter()
            .filter(|fact| !fact.approved)
            .count();
        EvidenceSnapshot {
            changed_files: self.changed_files.iter().cloned().collect(),
            tool_execution_records: self.tool_execution_records.len(),
            file_facts: self.file_facts.len(),
            command_facts: self.command_facts.len(),
            validation_facts: self.validation_facts.len(),
            passed_validation_facts,
            failed_validation_facts,
            permission_facts: self.permission_facts.len(),
            denied_permission_facts,
        }
    }

    pub fn runtime_validation_label(&self) -> Option<String> {
        let rollup = self.current_validation_rollup()?;
        if rollup.current_failed > 0 {
            return Some(format!(
                "failed:{}/{}",
                rollup.current_failed, rollup.current_total
            ));
        }
        let mut label = format!("passed:{}/{}", rollup.current_passed, rollup.current_total);
        if rollup.recovered_failed > 0 {
            label.push_str(&format!(" recovered_failed:{}", rollup.recovered_failed));
        }
        Some(label)
    }

    pub fn runtime_required_validation_label(
        &self,
        required_commands: &[String],
    ) -> Option<String> {
        let required = required_commands
            .iter()
            .map(|command| normalize_command_identity(command))
            .filter(|command| !command.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if required.is_empty() {
            return None;
        }

        let mut current = BTreeMap::<String, bool>::new();
        let mut failed_identities = BTreeSet::<String>::new();
        for fact in &self.command_facts {
            for required_command in &required {
                if command_fact_satisfies_required(fact, required_command) {
                    if !fact.success {
                        failed_identities.insert(required_command.clone());
                    }
                    current.insert(required_command.clone(), fact.success);
                }
            }
        }
        for fact in &self.validation_facts {
            let Some(command) = fact.command.as_deref() else {
                continue;
            };
            let command = normalize_command_identity(command);
            if required.contains(&command) {
                if !fact.passed {
                    failed_identities.insert(command.clone());
                }
                current.insert(command, fact.passed);
            }
        }

        if required
            .iter()
            .any(|command| !current.contains_key(command))
        {
            return None;
        }

        let current_total = required.len();
        let current_passed = required
            .iter()
            .filter(|command| current.get(*command).copied().unwrap_or(false))
            .count();
        let current_failed = current_total.saturating_sub(current_passed);
        if current_failed > 0 {
            return Some(format!("failed:{current_failed}/{current_total}"));
        }
        let recovered_failed = required
            .iter()
            .filter(|command| {
                failed_identities.contains(*command)
                    && current.get(*command).copied().unwrap_or(false)
            })
            .count();
        let mut label = format!("passed:{current_passed}/{current_total}");
        if recovered_failed > 0 {
            label.push_str(&format!(" recovered_failed:{recovered_failed}"));
        }
        Some(label)
    }

    fn current_validation_rollup(&self) -> Option<ValidationRollup> {
        if self.validation_facts.is_empty() {
            return None;
        }
        let mut order = Vec::<String>::new();
        let mut current = std::collections::BTreeMap::<String, ValidationEvidence>::new();
        let mut failed_identities = BTreeSet::<String>::new();
        for fact in &self.validation_facts {
            let identity = validation_identity(fact);
            if !current.contains_key(&identity) {
                order.push(identity.clone());
            }
            if !fact.passed {
                failed_identities.insert(identity.clone());
            }
            current.insert(identity, fact.clone());
        }
        let current_total = order.len();
        let current_passed = order
            .iter()
            .filter(|identity| {
                current
                    .get(*identity)
                    .map(|fact| fact.passed)
                    .unwrap_or(false)
            })
            .count();
        let current_failed = current_total.saturating_sub(current_passed);
        let recovered_failed = order
            .iter()
            .filter(|identity| {
                failed_identities.contains(*identity)
                    && current
                        .get(*identity)
                        .map(|fact| fact.passed)
                        .unwrap_or(false)
            })
            .count();
        Some(ValidationRollup {
            current_total,
            current_passed,
            current_failed,
            recovered_failed,
        })
    }

    pub fn changed_files(&self) -> Vec<String> {
        self.changed_files.iter().cloned().collect()
    }

    pub fn tool_execution_records(&self) -> &[ToolExecutionRecord] {
        &self.tool_execution_records
    }

    pub fn validation_facts(&self) -> &[ValidationEvidence] {
        &self.validation_facts
    }

    pub fn permission_facts(&self) -> &[PermissionEvidence] {
        &self.permission_facts
    }

    pub fn unsupported_filesystem_claims(&self, answer: &str) -> Vec<String> {
        if self.file_facts.is_empty() && self.command_facts.is_empty() {
            return Vec::new();
        }

        let evidence = self
            .file_facts
            .iter()
            .map(|fact| fact.summary.as_str())
            .chain(self.command_facts.iter().map(|fact| fact.summary.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
            .to_ascii_lowercase();
        let answer_lower = answer.to_ascii_lowercase();
        let mut gaps = Vec::new();

        if contains_any(answer, &["创建时间", "创建于"])
            || contains_any(&answer_lower, &["created", "creation time", "created at"])
        {
            let supported = contains_any(
                &evidence,
                &[
                    "birth",
                    "created",
                    "creation",
                    "created at",
                    "stat -f",
                    "stat --format",
                    "getfileinfo",
                    "创建时间",
                ],
            );
            if !supported {
                gaps.push("creation_time".to_string());
            }
        }

        if contains_any(answer, &["内容数", "项目数", "个项目", "个条目"])
            || contains_any(&answer_lower, &["item count", "entries", "items"])
        {
            let supported = contains_any(
                &evidence,
                &[
                    "total_entries",
                    "entries",
                    "item count",
                    "find ",
                    "wc -l",
                    "ls -1",
                ],
            );
            if !supported {
                gaps.push("item_count".to_string());
            }
        }

        if contains_any(answer, &["大小：", "大小:", " 字节"])
            || contains_any(&answer_lower, &["size:", " bytes", " byte"])
        {
            let supported = contains_any(
                &evidence,
                &["size", "bytes", "byte", "stat -f", "stat --format"],
            );
            if !supported {
                gaps.push("size".to_string());
            }
        }

        gaps
    }

    fn record_bash_tool_result(&mut self, tool_call: &ToolCall, result: &ToolResult) {
        let Some(command) = tool_call.arguments["command"]
            .as_str()
            .or_else(|| bash_result_command(result))
        else {
            return;
        };
        let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
        let command_kind = serde_json::to_value(classification.command_kind)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string));
        let command_category = serde_json::to_value(classification.category)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string));
        let validation_family = serde_json::to_value(classification.validation_family)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string));
        let safe_for_closeout = classification.safe_for_closeout;
        let summary = result_summary(result);
        self.command_facts.push(CommandEvidence {
            command: command.to_string(),
            success: result.success,
            command_kind,
            command_category,
            validation_family,
            safe_for_closeout,
            summary: summary.clone(),
        });
        if classification.is_safe_validation() {
            self.record_validation_result("bash", Some(command), result.success, summary);
        }
    }

    fn record_tool_execution_record(&mut self, tool_call: &ToolCall, result: &ToolResult) {
        let summary = tool_summary(result);
        let command = tool_call
            .arguments
            .get("command")
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                summary
                    .and_then(|summary| summary.get("command"))
                    .and_then(serde_json::Value::as_str)
            })
            .map(preview);
        let command_kind = summary_string(summary, "command_kind");
        let command_category = summary_string(summary, "command_category");
        let validation_family = summary_string(summary, "validation_family");
        let safe_for_closeout = summary
            .and_then(|summary| summary.get("safe_for_closeout"))
            .and_then(serde_json::Value::as_bool);
        let permission = tool_permission_record(result);
        let status = if permission.as_ref().is_some_and(|record| !record.approved) {
            ToolExecutionStatus::Denied
        } else if result.success {
            ToolExecutionStatus::Completed
        } else {
            ToolExecutionStatus::Failed
        };
        let changed_paths = changed_paths_for_tool_result(tool_call, result);
        let relevance = tool_execution_relevance(
            result,
            safe_for_closeout,
            validation_family.as_deref(),
            &changed_paths,
            permission.as_ref(),
        );
        self.tool_execution_records.push(ToolExecutionRecord {
            call_id: tool_call.id.clone(),
            tool: tool_call.name.clone(),
            status,
            arguments_hash: tool_arguments_hash(&tool_call.arguments),
            duration_ms: result.duration_ms,
            output_chars: result.content.chars().count(),
            error_code: result.error_code.as_ref().and_then(|code| {
                serde_json::to_value(code)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
            }),
            error_preview: result.error.as_deref().map(preview),
            permission,
            command,
            command_kind,
            command_category,
            validation_family,
            safe_for_closeout,
            terminal_task: terminal_task_record(result),
            changed_paths,
            relevance,
            execution: tool_execution_context_record(result),
        });
    }

    fn record_file_tool_result(&mut self, tool_call: &ToolCall, result: &ToolResult) {
        let path = tool_call
            .arguments
            .get("path")
            .or_else(|| tool_call.arguments.get("file_path"))
            .and_then(|value| value.as_str())
            .map(str::to_string);
        if result.success && matches!(tool_call.name.as_str(), "file_write" | "file_edit") {
            if let Some(path) = path.as_ref() {
                self.changed_files.insert(path.clone());
            }
        }
        self.file_facts.push(FileEvidence {
            tool: tool_call.name.clone(),
            path,
            success: result.success,
            kind: result_data_string(result, "kind"),
            line_start: result_data_u64(result, "line_start"),
            line_end: result_data_u64(result, "line_end"),
            total_lines: result_data_u64(result, "total_lines"),
            displayed_lines: result_data_u64(result, "displayed_lines"),
            truncated: result_data_bool(result, "truncated"),
            content_hash: result_data_string(result, "content_hash"),
            summary: file_result_summary(result),
        });
    }

    fn record_file_patch_tool_result(&mut self, result: &ToolResult) {
        let Some(files) = result
            .data
            .as_ref()
            .and_then(|data| data.get("files"))
            .and_then(serde_json::Value::as_array)
        else {
            return;
        };
        for file in files {
            let path = file
                .get("path")
                .or_else(|| file.get("resolved_path"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            if result.success {
                if let Some(path) = path.as_ref() {
                    self.changed_files.insert(path.clone());
                }
            }
            let diff = file.get("diff");
            self.file_facts.push(FileEvidence {
                tool: "file_patch".to_string(),
                path,
                success: result.success,
                kind: Some("patch".to_string()),
                line_start: nested_u64(diff, "changed_line_start"),
                line_end: nested_u64(diff, "changed_line_end"),
                total_lines: None,
                displayed_lines: None,
                truncated: nested_bool(diff, "preview_truncated"),
                content_hash: None,
                summary: file_patch_result_summary(result, file),
            });
        }
    }

    fn record_permission_tool_result(&mut self, tool_call: &ToolCall, result: &ToolResult) {
        let Some(permission_request) = result
            .data
            .as_ref()
            .and_then(|data| data.get("permission_request"))
        else {
            return;
        };
        let kind = permission_request
            .get("kind")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let summary = permission_request
            .get("rejection_feedback")
            .and_then(|value| value.as_str())
            .or(result.error.as_deref())
            .unwrap_or("permission request recorded");
        let summary = if let Some(recovery) = permission_request
            .get("recovery_feedback")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
        {
            format!("{summary} Recovery: {recovery}")
        } else {
            summary.to_string()
        };
        self.permission_facts.push(PermissionEvidence {
            tool: tool_call.name.clone(),
            kind,
            approved: permission_request_approved(permission_request, result.success),
            summary: preview(&summary),
        });
    }
}

fn tool_arguments_hash(arguments: &serde_json::Value) -> String {
    let rendered = serde_json::to_string(arguments).unwrap_or_else(|_| arguments.to_string());
    let digest = format!("{:x}", md5::compute(rendered));
    digest.chars().take(HASH_PREVIEW_CHARS).collect()
}

fn tool_summary(result: &ToolResult) -> Option<&serde_json::Value> {
    result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_summary"))
}

fn summary_string(summary: Option<&serde_json::Value>, key: &str) -> Option<String> {
    summary
        .and_then(|summary| summary.get(key))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn tool_permission_record(result: &ToolResult) -> Option<ToolPermissionRecord> {
    let permission_request = result
        .data
        .as_ref()
        .and_then(|data| data.get("permission_request"))?;
    let metadata = permission_request.get("metadata");
    Some(ToolPermissionRecord {
        kind: permission_request
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        approved: permission_request_approved(permission_request, result.success),
        request_id: json_string(permission_request, "id"),
        session_id: json_string(permission_request, "session_id"),
        patterns: json_string_array(permission_request, "patterns"),
        allowed_always_rules: json_string_array(permission_request, "allowed_always_rules"),
        source: ToolPermissionSourceRecord {
            permission_requires: nested_bool(metadata, "permission_requires"),
            tool_requires: nested_bool(metadata, "tool_requires"),
            raw_tool_requires: nested_bool(metadata, "raw_tool_requires"),
            drift_requires_approval: nested_bool(metadata, "drift_requires_approval"),
            permission_family: nested_string(metadata, "permission_family"),
            permission_decision: nested_string(metadata, "permission_decision"),
            risk_level: nested_string(metadata, "risk_level"),
        },
    })
}

fn permission_request_approved(permission_request: &serde_json::Value, fallback: bool) -> bool {
    permission_request
        .get("approved")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(fallback)
}

fn terminal_task_record(result: &ToolResult) -> Option<TerminalTaskRecord> {
    let data = result.data.as_ref()?;
    let task = data
        .get("tool_summary")
        .and_then(|summary| summary.get("terminal_task"))
        .or_else(|| data.get("terminal_task"))?;
    Some(TerminalTaskRecord {
        task_id: task_string(task, "task_id"),
        status: task_string(task, "status"),
        terminal_kind: task_string(task, "terminal_kind"),
        handle: task_string(task, "handle"),
        output_path: task_string(task, "output_path"),
        duration_ms: task.get("duration_ms").and_then(serde_json::Value::as_u64),
        exit_code: task.get("exit_code").and_then(serde_json::Value::as_i64),
    })
}

fn task_string(task: &serde_json::Value, key: &str) -> Option<String> {
    task.get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn changed_paths_for_tool_result(tool_call: &ToolCall, result: &ToolResult) -> Vec<String> {
    let mut paths = BTreeSet::new();
    if result.success && matches!(tool_call.name.as_str(), "file_write" | "file_edit") {
        if let Some(path) = tool_call.arguments["path"].as_str() {
            paths.insert(path.to_string());
        }
    }
    if tool_call.name == "file_patch" && result.success {
        if let Some(files) = result
            .data
            .as_ref()
            .and_then(|data| data.get("files"))
            .and_then(serde_json::Value::as_array)
        {
            for file in files {
                if let Some(path) = file
                    .get("path")
                    .or_else(|| file.get("resolved_path"))
                    .and_then(serde_json::Value::as_str)
                {
                    paths.insert(path.to_string());
                }
            }
        }
    }
    paths.into_iter().collect()
}

fn tool_execution_relevance(
    result: &ToolResult,
    safe_for_closeout: Option<bool>,
    validation_family: Option<&str>,
    changed_paths: &[String],
    permission: Option<&ToolPermissionRecord>,
) -> ToolExecutionRelevance {
    let validation = safe_for_closeout.unwrap_or(false) || validation_family.is_some();
    ToolExecutionRelevance {
        validation,
        closeout: validation || !changed_paths.is_empty() || permission.is_some(),
        repair: !result.success || !changed_paths.is_empty(),
    }
}

fn tool_execution_context_record(result: &ToolResult) -> ToolExecutionContextRecord {
    let Some(runtime) = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_runtime"))
    else {
        return ToolExecutionContextRecord::default();
    };
    let execution = runtime.get("execution");
    ToolExecutionContextRecord {
        route: runtime
            .get("route")
            .filter(|value| value.is_object())
            .map(tool_route_record),
        policy: runtime.get("policy").map(tool_execution_policy_record),
        parallel: nested_bool(execution, "parallel").unwrap_or(false),
        pre_executed: nested_bool(execution, "pre_executed").unwrap_or(false),
        action_checkpoint_active: nested_bool(execution, "action_checkpoint_active")
            .unwrap_or(false),
        has_changes_before_tools: nested_bool(execution, "has_changes_before_tools")
            .unwrap_or(false),
        exposed_tools_count: nested_usize(execution, "exposed_tools_count"),
    }
}

fn tool_route_record(value: &serde_json::Value) -> ToolRouteRecord {
    ToolRouteRecord {
        intent: nested_string(Some(value), "intent"),
        workflow: nested_string(Some(value), "workflow"),
        retrieval: nested_string(Some(value), "retrieval"),
        reasoning: nested_string(Some(value), "reasoning"),
        risk: nested_string(Some(value), "risk"),
    }
}

fn tool_execution_policy_record(value: &serde_json::Value) -> ToolExecutionPolicyRecord {
    ToolExecutionPolicyRecord {
        latency: nested_string(Some(value), "latency"),
        parallelism_limit: nested_usize(Some(value), "parallelism_limit"),
        max_tool_calls: nested_usize(Some(value), "max_tool_calls"),
        context_budget_tokens: nested_usize(Some(value), "context_budget_tokens"),
        allow_fallback_model: nested_bool(Some(value), "allow_fallback_model"),
        cost_ceiling_usd: nested_string(Some(value), "cost_ceiling_usd"),
    }
}

fn bash_result_command(result: &ToolResult) -> Option<&str> {
    result
        .data
        .as_ref()
        .and_then(|data| data.get("shell_result"))
        .and_then(|shell| shell.get("command"))
        .and_then(serde_json::Value::as_str)
}

fn result_summary(result: &ToolResult) -> String {
    let text = if !result.content.trim().is_empty() {
        result.content.as_str()
    } else {
        result.error.as_deref().unwrap_or("")
    };
    preview(text)
}

fn file_result_summary(result: &ToolResult) -> String {
    let summary = result_summary(result);
    let Some(data) = result.data.as_ref() else {
        return summary;
    };
    let mut metadata = Vec::new();
    for key in [
        "kind",
        "total_lines",
        "displayed_lines",
        "line_start",
        "line_end",
        "truncated",
        "read_coverage",
        "content_hash",
        "display_format",
    ] {
        let Some(value) = data.get(key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let rendered = value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string());
        metadata.push(format!("{key}={rendered}"));
    }
    if let Some(rendered) = diagnostics_metadata_summary(data.get("diagnostics")) {
        metadata.push(rendered);
    }
    if metadata.is_empty() {
        return summary;
    }
    preview(&format!("[metadata: {}] {}", metadata.join(" "), summary))
}

fn file_patch_result_summary(result: &ToolResult, file: &serde_json::Value) -> String {
    let mut metadata = Vec::new();
    metadata.push("kind=patch".to_string());
    for key in ["path", "replacements", "bytes_written"] {
        let Some(value) = file.get(key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let rendered = value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string());
        metadata.push(format!("{key}={rendered}"));
    }
    if let Some(diff) = file.get("diff") {
        for key in [
            "additions",
            "deletions",
            "changed_line_start",
            "changed_line_end",
            "preview_truncated",
        ] {
            let Some(value) = diff.get(key) else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            metadata.push(format!("{key}={value}"));
        }
    }
    preview(&format!(
        "[metadata: {}] {}",
        metadata.join(" "),
        result_summary(result)
    ))
}

fn diagnostics_metadata_summary(value: Option<&serde_json::Value>) -> Option<String> {
    let diagnostics = value?;
    let status = diagnostics
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let total = diagnostics
        .get("diagnostic_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let errors = diagnostics
        .get("error_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let warnings = diagnostics
        .get("warning_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let checked = diagnostics
        .get("checked")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let mut parts = vec![format!(
        "lsp_diagnostics=status:{status} checked:{checked} total:{total} errors:{errors} warnings:{warnings}"
    )];
    if let Some(first_error) = diagnostic_item_metadata(diagnostics.get("first_error")) {
        parts.push(format!("first_error:{first_error}"));
    }
    Some(parts.join(" "))
}

fn diagnostic_item_metadata(value: Option<&serde_json::Value>) -> Option<String> {
    let item = value?;
    if item.is_null() {
        return None;
    }
    let message = item.get("message").and_then(serde_json::Value::as_str)?;
    let line = item
        .get("range")
        .and_then(|range| range.get("start_line"))
        .and_then(serde_json::Value::as_u64)
        .map(|line| format!("line:{line}"))
        .unwrap_or_else(|| "line:unknown".to_string());
    let source = item
        .get("source")
        .and_then(serde_json::Value::as_str)
        .filter(|source| !source.is_empty())
        .map(|source| format!(" source:{source}"))
        .unwrap_or_default();
    let code = item
        .get("code")
        .filter(|code| !code.is_null())
        .map(|code| {
            code.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| code.to_string())
        })
        .filter(|code| !code.is_empty())
        .map(|code| format!(" code:{code}"))
        .unwrap_or_default();
    let mut preview = message.chars().take(80).collect::<String>();
    if message.chars().count() > 80 {
        preview.push_str("...");
    }
    Some(format!("{line}{source}{code} message:{preview}"))
}

fn nested_u64(value: Option<&serde_json::Value>, key: &str) -> Option<u64> {
    value
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_u64)
}

fn nested_usize(value: Option<&serde_json::Value>, key: &str) -> Option<usize> {
    nested_u64(value, key).and_then(|value| usize::try_from(value).ok())
}

fn nested_bool(value: Option<&serde_json::Value>, key: &str) -> Option<bool> {
    value
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_bool)
}

fn nested_string(value: Option<&serde_json::Value>, key: &str) -> Option<String> {
    value
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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

fn result_data_string(result: &ToolResult, key: &str) -> Option<String> {
    result
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn result_data_u64(result: &ToolResult, key: &str) -> Option<u64> {
    result
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_u64())
}

fn result_data_bool(result: &ToolResult, key: &str) -> Option<bool> {
    result
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_bool())
}

fn preview(text: &str) -> String {
    let trimmed = text.trim();
    let mut out = trimmed.chars().take(PREVIEW_CHARS).collect::<String>();
    if trimmed.chars().count() > PREVIEW_CHARS {
        out.push_str("...");
    }
    out
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn validation_identity(fact: &ValidationEvidence) -> String {
    if let Some(command) = fact.command.as_deref() {
        let normalized = normalize_command_identity(command);
        if !normalized.is_empty() {
            return format!("command:{normalized}");
        }
    }
    format!("source:{}", fact.source.trim())
}

fn normalize_command_identity(command: &str) -> String {
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn command_fact_satisfies_required(fact: &CommandEvidence, required_command: &str) -> bool {
    let executed = normalize_command_identity(&fact.command);
    executed == required_command
        || (fact.safe_for_closeout && shell_assertion_covers_required(&executed, required_command))
}

fn shell_assertion_covers_required(executed: &str, required_command: &str) -> bool {
    if !(required_command.starts_with("test ")
        || required_command.starts_with("[ ")
        || required_command.starts_with("[[ "))
    {
        return false;
    }
    let Some(rest) = executed.strip_prefix(required_command) else {
        return false;
    };
    let rest = rest.trim_start();
    rest.starts_with("&& ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: name.to_string(),
            arguments: args,
        }
    }

    #[test]
    fn records_file_write_as_changed_file_fact() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call("file_write", serde_json::json!({"path": "src/app.py"})),
            &ToolResult::success("Wrote file"),
        );

        let snapshot = ledger.snapshot();
        assert_eq!(snapshot.changed_files, vec!["src/app.py".to_string()]);
        assert_eq!(snapshot.tool_execution_records, 1);
        assert_eq!(snapshot.file_facts, 1);
        assert_eq!(snapshot.command_facts, 0);
    }

    #[test]
    fn records_file_read_fact_metadata_from_tool_result_data() {
        let mut ledger = EvidenceLedger::new();
        let result = ToolResult::success_with_data(
            "   2 | beta\n   3 | gamma",
            serde_json::json!({
                "kind": "file",
                "line_start": 2,
                "line_end": 3,
                "total_lines": 3,
                "displayed_lines": 2,
                "truncated": true,
                "content_hash": "abc123",
                "display_format": "line_numbered_content"
            }),
        );

        ledger.record_tool_result(
            &tool_call("file_read", serde_json::json!({"path": "src/lib.rs"})),
            &result,
        );

        let fact = &ledger.file_facts[0];
        assert_eq!(fact.kind.as_deref(), Some("file"));
        assert_eq!(fact.line_start, Some(2));
        assert_eq!(fact.line_end, Some(3));
        assert_eq!(fact.total_lines, Some(3));
        assert_eq!(fact.displayed_lines, Some(2));
        assert_eq!(fact.truncated, Some(true));
        assert_eq!(fact.content_hash.as_deref(), Some("abc123"));
        assert!(fact.summary.contains("line_numbered_content"));
    }

    #[test]
    fn records_file_edit_diagnostics_metadata_from_tool_result_data() {
        let mut ledger = EvidenceLedger::new();
        let result = ToolResult::success_with_data(
            "File edited successfully: src/lib.rs (1 replacement(s))",
            serde_json::json!({
                "path": "src/lib.rs",
                "replacements": 1,
                "diagnostics": {
                    "checked": true,
                    "status": "diagnostics_found",
                    "diagnostic_count": 2,
                    "error_count": 1,
                    "warning_count": 1,
                    "first_error": {
                        "message": "type mismatch in return value",
                        "source": "rust-analyzer",
                        "code": "E0308",
                        "range": {
                            "start_line": 7
                        }
                    }
                }
            }),
        );

        ledger.record_tool_result(
            &tool_call("file_edit", serde_json::json!({"path": "src/lib.rs"})),
            &result,
        );

        let fact = &ledger.file_facts[0];
        assert!(fact
            .summary
            .contains("lsp_diagnostics=status:diagnostics_found"));
        assert!(fact.summary.contains("errors:1"));
        assert!(fact.summary.contains("warnings:1"));
        assert!(fact.summary.contains("first_error:line:7"));
        assert!(fact.summary.contains("source:rust-analyzer"));
        assert!(fact.summary.contains("code:E0308"));
    }

    #[test]
    fn records_file_patch_files_as_changed_file_facts() {
        let mut ledger = EvidenceLedger::new();
        let result = ToolResult::success_with_data(
            "Applied file_patch successfully: 2 operation(s), 2 file(s)",
            serde_json::json!({
                "files": [
                    {
                        "path": "src/lib.rs",
                        "replacements": 1,
                        "bytes_written": 42,
                        "diff": {
                            "additions": 1,
                            "deletions": 1,
                            "changed_line_start": 3,
                            "changed_line_end": 3,
                            "preview_truncated": false
                        }
                    },
                    {
                        "path": "README.md",
                        "replacements": 1,
                        "bytes_written": 20,
                        "diff": {
                            "additions": 2,
                            "deletions": 1,
                            "changed_line_start": 7,
                            "changed_line_end": 8,
                            "preview_truncated": false
                        }
                    }
                ]
            }),
        );

        ledger.record_tool_result(
            &tool_call(
                "file_patch",
                serde_json::json!({
                    "operations": [
                        {"path": "src/lib.rs"},
                        {"path": "README.md"}
                    ]
                }),
            ),
            &result,
        );

        let snapshot = ledger.snapshot();
        assert_eq!(
            snapshot.changed_files,
            vec!["README.md".to_string(), "src/lib.rs".to_string()]
        );
        assert_eq!(snapshot.file_facts, 2);
        let fact = &ledger.file_facts[0];
        assert_eq!(fact.tool, "file_patch");
        assert_eq!(fact.path.as_deref(), Some("src/lib.rs"));
        assert_eq!(fact.kind.as_deref(), Some("patch"));
        assert_eq!(fact.line_start, Some(3));
        assert_eq!(fact.line_end, Some(3));
        assert_eq!(fact.truncated, Some(false));
        assert!(fact.summary.contains("bytes_written=42"));
        assert!(fact.summary.contains("changed_line_start=3"));
    }

    #[test]
    fn records_safe_bash_validation_as_command_and_validation_fact() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "cargo test -q"})),
            &ToolResult::success("test result: ok"),
        );

        let snapshot = ledger.snapshot();
        assert_eq!(snapshot.command_facts, 1);
        assert_eq!(snapshot.validation_facts, 1);
        assert_eq!(snapshot.passed_validation_facts, 1);
        assert_eq!(
            ledger.runtime_validation_label().as_deref(),
            Some("passed:1/1")
        );
    }

    #[test]
    fn records_tool_execution_record_with_command_and_terminal_metadata() {
        let mut ledger = EvidenceLedger::new();
        let mut result = ToolResult::success_with_data(
            "test result: ok",
            serde_json::json!({
                "terminal_task": {
                    "task_id": "shell_foreground_123",
                    "status": "completed",
                    "terminal_kind": "foreground_shell",
                    "duration_ms": 42,
                    "exit_code": 0
                },
                "tool_summary": {
                    "tool": "bash",
                    "call_id": "call_1",
                    "success": true,
                    "duration_ms": 42,
                    "output_chars": 15,
                    "command": "cargo test -q",
                    "command_kind": "validation",
                    "command_category": "test_run",
                    "validation_family": "cargo_test",
                    "safe_for_closeout": true,
                    "terminal_task": {
                        "task_id": "shell_foreground_123",
                        "status": "completed",
                        "terminal_kind": "foreground_shell",
                        "duration_ms": 42,
                        "exit_code": 0
                    }
                },
                "tool_runtime": {
                    "route": {
                        "intent": "code_change",
                        "workflow": "code_change",
                        "retrieval": "project",
                        "reasoning": "high",
                        "risk": "medium"
                    },
                    "policy": {
                        "latency": "deep",
                        "parallelism_limit": 4,
                        "max_tool_calls": 30,
                        "context_budget_tokens": 64000,
                        "allow_fallback_model": true,
                        "cost_ceiling_usd": "0.2500"
                    },
                    "execution": {
                        "parallel": false,
                        "pre_executed": false,
                        "action_checkpoint_active": true,
                        "has_changes_before_tools": true,
                        "exposed_tools_count": 15
                    }
                }
            }),
        );
        result.duration_ms = Some(42);

        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "cargo test -q"})),
            &result,
        );

        let records = ledger.tool_execution_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool, "bash");
        assert_eq!(records[0].status, ToolExecutionStatus::Completed);
        assert_eq!(records[0].arguments_hash.len(), HASH_PREVIEW_CHARS);
        assert_eq!(records[0].command.as_deref(), Some("cargo test -q"));
        assert_eq!(records[0].command_kind.as_deref(), Some("validation"));
        assert_eq!(records[0].validation_family.as_deref(), Some("cargo_test"));
        assert_eq!(records[0].safe_for_closeout, Some(true));
        assert_eq!(
            records[0].relevance,
            ToolExecutionRelevance {
                validation: true,
                closeout: true,
                repair: false,
            }
        );
        assert_eq!(
            records[0]
                .terminal_task
                .as_ref()
                .and_then(|task| task.task_id.as_deref()),
            Some("shell_foreground_123")
        );
        assert_eq!(
            records[0]
                .execution
                .route
                .as_ref()
                .and_then(|route| route.workflow.as_deref()),
            Some("code_change")
        );
        assert_eq!(
            records[0]
                .execution
                .policy
                .as_ref()
                .and_then(|policy| policy.max_tool_calls),
            Some(30)
        );
        assert!(records[0].execution.action_checkpoint_active);
        assert!(records[0].execution.has_changes_before_tools);
        assert_eq!(records[0].execution.exposed_tools_count, Some(15));
    }

    #[test]
    fn records_denied_permission_execution_record() {
        let mut ledger = EvidenceLedger::new();
        let mut result = ToolResult::error("permission denied");
        result.data = Some(serde_json::json!({
            "permission_request": {
                "id": "git_push",
                "session_id": "session-1",
                "kind": "write",
                "approved": false,
                "patterns": ["file_write"],
                "allowed_always_rules": ["file_read"],
                "metadata": {
                    "permission_requires": true,
                    "tool_requires": false,
                    "raw_tool_requires": false,
                    "drift_requires_approval": false,
                    "permission_family": "file",
                    "permission_decision": "Ask",
                    "risk_level": "High"
                },
                "rejection_feedback": "Denied by policy"
            }
        }));

        ledger.record_tool_result(
            &tool_call("file_write", serde_json::json!({"path": "src/lib.rs"})),
            &result,
        );

        let record = &ledger.tool_execution_records()[0];
        assert_eq!(record.status, ToolExecutionStatus::Denied);
        assert_eq!(
            record.relevance,
            ToolExecutionRelevance {
                validation: false,
                closeout: true,
                repair: true,
            }
        );
        assert_eq!(
            record
                .permission
                .as_ref()
                .and_then(|permission| permission.kind.as_deref()),
            Some("write")
        );
        let permission = record.permission.as_ref().unwrap();
        assert!(!permission.approved);
        assert_eq!(permission.request_id.as_deref(), Some("git_push"));
        assert_eq!(permission.session_id.as_deref(), Some("session-1"));
        assert_eq!(permission.patterns, vec!["file_write"]);
        assert_eq!(permission.allowed_always_rules, vec!["file_read"]);
        assert_eq!(permission.source.permission_requires, Some(true));
        assert_eq!(permission.source.permission_family.as_deref(), Some("file"));
        assert_eq!(
            permission.source.permission_decision.as_deref(),
            Some("Ask")
        );
        assert_eq!(permission.source.risk_level.as_deref(), Some("High"));
        assert_eq!(ledger.snapshot().denied_permission_facts, 1);
    }

    #[test]
    fn approved_permission_record_keeps_failed_tool_status_failed() {
        let mut ledger = EvidenceLedger::new();
        let mut result = ToolResult::error("remote rejected push");
        result.data = Some(serde_json::json!({
            "permission_request": {
                "id": "git_push",
                "session_id": "session-1",
                "kind": "runtime_rule",
                "approved": true,
                "patterns": ["git"],
                "metadata": {
                    "permission_requires": true,
                    "tool_requires": false,
                    "raw_tool_requires": false,
                    "drift_requires_approval": false,
                    "permission_family": "other",
                    "permission_decision": "Ask",
                    "risk_level": "High"
                },
                "rejection_feedback": "Permission denied: 'git' requires user confirmation."
            }
        }));

        ledger.record_tool_result(
            &tool_call("git", serde_json::json!({"action": "push"})),
            &result,
        );

        let record = &ledger.tool_execution_records()[0];
        assert_eq!(record.status, ToolExecutionStatus::Failed);
        assert!(record.permission.as_ref().unwrap().approved);
        assert_eq!(ledger.snapshot().permission_facts, 1);
        assert_eq!(ledger.snapshot().denied_permission_facts, 0);
    }

    #[test]
    fn records_shell_assertion_as_runtime_validation() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call(
                "bash",
                serde_json::json!({
                    "command": "test -d fixtures/core_quality/inspection_target/gex && echo PASS"
                }),
            ),
            &ToolResult::success("PASS: directory exists"),
        );

        let snapshot = ledger.snapshot();
        assert_eq!(snapshot.command_facts, 1);
        assert_eq!(snapshot.validation_facts, 1);
        assert_eq!(snapshot.passed_validation_facts, 1);
        assert_eq!(
            ledger.runtime_validation_label().as_deref(),
            Some("passed:1/1")
        );
    }

    #[test]
    fn required_validation_label_uses_required_commands_over_exploratory_failures() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call(
                "bash",
                serde_json::json!({
                    "command": "python3 -c \"import core_terminal_demo; print('import ok')\""
                }),
            ),
            &ToolResult::error("ModuleNotFoundError: No module named 'core_terminal_demo'"),
        );
        ledger.record_tool_result(
            &tool_call(
                "bash",
                serde_json::json!({
                    "command": "test -x .venv/bin/python && echo \"PASS: .venv/bin/python exists\""
                }),
            ),
            &ToolResult::success("PASS: .venv/bin/python exists"),
        );
        ledger.record_tool_result(
            &tool_call(
                "bash",
                serde_json::json!({
                    "command": ". .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'"
                }),
            ),
            &ToolResult::success("core-terminal-demo-ok"),
        );
        let required = vec![
            "test -x .venv/bin/python".to_string(),
            ". .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'".to_string(),
        ];

        assert_eq!(
            ledger.runtime_validation_label().as_deref(),
            Some("failed:1/2")
        );
        assert_eq!(
            ledger
                .runtime_required_validation_label(&required)
                .as_deref(),
            Some("passed:2/2")
        );
    }

    #[test]
    fn records_bash_validation_from_result_metadata_when_call_arguments_are_missing() {
        let mut ledger = EvidenceLedger::new();
        let result = ToolResult::success_with_data(
            "PASS: directory exists",
            serde_json::json!({
                "shell_result": {
                    "command": "if test -d fixtures/core_quality/inspection_target/gex; then echo PASS; else echo FAIL; fi"
                }
            }),
        );

        ledger.record_tool_result(
            &ToolCall {
                id: "call_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({}),
            },
            &result,
        );

        let snapshot = ledger.snapshot();
        assert_eq!(snapshot.command_facts, 1);
        assert_eq!(snapshot.validation_facts, 1);
        assert_eq!(
            ledger.runtime_validation_label().as_deref(),
            Some("passed:1/1")
        );
    }

    #[test]
    fn records_permission_denial_as_permission_fact() {
        let mut ledger = EvidenceLedger::new();
        let mut result = ToolResult::error("Permission denied: 'git' requires user confirmation.");
        result.error_code = Some(crate::tools::ToolErrorCode::PermissionDenied);
        result.data = Some(serde_json::json!({
            "permission_request": {
                "kind": "runtime_rule",
                "rejection_feedback": "Permission denied: 'git' requires user confirmation.",
                "recovery_feedback": "Ask the user to approve git push before retrying."
            }
        }));
        ledger.record_tool_result(
            &tool_call("git", serde_json::json!({"action": "push"})),
            &result,
        );

        let snapshot = ledger.snapshot();
        assert_eq!(snapshot.permission_facts, 1);
        assert_eq!(snapshot.denied_permission_facts, 1);
        assert_eq!(ledger.permission_facts()[0].tool, "git");
        assert_eq!(
            ledger.permission_facts()[0].kind.as_deref(),
            Some("runtime_rule")
        );
        assert!(ledger.permission_facts()[0]
            .summary
            .contains("Recovery: Ask the user"));
    }

    #[test]
    fn failed_validation_label_names_failures() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_validation_result("auto_verify", Some("cargo check"), false, "compile error");

        assert_eq!(
            ledger.runtime_validation_label().as_deref(),
            Some("failed:1/1")
        );
        assert_eq!(ledger.validation_facts()[0].summary, "compile error");
    }

    #[test]
    fn runtime_validation_label_uses_latest_result_per_command() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_validation_result(
            "bash",
            Some("cargo test -q tui -- --test-threads=1"),
            false,
            "provider header panic",
        );
        ledger.record_validation_result(
            "required_validation",
            Some("cargo    test -q tui -- --test-threads=1"),
            true,
            "test result: ok",
        );
        ledger.record_validation_result("code_review", None, true, "review passed");

        assert_eq!(
            ledger.runtime_validation_label().as_deref(),
            Some("passed:2/2 recovered_failed:1")
        );
        let snapshot = ledger.snapshot();
        assert_eq!(snapshot.validation_facts, 3);
        assert_eq!(snapshot.failed_validation_facts, 1);
    }

    #[test]
    fn runtime_validation_label_keeps_unrecovered_failures_current() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_validation_result(
            "bash",
            Some("cargo test -q tui -- --test-threads=1"),
            false,
            "test failed",
        );
        ledger.record_validation_result(
            "required_validation",
            Some("cargo test -q shell -- --test-threads=1"),
            true,
            "test result: ok",
        );

        assert_eq!(
            ledger.runtime_validation_label().as_deref(),
            Some("failed:1/2")
        );
    }

    #[tokio::test]
    async fn changed_files_diff_evidence_skips_empty_input() {
        let evidence = changed_files_diff_evidence(Path::new("."), &[]).await;

        assert!(evidence.is_none());
    }

    #[test]
    fn filesystem_grounding_flags_creation_time_without_evidence() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call(
                "bash",
                serde_json::json!({"command": "ls -la ~/Desktop | grep -i gex"}),
            ),
            &ToolResult::success("drwxr-xr-x  3 gex  staff  96 May 8  2024 gex"),
        );

        let gaps = ledger.unsupported_filesystem_claims("创建时间：2024 年 5 月 8 日");

        assert_eq!(gaps, vec!["creation_time".to_string()]);
    }

    #[test]
    fn filesystem_grounding_allows_creation_time_with_stat_evidence() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call(
                "bash",
                serde_json::json!({"command": "stat -f '%SB' ~/Desktop/gex"}),
            ),
            &ToolResult::success("May 8 00:00:00 2024\ncreated at"),
        );

        let gaps = ledger.unsupported_filesystem_claims("创建时间：May 8 00:00:00 2024");

        assert!(gaps.is_empty());
    }
}
