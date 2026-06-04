//! Task context bundle for non-trivial coding and agent workflows.
//!
//! A bundle captures the active goal, route, retrieved evidence, constraints,
//! risks, budgets, and acceptance checks that should travel with a task.

use crate::engine::context_ledger::{tool_context_evidence_entries, ContextLedgerEntry};
use crate::engine::intent_router::IntentRoute;
use crate::engine::lightweight_planner::{LightweightPlan, LightweightPlanner};
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::session_goal::SessionGoal;
use crate::engine::task_mode_score::TaskModeScore;
use crate::engine::workflow_contract::ProgrammingWorkflowJudgment;
use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MAX_COMPLETED_STEPS: usize = 12;
const MAX_OBSERVATIONS: usize = 12;
const MAX_EDIT_SNAPSHOTS: usize = 6;
const MAX_KEY_FINDINGS: usize = 12;
const MAX_HYPOTHESES: usize = 8;
const MAX_CANDIDATE_FOCUS: usize = 12;
const MAX_ROLLBACK_CANDIDATES: usize = 4;
const MAX_FAILED_STRATEGIES: usize = 8;
const MAX_ACTION_SCORE_HISTORY: usize = 12;
const MAX_STAGE_TRANSITIONS: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContextBundle {
    pub task_id: String,
    pub prompt_preview: String,
    pub working_dir: PathBuf,
    pub goal: Option<SessionGoal>,
    pub agent_state: AgentTaskState,
    pub route: IntentRoute,
    pub relevant_files: Vec<PathBuf>,
    pub constraints: Vec<String>,
    pub retrieval: Option<RetrievalContext>,
    pub risks: Vec<String>,
    pub tool_budget: TaskToolBudget,
    pub acceptance_checks: Vec<String>,
    pub workflow_judgment: Option<ProgrammingWorkflowJudgment>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TaskToolBudget {
    pub max_tool_calls: usize,
    pub max_seconds: u64,
    pub max_parallel: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskMode {
    Direct,
    Light,
    Full,
    HighRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStage {
    Understand,
    Plan,
    Edit,
    Validate,
    Repair,
    Closeout,
    Done,
}

impl AgentTaskStage {
    pub fn mva_stage_label(self) -> &'static str {
        match self {
            Self::Understand | Self::Plan => "diagnosis",
            Self::Edit | Self::Repair => "implementation",
            Self::Validate => "verification",
            Self::Closeout | Self::Done => "finalization",
        }
    }
}

pub fn mva_stage_transition_policy(from: AgentTaskStage, to: AgentTaskStage) -> &'static str {
    use AgentTaskStage::*;
    if from == to {
        return "no_change";
    }
    match (from, to) {
        (Understand, Plan)
        | (Understand, Edit)
        | (Understand, Validate)
        | (Plan, Edit)
        | (Plan, Validate)
        | (Edit, Validate)
        | (Validate, Closeout)
        | (Closeout, Done) => "expected",
        (Validate, Repair)
        | (Validate, Understand)
        | (Repair, Understand)
        | (Repair, Edit)
        | (Repair, Validate)
        | (Edit, Repair) => "repair_fallback",
        (_, Closeout) | (_, Done) => "terminal",
        _ => "unexpected",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    NotRequired,
    Pending,
    Verified,
    Failed,
    Blocked,
    UserDeferred,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopCheckStatus {
    Continue,
    Checkpoint,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskTerminalStatus {
    Completed,
    Partial,
    Blocked,
    Failed,
    NeedsUser,
    RolledBack,
    StoppedByUser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StopAction {
    #[default]
    Continue,
    Closeout,
    AskUser,
    Replan,
    Recover,
    RecommendRollback,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopCheckReason {
    NoIssue,
    NoProgress,
    FocusedRepairStalled,
    RepeatedToolFailure,
    DuplicateReadOnly,
    VerificationReady,
    BudgetExhausted,
    UncertaintyNotReduced,
    ConsecutiveValidationFailures,
    ConsecutiveEditFailures,
    ConsecutiveCommandFailures,
    ConsecutivePermissionBlocks,
    HighRiskNeedsUser,
    ActionDenied,
    ActionNeedsRevision,
    RollbackRecommended,
    UserInterrupted,
    ModelOutputInvalid,
    LowActionValueLoop,
    ScoreNotReducingUncertainty,
    RepeatedActionRevision,
}

impl StopCheckStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::Checkpoint => "checkpoint",
            Self::Stop => "stop",
        }
    }
}

impl StopCheckReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoIssue => "no_issue",
            Self::NoProgress => "no_progress",
            Self::FocusedRepairStalled => "focused_repair_stalled",
            Self::RepeatedToolFailure => "repeated_tool_failure",
            Self::DuplicateReadOnly => "duplicate_read_only",
            Self::VerificationReady => "verification_ready",
            Self::BudgetExhausted => "budget_exhausted",
            Self::UncertaintyNotReduced => "uncertainty_not_reduced",
            Self::ConsecutiveValidationFailures => "consecutive_validation_failures",
            Self::ConsecutiveEditFailures => "consecutive_edit_failures",
            Self::ConsecutiveCommandFailures => "consecutive_command_failures",
            Self::ConsecutivePermissionBlocks => "consecutive_permission_blocks",
            Self::HighRiskNeedsUser => "high_risk_needs_user",
            Self::ActionDenied => "action_denied",
            Self::ActionNeedsRevision => "action_needs_revision",
            Self::RollbackRecommended => "rollback_recommended",
            Self::UserInterrupted => "user_interrupted",
            Self::ModelOutputInvalid => "model_output_invalid",
            Self::LowActionValueLoop => "low_action_value_loop",
            Self::ScoreNotReducingUncertainty => "score_not_reducing_uncertainty",
            Self::RepeatedActionRevision => "repeated_action_revision",
        }
    }
}

