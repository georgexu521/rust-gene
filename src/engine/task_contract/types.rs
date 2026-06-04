//! Task contract types — enums, base structs, TaskContract, ContextPack, ExecutionReport.
//!
//! 从 `task_contract/mod.rs` 拆分出来的类型定义和基础实现。

use crate::engine::intent_router::RiskLevel;
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
