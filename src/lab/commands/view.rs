//! Read-only LabRun view and report command handlers.
//!
//! View commands render gates, dashboards, reports, artifacts, recovery state,
//! and certification status without mutating LabRun execution state.

use super::*;

pub(super) fn handle_gate_command(orchestrator: &LabOrchestrator, args: &str) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        return match orchestrator.required_gate_for_latest() {
            Ok(gate) => format!(
                "Required gate for stage '{}': artifact_type={} owner={:?}\nWrite with /lab gate satisfy <artifact_id> [validation_status] [evidence_ref]",
                gate.stage, gate.required_artifact_type, gate.owner
            ),
            Err(err) => format!("Failed to read required gate: {err}"),
        };
    }

    match parts.as_slice() {
        ["satisfy", artifact_id] => {
            write_gate(orchestrator, artifact_id, Some("not_verified"), None)
        }
        ["satisfy", artifact_id, validation_status] => {
            write_gate(orchestrator, artifact_id, Some(validation_status), None)
        }
        ["satisfy", artifact_id, validation_status, evidence_ref, ..] => write_gate(
            orchestrator,
            artifact_id,
            Some(validation_status),
            Some(evidence_ref),
        ),
        _ => "Usage: /lab gate [satisfy <artifact_id> [validation_status] [evidence_ref]]"
            .to_string(),
    }
}

pub(super) fn handle_artifact_accept_command(orchestrator: &LabOrchestrator, args: &str) -> String {
    let (artifact_id, note) = split_once(args);
    if artifact_id.trim().is_empty() {
        return "Usage: /lab accept <artifact_id> [note]".to_string();
    }
    match orchestrator.accept_artifact_latest(artifact_id, note) {
        Ok(gate) => format!(
            "Accepted artifact: {}\nGate: {} validation_status={}",
            gate.artifact_id.as_deref().unwrap_or_default(),
            gate.stage,
            gate.validation_status.as_deref().unwrap_or("none")
        ),
        Err(err) => format!("Failed to accept Lab artifact: {err}"),
    }
}

pub(super) fn handle_artifact_revise_command(orchestrator: &LabOrchestrator, args: &str) -> String {
    let (artifact_id, note) = split_once(args);
    if artifact_id.trim().is_empty() || note.trim().is_empty() {
        return "Usage: /lab revise <artifact_id> <note>".to_string();
    }
    match orchestrator.revise_artifact_latest(artifact_id, note) {
        Ok(gate) => format!(
            "Revision requested for artifact: {}\nGate: {} validation_status={}\nBlockers: {}",
            gate.artifact_id.as_deref().unwrap_or_default(),
            gate.stage,
            gate.validation_status.as_deref().unwrap_or("none"),
            gate.blockers.join("; ")
        ),
        Err(err) => format!("Failed to request Lab artifact revision: {err}"),
    }
}

