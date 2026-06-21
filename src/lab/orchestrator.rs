use crate::lab::context::{
    build_lab_context_packet_with_evidence_retries_and_artifact_refs,
    evaluate_lab_context_compression, LabContextEvidenceRefGroup,
};
use crate::lab::delegation::{
    build_graduate_task_dispatch, execute_graduate_task_with_agent_tool, graduate_agent_task_id,
};
use crate::lab::model::{
    ArtifactGate, GraduateDispatchRecord, GraduateDispatchStatus, GraduateResult, GraduateTask,
    LabArtifactEnvelope, LabArtifactStatus, LabArtifactType, LabBlockerReport, LabCloseoutStatus,
    LabCompressionAction, LabCompressionDecision, LabCompressionSummary, LabCycleSummary, LabEvent,
    LabMeetingRequest, LabMeetingSummary, LabRevisionTask, LabRole, LabRun, LabRunStatus,
    LabTaskStatus, PostdocIntegrationSummary, PostdocPlan, ProfessorPlan, ProfessorReview,
    ProfessorSteeringDecision, SponsorMessageStatus, SponsorMessageType, StageArtifact,
};
use crate::lab::store::LabStore;
use crate::tools::ToolContext;
use anyhow::anyhow;
use chrono::Utc;
use serde_json::Value;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageTransition {
    pub from_stage: &'static str,
    pub required_artifact_type: &'static str,
    pub required_owner: LabRole,
    pub to_stage: &'static str,
    pub next_owner: LabRole,
    pub next_action: &'static str,
}

const STAGE_TRANSITIONS: &[StageTransition] = &[
    StageTransition {
        from_stage: "professor_discussion",
        required_artifact_type: "ProfessorPlan",
        required_owner: LabRole::Professor,
        to_stage: "postdoc_plan",
        next_owner: LabRole::Postdoc,
        next_action: "postdoc_plan",
    },
    StageTransition {
        from_stage: "postdoc_plan",
        required_artifact_type: "PostdocPlan",
        required_owner: LabRole::Postdoc,
        to_stage: "graduate_work",
        next_owner: LabRole::Graduate,
        next_action: "graduate_work",
    },
    StageTransition {
        from_stage: "graduate_work",
        required_artifact_type: "GraduateResult",
        required_owner: LabRole::Graduate,
        to_stage: "postdoc_review",
        next_owner: LabRole::Postdoc,
        next_action: "postdoc_review",
    },
    StageTransition {
        from_stage: "postdoc_review",
        required_artifact_type: "PostdocIntegrationSummary",
        required_owner: LabRole::Postdoc,
        to_stage: "professor_review",
        next_owner: LabRole::Professor,
        next_action: "professor_review",
    },
    StageTransition {
        from_stage: "professor_review",
        required_artifact_type: "ProfessorReview",
        required_owner: LabRole::Professor,
        to_stage: "user_report",
        next_owner: LabRole::Professor,
        next_action: "user_report",
    },
];

#[derive(Debug, Clone)]
pub struct LabOrchestrator {
    store: LabStore,
}

