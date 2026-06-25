//! Artifact persistence operations for `LabStore`.
//!
//! This module records artifact bodies, validation gates, markdown reports, and
//! evidence links. It is the artifact source of truth used by LabRun closeout
//! and desktop previews.

use super::*;

impl LabStore {
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
        let semantic_blockers =
            crate::lab::artifact_semantics::stage_artifact_semantic_blockers(&artifact);
        if !semantic_blockers.is_empty() {
            return Err(anyhow!(
                "artifact gate '{}' semantic validation failed: {}",
                stage,
                semantic_blockers.join("; ")
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
}