pub(super) fn handle_review_command(
    orchestrator: &LabOrchestrator,
    store: &LabStore,
    args: &str,
) -> String {
    let (action, rest) = split_once(args);
    if action == "artifact" {
        if rest.trim().is_empty() {
            return "Usage: /lab review artifact <artifact_id> [instructions]".to_string();
        }
        return "Provider artifact review is available in the Lab Mode shell: /lab review artifact <artifact_id> [instructions]."
            .to_string();
    }
    if !action.is_empty() {
        return "Usage: /lab review [artifact <artifact_id> [instructions]]".to_string();
    }
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found for review.".to_string(),
        Err(err) => return format!("Failed to read LabRun review state: {err}"),
    };
    let gate = store
        .load_artifact_gate(&run.lab_run_id, &run.current_stage)
        .or_else(|_| orchestrator.required_gate_for_latest())
        .ok();
    let artifacts = match store.list_stage_artifacts(&run.lab_run_id) {
        Ok(artifacts) => artifacts,
        Err(err) => return format!("Failed to read LabRun review artifacts: {err}"),
    };
    let reports = match store.list_stage_artifact_report_paths(&run.lab_run_id) {
        Ok(reports) => reports,
        Err(err) => return format!("Failed to read LabRun review reports: {err}"),
    };
    let evidence = match store.list_evidence_refs(&run.lab_run_id) {
        Ok(evidence) => evidence,
        Err(err) => return format!("Failed to read LabRun review evidence: {err}"),
    };
    let tasks = match store.list_graduate_tasks(&run.lab_run_id) {
        Ok(tasks) => tasks,
        Err(err) => return format!("Failed to read LabRun review tasks: {err}"),
    };
    let dispatches = match store.list_graduate_dispatches(&run.lab_run_id) {
        Ok(dispatches) => dispatches,
        Err(err) => return format!("Failed to read LabRun review dispatches: {err}"),
    };
    let events = match store.list_run_events(&run.lab_run_id) {
        Ok(events) => events,
        Err(err) => {
            return format!("Failed to read LabRun review events: {err}");
        }
    };
    let worktree_proofs = graduate_worktree_proof_lines(&events, 3);
    let workspace_snapshots = graduate_workspace_snapshot_lines(&events, 4);
    let blocked_tasks = tasks
        .iter()
        .filter(|task| matches!(task.status, crate::lab::model::LabTaskStatus::Blocked))
        .count();
    let open_tasks = tasks.iter().filter(|task| task.status.is_open()).count();
    let latest_artifact = artifacts.last();
    let latest_report = reports.last();
    let mut lines = vec![
        format!("Lab review: {}", run.lab_run_id),
        format!(
            "Run: status={:?} stage={} owner={:?} cycle={} needs_user={}",
            run.status, run.current_stage, run.internal_owner, run.cycle_count, run.needs_user
        ),
        format!(
            "Artifacts: {} latest={}",
            artifacts.len(),
            latest_artifact
                .map(|artifact| artifact.artifact_id())
                .unwrap_or("none")
        ),
        format!(
            "Reports: {} latest={}",
            reports.len(),
            latest_report
                .map(|(_, path)| path.display().to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "Tasks: total={} open={} blocked={}",
            tasks.len(),
            open_tasks,
            blocked_tasks
        ),
        format!(
            "Evidence refs: {} blocked_reason={}",
            evidence.len(),
            run.blocked_reason.as_deref().unwrap_or("none")
        ),
    ];
    if let Some(gate) = gate {
        lines.push(format!(
            "Current gate: stage={} artifact_type={} owner={:?} artifact={} validation={} satisfied={}",
            gate.stage,
            gate.required_artifact_type,
            gate.owner,
            gate.artifact_id.as_deref().unwrap_or("none"),
            gate.validation_status.as_deref().unwrap_or("none"),
            gate.is_satisfied()
        ));
    } else {
        lines.push("Current gate: none for this stage".to_string());
    }
    lines.extend(graduate_cleanup_state_lines(&dispatches, 5));
    lines.extend(worktree_proofs);
    lines.extend(workspace_snapshots);
    lines.push("Next review actions:".to_string());
    if let Some(artifact) = latest_artifact {
        lines.push(format!(
            "  Provider artifact review: /lab review artifact {}",
            artifact.artifact_id()
        ));
    }
    if run.current_stage == "postdoc_review" {
        lines.push("  Create postdoc integration summary: /lab integrate [note]".to_string());
    }
    if run.current_stage == "professor_review" {
        lines.push("  Create professor final review: /lab professor-review [note]".to_string());
    }
    if blocked_tasks > 0 || run.blocked_reason.is_some() {
        lines.push("  Inspect blockers: /lab blocker status".to_string());
        lines.push("  Escalate blocker: /lab blocker escalate".to_string());
    }
    lines.push("  Inspect latest report: /lab report".to_string());
    lines.join("\n")
}

