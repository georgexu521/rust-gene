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

#[derive(Debug, Clone)]
pub struct LabStore {
    project_root: PathBuf,
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabSqliteIndexSummary {
    pub path: PathBuf,
    pub lab_runs: usize,
    pub lab_artifacts: usize,
    pub lab_events: usize,
    pub lab_tasks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabSqliteArtifactSummary {
    pub artifact_id: String,
    pub artifact_type: String,
    pub stage: String,
    pub status: String,
    pub validation_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabSqliteDashboardSummary {
    pub index: LabSqliteIndexSummary,
    pub latest_professor_artifact: Option<LabSqliteArtifactSummary>,
    pub latest_postdoc_artifact: Option<LabSqliteArtifactSummary>,
}

impl LabStore {
    pub fn for_project(project_root: impl AsRef<Path>) -> Self {
        let project_root = project_root.as_ref().to_path_buf();
        let root = project_root.join(".priority-agent").join("lab");
        Self { project_root, root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn create_proposal(
        &self,
        user_goal: &str,
        user_session_id: Option<String>,
    ) -> anyhow::Result<LabProposal> {
        self.create_proposal_with_intake(
            user_goal,
            user_session_id,
            LabProposalIntakeDraft::from_goal(user_goal),
            "proposal_created",
        )
    }

    pub fn create_proposal_with_intake(
        &self,
        user_goal: &str,
        user_session_id: Option<String>,
        intake: LabProposalIntakeDraft,
        event_type: &str,
    ) -> anyhow::Result<LabProposal> {
        let trimmed = user_goal.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("proposal goal cannot be empty"));
        }
        let event_type = event_type.trim();
        if event_type.is_empty() {
            return Err(anyhow!("proposal event type cannot be empty"));
        }

        let now = Utc::now();
        let proposal_id = next_id("labproposal");
        let project_root = self.project_root.display().to_string();
        let mut proposal = LabProposal::new(
            proposal_id.clone(),
            project_root,
            user_session_id,
            trimmed.to_string(),
            now,
        );
        proposal.apply_intake_draft(intake);
        let dir = self.proposal_dir(&proposal_id);
        fs::create_dir_all(&dir)?;
        atomic_write_json(&dir.join("proposal.json"), &proposal)?;
        self.append_project_event(LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: None,
            proposal_id: Some(proposal_id),
            event_type: event_type.to_string(),
            created_at: now,
            payload: serde_json::json!({ "user_goal": trimmed }),
        })?;
        Ok(proposal)
    }

    pub fn load_proposal(&self, proposal_id: &str) -> anyhow::Result<LabProposal> {
        read_json(&self.proposal_dir(proposal_id).join("proposal.json"))
    }

    pub fn approve_proposal(&self, proposal_id: &str) -> anyhow::Result<LabRun> {
        let mut proposal = self.load_proposal(proposal_id)?;
        if matches!(proposal.status, LabProposalStatus::Approved) {
            if let Some(existing) = proposal.approval.created_lab_run_id.as_deref() {
                return self.load_run(existing);
            }
        }

        let now = Utc::now();
        let lab_run_id = next_id("labrun");
        self.ensure_no_foreign_fresh_lease(None, now)?;
        proposal.status = LabProposalStatus::Approved;
        proposal.updated_at = now;
        proposal.approval.approved_by_user = true;
        proposal.approval.approved_at = Some(now);
        proposal.approval.created_lab_run_id = Some(lab_run_id.clone());

        let mut run = LabRun::from_proposal(lab_run_id.clone(), &proposal, now);
        run.status = LabRunStatus::Active;
        let lease = self.acquire_lease_for_run(&mut run, now)?;
        let proposal_dir = self.proposal_dir(proposal_id);
        let run_dir = self.run_dir(&lab_run_id);
        fs::create_dir_all(&proposal_dir)?;
        fs::create_dir_all(&run_dir)?;
        atomic_write_json(&proposal_dir.join("proposal.json"), &proposal)?;
        atomic_write_json(&run_dir.join("state.json"), &run)?;
        atomic_write_json(&run_dir.join("lease.json"), &lease)?;
        self.append_run_event(
            &lab_run_id,
            "labrun_created",
            serde_json::json!({
                "proposal_id": proposal_id,
                "user_goal": run.user_goal,
                "lease_id": lease.lease_id,
            }),
        )?;
        self.refresh_runs_index_entry(&run)?;
        self.write_active_run_pointer(&lab_run_id)?;
        Ok(run)
    }

    pub fn load_run(&self, lab_run_id: &str) -> anyhow::Result<LabRun> {
        read_json(&self.run_dir(lab_run_id).join("state.json"))
    }

    pub fn save_run(&self, run: &LabRun) -> anyhow::Result<()> {
        let dir = self.run_dir(&run.lab_run_id);
        fs::create_dir_all(&dir)?;
        atomic_write_json(&dir.join("state.json"), run)?;
        self.refresh_runs_index_entry(run)
    }

    pub fn record_run_event(
        &self,
        lab_run_id: &str,
        event_type: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.append_run_event(lab_run_id, event_type, payload)
    }

    pub fn list_run_events(&self, lab_run_id: &str) -> anyhow::Result<Vec<LabEvent>> {
        self.read_run_events(lab_run_id)
    }

    pub fn record_lab_failure(
        &self,
        lab_run_id: &str,
        source: &str,
        reason: &str,
    ) -> anyhow::Result<LabRun> {
        let source = source.trim();
        let reason = reason.trim();
        if source.is_empty() {
            return Err(anyhow!("lab failure source cannot be empty"));
        }
        if reason.is_empty() {
            return Err(anyhow!("lab failure reason cannot be empty"));
        }
        let mut run = self.load_run(lab_run_id)?;
        run.failure_count = run.failure_count.saturating_add(1);
        run.updated_at = Utc::now();
        let failure_budget = run.retry_budget.max_cycle_retries.max(1) as u64;
        if run.failure_count >= failure_budget {
            run.status = LabRunStatus::NeedsUser;
            run.needs_user = true;
            run.blocked_reason = Some(format!("failure budget reached after {source}: {reason}"));
            run.closeout_status = Some(LabCloseoutStatus::BlockedNeedsUser);
        }
        self.save_run(&run)?;
        self.append_run_event(
            lab_run_id,
            "lab_failure_recorded",
            serde_json::json!({
                "source": source,
                "reason": reason,
                "failure_count": run.failure_count,
                "failure_budget": failure_budget,
                "needs_user": run.needs_user,
                "blocked_reason": run.blocked_reason,
            }),
        )?;
        Ok(run)
    }

    pub fn latest_run(&self) -> anyhow::Result<Option<LabRun>> {
        if let Some(id) = self.read_active_run_pointer()? {
            if let Ok(run) = self.load_run(&id) {
                return Ok(Some(run));
            }
        }

        let mut runs = self.list_runs()?;
        Ok(runs.pop())
    }

    pub fn list_runs(&self) -> anyhow::Result<Vec<LabRun>> {
        let runs_dir = self.runs_dir();
        if !runs_dir.exists() {
            return Ok(Vec::new());
        }
        let mut runs = Vec::new();
        for entry in fs::read_dir(runs_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Ok(run) = read_json::<LabRun>(&entry.path().join("state.json")) {
                    runs.push(run);
                }
            }
        }
        runs.sort_by_key(|run| run.updated_at);
        Ok(runs)
    }

    pub fn load_runs_index(&self) -> anyhow::Result<Option<LabRunIndex>> {
        let path = self.runs_index_path();
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    pub fn rebuild_runs_index(&self) -> anyhow::Result<LabRunIndex> {
        let mut index = LabRunIndex::new(self.project_root.display().to_string(), Utc::now());
        index.entries = self
            .list_runs()?
            .iter()
            .map(LabRunIndexEntry::from_run)
            .collect();
        index.entries.sort_by_key(|entry| entry.updated_at);
        atomic_write_json(&self.runs_index_path(), &index)?;
        let _summary = self.rebuild_sqlite_index()?;
        Ok(index)
    }

    pub fn sqlite_index_path(&self) -> PathBuf {
        self.root.join("lab_index.sqlite3")
    }

    pub fn rebuild_sqlite_index(&self) -> anyhow::Result<LabSqliteIndexSummary> {
        fs::create_dir_all(&self.root)?;
        let path = self.sqlite_index_path();
        let mut conn = Connection::open(&path)
            .with_context(|| format!("failed to open Lab SQLite index {}", path.display()))?;
        self.ensure_sqlite_schema(&conn)?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM lab_runs", [])?;
        tx.execute("DELETE FROM lab_artifacts", [])?;
        tx.execute("DELETE FROM lab_events", [])?;
        tx.execute("DELETE FROM lab_tasks", [])?;

        let mut lab_runs = 0usize;
        let mut lab_artifacts = 0usize;
        let mut lab_events = 0usize;
        let mut lab_tasks = 0usize;
        for run in self.list_runs()? {
            let entry = LabRunIndexEntry::from_run(&run);
            tx.execute(
                "INSERT OR REPLACE INTO lab_runs (
                    lab_run_id, schema_version, project_root, proposal_id, status,
                    current_stage, internal_owner, needs_user, cycle_count, failure_count,
                    artifact_count, open_task_count, meeting_count, blocked_reason,
                    closeout_status, pause_reason, created_at, updated_at, state_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    entry.lab_run_id,
                    entry.schema_version as i64,
                    self.project_root.display().to_string(),
                    entry.proposal_id,
                    enum_json(&entry.status)?,
                    entry.current_stage,
                    enum_json(&entry.internal_owner)?,
                    if entry.needs_user { 1_i64 } else { 0_i64 },
                    entry.cycle_count as i64,
                    entry.failure_count as i64,
                    entry.artifact_count as i64,
                    entry.open_task_count as i64,
                    entry.meeting_count as i64,
                    entry.blocked_reason,
                    optional_enum_json(entry.closeout_status.as_ref())?,
                    entry.pause_reason,
                    entry.created_at.to_rfc3339(),
                    entry.updated_at.to_rfc3339(),
                    serde_json::to_string(&run)?,
                ],
            )?;
            lab_runs += 1;

            for artifact in self.list_stage_artifacts(&run.lab_run_id)? {
                tx.execute(
                    "INSERT OR REPLACE INTO lab_artifacts (
                        artifact_id, lab_run_id, artifact_type, stage, status,
                        validation_status, artifact_json
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        artifact.artifact_id(),
                        artifact.lab_run_id(),
                        artifact.artifact_type().as_str(),
                        artifact.stage(),
                        enum_json(&artifact.status())?,
                        artifact.validation_status(),
                        serde_json::to_string(&artifact)?,
                    ],
                )?;
                lab_artifacts += 1;
            }

            for task in self.list_graduate_tasks(&run.lab_run_id)? {
                tx.execute(
                    "INSERT OR REPLACE INTO lab_tasks (
                        task_id, lab_run_id, status, title, assigned_role, created_at,
                        updated_at, result_artifact_id, blocker, task_json
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        task.task_id,
                        task.lab_run_id,
                        enum_json(&task.status)?,
                        task.title,
                        enum_json(&task.assigned_role)?,
                        task.created_at.to_rfc3339(),
                        task.updated_at.to_rfc3339(),
                        task.result_artifact_id,
                        task.blocker,
                        serde_json::to_string(&task)?,
                    ],
                )?;
                lab_tasks += 1;
            }

            for event in self.read_run_events(&run.lab_run_id)? {
                tx.execute(
                    "INSERT OR REPLACE INTO lab_events (
                        event_id, lab_run_id, proposal_id, event_type, created_at, payload_json
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        event.event_id,
                        event.lab_run_id,
                        event.proposal_id,
                        event.event_type,
                        event.created_at.to_rfc3339(),
                        serde_json::to_string(&event.payload)?,
                    ],
                )?;
                lab_events += 1;
            }
        }
        tx.commit()?;
        Ok(LabSqliteIndexSummary {
            path,
            lab_runs,
            lab_artifacts,
            lab_events,
            lab_tasks,
        })
    }

    pub fn load_sqlite_index_summary(&self) -> anyhow::Result<Option<LabSqliteIndexSummary>> {
        let path = self.sqlite_index_path();
        if !path.exists() {
            return Ok(None);
        }
        let conn = Connection::open(&path)
            .with_context(|| format!("failed to open Lab SQLite index {}", path.display()))?;
        self.ensure_sqlite_schema(&conn)?;
        Ok(Some(LabSqliteIndexSummary {
            path,
            lab_runs: sqlite_count(&conn, "lab_runs")?,
            lab_artifacts: sqlite_count(&conn, "lab_artifacts")?,
            lab_events: sqlite_count(&conn, "lab_events")?,
            lab_tasks: sqlite_count(&conn, "lab_tasks")?,
        }))
    }

    pub fn load_sqlite_dashboard_summary(
        &self,
        lab_run_id: &str,
    ) -> anyhow::Result<Option<LabSqliteDashboardSummary>> {
        let path = self.sqlite_index_path();
        if !path.exists() {
            return Ok(None);
        }
        let conn = Connection::open(&path)
            .with_context(|| format!("failed to open Lab SQLite index {}", path.display()))?;
        self.ensure_sqlite_schema(&conn)?;
        let index = LabSqliteIndexSummary {
            path,
            lab_runs: sqlite_count(&conn, "lab_runs")?,
            lab_artifacts: sqlite_count(&conn, "lab_artifacts")?,
            lab_events: sqlite_count(&conn, "lab_events")?,
            lab_tasks: sqlite_count(&conn, "lab_tasks")?,
        };
        Ok(Some(LabSqliteDashboardSummary {
            latest_professor_artifact: latest_sqlite_artifact_for_role(
                &conn,
                lab_run_id,
                &[
                    "ProfessorPlan",
                    "ProfessorReview",
                    "ProfessorSteeringDecision",
                ],
            )?,
            latest_postdoc_artifact: latest_sqlite_artifact_for_role(
                &conn,
                lab_run_id,
                &[
                    "PostdocPlan",
                    "PostdocIntegrationSummary",
                    "LabRevisionTask",
                    "LabBlockerReport",
                ],
            )?,
            index,
        }))
    }

    pub fn latest_proposal(&self) -> anyhow::Result<Option<LabProposal>> {
        let proposals_dir = self.proposals_dir();
        if !proposals_dir.exists() {
            return Ok(None);
        }
        let mut proposals = Vec::new();
        for entry in fs::read_dir(proposals_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Ok(proposal) = read_json::<LabProposal>(&entry.path().join("proposal.json"))
                {
                    proposals.push(proposal);
                }
            }
        }
        proposals.sort_by_key(|proposal| proposal.updated_at);
        Ok(proposals.pop())
    }

    pub fn pause_latest_run(&self, reason: &str) -> anyhow::Result<LabRun> {
        let mut run = self
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found to pause"))?;
        let now = Utc::now();
        run.status = LabRunStatus::Paused;
        run.pause_reason = Some(reason.trim().to_string());
        run.paused_at = Some(now);
        run.lease_id = None;
        run.lease_owner = None;
        run.updated_at = now;
        self.save_run(&run)?;
        self.release_lease_for_run(&run.lab_run_id)?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_paused",
            serde_json::json!({
                "pause_reason": run.pause_reason,
            }),
        )?;
        Ok(run)
    }

    pub fn closeout_latest_run(
        &self,
        closeout_status: LabCloseoutStatus,
        note: &str,
    ) -> anyhow::Result<LabRun> {
        let mut run = self
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found to close out"))?;
        let now = Utc::now();
        let note = note.trim();
        run.status = match closeout_status {
            LabCloseoutStatus::CompletedVerified
            | LabCloseoutStatus::CompletedNotVerified
            | LabCloseoutStatus::Partial => LabRunStatus::Completed,
            LabCloseoutStatus::BlockedNeedsUser => LabRunStatus::NeedsUser,
            LabCloseoutStatus::Cancelled => LabRunStatus::Cancelled,
            LabCloseoutStatus::Failed => LabRunStatus::Failed,
        };
        run.closeout_status = Some(closeout_status);
        run.needs_user = matches!(closeout_status, LabCloseoutStatus::BlockedNeedsUser);
        run.blocked_reason = run.needs_user.then(|| {
            if note.is_empty() {
                "LabRun closeout requires user input".to_string()
            } else {
                note.to_string()
            }
        });
        run.pause_reason = None;
        run.paused_at = None;
        run.lease_id = None;
        run.lease_owner = None;
        run.updated_at = now;
        self.save_run(&run)?;
        self.release_lease_for_run(&run.lab_run_id)?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_closeout_recorded",
            serde_json::json!({
                "closeout_status": closeout_status,
                "run_status": run.status,
                "note": note,
                "needs_user": run.needs_user,
                "blocked_reason": run.blocked_reason,
            }),
        )?;
        Ok(run)
    }

    pub fn pause_latest_run_for_shutdown(&self) -> anyhow::Result<Option<LabRun>> {
        let Some(mut run) = self.latest_run()? else {
            return Ok(None);
        };
        if !matches!(run.status, LabRunStatus::Active) {
            return Ok(None);
        }
        if let Some(lease) = self.read_active_lease()? {
            if lease.lab_run_id != run.lab_run_id {
                return Ok(None);
            }
            if lease.lease_owner != lease_owner() {
                return Ok(None);
            }
        }

        let now = Utc::now();
        run.status = LabRunStatus::PausedShutdown;
        run.pause_reason = Some("app_shutdown".to_string());
        run.paused_at = Some(now);
        run.lease_id = None;
        run.lease_owner = None;
        run.updated_at = now;
        self.save_run(&run)?;
        self.release_lease_for_run(&run.lab_run_id)?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_shutdown_paused",
            serde_json::json!({
                "pause_reason": run.pause_reason,
                "paused_at": now,
            }),
        )?;
        Ok(Some(run))
    }

    pub fn resume_latest_run(&self) -> anyhow::Result<LabRun> {
        let mut run = self
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found to resume"))?;
        let now = Utc::now();
        self.ensure_no_foreign_fresh_lease(Some(&run.lab_run_id), now)?;
        let lease = self.acquire_lease_for_run(&mut run, now)?;
        run.status = LabRunStatus::Active;
        run.pause_reason = None;
        run.paused_at = None;
        run.updated_at = now;
        self.save_run(&run)?;
        self.write_active_run_pointer(&run.lab_run_id)?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_resumed",
            serde_json::json!({ "lease_id": lease.lease_id }),
        )?;
        Ok(run)
    }

    pub fn ensure_current_process_holds_fresh_lease(&self, run: &LabRun) -> anyhow::Result<()> {
        let Some(lease) = self.read_active_lease()? else {
            return Err(anyhow!(
                "LabRun {} cannot schedule work because the active lease is missing; resume the LabRun before continuing",
                run.lab_run_id
            ));
        };
        if lease.lab_run_id != run.lab_run_id {
            return Err(anyhow!(
                "LabRun {} cannot schedule work because active lease belongs to {} ({})",
                run.lab_run_id,
                lease.lab_run_id,
                lease.lease_owner
            ));
        }
        if lease.lease_owner != lease_owner() {
            return Err(anyhow!(
                "LabRun {} cannot schedule work because active lease is held by {}",
                run.lab_run_id,
                lease.lease_owner
            ));
        }
        if lease.is_stale_at(Utc::now()) {
            return Err(anyhow!(
                "LabRun {} cannot schedule work because active lease is stale; recover or resume before continuing",
                run.lab_run_id
            ));
        }
        if run.lease_id.as_deref() != Some(lease.lease_id.as_str()) {
            return Err(anyhow!(
                "LabRun {} cannot schedule work because run state lease does not match active lease",
                run.lab_run_id
            ));
        }
        Ok(())
    }

    pub fn claim_latest_active_run_for_current_process(&self) -> anyhow::Result<Option<LabRun>> {
        let Some(mut run) = self.latest_run()? else {
            return Ok(None);
        };
        if !matches!(run.status, LabRunStatus::Active) {
            return Ok(None);
        }
        let now = Utc::now();
        if let Some(lease) = self.read_active_lease()? {
            if lease.lab_run_id != run.lab_run_id {
                if lease.is_stale_at(now) {
                    self.release_lease_for_run(&lease.lab_run_id)?;
                }
                return Ok(None);
            }
            if !lease.is_stale_at(now) && lease.lease_owner != lease_owner() {
                return Ok(None);
            }
            if lease.is_stale_at(now) {
                self.release_lease_for_run(&lease.lab_run_id)?;
                self.append_run_event(
                    &run.lab_run_id,
                    "lab_command_stale_lease_claimed",
                    serde_json::json!({
                        "previous_lease_id": lease.lease_id,
                        "previous_lease_owner": lease.lease_owner,
                        "heartbeat_at": lease.heartbeat_at,
                        "claimed_at": now,
                    }),
                )?;
            }
        }

        let lease = self.acquire_lease_for_run(&mut run, now)?;
        run.updated_at = now;
        self.save_run(&run)?;
        self.write_active_run_pointer(&run.lab_run_id)?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_command_lease_claimed",
            serde_json::json!({
                "lease_id": lease.lease_id,
                "lease_owner": lease.lease_owner,
            }),
        )?;
        Ok(Some(run))
    }

    pub fn release_current_process_lease_without_pausing(
        &self,
    ) -> anyhow::Result<Option<LabLease>> {
        let Some(lease) = self.read_active_lease()? else {
            return Ok(None);
        };
        if lease.lease_owner != lease_owner() {
            return Ok(None);
        }
        let mut run = match self.load_run(&lease.lab_run_id) {
            Ok(run) => run,
            Err(_) => {
                self.release_lease_for_run(&lease.lab_run_id)?;
                return Ok(Some(lease));
            }
        };
        run.lease_id = None;
        run.lease_owner = None;
        run.updated_at = Utc::now();
        self.save_run(&run)?;
        self.release_lease_for_run(&run.lab_run_id)?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_command_lease_released",
            serde_json::json!({
                "lease_id": lease.lease_id,
                "lease_owner": lease.lease_owner,
            }),
        )?;
        Ok(Some(lease))
    }

    pub fn open_run_pointer(&self, lab_run_id: &str) -> anyhow::Result<LabRun> {
        let lab_run_id = lab_run_id.trim();
        if lab_run_id.is_empty() {
            return Err(anyhow!("lab_run_id cannot be empty"));
        }
        let run = self.load_run(lab_run_id)?;
        if let Some(lease) = self.read_active_lease()? {
            if lease.lab_run_id != run.lab_run_id && !lease.is_stale_at(Utc::now()) {
                return Err(anyhow!(
                    "cannot open LabRun {}; active lease is held by {} ({})",
                    run.lab_run_id,
                    lease.lab_run_id,
                    lease.lease_owner
                ));
            }
        }
        self.write_active_run_pointer(&run.lab_run_id)?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_run_opened",
            serde_json::json!({
                "opened_as_active_pointer": true,
                "status": run.status,
                "stage": run.current_stage,
            }),
        )?;
        Ok(run)
    }

    pub fn recover_stale_active_lease(&self) -> anyhow::Result<Option<LabLease>> {
        let now = Utc::now();
        let Some(lease) = self.read_active_lease()? else {
            return Ok(None);
        };
        if !lease.is_stale_at(now) {
            return Ok(None);
        }

        self.release_lease_for_run(&lease.lab_run_id)?;
        if let Ok(mut run) = self.load_run(&lease.lab_run_id) {
            run.lease_id = None;
            run.lease_owner = None;
            run.status = LabRunStatus::PausedShutdown;
            run.pause_reason = Some("stale_heartbeat".to_string());
            run.paused_at = Some(now);
            run.updated_at = now;
            self.save_run(&run)?;
            self.write_active_run_pointer(&run.lab_run_id)?;
            self.append_run_event(
                &run.lab_run_id,
                "lab_stale_lease_recovered",
                serde_json::json!({
                    "lease_id": lease.lease_id,
                    "lease_owner": lease.lease_owner,
                    "heartbeat_at": lease.heartbeat_at,
                    "recovered_at": now,
                }),
            )?;
        }
        Ok(Some(lease))
    }

    pub fn refresh_latest_run_heartbeat(&self) -> anyhow::Result<Option<LabLease>> {
        let Some(mut run) = self.latest_run()? else {
            return Ok(None);
        };
        if !matches!(run.status, LabRunStatus::Active) {
            return Ok(None);
        }
        let Some(mut lease) = self.read_active_lease()? else {
            return Ok(None);
        };
        if lease.lab_run_id != run.lab_run_id || lease.lease_owner != lease_owner() {
            return Ok(None);
        }
        let now = Utc::now();
        lease.heartbeat_at = now;
        run.heartbeat_at = Some(now);
        run.updated_at = now;
        atomic_write_json(&self.active_lease_path(), &lease)?;
        atomic_write_json(&self.run_dir(&run.lab_run_id).join("lease.json"), &lease)?;
        self.save_run(&run)?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_heartbeat",
            serde_json::json!({
                "lease_id": lease.lease_id,
                "heartbeat_at": lease.heartbeat_at,
            }),
        )?;
        Ok(Some(lease))
    }

    pub fn create_graduate_task(
        &self,
        lab_run_id: &str,
        title: &str,
        instructions: &str,
        allowed_scope: Vec<String>,
        required_validation: Vec<String>,
    ) -> anyhow::Result<GraduateTask> {
        let title = title.trim();
        if title.is_empty() {
            return Err(anyhow!("graduate task title cannot be empty"));
        }
        let instructions = instructions.trim();
        if instructions.is_empty() {
            return Err(anyhow!("graduate task instructions cannot be empty"));
        }
        let mut run = self.load_run(lab_run_id)?;
        let now = Utc::now();
        let task = GraduateTask {
            schema_version: LAB_SCHEMA_VERSION,
            task_id: next_id("gradtask"),
            lab_run_id: lab_run_id.to_string(),
            created_at: now,
            updated_at: now,
            created_by: LabRole::Postdoc,
            assigned_role: LabRole::Graduate,
            status: LabTaskStatus::Queued,
            title: title.to_string(),
            instructions: instructions.to_string(),
            allowed_scope: clean_string_vec(allowed_scope),
            required_validation: clean_string_vec(required_validation),
            evidence_ids: Vec::new(),
            result_artifact_id: None,
            blocker: None,
            cycle_id: Some(run.cycle_count.to_string()),
        };
        self.write_graduate_task(&task)?;
        sync_open_task(&mut run, &task);
        run.updated_at = now;
        self.save_run(&run)?;
        self.append_run_event(
            lab_run_id,
            "graduate_task_created",
            serde_json::json!({
                "task_id": &task.task_id,
                "title": &task.title,
                "allowed_scope": &task.allowed_scope,
                "required_validation": &task.required_validation,
            }),
        )?;
        Ok(task)
    }

    pub fn load_graduate_task(
        &self,
        lab_run_id: &str,
        task_id: &str,
    ) -> anyhow::Result<GraduateTask> {
        read_json(&self.task_path(lab_run_id, task_id))
    }

    pub fn list_graduate_tasks(&self, lab_run_id: &str) -> anyhow::Result<Vec<GraduateTask>> {
        let dir = self.task_dir(lab_run_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut tasks = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Ok(task) = read_json::<GraduateTask>(&entry.path()) {
                    tasks.push(task);
                }
            }
        }
        tasks.sort_by_key(|task| task.created_at);
        Ok(tasks)
    }

    pub fn latest_graduate_tasks(&self) -> anyhow::Result<Vec<GraduateTask>> {
        let Some(run) = self.latest_run()? else {
            return Ok(Vec::new());
        };
        self.list_graduate_tasks(&run.lab_run_id)
    }

    pub fn start_graduate_task(
        &self,
        lab_run_id: &str,
        task_id: &str,
    ) -> anyhow::Result<GraduateTask> {
        let mut task = self.load_graduate_task(lab_run_id, task_id)?;
        if !matches!(task.status, LabTaskStatus::Queued | LabTaskStatus::Blocked) {
            return Err(anyhow!(
                "graduate task {} cannot start from status {:?}",
                task.task_id,
                task.status
            ));
        }
        task.status = LabTaskStatus::InProgress;
        task.blocker = None;
        self.save_graduate_task_and_sync_run(task, "graduate_task_started")
    }

    pub fn complete_graduate_task(
        &self,
        lab_run_id: &str,
        task_id: &str,
        result_artifact_id: &str,
        evidence_ids: Vec<String>,
    ) -> anyhow::Result<GraduateTask> {
        let result_artifact_id = result_artifact_id.trim();
        if result_artifact_id.is_empty() {
            return Err(anyhow!("graduate task result_artifact_id cannot be empty"));
        }
        let mut task = self.load_graduate_task(lab_run_id, task_id)?;
        if !matches!(
            task.status,
            LabTaskStatus::Queued | LabTaskStatus::InProgress | LabTaskStatus::Blocked
        ) {
            return Err(anyhow!(
                "graduate task {} cannot complete from status {:?}",
                task.task_id,
                task.status
            ));
        }
        task.status = LabTaskStatus::Completed;
        task.result_artifact_id = Some(result_artifact_id.to_string());
        task.evidence_ids = clean_string_vec(evidence_ids);
        task.blocker = None;
        self.save_graduate_task_and_sync_run(task, "graduate_task_completed")
    }

    pub fn block_graduate_task(
        &self,
        lab_run_id: &str,
        task_id: &str,
        blocker: &str,
    ) -> anyhow::Result<GraduateTask> {
        let blocker = blocker.trim();
        if blocker.is_empty() {
            return Err(anyhow!("graduate task blocker cannot be empty"));
        }
        let mut task = self.load_graduate_task(lab_run_id, task_id)?;
        if !task.status.is_open() {
            return Err(anyhow!(
                "graduate task {} cannot block from status {:?}",
                task.task_id,
                task.status
            ));
        }
        task.status = LabTaskStatus::Blocked;
        task.blocker = Some(blocker.to_string());
        self.save_graduate_task_and_sync_run(task, "graduate_task_blocked")
    }

    pub fn revise_graduate_task(
        &self,
        lab_run_id: &str,
        task_id: &str,
        allowed_scope: Vec<String>,
        required_validation: Vec<String>,
        instructions: Option<&str>,
    ) -> anyhow::Result<GraduateTask> {
        let mut task = self.load_graduate_task(lab_run_id, task_id)?;
        if !task.status.is_open() {
            return Err(anyhow!(
                "graduate task {} cannot revise from status {:?}",
                task.task_id,
                task.status
            ));
        }

        task.allowed_scope = clean_string_vec(allowed_scope);
        task.required_validation = clean_string_vec(required_validation);
        if let Some(instructions) = instructions
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            task.instructions = format!(
                "{}\n\nPostdoc revision:\n{}",
                task.instructions.trim(),
                instructions
            );
        }

        if task.allowed_scope.is_empty() || task.required_validation.is_empty() {
            task.status = LabTaskStatus::Blocked;
            task.blocker = Some(
                if task.allowed_scope.is_empty() && task.required_validation.is_empty() {
                    "Graduate task revision is missing allowed_scope and required_validation."
                        .to_string()
                } else if task.allowed_scope.is_empty() {
                    "Graduate task revision is missing allowed_scope.".to_string()
                } else {
                    "Graduate task revision is missing required_validation.".to_string()
                },
            );
        } else {
            task.status = LabTaskStatus::Queued;
            task.blocker = None;
        }

        self.save_graduate_task_and_sync_run(task, "graduate_task_revised")
    }

    pub fn cancel_graduate_task(
        &self,
        lab_run_id: &str,
        task_id: &str,
        reason: Option<&str>,
    ) -> anyhow::Result<GraduateTask> {
        let mut task = self.load_graduate_task(lab_run_id, task_id)?;
        if !task.status.is_open() {
            return Err(anyhow!(
                "graduate task {} cannot cancel from status {:?}",
                task.task_id,
                task.status
            ));
        }
        task.status = LabTaskStatus::Cancelled;
        task.blocker = reason
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        self.save_graduate_task_and_sync_run(task, "graduate_task_cancelled")
    }

    pub fn record_graduate_dispatch(
        &self,
        lab_run_id: &str,
        task_id: &str,
        dispatch: GraduateTaskDispatch,
    ) -> anyhow::Result<GraduateDispatchRecord> {
        let _task = self.load_graduate_task(lab_run_id, task_id)?;
        let now = Utc::now();
        let record = GraduateDispatchRecord {
            schema_version: LAB_SCHEMA_VERSION,
            dispatch_id: next_id("graddispatch"),
            lab_run_id: lab_run_id.to_string(),
            task_id: task_id.to_string(),
            created_at: now,
            updated_at: now,
            status: GraduateDispatchStatus::Prepared,
            envelope: dispatch.envelope,
            agent_tool_params: dispatch.agent_tool_params,
            agent_id: None,
            result_artifact_id: None,
            error: None,
            cleanup_status: GraduateCleanupStatus::CleanupPending,
            cleanup_message: Some("graduate dispatch prepared; cleanup pending until worktree review/cleanup completes".to_string()),
            cleanup_updated_at: Some(now),
        };
        atomic_write_json(
            &self.dispatch_path(lab_run_id, &record.dispatch_id),
            &record,
        )?;
        self.append_run_event(
            lab_run_id,
            "graduate_dispatch_prepared",
            serde_json::json!({
                "dispatch_id": &record.dispatch_id,
                "task_id": task_id,
                "status": format!("{:?}", record.status),
                "envelope_id": &record.envelope.envelope_id,
                "cleanup_status": record.cleanup_status.as_str(),
            }),
        )?;
        Ok(record)
    }

    pub fn load_graduate_dispatch(
        &self,
        lab_run_id: &str,
        dispatch_id: &str,
    ) -> anyhow::Result<GraduateDispatchRecord> {
        read_json(&self.dispatch_path(lab_run_id, dispatch_id))
    }

    pub fn list_graduate_dispatches(
        &self,
        lab_run_id: &str,
    ) -> anyhow::Result<Vec<GraduateDispatchRecord>> {
        let dir = self.dispatch_dir(lab_run_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut dispatches = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Ok(dispatch) = read_json::<GraduateDispatchRecord>(&entry.path()) {
                    dispatches.push(dispatch);
                }
            }
        }
        dispatches.sort_by_key(|dispatch| dispatch.created_at);
        Ok(dispatches)
    }

    pub fn update_graduate_dispatch_status(
        &self,
        lab_run_id: &str,
        dispatch_id: &str,
        status: GraduateDispatchStatus,
        agent_id: Option<String>,
        result_artifact_id: Option<String>,
        error: Option<String>,
    ) -> anyhow::Result<GraduateDispatchRecord> {
        let mut record = self.load_graduate_dispatch(lab_run_id, dispatch_id)?;
        record.status = status;
        record.updated_at = Utc::now();
        if agent_id.is_some() {
            record.agent_id = agent_id;
        }
        if result_artifact_id.is_some() {
            record.result_artifact_id = result_artifact_id;
        }
        record.error = error;
        atomic_write_json(&self.dispatch_path(lab_run_id, dispatch_id), &record)?;
        self.append_run_event(
            lab_run_id,
            "graduate_dispatch_status_updated",
            serde_json::json!({
                "dispatch_id": &record.dispatch_id,
                "task_id": &record.task_id,
                "status": format!("{:?}", record.status),
                "agent_id": &record.agent_id,
                "result_artifact_id": &record.result_artifact_id,
                "error": &record.error,
            }),
        )?;
        Ok(record)
    }

    pub fn update_graduate_dispatch_cleanup_status(
        &self,
        lab_run_id: &str,
        dispatch_id: &str,
        cleanup_status: GraduateCleanupStatus,
        cleanup_message: Option<String>,
    ) -> anyhow::Result<GraduateDispatchRecord> {
        let mut record = self.load_graduate_dispatch(lab_run_id, dispatch_id)?;
        record.cleanup_status = cleanup_status;
        record.cleanup_message = cleanup_message;
        record.cleanup_updated_at = Some(Utc::now());
        record.updated_at = Utc::now();
        atomic_write_json(&self.dispatch_path(lab_run_id, dispatch_id), &record)?;
        self.append_run_event(
            lab_run_id,
            "graduate_dispatch_cleanup_updated",
            serde_json::json!({
                "dispatch_id": &record.dispatch_id,
                "task_id": &record.task_id,
                "cleanup_status": record.cleanup_status.as_str(),
                "cleanup_message": &record.cleanup_message,
            }),
        )?;
        Ok(record)
    }

    pub fn list_validation_retries(
        &self,
        lab_run_id: &str,
    ) -> anyhow::Result<Vec<LabValidationRetry>> {
        let dir = self.validation_retry_dir(lab_run_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut retries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Ok(retry) = read_json::<LabValidationRetry>(&entry.path()) {
                    retries.push(retry);
                }
            }
        }
        retries.sort_by_key(|retry| retry.created_at);
        Ok(retries)
    }

    pub fn record_validation_retry_and_repair_task(
        &self,
        lab_run_id: &str,
        task_id: &str,
        validation_summary: &str,
    ) -> anyhow::Result<LabValidationRetry> {
        let validation_summary = validation_summary.trim();
        if validation_summary.is_empty() {
            return Err(anyhow!("validation retry summary cannot be empty"));
        }
        let run = self.load_run(lab_run_id)?;
        let task = self.load_graduate_task(lab_run_id, task_id)?;
        let attempt = self
            .list_validation_retries(lab_run_id)?
            .iter()
            .filter(|retry| retry.task_id == task.task_id)
            .count() as u32
            + 1;
        let max_attempts = run.retry_budget.max_validation_retries_per_slice.max(1);
        let escalated = attempt > max_attempts;
        let mut retry = LabValidationRetry {
            schema_version: LAB_SCHEMA_VERSION,
            retry_id: next_id("validationretry"),
            lab_run_id: lab_run_id.to_string(),
            task_id: task.task_id.clone(),
            created_at: Utc::now(),
            attempt,
            validation_summary: validation_summary.to_string(),
            repair_task_id: None,
            escalated,
        };

        if escalated {
            let _ = self.block_graduate_task(lab_run_id, &task.task_id, validation_summary);
            let _ =
                self.record_lab_failure(lab_run_id, "validation_retry_budget", validation_summary);
        } else {
            let _ = self.block_graduate_task(lab_run_id, &task.task_id, validation_summary);
            let repair = self.create_graduate_task(
                lab_run_id,
                &format!("Repair validation for {}", task.title),
                &format!(
                    "Repair task {} after validation failure attempt {attempt}/{max_attempts}.\nFailure summary: {}\nOriginal instructions:\n{}",
                    task.task_id, validation_summary, task.instructions
                ),
                task.allowed_scope.clone(),
                task.required_validation.clone(),
            )?;
            retry.repair_task_id = Some(repair.task_id);
        }

        atomic_write_json(
            &self.validation_retry_path(lab_run_id, &retry.retry_id),
            &retry,
        )?;
        self.append_run_event(
            lab_run_id,
            "lab_validation_retry_recorded",
            serde_json::json!({
                "retry_id": &retry.retry_id,
                "task_id": &retry.task_id,
                "attempt": retry.attempt,
                "repair_task_id": &retry.repair_task_id,
                "escalated": retry.escalated,
                "validation_summary": &retry.validation_summary,
            }),
        )?;
        Ok(retry)
    }

    pub fn append_sponsor_message(&self, body: &str) -> anyhow::Result<SponsorMessage> {
        let run = self
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for sponsor message"))?;
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("sponsor message cannot be empty"));
        }
        let now = Utc::now();
        let message = SponsorMessage {
            schema_version: LAB_SCHEMA_VERSION,
            message_id: next_id("sponsor_msg"),
            lab_run_id: run.lab_run_id.clone(),
            created_at: now,
            message_type: SponsorMessageType::Concern,
            body: trimmed.to_string(),
            urgency: "normal".to_string(),
            status: SponsorMessageStatus::Queued,
        };
        self.append_sponsor_message_record(&run.lab_run_id, &message)?;
        self.append_run_event(
            &run.lab_run_id,
            "sponsor_message",
            serde_json::to_value(&message)?,
        )?;
        Ok(message)
    }

    pub fn intervene_latest_run(&self, body: &str) -> anyhow::Result<(LabRun, SponsorMessage)> {
        let mut run = self
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for intervention"))?;
        if matches!(
            run.status,
            LabRunStatus::Completed | LabRunStatus::Cancelled | LabRunStatus::Failed
        ) {
            return Err(anyhow!(
                "LabRun {} is terminal: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("intervention message cannot be empty"));
        }
        let now = Utc::now();
        let message = SponsorMessage {
            schema_version: LAB_SCHEMA_VERSION,
            message_id: next_id("sponsor_msg"),
            lab_run_id: run.lab_run_id.clone(),
            created_at: now,
            message_type: SponsorMessageType::PauseRequest,
            body: trimmed.to_string(),
            urgency: "high".to_string(),
            status: SponsorMessageStatus::Queued,
        };
        run.status = LabRunStatus::NeedsUser;
        run.needs_user = true;
        run.pause_reason = Some("sponsor_intervention".to_string());
        run.paused_at = Some(now);
        run.blocked_reason = Some(format!(
            "sponsor intervention queued: {}",
            trimmed.chars().take(160).collect::<String>()
        ));
        run.lease_id = None;
        run.lease_owner = None;
        run.updated_at = now;
        self.save_run(&run)?;
        self.release_lease_for_run(&run.lab_run_id)?;
        self.append_sponsor_message_record(&run.lab_run_id, &message)?;
        self.append_run_event(
            &run.lab_run_id,
            "sponsor_message",
            serde_json::to_value(&message)?,
        )?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_intervention_recorded",
            serde_json::json!({
                "message_id": message.message_id,
                "pause_reason": run.pause_reason,
                "blocked_reason": run.blocked_reason,
            }),
        )?;
        Ok((run, message))
    }

    pub fn list_sponsor_messages(&self, lab_run_id: &str) -> anyhow::Result<Vec<SponsorMessage>> {
        let path = self.run_dir(lab_run_id).join("sponsor_messages.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut messages = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            messages.push(
                serde_json::from_str::<SponsorMessage>(&line).with_context(|| {
                    format!("failed to parse sponsor message in {}", path.display())
                })?,
            );
        }
        Ok(messages)
    }

    pub fn update_latest_sponsor_message_status(
        &self,
        message_id: &str,
        status: SponsorMessageStatus,
        note: &str,
    ) -> anyhow::Result<SponsorMessage> {
        let run = self
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for sponsor message update"))?;
        let message_id = message_id.trim();
        if message_id.is_empty() {
            return Err(anyhow!("sponsor message id cannot be empty"));
        }
        let mut messages = self.list_sponsor_messages(&run.lab_run_id)?;
        let Some(message) = messages
            .iter_mut()
            .find(|message| message.message_id == message_id)
        else {
            return Err(anyhow!("sponsor message not found: {message_id}"));
        };
        message.status = status;
        let updated = message.clone();
        self.write_sponsor_messages(&run.lab_run_id, &messages)?;
        self.append_run_event(
            &run.lab_run_id,
            "sponsor_message_status_updated",
            serde_json::json!({
                "message_id": updated.message_id,
                "status": updated.status,
                "note": note.trim(),
            }),
        )?;
        Ok(updated)
    }

    pub fn write_scheduler_state(&self, state: &LabSchedulerState) -> anyhow::Result<()> {
        atomic_write_json(
            &self.run_dir(&state.lab_run_id).join("scheduler_state.json"),
            state,
        )?;
        self.append_run_event(
            &state.lab_run_id,
            "lab_scheduler_state",
            serde_json::json!({
                "status": format!("{:?}", state.status),
                "steps_completed": state.steps_completed,
                "max_steps": state.max_steps,
                "last_action": state.last_action,
                "last_message": state.last_message,
                "stop_reason": state.stop_reason,
            }),
        )
    }

    pub fn load_scheduler_state(
        &self,
        lab_run_id: &str,
    ) -> anyhow::Result<Option<LabSchedulerState>> {
        let path = self.run_dir(lab_run_id).join("scheduler_state.json");
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    pub fn recover_interrupted_scheduler(&self) -> anyhow::Result<Option<LabSchedulerState>> {
        let Some(run) = self.latest_run()? else {
            return Ok(None);
        };
        let Some(mut state) = self.load_scheduler_state(&run.lab_run_id)? else {
            return Ok(None);
        };
        if !matches!(
            state.status,
            LabSchedulerStatus::Running | LabSchedulerStatus::Stopping
        ) {
            return Ok(None);
        }

        let now = Utc::now();
        state.status = LabSchedulerStatus::PausedRestart;
        state.updated_at = now;
        state.stopped_at = Some(now);
        state.last_message = Some(
            "background scheduler was interrupted by process restart; use /lab background start to continue"
                .to_string(),
        );
        state.stop_reason = Some("process_restart".to_string());
        self.write_scheduler_state(&state)?;
        Ok(Some(state))
    }

    pub fn record_app_lifecycle_startup(
        &self,
        launch_mode: &str,
    ) -> anyhow::Result<LabAppLifecycleState> {
        self.record_app_lifecycle_startup_with_options(launch_mode, true)
    }

    pub fn record_app_lifecycle_startup_for_command(
        &self,
        launch_mode: &str,
    ) -> anyhow::Result<LabAppLifecycleState> {
        self.record_app_lifecycle_startup_with_options(launch_mode, false)
    }

    fn record_app_lifecycle_startup_with_options(
        &self,
        launch_mode: &str,
        recover_stale_lease: bool,
    ) -> anyhow::Result<LabAppLifecycleState> {
        if recover_stale_lease {
            self.recover_stale_active_lease()?;
        }
        let recovered_scheduler = self.recover_interrupted_scheduler()?;
        let now = Utc::now();
        let state = LabAppLifecycleState {
            schema_version: LAB_SCHEMA_VERSION,
            project_root: self.project_root.display().to_string(),
            launch_mode: launch_mode.trim().to_string(),
            process_id: std::process::id(),
            updated_at: now,
            last_startup_at: Some(now),
            last_shutdown_at: None,
            recovered_scheduler_lab_run_id: recovered_scheduler
                .as_ref()
                .map(|state| state.lab_run_id.clone()),
            recovered_scheduler_status: recovered_scheduler.as_ref().map(|state| state.status),
            shutdown_paused_lab_run_id: None,
            last_message: Some(match recovered_scheduler {
                Some(state) => format!(
                    "startup recovered interrupted scheduler for {} as {:?}",
                    state.lab_run_id, state.status
                ),
                None => "startup completed without interrupted scheduler recovery".to_string(),
            }),
        };
        self.write_app_lifecycle_state(&state)?;
        self.append_project_event(LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: state.recovered_scheduler_lab_run_id.clone(),
            proposal_id: None,
            event_type: "lab_app_lifecycle_startup".to_string(),
            created_at: now,
            payload: serde_json::json!({
                "launch_mode": state.launch_mode,
                "process_id": state.process_id,
                "recovered_scheduler_lab_run_id": state.recovered_scheduler_lab_run_id,
                "recovered_scheduler_status": state.recovered_scheduler_status,
            }),
        })?;
        Ok(state)
    }

    pub fn record_app_lifecycle_shutdown(
        &self,
        launch_mode: &str,
    ) -> anyhow::Result<LabAppLifecycleState> {
        let paused = self.pause_latest_run_for_shutdown()?;
        let existing = self.load_app_lifecycle_state()?;
        let now = Utc::now();
        let state = LabAppLifecycleState {
            schema_version: LAB_SCHEMA_VERSION,
            project_root: self.project_root.display().to_string(),
            launch_mode: launch_mode.trim().to_string(),
            process_id: std::process::id(),
            updated_at: now,
            last_startup_at: existing.as_ref().and_then(|state| state.last_startup_at),
            last_shutdown_at: Some(now),
            recovered_scheduler_lab_run_id: existing
                .as_ref()
                .and_then(|state| state.recovered_scheduler_lab_run_id.clone()),
            recovered_scheduler_status: existing
                .as_ref()
                .and_then(|state| state.recovered_scheduler_status),
            shutdown_paused_lab_run_id: paused.as_ref().map(|run| run.lab_run_id.clone()),
            last_message: Some(match paused {
                Some(run) => format!(
                    "shutdown paused LabRun {} at stage {}",
                    run.lab_run_id, run.current_stage
                ),
                None => "shutdown completed without active LabRun pause".to_string(),
            }),
        };
        self.write_app_lifecycle_state(&state)?;
        self.append_project_event(LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: state.shutdown_paused_lab_run_id.clone(),
            proposal_id: None,
            event_type: "lab_app_lifecycle_shutdown".to_string(),
            created_at: now,
            payload: serde_json::json!({
                "launch_mode": state.launch_mode,
                "process_id": state.process_id,
                "shutdown_paused_lab_run_id": state.shutdown_paused_lab_run_id,
            }),
        })?;
        Ok(state)
    }

    pub fn load_app_lifecycle_state(&self) -> anyhow::Result<Option<LabAppLifecycleState>> {
        let path = self.app_lifecycle_path();
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    fn write_app_lifecycle_state(&self, state: &LabAppLifecycleState) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root)?;
        atomic_write_json(&self.app_lifecycle_path(), state)
    }

    pub fn load_daemon_state(&self) -> anyhow::Result<Option<LabDaemonState>> {
        let path = self.daemon_state_path();
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    pub fn enable_daemon(
        &self,
        mode: LabDaemonMode,
        max_steps: usize,
        interval_ms: u64,
        instructions: &str,
    ) -> anyhow::Result<LabDaemonState> {
        self.enable_daemon_with_cycle_bound(
            mode,
            max_steps,
            default_daemon_max_steps_per_cycle(),
            interval_ms,
            instructions,
        )
    }

    pub fn enable_daemon_with_cycle_bound(
        &self,
        mode: LabDaemonMode,
        max_steps: usize,
        max_steps_per_cycle: usize,
        interval_ms: u64,
        instructions: &str,
    ) -> anyhow::Result<LabDaemonState> {
        let now = Utc::now();
        let max_steps = max_steps.clamp(1, 100);
        let max_steps_per_cycle = max_steps_per_cycle.clamp(1, 100);
        let interval_ms = interval_ms.clamp(100, 60_000);
        let state = LabDaemonState {
            schema_version: LAB_SCHEMA_VERSION,
            project_root: self.project_root.display().to_string(),
            enabled: true,
            mode,
            max_steps,
            max_steps_per_cycle,
            interval_ms,
            instructions: instructions.trim().to_string(),
            updated_at: now,
            last_enabled_at: Some(now),
            last_disabled_at: self
                .load_daemon_state()?
                .and_then(|state| state.last_disabled_at),
            last_started_at: None,
            last_started_lab_run_id: None,
            last_start_error: None,
            last_message: Some(
                "Lab daemon policy enabled; host process must start the scheduler loop."
                    .to_string(),
            ),
        };
        self.write_daemon_state(&state)?;
        self.append_project_event(LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: self.latest_run()?.map(|run| run.lab_run_id),
            proposal_id: None,
            event_type: "lab_daemon_enabled".to_string(),
            created_at: now,
            payload: serde_json::json!({
                "mode": state.mode,
                "max_steps": state.max_steps,
                "max_steps_per_cycle": state.max_steps_per_cycle,
                "interval_ms": state.interval_ms,
                "instructions": state.instructions,
            }),
        })?;
        Ok(state)
    }

    pub fn disable_daemon(&self, reason: &str) -> anyhow::Result<LabDaemonState> {
        let now = Utc::now();
        let previous = self.load_daemon_state()?;
        let state = LabDaemonState {
            schema_version: LAB_SCHEMA_VERSION,
            project_root: self.project_root.display().to_string(),
            enabled: false,
            mode: previous
                .as_ref()
                .map(|state| state.mode)
                .unwrap_or(LabDaemonMode::Strict),
            max_steps: previous.as_ref().map(|state| state.max_steps).unwrap_or(20),
            max_steps_per_cycle: previous
                .as_ref()
                .map(|state| state.max_steps_per_cycle)
                .unwrap_or_else(default_daemon_max_steps_per_cycle),
            interval_ms: previous
                .as_ref()
                .map(|state| state.interval_ms)
                .unwrap_or(1_000),
            instructions: previous
                .as_ref()
                .map(|state| state.instructions.clone())
                .unwrap_or_default(),
            updated_at: now,
            last_enabled_at: previous.as_ref().and_then(|state| state.last_enabled_at),
            last_disabled_at: Some(now),
            last_started_at: previous.as_ref().and_then(|state| state.last_started_at),
            last_started_lab_run_id: previous
                .as_ref()
                .and_then(|state| state.last_started_lab_run_id.clone()),
            last_start_error: previous
                .as_ref()
                .and_then(|state| state.last_start_error.clone()),
            last_message: Some(format!(
                "Lab daemon policy disabled: {}",
                note_or_default(reason, "user")
            )),
        };
        self.write_daemon_state(&state)?;
        self.append_project_event(LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: self.latest_run()?.map(|run| run.lab_run_id),
            proposal_id: None,
            event_type: "lab_daemon_disabled".to_string(),
            created_at: now,
            payload: serde_json::json!({
                "reason": reason.trim(),
            }),
        })?;
        Ok(state)
    }

    pub fn record_daemon_start_result(
        &self,
        lab_run_id: Option<&str>,
        error: Option<&str>,
    ) -> anyhow::Result<Option<LabDaemonState>> {
        let Some(mut state) = self.load_daemon_state()? else {
            return Ok(None);
        };
        let now = Utc::now();
        state.updated_at = now;
        state.last_started_at = Some(now);
        state.last_started_lab_run_id = lab_run_id.map(str::to_string);
        state.last_start_error = error
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        state.last_message = Some(match (lab_run_id, error) {
            (Some(id), None) => format!("Lab daemon execution started for {id}"),
            (_, Some(err)) => format!("Lab daemon execution failed to start: {}", err.trim()),
            _ => "Lab daemon execution start attempted without LabRun".to_string(),
        });
        self.write_daemon_state(&state)?;
        self.append_project_event(LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: state.last_started_lab_run_id.clone(),
            proposal_id: None,
            event_type: "lab_daemon_start_result".to_string(),
            created_at: now,
            payload: serde_json::json!({
                "lab_run_id": state.last_started_lab_run_id,
                "error": state.last_start_error,
                "mode": state.mode,
            }),
        })?;
        Ok(Some(state))
    }

    fn write_daemon_state(&self, state: &LabDaemonState) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root)?;
        atomic_write_json(&self.daemon_state_path(), state)
    }

    pub fn record_meeting_request(&self, topic: Option<&str>) -> anyhow::Result<LabRun> {
        let run = self
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for meeting"))?;
        self.append_run_event(
            &run.lab_run_id,
            "lab_meeting_requested",
            serde_json::json!({
                "topic": topic.unwrap_or("").trim(),
                "mutation_allowed": false,
            }),
        )?;
        Ok(run)
    }

    pub fn record_cost_usage(
        &self,
        lab_run_id: &str,
        role: LabRole,
        model: &str,
        tokens: LabCostTokens,
        estimated_cost_usd: f64,
        note: Option<&str>,
    ) -> anyhow::Result<LabCostUsage> {
        let prompt_tokens = tokens.prompt_tokens;
        let cached_tokens = tokens.cached_tokens.min(prompt_tokens);
        let cache_miss_tokens = prompt_tokens.saturating_sub(cached_tokens);
        let usage = LabCostUsage {
            schema_version: LAB_SCHEMA_VERSION,
            usage_id: next_id("labusage"),
            lab_run_id: lab_run_id.to_string(),
            created_at: Utc::now(),
            role,
            cycle_id: tokens.cycle_id,
            meeting_id: tokens.meeting_id,
            model: model.trim().to_string(),
            prompt_tokens,
            completion_tokens: tokens.completion_tokens,
            reasoning_tokens: tokens.reasoning_tokens,
            cached_tokens,
            cache_write_tokens: tokens.cache_write_tokens,
            cache_miss_tokens,
            total_tokens: prompt_tokens
                .saturating_add(tokens.completion_tokens)
                .saturating_add(tokens.reasoning_tokens),
            estimated_cost_usd: estimated_cost_usd.max(0.0),
            note: note
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        };
        let path = self.run_dir(lab_run_id).join("cost_usage.jsonl");
        append_jsonl(&path, &usage)?;
        self.append_run_event(
            lab_run_id,
            "lab_cost_usage_recorded",
            serde_json::json!({
                "usage_id": usage.usage_id,
                "role": format!("{:?}", usage.role),
                "model": usage.model,
                "total_tokens": usage.total_tokens,
                "cached_tokens": usage.cached_tokens,
                "cache_write_tokens": usage.cache_write_tokens,
                "cache_miss_tokens": usage.cache_miss_tokens,
                "estimated_cost_usd": usage.estimated_cost_usd,
            }),
        )?;
        Ok(usage)
    }

    pub fn list_cost_usage(&self, lab_run_id: &str) -> anyhow::Result<Vec<LabCostUsage>> {
        let path = self.run_dir(lab_run_id).join("cost_usage.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut usage = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            usage.push(
                serde_json::from_str::<LabCostUsage>(&line)
                    .with_context(|| format!("failed to parse cost usage in {}", path.display()))?,
            );
        }
        Ok(usage)
    }

    pub fn cost_summary(&self, lab_run_id: &str) -> anyhow::Result<LabCostSummary> {
        let mut summary = LabCostSummary::empty(lab_run_id);
        for usage in self.list_cost_usage(lab_run_id)? {
            summary.add_usage(&usage);
        }
        Ok(summary)
    }

    pub fn latest_cost_summary(&self) -> anyhow::Result<Option<LabCostSummary>> {
        let Some(run) = self.latest_run()? else {
            return Ok(None);
        };
        self.cost_summary(&run.lab_run_id).map(Some)
    }

    pub fn record_evidence_ref(
        &self,
        lab_run_id: &str,
        kind: LabEvidenceKind,
        role: LabRole,
        reference: &str,
        summary: &str,
        artifact_id: Option<&str>,
        cycle_id: Option<&str>,
    ) -> anyhow::Result<LabEvidenceRef> {
        let reference = reference.trim();
        if reference.is_empty() {
            return Err(anyhow!("evidence reference cannot be empty"));
        }
        let summary = summary.trim();
        if summary.is_empty() {
            return Err(anyhow!("evidence summary cannot be empty"));
        }

        let evidence = LabEvidenceRef {
            schema_version: LAB_SCHEMA_VERSION,
            evidence_id: next_id("labevidence"),
            lab_run_id: lab_run_id.to_string(),
            created_at: Utc::now(),
            kind,
            role,
            reference: reference.to_string(),
            summary: summary.to_string(),
            artifact_id: artifact_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            cycle_id: cycle_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            metadata_hash: evidence_metadata_hash(reference),
            estimated_summary_tokens: crate::engine::context_compressor::estimate_tokens(summary)
                as u64,
        };
        append_jsonl(
            &self.run_dir(lab_run_id).join("evidence_refs.jsonl"),
            &evidence,
        )?;
        self.append_run_event(
            lab_run_id,
            "lab_evidence_ref_recorded",
            serde_json::json!({
                "evidence_id": evidence.evidence_id,
                "kind": format!("{:?}", evidence.kind),
                "role": format!("{:?}", evidence.role),
                "reference": evidence.reference,
                "metadata_hash": evidence.metadata_hash,
            }),
        )?;
        Ok(evidence)
    }

    pub fn list_evidence_refs(&self, lab_run_id: &str) -> anyhow::Result<Vec<LabEvidenceRef>> {
        let path = self.run_dir(lab_run_id).join("evidence_refs.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut evidence = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            evidence.push(
                serde_json::from_str::<LabEvidenceRef>(&line).with_context(|| {
                    format!("failed to parse evidence ref in {}", path.display())
                })?,
            );
        }
        Ok(evidence)
    }

    pub fn latest_evidence_refs(&self) -> anyhow::Result<Vec<LabEvidenceRef>> {
        let Some(run) = self.latest_run()? else {
            return Ok(Vec::new());
        };
        self.list_evidence_refs(&run.lab_run_id)
    }

    pub fn record_provider_certification(
        &self,
        provider_id: &str,
        model: &str,
        kind: LabProviderCertificationKind,
        outcome: LabProviderCertificationOutcome,
        evidence_path: &str,
        summary: &str,
    ) -> anyhow::Result<LabProviderCertificationRecord> {
        let provider_id = provider_id.trim();
        if provider_id.is_empty() {
            return Err(anyhow!("provider_id cannot be empty"));
        }
        let model = model.trim();
        if model.is_empty() {
            return Err(anyhow!("model cannot be empty"));
        }
        let evidence_path = evidence_path.trim();
        if evidence_path.is_empty() {
            return Err(anyhow!(
                "provider certification evidence_path cannot be empty"
            ));
        }
        let summary = summary.trim();
        if summary.is_empty() {
            return Err(anyhow!("provider certification summary cannot be empty"));
        }
        let record = LabProviderCertificationRecord {
            schema_version: LAB_SCHEMA_VERSION,
            record_id: next_id("labprovidercert"),
            provider_id: provider_id.to_string(),
            model: model.to_string(),
            kind,
            outcome,
            recorded_at: Utc::now(),
            evidence_path: evidence_path.to_string(),
            summary: summary.to_string(),
        };
        append_jsonl(&self.provider_certifications_path(), &record)?;
        self.append_project_event(LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: None,
            proposal_id: None,
            event_type: "lab_provider_certification_recorded".to_string(),
            created_at: record.recorded_at,
            payload: serde_json::json!({
                "record_id": record.record_id,
                "provider_id": record.provider_id,
                "model": record.model,
                "kind": record.kind.as_str(),
                "outcome": record.outcome.as_str(),
                "evidence_path": record.evidence_path,
            }),
        })?;
        Ok(record)
    }

    pub fn list_provider_certifications(
        &self,
    ) -> anyhow::Result<Vec<LabProviderCertificationRecord>> {
        let path = self.provider_certifications_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut records = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            records.push(
                serde_json::from_str::<LabProviderCertificationRecord>(&line).with_context(
                    || {
                        format!(
                            "failed to parse provider certification in {}",
                            path.display()
                        )
                    },
                )?,
            );
        }
        Ok(records)
    }

    pub fn latest_provider_certification(
        &self,
        provider_id: &str,
        model: &str,
        kind: LabProviderCertificationKind,
    ) -> anyhow::Result<Option<LabProviderCertificationRecord>> {
        let provider_id = provider_id.trim();
        let model = model.trim();
        Ok(self
            .list_provider_certifications()?
            .into_iter()
            .filter(|record| {
                record.provider_id == provider_id && record.model == model && record.kind == kind
            })
            .max_by_key(|record| record.recorded_at))
    }

    pub fn record_compression_decision(
        &self,
        mut decision: LabCompressionDecision,
    ) -> anyhow::Result<LabCompressionDecision> {
        if decision.decision_id.trim().is_empty() {
            decision.decision_id = next_id("labcompression");
        }
        append_jsonl(
            &self
                .run_dir(&decision.lab_run_id)
                .join("compression_decisions.jsonl"),
            &decision,
        )?;
        self.append_run_event(
            &decision.lab_run_id,
            "lab_compression_decision_recorded",
            serde_json::json!({
                "decision_id": decision.decision_id,
                "role": format!("{:?}", decision.role),
                "action": format!("{:?}", decision.action),
                "packet_tokens": decision.packet_tokens,
                "context_budget_tokens": decision.context_budget_tokens,
                "usage_ratio_percent": decision.usage_ratio_percent,
                "stable_prefix_fingerprint": decision.stable_prefix_fingerprint,
                "dynamic_tail_fingerprint": decision.dynamic_tail_fingerprint,
            }),
        )?;
        Ok(decision)
    }

    pub fn list_compression_decisions(
        &self,
        lab_run_id: &str,
    ) -> anyhow::Result<Vec<LabCompressionDecision>> {
        let path = self.run_dir(lab_run_id).join("compression_decisions.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut decisions = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            decisions.push(
                serde_json::from_str::<LabCompressionDecision>(&line).with_context(|| {
                    format!("failed to parse compression decision in {}", path.display())
                })?,
            );
        }
        Ok(decisions)
    }

    pub fn write_artifact_gate(
        &self,
        lab_run_id: &str,
        gate: &ArtifactGate,
    ) -> anyhow::Result<PathBuf> {
        let dir = self.run_dir(lab_run_id).join("artifact_gates");
        fs::create_dir_all(&dir)?;
        let safe_stage = safe_path_component(&gate.stage);
        let path = dir.join(format!("{}.json", safe_stage));
        atomic_write_json(&path, gate)?;
        self.append_run_event(
            lab_run_id,
            "artifact_gate_written",
            serde_json::json!({
                "stage": gate.stage,
                "required_artifact_type": gate.required_artifact_type,
                "satisfied": gate.is_satisfied(),
            }),
        )?;
        Ok(path)
    }

    pub fn load_artifact_gate(
        &self,
        lab_run_id: &str,
        stage: &str,
    ) -> anyhow::Result<ArtifactGate> {
        read_json(
            &self
                .run_dir(lab_run_id)
                .join("artifact_gates")
                .join(format!("{}.json", safe_path_component(stage))),
        )
    }

    pub fn validate_artifact_gate(&self, lab_run_id: &str, stage: &str) -> anyhow::Result<()> {
        let gate = self.load_artifact_gate(lab_run_id, stage)?;
        let missing = gate.missing_fields();
        if !gate.blockers.is_empty() {
            return Err(anyhow!(
                "artifact gate '{}' is blocked: {}",
                stage,
                gate.blockers.join("; ")
            ));
        }
        if gate.validation_status.as_deref() == Some("needs_revision") {
            return Err(anyhow!("artifact gate '{}' needs revision", stage));
        }
        if !missing.is_empty() {
            return Err(anyhow!(
                "artifact gate '{}' is incomplete: missing {}",
                stage,
                missing.join(", ")
            ));
        }
        if gate.stage != stage {
            return Err(anyhow!(
                "artifact gate '{}' stage mismatch: gate stage is '{}'",
                stage,
                gate.stage
            ));
        }
        let artifact_id = gate.artifact_id.as_deref().unwrap_or_default();
        let artifact = self
            .load_stage_artifact(lab_run_id, artifact_id)
            .with_context(|| {
                format!(
                    "artifact gate '{}' references missing or malformed artifact '{}'",
                    stage, artifact_id
                )
            })?;
        if artifact.lab_run_id() != lab_run_id {
            return Err(anyhow!(
                "artifact gate '{}' artifact '{}' belongs to LabRun {}",
                stage,
                artifact_id,
                artifact.lab_run_id()
            ));
        }
        if artifact.stage() != gate.stage {
            return Err(anyhow!(
                "artifact gate '{}' artifact '{}' has stage '{}'",
                stage,
                artifact_id,
                artifact.stage()
            ));
        }
        if artifact.artifact_type().as_str() != gate.required_artifact_type {
            return Err(anyhow!(
                "artifact gate '{}' artifact '{}' has type {}, expected {}",
                stage,
                artifact_id,
                artifact.artifact_type().as_str(),
                gate.required_artifact_type
            ));
        }
        if artifact.owner() != gate.owner {
            return Err(anyhow!(
                "artifact gate '{}' artifact '{}' has owner {:?}, expected {:?}",
                stage,
                artifact_id,
                artifact.owner(),
                gate.owner
            ));
        }
        Ok(())
    }

    pub fn write_stage_artifact(&self, artifact: &StageArtifact) -> anyhow::Result<PathBuf> {
        let artifact_id = artifact.artifact_id().trim();
        if artifact_id.is_empty() {
            return Err(anyhow!("artifact_id cannot be empty"));
        }

        let lab_run_id = artifact.lab_run_id();
        let dir = self.run_dir(lab_run_id).join("artifacts");
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", safe_path_component(artifact_id)));
        atomic_write_json(&path, artifact)?;

        let mut run = self.load_run(lab_run_id)?;
        if !run.artifact_ids.iter().any(|id| id == artifact_id) {
            run.artifact_ids.push(artifact_id.to_string());
        }
        run.resume_cursor.active_artifact_id = Some(artifact_id.to_string());
        run.updated_at = Utc::now();
        self.save_run(&run)?;

        self.append_run_event(
            lab_run_id,
            "lab_artifact_written",
            serde_json::json!({
                "artifact_id": artifact_id,
                "artifact_type": artifact.artifact_type().as_str(),
                "stage": artifact.stage(),
                "path": path.display().to_string(),
            }),
        )?;
        Ok(path)
    }

    pub fn write_stage_artifact_report(&self, artifact: &StageArtifact) -> anyhow::Result<PathBuf> {
        let artifact_id = artifact.artifact_id().trim();
        if artifact_id.is_empty() {
            return Err(anyhow!("artifact_id cannot be empty"));
        }

        let lab_run_id = artifact.lab_run_id();
        let dir = self.run_dir(lab_run_id).join("reports");
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.md", safe_path_component(artifact_id)));
        atomic_write_text(&path, &render_stage_artifact_markdown(artifact))?;
        self.append_run_event(
            lab_run_id,
            "lab_report_written",
            serde_json::json!({
                "artifact_id": artifact_id,
                "artifact_type": artifact.artifact_type().as_str(),
                "stage": artifact.stage(),
                "path": path.display().to_string(),
            }),
        )?;
        Ok(path)
    }

    pub fn stage_artifact_report_path(&self, lab_run_id: &str, artifact_id: &str) -> PathBuf {
        self.run_dir(lab_run_id)
            .join("reports")
            .join(format!("{}.md", safe_path_component(artifact_id)))
    }

    pub fn list_stage_artifact_report_paths(
        &self,
        lab_run_id: &str,
    ) -> anyhow::Result<Vec<(String, PathBuf)>> {
        let run = self.load_run(lab_run_id)?;
        Ok(run
            .artifact_ids
            .iter()
            .map(|artifact_id| {
                (
                    artifact_id.clone(),
                    self.stage_artifact_report_path(lab_run_id, artifact_id),
                )
            })
            .filter(|(_, path)| path.exists())
            .collect())
    }

    pub fn load_stage_artifact(
        &self,
        lab_run_id: &str,
        artifact_id: &str,
    ) -> anyhow::Result<StageArtifact> {
        let artifact_id = artifact_id.trim();
        if artifact_id.is_empty() {
            return Err(anyhow!("artifact_id cannot be empty"));
        }
        read_json(
            &self
                .run_dir(lab_run_id)
                .join("artifacts")
                .join(format!("{}.json", safe_path_component(artifact_id))),
        )
    }

    pub fn list_stage_artifacts(&self, lab_run_id: &str) -> anyhow::Result<Vec<StageArtifact>> {
        let run = self.load_run(lab_run_id)?;
        let mut artifacts = Vec::new();
        for artifact_id in run.artifact_ids {
            artifacts.push(self.load_stage_artifact(lab_run_id, &artifact_id)?);
        }
        Ok(artifacts)
    }

    pub fn review_stage_artifact(
        &self,
        lab_run_id: &str,
        artifact_id: &str,
        status: LabArtifactStatus,
        validation_status: &str,
        note: Option<&str>,
    ) -> anyhow::Result<StageArtifact> {
        let mut artifact = self.load_stage_artifact(lab_run_id, artifact_id)?;
        artifact.set_review_state(status, Some(validation_status.trim().to_string()));
        let path = self.write_stage_artifact(&artifact)?;
        self.write_stage_artifact_report(&artifact)?;
        self.append_run_event(
            lab_run_id,
            "lab_artifact_reviewed",
            serde_json::json!({
                "artifact_id": artifact.artifact_id(),
                "artifact_type": artifact.artifact_type().as_str(),
                "stage": artifact.stage(),
                "status": format!("{:?}", artifact.status()),
                "validation_status": artifact.validation_status(),
                "note": note.unwrap_or("").trim(),
                "path": path.display().to_string(),
            }),
        )?;
        Ok(artifact)
    }

    fn append_project_event(&self, event: LabEvent) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root)?;
        append_jsonl(&self.root.join("events.jsonl"), &event)
    }

    fn append_run_event(
        &self,
        lab_run_id: &str,
        event_type: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        let event = LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: Some(lab_run_id.to_string()),
            proposal_id: None,
            event_type: event_type.to_string(),
            created_at: Utc::now(),
            payload,
        };
        let run_dir = self.run_dir(lab_run_id);
        fs::create_dir_all(&run_dir)?;
        append_jsonl(&run_dir.join("events.jsonl"), &event)
    }

    fn read_run_events(&self, lab_run_id: &str) -> anyhow::Result<Vec<LabEvent>> {
        let path = self.run_dir(lab_run_id).join("events.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let mut events = Vec::new();
        for line in std::io::BufReader::new(file).lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            events.push(
                serde_json::from_str::<LabEvent>(trimmed)
                    .with_context(|| format!("failed to parse {}", path.display()))?,
            );
        }
        Ok(events)
    }

    fn ensure_sqlite_schema(&self, conn: &Connection) -> anyhow::Result<()> {
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS lab_runs (
                lab_run_id TEXT PRIMARY KEY,
                schema_version INTEGER NOT NULL,
                project_root TEXT NOT NULL,
                proposal_id TEXT,
                status TEXT NOT NULL,
                current_stage TEXT NOT NULL,
                internal_owner TEXT NOT NULL,
                needs_user INTEGER NOT NULL,
                cycle_count INTEGER NOT NULL,
                failure_count INTEGER NOT NULL,
                artifact_count INTEGER NOT NULL,
                open_task_count INTEGER NOT NULL,
                meeting_count INTEGER NOT NULL,
                blocked_reason TEXT,
                closeout_status TEXT,
                pause_reason TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                state_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS lab_artifacts (
                artifact_id TEXT PRIMARY KEY,
                lab_run_id TEXT NOT NULL,
                artifact_type TEXT NOT NULL,
                stage TEXT NOT NULL,
                status TEXT NOT NULL,
                validation_status TEXT,
                artifact_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_lab_artifacts_run
                ON lab_artifacts(lab_run_id, stage, artifact_type);
            CREATE TABLE IF NOT EXISTS lab_events (
                event_id TEXT PRIMARY KEY,
                lab_run_id TEXT,
                proposal_id TEXT,
                event_type TEXT NOT NULL,
                created_at TEXT NOT NULL,
                payload_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_lab_events_run
                ON lab_events(lab_run_id, created_at);
            CREATE TABLE IF NOT EXISTS lab_tasks (
                task_id TEXT PRIMARY KEY,
                lab_run_id TEXT NOT NULL,
                status TEXT NOT NULL,
                title TEXT NOT NULL,
                assigned_role TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                result_artifact_id TEXT,
                blocker TEXT,
                task_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_lab_tasks_run
                ON lab_tasks(lab_run_id, status);
            ",
        )?;
        Ok(())
    }

    fn write_active_run_pointer(&self, lab_run_id: &str) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::write(self.root.join("active_run"), lab_run_id.as_bytes())?;
        Ok(())
    }

    fn read_active_run_pointer(&self) -> anyhow::Result<Option<String>> {
        let path = self.root.join("active_run");
        if !path.exists() {
            return Ok(None);
        }
        let value = fs::read_to_string(path)?.trim().to_string();
        Ok((!value.is_empty()).then_some(value))
    }

    fn proposals_dir(&self) -> PathBuf {
        self.root.join("proposals")
    }

    fn proposal_dir(&self, proposal_id: &str) -> PathBuf {
        self.proposals_dir().join(proposal_id)
    }

    fn runs_dir(&self) -> PathBuf {
        self.root.join("runs")
    }

    fn runs_index_path(&self) -> PathBuf {
        self.root.join("runs_index.json")
    }

    fn refresh_runs_index_entry(&self, run: &LabRun) -> anyhow::Result<()> {
        let mut index = self.load_runs_index()?.unwrap_or_else(|| {
            LabRunIndex::new(self.project_root.display().to_string(), Utc::now())
        });
        index.project_root = self.project_root.display().to_string();
        index.generated_at = Utc::now();
        index
            .entries
            .retain(|entry| entry.lab_run_id != run.lab_run_id);
        index.entries.push(LabRunIndexEntry::from_run(run));
        index.entries.sort_by_key(|entry| entry.updated_at);
        atomic_write_json(&self.runs_index_path(), &index)
    }

    fn app_lifecycle_path(&self) -> PathBuf {
        self.root.join("app_lifecycle.json")
    }

    fn daemon_state_path(&self) -> PathBuf {
        self.root.join("daemon_state.json")
    }

    fn provider_certifications_path(&self) -> PathBuf {
        self.root.join("provider_certifications.jsonl")
    }

    fn run_dir(&self, lab_run_id: &str) -> PathBuf {
        self.runs_dir().join(lab_run_id)
    }

    fn task_dir(&self, lab_run_id: &str) -> PathBuf {
        self.run_dir(lab_run_id).join("tasks")
    }

    fn task_path(&self, lab_run_id: &str, task_id: &str) -> PathBuf {
        self.task_dir(lab_run_id)
            .join(format!("{}.json", safe_path_component(task_id)))
    }

    fn dispatch_dir(&self, lab_run_id: &str) -> PathBuf {
        self.run_dir(lab_run_id).join("dispatches")
    }

    fn dispatch_path(&self, lab_run_id: &str, dispatch_id: &str) -> PathBuf {
        self.dispatch_dir(lab_run_id)
            .join(format!("{}.json", safe_path_component(dispatch_id)))
    }

    fn validation_retry_dir(&self, lab_run_id: &str) -> PathBuf {
        self.run_dir(lab_run_id).join("validation_retries")
    }

    fn validation_retry_path(&self, lab_run_id: &str, retry_id: &str) -> PathBuf {
        self.validation_retry_dir(lab_run_id)
            .join(format!("{}.json", safe_path_component(retry_id)))
    }

    fn write_graduate_task(&self, task: &GraduateTask) -> anyhow::Result<()> {
        atomic_write_json(&self.task_path(&task.lab_run_id, &task.task_id), task)
    }

    fn save_graduate_task_and_sync_run(
        &self,
        mut task: GraduateTask,
        event_type: &str,
    ) -> anyhow::Result<GraduateTask> {
        task.updated_at = Utc::now();
        self.write_graduate_task(&task)?;

        let mut run = self.load_run(&task.lab_run_id)?;
        sync_open_task(&mut run, &task);
        run.updated_at = task.updated_at;
        self.save_run(&run)?;

        self.append_run_event(
            &task.lab_run_id,
            event_type,
            serde_json::json!({
                "task_id": &task.task_id,
                "status": format!("{:?}", task.status),
                "result_artifact_id": &task.result_artifact_id,
                "evidence_ids": &task.evidence_ids,
                "blocker": &task.blocker,
            }),
        )?;
        Ok(task)
    }

    fn active_lease_path(&self) -> PathBuf {
        self.root.join("active_lease.json")
    }

    fn read_active_lease(&self) -> anyhow::Result<Option<LabLease>> {
        let path = self.active_lease_path();
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    fn ensure_no_foreign_fresh_lease(
        &self,
        lab_run_id: Option<&str>,
        now: chrono::DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let Some(lease) = self.read_active_lease()? else {
            return Ok(());
        };
        if lease.is_stale_at(now) {
            return Ok(());
        }
        if Some(lease.lab_run_id.as_str()) == lab_run_id && lease.lease_owner == lease_owner() {
            return Ok(());
        }
        Err(anyhow!(
            "active LabRun lease is held by {} for {}",
            lease.lease_owner,
            lease.lab_run_id
        ))
    }

    fn acquire_lease_for_run(
        &self,
        run: &mut LabRun,
        now: chrono::DateTime<Utc>,
    ) -> anyhow::Result<LabLease> {
        self.ensure_no_foreign_fresh_lease(Some(&run.lab_run_id), now)?;
        let lease = LabLease {
            schema_version: LAB_SCHEMA_VERSION,
            lease_id: run.lease_id.clone().unwrap_or_else(|| next_id("lease")),
            lab_run_id: run.lab_run_id.clone(),
            lease_owner: lease_owner(),
            lease_acquired_at: now,
            heartbeat_at: now,
            lease_ttl_seconds: run.lease_ttl_seconds,
        };
        run.lease_id = Some(lease.lease_id.clone());
        run.lease_owner = Some(lease.lease_owner.clone());
        run.heartbeat_at = Some(now);
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(self.run_dir(&run.lab_run_id))?;
        atomic_write_json(&self.active_lease_path(), &lease)?;
        atomic_write_json(&self.run_dir(&run.lab_run_id).join("lease.json"), &lease)?;
        Ok(lease)
    }

    fn release_lease_for_run(&self, lab_run_id: &str) -> anyhow::Result<()> {
        if let Some(lease) = self.read_active_lease()? {
            if lease.lab_run_id == lab_run_id {
                remove_file_if_exists(&self.active_lease_path())?;
            }
        }
        remove_file_if_exists(&self.run_dir(lab_run_id).join("lease.json"))?;
        Ok(())
    }

    fn append_sponsor_message_record(
        &self,
        lab_run_id: &str,
        message: &SponsorMessage,
    ) -> anyhow::Result<()> {
        append_jsonl(
            &self.run_dir(lab_run_id).join("sponsor_messages.jsonl"),
            message,
        )
    }

    fn write_sponsor_messages(
        &self,
        lab_run_id: &str,
        messages: &[SponsorMessage],
    ) -> anyhow::Result<()> {
        let mut content = String::new();
        for message in messages {
            content.push_str(&serde_json::to_string(message)?);
            content.push('\n');
        }
        atomic_write_text(
            &self.run_dir(lab_run_id).join("sponsor_messages.jsonl"),
            &content,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct LabCostTokens {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub cycle_id: Option<String>,
    pub meeting_id: Option<String>,
}

fn next_id(prefix: &str) -> String {
    format!(
        "{}_{}_{}",
        prefix,
        Utc::now().format("%Y%m%d%H%M%S"),
        Uuid::new_v4().simple()
    )
}

fn lease_owner() -> String {
    let host = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string());
    format!("pid:{}:host:{}", std::process::id(), host)
}

fn read_json<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> anyhow::Result<T> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}.tmp", Uuid::new_v4().simple()));
    let bytes = serde_json::to_vec_pretty(value)?;
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn atomic_write_text(path: &Path, value: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}.tmp", Uuid::new_v4().simple()));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(value.as_bytes())?;
        if !value.ends_with('\n') {
            file.write_all(b"\n")?;
        }
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn enum_json<T: Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_string())
}

fn optional_enum_json<T: Serialize>(value: Option<&T>) -> anyhow::Result<Option<String>> {
    value.map(enum_json).transpose()
}

fn sqlite_count(conn: &Connection, table: &str) -> anyhow::Result<usize> {
    let sql = match table {
        "lab_runs" => "SELECT COUNT(*) FROM lab_runs",
        "lab_artifacts" => "SELECT COUNT(*) FROM lab_artifacts",
        "lab_events" => "SELECT COUNT(*) FROM lab_events",
        "lab_tasks" => "SELECT COUNT(*) FROM lab_tasks",
        _ => return Err(anyhow!("unsupported Lab SQLite count table: {table}")),
    };
    let count: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    Ok(count.max(0) as usize)
}

fn latest_sqlite_artifact_for_role(
    conn: &Connection,
    lab_run_id: &str,
    artifact_types: &[&str],
) -> anyhow::Result<Option<LabSqliteArtifactSummary>> {
    let placeholders = artifact_types
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT artifact_id, artifact_type, stage, status, validation_status
         FROM lab_artifacts
         WHERE lab_run_id = ? AND artifact_type IN ({placeholders})
         ORDER BY rowid DESC
         LIMIT 1"
    );
    let mut params = Vec::with_capacity(artifact_types.len() + 1);
    params.push(lab_run_id);
    params.extend(artifact_types.iter().copied());
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params))?;
    if let Some(row) = rows.next()? {
        Ok(Some(LabSqliteArtifactSummary {
            artifact_id: row.get(0)?,
            artifact_type: row.get(1)?,
            stage: row.get(2)?,
            status: row.get(3)?,
            validation_status: row.get(4)?,
        }))
    } else {
        Ok(None)
    }
}

fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn safe_path_component(value: &str) -> String {
    let safe: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        "stage".to_string()
    } else {
        safe
    }
}

fn clean_string_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn sync_open_task(run: &mut LabRun, task: &GraduateTask) {
    if task.status.is_open() {
        if !run.open_task_ids.iter().any(|id| id == &task.task_id) {
            run.open_task_ids.push(task.task_id.clone());
        }
        if !run
            .resume_cursor
            .open_task_ids
            .iter()
            .any(|id| id == &task.task_id)
        {
            run.resume_cursor.open_task_ids.push(task.task_id.clone());
        }
    } else {
        run.open_task_ids.retain(|id| id != &task.task_id);
        run.resume_cursor
            .open_task_ids
            .retain(|id| id != &task.task_id);
    }
}

fn evidence_metadata_hash(reference: &str) -> Option<String> {
    let path = Path::new(reference);
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let payload = format!("{}:{}:{}", path.display(), metadata.len(), modified);
    Some(crate::engine::prompt_context::stable_fingerprint(&payload))
}

fn note_or_default<'a>(note: &'a str, default: &'a str) -> &'a str {
    let note = note.trim();
    if note.is_empty() {
        default
    } else {
        note
    }
}

#[cfg(test)]
mod tests {
    use crate::lab::orchestrator::LabOrchestrator;

