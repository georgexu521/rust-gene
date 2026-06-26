//! Deterministic LabRun role/stage policy overlay.
//!
//! Workflow gates decide when LabRun can advance. This overlay is narrower: it
//! screens normal tool actions so professor/postdoc turns cannot mutate project
//! files and graduate turns can mutate only within the current task scope.

use crate::lab::model::{LabRole, LabRun, LabRunStatus, LabTaskStatus};
use crate::lab::path_scope::{changed_files_within_scope, normalize_lab_relative_path};
use crate::lab::store::LabStore;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
pub struct LabRunExecutionContext {
    pub lab_mode_enabled: bool,
    pub lab_run_id: Option<String>,
    pub lab_stage: Option<String>,
    pub lab_role: Option<LabRole>,
    pub lab_status: Option<LabRunStatus>,
    pub lab_state_version: Option<String>,
    pub active_graduate_task_id: Option<String>,
    pub active_dispatch_id: Option<String>,
}

impl LabRunExecutionContext {
    pub fn from_metadata(metadata: &HashMap<String, String>) -> Option<Self> {
        let lab_mode_enabled = metadata
            .get("lab_mode_enabled")
            .is_some_and(|value| matches!(value.as_str(), "true" | "1" | "yes"));
        let has_lab_marker = lab_mode_enabled
            || metadata.contains_key("active_lab_run_id")
            || metadata.contains_key("active_graduate_task_id")
            || metadata.contains_key("active_dispatch_id");
        has_lab_marker.then(|| Self {
            lab_mode_enabled,
            lab_run_id: metadata.get("active_lab_run_id").cloned(),
            lab_stage: metadata.get("lab_stage").cloned(),
            lab_role: None,
            lab_status: None,
            lab_state_version: metadata.get("lab_state_version").cloned(),
            active_graduate_task_id: metadata.get("active_graduate_task_id").cloned(),
            active_dispatch_id: metadata.get("active_dispatch_id").cloned(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabRunPolicyReview {
    pub lab_run_id: Option<String>,
    pub state_version: Option<String>,
    pub reviewed_stage: Option<String>,
    pub reviewed_owner: Option<String>,
    pub reviewed_status: Option<String>,
    pub reviewed_at: Option<String>,
    pub active_graduate_task_id: Option<String>,
    pub active_dispatch_id: Option<String>,
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
            lab_run_id: None,
            state_version: None,
            reviewed_stage: None,
            reviewed_owner: None,
            reviewed_status: None,
            reviewed_at: None,
            active_graduate_task_id: None,
            active_dispatch_id: None,
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

    fn for_run(input: LabRunPolicyReviewInput<'_>) -> Self {
        Self {
            lab_run_id: Some(input.run.lab_run_id.clone()),
            state_version: Some(labrun_state_version(input.run)),
            reviewed_stage: Some(input.run.current_stage.clone()),
            reviewed_owner: Some(format!("{:?}", input.run.internal_owner)),
            reviewed_status: Some(format!("{:?}", input.run.status)),
            reviewed_at: Some(Utc::now().to_rfc3339()),
            active_graduate_task_id: input
                .context
                .and_then(|context| context.active_graduate_task_id.clone()),
            active_dispatch_id: input
                .context
                .and_then(|context| context.active_dispatch_id.clone()),
            applies: input.applies,
            allowed: input.allowed,
            role: Some(format!("{:?}", input.run.internal_owner)),
            stage: Some(input.run.current_stage.clone()),
            status: Some(format!("{:?}", input.run.status)),
            activation: input.activation.as_str().to_string(),
            activation_reason: input.activation_reason.to_string(),
            action_source: input.action_source.as_str().to_string(),
            action_family: input.action_family,
            reason: input.reason,
            paths: input.paths,
            allowed_scope: input.allowed_scope,
        }
    }
}

struct LabRunPolicyReviewInput<'a> {
    run: &'a LabRun,
    context: Option<&'a LabRunExecutionContext>,
    applies: bool,
    allowed: bool,
    activation: LabRunPolicyActivation,
    activation_reason: &'static str,
    action_source: LabRunActionSource,
    action_family: String,
    reason: String,
    paths: Vec<String>,
    allowed_scope: Vec<String>,
}

#[allow(dead_code)]
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

pub(crate) fn review_labrun_tool_action_with_context(
    project_root: &Path,
    tool_name: &str,
    read_only: Option<bool>,
    input_paths: &[String],
    context: Option<&LabRunExecutionContext>,
) -> LabRunPolicyReview {
    review_labrun_tool_action_with_source_and_context(
        project_root,
        tool_name,
        read_only,
        input_paths,
        LabRunActionSource::ModelTool,
        context,
    )
}

#[allow(dead_code)]
pub(crate) fn review_labrun_tool_action_with_source(
    project_root: &Path,
    tool_name: &str,
    read_only: Option<bool>,
    input_paths: &[String],
    action_source: LabRunActionSource,
) -> LabRunPolicyReview {
    review_labrun_tool_action_with_source_and_context(
        project_root,
        tool_name,
        read_only,
        input_paths,
        action_source,
        None,
    )
}

pub(crate) fn review_labrun_tool_action_with_source_and_context(
    project_root: &Path,
    tool_name: &str,
    read_only: Option<bool>,
    input_paths: &[String],
    action_source: LabRunActionSource,
    context: Option<&LabRunExecutionContext>,
) -> LabRunPolicyReview {
    let mut review = LabRunPolicyReview::not_applicable("no active LabRun");
    let store = LabStore::for_project(project_root);
    let run_result =
        if let Some(lab_run_id) = context.and_then(|context| context.lab_run_id.as_deref()) {
            store.load_run(lab_run_id)
        } else {
            match store.latest_run() {
                Ok(Some(run)) => Ok(run),
                Ok(None) => return review,
                Err(err) => Err(err),
            }
        };
    let run = match run_result {
        Ok(run) => run,
        Err(err) => {
            review.activation_reason = format!("failed to read LabRun policy state: {err}");
            return review;
        }
    };
    let activation = labrun_policy_activation_for_status(run.status);
    let activation_reason = activation_reason_for_status(run.status);
    if !activation_applies(activation) {
        return inactive_review_for_run(
            &run,
            activation,
            activation_reason,
            action_source,
            context,
        );
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
                return LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
                    run: &run,
                    context,
                    applies: true,
                    allowed: false,
                    action_family,
                    reason: err,
                    paths: input_paths.to_vec(),
                    allowed_scope: Vec::new(),
                    activation,
                    activation_reason,
                    action_source,
                });
            }
        }
    }
    normalized_paths.sort();
    normalized_paths.dedup();

