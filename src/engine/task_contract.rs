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
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskContractType {
    CodeChange,
    DocChange,
    FileTask,
    Analysis,
    Validation,
    Deploy,
    DataTask,
}

impl TaskContractType {
    pub fn label(self) -> &'static str {
        match self {
            Self::CodeChange => "code_change",
            Self::DocChange => "doc_change",
            Self::FileTask => "file_task",
            Self::Analysis => "analysis",
            Self::Validation => "validation",
            Self::Deploy => "deploy",
            Self::DataTask => "data_task",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssumptionSource {
    UserExplicit,
    PartnerInferred,
    ProjectMemory,
    DefaultPolicy,
}

impl AssumptionSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::UserExplicit => "user_explicit",
            Self::PartnerInferred => "partner_inferred",
            Self::ProjectMemory => "project_memory",
            Self::DefaultPolicy => "default_policy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    Low,
    Medium,
    High,
}

impl ConfidenceLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProfileMode {
    Standard,
    Constrained,
    ReviewRequired,
    HumanConfirm,
}

impl ModelProfileMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Constrained => "constrained",
            Self::ReviewRequired => "review_required",
            Self::HumanConfirm => "human_confirm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionReportStatus {
    Success,
    Partial,
    Failed,
    NotVerified,
}

impl ExecutionReportStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::NotVerified => "not_verified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryProposalStatus {
    Proposed,
    Accepted,
    Rejected,
    Applied,
    NotApplicable,
}

