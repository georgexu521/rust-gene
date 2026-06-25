//! Deterministic LabRun role/stage policy overlay.
//!
//! Workflow gates decide when LabRun can advance. This overlay is narrower: it
//! screens normal tool actions so professor/postdoc turns cannot mutate project
//! files and graduate turns can mutate only within the current task scope.

use crate::lab::model::{LabRole, LabRunStatus};
use crate::lab::path_scope::{changed_files_within_scope, normalize_lab_relative_path};
use crate::lab::store::LabStore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabRunPolicyReview {
    pub applies: bool,
    pub allowed: bool,
    pub role: Option<String>,
    pub stage: Option<String>,
    pub action_family: String,
    pub reason: String,
    pub paths: Vec<String>,
    pub allowed_scope: Vec<String>,
}

impl LabRunPolicyReview {
    pub fn not_applicable(reason: impl Into<String>) -> Self {
        Self {
            applies: false,
            allowed: true,
            role: None,
            stage: None,
            action_family: "none".to_string(),
            reason: reason.into(),
            paths: Vec::new(),
            allowed_scope: Vec::new(),
        }
    }
}

pub(crate) fn review_labrun_tool_action(
    project_root: &Path,
    tool_name: &str,
    read_only: Option<bool>,
    input_paths: &[String],
) -> LabRunPolicyReview {
    let mut review = LabRunPolicyReview::not_applicable("no active LabRun");
    let store = LabStore::for_project(project_root);
    let run = match store.latest_run() {
        Ok(Some(run)) if labrun_policy_applies_to_status(run.status) => run,
        Ok(Some(_)) => return LabRunPolicyReview::not_applicable("latest LabRun is terminal"),
        Ok(None) => return review,
        Err(err) => {
            review.reason = format!("failed to read LabRun policy state: {err}");
            return review;
        }
    };
    let action_family = action_family_for_tool(tool_name, read_only);
    let mut normalized_paths = Vec::new();
    for path in input_paths {
        match normalize_action_path(project_root, path) {
            Ok(path) => normalized_paths.push(path),
            Err(err) => {
                return LabRunPolicyReview {
                    applies: true,
                    allowed: false,
                    role: Some(format!("{:?}", run.internal_owner)),
                    stage: Some(run.current_stage),
                    action_family,
                    reason: err,
                    paths: input_paths.to_vec(),
                    allowed_scope: Vec::new(),
                };
            }
        }
    }
    normalized_paths.sort();
    normalized_paths.dedup();

    if action_family == "read" {
        return LabRunPolicyReview {
            applies: true,
            allowed: true,
            role: Some(format!("{:?}", run.internal_owner)),
            stage: Some(run.current_stage),
            action_family,
            reason: "read-only action allowed for current LabRun role".to_string(),
            paths: normalized_paths,
            allowed_scope: Vec::new(),
        };
    }

    match run.internal_owner {
        LabRole::Runtime => LabRunPolicyReview {
            applies: true,
            allowed: true,
            role: Some("Runtime".to_string()),
            stage: Some(run.current_stage),
            action_family,
            reason: "runtime-owned LabRun maintenance action allowed".to_string(),
            paths: normalized_paths,
            allowed_scope: Vec::new(),
        },
        LabRole::Professor | LabRole::Postdoc => LabRunPolicyReview {
            applies: true,
            allowed: false,
            role: Some(format!("{:?}", run.internal_owner)),
            stage: Some(run.current_stage),
            action_family,
            reason: format!(
                "{:?} LabRun role cannot mutate project files through normal tool actions",
                run.internal_owner
            ),
            paths: normalized_paths,
            allowed_scope: Vec::new(),
        },
        LabRole::Graduate => {
            review_graduate_mutation(&store, &run, action_family, normalized_paths)
        }
    }
}

pub(crate) fn record_labrun_policy_event(
    project_root: &Path,
    review: &LabRunPolicyReview,
) -> anyhow::Result<()> {
    if !review.applies {
        return Ok(());
    }
    let store = LabStore::for_project(project_root);
    let Some(run) = store.latest_run()? else {
        return Ok(());
    };
    let event_type = if review.allowed {
        "labrun_policy_allowed"
    } else {
        "labrun_policy_blocked"
    };
    store.record_run_event(
        &run.lab_run_id,
        event_type,
        serde_json::json!({
            "role": review.role,
            "stage": review.stage,
            "action_family": review.action_family,
            "paths": review.paths,
            "allowed_scope": review.allowed_scope,
            "reason": review.reason,
        }),
    )
}

