//! LabRun slash-command surface.
//!
//! Commands translate user/TUI shell input into `LabStore`, `LabOrchestrator`,
//! scheduler, and provider-drafting operations. This module should present
//! honest status and recovery information without bypassing LabRun gates.

use crate::agent::types::AgentId;
use crate::lab::context::{
    build_lab_context_packet_with_evidence_retries_and_artifact_refs,
    evaluate_lab_context_compression, LabContextPacket,
};
use crate::lab::delegation::{build_graduate_task_dispatch, graduate_agent_task_id};
use crate::lab::model::{
    GraduateCleanupStatus, GraduateDispatchRecord, LabArtifactEnvelope, LabArtifactStatus,
    LabArtifactType, LabCloseoutStatus, LabCostSummary, LabDaemonMode, LabEvidenceKind,
    LabProviderCertificationKind, LabProviderCertificationOutcome, LabProviderCertificationRecord,
    LabRole, LabRunStatus, ProfessorSteeringDecision, SponsorMessageStatus, StageArtifact,
};
use crate::lab::orchestrator::LabOrchestrator;
use crate::lab::provider_certification::provider_certification_report;
use crate::lab::scheduler::{
    background_scheduler_status, default_background_interval_ms, default_background_max_steps,
    start_background_hybrid_cycle_scheduler, start_background_hybrid_scheduler,
    start_background_scheduler, stop_background_scheduler, LabHybridCycleBackgroundRequest,
};
use crate::lab::store::{LabCostTokens, LabEvidenceRefInput, LabStore};
use crate::services::api::{
    ChatRequest, ChatResponse, Message as ApiMessage, Tool as ApiTool, ToolChoice,
};
use crate::tools::{AgentTool, Tool, ToolContext, WorktreeTool};
use chrono::Utc;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

mod daemon;
mod provider;
mod scheduler;
mod sponsor;
mod task;
mod view;
#[cfg(test)]
use daemon::render_launchd_plist;
#[cfg(test)]
use provider::{
    hard_subagent_mutation_proof, lab_graduate_durable_smoke_details,
    recover_provider_compare_durable_subagent,
};

