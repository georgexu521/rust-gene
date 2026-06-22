//! Graduate task execution bridge for LabRun.
//!
//! This module delegates scoped graduate work through the agent tool boundary
//! and records durable dispatch state. It must preserve evidence, cleanup, and
//! failure ownership for later recovery.

use super::*;

impl LabOrchestrator {
    pub async fn execute_graduate_task_latest_with_context(
        &self,
        task_id: &str,
        context: ToolContext,
    ) -> anyhow::Result<GraduateDispatchRecord> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for graduate dispatch execution"))?;
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        let task = self.store.load_graduate_task(&run.lab_run_id, task_id)?;
        let dispatch = build_graduate_task_dispatch(&task)?;
        let record =
            self.store
                .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)?;
        if matches!(task.status, LabTaskStatus::Queued | LabTaskStatus::Blocked) {
            self.store
                .start_graduate_task(&run.lab_run_id, &task.task_id)?;
        }
        let running = self.store.update_graduate_dispatch_status(
            &run.lab_run_id,
            &record.dispatch_id,
            GraduateDispatchStatus::Running,
            None,
            None,
            None,
        )?;
        if let Err(error) =
            crate::lab::provider_certification::validate_graduate_provider_for_execution(&context)
        {
            let _ = self
                .store
                .block_graduate_task(&run.lab_run_id, &task.task_id, &error);
            let _ = self.store.record_lab_failure(
                &run.lab_run_id,
                "graduate_provider_certification",
                &error,
            );
            return self.store.update_graduate_dispatch_status(
                &run.lab_run_id,
                &running.dispatch_id,
                GraduateDispatchStatus::Failed,
                None,
                None,
                Some(error),
            );
        }

        let before_snapshot = workspace_change_snapshot(&context.working_dir);
        self.record_graduate_workspace_snapshot(
            &run.lab_run_id,
            &task.task_id,
            &running.dispatch_id,
            "before",
            &before_snapshot,
            &[],
        )?;
        let result = execute_graduate_task_with_agent_tool(&task, context.clone()).await;
        let after_snapshot = workspace_change_snapshot(&context.working_dir);
        let agent_id = result
            .data
            .as_ref()
            .and_then(|data| data.get("agent_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let changed_by_agent = changed_paths_between(&before_snapshot, &after_snapshot);
        self.record_graduate_workspace_snapshot(
            &run.lab_run_id,
            &task.task_id,
            &running.dispatch_id,
            "after",
            &after_snapshot,
            &changed_by_agent,
        )?;
        if let Err(err) =
            validate_changed_files_within_scope(&task.allowed_scope, &changed_by_agent)
        {
            let error = format!("graduate agent modified files outside allowed_scope: {err}");
            let _ = self
                .store
                .block_graduate_task(&run.lab_run_id, &task.task_id, &error);
            let _ =
                self.store
                    .record_lab_failure(&run.lab_run_id, "graduate_scope_violation", &error);
            return self.store.update_graduate_dispatch_status(
                &run.lab_run_id,
                &running.dispatch_id,
                GraduateDispatchStatus::Failed,
                agent_id,
                None,
                Some(error),
            );
        }
        if result.success {
            if let Some(mut parsed) =
                parse_graduate_agent_result(result.data.as_ref(), &result.content)
            {
                let runtime_evidence = match runtime_verify_graduate_task_result(
                    &task,
                    &context,
                    agent_id.as_deref(),
                    &graduate_agent_task_id(&task),
                    &changed_by_agent,
                ) {
                    Ok(evidence) => evidence,
                    Err(err) => {
                        let error = err.to_string();
                        let _ =
                            self.store
                                .block_graduate_task(&run.lab_run_id, &task.task_id, &error);
                        let _ = self.store.record_lab_failure(
                            &run.lab_run_id,
                            "graduate_runtime_verification",
                            &error,
                        );
                        return self.store.update_graduate_dispatch_status(
                            &run.lab_run_id,
                            &running.dispatch_id,
                            GraduateDispatchStatus::Failed,
                            agent_id,
                            None,
                            Some(error),
                        );
                    }
                };
                parsed.changed_files = runtime_evidence.changed_files;
                parsed
                    .validation_attempts
                    .extend(runtime_evidence.validation_attempts);
                parsed.validation_attempts.sort();
                parsed.validation_attempts.dedup();
                let created = self.create_graduate_result_for_task_latest(
                    &task.task_id,
                    &parsed.task_summary,
                    parsed.changed_files,
                    parsed.validation_attempts,
                    parsed.blockers,
                    parsed.evidence_ids,
                )?;
                return self.store.update_graduate_dispatch_status(
                    &run.lab_run_id,
                    &running.dispatch_id,
                    GraduateDispatchStatus::Succeeded,
                    agent_id,
                    Some(created.artifact.artifact_id().to_string()),
                    None,
                );
            }
            match self.create_runtime_verified_graduate_result_for_unbound_success(
                &task,
                &context,
                agent_id.as_deref(),
                &changed_by_agent,
                &result.content,
            ) {
                Ok(created) => {
                    return self.store.update_graduate_dispatch_status(
                        &run.lab_run_id,
                        &running.dispatch_id,
                        GraduateDispatchStatus::Succeeded,
                        agent_id,
                        Some(created.artifact.artifact_id().to_string()),
                        None,
                    );
                }
                Err(err) => {
                    let _ = self.store.record_run_event(
                        &run.lab_run_id,
                        "graduate_unbound_runtime_verify_failed",
                        serde_json::json!({
                            "task_id": task.task_id,
                            "dispatch_id": running.dispatch_id,
                            "agent_id": agent_id,
                            "error": err.to_string(),
                        }),
                    );
                }
            }
            self.mark_unbound_graduate_success_failed(
                &run,
                &task,
                &running.dispatch_id,
                agent_id,
                &result.content,
            )
        } else {
            if durable_graduate_task_is_completed(&context, &task) {
                match self
                    .sync_graduate_agent_task_latest_with_context(&task.task_id, context.clone())
                {
                    Ok(_) => {
                        return self
                            .store
                            .load_graduate_dispatch(&run.lab_run_id, &running.dispatch_id);
                    }
                    Err(sync_error) => {
                        let error = sync_error.to_string();
                        if let Ok(saved) = self
                            .store
                            .load_graduate_dispatch(&run.lab_run_id, &running.dispatch_id)
                        {
                            if matches!(saved.status, GraduateDispatchStatus::Failed) {
                                return Ok(saved);
                            }
                        }
                        return self.store.update_graduate_dispatch_status(
                            &run.lab_run_id,
                            &running.dispatch_id,
                            GraduateDispatchStatus::Failed,
                            agent_id,
                            None,
                            Some(error),
                        );
                    }
                }
            }
            let error = result
                .error
                .clone()
                .or_else(|| (!result.content.trim().is_empty()).then_some(result.content.clone()))
                .unwrap_or_else(|| "graduate agent execution failed".to_string());
            let _ = self
                .store
                .block_graduate_task(&run.lab_run_id, &task.task_id, &error);
            let _ = self
                .store
                .record_lab_failure(&run.lab_run_id, "graduate_execution", &error);
            self.store.update_graduate_dispatch_status(
                &run.lab_run_id,
                &running.dispatch_id,
                GraduateDispatchStatus::Failed,
                agent_id,
                None,
                Some(error),
            )
        }
    }

    pub(super) fn create_runtime_verified_graduate_result_for_unbound_success(
        &self,
        task: &GraduateTask,
        context: &ToolContext,
        agent_id: Option<&str>,
        parent_changed_files: &[String],
        result_content: &str,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let agent_task_id = graduate_agent_task_id(task);
        let runtime_evidence = runtime_verify_graduate_task_result(
            task,
            context,
            agent_id,
            &agent_task_id,
            parent_changed_files,
        )?;
        let mut evidence_ids = vec![
            format!("agent_task:{agent_task_id}"),
            format!("runtime_verification:{}", task.task_id),
        ];
        if let Some(agent_id) = agent_id {
            evidence_ids.push(format!("agent:{agent_id}"));
        }
        evidence_ids.sort();
        evidence_ids.dedup();
        let preview = compact_result_preview(result_content, 240);
        let task_summary = if preview.is_empty() {
            "Runtime verified graduate task output without bindable GraduateResult JSON."
                .to_string()
        } else {
            format!(
                "Runtime verified graduate task output without bindable GraduateResult JSON. Subagent preview: {preview}"
            )
        };
        self.create_graduate_result_for_task_latest(
            &task.task_id,
            &task_summary,
            runtime_evidence.changed_files,
            runtime_evidence.validation_attempts,
            Vec::new(),
            evidence_ids,
        )
    }

    fn record_graduate_workspace_snapshot(
        &self,
        lab_run_id: &str,
        task_id: &str,
        dispatch_id: &str,
        phase: &str,
        snapshot: &BTreeMap<String, String>,
        changed_paths: &[String],
    ) -> anyhow::Result<()> {
        self.store.record_run_event(
            lab_run_id,
            "lab_graduate_workspace_snapshot",
            serde_json::json!({
                "task_id": task_id,
                "dispatch_id": dispatch_id,
                "phase": phase,
                "dirty_path_count": snapshot.len(),
                "dirty_paths": snapshot.keys().cloned().collect::<Vec<_>>(),
                "changed_path_count": changed_paths.len(),
                "changed_paths": changed_paths,
            }),
        )
    }

    pub(super) fn mark_unbound_graduate_success_failed(
        &self,
        run: &LabRun,
        task: &GraduateTask,
        dispatch_id: &str,
        agent_id: Option<String>,
        result_content: &str,
    ) -> anyhow::Result<GraduateDispatchRecord> {
        let preview = compact_result_preview(result_content, 600);
        let error = if preview.is_empty() {
            "graduate agent completed without bindable GraduateResult JSON".to_string()
        } else {
            format!(
                "graduate agent completed without bindable GraduateResult JSON; result_preview={preview}"
            )
        };
        let _ = self
            .store
            .block_graduate_task(&run.lab_run_id, &task.task_id, &error);
        let _ = self
            .store
            .record_lab_failure(&run.lab_run_id, "graduate_result_contract", &error);
        self.store.update_graduate_dispatch_status(
            &run.lab_run_id,
            dispatch_id,
            GraduateDispatchStatus::Failed,
            agent_id,
            None,
            Some(error),
        )
    }
}
