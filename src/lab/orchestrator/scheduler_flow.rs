use super::*;

impl LabOrchestrator {
    pub async fn run_scheduler_step_latest_with_context(
        &self,
        context: ToolContext,
    ) -> anyhow::Result<LabSchedulerStepResult> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for scheduler step"))?;
        if run.needs_user || matches!(run.status, LabRunStatus::NeedsUser) {
            return Ok(LabSchedulerStepResult {
                lab_run_id: run.lab_run_id,
                action: LabSchedulerStepAction::NeedsUser,
                stage: run.current_stage,
                task_id: None,
                dispatch_id: None,
                message: "LabRun is waiting for user review.".to_string(),
            });
        }
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        self.store.ensure_current_process_holds_fresh_lease(&run)?;

        if matches!(
            run.current_stage.as_str(),
            "postdoc_review" | "professor_review"
        ) {
            let from_stage = run.current_stage.clone();
            let required_artifact_type = transition_for_stage(&from_stage)
                .map(|transition| transition.required_artifact_type)
                .unwrap_or("role artifact");
            return Ok(LabSchedulerStepResult {
                lab_run_id: run.lab_run_id,
                action: LabSchedulerStepAction::Blocked,
                stage: from_stage.clone(),
                task_id: None,
                dispatch_id: None,
                message: format!(
                    "Scheduler stopped at {from_stage}; provider-backed or explicit {required_artifact_type} artifact is required before advancement."
                ),
            });
        }

        if run.current_stage == "graduate_work" {
            let tasks = self.store.list_graduate_tasks(&run.lab_run_id)?;
            if !tasks.iter().any(|task| task.status.is_open())
                && self
                    .store
                    .validate_artifact_gate(&run.lab_run_id, "graduate_work")
                    .is_ok()
            {
                let advanced = self.advance_latest()?;
                return Ok(LabSchedulerStepResult {
                    lab_run_id: advanced.lab_run_id.clone(),
                    action: LabSchedulerStepAction::TickAdvanced,
                    stage: advanced.current_stage.clone(),
                    task_id: None,
                    dispatch_id: None,
                    message: format!(
                        "Scheduler advanced LabRun from graduate_work to {} after verified GraduateResult.",
                        advanced.current_stage
                    ),
                });
            }
            if let Some(task) = tasks.iter().find(|task| {
                matches!(task.status, LabTaskStatus::InProgress)
                    && durable_graduate_task_is_completed(&context, task)
            }) {
                let task_id = task.task_id.clone();
                let synced = match self
                    .sync_graduate_agent_task_latest_with_context(&task_id, context.clone())
                {
                    Ok(synced) => synced,
                    Err(err) => {
                        return Ok(LabSchedulerStepResult {
                            lab_run_id: run.lab_run_id,
                            action: LabSchedulerStepAction::Blocked,
                            stage: "graduate_work".to_string(),
                            task_id: Some(task_id),
                            dispatch_id: None,
                            message: format!(
                                "Graduate durable subagent completion could not be synced: {err}"
                            ),
                        });
                    }
                };
                let refreshed_tasks = self.store.list_graduate_tasks(&run.lab_run_id)?;
                if !refreshed_tasks.iter().any(|task| task.status.is_open())
                    && self
                        .store
                        .validate_artifact_gate(&run.lab_run_id, "graduate_work")
                        .is_ok()
                {
                    let advanced = self.advance_latest()?;
                    return Ok(LabSchedulerStepResult {
                        lab_run_id: advanced.lab_run_id.clone(),
                        action: LabSchedulerStepAction::TickAdvanced,
                        stage: advanced.current_stage.clone(),
                        task_id: Some(task_id),
                        dispatch_id: None,
                        message: format!(
                            "Scheduler synced durable graduate result {} and advanced LabRun from graduate_work to {}.",
                            synced.artifact.artifact_id(),
                            advanced.current_stage
                        ),
                    });
                }
                return Ok(LabSchedulerStepResult {
                    lab_run_id: run.lab_run_id,
                    action: LabSchedulerStepAction::TickAdvanced,
                    stage: "graduate_work".to_string(),
                    task_id: Some(task_id),
                    dispatch_id: None,
                    message: format!(
                        "Scheduler synced durable graduate result {} and will continue remaining graduate tasks.",
                        synced.artifact.artifact_id()
                    ),
                });
            }
            if let Some(task) = tasks
                .iter()
                .find(|task| matches!(task.status, LabTaskStatus::Queued))
            {
                let task_id = task.task_id.clone();
                let dispatch = self
                    .execute_graduate_task_latest_with_context(&task_id, context)
                    .await?;
                return Ok(LabSchedulerStepResult {
                    lab_run_id: run.lab_run_id,
                    action: LabSchedulerStepAction::GraduateDispatched,
                    stage: "graduate_work".to_string(),
                    task_id: Some(task_id.clone()),
                    dispatch_id: Some(dispatch.dispatch_id.clone()),
                    message: format!(
                        "Graduate task {} dispatched with status {:?}.",
                        task_id, dispatch.status
                    ),
                });
            }
            let in_progress = tasks
                .iter()
                .find(|task| matches!(task.status, LabTaskStatus::InProgress));
            return Ok(LabSchedulerStepResult {
                lab_run_id: run.lab_run_id,
                action: LabSchedulerStepAction::Blocked,
                stage: run.current_stage,
                task_id: in_progress.map(|task| task.task_id.clone()),
                dispatch_id: None,
                message: if let Some(task) = in_progress {
                    format!(
                        "Graduate task {} is already in progress; scheduler will not start another task.",
                        task.task_id
                    )
                } else {
                    "graduate_work requires a queued GraduateTask or a bound GraduateResult artifact."
                        .to_string()
                },
            });
        }

