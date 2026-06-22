//! Graduate task command handlers.
//!
//! These commands create, run, and inspect scoped graduate tasks. Execution must
//! preserve dispatch state, cleanup status, and durable evidence.

use super::*;

pub(super) async fn handle_task_run_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let task_id = args.trim();
    if task_id.is_empty() || task_id.split_whitespace().count() != 1 {
        return "Usage: /lab task run <task_id>".to_string();
    }

    let orchestrator = LabOrchestrator::for_project(project_root);
    match orchestrator
        .execute_graduate_task_latest_with_context(task_id, tool_context)
        .await
    {
        Ok(dispatch) => {
            let mut lines = vec![
                format!("Graduate task run dispatched: {}", dispatch.dispatch_id),
                format!("Task: {}", dispatch.task_id),
                format!("Status: {:?}", dispatch.status),
                format!("Envelope: {}", dispatch.envelope.envelope_id),
            ];
            if let Some(agent_id) = dispatch.agent_id.as_deref() {
                lines.push(format!("Agent: {agent_id}"));
            }
            if let Some(error) = dispatch.error.as_deref() {
                lines.push(format!("Error: {error}"));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to run graduate task: {err}"),
    }
}

pub(super) async fn handle_task_sync_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let task_id = args.trim();
    if task_id.is_empty() || task_id.split_whitespace().count() != 1 {
        return "Usage: /lab task sync <task_id>".to_string();
    }

    let orchestrator = LabOrchestrator::for_project(project_root);
    match orchestrator.sync_graduate_agent_task_latest_with_context(task_id, tool_context) {
        Ok(created) => format!(
            "Synced graduate durable subagent result: {}\nArtifact: {}\nReport: {}\nGate status: {}",
            created.artifact.artifact_id(),
            created.path.display(),
            created.report_path.display(),
            if created.gate.is_satisfied() {
                "satisfied"
            } else {
                "not_satisfied"
            }
        ),
        Err(err) => format!("Failed to sync graduate durable subagent result: {err}"),
    }
}

pub(super) async fn handle_task_worktree_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let (action, rest) = split_once(args);
    let (task_id, extra) = split_once(rest);
    if !matches!(action, "review" | "merge" | "cleanup") || task_id.is_empty() {
        return "Usage: /lab task worktree <review|merge|cleanup> <task_id> [force]".to_string();
    }

    let store = LabStore::for_project(project_root);
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found for graduate task worktree action.".to_string(),
        Err(err) => {
            return format!("Failed to read LabRun for graduate task worktree action: {err}")
        }
    };
    let dispatches = match store.list_graduate_dispatches(&run.lab_run_id) {
        Ok(dispatches) => dispatches,
        Err(err) => return format!("Failed to read graduate dispatches: {err}"),
    };
    let Some((dispatch, agent_ref_kind, agent_ref)) =
        dispatches.iter().rev().find_map(|dispatch| {
            if dispatch.task_id != task_id {
                return None;
            }
            if let Some(agent_id) = dispatch.agent_id.as_deref() {
                return Some((dispatch, "agent_id", agent_id.to_string()));
            }
            dispatch
                .agent_tool_params
                .get("task_id")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(|task_id| (dispatch, "task_id", task_id.to_string()))
        })
    else {
        return format!(
            "No graduate dispatch with agent_id or durable task_id found for task {task_id}."
        );
    };
    let worktree_action = match action {
        "review" => "agent_review",
        "merge" => "agent_merge",
        "cleanup" => "agent_cleanup",
        _ => unreachable!(),
    };
    let force = extra
        .split_whitespace()
        .any(|part| matches!(part, "force" | "force=true"));
    let mut worktree_params = serde_json::json!({
        "action": worktree_action,
        "force": force,
    });
    worktree_params[agent_ref_kind] = serde_json::json!(agent_ref);
    let result = WorktreeTool.execute(worktree_params, tool_context).await;
    let cleanup_status =
        graduate_cleanup_status_for_worktree_action(worktree_action, result.success);
    let cleanup_message = format_graduate_cleanup_message(action, result.success, &result);
    let mut persistence_error = None;
    if let Some(cleanup_status) = cleanup_status {
        if let Err(err) = store.update_graduate_dispatch_cleanup_status(
            &run.lab_run_id,
            &dispatch.dispatch_id,
            cleanup_status,
            Some(cleanup_message.clone()),
        ) {
            persistence_error = Some(format!("cleanup status: {err}"));
        }
    }
    if persistence_error.is_none() {
        if let Err(err) = store.record_run_event(
            &run.lab_run_id,
            "lab_graduate_worktree_action",
            serde_json::json!({
                "task_id": task_id,
                "dispatch_id": dispatch.dispatch_id,
                "agent_id": dispatch.agent_id,
                "agent_ref_kind": agent_ref_kind,
                "agent_ref": agent_ref.clone(),
                "action": worktree_action,
                "success": result.success,
                "error": result.error.clone(),
                "cleanup_status": cleanup_status.map(GraduateCleanupStatus::as_str),
                "cleanup_message": cleanup_message,
                "result_data": result.data.clone(),
                "result_content_preview": compact_message_line(&result.content, 600),
            }),
        ) {
            persistence_error = Some(format!("worktree action event: {err}"));
        }
    }
    if let Some(persistence_error) = persistence_error {
        return format!(
            "Lab graduate worktree {} failed for task {} via {} {}: failed to persist worktree action state: {}",
            action, task_id, agent_ref_kind, agent_ref, persistence_error
        );
    }
    if result.success {
        format!(
            "Lab graduate worktree {} succeeded for task {} via {} {}.\n{}",
            action, task_id, agent_ref_kind, agent_ref, result.content
        )
    } else {
        format!(
            "Lab graduate worktree {} failed for task {} via {} {}: {}",
            action,
            task_id,
            agent_ref_kind,
            agent_ref,
            result
                .error
                .as_deref()
                .filter(|value| !value.is_empty())
                .unwrap_or(result.content.as_str())
        )
    }
}

