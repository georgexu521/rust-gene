//! Runtime-owned evidence ledger for facts the model must not invent.
//!
//! The ledger keeps file, command, and validation facts as structured runtime
//! data. User-facing summaries and workflow closeout can then cite evidence
//! without mixing raw tool metadata into model-visible text.

use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

const PREVIEW_CHARS: usize = 240;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceLedger {
    changed_files: BTreeSet<String>,
    file_facts: Vec<FileEvidence>,
    command_facts: Vec<CommandEvidence>,
    validation_facts: Vec<ValidationEvidence>,
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
pub struct EvidenceSnapshot {
    pub changed_files: Vec<String>,
    pub file_facts: usize,
    pub command_facts: usize,
    pub validation_facts: usize,
    pub passed_validation_facts: usize,
    pub failed_validation_facts: usize,
}

impl EvidenceLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_tool_result(&mut self, tool_call: &ToolCall, result: &ToolResult) {
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
        EvidenceSnapshot {
            changed_files: self.changed_files.iter().cloned().collect(),
            file_facts: self.file_facts.len(),
            command_facts: self.command_facts.len(),
            validation_facts: self.validation_facts.len(),
            passed_validation_facts,
            failed_validation_facts,
        }
    }

    pub fn runtime_validation_label(&self) -> Option<String> {
        let snapshot = self.snapshot();
        if snapshot.validation_facts == 0 {
            return None;
        }
        if snapshot.failed_validation_facts > 0 {
            Some(format!(
                "failed:{}/{}",
                snapshot.failed_validation_facts, snapshot.validation_facts
            ))
        } else {
            Some(format!(
                "passed:{}/{}",
                snapshot.passed_validation_facts, snapshot.validation_facts
            ))
        }
    }

    pub fn changed_files(&self) -> Vec<String> {
        self.changed_files.iter().cloned().collect()
    }

    pub fn validation_facts(&self) -> &[ValidationEvidence] {
        &self.validation_facts
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
    fn failed_validation_label_names_failures() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_validation_result("auto_verify", Some("cargo check"), false, "compile error");

        assert_eq!(
            ledger.runtime_validation_label().as_deref(),
            Some("failed:1/1")
        );
        assert_eq!(ledger.validation_facts()[0].summary, "compile error");
    }
}