fn review_graduate_mutation(
    store: &LabStore,
    run: &crate::lab::model::LabRun,
    action_family: String,
    normalized_paths: Vec<String>,
) -> LabRunPolicyReview {
    let mut allowed_scope = Vec::new();
    for task_id in &run.open_task_ids {
        if let Ok(task) = store.load_graduate_task(&run.lab_run_id, task_id) {
            allowed_scope.extend(task.allowed_scope);
        }
    }
    if allowed_scope.is_empty() {
        if let Ok(tasks) = store.list_graduate_tasks(&run.lab_run_id) {
            for task in tasks {
                if task.status.is_open() {
                    allowed_scope.extend(task.allowed_scope);
                }
            }
        }
    }
    allowed_scope.sort();
    allowed_scope.dedup();

    if normalized_paths.is_empty() {
        return LabRunPolicyReview {
            applies: true,
            allowed: false,
            role: Some("Graduate".to_string()),
            stage: Some(run.current_stage.clone()),
            action_family,
            reason: "graduate mutation requires explicit scoped paths".to_string(),
            paths: normalized_paths,
            allowed_scope,
        };
    }
    match changed_files_within_scope(&allowed_scope, &normalized_paths) {
        Ok(()) => LabRunPolicyReview {
            applies: true,
            allowed: true,
            role: Some("Graduate".to_string()),
            stage: Some(run.current_stage.clone()),
            action_family,
            reason: "graduate mutation paths are inside allowed_scope".to_string(),
            paths: normalized_paths,
            allowed_scope,
        },
        Err(err) => LabRunPolicyReview {
            applies: true,
            allowed: false,
            role: Some("Graduate".to_string()),
            stage: Some(run.current_stage.clone()),
            action_family,
            reason: format!("graduate mutation outside allowed_scope: {err}"),
            paths: normalized_paths,
            allowed_scope,
        },
    }
}

fn labrun_policy_applies_to_status(status: LabRunStatus) -> bool {
    !matches!(
        status,
        LabRunStatus::Completed | LabRunStatus::Failed | LabRunStatus::Cancelled
    )
}

fn action_family_for_tool(tool_name: &str, read_only: Option<bool>) -> String {
    if read_only == Some(true) {
        return "read".to_string();
    }
    match tool_name {
        "bash" => "shell_mutation".to_string(),
        "file_write" | "file_edit" | "file_patch" => "file_mutation".to_string(),
        "mcp" | "mcp_tool" | "plugin" | "plugin_tool" => "external_mutation".to_string(),
        _ if read_only == Some(false) => "mutation".to_string(),
        _ => "unknown_mutation".to_string(),
    }
}

fn normalize_action_path(project_root: &Path, path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("LabRun policy received an empty action path".to_string());
    }
    let path_buf = PathBuf::from(trimmed);
    let relative = if path_buf.is_absolute() {
        path_buf
            .strip_prefix(project_root)
            .map_err(|_| format!("action path is outside workspace: {trimmed}"))?
            .to_string_lossy()
            .to_string()
    } else {
        trimmed.to_string()
    };
    normalize_lab_relative_path(&relative).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::model::LabRole;
    use crate::lab::orchestrator::LabOrchestrator;

    #[test]
    fn postdoc_mutation_is_blocked() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        let review = review_labrun_tool_action(
            temp.path(),
            "file_edit",
            Some(false),
            &["src/lab/model.rs".to_string()],
        );

        assert!(review.applies);
        assert!(!review.allowed);
        assert_eq!(review.role.as_deref(), Some("Professor"));
        assert!(review.reason.contains("cannot mutate"));

        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_plan".to_string();
        saved.internal_owner = LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();
        let review = review_labrun_tool_action(
            temp.path(),
            "file_edit",
            Some(false),
            &["src/lab/model.rs".to_string()],
        );
        assert_eq!(review.role.as_deref(), Some("Postdoc"));
        assert!(!review.allowed);
    }

    #[test]
    fn graduate_mutation_must_match_allowed_scope() {
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
                "Update lab model.",
                vec!["src/lab".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "graduate_work".to_string();
        saved.internal_owner = LabRole::Graduate;
        saved.open_task_ids = vec![task.task_id];
        orchestrator.store().save_run(&saved).unwrap();

        let allowed = review_labrun_tool_action(
            temp.path(),
            "file_edit",
            Some(false),
            &["src/lab/model.rs".to_string()],
        );
        assert!(allowed.allowed);
        let blocked = review_labrun_tool_action(
            temp.path(),
            "file_edit",
            Some(false),
            &["README.md".to_string()],
        );
        assert!(!blocked.allowed);
        assert!(blocked.reason.contains("outside allowed_scope"));
    }

    #[test]
    fn read_only_actions_are_allowed() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        let review = review_labrun_tool_action(
            temp.path(),
            "file_read",
            Some(true),
            &["src/lab/model.rs".to_string()],
        );

        assert!(review.applies);
        assert!(review.allowed);
        assert_eq!(review.action_family, "read");
    }

    #[test]
    fn unknown_read_only_annotation_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();

        let review = review_labrun_tool_action(
            temp.path(),
            "custom_tool",
            None,
            &["src/lab/model.rs".to_string()],
        );

        assert!(review.applies);
        assert!(!review.allowed);
        assert_eq!(review.action_family, "unknown_mutation");
    }
}
