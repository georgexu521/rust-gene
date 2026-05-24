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
pub enum StopCheckReason {
    NoIssue,
    NoProgress,
    FocusedRepairStalled,
    RepeatedToolFailure,
    DuplicateReadOnly,
    VerificationReady,
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservationSummary {
    pub source: String,
    pub summary: String,
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
    pub reason: StopCheckReason,
    pub summary: String,
    pub no_code_progress_rounds: usize,
    pub action_checkpoint_active: bool,
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
    pub allowed_scope: Vec<String>,
    pub forbidden_actions: Vec<String>,
    pub completed_steps: Vec<CompletedStep>,
    pub observations: Vec<ObservationSummary>,
    #[serde(default)]
    pub edit_snapshots: Vec<EditStateSnapshot>,
    pub active_files: Vec<PathBuf>,
    pub risks: Vec<String>,
    pub verification_plan: VerificationPlan,
    pub done_condition: DoneCondition,
    #[serde(default)]
    pub stop_checks: Vec<StopCheckRecord>,
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
        self.stage = stage;
    }

    pub fn mark_done(&mut self, summary: impl Into<String>) {
        self.stage = AgentTaskStage::Done;
        self.done_condition.summary = summary.into();
        self.done_condition.satisfied = true;
    }

    pub fn record_stop_check(&mut self, record: StopCheckRecord) {
        self.stop_checks.push(record);
        const MAX_STOP_CHECKS: usize = 8;
        if self.stop_checks.len() > MAX_STOP_CHECKS {
            let overflow = self.stop_checks.len() - MAX_STOP_CHECKS;
            self.stop_checks.drain(0..overflow);
        }
    }

    fn observe_context_ledger_entry(&mut self, entry: ContextLedgerEntry) {
        match entry {
            ContextLedgerEntry::FileEdit(entry) => {
                for path in entry.paths.iter().chain(entry.resolved_paths.iter()) {
                    self.add_active_file(path);
                }
                let target = display_evidence_paths(&entry.paths, &entry.resolved_paths);
                if entry.success {
                    self.record_completed_step(
                        AgentTaskStage::Edit,
                        format!(
                            "{} changed {} file(s): {}",
                            entry.tool, entry.file_count, target
                        ),
                    );
                    if !matches!(self.stage, AgentTaskStage::Closeout | AgentTaskStage::Done) {
                        self.stage = AgentTaskStage::Validate;
                    }
                    self.record_edit_snapshot(format!("edit succeeded: {}", target));
                } else {
                    self.record_observation(
                        "context_ledger.file_edit",
                        format!(
                            "{} attempted change on {} but did not succeed",
                            entry.tool, target
                        ),
                    );
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.stage = AgentTaskStage::Repair;
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
                    self.record_completed_step(
                        AgentTaskStage::Validate,
                        format!("validation passed: {}", entry.command),
                    );
                    self.verification_plan.status = VerificationStatus::Verified;
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.stage = AgentTaskStage::Closeout;
                    }
                } else {
                    self.verification_plan.status = VerificationStatus::Failed;
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.stage = AgentTaskStage::Repair;
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
                    if matches!(
                        self.verification_plan.status,
                        VerificationStatus::Pending | VerificationStatus::NotRequired
                    ) {
                        self.verification_plan.status = VerificationStatus::UserDeferred;
                    }
                    if !matches!(self.stage, AgentTaskStage::Done) {
                        self.stage = AgentTaskStage::Repair;
                    }
                }
            }
            ContextLedgerEntry::ToolObservation(entry) => {
                for path in entry.files_read.iter().chain(entry.files_changed.iter()) {
                    self.add_active_file(path);
                }
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
        }
    }

    pub fn observe_tool_round(&mut self, observation: AgentToolRoundObservation) {
        if observation.has_successful_validation_commands {
            self.verification_plan.status = VerificationStatus::Verified;
            self.record_completed_step(AgentTaskStage::Validate, "validation succeeded");
            self.stage = AgentTaskStage::Closeout;
            return;
        }

        if observation.batch_has_unsuccessful_tools || observation.failed_tool_evidence_present {
            if matches!(self.verification_plan.status, VerificationStatus::Pending) {
                self.verification_plan.status = VerificationStatus::Failed;
            }
            self.record_observation("tool_round", "tool failure requires repair");
            self.stage = AgentTaskStage::Repair;
            self.record_edit_snapshot("tool round requires repair");
            return;
        }

        if observation.successful_write_tool
            || observation.used_write_tool
            || observation.has_worktree_changes
        {
            self.record_completed_step(AgentTaskStage::Edit, "code changes were applied");
            self.stage = AgentTaskStage::Validate;
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
            self.stage = AgentTaskStage::Edit;
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
                format!(
                    "{:?}: {:?}; {}",
                    record.status, record.reason, record.summary
                )
            })
            .unwrap_or_else(|| "none".to_string());
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
        let lightweight_plan = self
            .lightweight_plan
            .as_ref()
            .map(LightweightPlan::format_for_context_zone)
            .unwrap_or_else(|| "none".to_string());

        format!(
            "Goal: {}\nMode: {:?}\nMode score: {}\nLightweight plan: {}\nStage: {:?}\nActive files: {}\nRisks: {}\nVerification: {:?}; checks: {}\nRecent steps: {}\nRecent observations: {}\nRecent edit snapshots: {}\nStop check: {}\nDone: {}",
            self.main_goal,
            self.mode,
            self.mode_score.compact_summary(),
            lightweight_plan,
            self.stage,
            active_files,
            risks,
            self.verification_plan.status,
            checks,
            recent_steps,
            recent_observations,
            recent_snapshots,
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
        self.agent_state.add_required_check(check.clone());
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
