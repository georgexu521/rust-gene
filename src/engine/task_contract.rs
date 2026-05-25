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
