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

impl AgentTaskState {
    pub fn from_initial_context(
        prompt: &str,
        working_dir: &Path,
        route: &IntentRoute,
        goal: Option<&SessionGoal>,
    ) -> Self {
        let mode_score = TaskModeScore::from_route(prompt, route);
        let lightweight_plan = LightweightPlanner::plan(prompt, route, &mode_score);
        let main_goal = goal
            .map(|goal| goal.title.clone())
            .unwrap_or_else(|| preview(prompt, 160));
        let mut allowed_scope = vec![format!("working_dir: {}", working_dir.display())];
        if let Some(goal) = goal {
            allowed_scope.push(format!("goal: {}", goal.title));
        }

        Self {
            main_goal: if main_goal.trim().is_empty() {
                "current user request".to_string()
            } else {
                main_goal
            },
            mode: mode_score.mode,
            mode_score,
            lightweight_plan,
            stage: AgentTaskStage::initial_for(route),
            allowed_scope,
            forbidden_actions: default_forbidden_actions(route),
            completed_steps: Vec::new(),
            observations: Vec::new(),
            key_findings: Vec::new(),
            hypotheses: Vec::new(),
            candidate_focus: Vec::new(),
            edit_snapshots: Vec::new(),
            active_files: Vec::new(),
            risks: Vec::new(),
            verification_plan: VerificationPlan {
                required_checks: Vec::new(),
                status: VerificationStatus::initial_for(route),
            },
            done_condition: DoneCondition {
                summary: done_condition_for(route),
                satisfied: false,
            },
            stop_checks: Vec::new(),
            terminal_status: None,
            uncertainty_not_reduced_steps: 0,
            consecutive_validation_failures: 0,
            consecutive_edit_failures: 0,
            consecutive_command_failures: 0,
            consecutive_permission_blocks: 0,
            last_failure_family: None,
            last_progress_signal: None,
            rollback_candidates: Vec::new(),
            failed_strategies: Vec::new(),
            action_score_history: Vec::new(),
            stage_transitions: Vec::new(),
        }
    }

    pub fn add_active_file(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        if !self.active_files.contains(&path) {
            self.active_files.push(path);
        }
    }

    pub fn add_risk(&mut self, risk: impl Into<String>) {
        push_unique(&mut self.risks, risk.into());
    }

    pub fn add_required_check(&mut self, check: impl Into<String>) {
        push_unique(&mut self.verification_plan.required_checks, check.into());
        if self.verification_plan.status == VerificationStatus::NotRequired {
            self.verification_plan.status = VerificationStatus::Pending;
        }
    }

    pub fn record_observation(&mut self, source: impl Into<String>, summary: impl Into<String>) {
        let source = source.into();
        let summary = summary.into();
        if summary.trim().is_empty() {
            return;
        }
        if self
            .observations
            .iter()
            .any(|item| item.source == source && item.summary == summary)
        {
            return;
        }
        self.observations
            .push(ObservationSummary { source, summary });
        trim_front(&mut self.observations, MAX_OBSERVATIONS);
    }

    pub fn record_key_finding(
        &mut self,
        source: impl Into<String>,
        summary: impl Into<String>,
        evidence: Vec<String>,
    ) {
        let source = source.into();
        let summary = summary.into();
        if summary.trim().is_empty() {
            return;
        }
        if self
            .key_findings
            .iter()
            .any(|item| item.source == source && item.summary == summary)
        {
            return;
        }
        self.key_findings.push(TaskFinding {
            source,
            summary,
            evidence: evidence.into_iter().take(3).collect(),
        });
        trim_front(&mut self.key_findings, MAX_KEY_FINDINGS);
    }

    pub fn record_hypothesis(
        &mut self,
        hypothesis: impl Into<String>,
        confidence: u8,
        evidence: Vec<String>,
    ) {
        let hypothesis = hypothesis.into();
        if hypothesis.trim().is_empty() {
            return;
        }
        if let Some(existing) = self
            .hypotheses
            .iter_mut()
            .find(|item| item.hypothesis == hypothesis)
        {
            existing.confidence = existing.confidence.max(confidence.min(100));
            for item in evidence.into_iter().take(3) {
                push_unique(&mut existing.evidence, item);
            }
            trim_front(&mut existing.evidence, 5);
            return;
        }
        self.hypotheses.push(TaskHypothesis {
            hypothesis,
            confidence: confidence.min(100),
            evidence: evidence.into_iter().take(3).collect(),
        });
        trim_front(&mut self.hypotheses, MAX_HYPOTHESES);
    }

    pub fn record_candidate_focus(
        &mut self,
        target: impl Into<String>,
        reason: impl Into<String>,
        confidence: u8,
    ) {
        let target = target.into();
        let reason = reason.into();
        if target.trim().is_empty() {
            return;
        }
        if let Some(existing) = self
            .candidate_focus
            .iter_mut()
            .find(|item| item.target == target)
        {
            existing.confidence = existing.confidence.max(confidence.min(100));
            if !reason.trim().is_empty() {
                existing.reason = reason;
            }
            return;
        }
        self.candidate_focus.push(TaskFocus {
            target,
            reason,
            confidence: confidence.min(100),
        });
        trim_front(&mut self.candidate_focus, MAX_CANDIDATE_FOCUS);
    }

    pub fn record_completed_step(&mut self, stage: AgentTaskStage, summary: impl Into<String>) {
        let summary = summary.into();
        if summary.trim().is_empty() {
            return;
        }
        if self
            .completed_steps
            .iter()
            .any(|item| item.stage == stage && item.summary == summary)
        {
            return;
        }
        self.completed_steps.push(CompletedStep { stage, summary });
        trim_front(&mut self.completed_steps, MAX_COMPLETED_STEPS);
    }

    pub fn record_edit_snapshot(&mut self, label: impl Into<String>) {
        let label = label.into();
        if label.trim().is_empty() {
            return;
        }
        let snapshot = EditStateSnapshot {
            label,
            stage: self.stage,
            active_files: self.active_files.clone(),
            verification_status: self.verification_plan.status,
            recent_step: self.completed_steps.last().map(|step| step.summary.clone()),
            recent_observation: self
                .observations
                .last()
                .map(|observation| observation.summary.clone()),
        };
        if self.edit_snapshots.last() == Some(&snapshot) {
            return;
        }
        self.edit_snapshots.push(snapshot);
        trim_front(&mut self.edit_snapshots, MAX_EDIT_SNAPSHOTS);
    }

