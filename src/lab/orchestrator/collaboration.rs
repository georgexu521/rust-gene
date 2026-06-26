//! Collaboration artifacts and meeting flows for LabRun.
//!
//! These methods create Professor/Postdoc coordination artifacts, blocker
//! reports, revision tasks, and steering decisions. They must stay read-only
//! with respect to code execution unless delegated through explicit runtime
//! paths.

use super::*;

impl LabOrchestrator {
    pub fn create_cycle_summary_for_latest(
        &self,
        summary: &str,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for cycle summary"))?;
        let cost = self.store.cost_summary(&run.lab_run_id)?;
        let evidence = self.store.list_evidence_refs(&run.lab_run_id)?;
        let cycle_id = run.cycle_count.to_string();
        let now = Utc::now();
        let artifact_id = format!("artifact_cyclesummary_{}", Uuid::new_v4().simple());
        let evidence_ids = evidence
            .iter()
            .filter(|item| item.cycle_id.as_deref() == Some(cycle_id.as_str()))
            .map(|item| item.evidence_id.clone())
            .collect::<Vec<_>>();
        let mut artifact_evidence_refs = evidence_ids.clone();
        let stage_artifacts = self.store.list_stage_artifacts(&run.lab_run_id)?;
        if let Some(postdoc_summary) = stage_artifacts
            .iter()
            .filter_map(|artifact| match artifact {
                StageArtifact::PostdocIntegrationSummary(summary) => Some(summary),
                _ => None,
            })
            .next_back()
        {
            artifact_evidence_refs.push(format!("artifact:{}", postdoc_summary.artifact_id));
            artifact_evidence_refs.extend(postdoc_summary.evidence_refs.iter().cloned());
        }
        if let Some(professor_review) = stage_artifacts
            .iter()
            .filter_map(|artifact| match artifact {
                StageArtifact::ProfessorReview(review) => Some(review),
                _ => None,
            })
            .next_back()
        {
            artifact_evidence_refs.push(format!("artifact:{}", professor_review.artifact_id));
            artifact_evidence_refs.extend(professor_review.evidence_refs.iter().cloned());
        }
        artifact_evidence_refs.sort();
        artifact_evidence_refs.dedup();
        let mut artifact = StageArtifact::CycleSummary(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            LabArtifactType::CycleSummary,
            format!("Cycle {} summary", cycle_id),
            now,
            LabCycleSummary {
                cycle_id: cycle_id.clone(),
                current_stage: run.current_stage.clone(),
                owner: run.internal_owner,
                summary: note_or_placeholder(summary, "Cycle summary draft."),
                completed_items: Vec::new(),
                evidence_ids,
                total_tokens: cost.total_tokens,
                cache_hit_rate_percent: cost.cache_hit_rate_percent(),
                estimated_cost_usd: cost.estimated_cost_usd,
                next_action: "Continue LabRun orchestration from the current stage.".to_string(),
            },
        ));
        if let StageArtifact::CycleSummary(envelope) = &mut artifact {
            envelope.evidence_refs = artifact_evidence_refs.clone();
            envelope.validation_status = Some("read_only_runtime_summary".to_string());
        }
        let path = self.store.write_stage_artifact(&artifact)?;
        let report_path = self.store.write_stage_artifact_report(&artifact)?;

        let mut saved = self.store.load_run(&run.lab_run_id)?;
        saved.cycle_count = saved.cycle_count.saturating_add(1);
        saved.updated_at = Utc::now();
        self.store.save_run(&saved)?;
        self.store.record_run_event(
            &saved.lab_run_id,
            "lab_cycle_summarized",
            serde_json::json!({
                "cycle_id": cycle_id,
                "artifact_id": artifact.artifact_id(),
                "report_path": report_path.display().to_string(),
            }),
        )?;

