//! Graduate runtime verification and workspace evidence helpers.

use super::artifact_factory::clean_string_vec;
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn validate_changed_files_within_scope(
    allowed_scope: &[String],
    changed_files: &[String],
) -> anyhow::Result<()> {
    crate::lab::path_scope::changed_files_within_scope(allowed_scope, changed_files).map_err(
        |err| {
            anyhow!(
                "graduate result changed file is outside allowed_scope or invalid: {}; allowed_scope=({})",
                err,
                allowed_scope.join(", ")
            )
        },
    )
}

pub(super) fn durable_graduate_task_is_completed(
    context: &ToolContext,
    task: &GraduateTask,
) -> bool {
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
pub(super) struct GraduateRuntimeEvidence {
    pub(super) changed_files: Vec<String>,
    pub(super) validation_attempts: Vec<String>,
    pub(super) evidence_refs: Vec<String>,
    pub(super) provenance: LabEvidenceProvenance,
}

pub(super) fn runtime_verify_graduate_task_result(
    task: &GraduateTask,
    context: &ToolContext,
    agent_id: Option<&str>,
    agent_task_id: &str,
    dispatch_id: Option<&str>,
    parent_changed_files: &[String],
    provider_policy: &crate::lab::provider_certification::LabGraduateProviderExecutionPolicy,
) -> anyhow::Result<GraduateRuntimeEvidence> {
    let (verification_root, evidence_refs) = resolve_graduate_verification_root(
        task,
        context,
        agent_id,
        agent_task_id,
        provider_policy,
    )?;
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
    let store = LabStore::for_project(&context.working_dir);
    let mut provenance = graduate_runtime_provenance(
        task,
        context,
        agent_task_id,
        dispatch_id,
        &verification_root,
    );
    let validation = run_required_validation_commands(
        &verification_root,
        &task.required_validation,
        Some((&store, &provenance)),
    )?;
    provenance.validation_event_ids = validation.event_ids.clone();
    provenance.verified_at = Some(Utc::now());

    Ok(GraduateRuntimeEvidence {
        changed_files,
        validation_attempts: validation.attempts,
        evidence_refs,
        provenance,
    })
}

pub(super) fn resolve_graduate_verification_root(
    task: &GraduateTask,
    context: &ToolContext,
    agent_id: Option<&str>,
    agent_task_id: &str,
    provider_policy: &crate::lab::provider_certification::LabGraduateProviderExecutionPolicy,
) -> anyhow::Result<(PathBuf, Vec<String>)> {
    let agent_worktree = agent_id
        .and_then(|agent_id| agent_worktree_path(context, agent_id))
        .or_else(|| agent_worktree_path(context, agent_task_id));
    if provider_policy.isolated_worktree_required {
        let Some(worktree) = agent_worktree else {
            record_graduate_isolation_event(
                task,
                context,
                agent_id,
                agent_task_id,
                "lab_graduate_isolation_missing",
                "no isolated_worktree.path found in durable agent task state",
                None,
            );
            return Err(anyhow!(
                "graduate runtime verification requires isolated worktree proof for provider policy {}; no isolated_worktree.path found",
                provider_policy.certification.as_str()
            ));
        };
        if same_filesystem_path(&worktree, &context.working_dir) {
            record_graduate_isolation_event(
                task,
                context,
                agent_id,
                agent_task_id,
                "lab_graduate_isolation_missing",
                "isolated_worktree.path resolves to the parent workspace",
                Some(&worktree),
            );
            return Err(anyhow!(
                "graduate runtime verification requires a distinct isolated worktree; {} resolves to the parent workspace",
                worktree.display()
            ));
        }
        if !worktree.exists() {
            record_graduate_isolation_event(
                task,
                context,
                agent_id,
                agent_task_id,
                "lab_graduate_isolation_missing",
                "isolated_worktree.path does not exist",
                Some(&worktree),
            );
            return Err(anyhow!(
                "graduate runtime verification isolated worktree does not exist: {}",
                worktree.display()
            ));
        }
        record_graduate_isolation_event(
            task,
            context,
            agent_id,
            agent_task_id,
            "lab_graduate_isolation_verified",
            "isolated worktree proof accepted for graduate runtime verification",
            Some(&worktree),
        );
        return Ok((
            worktree,
            vec!["runtime_isolation:isolated_worktree".to_string()],
        ));
    }
    Ok((
        agent_worktree.unwrap_or_else(|| context.working_dir.clone()),
        Vec::new(),
    ))
}

fn record_graduate_isolation_event(
    task: &GraduateTask,
    context: &ToolContext,
    agent_id: Option<&str>,
    agent_task_id: &str,
    event_type: &str,
    reason: &str,
    worktree: Option<&Path>,
) {
    let store = LabStore::for_project(&context.working_dir);
    let _ = store.record_run_event(
        &task.lab_run_id,
        event_type,
        serde_json::json!({
            "task_id": task.task_id,
            "agent_id": agent_id,
            "agent_task_id": agent_task_id,
            "reason": reason,
            "worktree_path": worktree.map(|path| path.display().to_string()),
        }),
    );
}

pub(super) fn agent_worktree_path(context: &ToolContext, agent_id: &str) -> Option<PathBuf> {
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

pub(super) fn same_filesystem_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(super) fn current_git_changed_paths(
    worktree_root: &Path,
    target_root: Option<&Path>,
) -> Vec<String> {
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

pub(super) fn git_stdout(cwd: &Path, args: &[&str]) -> Option<String> {
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

pub(super) fn run_required_validation_commands(
    cwd: &Path,
    commands: &[String],
    lab_event_sink: Option<(&LabStore, &LabEvidenceProvenance)>,
) -> anyhow::Result<crate::lab::validation::LabValidationRunEvidence> {
    if let Some((store, provenance)) = lab_event_sink {
        crate::lab::validation::run_lab_validation_commands_for_lab_with_provenance(
            cwd, commands, store, provenance,
        )
    } else {
        Ok(crate::lab::validation::LabValidationRunEvidence {
            attempts: crate::lab::validation::run_lab_validation_commands(cwd, commands)?,
            event_ids: Vec::new(),
        })
    }
}

pub(super) fn graduate_runtime_provenance(
    task: &GraduateTask,
    context: &ToolContext,
    agent_task_id: &str,
    dispatch_id: Option<&str>,
    verification_root: &Path,
) -> LabEvidenceProvenance {
    let base_commit = git_stdout(&context.working_dir, &["rev-parse", "HEAD"]);
    let head_commit = git_stdout(verification_root, &["rev-parse", "HEAD"]);
    LabEvidenceProvenance {
        lab_run_id: Some(task.lab_run_id.clone()),
        cycle_id: task.cycle_id.clone(),
        source_postdoc_plan_artifact_id: task.source_postdoc_plan_artifact_id.clone(),
        graduate_task_id: Some(task.task_id.clone()),
        dispatch_id: dispatch_id.map(str::to_string),
        agent_task_id: Some(agent_task_id.to_string()),
        graduate_result_artifact_id: None,
        verification_root: Some(verification_root.display().to_string()),
        worktree_base_commit: base_commit,
        worktree_head_commit: head_commit,
        worktree_diff_hash: git_diff_hash_for_worktree(verification_root),
        validation_event_ids: Vec::new(),
        verified_at: None,
    }
}

fn git_diff_hash_for_worktree(worktree_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["diff", "--no-ext-diff"])
        .current_dir(worktree_root)
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(&output.stdout);
    Some(format!("{:x}", hasher.finalize()))
}

pub(super) fn workspace_change_snapshot(project_root: &Path) -> BTreeMap<String, String> {
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

pub(super) fn parse_git_status_path(line: &str) -> Option<String> {
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

pub(super) fn workspace_path_fingerprint(project_root: &Path, path: &str) -> String {
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

pub(super) fn changed_paths_between(
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

pub(super) fn is_internal_lab_runtime_path(path: &str) -> bool {
    crate::lab::path_scope::is_internal_lab_runtime_path(path)
}
