//! LabRun stage artifact model types.
//!
//! Stage artifacts are the handoff objects between Professor, Postdoc, and
//! Graduate roles. They are persisted by `LabStore`, rendered into reports, and
//! used by `LabOrchestrator` to decide whether a stage can advance.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// State set for lab artifact status in the LabRun workflow.
pub enum LabArtifactStatus {
    Draft,
    ReadyForHandoff,
    Accepted,
    NeedsRevision,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// State set for lab artifact type in the LabRun workflow.
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
    /// Entry point for as str.
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

    /// Entry point for stage.
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

    /// Entry point for owner.
    pub fn owner(self) -> LabRole {
        match self {
            Self::ProfessorPlan | Self::ProfessorReview | Self::ProfessorSteeringDecision => {
                LabRole::Professor
            }
            Self::PostdocPlan | Self::PostdocIntegrationSummary => LabRole::Postdoc,
            Self::GraduateResult => LabRole::Graduate,
            Self::CycleSummary => LabRole::Runtime,
            Self::CompressionSummary => LabRole::Runtime,
            Self::LabMeetingRequest | Self::LabMeetingSummary => LabRole::Runtime,
            Self::LabBlockerReport => LabRole::Postdoc,
            Self::LabRevisionTask => LabRole::Professor,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data model for lab artifact envelope in LabRun persistence or orchestration.
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
    /// Entry point for new.
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
/// Data model for professor plan in LabRun persistence or orchestration.
pub struct ProfessorPlan {
    pub problem_statement: String,
    pub strategic_direction: String,
    pub success_criteria: Vec<String>,
    pub constraints: Vec<String>,
    pub risks: Vec<String>,
    pub handoff_to_postdoc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data model for postdoc plan in LabRun persistence or orchestration.
pub struct PostdocPlan {
    pub implementation_summary: String,
    pub slices: Vec<String>,
    pub files_expected: Vec<String>,
    pub validation_plan: Vec<String>,
    pub graduate_handoff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data model for graduate result in LabRun persistence or orchestration.
pub struct GraduateResult {
    pub task_summary: String,
    pub changed_files: Vec<String>,
    pub validation_attempts: Vec<String>,
    pub blockers: Vec<String>,
    pub handoff_to_postdoc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data model for postdoc integration summary in LabRun persistence or orchestration.
pub struct PostdocIntegrationSummary {
    pub integration_summary: String,
    pub accepted_results: Vec<String>,
    pub validation_status: String,
    pub remaining_risks: Vec<String>,
    pub handoff_to_professor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data model for professor review in LabRun persistence or orchestration.
pub struct ProfessorReview {
    pub review_summary: String,
    pub strategic_assessment: String,
    pub accepted: bool,
    pub required_revisions: Vec<String>,
    pub user_report: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data model for professor steering decision in LabRun persistence or orchestration.
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
/// Data model for lab cycle summary in LabRun persistence or orchestration.
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
/// Data model for lab compression summary in LabRun persistence or orchestration.
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
/// Data model for lab meeting summary in LabRun persistence or orchestration.
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
/// Data model for lab meeting request in LabRun persistence or orchestration.
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
/// Data model for lab blocker report in LabRun persistence or orchestration.
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
/// Data model for lab revision task in LabRun persistence or orchestration.
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
/// State set for stage artifact in the LabRun workflow.
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
    /// Entry point for artifact id.
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

    /// Entry point for lab run id.
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

    /// Entry point for stage.
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

    /// Entry point for artifact type.
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

    /// Entry point for validation status.
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

    /// Entry point for evidence refs.
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

    /// Entry point for status.
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

    /// Entry point for owner.
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

    /// Entry point for set review state.
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
/// Data model for artifact gate in LabRun persistence or orchestration.
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
    /// Entry point for new.
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

    /// Entry point for missing fields.
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

    /// Entry point for is satisfied.
    pub fn is_satisfied(&self) -> bool {
        self.missing_fields().is_empty()
            && self.blockers.is_empty()
            && self.validation_status.as_deref() != Some("needs_revision")
    }
}
