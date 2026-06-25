//! Graduate task persistence for LabRun.
//!
//! Graduate task records bridge orchestrator decisions, delegated agent work,
//! cleanup state, and recovery dashboards. Keep task status and cleanup fields
//! explicit so stale worktree cleanup can be audited after interrupted runs.

use super::*;

impl LabStore {
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
        let allowed_scope = crate::lab::path_scope::normalize_lab_relative_paths(&allowed_scope)
            .with_context(|| "graduate task allowed_scope failed LabRun path normalization")?;
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
            allowed_scope,
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

        task.allowed_scope =
            crate::lab::path_scope::normalize_lab_relative_paths(&allowed_scope)
                .with_context(|| "graduate task allowed_scope failed LabRun path normalization")?;
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
}