/// Handles the non-provider LabRun command surface.
pub fn handle_lab_command(
    project_root: &Path,
    current_session_id: Option<String>,
    args: &str,
) -> String {
    let orchestrator = LabOrchestrator::for_project(project_root);
    let store = orchestrator.store();
    let trimmed = args.trim();
    if trimmed.is_empty() || trimmed == "help" {
        return lab_help();
    }

    let (subcommand, rest) = split_once(trimmed);
    match subcommand {
        "propose" | "proposal" => match store.create_proposal(rest, current_session_id) {
            Ok(proposal) => format!(
                "Lab proposal created: {}\nStatus: {:?}\nApprove with /lab approve {}",
                proposal.proposal_id, proposal.status, proposal.proposal_id
            ),
            Err(err) => format!("Failed to create Lab proposal: {err}"),
        },
        "approve" => {
            let proposal_id = rest.trim();
            if proposal_id.is_empty() {
                return "Usage: /lab approve <proposal_id>".to_string();
            }
            match orchestrator.approve_proposal(proposal_id) {
                Ok(run) => format!(
                    "LabRun created: {}\nStage: {}\nStatus: {:?}\nState: {}",
                    run.lab_run_id,
                    run.current_stage,
                    run.status,
                    store
                        .root()
                        .join("runs")
                        .join(&run.lab_run_id)
                        .join("state.json")
                        .display()
                ),
                Err(err) => format!("Failed to approve proposal: {err}"),
            }
        }
        "start" => {
            if rest.trim().is_empty() {
                return "Usage: /lab start <goal>".to_string();
            }
            match store.create_proposal(rest, current_session_id) {
                Ok(proposal) => format!(
                    "Lab proposal drafted: {}\nFormal approval is required before LabRun work starts.\nApprove with /lab approve {}",
                    proposal.proposal_id, proposal.proposal_id
                ),
                Err(err) => format!("Failed to draft Lab proposal: {err}"),
            }
        }
        "status" => view::lab_status(store),
        "runs" => view::handle_runs_command(store),
        "recovery" | "recover" => handle_recovery_command(project_root, store),
        "report" | "reports" => view::handle_report_command(store, rest),
        "dashboard" => view::handle_dashboard_command(project_root, &orchestrator, store),
        "provider" | "providers" | "certification" => {
            if rest.trim().starts_with("record ") {
                "Usage: /lab provider record <control-plane|graduate> <passed|failed> <evidence_path> [summary] requires the Lab Mode shell provider. In non-interactive mode use `pa lab --command \"provider record ...\" --with-provider`."
            } else if matches!(
                rest.trim(),
                "compare" | "compare-agents" | "agents" | "subagents"
            ) {
                "Usage: /lab provider compare requires the Lab Mode shell provider. In non-interactive mode use `pa lab --command \"provider compare\" --with-provider`."
            } else if matches!(rest.trim(), "diagnose-tools" | "tools") {
                "Usage: /lab provider diagnose-tools requires the Lab Mode shell provider. In non-interactive mode use `pa lab --command \"provider diagnose-tools\" --with-provider`."
            } else {
                "Usage: /lab provider requires the Lab Mode shell provider. In non-interactive mode use `pa lab --command \"provider\" --with-provider`."
            }
                .to_string()
        }
        "step" if rest.trim().starts_with("llm") => {
            "Usage: /lab step llm [instructions] requires the Lab Mode shell provider. In non-interactive mode use `pa lab --command \"step llm ...\" --with-provider`."
                .to_string()
        }
        "run" if matches!(split_once(rest).0, "llm" | "hybrid" | "hybrid-cycles") => {
            "Usage: /lab run <llm|hybrid|hybrid-cycles> [limits] [instructions] requires the Lab Mode shell provider. In non-interactive mode use `pa lab --command \"run hybrid ...\" --with-provider`."
                .to_string()
        }
        "lifecycle" => handle_lifecycle_command(store),
        "daemon" => daemon::handle_daemon_command(project_root, store, rest),
        "cost" => handle_cost_command(store, rest),
        "context" => handle_context_command(&orchestrator, store, rest),
        "compression" => handle_compression_command(&orchestrator, store, rest),
        "compress" => handle_compress_command(&orchestrator, rest),
        "evidence" => handle_evidence_command(store, rest),
        "cycle" => handle_cycle_command(&orchestrator, rest),
        "blocker" | "blockers" => handle_blocker_command(&orchestrator, store, rest),
        "message" | "messages" | "sponsor" => {
            sponsor::handle_sponsor_messages_command(&orchestrator, store, rest)
        }
        "task" | "tasks" => {
            task::handle_task_command(project_root, &orchestrator, store, subcommand, rest)
        }
        "tick" => handle_tick_command(&orchestrator),
        "advance" => match orchestrator.advance_latest() {
            Ok(run) => format!(
                "Advanced LabRun {} to stage {} (owner {:?}).",
                run.lab_run_id, run.current_stage, run.internal_owner
            ),
            Err(err) => format!("Failed to advance LabRun: {err}"),
        },
        "continue" => match orchestrator.continue_latest_from_user_report(rest) {
            Ok(run) => format!(
                "Continued LabRun {} into cycle {} at stage {} (owner {:?}).",
                run.lab_run_id, run.cycle_count, run.current_stage, run.internal_owner
            ),
            Err(err) => format!("Failed to continue LabRun: {err}"),
        },
        "repair" | "revision" => match orchestrator.resume_postdoc_revision_latest(rest) {
            Ok(run) => format!(
                "Resumed LabRun {} for postdoc revision at stage {} (owner {:?}).",
                run.lab_run_id, run.current_stage, run.internal_owner
            ),
            Err(err) => format!("Failed to resume postdoc revision: {err}"),
        },
        "gate" => view::handle_gate_command(&orchestrator, rest),
        "plan" => match orchestrator.create_current_stage_artifact_for_latest(rest) {
            Ok(created) => format!(
                "Created {} artifact: {}\nGate satisfied for stage '{}'.\nArtifact: {}\nReport: {}",
                created.artifact.artifact_type().as_str(),
                created.artifact.artifact_id(),
                created.gate.stage,
                created.path.display(),
                created.report_path.display()
            ),
            Err(err) => format!("Failed to create Lab artifact: {err}"),
        },
        "integrate" => match orchestrator.create_postdoc_integration_summary_for_latest(Some(rest)) {
            Ok(created) => format!(
                "Created postdoc integration summary: {}\nGate: {} ({})\nArtifact: {}\nReport: {}",
                created.artifact.artifact_id(),
                created.gate.stage,
                if created.gate.is_satisfied() {
                    "satisfied"
                } else {
                    "blocked"
                },
                created.path.display(),
                created.report_path.display()
            ),
            Err(err) => format!("Failed to create postdoc integration summary: {err}"),
        },
        "professor-review" => match orchestrator.create_professor_review_for_latest(Some(rest)) {
            Ok(created) => format!(
                "Created professor review: {}\nGate: {} ({})\nArtifact: {}\nReport: {}",
                created.artifact.artifact_id(),
                created.gate.stage,
                if created.gate.is_satisfied() {
                    "satisfied"
                } else {
                    "blocked"
                },
                created.path.display(),
                created.report_path.display()
            ),
            Err(err) => format!("Failed to create professor review: {err}"),
        },
        "accept" => view::handle_artifact_accept_command(&orchestrator, rest),
        "revise" => view::handle_artifact_revise_command(&orchestrator, rest),
        "pause" => {
            let reason = if rest.trim().is_empty() {
                "user"
            } else {
                rest.trim()
            };
            match store.pause_latest_run(reason) {
                Ok(run) => format!(
                    "Paused LabRun {} at stage {}.",
                    run.lab_run_id, run.current_stage
                ),
                Err(err) => format!("Failed to pause LabRun: {err}"),
            }
        }
        "resume" => match store.resume_latest_run() {
            Ok(run) => format!(
                "Resumed LabRun {} at stage {}. No mutating work starts until an implementation action is approved.",
                run.lab_run_id, run.current_stage
            ),
            Err(err) => format!("Failed to resume LabRun: {err}"),
        },
        "closeout" => {
            let (status, note) = split_once(rest);
            if status.trim().is_empty() {
                return "Usage: /lab closeout <auto|verified|not_verified|partial|blocked|failed> [note]"
                    .to_string();
            }
            if status.eq_ignore_ascii_case("auto") {
                return match orchestrator.closeout_latest_from_user_report(note) {
                    Ok(run) => format!(
                        "LabRun closeout recorded from final evidence: {}\nStatus: {:?}\nCloseout: {:?}",
                        run.lab_run_id, run.status, run.closeout_status
                    ),
                    Err(err) => format!("Failed to close out LabRun from final evidence: {err}"),
                };
            }
            let closeout_status = match parse_closeout_status(status) {
                Ok(status) => status,
                Err(err) => return err,
            };
            match store.closeout_latest_run(closeout_status, note) {
                Ok(run) => format!(
                    "LabRun closeout recorded: {}\nStatus: {:?}\nCloseout: {:?}",
                    run.lab_run_id, run.status, run.closeout_status
                ),
                Err(err) => format!("Failed to close out LabRun: {err}"),
            }
        }
        "professor" | "note" => match store.append_sponsor_message(rest) {
            Ok(message) => format!(
                "Message queued for professor: {}\nStatus: {:?}",
                message.message_id, message.status
            ),
            Err(err) => format!("Failed to queue professor message: {err}"),
        },
        "intervene" => {
            if rest.trim().is_empty() {
                return "Usage: /lab intervene <message for professor>".to_string();
            }
            match store.intervene_latest_run(rest) {
                Ok((run, message)) => format!(
                    "LabRun intervention queued: {}\nMessage: {}\nStatus: {:?}\nRun status: {:?}",
                    run.lab_run_id, message.message_id, message.status, run.status
                ),
                Err(err) => format!("Failed to record LabRun intervention: {err}"),
            }
        }
        "meeting" => {
            view::handle_meeting_command(project_root, &orchestrator, rest)
        }
        "open" => {
            let id = rest.trim();
            if id.is_empty() {
                return "Usage: /lab open <lab_run_id>".to_string();
            }
            match store.open_run_pointer(id) {
                Ok(run) => format!(
                    "Opened LabRun {} for inspection.\nStatus: {:?}\nStage: {}\nState: {}\nNo mutating work starts until /lab resume, /lab run, or another implementation action is used.",
                    run.lab_run_id,
                    run.status,
                    run.current_stage,
                    store
                        .root()
                        .join("runs")
                        .join(&run.lab_run_id)
                        .join("state.json")
                        .display()
                ),
                Err(err) => format!("Failed to open LabRun: {err}"),
            }
        }
        "close" => match store.closeout_latest_run(LabCloseoutStatus::Cancelled, "closed_by_user") {
            Ok(run) => format!("Closed LabRun {}.", run.lab_run_id),
            Err(err) => format!("Failed to close LabRun: {err}"),
        },
        "review" => view::handle_review_command(&orchestrator, store, rest),
        _ => format!("Unknown /lab command: {subcommand}\n\n{}", lab_help()),
    }
}

