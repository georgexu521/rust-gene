//! Runtime-owned evidence ledger for facts the model must not invent.
//!
//! The ledger keeps file, command, and validation facts as structured runtime
//! data. User-facing summaries and workflow closeout can then cite evidence
//! without mixing raw tool metadata into model-visible text.

use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const PREVIEW_CHARS: usize = 240;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceLedger {
    changed_files: BTreeSet<String>,
    file_facts: Vec<FileEvidence>,
    command_facts: Vec<CommandEvidence>,
    validation_facts: Vec<ValidationEvidence>,
    permission_facts: Vec<PermissionEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEvidence {
    pub tool: String,
    pub path: Option<String>,
    pub success: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandEvidence {
    pub command: String,
    pub success: bool,
    pub command_kind: Option<String>,
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
        self.record_permission_tool_result(tool_call, result);
        match tool_call.name.as_str() {
            "bash" => self.record_bash_tool_result(tool_call, result),
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
        let Some(command) = tool_call.arguments["command"].as_str() else {
            return;
        };
        let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
        let command_kind = serde_json::to_value(classification.command_kind)
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
            validation_family,
            safe_for_closeout,
            summary: summary.clone(),
        });
        if classification.is_safe_validation() {
            self.record_validation_result("bash", Some(command), result.success, summary);
        }
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
            summary: result_summary(result),
        });
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
        self.permission_facts.push(PermissionEvidence {
            tool: tool_call.name.clone(),
            kind,
            approved: result.success,
            summary: preview(summary),
        });
    }
}

fn result_summary(result: &ToolResult) -> String {
    let text = if !result.content.trim().is_empty() {
        result.content.as_str()
    } else {
        result.error.as_deref().unwrap_or("")
    };
    preview(text)
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
        let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ");
        if !normalized.is_empty() {
            return format!("command:{normalized}");
        }
    }
    format!("source:{}", fact.source.trim())
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
        assert_eq!(snapshot.file_facts, 1);
        assert_eq!(snapshot.command_facts, 0);
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
    fn records_permission_denial_as_permission_fact() {
        let mut ledger = EvidenceLedger::new();
        let mut result = ToolResult::error("Permission denied: 'git' requires user confirmation.");
        result.error_code = Some(crate::tools::ToolErrorCode::PermissionDenied);
        result.data = Some(serde_json::json!({
            "permission_request": {
                "kind": "runtime_rule",
                "rejection_feedback": "Permission denied: 'git' requires user confirmation."
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
