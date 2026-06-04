//! Typed partner-to-executor contract surfaces.
//!
//! These structures are a first implementation of the alignment objects:
//! `TaskContract`, `ContextPack`, and `ExecutionReport`. They are derived from
//! the existing runtime task bundle so the current executor remains the source
//! of truth while the product-facing contract becomes inspectable and testable.

use crate::engine::code_change_workflow::{StageValidationStatus, WorkflowCloseout};
use crate::engine::context_compressor::estimate_tokens;
use crate::engine::intent_router::{RiskLevel, WorkflowKind};
use crate::engine::retrieval_context::{RetrievalSource, TrustLevel};
use crate::engine::task_context::{AgentTaskMode, TaskContextBundle, VerificationStatus};
use crate::engine::workflow_contract::ProgrammingWorkflowJudgment;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::io::Write;

mod background_review;
mod memory_proposal;
mod proposal_conflict;
mod proposal_gates;
mod proposal_store;
mod types;

pub use background_review::*;
pub use memory_proposal::*;
pub use proposal_store::*;
pub use types::*;

pub(super) use proposal_conflict::{
    memory_proposal_conflict_groups, stable_memory_proposal_id, summarize_memory_proposal_conflicts,
};
pub(super) use proposal_gates::{
    memory_proposal_candidate_evidence_refs, memory_write_target_for_proposal_candidate,
    proposal_blocking_minimum_evidence_reason, proposal_blocking_sensitivity_reason,
    proposal_gate_report,
};

#[cfg(test)]
use proposal_store::memory_proposal_record_matches_filter;