    pub fn observe_tool_context_evidence(
        &mut self,
        tool_call: &ToolCall,
        result: &ToolResult,
    ) -> usize {
        let mut observed = 0;
        for entry in tool_context_evidence_entries(tool_call, result) {
            self.observe_context_ledger_entry(entry);
            observed += 1;
        }
        observed
    }

    pub fn set_stage(&mut self, stage: AgentTaskStage) {
        self.transition_to_stage(stage, "manual", "stage set by runtime caller", 0);
    }

    pub fn mark_done(&mut self, summary: impl Into<String>) {
        self.transition_to_stage(
            AgentTaskStage::Done,
            "done_condition",
            "task marked done",
            1,
        );
        self.done_condition.summary = summary.into();
        self.done_condition.satisfied = true;
    }

    pub fn transition_to_stage(
        &mut self,
        next: AgentTaskStage,
        source: impl Into<String>,
        reason: impl Into<String>,
        evidence_items: usize,
    ) {
        if self.stage == next {
            return;
        }
        let previous = self.stage;
        self.stage = next;
        self.stage_transitions.push(TaskStageTransition {
            from: previous,
            to: next,
            mva_from: previous.mva_stage_label().to_string(),
            mva_to: next.mva_stage_label().to_string(),
            policy: mva_stage_transition_policy(previous, next).to_string(),
            source: source.into(),
            reason: reason.into(),
            evidence_items,
        });
        trim_front(&mut self.stage_transitions, MAX_STAGE_TRANSITIONS);
    }

    pub fn record_stop_check(&mut self, record: StopCheckRecord) {
        if let Some(status) = record.terminal_status {
            self.terminal_status = Some(status);
        }
        if let Some(failure_type) = &record.failure_type {
            self.last_failure_family = Some(failure_type.clone());
        }
        if record.action == StopAction::RecommendRollback {
            if let Some(candidate) = &record.rollback_candidate {
                self.record_rollback_candidate(candidate.clone());
            }
        }
        if matches!(
            record.status,
            StopCheckStatus::Checkpoint | StopCheckStatus::Stop
        ) && matches!(
            record.reason,
            StopCheckReason::NoProgress
                | StopCheckReason::FocusedRepairStalled
                | StopCheckReason::RepeatedToolFailure
                | StopCheckReason::ConsecutiveValidationFailures
                | StopCheckReason::ConsecutiveEditFailures
                | StopCheckReason::ConsecutiveCommandFailures
                | StopCheckReason::ConsecutivePermissionBlocks
                | StopCheckReason::UncertaintyNotReduced
                | StopCheckReason::ModelOutputInvalid
                | StopCheckReason::LowActionValueLoop
                | StopCheckReason::ScoreNotReducingUncertainty
                | StopCheckReason::RepeatedActionRevision
        ) {
            self.record_failed_strategy(FailedStrategyRecord {
                failed_strategy: record.reason.label().to_string(),
                reason: record.summary.clone(),
                better_strategy: record
                    .next_action
                    .clone()
                    .unwrap_or_else(|| record.action.label().to_string()),
                recovery_plan_id: record.recovery_plan_id.clone(),
                rollback_status: record.rollback_candidate.as_ref().map(|candidate| {
                    if candidate.auto_allowed {
                        "candidate_auto_allowed".to_string()
                    } else {
                        "candidate_requires_review".to_string()
                    }
                }),
            });
        }
        self.stop_checks.push(record);
        const MAX_STOP_CHECKS: usize = 8;
        if self.stop_checks.len() > MAX_STOP_CHECKS {
            let overflow = self.stop_checks.len() - MAX_STOP_CHECKS;
            self.stop_checks.drain(0..overflow);
        }
    }

    pub fn record_action_score(&mut self, record: ActionScoreRecord) {
        self.action_score_history.push(record);
        trim_front(&mut self.action_score_history, MAX_ACTION_SCORE_HISTORY);
    }

    pub fn consecutive_low_action_scores(&self) -> usize {
        self.action_score_history
            .iter()
            .rev()
            .take_while(|record| record.action_score <= 3)
            .count()
    }

    pub fn consecutive_high_risk_low_value_actions(&self) -> usize {
        self.action_score_history
            .iter()
            .rev()
            .take_while(|record| record.risk >= 8 && record.value <= 5)
            .count()
    }

    pub fn score_without_uncertainty_reduction_rounds(&self) -> usize {
        self.action_score_history
            .iter()
            .rev()
            .take_while(|record| {
                record.action_score <= 8
                    || (record.uncertainty_reduction <= 3 && !record.reduced_uncertainty)
            })
            .count()
    }

    pub fn repeated_revised_action_count(&self) -> usize {
        self.action_score_history
            .iter()
            .rev()
            .take_while(|record| {
                record
                    .review_decision
                    .as_deref()
                    .map(|decision| matches!(decision, "revise" | "denied" | "deny"))
                    .unwrap_or(false)
            })
            .count()
    }

    pub fn record_rollback_candidate(&mut self, candidate: RollbackCandidate) {
        if candidate.paths.is_empty()
            && candidate.checkpoint_id.is_none()
            && candidate.file_change_id.is_none()
            && candidate.tool_round_id.is_none()
        {
            return;
        }
        if self.rollback_candidates.iter().any(|existing| {
            existing.checkpoint_id == candidate.checkpoint_id
                && existing.file_change_id == candidate.file_change_id
                && existing.tool_round_id == candidate.tool_round_id
                && existing.paths == candidate.paths
        }) {
            return;
        }
        self.rollback_candidates.push(candidate);
        trim_front(&mut self.rollback_candidates, MAX_ROLLBACK_CANDIDATES);
    }

    pub fn record_failed_strategy(&mut self, record: FailedStrategyRecord) {
        if record.failed_strategy.trim().is_empty() || record.reason.trim().is_empty() {
            return;
        }
        if self.failed_strategies.iter().any(|existing| {
            existing.failed_strategy == record.failed_strategy && existing.reason == record.reason
        }) {
            return;
        }
        self.failed_strategies.push(record);
        trim_front(&mut self.failed_strategies, MAX_FAILED_STRATEGIES);
    }

