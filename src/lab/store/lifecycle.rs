//! LabRun lifecycle persistence operations.
//!
//! This module updates run status, sponsor messages, provider certification,
//! daemon policy, and recovery-oriented lifecycle state. These writes are part
//! of the persisted runtime contract and should remain evidence-friendly.

use super::*;

impl LabStore {
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

    pub(super) fn record_app_lifecycle_startup_with_options(
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

    pub(super) fn write_app_lifecycle_state(
        &self,
        state: &LabAppLifecycleState,
    ) -> anyhow::Result<()> {
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

    pub(super) fn write_daemon_state(&self, state: &LabDaemonState) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root)?;
        atomic_write_json(&self.daemon_state_path(), state)
    }
}