/// Handles LabRun commands that need provider or tool context.
pub async fn handle_lab_command_with_context(
    project_root: &Path,
    current_session_id: Option<String>,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let trimmed = args.trim();
    if let Some(rest) = trimmed.strip_prefix("task worktree ") {
        return task::handle_task_worktree_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("tasks worktree ") {
        return task::handle_task_worktree_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("task run ") {
        return task::handle_task_run_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("tasks run ") {
        return task::handle_task_run_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("task sync ") {
        return task::handle_task_sync_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("tasks sync ") {
        return task::handle_task_sync_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("messages classify ") {
        return sponsor::handle_sponsor_message_classify_command(project_root, rest, tool_context)
            .await;
    }
    if let Some(rest) = trimmed.strip_prefix("message classify ") {
        return sponsor::handle_sponsor_message_classify_command(project_root, rest, tool_context)
            .await;
    }
    if let Some(rest) = trimmed.strip_prefix("sponsor classify ") {
        return sponsor::handle_sponsor_message_classify_command(project_root, rest, tool_context)
            .await;
    }
    if let Some(rest) = trimmed.strip_prefix("propose llm ") {
        return handle_proposal_llm_command(project_root, current_session_id, rest, tool_context)
            .await;
    }
    if let Some(rest) = trimmed.strip_prefix("proposal llm ") {
        return handle_proposal_llm_command(project_root, current_session_id, rest, tool_context)
            .await;
    }
    if trimmed == "meeting llm" {
        return handle_meeting_llm_command(project_root, "", tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("meeting llm ") {
        return handle_meeting_llm_command(project_root, rest, tool_context).await;
    }
    if matches!(
        trimmed,
        "provider compare"
            | "providers compare"
            | "certification compare"
            | "provider compare-agents"
            | "providers compare-agents"
            | "certification compare-agents"
    ) {
        return provider::handle_provider_compare_command(project_root, tool_context).await;
    }
    if let Some(rest) = trimmed
        .strip_prefix("provider record ")
        .or_else(|| trimmed.strip_prefix("providers record "))
        .or_else(|| trimmed.strip_prefix("certification record "))
    {
        return provider::handle_provider_record_command(project_root, rest, tool_context);
    }
    if matches!(
        trimmed,
        "provider diagnose-tools"
            | "providers diagnose-tools"
            | "certification diagnose-tools"
            | "provider tools"
            | "providers tools"
    ) {
        return provider::handle_provider_tool_diagnostics_command(tool_context).await;
    }
    if matches!(trimmed, "provider" | "providers" | "certification") {
        return provider::handle_provider_command(project_root, tool_context);
    }
    if trimmed == "step llm" {
        return handle_provider_stage_step_command(project_root, "", tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("step llm ") {
        return handle_provider_stage_step_command(project_root, rest, tool_context).await;
    }
    if trimmed == "step" {
        return scheduler::handle_scheduler_step_command(project_root, tool_context).await;
    }
    if trimmed == "run" || trimmed.starts_with("run ") {
        let args = trimmed.strip_prefix("run").unwrap_or("").trim();
        return scheduler::handle_scheduler_run_command(project_root, args, tool_context).await;
    }
    if trimmed == "background" || trimmed.starts_with("background ") {
        let args = trimmed.strip_prefix("background").unwrap_or("").trim();
        return scheduler::handle_background_command(project_root, args, tool_context).await;
    }
    handle_lab_command(project_root, current_session_id, args)
}

async fn handle_proposal_llm_command(
    project_root: &Path,
    current_session_id: Option<String>,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let user_goal = args.trim();
    if user_goal.is_empty() {
        return "Usage: /lab propose llm <project idea>".to_string();
    }
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab propose llm <project idea> requires an active Lab Mode provider."
            .to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab propose llm <project idea> requires an active model.".to_string();
    }
    match crate::lab::draft::draft_lab_proposal_with_provider(
        project_root,
        provider,
        tool_context.model,
        user_goal,
        current_session_id,
    )
    .await
    {
        Ok(outcome) => format!(
            "Professor drafted Lab proposal: {}\nRecommended mode: {:?}\nProblem: {}\nDesired outcome: {}\nSuccess criteria: {}\nFormal approval is required before LabRun work starts.\nApprove with /lab approve {}",
            outcome.proposal.proposal_id,
            outcome.proposal.recommended_mode,
            outcome.proposal.problem_statement,
            outcome.proposal.desired_outcome,
            if outcome.proposal.success_criteria.is_empty() {
                "none".to_string()
            } else {
                outcome.proposal.success_criteria.join("; ")
            },
            outcome.proposal.proposal_id
        ),
        Err(err) => format!(
            "Failed to draft professor Lab proposal: {}",
            format_error_chain(&err)
        ),
    }
}

async fn handle_meeting_llm_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab meeting llm [topic] requires an active Lab Mode provider.".to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab meeting llm [topic] requires an active model.".to_string();
    }
    let topic = args.trim();
    let topic = (!topic.is_empty()).then_some(topic);
    match crate::lab::draft::draft_lab_meeting_with_provider(
        project_root,
        provider,
        tool_context.model,
        topic,
    )
    .await
    {
        Ok(outcome) => format!(
            "Provider Lab meeting summary created: {}\nThis meeting is read-only and does not mutate code.\nArtifact: {}\nReport: {}\nUsage recorded: {}",
            outcome.created.artifact.artifact_id(),
            outcome.created.path.display(),
            outcome.created.report_path.display(),
            outcome.usage.is_some()
        ),
        Err(err) => format!(
            "Failed to draft provider Lab meeting: {}",
            format_error_chain(&err)
        ),
    }
}

async fn handle_provider_stage_step_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab step llm [instructions] requires an active Lab Mode provider."
            .to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab step llm [instructions] requires an active model.".to_string();
    }
    match crate::lab::draft::run_provider_stage_step(
        project_root,
        provider,
        tool_context.model,
        args.trim(),
    )
    .await
    {
        Ok(outcome) => format!(
            "Provider Lab step: {}\nFrom: {}\nTo: {}\nArtifact: {}\nReview: {:?} ({})\nAdvanced: {}",
            outcome.lab_run_id,
            outcome.from_stage,
            outcome.to_stage,
            outcome.artifact_id,
            outcome.review_decision,
            outcome.review_note,
            outcome.advanced
        ),
        Err(err) => format!(
            "Failed to run provider Lab step: {}",
            format_error_chain(&err)
        ),
    }
}

fn handle_lifecycle_command(store: &LabStore) -> String {
    match store.load_app_lifecycle_state() {
        Ok(Some(state)) => {
            let mut lines = vec![
                format!("Lab app lifecycle: {}", state.project_root),
                format!("Launch mode: {}", state.launch_mode),
                format!("Process id: {}", state.process_id),
                format!(
                    "Last startup: {}",
                    state
                        .last_startup_at
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_else(|| "none".to_string())
                ),
                format!(
                    "Last shutdown: {}",
                    state
                        .last_shutdown_at
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_else(|| "none".to_string())
                ),
            ];
            if let Some(lab_run_id) = state.recovered_scheduler_lab_run_id {
                lines.push(format!(
                    "Recovered scheduler: {} ({:?})",
                    lab_run_id, state.recovered_scheduler_status
                ));
            } else {
                lines.push("Recovered scheduler: none".to_string());
            }
            if let Some(lab_run_id) = state.shutdown_paused_lab_run_id {
                lines.push(format!("Shutdown paused LabRun: {lab_run_id}"));
            } else {
                lines.push("Shutdown paused LabRun: none".to_string());
            }
            if let Some(message) = state.last_message {
                lines.push(format!("Last message: {message}"));
            }
            lines.join("\n")
        }
        Ok(None) => "No Lab app lifecycle state found.".to_string(),
        Err(err) => format!("Failed to read Lab app lifecycle: {err}"),
    }
}

fn handle_recovery_command(project_root: &Path, store: &LabStore) -> String {
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found for recovery.".to_string(),
        Err(err) => return format!("Failed to read LabRun recovery state: {err}"),
    };
    let tasks = match store.list_graduate_tasks(&run.lab_run_id) {
        Ok(tasks) => tasks,
        Err(err) => return format!("Failed to read LabRun recovery tasks: {err}"),
    };
    let dispatches = match store.list_graduate_dispatches(&run.lab_run_id) {
        Ok(dispatches) => dispatches,
        Err(err) => return format!("Failed to read LabRun recovery dispatches: {err}"),
    };
    let open_tasks = tasks
        .iter()
        .filter(|task| task.status.is_open())
        .map(|task| task.task_id.as_str())
        .collect::<Vec<_>>();
    let scheduler_line = match background_scheduler_status(project_root) {
        Ok(status) => match status.persisted {
            Some(state) => format!(
                "Scheduler: {:?} steps={}/{} reason={}",
                state.status,
                state.steps_completed,
                state.max_steps,
                state.stop_reason.as_deref().unwrap_or("none")
            ),
            None => "Scheduler: none".to_string(),
        },
        Err(err) => format!("Scheduler: unavailable ({err})"),
    };
    let lifecycle_line = match store.load_app_lifecycle_state() {
        Ok(Some(state)) => format!(
            "Lifecycle: startup={} shutdown={} message={}",
            state
                .last_startup_at
                .map(|time| time.to_rfc3339())
                .unwrap_or_else(|| "none".to_string()),
            state
                .last_shutdown_at
                .map(|time| time.to_rfc3339())
                .unwrap_or_else(|| "none".to_string()),
            state.last_message.unwrap_or_else(|| "none".to_string())
        ),
        Ok(None) => "Lifecycle: none".to_string(),
        Err(err) => format!("Lifecycle: unavailable ({err})"),
    };
    let recovery_state = if matches!(
        run.status,
        LabRunStatus::Paused | LabRunStatus::PausedShutdown | LabRunStatus::NeedsUser
    ) {
        "available"
    } else if matches!(run.status, LabRunStatus::Active) {
        "not needed; latest LabRun is active"
    } else if matches!(run.status, LabRunStatus::Blocked) {
        "blocked; inspect blocker before resuming work"
    } else {
        "not available for this status"
    };
    let mut lines = vec![
        format!("Lab recovery: {}", run.lab_run_id),
        format!(
            "Run: status={:?} stage={} owner={:?} needs_user={}",
            run.status, run.current_stage, run.internal_owner, run.needs_user
        ),
        format!(
            "Recovery: {} pause_reason={} paused_at={}",
            recovery_state,
            run.pause_reason.as_deref().unwrap_or("none"),
            run.paused_at
                .map(|time| time.to_rfc3339())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "Resume cursor: stage={} owner={:?} artifact={} open_tasks={}",
            run.resume_cursor.current_stage,
            run.resume_cursor.internal_owner,
            run.resume_cursor
                .active_artifact_id
                .as_deref()
                .unwrap_or("none"),
            if open_tasks.is_empty() {
                "none".to_string()
            } else {
                open_tasks.join(",")
            }
        ),
        format!(
            "Lease: id={} owner={}",
            run.lease_id.as_deref().unwrap_or("none"),
            run.lease_owner.as_deref().unwrap_or("none")
        ),
        scheduler_line,
        lifecycle_line,
    ];
    lines.extend(view::graduate_cleanup_state_lines(&dispatches, 5));
    lines.push("Options:".to_string());
    if matches!(
        run.status,
        LabRunStatus::Paused | LabRunStatus::PausedShutdown | LabRunStatus::NeedsUser
    ) {
        lines.push("  Continue: /lab resume".to_string());
    }
    lines.push("  Inspect: /lab dashboard".to_string());
    lines.push("  Keep paused: no action".to_string());
    lines.push("  Close/cancel: /lab close or /lab closeout blocked <note>".to_string());
    lines.join("\n")
}

fn lab_help() -> String {
    [
        "Lab commands:",
        "  /lab propose <idea>          Draft professor intake proposal",
        "  /lab propose llm <idea>      Ask Professor provider to structure intake proposal",
        "  /lab approve <proposal_id>   Formally create LabRun",
        "  /lab start <goal>            Shortcut: draft proposal, still requires approval",
        "  /lab status                  Show latest proposal or LabRun",
        "  /lab runs                    List recent LabRuns",
        "  /lab provider                Show active provider Lab diagnostics",
        "  /lab provider compare        Compare generic subagent vs Lab graduate on the same provider",
        "  /lab provider diagnose-tools Run direct provider function-call probes",
        "  /lab recovery                Show paused/recoverable LabRun options",
        "  /lab report [list|latest]    Show latest generated Lab report preview",
        "  /lab dashboard               Show LabRun status panel summary",
        "  /lab lifecycle               Show app-owned Lab lifecycle state",
        "  /lab daemon [status|health|enable [strict|hybrid|hybrid-cycles] [max_steps] [max_steps_per_cycle] [interval_ms] [instructions]|start|launchd [label]|service [status|install|uninstall|load|unload|restart|supervise|commands] [label]|disable]",
        "                               Persist app-owned background daemon policy",
        "  /lab cost                    Show LabRun token/cache/cost summary",
        "  /lab cost record <role> <model> <prompt> <completion> [reasoning] [cached] [cache_write] [cost] [note]",
        "  /lab context [role]          Show LabRun context packet fingerprints",
        "  /lab compression [role]      Evaluate and record context compression decision",
        "  /lab compress [role]         Write compression summary when recommended/required",
        "  /lab evidence add <kind> <ref> <summary>",
        "  /lab evidence list           Show refs-only evidence index",
        "  /lab cycle summary <text>    Write cycle summary artifact and report",
        "  /lab blocker [status|report [note]|escalate]",
        "  /lab integrate [note]        Summarize GraduateResult artifacts for postdoc_review",
        "  /lab professor-review [note] Create final professor review from postdoc evidence",
        "  /lab professor-review llm [instructions]",
        "                               Ask provider Professor to review postdoc evidence",
        "  /lab task list               Show graduate tasks",
        "  /lab task create <title> | <scope_csv> | <validation_csv> | <instructions>",
        "  /lab task start <task_id>",
        "  /lab task complete <task_id> <result_artifact_id> [evidence_csv]",
        "  /lab task result <task_id> | <changed_csv> | <validation_csv> | <blockers_csv> | <evidence_csv> | <summary>",
        "  /lab task bind-json <task_id> <json_file>",
        "                               Bind graduate agent JSON into a GraduateResult artifact",
        "  /lab task block <task_id> <reason>",
        "  /lab task revise <task_id> | <scope_csv> | <validation_csv> | [instructions]",
        "  /lab task retry <task_id> | <validation_summary>",
        "  /lab task cancel <task_id> [reason]",
        "  /lab task envelope <task_id>      Render graduate task agent envelope",
        "  /lab task dispatch <task_id>      Persist prepared graduate dispatch",
        "  /lab task run <task_id>           Run graduate task through runtime agent tool",
        "  /lab task sync <task_id>          Sync completed durable graduate subagent result",
        "  /lab task worktree <review|merge|cleanup> <task_id> [force]",
        "  /lab step                    Run one strict scheduler step",
        "  /lab step llm [instructions] Run one provider-backed draft/review/advance step",
        "  /lab run [max_steps]         Run bounded strict scheduler steps",
        "  /lab run llm [max_steps] [instructions]",
        "                               Run provider-backed stages until a boundary",
        "  /lab run hybrid [max_steps] [instructions]",
        "                               Use provider stages plus strict graduate scheduler",
        "  /lab run hybrid-cycles [max_cycles] [max_steps_per_cycle] [instructions]",
        "                               Explicitly continue bounded hybrid cycles after user_report",
        "  /lab background [status|start [max_steps] [interval_ms]|stop|recover]",
        "  /lab background hybrid [max_steps] [interval_ms] [instructions]",
        "                               Run provider-backed hybrid scheduler in process",
        "  /lab background hybrid-cycles [max_cycles] [max_steps_per_cycle] [interval_ms] [instructions]",
        "                               Explicitly run bounded provider-backed hybrid cycles in process",
        "  /lab tick                    Run one deterministic LabRun orchestration step",
        "  /lab plan <note>             Create current-stage artifact and satisfy its gate",
        "  /lab draft [instructions]    Ask current provider to draft current-stage artifact",
        "  /lab accept <artifact_id> [note]",
        "  /lab revise <artifact_id> <note>",
        "  /lab review                 Show current LabRun review summary",
        "  /lab review artifact <artifact_id> [instructions]",
        "  /lab gate                    Show required gate for current stage",
        "  /lab gate satisfy <artifact_id> [validation_status] [evidence_ref]",
        "  /lab advance                 Advance only after current gate is satisfied",
        "  /lab continue [note]         Start the next cycle from user_report",
        "  /lab repair [note]           Resume pending professor revision at postdoc_plan",
        "  /lab professor <message>     Send sponsor message to professor",
        "  /lab note <message>          Alias for /lab professor",
        "  /lab intervene <message>     Pause LabRun and queue urgent professor message",
        "  /lab messages                List professor side-channel messages",
        "  /lab messages <classify|decision|review|meeting|task|reject|apply> <message_id> [note]",
        "  /lab meeting [topic]         Write read-only lab meeting summary",
        "  /lab meeting llm [topic]     Ask provider to draft read-only professor/postdoc meeting summary",
        "  /lab meeting recommend       Show runtime escalation signals",
        "  /lab meeting open [topic]    Open recommended or explicit read-only meeting",
        "  /lab pause [reason]          Pause latest LabRun",
        "  /lab resume                  Resume latest LabRun state",
        "  /lab closeout <auto|verified|not_verified|partial|blocked|failed> [note]",
        "  /lab open <lab_run_id>       Open LabRun for inspection without resuming",
        "  /lab close                   Cancel latest LabRun",
    ]
    .join("\n")
}

fn handle_cycle_command(orchestrator: &LabOrchestrator, args: &str) -> String {
    let trimmed = args.trim();
    let Some(summary) = trimmed.strip_prefix("summary ") else {
        return "Usage: /lab cycle summary <text>".to_string();
    };
    if summary.trim().is_empty() {
        return "Usage: /lab cycle summary <text>".to_string();
    }
    match orchestrator.create_cycle_summary_for_latest(summary) {
        Ok(created) => format!(
            "Created cycle summary: {}\nArtifact: {}\nReport: {}",
            created.artifact.artifact_id(),
            created.path.display(),
            created.report_path.display()
        ),
        Err(err) => format!("Failed to create cycle summary: {err}"),
    }
}

fn handle_blocker_command(orchestrator: &LabOrchestrator, store: &LabStore, args: &str) -> String {
    let (action, rest) = split_once(args);
    match action {
        "" | "status" => match store.latest_run() {
            Ok(Some(run)) => {
                let tasks = match store.list_graduate_tasks(&run.lab_run_id) {
                    Ok(tasks) => tasks,
                    Err(err) => return format!("Failed to read graduate tasks: {err}"),
                };
                let blocked = tasks
                    .iter()
                    .filter(|task| matches!(task.status, crate::lab::model::LabTaskStatus::Blocked))
                    .count();
                let retries = match store.list_validation_retries(&run.lab_run_id) {
                    Ok(retries) => retries,
                    Err(err) => return format!("Failed to read validation retries: {err}"),
                };
                let escalated = retries.iter().filter(|retry| retry.escalated).count();
                format!(
                    "Lab blockers for {}: blocked_tasks={} validation_retries={} escalated_retries={} failure_count={} blocked_reason={}",
                    run.lab_run_id,
                    blocked,
                    retries.len(),
                    escalated,
                    run.failure_count,
                    run.blocked_reason.as_deref().unwrap_or("none")
                )
            }
            Ok(None) => "No LabRun found.".to_string(),
            Err(err) => format!("Failed to read Lab blockers: {err}"),
        },
        "report" => {
            let note = (!rest.trim().is_empty()).then_some(rest);
            match orchestrator.create_blocker_report_for_latest(note) {
                Ok(created) => format!(
                    "Lab blocker report created: {}\nArtifact: {}\nReport: {}",
                    created.artifact.artifact_id(),
                    created.path.display(),
                    created.report_path.display()
                ),
                Err(err) => format!("Failed to create Lab blocker report: {err}"),
            }
        }
        "escalate" => match orchestrator.escalate_latest_blocker_to_professor_review() {
            Ok(run) => format!(
                "Escalated Lab blocker to professor review: {}\nStage: {}\nOwner: {:?}",
                run.lab_run_id, run.current_stage, run.internal_owner
            ),
            Err(err) => format!("Failed to escalate Lab blocker: {err}"),
        },
        _ => "Usage: /lab blocker [status|report [note]|escalate]".to_string(),
    }
}

fn handle_tick_command(orchestrator: &LabOrchestrator) -> String {
    match orchestrator.tick_latest() {
        Ok(result) => {
            let mut lines = vec![
                format!("Lab tick: {:?}", result.status),
                format!("LabRun: {}", result.lab_run_id),
                format!("Stage: {} -> {}", result.from_stage, result.to_stage),
                format!("Owner: {:?}", result.owner),
            ];
            if let Some(artifact_id) = result.artifact_id {
                lines.push(format!("Artifact: {artifact_id}"));
            }
            if let Some(report_path) = result.report_path {
                lines.push(format!("Report: {}", report_path.display()));
            }
            if let Some(compression_artifact_id) = result.compression_artifact_id {
                lines.push(format!("Compression: {compression_artifact_id}"));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to tick LabRun: {err}"),
    }
}

fn handle_context_command(orchestrator: &LabOrchestrator, store: &LabStore, args: &str) -> String {
    let role = if args.trim().is_empty() {
        LabRole::Professor
    } else {
        match parse_lab_role(args.trim()) {
            Ok(role) => role,
            Err(err) => return err,
        }
    };
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found for context packet.".to_string(),
        Err(err) => return format!("Failed to read LabRun for context packet: {err}"),
    };
    let cost = match store.cost_summary(&run.lab_run_id) {
        Ok(summary) => summary,
        Err(err) => return format!("Failed to read Lab cost summary: {err}"),
    };
    let evidence = match store.list_evidence_refs(&run.lab_run_id) {
        Ok(evidence) => evidence,
        Err(err) => return format!("Failed to read Lab evidence refs: {err}"),
    };
    let validation_retries = match store.list_validation_retries(&run.lab_run_id) {
        Ok(retries) => retries,
        Err(err) => return format!("Failed to read Lab validation retries: {err}"),
    };
    let artifact_gate_refs =
        match orchestrator.artifact_gate_evidence_context_for_run(&run.lab_run_id, 20) {
            Ok(refs) => refs,
            Err(err) => return format!("Failed to read Lab artifact/gate evidence refs: {err}"),
        };
    format_context_packet(
        &build_lab_context_packet_with_evidence_retries_and_artifact_refs(
            &run,
            role,
            &cost,
            &evidence,
            &validation_retries,
            &artifact_gate_refs,
        ),
    )
}

fn format_context_packet(packet: &LabContextPacket) -> String {
    let mut lines = vec![
        format!("Lab context packet: {}", packet.lab_run_id),
        format!("Role: {:?}", packet.role),
        format!(
            "Stable prefix: hash={} tokens={}",
            packet.stable_prefix_fingerprint, packet.stable_prefix_tokens
        ),
        format!(
            "Dynamic tail: hash={} tokens={}",
            packet.dynamic_tail_fingerprint, packet.dynamic_tail_tokens
        ),
        format!("Total estimated tokens: {}", packet.total_estimated_tokens),
        "Layers:".to_string(),
    ];
    for layer in &packet.layers {
        lines.push(format!(
            "  {} {} {:?} tokens={}",
            layer.layer, layer.label, layer.stability, layer.estimated_tokens
        ));
    }
    lines.join("\n")
}

fn handle_compression_command(
    orchestrator: &LabOrchestrator,
    store: &LabStore,
    args: &str,
) -> String {
    let role = if args.trim().is_empty() {
        LabRole::Professor
    } else {
        match parse_lab_role(args.trim()) {
            Ok(role) => role,
            Err(err) => return err,
        }
    };
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found for compression decision.".to_string(),
        Err(err) => return format!("Failed to read LabRun for compression decision: {err}"),
    };
    let cost = match store.cost_summary(&run.lab_run_id) {
        Ok(summary) => summary,
        Err(err) => return format!("Failed to read Lab cost summary: {err}"),
    };
    let evidence = match store.list_evidence_refs(&run.lab_run_id) {
        Ok(evidence) => evidence,
        Err(err) => return format!("Failed to read Lab evidence refs: {err}"),
    };
    let artifact_gate_refs =
        match orchestrator.artifact_gate_evidence_context_for_run(&run.lab_run_id, 20) {
            Ok(refs) => refs,
            Err(err) => return format!("Failed to read Lab artifact/gate evidence refs: {err}"),
        };
    let packet = build_lab_context_packet_with_evidence_retries_and_artifact_refs(
        &run,
        role,
        &cost,
        &evidence,
        &[],
        &artifact_gate_refs,
    );
    let decision = evaluate_lab_context_compression(&run, &packet);
    match store.record_compression_decision(decision) {
        Ok(decision) => format!(
            "Lab compression decision: {} role={:?} action={:?}\npacket_tokens={} budget={} usage={:.1}%\nstable_hash={} dynamic_hash={}\n{}",
            decision.decision_id,
            decision.role,
            decision.action,
            decision.packet_tokens,
            decision.context_budget_tokens,
            decision.usage_ratio_percent,
            decision.stable_prefix_fingerprint,
            decision.dynamic_tail_fingerprint,
            decision.reason
        ),
        Err(err) => format!("Failed to record Lab compression decision: {err}"),
    }
}

fn handle_compress_command(orchestrator: &LabOrchestrator, args: &str) -> String {
    let role = if args.trim().is_empty() {
        LabRole::Professor
    } else {
        match parse_lab_role(args.trim()) {
            Ok(role) => role,
            Err(err) => return err,
        }
    };
    match orchestrator.create_compression_summary_for_latest(role) {
        Ok(Some(created)) => format!(
            "Created compression summary: {}\nArtifact: {}\nReport: {}",
            created.artifact.artifact_id(),
            created.path.display(),
            created.report_path.display()
        ),
        Ok(None) => "No compression needed for current LabRun context.".to_string(),
        Err(err) => format!("Failed to create compression summary: {err}"),
    }
}

fn handle_evidence_command(store: &LabStore, args: &str) -> String {
    let trimmed = args.trim();
    if trimmed.is_empty() || trimmed == "list" {
        return list_evidence_refs(store);
    }
    let Some(rest) = trimmed.strip_prefix("add ") else {
        return "Usage: /lab evidence [list|add <kind> <ref> <summary>]".to_string();
    };
    let (kind, rest) = split_once(rest);
    let (reference, summary) = split_once(rest);
    if kind.is_empty() || reference.is_empty() || summary.is_empty() {
        return "Usage: /lab evidence add <kind> <ref> <summary>".to_string();
    }
    let kind = match parse_evidence_kind(kind) {
        Ok(kind) => kind,
        Err(err) => return err,
    };
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found for evidence ref.".to_string(),
        Err(err) => return format!("Failed to read LabRun for evidence ref: {err}"),
    };
    let cycle_id = run.cycle_count.to_string();
    match store.record_evidence_ref(LabEvidenceRefInput {
        lab_run_id: &run.lab_run_id,
        kind,
        role: run.internal_owner,
        reference,
        summary,
        artifact_id: run.resume_cursor.active_artifact_id.as_deref(),
        cycle_id: Some(&cycle_id),
    }) {
        Ok(evidence) => format!(
            "Recorded Lab evidence ref: {} kind={:?} ref={} hash={}",
            evidence.evidence_id,
            evidence.kind,
            evidence.reference,
            evidence.metadata_hash.as_deref().unwrap_or("none")
        ),
        Err(err) => format!("Failed to record Lab evidence ref: {err}"),
    }
}

fn list_evidence_refs(store: &LabStore) -> String {
    let evidence = match store.latest_evidence_refs() {
        Ok(evidence) => evidence,
        Err(err) => return format!("Failed to read Lab evidence refs: {err}"),
    };
    if evidence.is_empty() {
        return "No Lab evidence refs recorded.".to_string();
    }
    let mut lines = vec![format!("Lab evidence refs: {}", evidence.len())];
    for item in evidence.iter().rev().take(20).rev() {
        lines.push(format!(
            "{} {:?} {:?} ref={} summary={} hash={}",
            item.evidence_id,
            item.kind,
            item.role,
            item.reference,
            item.summary,
            item.metadata_hash.as_deref().unwrap_or("none")
        ));
    }
    lines.join("\n")
}

fn handle_cost_command(store: &LabStore, args: &str) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        return match store.latest_cost_summary() {
            Ok(Some(summary)) => format_cost_summary(&summary),
            Ok(None) => "No LabRun found for cost summary.".to_string(),
            Err(err) => format!("Failed to read Lab cost summary: {err}"),
        };
    }

    match parts.as_slice() {
        ["record", role, model, prompt, completion] => record_cost_usage(
            store,
            role,
            model,
            prompt,
            completion,
            None,
            None,
            None,
            None,
            None,
        ),
        ["record", role, model, prompt, completion, reasoning] => record_cost_usage(
            store,
            role,
            model,
            prompt,
            completion,
            Some(reasoning),
            None,
            None,
            None,
            None,
        ),
        ["record", role, model, prompt, completion, reasoning, cached] => record_cost_usage(
            store,
            role,
            model,
            prompt,
            completion,
            Some(reasoning),
            Some(cached),
            None,
            None,
            None,
        ),
        ["record", role, model, prompt, completion, reasoning, cached, cache_write] => {
            record_cost_usage(
                store,
                role,
                model,
                prompt,
                completion,
                Some(reasoning),
                Some(cached),
                Some(cache_write),
                None,
                None,
            )
        }
        [
            "record",
            role,
            model,
            prompt,
            completion,
            reasoning,
            cached,
            cache_write,
            cost,
            note @ ..,
        ] => record_cost_usage(
            store,
            role,
            model,
            prompt,
            completion,
            Some(reasoning),
            Some(cached),
            Some(cache_write),
            Some(cost),
            Some(&note.join(" ")),
        ),
        _ => "Usage: /lab cost [record <role> <model> <prompt> <completion> [reasoning] [cached] [cache_write] [cost] [note]]".to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn record_cost_usage(
    store: &LabStore,
    role: &str,
    model: &str,
    prompt: &str,
    completion: &str,
    reasoning: Option<&str>,
    cached: Option<&str>,
    cache_write: Option<&str>,
    cost: Option<&str>,
    note: Option<&str>,
) -> String {
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found for cost usage.".to_string(),
        Err(err) => return format!("Failed to read LabRun for cost usage: {err}"),
    };
    let role = match parse_lab_role(role) {
        Ok(role) => role,
        Err(err) => return err,
    };
    let prompt_tokens = match parse_u64("prompt", prompt) {
        Ok(value) => value,
        Err(err) => return err,
    };
    let completion_tokens = match parse_u64("completion", completion) {
        Ok(value) => value,
        Err(err) => return err,
    };
    let reasoning_tokens = match parse_optional_u64("reasoning", reasoning) {
        Ok(value) => value,
        Err(err) => return err,
    };
    let cached_tokens = match parse_optional_u64("cached", cached) {
        Ok(value) => value,
        Err(err) => return err,
    };
    let cache_write_tokens = match parse_optional_u64("cache_write", cache_write) {
        Ok(value) => value,
        Err(err) => return err,
    };
    let estimated_cost_usd = match cost {
        Some(value) => match value.parse::<f64>() {
            Ok(value) if value.is_finite() && value >= 0.0 => value,
            _ => return format!("Invalid cost: {value}"),
        },
        None => 0.0,
    };
    let tokens = LabCostTokens {
        prompt_tokens,
        completion_tokens,
        reasoning_tokens,
        cached_tokens,
        cache_write_tokens,
        cycle_id: Some(run.cycle_count.to_string()),
        meeting_id: None,
    };
    match store.record_cost_usage(
        &run.lab_run_id,
        role,
        model,
        tokens,
        estimated_cost_usd,
        note,
    ) {
        Ok(usage) => format!(
            "Recorded Lab cost usage: {} role={:?} total={} cached={} cache_write={} miss={} cost=${:.6}",
            usage.usage_id,
            usage.role,
            usage.total_tokens,
            usage.cached_tokens,
            usage.cache_write_tokens,
            usage.cache_miss_tokens,
            usage.estimated_cost_usd
        ),
        Err(err) => format!("Failed to record Lab cost usage: {err}"),
    }
}

fn format_cost_summary(summary: &LabCostSummary) -> String {
    let mut lines = vec![
        format!("LabRun cost summary: {}", summary.lab_run_id),
        format!("Requests: {}", summary.requests),
        format!(
            "Tokens: total={} prompt={} completion={} reasoning={}",
            summary.total_tokens,
            summary.prompt_tokens,
            summary.completion_tokens,
            summary.reasoning_tokens
        ),
        format!(
            "Cache: cached={} write={} miss={} hit_rate={:.1}%",
            summary.cached_tokens,
            summary.cache_write_tokens,
            summary.cache_miss_tokens,
            summary.cache_hit_rate_percent()
        ),
        format!("Estimated cost: ${:.6}", summary.estimated_cost_usd),
    ];
    if summary.by_role.is_empty() {
        lines.push("By role: no usage recorded".to_string());
    } else {
        lines.push("By role:".to_string());
        for role in &summary.by_role {
            lines.push(format!(
                "  {:?}: requests={} total={} prompt={} completion={} reasoning={} cached={} write={} miss={} cost=${:.6}",
                role.role,
                role.requests,
                role.total_tokens,
                role.prompt_tokens,
                role.completion_tokens,
                role.reasoning_tokens,
                role.cached_tokens,
                role.cache_write_tokens,
                role.cache_miss_tokens,
                role.estimated_cost_usd
            ));
        }
    }
    lines.join("\n")
}

fn parse_lab_role(value: &str) -> Result<LabRole, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "professor" => Ok(LabRole::Professor),
        "postdoc" => Ok(LabRole::Postdoc),
        "graduate" => Ok(LabRole::Graduate),
        "runtime" => Ok(LabRole::Runtime),
        _ => Err(format!(
            "Invalid Lab role: {value}. Use professor, postdoc, graduate, or runtime."
        )),
    }
}