fn graduate_cleanup_status_for_worktree_action(
    worktree_action: &str,
    success: bool,
) -> Option<GraduateCleanupStatus> {
    match (worktree_action, success) {
        ("agent_cleanup", true) => Some(GraduateCleanupStatus::CleanupDone),
        ("agent_cleanup", false) => Some(GraduateCleanupStatus::CleanupBlocked),
        ("agent_review" | "agent_merge", _) => Some(GraduateCleanupStatus::CleanupPending),
        _ => None,
    }
}

fn format_graduate_cleanup_message(
    action: &str,
    success: bool,
    result: &crate::tools::ToolResult,
) -> String {
    if success {
        return format!("worktree {action} succeeded");
    }
    let detail = result
        .error
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(result.content.as_str());
    format!(
        "worktree {action} failed: {}",
        compact_message_line(detail, 400)
    )
}

pub(super) fn handle_task_command(
    project_root: &Path,
    orchestrator: &LabOrchestrator,
    store: &LabStore,
    subcommand: &str,
    args: &str,
) -> String {
    let trimmed = args.trim();
    if subcommand == "tasks" && trimmed.is_empty() {
        return list_graduate_tasks(store);
    }
    if trimmed.is_empty() || trimmed == "list" {
        return list_graduate_tasks(store);
    }

    let (action, rest) = split_once(trimmed);
    match action {
        "list" => list_graduate_tasks(store),
        "create" => {
            let run = match store.latest_run() {
                Ok(Some(run)) => run,
                Ok(None) => return "No LabRun found for graduate task.".to_string(),
                Err(err) => return format!("Failed to read LabRun for graduate task: {err}"),
            };
            let (title, allowed_scope, required_validation, instructions) =
                match parse_task_create(rest) {
                    Ok(parsed) => parsed,
                    Err(err) => return err,
                };
            match store.create_graduate_task(
                &run.lab_run_id,
                &title,
                &instructions,
                allowed_scope,
                required_validation,
            ) {
                Ok(task) => format!(
                    "Created graduate task: {}\nStatus: {:?}\nScope: {}\nValidation: {}",
                    task.task_id,
                    task.status,
                    format_list(&task.allowed_scope),
                    format_list(&task.required_validation)
                ),
                Err(err) => format!("Failed to create graduate task: {err}"),
            }
        }
        "envelope" => {
            let (task_id, extra) = split_once(rest);
            if task_id.is_empty() || !extra.is_empty() {
                return "Usage: /lab task envelope <task_id>".to_string();
            }
            let run = match store.latest_run() {
                Ok(Some(run)) => run,
                Ok(None) => return "No LabRun found for graduate task.".to_string(),
                Err(err) => return format!("Failed to read LabRun for graduate task: {err}"),
            };
            let task = match store.load_graduate_task(&run.lab_run_id, task_id) {
                Ok(task) => task,
                Err(err) => return format!("Failed to read graduate task: {err}"),
            };
            match build_graduate_task_dispatch(&task) {
                Ok(dispatch) => {
                    let params = serde_json::to_string_pretty(&dispatch.agent_tool_params)
                        .unwrap_or_else(|_| "{}".to_string());
                    format!(
                        "Graduate task envelope: {}\nTo: {}\nExpected artifacts: {}\nAgent tool params:\n{}",
                        dispatch.envelope.envelope_id,
                        dispatch
                            .envelope
                            .to
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| "none".to_string()),
                        dispatch.envelope.expected_artifacts.join(","),
                        params
                    )
                }
                Err(err) => format!("Failed to build graduate task envelope: {err}"),
            }
        }
        "dispatch" => {
            let (task_id, extra) = split_once(rest);
            if task_id.is_empty() || !extra.is_empty() {
                return "Usage: /lab task dispatch <task_id>".to_string();
            }
            let run = match store.latest_run() {
                Ok(Some(run)) => run,
                Ok(None) => return "No LabRun found for graduate task.".to_string(),
                Err(err) => return format!("Failed to read LabRun for graduate task: {err}"),
            };
            let task = match store.load_graduate_task(&run.lab_run_id, task_id) {
                Ok(task) => task,
                Err(err) => return format!("Failed to read graduate task: {err}"),
            };
            let dispatch = match build_graduate_task_dispatch(&task) {
                Ok(dispatch) => dispatch,
                Err(err) => return format!("Failed to build graduate task dispatch: {err}"),
            };
            match store.record_graduate_dispatch(&run.lab_run_id, task_id, dispatch) {
                Ok(record) => format!(
                    "Prepared graduate dispatch: {}\nTask: {}\nEnvelope: {}\nStatus: {:?}\nDispatch: {}",
                    record.dispatch_id,
                    record.task_id,
                    record.envelope.envelope_id,
                    record.status,
                    store
                        .root()
                        .join("runs")
                        .join(&run.lab_run_id)
                        .join("dispatches")
                        .join(format!("{}.json", record.dispatch_id))
                        .display()
                ),
                Err(err) => format!("Failed to record graduate dispatch: {err}"),
            }
        }
        "run" => {
            "Usage: /lab task run <task_id> requires runtime ToolContext; use the Lab Mode shell command."
                .to_string()
        }
        "sync" => {
            "Usage: /lab task sync <task_id> requires runtime ToolContext; use the Lab Mode shell command."
                .to_string()
        }
        "start" => {
            let (task_id, extra) = split_once(rest);
            if task_id.is_empty() || !extra.is_empty() {
                return "Usage: /lab task start <task_id>".to_string();
            }
            let run = match store.latest_run() {
                Ok(Some(run)) => run,
                Ok(None) => return "No LabRun found for graduate task.".to_string(),
                Err(err) => return format!("Failed to read LabRun for graduate task: {err}"),
            };
            match store.start_graduate_task(&run.lab_run_id, task_id) {
                Ok(task) => format!(
                    "Started graduate task: {}\nStatus: {:?}",
                    task.task_id, task.status
                ),
                Err(err) => format!("Failed to start graduate task: {err}"),
            }
        }
        "complete" => {
            let (task_id, rest) = split_once(rest);
            let (result_artifact_id, evidence_csv) = split_once(rest);
            if task_id.is_empty() || result_artifact_id.is_empty() {
                return "Usage: /lab task complete <task_id> <result_artifact_id> [evidence_csv]"
                    .to_string();
            }
            let run = match store.latest_run() {
                Ok(Some(run)) => run,
                Ok(None) => return "No LabRun found for graduate task.".to_string(),
                Err(err) => return format!("Failed to read LabRun for graduate task: {err}"),
            };
            match store.complete_graduate_task(
                &run.lab_run_id,
                task_id,
                result_artifact_id,
                split_csv(evidence_csv),
            ) {
                Ok(task) => format!(
                    "Completed graduate task: {}\nResult: {}\nEvidence: {}",
                    task.task_id,
                    task.result_artifact_id.as_deref().unwrap_or("none"),
                    format_list(&task.evidence_ids)
                ),
                Err(err) => format!("Failed to complete graduate task: {err}"),
            }
        }
        "result" => {
            let parsed = match parse_task_result(rest) {
                Ok(parsed) => parsed,
                Err(err) => return err,
            };
            match orchestrator.create_graduate_result_for_task_latest(
                &parsed.task_id,
                &parsed.summary,
                parsed.changed_files,
                parsed.validation_attempts,
                parsed.blockers,
                parsed.evidence_ids,
            ) {
                Ok(created) => format!(
                    "Created graduate result artifact: {}\nArtifact: {}\nReport: {}\nGate status: {}",
                    created.artifact.artifact_id(),
                    created.path.display(),
                    created.report_path.display(),
                    if created.gate.is_satisfied() {
                        "satisfied"
                    } else {
                        "not_satisfied"
                    }
                ),
                Err(err) => format!("Failed to create graduate result artifact: {err}"),
            }
        }
        "bind-json" => {
            let (task_id, json_file) = split_once(rest);
            if task_id.is_empty() || json_file.trim().is_empty() {
                return "Usage: /lab task bind-json <task_id> <json_file>".to_string();
            }
            let json = match read_lab_command_file(project_root, json_file.trim()) {
                Ok(json) => json,
                Err(err) => return err,
            };
            match orchestrator.bind_graduate_agent_json_for_task_latest(task_id, &json) {
                Ok(created) => format!(
                    "Bound graduate agent JSON result: {}\nArtifact: {}\nReport: {}\nGate status: {}",
                    created.artifact.artifact_id(),
                    created.path.display(),
                    created.report_path.display(),
                    if created.gate.is_satisfied() {
                        "satisfied"
                    } else {
                        "not_satisfied"
                    }
                ),
                Err(err) => format!("Failed to bind graduate agent JSON result: {err}"),
            }
        }
        "block" => {
            let (task_id, reason) = split_once(rest);
            if task_id.is_empty() || reason.is_empty() {
                return "Usage: /lab task block <task_id> <reason>".to_string();
            }
            let run = match store.latest_run() {
                Ok(Some(run)) => run,
                Ok(None) => return "No LabRun found for graduate task.".to_string(),
                Err(err) => return format!("Failed to read LabRun for graduate task: {err}"),
            };
            match store.block_graduate_task(&run.lab_run_id, task_id, reason) {
                Ok(task) => format!(
                    "Blocked graduate task: {}\nReason: {}",
                    task.task_id,
                    task.blocker.as_deref().unwrap_or("none")
                ),
                Err(err) => format!("Failed to block graduate task: {err}"),
            }
        }
        "revise" => {
            let parts = rest.split('|').map(str::trim).collect::<Vec<_>>();
            if parts.len() < 3 || parts[0].is_empty() {
                return "Usage: /lab task revise <task_id> | <scope_csv> | <validation_csv> | [instructions]"
                    .to_string();
            }
            let run = match store.latest_run() {
                Ok(Some(run)) => run,
                Ok(None) => return "No LabRun found for graduate task revision.".to_string(),
                Err(err) => {
                    return format!("Failed to read LabRun for graduate task revision: {err}")
                }
            };
            let instructions = parts.get(3).copied().unwrap_or("");
            match store.revise_graduate_task(
                &run.lab_run_id,
                parts[0],
                split_csv(parts[1]),
                split_csv(parts[2]),
                Some(instructions),
            ) {
                Ok(task) => format!(
                    "Revised graduate task: {}\nStatus: {:?}\nScope: {}\nValidation: {}\nBlocker: {}",
                    task.task_id,
                    task.status,
                    format_list(&task.allowed_scope),
                    format_list(&task.required_validation),
                    task.blocker.as_deref().unwrap_or("none")
                ),
                Err(err) => format!("Failed to revise graduate task: {err}"),
            }
        }
        "retry" => {
            let parts = rest.split('|').map(str::trim).collect::<Vec<_>>();
            if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
                return "Usage: /lab task retry <task_id> | <validation_summary>".to_string();
            }
            let run = match store.latest_run() {
                Ok(Some(run)) => run,
                Ok(None) => return "No LabRun found for graduate task retry.".to_string(),
                Err(err) => return format!("Failed to read LabRun for graduate task retry: {err}"),
            };
            match store.record_validation_retry_and_repair_task(&run.lab_run_id, parts[0], parts[1])
            {
                Ok(retry) => format!(
                    "Recorded validation retry: {}\nAttempt: {}\nRepair task: {}\nEscalated: {}",
                    retry.retry_id,
                    retry.attempt,
                    retry.repair_task_id.as_deref().unwrap_or("none"),
                    retry.escalated
                ),
                Err(err) => format!("Failed to record validation retry: {err}"),
            }
        }
        "cancel" => {
            let (task_id, reason) = split_once(rest);
            if task_id.is_empty() {
                return "Usage: /lab task cancel <task_id> [reason]".to_string();
            }
            let run = match store.latest_run() {
                Ok(Some(run)) => run,
                Ok(None) => return "No LabRun found for graduate task.".to_string(),
                Err(err) => return format!("Failed to read LabRun for graduate task: {err}"),
            };
            match store.cancel_graduate_task(&run.lab_run_id, task_id, Some(reason)) {
                Ok(task) => format!(
                    "Cancelled graduate task: {}\nReason: {}",
                    task.task_id,
                    task.blocker.as_deref().unwrap_or("none")
                ),
                Err(err) => format!("Failed to cancel graduate task: {err}"),
            }
        }
        _ => {
            "Usage: /lab task [list|create|envelope|dispatch|run|sync|start|complete|result|bind-json|block|revise|retry|cancel]"
                .to_string()
        }
    }
}

