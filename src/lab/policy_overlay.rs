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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LabRunActionSource {
    ModelTool,
    RuntimeMaintenance,
}

impl LabRunActionSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::ModelTool => "model_tool",
            Self::RuntimeMaintenance => "runtime_maintenance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabRunPolicyActivation {
    ActiveLabMode,
    PausedProtection,
    NeedsUserProtection,
    BlockedProtection,
    Inactive,
}

impl LabRunPolicyActivation {
    fn as_str(self) -> &'static str {
        match self {
            Self::ActiveLabMode => "active_lab_mode",
            Self::PausedProtection => "paused_protection",
            Self::NeedsUserProtection => "needs_user_protection",
            Self::BlockedProtection => "blocked_protection",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabRunPolicyReview {
    pub applies: bool,
    pub allowed: bool,
    pub role: Option<String>,
    pub stage: Option<String>,
    pub status: Option<String>,
    pub activation: String,
    pub activation_reason: String,
    pub action_source: String,
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
            status: None,
            activation: "inactive".to_string(),
            activation_reason: reason.into(),
            action_source: LabRunActionSource::ModelTool.as_str().to_string(),
            action_family: "none".to_string(),
            reason: "LabRun policy overlay is not active for this action".to_string(),
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
    review_labrun_tool_action_with_source(
        project_root,
        tool_name,
        read_only,
        input_paths,
        LabRunActionSource::ModelTool,
    )
}

pub(crate) fn review_labrun_tool_action_with_source(
    project_root: &Path,
    tool_name: &str,
    read_only: Option<bool>,
    input_paths: &[String],
    action_source: LabRunActionSource,
) -> LabRunPolicyReview {
    let mut review = LabRunPolicyReview::not_applicable("no active LabRun");
    let store = LabStore::for_project(project_root);
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return review,
        Err(err) => {
            review.activation_reason = format!("failed to read LabRun policy state: {err}");
            return review;
        }
    };
    let activation = labrun_policy_activation_for_status(run.status);
    let activation_reason = activation_reason_for_status(run.status);
    if !activation_applies(activation) {
        return inactive_review_for_run(&run, activation, activation_reason, action_source);
    }
    let action_family = action_family_for_tool(tool_name, read_only);
    let mut normalized_paths = Vec::new();
    for path in input_paths {
        let normalized = match action_source {
            LabRunActionSource::ModelTool => normalize_action_path(project_root, path),
            LabRunActionSource::RuntimeMaintenance => {
                normalize_runtime_maintenance_path(project_root, path)
            }
        };
        match normalized {
            Ok(path) => normalized_paths.push(path),
            Err(err) => {
                return LabRunPolicyReview {
                    applies: true,
                    allowed: false,
                    role: Some(format!("{:?}", run.internal_owner)),
                    stage: Some(run.current_stage),
                    status: Some(format!("{:?}", run.status)),
                    activation: activation.as_str().to_string(),
                    activation_reason: activation_reason.to_string(),
                    action_source: action_source.as_str().to_string(),
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
            status: Some(format!("{:?}", run.status)),
            activation: activation.as_str().to_string(),
            activation_reason: activation_reason.to_string(),
            action_source: action_source.as_str().to_string(),
            action_family,
            reason: "read-only action allowed for current LabRun role".to_string(),
            paths: normalized_paths,
            allowed_scope: Vec::new(),
        };
    }

    match run.internal_owner {
        LabRole::Runtime => review_runtime_mutation(
            &run,
            activation,
            activation_reason,
            action_source,
            action_family,
            normalized_paths,
        ),
        LabRole::Professor | LabRole::Postdoc => LabRunPolicyReview {
            applies: true,
            allowed: false,
            role: Some(format!("{:?}", run.internal_owner)),
            stage: Some(run.current_stage),
            status: Some(format!("{:?}", run.status)),
            activation: activation.as_str().to_string(),
            activation_reason: activation_reason.to_string(),
            action_source: action_source.as_str().to_string(),
            action_family,
            reason: format!(
                "{:?} LabRun role cannot mutate project files through normal tool actions",
                run.internal_owner
            ),
            paths: normalized_paths,
            allowed_scope: Vec::new(),
        },
        LabRole::Graduate => review_graduate_mutation(
            &store,
            &run,
            activation,
            activation_reason,
            action_source,
            action_family,
            normalized_paths,
        ),
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
            "status": review.status,
            "activation": review.activation,
            "activation_reason": review.activation_reason,
            "action_source": review.action_source,
            "action_family": review.action_family,
            "paths": review.paths,
            "allowed_scope": review.allowed_scope,
            "reason": review.reason,
        }),
    )
}

fn inactive_review_for_run(
    run: &crate::lab::model::LabRun,
    activation: LabRunPolicyActivation,
    activation_reason: &'static str,
    action_source: LabRunActionSource,
) -> LabRunPolicyReview {
    LabRunPolicyReview {
        applies: false,
        allowed: true,
        role: Some(format!("{:?}", run.internal_owner)),
        stage: Some(run.current_stage.clone()),
        status: Some(format!("{:?}", run.status)),
        activation: activation.as_str().to_string(),
        activation_reason: activation_reason.to_string(),
        action_source: action_source.as_str().to_string(),
        action_family: "none".to_string(),
        reason: "LabRun policy overlay is inactive for this run status".to_string(),
        paths: Vec::new(),
        allowed_scope: Vec::new(),
    }
}

fn review_runtime_mutation(
    run: &crate::lab::model::LabRun,
    activation: LabRunPolicyActivation,
    activation_reason: &'static str,
    action_source: LabRunActionSource,
    action_family: String,
    normalized_paths: Vec<String>,
) -> LabRunPolicyReview {
    let maintenance_paths_allowed = action_source == LabRunActionSource::RuntimeMaintenance
        && !normalized_paths.is_empty()
        && normalized_paths
            .iter()
            .all(|path| path == ".priority-agent/lab" || path.starts_with(".priority-agent/lab/"));
    if maintenance_paths_allowed {
        return LabRunPolicyReview {
            applies: true,
            allowed: true,
            role: Some("Runtime".to_string()),
            stage: Some(run.current_stage.clone()),
            status: Some(format!("{:?}", run.status)),
            activation: activation.as_str().to_string(),
            activation_reason: activation_reason.to_string(),
            action_source: action_source.as_str().to_string(),
            action_family,
            reason: "runtime maintenance mutation is limited to .priority-agent/lab".to_string(),
            paths: normalized_paths,
            allowed_scope: vec![".priority-agent/lab".to_string()],
        };
    }
    LabRunPolicyReview {
        applies: true,
        allowed: false,
        role: Some("Runtime".to_string()),
        stage: Some(run.current_stage.clone()),
        status: Some(format!("{:?}", run.status)),
        activation: activation.as_str().to_string(),
        activation_reason: activation_reason.to_string(),
        action_source: action_source.as_str().to_string(),
        action_family,
        reason: "Runtime owner does not grant model mutation permission; use internal LabRun maintenance or a scoped graduate task".to_string(),
        paths: normalized_paths,
        allowed_scope: vec![".priority-agent/lab".to_string()],
    }
}

fn review_graduate_mutation(
    store: &LabStore,
    run: &crate::lab::model::LabRun,
    activation: LabRunPolicyActivation,
    activation_reason: &'static str,
    action_source: LabRunActionSource,
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
            status: Some(format!("{:?}", run.status)),
            activation: activation.as_str().to_string(),
            activation_reason: activation_reason.to_string(),
            action_source: action_source.as_str().to_string(),
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
            status: Some(format!("{:?}", run.status)),
            activation: activation.as_str().to_string(),
            activation_reason: activation_reason.to_string(),
            action_source: action_source.as_str().to_string(),
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
            status: Some(format!("{:?}", run.status)),
            activation: activation.as_str().to_string(),
            activation_reason: activation_reason.to_string(),
            action_source: action_source.as_str().to_string(),
            action_family,
            reason: format!("graduate mutation outside allowed_scope: {err}"),
            paths: normalized_paths,
            allowed_scope,
        },
    }
}