impl MemoryProposalStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Applied => "applied",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAssumption {
    pub assumption: String,
    pub source: AssumptionSource,
    pub confidence: ConfidenceLevel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskContractScope {
    pub files_allowed: Vec<String>,
    pub files_forbidden: Vec<String>,
    pub commands_allowed: Vec<String>,
    pub commands_forbidden: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskContractConstraints {
    pub must_do: Vec<String>,
    pub must_not_do: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskValidationContract {
    pub required_commands: Vec<String>,
    pub proof_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRiskContract {
    pub level: RiskLevel,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskReportSchema {
    pub allowed_statuses: Vec<ExecutionReportStatus>,
    pub include_changed_files: bool,
    pub include_validation_evidence: bool,
    pub include_risks: bool,
    pub include_next_steps: bool,
}

impl Default for TaskReportSchema {
    fn default() -> Self {
        Self {
            allowed_statuses: vec![
                ExecutionReportStatus::Success,
                ExecutionReportStatus::Partial,
                ExecutionReportStatus::Failed,
                ExecutionReportStatus::NotVerified,
            ],
            include_changed_files: true,
            include_validation_evidence: true,
            include_risks: true,
            include_next_steps: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskContract {
    pub task_id: String,
    pub task_type: TaskContractType,
    pub objective: String,
    pub user_context: Vec<String>,
    pub project_context: Vec<String>,
    pub assumptions: Vec<TaskAssumption>,
    pub scope: TaskContractScope,
    pub constraints: TaskContractConstraints,
    pub acceptance_criteria: Vec<String>,
    pub validation: TaskValidationContract,
    pub risk: TaskRiskContract,
    pub model_profile: ModelProfileMode,
    pub report_schema: TaskReportSchema,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPackBudget {
    pub max_user_facts: usize,
    pub max_project_facts: usize,
    pub max_recent_observations: usize,
    pub max_failure_summaries: usize,
    pub max_memory_records: usize,
    pub max_skill_summaries: usize,
    pub max_total_estimated_tokens: usize,
}

impl Default for ContextPackBudget {
    fn default() -> Self {
        Self {
            max_user_facts: 5,
            max_project_facts: 10,
            max_recent_observations: 8,
            max_failure_summaries: 3,
            max_memory_records: 5,
            max_skill_summaries: 2,
            max_total_estimated_tokens: 4_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFact {
    pub fact: String,
    pub provenance: String,
    pub trust: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPack {
    pub task_id: String,
    pub objective: String,
    pub current_stage: String,
    pub allowed_files: Vec<String>,
    pub forbidden_actions: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub project_facts: Vec<ContextFact>,
    pub recent_observations: Vec<String>,
    pub failure_summaries: Vec<String>,
    pub memory_records: Vec<ContextFact>,
    pub skill_summaries: Vec<String>,
    pub budget: ContextPackBudget,
    pub estimated_tokens: usize,
    pub overflow_items: usize,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub task_id: String,
    pub objective: String,
    pub status: ExecutionReportStatus,
    pub changed_files: Vec<String>,
    pub validation_evidence: Vec<String>,
    pub risks: Vec<String>,
    pub next_steps: Vec<String>,
    pub assumptions: Vec<TaskAssumption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProposalCandidate {
    pub kind: String,
    pub scope: String,
    pub content: String,
    pub evidence: Vec<String>,
}

fn default_memory_proposal_source() -> String {
    "closeout".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProposal {
    pub task_id: String,
    #[serde(default = "default_memory_proposal_source")]
    pub source: String,
    pub status: MemoryProposalStatus,
    pub candidates: Vec<MemoryProposalCandidate>,
    pub write_policy: String,
    pub write_performed: bool,
    pub reason: String,
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

#[derive(Debug, Clone)]
pub struct MemoryProposalReviewStore {
    path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryProposalBatchFilter {
    pub source: Option<String>,
    pub scope: Option<String>,
    pub project: Option<String>,
    pub status: Option<MemoryProposalStatus>,
    pub stale_days: Option<i64>,
    pub duplicate_only: bool,
    pub blocked_only: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryProposalBatchUpdate {
    pub matched: usize,
    pub updated: usize,
    pub proposal_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryProposalBatchApply {
    pub matched: usize,
    pub applied: usize,
    pub applied_candidates: usize,
    pub failed: usize,
    pub proposal_ids: Vec<String>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryProposalConflictResolution {
    pub kept_id: String,
    pub accepted_keep: bool,
    pub rejected_ids: Vec<String>,
    pub conflict_groups: usize,
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

impl MemoryProposal {
    pub fn from_execution_report(report: &ExecutionReport) -> Self {
        let mut candidates = Vec::new();
        match report.status {
            ExecutionReportStatus::Success => {
                if !report.changed_files.is_empty() && !report.validation_evidence.is_empty() {
                    candidates.push(MemoryProposalCandidate {
                        kind: "successful_fix".to_string(),
                        scope: "project".to_string(),
                        content: format!(
                            "Completed `{}` with changed files: {}; validation: {}",
                            compact_text(&report.objective, 180),
                            format_list_limited(&report.changed_files, 5),
                            compact_text(&report.validation_evidence.join("; "), 220)
                        ),
                        evidence: proposal_evidence(report),
                    });
                }
            }
            ExecutionReportStatus::Partial
            | ExecutionReportStatus::Failed
            | ExecutionReportStatus::NotVerified => {
                let has_evidence =
                    !report.validation_evidence.is_empty() || !report.risks.is_empty();
                if has_evidence {
                    candidates.push(MemoryProposalCandidate {
                        kind: "failure_pattern".to_string(),
                        scope: "project".to_string(),
                        content: format!(
                            "Task `{}` ended {}; risks: {}",
                            compact_text(&report.objective, 180),
                            report.status.label(),
                            format_list_limited(&report.risks, 5)
                        ),
                        evidence: proposal_evidence(report),
                    });
                }
            }
        }
        let status = if candidates.is_empty() {
            MemoryProposalStatus::NotApplicable
        } else {
            MemoryProposalStatus::Proposed
        };
        let reason = if candidates.is_empty() {
            "no durable evidence-backed memory candidate was produced".to_string()
        } else {
            "candidate memory requires review before persistence".to_string()
        };
        Self {
            task_id: report.task_id.clone(),
            source: "closeout".to_string(),
            status,
            candidates,
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason,
        }
    }

    pub fn evidence_items(&self) -> usize {
        self.candidates
            .iter()
            .map(|candidate| candidate.evidence.len())
            .sum()
    }

    pub fn candidate_kinds(&self) -> Vec<String> {
        let mut kinds = self
            .candidates
            .iter()
            .map(|candidate| candidate.kind.clone())
            .collect::<Vec<_>>();
        dedupe(&mut kinds);
        kinds
    }

    pub fn compact_summary(&self) -> String {
        format!(
            "MemoryProposal id={} status={} candidates={} write_policy={} write_performed={} evidence={}",
            self.task_id,
            self.status.label(),
            self.candidates.len(),
            self.write_policy,
            self.write_performed,
            self.evidence_items()
        )
    }

    pub fn format_for_final_response(&self) -> String {
        if self.candidates.is_empty() {
            return String::new();
        }
        let mut lines = vec![
            "\nMemory proposal:".to_string(),
            format!(
                "- Status: {} candidates={} evidence={}",
                self.status.label(),
                self.candidates.len(),
                self.evidence_items()
            ),
            format!(
                "- Write policy: {} write_performed={}",
                self.write_policy, self.write_performed
            ),
            format!("- Reason: {}", self.reason),
        ];
        for candidate in self.candidates.iter().take(3) {
            lines.push(format!(
                "- Candidate: kind={} scope={} evidence={} :: {}",
                candidate.kind,
                candidate.scope,
                candidate.evidence.len(),
                compact_text(&candidate.content, 180)
            ));
        }
        lines.join("\n")
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

impl MemoryProposalReviewStore {
    pub fn default_path() -> PathBuf {
        if let Ok(path) = std::env::var("PRIORITY_AGENT_MEMORY_PROPOSALS_PATH") {
            return PathBuf::from(path);
        }
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("memory_proposals.jsonl")
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn list(&self) -> Vec<MemoryProposal> {
        self.list_records()
            .into_iter()
            .map(|record| record.proposal)
            .collect()
    }

    pub fn list_records(&self) -> Vec<MemoryProposalReviewRecord> {
        let content = std::fs::read_to_string(&self.path).unwrap_or_default();
        let mut latest = HashMap::<String, MemoryProposalReviewRecord>::new();
        for line in content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let Ok(record) = serde_json::from_str::<MemoryProposalReviewRecord>(line) else {
                continue;
            };
            let mut record = record;
            if record.id.trim().is_empty() || record.id == record.proposal.task_id {
                record.id = stable_memory_proposal_id(&record.proposal);
            }
            let key = if record.proposal.task_id.trim().is_empty() {
                record.id.clone()
            } else {
                record.proposal.task_id.clone()
            };
            latest.insert(key, record);
        }
        let mut records = latest.into_values().collect::<Vec<_>>();
        let all_records = records.clone();
        for record in &mut records {
            if record.project_id.is_none() && record.project_labels.is_empty() {
                let (project_id, project_labels) = current_memory_proposal_project_identity();
                record.project_id = project_id;
                record.project_labels = project_labels;
            }
            let conflict_groups = memory_proposal_conflict_groups(&record.proposal, &all_records);
            if record.conflict_groups.is_empty() {
                record.conflict_groups = conflict_groups;
            }
            if record.duplicate_conflict_summary.trim().is_empty()
                || record.duplicate_conflict_summary == "not_checked"
            {
                record.duplicate_conflict_summary =
                    summarize_memory_proposal_conflicts(&record.conflict_groups);
            }
            if !record
                .gate_report
                .iter()
                .any(|gate| gate.gate == "duplicate_conflict")
            {
                record.gate_report =
                    proposal_gate_report(&record.proposal, &record.conflict_groups);
            }
        }
        records.sort_by(|a, b| {
            a.proposal
                .task_id
                .cmp(&b.proposal.task_id)
                .then_with(|| a.id.cmp(&b.id))
        });
        records
    }

    pub fn get(&self, id_or_prefix: &str) -> Option<MemoryProposal> {
        self.get_record(id_or_prefix).map(|record| record.proposal)
    }

    pub fn get_record(&self, id_or_prefix: &str) -> Option<MemoryProposalReviewRecord> {
        self.list_records().into_iter().find(|record| {
            record.id == id_or_prefix
                || record.id.starts_with(id_or_prefix)
                || record.proposal.task_id == id_or_prefix
                || record.proposal.task_id.starts_with(id_or_prefix)
        })
    }

    pub fn upsert(&self, proposal: &MemoryProposal) -> anyhow::Result<()> {
        if proposal.status == MemoryProposalStatus::NotApplicable {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let now = chrono::Utc::now().to_rfc3339();
        let previous = self.get_record(&proposal.task_id);
        let created_at = previous
            .as_ref()
            .map(|record| record.created_at.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| now.clone());
        let mut status_history = previous
            .as_ref()
            .map(|record| record.status_history.clone())
            .unwrap_or_default();
        let should_append_history = status_history
            .last()
            .map(|entry| entry.status != proposal.status || entry.reason != proposal.reason)
            .unwrap_or(true);
        if should_append_history {
            status_history.push(MemoryProposalStatusHistoryEntry {
                at: now.clone(),
                status: proposal.status,
                reason: proposal.reason.clone(),
            });
        }
        let sibling_records = self
            .list_records()
            .into_iter()
            .filter(|record| record.proposal.task_id != proposal.task_id)
            .collect::<Vec<_>>();
        let conflict_groups = memory_proposal_conflict_groups(proposal, &sibling_records);
        let duplicate_conflict_summary = summarize_memory_proposal_conflicts(&conflict_groups);
        let record_id = previous
            .as_ref()
            .map(|record| record.id.clone())
            .filter(|id| !id.trim().is_empty() && id != &proposal.task_id)
            .unwrap_or_else(|| stable_memory_proposal_id(proposal));
        let (default_project_id, default_project_labels) =
            current_memory_proposal_project_identity();
        let project_id = previous
            .as_ref()
            .and_then(|record| record.project_id.clone())
            .or(default_project_id);
        let project_labels = previous
            .as_ref()
            .map(|record| record.project_labels.clone())
            .filter(|labels| !labels.is_empty())
            .unwrap_or(default_project_labels);
        let record = MemoryProposalReviewRecord {
            id: record_id,
            proposal: proposal.clone(),
            created_at,
            updated_at: now,
            source_session: std::env::var("PRIORITY_AGENT_SESSION_ID").ok(),
            source_task: proposal.task_id.clone(),
            source: proposal.source.clone(),
            active_scope: infer_proposal_active_scope(proposal),
            project_id,
            project_labels,
            gate_report: proposal_gate_report(proposal, &conflict_groups),
            duplicate_conflict_summary: if duplicate_conflict_summary != "not_checked" {
                duplicate_conflict_summary
            } else {
                previous
                    .and_then(|record| {
                        if record.duplicate_conflict_summary.trim().is_empty() {
                            None
                        } else {
                            Some(record.duplicate_conflict_summary)
                        }
                    })
                    .unwrap_or_else(|| "not_checked".to_string())
            },
            conflict_groups,
            status_history,
        };
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", serde_json::to_string(&record)?)?;
        self.record_review_operation(&record)?;
        Ok(())
    }

    fn record_review_operation(&self, record: &MemoryProposalReviewRecord) -> anyhow::Result<()> {
        let Some(base_dir) = self.review_operation_base_dir() else {
            return Ok(());
        };
        let provider = crate::memory::LocalMemoryProvider::with_base_dir(base_dir);
        provider.append_operation_journal_entry(&crate::memory::MemoryOperationJournalEntry::new(
            memory_proposal_review_operation(record),
            Some(record.id.clone()),
            Some(record.proposal.task_id.clone()),
            record.proposal.status.label(),
            record.proposal.reason.clone(),
            record.proposal.candidates.len(),
        ))
    }

    fn review_operation_base_dir(&self) -> Option<PathBuf> {
        let parent = self.path.parent()?;
        if self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "memory_proposals.jsonl")
        {
            return Some(parent.to_path_buf());
        }
        let stem = self
            .path
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("memory_proposals");
        Some(parent.join(format!(".{stem}-review")))
    }

    pub fn update_status(
        &self,
        id_or_prefix: &str,
        status: MemoryProposalStatus,
    ) -> anyhow::Result<Option<MemoryProposal>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        proposal.status = status;
        proposal.reason = memory_proposal_status_reason(status).to_string();
        self.upsert(&proposal)?;
        Ok(Some(proposal))
    }

    pub fn update_status_with_reason(
        &self,
        id_or_prefix: &str,
        status: MemoryProposalStatus,
        reason: impl Into<String>,
    ) -> anyhow::Result<Option<MemoryProposal>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        proposal.status = status;
        proposal.reason = reason.into();
        self.upsert(&proposal)?;
        Ok(Some(proposal))
    }

    pub fn batch_update_status(
        &self,
        filter: MemoryProposalBatchFilter,
        status: MemoryProposalStatus,
        reason: impl Into<String>,
    ) -> anyhow::Result<MemoryProposalBatchUpdate> {
        let reason = reason.into();
        let mut result = MemoryProposalBatchUpdate::default();
        for record in self.list_records() {
            if !memory_proposal_record_matches_filter(&record, &filter) {
                continue;
            }
            result.matched += 1;
            if record.proposal.status == status && record.proposal.reason == reason {
                continue;
            }
            let mut proposal = record.proposal;
            proposal.status = status;
            proposal.reason = reason.clone();
            self.upsert(&proposal)?;
            result.updated += 1;
            result.proposal_ids.push(proposal.task_id);
        }
        Ok(result)
    }

    pub fn batch_apply(
        &self,
        filter: MemoryProposalBatchFilter,
        memory: &mut crate::memory::MemoryManager,
    ) -> anyhow::Result<MemoryProposalBatchApply> {
        let mut filter = filter;
        if filter.status.is_none() {
            filter.status = Some(MemoryProposalStatus::Accepted);
        }
        let mut result = MemoryProposalBatchApply::default();
        for record in self.list_records() {
            if !memory_proposal_record_matches_filter(&record, &filter) {
                continue;
            }
            result.matched += 1;
            match self.apply(&record.proposal.task_id, memory) {
                Ok(Some((proposal, applied_candidates))) => {
                    result.applied += 1;
                    result.applied_candidates += applied_candidates;
                    result.proposal_ids.push(proposal.task_id);
                }
                Ok(None) => {
                    result.failed += 1;
                    result
                        .failures
                        .push(format!("{}: not found", record.proposal.task_id));
                }
                Err(error) => {
                    result.failed += 1;
                    result
                        .failures
                        .push(format!("{}: {}", record.proposal.task_id, error));
                }
            }
        }
        Ok(result)
    }

    pub fn supersede(
        &self,
        old_id_or_prefix: &str,
        new_id_or_prefix: &str,
    ) -> anyhow::Result<Option<MemoryProposal>> {
        let Some(new_proposal) = self.get(new_id_or_prefix) else {
            anyhow::bail!(
                "replacement memory proposal '{}' was not found",
                new_id_or_prefix
            );
        };
        self.update_status_with_reason(
            old_id_or_prefix,
            MemoryProposalStatus::Rejected,
            format!("superseded by memory proposal {}", new_proposal.task_id),
        )
    }

    pub fn resolve_conflict_keep(
        &self,
        keep_id_or_prefix: &str,
    ) -> anyhow::Result<Option<MemoryProposalConflictResolution>> {
        let Some(keep_record) = self.get_record(keep_id_or_prefix) else {
            return Ok(None);
        };
        let keep_id = keep_record.id.clone();
        let mut peer_ids = std::collections::BTreeSet::<String>::new();
        for group in &keep_record.conflict_groups {
            for matched in &group.matches {
                if matched.proposal_id != keep_id {
                    peer_ids.insert(matched.proposal_id.clone());
                }
            }
        }

        let mut accepted_keep = false;
        if keep_record.proposal.status != MemoryProposalStatus::Applied
            && keep_record.proposal.status != MemoryProposalStatus::Accepted
        {
            let mut keep = keep_record.proposal.clone();
            keep.status = MemoryProposalStatus::Accepted;
            keep.reason =
                "accepted as the kept memory proposal for duplicate/conflict resolution; apply separately"
                    .to_string();
            self.upsert(&keep)?;
            accepted_keep = true;
        }

        let mut rejected_ids = Vec::new();
        for peer_id in peer_ids {
            let Some(peer) = self.get(&peer_id) else {
                continue;
            };
            if matches!(
                peer.status,
                MemoryProposalStatus::Applied
                    | MemoryProposalStatus::Rejected
                    | MemoryProposalStatus::NotApplicable
            ) {
                continue;
            }
            if let Some(updated) = self.update_status_with_reason(
                &peer_id,
                MemoryProposalStatus::Rejected,
                format!("resolved duplicate/conflict by keeping memory proposal {keep_id}"),
            )? {
                rejected_ids.push(updated.task_id);
            }
        }

        Ok(Some(MemoryProposalConflictResolution {
            kept_id: keep_id,
            accepted_keep,
            rejected_ids,
            conflict_groups: keep_record.conflict_groups.len(),
        }))
    }

    pub fn edit_first_candidate(
        &self,
        id_or_prefix: &str,
        content: impl Into<String>,
    ) -> anyhow::Result<Option<MemoryProposal>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        if proposal.status == MemoryProposalStatus::Applied {
            anyhow::bail!(
                "memory proposal {} is already applied; create a new proposal instead",
                proposal.task_id
            );
        }
        let Some(candidate) = proposal.candidates.first_mut() else {
            anyhow::bail!(
                "memory proposal {} has no editable candidates",
                proposal.task_id
            );
        };
        candidate.content = content.into();
        proposal.status = MemoryProposalStatus::Proposed;
        proposal.write_performed = false;
        proposal.reason =
            "edited candidate content; review and accept again before apply".to_string();
        self.upsert(&proposal)?;
        Ok(Some(proposal))
    }

    pub fn edit_and_apply(
        &self,
        id_or_prefix: &str,
        content: impl Into<String>,
        memory: &mut crate::memory::MemoryManager,
    ) -> anyhow::Result<Option<(MemoryProposal, usize)>> {
        let Some(mut proposal) = self.edit_first_candidate(id_or_prefix, content)? else {
            return Ok(None);
        };
        proposal.status = MemoryProposalStatus::Accepted;
        proposal.reason = "edited candidate content and accepted for memory apply".to_string();
        self.upsert(&proposal)?;
        self.apply(&proposal.task_id, memory)
    }

    pub fn apply(
        &self,
        id_or_prefix: &str,
        memory: &mut crate::memory::MemoryManager,
    ) -> anyhow::Result<Option<(MemoryProposal, usize)>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        if proposal.status != MemoryProposalStatus::Accepted {
            anyhow::bail!(
                "memory proposal {} is {}; accept it before apply",
                proposal.task_id,
                proposal.status.label()
            );
        }
        if let Some(reason) = proposal_blocking_sensitivity_reason(&proposal) {
            anyhow::bail!(
                "memory proposal {} cannot be applied because sensitivity gate blocked it: {}",
                proposal.task_id,
                reason
            );
        }
        if let Some(reason) = proposal_blocking_minimum_evidence_reason(&proposal) {
            anyhow::bail!(
                "memory proposal {} cannot be applied because minimum evidence gate blocked it: {}",
                proposal.task_id,
                reason
            );
        }
        if proposal.source != "repair" {
            if let Some(reason) = self.proposal_blocking_unresolved_conflict_reason(&proposal) {
                anyhow::bail!(
                    "memory proposal {} cannot be applied because conflict review is unresolved: {}",
                    proposal.task_id,
                    reason
                );
            }
        }
        if proposal.source == "repair" {
            let applied = memory.apply_projection_repair_proposal(&proposal)?;
            proposal.status = MemoryProposalStatus::Applied;
            proposal.write_performed = applied > 0;
            proposal.reason = format!("applied {} projection repair(s)", applied);
            self.upsert(&proposal)?;
            return Ok(Some((proposal, applied)));
        }
        let mut applied = 0usize;
        for candidate in &proposal.candidates {
            let mut memory_candidate = memory
                .candidate_from_content(
                    &candidate.content,
                    &candidate.kind,
                    "memory_proposal_review",
                )
                .explicit(true);
            memory_candidate.evidence = memory_proposal_candidate_evidence_refs(
                &proposal.task_id,
                &proposal.source,
                candidate,
            );
            let target = memory_write_target_for_proposal_candidate(candidate);
            let outcome = memory.submit_candidate(memory_candidate, target);
            if matches!(
                outcome.status,
                crate::memory::manager::MemoryWriteOutcomeStatus::Saved
                    | crate::memory::manager::MemoryWriteOutcomeStatus::Duplicate
            ) {
                applied += 1;
            }
        }
        proposal.status = MemoryProposalStatus::Applied;
        proposal.write_performed = applied > 0;
        proposal.reason = format!("applied {} candidate(s) to long-term memory", applied);
        self.upsert(&proposal)?;
        Ok(Some((proposal, applied)))
    }

    fn proposal_blocking_unresolved_conflict_reason(
        &self,
        proposal: &MemoryProposal,
    ) -> Option<String> {
        let record = self.get_record(&proposal.task_id)?;
        let proposal_id = record.id.clone();
        let blockers = record
            .conflict_groups
            .iter()
            .filter(|group| group.group_type == "conflict")
            .flat_map(|group| {
                group
                    .matches
                    .iter()
                    .filter(|matched| {
                        matched.proposal_id != proposal_id
                            && matches!(
                                matched.status,
                                MemoryProposalStatus::Proposed | MemoryProposalStatus::Accepted
                            )
                    })
                    .map(|matched| {
                        format!(
                            "{}#{}:{}:{}={}",
                            matched.proposal_id,
                            matched.candidate_index + 1,
                            matched.status.label(),
                            group.key,
                            compact_text(&matched.value, 80)
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .take(6)
            .collect::<Vec<_>>();
        if blockers.is_empty() {
            return None;
        }
        Some(format!(
            "{}; review with /memory-proposals conflicts, then run /memory-proposals resolve-conflict {} or reject/edit the conflicting proposal",
            blockers.join(", "),
            proposal.task_id
        ))
    }
}

impl Default for MemoryProposalReviewStore {
    fn default() -> Self {
        Self::new(Self::default_path())
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
    if failure_count > 0
        || bundle.agent_state.consecutive_low_action_scores() >= 2
        || bundle.agent_state.mode_score.uncertainty >= 7
    {
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

fn dedupe(items: &mut Vec<String>) {
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

fn compact_text(value: &str, max_chars: usize) -> String {
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

fn proposal_evidence(report: &ExecutionReport) -> Vec<String> {
    let mut evidence = Vec::new();
    for file in &report.changed_files {
        push_unique(&mut evidence, format!("changed_file: {file}"));
    }
    for validation in &report.validation_evidence {
        push_unique(
            &mut evidence,
            format!("validation: {}", compact_text(validation, 220)),
        );
    }
    for risk in &report.risks {
        if risk != "none recorded" {
            push_unique(&mut evidence, format!("risk: {}", compact_text(risk, 180)));
        }
    }
    evidence
}

fn infer_proposal_active_scope(proposal: &MemoryProposal) -> String {
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

fn current_memory_proposal_project_identity() -> (Option<String>, Vec<String>) {
    let scope = crate::memory::MemoryScope::default();
    let identity = scope.identity();
    if identity.kind == crate::memory::types::MemoryScopeKind::Project {
        (Some(identity.id), identity.labels)
    } else {
        (None, identity.labels)
    }
}

fn memory_write_target_for_proposal_candidate(
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

fn proposal_gate_report(
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

fn proposal_blocking_sensitivity_reason(proposal: &MemoryProposal) -> Option<String> {
    proposal_sensitivity_findings(proposal)
        .into_iter()
        .find(|finding| finding.status == "blocked")
        .map(|finding| format!("{}:{}", finding.sensitivity, finding.reason))
}

fn proposal_blocking_minimum_evidence_reason(proposal: &MemoryProposal) -> Option<String> {
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

fn memory_proposal_candidate_evidence_refs(
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

fn memory_proposal_conflict_groups(
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

fn summarize_memory_proposal_conflicts(groups: &[MemoryProposalConflictGroup]) -> String {
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

fn memory_proposal_status_reason(status: MemoryProposalStatus) -> &'static str {
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

fn stable_memory_proposal_id(proposal: &MemoryProposal) -> String {
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

fn memory_proposal_review_operation(record: &MemoryProposalReviewRecord) -> &'static str {
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

fn memory_proposal_record_matches_filter(
    record: &MemoryProposalReviewRecord,
    filter: &MemoryProposalBatchFilter,
) -> bool {
    if let Some(source) = filter.source.as_deref() {
        if record.source != source {
            return false;
        }
    }
    if let Some(scope) = filter.scope.as_deref() {
        let has_scope = record
            .proposal
            .candidates
            .iter()
            .any(|candidate| candidate.scope == scope)
            || record
                .active_scope
                .split(',')
                .any(|item| item.trim() == scope);
        if !has_scope {
            return false;
        }
    }
    if let Some(project) = filter.project.as_deref() {
        if !memory_proposal_record_matches_project(record, project) {
            return false;
        }
    }
    if let Some(status) = filter.status {
        if record.proposal.status != status {
            return false;
        }
    }
    if let Some(days) = filter.stale_days {
        let Ok(created_at) = chrono::DateTime::parse_from_rfc3339(&record.created_at) else {
            return false;
        };
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days.max(0));
        if created_at.with_timezone(&chrono::Utc) > cutoff {
            return false;
        }
    }
    if filter.duplicate_only && !memory_proposal_record_looks_duplicate(record) {
        return false;
    }
    if filter.blocked_only && !memory_proposal_record_is_blocked(record) {
        return false;
    }
    true
}

fn memory_proposal_record_matches_project(
    record: &MemoryProposalReviewRecord,
    project: &str,
) -> bool {
    let needle = project.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }
    record
        .project_id
        .as_deref()
        .map(|id| id.to_ascii_lowercase().contains(&needle))
        .unwrap_or(false)
        || record
            .project_labels
            .iter()
            .any(|label| label.to_ascii_lowercase().contains(&needle))
}

fn memory_proposal_record_is_blocked(record: &MemoryProposalReviewRecord) -> bool {
    record
        .gate_report
        .iter()
        .any(|gate| matches!(gate.status.as_str(), "blocked" | "missing"))
}

fn memory_proposal_record_looks_duplicate(record: &MemoryProposalReviewRecord) -> bool {
    let duplicate_summary = record.duplicate_conflict_summary.to_ascii_lowercase();
    if !duplicate_summary.trim().is_empty()
        && duplicate_summary != "not_checked"
        && (duplicate_summary.contains("duplicate") || duplicate_summary.contains("conflict"))
    {
        return true;
    }
    let reason = record.proposal.reason.to_ascii_lowercase();
    if reason.contains("duplicate") {
        return true;
    }
    record.proposal.candidates.iter().any(|candidate| {
        candidate
            .evidence
            .iter()
            .any(|evidence| evidence.to_ascii_lowercase().contains("duplicate"))
            || candidate
                .content
                .to_ascii_lowercase()
                .contains("duplicate memory")
    })
}

fn format_list(items: &[String]) -> String {
    format_list_limited(items, 8)
}

fn format_list_limited(items: &[String], max: usize) -> String {
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
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::retrieval_context::{RetrievalContext, RetrievalItem};
    use crate::engine::task_context::{
        ActionScoreRecord, AgentTaskStage, StopAction, StopCheckReason, StopCheckRecord,
        StopCheckStatus,
    };
    use serde_json::json;

    #[test]
    fn task_contract_materializes_assumptions_scope_and_validation() {
        let prompt = "修改 src/lib.rs";
        let route = IntentRouter::new().route(prompt);
        let mut bundle = TaskContextBundle::new(prompt, ".", route, None);
        bundle.add_file("src/lib.rs");
        bundle.add_constraint("resource_policy=standard");
        bundle.add_acceptance_check("cargo test -q");

        let contract = bundle.task_contract(&["cargo test -q".to_string()]);

        assert_eq!(contract.task_type, TaskContractType::CodeChange);
        assert_eq!(contract.scope.files_allowed, vec!["src/lib.rs"]);
        assert!(contract.validation.proof_required);
        assert!(contract
            .assumptions
            .iter()
            .any(|item| item.source == AssumptionSource::DefaultPolicy));
        assert_eq!(contract.model_profile, ModelProfileMode::Standard);
        assert!(contract.compact_summary().contains("TaskContract id="));
    }

    #[test]
    fn task_contract_uses_review_required_after_failed_validation() {
        let route = IntentRouter::new().route("修复 src/lib.rs 里的测试失败");
        let mut bundle = TaskContextBundle::new("修复 src/lib.rs 里的测试失败", ".", route, None);
        bundle.agent_state.verification_plan.status = VerificationStatus::Failed;

        let contract = bundle.task_contract(&[]);

        assert_eq!(contract.model_profile, ModelProfileMode::ReviewRequired);
    }

    #[test]
    fn task_contract_uses_constrained_profile_for_low_action_score_loop() {
        let route = IntentRouter::new().route("修改 src/lib.rs");
        let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
        for idx in 0..2 {
            bundle.agent_state.record_action_score(ActionScoreRecord {
                tool: format!("tool-{idx}"),
                stage: "Edit".to_string(),
                action_score: 2,
                value: 2,
                risk: 2,
                uncertainty_reduction: 1,
                cost: 3,
                reversibility: 8,
                scope_fit: 8,
                formula_stage: None,
                formula_version: None,
                review_decision: None,
                reduced_uncertainty: false,
            });
        }

        let contract = bundle.task_contract(&[]);

        assert_eq!(contract.model_profile, ModelProfileMode::Constrained);
    }

    #[test]
    fn context_pack_applies_budgets_and_provenance() {
        let route = IntentRouter::new().route("分析项目");
        let mut retrieval = RetrievalContext::new("分析项目", route.retrieval);
        for idx in 0..12 {
            retrieval.add_item(RetrievalItem::new(
                RetrievalSource::Project,
                format!("project fact {idx}"),
                format!("content {idx}"),
                0.8,
                format!("project.index:{idx}"),
                TrustLevel::High,
            ));
        }
        for idx in 0..7 {
            retrieval.add_item(RetrievalItem::new(
                RetrievalSource::Memory,
                format!("memory fact {idx}"),
                format!("memory {idx}"),
                0.7,
                format!("memory.match:{idx}"),
                TrustLevel::Medium,
            ));
        }
        let mut bundle =
            TaskContextBundle::new("分析项目", ".", route, None).with_retrieval(retrieval);
        for idx in 0..10 {
            bundle
                .agent_state
                .record_observation("test", format!("observation {idx}"));
        }
        bundle.agent_state.record_stop_check(StopCheckRecord {
            status: StopCheckStatus::Stop,
            terminal_status: None,
            action: StopAction::Closeout,
            reason: StopCheckReason::NoProgress,
            summary: "no progress".to_string(),
            evidence: Vec::new(),
            failure_type: Some("no_progress".to_string()),
            recovery_plan_id: None,
            rollback_candidate: None,
            next_action: Some("ask for missing scope".to_string()),
            no_code_progress_rounds: 2,
            action_checkpoint_active: false,
        });

        let contract = bundle.task_contract(&[]);
        let pack = bundle.context_pack(&contract);

        assert_eq!(pack.project_facts.len(), pack.budget.max_project_facts);
        assert_eq!(pack.memory_records.len(), pack.budget.max_memory_records);
        assert_eq!(
            pack.recent_observations.len(),
            pack.budget.max_recent_observations
        );
        assert!(!pack.failure_summaries.is_empty());
        assert!(pack.overflow_items > 0);
        assert_eq!(pack.fingerprint.len(), 12);
        assert!(pack.compact_summary().contains("ContextPack id="));
    }

    #[test]
    fn execution_report_maps_closeout_statuses() {
        let route = IntentRouter::new().route("修改 src/lib.rs");
        let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
        bundle.add_file("src/lib.rs");
        let contract = bundle.task_contract(&["cargo test -q".to_string()]);
        let closeout = WorkflowCloseout {
            status: StageValidationStatus::NotVerified,
            risk: RiskLevel::Medium,
            changed_files: vec!["src/lib.rs".to_string()],
            validation: vec!["verification proof: not_run".to_string()],
            acceptance: Vec::new(),
            residual_risks: vec!["validation was not run".to_string()],
        };

        let report = ExecutionReport::from_closeout(&contract, &closeout);

        assert_eq!(report.status, ExecutionReportStatus::NotVerified);
        assert_eq!(report.changed_files, vec!["src/lib.rs"]);
        assert_eq!(report.next_steps.len(), 1);
        assert!(report.compact_summary().contains("status=not_verified"));
    }

    #[test]
    fn memory_proposal_is_review_only_and_evidence_backed() {
        let report = ExecutionReport {
            task_id: "task-1".to_string(),
            objective: "修改 src/lib.rs".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: vec!["src/lib.rs".to_string()],
            validation_evidence: vec!["cargo test -q: passed".to_string()],
            risks: vec!["none recorded".to_string()],
            next_steps: Vec::new(),
            assumptions: Vec::new(),
        };

        let proposal = MemoryProposal::from_execution_report(&report);

        assert_eq!(proposal.status, MemoryProposalStatus::Proposed);
        assert_eq!(proposal.candidates.len(), 1);
        assert_eq!(proposal.candidates[0].kind, "successful_fix");
        assert!(!proposal.write_performed);
        assert_eq!(proposal.write_policy, "review_required");
        assert!(proposal.evidence_items() >= 2);
        assert!(proposal.compact_summary().contains("write_performed=false"));
    }

    #[test]
    fn memory_proposal_skips_unevidenced_direct_success() {
        let report = ExecutionReport {
            task_id: "task-1".to_string(),
            objective: "回答问题".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: Vec::new(),
            validation_evidence: Vec::new(),
            risks: vec!["none recorded".to_string()],
            next_steps: Vec::new(),
            assumptions: Vec::new(),
        };

        let proposal = MemoryProposal::from_execution_report(&report);

        assert_eq!(proposal.status, MemoryProposalStatus::NotApplicable);
        assert!(proposal.candidates.is_empty());
        assert!(!proposal.write_performed);
    }

    #[test]
    fn memory_proposal_review_store_tracks_status_by_prefix() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-memory-proposals-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(path.clone());
        let report = ExecutionReport {
            task_id: "task-review-123".to_string(),
            objective: "fix parser".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: vec!["src/parser.rs".to_string()],
            validation_evidence: vec!["cargo test parser: passed".to_string()],
            risks: Vec::new(),
            next_steps: Vec::new(),
            assumptions: Vec::new(),
        };
        let proposal = MemoryProposal::from_execution_report(&report);

        store.upsert(&proposal).unwrap();
        let updated = store
            .update_status("task-review", MemoryProposalStatus::Accepted)
            .unwrap()
            .unwrap();

        assert_eq!(updated.status, MemoryProposalStatus::Accepted);
        assert_eq!(store.list().len(), 1);
        assert_eq!(
            store.get("task-review").unwrap().status,
            MemoryProposalStatus::Accepted
        );
        let record = store.get_record("task-review").unwrap();
        assert!(record.id.starts_with("mp-"));
        assert_ne!(record.id, "task-review-123");
        assert_eq!(
            store.get_record(&record.id).unwrap().proposal.task_id,
            "task-review-123"
        );
        assert_eq!(record.source_task, "task-review-123");
        assert_eq!(record.active_scope, "project");
        assert!(record.project_id.is_some());
        assert!(memory_proposal_record_matches_filter(
            &record,
            &MemoryProposalBatchFilter {
                project: record.project_id.clone(),
                ..Default::default()
            }
        ));
        assert!(!memory_proposal_record_matches_filter(
            &record,
            &MemoryProposalBatchFilter {
                project: Some("definitely-not-this-project".to_string()),
                ..Default::default()
            }
        ));
        assert!(record
            .gate_report
            .iter()
            .any(|gate| gate.gate == "write_policy" && gate.status == "passed"));
        assert!(record.status_history.iter().any(|entry| {
            entry.status == MemoryProposalStatus::Proposed
                || entry.status == MemoryProposalStatus::Accepted
        }));
        let edited = store
            .edit_first_candidate("task-review", "Completed parser fix with cargo test parser")
            .unwrap()
            .unwrap();
        assert_eq!(edited.status, MemoryProposalStatus::Proposed);
        assert_eq!(
            edited.candidates[0].content,
            "Completed parser fix with cargo test parser"
        );
        let edited_record = store.get_record("task-review").unwrap();
        assert_eq!(edited_record.id, record.id);
        assert!(edited_record
            .status_history
            .iter()
            .any(|entry| entry.reason.contains("edited candidate content")));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_proposal_review_actions_are_operation_journaled() {
        let base = std::env::temp_dir().join(format!(
            "priority-agent-memory-proposal-review-journal-{}",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
        let proposal = MemoryProposal {
            task_id: "review-journal-proposal".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "User preference: gex explicitly prefers concise Chinese updates."
                    .to_string(),
                evidence: vec!["user_statement: gex prefers concise Chinese updates".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        store.upsert(&proposal).unwrap();
        let record_id = store.get_record("review-journal-proposal").unwrap().id;
        store
            .update_status("review-journal-proposal", MemoryProposalStatus::Accepted)
            .unwrap()
            .unwrap();
        let mut memory = crate::memory::MemoryManager::with_base_dir(base.clone());
        store
            .apply("review-journal-proposal", &mut memory)
            .unwrap()
            .unwrap();

        let journal = crate::memory::LocalMemoryProvider::with_base_dir(&base)
            .operation_journal_entries()
            .unwrap();

        assert!(journal
            .iter()
            .any(|entry| entry.operation == "memory_proposal_create"
                && entry.record_id.as_deref() == Some(record_id.as_str())
                && entry.candidate_id.as_deref() == Some("review-journal-proposal")));
        assert!(journal
            .iter()
            .any(|entry| entry.operation == "memory_proposal_accept"
                && entry.record_id.as_deref() == Some(record_id.as_str())
                && entry.status == "accepted"));
        assert!(journal
            .iter()
            .any(|entry| entry.operation == "memory_proposal_apply"
                && entry.record_id.as_deref() == Some(record_id.as_str())
                && entry.status == "applied"));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn memory_proposal_gate_reports_topic_scope_identity() {
        let explicit_topic = MemoryProposal {
            task_id: "topic-explicit".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "workflow_convention".to_string(),
                scope: "topic:Rust Workflow".to_string(),
                content: "Rust workflow convention: run cargo test before closeout.".to_string(),
                evidence: vec!["source_task: topic-explicit".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "test".to_string(),
        };
        let bare_topic = MemoryProposal {
            task_id: "topic-ambiguous".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "workflow_convention".to_string(),
                scope: "topic".to_string(),
                content: "Workflow convention: run cargo test before closeout.".to_string(),
                evidence: vec!["source_task: topic-ambiguous".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "test".to_string(),
        };

        let explicit_gate = proposal_gate_report(&explicit_topic, &[])
            .into_iter()
            .find(|gate| gate.gate == "scope_identity")
            .expect("scope identity gate");
        let ambiguous_gate = proposal_gate_report(&bare_topic, &[])
            .into_iter()
            .find(|gate| gate.gate == "scope_identity")
            .expect("scope identity gate");
        let ambiguous_candidate_gate = proposal_gate_report(&bare_topic, &[])
            .into_iter()
            .find(|gate| gate.gate == "scope_identity" && gate.candidate_index == Some(0))
            .expect("candidate scope identity gate");

        assert_eq!(explicit_gate.status, "passed");
        assert!(explicit_gate.reason.contains("topic:rust-workflow"));
        assert_eq!(ambiguous_gate.status, "review_required");
        assert!(ambiguous_gate.reason.contains("ambiguous_topic"));
        assert_eq!(ambiguous_candidate_gate.status, "review_required");
        assert!(ambiguous_candidate_gate
            .reason
            .contains("ambiguous_topic:missing_topic_id"));
    }

    #[test]
    fn memory_proposal_gate_reports_kind_specific_evidence_minimums() {
        let explicit_preference = MemoryProposal {
            task_id: "pref-explicit".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "User preference: answer in Chinese.".to_string(),
                evidence: vec!["user: answer in Chinese".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "test".to_string(),
        };
        let inferred_preference = MemoryProposal {
            task_id: "pref-inferred".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "User preference: answer in Chinese.".to_string(),
                evidence: vec!["background: inferred language preference".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "test".to_string(),
        };
        let validation_baseline = MemoryProposal {
            task_id: "validation-baseline".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "validation_baseline".to_string(),
                scope: "project".to_string(),
                content: "Validation baseline: cargo test -q".to_string(),
                evidence: vec![
                    "source_task: validation-baseline".to_string(),
                    "closeout: status=success validation=1".to_string(),
                    "cargo test -q passed".to_string(),
                ],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "test".to_string(),
        };

        let explicit_gate = proposal_gate_report(&explicit_preference, &[])
            .into_iter()
            .find(|gate| gate.gate == "minimum_evidence")
            .expect("minimum evidence gate");
        let inferred_gate = proposal_gate_report(&inferred_preference, &[])
            .into_iter()
            .find(|gate| gate.gate == "minimum_evidence")
            .expect("minimum evidence gate");
        let validation_gate = proposal_gate_report(&validation_baseline, &[])
            .into_iter()
            .find(|gate| gate.gate == "minimum_evidence")
            .expect("minimum evidence gate");
        let inferred_candidate_gate = proposal_gate_report(&inferred_preference, &[])
            .into_iter()
            .find(|gate| gate.gate == "minimum_evidence" && gate.candidate_index == Some(0))
            .expect("candidate minimum evidence gate");

        assert_eq!(explicit_gate.status, "passed");
        assert!(explicit_gate.reason.contains("explicit_user_statement"));
        assert_eq!(inferred_gate.status, "review_required");
        assert!(inferred_gate.reason.contains("explicit_user_statement"));
        assert_eq!(validation_gate.status, "passed");
        assert!(validation_gate.reason.contains("validation_evidence"));
        assert_eq!(inferred_candidate_gate.status, "review_required");
        assert!(inferred_candidate_gate
            .reason
            .contains("explicit_user_statement"));
    }

    #[test]
    fn memory_proposal_gate_reports_sensitivity_and_apply_blocks_secret() {
        let base = std::env::temp_dir().join(format!(
            "priority-agent-sensitive-proposal-{}",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
        let local_path = MemoryProposal {
            task_id: "local-path-proposal".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "project_fact".to_string(),
                scope: "project".to_string(),
                content: "Local path for this project: /Users/gex/src/rust-agent".to_string(),
                evidence: vec!["source_task: local-path-proposal".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "test".to_string(),
        };
        let secret = MemoryProposal {
            task_id: "secret-proposal".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Accepted,
            candidates: vec![MemoryProposalCandidate {
                kind: "project_fact".to_string(),
                scope: "project".to_string(),
                content: "OPENAI_API_KEY=sk-123456789012345678901234".to_string(),
                evidence: vec!["source_task: secret-proposal".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "accepted for test".to_string(),
        };

        let local_gate = proposal_gate_report(&local_path, &[])
            .into_iter()
            .find(|gate| gate.gate == "sensitivity")
            .expect("sensitivity gate");
        let secret_gate = proposal_gate_report(&secret, &[])
            .into_iter()
            .find(|gate| gate.gate == "sensitivity")
            .expect("sensitivity gate");
        store.upsert(&secret).unwrap();
        let secret_candidate_gate = store
            .get_record("secret-proposal")
            .unwrap()
            .gate_report
            .into_iter()
            .find(|gate| gate.gate == "sensitivity" && gate.candidate_index == Some(0))
            .expect("candidate sensitivity gate");
        let secret_record = store.get_record("secret-proposal").unwrap();
        let mut manager = crate::memory::MemoryManager::with_base_dir(base.clone());
        let apply_error = store
            .apply("secret-proposal", &mut manager)
            .expect_err("secret proposal apply should be blocked");

        assert_eq!(local_gate.status, "review_required");
        assert!(local_gate.reason.contains("private_user_data"));
        assert_eq!(secret_gate.status, "blocked");
        assert!(secret_gate.reason.contains("secret_or_credential"));
        assert_eq!(secret_candidate_gate.status, "blocked");
        assert!(secret_candidate_gate
            .reason
            .contains("secret_or_credential"));
        assert!(memory_proposal_record_matches_filter(
            &secret_record,
            &MemoryProposalBatchFilter {
                blocked_only: true,
                ..Default::default()
            }
        ));
        assert!(apply_error.to_string().contains("sensitivity gate blocked"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn accepted_proposal_apply_preserves_evidence_refs_on_memory_record() {
        let base = std::env::temp_dir().join(format!(
            "priority-agent-proposal-evidence-{}",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
        let proposal = MemoryProposal {
            task_id: "evidence-apply".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Accepted,
            candidates: vec![MemoryProposalCandidate {
                kind: "project_fact".to_string(),
                scope: "project".to_string(),
                content: "Project fact: memory proposal apply preserves evidence refs.".to_string(),
                evidence: vec![
                    "source_task: evidence-apply".to_string(),
                    "closeout: status=success validation=1".to_string(),
                    "cargo test -q passed".to_string(),
                ],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "accepted for evidence apply test".to_string(),
        };
        let mut manager = crate::memory::MemoryManager::with_base_dir(base.clone());

        store.upsert(&proposal).unwrap();
        let applied = store
            .apply("evidence-apply", &mut manager)
            .unwrap()
            .unwrap();
        let records = manager.memory_records();

        assert_eq!(applied.1, 1);
        let record = records
            .iter()
            .find(|record| record.content.contains("preserves evidence refs"))
            .expect("applied memory record");
        assert!(record
            .evidence
            .iter()
            .any(|evidence| evidence.source == "memory_proposal:evidence-apply"));
        assert!(record.evidence.iter().any(|evidence| {
            evidence.summary.contains("cargo test -q passed")
                && matches!(evidence.kind, crate::memory::MemoryEvidenceKind::ToolOutput)
        }));
        assert!(record
            .evidence
            .iter()
            .any(|evidence| evidence.summary.contains("closeout:")));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn accepted_proposal_apply_blocks_missing_candidate_evidence() {
        let base = std::env::temp_dir().join(format!(
            "priority-agent-proposal-missing-evidence-{}",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
        let proposal = MemoryProposal {
            task_id: "missing-evidence-apply".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Accepted,
            candidates: vec![MemoryProposalCandidate {
                kind: "project_fact".to_string(),
                scope: "project".to_string(),
                content: "Project fact: this should not apply without evidence.".to_string(),
                evidence: Vec::new(),
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "accepted for missing evidence test".to_string(),
        };
        let mut manager = crate::memory::MemoryManager::with_base_dir(base.clone());

        store.upsert(&proposal).unwrap();
        let error = store
            .apply("missing-evidence-apply", &mut manager)
            .expect_err("missing evidence should block apply");

        assert!(error.to_string().contains("minimum evidence gate blocked"));
        assert!(manager.memory_records().is_empty());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn accepted_topic_scope_proposal_applies_to_named_topic_file() {
        let base = std::env::temp_dir().join(format!(
            "priority-agent-topic-scope-{}",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
        let proposal = MemoryProposal {
            task_id: "topic-apply".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Accepted,
            candidates: vec![MemoryProposalCandidate {
                kind: "workflow_convention".to_string(),
                scope: "topic:Rust Workflow".to_string(),
                content: "Rust workflow convention: run cargo test before closeout.".to_string(),
                evidence: vec!["source_task: topic-apply".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "accepted for apply".to_string(),
        };
        let mut manager = crate::memory::MemoryManager::with_base_dir(base.clone());

        store.upsert(&proposal).unwrap();
        let applied = store.apply("topic-apply", &mut manager).unwrap().unwrap();

        assert_eq!(applied.1, 1);
        let topic_content = std::fs::read_to_string(base.join("memory").join("rust-workflow.md"))
            .unwrap_or_default();
        assert!(topic_content.contains("Rust workflow convention"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn memory_proposal_review_store_batch_updates_by_source_scope_and_status() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-memory-proposals-batch-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(path.clone());
        let mut closeout = MemoryProposal::from_execution_report(&ExecutionReport {
            task_id: "task-batch-closeout".to_string(),
            objective: "fix parser".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: vec!["src/parser.rs".to_string()],
            validation_evidence: vec!["cargo test parser: passed".to_string()],
            risks: Vec::new(),
            next_steps: Vec::new(),
            assumptions: Vec::new(),
        });
        closeout.source = "closeout".to_string();
        let background = MemoryProposal {
            task_id: "task-batch-background".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "next_step".to_string(),
                scope: "project".to_string(),
                content: "Next step: rerun parser eval".to_string(),
                evidence: vec!["closeout: parser eval remains".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        store.upsert(&closeout).unwrap();
        store.upsert(&background).unwrap();

        let result = store
            .batch_update_status(
                MemoryProposalBatchFilter {
                    source: Some("background".to_string()),
                    scope: Some("project".to_string()),
                    status: Some(MemoryProposalStatus::Proposed),
                    ..Default::default()
                },
                MemoryProposalStatus::Accepted,
                "batch accepted for memory apply",
            )
            .unwrap();

        assert_eq!(result.matched, 1);
        assert_eq!(result.updated, 1);
        assert_eq!(
            store.get("task-batch-background").unwrap().status,
            MemoryProposalStatus::Accepted
        );
        assert_eq!(
            store.get("task-batch-closeout").unwrap().status,
            MemoryProposalStatus::Proposed
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_proposal_review_store_batch_applies_accepted_filtered_proposals() {
        let base = std::env::temp_dir().join(format!(
            "priority-agent-memory-proposals-batch-apply-{}",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
        let mut memory = crate::memory::MemoryManager::with_base_dir(base.clone());
        let accepted_user = MemoryProposal {
            task_id: "batch-apply-user".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Accepted,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "User prefers concise Chinese status updates.".to_string(),
                evidence: vec!["user_statement: prefer concise Chinese updates".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "accepted for batch apply".to_string(),
        };
        let accepted_project = MemoryProposal {
            task_id: "batch-apply-project".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Accepted,
            candidates: vec![MemoryProposalCandidate {
                kind: "workflow_convention".to_string(),
                scope: "project".to_string(),
                content: "Project convention: run cargo test before memory closeout.".to_string(),
                evidence: vec![
                    "source_task: batch-apply-project".to_string(),
                    "closeout: validation baseline".to_string(),
                ],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "accepted for batch apply".to_string(),
        };
        let proposed = MemoryProposal {
            task_id: "batch-apply-proposed".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "Pending user preference should not apply yet.".to_string(),
                evidence: vec!["user_statement: pending".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "pending review".to_string(),
        };
        store.upsert(&accepted_user).unwrap();
        store.upsert(&accepted_project).unwrap();
        store.upsert(&proposed).unwrap();

        let result = store
            .batch_apply(
                MemoryProposalBatchFilter {
                    scope: Some("user".to_string()),
                    ..Default::default()
                },
                &mut memory,
            )
            .unwrap();

        assert_eq!(result.matched, 1);
        assert_eq!(result.applied, 1);
        assert_eq!(result.applied_candidates, 1);
        assert_eq!(result.failed, 0);
        assert_eq!(result.proposal_ids, vec!["batch-apply-user".to_string()]);
        assert_eq!(
            store.get("batch-apply-user").unwrap().status,
            MemoryProposalStatus::Applied
        );
        assert_eq!(
            store.get("batch-apply-project").unwrap().status,
            MemoryProposalStatus::Accepted
        );
        assert_eq!(
            store.get("batch-apply-proposed").unwrap().status,
            MemoryProposalStatus::Proposed
        );
        assert!(std::fs::read_to_string(base.join("USER.md"))
            .unwrap_or_default()
            .contains("concise Chinese"));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn memory_proposal_review_store_rejects_stale_duplicate_and_superseded() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-memory-proposals-stale-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(path.clone());
        let stale = MemoryProposal {
            task_id: "task-stale".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "note".to_string(),
                scope: "project".to_string(),
                content: "Old stale memory candidate".to_string(),
                evidence: vec!["background: old".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        let duplicate = MemoryProposal {
            task_id: "task-duplicate".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "note".to_string(),
                scope: "project".to_string(),
                content: "Duplicate memory candidate".to_string(),
                evidence: vec!["duplicate: existing record".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        let replacement = MemoryProposal {
            task_id: "task-replacement".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "note".to_string(),
                scope: "project".to_string(),
                content: "Replacement memory candidate".to_string(),
                evidence: vec!["closeout: newer".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        store.upsert(&stale).unwrap();
        store.upsert(&duplicate).unwrap();
        store.upsert(&replacement).unwrap();
        let mut stale_record = store.get_record("task-stale").unwrap();
        stale_record.created_at = (chrono::Utc::now() - chrono::Duration::days(45)).to_rfc3339();
        let mut duplicate_record = store.get_record("task-duplicate").unwrap();
        duplicate_record.duplicate_conflict_summary = "duplicate existing memory".to_string();
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(file, "{}", serde_json::to_string(&stale_record).unwrap()).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::to_string(&duplicate_record).unwrap()
        )
        .unwrap();

        let stale_result = store
            .batch_update_status(
                MemoryProposalBatchFilter {
                    status: Some(MemoryProposalStatus::Proposed),
                    stale_days: Some(30),
                    ..Default::default()
                },
                MemoryProposalStatus::Rejected,
                "batch rejected as stale proposal",
            )
            .unwrap();
        let duplicate_result = store
            .batch_update_status(
                MemoryProposalBatchFilter {
                    status: Some(MemoryProposalStatus::Proposed),
                    duplicate_only: true,
                    ..Default::default()
                },
                MemoryProposalStatus::Rejected,
                "batch rejected as duplicate/conflicting",
            )
            .unwrap();
        let superseded = store
            .supersede("task-replacement", "task-duplicate")
            .unwrap()
            .unwrap();

        assert_eq!(stale_result.updated, 1);
        assert_eq!(duplicate_result.updated, 1);
        assert_eq!(
            store.get("task-stale").unwrap().status,
            MemoryProposalStatus::Rejected
        );
        assert_eq!(
            store.get("task-duplicate").unwrap().status,
            MemoryProposalStatus::Rejected
        );
        assert_eq!(superseded.status, MemoryProposalStatus::Rejected);
        assert!(superseded.reason.contains("superseded by memory proposal"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_proposal_review_store_groups_duplicate_and_preference_conflicts() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-memory-proposals-conflicts-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(path.clone());
        let chinese = MemoryProposal {
            task_id: "pref-chinese".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "language: Chinese".to_string(),
                evidence: vec!["user: answer in Chinese".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        let english = MemoryProposal {
            task_id: "pref-english".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "language: English".to_string(),
                evidence: vec!["background: inferred language preference".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        let duplicate = MemoryProposal {
            task_id: "pref-chinese-duplicate".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "language: Chinese".to_string(),
                evidence: vec!["background: same preference".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };

        store.upsert(&chinese).unwrap();
        store.upsert(&english).unwrap();
        store.upsert(&duplicate).unwrap();

        let english_record = store.get_record("pref-english").unwrap();
        assert!(english_record
            .conflict_groups
            .iter()
            .any(|group| group.group_type == "conflict"
                && group.key == "language"
                && group.matches.len() == 2));
        assert!(english_record
            .gate_report
            .iter()
            .any(|gate| gate.gate == "duplicate_conflict" && gate.status == "review_required"));

        let duplicate_record = store.get_record("pref-chinese-duplicate").unwrap();
        assert!(duplicate_record
            .conflict_groups
            .iter()
            .any(|group| group.group_type == "duplicate"
                && group.key == "language"
                && group.matches.len() == 2));
        assert!(duplicate_record
            .duplicate_conflict_summary
            .contains("duplicates=1"));
        let duplicate_only = store
            .batch_update_status(
                MemoryProposalBatchFilter {
                    status: Some(MemoryProposalStatus::Proposed),
                    duplicate_only: true,
                    ..Default::default()
                },
                MemoryProposalStatus::Rejected,
                "batch rejected as duplicate/conflicting",
            )
            .unwrap();
        assert_eq!(duplicate_only.updated, 3);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_proposal_review_store_resolves_conflict_by_accepting_keep_and_rejecting_peers() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-memory-proposals-resolve-conflict-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(path.clone());
        let keep = MemoryProposal {
            task_id: "pref-keep".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "language: Chinese".to_string(),
                evidence: vec!["user: answer in Chinese".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        let conflict = MemoryProposal {
            task_id: "pref-conflict".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "language: English".to_string(),
                evidence: vec!["background: inferred language preference".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        let duplicate = MemoryProposal {
            task_id: "pref-duplicate".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "language: Chinese".to_string(),
                evidence: vec!["background: same preference".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        store.upsert(&keep).unwrap();
        store.upsert(&conflict).unwrap();
        store.upsert(&duplicate).unwrap();
        let keep_id = store.get_record("pref-keep").unwrap().id;

        let result = store.resolve_conflict_keep(&keep_id).unwrap().unwrap();

        assert_eq!(result.kept_id, keep_id);
        assert!(result.accepted_keep);
        assert_eq!(result.conflict_groups, 2);
        assert_eq!(result.rejected_ids.len(), 2);
        assert_eq!(
            store.get("pref-keep").unwrap().status,
            MemoryProposalStatus::Accepted
        );
        assert_eq!(
            store.get("pref-conflict").unwrap().status,
            MemoryProposalStatus::Rejected
        );
        assert_eq!(
            store.get("pref-duplicate").unwrap().status,
            MemoryProposalStatus::Rejected
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_proposal_apply_blocks_unresolved_active_conflicts() {
        let base = std::env::temp_dir().join(format!(
            "priority-agent-memory-proposals-apply-conflict-{}",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
        let keep = MemoryProposal {
            task_id: "pref-apply-keep".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Accepted,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "User preference: gex explicitly prefers concise Chinese status updates."
                    .to_string(),
                evidence: vec!["user_statement: gex prefers concise Chinese updates".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "accepted for apply".to_string(),
        };
        let conflict = MemoryProposal {
            task_id: "pref-apply-conflict".to_string(),
            source: "background".to_string(),
            status: MemoryProposalStatus::Accepted,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "User preference: gex explicitly prefers concise English status updates."
                    .to_string(),
                evidence: vec!["user_statement: gex prefers concise English updates".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "accepted for apply".to_string(),
        };
        store.upsert(&keep).unwrap();
        store.upsert(&conflict).unwrap();
        let mut memory = crate::memory::MemoryManager::with_base_dir(base.clone());

        let error = store
            .apply("pref-apply-keep", &mut memory)
            .expect_err("unresolved active conflict should block durable apply");

        assert!(error.to_string().contains("conflict review is unresolved"));
        assert_eq!(
            store.get("pref-apply-keep").unwrap().status,
            MemoryProposalStatus::Accepted
        );
        assert!(std::fs::read_to_string(base.join("USER.md"))
            .unwrap_or_default()
            .is_empty());

        store
            .resolve_conflict_keep("pref-apply-keep")
            .unwrap()
            .unwrap();
        let (applied, candidate_count) = store
            .apply("pref-apply-keep", &mut memory)
            .unwrap()
            .unwrap();

        assert_eq!(candidate_count, 1);
        assert_eq!(applied.status, MemoryProposalStatus::Applied);
        assert_eq!(
            store.get("pref-apply-conflict").unwrap().status,
            MemoryProposalStatus::Rejected
        );
        assert!(std::fs::read_to_string(base.join("USER.md"))
            .unwrap_or_default()
            .contains("concise Chinese status updates"));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn memory_proposal_review_store_edit_and_apply_records_review_history() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-memory-proposals-edit-apply-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let base = std::env::temp_dir().join(format!(
            "priority-agent-memory-edit-apply-{}",
            uuid::Uuid::new_v4()
        ));
        let store = MemoryProposalReviewStore::new(path.clone());
        let proposal = MemoryProposal {
            task_id: "edit-apply-proposal".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "decision".to_string(),
                scope: "project".to_string(),
                content: "project_decision: old wording".to_string(),
                evidence: vec!["review: user requested edit".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        store.upsert(&proposal).unwrap();
        let mut memory = crate::memory::MemoryManager::with_base_dir(base.clone());

        let (applied_proposal, applied) = store
            .edit_and_apply(
                "edit-apply-proposal",
                "project_decision: edited wording",
                &mut memory,
            )
            .unwrap()
            .unwrap();

        assert_eq!(applied, 1);
        assert_eq!(applied_proposal.status, MemoryProposalStatus::Applied);
        assert!(applied_proposal.write_performed);
        let record = store.get_record("edit-apply-proposal").unwrap();
        assert_eq!(record.proposal.status, MemoryProposalStatus::Applied);
        assert!(record
            .status_history
            .iter()
            .any(|entry| entry.status == MemoryProposalStatus::Accepted
                && entry.reason.contains("edited candidate content")));
        assert!(memory
            .memory_records()
            .iter()
            .any(|record| record.content.contains("edited wording")));

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn background_memory_review_output_requires_strict_schema() {
        let valid = r#"{
            "candidates": [{
                "kind": "next_step",
                "scope": "project",
                "content": "Next step: run focused parser tests",
                "evidence": ["closeout: parser work remains"]
            }],
            "no_op_reason": null,
            "rejected_observations": []
        }"#;

        let output = BackgroundMemoryReviewOutput::strict_from_json(valid).unwrap();

        assert_eq!(output.candidates.len(), 1);
        assert!(BackgroundMemoryReviewOutput::strict_from_json("not json").is_err());
        assert!(BackgroundMemoryReviewOutput::strict_from_json(
            r#"{"candidates":[],"no_op_reason":null,"rejected_observations":[]}"#
        )
        .is_err());
    }

    #[test]
    fn background_memory_review_worker_creates_proposal_only_candidates() {
        let report = ExecutionReport {
            task_id: "task-background-review".to_string(),
            objective: "finish parser repair".to_string(),
            status: ExecutionReportStatus::Partial,
            changed_files: vec!["src/parser.rs".to_string()],
            validation_evidence: vec!["cargo test parser: failed on edge case".to_string()],
            risks: vec!["edge case remains unresolved".to_string()],
            next_steps: vec!["repair parser edge case".to_string()],
            assumptions: Vec::new(),
        };
        let packet = BackgroundReviewPacket::from_execution_report(&report, &[]);

        let output = BackgroundMemoryReviewWorker::review_execution_report(&packet, &report);
        let proposal = BackgroundMemoryReviewWorker::proposal_from_output(&packet, output);

        assert_eq!(proposal.source, "background");
        assert_eq!(proposal.status, MemoryProposalStatus::Proposed);
        assert_eq!(proposal.write_policy, "review_required");
        assert!(!proposal.write_performed);
        assert!(proposal
            .candidates
            .iter()
            .any(|candidate| candidate.kind == "next_step"));
        assert!(proposal
            .candidates
            .iter()
            .all(|candidate| !candidate.evidence.is_empty()));
    }

    #[test]
    fn serialized_contract_uses_documented_field_names() {
        let route = IntentRouter::new().route("分析项目");
        let bundle = TaskContextBundle::new("分析项目", ".", route, None);
        let contract = bundle.task_contract(&[]);

        let value = serde_json::to_value(&contract).expect("json");

        assert!(value.get("task_type").is_some());
        assert_eq!(value["model_profile"], json!("standard"));
        assert!(value["assumptions"][0].get("source").is_some());
    }

    #[test]
    fn context_pack_stage_tracks_agent_state() {
        let route = IntentRouter::new().route("修改 src/lib.rs");
        let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
        bundle.agent_state.set_stage(AgentTaskStage::Validate);
        let contract = bundle.task_contract(&[]);

        let pack = bundle.context_pack(&contract);

        assert_eq!(pack.current_stage, "Validate");
    }
}
