//! LabRun execution binding passed from parent orchestration into child tools.
//!
//! The binding is the durable bridge between a GraduateTask dispatch and the
//! actual tool calls made by the child agent. It is intentionally carried in
//! `ToolContext` metadata so every action-review gate can enforce the same
//! scope without rediscovering LabRun state from an isolated worktree.

use crate::lab::model::GraduateTask;
use crate::lab::path_scope::{changed_files_within_scope, normalize_lab_relative_paths};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) const LAB_EXECUTION_BINDING_METADATA_KEY: &str = "lab_execution_binding";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabExecutionBinding {
    pub project_root: PathBuf,
    pub lab_run_id: String,
    pub cycle_id: Option<String>,
    pub source_postdoc_plan_artifact_id: Option<String>,
    pub graduate_task_id: String,
    pub dispatch_id: String,
    pub agent_task_id: String,
    pub allowed_scope: Vec<String>,
    pub verification_root: PathBuf,
    pub lab_state_version: Option<String>,
}

impl LabExecutionBinding {
    pub(crate) fn for_graduate_task(
        task: &GraduateTask,
        dispatch_id: impl Into<String>,
        agent_task_id: impl Into<String>,
        project_root: impl Into<PathBuf>,
        verification_root: impl Into<PathBuf>,
        lab_state_version: Option<String>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            project_root: project_root.into(),
            lab_run_id: task.lab_run_id.clone(),
            cycle_id: task.cycle_id.clone(),
            source_postdoc_plan_artifact_id: task.source_postdoc_plan_artifact_id.clone(),
            graduate_task_id: task.task_id.clone(),
            dispatch_id: dispatch_id.into(),
            agent_task_id: agent_task_id.into(),
            allowed_scope: normalize_lab_relative_paths(&task.allowed_scope)?,
            verification_root: verification_root.into(),
            lab_state_version,
        })
    }

    pub(crate) fn with_verification_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.verification_root = root.into();
        self
    }

    pub(crate) fn to_metadata_value(&self) -> anyhow::Result<String> {
        serde_json::to_string(self).map_err(Into::into)
    }

    pub(crate) fn insert_into_metadata(
        &self,
        metadata: &mut HashMap<String, String>,
    ) -> anyhow::Result<()> {
        metadata.insert(
            LAB_EXECUTION_BINDING_METADATA_KEY.to_string(),
            self.to_metadata_value()?,
        );
        metadata.insert("lab_mode_enabled".to_string(), "true".to_string());
        metadata.insert("active_lab_run_id".to_string(), self.lab_run_id.clone());
        metadata.insert(
            "active_graduate_task_id".to_string(),
            self.graduate_task_id.clone(),
        );
        metadata.insert("active_dispatch_id".to_string(), self.dispatch_id.clone());
        if let Some(version) = self.lab_state_version.as_ref() {
            metadata.insert("lab_state_version".to_string(), version.clone());
        }
        Ok(())
    }

    pub(crate) fn from_metadata(
        metadata: &HashMap<String, String>,
    ) -> Result<Option<Self>, String> {
        let Some(raw) = metadata.get(LAB_EXECUTION_BINDING_METADATA_KEY) else {
            return Ok(None);
        };
        serde_json::from_str::<Self>(raw)
            .map(Some)
            .map_err(|err| format!("invalid LabRun execution binding metadata: {err}"))
    }

    pub(crate) fn requires_scope_enforcement(&self) -> bool {
        !self.graduate_task_id.trim().is_empty()
            && !self.dispatch_id.trim().is_empty()
            && !self.allowed_scope.is_empty()
    }

    pub(crate) fn validate_paths_within_scope<I, S>(&self, paths: I) -> Result<(), String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if !self.requires_scope_enforcement() {
            return Err("LabRun Graduate execution binding is incomplete".to_string());
        }
        let paths = paths
            .into_iter()
            .map(|path| self.normalize_action_path_for_scope(path.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        changed_files_within_scope(&self.allowed_scope, &paths).map_err(|err| err.to_string())
    }

    fn normalize_action_path_for_scope(&self, path: &str) -> Result<String, String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err("LabRun execution binding received an empty action path".to_string());
        }
        let path_buf = PathBuf::from(trimmed);
        let relative = if path_buf.is_absolute() {
            if let Ok(relative) = path_buf.strip_prefix(&self.verification_root) {
                relative.to_string_lossy().to_string()
            } else if let Ok(relative) = path_buf.strip_prefix(&self.project_root) {
                relative.to_string_lossy().to_string()
            } else {
                return Err(format!(
                    "action path is outside LabRun execution root: {trimmed}"
                ));
            }
        } else {
            trimmed.to_string()
        };
        crate::lab::path_scope::normalize_lab_relative_path(&relative)
            .map_err(|err| err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::model::{LabRole, LabTaskStatus, LAB_SCHEMA_VERSION};
    use chrono::Utc;

    fn task() -> GraduateTask {
        let now = Utc::now();
        GraduateTask {
            schema_version: LAB_SCHEMA_VERSION,
            task_id: "gradtask_test".to_string(),
            lab_run_id: "labrun_test".to_string(),
            created_at: now,
            updated_at: now,
            created_by: LabRole::Postdoc,
            assigned_role: LabRole::Graduate,
            status: LabTaskStatus::Queued,
            title: "Implement".to_string(),
            instructions: "Change src/lab only.".to_string(),
            allowed_scope: vec!["src/lab".to_string()],
            required_validation: vec!["cargo check -q".to_string()],
            evidence_ids: Vec::new(),
            result_artifact_id: None,
            blocker: None,
            cycle_id: Some("1".to_string()),
            source_postdoc_plan_artifact_id: Some("artifact_plan".to_string()),
        }
    }

    #[test]
    fn binding_round_trips_through_metadata() {
        let binding = LabExecutionBinding::for_graduate_task(
            &task(),
            "dispatch_test",
            "agent_task",
            "/project",
            "/project/.worktree",
            Some("state_v1".to_string()),
        )
        .unwrap();
        let mut metadata = HashMap::new();
        binding.insert_into_metadata(&mut metadata).unwrap();

        let parsed = LabExecutionBinding::from_metadata(&metadata)
            .unwrap()
            .unwrap();

        assert_eq!(parsed.lab_run_id, "labrun_test");
        assert_eq!(parsed.graduate_task_id, "gradtask_test");
        assert_eq!(parsed.dispatch_id, "dispatch_test");
        assert_eq!(parsed.allowed_scope, vec!["src/lab".to_string()]);
        assert_eq!(metadata["active_lab_run_id"], "labrun_test");
        assert_eq!(metadata["active_graduate_task_id"], "gradtask_test");
        assert_eq!(metadata["active_dispatch_id"], "dispatch_test");
    }

    #[test]
    fn binding_scope_uses_lab_path_normalization() {
        let binding = LabExecutionBinding::for_graduate_task(
            &task(),
            "dispatch_test",
            "agent_task",
            "/project",
            "/project/.worktree",
            None,
        )
        .unwrap();

        binding
            .validate_paths_within_scope(["src/lab/mod.rs"])
            .unwrap();
        assert!(binding.validate_paths_within_scope(["README.md"]).is_err());
        assert!(binding.validate_paths_within_scope(["../escape"]).is_err());
    }
}
