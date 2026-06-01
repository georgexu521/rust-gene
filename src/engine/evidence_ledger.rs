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
    pub operation_kind: Option<String>,
    pub read_only: Option<bool>,
    pub concurrency_safe: Option<bool>,
    pub destructive: Option<bool>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub search_hint: Option<String>,
    #[serde(default)]
    pub should_defer: Option<bool>,
    #[serde(default)]
    pub always_load: Option<bool>,
    #[serde(default)]
    pub strict_schema: Option<bool>,
    #[serde(default)]
    pub interrupt_behavior: Option<String>,
    #[serde(default)]
    pub requires_user_interaction: Option<bool>,
    #[serde(default)]
    pub open_world: Option<bool>,
    #[serde(default)]
    pub search_or_read: ToolSearchOrReadRecord,
    #[serde(default)]
    pub input_paths: Vec<String>,
    #[serde(default)]
    pub permission_matcher_input: Option<String>,
    #[serde(default)]
    pub transcript_summary: Option<String>,
    #[serde(default)]
    pub ui_render_kind: Option<String>,
    pub status: ToolExecutionStatus,
    pub arguments_hash: String,
    pub duration_ms: Option<u64>,
    pub output_chars: usize,
    pub output: ToolOutputMetadataRecord,
    pub error_code: Option<String>,
    pub error_preview: Option<String>,
    pub permission: Option<ToolPermissionRecord>,
    pub command: Option<String>,
    #[serde(default)]
    pub normalized_command: Option<String>,
    pub command_kind: Option<String>,
    pub command_category: Option<String>,
    pub validation_family: Option<String>,
    #[serde(default)]
    pub path_patterns: Vec<String>,
    #[serde(default)]
    pub absolute_path_patterns: Vec<String>,
    pub safe_for_closeout: Option<bool>,
    #[serde(default)]
    pub network_access: Option<bool>,
    #[serde(default)]
    pub external_path_access: Option<bool>,
    #[serde(default)]
    pub compound_command: Option<bool>,
    #[serde(default)]
    pub shell_control_operators: Vec<String>,
    #[serde(default)]
    pub risky_shell_wrapper: Option<bool>,
    #[serde(default)]
    pub expected_silent_output: Option<bool>,
    #[serde(default)]
    pub permission_rule_suggestions: Vec<serde_json::Value>,
    pub terminal_task: Option<TerminalTaskRecord>,
    pub changed_paths: Vec<String>,
    pub file_evidence: Vec<ToolFileEvidenceLink>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSearchOrReadRecord {
    pub is_search: bool,
    pub is_read: bool,
    pub is_list: bool,
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
    pub permission_source: Option<String>,
    pub resolved_permission_source: Option<String>,
    pub permission_requires: Option<bool>,
    pub tool_requires: Option<bool>,
    pub raw_tool_requires: Option<bool>,
    pub drift_requires_approval: Option<bool>,
    pub permission_family: Option<String>,
    pub permission_decision: Option<String>,
    pub risk_level: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolOutputMetadataRecord {
    pub data_keys: Vec<String>,
    pub summary_keys: Vec<String>,
    pub display_format: Option<String>,
    pub truncated: Option<bool>,
    pub file_count: Option<usize>,
    pub diagnostics: Option<ToolDiagnosticsMetadataRecord>,
    pub shell_status: Option<String>,
    pub shell_evidence_status: Option<String>,
    pub shell_exit_code: Option<i64>,
    pub shell_background: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolDiagnosticsMetadataRecord {
    pub status: Option<String>,
    pub checked: Option<bool>,
    pub diagnostic_count: Option<u64>,
    pub error_count: Option<u64>,
    pub warning_count: Option<u64>,
    pub first_error_line: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolFileEvidenceLink {
    pub fact_index: usize,
    pub path: Option<String>,
    pub kind: Option<String>,
    pub line_start: Option<u64>,
    pub line_end: Option<u64>,
    pub content_hash: Option<String>,
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
    pub policy: ToolExecutionRelevancePolicyRecord,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionRelevancePolicyRecord {
    pub route_workflow: Option<String>,
    pub closeout_reasons: Vec<String>,
    pub repair_reasons: Vec<String>,
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
    pub started_at_unix_ms: Option<u64>,
    pub finished_at_unix_ms: Option<u64>,
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
    #[serde(default)]
    pub normalized_command: String,
    pub success: bool,
    pub command_kind: Option<String>,
    pub command_category: Option<String>,
    pub validation_family: Option<String>,
    #[serde(default)]
    pub path_patterns: Vec<String>,
    #[serde(default)]
    pub absolute_path_patterns: Vec<String>,
    pub safe_for_closeout: bool,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub external_path_access: bool,
    #[serde(default)]
    pub compound_command: bool,
    #[serde(default)]
    pub shell_control_operators: Vec<String>,
    #[serde(default)]
    pub risky_shell_wrapper: bool,
    #[serde(default)]
    pub expected_silent_output: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationEvidence {
    pub source: String,
    pub command: Option<String>,
    pub passed: bool,
    pub summary: String,
    #[serde(default)]
    pub proof_kind: Option<VerificationProofKind>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub command_status: Option<String>,
    #[serde(default)]
    pub validation_family: Option<String>,
    #[serde(default)]
    pub source_agent: Option<String>,
    #[serde(default)]
    pub parent_verified: Option<bool>,
    #[serde(default)]
    pub related_to_changed_files: Option<String>,
    #[serde(default)]
    pub residual_risk: Option<String>,
    #[serde(default)]
    pub claim_id: Option<String>,
    #[serde(default)]
    pub claim_type: Option<String>,
    #[serde(default)]
    pub parent_command: Option<String>,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
    #[serde(default)]
    pub verification_verdict: Option<String>,
    #[serde(default)]
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionEvidence {
    pub tool: String,
    pub kind: Option<String>,
    pub approved: bool,
    #[serde(default)]
    pub source: Option<String>,
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

fn summary_string_array(summary: Option<&serde_json::Value>, key: &str) -> Vec<String> {
    summary
        .and_then(|summary| summary.get(key))
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

fn summary_bool(summary: Option<&serde_json::Value>, key: &str) -> Option<bool> {
    summary
        .and_then(|summary| summary.get(key))
        .and_then(serde_json::Value::as_bool)
}

fn summary_search_or_read(summary: Option<&serde_json::Value>) -> ToolSearchOrReadRecord {
    let Some(value) = summary.and_then(|summary| summary.get("search_or_read")) else {
        return ToolSearchOrReadRecord::default();
    };
    ToolSearchOrReadRecord {
        is_search: value
            .get("is_search")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        is_read: value
            .get("is_read")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        is_list: value
            .get("is_list")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    }
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
            permission_source: nested_string(metadata, "permission_source")
                .or_else(|| json_string(permission_request, "permission_source")),
            resolved_permission_source: nested_string(metadata, "resolved_permission_source")
                .or_else(|| json_string(permission_request, "permission_source")),
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

fn permission_source_label(permission_request: &serde_json::Value) -> Option<String> {
    permission_request
        .get("permission_source")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            permission_request
                .get("metadata")
                .and_then(|metadata| nested_string(Some(metadata), "resolved_permission_source"))
        })
        .or_else(|| {
            permission_request
                .get("metadata")
                .and_then(|metadata| nested_string(Some(metadata), "permission_source"))
        })
}

fn subagent_proof_kind(data: &serde_json::Value) -> Option<VerificationProofKind> {
    let label = data
        .get("verification_proof_kind")
        .or_else(|| data.get("proof_kind"))
        .and_then(serde_json::Value::as_str)?;
    let value = serde_json::Value::String(label.to_string());
    let proof_kind = serde_json::from_value::<VerificationProofKind>(value).ok()?;
    match proof_kind {
        VerificationProofKind::SubagentClaimOnly
        | VerificationProofKind::ParentVerifiedSubagentResult => Some(proof_kind),
        _ => None,
    }
}

struct ParentVerificationBindingInput<'a> {
    data: &'a serde_json::Value,
    source_agent: &'a str,
    parent_verified: bool,
    claim_id: Option<&'a str>,
    claim_type: Option<&'a str>,
    artifact_ids: &'a [String],
    verification_verdict: Option<&'a str>,
    verified_at: Option<&'a str>,
}

fn parent_verification_record_is_bound(input: ParentVerificationBindingInput<'_>) -> bool {
    input.parent_verified
        && !matches!(input.source_agent.trim(), "" | "unknown")
        && json_string(input.data, "scope").as_deref() == Some("parent_runtime_verification")
        && input.claim_id.is_some_and(|value| !value.trim().is_empty())
        && input
            .claim_type
            .is_some_and(|value| !value.trim().is_empty())
        && !input.artifact_ids.is_empty()
        && related_to_changed_files_is_bound(input.data)
        && input
            .verification_verdict
            .is_some_and(|value| value.trim().to_ascii_lowercase().starts_with("verified"))
        && input
            .verified_at
            .is_some_and(|value| !value.trim().is_empty())
}

fn related_to_changed_files_is_bound(data: &serde_json::Value) -> bool {
    match data.get("related_to_changed_files") {
        Some(serde_json::Value::Bool(_)) => true,
        Some(serde_json::Value::String(value)) => {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty()
                && !matches!(
                    normalized.as_str(),
                    "unknown" | "unknown_child_worktree" | "unbound"
                )
        }
        _ => false,
    }
}

fn subagent_parent_artifact_ids(data: &serde_json::Value) -> Vec<String> {
    let mut ids = json_string_array(data, "artifact_ids");
    for key in ["artifact_id", "tool_run_id", "parent_tool_run_id"] {
        if let Some(id) = json_identifier(data, key) {
            ids.push(id);
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

fn json_identifier(value: &serde_json::Value, key: &str) -> Option<String> {
    let value = value.get(key)?;
    if let Some(text) = value.as_str().filter(|value| !value.trim().is_empty()) {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    None
}

fn result_has_subagent_proof(data: &serde_json::Value) -> bool {
    subagent_proof_kind(data).is_some()
        || data
            .get("results")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| items.iter().any(result_has_subagent_proof))
        || data
            .get("branches")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| items.iter().any(result_has_subagent_proof))
}

fn tool_output_metadata_record(result: &ToolResult) -> ToolOutputMetadataRecord {
    let data = result.data.as_ref();
    let summary = data
        .and_then(|data| data.get("tool_summary"))
        .filter(|value| value.is_object());
    let diagnostics = data.and_then(|data| data.get("diagnostics"));
    let shell = data.and_then(|data| data.get("shell_result"));
    ToolOutputMetadataRecord {
        data_keys: json_object_keys(data),
        summary_keys: json_object_keys(summary),
        display_format: nested_string(data, "display_format"),
        truncated: nested_bool(data, "truncated"),
        file_count: data
            .and_then(|data| data.get("files"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        diagnostics: diagnostics
            .filter(|value| value.is_object())
            .map(tool_diagnostics_metadata_record),
        shell_status: nested_string(shell, "status"),
        shell_evidence_status: nested_string(shell, "evidence_status"),
        shell_exit_code: nested_i64(shell, "exit_code"),
        shell_background: nested_bool(shell, "background"),
    }
}

fn tool_diagnostics_metadata_record(value: &serde_json::Value) -> ToolDiagnosticsMetadataRecord {
    ToolDiagnosticsMetadataRecord {
        status: nested_string(Some(value), "status"),
        checked: nested_bool(Some(value), "checked"),
        diagnostic_count: nested_u64(Some(value), "diagnostic_count"),
        error_count: nested_u64(Some(value), "error_count"),
        warning_count: nested_u64(Some(value), "warning_count"),
        first_error_line: value
            .get("first_error")
            .and_then(|error| error.get("range"))
            .and_then(|range| range.get("start_line"))
            .and_then(serde_json::Value::as_u64),
    }
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
    if tool_call.name == "bash" && result.success {
        if let Some(command) = tool_call.arguments["command"].as_str() {
            let classification =
                crate::tools::bash_tool::command_classifier::classify_command(command);
            if matches!(
                classification.category,
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::FileMutation
                    | crate::tools::bash_tool::command_classifier::ShellCommandCategory::GitMutation
            ) {
                paths.extend(classification.path_patterns);
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
    route: Option<&ToolRouteRecord>,
) -> ToolExecutionRelevance {
    let subagent_proof = result.data.as_ref().is_some_and(result_has_subagent_proof);
    let validation =
        safe_for_closeout.unwrap_or(false) || validation_family.is_some() || subagent_proof;
    let closeout = validation || !changed_paths.is_empty() || permission.is_some();
    let repair = !result.success || !changed_paths.is_empty();
    let mut closeout_reasons = Vec::new();
    if validation {
        closeout_reasons.push("validation".to_string());
    }
    if !changed_paths.is_empty() {
        closeout_reasons.push("changed_paths".to_string());
    }
    if permission.is_some() {
        closeout_reasons.push("permission".to_string());
    }
    let mut repair_reasons = Vec::new();
    if !result.success {
        repair_reasons.push("tool_failed".to_string());
    }
    if !changed_paths.is_empty() {
        repair_reasons.push("changed_paths".to_string());
    }
    ToolExecutionRelevance {
        validation,
        closeout,
        repair,
        policy: ToolExecutionRelevancePolicyRecord {
            route_workflow: route.and_then(|route| route.workflow.clone()),
            closeout_reasons,
            repair_reasons,
        },
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
        started_at_unix_ms: nested_u64(execution, "started_at_unix_ms"),
        finished_at_unix_ms: nested_u64(execution, "finished_at_unix_ms"),
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

fn nested_i64(value: Option<&serde_json::Value>, key: &str) -> Option<i64> {
    value
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_i64)
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

fn json_object_keys(value: Option<&serde_json::Value>) -> Vec<String> {
    let mut keys: Vec<String> = value
        .and_then(serde_json::Value::as_object)
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    keys.sort();
    keys
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
    let fallback;
    let executed = if fact.normalized_command.is_empty() {
        fallback = normalize_command_identity(&fact.command);
        fallback.as_str()
    } else {
        fact.normalized_command.as_str()
    };
    executed == required_command
        || (fact.safe_for_closeout && shell_assertion_covers_required(executed, required_command))
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

fn tool_record_relevant_for_repair(
    record: &ToolExecutionRecord,
    failed_command_identities: &BTreeSet<String>,
) -> bool {
    matches!(
        record.status,
        ToolExecutionStatus::Failed | ToolExecutionStatus::Denied
    ) || record.relevance.repair
        || !record.changed_paths.is_empty()
        || record.command.as_deref().is_some_and(|command| {
            failed_command_identities.contains(&normalize_command_identity(command))
        })
}

fn format_repair_tool_record(record: &ToolExecutionRecord, file_facts: &[FileEvidence]) -> String {
    let terminal = record
        .terminal_task
        .as_ref()
        .map(|task| {
            format!(
                "status={} exit={} task={}",
                task.status.as_deref().unwrap_or("unknown"),
                task.exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                task.task_id.as_deref().unwrap_or("none")
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let file_evidence = record
        .file_evidence
        .iter()
        .take(3)
        .filter_map(|link| {
            let fact = file_facts.get(link.fact_index)?;
            Some(format!(
                "{}:{}-{} kind={} success={}",
                link.path
                    .as_deref()
                    .or(fact.path.as_deref())
                    .unwrap_or("unknown"),
                link.line_start
                    .or(fact.line_start)
                    .map(|line| line.to_string())
                    .unwrap_or_else(|| "?".to_string()),
                link.line_end
                    .or(fact.line_end)
                    .map(|line| line.to_string())
                    .unwrap_or_else(|| "?".to_string()),
                link.kind
                    .as_deref()
                    .or(fact.kind.as_deref())
                    .unwrap_or("unknown"),
                fact.success
            ))
        })
        .collect::<Vec<_>>();
    let changed_paths = if record.changed_paths.is_empty() {
        "none".to_string()
    } else {
        record.changed_paths.join(",")
    };
    let path_patterns = if record.path_patterns.is_empty() {
        "none".to_string()
    } else {
        record.path_patterns.join(",")
    };
    let file_evidence = if file_evidence.is_empty() {
        "none".to_string()
    } else {
        file_evidence.join(" | ")
    };
    let repair_reasons = if record.relevance.policy.repair_reasons.is_empty() {
        "none".to_string()
    } else {
        record.relevance.policy.repair_reasons.join(",")
    };
    let status = match record.status {
        ToolExecutionStatus::Completed => "completed",
        ToolExecutionStatus::Failed => "failed",
        ToolExecutionStatus::Denied => "denied",
    };

    preview(&format!(
        "tool record evidence: tool={} status={} command={} normalized_command={} validation_family={} path_patterns={} network_access={} external_path_access={} expected_silent_output={} safe_for_closeout={} duration_ms={} output_chars={} terminal={} changed_paths={} file_evidence={} repair_reasons={} error={}",
        record.tool,
        status,
        record.command.as_deref().unwrap_or("none"),
        record.normalized_command.as_deref().unwrap_or("none"),
        record.validation_family.as_deref().unwrap_or("none"),
        path_patterns,
        record
            .network_access
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        record
            .external_path_access
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        record
            .expected_silent_output
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        record
            .safe_for_closeout
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        record
            .duration_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        record.output_chars,
        terminal,
        changed_paths,
        file_evidence,
        repair_reasons,
        record.error_preview.as_deref().unwrap_or("none")
    ))
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
        let record = &ledger.tool_execution_records()[0];
        assert_eq!(record.file_evidence.len(), 1);
        assert_eq!(record.file_evidence[0].fact_index, 0);
        assert_eq!(record.file_evidence[0].path.as_deref(), Some("src/app.py"));
    }

    #[test]
    fn records_tool_contract_semantics_from_summary() {
        let mut ledger = EvidenceLedger::new();
        let result = ToolResult::success_with_data(
            "listed files",
            serde_json::json!({
                "tool_summary": {
                    "operation_kind": "list",
                    "read_only": true,
                    "concurrency_safe": true,
                    "destructive": false
                }
            }),
        );

        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "ls -la"})),
            &result,
        );

        let record = &ledger.tool_execution_records()[0];
        assert_eq!(record.operation_kind.as_deref(), Some("list"));
        assert_eq!(record.read_only, Some(true));
        assert_eq!(record.concurrency_safe, Some(true));
        assert_eq!(record.destructive, Some(false));
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
        let output = &ledger.tool_execution_records()[0].output;
        assert!(output.data_keys.iter().any(|key| key == "content_hash"));
        assert_eq!(
            output.display_format.as_deref(),
            Some("line_numbered_content")
        );
        assert_eq!(output.truncated, Some(true));
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
        let diagnostics = ledger.tool_execution_records()[0]
            .output
            .diagnostics
            .as_ref()
            .expect("diagnostics metadata should be recorded");
        assert_eq!(diagnostics.status.as_deref(), Some("diagnostics_found"));
        assert_eq!(diagnostics.diagnostic_count, Some(2));
        assert_eq!(diagnostics.first_error_line, Some(7));
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
        let record = &ledger.tool_execution_records()[0];
        assert_eq!(record.file_evidence.len(), 2);
        assert_eq!(record.file_evidence[0].fact_index, 0);
        assert_eq!(record.file_evidence[0].path.as_deref(), Some("src/lib.rs"));
        assert_eq!(record.file_evidence[0].line_start, Some(3));
        assert_eq!(record.file_evidence[1].fact_index, 1);
        assert_eq!(record.file_evidence[1].path.as_deref(), Some("README.md"));
        assert_eq!(record.file_evidence[1].line_end, Some(8));
        assert_eq!(record.output.file_count, Some(2));
    }

    #[test]
    fn records_safe_bash_validation_as_command_and_validation_fact() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "cargo test -q src"})),
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
        assert_eq!(
            ledger.command_facts[0].normalized_command,
            "cargo test -q src"
        );
        assert_eq!(ledger.command_facts[0].path_patterns, vec!["src"]);
        assert!(!ledger.command_facts[0].network_access);
        assert!(!ledger.command_facts[0].external_path_access);
        assert!(!ledger.command_facts[0].compound_command);
    }

    #[test]
    fn records_shell_risk_facts_from_tool_summary() {
        let mut ledger = EvidenceLedger::new();
        let result = ToolResult::success_with_data(
            "ok",
            serde_json::json!({
                "tool_summary": {
                    "command": "curl https://example.com -o /tmp/out.json",
                    "network_access": true,
                    "external_path_access": true,
                    "absolute_path_patterns": ["/tmp/out.json"],
                    "compound_command": false,
                    "shell_control_operators": [],
                    "risky_shell_wrapper": false,
                    "expected_silent_output": false,
                    "permission_rule_suggestions": [
                        {
                            "pattern": "curl https://example.com -o /tmp/out.json",
                            "scope": "exact",
                            "stable": false,
                            "reason": "exact command for this permission review"
                        }
                    ]
                }
            }),
        );

        ledger.record_tool_result(
            &tool_call(
                "bash",
                serde_json::json!({"command": "curl https://example.com -o /tmp/out.json"}),
            ),
            &result,
        );

        let record = &ledger.tool_execution_records()[0];
        assert_eq!(record.network_access, Some(true));
        assert_eq!(record.external_path_access, Some(true));
        assert_eq!(record.absolute_path_patterns, vec!["/tmp/out.json"]);
        assert_eq!(record.compound_command, Some(false));
        assert_eq!(record.risky_shell_wrapper, Some(false));
        assert_eq!(record.expected_silent_output, Some(false));
        assert_eq!(record.permission_rule_suggestions.len(), 1);
    }

    #[test]
    fn records_bash_mutation_path_patterns_as_changed_paths() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "git add src/lib.rs"})),
            &ToolResult::success(""),
        );

        let record = &ledger.tool_execution_records()[0];
        assert_eq!(record.changed_paths, vec!["src/lib.rs"]);
        assert!(record.relevance.closeout);
        assert!(record.relevance.repair);
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
                    "path_patterns": ["src/lib.rs"],
                    "safe_for_closeout": true,
                    "operation_kind": "shell",
                    "read_only": false,
                    "concurrency_safe": false,
                    "destructive": false,
                    "aliases": ["shell"],
                    "search_hint": "shell validation git package managers",
                    "should_defer": false,
                    "always_load": false,
                    "strict_schema": true,
                    "interrupt_behavior": "block",
                    "requires_user_interaction": false,
                    "open_world": false,
                    "search_or_read": {
                        "is_search": false,
                        "is_read": false,
                        "is_list": false
                    },
                    "input_paths": ["src/lib.rs"],
                    "permission_matcher_input": "cargo test -q",
                    "transcript_summary": "cargo test -q",
                    "ui_render_kind": "shell",
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
                        "exposed_tools_count": 15,
                        "started_at_unix_ms": 1770000000000u64,
                        "finished_at_unix_ms": 1770000000042u64
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
        assert_eq!(
            records[0].normalized_command.as_deref(),
            Some("cargo test -q")
        );
        assert_eq!(records[0].command_kind.as_deref(), Some("validation"));
        assert_eq!(records[0].validation_family.as_deref(), Some("cargo_test"));
        assert_eq!(records[0].path_patterns, vec!["src/lib.rs"]);
        assert_eq!(records[0].safe_for_closeout, Some(true));
        assert_eq!(records[0].operation_kind.as_deref(), Some("shell"));
        assert_eq!(records[0].read_only, Some(false));
        assert_eq!(records[0].concurrency_safe, Some(false));
        assert_eq!(records[0].destructive, Some(false));
        assert_eq!(records[0].aliases, vec!["shell"]);
        assert_eq!(
            records[0].search_hint.as_deref(),
            Some("shell validation git package managers")
        );
        assert_eq!(records[0].strict_schema, Some(true));
        assert_eq!(records[0].interrupt_behavior.as_deref(), Some("block"));
        assert_eq!(records[0].requires_user_interaction, Some(false));
        assert_eq!(records[0].open_world, Some(false));
        assert!(!records[0].search_or_read.is_search);
        assert_eq!(records[0].input_paths, vec!["src/lib.rs"]);
        assert_eq!(
            records[0].permission_matcher_input.as_deref(),
            Some("cargo test -q")
        );
        assert_eq!(
            records[0].transcript_summary.as_deref(),
            Some("cargo test -q")
        );
        assert_eq!(records[0].ui_render_kind.as_deref(), Some("shell"));
        assert_eq!(
            records[0].relevance,
            ToolExecutionRelevance {
                validation: true,
                closeout: true,
                repair: false,
                policy: ToolExecutionRelevancePolicyRecord {
                    route_workflow: Some("code_change".to_string()),
                    closeout_reasons: vec!["validation".to_string()],
                    repair_reasons: Vec::new(),
                },
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
        assert_eq!(
            records[0].execution.started_at_unix_ms,
            Some(1_770_000_000_000)
        );
        assert_eq!(
            records[0].execution.finished_at_unix_ms,
            Some(1_770_000_000_042)
        );
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
                    "permission_source": "config_project_ask",
                    "resolved_permission_source": "user_once_reject",
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
                policy: ToolExecutionRelevancePolicyRecord {
                    route_workflow: None,
                    closeout_reasons: vec!["permission".to_string()],
                    repair_reasons: vec!["tool_failed".to_string()],
                },
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
        assert_eq!(
            permission.source.permission_source.as_deref(),
            Some("config_project_ask")
        );
        assert_eq!(
            permission.source.resolved_permission_source.as_deref(),
            Some("user_once_reject")
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
    fn verification_proof_reports_missing_required_commands_as_not_run() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "cargo test -q"})),
            &ToolResult::success("test result: ok"),
        );
        let required = vec!["cargo test -q".to_string(), "cargo fmt --check".to_string()];

        let proof = ledger.verification_proof(VerificationProofRequest {
            required_commands: &required,
            requires_validation: true,
            task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
            support_context: VerificationProofSupportContext::code_change(),
        });

        assert_eq!(proof.status, VerificationProofStatus::NotRun);
        assert_eq!(proof.required_total, 2);
        assert_eq!(proof.required_passed, 1);
        assert_eq!(proof.required_missing, 1);
        assert_eq!(
            proof.missing_required_commands,
            vec!["cargo fmt --check".to_string()]
        );
        assert!(proof
            .validation_line()
            .contains("verification proof: not_run"));
    }

    #[test]
    fn verification_proof_prefers_required_validation_success_over_prior_user_deferred_state() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_validation_result(
            "run_tests",
            Some("python3 fixtures/example/test_slugify.py"),
            true,
            "OK",
        );
        let required = vec!["python3 fixtures/example/test_slugify.py".to_string()];

        let proof = ledger.verification_proof(VerificationProofRequest {
            required_commands: &required,
            requires_validation: true,
            task_verification_status: crate::engine::task_context::VerificationStatus::UserDeferred,
            support_context: VerificationProofSupportContext::code_change(),
        });

        assert_eq!(proof.status, VerificationProofStatus::Verified);
        assert_eq!(
            proof.derived_support.status,
            VerificationProofStatus::Verified
        );
        assert!(proof
            .proof_kinds
            .contains(&VerificationProofKind::CommandPassed));
        assert!(proof
            .proof_kinds
            .contains(&VerificationProofKind::RequiredValidationPassed));
        assert_eq!(proof.required_passed, 1);
        assert!(proof.summary.contains("required validation passed 1/1"));
    }

    #[test]
    fn verification_proof_does_not_trust_verified_task_state_without_ledger_evidence() {
        let ledger = EvidenceLedger::new();

        let proof = ledger.verification_proof(VerificationProofRequest {
            required_commands: &[],
            requires_validation: true,
            task_verification_status: crate::engine::task_context::VerificationStatus::Verified,
            support_context: VerificationProofSupportContext::code_change(),
        });

        assert_eq!(proof.status, VerificationProofStatus::Unavailable);
        assert!(proof
            .summary
            .contains("ledger has no verification evidence"));
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
    fn records_agent_tool_result_as_subagent_claim_only_proof() {
        let mut ledger = EvidenceLedger::new();
        let result = ToolResult::success_with_data(
            "Sub-agent agent_1 completed with status: Completed",
            serde_json::json!({
                "agent_id": "agent_1",
                "source_agent": "agent_1",
                "status": "completed",
                "result": "review says tests look good",
                "proof_kind": "subagent_claim_only",
                "verification_proof_kind": "subagent_claim_only",
                "subagent_output_kind": "SubagentVerificationClaim",
                "parent_verified": false,
                "scope": "subagent_result",
                "related_to_changed_files": "none",
                "residual_risk": "subagent output is a claim until parent runtime verification"
            }),
        );

        ledger.record_tool_result(
            &tool_call(
                "agent",
                serde_json::json!({"description": "review", "prompt": "check"}),
            ),
            &result,
        );

        let facts = ledger.validation_facts();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].source, "agent:agent_1");
        assert!(facts[0].passed);
        assert_eq!(
            facts[0].proof_kind,
            Some(VerificationProofKind::SubagentClaimOnly)
        );
        assert_eq!(facts[0].source_agent.as_deref(), Some("agent_1"));
        assert_eq!(facts[0].parent_verified, Some(false));
        assert!(ledger.tool_execution_records()[0].relevance.validation);
        assert!(ledger.tool_execution_records()[0].relevance.closeout);

        let proof = ledger.verification_proof(VerificationProofRequest {
            required_commands: &[],
            requires_validation: true,
            task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
            support_context: VerificationProofSupportContext::code_change(),
        });

        assert_eq!(proof.status, VerificationProofStatus::Verified);
        assert_eq!(
            proof.derived_support.status,
            VerificationProofStatus::Partial
        );
        assert!(!proof.derived_support.supports_verified);
        assert!(proof
            .proof_kinds
            .contains(&VerificationProofKind::SubagentClaimOnly));
    }

    #[test]
    fn parent_runtime_validation_does_not_promote_subagent_claim_without_explicit_parent_record() {
        let mut ledger = EvidenceLedger::new();
        let subagent_result = ToolResult::success_with_data(
            "Sub-agent agent_1 completed with status: Completed",
            serde_json::json!({
                "agent_id": "agent_1",
                "source_agent": "agent_1",
                "status": "completed",
                "result": "review says the target behavior is present",
                "verification_proof_kind": "subagent_claim_only",
                "subagent_output_kind": "SubagentVerificationClaim",
                "parent_verified": false,
                "scope": "subagent_result",
            }),
        );
        ledger.record_tool_result(
            &tool_call(
                "agent",
                serde_json::json!({"description": "review", "prompt": "check"}),
            ),
            &subagent_result,
        );

        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "cargo check -q"})),
            &ToolResult::success("cargo check finished successfully"),
        );

        let proof = ledger.verification_proof(VerificationProofRequest {
            required_commands: &[],
            requires_validation: true,
            task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
            support_context: ledger
                .verification_proof_support_context(VerificationProofTaskType::SubagentReview, &[]),
        });

        assert!(proof
            .proof_kinds
            .contains(&VerificationProofKind::SubagentClaimOnly));
        assert!(!proof
            .proof_kinds
            .contains(&VerificationProofKind::ParentVerifiedSubagentResult));
        assert_eq!(
            proof.derived_support.status,
            VerificationProofStatus::Partial
        );
        assert!(!proof.derived_support.supports_verified);
    }

    #[test]
    fn non_validation_parent_command_does_not_promote_subagent_claim() {
        let mut ledger = EvidenceLedger::new();
        let subagent_result = ToolResult::success_with_data(
            "Sub-agent agent_1 completed with status: Completed",
            serde_json::json!({
                "agent_id": "agent_1",
                "status": "completed",
                "result": "review says the target behavior is present",
                "verification_proof_kind": "subagent_claim_only",
                "subagent_output_kind": "SubagentVerificationClaim",
                "parent_verified": false,
            }),
        );
        ledger.record_tool_result(
            &tool_call(
                "agent",
                serde_json::json!({"description": "review", "prompt": "check"}),
            ),
            &subagent_result,
        );
        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "echo inspected"})),
            &ToolResult::success("inspected"),
        );

        let proof = ledger.verification_proof(VerificationProofRequest {
            required_commands: &[],
            requires_validation: true,
            task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
            support_context: ledger
                .verification_proof_support_context(VerificationProofTaskType::SubagentReview, &[]),
        });

        assert!(!proof
            .proof_kinds
            .contains(&VerificationProofKind::ParentVerifiedSubagentResult));
        assert_eq!(
            proof.derived_support.status,
            VerificationProofStatus::Partial
        );
        assert!(!proof.derived_support.supports_verified);
    }

    #[test]
    fn records_parent_verified_subagent_result_as_verified_support() {
        let mut ledger = EvidenceLedger::new();
        let result = ToolResult::success_with_data(
            "Parent runtime verified sub-agent agent_1",
            serde_json::json!({
                "agent_id": "agent_1",
                "source_agent": "agent_1",
                "status": "verified",
                "result": "parent reran focused checks",
                "verification_proof_kind": "parent_verified_subagent_result",
                "subagent_output_kind": "SubagentVerificationClaim",
                "parent_verified": true,
                "scope": "parent_runtime_verification",
                "claim_id": "claim_agent_1_compile",
                "claim_type": "compile_check",
                "parent_command": "cargo check -q",
                "artifact_ids": ["tool_run_789"],
                "verification_verdict": "verified_for_compile_only",
                "verified_at": "2026-05-26T00:00:00Z",
                "related_to_changed_files": "yes",
                "residual_risk": "parent runtime verified subagent result"
            }),
        );

        ledger.record_tool_result(
            &tool_call("agent", serde_json::json!({"action": "resume"})),
            &result,
        );

        let proof = ledger.verification_proof(VerificationProofRequest {
            required_commands: &[],
            requires_validation: true,
            task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
            support_context: ledger
                .verification_proof_support_context(VerificationProofTaskType::SubagentReview, &[]),
        });

        assert_eq!(proof.status, VerificationProofStatus::Verified);
        assert_eq!(
            proof.derived_support.status,
            VerificationProofStatus::Verified
        );
        assert!(proof.derived_support.supports_verified);
        assert!(proof
            .proof_kinds
            .contains(&VerificationProofKind::ParentVerifiedSubagentResult));
        let fact = &ledger.validation_facts()[0];
        assert_eq!(fact.claim_id.as_deref(), Some("claim_agent_1_compile"));
        assert_eq!(fact.artifact_ids, vec!["tool_run_789".to_string()]);
        assert_eq!(
            fact.verification_verdict.as_deref(),
            Some("verified_for_compile_only")
        );
    }

    #[test]
    fn unbound_parent_verified_subagent_record_is_downgraded_to_claim_only() {
        let mut ledger = EvidenceLedger::new();
        let result = ToolResult::success_with_data(
            "Parent runtime verified sub-agent agent_1",
            serde_json::json!({
                "agent_id": "agent_1",
                "source_agent": "agent_1",
                "status": "verified",
                "result": "parent says checks passed but does not bind the claim",
                "verification_proof_kind": "parent_verified_subagent_result",
                "subagent_output_kind": "SubagentVerificationClaim",
                "parent_verified": true,
                "scope": "parent_runtime_verification",
                "related_to_changed_files": "yes"
            }),
        );

        ledger.record_tool_result(
            &tool_call("agent", serde_json::json!({"action": "resume"})),
            &result,
        );

        let proof = ledger.verification_proof(VerificationProofRequest {
            required_commands: &[],
            requires_validation: true,
            task_verification_status: crate::engine::task_context::VerificationStatus::Pending,
            support_context: ledger
                .verification_proof_support_context(VerificationProofTaskType::SubagentReview, &[]),
        });

        assert!(proof
            .proof_kinds
            .contains(&VerificationProofKind::SubagentClaimOnly));
        assert!(!proof
            .proof_kinds
            .contains(&VerificationProofKind::ParentVerifiedSubagentResult));
        assert_eq!(
            proof.derived_support.status,
            VerificationProofStatus::Partial
        );
        assert!(!proof.derived_support.supports_verified);
        let fact = &ledger.validation_facts()[0];
        assert_eq!(
            fact.proof_kind,
            Some(VerificationProofKind::SubagentClaimOnly)
        );
        assert_eq!(fact.parent_verified, Some(false));
    }

    #[test]
    fn records_permission_denial_as_permission_fact() {
        let mut ledger = EvidenceLedger::new();
        let mut result = ToolResult::error("Permission denied: 'git' requires user confirmation.");
        result.error_code = Some(crate::tools::ToolErrorCode::PermissionDenied);
        result.data = Some(serde_json::json!({
            "permission_request": {
                "kind": "runtime_rule",
                "permission_source": "hook_deny",
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
        assert_eq!(
            ledger.permission_facts()[0].source.as_deref(),
            Some("hook_deny")
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
    fn repair_tool_record_evidence_uses_failed_and_changed_records() {
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call(
                "grep",
                serde_json::json!({"pattern": "ok", "path": "src/lib.rs"}),
            ),
            &ToolResult::success("src/lib.rs:1:ok"),
        );
        ledger.record_tool_result(
            &tool_call("file_edit", serde_json::json!({"path": "src/lib.rs"})),
            &ToolResult::success("File edited successfully: src/lib.rs"),
        );
        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "cargo test -q"})),
            &ToolResult::error_with_content("command exited 101", "test failed"),
        );

        let evidence = ledger.repair_tool_record_evidence(&["cargo test -q".to_string()]);

        assert_eq!(evidence.len(), 2);
        assert!(evidence[0].contains("tool=bash"));
        assert!(evidence[0].contains("status=failed"));
        assert!(evidence[0].contains("command=cargo test -q"));
        assert!(evidence[1].contains("tool=file_edit"));
        assert!(!evidence.iter().any(|item| item.contains("tool=grep")));
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
