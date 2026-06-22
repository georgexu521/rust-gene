use crate::lab::context::{
    build_lab_context_packet_with_evidence_retries_and_artifact_refs,
    evaluate_lab_context_compression, LabContextEvidenceRefGroup,
};
use crate::lab::delegation::{
    build_graduate_task_dispatch, execute_graduate_task_with_agent_tool, graduate_agent_task_id,
};
use crate::lab::model::{
    ArtifactGate, GraduateDispatchRecord, GraduateDispatchStatus, GraduateResult, GraduateTask,
    LabArtifactEnvelope, LabArtifactStatus, LabArtifactType, LabBlockerReport, LabCloseoutStatus,
    LabCompressionAction, LabCompressionDecision, LabCompressionSummary, LabCycleSummary, LabEvent,
    LabMeetingRequest, LabMeetingSummary, LabRevisionTask, LabRole, LabRun, LabRunStatus,
    LabTaskStatus, PostdocIntegrationSummary, PostdocPlan, ProfessorPlan, ProfessorReview,
    ProfessorSteeringDecision, SponsorMessageStatus, SponsorMessageType, StageArtifact,
};
use crate::lab::store::LabStore;
use crate::tools::ToolContext;
use anyhow::anyhow;
use chrono::Utc;
use serde_json::Value;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

mod collaboration;
mod graduate;
mod runtime;
mod scheduler_flow;
mod stage_flow;

use runtime::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Static transition rule between two LabRun stages.
pub struct StageTransition {
    pub from_stage: &'static str,
    pub required_artifact_type: &'static str,
    pub required_owner: LabRole,
    pub to_stage: &'static str,
    pub next_owner: LabRole,
    pub next_action: &'static str,
}

const STAGE_TRANSITIONS: &[StageTransition] = &[
    StageTransition {
        from_stage: "professor_discussion",
        required_artifact_type: "ProfessorPlan",
        required_owner: LabRole::Professor,
        to_stage: "postdoc_plan",
        next_owner: LabRole::Postdoc,
        next_action: "postdoc_plan",
    },
    StageTransition {
        from_stage: "postdoc_plan",
        required_artifact_type: "PostdocPlan",
        required_owner: LabRole::Postdoc,
        to_stage: "graduate_work",
        next_owner: LabRole::Graduate,
        next_action: "graduate_work",
    },
    StageTransition {
        from_stage: "graduate_work",
        required_artifact_type: "GraduateResult",
        required_owner: LabRole::Graduate,
        to_stage: "postdoc_review",
        next_owner: LabRole::Postdoc,
        next_action: "postdoc_review",
    },
    StageTransition {
        from_stage: "postdoc_review",
        required_artifact_type: "PostdocIntegrationSummary",
        required_owner: LabRole::Postdoc,
        to_stage: "professor_review",
        next_owner: LabRole::Professor,
        next_action: "professor_review",
    },
    StageTransition {
        from_stage: "professor_review",
        required_artifact_type: "ProfessorReview",
        required_owner: LabRole::Professor,
        to_stage: "user_report",
        next_owner: LabRole::Professor,
        next_action: "user_report",
    },
];

#[derive(Debug, Clone)]
/// Coordinates LabRun stage transitions, runtime checks, and artifact gates.
pub struct LabOrchestrator {
    store: LabStore,
}

#[derive(Debug, Clone)]
/// Persisted artifact plus its report and validation gate.
pub struct CreatedStageArtifact {
    pub artifact: StageArtifact,
    pub path: PathBuf,
    pub report_path: PathBuf,
    pub gate: ArtifactGate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Read-only recommendation for opening a professor/postdoc meeting.
pub struct LabMeetingRecommendation {
    pub lab_run_id: String,
    pub recommended: bool,
    pub topic: String,
    pub reason: String,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Outcome class for one deterministic LabRun tick.
pub enum LabTickStatus {
    Advanced,
    Blocked,
    NeedsUser,
}

#[derive(Debug, Clone)]
/// Result of one deterministic LabRun tick.
pub struct LabTickResult {
    pub lab_run_id: String,
    pub status: LabTickStatus,
    pub from_stage: String,
    pub to_stage: String,
    pub owner: LabRole,
    pub artifact_id: Option<String>,
    pub report_path: Option<PathBuf>,
    pub compression_artifact_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Action selected by one LabRun scheduler step.
pub enum LabSchedulerStepAction {
    TickAdvanced,
    GraduateDispatched,
    NeedsUser,
    Blocked,
}

#[derive(Debug, Clone)]
/// Observed result of one LabRun scheduler step.
pub struct LabSchedulerStepResult {
    pub lab_run_id: String,
    pub action: LabSchedulerStepAction,
    pub stage: String,
    pub task_id: Option<String>,
    pub dispatch_id: Option<String>,
    pub message: String,
}

impl LabOrchestrator {
    /// Entry point for for project.
    pub fn for_project(project_root: impl AsRef<Path>) -> Self {
        Self {
            store: LabStore::for_project(project_root),
        }
    }

    /// Entry point for store.
    pub fn store(&self) -> &LabStore {
        &self.store
    }
}

#[cfg(test)]
mod tests;