impl TaskTerminalStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Partial => "partial",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::NeedsUser => "needs_user",
            Self::RolledBack => "rolled_back",
            Self::StoppedByUser => "stopped_by_user",
        }
    }
}

impl StopAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::Closeout => "closeout",
            Self::AskUser => "ask_user",
            Self::Replan => "replan",
            Self::Recover => "recover",
            Self::RecommendRollback => "recommend_rollback",
            Self::Stop => "stop",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservationSummary {
    pub source: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskFinding {
    pub source: String,
    pub summary: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskHypothesis {
    pub hypothesis: String,
    pub confidence: u8,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskFocus {
    pub target: String,
    pub reason: String,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskStageTransition {
    pub from: AgentTaskStage,
    pub to: AgentTaskStage,
    #[serde(default)]
    pub mva_from: String,
    #[serde(default)]
    pub mva_to: String,
    #[serde(default)]
    pub policy: String,
    pub source: String,
    pub reason: String,
    pub evidence_items: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MvaStateSnapshot {
    pub goal: String,
    pub mode: AgentTaskMode,
    pub mva_stage: String,
    pub internal_stage: AgentTaskStage,
    pub recent_step: Option<String>,
    pub recent_observation: Option<String>,
    pub relevant_files: Vec<String>,
    pub failure_count: usize,
    pub max_tool_calls: usize,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RollbackCandidate {
    pub checkpoint_id: Option<String>,
    pub file_change_id: Option<String>,
    pub tool_round_id: Option<String>,
    pub paths: Vec<String>,
    pub reason: String,
    pub confidence: u8,
    pub auto_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailedStrategyRecord {
    pub failed_strategy: String,
    pub reason: String,
    pub better_strategy: String,
    pub recovery_plan_id: Option<String>,
    pub rollback_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletedStep {
    pub stage: AgentTaskStage,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditStateSnapshot {
    pub label: String,
    pub stage: AgentTaskStage,
    pub active_files: Vec<PathBuf>,
    pub verification_status: VerificationStatus,
    pub recent_step: Option<String>,
    pub recent_observation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationPlan {
    pub required_checks: Vec<String>,
    pub status: VerificationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneCondition {
    pub summary: String,
    pub satisfied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopCheckRecord {
    pub status: StopCheckStatus,
    #[serde(default)]
    pub terminal_status: Option<TaskTerminalStatus>,
    #[serde(default)]
    pub action: StopAction,
    pub reason: StopCheckReason,
    pub summary: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub failure_type: Option<String>,
    #[serde(default)]
    pub recovery_plan_id: Option<String>,
    #[serde(default)]
    pub rollback_candidate: Option<RollbackCandidate>,
    #[serde(default)]
    pub next_action: Option<String>,
    pub no_code_progress_rounds: usize,
    pub action_checkpoint_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionScoreRecord {
    pub tool: String,
    pub stage: String,
    pub action_score: i16,
    pub value: u8,
    pub risk: u8,
    pub uncertainty_reduction: u8,
    pub cost: u8,
    pub reversibility: u8,
    pub scope_fit: u8,
    #[serde(default)]
    pub formula_stage: Option<String>,
    #[serde(default)]
    pub formula_version: Option<String>,
    #[serde(default)]
    pub review_decision: Option<String>,
    pub reduced_uncertainty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskState {
    pub main_goal: String,
    pub mode: AgentTaskMode,
    #[serde(default)]
    pub mode_score: TaskModeScore,
    #[serde(default)]
    pub lightweight_plan: Option<LightweightPlan>,
    pub stage: AgentTaskStage,
    #[serde(default)]
    pub terminal_status: Option<TaskTerminalStatus>,
    pub allowed_scope: Vec<String>,
    pub forbidden_actions: Vec<String>,
    pub completed_steps: Vec<CompletedStep>,
    pub observations: Vec<ObservationSummary>,
    #[serde(default)]
    pub key_findings: Vec<TaskFinding>,
    #[serde(default)]
    pub hypotheses: Vec<TaskHypothesis>,
    #[serde(default)]
    pub candidate_focus: Vec<TaskFocus>,
    #[serde(default)]
    pub edit_snapshots: Vec<EditStateSnapshot>,
    pub active_files: Vec<PathBuf>,
    pub risks: Vec<String>,
    pub verification_plan: VerificationPlan,
    pub done_condition: DoneCondition,
    #[serde(default)]
    pub stop_checks: Vec<StopCheckRecord>,
    #[serde(default)]
    pub uncertainty_not_reduced_steps: usize,
    #[serde(default)]
    pub consecutive_validation_failures: usize,
    #[serde(default)]
    pub consecutive_edit_failures: usize,
    #[serde(default)]
    pub consecutive_command_failures: usize,
    #[serde(default)]
    pub consecutive_permission_blocks: usize,
    #[serde(default)]
    pub last_failure_family: Option<String>,
    #[serde(default)]
    pub last_progress_signal: Option<String>,
    #[serde(default)]
    pub rollback_candidates: Vec<RollbackCandidate>,
    #[serde(default)]
    pub failed_strategies: Vec<FailedStrategyRecord>,
    #[serde(default)]
    pub action_score_history: Vec<ActionScoreRecord>,
    #[serde(default)]
    pub stage_transitions: Vec<TaskStageTransition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentToolRoundObservation {
    pub any_tool_success: bool,
    pub batch_has_unsuccessful_tools: bool,
    pub used_write_tool: bool,
    pub successful_write_tool: bool,
    pub has_worktree_changes: bool,
    pub has_successful_validation_commands: bool,
    pub failed_tool_evidence_present: bool,
}

impl Default for TaskToolBudget {
    fn default() -> Self {
        Self {
            max_tool_calls: 25,
            max_seconds: 600,
            max_parallel: 4,
        }
    }
}

mod state;

impl AgentTaskStage {
    fn initial_for(route: &IntentRoute) -> Self {
        match route.workflow {
            crate::engine::intent_router::WorkflowKind::Direct => Self::Understand,
            _ => Self::Understand,
        }
    }
}

impl VerificationStatus {
    fn initial_for(route: &IntentRoute) -> Self {
        match route.workflow {
            crate::engine::intent_router::WorkflowKind::Direct => Self::NotRequired,
            _ => Self::Pending,
        }
    }
}

impl TaskContextBundle {
    pub fn new(
        prompt: &str,
        working_dir: impl AsRef<Path>,
        route: IntentRoute,
        goal: Option<SessionGoal>,
    ) -> Self {
        let working_dir = working_dir.as_ref().to_path_buf();
        let agent_state =
            AgentTaskState::from_initial_context(prompt, &working_dir, &route, goal.as_ref());
        Self {
            task_id: uuid::Uuid::new_v4().to_string(),
            prompt_preview: preview(prompt, 160),
            working_dir,
            goal,
            agent_state,
            route,
            relevant_files: Vec::new(),
            constraints: Vec::new(),
            retrieval: None,
            risks: Vec::new(),
            tool_budget: TaskToolBudget::default(),
            acceptance_checks: Vec::new(),
            workflow_judgment: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_retrieval(mut self, retrieval: RetrievalContext) -> Self {
        self.retrieval = Some(retrieval);
        self
    }

    pub fn add_file(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        if !self.relevant_files.contains(&path) {
            self.agent_state.add_active_file(path.clone());
            self.relevant_files.push(path);
        }
    }

    pub fn add_constraint(&mut self, constraint: impl Into<String>) {
        push_unique(&mut self.constraints, constraint.into());
    }

    pub fn add_risk(&mut self, risk: impl Into<String>) {
        let risk = risk.into();
        self.agent_state.add_risk(risk.clone());
        push_unique(&mut self.risks, risk);
    }

    pub fn add_acceptance_check(&mut self, check: impl Into<String>) {
        let check = check.into();
        if matches!(
            self.route.workflow,
            crate::engine::intent_router::WorkflowKind::CodeChange
                | crate::engine::intent_router::WorkflowKind::BugFix
        ) {
            self.agent_state.add_required_check(check.clone());
        }
        push_unique(&mut self.acceptance_checks, check);
    }

    pub fn apply_workflow_judgment(&mut self, judgment: ProgrammingWorkflowJudgment) {
        for assumption in &judgment.assumptions {
            self.add_constraint(format!("assumption: {}", assumption));
        }
        for risk in judgment.risk_notes() {
            self.add_risk(risk);
        }
        for check in judgment.acceptance_checks() {
            self.add_acceptance_check(check);
        }
        self.workflow_judgment = Some(judgment);
    }

    pub fn needs_stronger_acceptance(&self) -> bool {
        matches!(
            self.route.workflow,
            crate::engine::intent_router::WorkflowKind::CodeChange
                | crate::engine::intent_router::WorkflowKind::BugFix
        ) && self.acceptance_checks.is_empty()
    }

    pub fn mva_state_snapshot(&self) -> MvaStateSnapshot {
        let failure_count = self.agent_state.uncertainty_not_reduced_steps
            + self.agent_state.consecutive_validation_failures
            + self.agent_state.consecutive_edit_failures
            + self.agent_state.consecutive_command_failures
            + self.agent_state.consecutive_permission_blocks;
        MvaStateSnapshot {
            goal: self
                .goal
                .as_ref()
                .map(|goal| goal.title.clone())
                .unwrap_or_else(|| self.prompt_preview.clone()),
            mode: self.agent_state.mode,
            mva_stage: self.agent_state.stage.mva_stage_label().to_string(),
            internal_stage: self.agent_state.stage,
            recent_step: self
                .agent_state
                .completed_steps
                .last()
                .map(|step| step.summary.clone()),
            recent_observation: self
                .agent_state
                .observations
                .last()
                .map(|observation| observation.summary.clone()),
            relevant_files: self
                .relevant_files
                .iter()
                .chain(self.agent_state.active_files.iter())
                .map(|path| path.display().to_string())
                .fold(Vec::new(), |mut acc, path| {
                    if !acc.contains(&path) {
                        acc.push(path);
                    }
                    acc
                }),
            failure_count,
            max_tool_calls: self.tool_budget.max_tool_calls,
            done: self.agent_state.done_condition.satisfied,
        }
    }
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !items.contains(&value) {
        items.push(value);
    }
}

fn trim_front<T>(items: &mut Vec<T>, max: usize) {
    if items.len() > max {
        let overflow = items.len() - max;
        items.drain(0..overflow);
    }
}

fn display_evidence_paths(paths: &[String], resolved_paths: &[String]) -> String {
    let values = if paths.is_empty() {
        resolved_paths
    } else {
        paths
    };
    if values.is_empty() {
        return "unknown path".to_string();
    }
    let mut rendered = values
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if values.len() > 3 {
        rendered.push_str(&format!(", +{} more", values.len() - 3));
    }
    rendered
}

fn preview(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn default_forbidden_actions(route: &IntentRoute) -> Vec<String> {
    let mut actions = vec!["destructive work outside the requested scope".to_string()];
    if route.workflow == crate::engine::intent_router::WorkflowKind::Direct {
        actions.push("local mutation without an explicit user request".to_string());
    }
    actions
}

fn done_condition_for(route: &IntentRoute) -> String {
    match route.workflow {
        crate::engine::intent_router::WorkflowKind::Direct => {
            "answer the current user request concisely".to_string()
        }
        crate::engine::intent_router::WorkflowKind::CodeChange
        | crate::engine::intent_router::WorkflowKind::BugFix => {
            "requested code change is implemented and verification evidence is recorded".to_string()
        }
        crate::engine::intent_router::WorkflowKind::Research => {
            "requested evidence has been inspected and summarized".to_string()
        }
        crate::engine::intent_router::WorkflowKind::Planning => {
            "plan or design recommendation is complete enough for the next action".to_string()
        }
        crate::engine::intent_router::WorkflowKind::Delegation => {
            "delegated work is assigned or summarized with status".to_string()
        }
    }
}

#[cfg(test)]
mod tests;
