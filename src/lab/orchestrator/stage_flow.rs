//! User-visible LabRun stage transition operations.
//!
//! This module approves proposals, satisfies artifact gates, advances stages,
//! and computes required gates. All stage changes should pass through these
//! typed transitions rather than direct state mutation.

use super::*;

impl LabOrchestrator {
    pub fn approve_proposal(&self, proposal_id: &str) -> anyhow::Result<LabRun> {
        let run = self.store.approve_proposal(proposal_id)?;
        self.ensure_gate_for_current_stage(&run)?;
        Ok(run)
    }

    pub fn advance_latest(&self) -> anyhow::Result<LabRun> {
        let mut run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found to advance"))?;
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        let transition = transition_for_stage(&run.current_stage).ok_or_else(|| {
            anyhow!(
                "LabRun {} has no configured transition from stage '{}'",
                run.lab_run_id,
                run.current_stage
            )
        })?;

        self.store
            .validate_artifact_gate(&run.lab_run_id, transition.from_stage)?;
        let previous_stage = run.current_stage.clone();
        let now = Utc::now();
        run.current_stage = transition.to_stage.to_string();
        run.internal_owner = transition.next_owner;
        run.needs_user = transition.to_stage == "user_report";
        if run.needs_user {
            run.status = LabRunStatus::NeedsUser;
        }
        run.updated_at = now;
        run.resume_cursor.current_stage = run.current_stage.clone();
        run.resume_cursor.internal_owner = run.internal_owner;
        run.resume_cursor.last_event_seq = run.resume_cursor.last_event_seq.saturating_add(1);
        self.store.save_run(&run)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_stage_advanced",
            serde_json::json!({
                "from_stage": previous_stage,
                "to_stage": run.current_stage,
                "internal_owner": format!("{:?}", run.internal_owner),
            }),
        )?;
        self.ensure_gate_for_current_stage(&run)?;
        Ok(run)
    }

    pub fn tick_latest(&self) -> anyhow::Result<LabTickResult> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found to tick"))?;
        if run.needs_user || matches!(run.status, LabRunStatus::NeedsUser) {
            return Ok(LabTickResult {
                lab_run_id: run.lab_run_id.clone(),
                status: LabTickStatus::NeedsUser,
                from_stage: run.current_stage.clone(),
                to_stage: run.current_stage.clone(),
                owner: run.internal_owner,
                artifact_id: None,
                report_path: None,
                compression_artifact_id: None,
            });
        }
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        if transition_for_stage(&run.current_stage).is_none() {
            return Ok(LabTickResult {
                lab_run_id: run.lab_run_id.clone(),
                status: LabTickStatus::NeedsUser,
                from_stage: run.current_stage.clone(),
                to_stage: run.current_stage.clone(),
                owner: run.internal_owner,
                artifact_id: None,
                report_path: None,
                compression_artifact_id: None,
            });
        }

        let from_stage = run.current_stage.clone();
        let transition = transition_for_stage(&from_stage).expect("transition checked above");
        if let Err(err) = self
            .store
            .validate_artifact_gate(&run.lab_run_id, transition.from_stage)
        {
            self.store.record_run_event(
                &run.lab_run_id,
                "lab_tick_blocked_missing_role_artifact",
                serde_json::json!({
                    "stage": from_stage,
                    "required_artifact_type": transition.required_artifact_type,
                    "required_owner": format!("{:?}", transition.required_owner),
                    "reason": err.to_string(),
                }),
            )?;
            return Ok(LabTickResult {
                lab_run_id: run.lab_run_id,
                status: LabTickStatus::Blocked,
                from_stage: from_stage.clone(),
                to_stage: from_stage,
                owner: run.internal_owner,
                artifact_id: None,
                report_path: None,
                compression_artifact_id: None,
            });
        }
        let advanced = self.advance_latest()?;
        let compression_artifact_id = if advanced.cost_policy.auto_compress_after_cycle {
            self.create_compression_summary_for_latest(advanced.internal_owner)?
                .map(|created| created.artifact.artifact_id().to_string())
        } else {
            None
        };
        self.store.record_run_event(
            &advanced.lab_run_id,
            "lab_tick_completed",
            serde_json::json!({
                "from_stage": from_stage,
                "to_stage": advanced.current_stage,
                "compression_artifact_id": compression_artifact_id,
                "needs_user": advanced.needs_user,
            }),
        )?;

        Ok(LabTickResult {
            lab_run_id: advanced.lab_run_id,
            status: if advanced.needs_user {
                LabTickStatus::NeedsUser
            } else {
                LabTickStatus::Advanced
            },
            from_stage,
            to_stage: advanced.current_stage,
            owner: advanced.internal_owner,
            artifact_id: None,
            report_path: None,
            compression_artifact_id,
        })
    }

    pub fn write_satisfied_gate_for_latest(
        &self,
        artifact_id: &str,
        validation_status: Option<&str>,
        evidence_ref: Option<&str>,
    ) -> anyhow::Result<ArtifactGate> {
        let evidence_refs = evidence_ref.into_iter().collect::<Vec<_>>();
        self.write_satisfied_gate_for_latest_with_evidence_refs(
            artifact_id,
            validation_status,
            &evidence_refs,
        )
    }

    pub fn write_satisfied_gate_for_latest_with_evidence_refs(
        &self,
        artifact_id: &str,
        validation_status: Option<&str>,
        evidence_refs: &[&str],
    ) -> anyhow::Result<ArtifactGate> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for artifact gate"))?;
        let transition = transition_for_stage(&run.current_stage).ok_or_else(|| {
            anyhow!(
                "LabRun {} has no configured gate for stage '{}'",
                run.lab_run_id,
                run.current_stage
            )
        })?;
        let mut gate = ArtifactGate::new(
            transition.from_stage,
            transition.required_artifact_type,
            transition.required_owner,
        );
        gate.artifact_id = Some(artifact_id.trim().to_string());
        gate.next_action = Some(transition.next_action.to_string());
        gate.validation_status = validation_status.map(|value| value.trim().to_string());
        for evidence_ref in evidence_refs
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            gate.evidence_refs.push(evidence_ref.to_string());
        }
        gate.evidence_refs.sort();
        gate.evidence_refs.dedup();
        self.store.write_artifact_gate(&run.lab_run_id, &gate)?;
        self.store
            .validate_artifact_gate(&run.lab_run_id, transition.from_stage)?;
        Ok(gate)
    }

    pub fn accept_artifact_latest(
        &self,
        artifact_id: &str,
        note: &str,
    ) -> anyhow::Result<ArtifactGate> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for artifact acceptance"))?;
        let artifact = self.store.review_stage_artifact(
            &run.lab_run_id,
            artifact_id,
            LabArtifactStatus::Accepted,
            "accepted",
            Some(note),
        )?;
        let transition = transition_for_stage(artifact.stage()).ok_or_else(|| {
            anyhow!(
                "LabRun {} has no configured transition for artifact stage '{}'",
                run.lab_run_id,
                artifact.stage()
            )
        })?;
        let mut gate = ArtifactGate::new(
            transition.from_stage,
            transition.required_artifact_type,
            transition.required_owner,
        );
        gate.artifact_id = Some(artifact.artifact_id().to_string());
        gate.validation_status = artifact.validation_status().map(str::to_string);
        gate.next_action = Some(transition.next_action.to_string());
        gate.evidence_refs.push(
            self.store
                .root()
                .join("runs")
                .join(&run.lab_run_id)
                .join("artifacts")
                .join(format!("{}.json", artifact.artifact_id()))
                .display()
                .to_string(),
        );
        self.store.write_artifact_gate(&run.lab_run_id, &gate)?;
        self.store
            .validate_artifact_gate(&run.lab_run_id, transition.from_stage)?;
        let generated_tasks = self.queue_graduate_tasks_from_postdoc_plan_artifact(&artifact)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_artifact_accepted",
            serde_json::json!({
                "artifact_id": artifact.artifact_id(),
                "stage": artifact.stage(),
                "note": note.trim(),
                "generated_graduate_task_ids": generated_tasks
                    .iter()
                    .map(|task| task.task_id.as_str())
                    .collect::<Vec<_>>(),
            }),
        )?;
        Ok(gate)
    }

    pub fn revise_artifact_latest(
        &self,
        artifact_id: &str,
        note: &str,
    ) -> anyhow::Result<ArtifactGate> {
        let note = note.trim();
        if note.is_empty() {
            return Err(anyhow!("revision note cannot be empty"));
        }
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for artifact revision"))?;
        let artifact = self.store.review_stage_artifact(
            &run.lab_run_id,
            artifact_id,
            LabArtifactStatus::NeedsRevision,
            "needs_revision",
            Some(note),
        )?;
        let transition = transition_for_stage(artifact.stage()).ok_or_else(|| {
            anyhow!(
                "LabRun {} has no configured transition for artifact stage '{}'",
                run.lab_run_id,
                artifact.stage()
            )
        })?;
        let mut gate = ArtifactGate::new(
            transition.from_stage,
            transition.required_artifact_type,
            transition.required_owner,
        );
        gate.artifact_id = Some(artifact.artifact_id().to_string());
        gate.validation_status = artifact.validation_status().map(str::to_string);
        gate.next_action = Some("revise_artifact".to_string());
        gate.blockers.push(note.to_string());
        self.store.write_artifact_gate(&run.lab_run_id, &gate)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_artifact_revision_requested",
            serde_json::json!({
                "artifact_id": artifact.artifact_id(),
                "stage": artifact.stage(),
                "note": note,
            }),
        )?;
        Ok(gate)
    }

    fn queue_graduate_tasks_from_postdoc_plan_artifact(
        &self,
        artifact: &StageArtifact,
    ) -> anyhow::Result<Vec<GraduateTask>> {
        let StageArtifact::PostdocPlan(plan_artifact) = artifact else {
            return Ok(Vec::new());
        };
        if !matches!(plan_artifact.status, LabArtifactStatus::Accepted)
            || plan_artifact.validation_status.as_deref() != Some("accepted")
        {
            return Ok(Vec::new());
        }

        let marker = postdoc_plan_task_marker(&plan_artifact.artifact_id);
        let existing_tasks = self.store.list_graduate_tasks(&plan_artifact.lab_run_id)?;
        if existing_tasks
            .iter()
            .any(|task| task.instructions.contains(&marker))
        {
            return Ok(Vec::new());
        }

        let body = &plan_artifact.body;
        let slices = if body.slices.is_empty() {
            vec!["Implement the accepted postdoc plan.".to_string()]
        } else {
            body.slices.clone()
        };
        let mut generated = Vec::new();
        for (idx, slice) in slices.iter().enumerate() {
            let title = format!("Postdoc slice {}: {}", idx + 1, compact_task_title(slice));
            let instructions = format!(
                "{marker}\n\
                 Accepted PostdocPlan: {artifact_id}\n\
                 Implementation summary: {summary}\n\
                 Slice: {slice}\n\
                 Graduate handoff: {handoff}\n\
                 \n\
                 Return a GraduateResult with changed files, validation attempts, blockers, and handoff notes.",
                artifact_id = plan_artifact.artifact_id,
                summary = body.implementation_summary.trim(),
                slice = slice.trim(),
                handoff = body.graduate_handoff.trim(),
            );
            let task = self.store.create_graduate_task(
                &plan_artifact.lab_run_id,
                &title,
                &instructions,
                body.files_expected.clone(),
                body.validation_plan.clone(),
            )?;
            let task = if task.allowed_scope.is_empty() || task.required_validation.is_empty() {
                let reason = if task.allowed_scope.is_empty() && task.required_validation.is_empty()
                {
                    "Accepted PostdocPlan is missing files_expected and validation_plan; postdoc must revise scope before graduate execution."
                } else if task.allowed_scope.is_empty() {
                    "Accepted PostdocPlan is missing files_expected; postdoc must set allowed_scope before graduate execution."
                } else {
                    "Accepted PostdocPlan is missing validation_plan; postdoc must set required_validation before graduate execution."
                };
                self.store
                    .block_graduate_task(&plan_artifact.lab_run_id, &task.task_id, reason)?
            } else {
                task
            };
            generated.push(task);
        }
        self.store.record_run_event(
            &plan_artifact.lab_run_id,
            "graduate_tasks_queued_from_postdoc_plan",
            serde_json::json!({
                "artifact_id": plan_artifact.artifact_id,
                "task_ids": generated
                    .iter()
                    .map(|task| task.task_id.as_str())
                    .collect::<Vec<_>>(),
                "task_count": generated.len(),
            }),
        )?;
        Ok(generated)
    }

    pub fn create_current_stage_artifact_for_latest(
        &self,
        note: &str,
    ) -> anyhow::Result<CreatedStageArtifact> {
        let run = self
            .store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for stage artifact"))?;
        if !matches!(run.status, LabRunStatus::Active) {
            return Err(anyhow!(
                "LabRun {} is not active: {:?}",
                run.lab_run_id,
                run.status
            ));
        }
        let transition = transition_for_stage(&run.current_stage).ok_or_else(|| {
            anyhow!(
                "LabRun {} has no configured artifact for stage '{}'",
                run.lab_run_id,
                run.current_stage
            )
        })?;
        let artifact_type = artifact_type_for_stage(transition.from_stage)?;
        let mut artifact = build_stage_artifact(&run, artifact_type, note);
        let consumed_revision_artifact_id =
            self.apply_pending_revision_task_to_postdoc_plan(&run, &mut artifact)?;
        let path = self.store.write_stage_artifact(&artifact)?;
        let report_path = self.store.write_stage_artifact_report(&artifact)?;
        if let Some(revision_artifact_id) = consumed_revision_artifact_id.as_deref() {
            self.mark_revision_task_consumed_by_postdoc_plan(
                &run,
                revision_artifact_id,
                artifact.artifact_id(),
            )?;
        }
        let evidence_ref = path.display().to_string();
        let gate = self.write_satisfied_gate_for_latest(
            artifact.artifact_id(),
            artifact.validation_status(),
            Some(&evidence_ref),
        )?;
        Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        })
    }

    pub fn pending_revision_context_for_run(&self, run: &LabRun) -> anyhow::Result<Option<String>> {
        let Some(revision) = self.pending_revision_task_for_run(&run.lab_run_id)? else {
            return Ok(None);
        };
        Ok(Some(format!(
            "pending_revision_task_id: {}\nsource_review_artifact_id: {}\nassigned_role: {:?}\nsummary: {}\nrequired_revisions:\n{}\nnext_action: {}",
            revision.artifact_id,
            revision.body.source_review_artifact_id,
            revision.body.assigned_role,
            revision.body.summary,
            revision
                .body
                .required_revisions
                .iter()
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n"),
            revision.body.next_action
        )))
    }

    pub fn apply_pending_revision_task_to_postdoc_plan(
        &self,
        run: &LabRun,
        artifact: &mut StageArtifact,
    ) -> anyhow::Result<Option<String>> {
        let StageArtifact::PostdocPlan(plan) = artifact else {
            return Ok(None);
        };
        let Some(revision) = self.pending_revision_task_for_run(&run.lab_run_id)? else {
            return Ok(None);
        };
        if !plan
            .evidence_refs
            .iter()
            .any(|item| item == &format!("artifact:{}", revision.artifact_id))
        {
            plan.evidence_refs
                .push(format!("artifact:{}", revision.artifact_id));
        }
        if !revision
            .body
            .required_revisions
            .iter()
            .all(|item| plan.body.slices.iter().any(|slice| slice.contains(item)))
        {
            let revision_slice = format!(
                "Address professor revision task {}: {}",
                revision.body.revision_id,
                revision.body.required_revisions.join("; ")
            );
            if !plan
                .body
                .slices
                .iter()
                .any(|slice| slice == &revision_slice)
            {
                plan.body.slices.insert(0, revision_slice);
            }
        }
        if !plan
            .body
            .graduate_handoff
            .contains(&revision.body.source_review_artifact_id)
        {
            plan.body.graduate_handoff = format!(
                "{}\n\nProfessor revision source: {}. Required revisions: {}",
                plan.body.graduate_handoff.trim(),
                revision.body.source_review_artifact_id,
                revision.body.required_revisions.join("; ")
            )
            .trim()
            .to_string();
        }
        Ok(Some(revision.artifact_id))
    }

    pub fn mark_revision_task_consumed_by_postdoc_plan(
        &self,
        run: &LabRun,
        revision_artifact_id: &str,
        postdoc_plan_artifact_id: &str,
    ) -> anyhow::Result<()> {
        let artifact = self
            .store
            .load_stage_artifact(&run.lab_run_id, revision_artifact_id)?;
        let StageArtifact::LabRevisionTask(mut revision) = artifact else {
            return Err(anyhow!(
                "artifact {} is not a LabRevisionTask",
                revision_artifact_id
            ));
        };
        revision.status = LabArtifactStatus::Accepted;
        revision.validation_status = Some("consumed".to_string());
        revision.updated_at = Utc::now();
        let revision_artifact = StageArtifact::LabRevisionTask(revision.clone());
        self.store.write_stage_artifact(&revision_artifact)?;
        self.store.write_stage_artifact_report(&revision_artifact)?;
        self.store.record_run_event(
            &run.lab_run_id,
            "lab_revision_task_consumed_by_postdoc_plan",
            serde_json::json!({
                "revision_task_artifact_id": revision.artifact_id,
                "postdoc_plan_artifact_id": postdoc_plan_artifact_id,
            }),
        )?;
        Ok(())
    }

    pub(super) fn pending_revision_task_for_run(
        &self,
        lab_run_id: &str,
    ) -> anyhow::Result<Option<LabArtifactEnvelope<LabRevisionTask>>> {
        Ok(self
            .store
            .list_stage_artifacts(lab_run_id)?
            .into_iter()
            .filter_map(|artifact| match artifact {
                StageArtifact::LabRevisionTask(revision)
                    if revision.body.assigned_role == LabRole::Postdoc
                        && revision.validation_status.as_deref() != Some("consumed") =>
                {
                    Some(revision)
                }
                _ => None,
            })
            .next_back())
    }
}