    use super::*;

    #[test]
    fn proposal_approval_creates_run_and_persists_events() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());

        let proposal = store
            .create_proposal("Build the LabRun workflow", Some("session_1".to_string()))
            .unwrap();
        assert!(store
            .root()
            .join("proposals")
            .join(&proposal.proposal_id)
            .join("proposal.json")
            .exists());

        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        assert_eq!(
            run.proposal_id.as_deref(),
            Some(proposal.proposal_id.as_str())
        );
        assert_eq!(run.status, LabRunStatus::Active);
        assert!(run.lease_id.is_some());
        assert!(store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("state.json")
            .exists());
        assert!(store.root().join("active_lease.json").exists());
        assert!(store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("events.jsonl")
            .exists());
        assert_eq!(
            store.latest_run().unwrap().unwrap().lab_run_id,
            run.lab_run_id
        );
    }

    #[test]
    fn labrun_index_updates_from_state_changes_and_can_rebuild() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());

        let proposal = store.create_proposal("Build indexed LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        let index = store.load_runs_index().unwrap().unwrap();
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].lab_run_id, run.lab_run_id);
        assert_eq!(index.entries[0].status, LabRunStatus::Active);
        assert_eq!(index.entries[0].current_stage, "professor_discussion");

        let paused = store.pause_latest_run("manual checkpoint").unwrap();
        let updated_index = store.load_runs_index().unwrap().unwrap();
        let entry = updated_index
            .entries
            .iter()
            .find(|entry| entry.lab_run_id == paused.lab_run_id)
            .unwrap();
        assert_eq!(entry.status, LabRunStatus::Paused);
        assert_eq!(entry.pause_reason.as_deref(), Some("manual checkpoint"));

        fs::remove_file(store.runs_index_path()).unwrap();
        assert!(store.load_runs_index().unwrap().is_none());
        let rebuilt = store.rebuild_runs_index().unwrap();
        assert_eq!(rebuilt.entries.len(), 1);
        assert_eq!(rebuilt.entries[0].lab_run_id, paused.lab_run_id);
        assert_eq!(rebuilt.entries[0].status, LabRunStatus::Paused);
        assert!(store.runs_index_path().exists());
    }

    #[test]
    fn sqlite_index_imports_file_backed_labrun_tables() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let store = orchestrator.store();

        let proposal = store.create_proposal("Build indexed LabRun", None).unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        orchestrator
            .create_current_stage_artifact_for_latest("Professor plan")
            .unwrap();
        store
            .create_graduate_task(
                &run.lab_run_id,
                "Implement indexed persistence",
                "Mirror file-backed LabRun state into SQLite.",
                vec!["src/lab/store.rs".to_string()],
                vec!["cargo test -q lab".to_string()],
            )
            .unwrap();

        let summary = store.rebuild_sqlite_index().unwrap();

        assert_eq!(summary.path, store.sqlite_index_path());
        assert_eq!(summary.lab_runs, 1);
        assert_eq!(summary.lab_artifacts, 1);
        assert!(summary.lab_events >= 3);
        assert_eq!(summary.lab_tasks, 1);
        let loaded = store.load_sqlite_index_summary().unwrap().unwrap();
        assert_eq!(loaded, summary);
    }

    #[test]
    fn sponsor_message_is_event_not_direct_task() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        let msg = store
            .append_sponsor_message("This is drifting from the product goal")
            .unwrap();

        assert_eq!(msg.lab_run_id, run.lab_run_id);
        assert_eq!(msg.status, SponsorMessageStatus::Queued);
        let messages = store.list_sponsor_messages(&run.lab_run_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id, msg.message_id);
        assert_eq!(messages[0].message_type, SponsorMessageType::Concern);
        let events = fs::read_to_string(
            store
                .root()
                .join("runs")
                .join(&run.lab_run_id)
                .join("events.jsonl"),
        )
        .unwrap();
        assert!(events.contains("sponsor_message"));
        assert!(events.contains("This is drifting"));
    }

    #[test]
    fn intervention_pauses_run_and_queues_urgent_professor_message() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        assert!(store.root().join("active_lease.json").exists());

        let (intervened, message) = store
            .intervene_latest_run("Stop and reassess scope before continuing")
            .unwrap();

        assert_eq!(intervened.lab_run_id, run.lab_run_id);
        assert_eq!(intervened.status, LabRunStatus::NeedsUser);
        assert!(intervened.needs_user);
        assert_eq!(
            intervened.pause_reason.as_deref(),
            Some("sponsor_intervention")
        );
        assert_eq!(message.message_type, SponsorMessageType::PauseRequest);
        assert_eq!(message.urgency, "high");
        assert_eq!(message.status, SponsorMessageStatus::Queued);
        let messages = store.list_sponsor_messages(&run.lab_run_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id, message.message_id);
        assert!(!store.root().join("active_lease.json").exists());
        let events = fs::read_to_string(
            store
                .root()
                .join("runs")
                .join(&run.lab_run_id)
                .join("events.jsonl"),
        )
        .unwrap();
        assert!(events.contains("lab_intervention_recorded"));
        assert!(events.contains("Stop and reassess scope"));
    }

    #[test]
    fn sponsor_message_status_update_rewrites_inbox_and_records_event() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let msg = store
            .append_sponsor_message("Please turn this into a lab meeting")
            .unwrap();

        let updated = store
            .update_latest_sponsor_message_status(
                &msg.message_id,
                SponsorMessageStatus::ConvertedToMeeting,
                "meeting requested",
            )
            .unwrap();

        assert_eq!(updated.status, SponsorMessageStatus::ConvertedToMeeting);
        let messages = store.list_sponsor_messages(&run.lab_run_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].status, SponsorMessageStatus::ConvertedToMeeting);
        let events = fs::read_to_string(
            store
                .root()
                .join("runs")
                .join(&run.lab_run_id)
                .join("events.jsonl"),
        )
        .unwrap();
        assert!(events.contains("sponsor_message_status_updated"));
        assert!(events.contains("meeting requested"));
    }

    #[test]
    fn pause_releases_active_lease_and_resume_reacquires_it() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        assert!(store.root().join("active_lease.json").exists());

        let paused = store.pause_latest_run("user").unwrap();
        assert_eq!(paused.lab_run_id, run.lab_run_id);
        assert_eq!(paused.status, LabRunStatus::Paused);
        assert!(!store.root().join("active_lease.json").exists());

        let resumed = store.resume_latest_run().unwrap();
        assert_eq!(resumed.status, LabRunStatus::Active);
        assert!(resumed.lease_id.is_some());
        assert!(store.root().join("active_lease.json").exists());
    }

    #[test]
    fn closeout_marks_run_completed_and_releases_active_lease() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        assert!(store.root().join("active_lease.json").exists());

        let closed = store
            .closeout_latest_run(LabCloseoutStatus::CompletedVerified, "validation passed")
            .unwrap();

        assert_eq!(closed.lab_run_id, run.lab_run_id);
        assert_eq!(closed.status, LabRunStatus::Completed);
        assert_eq!(
            closed.closeout_status,
            Some(LabCloseoutStatus::CompletedVerified)
        );
        assert!(!closed.needs_user);
        assert!(closed.lease_id.is_none());
        assert!(!store.root().join("active_lease.json").exists());
        let events = fs::read_to_string(
            store
                .root()
                .join("runs")
                .join(&run.lab_run_id)
                .join("events.jsonl"),
        )
        .unwrap();
        assert!(events.contains("lab_closeout_recorded"));
        assert!(events.contains("validation passed"));
    }

    #[test]
    fn stale_active_lease_recovery_pauses_run_for_resume() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let lease_path = store.root().join("active_lease.json");
        let mut lease: LabLease = read_json(&lease_path).unwrap();
        lease.heartbeat_at =
            Utc::now() - chrono::Duration::seconds(lease.lease_ttl_seconds as i64 + 5);
        atomic_write_json(&lease_path, &lease).unwrap();

        let recovered = store.recover_stale_active_lease().unwrap();

        assert!(recovered.is_some());
        assert!(!lease_path.exists());
        let saved = store.load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.status, LabRunStatus::PausedShutdown);
        assert_eq!(saved.pause_reason.as_deref(), Some("stale_heartbeat"));
        assert!(saved.lease_id.is_none());
        assert_eq!(
            store.latest_run().unwrap().unwrap().lab_run_id,
            run.lab_run_id
        );
    }

    #[test]
    fn command_claims_stale_active_lease_without_pausing_run() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let lease_path = store.root().join("active_lease.json");
        let mut lease: LabLease = read_json(&lease_path).unwrap();
        lease.heartbeat_at =
            Utc::now() - chrono::Duration::seconds(lease.lease_ttl_seconds as i64 + 5);
        atomic_write_json(&lease_path, &lease).unwrap();

        let claimed = store
            .claim_latest_active_run_for_current_process()
            .unwrap()
            .unwrap();

        assert_eq!(claimed.lab_run_id, run.lab_run_id);
        assert_eq!(claimed.status, LabRunStatus::Active);
        assert!(claimed.lease_id.is_some());
        assert!(store.root().join("active_lease.json").exists());
        let saved = store.load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.status, LabRunStatus::Active);
        assert_eq!(saved.pause_reason, None);
        let events = fs::read_to_string(
            store
                .root()
                .join("runs")
                .join(&run.lab_run_id)
                .join("events.jsonl"),
        )
        .unwrap();
        assert!(events.contains("lab_command_stale_lease_claimed"));
        assert!(events.contains("lab_command_lease_claimed"));

        let released = store
            .release_current_process_lease_without_pausing()
            .unwrap();
        assert!(released.is_some());
        assert!(!store.root().join("active_lease.json").exists());
        let saved = store.load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.status, LabRunStatus::Active);
        assert!(saved.lease_id.is_none());
        assert!(saved.lease_owner.is_none());
    }

    #[test]
    fn command_claims_active_run_when_lease_file_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        std::fs::remove_file(store.root().join("active_lease.json")).unwrap();

        let claimed = store
            .claim_latest_active_run_for_current_process()
            .unwrap()
            .unwrap();

        assert_eq!(claimed.lab_run_id, run.lab_run_id);
        assert!(store.root().join("active_lease.json").exists());
        let saved = store.load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.status, LabRunStatus::Active);
        assert!(saved.lease_id.is_some());
        assert!(saved.lease_owner.is_some());

        let released = store
            .release_current_process_lease_without_pausing()
            .unwrap();
        assert!(released.is_some());
        assert!(!store.root().join("active_lease.json").exists());
        let saved = store.load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.status, LabRunStatus::Active);
        assert!(saved.lease_id.is_none());
        assert!(saved.lease_owner.is_none());
    }

    #[test]
    fn shutdown_pause_releases_lease_and_preserves_resume_target() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        let paused = store.pause_latest_run_for_shutdown().unwrap().unwrap();

        assert_eq!(paused.lab_run_id, run.lab_run_id);
        assert_eq!(paused.status, LabRunStatus::PausedShutdown);
        assert_eq!(paused.pause_reason.as_deref(), Some("app_shutdown"));
        assert!(!store.root().join("active_lease.json").exists());
        let resumed = store.resume_latest_run().unwrap();
        assert_eq!(resumed.status, LabRunStatus::Active);
        assert!(resumed.lease_id.is_some());
    }

    #[test]
    fn app_lifecycle_startup_recovers_interrupted_scheduler_state() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let now = Utc::now();
        store
            .write_scheduler_state(&LabSchedulerState {
                schema_version: LAB_SCHEMA_VERSION,
                lab_run_id: run.lab_run_id.clone(),
                status: LabSchedulerStatus::Running,
                updated_at: now,
                started_at: Some(now),
                stopped_at: None,
                max_steps: 20,
                steps_completed: 3,
                interval_ms: 1_000,
                last_action: Some("TickAdvanced".to_string()),
                last_message: Some("background scheduler started".to_string()),
                stop_reason: None,
            })
            .unwrap();

        let lifecycle = store.record_app_lifecycle_startup("lab_cli").unwrap();

        assert_eq!(lifecycle.launch_mode, "lab_cli");
        assert_eq!(
            lifecycle.recovered_scheduler_lab_run_id.as_deref(),
            Some(run.lab_run_id.as_str())
        );
        assert_eq!(
            lifecycle.recovered_scheduler_status,
            Some(LabSchedulerStatus::PausedRestart)
        );
        assert!(store.root().join("app_lifecycle.json").exists());
        let loaded = store.load_app_lifecycle_state().unwrap().unwrap();
        assert_eq!(
            loaded.recovered_scheduler_status,
            lifecycle.recovered_scheduler_status
        );
    }

    #[test]
    fn app_lifecycle_shutdown_pauses_active_run_and_records_state() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        let lifecycle = store.record_app_lifecycle_shutdown("lab_cli").unwrap();

        assert_eq!(
            lifecycle.shutdown_paused_lab_run_id.as_deref(),
            Some(run.lab_run_id.as_str())
        );
        assert!(lifecycle.last_shutdown_at.is_some());
        let saved = store.load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.status, LabRunStatus::PausedShutdown);
        assert_eq!(saved.pause_reason.as_deref(), Some("app_shutdown"));
        let loaded = store.load_app_lifecycle_state().unwrap().unwrap();
        assert_eq!(
            loaded.shutdown_paused_lab_run_id.as_deref(),
            Some(run.lab_run_id.as_str())
        );
    }

    #[test]
    fn daemon_policy_enable_and_disable_are_persisted() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        let enabled = store
            .enable_daemon(
                LabDaemonMode::Hybrid,
                9,
                250,
                "continue postdoc repair loop",
            )
            .unwrap();

        assert!(enabled.enabled);
        assert_eq!(enabled.mode, LabDaemonMode::Hybrid);
        assert_eq!(enabled.max_steps, 9);
        assert_eq!(enabled.interval_ms, 250);
        assert_eq!(enabled.instructions, "continue postdoc repair loop");
        assert!(store.root().join("daemon_state.json").exists());
        let loaded = store.load_daemon_state().unwrap().unwrap();
        assert_eq!(loaded.mode, LabDaemonMode::Hybrid);

        let started = store
            .record_daemon_start_result(Some(&run.lab_run_id), None)
            .unwrap()
            .unwrap();
        assert_eq!(
            started.last_started_lab_run_id.as_deref(),
            Some(run.lab_run_id.as_str())
        );
        assert!(started.last_started_at.is_some());
        assert!(started.last_start_error.is_none());

        let disabled = store.disable_daemon("user paused lab daemon").unwrap();

        assert!(!disabled.enabled);
        assert_eq!(disabled.mode, LabDaemonMode::Hybrid);
        assert_eq!(disabled.last_enabled_at, enabled.last_enabled_at);
        assert!(disabled.last_disabled_at.is_some());
        assert!(disabled
            .last_message
            .as_deref()
            .unwrap_or_default()
            .contains("user paused lab daemon"));
    }

    #[test]
    fn daemon_policy_persists_hybrid_cycles_mode() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        store.approve_proposal(&proposal.proposal_id).unwrap();

        let enabled = store
            .enable_daemon_with_cycle_bound(
                LabDaemonMode::HybridCycles,
                4,
                6,
                500,
                "continue bounded lab cycles",
            )
            .unwrap();

        assert!(enabled.enabled);
        assert_eq!(enabled.mode, LabDaemonMode::HybridCycles);
        assert_eq!(enabled.max_steps, 4);
        assert_eq!(enabled.max_steps_per_cycle, 6);
        assert_eq!(enabled.interval_ms, 500);
        assert_eq!(enabled.instructions, "continue bounded lab cycles");
        let loaded = store.load_daemon_state().unwrap().unwrap();
        assert_eq!(loaded.mode, LabDaemonMode::HybridCycles);
        assert_eq!(loaded.max_steps_per_cycle, 6);
    }

    #[test]
    fn interrupted_scheduler_recovery_marks_running_state_resumable() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let now = Utc::now();
        store
            .write_scheduler_state(&LabSchedulerState {
                schema_version: LAB_SCHEMA_VERSION,
                lab_run_id: run.lab_run_id.clone(),
                status: LabSchedulerStatus::Running,
                updated_at: now,
                started_at: Some(now),
                stopped_at: None,
                max_steps: 20,
                steps_completed: 3,
                interval_ms: 1_000,
                last_action: Some("TickAdvanced".to_string()),
                last_message: Some("background scheduler started".to_string()),
                stop_reason: None,
            })
            .unwrap();

        let recovered = store.recover_interrupted_scheduler().unwrap().unwrap();

        assert_eq!(recovered.status, LabSchedulerStatus::PausedRestart);
        assert_eq!(recovered.stop_reason.as_deref(), Some("process_restart"));
        assert_eq!(recovered.steps_completed, 3);
        assert!(recovered.stopped_at.is_some());
        let saved = store
            .load_scheduler_state(&run.lab_run_id)
            .unwrap()
            .unwrap();
        assert_eq!(saved.status, LabSchedulerStatus::PausedRestart);
    }

    #[test]
    fn lab_failure_accounting_escalates_after_retry_budget() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        let first = store
            .record_lab_failure(&run.lab_run_id, "graduate_execution", "first failure")
            .unwrap();
        assert_eq!(first.failure_count, 1);
        assert_eq!(first.status, LabRunStatus::Active);
        assert!(!first.needs_user);

        let second = store
            .record_lab_failure(&run.lab_run_id, "graduate_execution", "second failure")
            .unwrap();
        assert_eq!(second.failure_count, 2);
        assert_eq!(second.status, LabRunStatus::NeedsUser);
        assert!(second.needs_user);
        assert_eq!(
            second.closeout_status,
            Some(LabCloseoutStatus::BlockedNeedsUser)
        );
        assert!(second
            .blocked_reason
            .as_deref()
            .unwrap_or_default()
            .contains("failure budget reached"));
    }

    #[test]
    fn graduate_task_revision_requeues_blocked_task_when_scope_is_complete() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Postdoc plan lacks scope.",
                Vec::new(),
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let blocked = store
            .block_graduate_task(&run.lab_run_id, &task.task_id, "missing allowed_scope")
            .unwrap();
        assert_eq!(blocked.status, LabTaskStatus::Blocked);

        let revised = store
            .revise_graduate_task(
                &run.lab_run_id,
                &task.task_id,
                vec!["src/lab/store.rs".to_string()],
                vec!["cargo check -q --tests".to_string()],
                Some("Use the narrowed LabStore scope."),
            )
            .unwrap();

        assert_eq!(revised.status, LabTaskStatus::Queued);
        assert_eq!(revised.blocker, None);
        assert_eq!(revised.allowed_scope, vec!["src/lab/store.rs".to_string()]);
        assert_eq!(
            revised.required_validation,
            vec!["cargo check -q --tests".to_string()]
        );
        assert!(revised
            .instructions
            .contains("Postdoc revision:\nUse the narrowed LabStore scope."));
    }

    #[test]
    fn graduate_task_revision_stays_blocked_when_scope_is_incomplete() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Postdoc plan lacks validation.",
                vec!["src/lab/store.rs".to_string()],
                Vec::new(),
            )
            .unwrap();

        let revised = store
            .revise_graduate_task(
                &run.lab_run_id,
                &task.task_id,
                vec!["src/lab/store.rs".to_string()],
                Vec::new(),
                None,
            )
            .unwrap();

        assert_eq!(revised.status, LabTaskStatus::Blocked);
        assert!(revised
            .blocker
            .as_deref()
            .unwrap_or("")
            .contains("missing required_validation"));
    }

    #[test]
    fn validation_retry_creates_repair_task_then_escalates_after_budget() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();

        let first = store
            .record_validation_retry_and_repair_task(
                &run.lab_run_id,
                &task.task_id,
                "cargo check failed",
            )
            .unwrap();

        assert_eq!(first.attempt, 1);
        assert!(!first.escalated);
        let repair_id = first.repair_task_id.as_deref().unwrap();
        let repair = store
            .load_graduate_task(&run.lab_run_id, repair_id)
            .unwrap();
        assert!(repair.title.starts_with("Repair validation for"));
        let retries = store.list_validation_retries(&run.lab_run_id).unwrap();
        assert_eq!(retries.len(), 1);

        let second = store
            .record_validation_retry_and_repair_task(
                &run.lab_run_id,
                &task.task_id,
                "cargo test failed",
            )
            .unwrap();
        assert_eq!(second.attempt, 2);
        assert!(second.repair_task_id.is_some());

        let third = store
            .record_validation_retry_and_repair_task(
                &run.lab_run_id,
                &task.task_id,
                "validation still failing",
            )
            .unwrap();
        assert_eq!(third.attempt, 3);
        assert!(third.escalated);
        assert!(third.repair_task_id.is_none());
        let saved_run = store.load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved_run.failure_count, 1);
    }

    #[test]
    fn cost_usage_records_cache_shape_and_summarizes_by_role() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        store
            .record_cost_usage(
                &run.lab_run_id,
                LabRole::Professor,
                "test-model",
                LabCostTokens {
                    prompt_tokens: 1_000,
                    completion_tokens: 200,
                    reasoning_tokens: 50,
                    cached_tokens: 700,
                    cache_write_tokens: 120,
                    cycle_id: Some("0".to_string()),
                    meeting_id: None,
                },
                0.0123,
                Some("professor draft"),
            )
            .unwrap();
        store
            .record_cost_usage(
                &run.lab_run_id,
                LabRole::Postdoc,
                "test-model",
                LabCostTokens {
                    prompt_tokens: 500,
                    completion_tokens: 100,
                    reasoning_tokens: 0,
                    cached_tokens: 100,
                    cache_write_tokens: 20,
                    cycle_id: Some("0".to_string()),
                    meeting_id: None,
                },
                0.004,
                None,
            )
            .unwrap();

        let summary = store.cost_summary(&run.lab_run_id).unwrap();

        assert_eq!(summary.requests, 2);
        assert_eq!(summary.prompt_tokens, 1_500);
        assert_eq!(summary.completion_tokens, 300);
        assert_eq!(summary.reasoning_tokens, 50);
        assert_eq!(summary.cached_tokens, 800);
        assert_eq!(summary.cache_write_tokens, 140);
        assert_eq!(summary.cache_miss_tokens, 700);
        assert_eq!(summary.total_tokens, 1_850);
        assert_eq!(summary.by_role.len(), 2);
        assert!(store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("cost_usage.jsonl")
            .exists());
    }

    #[test]
    fn evidence_refs_are_refs_only_and_listable() {
        let temp = tempfile::tempdir().unwrap();
        let evidence_file = temp.path().join("proof.txt");
        fs::write(&evidence_file, "validation proof").unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        let evidence = store
            .record_evidence_ref(
                &run.lab_run_id,
                LabEvidenceKind::File,
                LabRole::Postdoc,
                &evidence_file.display().to_string(),
                "cargo check passed",
                None,
                Some("0"),
            )
            .unwrap();

        assert_eq!(evidence.kind, LabEvidenceKind::File);
        assert_eq!(evidence.summary, "cargo check passed");
        assert!(evidence.metadata_hash.is_some());
        let listed = store.list_evidence_refs(&run.lab_run_id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].evidence_id, evidence.evidence_id);
        assert!(store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("evidence_refs.jsonl")
            .exists());
    }

    #[test]
    fn provider_certifications_are_project_level_and_latest_wins() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());

        let failed = store
            .record_provider_certification(
                "deepseek",
                "deepseek-v4-flash",
                LabProviderCertificationKind::Graduate,
                LabProviderCertificationOutcome::Failed,
                "target/lab-live-validation/failed/report.md",
                "graduate runtime verification failed",
            )
            .unwrap();
        let passed = store
            .record_provider_certification(
                "deepseek",
                "deepseek-v4-flash",
                LabProviderCertificationKind::Graduate,
                LabProviderCertificationOutcome::Passed,
                "target/lab-live-validation/passed/report.md",
                "graduate runtime verification passed",
            )
            .unwrap();

        let listed = store.list_provider_certifications().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].record_id, failed.record_id);
        let latest = store
            .latest_provider_certification(
                "deepseek",
                "deepseek-v4-flash",
                LabProviderCertificationKind::Graduate,
            )
            .unwrap()
            .unwrap();
        assert_eq!(latest.record_id, passed.record_id);
        assert_eq!(latest.outcome, LabProviderCertificationOutcome::Passed);
        assert!(store.root().join("provider_certifications.jsonl").exists());
    }

    #[test]
    fn graduate_tasks_sync_open_task_resume_cursor() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Implement one narrow slice",
                "Only touch the listed files and report validation evidence.",
                vec![
                    "src/lab/model.rs".to_string(),
                    "src/lab/store.rs".to_string(),
                ],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();

        assert_eq!(task.status, LabTaskStatus::Queued);
        let saved = store.load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.open_task_ids, vec![task.task_id.clone()]);
        assert_eq!(
            saved.resume_cursor.open_task_ids,
            vec![task.task_id.clone()]
        );
        assert!(store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("tasks")
            .join(format!("{}.json", task.task_id))
            .exists());

        let started = store
            .start_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(started.status, LabTaskStatus::InProgress);
        let completed = store
            .complete_graduate_task(
                &run.lab_run_id,
                &task.task_id,
                "artifact_graduate_result_001",
                vec!["labevidence_001".to_string()],
            )
            .unwrap();

        assert_eq!(completed.status, LabTaskStatus::Completed);
        assert_eq!(
            completed.result_artifact_id.as_deref(),
            Some("artifact_graduate_result_001")
        );
        let saved = store.load_run(&run.lab_run_id).unwrap();
        assert!(saved.open_task_ids.is_empty());
        assert!(saved.resume_cursor.open_task_ids.is_empty());
    }

    #[test]
    fn graduate_dispatch_records_are_persisted_and_listable() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Implement one narrow slice",
                "Only touch the listed files and report validation evidence.",
                vec!["src/lab/delegation.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let dispatch = crate::lab::delegation::build_graduate_task_dispatch(&task).unwrap();

        let record = store
            .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
            .unwrap();

        assert_eq!(record.status, GraduateDispatchStatus::Prepared);
        assert_eq!(record.cleanup_status, GraduateCleanupStatus::CleanupPending);
        assert!(record
            .cleanup_message
            .as_deref()
            .unwrap_or_default()
            .contains("cleanup pending"));
        assert!(record.cleanup_updated_at.is_some());
        assert_eq!(record.task_id, task.task_id);
        assert_eq!(
            record.agent_tool_params["profile"].as_str(),
            Some("lab-graduate")
        );
        let listed = store.list_graduate_dispatches(&run.lab_run_id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].dispatch_id, record.dispatch_id);
        assert_eq!(
            listed[0].cleanup_status,
            GraduateCleanupStatus::CleanupPending
        );
        assert!(store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("dispatches")
            .join(format!("{}.json", record.dispatch_id))
            .exists());
    }

    #[test]
    fn compression_decisions_are_persisted() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();

        let decision = store
            .record_compression_decision(LabCompressionDecision {
                schema_version: LAB_SCHEMA_VERSION,
                decision_id: String::new(),
                lab_run_id: run.lab_run_id.clone(),
                created_at: Utc::now(),
                role: LabRole::Professor,
                action: crate::lab::model::LabCompressionAction::Recommend,
                reason: "near budget".to_string(),
                context_budget_tokens: 100,
                packet_tokens: 70,
                usage_ratio_percent: 70.0,
                stable_prefix_fingerprint: "stable".to_string(),
                dynamic_tail_fingerprint: "dynamic".to_string(),
                cycle_id: Some("0".to_string()),
            })
            .unwrap();

        assert!(!decision.decision_id.is_empty());
        let listed = store.list_compression_decisions(&run.lab_run_id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].decision_id, decision.decision_id);
        assert!(store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("compression_decisions.jsonl")
            .exists());
    }

    #[test]
    fn artifact_gate_validation_blocks_missing_handoff_fields() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let mut gate = ArtifactGate::new(
            "postdoc_review",
            "PostdocIntegrationSummary",
            crate::lab::model::LabRole::Postdoc,
        );

        let path = store.write_artifact_gate(&run.lab_run_id, &gate).unwrap();
        assert!(path.exists());
        let err = store
            .validate_artifact_gate(&run.lab_run_id, "postdoc_review")
            .unwrap_err()
            .to_string();
        assert!(err.contains("artifact_id"));
        assert!(err.contains("next_action"));

        gate.artifact_id = Some("artifact_postdoc_summary_001".to_string());
        gate.next_action = Some("professor_review".to_string());
        gate.validation_status = Some("not_verified".to_string());
        store.write_artifact_gate(&run.lab_run_id, &gate).unwrap();
        let err = store
            .validate_artifact_gate(&run.lab_run_id, "postdoc_review")
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing or malformed artifact"));

        let wrong_artifact =
            StageArtifact::ProfessorPlan(crate::lab::model::LabArtifactEnvelope::new(
                "artifact_postdoc_summary_001".to_string(),
                run.lab_run_id.clone(),
                crate::lab::model::LabArtifactType::ProfessorPlan,
                "Wrong artifact type".to_string(),
                Utc::now(),
                crate::lab::model::ProfessorPlan {
                    problem_statement: "Build LabRun".to_string(),
                    strategic_direction: "Wrong stage for this gate.".to_string(),
                    success_criteria: Vec::new(),
                    constraints: Vec::new(),
                    risks: Vec::new(),
                    handoff_to_postdoc: "Not a postdoc integration summary.".to_string(),
                },
            ));
        store.write_stage_artifact(&wrong_artifact).unwrap();
        let err = store
            .validate_artifact_gate(&run.lab_run_id, "postdoc_review")
            .unwrap_err()
            .to_string();
        assert!(err.contains("has stage 'professor_discussion'"));

        let artifact =
            StageArtifact::PostdocIntegrationSummary(crate::lab::model::LabArtifactEnvelope::new(
                "artifact_postdoc_summary_001".to_string(),
                run.lab_run_id.clone(),
                crate::lab::model::LabArtifactType::PostdocIntegrationSummary,
                "Postdoc integration summary".to_string(),
                Utc::now(),
                crate::lab::model::PostdocIntegrationSummary {
                    integration_summary: "Integrated graduate result.".to_string(),
                    accepted_results: vec!["artifact_graduate_result_001".to_string()],
                    validation_status: "validated".to_string(),
                    remaining_risks: Vec::new(),
                    handoff_to_professor: "Ready for professor review.".to_string(),
                },
            ));
        store.write_stage_artifact(&artifact).unwrap();
        store.write_stage_artifact_report(&artifact).unwrap();
        store
            .validate_artifact_gate(&run.lab_run_id, "postdoc_review")
            .unwrap();
    }
}
