//! Runtime-owned evidence ledger for facts the model must not invent.
//!
//! The ledger keeps file, command, and validation facts as structured runtime
//! data. User-facing summaries and workflow closeout can then cite evidence
//! without mixing raw tool metadata into model-visible text.

use crate::engine::verification_proof::{
    VerificationProof, VerificationProofKind, VerificationProofRequest, VerificationProofStatus,
    VerificationProofSupportContext, VerificationProofTaskType,
};
use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

const PREVIEW_CHARS: usize = 240;
const HASH_PREVIEW_CHARS: usize = 16;
const MAX_REPAIR_TOOL_RECORDS: usize = 8;

mod records;
pub use records::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceLedger {
    changed_files: BTreeSet<String>,
    tool_execution_records: Vec<ToolExecutionRecord>,
    file_facts: Vec<FileEvidence>,
    command_facts: Vec<CommandEvidence>,
    validation_facts: Vec<ValidationEvidence>,
    permission_facts: Vec<PermissionEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidationRollup {
    current_total: usize,
    current_passed: usize,
    current_failed: usize,
    recovered_failed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequiredValidationRollup {
    total: usize,
    passed: usize,
    failed: usize,
    missing: usize,
    recovered_failed: usize,
    passed_commands: Vec<String>,
    failed_commands: Vec<String>,
    missing_commands: Vec<String>,
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
        let file_fact_start = self.file_facts.len();
        match tool_call.name.as_str() {
            "file_patch" => self.record_file_patch_tool_result(result),
            "file_read" | "file_write" | "file_edit" | "glob" | "grep" => {
                self.record_file_tool_result(tool_call, result)
            }
            _ => {}
        }
        let file_evidence = self.file_evidence_links_since(file_fact_start);
        self.record_tool_execution_record(tool_call, result, file_evidence);
        self.record_permission_tool_result(tool_call, result);
        if tool_call.name == "bash" {
            self.record_bash_tool_result(tool_call, result);
        }
        if tool_call.name == "agent" {
            self.record_agent_tool_result(result);
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
        self.record_validation_result_with_kind(source, command, passed, summary, None);
    }

    pub fn record_validation_result_with_kind(
        &mut self,
        source: impl Into<String>,
        command: Option<&str>,
        passed: bool,
        summary: impl AsRef<str>,
        proof_kind: Option<VerificationProofKind>,
    ) {
        self.validation_facts.push(ValidationEvidence {
            source: source.into(),
            command: command.map(str::to_string),
            passed,
            summary: preview(summary.as_ref()),
            proof_kind,
            scope: None,
            command_status: Some(if passed { "passed" } else { "failed" }.to_string()),
            validation_family: None,
            source_agent: None,
            parent_verified: None,
            related_to_changed_files: None,
            residual_risk: None,
            claim_id: None,
            claim_type: None,
            parent_command: None,
            artifact_ids: Vec::new(),
            verification_verdict: None,
            verified_at: None,
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
        let rollup = self.required_validation_rollup(required_commands)?;
        if rollup.missing > 0 {
            return None;
        }

        if rollup.failed > 0 {
            return Some(format!("failed:{}/{}", rollup.failed, rollup.total));
        }
        let mut label = format!("passed:{}/{}", rollup.passed, rollup.total);
        if rollup.recovered_failed > 0 {
            label.push_str(&format!(" recovered_failed:{}", rollup.recovered_failed));
        }
        Some(label)
    }

    pub fn verification_proof(&self, request: VerificationProofRequest<'_>) -> VerificationProof {
        use crate::engine::task_context::VerificationStatus;

        let required_rollup = self.required_validation_rollup(request.required_commands);
        let validation_rollup = self.current_validation_rollup();
        let proof_kinds =
            self.proof_kinds_for_rollups(required_rollup.as_ref(), validation_rollup.as_ref());
        let direct_read_only_without_required_validation = !request.requires_validation
            && request.required_commands.is_empty()
            && matches!(
                request.support_context.task_type,
                VerificationProofTaskType::DirectAnswer | VerificationProofTaskType::ReadOnlyAudit
            );

        let mut proof = if let Some(rollup) = required_rollup.as_ref() {
            if rollup.missing > 0 {
                VerificationProof::new(
                    VerificationProofStatus::NotRun,
                    format!(
                        "required validation missing {}/{} commands",
                        rollup.missing, rollup.total
                    ),
                )
            } else if rollup.failed > 0 {
                VerificationProof::new(
                    VerificationProofStatus::Failed,
                    format!(
                        "required validation failed {}/{} commands",
                        rollup.failed, rollup.total
                    ),
                )
            } else {
                VerificationProof::new(
                    VerificationProofStatus::Verified,
                    format!(
                        "required validation passed {}/{} commands",
                        rollup.passed, rollup.total
                    ),
                )
            }
        } else if request.task_verification_status == VerificationStatus::Blocked {
            VerificationProof::new(VerificationProofStatus::Blocked, "verification is blocked")
        } else if request.task_verification_status == VerificationStatus::UserDeferred {
            VerificationProof::new(
                VerificationProofStatus::UserDeferred,
                "user deferred verification",
            )
        } else if request.task_verification_status == VerificationStatus::Unavailable {
            VerificationProof::new(
                VerificationProofStatus::Unavailable,
                "verification evidence is unavailable",
            )
        } else if !request.required_commands.is_empty() {
            VerificationProof::new(
                VerificationProofStatus::NotRun,
                "required validation commands were not recognized",
            )
        } else if let Some(rollup) = validation_rollup {
            if rollup.current_failed > 0 {
                VerificationProof::new(
                    VerificationProofStatus::Failed,
                    format!(
                        "validation failed {}/{} current checks",
                        rollup.current_failed, rollup.current_total
                    ),
                )
            } else if rollup.current_passed > 0 {
                let mut proof = VerificationProof::new(
                    VerificationProofStatus::Verified,
                    format!(
                        "validation passed {}/{} current checks",
                        rollup.current_passed, rollup.current_total
                    ),
                );
                proof.recovered_failed = rollup.recovered_failed;
                proof
            } else if request.requires_validation {
                VerificationProof::new(
                    VerificationProofStatus::NotRun,
                    "validation required but no passing evidence was recorded",
                )
            } else {
                VerificationProof::new(
                    VerificationProofStatus::NotApplicable,
                    "validation not applicable to this task",
                )
            }
        } else if direct_read_only_without_required_validation {
            VerificationProof::new(
                VerificationProofStatus::NotApplicable,
                "validation not required for read-only direct answer",
            )
        } else {
            match request.task_verification_status {
                VerificationStatus::Verified => VerificationProof::new(
                    VerificationProofStatus::Unavailable,
                    "task state says verified, but ledger has no verification evidence",
                ),
                VerificationStatus::Failed => VerificationProof::new(
                    VerificationProofStatus::Failed,
                    "task state reports failed verification without ledger evidence",
                ),
                VerificationStatus::NotRequired => VerificationProof::new(
                    VerificationProofStatus::NotApplicable,
                    "validation not required for this task",
                ),
                VerificationStatus::Pending if request.requires_validation => {
                    VerificationProof::new(
                        VerificationProofStatus::NotRun,
                        "validation required but no evidence was recorded",
                    )
                }
                VerificationStatus::Pending => VerificationProof::new(
                    VerificationProofStatus::NotApplicable,
                    "no validation requirement was recorded",
                ),
                VerificationStatus::Blocked
                | VerificationStatus::UserDeferred
                | VerificationStatus::Unavailable => unreachable!("handled above"),
            }
        };

        if let Some(rollup) = required_rollup {
            proof.required_total = rollup.total;
            proof.required_passed = rollup.passed;
            proof.required_failed = rollup.failed;
            proof.required_missing = rollup.missing;
            proof.recovered_failed = rollup.recovered_failed;
            proof.passed_commands = rollup.passed_commands;
            proof.failed_commands = rollup.failed_commands;
            proof.missing_required_commands = rollup.missing_commands;
        }
        if let Some(rollup) = validation_rollup {
            proof.validation_total = rollup.current_total;
            proof.validation_passed = rollup.current_passed;
            proof.validation_failed = rollup.current_failed;
            proof.recovered_failed = proof.recovered_failed.max(rollup.recovered_failed);
        }
        proof.evidence_items = self.validation_facts.len();
        proof.proof_kinds = proof_kinds;
        proof.apply_derived_support(request.support_context);
        proof
    }

    pub fn verification_proof_support_context(
        &self,
        task_type: VerificationProofTaskType,
        required_commands: &[String],
    ) -> VerificationProofSupportContext {
        let required_rollup = self.required_validation_rollup(required_commands);
        let validation_rollup = self.current_validation_rollup();
        let required_passed = required_rollup.as_ref().is_some_and(|rollup| {
            rollup.total > 0 && rollup.missing == 0 && rollup.failed == 0 && rollup.passed > 0
        });
        let validation_passed = validation_rollup.is_some_and(|rollup| {
            rollup.current_total > 0 && rollup.current_failed == 0 && rollup.current_passed > 0
        });
        let accepted_validation_family = required_passed
            || self.validation_facts.iter().any(|fact| {
                fact.passed
                    && fact
                        .validation_family
                        .as_deref()
                        .is_some_and(|family| !family.trim().is_empty())
            });
        let parent_verified = self.parent_verified_subagent_result();

        VerificationProofSupportContext {
            task_type,
            accepted_validation_family,
            focused_validation_passed: required_passed || validation_passed,
            parent_verified,
        }
    }

    fn proof_kinds_for_rollups(
        &self,
        required_rollup: Option<&RequiredValidationRollup>,
        validation_rollup: Option<&ValidationRollup>,
    ) -> Vec<VerificationProofKind> {
        let mut kinds = self
            .validation_facts
            .iter()
            .filter_map(|fact| fact.proof_kind)
            .collect::<BTreeSet<_>>();
        let has_explicit_kinds = !kinds.is_empty();

        if self.parent_verified_subagent_result() {
            kinds.insert(VerificationProofKind::ParentVerifiedSubagentResult);
        }

        if required_rollup.is_some_and(|rollup| {
            rollup.total > 0 && rollup.missing == 0 && rollup.failed == 0 && rollup.passed > 0
        }) {
            kinds.insert(VerificationProofKind::RequiredValidationPassed);
            kinds.insert(VerificationProofKind::CommandPassed);
        } else if !has_explicit_kinds
            && validation_rollup.is_some_and(|rollup| {
                rollup.current_total > 0 && rollup.current_failed == 0 && rollup.current_passed > 0
            })
        {
            kinds.insert(VerificationProofKind::CommandPassed);
        }

        kinds.into_iter().collect()
    }

    fn parent_verified_subagent_result(&self) -> bool {
        self.explicit_parent_verified_subagent_result()
    }

    fn explicit_parent_verified_subagent_result(&self) -> bool {
        self.validation_facts.iter().any(|fact| {
            fact.passed
                && fact.parent_verified == Some(true)
                && fact.proof_kind == Some(VerificationProofKind::ParentVerifiedSubagentResult)
        })
    }

    fn required_validation_rollup(
        &self,
        required_commands: &[String],
    ) -> Option<RequiredValidationRollup> {
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

        let mut passed_commands = Vec::new();
        let mut failed_commands = Vec::new();
        let mut missing_commands = Vec::new();
        for command in &required {
            match current.get(command).copied() {
                Some(true) => passed_commands.push(command.clone()),
                Some(false) => failed_commands.push(command.clone()),
                None => missing_commands.push(command.clone()),
            }
        }
        let recovered_failed = required
            .iter()
            .filter(|command| {
                failed_identities.contains(*command)
                    && current.get(*command).copied().unwrap_or(false)
            })
            .count();

        Some(RequiredValidationRollup {
            total: required.len(),
            passed: passed_commands.len(),
            failed: failed_commands.len(),
            missing: missing_commands.len(),
            recovered_failed,
            passed_commands,
            failed_commands,
            missing_commands,
        })
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

    pub fn closeout_tool_evidence_summary(&self) -> Option<String> {
        if self.tool_execution_records.is_empty() {
            return None;
        }

        let completed = self
            .tool_execution_records
            .iter()
            .filter(|record| record.status == ToolExecutionStatus::Completed)
            .count();
        let failed = self
            .tool_execution_records
            .iter()
            .filter(|record| record.status == ToolExecutionStatus::Failed)
            .count();
        let denied = self
            .tool_execution_records
            .iter()
            .filter(|record| record.status == ToolExecutionStatus::Denied)
            .count();
        let validation = self
            .tool_execution_records
            .iter()
            .filter(|record| record.relevance.validation)
            .count();
        let closeout = self
            .tool_execution_records
            .iter()
            .filter(|record| record.relevance.closeout)
            .count();
        let repair = self
            .tool_execution_records
            .iter()
            .filter(|record| record.relevance.repair)
            .count();
        let changed = self
            .tool_execution_records
            .iter()
            .filter(|record| !record.changed_paths.is_empty())
            .count();
        let mut workflows = BTreeSet::new();
        let mut commands = Vec::new();
        for record in &self.tool_execution_records {
            if let Some(workflow) = record
                .execution
                .route
                .as_ref()
                .and_then(|route| route.workflow.as_deref())
            {
                workflows.insert(workflow.to_string());
            }
            if let Some(command) = record.command.as_deref() {
                if commands.len() < 3 {
                    commands.push(preview(command));
                }
            }
        }
        let workflow_label = if workflows.is_empty() {
            "none".to_string()
        } else {
            workflows.into_iter().collect::<Vec<_>>().join(",")
        };
        let command_label = if commands.is_empty() {
            "none".to_string()
        } else {
            commands.join(" | ")
        };

        Some(preview(&format!(
            "tool evidence: records={} completed={} failed={} denied={} validation={} closeout={} repair={} changed={} workflows={} commands={}",
            self.tool_execution_records.len(),
            completed,
            failed,
            denied,
            validation,
            closeout,
            repair,
            changed,
            workflow_label,
            command_label
        )))
    }

    pub fn repair_tool_record_evidence(&self, failed_commands: &[String]) -> Vec<String> {
        if self.tool_execution_records.is_empty() {
            return Vec::new();
        }

        let failed_command_identities = failed_commands
            .iter()
            .map(|command| normalize_command_identity(command))
            .filter(|command| !command.is_empty())
            .collect::<BTreeSet<_>>();

        let mut evidence = Vec::new();
        for record in self.tool_execution_records.iter().rev() {
            if evidence.len() >= MAX_REPAIR_TOOL_RECORDS {
                break;
            }
            if !tool_record_relevant_for_repair(record, &failed_command_identities) {
                continue;
            }
            evidence.push(format_repair_tool_record(record, &self.file_facts));
        }
        evidence
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
            normalized_command: normalize_command_identity(command),
            success: result.success,
            command_kind,
            command_category,
            validation_family,
            path_patterns: classification.path_patterns.clone(),
            absolute_path_patterns: classification.absolute_path_patterns.clone(),
            safe_for_closeout,
            network_access: classification.network_access,
            external_path_access: classification.external_path_access,
            compound_command: classification.compound_command,
            shell_control_operators: classification.shell_control_operators.clone(),
            risky_shell_wrapper: classification.risky_shell_wrapper,
            expected_silent_output: classification.expected_silent_output,
            summary: summary.clone(),
        });
        if classification.is_safe_validation() {
            self.record_validation_result("bash", Some(command), result.success, summary);
        }
    }

    fn file_evidence_links_since(&self, start: usize) -> Vec<ToolFileEvidenceLink> {
        self.file_facts
            .iter()
            .enumerate()
            .skip(start)
            .map(|(fact_index, fact)| ToolFileEvidenceLink {
                fact_index,
                path: fact.path.clone(),
                kind: fact.kind.clone(),
                line_start: fact.line_start,
                line_end: fact.line_end,
                content_hash: fact.content_hash.clone(),
            })
            .collect()
    }

    fn record_tool_execution_record(
        &mut self,
        tool_call: &ToolCall,
        result: &ToolResult,
        file_evidence: Vec<ToolFileEvidenceLink>,
    ) {
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
        let normalized_command = command.as_deref().map(normalize_command_identity);
        let command_kind = summary_string(summary, "command_kind");
        let command_category = summary_string(summary, "command_category");
        let validation_family = summary_string(summary, "validation_family");
        let path_patterns = summary_string_array(summary, "path_patterns");
        let absolute_path_patterns = summary_string_array(summary, "absolute_path_patterns");
        let network_access = summary_bool(summary, "network_access");
        let external_path_access = summary_bool(summary, "external_path_access");
        let compound_command = summary_bool(summary, "compound_command");
        let shell_control_operators = summary_string_array(summary, "shell_control_operators");
        let risky_shell_wrapper = summary_bool(summary, "risky_shell_wrapper");
        let expected_silent_output = summary_bool(summary, "expected_silent_output");
        let permission_rule_suggestions = summary
            .and_then(|summary| summary.get("permission_rule_suggestions"))
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let operation_kind = summary_string(summary, "operation_kind");
        let read_only = summary_bool(summary, "read_only");
        let concurrency_safe = summary_bool(summary, "concurrency_safe");
        let destructive = summary_bool(summary, "destructive");
        let aliases = summary_string_array(summary, "aliases");
        let search_hint = summary_string(summary, "search_hint");
        let should_defer = summary_bool(summary, "should_defer");
        let always_load = summary_bool(summary, "always_load");
        let strict_schema = summary_bool(summary, "strict_schema");
        let interrupt_behavior = summary_string(summary, "interrupt_behavior");
        let requires_user_interaction = summary_bool(summary, "requires_user_interaction");
        let open_world = summary_bool(summary, "open_world");
        let search_or_read = summary_search_or_read(summary);
        let input_paths = summary_string_array(summary, "input_paths");
        let permission_matcher_input = summary_string(summary, "permission_matcher_input");
        let transcript_summary = summary_string(summary, "transcript_summary");
        let ui_render_kind = summary_string(summary, "ui_render_kind");
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
        if result.success {
            for path in &changed_paths {
                self.changed_files.insert(path.clone());
            }
        }
        let execution = tool_execution_context_record(result);
        let relevance = tool_execution_relevance(
            result,
            safe_for_closeout,
            validation_family.as_deref(),
            &changed_paths,
            permission.as_ref(),
            execution.route.as_ref(),
        );
        self.tool_execution_records.push(ToolExecutionRecord {
            call_id: tool_call.id.clone(),
            tool: tool_call.name.clone(),
            operation_kind,
            read_only,
            concurrency_safe,
            destructive,
            aliases,
            search_hint,
            should_defer,
            always_load,
            strict_schema,
            interrupt_behavior,
            requires_user_interaction,
            open_world,
            search_or_read,
            input_paths,
            permission_matcher_input,
            transcript_summary,
            ui_render_kind,
            status,
            arguments_hash: tool_arguments_hash(&tool_call.arguments),
            duration_ms: result.duration_ms,
            output_chars: result.content.chars().count(),
            output: tool_output_metadata_record(result),
            error_code: result.error_code.as_ref().and_then(|code| {
                serde_json::to_value(code)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
            }),
            error_preview: result.error.as_deref().map(preview),
            permission,
            command,
            normalized_command,
            command_kind,
            command_category,
            validation_family,
            path_patterns,
            absolute_path_patterns,
            safe_for_closeout,
            network_access,
            external_path_access,
            compound_command,
            shell_control_operators,
            risky_shell_wrapper,
            expected_silent_output,
            permission_rule_suggestions,
            terminal_task: terminal_task_record(result),
            changed_paths,
            file_evidence,
            relevance,
            execution,
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
            source: permission_source_label(permission_request),
            summary: preview(&summary),
        });
    }

    fn record_agent_tool_result(&mut self, result: &ToolResult) {
        let Some(data) = result.data.as_ref() else {
            return;
        };

        let mut recorded = false;
        for key in ["results", "branches"] {
            let Some(items) = data.get(key).and_then(serde_json::Value::as_array) else {
                continue;
            };
            for item in items {
                if self.record_subagent_validation_fact(result.success, item, result) {
                    recorded = true;
                }
            }
        }
        if !recorded {
            self.record_subagent_validation_fact(result.success, data, result);
        }
    }

    fn record_subagent_validation_fact(
        &mut self,
        tool_success: bool,
        data: &serde_json::Value,
        result: &ToolResult,
    ) -> bool {
        let Some(raw_proof_kind) = subagent_proof_kind(data) else {
            return false;
        };
        let source_agent = json_string(data, "source_agent")
            .or_else(|| json_string(data, "agent_id"))
            .unwrap_or_else(|| "unknown".to_string());
        let status = json_string(data, "status").unwrap_or_else(|| {
            if tool_success {
                "completed".to_string()
            } else {
                "failed".to_string()
            }
        });
        let parent_verified = data
            .get("parent_verified")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let claim_id = json_string(data, "claim_id");
        let claim_type = json_string(data, "claim_type");
        let parent_command = json_string(data, "parent_command");
        let artifact_ids = subagent_parent_artifact_ids(data);
        let verification_verdict =
            json_string(data, "verification_verdict").or_else(|| json_string(data, "verdict"));
        let verified_at = json_string(data, "verified_at");
        let bound_parent_verification = raw_proof_kind
            == VerificationProofKind::ParentVerifiedSubagentResult
            && parent_verification_record_is_bound(ParentVerificationBindingInput {
                data,
                source_agent: &source_agent,
                parent_verified,
                claim_id: claim_id.as_deref(),
                claim_type: claim_type.as_deref(),
                artifact_ids: &artifact_ids,
                verification_verdict: verification_verdict.as_deref(),
                verified_at: verified_at.as_deref(),
            });
        let proof_kind = if raw_proof_kind == VerificationProofKind::ParentVerifiedSubagentResult
            && !bound_parent_verification
        {
            VerificationProofKind::SubagentClaimOnly
        } else {
            raw_proof_kind
        };
        let parent_verified_for_proof = parent_verified && bound_parent_verification;
        let passed =
            tool_success && matches!(status.as_str(), "completed" | "success" | "verified");
        let output_kind = json_string(data, "subagent_output_kind")
            .unwrap_or_else(|| "SubagentFinding".to_string());
        let content = json_string(data, "result")
            .or_else(|| json_string(data, "content"))
            .unwrap_or_else(|| result_summary(result));
        let summary = format!(
            "subagent {source_agent} {output_kind} status={status} parent_verified={parent_verified_for_proof}: {content}"
        );
        let command_status = if passed { "passed" } else { "failed" };
        let residual_risk = json_string(data, "residual_risk").or_else(|| {
            (raw_proof_kind == VerificationProofKind::ParentVerifiedSubagentResult
                && !bound_parent_verification)
                .then(|| {
                    "parent verification record missing explicit binding; downgraded to subagent claim only".to_string()
                })
        });

        self.validation_facts.push(ValidationEvidence {
            source: format!("agent:{source_agent}"),
            command: None,
            passed,
            summary: preview(&summary),
            proof_kind: Some(proof_kind),
            scope: json_string(data, "scope").or_else(|| Some("subagent_result".to_string())),
            command_status: Some(command_status.to_string()),
            validation_family: Some("subagent".to_string()),
            source_agent: Some(source_agent),
            parent_verified: Some(parent_verified_for_proof),
            related_to_changed_files: json_string(data, "related_to_changed_files"),
            residual_risk,
            claim_id,
            claim_type,
            parent_command,
            artifact_ids,
            verification_verdict,
            verified_at,
        });
        true
    }
}

mod tool_records;
use tool_records::*;

#[cfg(test)]
mod tests;