    if action_family == "read" {
        return LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
            run: &run,
            context,
            applies: true,
            allowed: true,
            action_family,
            reason: "read-only action allowed for current LabRun role".to_string(),
            paths: normalized_paths,
            allowed_scope: Vec::new(),
            activation,
            activation_reason,
            action_source,
        });
    }

    match run.internal_owner {
        LabRole::Runtime => review_runtime_mutation(
            &run,
            activation,
            activation_reason,
            action_source,
            action_family,
            normalized_paths,
            context,
        ),
        LabRole::Professor | LabRole::Postdoc => {
            LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
                run: &run,
                context,
                applies: true,
                allowed: false,
                action_family,
                reason: format!(
                    "{:?} LabRun role cannot mutate project files through normal tool actions",
                    run.internal_owner
                ),
                paths: normalized_paths,
                allowed_scope: Vec::new(),
                activation,
                activation_reason,
                action_source,
            })
        }
        LabRole::Graduate => review_graduate_mutation(
            &store,
            &run,
            activation,
            activation_reason,
            action_source,
            action_family,
            normalized_paths,
            context,
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
    let Some(lab_run_id) = review.lab_run_id.as_deref() else {
        return Ok(());
    };
    let event_type = if review.allowed {
        "labrun_policy_allowed"
    } else {
        "labrun_policy_blocked"
    };
    store.record_run_event(
        lab_run_id,
        event_type,
        serde_json::json!({
            "lab_run_id": review.lab_run_id,
            "state_version": review.state_version,
            "reviewed_stage": review.reviewed_stage,
            "reviewed_owner": review.reviewed_owner,
            "reviewed_status": review.reviewed_status,
            "reviewed_at": review.reviewed_at,
            "active_graduate_task_id": review.active_graduate_task_id,
            "active_dispatch_id": review.active_dispatch_id,
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

pub(crate) fn revalidate_labrun_policy_review(
    project_root: &Path,
    review: &LabRunPolicyReview,
) -> Result<(), String> {
    if !review.applies || !review.allowed || review.action_family == "read" {
        return Ok(());
    }
    let Some(lab_run_id) = review.lab_run_id.as_deref() else {
        return Ok(());
    };
    let Some(expected_version) = review.state_version.as_deref() else {
        return Err("labrun_policy_state_missing".to_string());
    };
    let store = LabStore::for_project(project_root);
    let run = store
        .load_run(lab_run_id)
        .map_err(|err| format!("labrun_policy_state_unavailable: {err}"))?;
    let current_version = labrun_state_version(&run);
    if current_version != expected_version {
        return Err(format!(
            "labrun_policy_state_changed: reviewed={} current={}",
            expected_version, current_version
        ));
    }
    Ok(())
}

fn inactive_review_for_run(
    run: &LabRun,
    activation: LabRunPolicyActivation,
    activation_reason: &'static str,
    action_source: LabRunActionSource,
    context: Option<&LabRunExecutionContext>,
) -> LabRunPolicyReview {
    LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
        run,
        context,
        applies: false,
        allowed: true,
        action_family: "none".to_string(),
        reason: "LabRun policy overlay is inactive for this run status".to_string(),
        paths: Vec::new(),
        allowed_scope: Vec::new(),
        activation,
        activation_reason,
        action_source,
    })
}

fn review_runtime_mutation(
    run: &LabRun,
    activation: LabRunPolicyActivation,
    activation_reason: &'static str,
    action_source: LabRunActionSource,
    action_family: String,
    normalized_paths: Vec<String>,
    context: Option<&LabRunExecutionContext>,
) -> LabRunPolicyReview {
    let maintenance_paths_allowed = action_source == LabRunActionSource::RuntimeMaintenance
        && !normalized_paths.is_empty()
        && normalized_paths
            .iter()
            .all(|path| path == ".priority-agent/lab" || path.starts_with(".priority-agent/lab/"));
    if maintenance_paths_allowed {
        return LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
            run,
            context,
            applies: true,
            allowed: true,
            action_family,
            reason: "runtime maintenance mutation is limited to .priority-agent/lab".to_string(),
            paths: normalized_paths,
            allowed_scope: vec![".priority-agent/lab".to_string()],
            activation,
            activation_reason,
            action_source,
        });
    }
    LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
        run,
        context,
        applies: true,
        allowed: false,
        action_family,
        reason: "Runtime owner does not grant model mutation permission; use internal LabRun maintenance or a scoped graduate task".to_string(),
        paths: normalized_paths,
        allowed_scope: vec![".priority-agent/lab".to_string()],
        activation,
        activation_reason,
        action_source,
    })
}