        let mut gate = ArtifactGate::new("cycle_summary", "CycleSummary", LabRole::Runtime);
        gate.artifact_id = Some(artifact.artifact_id().to_string());
        gate.next_action = Some("continue_labrun".to_string());
        gate.validation_status = artifact.validation_status().map(str::to_string);
        gate.evidence_refs.push(path.display().to_string());
        gate.evidence_refs.extend(artifact_evidence_refs);
        gate.evidence_refs.sort();
        gate.evidence_refs.dedup();
        Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        })
    }

    pub fn create_meeting_summary_for_latest(
        &self,
        topic: Option<&str>,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let mut run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for meeting"))?;
        let meeting_id = format!("meeting_{}", Uuid::new_v4().simple());
        let topic = topic
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("general LabRun progress review")
            .to_string();
        let cost = self.store.cost_summary(&run.lab_run_id)?;
        let evidence = self.store.list_evidence_refs(&run.lab_run_id)?;
        let evidence_ids = evidence
            .iter()
            .rev()
            .take(20)
            .map(|item| item.evidence_id.clone())
            .collect::<Vec<_>>();
        let open_tasks = self.store.latest_graduate_tasks()?;
        let blocked_tasks = open_tasks
            .iter()
            .filter(|task| matches!(task.status, LabTaskStatus::Blocked))
            .map(|task| task.task_id.clone())
            .collect::<Vec<_>>();
        let next_actions = if run.needs_user || matches!(run.status, LabRunStatus::NeedsUser) {
            vec!["ask_user_for_direction".to_string()]
        } else if !blocked_tasks.is_empty() {
            vec![format!(
                "resolve blocked graduate tasks: {}",
                blocked_tasks.join(", ")
            )]
        } else if !run.open_task_ids.is_empty() {
            vec![format!(
                "continue open graduate tasks: {}",
                run.open_task_ids.join(", ")
            )]
        } else {
            vec![format!("continue stage {}", run.current_stage)]
        };
        let artifact_evidence_refs = evidence_ids.clone();
        let artifact_id = format!("artifact_labmeeting_{}", Uuid::new_v4().simple());
        let mut artifact = StageArtifact::LabMeetingSummary(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            LabArtifactType::LabMeetingSummary,
            format!("Lab meeting summary for {}", topic),
            Utc::now(),
            LabMeetingSummary {
                meeting_id: meeting_id.clone(),
                topic: topic.clone(),
                current_stage: run.current_stage.clone(),
                professor_view: format!(
                    "Strategic review should stay focused on stage '{}' and avoid scope expansion without sponsor approval.",
                    run.current_stage
                ),
                postdoc_view: format!(
                    "Implementation state: {} open task(s), {} artifact(s), {} failure(s).",
                    run.open_task_ids.len(),
                    run.artifact_ids.len(),
                    run.failure_count
                ),
                decision: "continue_current_plan".to_string(),
                next_actions,
                evidence_ids: evidence_ids.clone(),
                total_tokens: cost.total_tokens,
                cache_hit_rate_percent: cost.cache_hit_rate_percent(),
            },
        ));
        if let StageArtifact::LabMeetingSummary(envelope) = &mut artifact {
            envelope.validation_status = Some("read_only_runtime_summary".to_string());
            envelope.evidence_refs = artifact_evidence_refs.clone();
        }
        let path = self.store.write_stage_artifact(&artifact)?;
        let report_path = self.store.write_stage_artifact_report(&artifact)?;
        run.meeting_ids.push(meeting_id.clone());
        run.artifact_ids.push(artifact.artifact_id().to_string());
        run.updated_at = Utc::now();
        self.store.save_run(&run)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_meeting_summary_written",
            serde_json::json!({
                "meeting_id": meeting_id,
                "topic": topic,
                "artifact_id": artifact.artifact_id(),
                "report_path": report_path.display().to_string(),
                "evidence_refs": artifact_evidence_refs,
            }),
        )?;
        let mut gate = ArtifactGate::new("lab_meeting", "LabMeetingSummary", LabRole::Runtime);
        gate.artifact_id = Some(artifact.artifact_id().to_string());
        gate.validation_status = artifact.validation_status().map(str::to_string);
        gate.evidence_refs.push(path.display().to_string());
        gate.evidence_refs.extend(artifact_evidence_refs);
        gate.evidence_refs.sort();
        gate.evidence_refs.dedup();
        gate.next_action = Some("continue_labrun".to_string());
        Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        })
    }

    pub fn create_graduate_result_for_task_latest(
        &self,
        task_id: &str,
        task_summary: &str,
        changed_files: Vec<String>,
        validation_attempts: Vec<String>,
        blockers: Vec<String>,
        evidence_ids: Vec<String>,
    ) -> anyhow::Result<CreatedStageArtifact> {
        self.create_graduate_result_for_task_latest_with_provenance(
            task_id,
            task_summary,
            changed_files,
            validation_attempts,
            blockers,
            evidence_ids,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create_graduate_result_for_task_latest_with_provenance(
        &self,
        task_id: &str,
        task_summary: &str,
        changed_files: Vec<String>,
        validation_attempts: Vec<String>,
        blockers: Vec<String>,
        evidence_ids: Vec<String>,
        provenance: Option<LabEvidenceProvenance>,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for graduate result"))?;
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        let task = self.store.load_graduate_task(&run.lab_run_id, task_id)?;
        if task.lab_run_id != run.lab_run_id {
            return Err(anyhow!(
                "graduate task {} belongs to {}, not {}",
                task.task_id,
                task.lab_run_id,
                run.lab_run_id
            ));
        }
        let changed_files = clean_string_vec(changed_files);
        validate_changed_files_within_scope(&task.allowed_scope, &changed_files)?;
        let validation_attempts = clean_string_vec(validation_attempts);
        let blockers = clean_string_vec(blockers);
        let evidence_ids = clean_string_vec(evidence_ids);

        let artifact_id = format!("artifact_graduateresult_{}", Uuid::new_v4().simple());
        let mut provenance = provenance.unwrap_or_else(|| LabEvidenceProvenance {
            lab_run_id: Some(run.lab_run_id.clone()),
            cycle_id: task.cycle_id.clone(),
            source_postdoc_plan_artifact_id: task.source_postdoc_plan_artifact_id.clone(),
            graduate_task_id: Some(task.task_id.clone()),
            ..LabEvidenceProvenance::default()
        });
        provenance.lab_run_id = Some(run.lab_run_id.clone());
        provenance.cycle_id = provenance.cycle_id.or_else(|| task.cycle_id.clone());
        provenance.source_postdoc_plan_artifact_id = provenance
            .source_postdoc_plan_artifact_id
            .or_else(|| task.source_postdoc_plan_artifact_id.clone());
        provenance.graduate_task_id = Some(task.task_id.clone());
        provenance.graduate_result_artifact_id = Some(artifact_id.clone());
        let mut envelope = LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            LabArtifactType::GraduateResult,
            format!("Graduate result for {}", task.title),
            Utc::now(),
            GraduateResult {
                task_summary: note_or_placeholder(task_summary, "Graduate task result."),
                changed_files,
                validation_attempts,
                blockers,
                handoff_to_postdoc: format!(
                    "Review graduate task {}. This result is a claim until parent verification.",
                    task.task_id
                ),
                provenance,
            },
        );
        envelope.evidence_refs = evidence_ids.clone();
        envelope.validation_status = Some("subagent_report_not_parent_verified".to_string());
        let artifact = StageArtifact::GraduateResult(envelope);
        let path = self.store.write_stage_artifact(&artifact)?;
        let report_path = self.store.write_stage_artifact_report(&artifact)?;
        self.store.complete_graduate_task(
            &run.lab_run_id,
            &task.task_id,
            artifact.artifact_id(),
            evidence_ids,
        )?;

        let gate = if run.current_stage == "graduate_work" {
            self.write_satisfied_gate_for_latest(
                artifact.artifact_id(),
                artifact.validation_status(),
                Some(&path.display().to_string()),
            )?
        } else {
            let mut gate = ArtifactGate::new(
                "graduate_work",
                LabArtifactType::GraduateResult.as_str(),
                LabRole::Graduate,
            );
            gate.artifact_id = Some(artifact.artifact_id().to_string());
            gate.validation_status = artifact.validation_status().map(str::to_string);
            gate.next_action = Some("postdoc_review".to_string());
            gate.evidence_refs.push(path.display().to_string());
            gate
        };

        self.store.record_run_event(
            &run.lab_run_id,
            "graduate_result_artifact_bound",
            serde_json::json!({
                "task_id": task.task_id,
                "artifact_id": artifact.artifact_id(),
                "report_path": report_path.display().to_string(),
                "validation_status": artifact.validation_status(),
                "provenance": match &artifact {
                    StageArtifact::GraduateResult(envelope) => {
                        serde_json::to_value(&envelope.body.provenance).unwrap_or_default()
                    }
                    _ => serde_json::Value::Null,
                },
            }),
        )?;

        Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        })
    }

    pub fn bind_graduate_agent_json_for_task_latest(
        &self,
        task_id: &str,
        agent_json: &str,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let value = serde_json::from_str::<Value>(agent_json.trim())
            .map_err(|err| anyhow!("invalid graduate agent JSON: {err}"))?;
        let parsed = parse_graduate_agent_result(Some(&value), agent_json)
            .ok_or_else(|| anyhow!("graduate agent JSON did not match GraduateResult contract"))?;
        self.create_graduate_result_for_task_latest(
            task_id,
            &parsed.task_summary,
            parsed.changed_files,
            parsed.validation_attempts,
            parsed.blockers,
            parsed.evidence_ids,
        )
    }

    pub fn sync_graduate_agent_task_latest_with_context(
        &self,
        task_id: &str,
        context: ToolContext,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for graduate task sync"))?;
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        let task = self.store.load_graduate_task(&run.lab_run_id, task_id)?;
        let agent_task_id = graduate_agent_task_id(&task);
        let session_store = context
            .session_store
            .as_ref()
            .ok_or_else(|| anyhow!("graduate task sync requires a SessionStore"))?;
        let state = session_store
            .agent_task_state(&context.session_id, &agent_task_id)?
            .ok_or_else(|| anyhow!("no durable subagent state found for {agent_task_id}"))?;
        if state.profile.as_deref() != Some("lab-graduate") {
            return Err(anyhow!(
                "durable subagent task {} is profile {:?}, not lab-graduate",
                agent_task_id,
                state.profile
            ));
        }
        if state.status != "completed" {
            return Err(anyhow!(
                "durable subagent task {} is not completed: {}",
                agent_task_id,
                state.status
            ));
        }
        let Some(artifact_id) = state.result_artifact_id else {
            let error = format!("durable subagent task {agent_task_id} has no result artifact");
            self.fail_synced_graduate_task(&run, &task, &state.agent_id, &error)?;
            return Err(anyhow!(error));
        };
        let artifact = match session_store.agent_artifact(&context.session_id, artifact_id)? {
            Some(artifact) => artifact,
            None => {
                let error = format!("agent artifact {artifact_id} not found for {agent_task_id}");
                self.fail_synced_graduate_task(&run, &task, &state.agent_id, &error)?;
                return Err(anyhow!(error));
            }
        };
        if artifact.status != "completed" {
            let error = format!(
                "agent artifact {} for {} is not completed: {}",
                artifact_id, agent_task_id, artifact.status
            );
            self.fail_synced_graduate_task(&run, &task, &state.agent_id, &error)?;
            return Err(anyhow!(error));
        }
        let provider_policy =
            crate::lab::provider_certification::graduate_provider_execution_policy(&context);
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_graduate_provider_execution_policy",
            serde_json::json!({
                "task_id": task.task_id,
                "agent_task_id": agent_task_id,
                "certification": provider_policy.certification.as_str(),
                "execution_allowed": provider_policy.execution_allowed,
                "isolated_worktree_required": provider_policy.isolated_worktree_required,
                "controlled_validation_required": provider_policy.controlled_validation_required,
                "postdoc_audit_required": provider_policy.postdoc_audit_required,
                "user_override_required": provider_policy.user_override_required,
                "proof_labels": provider_policy.proof_labels.clone(),
                "reason": provider_policy.reason.clone(),
            }),
        )?;
        if !provider_policy.execution_allowed {
            let error = provider_policy.reason.clone();
            self.fail_synced_graduate_task(&run, &task, &state.agent_id, &error)?;
            return Err(anyhow!(error));
        }
        let mut parsed = match parse_graduate_agent_result(
            Some(&artifact.payload),
            &artifact.output,
        ) {
            Some(parsed) => parsed,
            None => {
                let error = format!(
                    "completed graduate subagent {} result artifact {} did not match GraduateResult contract",
                    agent_task_id, artifact_id
                );
                self.fail_synced_graduate_task(&run, &task, &state.agent_id, &error)?;
                return Err(anyhow!(error));
            }
        };
        let dispatch =
            self.latest_dispatch_for_agent_task(&run.lab_run_id, &task.task_id, &agent_task_id)?;
        let runtime_evidence = match runtime_verify_graduate_task_result(
            &task,
            &context,
            Some(&state.agent_id),
            &agent_task_id,
            dispatch
                .as_ref()
                .map(|dispatch| dispatch.dispatch_id.as_str()),
            &[],
            &provider_policy,
        ) {
            Ok(evidence) => evidence,
            Err(err) => {
                let error = format!(
                    "graduate durable task {} failed runtime verification: {}",
                    agent_task_id, err
                );
                self.fail_synced_graduate_task(&run, &task, &state.agent_id, &error)?;
                return Err(anyhow!(error));
            }
        };
        parsed.changed_files = runtime_evidence.changed_files;
        parsed
            .validation_attempts
            .extend(runtime_evidence.validation_attempts);
        parsed.validation_attempts.sort();
        parsed.validation_attempts.dedup();
        parsed.evidence_ids.extend(runtime_evidence.evidence_refs);
        parsed
            .evidence_ids
            .push(format!("agent_task:{}", agent_task_id));
        parsed
            .evidence_ids
            .push(format!("agent_artifact:{}", artifact_id));
        parsed.evidence_ids.extend(
            provider_policy
                .proof_labels
                .iter()
                .map(|label| format!("provider_policy:{label}")),
        );
        parsed.evidence_ids.sort();
        parsed.evidence_ids.dedup();

        let created = self.create_graduate_result_for_task_latest_with_provenance(
            &task.task_id,
            &parsed.task_summary,
            parsed.changed_files,
            parsed.validation_attempts,
            parsed.blockers,
            parsed.evidence_ids,
            Some(runtime_evidence.provenance),
        )?;
        if let Some(dispatch) = dispatch {
            self.store.update_graduate_dispatch_status(
                &run.lab_run_id,
                &dispatch.dispatch_id,
                GraduateDispatchStatus::Succeeded,
                Some(state.agent_id.clone()),
                Some(created.artifact.artifact_id().to_string()),
                None,
            )?;
        }
        self.store.record_run_event(
            &run.lab_run_id,
            "graduate_agent_task_synced",
            serde_json::json!({
                "task_id": task.task_id,
                "agent_task_id": agent_task_id,
                "agent_id": state.agent_id,
                "agent_artifact_id": artifact_id,
                "graduate_result_artifact_id": created.artifact.artifact_id(),
            }),
        )?;
        Ok(created)
    }

    fn latest_dispatch_for_agent_task(
        &self,
        lab_run_id: &str,
        task_id: &str,
        agent_task_id: &str,
    ) -> anyhow::Result<Option<GraduateDispatchRecord>> {
        Ok(self
            .store
            .list_graduate_dispatches(lab_run_id)?
            .into_iter()
            .rev()
            .find(|dispatch| {
                dispatch.task_id == task_id
                    && dispatch.agent_tool_params["task_id"].as_str() == Some(agent_task_id)
            }))
    }

    fn fail_synced_graduate_task(
        &self,
        run: &LabRun,
        task: &GraduateTask,
        agent_id: &str,
        error: &str,
    ) -> anyhow::Result<()> {
        if task.status.is_open() {
            self.store
                .block_graduate_task(&run.lab_run_id, &task.task_id, error)?;
        }
        self.store
            .record_lab_failure(&run.lab_run_id, "graduate_agent_task_sync", error)?;
        let agent_task_id = graduate_agent_task_id(task);
        if let Some(dispatch) =
            self.latest_dispatch_for_agent_task(&run.lab_run_id, &task.task_id, &agent_task_id)?
        {
            self.store.update_graduate_dispatch_status(
                &run.lab_run_id,
                &dispatch.dispatch_id,
                GraduateDispatchStatus::Failed,
                Some(agent_id.to_string()),
                None,
                Some(error.to_string()),
            )?;
        }
        Ok(())
    }

    pub fn create_postdoc_integration_summary_for_latest(
        &self,
        note: Option<&str>,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for postdoc integration summary"))?;
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        if run.current_stage != "postdoc_review" {
            return Err(anyhow!(
                "LabRun {} is at stage '{}', not postdoc_review",
                run.lab_run_id,
                run.current_stage
            ));
        }

        let stage_artifacts = self.store.list_stage_artifacts(&run.lab_run_id)?;
        let current_postdoc_plan_id =
            stage_artifacts
                .iter()
                .rev()
                .find_map(|artifact| match artifact {
                    StageArtifact::PostdocPlan(plan)
                        if plan.status == LabArtifactStatus::Accepted
                            && plan.validation_status.as_deref() == Some("accepted") =>
                    {
                        (plan.created_at <= run.updated_at
                            && plan.lab_run_id == run.lab_run_id
                            && plan.stage == "postdoc_plan")
                            .then(|| plan.artifact_id.clone())
                    }
                    _ => None,
                });
        let current_cycle_id = run.cycle_count.to_string();
        let current_tasks = self
            .store
            .list_graduate_tasks(&run.lab_run_id)?
            .into_iter()
            .filter(|task| task.cycle_id.as_deref() == Some(current_cycle_id.as_str()))
            .filter(|task| {
                current_postdoc_plan_id.as_ref().is_none_or(|plan_id| {
                    task.source_postdoc_plan_artifact_id.as_deref() == Some(plan_id.as_str())
                })
            })
            .collect::<Vec<_>>();
        let current_task_ids = current_tasks
            .iter()
            .map(|task| task.task_id.clone())
            .collect::<BTreeSet<_>>();
        let all_graduate_results = stage_artifacts
            .into_iter()
            .filter_map(|artifact| match artifact {
                StageArtifact::GraduateResult(result) => Some(result),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut excluded_results = Vec::new();
        let graduate_results = all_graduate_results
            .into_iter()
            .filter(|result| {
                let provenance = &result.body.provenance;
                let matches_cycle = provenance.cycle_id.as_deref() == Some(current_cycle_id.as_str());
                let matches_plan = current_postdoc_plan_id.as_ref().is_none_or(|plan_id| {
                    provenance.source_postdoc_plan_artifact_id.as_deref()
                        == Some(plan_id.as_str())
                });
                let matches_task = provenance
                    .graduate_task_id
                    .as_deref()
                    .is_some_and(|task_id| current_task_ids.contains(task_id));
                let current = matches_cycle && matches_plan && matches_task;
                if !current {
                    excluded_results.push(format!(
                        "{} excluded from current postdoc integration: cycle_match={} plan_match={} task_match={}",
                        result.artifact_id, matches_cycle, matches_plan, matches_task
                    ));
                }
                current
            })
            .collect::<Vec<_>>();
        if graduate_results.is_empty() {
            return Err(anyhow!(
                "LabRun {} has no GraduateResult artifacts to integrate",
                run.lab_run_id
            ));
        }

        let mut accepted_results = Vec::new();
        let mut remaining_risks = Vec::new();
        let mut evidence_refs = Vec::new();
        remaining_risks.extend(excluded_results);
        if current_postdoc_plan_id.is_none() {
            remaining_risks.push(
                "No accepted current-cycle PostdocPlan provenance found; legacy current-cycle results are treated as unverified."
                    .to_string(),
            );
        }
        for result in &graduate_results {
            evidence_refs.push(format!("artifact:{}", result.artifact_id));
            evidence_refs.extend(result.evidence_refs.iter().cloned());
            let provenance = &result.body.provenance;
            if provenance.dispatch_id.is_none() {
                remaining_risks.push(format!(
                    "{} has no dispatch-bound provenance",
                    result.artifact_id
                ));
            }
            if provenance.validation_event_ids.is_empty() {
                remaining_risks.push(format!(
                    "{} has no task-bound validation event ids",
                    result.artifact_id
                ));
            }
            if provenance.verification_root.is_none() {
                remaining_risks.push(format!(
                    "{} has no verification_root provenance",
                    result.artifact_id
                ));
            }
            if result.body.validation_attempts.is_empty() {
                remaining_risks.push(format!(
                    "{} has no validation attempts recorded",
                    result.artifact_id
                ));
            }
            if result.body.blockers.is_empty() {
                accepted_results.push(format!(
                    "{}: {}",
                    result.artifact_id, result.body.task_summary
                ));
            } else {
                remaining_risks.push(format!(
                    "{} blockers: {}",
                    result.artifact_id,
                    result.body.blockers.join("; ")
                ));
            }
            if result.validation_status.as_deref() == Some("subagent_report_not_parent_verified") {
                remaining_risks.push(format!(
                    "{} is a graduate subagent report pending parent verification",
                    result.artifact_id
                ));
            }
        }
        let worktree_proof =
            collect_graduate_worktree_proof_for_postdoc(&self.store, &run.lab_run_id, 5)?;
        accepted_results.extend(worktree_proof.accepted_results);
        remaining_risks.extend(worktree_proof.remaining_risks);
        evidence_refs.extend(worktree_proof.evidence_refs);
        let workspace_proof =
            collect_graduate_workspace_snapshot_proof_for_postdoc(&self.store, &run.lab_run_id, 8)?;
        accepted_results.extend(workspace_proof.accepted_results);
        remaining_risks.extend(workspace_proof.remaining_risks);
        evidence_refs.extend(workspace_proof.evidence_refs);
        let postdoc_audit =
            collect_postdoc_read_only_audit_proof(&self.store, &run, &graduate_results)?;
        accepted_results.extend(postdoc_audit.accepted_results);
        remaining_risks.extend(postdoc_audit.remaining_risks);
        evidence_refs.extend(postdoc_audit.evidence_refs);

        if accepted_results.is_empty() {
            remaining_risks
                .push("No graduate result is acceptable for professor handoff yet.".to_string());
        }
        remaining_risks.sort();
        remaining_risks.dedup();
        evidence_refs.sort();
        evidence_refs.dedup();

        let validation_status = if remaining_risks.iter().any(|risk| {
            risk.contains("blockers:")
                || risk.contains("no validation attempts")
                || risk.contains("no task-bound validation event ids")
                || risk.contains("no dispatch-bound provenance")
                || risk.contains("no verification_root provenance")
                || risk.contains("No accepted current-cycle PostdocPlan")
                || risk.contains("No graduate result")
                || risk.contains("audit risk:")
        }) {
            "needs_revision"
        } else {
            "postdoc_integrated_pending_professor_review"
        };
        let note = note.unwrap_or("").trim();
        let artifact_id = format!(
            "artifact_postdocintegrationsummary_{}",
            Uuid::new_v4().simple()
        );
        let mut artifact = StageArtifact::PostdocIntegrationSummary(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            LabArtifactType::PostdocIntegrationSummary,
            "Postdoc integration summary".to_string(),
            Utc::now(),
            PostdocIntegrationSummary {
                integration_summary: if note.is_empty() {
                    format!(
                        "Integrated {} graduate result artifact(s) for professor review.",
                        graduate_results.len()
                    )
                } else {
                    note.to_string()
                },
                accepted_results,
                validation_status: validation_status.to_string(),
                remaining_risks: remaining_risks.clone(),
                handoff_to_professor:
                    "Review strategic fit, completeness, validation evidence, remaining risks, and whether repeated failures, stalled progress, blocker reports, sponsor feedback, or poor progress-to-cost ratio require professor steering before user-facing closeout."
                        .to_string(),
            },
        ));
        if let StageArtifact::PostdocIntegrationSummary(envelope) = &mut artifact {
            envelope.evidence_refs = evidence_refs.clone();
            envelope.validation_status = Some(validation_status.to_string());
        }
        let path = self.store.write_stage_artifact(&artifact)?;
        let report_path = self.store.write_stage_artifact_report(&artifact)?;

        let mut gate = ArtifactGate::new(
            "postdoc_review",
            LabArtifactType::PostdocIntegrationSummary.as_str(),
            LabRole::Postdoc,
        );
        gate.artifact_id = Some(artifact.artifact_id().to_string());
        gate.validation_status = artifact.validation_status().map(str::to_string);
        gate.evidence_refs.push(path.display().to_string());
        gate.evidence_refs.extend(evidence_refs.clone());
        gate.evidence_refs.sort();
        gate.evidence_refs.dedup();
        if validation_status == "needs_revision" {
            gate.blockers = remaining_risks;
            gate.next_action = Some("graduate_repair".to_string());
        } else {
            gate.next_action = Some("professor_review".to_string());
        }
        self.store.write_artifact_gate(&run.lab_run_id, &gate)?;
        if gate.is_satisfied() {
            self.store
                .validate_artifact_gate(&run.lab_run_id, "postdoc_review")?;
        }
        self.store.record_run_event(
            &run.lab_run_id,
            "postdoc_integration_summary_written",
            serde_json::json!({
                "artifact_id": artifact.artifact_id(),
                "report_path": report_path.display().to_string(),
                "validation_status": artifact.validation_status(),
                "accepted_results": match &artifact {
                    StageArtifact::PostdocIntegrationSummary(envelope) => &envelope.body.accepted_results,
                    _ => unreachable!(),
                },
                "evidence_refs": evidence_refs,
            }),
        )?;

        Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        })
    }

    pub fn create_professor_review_for_latest(
        &self,
        note: Option<&str>,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for professor review"))?;
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        if run.current_stage != "professor_review" {
            return Err(anyhow!(
                "LabRun {} is at stage '{}', not professor_review",
                run.lab_run_id,
                run.current_stage
            ));
        }

        let integration = self
            .store
            .list_stage_artifacts(&run.lab_run_id)?
            .into_iter()
            .filter_map(|artifact| match artifact {
                StageArtifact::PostdocIntegrationSummary(summary) => Some(summary),
                _ => None,
            })
            .next_back()
            .ok_or_else(|| {
                anyhow!(
                    "LabRun {} has no PostdocIntegrationSummary artifact for professor review",
                    run.lab_run_id
                )
            })?;

        let accepted = false;
        let mut required_revisions = Vec::new();
        required_revisions.extend(integration.body.remaining_risks.clone());
        required_revisions.push(
            "Deterministic professor review is a runtime placeholder; provider or explicit professor review is required before closeout."
                .to_string(),
        );
        if integration.body.accepted_results.is_empty() {
            required_revisions
                .push("Postdoc integration has no accepted graduate results.".to_string());
        }
        if integration.body.validation_status == "needs_revision" {
            required_revisions.push("Postdoc integration is marked needs_revision.".to_string());
        }
        required_revisions.sort();
        required_revisions.dedup();

        let note = note.unwrap_or("").trim();
        let artifact_id = format!("artifact_professorreview_{}", Uuid::new_v4().simple());
        let user_report = format!(
            "LabRun {} is not ready for closeout from deterministic professor review. Required revisions: {}",
            run.lab_run_id,
            required_revisions.join("; ")
        );
        let mut professor_evidence_refs = vec![
            format!("artifact:{}", integration.artifact_id),
            format!("stage:{}", integration.stage),
        ];
        professor_evidence_refs.extend(integration.evidence_refs.iter().cloned());
        professor_evidence_refs.sort();
        professor_evidence_refs.dedup();
        let mut artifact = StageArtifact::ProfessorReview(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            LabArtifactType::ProfessorReview,
            "Professor review".to_string(),
            Utc::now(),
            ProfessorReview {
                review_summary: if note.is_empty() {
                    format!(
                        "Reviewed postdoc integration artifact {}.",
                        integration.artifact_id
                    )
                } else {
                    note.to_string()
                },
                strategic_assessment: if accepted {
                    format!(
                        "Accepted implementation evidence from postdoc integration. Remaining risks for user report: {}",
                        if integration.body.remaining_risks.is_empty() {
                            "none".to_string()
                        } else {
                            integration.body.remaining_risks.join("; ")
                        }
                    )
                } else {
                    "Professor review requires revision before user-facing closeout.".to_string()
                },
                accepted,
                required_revisions: required_revisions.clone(),
                user_report,
            },
        ));
        if let StageArtifact::ProfessorReview(envelope) = &mut artifact {
            envelope.evidence_refs = professor_evidence_refs.clone();
            envelope.validation_status = Some(if accepted {
                "validated".to_string()
            } else {
                "needs_revision".to_string()
            });
        }
        let path = self.store.write_stage_artifact(&artifact)?;
        let report_path = self.store.write_stage_artifact_report(&artifact)?;

        let mut gate = ArtifactGate::new(
            "professor_review",
            LabArtifactType::ProfessorReview.as_str(),
            LabRole::Professor,
        );
        gate.artifact_id = Some(artifact.artifact_id().to_string());
        gate.validation_status = artifact.validation_status().map(str::to_string);
        gate.evidence_refs.push(path.display().to_string());
        gate.evidence_refs.extend(professor_evidence_refs.clone());
        gate.evidence_refs.sort();
        gate.evidence_refs.dedup();
        if accepted {
            gate.next_action = Some("user_report".to_string());
        } else {
            gate.blockers = required_revisions;
            gate.next_action = Some("postdoc_revision".to_string());
        }
        self.store.write_artifact_gate(&run.lab_run_id, &gate)?;
        if gate.is_satisfied() {
            self.store
                .validate_artifact_gate(&run.lab_run_id, "professor_review")?;
        }
        self.store.record_run_event(
            &run.lab_run_id,
            "professor_review_written",
            serde_json::json!({
                "artifact_id": artifact.artifact_id(),
                "postdoc_integration_artifact_id": integration.artifact_id,
                "accepted": accepted,
                "report_path": report_path.display().to_string(),
                "validation_status": artifact.validation_status(),
                "evidence_refs": professor_evidence_refs,
            }),
        )?;
        if !accepted {
            self.create_revision_task_from_professor_review_artifact(&run, &artifact)?;
        }

        Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        })
    }

    pub fn create_revision_task_from_professor_review_artifact(
        &self,
        run: &LabRun,
        artifact: &StageArtifact,
    ) -> anyhow::Result<Option<CreatedStageArtifact>> {
        let StageArtifact::ProfessorReview(review) = artifact else {
            return Ok(None);
        };
        if review.body.accepted {
            return Ok(None);
        }
        let mut required_revisions = review.body.required_revisions.clone();
        if required_revisions.is_empty() {
            required_revisions.push(
                "Professor review rejected closeout without concrete revision items.".to_string(),
            );
        }
        required_revisions.sort();
        required_revisions.dedup();
        let revision_id = format!("revision_{}", Uuid::new_v4().simple());
        let mut revision_evidence_refs = vec![format!("artifact:{}", review.artifact_id)];
        revision_evidence_refs.extend(review.evidence_refs.iter().cloned());
        revision_evidence_refs.sort();
        revision_evidence_refs.dedup();
        let mut revision = StageArtifact::LabRevisionTask(LabArtifactEnvelope::new(
            format!("artifact_labrevisiontask_{}", Uuid::new_v4().simple()),
            run.lab_run_id.clone(),
            LabArtifactType::LabRevisionTask,
            "Professor requested postdoc revision".to_string(),
            Utc::now(),
            LabRevisionTask {
                revision_id,
                source_review_artifact_id: review.artifact_id.clone(),
                assigned_role: LabRole::Postdoc,
                summary: review.body.review_summary.clone(),
                required_revisions,
                evidence_ids: review.evidence_refs.clone(),
                next_action: "Postdoc should revise the integration, create or repair graduate tasks if needed, then rerun postdoc/professor review."
                    .to_string(),
            },
        ));
        if let StageArtifact::LabRevisionTask(envelope) = &mut revision {
            envelope.evidence_refs = revision_evidence_refs.clone();
            envelope.validation_status = Some("not_started".to_string());
        }
        let path = self.store.write_stage_artifact(&revision)?;
        let report_path = self.store.write_stage_artifact_report(&revision)?;
        let mut gate = ArtifactGate::new(
            "postdoc_revision",
            LabArtifactType::LabRevisionTask.as_str(),
            LabRole::Postdoc,
        );
        gate.artifact_id = Some(revision.artifact_id().to_string());
        gate.validation_status = revision.validation_status().map(str::to_string);
        gate.evidence_refs.push(path.display().to_string());
        gate.evidence_refs.extend(revision_evidence_refs.clone());
        gate.evidence_refs.sort();
        gate.evidence_refs.dedup();
        gate.next_action = Some("revise_postdoc_integration".to_string());
        self.store.write_artifact_gate(&run.lab_run_id, &gate)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_revision_task_written",
            serde_json::json!({
                "artifact_id": revision.artifact_id(),
                "source_review_artifact_id": review.artifact_id,
                "assigned_role": "postdoc",
                "report_path": report_path.display().to_string(),
                "evidence_refs": revision_evidence_refs,
            }),
        )?;
        Ok(Some(CreatedStageArtifact {
            artifact: revision,
            path,
            report_path,
            gate,
        }))
    }

    pub fn meeting_recommendation_for_latest(&self) -> anyhow::Result<LabMeetingRecommendation> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for meeting recommendation"))?;
        let tasks = self.store.list_graduate_tasks(&run.lab_run_id)?;
        let dispatches = self.store.list_graduate_dispatches(&run.lab_run_id)?;
        let artifacts = self.store.list_stage_artifacts(&run.lab_run_id)?;
        let cost = self.store.cost_summary(&run.lab_run_id)?;
        let compression_decisions = self.store.list_compression_decisions(&run.lab_run_id)?;
        let blocked_tasks = tasks
            .iter()
            .filter(|task| matches!(task.status, LabTaskStatus::Blocked))
            .collect::<Vec<_>>();
        let mut failed_by_task = BTreeMap::<String, usize>::new();
        for dispatch in dispatches
            .iter()
            .filter(|dispatch| matches!(dispatch.status, GraduateDispatchStatus::Failed))
        {
            *failed_by_task.entry(dispatch.task_id.clone()).or_default() += 1;
        }
        let graduate_retry_limit = run.retry_budget.max_graduate_retries_per_task.max(1) as usize;
        let repeated_failures = failed_by_task
            .iter()
            .filter(|(_, count)| **count >= graduate_retry_limit)
            .map(|(task_id, count)| format!("{task_id}:{count}"))
            .collect::<Vec<_>>();
        let cycle_failure_limit = run.retry_budget.max_cycle_retries.max(1) as u64;

        let mut signals = Vec::new();
        if !blocked_tasks.is_empty() {
            signals.push(format!("blocked_tasks={}", blocked_tasks.len()));
            for task in blocked_tasks.iter().take(5) {
                signals.push(format!(
                    "blocked_task:{}:{}",
                    task.task_id,
                    task.blocker.as_deref().unwrap_or("no blocker recorded")
                ));
            }
        }
        if !repeated_failures.is_empty() {
            signals.push(format!(
                "repeated_failed_dispatches={}",
                repeated_failures.join(",")
            ));
        }
        if run.failure_count >= cycle_failure_limit {
            signals.push(format!(
                "cycle_failure_budget_reached={}/{}",
                run.failure_count, cycle_failure_limit
            ));
        }
        let completed_or_rejected_tasks = tasks
            .iter()
            .filter(|task| {
                matches!(
                    task.status,
                    LabTaskStatus::Completed | LabTaskStatus::Blocked
                )
            })
            .count();
        if completed_or_rejected_tasks > 0 && completed_or_rejected_tasks % 3 == 0 {
            signals.push(format!(
                "mandatory_checkpoint:graduate_task_interval={completed_or_rejected_tasks}/3"
            ));
        }
        let accepted_integrations = artifacts
            .iter()
            .filter_map(|artifact| match artifact {
                StageArtifact::PostdocIntegrationSummary(summary)
                    if summary.body.validation_status != "needs_revision"
                        && !summary.body.accepted_results.is_empty() =>
                {
                    Some(())
                }
                _ => None,
            })
            .count();
        if accepted_integrations > 0 && accepted_integrations % 2 == 0 {
            signals.push(format!(
                "mandatory_checkpoint:postdoc_acceptance_interval={accepted_integrations}/2"
            ));
        }
        if run.current_stage == "user_report" {
            signals.push("mandatory_checkpoint:before_user_closeout".to_string());
        }
        if run.cycle_count > 0 && run.current_stage == "professor_discussion" {
            signals.push(format!(
                "mandatory_checkpoint:cycle_boundary={}",
                run.cycle_count
            ));
        }
        if cost.total_tokens >= run.cost_policy.max_cycle_tokens {
            signals.push(format!(
                "mandatory_checkpoint:cost_budget_reached={}/{}",
                cost.total_tokens, run.cost_policy.max_cycle_tokens
            ));
        }
        if compression_decisions.len() >= 2 {
            signals.push(format!(
                "mandatory_checkpoint:compression_count={}",
                compression_decisions.len()
            ));
        }

        let recommended = !signals.is_empty();
        let topic = if signals
            .iter()
            .any(|signal| signal.starts_with("mandatory_checkpoint:"))
        {
            format!(
                "run mandatory professor checkpoint at stage {}",
                run.current_stage
            )
        } else if !blocked_tasks.is_empty() {
            format!(
                "resolve {} blocked graduate task(s) at stage {}",
                blocked_tasks.len(),
                run.current_stage
            )
        } else if !repeated_failures.is_empty() {
            format!(
                "review repeated graduate dispatch failures at stage {}",
                run.current_stage
            )
        } else if run.failure_count >= cycle_failure_limit {
            format!(
                "review LabRun failure budget at stage {}",
                run.current_stage
            )
        } else {
            format!("no meeting recommended for stage {}", run.current_stage)
        };
        let reason = if recommended {
            "runtime_escalation_signals_present".to_string()
        } else {
            "no blocker or repeated failure signals".to_string()
        };
        Ok(LabMeetingRecommendation {
            lab_run_id: run.lab_run_id,
            recommended,
            topic,
            reason,
            signals,
        })
    }

    pub fn create_meeting_request_for_latest(
        &self,
        recommendation: &LabMeetingRecommendation,
    ) -> anyhow::Result<CreatedStageArtifact> {
        if !recommendation.recommended {
            return Err(anyhow!(
                "cannot create Lab meeting request without a meeting recommendation: {}",
                recommendation.reason
            ));
        }
        let mut run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for meeting request"))?;
        if run.lab_run_id != recommendation.lab_run_id {
            return Err(anyhow!(
                "meeting recommendation belongs to {}, but latest LabRun is {}",
                recommendation.lab_run_id,
                run.lab_run_id
            ));
        }

        let request_id = format!("meeting_request_{}", Uuid::new_v4().simple());
        let artifact_id = format!("artifact_labmeetingrequest_{}", Uuid::new_v4().simple());
        let mut artifact = StageArtifact::LabMeetingRequest(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            LabArtifactType::LabMeetingRequest,
            format!(
                "Runtime escalation signal meeting request for {}",
                run.current_stage
            ),
            Utc::now(),
            LabMeetingRequest {
                request_id: request_id.clone(),
                topic: recommendation.topic.clone(),
                current_stage: run.current_stage.clone(),
                reason: recommendation.reason.clone(),
                signals: recommendation.signals.clone(),
                requested_by: LabRole::Runtime,
                next_action: "open_read_only_lab_meeting".to_string(),
            },
        ));
        if let StageArtifact::LabMeetingRequest(envelope) = &mut artifact {
            envelope.validation_status = Some("runtime_escalation_signal".to_string());
        }
        let path = self.store.write_stage_artifact(&artifact)?;
        let report_path = self.store.write_stage_artifact_report(&artifact)?;
        run.artifact_ids.push(artifact.artifact_id().to_string());
        run.updated_at = Utc::now();
        self.store.save_run(&run)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_meeting_request_written",
            serde_json::json!({
                "request_id": request_id,
                "artifact_id": artifact.artifact_id(),
                "topic": &recommendation.topic,
                "reason": &recommendation.reason,
                "signals": &recommendation.signals,
                "report_path": report_path.display().to_string(),
            }),
        )?;
        let mut gate = ArtifactGate::new(
            "lab_meeting_request",
            "LabMeetingRequest",
            LabRole::Professor,
        );
        gate.artifact_id = Some(artifact.artifact_id().to_string());
        gate.validation_status = artifact.validation_status().map(str::to_string);
        gate.evidence_refs.push(path.display().to_string());
        gate.next_action = Some("open_read_only_lab_meeting".to_string());
        Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        })
    }

    pub fn create_blocker_report_for_latest(
        &self,
        note: Option<&str>,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let mut run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for blocker report"))?;
        let tasks = self.store.list_graduate_tasks(&run.lab_run_id)?;
        let dispatches = self.store.list_graduate_dispatches(&run.lab_run_id)?;
        let mut blocker_evidence_refs = Vec::new();
        let blocked_tasks = tasks
            .iter()
            .filter(|task| matches!(task.status, LabTaskStatus::Blocked))
            .map(|task| {
                blocker_evidence_refs.push(format!("task:{}", task.task_id));
                if let Some(result_artifact_id) = task.result_artifact_id.as_deref() {
                    blocker_evidence_refs.push(format!("artifact:{result_artifact_id}"));
                }
                blocker_evidence_refs.extend(task.evidence_ids.iter().cloned());
                format!(
                    "{}: {} ({})",
                    task.task_id,
                    task.title,
                    task.blocker.as_deref().unwrap_or("no blocker recorded")
                )
            })
            .collect::<Vec<_>>();
        let failed_dispatches = dispatches
            .iter()
            .filter(|dispatch| matches!(dispatch.status, GraduateDispatchStatus::Failed))
            .map(|dispatch| {
                blocker_evidence_refs.push(format!("dispatch:{}", dispatch.dispatch_id));
                blocker_evidence_refs.push(format!("task:{}", dispatch.task_id));
                if let Some(result_artifact_id) = dispatch.result_artifact_id.as_deref() {
                    blocker_evidence_refs.push(format!("artifact:{result_artifact_id}"));
                }
                format!(
                    "{} task={} error={}",
                    dispatch.dispatch_id,
                    dispatch.task_id,
                    dispatch.error.as_deref().unwrap_or("none")
                )
            })
            .collect::<Vec<_>>();
        blocker_evidence_refs.sort();
        blocker_evidence_refs.dedup();
        let recommendation = self.meeting_recommendation_for_latest()?;
        let summary = note
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                if blocked_tasks.is_empty() && failed_dispatches.is_empty() {
                    "No active graduate blockers or failed dispatches recorded.".to_string()
                } else {
                    format!(
                        "{} blocked task(s), {} failed dispatch(es).",
                        blocked_tasks.len(),
                        failed_dispatches.len()
                    )
                }
            });
        let blocker_id = format!("blocker_{}", Uuid::new_v4().simple());
        let artifact_id = format!("artifact_blockerreport_{}", Uuid::new_v4().simple());
        let mut artifact = StageArtifact::LabBlockerReport(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            LabArtifactType::LabBlockerReport,
            format!("Postdoc blocker report for {}", run.current_stage),
            Utc::now(),
            LabBlockerReport {
                blocker_id: blocker_id.clone(),
                current_stage: run.current_stage.clone(),
                summary,
                blocked_tasks,
                failed_dispatches,
                failure_count: run.failure_count,
                recommendation: recommendation.topic,
                handoff_to_professor:
                    "Review whether to revise plan, open a lab meeting, ask user, or continue."
                        .to_string(),
            },
        ));
        if let StageArtifact::LabBlockerReport(envelope) = &mut artifact {
            envelope.evidence_refs = blocker_evidence_refs.clone();
            envelope.validation_status = Some("postdoc_blocker_report".to_string());
        }
        let path = self.store.write_stage_artifact(&artifact)?;
        let report_path = self.store.write_stage_artifact_report(&artifact)?;
        run.artifact_ids.push(artifact.artifact_id().to_string());
        run.blocked_reason = Some(blocker_id.clone());
        run.updated_at = Utc::now();
        self.store.save_run(&run)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_blocker_report_written",
            serde_json::json!({
                "blocker_id": blocker_id,
                "artifact_id": artifact.artifact_id(),
                "report_path": report_path.display().to_string(),
                "evidence_refs": blocker_evidence_refs.clone(),
            }),
        )?;
        let mut gate = ArtifactGate::new("blocker_report", "LabBlockerReport", LabRole::Postdoc);
        gate.artifact_id = Some(artifact.artifact_id().to_string());
        gate.validation_status = artifact.validation_status().map(str::to_string);
        gate.evidence_refs.push(path.display().to_string());
        gate.evidence_refs.extend(blocker_evidence_refs);
        gate.evidence_refs.sort();
        gate.evidence_refs.dedup();
        gate.next_action = Some("professor_review_blocker".to_string());
        Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        })
    }

    pub fn escalate_latest_blocker_to_professor_review(&self) -> anyhow::Result<LabRun> {
        let mut run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for blocker escalation"))?;
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        let blocked_reason = run.blocked_reason.clone().ok_or_else(|| {
            anyhow!(
                "LabRun {} has no blocker report to escalate",
                run.lab_run_id
            )
        })?;
        let previous_stage = run.current_stage.clone();
        run.current_stage = "professor_review".to_string();
        run.internal_owner = LabRole::Professor;
        run.resume_cursor.current_stage = run.current_stage.clone();
        run.resume_cursor.internal_owner = run.internal_owner;
        run.resume_cursor.last_event_seq = run.resume_cursor.last_event_seq.saturating_add(1);
        run.updated_at = Utc::now();
        self.store.save_run(&run)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_blocker_escalated_to_professor",
            serde_json::json!({
                "from_stage": previous_stage,
                "to_stage": run.current_stage,
                "blocked_reason": blocked_reason,
            }),
        )?;
        self.ensure_gate_for_current_stage(&run)?;
        Ok(run)
    }
}