fn graduate_worktree_proof_lines(
    events: &[crate::lab::model::LabEvent],
    limit: usize,
) -> Vec<String> {
    let mut proofs = events
        .iter()
        .rev()
        .filter(|event| event.event_type == "lab_graduate_worktree_action")
        .take(limit)
        .map(format_graduate_worktree_proof_event)
        .collect::<Vec<_>>();
    proofs.reverse();
    if proofs.is_empty() {
        vec!["Graduate worktree proof: none".to_string()]
    } else {
        let mut lines = vec!["Graduate worktree proof:".to_string()];
        lines.extend(proofs.into_iter().map(|line| format!("  {line}")));
        lines
    }
}

pub(super) fn graduate_cleanup_state_lines(
    dispatches: &[GraduateDispatchRecord],
    limit: usize,
) -> Vec<String> {
    if dispatches.is_empty() {
        return vec!["Graduate cleanup states: none".to_string()];
    }
    let pending = dispatches
        .iter()
        .filter(|dispatch| dispatch.cleanup_status == GraduateCleanupStatus::CleanupPending)
        .count();
    let done = dispatches
        .iter()
        .filter(|dispatch| dispatch.cleanup_status == GraduateCleanupStatus::CleanupDone)
        .count();
    let blocked = dispatches
        .iter()
        .filter(|dispatch| dispatch.cleanup_status == GraduateCleanupStatus::CleanupBlocked)
        .count();
    let mut recent = dispatches.iter().rev().take(limit).collect::<Vec<_>>();
    recent.reverse();

    let mut lines = vec![format!(
        "Graduate cleanup states: pending={} done={} blocked={}",
        pending, done, blocked
    )];
    for dispatch in recent {
        lines.push(format!(
            "  task={} dispatch={} status={} agent={} result={} updated={} message={}",
            dispatch.task_id,
            dispatch.dispatch_id,
            dispatch.cleanup_status.as_str(),
            dispatch.agent_id.as_deref().unwrap_or("none"),
            dispatch.result_artifact_id.as_deref().unwrap_or("none"),
            dispatch
                .cleanup_updated_at
                .map(|time| time.to_rfc3339())
                .unwrap_or_else(|| "none".to_string()),
            dispatch.cleanup_message.as_deref().unwrap_or("none")
        ));
    }
    lines
}