#[allow(clippy::too_many_arguments)]
fn review_graduate_mutation(
    store: &LabStore,
    run: &LabRun,
    activation: LabRunPolicyActivation,
    activation_reason: &'static str,
    action_source: LabRunActionSource,
    action_family: String,
    normalized_paths: Vec<String>,
    context: Option<&LabRunExecutionContext>,
) -> LabRunPolicyReview {
    let mut allowed_scope = Vec::new();
    let active_task_id = context
        .and_then(|context| context.active_graduate_task_id.as_deref())
        .or_else(|| (run.open_task_ids.len() == 1).then(|| run.open_task_ids[0].as_str()));
    let Some(active_task_id) = active_task_id else {
        return LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
            run,
            context,
            applies: true,
            allowed: false,
            activation,
            activation_reason,
            action_source,
            action_family,
            reason: "graduate mutation requires one explicit active graduate task; multiple or zero open tasks cannot be merged for scope".to_string(),
            paths: normalized_paths,
            allowed_scope,
        });
    };
    if let Ok(task) = store.load_graduate_task(&run.lab_run_id, active_task_id) {
        if task.status.is_open() || task.status == LabTaskStatus::Completed {
            allowed_scope.extend(task.allowed_scope);
        }
    } else {
        return LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
            run,
            context,
            applies: true,
            allowed: false,
            activation,
            activation_reason,
            action_source,
            action_family,
            reason: format!("active graduate task {active_task_id} was not found"),
            paths: normalized_paths,
            allowed_scope,
        });
    }
    allowed_scope.sort();
    allowed_scope.dedup();

    if normalized_paths.is_empty() {
        return LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
            run,
            context,
            applies: true,
            allowed: false,
            action_family,
            reason: "graduate mutation requires explicit scoped paths".to_string(),
            paths: normalized_paths,
            allowed_scope,
            activation,
            activation_reason,
            action_source,
        });
    }
    match changed_files_within_scope(&allowed_scope, &normalized_paths) {
        Ok(()) => LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
            run,
            context,
            applies: true,
            allowed: true,
            action_family,
            reason: format!(
                "graduate mutation paths are inside allowed_scope for active task {active_task_id}"
            ),
            paths: normalized_paths,
            allowed_scope,
            activation,
            activation_reason,
            action_source,
        }),
        Err(err) => LabRunPolicyReview::for_run(LabRunPolicyReviewInput {
            run,
            context,
            applies: true,
            allowed: false,
            action_family,
            reason: format!("graduate mutation outside allowed_scope: {err}"),
            paths: normalized_paths,
            allowed_scope,
            activation,
            activation_reason,
            action_source,
        }),
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