        let tick = self.tick_latest()?;
        Ok(LabSchedulerStepResult {
            lab_run_id: tick.lab_run_id,
            action: match tick.status {
                LabTickStatus::Advanced => LabSchedulerStepAction::TickAdvanced,
                LabTickStatus::Blocked => LabSchedulerStepAction::Blocked,
                LabTickStatus::NeedsUser => LabSchedulerStepAction::NeedsUser,
            },
            stage: tick.to_stage,
            task_id: None,
            dispatch_id: None,
            message: match tick.status {
                LabTickStatus::Advanced => {
                    format!("Scheduler advanced LabRun from {}.", tick.from_stage)
                }
                LabTickStatus::Blocked => {
                    format!("Scheduler blocked at {}.", tick.from_stage)
                }
                LabTickStatus::NeedsUser => {
                    format!(
                        "Scheduler stopped at {}; LabRun needs user review.",
                        tick.from_stage
                    )
                }
            },
        })
    }

    pub async fn run_scheduler_steps_latest_with_context(
        &self,
        max_steps: usize,
        context: ToolContext,
    ) -> anyhow::Result<Vec<LabSchedulerStepResult>> {
        let max_steps = max_steps.clamp(1, 20);
        let mut results = Vec::new();
        for _ in 0..max_steps {
            let step = self
                .run_scheduler_step_latest_with_context(context.clone())
                .await?;
            let should_stop = !matches!(step.action, LabSchedulerStepAction::TickAdvanced);
            results.push(step);
            if should_stop {
                break;
            }
        }
        Ok(results)
    }

    pub fn artifact_gate_evidence_context_for_run(
        &self,
        lab_run_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<LabContextEvidenceRefGroup>> {
        let limit = limit.clamp(1, 100);
        let mut groups = Vec::new();
        let artifacts = self.store.list_stage_artifacts(lab_run_id)?;
        for artifact in artifacts.iter().rev().take(limit).rev() {
            let refs = artifact
                .evidence_refs()
                .iter()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            if refs.is_empty() {
                continue;
            }
            groups.push(LabContextEvidenceRefGroup {
                source: format!(
                    "artifact:{}:{:?}",
                    artifact.artifact_id(),
                    artifact.artifact_type()
                ),
                evidence_refs: refs,
            });
        }

        let mut stages = STAGE_TRANSITIONS
            .iter()
            .map(|transition| transition.from_stage)
            .chain([
                "cycle_summary",
                "lab_meeting",
                "compression_summary",
                "blocker_report",
                "postdoc_revision",
            ])
            .collect::<Vec<_>>();
        stages.sort();
        stages.dedup();
        for stage in stages.into_iter().rev().take(limit).rev() {
            let Ok(gate) = self.store.load_artifact_gate(lab_run_id, stage) else {
                continue;
            };
            let refs = gate
                .evidence_refs
                .iter()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            if refs.is_empty() {
                continue;
            }
            groups.push(LabContextEvidenceRefGroup {
                source: format!("gate:{}:{}", gate.stage, gate.required_artifact_type),
                evidence_refs: refs,
            });
        }
        Ok(groups)
    }

    pub fn create_compression_summary_for_latest(
        &self,
        role: LabRole,
    ) -> anyhow::Result<Option<CreatedStageArtifact>> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for compression summary"))?;
        let cost = self.store.cost_summary(&run.lab_run_id)?;
        let evidence = self.store.list_evidence_refs(&run.lab_run_id)?;
        let artifact_gate_refs =
            self.artifact_gate_evidence_context_for_run(&run.lab_run_id, 20)?;
        let packet = build_lab_context_packet_with_evidence_retries_and_artifact_refs(
            &run,
            role,
            &cost,
            &evidence,
            &[],
            &artifact_gate_refs,
        );
        let decision = self
            .store
            .record_compression_decision(evaluate_lab_context_compression(&run, &packet))?;
        if matches!(decision.action, LabCompressionAction::None) {
            return Ok(None);
        }
        self.write_compression_summary_for_decision(&run, &decision, false)
            .map(Some)
    }

    pub fn auto_create_compression_summary_for_decision(
        &self,
        decision: &LabCompressionDecision,
    ) -> anyhow::Result<Option<CreatedStageArtifact>> {
        if matches!(decision.action, LabCompressionAction::None) {
            return Ok(None);
        }
        let run = self.store.load_run(&decision.lab_run_id)?;
        if !run.cost_policy.auto_compress_after_cycle {
            return Ok(None);
        }
        if self.has_compression_summary_for_decision_cycle(decision)? {
            self.store.record_run_event(
                &decision.lab_run_id,
                "lab_auto_compression_summary_skipped",
                serde_json::json!({
                    "decision_id": decision.decision_id,
                    "role": format!("{:?}", decision.role),
                    "cycle_id": decision.cycle_id,
                    "reason": "compression summary already exists for this role and cycle",
                }),
            )?;
            return Ok(None);
        }
        self.write_compression_summary_for_decision(&run, decision, true)
            .map(Some)
    }

    fn write_compression_summary_for_decision(
        &self,
        run: &LabRun,
        decision: &LabCompressionDecision,
        automatic: bool,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let cost = self.store.cost_summary(&run.lab_run_id)?;
        let evidence = self.store.list_evidence_refs(&run.lab_run_id)?;
        let artifact_gate_refs =
            self.artifact_gate_evidence_context_for_run(&run.lab_run_id, 20)?;
        let packet = build_lab_context_packet_with_evidence_retries_and_artifact_refs(
            run,
            decision.role,
            &cost,
            &evidence,
            &[],
            &artifact_gate_refs,
        );
        let artifact_id = format!("artifact_compressionsummary_{}", Uuid::new_v4().simple());
        let evidence_ids = evidence
            .iter()
            .rev()
            .take(20)
            .map(|item| item.evidence_id.clone())
            .collect::<Vec<_>>();
        let retained_layers = packet
            .stable_layers()
            .chain(packet.dynamic_layers())
            .map(|layer| format!("{}:{}", layer.layer, layer.label))
            .collect::<Vec<_>>();
        let compressed_summary = format!(
            "LabRun {} compressed dynamic context for {:?}. Current stage: {}. Active artifact: {}. Evidence refs retained: {}. Cost total tokens: {}. Cache hit rate: {:.1}%.",
            run.lab_run_id,
            decision.role,
            run.current_stage,
            run.resume_cursor.active_artifact_id.as_deref().unwrap_or("none"),
            evidence_ids.len(),
            cost.total_tokens,
            cost.cache_hit_rate_percent()
        );
        let artifact = StageArtifact::CompressionSummary(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            LabArtifactType::CompressionSummary,
            format!("Compression summary for {:?}", decision.role),
            Utc::now(),
            LabCompressionSummary {
                decision_id: decision.decision_id.clone(),
                role: decision.role,
                action: decision.action,
                reason: decision.reason.clone(),
                before_tokens: decision.packet_tokens,
                target_budget_tokens: decision.context_budget_tokens,
                usage_ratio_percent: decision.usage_ratio_percent,
                stable_prefix_fingerprint: decision.stable_prefix_fingerprint.clone(),
                dynamic_tail_fingerprint: decision.dynamic_tail_fingerprint.clone(),
                retained_layers,
                evidence_ids,
                compressed_summary,
                next_action: "Use this summary plus refs-only evidence instead of raw dynamic context where possible.".to_string(),
            },
        ));
        let path = self.store.write_stage_artifact(&artifact)?;
        let report_path = self.store.write_stage_artifact_report(&artifact)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_compression_summary_written",
            serde_json::json!({
                "decision_id": decision.decision_id,
                "artifact_id": artifact.artifact_id(),
                "report_path": report_path.display().to_string(),
                "automatic": automatic,
            }),
        )?;
        let mut gate = ArtifactGate::new(
            "compression_summary",
            "CompressionSummary",
            LabRole::Runtime,
        );
        gate.artifact_id = Some(artifact.artifact_id().to_string());
        gate.next_action = Some("use_compressed_lab_context".to_string());
        gate.validation_status = artifact.validation_status().map(str::to_string);
        gate.evidence_refs.push(path.display().to_string());
        Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        })
    }

    fn has_compression_summary_for_decision_cycle(
        &self,
        decision: &LabCompressionDecision,
    ) -> anyhow::Result<bool> {
        let decisions = self
            .store
            .list_compression_decisions(&decision.lab_run_id)?;
        for artifact in self.store.list_stage_artifacts(&decision.lab_run_id)? {
            let StageArtifact::CompressionSummary(summary) = artifact else {
                continue;
            };
            if summary.body.role != decision.role {
                continue;
            }
            let Some(existing_decision) = decisions
                .iter()
                .find(|item| item.decision_id == summary.body.decision_id)
            else {
                if summary.body.stable_prefix_fingerprint == decision.stable_prefix_fingerprint
                    && summary.body.dynamic_tail_fingerprint == decision.dynamic_tail_fingerprint
                    && summary.body.action == decision.action
                {
                    return Ok(true);
                }
                continue;
            };
            if existing_decision.cycle_id == decision.cycle_id
                && existing_decision.action == decision.action
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn required_gate_for_latest(&self) -> anyhow::Result<ArtifactGate> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for artifact gate"))?;
        self.required_gate_for_run(&run)
    }

    pub fn closeout_latest_from_user_report(&self, note: &str) -> anyhow::Result<LabRun> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found to close out"))?;
        if run.current_stage != "user_report" {
            return Err(anyhow!(
                "LabRun {} is at stage '{}', not user_report",
                run.lab_run_id,
                run.current_stage
            ));
        }
        self.store
            .validate_artifact_gate(&run.lab_run_id, "professor_review")?;
        let gate = self
            .store
            .load_artifact_gate(&run.lab_run_id, "professor_review")?;
        let closeout_status = closeout_status_from_gate(&gate);
        let note = if note.trim().is_empty() {
            format!(
                "closeout derived from final professor_review gate: artifact_id={:?}, validation_status={:?}",
                gate.artifact_id, gate.validation_status
            )
        } else {
            note.trim().to_string()
        };
        self.store
            .closeout_latest_run(closeout_status, note.as_str())
    }

    pub fn continue_latest_from_user_report(&self, note: &str) -> anyhow::Result<LabRun> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found to continue"))?;
        if matches!(
            run.status,
            LabRunStatus::Completed | LabRunStatus::Cancelled | LabRunStatus::Failed
        ) {
            return Err(anyhow!(
                "LabRun {} is terminal and cannot continue: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        if run.current_stage != "user_report" {
            return Err(anyhow!(
                "LabRun {} is at stage '{}', not user_report",
                run.lab_run_id,
                run.current_stage
            ));
        }

        let summary_note = if note.trim().is_empty() {
            "Continue LabRun into the next professor/postdoc/graduate cycle."
        } else {
            note.trim()
        };
        let cycle_summary = self.create_cycle_summary_for_latest(summary_note)?;
        let mut run = self.store.load_run(&run.lab_run_id)?;
        let previous_stage = run.current_stage.clone();
        run.status = LabRunStatus::Active;
        run.current_stage = "professor_discussion".to_string();
        run.internal_owner = LabRole::Professor;
        run.needs_user = false;
        run.blocked_reason = None;
        run.closeout_status = None;
        run.updated_at = Utc::now();
        run.resume_cursor.current_stage = run.current_stage.clone();
        run.resume_cursor.internal_owner = run.internal_owner;
        run.resume_cursor.last_event_seq = run.resume_cursor.last_event_seq.saturating_add(1);
        self.store.save_run(&run)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_cycle_continued",
            serde_json::json!({
                "from_stage": previous_stage,
                "to_stage": run.current_stage,
                "cycle_count": run.cycle_count,
                "cycle_summary_artifact_id": cycle_summary.artifact.artifact_id(),
                "cycle_summary_report_path": cycle_summary.report_path.display().to_string(),
            }),
        )?;
        self.ensure_gate_for_current_stage(&run)?;
        Ok(run)
    }

    pub fn resume_postdoc_revision_latest(&self, note: &str) -> anyhow::Result<LabRun> {
        let mut run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for postdoc revision resume"))?;
        if matches!(
            run.status,
            LabRunStatus::Completed | LabRunStatus::Cancelled | LabRunStatus::Failed
        ) {
            return Err(anyhow!(
                "LabRun {} is terminal and cannot resume postdoc revision: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        let revision = self
            .pending_revision_task_for_run(&run.lab_run_id)?
            .ok_or_else(|| {
                anyhow!(
                    "LabRun {} has no pending postdoc revision task",
                    run.lab_run_id
                )
            })?;
        let previous_stage = run.current_stage.clone();
        let note = note.trim();
        run.status = LabRunStatus::Active;
        run.current_stage = "postdoc_plan".to_string();
        run.internal_owner = LabRole::Postdoc;
        run.needs_user = false;
        run.blocked_reason = None;
        run.closeout_status = None;
        run.updated_at = Utc::now();
        run.resume_cursor.current_stage = run.current_stage.clone();
        run.resume_cursor.internal_owner = run.internal_owner;
        run.resume_cursor.last_event_seq = run.resume_cursor.last_event_seq.saturating_add(1);
        self.store.save_run(&run)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_postdoc_revision_resumed",
            serde_json::json!({
                "from_stage": previous_stage,
                "to_stage": run.current_stage,
                "revision_task_artifact_id": revision.artifact_id,
                "source_review_artifact_id": revision.body.source_review_artifact_id,
                "note": if note.is_empty() {
                    "resume pending professor revision"
                } else {
                    note
                },
            }),
        )?;
        self.ensure_gate_for_current_stage(&run)?;
        Ok(run)
    }

    pub(super) fn ensure_gate_for_current_stage(&self, run: &LabRun) -> anyhow::Result<()> {
        let Some(transition) = transition_for_stage(&run.current_stage) else {
            return Ok(());
        };
        let gate = ArtifactGate::new(
            transition.from_stage,
            transition.required_artifact_type,
            transition.required_owner,
        );
        self.store.write_artifact_gate(&run.lab_run_id, &gate)?;
        Ok(())
    }

    fn required_gate_for_run(&self, run: &LabRun) -> anyhow::Result<ArtifactGate> {
        let transition = transition_for_stage(&run.current_stage).ok_or_else(|| {
            anyhow!(
                "LabRun {} has no configured gate for stage '{}'",
                run.lab_run_id,
                run.current_stage
            )
        })?;
        Ok(ArtifactGate::new(
            transition.from_stage,
            transition.required_artifact_type,
            transition.required_owner,
        ))
    }
}