#[derive(Debug, Clone)]
pub struct CreatedStageArtifact {
    pub artifact: StageArtifact,
    pub path: PathBuf,
    pub report_path: PathBuf,
    pub gate: ArtifactGate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabMeetingRecommendation {
    pub lab_run_id: String,
    pub recommended: bool,
    pub topic: String,
    pub reason: String,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabTickStatus {
    Advanced,
    Blocked,
    NeedsUser,
}

#[derive(Debug, Clone)]
pub struct LabTickResult {
    pub lab_run_id: String,
    pub status: LabTickStatus,
    pub from_stage: String,
    pub to_stage: String,
    pub owner: LabRole,
    pub artifact_id: Option<String>,
    pub report_path: Option<PathBuf>,
    pub compression_artifact_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabSchedulerStepAction {
    TickAdvanced,
    GraduateDispatched,
    NeedsUser,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct LabSchedulerStepResult {
    pub lab_run_id: String,
    pub action: LabSchedulerStepAction,
    pub stage: String,
    pub task_id: Option<String>,
    pub dispatch_id: Option<String>,
    pub message: String,
}

impl LabOrchestrator {
    pub fn for_project(project_root: impl AsRef<Path>) -> Self {
        Self {
            store: LabStore::for_project(project_root),
        }
    }

    pub fn store(&self) -> &LabStore {
        &self.store
    }

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

    fn pending_revision_task_for_run(
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
            .last())
    }

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
            .last()
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
            .last()
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
        let runtime_evidence = match runtime_verify_graduate_task_result(
            &task,
            &context,
            Some(&state.agent_id),
            &agent_task_id,
            &[],
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
        parsed
            .evidence_ids
            .push(format!("agent_task:{}", agent_task_id));
        parsed
            .evidence_ids
            .push(format!("agent_artifact:{}", artifact_id));
        parsed.evidence_ids.sort();
        parsed.evidence_ids.dedup();

        let created = self.create_graduate_result_for_task_latest(
            &task.task_id,
            &parsed.task_summary,
            parsed.changed_files,
            parsed.validation_attempts,
            parsed.blockers,
            parsed.evidence_ids,
        )?;
        if let Some(dispatch) =
            self.latest_dispatch_for_agent_task(&run.lab_run_id, &task.task_id, &agent_task_id)?
        {
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

        let graduate_results = self
            .store
            .list_stage_artifacts(&run.lab_run_id)?
            .into_iter()
            .filter_map(|artifact| match artifact {
                StageArtifact::GraduateResult(result) => Some(result),
                _ => None,
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
        for result in &graduate_results {
            evidence_refs.push(format!("artifact:{}", result.artifact_id));
            evidence_refs.extend(result.evidence_refs.iter().cloned());
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
                || risk.contains("No graduate result")
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
            .last()
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

    fn create_runtime_verified_graduate_result_for_unbound_success(
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

    fn mark_unbound_graduate_success_failed(
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
            return Ok(LabSchedulerStepResult {
                lab_run_id: run.lab_run_id,
                action: LabSchedulerStepAction::Blocked,
                stage: from_stage.clone(),
                task_id: None,
                dispatch_id: None,
                message: format!(
                    "Scheduler stopped at {from_stage}; provider-backed or explicit role review artifact is required before advancement."
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
            message: format!("Scheduler advanced LabRun from {}.", tick.from_stage),
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

    fn ensure_gate_for_current_stage(&self, run: &LabRun) -> anyhow::Result<()> {
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

fn transition_for_stage(stage: &str) -> Option<StageTransition> {
    STAGE_TRANSITIONS
        .iter()
        .copied()
        .find(|transition| transition.from_stage == stage)
}

fn artifact_type_for_stage(stage: &str) -> anyhow::Result<LabArtifactType> {
    match stage {
        "professor_discussion" => Ok(LabArtifactType::ProfessorPlan),
        "postdoc_plan" => Ok(LabArtifactType::PostdocPlan),
        "graduate_work" => Ok(LabArtifactType::GraduateResult),
        "postdoc_review" => Ok(LabArtifactType::PostdocIntegrationSummary),
        "professor_review" => Ok(LabArtifactType::ProfessorReview),
        _ => Err(anyhow!("unknown LabRun artifact stage: {stage}")),
    }
}

#[derive(Debug, Default)]
struct PostdocWorktreeProof {
    accepted_results: Vec<String>,
    remaining_risks: Vec<String>,
    evidence_refs: Vec<String>,
}

fn collect_graduate_worktree_proof_for_postdoc(
    store: &LabStore,
    lab_run_id: &str,
    limit: usize,
) -> anyhow::Result<PostdocWorktreeProof> {
    let events = store.list_run_events(lab_run_id)?;
    let mut proof = PostdocWorktreeProof::default();
    let mut recent_events = events
        .iter()
        .rev()
        .filter(|event| event.event_type == "lab_graduate_worktree_action")
        .take(limit)
        .collect::<Vec<_>>();
    recent_events.reverse();

    for event in recent_events {
        proof
            .evidence_refs
            .push(format!("event:{}", event.event_id));
        let summary = format_graduate_worktree_proof_for_postdoc(event);
        if event
            .payload
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            proof
                .accepted_results
                .push(format!("runtime worktree proof: {summary}"));
        } else {
            proof
                .remaining_risks
                .push(format!("runtime worktree proof failed: {summary}"));
        }
    }

    Ok(proof)
}

fn format_graduate_worktree_proof_for_postdoc(event: &LabEvent) -> String {
    let payload = &event.payload;
    let result_data = payload.get("result_data").unwrap_or(&Value::Null);
    let action = payload
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let agent_ref_kind = payload
        .get("agent_ref_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let agent_ref = payload
        .get("agent_ref")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let merge_kind = result_data
        .get("merge_kind")
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    let dirty = result_data
        .get("dirty")
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let path = result_data
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    format!(
        "{} task={} ref={}:{} merge_kind={} dirty={} path={}",
        action, task_id, agent_ref_kind, agent_ref, merge_kind, dirty, path
    )
}

fn collect_graduate_workspace_snapshot_proof_for_postdoc(
    store: &LabStore,
    lab_run_id: &str,
    limit: usize,
) -> anyhow::Result<PostdocWorktreeProof> {
    let events = store.list_run_events(lab_run_id)?;
    let mut proof = PostdocWorktreeProof::default();
    let mut recent_events = events
        .iter()
        .rev()
        .filter(|event| event.event_type == "lab_graduate_workspace_snapshot")
        .take(limit)
        .collect::<Vec<_>>();
    recent_events.reverse();

    for event in recent_events {
        proof
            .evidence_refs
            .push(format!("event:{}", event.event_id));
        let summary = format_graduate_workspace_snapshot_for_postdoc(event);
        let phase = event
            .payload
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let dirty_count = event
            .payload
            .get("dirty_path_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let changed_count = event
            .payload
            .get("changed_path_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if phase == "before" && dirty_count > 0 {
            proof
                .remaining_risks
                .push(format!("pre-existing workspace changes: {summary}"));
        } else if phase == "after" && changed_count > 0 {
            proof
                .accepted_results
                .push(format!("runtime workspace delta: {summary}"));
        }
    }

    Ok(proof)
}

fn format_graduate_workspace_snapshot_for_postdoc(event: &LabEvent) -> String {
    let payload = &event.payload;
    let phase = payload
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let task_id = payload
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let dispatch_id = payload
        .get("dispatch_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let dirty_paths = value_string_list(payload.get("dirty_paths"));
    let changed_paths = value_string_list(payload.get("changed_paths"));
    format!(
        "{} task={} dispatch={} dirty=[{}] changed=[{}]",
        phase,
        task_id,
        dispatch_id,
        summarize_paths_for_runtime_proof(&dirty_paths),
        summarize_paths_for_runtime_proof(&changed_paths)
    )
}

fn value_string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn summarize_paths_for_runtime_proof(paths: &[String]) -> String {
    if paths.is_empty() {
        return "none".to_string();
    }
    let mut shown = paths.iter().take(5).cloned().collect::<Vec<_>>();
    if paths.len() > shown.len() {
        shown.push(format!("+{} more", paths.len() - shown.len()));
    }
    shown.join(",")
}

fn build_stage_artifact(run: &LabRun, artifact_type: LabArtifactType, note: &str) -> StageArtifact {
    let now = Utc::now();
    let note = note.trim();
    let title = if note.is_empty() {
        format!("{} for {}", artifact_type.as_str(), run.lab_run_id)
    } else {
        note.lines().next().unwrap_or(note).trim().to_string()
    };
    let artifact_id = format!(
        "artifact_{}_{}",
        artifact_type.as_str().to_ascii_lowercase(),
        Uuid::new_v4().simple()
    );
    match artifact_type {
        LabArtifactType::ProfessorPlan => StageArtifact::ProfessorPlan(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            ProfessorPlan {
                problem_statement: run.user_goal.clone(),
                strategic_direction: note_or_placeholder(note, "Initial professor direction."),
                success_criteria: vec![
                    "User-visible result is reviewed before closeout.".to_string()
                ],
                constraints: vec![
                    "Do not bypass runtime permission, checkpoint, or validation gates."
                        .to_string(),
                ],
                risks: vec![
                    "Plan content is a runtime draft until reviewed by the professor model."
                        .to_string(),
                ],
                handoff_to_postdoc:
                    "Create an implementation plan with slices, expected files, and validation."
                        .to_string(),
            },
        )),
        LabArtifactType::PostdocPlan => StageArtifact::PostdocPlan(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            PostdocPlan {
                implementation_summary: note_or_placeholder(
                    note,
                    "Postdoc implementation plan draft.",
                ),
                slices: vec!["Implement the smallest verifiable next slice.".to_string()],
                files_expected: Vec::new(),
                validation_plan: vec!["Run the narrowest relevant validation gate.".to_string()],
                graduate_handoff:
                    "Execute the current slice and report changed files, proof, and blockers."
                        .to_string(),
            },
        )),
        LabArtifactType::GraduateResult => StageArtifact::GraduateResult(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            GraduateResult {
                task_summary: note_or_placeholder(note, "Graduate task result draft."),
                changed_files: Vec::new(),
                validation_attempts: Vec::new(),
                blockers: Vec::new(),
                handoff_to_postdoc: "Review implementation quality and integration readiness."
                    .to_string(),
            },
        )),
        LabArtifactType::PostdocIntegrationSummary => {
            StageArtifact::PostdocIntegrationSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                PostdocIntegrationSummary {
                    integration_summary: note_or_placeholder(
                        note,
                        "Postdoc integration summary draft.",
                    ),
                    accepted_results: Vec::new(),
                    validation_status: "not_verified".to_string(),
                    remaining_risks: Vec::new(),
                    handoff_to_professor:
                        "Review strategic fit, completeness, and user-facing closeout.".to_string(),
                },
            ))
        }
        LabArtifactType::ProfessorReview => {
            StageArtifact::ProfessorReview(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                ProfessorReview {
                    review_summary: note_or_placeholder(note, "Professor review draft."),
                    strategic_assessment: "Strategic assessment requires professor model review."
                        .to_string(),
                    accepted: false,
                    required_revisions: Vec::new(),
                    user_report: "Prepare a concise user-facing report before closeout."
                        .to_string(),
                },
            ))
        }
        LabArtifactType::CycleSummary => StageArtifact::CycleSummary(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            LabCycleSummary {
                cycle_id: run.cycle_count.to_string(),
                current_stage: run.current_stage.clone(),
                owner: run.internal_owner,
                summary: note_or_placeholder(note, "Cycle summary draft."),
                completed_items: Vec::new(),
                evidence_ids: Vec::new(),
                total_tokens: 0,
                cache_hit_rate_percent: 0.0,
                estimated_cost_usd: 0.0,
                next_action: "Continue LabRun orchestration from the current stage.".to_string(),
            },
        )),
        LabArtifactType::CompressionSummary => {
            StageArtifact::CompressionSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                LabCompressionSummary {
                    decision_id: String::new(),
                    role: run.internal_owner,
                    action: LabCompressionAction::None,
                    reason: "Compression summary placeholder.".to_string(),
                    before_tokens: 0,
                    target_budget_tokens: 0,
                    usage_ratio_percent: 0.0,
                    stable_prefix_fingerprint: String::new(),
                    dynamic_tail_fingerprint: String::new(),
                    retained_layers: Vec::new(),
                    evidence_ids: Vec::new(),
                    compressed_summary: note_or_placeholder(note, "Compression summary draft."),
                    next_action: "Continue LabRun orchestration from the current stage."
                        .to_string(),
                },
            ))
        }
        LabArtifactType::LabMeetingRequest => {
            StageArtifact::LabMeetingRequest(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!(
                    "Runtime escalation signal meeting request for {}",
                    run.current_stage
                ),
                now,
                LabMeetingRequest {
                    request_id: "meeting_request_placeholder".to_string(),
                    topic: note_or_placeholder(note, "General LabRun meeting request."),
                    current_stage: run.current_stage.clone(),
                    reason: "runtime_placeholder".to_string(),
                    signals: Vec::new(),
                    requested_by: LabRole::Runtime,
                    next_action: "open_read_only_lab_meeting".to_string(),
                },
            ))
        }
        LabArtifactType::LabMeetingSummary => {
            StageArtifact::LabMeetingSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Lab meeting summary for {}", run.lab_run_id),
                now,
                LabMeetingSummary {
                    meeting_id: "meeting_placeholder".to_string(),
                    topic: note_or_placeholder(note, "General LabRun meeting."),
                    current_stage: run.current_stage.clone(),
                    professor_view: "Runtime placeholder professor view.".to_string(),
                    postdoc_view: "Runtime placeholder postdoc view.".to_string(),
                    decision: "continue_current_plan".to_string(),
                    next_actions: vec!["continue_labrun".to_string()],
                    evidence_ids: Vec::new(),
                    total_tokens: 0,
                    cache_hit_rate_percent: 0.0,
                },
            ))
        }
        LabArtifactType::LabBlockerReport => {
            StageArtifact::LabBlockerReport(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Postdoc blocker report for {}", run.current_stage),
                now,
                LabBlockerReport {
                    blocker_id: "blocker_placeholder".to_string(),
                    current_stage: run.current_stage.clone(),
                    summary: note_or_placeholder(note, "No blocker summary provided."),
                    blocked_tasks: Vec::new(),
                    failed_dispatches: Vec::new(),
                    failure_count: run.failure_count,
                    recommendation: "continue_current_plan".to_string(),
                    handoff_to_professor: "Review blocker state.".to_string(),
                },
            ))
        }
        LabArtifactType::LabRevisionTask => {
            StageArtifact::LabRevisionTask(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Postdoc revision task for {}", run.lab_run_id),
                now,
                LabRevisionTask {
                    revision_id: "revision_placeholder".to_string(),
                    source_review_artifact_id: String::new(),
                    assigned_role: LabRole::Postdoc,
                    summary: note_or_placeholder(note, "Professor requested postdoc revision."),
                    required_revisions: Vec::new(),
                    evidence_ids: Vec::new(),
                    next_action:
                        "Revise postdoc integration before professor review can close out."
                            .to_string(),
                },
            ))
        }
        LabArtifactType::ProfessorSteeringDecision => {
            StageArtifact::ProfessorSteeringDecision(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Professor steering decision for {}", run.lab_run_id),
                now,
                ProfessorSteeringDecision {
                    decision_id: "professor_steering_placeholder".to_string(),
                    source_message_id: String::new(),
                    decision: "pending_professor_review".to_string(),
                    status: SponsorMessageStatus::Queued,
                    message_type: SponsorMessageType::Concern,
                    urgency: "normal".to_string(),
                    rationale: note_or_placeholder(note, "No steering rationale provided."),
                    next_action: "Review sponsor message before applying any LabRun change."
                        .to_string(),
                    message_summary: String::new(),
                },
            ))
        }
    }
}

fn note_or_placeholder(note: &str, placeholder: &str) -> String {
    if note.trim().is_empty() {
        placeholder.to_string()
    } else {
        note.trim().to_string()
    }
}

fn postdoc_plan_task_marker(artifact_id: &str) -> String {
    format!("postdoc_plan_artifact_id={}", artifact_id.trim())
}

fn compact_task_title(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 72 {
        return compact;
    }
    let mut out = compact.chars().take(69).collect::<String>();
    out.push_str("...");
    out
}

fn compact_result_preview(value: &str, limit: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= limit {
        return compact;
    }
    let keep = limit.saturating_sub(3);
    let mut out = compact.chars().take(keep).collect::<String>();
    out.push_str("...");
    out
}

fn clean_string_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn validate_changed_files_within_scope(
    allowed_scope: &[String],
    changed_files: &[String],
) -> anyhow::Result<()> {
    if changed_files.is_empty() {
        return Ok(());
    }
    if allowed_scope.is_empty() {
        return Err(anyhow!(
            "graduate result cannot report changed files without allowed_scope"
        ));
    }
    let outside = changed_files
        .iter()
        .find(|file| !path_matches_any_scope(file, allowed_scope));
    if let Some(file) = outside {
        return Err(anyhow!(
            "graduate result changed file '{}' is outside allowed_scope ({})",
            file,
            allowed_scope.join(", ")
        ));
    }
    Ok(())
}

fn path_matches_any_scope(file: &str, allowed_scope: &[String]) -> bool {
    let file = file.trim().trim_start_matches("./");
    allowed_scope.iter().any(|scope| {
        let scope = scope.trim().trim_start_matches("./");
        if scope.is_empty() {
            return false;
        }
        file == scope || file.starts_with(&format!("{}/", scope.trim_end_matches('/')))
    })
}

fn durable_graduate_task_is_completed(context: &ToolContext, task: &GraduateTask) -> bool {
    let Some(store) = context.session_store.as_ref() else {
        return false;
    };
    let agent_task_id = graduate_agent_task_id(task);
    matches!(
        store.agent_task_state(&context.session_id, &agent_task_id),
        Ok(Some(state))
            if state.profile.as_deref() == Some("lab-graduate")
                && state.status == "completed"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraduateRuntimeEvidence {
    changed_files: Vec<String>,
    validation_attempts: Vec<String>,
}

fn runtime_verify_graduate_task_result(
    task: &GraduateTask,
    context: &ToolContext,
    agent_id: Option<&str>,
    agent_task_id: &str,
    parent_changed_files: &[String],
) -> anyhow::Result<GraduateRuntimeEvidence> {
    let verification_root = agent_id
        .and_then(|agent_id| agent_worktree_path(context, agent_id))
        .or_else(|| agent_worktree_path(context, agent_task_id))
        .unwrap_or_else(|| context.working_dir.clone());
    if !verification_root.exists() {
        return Err(anyhow!(
            "graduate runtime verification worktree does not exist: {}",
            verification_root.display()
        ));
    }

    let changed_files = if same_filesystem_path(&verification_root, &context.working_dir) {
        clean_string_vec(parent_changed_files.to_vec())
    } else {
        current_git_changed_paths(&verification_root, Some(&context.working_dir))
    };
    if changed_files.is_empty() {
        return Err(anyhow!(
            "graduate runtime verification found no actual file changes in {}",
            verification_root.display()
        ));
    }
    validate_changed_files_within_scope(&task.allowed_scope, &changed_files)?;
    let validation_attempts =
        run_required_validation_commands(&verification_root, &task.required_validation)?;

    Ok(GraduateRuntimeEvidence {
        changed_files,
        validation_attempts,
    })
}

fn agent_worktree_path(context: &ToolContext, agent_id: &str) -> Option<PathBuf> {
    let store = context.session_store.as_ref()?;
    let state = store
        .agent_task_state(&context.session_id, agent_id)
        .ok()
        .flatten()?;
    state
        .payload
        .get("isolated_worktree")?
        .get("path")?
        .as_str()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
}

fn same_filesystem_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn current_git_changed_paths(worktree_root: &Path, target_root: Option<&Path>) -> Vec<String> {
    let mut paths = workspace_change_snapshot(worktree_root)
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    if let Some(target_root) = target_root {
        if let Some(base) = git_stdout(target_root, &["rev-parse", "HEAD"]) {
            if let Some(committed) = git_stdout(
                worktree_root,
                &["diff", "--name-only", &format!("{}...HEAD", base)],
            ) {
                paths.extend(
                    committed
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty() && !is_internal_lab_runtime_path(line))
                        .map(str::to_string),
                );
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn git_stdout(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn run_required_validation_commands(
    cwd: &Path,
    commands: &[String],
) -> anyhow::Result<Vec<String>> {
    let mut attempts = Vec::new();
    for command in commands {
        let command = command.trim();
        if command.is_empty() {
            continue;
        }
        let output = Command::new("sh")
            .arg("-lc")
            .arg(command)
            .current_dir(cwd)
            .output()
            .map_err(|err| anyhow!("failed to run required validation `{command}`: {err}"))?;
        if output.status.success() {
            attempts.push(format!("runtime validation `{command}` passed"));
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "required validation `{}` failed with status {:?}; stdout={}; stderr={}",
                command,
                output.status.code(),
                compact_result_preview(&stdout, 240),
                compact_result_preview(&stderr, 240)
            ));
        }
    }
    Ok(attempts)
}

fn workspace_change_snapshot(project_root: &Path) -> BTreeMap<String, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(project_root)
        .output();
    let Ok(output) = output else {
        return BTreeMap::new();
    };
    if !output.status.success() {
        return BTreeMap::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_git_status_path)
        .filter(|path| !is_internal_lab_runtime_path(path))
        .map(|path| {
            let fingerprint = workspace_path_fingerprint(project_root, &path);
            (path, fingerprint)
        })
        .collect()
}

fn parse_git_status_path(line: &str) -> Option<String> {
    if line.len() < 4 {
        return None;
    }
    let path = line[3..].trim();
    if path.is_empty() {
        return None;
    }
    Some(
        path.rsplit_once(" -> ")
            .map(|(_, renamed)| renamed)
            .unwrap_or(path)
            .trim_matches('"')
            .trim_start_matches("./")
            .to_string(),
    )
}

fn closeout_status_from_gate(gate: &ArtifactGate) -> LabCloseoutStatus {
    match gate
        .validation_status
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "verified" | "validated" | "passed" | "success" => LabCloseoutStatus::CompletedVerified,
        "partial" | "partially_verified" | "partially_completed" => LabCloseoutStatus::Partial,
        "blocked" | "blocked_needs_user" | "needs_user" => LabCloseoutStatus::BlockedNeedsUser,
        "failed" | "failure" => LabCloseoutStatus::Failed,
        _ => LabCloseoutStatus::CompletedNotVerified,
    }
}

fn workspace_path_fingerprint(project_root: &Path, path: &str) -> String {
    let full_path = project_root.join(path);
    let Ok(metadata) = std::fs::metadata(&full_path) else {
        return "missing".to_string();
    };
    if !metadata.is_file() {
        return format!("non_file:{}", metadata.len());
    }
    let Ok(bytes) = std::fs::read(&full_path) else {
        return format!("unreadable:{}", metadata.len());
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("file:{}:{:x}", metadata.len(), hasher.finish())
}

fn changed_paths_between(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<String> {
    after
        .iter()
        .filter_map(|(path, fingerprint)| {
            (!is_internal_lab_runtime_path(path) && before.get(path) != Some(fingerprint))
                .then_some(path.clone())
        })
        .collect()
}

fn is_internal_lab_runtime_path(path: &str) -> bool {
    let path = path.trim().trim_start_matches("./");
    path.starts_with(".priority-agent/")
        || path == ".priority-agent"
        || path.starts_with(".git/")
        || path == ".git"
        || path.starts_with(".claude/worktrees/")
        || path == ".claude/worktrees"
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedGraduateAgentResult {
    task_summary: String,
    changed_files: Vec<String>,
    validation_attempts: Vec<String>,
    blockers: Vec<String>,
    evidence_ids: Vec<String>,
}

fn parse_graduate_agent_result(
    data: Option<&Value>,
    content: &str,
) -> Option<ParsedGraduateAgentResult> {
    if let Some(data) = data {
        if let Some(parsed) = parse_graduate_agent_result_value(data) {
            return Some(parsed);
        }
        if let Some(result) = data.get("result").and_then(Value::as_str) {
            if let Some(value) = parse_json_value_from_text(result) {
                if let Some(parsed) = parse_graduate_agent_result_value(&value) {
                    return Some(parsed);
                }
            }
        }
    }
    parse_json_value_from_text(content).and_then(|value| parse_graduate_agent_result_value(&value))
}

fn parse_json_value_from_text(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Some(value);
    }
    if let Some(fenced) = trimmed.strip_prefix("```") {
        let body = fenced.lines().skip(1).collect::<Vec<_>>().join("\n");
        let body = body
            .trim()
            .strip_suffix("```")
            .unwrap_or(body.trim())
            .trim();
        if let Ok(value) = serde_json::from_str::<Value>(body) {
            return Some(value);
        }
    }
    let start = trimmed.find('{')?;
    for end in trimmed.rmatch_indices('}').map(|(idx, _)| idx + 1) {
        if end <= start {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&trimmed[start..end]) {
            return Some(value);
        }
    }
    None
}

fn parse_graduate_agent_result_value(value: &Value) -> Option<ParsedGraduateAgentResult> {
    let value = value
        .get("graduate_result")
        .or_else(|| value.get("result_json"))
        .unwrap_or(value);
    let task_summary = string_field(value, &["task_summary", "summary", "handoff_summary"])?;
    let validation_attempts = string_array_field(
        value,
        &["validation_attempts", "validation_results", "validation"],
    );
    if validation_attempts.is_empty() {
        return None;
    }
    Some(ParsedGraduateAgentResult {
        task_summary,
        changed_files: string_array_field(value, &["changed_files", "files_changed"]),
        validation_attempts,
        blockers: string_array_field(value, &["blockers", "risks"]),
        evidence_ids: string_array_field(value, &["evidence_ids", "evidence_refs"]),
    })
}

fn string_field(value: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_array_field(value: &Value, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .find_map(|name| value.get(*name))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::model::LAB_SCHEMA_VERSION;
    use std::sync::Arc;

    fn init_git_dir(path: &Path) {
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .expect("git init");
        std::process::Command::new("git")
            .args(["config", "user.email", "lab@example.test"])
            .current_dir(path)
            .output()
            .expect("git config email");
        std::process::Command::new("git")
            .args(["config", "user.name", "Lab Test"])
            .current_dir(path)
            .output()
            .expect("git config name");
        std::fs::write(path.join("README.md"), "base\n").expect("write base");
        std::process::Command::new("git")
            .args(["add", "README.md"])
            .current_dir(path)
            .output()
            .expect("git add");
        std::process::Command::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(path)
            .output()
            .expect("git commit");
    }

    fn lab_context_with_agent_worktree(
        project_root: &Path,
        session_id: &str,
        agent_id: &str,
        worktree_path: &Path,
    ) -> ToolContext {
        lab_context_with_agent_worktree_task_id(
            project_root,
            session_id,
            agent_id,
            agent_id,
            worktree_path,
        )
    }

    fn lab_context_with_agent_worktree_task_id(
        project_root: &Path,
        session_id: &str,
        task_id: &str,
        agent_id: &str,
        worktree_path: &Path,
    ) -> ToolContext {
        let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        store
            .create_session(session_id, "lab runtime test", "test-model", None)
            .unwrap();
        store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: session_id.to_string(),
                task_id: task_id.to_string(),
                agent_id: agent_id.to_string(),
                profile: Some("lab-graduate".to_string()),
                role: "implementation".to_string(),
                status: "completed".to_string(),
                description: "graduate runtime test".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: None,
                cleanup_hooks: vec!["worktree_cleanup".to_string()],
                payload: serde_json::json!({
                    "isolated_worktree": {
                        "path": worktree_path.to_string_lossy().to_string(),
                        "branch": "codex/agent-test"
                    }
                }),
            })
            .unwrap();
        ToolContext::new(project_root, session_id).with_session_store(store)
    }

    #[test]
    fn approve_creates_initial_professor_plan_gate() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();

        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        let gate_path = orchestrator
            .store()
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("artifact_gates")
            .join("professor_discussion.json");
        assert!(gate_path.exists());
        let err = orchestrator.advance_latest().unwrap_err().to_string();
        assert!(err.contains("artifact_id"));
    }

    #[test]
    fn satisfied_gate_allows_stage_advance_and_creates_next_gate() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        orchestrator
            .write_satisfied_gate_for_latest(
                "artifact_professor_plan_001",
                Some("not_verified"),
                None,
            )
            .unwrap();
        let advanced = orchestrator.advance_latest().unwrap();

        assert_eq!(advanced.lab_run_id, run.lab_run_id);
        assert_eq!(advanced.current_stage, "postdoc_plan");
        assert_eq!(advanced.internal_owner, LabRole::Postdoc);
        assert!(orchestrator
            .store()
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("artifact_gates")
            .join("postdoc_plan.json")
            .exists());
    }

    #[test]
    fn current_stage_artifact_is_persisted_and_satisfies_gate() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        let created = orchestrator
            .create_current_stage_artifact_for_latest("Professor direction")
            .unwrap();
        assert_eq!(
            created.artifact.artifact_type(),
            LabArtifactType::ProfessorPlan
        );
        assert!(created.path.exists());
        assert_eq!(created.gate.stage, "professor_discussion");
        assert!(created.report_path.exists());

        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert!(saved
            .artifact_ids
            .iter()
            .any(|id| id == created.artifact.artifact_id()));
        assert_eq!(
            saved.resume_cursor.active_artifact_id.as_deref(),
            Some(created.artifact.artifact_id())
        );

        let advanced = orchestrator.advance_latest().unwrap();
        assert_eq!(advanced.current_stage, "postdoc_plan");
    }

    #[test]
    fn accepting_postdoc_plan_queues_graduate_tasks_once() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let professor = orchestrator
            .create_current_stage_artifact_for_latest("Professor direction")
            .unwrap();
        orchestrator
            .accept_artifact_latest(professor.artifact.artifact_id(), "accepted")
            .unwrap();
        let run = orchestrator.advance_latest().unwrap();
        assert_eq!(run.current_stage, "postdoc_plan");

        let postdoc = StageArtifact::PostdocPlan(LabArtifactEnvelope::new(
            "artifact_postdoc_plan_queue_test".to_string(),
            run.lab_run_id.clone(),
            LabArtifactType::PostdocPlan,
            "Postdoc implementation plan".to_string(),
            Utc::now(),
            PostdocPlan {
                implementation_summary: "Implement two concrete slices.".to_string(),
                slices: vec![
                    "Add runtime queue bridge".to_string(),
                    "Verify scheduler handoff".to_string(),
                ],
                files_expected: vec!["src/lab/orchestrator.rs".to_string()],
                validation_plan: vec!["cargo check -q --tests".to_string()],
                graduate_handoff: "Implement only the scoped files and report proof.".to_string(),
            },
        ));
        orchestrator.store().write_stage_artifact(&postdoc).unwrap();
        orchestrator
            .store()
            .write_stage_artifact_report(&postdoc)
            .unwrap();

        orchestrator
            .accept_artifact_latest(postdoc.artifact_id(), "accepted")
            .unwrap();
        let tasks = orchestrator
            .store()
            .list_graduate_tasks(&run.lab_run_id)
            .unwrap();
        assert_eq!(tasks.len(), 2);
        assert!(tasks
            .iter()
            .all(|task| matches!(task.status, LabTaskStatus::Queued)));
        assert!(tasks.iter().all(|task| task
            .instructions
            .contains("postdoc_plan_artifact_id=artifact_postdoc_plan_queue_test")));
        assert_eq!(
            tasks[0].allowed_scope,
            vec!["src/lab/orchestrator.rs".to_string()]
        );

        orchestrator
            .accept_artifact_latest(postdoc.artifact_id(), "accepted again")
            .unwrap();
        let deduped = orchestrator
            .store()
            .list_graduate_tasks(&run.lab_run_id)
            .unwrap();
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn accepting_postdoc_plan_without_scope_blocks_generated_task() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let professor = orchestrator
            .create_current_stage_artifact_for_latest("Professor direction")
            .unwrap();
        orchestrator
            .accept_artifact_latest(professor.artifact.artifact_id(), "accepted")
            .unwrap();
        let run = orchestrator.advance_latest().unwrap();

        let postdoc = orchestrator
            .create_current_stage_artifact_for_latest("Postdoc plan missing scope")
            .unwrap();
        orchestrator
            .accept_artifact_latest(postdoc.artifact.artifact_id(), "accepted")
            .unwrap();

        let tasks = orchestrator
            .store()
            .list_graduate_tasks(&run.lab_run_id)
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, LabTaskStatus::Blocked);
        assert!(tasks[0]
            .blocker
            .as_deref()
            .unwrap_or("")
            .contains("missing files_expected"));
    }