fn labrun_policy_activation_for_status(status: LabRunStatus) -> LabRunPolicyActivation {
    match status {
        LabRunStatus::Active => LabRunPolicyActivation::ActiveLabMode,
        LabRunStatus::Paused | LabRunStatus::PausedShutdown => {
            LabRunPolicyActivation::PausedProtection
        }
        LabRunStatus::NeedsUser => LabRunPolicyActivation::NeedsUserProtection,
        LabRunStatus::Blocked => LabRunPolicyActivation::BlockedProtection,
        LabRunStatus::Created
        | LabRunStatus::Completed
        | LabRunStatus::Failed
        | LabRunStatus::Cancelled => LabRunPolicyActivation::Inactive,
    }
}

fn activation_applies(activation: LabRunPolicyActivation) -> bool {
    matches!(activation, LabRunPolicyActivation::ActiveLabMode)
}

fn activation_reason_for_status(status: LabRunStatus) -> &'static str {
    match status {
        LabRunStatus::Active => "active LabRun role/stage policy applies",
        LabRunStatus::Paused | LabRunStatus::PausedShutdown => {
            "paused LabRun does not hard-block ordinary tool actions"
        }
        LabRunStatus::NeedsUser => {
            "needs-user LabRun requires explicit resume or recovery before overlay enforcement"
        }
        LabRunStatus::Blocked => {
            "blocked LabRun requires explicit unblock or recovery before overlay enforcement"
        }
        LabRunStatus::Created => "created LabRun is not active yet",
        LabRunStatus::Completed | LabRunStatus::Failed | LabRunStatus::Cancelled => {
            "terminal LabRun does not apply policy overlay"
        }
    }
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

