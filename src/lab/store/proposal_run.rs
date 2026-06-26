//! LabRun proposal and run persistence operations.
//!
//! This module creates proposals, approves them into runs, updates active run
//! state, and maintains the project-level run index. Stage transitions should
//! still flow through `LabOrchestrator`.

use super::*;

impl LabStore {
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

    pub fn record_run_event_returning(
        &self,
        lab_run_id: &str,
        event_type: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<LabEvent> {
        self.append_run_event_returning(lab_run_id, event_type, payload)
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
}