fn parse_evidence_kind(value: &str) -> Result<LabEvidenceKind, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "file" => Ok(LabEvidenceKind::File),
        "diff" => Ok(LabEvidenceKind::Diff),
        "log" => Ok(LabEvidenceKind::Log),
        "command" => Ok(LabEvidenceKind::Command),
        "artifact" => Ok(LabEvidenceKind::Artifact),
        "url" => Ok(LabEvidenceKind::Url),
        "note" => Ok(LabEvidenceKind::Note),
        _ => Err(format!(
            "Invalid evidence kind: {value}. Use file, diff, log, command, artifact, url, or note."
        )),
    }
}

fn parse_closeout_status(value: &str) -> Result<LabCloseoutStatus, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "verified" | "completed_verified" | "complete_verified" => {
            Ok(LabCloseoutStatus::CompletedVerified)
        }
        "not_verified" | "completed_not_verified" | "complete_not_verified" | "unverified" => {
            Ok(LabCloseoutStatus::CompletedNotVerified)
        }
        "partial" | "partially_completed" => Ok(LabCloseoutStatus::Partial),
        "blocked" | "blocked_needs_user" | "needs_user" => {
            Ok(LabCloseoutStatus::BlockedNeedsUser)
        }
        "failed" | "failure" => Ok(LabCloseoutStatus::Failed),
        "cancelled" | "canceled" => Ok(LabCloseoutStatus::Cancelled),
        _ => Err(format!(
            "Invalid closeout status: {value}. Use verified, not_verified, partial, blocked, or failed."
        )),
    }
}

fn parse_u64(name: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("Invalid {name} token count: {value}"))
}

fn parse_optional_u64(name: &str, value: Option<&str>) -> Result<u64, String> {
    value.map_or(Ok(0), |value| parse_u64(name, value))
}

fn format_error_chain(err: &anyhow::Error) -> String {
    err.chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}

fn split_once(input: &str) -> (&str, &str) {
    match input.split_once(char::is_whitespace) {
        Some((head, tail)) => (head, tail.trim()),
        None => (input, ""),
    }
}

fn compact_message_line(input: &str, max_chars: usize) -> String {
    let mut compact = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > max_chars {
        compact = compact.chars().take(max_chars).collect::<String>();
        compact.push_str("...");
    }
    compact
}

#[cfg(test)]
mod tests;