pub(crate) fn labrun_state_version(run: &LabRun) -> String {
    format!(
        "{}|{:?}|{}|{:?}|{}|{}",
        run.updated_at.to_rfc3339(),
        run.status,
        run.current_stage,
        run.internal_owner,
        run.cycle_count,
        run.open_task_ids.join(",")
    )
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
    fn graduate_policy_uses_single_active_task_scope() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task_a = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement API slice",
                "Update API files.",
                vec!["src/api".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let task_b = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement memory slice",
                "Update memory files.",
                vec!["src/memory".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "graduate_work".to_string();
        saved.internal_owner = LabRole::Graduate;
        saved.open_task_ids = vec![task_a.task_id.clone(), task_b.task_id.clone()];
        orchestrator.store().save_run(&saved).unwrap();
        let context = LabRunExecutionContext {
            lab_mode_enabled: true,
            lab_run_id: Some(run.lab_run_id.clone()),
            lab_stage: Some("graduate_work".to_string()),
            lab_role: Some(LabRole::Graduate),
            lab_status: Some(LabRunStatus::Active),
            lab_state_version: None,
            active_graduate_task_id: Some(task_a.task_id.clone()),
            active_dispatch_id: Some("dispatch_task_a".to_string()),
        };

        let allowed = review_labrun_tool_action_with_context(
            temp.path(),
            "file_edit",
            Some(false),
            &["src/api/routes.rs".to_string()],
            Some(&context),
        );
        assert!(allowed.allowed);
        assert_eq!(
            allowed.active_graduate_task_id.as_deref(),
            Some(task_a.task_id.as_str())
        );
        assert_eq!(allowed.allowed_scope, vec!["src/api".to_string()]);

        let denied_other_task_scope = review_labrun_tool_action_with_context(
            temp.path(),
            "file_edit",
            Some(false),
            &["src/memory/manager.rs".to_string()],
            Some(&context),
        );
        assert!(!denied_other_task_scope.allowed);
        assert_eq!(
            denied_other_task_scope.allowed_scope,
            vec!["src/api".to_string()]
        );
        assert!(denied_other_task_scope
            .reason
            .contains("outside allowed_scope"));

        let denied_without_active_task = review_labrun_tool_action(
            temp.path(),
            "file_edit",
            Some(false),
            &["src/api/routes.rs".to_string()],
        );
        assert!(!denied_without_active_task.allowed);
        assert!(denied_without_active_task
            .reason
            .contains("requires one explicit active graduate task"));
    }

    #[test]
    fn policy_review_revalidates_state_version_before_mutation() {
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
        saved.open_task_ids = vec![task.task_id.clone()];
        orchestrator.store().save_run(&saved).unwrap();

        let review = review_labrun_tool_action(
            temp.path(),
            "file_edit",
            Some(false),
            &["src/lab/model.rs".to_string()],
        );
        assert!(review.allowed);
        revalidate_labrun_policy_review(temp.path(), &review).unwrap();

        let mut changed = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        changed.open_task_ids.clear();
        orchestrator.store().save_run(&changed).unwrap();
        let err = revalidate_labrun_policy_review(temp.path(), &review)
            .expect_err("stale LabRun policy review should fail closed");
        assert!(err.contains("labrun_policy_state_changed"));
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