fn default_memory_proposal_source() -> String {
    "closeout".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProposalReviewRecord {
    #[serde(default)]
    pub id: String,
    pub proposal: MemoryProposal,
    #[serde(default)]
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub source_session: Option<String>,
    #[serde(default)]
    pub source_task: String,
    #[serde(default = "default_memory_proposal_source")]
    pub source: String,
    #[serde(default)]
    pub active_scope: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub project_labels: Vec<String>,
    #[serde(default)]
    pub gate_report: Vec<MemoryProposalGateDecision>,
    #[serde(default)]
    pub duplicate_conflict_summary: String,
    #[serde(default)]
    pub conflict_groups: Vec<MemoryProposalConflictGroup>,
    #[serde(default)]
    pub status_history: Vec<MemoryProposalStatusHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProposalConflictGroup {
    pub group_type: String,
    pub key: String,
    pub scope: String,
    pub kind: String,
    pub matches: Vec<MemoryProposalConflictMatch>,
    pub resolution_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProposalConflictMatch {
    pub proposal_id: String,
    pub candidate_index: usize,
    pub status: MemoryProposalStatus,
    pub source: String,
    pub value: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProposalGateDecision {
    pub gate: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_index: Option<usize>,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProposalStatusHistoryEntry {
    pub at: String,
    pub status: MemoryProposalStatus,
    pub reason: String,
}

impl TaskContract {
    pub fn from_bundle(
        bundle: &TaskContextBundle,
        required_validation_commands: &[String],
    ) -> Self {
        let mut validation_commands = required_validation_commands.to_vec();
        for check in &bundle.agent_state.verification_plan.required_checks {
            push_unique(&mut validation_commands, check.clone());
        }

        let mut files_allowed = bundle
            .relevant_files
            .iter()
            .chain(bundle.agent_state.active_files.iter())
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        dedupe(&mut files_allowed);
        if files_allowed.is_empty() {
            files_allowed = bundle.agent_state.allowed_scope.clone();
        }

        let assumptions = assumptions_from_bundle(bundle);
        let model_profile = derive_model_profile(bundle);
        let project_context = project_context_from_bundle(bundle);
        let acceptance_criteria = acceptance_criteria_from_bundle(bundle);
        let proof_required = validation_proof_required(bundle, &validation_commands);

        Self {
            task_id: bundle.task_id.clone(),
            task_type: task_type_from_workflow(bundle.route.workflow),
            objective: bundle.agent_state.main_goal.clone(),
            user_context: vec![format!("current_request: {}", bundle.prompt_preview)],
            project_context,
            assumptions,
            scope: TaskContractScope {
                files_allowed,
                files_forbidden: Vec::new(),
                commands_allowed: validation_commands.clone(),
                commands_forbidden: forbidden_commands_from_actions(
                    &bundle.agent_state.forbidden_actions,
                ),
            },
            constraints: TaskContractConstraints {
                must_do: bundle.constraints.clone(),
                must_not_do: bundle.agent_state.forbidden_actions.clone(),
            },
            acceptance_criteria,
            validation: TaskValidationContract {
                required_commands: validation_commands,
                proof_required,
            },
            risk: TaskRiskContract {
                level: bundle.route.risk,
                reasons: bundle.risks.clone(),
            },
            model_profile,
            report_schema: TaskReportSchema::default(),
        }
    }

    pub fn compact_summary(&self) -> String {
        format!(
            "TaskContract id={} type={:?} profile={} assumptions={} files={} validation={} proof_required={}",
            self.task_id,
            self.task_type,
            self.model_profile.label(),
            self.assumptions.len(),
            self.scope.files_allowed.len(),
            self.validation.required_commands.len(),
            self.validation.proof_required
        )
    }

    pub fn should_inject_executor_context(&self) -> bool {
        !matches!(self.task_type, TaskContractType::Analysis)
            || self.validation.proof_required
            || !matches!(self.model_profile, ModelProfileMode::Standard)
    }

    pub fn format_for_context_zone(&self) -> String {
        let assumptions = if self.assumptions.is_empty() {
            "none".to_string()
        } else {
            self.assumptions
                .iter()
                .take(5)
                .map(|item| {
                    format!(
                        "{}:{}: {}",
                        item.source.label(),
                        item.confidence.label(),
                        compact_text(&item.assumption, 160)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        format!(
            "id: {}\ntype: {}\nobjective: {}\nmodel_profile: {}\nassumptions: {}\nscope_files: {}\nvalidation: proof_required={}; commands={}\nacceptance: {}\nconstraints: must_do={}; must_not_do={}",
            self.task_id,
            self.task_type.label(),
            compact_text(&self.objective, 220),
            self.model_profile.label(),
            assumptions,
            format_list(&self.scope.files_allowed),
            self.validation.proof_required,
            format_list(&self.validation.required_commands),
            format_list(&self.acceptance_criteria),
            format_list(&self.constraints.must_do),
            format_list(&self.constraints.must_not_do),
        )
    }
}

impl ContextPack {
    pub fn from_bundle(bundle: &TaskContextBundle, contract: &TaskContract) -> Self {
        let budget = ContextPackBudget::default();
        let mut overflow_items = 0;

        let mut project_facts = Vec::new();
        for scope in &bundle.agent_state.allowed_scope {
            project_facts.push(ContextFact {
                fact: scope.clone(),
                provenance: "task_state.allowed_scope".to_string(),
                trust: "high".to_string(),
            });
        }
        if let Some(retrieval) = &bundle.retrieval {
            for item in retrieval
                .items
                .iter()
                .filter(|item| item.source != RetrievalSource::Memory)
            {
                project_facts.push(ContextFact {
                    fact: format!(
                        "{}: {}",
                        item.title,
                        compact_text(&item.content_preview, 240)
                    ),
                    provenance: item.provenance.clone(),
                    trust: trust_label(item.trust).to_string(),
                });
            }
        }
        overflow_items += truncate_with_overflow(&mut project_facts, budget.max_project_facts);

        let mut memory_records = bundle
            .retrieval
            .as_ref()
            .map(|retrieval| {
                retrieval
                    .items
                    .iter()
                    .filter(|item| item.source == RetrievalSource::Memory)
                    .map(|item| ContextFact {
                        fact: format!(
                            "{}: {}",
                            item.title,
                            compact_text(&item.content_preview, 240)
                        ),
                        provenance: item.provenance.clone(),
                        trust: trust_label(item.trust).to_string(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        overflow_items += truncate_with_overflow(&mut memory_records, budget.max_memory_records);

        let mut recent_observations = bundle
            .agent_state
            .observations
            .iter()
            .rev()
            .map(|item| format!("{}: {}", item.source, item.summary))
            .collect::<Vec<_>>();
        overflow_items +=
            truncate_with_overflow(&mut recent_observations, budget.max_recent_observations);

        let mut failure_summaries = bundle
            .agent_state
            .stop_checks
            .iter()
            .rev()
            .filter(|record| record.reason.label() != "no_issue")
            .map(|record| {
                format!(
                    "{}: {} -> {}",
                    record.reason.label(),
                    record.summary,
                    record.next_action.as_deref().unwrap_or("none")
                )
            })
            .chain(
                bundle
                    .agent_state
                    .failed_strategies
                    .iter()
                    .rev()
                    .map(|record| {
                        format!(
                            "{}: {}; better={}",
                            record.failed_strategy, record.reason, record.better_strategy
                        )
                    }),
            )
            .collect::<Vec<_>>();
        overflow_items +=
            truncate_with_overflow(&mut failure_summaries, budget.max_failure_summaries);

        let skill_summaries = Vec::<String>::new();
        let rendered_for_budget = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            contract.objective,
            project_facts
                .iter()
                .map(|fact| &fact.fact)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n"),
            recent_observations.join("\n"),
            failure_summaries.join("\n"),
            memory_records
                .iter()
                .map(|fact| &fact.fact)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n"),
            skill_summaries.join("\n")
        );
        let estimated_tokens = estimate_tokens(&rendered_for_budget) as usize;
        let fingerprint = crate::engine::prompt_context::stable_fingerprint(&rendered_for_budget);

        Self {
            task_id: bundle.task_id.clone(),
            objective: contract.objective.clone(),
            current_stage: format!("{:?}", bundle.agent_state.stage),
            allowed_files: contract.scope.files_allowed.clone(),
            forbidden_actions: bundle.agent_state.forbidden_actions.clone(),
            acceptance_criteria: contract.acceptance_criteria.clone(),
            project_facts,
            recent_observations,
            failure_summaries,
            memory_records,
            skill_summaries,
            budget,
            estimated_tokens,
            overflow_items,
            fingerprint,
        }
    }

    pub fn compact_summary(&self) -> String {
        format!(
            "ContextPack id={} stage={} project_facts={} memory_records={} observations={} failures={} tokens~{} overflow={}",
            self.task_id,
            self.current_stage,
            self.project_facts.len(),
            self.memory_records.len(),
            self.recent_observations.len(),
            self.failure_summaries.len(),
            self.estimated_tokens,
            self.overflow_items
        )
    }

    pub fn format_for_context_zone(&self) -> String {
        format!(
            "id: {}\nstage: {}\nbudget: estimated_tokens={}; max_tokens={}; overflow_items={}; fingerprint={}\nallowed_files: {}\nforbidden_actions: {}\nproject_facts: {}\nmemory_records: {}\nrecent_observations: {}\nfailure_summaries: {}",
            self.task_id,
            self.current_stage,
            self.estimated_tokens,
            self.budget.max_total_estimated_tokens,
            self.overflow_items,
            self.fingerprint,
            format_list(&self.allowed_files),
            format_list(&self.forbidden_actions),
            format_facts(&self.project_facts, 5),
            format_facts(&self.memory_records, 3),
            format_list_limited(&self.recent_observations, 5),
            format_list_limited(&self.failure_summaries, 3),
        )
    }
}

impl ExecutionReport {
    pub fn from_closeout(contract: &TaskContract, closeout: &WorkflowCloseout) -> Self {
        let status = match closeout.status {
            StageValidationStatus::Passed => ExecutionReportStatus::Success,
            StageValidationStatus::Partial => ExecutionReportStatus::Partial,
            StageValidationStatus::Failed => ExecutionReportStatus::Failed,
            StageValidationStatus::NotVerified => ExecutionReportStatus::NotVerified,
        };
        let next_steps = closeout
            .residual_risks
            .iter()
            .filter(|risk| risk.as_str() != "none recorded")
            .map(|risk| format!("resolve residual risk: {risk}"))
            .collect();
        Self {
            task_id: contract.task_id.clone(),
            objective: contract.objective.clone(),
            status,
            changed_files: closeout.changed_files.clone(),
            validation_evidence: closeout.validation.clone(),
            risks: closeout.residual_risks.clone(),
            next_steps,
            assumptions: contract.assumptions.clone(),
        }
    }

    pub fn compact_summary(&self) -> String {
        format!(
            "ExecutionReport id={} status={} changed_files={} validation={} risks={} next_steps={}",
            self.task_id,
            self.status.label(),
            self.changed_files.len(),
            self.validation_evidence.len(),
            self.risks.len(),
            self.next_steps.len()
        )
    }
}

pub trait TaskContractBundleExt {
    fn task_contract(&self, required_validation_commands: &[String]) -> TaskContract;
    fn context_pack(&self, contract: &TaskContract) -> ContextPack;
}

impl TaskContractBundleExt for TaskContextBundle {
    fn task_contract(&self, required_validation_commands: &[String]) -> TaskContract {
        TaskContract::from_bundle(self, required_validation_commands)
    }

    fn context_pack(&self, contract: &TaskContract) -> ContextPack {
        ContextPack::from_bundle(self, contract)
    }
}

fn task_type_from_workflow(workflow: WorkflowKind) -> TaskContractType {
    match workflow {
        WorkflowKind::CodeChange | WorkflowKind::BugFix => TaskContractType::CodeChange,
        WorkflowKind::Direct | WorkflowKind::Research | WorkflowKind::Planning => {
            TaskContractType::Analysis
        }
        WorkflowKind::Delegation => TaskContractType::FileTask,
    }
}

fn assumptions_from_bundle(bundle: &TaskContextBundle) -> Vec<TaskAssumption> {
    let mut assumptions = Vec::new();
    if let Some(judgment) = &bundle.workflow_judgment {
        assumptions.extend(assumptions_from_judgment(judgment));
    }
    if assumptions.is_empty() {
        assumptions.push(TaskAssumption {
            assumption: match bundle.route.workflow {
                WorkflowKind::Direct => "direct response should not mutate local state".to_string(),
                _ => "stay within the current workspace and requested task scope".to_string(),
            },
            source: AssumptionSource::DefaultPolicy,
            confidence: ConfidenceLevel::High,
        });
    }
    assumptions
}

fn assumptions_from_judgment(judgment: &ProgrammingWorkflowJudgment) -> Vec<TaskAssumption> {
    judgment
        .assumptions
        .iter()
        .filter(|assumption| !assumption.trim().is_empty())
        .map(|assumption| TaskAssumption {
            assumption: assumption.clone(),
            source: AssumptionSource::PartnerInferred,
            confidence: ConfidenceLevel::Medium,
        })
        .collect()
}

fn project_context_from_bundle(bundle: &TaskContextBundle) -> Vec<String> {
    let mut context = bundle.agent_state.allowed_scope.clone();
    if let Some(goal) = &bundle.goal {
        push_unique(&mut context, format!("goal: {}", goal.title));
    }
    for file in &bundle.relevant_files {
        push_unique(&mut context, format!("file: {}", file.display()));
    }
    context
}

fn acceptance_criteria_from_bundle(bundle: &TaskContextBundle) -> Vec<String> {
    if !bundle.acceptance_checks.is_empty() {
        return bundle.acceptance_checks.clone();
    }
    vec![bundle.agent_state.done_condition.summary.clone()]
}

fn validation_proof_required(bundle: &TaskContextBundle, validation_commands: &[String]) -> bool {
    !validation_commands.is_empty()
        || !matches!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::NotRequired
        )
        || matches!(
            bundle.route.workflow,
            WorkflowKind::CodeChange | WorkflowKind::BugFix
        )
}

fn derive_model_profile(bundle: &TaskContextBundle) -> ModelProfileMode {
    if bundle.agent_state.mode == AgentTaskMode::HighRisk {
        return ModelProfileMode::HumanConfirm;
    }
    if matches!(
        bundle.agent_state.verification_plan.status,
        VerificationStatus::Failed
            | VerificationStatus::Blocked
            | VerificationStatus::UserDeferred
            | VerificationStatus::Unavailable
    ) || bundle.route.risk == RiskLevel::High
    {
        return ModelProfileMode::ReviewRequired;
    }
    let failure_count = bundle.agent_state.consecutive_validation_failures
        + bundle.agent_state.consecutive_edit_failures
        + bundle.agent_state.consecutive_command_failures
        + bundle.agent_state.consecutive_permission_blocks;
    // Low action-score history is advisory trace. Do not constrain the model's
    // tool surface solely because runtime scoring disliked prior actions.
    if failure_count > 0 || bundle.agent_state.mode_score.uncertainty >= 7 {
        return ModelProfileMode::Constrained;
    }
    ModelProfileMode::Standard
}

fn forbidden_commands_from_actions(actions: &[String]) -> Vec<String> {
    actions
        .iter()
        .filter(|action| {
            let lower = action.to_ascii_lowercase();
            lower.contains("destructive")
                || lower.contains("mutation")
                || lower.contains("delete")
                || lower.contains("remove")
        })
        .cloned()
        .collect()
}

fn trust_label(trust: TrustLevel) -> &'static str {
    match trust {
        TrustLevel::Low => "low",
        TrustLevel::Medium => "medium",
        TrustLevel::High => "high",
    }
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !items.contains(&value) {
        items.push(value);
    }
}

pub(super) fn dedupe(items: &mut Vec<String>) {
    let mut deduped = Vec::new();
    for item in std::mem::take(items) {
        push_unique(&mut deduped, item);
    }
    *items = deduped;
}

fn truncate_with_overflow<T>(items: &mut Vec<T>, max: usize) -> usize {
    if items.len() <= max {
        return 0;
    }
    let overflow = items.len() - max;
    items.truncate(max);
    overflow
}

pub(super) fn compact_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    let mut out = trimmed
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

pub(super) fn infer_proposal_active_scope(proposal: &MemoryProposal) -> String {
    let mut scopes = proposal
        .candidates
        .iter()
        .map(|candidate| candidate.scope.clone())
        .collect::<Vec<_>>();
    dedupe(&mut scopes);
    if scopes.is_empty() {
        "none".to_string()
    } else {
        scopes.join(",")
    }
}

pub(super) fn current_memory_proposal_project_identity() -> (Option<String>, Vec<String>) {
    let scope = crate::memory::MemoryScope::default();
    let identity = scope.identity();
    if identity.kind == crate::memory::types::MemoryScopeKind::Project {
        (Some(identity.id), identity.labels)
    } else {
        (None, identity.labels)
    }
}

pub(super) fn memory_proposal_status_reason(status: MemoryProposalStatus) -> &'static str {
    match status {
        MemoryProposalStatus::Accepted => "accepted for memory apply",
        MemoryProposalStatus::Rejected => "rejected by review",
        MemoryProposalStatus::Applied => "applied to long-term memory",
        MemoryProposalStatus::Proposed => "candidate memory requires review before persistence",
        MemoryProposalStatus::NotApplicable => {
            "no durable evidence-backed memory candidate was produced"
        }
    }
}

pub(super) fn memory_proposal_review_operation(
    record: &MemoryProposalReviewRecord,
) -> &'static str {
    let reason = record.proposal.reason.to_ascii_lowercase();
    match record.proposal.status {
        MemoryProposalStatus::Accepted => {
            if reason.contains("conflict resolution") || reason.contains("kept memory proposal") {
                "memory_proposal_conflict_keep"
            } else if reason.contains("edited candidate content") {
                "memory_proposal_edit_accept"
            } else if reason.contains("batch") {
                "memory_proposal_batch_accept"
            } else {
                "memory_proposal_accept"
            }
        }
        MemoryProposalStatus::Rejected => {
            if reason.contains("superseded") {
                "memory_proposal_supersede"
            } else if reason.contains("resolved duplicate/conflict") {
                "memory_proposal_conflict_reject"
            } else if reason.contains("batch") {
                "memory_proposal_batch_reject"
            } else {
                "memory_proposal_reject"
            }
        }
        MemoryProposalStatus::Applied => {
            if record.proposal.source == "repair" {
                "memory_proposal_repair_apply"
            } else {
                "memory_proposal_apply"
            }
        }
        MemoryProposalStatus::Proposed => {
            if reason.contains("edited candidate content") {
                "memory_proposal_edit"
            } else {
                "memory_proposal_create"
            }
        }
        MemoryProposalStatus::NotApplicable => "memory_proposal_not_applicable",
    }
}

fn format_list(items: &[String]) -> String {
    format_list_limited(items, 8)
}

pub(super) fn format_list_limited(items: &[String], max: usize) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    let mut rendered = items
        .iter()
        .take(max)
        .map(|item| compact_text(item, 160))
        .collect::<Vec<_>>();
    if items.len() > max {
        rendered.push(format!("+{} more", items.len() - max));
    }
    rendered.join("; ")
}

fn format_facts(facts: &[ContextFact], max: usize) -> String {
    if facts.is_empty() {
        return "none".to_string();
    }
    let mut rendered = facts
        .iter()
        .take(max)
        .map(|fact| {
            format!(
                "{}:{}: {}",
                compact_text(&fact.provenance, 80),
                fact.trust,
                compact_text(&fact.fact, 160)
            )
        })
        .collect::<Vec<_>>();
    if facts.len() > max {
        rendered.push(format!("+{} more", facts.len() - max));
    }
    rendered.join("; ")
}

#[cfg(test)]
mod tests;