    #[test]
    fn cycle_summary_persists_report_and_advances_cycle_count() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        let created = orchestrator
            .create_cycle_summary_for_latest("Finished initial planning slice")
            .unwrap();

        assert_eq!(
            created.artifact.artifact_type(),
            LabArtifactType::CycleSummary
        );
        assert!(created.path.exists());
        assert!(created.report_path.exists());
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.cycle_count, 1);
        assert!(saved
            .artifact_ids
            .iter()
            .any(|id| id == created.artifact.artifact_id()));
    }

    #[test]
    fn compression_summary_is_written_when_decision_requires_it() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.cost_policy.professor_context_budget = 10;
        orchestrator.store().save_run(&run).unwrap();

        let created = orchestrator
            .create_compression_summary_for_latest(LabRole::Professor)
            .unwrap()
            .expect("small budget should require compression");

        assert_eq!(
            created.artifact.artifact_type(),
            LabArtifactType::CompressionSummary
        );
        assert!(created.path.exists());
        assert!(created.report_path.exists());
        let decisions = orchestrator
            .store()
            .list_compression_decisions(&run.lab_run_id)
            .unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].action, LabCompressionAction::Required);
    }

    #[test]
    fn meeting_summary_writes_read_only_artifact_and_tracks_meeting_id() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let evidence = orchestrator
            .store()
            .record_evidence_ref(
                &run.lab_run_id,
                crate::lab::model::LabEvidenceKind::File,
                LabRole::Postdoc,
                "target/meeting-proof.txt",
                "meeting proof",
                None,
                Some("0"),
            )
            .unwrap();

        let created = orchestrator
            .create_meeting_summary_for_latest(Some("review blocked work"))
            .unwrap();

        assert_eq!(
            created.artifact.artifact_type(),
            LabArtifactType::LabMeetingSummary
        );
        assert!(created.path.exists());
        assert!(created.report_path.exists());
        assert!(created
            .gate
            .evidence_refs
            .iter()
            .any(|item| item == &evidence.evidence_id));
        match &created.artifact {
            StageArtifact::LabMeetingSummary(envelope) => {
                assert!(envelope
                    .evidence_refs
                    .iter()
                    .any(|item| item == &evidence.evidence_id));
            }
            other => panic!(
                "expected LabMeetingSummary, got {:?}",
                other.artifact_type()
            ),
        }
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.meeting_ids.len(), 1);
        assert!(saved
            .artifact_ids
            .iter()
            .any(|id| id == created.artifact.artifact_id()));
    }

    #[test]
    fn meeting_recommendation_uses_blocked_task_signal() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .store()
            .block_graduate_task(&run.lab_run_id, &task.task_id, "validation failed twice")
            .unwrap();

        let recommendation = orchestrator.meeting_recommendation_for_latest().unwrap();

        assert!(recommendation.recommended);
        assert!(recommendation.topic.contains("blocked graduate task"));
        assert!(recommendation
            .signals
            .iter()
            .any(|signal| signal.starts_with("blocked_task:")));
    }

    #[test]
    fn meeting_request_persists_runtime_escalation_signal_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .store()
            .block_graduate_task(&run.lab_run_id, &task.task_id, "validation failed twice")
            .unwrap();

        let recommendation = orchestrator.meeting_recommendation_for_latest().unwrap();
        let created = orchestrator
            .create_meeting_request_for_latest(&recommendation)
            .unwrap();

        assert_eq!(
            created.artifact.artifact_type(),
            LabArtifactType::LabMeetingRequest
        );
        assert_eq!(created.gate.stage, "lab_meeting_request");
        assert_eq!(
            created.gate.next_action.as_deref(),
            Some("open_read_only_lab_meeting")
        );
        assert!(created.path.exists());
        assert!(created.report_path.exists());
        let StageArtifact::LabMeetingRequest(request) = &created.artifact else {
            panic!("expected LabMeetingRequest artifact");
        };
        assert_eq!(request.owner, LabRole::Runtime);
        assert_eq!(
            request.validation_status.as_deref(),
            Some("runtime_escalation_signal")
        );
        assert_eq!(request.body.reason, "runtime_escalation_signals_present");
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert!(saved
            .artifact_ids
            .iter()
            .any(|id| id == created.artifact.artifact_id()));
    }

    #[test]
    fn blocker_report_writes_postdoc_handoff_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .store()
            .block_graduate_task(&run.lab_run_id, &task.task_id, "scope is unclear")
            .unwrap();

        let created = orchestrator
            .create_blocker_report_for_latest(Some("Need professor decision"))
            .unwrap();

        assert_eq!(
            created.artifact.artifact_type(),
            LabArtifactType::LabBlockerReport
        );
        assert!(created.path.exists());
        assert!(created.report_path.exists());
        let task_ref = format!("task:{}", task.task_id);
        assert!(created
            .gate
            .evidence_refs
            .iter()
            .any(|item| item == &task_ref));
        match &created.artifact {
            StageArtifact::LabBlockerReport(report) => {
                assert!(report.evidence_refs.iter().any(|item| item == &task_ref));
            }
            other => panic!("expected LabBlockerReport, got {:?}", other.artifact_type()),
        }
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert!(saved
            .artifact_ids
            .iter()
            .any(|id| id == created.artifact.artifact_id()));
        assert!(saved
            .blocked_reason
            .as_deref()
            .unwrap_or_default()
            .starts_with("blocker_"));
    }

    #[test]
    fn blocker_escalation_moves_run_to_professor_review_gate() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .store()
            .block_graduate_task(&run.lab_run_id, &task.task_id, "scope is unclear")
            .unwrap();
        orchestrator
            .create_blocker_report_for_latest(Some("Need professor decision"))
            .unwrap();

        let escalated = orchestrator
            .escalate_latest_blocker_to_professor_review()
            .unwrap();

        assert_eq!(escalated.current_stage, "professor_review");
        assert_eq!(escalated.internal_owner, LabRole::Professor);
        let gate = orchestrator.required_gate_for_latest().unwrap();
        assert_eq!(gate.stage, "professor_review");
        assert_eq!(gate.required_artifact_type, "ProfessorReview");
    }

    #[test]
    fn graduate_result_artifact_completes_task_and_preserves_not_verified_status() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab task result binding.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();

        let created = orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented result binding.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                vec!["labevidence_001".to_string()],
            )
            .unwrap();

        assert_eq!(
            created.artifact.artifact_type(),
            LabArtifactType::GraduateResult
        );
        assert_eq!(
            created.artifact.validation_status(),
            Some("subagent_report_not_parent_verified")
        );
        assert!(created.path.exists());
        assert!(created.report_path.exists());
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(
            saved_task.result_artifact_id.as_deref(),
            Some(created.artifact.artifact_id())
        );
        let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert!(saved_run.open_task_ids.is_empty());
    }

    #[test]
    fn postdoc_integration_summary_accepts_unblocked_graduate_results() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab integration bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let result = orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented integration bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();

        let created = orchestrator
            .create_postdoc_integration_summary_for_latest(Some("Postdoc verified result shape."))
            .unwrap();

        assert_eq!(
            created.artifact.artifact_type(),
            LabArtifactType::PostdocIntegrationSummary
        );
        assert!(created.gate.is_satisfied());
        assert_eq!(
            created.artifact.validation_status(),
            Some("postdoc_integrated_pending_professor_review")
        );
        match created.artifact {
            StageArtifact::PostdocIntegrationSummary(envelope) => {
                assert!(envelope
                    .body
                    .accepted_results
                    .iter()
                    .any(|item| item.contains(result.artifact.artifact_id())));
                assert!(envelope
                    .body
                    .remaining_risks
                    .iter()
                    .any(|risk| risk.contains("pending parent verification")));
            }
            other => panic!(
                "expected integration summary, got {:?}",
                other.artifact_type()
            ),
        }
    }

    #[test]
    fn postdoc_integration_summary_includes_graduate_worktree_runtime_proof() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab integration bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented integration bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let durable_agent_ref = format!("lab-graduate-{}", task.task_id);
        orchestrator
            .store()
            .record_run_event(
                &run.lab_run_id,
                "lab_graduate_worktree_action",
                serde_json::json!({
                    "task_id": task.task_id,
                    "agent_ref_kind": "task_id",
                    "agent_ref": durable_agent_ref,
                    "action": "agent_merge",
                    "success": true,
                    "result_data": {
                        "merge_kind": "tracked_diff",
                        "dirty": false,
                        "path": temp.path().join(".priority-agent/worktrees/lab-graduate").display().to_string(),
                    },
                }),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();

        let created = orchestrator
            .create_postdoc_integration_summary_for_latest(Some(
                "Postdoc verified runtime worktree proof.",
            ))
            .unwrap();

        let report = std::fs::read_to_string(&created.report_path).unwrap();
        match created.artifact {
            StageArtifact::PostdocIntegrationSummary(envelope) => {
                assert!(envelope
                    .body
                    .accepted_results
                    .iter()
                    .any(|item| item.contains("runtime worktree proof: agent_merge")
                        && item.contains("merge_kind=tracked_diff")));
                assert!(envelope
                    .evidence_refs
                    .iter()
                    .any(|item| item.starts_with("event:event_")));
            }
            other => panic!(
                "expected integration summary, got {:?}",
                other.artifact_type()
            ),
        }
        assert!(report.contains("runtime worktree proof: agent_merge"));
        assert!(report.contains("merge_kind=tracked_diff"));
        assert!(report.contains("event:event_"));
    }

    #[test]
    fn postdoc_integration_summary_includes_workspace_snapshot_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab integration bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented integration bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        orchestrator
            .store()
            .record_run_event(
                &run.lab_run_id,
                "lab_graduate_workspace_snapshot",
                serde_json::json!({
                    "task_id": task.task_id,
                    "dispatch_id": "dispatch_before_snapshot",
                    "phase": "before",
                    "dirty_path_count": 2,
                    "dirty_paths": [
                        "preexisting-user-change.txt",
                        "src/lib.rs"
                    ],
                    "changed_path_count": 0,
                    "changed_paths": [],
                }),
            )
            .unwrap();
        orchestrator
            .store()
            .record_run_event(
                &run.lab_run_id,
                "lab_graduate_workspace_snapshot",
                serde_json::json!({
                    "task_id": task.task_id,
                    "dispatch_id": "dispatch_after_snapshot",
                    "phase": "after",
                    "dirty_path_count": 3,
                    "dirty_paths": [
                        "preexisting-user-change.txt",
                        "src/lib.rs",
                        "src/lab/model.rs"
                    ],
                    "changed_path_count": 1,
                    "changed_paths": [
                        "src/lab/model.rs"
                    ],
                }),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();

        let created = orchestrator
            .create_postdoc_integration_summary_for_latest(Some(
                "Postdoc checked workspace snapshots.",
            ))
            .unwrap();

        assert!(created.gate.is_satisfied());
        assert!(created
            .gate
            .evidence_refs
            .iter()
            .any(|item| item.starts_with("event:event_")));
        let report = std::fs::read_to_string(&created.report_path).unwrap();
        match created.artifact {
            StageArtifact::PostdocIntegrationSummary(envelope) => {
                assert!(envelope.body.accepted_results.iter().any(|item| {
                    item.contains("runtime workspace delta: after task=")
                        && item.contains("changed=[src/lab/model.rs]")
                }));
                assert!(envelope.body.remaining_risks.iter().any(|risk| {
                    risk.contains("pre-existing workspace changes: before task=")
                        && risk.contains("dirty=[preexisting-user-change.txt,src/lib.rs]")
                }));
                assert!(envelope
                    .evidence_refs
                    .iter()
                    .any(|item| item.starts_with("event:event_")));
            }
            other => panic!(
                "expected integration summary, got {:?}",
                other.artifact_type()
            ),
        }
        assert!(report.contains("runtime workspace delta: after task="));
        assert!(report.contains("changed=[src/lab/model.rs]"));
        assert!(report.contains("pre-existing workspace changes: before task="));
        assert!(report.contains("dirty=[preexisting-user-change.txt,src/lib.rs]"));
    }

    #[test]
    fn postdoc_integration_summary_blocks_on_graduate_result_blockers() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab integration bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Could not complete integration.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q failed".to_string()],
                vec!["validation still fails".to_string()],
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();

        let created = orchestrator
            .create_postdoc_integration_summary_for_latest(None)
            .unwrap();

        assert!(!created.gate.is_satisfied());
        assert_eq!(
            created.gate.validation_status.as_deref(),
            Some("needs_revision")
        );
        assert!(created
            .gate
            .blockers
            .iter()
            .any(|blocker| blocker.contains("validation still fails")));
    }

    #[test]
    fn professor_review_accepts_valid_postdoc_integration() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab professor review bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented professor review bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();
        orchestrator
            .store()
            .record_run_event(
                &run.lab_run_id,
                "lab_graduate_workspace_snapshot",
                serde_json::json!({
                    "task_id": task.task_id,
                    "dispatch_id": "dispatch_professor_review_snapshot",
                    "phase": "after",
                    "dirty_path_count": 1,
                    "dirty_paths": ["src/lab/orchestrator.rs"],
                    "changed_path_count": 1,
                    "changed_paths": ["src/lab/orchestrator.rs"],
                }),
            )
            .unwrap();
        orchestrator
            .create_postdoc_integration_summary_for_latest(Some("Postdoc integrated result."))
            .unwrap();
        let advanced = orchestrator.advance_latest().unwrap();
        assert_eq!(advanced.current_stage, "professor_review");

        let review = orchestrator
            .create_professor_review_for_latest(Some("Professor accepts the evidence."))
            .unwrap();

        assert!(review.gate.is_satisfied());
        assert_eq!(review.gate.validation_status.as_deref(), Some("validated"));
        assert!(review
            .gate
            .evidence_refs
            .iter()
            .any(|item| item.starts_with("event:event_")));
        let report = std::fs::read_to_string(&review.report_path).unwrap();
        assert!(report.contains("event:event_"));
        match review.artifact {
            StageArtifact::ProfessorReview(envelope) => {
                assert!(envelope.body.accepted);
                assert!(envelope.body.required_revisions.is_empty());
                assert!(envelope.body.user_report.contains("ready for user review"));
                assert!(envelope
                    .evidence_refs
                    .iter()
                    .any(|item| item.starts_with("event:event_")));
            }
            other => panic!("expected professor review, got {:?}", other.artifact_type()),
        }
        let user_report = orchestrator.advance_latest().unwrap();
        assert_eq!(user_report.current_stage, "user_report");
        assert!(user_report.needs_user);
    }

    #[test]
    fn professor_review_blocks_needs_revision_integration() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab professor review bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Blocked professor review bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q failed".to_string()],
                vec!["validation still fails".to_string()],
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();
        orchestrator
            .create_postdoc_integration_summary_for_latest(None)
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "professor_review".to_string();
        saved.internal_owner = LabRole::Professor;
        orchestrator.store().save_run(&saved).unwrap();

        let review = orchestrator
            .create_professor_review_for_latest(None)
            .unwrap();

        assert!(!review.gate.is_satisfied());
        assert_eq!(
            review.gate.validation_status.as_deref(),
            Some("needs_revision")
        );
        assert!(review
            .gate
            .blockers
            .iter()
            .any(|blocker| blocker.contains("validation still fails")));
    }

    #[test]
    fn postdoc_plan_consumes_pending_professor_revision_task() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab professor review bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Blocked professor review bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q failed".to_string()],
                vec!["validation still fails".to_string()],
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();
        orchestrator
            .create_postdoc_integration_summary_for_latest(None)
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "professor_review".to_string();
        saved.internal_owner = LabRole::Professor;
        orchestrator.store().save_run(&saved).unwrap();

        let review = orchestrator
            .create_professor_review_for_latest(None)
            .unwrap();
        let inherited_review_ref = match &review.artifact {
            StageArtifact::ProfessorReview(envelope) => envelope
                .evidence_refs
                .iter()
                .find(|item| item.starts_with("artifact:artifact_postdocintegrationsummary_"))
                .cloned()
                .expect("professor review inherited postdoc evidence"),
            other => panic!("expected ProfessorReview, got {:?}", other.artifact_type()),
        };
        let revision_artifact_id = orchestrator
            .store()
            .list_stage_artifacts(&run.lab_run_id)
            .unwrap()
            .into_iter()
            .find_map(|artifact| match artifact {
                StageArtifact::LabRevisionTask(revision) => Some(revision.artifact_id),
                _ => None,
            })
            .expect("revision task artifact");
        let revision_artifact = orchestrator
            .store()
            .load_stage_artifact(&run.lab_run_id, &revision_artifact_id)
            .unwrap();
        match &revision_artifact {
            StageArtifact::LabRevisionTask(revision) => {
                assert!(revision
                    .evidence_refs
                    .iter()
                    .any(|item| item == &format!("artifact:{}", review.artifact.artifact_id())));
                assert!(revision
                    .evidence_refs
                    .iter()
                    .any(|item| item == &inherited_review_ref));
            }
            other => panic!("expected LabRevisionTask, got {:?}", other.artifact_type()),
        }
        let revision_gate = orchestrator
            .store()
            .load_artifact_gate(&run.lab_run_id, "postdoc_revision")
            .unwrap();
        assert!(revision_gate
            .evidence_refs
            .iter()
            .any(|item| item == &inherited_review_ref));

        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_plan".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();
        let postdoc_plan = orchestrator
            .create_current_stage_artifact_for_latest("Revise according to professor feedback.")
            .unwrap();

        match postdoc_plan.artifact {
            StageArtifact::PostdocPlan(plan) => {
                assert!(plan
                    .evidence_refs
                    .iter()
                    .any(|item| item == &format!("artifact:{revision_artifact_id}")));
                assert!(plan
                    .body
                    .graduate_handoff
                    .contains(review.artifact.artifact_id()));
                assert!(plan.body.slices.iter().any(|slice| {
                    slice.contains("validation still fails")
                        || slice.contains("Postdoc integration is marked needs_revision")
                }));
            }
            other => panic!("expected PostdocPlan, got {:?}", other.artifact_type()),
        }

        let consumed = orchestrator
            .store()
            .load_stage_artifact(&run.lab_run_id, &revision_artifact_id)
            .unwrap();
        assert_eq!(consumed.validation_status(), Some("consumed"));
    }

    #[test]
    fn graduate_result_rejects_changed_files_outside_allowed_scope() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();

        let err = orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Changed unrelated file.",
                vec!["src/main.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap_err()
            .to_string();

        assert!(err.contains("outside allowed_scope"));
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, crate::lab::model::LabTaskStatus::Queued);
    }

    #[test]
    fn workspace_change_delta_ignores_preexisting_dirty_files() {
        let before = BTreeMap::from([
            ("src/lib.rs".to_string(), "file:1:aaa".to_string()),
            ("src/main.rs".to_string(), "file:1:bbb".to_string()),
        ]);
        let after = BTreeMap::from([
            (
                ".claude/worktrees/agent-live-proof/".to_string(),
                "non_file:64".to_string(),
            ),
            (
                ".priority-agent/lab/events.jsonl".to_string(),
                "file:1:eee".to_string(),
            ),
            ("src/lib.rs".to_string(), "file:1:aaa".to_string()),
            ("src/lab/model.rs".to_string(), "file:1:ddd".to_string()),
            ("src/main.rs".to_string(), "file:2:ccc".to_string()),
        ]);

        let changed = changed_paths_between(&before, &after);

        assert_eq!(
            changed,
            vec!["src/lab/model.rs".to_string(), "src/main.rs".to_string()]
        );
        assert!(validate_changed_files_within_scope(
            &["src/lab".to_string(), "src/main.rs".to_string()],
            &changed,
        )
        .is_ok());
        assert!(validate_changed_files_within_scope(&["src/lab".to_string()], &changed).is_err());
    }

    #[test]
    fn graduate_runtime_verification_rejects_missing_actual_changes() {
        let project = tempfile::tempdir().unwrap();
        init_git_dir(project.path());
        let context = lab_context_with_agent_worktree(
            project.path(),
            "lab-provider-command",
            "agent_1",
            project.path(),
        );
        let task = GraduateTask {
            schema_version: LAB_SCHEMA_VERSION,
            task_id: "gradtask_test".to_string(),
            lab_run_id: "labrun_test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: LabRole::Postdoc,
            assigned_role: LabRole::Graduate,
            status: LabTaskStatus::InProgress,
            title: "Write proof".to_string(),
            instructions: "Create proof.txt".to_string(),
            allowed_scope: vec!["proof.txt".to_string()],
            required_validation: vec!["test -f proof.txt".to_string()],
            evidence_ids: Vec::new(),
            result_artifact_id: None,
            blocker: None,
            cycle_id: Some("0".to_string()),
        };

        let err = runtime_verify_graduate_task_result(
            &task,
            &context,
            Some("agent_1"),
            "lab-graduate-gradtask_test",
            &[],
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("no actual file changes"));
    }

    #[test]
    fn graduate_runtime_verification_checks_worktree_scope_and_validation() {
        let project = tempfile::tempdir().unwrap();
        init_git_dir(project.path());
        let worktree = tempfile::tempdir().unwrap();
        init_git_dir(worktree.path());
        std::fs::write(worktree.path().join("proof.txt"), "verified\n").unwrap();
        let context = lab_context_with_agent_worktree(
            project.path(),
            "lab-provider-command",
            "agent_1",
            worktree.path(),
        );
        let task = GraduateTask {
            schema_version: LAB_SCHEMA_VERSION,
            task_id: "gradtask_test".to_string(),
            lab_run_id: "labrun_test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: LabRole::Postdoc,
            assigned_role: LabRole::Graduate,
            status: LabTaskStatus::InProgress,
            title: "Write proof".to_string(),
            instructions: "Create proof.txt".to_string(),
            allowed_scope: vec!["proof.txt".to_string()],
            required_validation: vec!["test -f proof.txt".to_string()],
            evidence_ids: Vec::new(),
            result_artifact_id: None,
            blocker: None,
            cycle_id: Some("0".to_string()),
        };

        let evidence = runtime_verify_graduate_task_result(
            &task,
            &context,
            Some("agent_1"),
            "lab-graduate-gradtask_test",
            &[],
        )
        .unwrap();

        assert_eq!(evidence.changed_files, vec!["proof.txt".to_string()]);
        assert!(evidence
            .validation_attempts
            .contains(&"runtime validation `test -f proof.txt` passed".to_string()));
    }

    #[test]
    fn graduate_runtime_verification_falls_back_to_durable_task_id() {
        let project = tempfile::tempdir().unwrap();
        init_git_dir(project.path());
        let worktree = tempfile::tempdir().unwrap();
        init_git_dir(worktree.path());
        std::fs::write(worktree.path().join("proof.txt"), "verified\n").unwrap();
        let context = lab_context_with_agent_worktree_task_id(
            project.path(),
            "lab-provider-command",
            "lab-graduate-gradtask_test",
            "agent_1",
            worktree.path(),
        );
        let task = GraduateTask {
            schema_version: LAB_SCHEMA_VERSION,
            task_id: "gradtask_test".to_string(),
            lab_run_id: "labrun_test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: LabRole::Postdoc,
            assigned_role: LabRole::Graduate,
            status: LabTaskStatus::InProgress,
            title: "Write proof".to_string(),
            instructions: "Create proof.txt".to_string(),
            allowed_scope: vec!["proof.txt".to_string()],
            required_validation: vec!["test -f proof.txt".to_string()],
            evidence_ids: Vec::new(),
            result_artifact_id: None,
            blocker: None,
            cycle_id: Some("0".to_string()),
        };

        let evidence = runtime_verify_graduate_task_result(
            &task,
            &context,
            Some("unknown_agent"),
            "lab-graduate-gradtask_test",
            &[],
        )
        .unwrap();

        assert_eq!(evidence.changed_files, vec!["proof.txt".to_string()]);
        assert!(evidence
            .validation_attempts
            .contains(&"runtime validation `test -f proof.txt` passed".to_string()));
    }

    #[test]
    fn git_status_path_parser_handles_renames_and_quotes() {
        assert_eq!(
            parse_git_status_path(" M src/lib.rs").as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(
            parse_git_status_path("R  old.rs -> src/new.rs").as_deref(),
            Some("src/new.rs")
        );
        assert_eq!(
            parse_git_status_path("?? \"docs/lab plan.md\"").as_deref(),
            Some("docs/lab plan.md")
        );
    }

    #[test]
    fn graduate_agent_result_parser_requires_structured_validation() {
        let data = serde_json::json!({
            "graduate_result": {
                "summary": "Implemented the scoped slice.",
                "changed_files": ["src/lab/orchestrator.rs"],
                "validation_results": ["cargo check -q passed"],
                "blockers": [],
                "evidence_ids": ["labevidence_1"]
            }
        });

        let parsed = parse_graduate_agent_result(Some(&data), "").unwrap();

        assert_eq!(parsed.task_summary, "Implemented the scoped slice.");
        assert_eq!(parsed.changed_files, vec!["src/lab/orchestrator.rs"]);
        assert_eq!(parsed.validation_attempts, vec!["cargo check -q passed"]);
        assert_eq!(parsed.evidence_ids, vec!["labevidence_1"]);
        let fenced = parse_graduate_agent_result(
            None,
            r#"```json
{"graduate_result":{"summary":"Implemented fenced JSON.","changed_files":["src/lab/model.rs"],"validation_results":["cargo check -q passed"],"blockers":[],"evidence_ids":[]}}
```"#,
        )
        .unwrap();
        assert_eq!(fenced.task_summary, "Implemented fenced JSON.");
        let prose = parse_graduate_agent_result(
            None,
            r#"Done:
{"graduate_result":{"summary":"Implemented prose JSON.","changed_files":["src/lab/model.rs"],"validation_results":["cargo check -q passed"],"blockers":[],"evidence_ids":[]}}
Thanks."#,
        )
        .unwrap();
        assert_eq!(prose.task_summary, "Implemented prose JSON.");
        assert!(parse_graduate_agent_result(None, "plain text result").is_none());
        assert!(parse_graduate_agent_result(
            Some(&serde_json::json!({"summary": "missing validation"})),
            ""
        )
        .is_none());
    }

    #[test]
    fn unbound_graduate_success_is_failed_and_blocks_task() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let dispatch = build_graduate_task_dispatch(&task).unwrap();
        let record = orchestrator
            .store()
            .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
            .unwrap();
        orchestrator
            .store()
            .start_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();

        let failed = orchestrator
            .mark_unbound_graduate_success_failed(
                &run,
                &task,
                &record.dispatch_id,
                Some("agent_test".to_string()),
                "I finished it, but I did not return JSON.",
            )
            .unwrap();

        assert_eq!(failed.status, GraduateDispatchStatus::Failed);
        assert_eq!(failed.agent_id.as_deref(), Some("agent_test"));
        assert!(failed
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("without bindable GraduateResult JSON"));
        assert!(failed
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("result_preview=I finished it"));
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, LabTaskStatus::Blocked);
        assert!(saved_task
            .blocker
            .as_deref()
            .unwrap_or_default()
            .contains("without bindable GraduateResult JSON"));
        let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved_run.failure_count, 1);
    }

    #[test]
    fn unbound_graduate_success_can_bind_runtime_verified_result() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Create proof file",
                "Create only the proof file.",
                vec!["lab-live-graduate-proof.md".to_string()],
                vec!["test -f lab-live-graduate-proof.md".to_string()],
            )
            .unwrap();
        orchestrator
            .store()
            .start_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();

        let worktree = temp.path().join("graduate-unbound-runtime-worktree");
        std::fs::create_dir_all(&worktree).unwrap();
        init_git_dir(&worktree);
        std::fs::write(
            worktree.join("lab-live-graduate-proof.md"),
            "runtime verified\n",
        )
        .unwrap();
        let agent_task_id = graduate_agent_task_id(&task);
        let context = lab_context_with_agent_worktree_task_id(
            temp.path(),
            "lab-test",
            &agent_task_id,
            "agent_runtime_verified",
            &worktree,
        );

        let created = orchestrator
            .create_runtime_verified_graduate_result_for_unbound_success(
                &task,
                &context,
                Some("agent_runtime_verified"),
                &[],
                "The iteration limit was reached before final JSON.",
            )
            .unwrap();

        match created.artifact {
            StageArtifact::GraduateResult(envelope) => {
                assert_eq!(
                    envelope.body.changed_files,
                    vec!["lab-live-graduate-proof.md".to_string()]
                );
                assert!(envelope.body.validation_attempts.contains(
                    &"runtime validation `test -f lab-live-graduate-proof.md` passed".to_string()
                ));
                assert!(envelope
                    .body
                    .task_summary
                    .contains("without bindable GraduateResult JSON"));
                assert!(envelope
                    .evidence_refs
                    .contains(&format!("agent_task:{agent_task_id}")));
                assert!(envelope
                    .evidence_refs
                    .contains(&"agent:agent_runtime_verified".to_string()));
            }
            other => panic!("expected GraduateResult, got {:?}", other.artifact_type()),
        }
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, LabTaskStatus::Completed);
    }

    #[tokio::test]
    async fn graduate_dispatch_execution_records_failure_without_agent_manager() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();

        let dispatch = orchestrator
            .execute_graduate_task_latest_with_context(
                &task.task_id,
                ToolContext::new(temp.path(), "lab-test"),
            )
            .await
            .unwrap();

        assert_eq!(dispatch.status, GraduateDispatchStatus::Failed);
        assert!(dispatch
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("AgentManager not available"));
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, LabTaskStatus::Blocked);
        let dispatches = orchestrator
            .store()
            .list_graduate_dispatches(&run.lab_run_id)
            .unwrap();
        assert_eq!(dispatches.len(), 1);
        assert_eq!(dispatches[0].status, GraduateDispatchStatus::Failed);
        let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved_run.failure_count, 1);
        assert_eq!(saved_run.status, LabRunStatus::Active);
    }

    #[tokio::test]
    async fn graduate_dispatch_records_workspace_snapshots_around_execution() {
        let temp = tempfile::tempdir().unwrap();
        init_git_dir(temp.path());
        std::fs::write(
            temp.path().join("preexisting-user-change.txt"),
            "user edit\n",
        )
        .unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();

        let dispatch = orchestrator
            .execute_graduate_task_latest_with_context(
                &task.task_id,
                ToolContext::new(temp.path(), "lab-test"),
            )
            .await
            .unwrap();

        assert_eq!(dispatch.status, GraduateDispatchStatus::Failed);
        let events = orchestrator
            .store()
            .list_run_events(&run.lab_run_id)
            .unwrap()
            .into_iter()
            .filter(|event| event.event_type == "lab_graduate_workspace_snapshot")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload["phase"], "before");
        assert_eq!(events[0].payload["task_id"], task.task_id);
        assert_eq!(events[0].payload["dispatch_id"], dispatch.dispatch_id);
        assert_eq!(events[0].payload["dirty_path_count"], 1);
        assert_eq!(
            events[0].payload["dirty_paths"][0],
            "preexisting-user-change.txt"
        );
        assert_eq!(events[1].payload["phase"], "after");
        assert_eq!(events[1].payload["changed_path_count"], 0);
    }

    #[tokio::test]
    async fn graduate_dispatch_binds_completed_durable_agent_state_without_agent_manager() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Bind durable graduate result",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["test -f src/lab/model.rs".to_string()],
            )
            .unwrap();

        let worktree = temp.path().join("graduate-durable-run-worktree");
        std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
        init_git_dir(&worktree);
        std::fs::write(worktree.join("src/lab/model.rs"), "durable graduate edit\n").unwrap();
        let agent_task_id = graduate_agent_task_id(&task);
        let context = lab_context_with_agent_worktree_task_id(
            temp.path(),
            "lab-test",
            &agent_task_id,
            "agent_durable_run",
            &worktree,
        );
        let session_store = context.session_store.as_ref().unwrap();
        let agent_artifact_id = session_store
            .add_agent_artifact(
                "lab-test",
                "agent_durable_run",
                Some("lab-graduate"),
                "implementation",
                "completed",
                "durable graduate result",
                r#"{"graduate_result":{"summary":"Durable graduate result was bound.","changed_files":["src/lab/model.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
                &serde_json::json!({"completion_sink": "agent_manager", "tools_used": ["file_write", "bash"]}),
            )
            .unwrap();
        session_store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: "lab-test".to_string(),
                task_id: agent_task_id.clone(),
                agent_id: "agent_durable_run".to_string(),
                profile: Some("lab-graduate".to_string()),
                role: "implementation".to_string(),
                status: "completed".to_string(),
                description: "durable graduate result".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: Some(agent_artifact_id),
                cleanup_hooks: vec!["worktree_cleanup".to_string()],
                payload: serde_json::json!({
                    "completion_sink": "agent_manager",
                    "tools_used": ["file_write", "bash"],
                    "isolated_worktree": {
                        "path": worktree.to_string_lossy().to_string(),
                        "branch": "codex/graduate-durable-run"
                    }
                }),
            })
            .unwrap();

        let dispatch = orchestrator
            .execute_graduate_task_latest_with_context(&task.task_id, context)
            .await
            .unwrap();

        assert_eq!(dispatch.status, GraduateDispatchStatus::Succeeded);
        assert_eq!(dispatch.agent_id.as_deref(), Some("agent_durable_run"));
        assert!(dispatch.result_artifact_id.is_some());
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, LabTaskStatus::Completed);
        let artifact = orchestrator
            .store()
            .load_stage_artifact(
                &run.lab_run_id,
                dispatch.result_artifact_id.as_deref().unwrap(),
            )
            .unwrap();
        match artifact {
            StageArtifact::GraduateResult(envelope) => {
                assert_eq!(
                    envelope.body.changed_files,
                    vec!["src/lab/model.rs".to_string()]
                );
                assert!(envelope
                    .body
                    .validation_attempts
                    .iter()
                    .any(|item| item.contains("runtime validation")));
            }
            other => panic!("expected GraduateResult, got {:?}", other.artifact_type()),
        }
    }

    #[tokio::test]
    async fn graduate_dispatch_is_not_blocked_by_provider_name_before_agent_run() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let mut context = ToolContext::new(temp.path(), "lab-test").with_model("deepseek-v4-flash");
        context
            .metadata
            .insert("provider_id".to_string(), "deepseek".to_string());

        let dispatch = orchestrator
            .execute_graduate_task_latest_with_context(&task.task_id, context)
            .await
            .unwrap();

        assert_eq!(dispatch.status, GraduateDispatchStatus::Failed);
        let error = dispatch.error.as_deref().unwrap_or_default().to_string();
        assert!(!error.contains("not certified"));
        assert!(!error.contains("formal Lab graduate certification"));
        assert!(!error.contains("graduate provider"));
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, LabTaskStatus::Blocked);
        let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved_run.failure_count, 1);
    }

    #[test]
    fn graduate_agent_task_sync_binds_completed_durable_state_after_runtime_verification() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.current_stage = "graduate_work".to_string();
        run.internal_owner = LabRole::Graduate;
        orchestrator.store().save_run(&run).unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement durable sync",
                "Update only the lab orchestrator.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["test -f src/lab/orchestrator.rs".to_string()],
            )
            .unwrap();
        let dispatch = build_graduate_task_dispatch(&task).unwrap();
        let record = orchestrator
            .store()
            .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
            .unwrap();
        orchestrator
            .store()
            .start_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();

        let worktree = temp.path().join("graduate-worktree");
        std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
        init_git_dir(&worktree);
        std::fs::write(
            worktree.join("src/lab/orchestrator.rs"),
            "verified graduate edit\n",
        )
        .unwrap();
        let agent_task_id = graduate_agent_task_id(&task);
        let context = lab_context_with_agent_worktree_task_id(
            temp.path(),
            "lab-test",
            &agent_task_id,
            "agent_sync",
            &worktree,
        );
        let session_store = context.session_store.as_ref().unwrap();
        let agent_artifact_id = session_store
            .add_agent_artifact(
                "lab-test",
                "agent_sync",
                Some("lab-graduate"),
                "implementation",
                "completed",
                "graduate durable sync result",
                r#"{"graduate_result":{"summary":"Synced durable graduate result.","changed_files":["src/lab/orchestrator.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
                &serde_json::json!({"completion_sink": "agent_manager"}),
            )
            .unwrap();
        session_store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: "lab-test".to_string(),
                task_id: agent_task_id.clone(),
                agent_id: "agent_sync".to_string(),
                profile: Some("lab-graduate".to_string()),
                role: "implementation".to_string(),
                status: "completed".to_string(),
                description: "graduate durable sync result".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: Some(agent_artifact_id),
                cleanup_hooks: vec!["worktree_cleanup".to_string()],
                payload: serde_json::json!({
                    "completion_sink": "agent_manager",
                    "tools_used": ["file_write", "bash"],
                    "isolated_worktree": {
                        "path": worktree.to_string_lossy().to_string(),
                        "branch": "codex/graduate-sync"
                    }
                }),
            })
            .unwrap();

        let created = orchestrator
            .sync_graduate_agent_task_latest_with_context(&task.task_id, context)
            .unwrap();
        let graduate_result_artifact_id = created.artifact.artifact_id().to_string();

        match &created.artifact {
            StageArtifact::GraduateResult(envelope) => {
                assert_eq!(
                    envelope.body.changed_files,
                    vec!["src/lab/orchestrator.rs".to_string()]
                );
                assert!(envelope
                    .body
                    .validation_attempts
                    .iter()
                    .any(|attempt| attempt
                        == "runtime validation `test -f src/lab/orchestrator.rs` passed"));
                assert!(envelope
                    .evidence_refs
                    .contains(&format!("agent_task:{agent_task_id}")));
                assert!(envelope
                    .evidence_refs
                    .contains(&format!("agent_artifact:{agent_artifact_id}")));
            }
            other => panic!("expected GraduateResult, got {:?}", other.artifact_type()),
        }
        assert!(created.gate.is_satisfied());
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, LabTaskStatus::Completed);
        let saved_dispatch = orchestrator
            .store()
            .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
            .unwrap();
        assert_eq!(saved_dispatch.status, GraduateDispatchStatus::Succeeded);
        assert_eq!(saved_dispatch.agent_id.as_deref(), Some("agent_sync"));
        assert_eq!(
            saved_dispatch.result_artifact_id.as_deref(),
            Some(graduate_result_artifact_id.as_str())
        );
    }

    #[tokio::test]
    async fn repeated_graduate_dispatch_failures_escalate_to_needs_user() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();

        let _ = orchestrator
            .execute_graduate_task_latest_with_context(
                &task.task_id,
                ToolContext::new(temp.path(), "lab-test"),
            )
            .await
            .unwrap();
        let second = orchestrator
            .execute_graduate_task_latest_with_context(
                &task.task_id,
                ToolContext::new(temp.path(), "lab-test"),
            )
            .await
            .unwrap();

        assert_eq!(second.status, GraduateDispatchStatus::Failed);
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.failure_count, 2);
        assert_eq!(saved.status, LabRunStatus::NeedsUser);
        assert!(saved.needs_user);
    }

    #[tokio::test]
    async fn scheduler_blocks_graduate_work_without_queued_task() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.current_stage = "graduate_work".to_string();
        run.internal_owner = LabRole::Graduate;
        orchestrator.store().save_run(&run).unwrap();

        let step = orchestrator
            .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
            .await
            .unwrap();

        assert_eq!(step.action, LabSchedulerStepAction::Blocked);
        assert!(step.message.contains("requires a queued GraduateTask"));
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert!(saved.artifact_ids.is_empty());
    }

    #[tokio::test]
    async fn scheduler_refuses_to_run_without_active_lease() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        std::fs::remove_file(orchestrator.store().root().join("active_lease.json")).unwrap();

        let err = orchestrator
            .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
            .await
            .unwrap_err()
            .to_string();

        assert!(err.contains("active lease is missing"));
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert!(saved.artifact_ids.is_empty());
    }

    #[tokio::test]
    async fn scheduler_dispatches_queued_graduate_task() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.current_stage = "graduate_work".to_string();
        run.internal_owner = LabRole::Graduate;
        orchestrator.store().save_run(&run).unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab model.",
                vec!["src/lab/model.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();

        let step = orchestrator
            .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
            .await
            .unwrap();

        assert_eq!(step.action, LabSchedulerStepAction::GraduateDispatched);
        assert_eq!(step.task_id.as_deref(), Some(task.task_id.as_str()));
        assert!(step.dispatch_id.is_some());
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, LabTaskStatus::Blocked);
    }

    #[tokio::test]
    async fn scheduler_advances_after_verified_graduate_result() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.current_stage = "graduate_work".to_string();
        run.internal_owner = LabRole::Graduate;
        orchestrator.store().save_run(&run).unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update only the lab orchestrator.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented scoped slice.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["runtime validation `cargo check -q` passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();

        let step = orchestrator
            .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
            .await
            .unwrap();

        assert_eq!(step.action, LabSchedulerStepAction::TickAdvanced);
        assert_eq!(step.stage, "postdoc_review");
        assert!(step.message.contains("verified GraduateResult"));
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.current_stage, "postdoc_review");
        assert_eq!(saved.internal_owner, LabRole::Postdoc);
    }

    #[tokio::test]
    async fn scheduler_syncs_completed_durable_graduate_task_before_blocking_in_progress() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.current_stage = "graduate_work".to_string();
        run.internal_owner = LabRole::Graduate;
        orchestrator.store().save_run(&run).unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Sync completed durable task",
                "Update only the lab orchestrator.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["test -f src/lab/orchestrator.rs".to_string()],
            )
            .unwrap();
        let dispatch = build_graduate_task_dispatch(&task).unwrap();
        let record = orchestrator
            .store()
            .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
            .unwrap();
        orchestrator
            .store()
            .start_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();

        let worktree = temp.path().join("graduate-scheduler-sync-worktree");
        std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
        init_git_dir(&worktree);
        std::fs::write(
            worktree.join("src/lab/orchestrator.rs"),
            "scheduler durable graduate edit\n",
        )
        .unwrap();
        let agent_task_id = graduate_agent_task_id(&task);
        let context = lab_context_with_agent_worktree_task_id(
            temp.path(),
            "lab-test",
            &agent_task_id,
            "agent_scheduler_sync",
            &worktree,
        );
        let session_store = context.session_store.as_ref().unwrap();
        let agent_artifact_id = session_store
            .add_agent_artifact(
                "lab-test",
                "agent_scheduler_sync",
                Some("lab-graduate"),
                "implementation",
                "completed",
                "graduate scheduler durable sync result",
                r#"{"graduate_result":{"summary":"Scheduler synced durable graduate result.","changed_files":["src/lab/orchestrator.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
                &serde_json::json!({"completion_sink": "agent_manager"}),
            )
            .unwrap();
        session_store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: "lab-test".to_string(),
                task_id: agent_task_id.clone(),
                agent_id: "agent_scheduler_sync".to_string(),
                profile: Some("lab-graduate".to_string()),
                role: "implementation".to_string(),
                status: "completed".to_string(),
                description: "graduate scheduler durable sync result".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: Some(agent_artifact_id),
                cleanup_hooks: vec!["worktree_cleanup".to_string()],
                payload: serde_json::json!({
                    "completion_sink": "agent_manager",
                    "tools_used": ["file_write", "bash"],
                    "isolated_worktree": {
                        "path": worktree.to_string_lossy().to_string(),
                        "branch": "codex/graduate-scheduler-sync"
                    }
                }),
            })
            .unwrap();

        let step = orchestrator
            .run_scheduler_step_latest_with_context(context)
            .await
            .unwrap();

        assert_eq!(step.action, LabSchedulerStepAction::TickAdvanced);
        assert_eq!(step.stage, "postdoc_review");
        assert_eq!(step.task_id.as_deref(), Some(task.task_id.as_str()));
        assert!(step.message.contains("synced durable graduate result"));
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, LabTaskStatus::Completed);
        let saved_dispatch = orchestrator
            .store()
            .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
            .unwrap();
        assert_eq!(saved_dispatch.status, GraduateDispatchStatus::Succeeded);
        assert_eq!(
            saved_dispatch.agent_id.as_deref(),
            Some("agent_scheduler_sync")
        );
        let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved_run.current_stage, "postdoc_review");
        assert_eq!(saved_run.internal_owner, LabRole::Postdoc);
    }

    #[tokio::test]
    async fn scheduler_blocks_completed_durable_graduate_task_without_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.current_stage = "graduate_work".to_string();
        run.internal_owner = LabRole::Graduate;
        orchestrator.store().save_run(&run).unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Sync incomplete durable task",
                "Update only the lab orchestrator.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["test -f src/lab/orchestrator.rs".to_string()],
            )
            .unwrap();
        let dispatch = build_graduate_task_dispatch(&task).unwrap();
        let record = orchestrator
            .store()
            .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
            .unwrap();
        orchestrator
            .store()
            .start_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        let agent_task_id = graduate_agent_task_id(&task);
        let context = lab_context_with_agent_worktree_task_id(
            temp.path(),
            "lab-test",
            &agent_task_id,
            "agent_missing_artifact",
            temp.path(),
        );

        let step = orchestrator
            .run_scheduler_step_latest_with_context(context)
            .await
            .unwrap();

        assert_eq!(step.action, LabSchedulerStepAction::Blocked);
        assert_eq!(step.stage, "graduate_work");
        assert_eq!(step.task_id.as_deref(), Some(task.task_id.as_str()));
        assert!(step.message.contains("has no result artifact"));
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(saved_task.status, LabTaskStatus::Blocked);
        assert!(saved_task
            .blocker
            .as_deref()
            .unwrap_or_default()
            .contains("has no result artifact"));
        let saved_dispatch = orchestrator
            .store()
            .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
            .unwrap();
        assert_eq!(saved_dispatch.status, GraduateDispatchStatus::Failed);
        assert_eq!(
            saved_dispatch.agent_id.as_deref(),
            Some("agent_missing_artifact")
        );
        assert!(saved_dispatch
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("has no result artifact"));
        let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved_run.failure_count, 1);
    }

    #[tokio::test]
    async fn scheduler_stops_at_role_review_boundaries() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update scheduler review bridges.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented scheduler review bridges.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();

        let postdoc = orchestrator
            .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
            .await
            .unwrap();
        assert_eq!(postdoc.action, LabSchedulerStepAction::Blocked);
        assert_eq!(postdoc.stage, "postdoc_review");
        assert!(postdoc.message.contains("role review artifact is required"));
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.current_stage, "postdoc_review");
        assert!(!saved.needs_user);
    }

    #[tokio::test]
    async fn explicit_professor_review_writes_revision_task_without_scheduler_auto_repair() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Repair validation failure.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Could not complete validation.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q failed".to_string()],
                vec!["validation still fails".to_string()],
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();
        orchestrator
            .create_postdoc_integration_summary_for_latest(None)
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "professor_review".to_string();
        saved.internal_owner = LabRole::Professor;
        orchestrator.store().save_run(&saved).unwrap();
        let professor_review = orchestrator
            .create_professor_review_for_latest(Some("Explicit professor revision request."))
            .unwrap();

        let step = orchestrator
            .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
            .await
            .unwrap();

        assert_eq!(step.action, LabSchedulerStepAction::Blocked);
        assert_eq!(step.stage, "professor_review");
        assert!(step.message.contains("role review artifact is required"));
        let resumed = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(resumed.current_stage, "professor_review");
        assert_eq!(resumed.internal_owner, LabRole::Professor);
        assert!(!resumed.needs_user);
        let revision_artifact_id = orchestrator
            .store()
            .list_stage_artifacts(&run.lab_run_id)
            .unwrap()
            .into_iter()
            .find_map(|artifact| match artifact {
                StageArtifact::LabRevisionTask(revision) => Some(revision.artifact_id),
                _ => None,
            })
            .expect("revision task artifact");
        assert!(professor_review
            .gate
            .blockers
            .iter()
            .any(|blocker| blocker.contains("runtime placeholder")));
        let gate = orchestrator
            .store()
            .load_artifact_gate(&run.lab_run_id, "postdoc_revision")
            .unwrap();
        assert_eq!(gate.required_artifact_type, "LabRevisionTask");
        assert_eq!(
            gate.artifact_id.as_deref(),
            Some(revision_artifact_id.as_str())
        );
        let revision = orchestrator
            .store()
            .load_stage_artifact(&run.lab_run_id, &revision_artifact_id)
            .unwrap();
        assert_eq!(revision.validation_status(), Some("not_started"));
    }

    #[tokio::test]
    async fn scheduler_blocks_when_postdoc_review_bridge_is_blocked() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update scheduler review bridges.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Could not finish scheduler review bridge.",
                vec!["src/lab/orchestrator.rs".to_string()],
                vec!["cargo check -q failed".to_string()],
                vec!["validation still fails".to_string()],
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();

        let step = orchestrator
            .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
            .await
            .unwrap();

        assert_eq!(step.action, LabSchedulerStepAction::Blocked);
        assert_eq!(step.stage, "postdoc_review");
        assert!(step.message.contains("PostdocIntegrationSummary"));
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.current_stage, "postdoc_review");
    }

    #[test]
    fn tick_blocks_without_current_stage_artifact_gate() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        let tick = orchestrator.tick_latest().unwrap();

        assert_eq!(tick.status, LabTickStatus::Blocked);
        assert_eq!(tick.from_stage, "professor_discussion");
        assert_eq!(tick.to_stage, "professor_discussion");
        assert!(tick.artifact_id.is_none());
        assert!(tick.report_path.is_none());
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.current_stage, "professor_discussion");
        assert_eq!(saved.internal_owner, LabRole::Professor);
    }

    #[test]
    fn tick_remains_blocked_until_role_artifact_exists() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        let first = orchestrator.tick_latest().unwrap();
        assert_eq!(first.status, LabTickStatus::Blocked);
        assert_eq!(first.to_stage, "professor_discussion");

        let second = orchestrator.tick_latest().unwrap();
        assert_eq!(second.status, LabTickStatus::Blocked);
        assert_eq!(second.to_stage, "professor_discussion");
        let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.status, LabRunStatus::Active);
        assert!(!saved.needs_user);
        assert_eq!(saved.current_stage, "professor_discussion");
    }

    #[test]
    fn continue_from_user_report_starts_next_cycle_with_fresh_professor_gate() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        for _ in 0..5 {
            orchestrator.tick_latest().unwrap();
        }

        let continued = orchestrator
            .continue_latest_from_user_report("first cycle reviewed; continue")
            .unwrap();

        assert_eq!(continued.lab_run_id, run.lab_run_id);
        assert_eq!(continued.status, LabRunStatus::Active);
        assert_eq!(continued.current_stage, "professor_discussion");
        assert_eq!(continued.internal_owner, LabRole::Professor);
        assert!(!continued.needs_user);
        assert_eq!(continued.cycle_count, 1);
        let gate = orchestrator
            .store()
            .load_artifact_gate(&run.lab_run_id, "professor_discussion")
            .unwrap();
        assert_eq!(gate.required_artifact_type, "ProfessorPlan");
        assert!(gate.artifact_id.is_none());
        let artifacts = orchestrator
            .store()
            .list_stage_artifacts(&run.lab_run_id)
            .unwrap();
        let cycle_summary = artifacts
            .iter()
            .find_map(|artifact| match artifact {
                StageArtifact::CycleSummary(summary) => Some(summary),
                _ => None,
            })
            .expect("cycle summary");
        assert_eq!(
            cycle_summary.validation_status.as_deref(),
            Some("read_only_runtime_summary")
        );
        assert!(cycle_summary
            .evidence_refs
            .iter()
            .any(|item| item.starts_with("artifact:artifact_professorreview_")));
        assert!(cycle_summary
            .evidence_refs
            .iter()
            .any(|item| item.starts_with("artifact:artifact_postdocintegrationsummary_")));
    }

    #[test]
    fn final_user_report_closeout_derives_status_from_professor_gate() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        for _ in 0..5 {
            orchestrator.tick_latest().unwrap();
        }

        let closed = orchestrator
            .closeout_latest_from_user_report("final report shown to user")
            .unwrap();

        assert_eq!(closed.lab_run_id, run.lab_run_id);
        assert_eq!(closed.status, LabRunStatus::Completed);
        assert_eq!(
            closed.closeout_status,
            Some(LabCloseoutStatus::CompletedNotVerified)
        );
        assert!(!closed.needs_user);
        assert!(!orchestrator
            .store()
            .root()
            .join("active_lease.json")
            .exists());
    }
}