fn list_graduate_tasks(store: &LabStore) -> String {
    let tasks = match store.latest_graduate_tasks() {
        Ok(tasks) => tasks,
        Err(err) => return format!("Failed to read graduate tasks: {err}"),
    };
    if tasks.is_empty() {
        return "No graduate tasks recorded.".to_string();
    }
    let open = tasks.iter().filter(|task| task.status.is_open()).count();
    let mut lines = vec![format!(
        "Graduate tasks: {} total, {} open",
        tasks.len(),
        open
    )];
    for task in tasks.iter().rev().take(20).rev() {
        lines.push(format!(
            "{} {:?} title={} scope={} validation={} result={} blocker={}",
            task.task_id,
            task.status,
            task.title,
            format_list(&task.allowed_scope),
            format_list(&task.required_validation),
            task.result_artifact_id.as_deref().unwrap_or("none"),
            task.blocker.as_deref().unwrap_or("none")
        ));
    }
    lines.join("\n")
}

fn parse_task_create(args: &str) -> Result<(String, Vec<String>, Vec<String>, String), String> {
    let parts = args.split('|').map(str::trim).collect::<Vec<_>>();
    if parts.len() < 4 {
        return Err(
            "Usage: /lab task create <title> | <scope_csv> | <validation_csv> | <instructions>"
                .to_string(),
        );
    }
    let title = parts[0].to_string();
    let instructions = parts[3..].join(" | ");
    if title.trim().is_empty() || instructions.trim().is_empty() {
        return Err(
            "Usage: /lab task create <title> | <scope_csv> | <validation_csv> | <instructions>"
                .to_string(),
        );
    }
    Ok((
        title,
        split_csv(parts[1]),
        split_csv(parts[2]),
        instructions,
    ))
}