fn format_graduate_worktree_proof_event(event: &crate::lab::model::LabEvent) -> String {
    let payload = &event.payload;
    let result_data = payload
        .get("result_data")
        .unwrap_or(&serde_json::Value::Null);
    let action = payload
        .get("action")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let task_id = payload
        .get("task_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let agent_ref_kind = payload
        .get("agent_ref_kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let agent_ref = payload
        .get("agent_ref")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let success = payload
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let merge_kind = result_data
        .get("merge_kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("n/a");
    let dirty = result_data
        .get("dirty")
        .and_then(serde_json::Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let path = result_data
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("n/a");
    format!(
        "{} task={} success={} ref={}:{} merge_kind={} dirty={} path={}",
        action, task_id, success, agent_ref_kind, agent_ref, merge_kind, dirty, path
    )
}

fn graduate_workspace_snapshot_lines(
    events: &[crate::lab::model::LabEvent],
    limit: usize,
) -> Vec<String> {
    let mut snapshots = events
        .iter()
        .rev()
        .filter(|event| event.event_type == "lab_graduate_workspace_snapshot")
        .take(limit)
        .map(format_graduate_workspace_snapshot_event)
        .collect::<Vec<_>>();
    snapshots.reverse();
    if snapshots.is_empty() {
        vec!["Graduate workspace snapshots: none".to_string()]
    } else {
        let mut lines = vec!["Graduate workspace snapshots:".to_string()];
        lines.extend(snapshots.into_iter().map(|line| format!("  {line}")));
        lines
    }
}

fn format_graduate_workspace_snapshot_event(event: &crate::lab::model::LabEvent) -> String {
    let payload = &event.payload;
    let phase = payload
        .get("phase")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let task_id = payload
        .get("task_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let dispatch_id = payload
        .get("dispatch_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let dirty_count = payload
        .get("dirty_path_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let changed_count = payload
        .get("changed_path_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let dirty_paths = json_string_list(payload.get("dirty_paths"));
    let changed_paths = json_string_list(payload.get("changed_paths"));
    format!(
        "{} task={} dispatch={} dirty={} [{}] changed={} [{}]",
        phase,
        task_id,
        dispatch_id,
        dirty_count,
        summarize_paths(&dirty_paths),
        changed_count,
        summarize_paths(&changed_paths)
    )
}

fn json_string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn summarize_paths(paths: &[String]) -> String {
    if paths.is_empty() {
        return "none".to_string();
    }
    let mut shown = paths.iter().take(5).cloned().collect::<Vec<_>>();
    if paths.len() > shown.len() {
        shown.push(format!("+{} more", paths.len() - shown.len()));
    }
    shown.join(",")
}

fn write_gate(
    orchestrator: &LabOrchestrator,
    artifact_id: &str,
    validation_status: Option<&str>,
    evidence_ref: Option<&str>,
) -> String {
    match orchestrator.write_satisfied_gate_for_latest(artifact_id, validation_status, evidence_ref)
    {
        Ok(gate) => format!(
            "Artifact gate satisfied for stage '{}': artifact_id={}",
            gate.stage,
            gate.artifact_id.unwrap_or_default()
        ),
        Err(err) => format!("Failed to satisfy artifact gate: {err}"),
    }
}

pub(super) fn lab_status(store: &LabStore) -> String {
    match store.latest_run() {
        Ok(Some(run)) => {
            let index_line = match store.load_runs_index() {
                Ok(Some(index)) => {
                    let indexed = index
                        .entries
                        .iter()
                        .find(|entry| entry.lab_run_id == run.lab_run_id)
                        .map(|entry| {
                            format!(
                                "matched stage={} owner={:?} updated={}",
                                entry.current_stage, entry.internal_owner, entry.updated_at
                            )
                        })
                        .unwrap_or_else(|| "latest run missing from index".to_string());
                    format!(
                        "Index: {} entries={} latest={}",
                        store.root().join("runs_index.json").display(),
                        index.entries.len(),
                        indexed
                    )
                }
                Ok(None) => format!(
                    "Index: missing ({})",
                    store.root().join("runs_index.json").display()
                ),
                Err(err) => format!("Index: unavailable ({err})"),
            };
            let sqlite_line = match store.load_sqlite_index_summary() {
                Ok(Some(summary)) => format!(
                    "SQLite index: {} runs={} artifacts={} events={} tasks={}",
                    summary.path.display(),
                    summary.lab_runs,
                    summary.lab_artifacts,
                    summary.lab_events,
                    summary.lab_tasks
                ),
                Ok(None) => format!("SQLite index: missing ({})", store.sqlite_index_path().display()),
                Err(err) => format!("SQLite index: unavailable ({err})"),
            };
            [
                format!("Latest LabRun: {}", run.lab_run_id),
                format!("Status: {:?}", run.status),
                format!("Stage: {}", run.current_stage),
                format!("Owner: {:?}", run.internal_owner),
                format!("Cycles: {}", run.cycle_count),
                format!("Proposal: {}", run.proposal_id.as_deref().unwrap_or("none")),
                format!(
                    "State: {}",
                    store
                        .root()
                        .join("runs")
                        .join(&run.lab_run_id)
                        .join("state.json")
                        .display()
                ),
                index_line,
                sqlite_line,
            ]
            .join("\n")
        }
        Ok(None) => match store.latest_proposal() {
            Ok(Some(proposal)) => format!(
                "No LabRun yet.\nLatest proposal: {}\nStatus: {:?}\nGoal: {}\nApprove with /lab approve {}",
                proposal.proposal_id, proposal.status, proposal.user_goal, proposal.proposal_id
            ),
            Ok(None) => "No LabRun or proposal found. Start with /lab propose <idea>.".to_string(),
            Err(err) => format!("Failed to read latest proposal: {err}"),
        },
        Err(err) => format!("Failed to read Lab status: {err}"),
    }
}

pub(super) fn handle_meeting_command(
    project_root: &Path,
    orchestrator: &LabOrchestrator,
    args: &str,
) -> String {
    let (action, rest) = split_once(args);
    match action {
        "recommend" => render_meeting_recommendation(orchestrator),
        "open" => open_recommended_meeting(orchestrator, rest),
        "llm" => {
            let topic = rest.trim();
            let command = if topic.is_empty() {
                "meeting llm".to_string()
            } else {
                format!("meeting llm {topic}")
            };
            format!(
                "Usage: /lab {command} requires the Lab Mode shell provider. In non-interactive mode use `pa lab --command \"{command}\" --with-provider` from {}.",
                project_root.display()
            )
        }
        _ => {
            let topic = (!args.trim().is_empty()).then_some(args);
            match orchestrator.create_meeting_summary_for_latest(topic) {
                Ok(created) => format!(
                    "Lab meeting summary created: {}\nThis meeting is read-only and does not mutate code.\nArtifact: {}\nReport: {}",
                    created.artifact.artifact_id(),
                    created.path.display(),
                    created.report_path.display()
                ),
                Err(err) => format!("Failed to request Lab meeting: {err}"),
            }
        }
    }
}

fn render_meeting_recommendation(orchestrator: &LabOrchestrator) -> String {
    match orchestrator.meeting_recommendation_for_latest() {
        Ok(recommendation) => {
            let mut lines = vec![
                format!(
                    "Lab runtime escalation signals: {}",
                    recommendation.lab_run_id
                ),
                format!("Suggested meeting: {}", recommendation.recommended),
                format!("Reason: {}", recommendation.reason),
                format!("Topic: {}", recommendation.topic),
            ];
            if recommendation.signals.is_empty() {
                lines.push("Signals: none".to_string());
            } else {
                lines.push(format!("Signals: {}", recommendation.signals.join("; ")));
            }
            if recommendation.recommended {
                lines.push(format!(
                    "Open meeting with /lab meeting open {}",
                    recommendation.topic
                ));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to evaluate Lab runtime escalation signals: {err}"),
    }
}

fn open_recommended_meeting(orchestrator: &LabOrchestrator, args: &str) -> String {
    let explicit_topic = args.trim();
    let mut request_line = None;
    let topic = if explicit_topic.is_empty() {
        let recommendation = match orchestrator.meeting_recommendation_for_latest() {
            Ok(recommendation) => recommendation,
            Err(err) => return format!("Failed to evaluate Lab runtime escalation signals: {err}"),
        };
        if !recommendation.recommended {
            return format!(
                "No runtime escalation signal is open for {}.\nReason: {}\nUse /lab meeting <topic> to create a manual read-only meeting.",
                recommendation.lab_run_id, recommendation.reason
            );
        }
        match orchestrator.create_meeting_request_for_latest(&recommendation) {
            Ok(created) => {
                request_line = Some(format!(
                    "Request: {}\nRequest report: {}",
                    created.artifact.artifact_id(),
                    created.report_path.display()
                ));
            }
            Err(err) => return format!("Failed to write Lab meeting request: {err}"),
        }
        recommendation.topic
    } else {
        explicit_topic.to_string()
    };

    match orchestrator.create_meeting_summary_for_latest(Some(&topic)) {
        Ok(created) => {
            let source = if explicit_topic.is_empty() {
                "runtime escalation signal"
            } else {
                "manual topic"
            };
            let mut lines = vec![
                format!(
                    "Lab meeting opened from {source}: {}",
                    created.artifact.artifact_id()
                ),
                "This meeting is read-only and does not mutate code.".to_string(),
                format!("Topic: {topic}"),
            ];
            if let Some(request_line) = request_line {
                lines.push(request_line);
            }
            lines.push(format!("Artifact: {}", created.path.display()));
            lines.push(format!("Report: {}", created.report_path.display()));
            lines.join("\n")
        }
        Err(err) => format!("Failed to open Lab meeting: {err}"),
    }
}

pub(super) fn handle_runs_command(store: &LabStore) -> String {
    let index = match store.rebuild_runs_index() {
        Ok(index) => index,
        Err(err) => return format!("Failed to rebuild LabRun index: {err}"),
    };
    if index.entries.is_empty() {
        return "No LabRuns found. Start with /lab propose <idea>.".to_string();
    }
    let active_id = store.latest_run().ok().flatten().map(|run| run.lab_run_id);
    let mut lines = vec![
        "Lab runs:".to_string(),
        format!("Total: {}", index.entries.len()),
        format!("Index: {}", store.root().join("runs_index.json").display()),
        "Open one with /lab open <lab_run_id>".to_string(),
    ];
    for entry in index.entries.iter().rev().take(20) {
        let marker = if active_id.as_deref() == Some(entry.lab_run_id.as_str()) {
            "*"
        } else {
            "-"
        };
        lines.push(format!(
            "{} {} status={:?} stage={} owner={:?} updated={} tasks={} artifacts={} pause={}",
            marker,
            entry.lab_run_id,
            entry.status,
            entry.current_stage,
            entry.internal_owner,
            entry.updated_at.to_rfc3339(),
            entry.open_task_count,
            entry.artifact_count,
            entry.pause_reason.as_deref().unwrap_or("none")
        ));
    }
    lines.join("\n")
}

pub(super) fn handle_report_command(store: &LabStore, args: &str) -> String {
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found for reports.".to_string(),
        Err(err) => return format!("Failed to read LabRun reports: {err}"),
    };
    let reports = match store.list_stage_artifact_report_paths(&run.lab_run_id) {
        Ok(reports) => reports,
        Err(err) => return format!("Failed to list LabRun reports: {err}"),
    };
    if reports.is_empty() {
        return format!("No Lab reports found for {}.", run.lab_run_id);
    }
    let trimmed = args.trim();
    if trimmed == "list" || trimmed == "ls" {
        let mut lines = vec![
            format!("Lab reports: {}", run.lab_run_id),
            format!("Reports: {}", reports.len()),
        ];
        for (artifact_id, path) in reports.iter().rev().take(10).rev() {
            lines.push(format!("- {} {}", artifact_id, path.display()));
        }
        return lines.join("\n");
    }
    let selected = if trimmed.is_empty() || trimmed == "latest" {
        reports.last()
    } else {
        reports
            .iter()
            .find(|(artifact_id, _)| artifact_id == trimmed)
    };
    let Some((artifact_id, path)) = selected else {
        return format!("Lab report not found for artifact '{trimmed}'. Use /lab report list.");
    };
    match fs::read_to_string(path) {
        Ok(content) => format!(
            "Lab report: {}\nArtifact: {}\nPath: {}\nPreview: {}",
            run.lab_run_id,
            artifact_id,
            path.display(),
            compact_message_line(&content, 1_200)
        ),
        Err(err) => format!("Failed to read Lab report {}: {err}", path.display()),
    }
}

pub(super) fn handle_dashboard_command(
    project_root: &Path,
    orchestrator: &LabOrchestrator,
    store: &LabStore,
) -> String {
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found for dashboard.".to_string(),
        Err(err) => return format!("Failed to read LabRun dashboard: {err}"),
    };
    let tasks = match store.list_graduate_tasks(&run.lab_run_id) {
        Ok(tasks) => tasks,
        Err(err) => return format!("Failed to read Lab dashboard tasks: {err}"),
    };
    let open_tasks = tasks.iter().filter(|task| task.status.is_open()).count();
    let blocked_tasks = tasks
        .iter()
        .filter(|task| matches!(task.status, crate::lab::model::LabTaskStatus::Blocked))
        .count();
    let retries = match store.list_validation_retries(&run.lab_run_id) {
        Ok(retries) => retries,
        Err(err) => return format!("Failed to read Lab dashboard retries: {err}"),
    };
    let escalated_retries = retries.iter().filter(|retry| retry.escalated).count();
    let cost = match store.cost_summary(&run.lab_run_id) {
        Ok(cost) => cost,
        Err(err) => return format!("Failed to read Lab dashboard cost: {err}"),
    };
    let events = match store.list_run_events(&run.lab_run_id) {
        Ok(events) => events,
        Err(err) => return format!("Failed to read Lab dashboard events: {err}"),
    };
    let dispatches = match store.list_graduate_dispatches(&run.lab_run_id) {
        Ok(dispatches) => dispatches,
        Err(err) => return format!("Failed to read Lab dashboard dispatches: {err}"),
    };
    let worktree_proofs = graduate_worktree_proof_lines(&events, 2);
    let workspace_snapshots = graduate_workspace_snapshot_lines(&events, 2);
    let meeting = match orchestrator.meeting_recommendation_for_latest() {
        Ok(meeting) => meeting,
        Err(err) => return format!("Failed to evaluate Lab runtime escalation signals: {err}"),
    };
    let scheduler_line = match background_scheduler_status(project_root) {
        Ok(status) => {
            let persisted = status
                .persisted
                .map(|state| format!("{:?}", state.status))
                .unwrap_or_else(|| "none".to_string());
            format!(
                "Scheduler: running_in_process={} persisted={}",
                status.running_in_process, persisted
            )
        }
        Err(err) => format!("Scheduler: unavailable ({err})"),
    };
    let sqlite_dashboard_line = match store.load_sqlite_dashboard_summary(&run.lab_run_id) {
        Ok(Some(summary)) => format!(
            "Indexed dashboard: sqlite={} runs={} artifacts={} events={} tasks={} professor={} postdoc={}",
            summary.index.path.display(),
            summary.index.lab_runs,
            summary.index.lab_artifacts,
            summary.index.lab_events,
            summary.index.lab_tasks,
            format_sqlite_artifact_summary(summary.latest_professor_artifact.as_ref()),
            format_sqlite_artifact_summary(summary.latest_postdoc_artifact.as_ref())
        ),
        Ok(None) => format!(
            "Indexed dashboard: missing ({})",
            store.sqlite_index_path().display()
        ),
        Err(err) => format!("Indexed dashboard: unavailable ({err})"),
    };

    let mut lines = vec![
        format!("Lab dashboard: {}", run.lab_run_id),
        format!(
            "Run: status={:?} stage={} owner={:?} needs_user={}",
            run.status, run.current_stage, run.internal_owner, run.needs_user
        ),
        format!(
            "Progress: cycles={} failures={} artifacts={} meetings={}",
            run.cycle_count,
            run.failure_count,
            run.artifact_ids.len(),
            run.meeting_ids.len()
        ),
        format!(
            "Tasks: total={} open={} blocked={}",
            tasks.len(),
            open_tasks,
            blocked_tasks
        ),
        format!(
            "Validation retries: total={} escalated={}",
            retries.len(),
            escalated_retries
        ),
        format!(
            "Cost: requests={} total_tokens={} cache_hit_rate={:.1}% estimated_cost_usd={:.6}",
            cost.requests,
            cost.total_tokens,
            cost.cache_hit_rate_percent(),
            cost.estimated_cost_usd
        ),
        format!(
            "Runtime escalation signals: suggested_meeting={} topic={} reason={}",
            meeting.recommended, meeting.topic, meeting.reason
        ),
        scheduler_line,
        sqlite_dashboard_line,
        format!(
            "Blocked reason: {}",
            run.blocked_reason.as_deref().unwrap_or("none")
        ),
    ];
    lines.extend(graduate_cleanup_state_lines(&dispatches, 5));
    lines.extend(worktree_proofs);
    lines.extend(workspace_snapshots);
    lines.join("\n")
}

fn format_sqlite_artifact_summary(
    artifact: Option<&crate::lab::store::LabSqliteArtifactSummary>,
) -> String {
    artifact
        .map(|artifact| {
            format!(
                "{}:{} stage={} status={} validation={}",
                artifact.artifact_type,
                artifact.artifact_id,
                artifact.stage,
                artifact.status,
                artifact.validation_status.as_deref().unwrap_or("none")
            )
        })
        .unwrap_or_else(|| "none".to_string())
}
