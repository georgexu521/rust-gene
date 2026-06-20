use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::agent::envelope::AgentTaskEnvelope;

pub const LAB_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_LEASE_TTL_SECONDS: u64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabProposalStatus {
    Draft,
    AwaitingApproval,
    Approved,
    Rejected,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedMode {
    Direct,
    Goal,
    Labrun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabProposalApproval {
    pub approved_by_user: bool,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_lab_run_id: Option<String>,
}

impl Default for LabProposalApproval {
    fn default() -> Self {
        Self {
            approved_by_user: false,
            approved_at: None,
            created_lab_run_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabProposal {
    pub schema_version: u32,
    pub proposal_id: String,
    pub status: LabProposalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub project_root: String,
    pub user_session_id: Option<String>,
    pub user_goal: String,
    pub problem_statement: String,
    pub desired_outcome: String,
    pub scope: Vec<String>,
    pub non_goals: Vec<String>,
    pub constraints: Vec<String>,
    pub risks: Vec<String>,
    pub success_criteria: Vec<String>,
    pub recommended_mode: RecommendedMode,
    pub professor_rationale: String,
    pub approval: LabProposalApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabProposalIntakeDraft {
    pub problem_statement: String,
    pub desired_outcome: String,
    pub scope: Vec<String>,
    pub non_goals: Vec<String>,
    pub constraints: Vec<String>,
    pub risks: Vec<String>,
    pub success_criteria: Vec<String>,
    pub recommended_mode: RecommendedMode,
    pub professor_rationale: String,
}

impl LabProposalIntakeDraft {
    pub fn from_goal(user_goal: &str) -> Self {
        Self {
            problem_statement: user_goal.trim().to_string(),
            desired_outcome: String::new(),
            scope: Vec::new(),
            non_goals: Vec::new(),
            constraints: Vec::new(),
            risks: Vec::new(),
            success_criteria: Vec::new(),
            recommended_mode: RecommendedMode::Labrun,
            professor_rationale: "Professor intake draft; refine before approval if needed."
                .to_string(),
        }
    }
}

impl LabProposal {
    pub fn new(
        proposal_id: String,
        project_root: String,
        user_session_id: Option<String>,
        user_goal: String,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            schema_version: LAB_SCHEMA_VERSION,
            proposal_id,
            status: LabProposalStatus::AwaitingApproval,
            created_at: now,
            updated_at: now,
            project_root,
            user_session_id,
            problem_statement: user_goal.clone(),
            desired_outcome: String::new(),
            user_goal,
            scope: Vec::new(),
            non_goals: Vec::new(),
            constraints: Vec::new(),
            risks: Vec::new(),
            success_criteria: Vec::new(),
            recommended_mode: RecommendedMode::Labrun,
            professor_rationale: "Professor intake draft; refine before approval if needed."
                .to_string(),
            approval: LabProposalApproval::default(),
        }
    }

    pub fn apply_intake_draft(&mut self, draft: LabProposalIntakeDraft) {
        self.problem_statement = draft.problem_statement;
        self.desired_outcome = draft.desired_outcome;
        self.scope = draft.scope;
        self.non_goals = draft.non_goals;
        self.constraints = draft.constraints;
        self.risks = draft.risks;
        self.success_criteria = draft.success_criteria;
        self.recommended_mode = draft.recommended_mode;
        self.professor_rationale = draft.professor_rationale;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabRunKind {
    ArchitecturePlan,
    Implementation,
    LabMeeting,
    ProfessorReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabRunStatus {
    Created,
    Active,
    Paused,
    PausedShutdown,
    Blocked,
    Completed,
    Failed,
    NeedsUser,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabRole {
    Professor,
    Postdoc,
    Graduate,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabTaskStatus {
    Queued,
    InProgress,
    Blocked,
    Completed,
    Cancelled,
}

impl LabTaskStatus {
    pub fn is_open(self) -> bool {
        matches!(self, Self::Queued | Self::InProgress | Self::Blocked)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabProviderCertificationKind {
    ControlPlane,
    Graduate,
}

impl LabProviderCertificationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ControlPlane => "control_plane",
            Self::Graduate => "graduate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabProviderCertificationOutcome {
    Passed,
    Failed,
}

impl LabProviderCertificationOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabProviderCertificationRecord {
    pub schema_version: u32,
    pub record_id: String,
    pub provider_id: String,
    pub model: String,
    pub kind: LabProviderCertificationKind,
    pub outcome: LabProviderCertificationOutcome,
    pub recorded_at: DateTime<Utc>,
    pub evidence_path: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraduateTask {
    pub schema_version: u32,
    pub task_id: String,
    pub lab_run_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: LabRole,
    pub assigned_role: LabRole,
    pub status: LabTaskStatus,
    pub title: String,
    pub instructions: String,
    pub allowed_scope: Vec<String>,
    pub required_validation: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub result_artifact_id: Option<String>,
    pub blocker: Option<String>,
    pub cycle_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraduateDispatchStatus {
    Prepared,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraduateDispatchRecord {
    pub schema_version: u32,
    pub dispatch_id: String,
    pub lab_run_id: String,
    pub task_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: GraduateDispatchStatus,
    pub envelope: AgentTaskEnvelope,
    pub agent_tool_params: serde_json::Value,
    pub agent_id: Option<String>,
    pub result_artifact_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabValidationRetry {
    pub schema_version: u32,
    pub retry_id: String,
    pub lab_run_id: String,
    pub task_id: String,
    pub created_at: DateTime<Utc>,
    pub attempt: u32,
    pub validation_summary: String,
    pub repair_task_id: Option<String>,
    pub escalated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabSchedulerStatus {
    Idle,
    Running,
    Stopping,
    PausedRestart,
    Stopped,
    Blocked,
    NeedsUser,
    Failed,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabSchedulerState {
    pub schema_version: u32,
    pub lab_run_id: String,
    pub status: LabSchedulerStatus,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub max_steps: usize,
    pub steps_completed: usize,
    pub interval_ms: u64,
    pub last_action: Option<String>,
    pub last_message: Option<String>,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabAppLifecycleState {
    pub schema_version: u32,
    pub project_root: String,
    pub launch_mode: String,
    pub process_id: u32,
    pub updated_at: DateTime<Utc>,
    pub last_startup_at: Option<DateTime<Utc>>,
    pub last_shutdown_at: Option<DateTime<Utc>>,
    pub recovered_scheduler_lab_run_id: Option<String>,
    pub recovered_scheduler_status: Option<LabSchedulerStatus>,
    pub shutdown_paused_lab_run_id: Option<String>,
    pub last_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabDaemonMode {
    Strict,
    Hybrid,
    HybridCycles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabDaemonState {
    pub schema_version: u32,
    pub project_root: String,
    pub enabled: bool,
    pub mode: LabDaemonMode,
    pub max_steps: usize,
    #[serde(default = "default_daemon_max_steps_per_cycle")]
    pub max_steps_per_cycle: usize,
    pub interval_ms: u64,
    pub instructions: String,
    pub updated_at: DateTime<Utc>,
    pub last_enabled_at: Option<DateTime<Utc>>,
    pub last_disabled_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_started_lab_run_id: Option<String>,
    #[serde(default)]
    pub last_start_error: Option<String>,
    pub last_message: Option<String>,
}

pub fn default_daemon_max_steps_per_cycle() -> usize {
    5
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeCursor {
    pub last_event_seq: u64,
    pub current_stage: String,
    pub internal_owner: LabRole,
    pub active_artifact_id: Option<String>,
    pub open_task_ids: Vec<String>,
}

impl Default for ResumeCursor {
    fn default() -> Self {
        Self {
            last_event_seq: 0,
            current_stage: "professor_plan".to_string(),
            internal_owner: LabRole::Professor,
            active_artifact_id: None,
            open_task_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleProfile {
    pub profile: String,
    pub model_policy: String,
    pub prompt_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabRoles {
    pub professor: RoleProfile,
    pub postdoc: RoleProfile,
    pub graduate: RoleProfile,
}

impl Default for LabRoles {
    fn default() -> Self {
        Self {
            professor: RoleProfile {
                profile: "lab-professor".to_string(),
                model_policy: "high_reasoning".to_string(),
                prompt_version: "lab-professor.v1".to_string(),
            },
            postdoc: RoleProfile {
                profile: "lab-postdoc".to_string(),
                model_policy: "code_reasoning".to_string(),
                prompt_version: "lab-postdoc.v1".to_string(),
            },
            graduate: RoleProfile {
                profile: "lab-graduate".to_string(),
                model_policy: "coding_worker".to_string(),
                prompt_version: "lab-graduate.v1".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabCostPolicy {
    pub mode: String,
    pub max_cycle_tokens: u64,
    pub max_meeting_rounds: u32,
    pub professor_context_budget: u64,
    pub postdoc_context_budget: u64,
    pub graduate_context_budget: u64,
    pub meeting_context_budget: u64,
    pub auto_compress_after_cycle: bool,
    pub evidence_default: String,
}

impl Default for LabCostPolicy {
    fn default() -> Self {
        Self {
            mode: "balanced".to_string(),
            max_cycle_tokens: 200_000,
            max_meeting_rounds: 3,
            professor_context_budget: 24_000,
            postdoc_context_budget: 30_000,
            graduate_context_budget: 12_000,
            meeting_context_budget: 36_000,
            auto_compress_after_cycle: true,
            evidence_default: "refs_only".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabCostUsage {
    pub schema_version: u32,
    pub usage_id: String,
    pub lab_run_id: String,
    pub created_at: DateTime<Utc>,
    pub role: LabRole,
    pub cycle_id: Option<String>,
    pub meeting_id: Option<String>,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_miss_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabRoleCostSummary {
    pub role: LabRole,
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_miss_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
}

impl LabRoleCostSummary {
    pub fn new(role: LabRole) -> Self {
        Self {
            role,
            requests: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            reasoning_tokens: 0,
            cached_tokens: 0,
            cache_write_tokens: 0,
            cache_miss_tokens: 0,
            total_tokens: 0,
            estimated_cost_usd: 0.0,
        }
    }

    pub fn add_usage(&mut self, usage: &LabCostUsage) {
        self.requests = self.requests.saturating_add(1);
        self.prompt_tokens = self.prompt_tokens.saturating_add(usage.prompt_tokens);
        self.completion_tokens = self
            .completion_tokens
            .saturating_add(usage.completion_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(usage.reasoning_tokens);
        self.cached_tokens = self.cached_tokens.saturating_add(usage.cached_tokens);
        self.cache_write_tokens = self
            .cache_write_tokens
            .saturating_add(usage.cache_write_tokens);
        self.cache_miss_tokens = self
            .cache_miss_tokens
            .saturating_add(usage.cache_miss_tokens);
        self.total_tokens = self.total_tokens.saturating_add(usage.total_tokens);
        self.estimated_cost_usd += usage.estimated_cost_usd;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabCostSummary {
    pub schema_version: u32,
    pub lab_run_id: String,
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_miss_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
    pub by_role: Vec<LabRoleCostSummary>,
}

impl LabCostSummary {
    pub fn empty(lab_run_id: impl Into<String>) -> Self {
        Self {
            schema_version: LAB_SCHEMA_VERSION,
            lab_run_id: lab_run_id.into(),
            requests: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            reasoning_tokens: 0,
            cached_tokens: 0,
            cache_write_tokens: 0,
            cache_miss_tokens: 0,
            total_tokens: 0,
            estimated_cost_usd: 0.0,
            by_role: Vec::new(),
        }
    }

    pub fn add_usage(&mut self, usage: &LabCostUsage) {
        self.requests = self.requests.saturating_add(1);
        self.prompt_tokens = self.prompt_tokens.saturating_add(usage.prompt_tokens);
        self.completion_tokens = self
            .completion_tokens
            .saturating_add(usage.completion_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(usage.reasoning_tokens);
        self.cached_tokens = self.cached_tokens.saturating_add(usage.cached_tokens);
        self.cache_write_tokens = self
            .cache_write_tokens
            .saturating_add(usage.cache_write_tokens);
        self.cache_miss_tokens = self
            .cache_miss_tokens
            .saturating_add(usage.cache_miss_tokens);
        self.total_tokens = self.total_tokens.saturating_add(usage.total_tokens);
        self.estimated_cost_usd += usage.estimated_cost_usd;

        if let Some(role_summary) = self
            .by_role
            .iter_mut()
            .find(|summary| summary.role == usage.role)
        {
            role_summary.add_usage(usage);
        } else {
            let mut role_summary = LabRoleCostSummary::new(usage.role);
            role_summary.add_usage(usage);
            self.by_role.push(role_summary);
        }
    }

    pub fn cache_hit_rate_percent(&self) -> f64 {
        if self.prompt_tokens == 0 {
            return 0.0;
        }
        (self.cached_tokens as f64 / self.prompt_tokens as f64) * 100.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabEvidenceKind {
    File,
    Diff,
    Log,
    Command,
    Artifact,
    Url,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabEvidenceRef {
    pub schema_version: u32,
    pub evidence_id: String,
    pub lab_run_id: String,
    pub created_at: DateTime<Utc>,
    pub kind: LabEvidenceKind,
    pub role: LabRole,
    pub reference: String,
    pub summary: String,
    pub artifact_id: Option<String>,
    pub cycle_id: Option<String>,
    pub metadata_hash: Option<String>,
    pub estimated_summary_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabCompressionAction {
    None,
    Recommend,
    Required,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabCompressionDecision {
    pub schema_version: u32,
    pub decision_id: String,
    pub lab_run_id: String,
    pub created_at: DateTime<Utc>,
    pub role: LabRole,
    pub action: LabCompressionAction,
    pub reason: String,
    pub context_budget_tokens: u64,
    pub packet_tokens: u64,
    pub usage_ratio_percent: f64,
    pub stable_prefix_fingerprint: String,
    pub dynamic_tail_fingerprint: String,
    pub cycle_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryBudget {
    pub max_cycle_retries: u32,
    pub max_graduate_retries_per_task: u32,
    pub max_validation_retries_per_slice: u32,
}

impl Default for RetryBudget {
    fn default() -> Self {
        Self {
            max_cycle_retries: 2,
            max_graduate_retries_per_task: 2,
            max_validation_retries_per_slice: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabCloseoutStatus {
    CompletedVerified,
    CompletedNotVerified,
    Partial,
    BlockedNeedsUser,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabRun {
    pub schema_version: u32,
    pub lab_run_id: String,
    pub proposal_id: Option<String>,
    pub kind: LabRunKind,
    pub status: LabRunStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub user_goal: String,
    pub project_root: String,
    pub user_session_id: Option<String>,
    pub top_level_mode: String,
    pub user_visible_role: LabRole,
    pub current_stage: String,
    pub internal_owner: LabRole,
    pub needs_user: bool,
    pub pause_reason: Option<String>,
    pub paused_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub lease_id: Option<String>,
    pub lease_owner: Option<String>,
    pub lease_ttl_seconds: u64,
    pub resume_cursor: ResumeCursor,
    pub roles: LabRoles,
    pub cost_policy: LabCostPolicy,
    pub artifact_ids: Vec<String>,
    pub cycle_count: u64,
    pub failure_count: u64,
    pub retry_budget: RetryBudget,
    pub meeting_ids: Vec<String>,
    pub open_task_ids: Vec<String>,
    pub blocked_reason: Option<String>,
    pub closeout_status: Option<LabCloseoutStatus>,
}

impl LabRun {
    pub fn from_proposal(lab_run_id: String, proposal: &LabProposal, now: DateTime<Utc>) -> Self {
        Self {
            schema_version: LAB_SCHEMA_VERSION,
            lab_run_id,
            proposal_id: Some(proposal.proposal_id.clone()),
            kind: LabRunKind::ArchitecturePlan,
            status: LabRunStatus::Created,
            created_at: now,
            updated_at: now,
            user_goal: proposal.user_goal.clone(),
            project_root: proposal.project_root.clone(),
            user_session_id: proposal.user_session_id.clone(),
            top_level_mode: "lab".to_string(),
            user_visible_role: LabRole::Professor,
            current_stage: "professor_discussion".to_string(),
            internal_owner: LabRole::Professor,
            needs_user: false,
            pause_reason: None,
            paused_at: None,
            heartbeat_at: Some(now),
            lease_id: None,
            lease_owner: None,
            lease_ttl_seconds: DEFAULT_LEASE_TTL_SECONDS,
            resume_cursor: ResumeCursor {
                current_stage: "professor_discussion".to_string(),
                ..ResumeCursor::default()
            },
            roles: LabRoles::default(),
            cost_policy: LabCostPolicy::default(),
            artifact_ids: Vec::new(),
            cycle_count: 0,
            failure_count: 0,
            retry_budget: RetryBudget::default(),
            meeting_ids: Vec::new(),
            open_task_ids: Vec::new(),
            blocked_reason: None,
            closeout_status: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabRunIndex {
    pub schema_version: u32,
    pub project_root: String,
    pub generated_at: DateTime<Utc>,
    pub entries: Vec<LabRunIndexEntry>,
}

impl LabRunIndex {
    pub fn new(project_root: String, generated_at: DateTime<Utc>) -> Self {
        Self {
            schema_version: LAB_SCHEMA_VERSION,
            project_root,
            generated_at,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabRunIndexEntry {
    pub schema_version: u32,
    pub lab_run_id: String,
    pub proposal_id: Option<String>,
    pub status: LabRunStatus,
    pub current_stage: String,
    pub internal_owner: LabRole,
    pub needs_user: bool,
    pub cycle_count: u64,
    pub failure_count: u64,
    pub artifact_count: usize,
    pub open_task_count: usize,
    pub meeting_count: usize,
    pub blocked_reason: Option<String>,
    pub closeout_status: Option<LabCloseoutStatus>,
    pub pause_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LabRunIndexEntry {
    pub fn from_run(run: &LabRun) -> Self {
        Self {
            schema_version: LAB_SCHEMA_VERSION,
            lab_run_id: run.lab_run_id.clone(),
            proposal_id: run.proposal_id.clone(),
            status: run.status,
            current_stage: run.current_stage.clone(),
            internal_owner: run.internal_owner,
            needs_user: run.needs_user,
            cycle_count: run.cycle_count,
            failure_count: run.failure_count,
            artifact_count: run.artifact_ids.len(),
            open_task_count: run.open_task_ids.len(),
            meeting_count: run.meeting_ids.len(),
            blocked_reason: run.blocked_reason.clone(),
            closeout_status: run.closeout_status,
            pause_reason: run.pause_reason.clone(),
            created_at: run.created_at,
            updated_at: run.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabLease {
    pub schema_version: u32,
    pub lease_id: String,
    pub lab_run_id: String,
    pub lease_owner: String,
    pub lease_acquired_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
    pub lease_ttl_seconds: u64,
}

impl LabLease {
    pub fn is_stale_at(&self, now: DateTime<Utc>) -> bool {
        let elapsed = now.signed_duration_since(self.heartbeat_at);
        elapsed.num_seconds() > self.lease_ttl_seconds as i64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SponsorMessageType {
    Question,
    Concern,
    Correction,
    ScopeChange,
    PauseRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SponsorMessageStatus {
    Queued,
    Reviewed,
    ConvertedToTask,
    ConvertedToMeeting,
    Applied,
    Rejected,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SponsorMessage {
    pub schema_version: u32,
    pub message_id: String,
    pub lab_run_id: String,
    pub created_at: DateTime<Utc>,
    pub message_type: SponsorMessageType,
    pub body: String,
    pub urgency: String,
    pub status: SponsorMessageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabEvent {
    pub schema_version: u32,
    pub event_id: String,
    pub lab_run_id: Option<String>,
    pub proposal_id: Option<String>,
    pub event_type: String,
    pub created_at: DateTime<Utc>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabArtifactStatus {
    Draft,
    ReadyForHandoff,
    Accepted,
    NeedsRevision,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LabArtifactType {
    ProfessorPlan,
    PostdocPlan,
    GraduateResult,
    PostdocIntegrationSummary,
    ProfessorReview,
    CycleSummary,
    CompressionSummary,
    LabMeetingRequest,
    LabMeetingSummary,
    LabBlockerReport,
    LabRevisionTask,
    ProfessorSteeringDecision,
}

impl LabArtifactType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProfessorPlan => "ProfessorPlan",
            Self::PostdocPlan => "PostdocPlan",
            Self::GraduateResult => "GraduateResult",
            Self::PostdocIntegrationSummary => "PostdocIntegrationSummary",
            Self::ProfessorReview => "ProfessorReview",
            Self::CycleSummary => "CycleSummary",
            Self::CompressionSummary => "CompressionSummary",
            Self::LabMeetingRequest => "LabMeetingRequest",
            Self::LabMeetingSummary => "LabMeetingSummary",
            Self::LabBlockerReport => "LabBlockerReport",
            Self::LabRevisionTask => "LabRevisionTask",
            Self::ProfessorSteeringDecision => "ProfessorSteeringDecision",
        }
    }

    pub fn stage(self) -> &'static str {
        match self {
            Self::ProfessorPlan => "professor_discussion",
            Self::PostdocPlan => "postdoc_plan",
            Self::GraduateResult => "graduate_work",
            Self::PostdocIntegrationSummary => "postdoc_review",
            Self::ProfessorReview => "professor_review",
            Self::CycleSummary => "cycle_summary",
            Self::CompressionSummary => "compression_summary",
            Self::LabMeetingRequest => "lab_meeting_request",
            Self::LabMeetingSummary => "lab_meeting",
            Self::LabBlockerReport => "blocker_report",
            Self::LabRevisionTask => "postdoc_revision",
            Self::ProfessorSteeringDecision => "professor_steering",
        }
    }

    pub fn owner(self) -> LabRole {
        match self {
            Self::ProfessorPlan
            | Self::ProfessorReview
            | Self::ProfessorSteeringDecision
            | Self::LabMeetingRequest => LabRole::Professor,
            Self::PostdocPlan | Self::PostdocIntegrationSummary => LabRole::Postdoc,
            Self::GraduateResult => LabRole::Graduate,
            Self::CycleSummary => LabRole::Runtime,
            Self::CompressionSummary => LabRole::Runtime,
            Self::LabMeetingSummary => LabRole::Runtime,
            Self::LabBlockerReport => LabRole::Postdoc,
            Self::LabRevisionTask => LabRole::Professor,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabArtifactEnvelope<T> {
    pub schema_version: u32,
    pub artifact_id: String,
    pub lab_run_id: String,
    pub artifact_type: LabArtifactType,
    pub stage: String,
    pub owner: LabRole,
    pub status: LabArtifactStatus,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub evidence_refs: Vec<String>,
    pub validation_status: Option<String>,
    pub body: T,
}

impl<T> LabArtifactEnvelope<T> {
    pub fn new(
        artifact_id: String,
        lab_run_id: String,
        artifact_type: LabArtifactType,
        title: String,
        now: DateTime<Utc>,
        body: T,
    ) -> Self {
        Self {
            schema_version: LAB_SCHEMA_VERSION,
            artifact_id,
            lab_run_id,
            artifact_type,
            stage: artifact_type.stage().to_string(),
            owner: artifact_type.owner(),
            status: LabArtifactStatus::ReadyForHandoff,
            title,
            created_at: now,
            updated_at: now,
            evidence_refs: Vec::new(),
            validation_status: Some("not_verified".to_string()),
            body,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfessorPlan {
    pub problem_statement: String,
    pub strategic_direction: String,
    pub success_criteria: Vec<String>,
    pub constraints: Vec<String>,
    pub risks: Vec<String>,
    pub handoff_to_postdoc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostdocPlan {
    pub implementation_summary: String,
    pub slices: Vec<String>,
    pub files_expected: Vec<String>,
    pub validation_plan: Vec<String>,
    pub graduate_handoff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraduateResult {
    pub task_summary: String,
    pub changed_files: Vec<String>,
    pub validation_attempts: Vec<String>,
    pub blockers: Vec<String>,
    pub handoff_to_postdoc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostdocIntegrationSummary {
    pub integration_summary: String,
    pub accepted_results: Vec<String>,
    pub validation_status: String,
    pub remaining_risks: Vec<String>,
    pub handoff_to_professor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfessorReview {
    pub review_summary: String,
    pub strategic_assessment: String,
    pub accepted: bool,
    pub required_revisions: Vec<String>,
    pub user_report: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfessorSteeringDecision {
    pub decision_id: String,
    pub source_message_id: String,
    pub decision: String,
    pub status: SponsorMessageStatus,
    pub message_type: SponsorMessageType,
    pub urgency: String,
    pub rationale: String,
    pub next_action: String,
    pub message_summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabCycleSummary {
    pub cycle_id: String,
    pub current_stage: String,
    pub owner: LabRole,
    pub summary: String,
    pub completed_items: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub total_tokens: u64,
    pub cache_hit_rate_percent: f64,
    pub estimated_cost_usd: f64,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabCompressionSummary {
    pub decision_id: String,
    pub role: LabRole,
    pub action: LabCompressionAction,
    pub reason: String,
    pub before_tokens: u64,
    pub target_budget_tokens: u64,
    pub usage_ratio_percent: f64,
    pub stable_prefix_fingerprint: String,
    pub dynamic_tail_fingerprint: String,
    pub retained_layers: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub compressed_summary: String,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabMeetingSummary {
    pub meeting_id: String,
    pub topic: String,
    pub current_stage: String,
    pub professor_view: String,
    pub postdoc_view: String,
    pub decision: String,
    pub next_actions: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub total_tokens: u64,
    pub cache_hit_rate_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabMeetingRequest {
    pub request_id: String,
    pub topic: String,
    pub current_stage: String,
    pub reason: String,
    pub signals: Vec<String>,
    pub requested_by: LabRole,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabBlockerReport {
    pub blocker_id: String,
    pub current_stage: String,
    pub summary: String,
    pub blocked_tasks: Vec<String>,
    pub failed_dispatches: Vec<String>,
    pub failure_count: u64,
    pub recommendation: String,
    pub handoff_to_professor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabRevisionTask {
    pub revision_id: String,
    pub source_review_artifact_id: String,
    pub assigned_role: LabRole,
    pub summary: String,
    pub required_revisions: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "artifact_type", content = "artifact")]
pub enum StageArtifact {
    ProfessorPlan(LabArtifactEnvelope<ProfessorPlan>),
    PostdocPlan(LabArtifactEnvelope<PostdocPlan>),
    GraduateResult(LabArtifactEnvelope<GraduateResult>),
    PostdocIntegrationSummary(LabArtifactEnvelope<PostdocIntegrationSummary>),
    ProfessorReview(LabArtifactEnvelope<ProfessorReview>),
    CycleSummary(LabArtifactEnvelope<LabCycleSummary>),
    CompressionSummary(LabArtifactEnvelope<LabCompressionSummary>),
    LabMeetingRequest(LabArtifactEnvelope<LabMeetingRequest>),
    LabMeetingSummary(LabArtifactEnvelope<LabMeetingSummary>),
    LabBlockerReport(LabArtifactEnvelope<LabBlockerReport>),
    LabRevisionTask(LabArtifactEnvelope<LabRevisionTask>),
    ProfessorSteeringDecision(LabArtifactEnvelope<ProfessorSteeringDecision>),
}

impl StageArtifact {
    pub fn artifact_id(&self) -> &str {
        match self {
            Self::ProfessorPlan(artifact) => &artifact.artifact_id,
            Self::PostdocPlan(artifact) => &artifact.artifact_id,
            Self::GraduateResult(artifact) => &artifact.artifact_id,
            Self::PostdocIntegrationSummary(artifact) => &artifact.artifact_id,
            Self::ProfessorReview(artifact) => &artifact.artifact_id,
            Self::CycleSummary(artifact) => &artifact.artifact_id,
            Self::CompressionSummary(artifact) => &artifact.artifact_id,
            Self::LabMeetingRequest(artifact) => &artifact.artifact_id,
            Self::LabMeetingSummary(artifact) => &artifact.artifact_id,
            Self::LabBlockerReport(artifact) => &artifact.artifact_id,
            Self::LabRevisionTask(artifact) => &artifact.artifact_id,
            Self::ProfessorSteeringDecision(artifact) => &artifact.artifact_id,
        }
    }

    pub fn lab_run_id(&self) -> &str {
        match self {
            Self::ProfessorPlan(artifact) => &artifact.lab_run_id,
            Self::PostdocPlan(artifact) => &artifact.lab_run_id,
            Self::GraduateResult(artifact) => &artifact.lab_run_id,
            Self::PostdocIntegrationSummary(artifact) => &artifact.lab_run_id,
            Self::ProfessorReview(artifact) => &artifact.lab_run_id,
            Self::CycleSummary(artifact) => &artifact.lab_run_id,
            Self::CompressionSummary(artifact) => &artifact.lab_run_id,
            Self::LabMeetingRequest(artifact) => &artifact.lab_run_id,
            Self::LabMeetingSummary(artifact) => &artifact.lab_run_id,
            Self::LabBlockerReport(artifact) => &artifact.lab_run_id,
            Self::LabRevisionTask(artifact) => &artifact.lab_run_id,
            Self::ProfessorSteeringDecision(artifact) => &artifact.lab_run_id,
        }
    }

    pub fn stage(&self) -> &str {
        match self {
            Self::ProfessorPlan(artifact) => &artifact.stage,
            Self::PostdocPlan(artifact) => &artifact.stage,
            Self::GraduateResult(artifact) => &artifact.stage,
            Self::PostdocIntegrationSummary(artifact) => &artifact.stage,
            Self::ProfessorReview(artifact) => &artifact.stage,
            Self::CycleSummary(artifact) => &artifact.stage,
            Self::CompressionSummary(artifact) => &artifact.stage,
            Self::LabMeetingRequest(artifact) => &artifact.stage,
            Self::LabMeetingSummary(artifact) => &artifact.stage,
            Self::LabBlockerReport(artifact) => &artifact.stage,
            Self::LabRevisionTask(artifact) => &artifact.stage,
            Self::ProfessorSteeringDecision(artifact) => &artifact.stage,
        }
    }

    pub fn artifact_type(&self) -> LabArtifactType {
        match self {
            Self::ProfessorPlan(_) => LabArtifactType::ProfessorPlan,
            Self::PostdocPlan(_) => LabArtifactType::PostdocPlan,
            Self::GraduateResult(_) => LabArtifactType::GraduateResult,
            Self::PostdocIntegrationSummary(_) => LabArtifactType::PostdocIntegrationSummary,
            Self::ProfessorReview(_) => LabArtifactType::ProfessorReview,
            Self::CycleSummary(_) => LabArtifactType::CycleSummary,
            Self::CompressionSummary(_) => LabArtifactType::CompressionSummary,
            Self::LabMeetingRequest(_) => LabArtifactType::LabMeetingRequest,
            Self::LabMeetingSummary(_) => LabArtifactType::LabMeetingSummary,
            Self::LabBlockerReport(_) => LabArtifactType::LabBlockerReport,
            Self::LabRevisionTask(_) => LabArtifactType::LabRevisionTask,
            Self::ProfessorSteeringDecision(_) => LabArtifactType::ProfessorSteeringDecision,
        }
    }

    pub fn validation_status(&self) -> Option<&str> {
        match self {
            Self::ProfessorPlan(artifact) => artifact.validation_status.as_deref(),
            Self::PostdocPlan(artifact) => artifact.validation_status.as_deref(),
            Self::GraduateResult(artifact) => artifact.validation_status.as_deref(),
            Self::PostdocIntegrationSummary(artifact) => artifact.validation_status.as_deref(),
            Self::ProfessorReview(artifact) => artifact.validation_status.as_deref(),
            Self::CycleSummary(artifact) => artifact.validation_status.as_deref(),
            Self::CompressionSummary(artifact) => artifact.validation_status.as_deref(),
            Self::LabMeetingRequest(artifact) => artifact.validation_status.as_deref(),
            Self::LabMeetingSummary(artifact) => artifact.validation_status.as_deref(),
            Self::LabBlockerReport(artifact) => artifact.validation_status.as_deref(),
            Self::LabRevisionTask(artifact) => artifact.validation_status.as_deref(),
            Self::ProfessorSteeringDecision(artifact) => artifact.validation_status.as_deref(),
        }
    }

    pub fn evidence_refs(&self) -> &[String] {
        match self {
            Self::ProfessorPlan(artifact) => &artifact.evidence_refs,
            Self::PostdocPlan(artifact) => &artifact.evidence_refs,
            Self::GraduateResult(artifact) => &artifact.evidence_refs,
            Self::PostdocIntegrationSummary(artifact) => &artifact.evidence_refs,
            Self::ProfessorReview(artifact) => &artifact.evidence_refs,
            Self::CycleSummary(artifact) => &artifact.evidence_refs,
            Self::CompressionSummary(artifact) => &artifact.evidence_refs,
            Self::LabMeetingRequest(artifact) => &artifact.evidence_refs,
            Self::LabMeetingSummary(artifact) => &artifact.evidence_refs,
            Self::LabBlockerReport(artifact) => &artifact.evidence_refs,
            Self::LabRevisionTask(artifact) => &artifact.evidence_refs,
            Self::ProfessorSteeringDecision(artifact) => &artifact.evidence_refs,
        }
    }

    pub fn status(&self) -> LabArtifactStatus {
        match self {
            Self::ProfessorPlan(artifact) => artifact.status,
            Self::PostdocPlan(artifact) => artifact.status,
            Self::GraduateResult(artifact) => artifact.status,
            Self::PostdocIntegrationSummary(artifact) => artifact.status,
            Self::ProfessorReview(artifact) => artifact.status,
            Self::CycleSummary(artifact) => artifact.status,
            Self::CompressionSummary(artifact) => artifact.status,
            Self::LabMeetingRequest(artifact) => artifact.status,
            Self::LabMeetingSummary(artifact) => artifact.status,
            Self::LabBlockerReport(artifact) => artifact.status,
            Self::LabRevisionTask(artifact) => artifact.status,
            Self::ProfessorSteeringDecision(artifact) => artifact.status,
        }
    }

    pub fn owner(&self) -> LabRole {
        match self {
            Self::ProfessorPlan(artifact) => artifact.owner,
            Self::PostdocPlan(artifact) => artifact.owner,
            Self::GraduateResult(artifact) => artifact.owner,
            Self::PostdocIntegrationSummary(artifact) => artifact.owner,
            Self::ProfessorReview(artifact) => artifact.owner,
            Self::CycleSummary(artifact) => artifact.owner,
            Self::CompressionSummary(artifact) => artifact.owner,
            Self::LabMeetingRequest(artifact) => artifact.owner,
            Self::LabMeetingSummary(artifact) => artifact.owner,
            Self::LabBlockerReport(artifact) => artifact.owner,
            Self::LabRevisionTask(artifact) => artifact.owner,
            Self::ProfessorSteeringDecision(artifact) => artifact.owner,
        }
    }

    pub fn set_review_state(
        &mut self,
        status: LabArtifactStatus,
        validation_status: Option<String>,
    ) {
        match self {
            Self::ProfessorPlan(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::PostdocPlan(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::GraduateResult(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::PostdocIntegrationSummary(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::ProfessorReview(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::CycleSummary(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::CompressionSummary(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::LabMeetingRequest(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::LabMeetingSummary(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::LabBlockerReport(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::LabRevisionTask(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
            Self::ProfessorSteeringDecision(artifact) => {
                artifact.status = status;
                artifact.validation_status = validation_status;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactGate {
    pub schema_version: u32,
    pub stage: String,
    pub required_artifact_type: String,
    pub owner: LabRole,
    pub artifact_id: Option<String>,
    pub evidence_refs: Vec<String>,
    pub validation_status: Option<String>,
    pub blockers: Vec<String>,
    pub next_action: Option<String>,
}

impl ArtifactGate {
    pub fn new(stage: impl Into<String>, artifact_type: impl Into<String>, owner: LabRole) -> Self {
        Self {
            schema_version: LAB_SCHEMA_VERSION,
            stage: stage.into(),
            required_artifact_type: artifact_type.into(),
            owner,
            artifact_id: None,
            evidence_refs: Vec::new(),
            validation_status: None,
            blockers: Vec::new(),
            next_action: None,
        }
    }

    pub fn missing_fields(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.artifact_id.as_deref().unwrap_or("").trim().is_empty() {
            missing.push("artifact_id");
        }
        if self.next_action.as_deref().unwrap_or("").trim().is_empty() {
            missing.push("next_action");
        }
        if self.evidence_refs.is_empty() && self.validation_status.is_none() {
            missing.push("evidence_refs_or_validation_status");
        }
        missing
    }

    pub fn is_satisfied(&self) -> bool {
        self.missing_fields().is_empty()
            && self.blockers.is_empty()
            && self.validation_status.as_deref() != Some("needs_revision")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_run_from_proposal_sets_professor_intake_boundary() {
        let now = Utc::now();
        let proposal = LabProposal::new(
            "labproposal_test".to_string(),
            "/tmp/project".to_string(),
            Some("session_test".to_string()),
            "Build LabRun".to_string(),
            now,
        );

        let run = LabRun::from_proposal("labrun_test".to_string(), &proposal, now);

        assert_eq!(run.proposal_id.as_deref(), Some("labproposal_test"));
        assert_eq!(run.status, LabRunStatus::Created);
        assert_eq!(run.current_stage, "professor_discussion");
        assert_eq!(run.internal_owner, LabRole::Professor);
        assert_eq!(run.roles.professor.prompt_version, "lab-professor.v1");
        assert_eq!(run.cost_policy.evidence_default, "refs_only");
    }

    #[test]
    fn artifact_gate_requires_handoff_proof_fields() {
        let mut gate = ArtifactGate::new(
            "postdoc_review",
            "PostdocIntegrationSummary",
            LabRole::Postdoc,
        );
        assert!(!gate.is_satisfied());
        assert_eq!(
            gate.missing_fields(),
            vec![
                "artifact_id",
                "next_action",
                "evidence_refs_or_validation_status"
            ]
        );

        gate.artifact_id = Some("artifact_postdoc_summary_001".to_string());
        gate.next_action = Some("professor_review".to_string());
        gate.validation_status = Some("not_verified".to_string());

        assert!(gate.is_satisfied());
    }
}
