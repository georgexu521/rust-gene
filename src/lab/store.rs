use crate::lab::delegation::GraduateTaskDispatch;
use crate::lab::model::{
    default_daemon_max_steps_per_cycle, ArtifactGate, GraduateCleanupStatus,
    GraduateDispatchRecord, GraduateDispatchStatus, GraduateTask, LabAppLifecycleState,
    LabArtifactStatus, LabCloseoutStatus, LabCompressionDecision, LabCostSummary, LabCostUsage,
    LabDaemonMode, LabDaemonState, LabEvent, LabEvidenceKind, LabEvidenceRef, LabLease,
    LabProposal, LabProposalIntakeDraft, LabProposalStatus, LabProviderCertificationKind,
    LabProviderCertificationOutcome, LabProviderCertificationRecord, LabRole, LabRun, LabRunIndex,
    LabRunIndexEntry, LabRunStatus, LabSchedulerState, LabSchedulerStatus, LabTaskStatus,
    LabValidationRetry, SponsorMessage, SponsorMessageStatus, SponsorMessageType, StageArtifact,
    LAB_SCHEMA_VERSION,
};
use crate::lab::report::render_stage_artifact_markdown;
use anyhow::{anyhow, Context};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

mod artifacts;
mod graduate_tasks;
mod internal;
mod lifecycle;
mod metrics;
mod proposal_run;
mod runtime;

/// Data model for lab cost tokens in LabRun persistence or orchestration.
pub use runtime::LabCostTokens;
use runtime::*;

#[derive(Debug, Clone)]
/// File and sqlite-backed persistence boundary for LabRun state.
pub struct LabStore {
    project_root: PathBuf,
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data model for lab sqlite index summary in LabRun persistence or orchestration.
pub struct LabSqliteIndexSummary {
    pub path: PathBuf,
    pub lab_runs: usize,
    pub lab_artifacts: usize,
    pub lab_events: usize,
    pub lab_tasks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data model for lab sqlite artifact summary in LabRun persistence or orchestration.
pub struct LabSqliteArtifactSummary {
    pub artifact_id: String,
    pub artifact_type: String,
    pub stage: String,
    pub status: String,
    pub validation_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data model for lab sqlite dashboard summary in LabRun persistence or orchestration.
pub struct LabSqliteDashboardSummary {
    pub index: LabSqliteIndexSummary,
    pub latest_professor_artifact: Option<LabSqliteArtifactSummary>,
    pub latest_postdoc_artifact: Option<LabSqliteArtifactSummary>,
}

/// Input object for recording a LabRun evidence reference.
pub struct LabEvidenceRefInput<'a> {
    pub lab_run_id: &'a str,
    pub kind: LabEvidenceKind,
    pub role: LabRole,
    pub reference: &'a str,
    pub summary: &'a str,
    pub artifact_id: Option<&'a str>,
    pub cycle_id: Option<&'a str>,
}

impl LabStore {
    /// Entry point for for project.
    pub fn for_project(project_root: impl AsRef<Path>) -> Self {
        let project_root = project_root.as_ref().to_path_buf();
        let root = project_root.join(".priority-agent").join("lab");
        Self { project_root, root }
    }

    /// Entry point for root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Entry point for project root.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}

#[cfg(test)]
mod tests;