struct ParsedTaskResult {
    task_id: String,
    changed_files: Vec<String>,
    validation_attempts: Vec<String>,
    blockers: Vec<String>,
    evidence_ids: Vec<String>,
    summary: String,
}

fn parse_task_result(args: &str) -> Result<ParsedTaskResult, String> {
    let parts = args.split('|').map(str::trim).collect::<Vec<_>>();
    if parts.len() < 6 {
        return Err("Usage: /lab task result <task_id> | <changed_csv> | <validation_csv> | <blockers_csv> | <evidence_csv> | <summary>".to_string());
    }
    let task_id = parts[0].to_string();
    let summary = parts[5..].join(" | ");
    if task_id.trim().is_empty() || summary.trim().is_empty() {
        return Err("Usage: /lab task result <task_id> | <changed_csv> | <validation_csv> | <blockers_csv> | <evidence_csv> | <summary>".to_string());
    }
    Ok(ParsedTaskResult {
        task_id,
        changed_files: split_csv(parts[1]),
        validation_attempts: split_csv(parts[2]),
        blockers: split_csv(parts[3]),
        evidence_ids: split_csv(parts[4]),
        summary,
    })
}

fn read_lab_command_file(project_root: &Path, path: &str) -> Result<String, String> {
    let path = PathBuf::from(path);
    let resolved = if path.is_absolute() {
        path
    } else {
        project_root.join(path)
    };
    fs::read_to_string(&resolved).map_err(|err| {
        format!(
            "Failed to read Lab command file {}: {err}",
            resolved.display()
        )
    })
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn format_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(",")
    }
}