    fn observe_context_ledger_entry(&mut self, entry: ContextLedgerEntry) {
        match entry {
            ContextLedgerEntry::FileEdit(entry) => {
                for path in entry.paths.iter().chain(entry.resolved_paths.iter()) {
                    self.add_active_file(path);
                }
                let target = display_evidence_paths(&entry.paths, &entry.resolved_paths);
                if entry.success {
                    self.consecutive_edit_failures = 0;
                    self.mark_progress(format!("edit succeeded: {}", target));
                    self.record_completed_step(
                        AgentTaskStage::Edit,
                        format!(
                            "{} changed {} file(s): {}",
                            entry.tool, entry.file_count, target
                        ),
                    );
                    if !matches!(self.stage, AgentTaskStage::Closeout | AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Validate,
                            "context_ledger.file_edit",
                            "successful edit requires validation",
                            1,
                        );
                    }
                    self.record_edit_snapshot(format!("edit succeeded: {}", target));
                } else {
                    self.consecutive_edit_failures += 1;
                    self.last_failure_family = Some("edit".to_string());
                    self.record_observation(
                        "context_ledger.file_edit",
                        format!(
                            "{} attempted change on {} but did not succeed",
                            entry.tool, target
                        ),
                    );
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Repair,
                            "context_ledger.file_edit",
                            "failed edit requires repair",
                            1,
                        );
                    }
                    self.record_edit_snapshot(format!("edit failed: {}", target));
                }
            }
            ContextLedgerEntry::Diff(entry) => {
                let target = entry
                    .command
                    .as_deref()
                    .or(entry.path.as_deref())
                    .or(entry.action.as_deref())
                    .unwrap_or("diff");
                self.record_observation(
                    "context_ledger.diff",
                    format!(
                        "{} inspected {}: changed={}, success={}",
                        entry.tool, target, entry.changed, entry.success
                    ),
                );
            }
            ContextLedgerEntry::Validation(entry) => {
                let status = if entry.success { "passed" } else { "failed" };
                self.record_observation(
                    "context_ledger.validation",
                    format!(
                        "validation {} {} with exit {}",
                        entry.command,
                        status,
                        entry
                            .exit_code
                            .map(|code| code.to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    ),
                );
                if entry.success {
                    self.consecutive_validation_failures = 0;
                    self.consecutive_command_failures = 0;
                    self.mark_progress(format!("validation passed: {}", entry.command));
                    self.record_completed_step(
                        AgentTaskStage::Validate,
                        format!("validation passed: {}", entry.command),
                    );
                    self.verification_plan.status = VerificationStatus::Verified;
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Closeout,
                            "context_ledger.validation",
                            "successful validation is ready for closeout",
                            1,
                        );
                    }
                } else {
                    self.consecutive_validation_failures += 1;
                    self.consecutive_command_failures += 1;
                    self.last_failure_family = Some("validation".to_string());
                    self.verification_plan.status = VerificationStatus::Failed;
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Repair,
                            "context_ledger.validation",
                            "failed validation requires repair",
                            1,
                        );
                    }
                    self.record_edit_snapshot(format!("validation failed: {}", entry.command));
                }
            }
            ContextLedgerEntry::UserConfirmation(entry) => {
                let kind = entry.kind.as_deref().unwrap_or("permission");
                self.record_observation(
                    "context_ledger.user_confirmation",
                    format!(
                        "user {} {} for {}",
                        if entry.approved { "approved" } else { "denied" },
                        kind,
                        entry.tool
                    ),
                );
                if !entry.approved {
                    self.consecutive_permission_blocks += 1;
                    self.last_failure_family = Some("permission".to_string());
                    if matches!(
                        self.verification_plan.status,
                        VerificationStatus::Pending | VerificationStatus::NotRequired
                    ) {
                        self.verification_plan.status = VerificationStatus::UserDeferred;
                    }
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.transition_to_stage(
                            AgentTaskStage::Repair,
                            "context_ledger.user_confirmation",
                            "user denied or blocked the action",
                            1,
                        );
                    }
                } else {
                    self.consecutive_permission_blocks = 0;
                    self.mark_progress(format!("user approved {} for {}", kind, entry.tool));
                }
            }
            ContextLedgerEntry::ToolObservation(entry) => {
                for path in entry.files_read.iter().chain(entry.files_changed.iter()) {
                    self.add_active_file(path);
                }
                if entry.store_in_state
                    || !entry.key_findings.is_empty()
                    || !entry.evidence.is_empty()
                {
                    self.record_observation(
                        "tool_observation",
                        format!(
                            "{} {}: {}",
                            entry.tool,
                            entry.status,
                            preview(&entry.summary, 160)
                        ),
                    );
                }
                for finding in &entry.key_findings {
                    self.record_key_finding(
                        format!("tool_observation.{}", entry.result_kind),
                        finding.clone(),
                        entry.evidence.clone(),
                    );
                }
                if let Some(impact) = &entry.impact_on_goal {
                    self.record_key_finding(
                        "tool_observation.impact",
                        impact.clone(),
                        entry.evidence.clone(),
                    );
                }
                for attention in &entry.next_attention {
                    self.record_key_finding(
                        "tool_observation.next_attention",
                        attention.clone(),
                        entry.evidence.clone(),
                    );
                }
                for hypothesis in &entry.hypothesis_updates {
                    self.record_hypothesis(
                        hypothesis.clone(),
                        entry.confidence.unwrap_or(70),
                        entry.evidence.clone(),
                    );
                }
                for focus in entry
                    .candidate_focus
                    .iter()
                    .chain(entry.files_read.iter())
                    .chain(entry.files_changed.iter())
                {
                    self.record_candidate_focus(
                        focus.clone(),
                        format!("{} observation", entry.result_kind),
                        entry.confidence.unwrap_or(70),
                    );
                }
                if let Some(risk_note) = &entry.risk_note {
                    self.add_risk(risk_note.clone());
                }
                self.record_action_score_from_tool_observation(&entry);
                self.update_progress_from_tool_observation(&entry);
            }
        }
    }

    fn record_action_score_from_tool_observation(
        &mut self,
        entry: &crate::engine::context_ledger::ToolObservationLedgerEntry,
    ) {
        let Some(action_score) = entry.action_score else {
            return;
        };
        let Some(value) = entry.action_value else {
            return;
        };
        let Some(risk) = entry.action_risk else {
            return;
        };
        let Some(uncertainty_reduction) = entry.action_uncertainty_reduction else {
            return;
        };
        let Some(cost) = entry.action_cost else {
            return;
        };
        let Some(reversibility) = entry.action_reversibility else {
            return;
        };
        let Some(scope_fit) = entry.action_scope_fit else {
            return;
        };

        self.record_action_score(ActionScoreRecord {
            tool: entry.tool.clone(),
            stage: entry
                .action_stage
                .clone()
                .unwrap_or_else(|| format!("{:?}", self.stage)),
            action_score,
            value,
            risk,
            uncertainty_reduction,
            cost,
            reversibility,
            scope_fit,
            formula_stage: entry.action_formula_stage.clone(),
            formula_version: entry.action_formula_version.clone(),
            review_decision: entry.action_review_decision.clone(),
            reduced_uncertainty: entry.reduced_uncertainty,
        });
    }

    fn mark_progress(&mut self, signal: String) {
        if signal.trim().is_empty() {
            return;
        }
        self.uncertainty_not_reduced_steps = 0;
        self.last_progress_signal = Some(preview(&signal, 160));
    }

    fn mark_uncertainty_not_reduced(&mut self) {
        self.uncertainty_not_reduced_steps += 1;
    }

    fn update_progress_from_tool_observation(
        &mut self,
        entry: &crate::engine::context_ledger::ToolObservationLedgerEntry,
    ) {
        let status = entry.status.as_str();
        let result_kind = entry.result_kind.as_str();
        let success = status == "success" || status == "ok" || status == "passed";
        let failed = matches!(
            status,
            "failed" | "error" | "denied" | "rejected" | "blocked"
        );

        if entry.reduced_uncertainty || success || !entry.key_findings.is_empty() {
            self.mark_progress(format!(
                "{} {} observation reduced uncertainty",
                entry.tool, result_kind
            ));
        } else if entry.include_in_next_context || entry.store_in_state {
            self.mark_uncertainty_not_reduced();
        }

        if matches!(result_kind, "validation" | "test" | "command_validation") {
            if success {
                self.consecutive_validation_failures = 0;
                self.consecutive_command_failures = 0;
            } else if failed {
                self.consecutive_validation_failures += 1;
                self.consecutive_command_failures += 1;
                self.last_failure_family = entry
                    .failure_type
                    .clone()
                    .or_else(|| Some("validation".to_string()));
            }
        } else if matches!(result_kind, "edit" | "file_edit" | "patch" | "write") {
            if success {
                self.consecutive_edit_failures = 0;
            } else if failed {
                self.consecutive_edit_failures += 1;
                self.last_failure_family = entry
                    .failure_type
                    .clone()
                    .or_else(|| Some("edit".to_string()));
            }
        } else if matches!(result_kind, "command" | "bash" | "shell") {
            if success {
                self.consecutive_command_failures = 0;
            } else if failed {
                self.consecutive_command_failures += 1;
                self.last_failure_family = entry
                    .failure_type
                    .clone()
                    .or_else(|| Some("command".to_string()));
            }
        } else if failed && entry.failure_type.is_some() {
            self.last_failure_family = entry.failure_type.clone();
        }

        let permission_denied = entry
            .permission_decision
            .as_deref()
            .is_some_and(|decision| matches!(decision, "denied" | "blocked" | "rejected"))
            || matches!(status, "denied" | "blocked");
        if permission_denied {
            self.consecutive_permission_blocks += 1;
            self.last_failure_family = Some("permission".to_string());
        }

        let should_recommend_rollback = failed
            && entry.checkpoint_id.is_some()
            && (!entry.files_changed.is_empty()
                || matches!(result_kind, "edit" | "file_edit" | "patch" | "write"));
        if should_recommend_rollback {
            self.record_rollback_candidate(RollbackCandidate {
                checkpoint_id: entry.checkpoint_id.clone(),
                file_change_id: None,
                tool_round_id: Some(entry.call_id.clone()),
                paths: entry.files_changed.clone(),
                reason: entry.risk_note.clone().unwrap_or_else(|| {
                    format!("{} failed after a checkpointed change", entry.tool)
                }),
                confidence: entry.confidence.unwrap_or(75),
                auto_allowed: false,
            });
        }

        if success && matches!(entry.tool.as_str(), "rewind" | "rollback") {
            self.terminal_status = Some(TaskTerminalStatus::RolledBack);
        }
    }

    pub fn observe_tool_round(&mut self, observation: AgentToolRoundObservation) {
        if observation.has_successful_validation_commands {
            self.verification_plan.status = VerificationStatus::Verified;
            self.record_completed_step(AgentTaskStage::Validate, "validation succeeded");
            self.transition_to_stage(
                AgentTaskStage::Closeout,
                "tool_round",
                "validation succeeded",
                1,
            );
            return;
        }

        if observation.batch_has_unsuccessful_tools || observation.failed_tool_evidence_present {
            if matches!(self.verification_plan.status, VerificationStatus::Pending) {
                self.verification_plan.status = VerificationStatus::Failed;
            }
            self.record_observation("tool_round", "tool failure requires repair");
            self.transition_to_stage(
                AgentTaskStage::Repair,
                "tool_round",
                "tool failure requires repair",
                1,
            );
            self.record_edit_snapshot("tool round requires repair");
            return;
        }

        if observation.successful_write_tool
            || observation.used_write_tool
            || observation.has_worktree_changes
        {
            self.record_completed_step(AgentTaskStage::Edit, "code changes were applied");
            self.transition_to_stage(
                AgentTaskStage::Validate,
                "tool_round",
                "code changes were applied",
                1,
            );
            self.record_edit_snapshot("tool round applied changes");
            return;
        }

        if observation.any_tool_success
            && matches!(
                self.stage,
                AgentTaskStage::Understand | AgentTaskStage::Plan
            )
        {
            self.record_completed_step(self.stage, "initial context was inspected");
            self.transition_to_stage(
                AgentTaskStage::Edit,
                "tool_round",
                "initial context was inspected",
                1,
            );
        }
    }

    pub fn format_for_context_zone(&self) -> String {
        let active_files = if self.active_files.is_empty() {
            "none".to_string()
        } else {
            self.active_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let risks = if self.risks.is_empty() {
            "none".to_string()
        } else {
            self.risks.join("; ")
        };
        let checks = if self.verification_plan.required_checks.is_empty() {
            "none".to_string()
        } else {
            self.verification_plan.required_checks.join("; ")
        };
        let stop_check = self
            .stop_checks
            .last()
            .map(|record| {
                let terminal = record
                    .terminal_status
                    .map(|status| status.label())
                    .unwrap_or("none");
                let failure = record.failure_type.as_deref().unwrap_or("none");
                let recovery = record.recovery_plan_id.as_deref().unwrap_or("none");
                let rollback = record
                    .rollback_candidate
                    .as_ref()
                    .and_then(|candidate| candidate.checkpoint_id.as_deref())
                    .unwrap_or("none");
                format!(
                    "{}: reason={} terminal={} action={} failure={} recovery={} rollback={} summary={}",
                    record.status.label(),
                    record.reason.label(),
                    terminal,
                    record.action.label(),
                    failure,
                    recovery,
                    rollback,
                    preview(&record.summary, 160)
                )
            })
            .unwrap_or_else(|| "none".to_string());
        let terminal_status = self
            .terminal_status
            .map(|status| status.label())
            .unwrap_or("none");
        let failure_counters = format!(
            "uncertainty={}, validation={}, edit={}, command={}, permission={}, low_score={}, score_no_uncertainty={}, revised_actions={}",
            self.uncertainty_not_reduced_steps,
            self.consecutive_validation_failures,
            self.consecutive_edit_failures,
            self.consecutive_command_failures,
            self.consecutive_permission_blocks,
            self.consecutive_low_action_scores(),
            self.score_without_uncertainty_reduction_rounds(),
            self.repeated_revised_action_count()
        );
        let rollback_candidates = if self.rollback_candidates.is_empty() {
            "none".to_string()
        } else {
            self.rollback_candidates
                .iter()
                .rev()
                .take(2)
                .map(|candidate| {
                    format!(
                        "checkpoint={} paths={} reason={}",
                        candidate.checkpoint_id.as_deref().unwrap_or("none"),
                        preview(&candidate.paths.join(", "), 80),
                        preview(&candidate.reason, 80)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let recent_steps = if self.completed_steps.is_empty() {
            "none".to_string()
        } else {
            self.completed_steps
                .iter()
                .rev()
                .take(3)
                .map(|step| format!("{:?}: {}", step.stage, preview(&step.summary, 120)))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let stage_transitions = if self.stage_transitions.is_empty() {
            "none".to_string()
        } else {
            self.stage_transitions
                .iter()
                .rev()
                .take(3)
                .map(|transition| {
                    format!(
                        "{:?}->{:?} via {}: {} evidence={}",
                        transition.from,
                        transition.to,
                        transition.source,
                        preview(&transition.reason, 100),
                        transition.evidence_items
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let recent_observations = if self.observations.is_empty() {
            "none".to_string()
        } else {
            self.observations
                .iter()
                .rev()
                .take(3)
                .map(|observation| {
                    format!(
                        "{}: {}",
                        observation.source,
                        preview(&observation.summary, 120)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let recent_snapshots = if self.edit_snapshots.is_empty() {
            "none".to_string()
        } else {
            self.edit_snapshots
                .iter()
                .rev()
                .take(3)
                .map(|snapshot| {
                    let files = if snapshot.active_files.is_empty() {
                        "none".to_string()
                    } else {
                        snapshot
                            .active_files
                            .iter()
                            .take(3)
                            .map(|path| path.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    format!(
                        "{}: stage={:?}, verification={:?}, files={}",
                        preview(&snapshot.label, 80),
                        snapshot.stage,
                        snapshot.verification_status,
                        files
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let key_findings = if self.key_findings.is_empty() {
            "none".to_string()
        } else {
            self.key_findings
                .iter()
                .rev()
                .take(3)
                .map(|finding| {
                    let evidence = if finding.evidence.is_empty() {
                        String::new()
                    } else {
                        format!(" evidence={}", preview(&finding.evidence.join(" | "), 120))
                    };
                    format!(
                        "{}: {}{}",
                        finding.source,
                        preview(&finding.summary, 120),
                        evidence
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let hypotheses = if self.hypotheses.is_empty() {
            "none".to_string()
        } else {
            self.hypotheses
                .iter()
                .rev()
                .take(3)
                .map(|hypothesis| {
                    format!(
                        "{} ({}%)",
                        preview(&hypothesis.hypothesis, 120),
                        hypothesis.confidence
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let candidate_focus = if self.candidate_focus.is_empty() {
            "none".to_string()
        } else {
            self.candidate_focus
                .iter()
                .rev()
                .take(4)
                .map(|focus| {
                    format!(
                        "{} ({}%, {})",
                        preview(&focus.target, 80),
                        focus.confidence,
                        preview(&focus.reason, 80)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let lightweight_plan = self
            .lightweight_plan
            .as_ref()
            .map(LightweightPlan::format_for_context_zone)
            .unwrap_or_else(|| "none".to_string());
        let action_scores = if self.action_score_history.is_empty() {
            "none".to_string()
        } else {
            self.action_score_history
                .iter()
                .rev()
                .take(3)
                .map(|record| {
                    format!(
                        "{} stage={} score={} value={} risk={} uncertainty={} scope={} review={} reduced_uncertainty={}",
                        record.tool,
                        record.stage,
                        record.action_score,
                        record.value,
                        record.risk,
                        record.uncertainty_reduction,
                        record.scope_fit,
                        record.review_decision.as_deref().unwrap_or("none"),
                        record.reduced_uncertainty
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };

        format!(
            "Goal: {}\nMode: {:?}\nMode score: {}\nLightweight plan: {}\nStage: {:?}\nTerminal status: {}\nActive files: {}\nRisks: {}\nVerification: {:?}; checks: {}\nFailure counters: {}\nRecent action scores: {}\nRecent steps: {}\nStage transitions: {}\nRecent observations: {}\nKey findings: {}\nHypotheses: {}\nCandidate focus: {}\nRecent edit snapshots: {}\nRollback candidates: {}\nStop check: {}\nDone: {}",
            self.main_goal,
            self.mode,
            self.mode_score.compact_summary(),
            lightweight_plan,
            self.stage,
            terminal_status,
            active_files,
            risks,
            self.verification_plan.status,
            checks,
            failure_counters,
            action_scores,
            recent_steps,
            stage_transitions,
            recent_observations,
            key_findings,
            hypotheses,
            candidate_focus,
            recent_snapshots,
            rollback_candidates,
            stop_check,
            self.done_condition.satisfied
        )
    }
}

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
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::services::api::ToolCall;
    use crate::tools::ToolResult;
    use serde_json::json;

    #[test]
    fn bundle_flags_missing_acceptance_for_code_change() {
        let route = IntentRouter::new().route("修改 CLI 状态栏");
        let bundle = TaskContextBundle::new("修改 CLI 状态栏", ".", route, None);
        assert!(bundle.needs_stronger_acceptance());
    }

    #[test]
    fn bundle_deduplicates_context_lists() {
        let route = IntentRouter::new().route("你好");
        let mut bundle = TaskContextBundle::new("你好", ".", route, None);
        bundle.add_constraint("keep it short");
        bundle.add_constraint("keep it short");
        bundle.add_file("src/main.rs");
        bundle.add_file("src/main.rs");
        assert_eq!(bundle.constraints.len(), 1);
        assert_eq!(bundle.relevant_files.len(), 1);
        assert_eq!(bundle.agent_state.active_files.len(), 1);
    }

    #[test]
    fn bundle_applies_model_workflow_judgment() {
        let route = IntentRouter::new().route("实现一个网站");
        let mut bundle = TaskContextBundle::new("实现一个网站", ".", route, None);
        let judgment = crate::engine::workflow_contract::ProgrammingWorkflowJudgment {
            task_type: "website".into(),
            complexity: crate::engine::workflow_contract::TaskComplexity::Medium,
            risk: crate::engine::intent_router::RiskLevel::Medium,
            requirement_complete_enough: true,
            needs_user_questions: false,
            question_reason: None,
            questions: Vec::new(),
            assumptions: vec!["Use local storage".into()],
            guided_reasoning_required: false,
            guided_reasoning_triggers: Vec::new(),
            plan: Vec::new(),
            acceptance: crate::engine::workflow_contract::AcceptanceContract::pending(
                "实现一个网站",
                vec!["Main page renders".into()],
                Vec::new(),
            ),
        };

        bundle.apply_workflow_judgment(judgment);

        assert!(bundle.workflow_judgment.is_some());
        assert!(bundle
            .constraints
            .iter()
            .any(|item| item.contains("Use local storage")));
        assert!(bundle
            .acceptance_checks
            .iter()
            .any(|item| item == "Main page renders"));
        assert!(bundle
            .agent_state
            .verification_plan
            .required_checks
            .iter()
            .any(|item| item == "Main page renders"));
        assert!(bundle
            .agent_state
            .risks
            .iter()
            .any(|item| item.contains("model-judged risk")));
        assert!(!bundle.needs_stronger_acceptance());
    }

    #[test]
    fn direct_acceptance_checks_do_not_create_validation_requirements() {
        let route = IntentRouter::new().route("只读检查 src/engine/intent_router.rs，不要修改文件");
        let mut bundle = TaskContextBundle::new(
            "只读检查 src/engine/intent_router.rs，不要修改文件",
            ".",
            route,
            None,
        );

        bundle.add_acceptance_check("最终答案包含路由结论");

        assert!(bundle
            .acceptance_checks
            .iter()
            .any(|item| item == "最终答案包含路由结论"));
        assert!(bundle
            .agent_state
            .verification_plan
            .required_checks
            .is_empty());
        assert_eq!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::NotRequired
        );
    }

    #[test]
    fn agent_task_state_initializes_from_route_and_goal() {
        let route = IntentRouter::new().route("修复 src/main.rs 里的报错");
        let bundle = TaskContextBundle::new("修复 src/main.rs 里的报错", ".", route, None);

        assert_eq!(bundle.agent_state.mode, AgentTaskMode::Full);
        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Understand);
        assert_eq!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::Pending
        );
        assert!(bundle
            .agent_state
            .forbidden_actions
            .iter()
            .any(|item| item.contains("outside the requested scope")));
    }

    #[test]
    fn agent_task_state_formats_context_zone_summary() {
        let route = IntentRouter::new().route("你好");
        let mut bundle = TaskContextBundle::new("你好", ".", route, None);
        bundle
            .agent_state
            .record_observation("test", "saw greeting");
        bundle.agent_state.mark_done("answered greeting");

        let rendered = bundle.agent_state.format_for_context_zone();

        assert!(rendered.contains("Goal: 你好"));
        assert!(rendered.contains("Mode: Direct"));
        assert!(rendered.contains("Mode score: Direct"));
        assert!(rendered.contains("Lightweight plan: none"));
        assert!(rendered.contains("Done: true"));
    }

    #[test]
    fn tool_assisted_direct_tasks_get_light_mode_and_plan() {
        let route = IntentRouter::new().route("请帮我看看桌面有没有 gex 文件夹");
        let bundle = TaskContextBundle::new("请帮我看看桌面有没有 gex 文件夹", ".", route, None);

        assert_eq!(bundle.agent_state.mode, AgentTaskMode::Light);
        assert_eq!(bundle.agent_state.mode_score.mode, AgentTaskMode::Light);
        let plan = bundle
            .agent_state
            .lightweight_plan
            .as_ref()
            .expect("light plan");
        assert!(plan.heavy_contract_avoided);
        assert!(plan
            .steps
            .iter()
            .any(|step| step.action.contains("glob") || step.action.contains("file_read")));
        let rendered = bundle.agent_state.format_for_context_zone();
        assert!(rendered.contains("Mode: Light"));
        assert!(rendered.contains("Lightweight plan:"));
        assert!(rendered.contains("verification_required="));
    }

    #[test]
    fn agent_task_state_records_bounded_observations_and_steps() {
        let route = IntentRouter::new().route("修改 src/main.rs");
        let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

        for index in 0..20 {
            bundle
                .agent_state
                .record_observation("test", format!("observation {index}"));
            bundle
                .agent_state
                .record_completed_step(AgentTaskStage::Understand, format!("step {index}"));
        }

        assert_eq!(bundle.agent_state.observations.len(), MAX_OBSERVATIONS);
        assert_eq!(
            bundle.agent_state.completed_steps.len(),
            MAX_COMPLETED_STEPS
        );
        assert_eq!(bundle.agent_state.observations[0].summary, "observation 8");
        assert_eq!(bundle.agent_state.completed_steps[0].summary, "step 8");
    }

    #[test]
    fn agent_task_state_updates_from_tool_context_evidence() {
        let route = IntentRouter::new().route("修改 src/lib.rs");
        let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
        let edit_call = ToolCall {
            id: "call_edit".to_string(),
            name: "file_edit".to_string(),
            arguments: json!({"path": "src/lib.rs"}),
        };
        let edit_result = ToolResult::success_with_data(
            "edited",
            json!({
                "path": "src/lib.rs",
                "resolved_path": "/tmp/project/src/lib.rs",
                "replacements": 1,
                "bytes_written": 42,
                "diff": {
                    "additions": 2,
                    "deletions": 1,
                    "changed_line_start": 4,
                    "changed_line_end": 5,
                    "unified_diff": "@@ -4 +4 @@\n-old\n+new\n"
                }
            }),
        );

        let observed = bundle
            .agent_state
            .observe_tool_context_evidence(&edit_call, &edit_result);

        assert_eq!(observed, 1);
        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Validate);
        assert!(bundle
            .agent_state
            .active_files
            .iter()
            .any(|path| path == &PathBuf::from("src/lib.rs")));
        assert!(bundle.agent_state.completed_steps.iter().any(|step| {
            step.stage == AgentTaskStage::Edit && step.summary.contains("src/lib.rs")
        }));
        assert_eq!(bundle.agent_state.edit_snapshots.len(), 1);
        assert_eq!(
            bundle.agent_state.edit_snapshots[0].stage,
            AgentTaskStage::Validate
        );
        assert!(bundle.agent_state.edit_snapshots[0]
            .label
            .contains("edit succeeded"));

        let validation_call = ToolCall {
            id: "call_validation".to_string(),
            name: "bash".to_string(),
            arguments: json!({"command": "cargo test -q"}),
        };
        let validation_result = ToolResult::success_with_data(
            "ok",
            json!({
                "shell_result": {
                    "command": "cargo test -q",
                    "cwd": "/tmp/project",
                    "exit_code": 0,
                    "timed_out": false
                },
                "permission_request": {
                    "id": "perm_1",
                    "kind": "bash",
                    "approved": true,
                    "patterns": ["cargo test -q"],
                    "allowed_always_rules": [],
                    "metadata": {
                        "risk_level": "low",
                        "permission_decision": "allow_once"
                    }
                }
            }),
        );

        let observed = bundle
            .agent_state
            .observe_tool_context_evidence(&validation_call, &validation_result);

        assert_eq!(observed, 2);
        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Closeout);
        assert_eq!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::Verified
        );
        let rendered = bundle.agent_state.format_for_context_zone();
        assert!(rendered.contains("Recent steps:"));
        assert!(rendered.contains("validation passed: cargo test -q"));
        assert!(rendered.contains("Recent observations:"));
        assert!(rendered.contains("user approved bash"));
        assert!(rendered.contains("Recent edit snapshots:"));
        assert!(rendered.contains("edit succeeded"));
    }

    #[test]
    fn agent_task_state_records_repair_snapshot_after_failed_validation() {
        let route = IntentRouter::new().route("修改 src/lib.rs");
        let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
        bundle.agent_state.add_active_file("src/lib.rs");
        bundle
            .agent_state
            .record_completed_step(AgentTaskStage::Edit, "file_edit changed src/lib.rs");
        bundle.agent_state.set_stage(AgentTaskStage::Validate);

        let validation_call = ToolCall {
            id: "call_validation".to_string(),
            name: "bash".to_string(),
            arguments: json!({"command": "cargo test -q"}),
        };
        let mut validation_result = ToolResult::error("tests failed");
        validation_result.data = Some(json!({
            "shell_result": {
                "command": "cargo test -q",
                "cwd": "/tmp/project",
                "exit_code": 101,
                "timed_out": false
            }
        }));

        let observed = bundle
            .agent_state
            .observe_tool_context_evidence(&validation_call, &validation_result);

        assert_eq!(observed, 1);
        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Repair);
        assert_eq!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::Failed
        );
        let snapshot = bundle
            .agent_state
            .edit_snapshots
            .last()
            .expect("failed validation should record repair snapshot");
        assert!(snapshot.label.contains("validation failed"));
        assert_eq!(snapshot.stage, AgentTaskStage::Repair);
        assert_eq!(snapshot.verification_status, VerificationStatus::Failed);
        assert!(snapshot
            .active_files
            .iter()
            .any(|path| path == &PathBuf::from("src/lib.rs")));
    }

    #[test]
    fn agent_task_state_keeps_bounded_edit_snapshots() {
        let route = IntentRouter::new().route("修改 src/main.rs");
        let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

        for index in 0..10 {
            bundle
                .agent_state
                .record_edit_snapshot(format!("snapshot {index}"));
        }

        assert_eq!(bundle.agent_state.edit_snapshots.len(), MAX_EDIT_SNAPSHOTS);
        assert_eq!(bundle.agent_state.edit_snapshots[0].label, "snapshot 4");
    }

    #[test]
    fn agent_task_state_records_tool_observation_metadata() {
        let route = IntentRouter::new().route("查看 src/lib.rs");
        let mut bundle = TaskContextBundle::new("查看 src/lib.rs", ".", route, None);
        let call = ToolCall {
            id: "call_read".to_string(),
            name: "file_read".to_string(),
            arguments: json!({"path": "src/lib.rs"}),
        };
        let result = ToolResult::success_with_data(
            "read file",
            json!({
                "tool_observation": {
                    "schema": "tool_observation.v1",
                    "tool": "file_read",
                    "call_id": "call_read",
                    "status": "success",
                    "summary": "file_read succeeded: read src/lib.rs",
                    "files_read": ["src/lib.rs"],
                    "files_changed": [],
                    "command_run": null,
                    "validation_result": null,
                    "permission_decision": null,
                    "checkpoint_id": null,
                    "artifact_path": null,
                    "state_updates": ["files_read"],
                    "recommended_next_action": null
                },
                "action_decision": {
                    "action": {
                        "stage": "Understand"
                    },
                    "scores": {
                        "value": 7,
                        "risk": 1,
                        "uncertainty_reduction": 8,
                        "cost": 2,
                        "reversibility": 10,
                        "scope_fit": 9,
                        "action_score": 24
                    },
                    "score_computation": {
                        "formula_stage": "diagnosis",
                        "formula_version": "action_score.v1"
                    }
                },
                "action_review": {
                    "decision": "allow"
                }
            }),
        );

        let observed = bundle
            .agent_state
            .observe_tool_context_evidence(&call, &result);

        assert_eq!(observed, 1);
        assert!(bundle
            .agent_state
            .active_files
            .contains(&PathBuf::from("src/lib.rs")));
        assert!(bundle
            .agent_state
            .observations
            .iter()
            .any(|observation| observation.source == "tool_observation"
                && observation.summary.contains("file_read success")));
        assert_eq!(bundle.agent_state.action_score_history.len(), 1);
        let score = &bundle.agent_state.action_score_history[0];
        assert_eq!(score.action_score, 24);
        assert_eq!(score.scope_fit, 9);
        assert_eq!(score.review_decision.as_deref(), Some("allow"));
    }

    #[test]
    fn agent_task_state_records_structured_observer_findings() {
        let route = IntentRouter::new().route("修复 cargo test 失败");
        let mut bundle = TaskContextBundle::new("修复 cargo test 失败", ".", route, None);
        let call = ToolCall {
            id: "call_test".to_string(),
            name: "bash".to_string(),
            arguments: json!({"command": "cargo test -q"}),
        };
        let result = ToolResult::error_with_content(
            "cargo test failed",
            "test auth::login --- FAILED\nerror[E0425]: cannot find value `token`",
        );
        let mut result = result;
        result.data = Some(json!({
            "tool_observation": {
                "schema": "tool_observation.v1",
                "tool": "bash",
                "call_id": "call_test",
                "status": "failed",
                "result_kind": "validation",
                "summary": "Validation `cargo test -q` failed.",
                "key_findings": ["Failed tests: auth::login."],
                "evidence": [{"kind": "diagnostic", "text": "error[E0425]: cannot find value `token`"}],
                "impact_on_goal": "Narrows the next step to repairing the reported validation failure.",
                "next_attention": ["Rerun `cargo test -q` after the next patch."],
                "files_read": [],
                "files_changed": [],
                "command_run": "cargo test -q",
                "validation_result": "failed",
                "permission_decision": null,
                "checkpoint_id": null,
                "artifact_path": null,
                "state_updates": ["validation_result"],
                "recommended_next_action": null,
                "include_in_next_context": true,
                "store_in_state": true,
                "confidence": 90,
                "raw_result_ref": null,
                "hypothesis_updates": [{
                    "hypothesis": "current implementation does not satisfy the latest validation",
                    "confidence": 80,
                    "evidence": ["error[E0425]"]
                }],
                "candidate_focus": ["src/auth/login.rs"],
                "reduced_uncertainty": true,
                "risk_note": null
            }
        }));

        let observed = bundle
            .agent_state
            .observe_tool_context_evidence(&call, &result);

        assert_eq!(observed, 2);
        assert!(bundle
            .agent_state
            .key_findings
            .iter()
            .any(|finding| finding.summary.contains("auth::login")));
        assert!(bundle
            .agent_state
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.hypothesis.contains("latest validation")));
        assert!(bundle
            .agent_state
            .candidate_focus
            .iter()
            .any(|focus| focus.target == "src/auth/login.rs"));
        let rendered = bundle.agent_state.format_for_context_zone();
        assert!(rendered.contains("Key findings:"));
        assert!(rendered.contains("Hypotheses:"));
        assert!(rendered.contains("Candidate focus:"));
    }

    #[test]
    fn agent_task_state_marks_denied_confirmation_user_deferred() {
        let route = IntentRouter::new().route("修改 src/lib.rs");
        let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
        let call = ToolCall {
            id: "call_denied".to_string(),
            name: "bash".to_string(),
            arguments: json!({"command": "rm -rf target"}),
        };
        let result = ToolResult::error_with_content(
            "Permission denied",
            json!({
                "permission_request": {
                    "id": "perm_1",
                    "kind": "bash",
                    "approved": false,
                    "patterns": ["rm -rf target"],
                    "allowed_always_rules": [],
                    "metadata": {
                        "risk_level": "high",
                        "permission_decision": "deny"
                    }
                }
            })
            .to_string(),
        );
        let mut result = result;
        result.data = Some(json!({
            "permission_request": {
                "id": "perm_1",
                "kind": "bash",
                "approved": false,
                "patterns": ["rm -rf target"],
                "allowed_always_rules": [],
                "metadata": {
                    "risk_level": "high",
                    "permission_decision": "deny"
                }
            }
        }));

        let observed = bundle
            .agent_state
            .observe_tool_context_evidence(&call, &result);

        assert_eq!(observed, 1);
        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Repair);
        assert_eq!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::UserDeferred
        );
        assert!(bundle
            .agent_state
            .observations
            .iter()
            .any(|observation| observation.summary.contains("user denied bash")));
    }

    #[test]
    fn agent_task_state_advances_from_understand_to_edit_after_successful_inspection() {
        let route = IntentRouter::new().route("修改 src/main.rs");
        let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

        bundle
            .agent_state
            .observe_tool_round(AgentToolRoundObservation {
                any_tool_success: true,
                batch_has_unsuccessful_tools: false,
                used_write_tool: false,
                successful_write_tool: false,
                has_worktree_changes: false,
                has_successful_validation_commands: false,
                failed_tool_evidence_present: false,
            });

        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Edit);
        assert!(bundle
            .agent_state
            .completed_steps
            .iter()
            .any(|step| step.stage == AgentTaskStage::Understand));
    }

    #[test]
    fn agent_task_state_advances_to_validate_after_write_and_closeout_after_validation() {
        let route = IntentRouter::new().route("修改 src/main.rs");
        let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

        bundle
            .agent_state
            .observe_tool_round(AgentToolRoundObservation {
                any_tool_success: true,
                batch_has_unsuccessful_tools: false,
                used_write_tool: true,
                successful_write_tool: true,
                has_worktree_changes: true,
                has_successful_validation_commands: false,
                failed_tool_evidence_present: false,
            });
        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Validate);

        bundle
            .agent_state
            .observe_tool_round(AgentToolRoundObservation {
                any_tool_success: true,
                batch_has_unsuccessful_tools: false,
                used_write_tool: false,
                successful_write_tool: false,
                has_worktree_changes: true,
                has_successful_validation_commands: true,
                failed_tool_evidence_present: false,
            });

        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Closeout);
        assert_eq!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::Verified
        );
    }

    #[test]
    fn agent_task_state_moves_to_repair_after_failed_tool_round() {
        let route = IntentRouter::new().route("修改 src/main.rs");
        let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);

        bundle
            .agent_state
            .observe_tool_round(AgentToolRoundObservation {
                any_tool_success: false,
                batch_has_unsuccessful_tools: true,
                used_write_tool: false,
                successful_write_tool: false,
                has_worktree_changes: false,
                has_successful_validation_commands: false,
                failed_tool_evidence_present: true,
            });

        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Repair);
        assert_eq!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::Failed
        );
    }
}