fn normalize_runtime_maintenance_path(project_root: &Path, path: &str) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("LabRun policy received an empty runtime maintenance path".to_string());
    }
    let path_buf = PathBuf::from(trimmed);
    let relative = if path_buf.is_absolute() {
        path_buf
            .strip_prefix(project_root)
            .map_err(|_| format!("runtime maintenance path is outside workspace: {trimmed}"))?
            .to_string_lossy()
            .to_string()
    } else {
        trimmed.to_string()
    };
    let normalized = normalize_basic_relative_path(&relative)?;
    if normalized == ".priority-agent/lab" || normalized.starts_with(".priority-agent/lab/") {
        Ok(normalized)
    } else {
        Err(format!(
            "runtime maintenance mutation must stay under .priority-agent/lab: {trimmed}"
        ))
    }
}

fn normalize_basic_relative_path(path: &str) -> Result<String, String> {
    let original = path;
    let mut path = path.trim().replace('\\', "/");
    while let Some(stripped) = path.strip_prefix("./") {
        path = stripped.to_string();
    }
    while path.ends_with('/') {
        path.pop();
    }
    if path.is_empty() || path == "." {
        return Err(format!(
            "invalid runtime maintenance path '{original}': empty path"
        ));
    }
    if path.starts_with('/') || has_windows_drive_prefix(&path) {
        return Err(format!(
            "invalid runtime maintenance path '{original}': absolute paths are not allowed"
        ));
    }
    let mut parts = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" => {
                return Err(format!(
                    "invalid runtime maintenance path '{original}': empty path segment"
                ))
            }
            "." => {}
            ".." => {
                return Err(format!(
                    "invalid runtime maintenance path '{original}': parent traversal is not allowed"
                ))
            }
            value => parts.push(value),
        }
    }
    Ok(parts.join("/"))
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::model::{LabRole, LabRunStatus};
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

    #[test]
    fn paused_and_needs_user_runs_do_not_hard_block_ordinary_tools() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_plan".to_string();
        saved.internal_owner = LabRole::Postdoc;
        saved.status = LabRunStatus::Paused;
        orchestrator.store().save_run(&saved).unwrap();

        let paused = review_labrun_tool_action(
            temp.path(),
            "file_edit",
            Some(false),
            &["src/lab/model.rs".to_string()],
        );

        assert!(!paused.applies);
        assert!(paused.allowed);
        assert_eq!(paused.activation, "paused_protection");
        assert!(paused.activation_reason.contains("paused LabRun"));

        saved.status = LabRunStatus::NeedsUser;
        orchestrator.store().save_run(&saved).unwrap();
        let needs_user = review_labrun_tool_action(
            temp.path(),
            "file_edit",
            Some(false),
            &["src/lab/model.rs".to_string()],
        );

        assert!(!needs_user.applies);
        assert!(needs_user.allowed);
        assert_eq!(needs_user.activation, "needs_user_protection");
        assert!(needs_user.activation_reason.contains("requires explicit"));
    }

    #[test]
    fn runtime_owner_does_not_grant_model_tool_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "runtime_maintenance".to_string();
        saved.internal_owner = LabRole::Runtime;
        orchestrator.store().save_run(&saved).unwrap();

        let blocked = review_labrun_tool_action(
            temp.path(),
            "file_edit",
            Some(false),
            &["README.md".to_string()],
        );

        assert!(blocked.applies);
        assert!(!blocked.allowed);
        assert_eq!(blocked.role.as_deref(), Some("Runtime"));
        assert_eq!(blocked.action_source, "model_tool");
        assert!(blocked.reason.contains("does not grant model mutation"));

        let read = review_labrun_tool_action(
            temp.path(),
            "file_read",
            Some(true),
            &["README.md".to_string()],
        );
        assert!(read.applies);
        assert!(read.allowed);
    }

    #[test]
    fn runtime_maintenance_source_is_limited_to_lab_state_paths() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "runtime_maintenance".to_string();
        saved.internal_owner = LabRole::Runtime;
        orchestrator.store().save_run(&saved).unwrap();

        let allowed = review_labrun_tool_action_with_source(
            temp.path(),
            "file_write",
            Some(false),
            &[".priority-agent/lab/runs/run.json".to_string()],
            LabRunActionSource::RuntimeMaintenance,
        );
        assert!(allowed.allowed);
        assert_eq!(allowed.action_source, "runtime_maintenance");

        let blocked = review_labrun_tool_action_with_source(
            temp.path(),
            "file_write",
            Some(false),
            &["README.md".to_string()],
            LabRunActionSource::RuntimeMaintenance,
        );
        assert!(!blocked.allowed);
        assert!(blocked.reason.contains(".priority-agent/lab"));
    }
}
