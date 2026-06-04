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

mod memory_proposal;
mod proposal_store;
mod types;

pub use memory_proposal::*;
pub use proposal_store::*;
pub use types::*;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundReviewPacket {
    pub transcript_excerpt_ids: Vec<String>,
    pub closeout_summary: String,
    pub tool_result_summaries: Vec<String>,
    pub existing_memory_digest: String,
    pub recent_rejected_proposals: Vec<String>,
    pub active_scope: String,
    pub source_task: String,
    pub max_candidate_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundRejectedObservation {
    pub observation: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundMemoryReviewOutput {
    pub candidates: Vec<MemoryProposalCandidate>,
    pub no_op_reason: Option<String>,
    pub rejected_observations: Vec<BackgroundRejectedObservation>,
}

pub struct BackgroundMemoryReviewWorker;

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

impl BackgroundReviewPacket {
    pub fn from_execution_report(report: &ExecutionReport, recent: &[MemoryProposal]) -> Self {
        let rejected = recent
            .iter()
            .filter(|proposal| proposal.status == MemoryProposalStatus::Rejected)
            .take(8)
            .map(|proposal| {
                format!(
                    "{}:{}:{}",
                    proposal.task_id,
                    proposal.source,
                    compact_text(&proposal.reason, 140)
                )
            })
            .collect::<Vec<_>>();
        Self {
            transcript_excerpt_ids: vec![report.task_id.clone()],
            closeout_summary: format!(
                "status={} objective={} changed_files={} validation={} risks={} next_steps={}",
                report.status.label(),
                compact_text(&report.objective, 180),
                report.changed_files.len(),
                report.validation_evidence.len(),
                report.risks.len(),
                report.next_steps.len()
            ),
            tool_result_summaries: report
                .validation_evidence
                .iter()
                .take(8)
                .map(|item| compact_text(item, 220))
                .collect(),
            existing_memory_digest: recent
                .iter()
                .take(8)
                .map(|proposal| {
                    format!(
                        "{}:{}:{}",
                        proposal.task_id,
                        proposal.status.label(),
                        proposal.candidate_kinds().join("+")
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
            recent_rejected_proposals: rejected,
            active_scope: "project".to_string(),
            source_task: report.task_id.clone(),
            max_candidate_count: 3,
        }
    }
}

impl BackgroundMemoryReviewOutput {
    pub fn strict_from_json(text: &str) -> anyhow::Result<Self> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            anyhow::bail!("background memory review output is empty");
        }
        let output: Self = serde_json::from_str(trimmed)?;
        output.validate()?;
        Ok(output)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.candidates.is_empty()
            && self
                .no_op_reason
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
        {
            anyhow::bail!(
                "background memory review output must include candidates or no_op_reason"
            );
        }
        for candidate in &self.candidates {
            if candidate.kind.trim().is_empty()
                || candidate.scope.trim().is_empty()
                || candidate.content.trim().is_empty()
            {
                anyhow::bail!(
                    "background memory review candidate must include kind, scope, and content"
                );
            }
            if candidate.evidence.is_empty() {
                anyhow::bail!("background memory review candidate must include evidence");
            }
        }
        Ok(())
    }
}

impl BackgroundMemoryReviewWorker {
    pub fn review_execution_report(
        packet: &BackgroundReviewPacket,
        report: &ExecutionReport,
    ) -> BackgroundMemoryReviewOutput {
        let mut candidates = Vec::new();
        let mut rejected_observations = Vec::new();

        for next_step in report.next_steps.iter().take(packet.max_candidate_count) {
            candidates.push(MemoryProposalCandidate {
                kind: "next_step".to_string(),
                scope: packet.active_scope.clone(),
                content: format!(
                    "Next step after `{}`: {}",
                    compact_text(&report.objective, 140),
                    compact_text(next_step, 220)
                ),
                evidence: vec![
                    format!("source_task: {}", packet.source_task),
                    format!("closeout: {}", packet.closeout_summary),
                    format!("next_step: {}", compact_text(next_step, 220)),
                ],
            });
        }

        for risk in report
            .risks
            .iter()
            .filter(|risk| risk.as_str() != "none recorded")
            .take(packet.max_candidate_count.saturating_sub(candidates.len()))
        {
            candidates.push(MemoryProposalCandidate {
                kind: "open_risk".to_string(),
                scope: packet.active_scope.clone(),
                content: format!(
                    "Open risk after `{}`: {}",
                    compact_text(&report.objective, 140),
                    compact_text(risk, 220)
                ),
                evidence: vec![
                    format!("source_task: {}", packet.source_task),
                    format!("closeout: {}", packet.closeout_summary),
                    format!("risk: {}", compact_text(risk, 220)),
                ],
            });
        }

        if report.validation_evidence.is_empty() {
            rejected_observations.push(BackgroundRejectedObservation {
                observation: "validation_baseline".to_string(),
                reason: "no validation evidence in closeout packet".to_string(),
            });
        } else if candidates.len() < packet.max_candidate_count {
            candidates.push(MemoryProposalCandidate {
                kind: "validation_baseline".to_string(),
                scope: packet.active_scope.clone(),
                content: format!(
                    "Validation baseline for `{}`: {}",
                    compact_text(&report.objective, 140),
                    compact_text(&report.validation_evidence.join("; "), 260)
                ),
                evidence: std::iter::once(format!("source_task: {}", packet.source_task))
                    .chain(std::iter::once(format!(
                        "closeout: {}",
                        packet.closeout_summary
                    )))
                    .chain(packet.tool_result_summaries.clone())
                    .collect(),
            });
        }

        let no_op_reason = if candidates.is_empty() {
            Some("closeout packet did not contain durable project progress candidates".to_string())
        } else {
            None
        };
        BackgroundMemoryReviewOutput {
            candidates,
            no_op_reason,
            rejected_observations,
        }
    }

    pub fn proposal_from_output(
        packet: &BackgroundReviewPacket,
        output: BackgroundMemoryReviewOutput,
    ) -> MemoryProposal {
        let status = if output.candidates.is_empty() {
            MemoryProposalStatus::NotApplicable
        } else {
            MemoryProposalStatus::Proposed
        };
        let reason = if let Some(no_op) = output.no_op_reason {
            no_op
        } else {
            "background review produced review-required memory proposal candidates".to_string()
        };
        MemoryProposal {
            task_id: format!("background-{}", packet.source_task),
            source: "background".to_string(),
            status,
            candidates: output.candidates,
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason,
        }
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

pub(super) fn memory_write_target_for_proposal_candidate(
    candidate: &MemoryProposalCandidate,
) -> crate::memory::MemoryWriteTarget {
    let scope = candidate.scope.trim();
    if let Some(topic) = scope.strip_prefix("topic:") {
        if let Some(topic) = normalize_proposal_scope_component(topic) {
            return crate::memory::MemoryWriteTarget::Topic(topic);
        }
    }
    match scope {
        "user" => crate::memory::MemoryWriteTarget::User,
        "topic" => crate::memory::MemoryWriteTarget::Topic(candidate.kind.clone()),
        "project" => crate::memory::MemoryWriteTarget::Index,
        _ => crate::memory::MemoryWriteTarget::Auto,
    }
}

pub(super) fn proposal_gate_report(
    proposal: &MemoryProposal,
    conflict_groups: &[MemoryProposalConflictGroup],
) -> Vec<MemoryProposalGateDecision> {
    let mut gates = Vec::new();
    gates.push(MemoryProposalGateDecision {
        gate: "write_policy".to_string(),
        candidate_index: None,
        status: if proposal.write_policy == "review_required" {
            "passed".to_string()
        } else {
            "warn".to_string()
        },
        reason: format!("write_policy={}", proposal.write_policy),
    });
    gates.push(MemoryProposalGateDecision {
        gate: "evidence".to_string(),
        candidate_index: None,
        status: if proposal.evidence_items() > 0 {
            "passed".to_string()
        } else {
            "missing".to_string()
        },
        reason: format!("evidence_items={}", proposal.evidence_items()),
    });
    let evidence_findings = proposal_evidence_minimum_findings(proposal);
    let missing_evidence = evidence_findings
        .iter()
        .filter(|finding| finding.status == "missing")
        .count();
    let review_required_evidence = evidence_findings
        .iter()
        .filter(|finding| finding.status == "review_required")
        .count();
    gates.push(MemoryProposalGateDecision {
        gate: "minimum_evidence".to_string(),
        candidate_index: None,
        status: if missing_evidence > 0 {
            "missing".to_string()
        } else if review_required_evidence > 0 {
            "review_required".to_string()
        } else {
            "passed".to_string()
        },
        reason: if evidence_findings.is_empty() {
            "candidate_evidence=0".to_string()
        } else {
            evidence_findings
                .iter()
                .map(|finding| {
                    format!(
                        "{}:{}:{}",
                        finding.kind, finding.status, finding.requirement
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    for (idx, finding) in evidence_findings.iter().enumerate() {
        gates.push(MemoryProposalGateDecision {
            gate: "minimum_evidence".to_string(),
            candidate_index: Some(idx),
            status: finding.status.to_string(),
            reason: format!("{}:{}", finding.kind, finding.requirement),
        });
    }
    let sensitivity_findings = proposal_sensitivity_findings(proposal);
    let blocked_sensitivity = sensitivity_findings
        .iter()
        .filter(|finding| finding.status == "blocked")
        .count();
    let review_required_sensitivity = sensitivity_findings
        .iter()
        .filter(|finding| finding.status == "review_required")
        .count();
    gates.push(MemoryProposalGateDecision {
        gate: "sensitivity".to_string(),
        candidate_index: None,
        status: if blocked_sensitivity > 0 {
            "blocked".to_string()
        } else if review_required_sensitivity > 0 {
            "review_required".to_string()
        } else {
            "passed".to_string()
        },
        reason: if sensitivity_findings.is_empty() {
            "candidate_sensitivity=0".to_string()
        } else {
            sensitivity_findings
                .iter()
                .map(|finding| {
                    format!(
                        "{}:{}:{}:{}",
                        finding.kind, finding.status, finding.sensitivity, finding.reason
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    for (idx, finding) in sensitivity_findings.iter().enumerate() {
        gates.push(MemoryProposalGateDecision {
            gate: "sensitivity".to_string(),
            candidate_index: Some(idx),
            status: finding.status.to_string(),
            reason: format!(
                "{}:{}:{}",
                finding.kind, finding.sensitivity, finding.reason
            ),
        });
    }
    gates.push(MemoryProposalGateDecision {
        gate: "durable_write".to_string(),
        candidate_index: None,
        status: if proposal.write_performed {
            "warn".to_string()
        } else {
            "passed".to_string()
        },
        reason: format!("write_performed={}", proposal.write_performed),
    });
    gates.push(MemoryProposalGateDecision {
        gate: "candidate_count".to_string(),
        candidate_index: None,
        status: if proposal.candidates.is_empty() {
            "missing".to_string()
        } else {
            "passed".to_string()
        },
        reason: format!("candidates={}", proposal.candidates.len()),
    });
    let scope_findings = proposal_scope_identity_findings(proposal);
    let ambiguous_count = scope_findings
        .iter()
        .filter(|finding| finding.status == "review_required")
        .count();
    let invalid_count = scope_findings
        .iter()
        .filter(|finding| finding.status == "missing")
        .count();
    gates.push(MemoryProposalGateDecision {
        gate: "scope_identity".to_string(),
        candidate_index: None,
        status: if invalid_count > 0 {
            "missing".to_string()
        } else if ambiguous_count > 0 {
            "review_required".to_string()
        } else {
            "passed".to_string()
        },
        reason: if scope_findings.is_empty() {
            "candidate_scopes=0".to_string()
        } else {
            scope_findings
                .iter()
                .map(|finding| {
                    format!(
                        "{}:{}:{}",
                        finding.scope, finding.status, finding.identity_label
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    for (idx, finding) in scope_findings.iter().enumerate() {
        gates.push(MemoryProposalGateDecision {
            gate: "scope_identity".to_string(),
            candidate_index: Some(idx),
            status: finding.status.to_string(),
            reason: format!("{}:{}", finding.scope, finding.identity_label),
        });
    }
    let conflict_count = conflict_groups
        .iter()
        .filter(|group| group.group_type == "conflict")
        .count();
    let duplicate_count = conflict_groups
        .iter()
        .filter(|group| group.group_type == "duplicate")
        .count();
    gates.push(MemoryProposalGateDecision {
        gate: "duplicate_conflict".to_string(),
        candidate_index: None,
        status: if conflict_count > 0 {
            "review_required".to_string()
        } else if duplicate_count > 0 {
            "warn".to_string()
        } else {
            "passed".to_string()
        },
        reason: format!("duplicates={duplicate_count} conflicts={conflict_count}"),
    });
    gates
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProposalSensitivityFinding {
    kind: String,
    status: &'static str,
    sensitivity: &'static str,
    reason: String,
}

pub(super) fn proposal_blocking_sensitivity_reason(proposal: &MemoryProposal) -> Option<String> {
    proposal_sensitivity_findings(proposal)
        .into_iter()
        .find(|finding| finding.status == "blocked")
        .map(|finding| format!("{}:{}", finding.sensitivity, finding.reason))
}

pub(super) fn proposal_blocking_minimum_evidence_reason(
    proposal: &MemoryProposal,
) -> Option<String> {
    proposal_evidence_minimum_findings(proposal)
        .into_iter()
        .find(|finding| finding.status == "missing")
        .map(|finding| format!("{}:{}", finding.kind, finding.requirement))
}

fn proposal_sensitivity_findings(proposal: &MemoryProposal) -> Vec<ProposalSensitivityFinding> {
    proposal
        .candidates
        .iter()
        .map(proposal_candidate_sensitivity)
        .collect()
}

fn proposal_candidate_sensitivity(
    candidate: &MemoryProposalCandidate,
) -> ProposalSensitivityFinding {
    let kind = normalize_proposal_kind(&candidate.kind);
    match crate::memory::scan_memory_content(&candidate.content) {
        Ok(crate::memory::SensitivityLevel::Public) => ProposalSensitivityFinding {
            kind,
            status: "passed",
            sensitivity: "public_project_fact",
            reason: "public_or_project_fact".to_string(),
        },
        Ok(crate::memory::SensitivityLevel::LocalOnly) => ProposalSensitivityFinding {
            kind,
            status: "review_required",
            sensitivity: "private_user_data",
            reason: "local_only_memory_requires_review_and_minimization".to_string(),
        },
        Ok(crate::memory::SensitivityLevel::SecretLike) => ProposalSensitivityFinding {
            kind,
            status: "blocked",
            sensitivity: "secret_or_credential",
            reason: "secret_like_content".to_string(),
        },
        Ok(crate::memory::SensitivityLevel::Unsafe) => ProposalSensitivityFinding {
            kind,
            status: "blocked",
            sensitivity: "security_sensitive_instruction",
            reason: "unsafe_content".to_string(),
        },
        Err(issue) => ProposalSensitivityFinding {
            kind,
            status: "blocked",
            sensitivity: match issue.sensitivity {
                crate::memory::SensitivityLevel::SecretLike => "secret_or_credential",
                crate::memory::SensitivityLevel::Unsafe => "security_sensitive_instruction",
                crate::memory::SensitivityLevel::LocalOnly => "private_user_data",
                crate::memory::SensitivityLevel::Public => "public_project_fact",
            },
            reason: issue.code,
        },
    }
}

pub(super) fn memory_proposal_candidate_evidence_refs(
    proposal_id: &str,
    proposal_source: &str,
    candidate: &MemoryProposalCandidate,
) -> Vec<crate::memory::MemoryEvidenceRef> {
    let mut refs = candidate
        .evidence
        .iter()
        .enumerate()
        .map(|(idx, evidence)| {
            crate::memory::MemoryEvidenceRef::new(
                memory_proposal_evidence_kind(evidence),
                format!("memory_proposal:{proposal_id}:evidence:{idx}"),
                evidence.clone(),
                memory_proposal_evidence_confidence(evidence),
            )
        })
        .collect::<Vec<_>>();
    refs.push(crate::memory::MemoryEvidenceRef::new(
        crate::memory::MemoryEvidenceKind::RuntimeObservation,
        format!("memory_proposal:{proposal_id}"),
        format!(
            "accepted proposal source={proposal_source} kind={}",
            candidate.kind
        ),
        0.75,
    ));
    refs
}

fn memory_proposal_evidence_kind(evidence: &str) -> crate::memory::MemoryEvidenceKind {
    let lower = evidence.to_ascii_lowercase();
    if lower.contains("user:") || lower.contains("user_statement") || lower.contains("user message")
    {
        crate::memory::MemoryEvidenceKind::UserStatement
    } else if lower.contains("tool:")
        || lower.contains("tool_output")
        || lower.contains("validation:")
        || lower.contains("cargo ")
        || lower.contains("npm ")
        || lower.contains("pytest")
        || lower.contains("command:")
    {
        crate::memory::MemoryEvidenceKind::ToolOutput
    } else if lower.contains("file:") || lower.contains("changed_files") {
        crate::memory::MemoryEvidenceKind::File
    } else if lower.contains("trace:") {
        crate::memory::MemoryEvidenceKind::Trace
    } else if lower.contains("learning") || lower.contains("experience") {
        crate::memory::MemoryEvidenceKind::LearningEvent
    } else if lower.contains("background:") || lower.contains("inferred") {
        crate::memory::MemoryEvidenceKind::Inference
    } else {
        crate::memory::MemoryEvidenceKind::RuntimeObservation
    }
}

fn memory_proposal_evidence_confidence(evidence: &str) -> f32 {
    match memory_proposal_evidence_kind(evidence) {
        crate::memory::MemoryEvidenceKind::UserStatement => 0.90,
        crate::memory::MemoryEvidenceKind::ToolOutput | crate::memory::MemoryEvidenceKind::File => {
            0.85
        }
        crate::memory::MemoryEvidenceKind::Trace
        | crate::memory::MemoryEvidenceKind::RuntimeObservation => 0.75,
        crate::memory::MemoryEvidenceKind::LearningEvent => 0.70,
        crate::memory::MemoryEvidenceKind::Inference => 0.45,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProposalEvidenceMinimumFinding {
    kind: String,
    status: &'static str,
    requirement: &'static str,
}

fn proposal_evidence_minimum_findings(
    proposal: &MemoryProposal,
) -> Vec<ProposalEvidenceMinimumFinding> {
    proposal
        .candidates
        .iter()
        .map(proposal_candidate_evidence_minimum)
        .collect()
}

fn proposal_candidate_evidence_minimum(
    candidate: &MemoryProposalCandidate,
) -> ProposalEvidenceMinimumFinding {
    let kind = normalize_proposal_kind(&candidate.kind);
    if candidate.evidence.is_empty() {
        return ProposalEvidenceMinimumFinding {
            kind,
            status: "missing",
            requirement: "at_least_one_evidence_item",
        };
    }

    let evidence = candidate
        .evidence
        .iter()
        .map(|item| item.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let has_source_task = evidence_contains_any(&evidence, &["source_task:", "source task:"]);
    let has_closeout = evidence_contains_any(&evidence, &["closeout:", "execution_report:"]);
    let has_user_statement =
        evidence_contains_any(&evidence, &["user:", "user_statement", "user message"]);
    let has_tool_or_file = evidence_contains_any(
        &evidence,
        &[
            "tool:",
            "tool_output",
            "file:",
            "validation:",
            "cargo ",
            "npm ",
            "pytest",
            "command:",
        ],
    );
    let has_risk = evidence_contains_any(&evidence, &["risk:", "residual risk"]);
    let has_next_step = evidence_contains_any(&evidence, &["next_step:", "next step:"]);

    match kind.as_str() {
        "user_preference" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_user_statement {
                "passed"
            } else {
                "review_required"
            },
            requirement: "explicit_user_statement",
        },
        "project_status" | "next_step" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_source_task && has_closeout && (has_next_step || has_tool_or_file) {
                "passed"
            } else {
                "review_required"
            },
            requirement: "source_task_closeout_and_progress_evidence",
        },
        "open_risk" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_source_task && has_closeout && has_risk {
                "passed"
            } else {
                "review_required"
            },
            requirement: "source_task_closeout_and_risk_evidence",
        },
        "validation_baseline" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_source_task && has_closeout && has_tool_or_file {
                "passed"
            } else {
                "review_required"
            },
            requirement: "source_task_closeout_and_validation_evidence",
        },
        "successful_fix" | "failure_pattern" | "tool_quirk" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_tool_or_file || has_closeout {
                "passed"
            } else {
                "review_required"
            },
            requirement: "tool_file_trace_or_closeout_evidence",
        },
        "project_fact" | "workflow_convention" | "decision" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_source_task || has_tool_or_file || has_closeout || has_user_statement {
                "passed"
            } else {
                "review_required"
            },
            requirement: "source_tool_file_closeout_or_user_evidence",
        },
        _ => ProposalEvidenceMinimumFinding {
            kind,
            status: "passed",
            requirement: "at_least_one_evidence_item",
        },
    }
}

fn normalize_proposal_kind(kind: &str) -> String {
    kind.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn evidence_contains_any(evidence: &[String], needles: &[&str]) -> bool {
    evidence
        .iter()
        .any(|item| needles.iter().any(|needle| item.contains(needle)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProposalScopeIdentityFinding {
    scope: String,
    status: &'static str,
    identity_label: String,
}

fn proposal_scope_identity_findings(
    proposal: &MemoryProposal,
) -> Vec<ProposalScopeIdentityFinding> {
    proposal
        .candidates
        .iter()
        .map(proposal_candidate_scope_identity)
        .collect()
}

fn proposal_candidate_scope_identity(
    candidate: &MemoryProposalCandidate,
) -> ProposalScopeIdentityFinding {
    let scope = candidate.scope.trim().to_ascii_lowercase();
    if scope.is_empty() {
        return ProposalScopeIdentityFinding {
            scope,
            status: "missing",
            identity_label: "missing".to_string(),
        };
    }
    if let Some(topic) = scope.strip_prefix("topic:") {
        return match normalize_proposal_scope_component(topic) {
            Some(topic) => ProposalScopeIdentityFinding {
                scope,
                status: "passed",
                identity_label: format!("topic:{topic}"),
            },
            None => ProposalScopeIdentityFinding {
                scope,
                status: "missing",
                identity_label: "invalid_topic".to_string(),
            },
        };
    }
    match scope.as_str() {
        "user" | "project" | "session" | "agent" => ProposalScopeIdentityFinding {
            identity_label: scope.clone(),
            scope,
            status: "passed",
        },
        "topic" => ProposalScopeIdentityFinding {
            scope,
            status: "review_required",
            identity_label: "ambiguous_topic:missing_topic_id".to_string(),
        },
        _ => ProposalScopeIdentityFinding {
            scope,
            status: "missing",
            identity_label: "unknown_scope".to_string(),
        },
    }
}

fn normalize_proposal_scope_component(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if (ch == '-' || ch == '_' || ch == '.' || ch.is_whitespace())
            && !last_dash
            && !out.is_empty()
        {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryProposalCandidateSignal {
    proposal_id: String,
    candidate_index: usize,
    status: MemoryProposalStatus,
    source: String,
    scope: String,
    kind: String,
    key: String,
    value: String,
    has_explicit_key: bool,
    normalized_value: String,
    normalized_content: String,
    content: String,
}

pub(super) fn memory_proposal_conflict_groups(
    proposal: &MemoryProposal,
    peer_records: &[MemoryProposalReviewRecord],
) -> Vec<MemoryProposalConflictGroup> {
    let current = memory_proposal_candidate_signals(proposal);
    if current.is_empty() {
        return Vec::new();
    }
    let mut peers = peer_records
        .iter()
        .filter(|record| {
            !matches!(
                record.proposal.status,
                MemoryProposalStatus::Rejected | MemoryProposalStatus::NotApplicable
            )
        })
        .flat_map(|record| memory_proposal_candidate_signals(&record.proposal))
        .collect::<Vec<_>>();
    peers.extend(current.clone());

    let mut groups = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for signal in current {
        let duplicate_matches = peers
            .iter()
            .filter(|peer| {
                peer.proposal_id != signal.proposal_id
                    && peer.normalized_content == signal.normalized_content
            })
            .cloned()
            .collect::<Vec<_>>();
        if !duplicate_matches.is_empty() {
            let mut matches = vec![signal.clone()];
            matches.extend(duplicate_matches);
            push_memory_proposal_conflict_group(
                &mut groups,
                &mut seen,
                "duplicate",
                &signal,
                matches,
            );
        }

        let conflict_matches = peers
            .iter()
            .filter(|peer| {
                peer.proposal_id != signal.proposal_id
                    && peer.scope == signal.scope
                    && peer.kind == signal.kind
                    && peer.key == signal.key
                    && peer.has_explicit_key
                    && signal.has_explicit_key
                    && peer.normalized_value != signal.normalized_value
            })
            .cloned()
            .collect::<Vec<_>>();
        if !conflict_matches.is_empty() {
            let mut matches = vec![signal.clone()];
            matches.extend(conflict_matches);
            push_memory_proposal_conflict_group(
                &mut groups,
                &mut seen,
                "conflict",
                &signal,
                matches,
            );
        }
    }
    groups.sort_by(|a, b| {
        a.group_type
            .cmp(&b.group_type)
            .then_with(|| a.scope.cmp(&b.scope))
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.key.cmp(&b.key))
    });
    groups
}

fn memory_proposal_candidate_signals(
    proposal: &MemoryProposal,
) -> Vec<MemoryProposalCandidateSignal> {
    proposal
        .candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, candidate)| memory_proposal_candidate_signal(proposal, idx, candidate))
        .collect()
}

fn memory_proposal_candidate_signal(
    proposal: &MemoryProposal,
    candidate_index: usize,
    candidate: &MemoryProposalCandidate,
) -> Option<MemoryProposalCandidateSignal> {
    let content = candidate.content.trim();
    if content.is_empty() {
        return None;
    }
    let explicit_pair = content
        .lines()
        .map(str::trim)
        .find_map(|line| line.split_once(':'));
    let (raw_key, raw_value, has_explicit_key) = explicit_pair
        .map(|(key, value)| (key.trim(), value.trim(), true))
        .unwrap_or((candidate.kind.as_str(), content, false));
    let key = normalize_memory_proposal_key(raw_key, &candidate.kind);
    let value = raw_value.trim().trim_matches(['`', '"', '\'']).to_string();
    let normalized_value = normalize_memory_proposal_text(&value);
    let normalized_content = normalize_memory_proposal_text(content);
    if key.is_empty() || normalized_value.is_empty() || normalized_content.is_empty() {
        return None;
    }
    Some(MemoryProposalCandidateSignal {
        proposal_id: stable_memory_proposal_id(proposal),
        candidate_index,
        status: proposal.status,
        source: proposal.source.clone(),
        scope: candidate.scope.trim().to_ascii_lowercase(),
        kind: candidate.kind.trim().to_ascii_lowercase(),
        key,
        value,
        has_explicit_key,
        normalized_value,
        normalized_content,
        content: content.to_string(),
    })
}

fn normalize_memory_proposal_key(raw_key: &str, kind: &str) -> String {
    let key = normalize_memory_proposal_text(raw_key);
    match key.as_str() {
        "" | "memory" | "note" | "preference" | "user preference" | "project" | "project fact"
        | "project convention" | "convention" => normalize_memory_proposal_text(kind),
        _ => key,
    }
}

fn normalize_memory_proposal_text(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_memory_proposal_conflict_group(
    groups: &mut Vec<MemoryProposalConflictGroup>,
    seen: &mut std::collections::HashSet<String>,
    group_type: &str,
    signal: &MemoryProposalCandidateSignal,
    mut matches: Vec<MemoryProposalCandidateSignal>,
) {
    matches.sort_by(|a, b| {
        a.proposal_id
            .cmp(&b.proposal_id)
            .then_with(|| a.candidate_index.cmp(&b.candidate_index))
    });
    matches
        .dedup_by(|a, b| a.proposal_id == b.proposal_id && a.candidate_index == b.candidate_index);
    let identity = format!(
        "{}:{}:{}:{}:{}",
        group_type,
        signal.scope,
        signal.kind,
        signal.key,
        matches
            .iter()
            .map(|item| format!("{}#{}", item.proposal_id, item.candidate_index))
            .collect::<Vec<_>>()
            .join(",")
    );
    if !seen.insert(identity) {
        return;
    }
    groups.push(MemoryProposalConflictGroup {
        group_type: group_type.to_string(),
        key: signal.key.clone(),
        scope: signal.scope.clone(),
        kind: signal.kind.clone(),
        matches: matches
            .into_iter()
            .map(|item| MemoryProposalConflictMatch {
                proposal_id: item.proposal_id,
                candidate_index: item.candidate_index,
                status: item.status,
                source: item.source,
                value: compact_text(&item.value, 160),
                content: compact_text(&item.content, 220),
            })
            .collect(),
        resolution_hint: if group_type == "duplicate" {
            "reject duplicate or keep one proposal before apply".to_string()
        } else if signal.kind == "user_preference" || signal.kind == "preference" {
            "prefer newer explicit user correction; reject or edit the older preference".to_string()
        } else {
            "accept one candidate, reject/edit the conflicting candidate, or supersede explicitly"
                .to_string()
        },
    });
}

pub(super) fn summarize_memory_proposal_conflicts(
    groups: &[MemoryProposalConflictGroup],
) -> String {
    if groups.is_empty() {
        return "not_checked".to_string();
    }
    let duplicates = groups
        .iter()
        .filter(|group| group.group_type == "duplicate")
        .count();
    let conflicts = groups
        .iter()
        .filter(|group| group.group_type == "conflict")
        .count();
    let keys = groups
        .iter()
        .take(4)
        .map(|group| format!("{}:{}:{}", group.scope, group.kind, group.key))
        .collect::<Vec<_>>()
        .join(", ");
    format!("duplicates={duplicates} conflicts={conflicts} keys={keys}")
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

pub(super) fn stable_memory_proposal_id(proposal: &MemoryProposal) -> String {
    let seed = if proposal.task_id.trim().is_empty() {
        let candidates = proposal
            .candidates
            .iter()
            .map(|candidate| {
                format!(
                    "{}\x1f{}\x1f{}",
                    candidate.kind, candidate.scope, candidate.content
                )
            })
            .collect::<Vec<_>>()
            .join("\x1e");
        format!("v1\x1f{}\x1f{}", proposal.source, candidates)
    } else {
        format!("v1\x1f{}\x1f{}", proposal.source, proposal.task_id)
    };
    let digest = format!("{:x}", md5::compute(seed));
    format!("mp-{}", &digest[..16])
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
