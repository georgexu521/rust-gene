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
    start_background_scheduler, stop_background_scheduler,
};
use crate::lab::store::{LabCostTokens, LabStore};
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
        "status" => lab_status(&store),
        "runs" => handle_runs_command(store),
        "recovery" | "recover" => handle_recovery_command(project_root, store),
        "report" | "reports" => handle_report_command(store, rest),
        "dashboard" => handle_dashboard_command(project_root, &orchestrator, store),
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
        "daemon" => handle_daemon_command(project_root, store, rest),
        "cost" => handle_cost_command(store, rest),
        "context" => handle_context_command(&orchestrator, store, rest),
        "compression" => handle_compression_command(&orchestrator, store, rest),
        "compress" => handle_compress_command(&orchestrator, rest),
        "evidence" => handle_evidence_command(store, rest),
        "cycle" => handle_cycle_command(&orchestrator, rest),
        "blocker" | "blockers" => handle_blocker_command(&orchestrator, store, rest),
        "message" | "messages" | "sponsor" => {
            handle_sponsor_messages_command(&orchestrator, store, rest)
        }
        "task" | "tasks" => {
            handle_task_command(project_root, &orchestrator, store, subcommand, rest)
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
        "gate" => handle_gate_command(&orchestrator, rest),
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
        "accept" => handle_artifact_accept_command(&orchestrator, rest),
        "revise" => handle_artifact_revise_command(&orchestrator, rest),
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
            handle_meeting_command(project_root, &orchestrator, rest)
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
        "review" => handle_review_command(&orchestrator, store, rest),
        _ => format!("Unknown /lab command: {subcommand}\n\n{}", lab_help()),
    }
}

pub async fn handle_lab_command_with_context(
    project_root: &Path,
    current_session_id: Option<String>,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let trimmed = args.trim();
    if let Some(rest) = trimmed.strip_prefix("task worktree ") {
        return handle_task_worktree_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("tasks worktree ") {
        return handle_task_worktree_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("task run ") {
        return handle_task_run_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("tasks run ") {
        return handle_task_run_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("task sync ") {
        return handle_task_sync_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("tasks sync ") {
        return handle_task_sync_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("messages classify ") {
        return handle_sponsor_message_classify_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("message classify ") {
        return handle_sponsor_message_classify_command(project_root, rest, tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("sponsor classify ") {
        return handle_sponsor_message_classify_command(project_root, rest, tool_context).await;
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
        return handle_provider_compare_command(project_root, tool_context).await;
    }
    if let Some(rest) = trimmed
        .strip_prefix("provider record ")
        .or_else(|| trimmed.strip_prefix("providers record "))
        .or_else(|| trimmed.strip_prefix("certification record "))
    {
        return handle_provider_record_command(project_root, rest, tool_context);
    }
    if matches!(
        trimmed,
        "provider diagnose-tools"
            | "providers diagnose-tools"
            | "certification diagnose-tools"
            | "provider tools"
            | "providers tools"
    ) {
        return handle_provider_tool_diagnostics_command(tool_context).await;
    }
    if matches!(trimmed, "provider" | "providers" | "certification") {
        return handle_provider_command(project_root, tool_context);
    }
    if trimmed == "step llm" {
        return handle_provider_stage_step_command(project_root, "", tool_context).await;
    }
    if let Some(rest) = trimmed.strip_prefix("step llm ") {
        return handle_provider_stage_step_command(project_root, rest, tool_context).await;
    }
    if trimmed == "step" {
        return handle_scheduler_step_command(project_root, tool_context).await;
    }
    if trimmed == "run" || trimmed.starts_with("run ") {
        let args = trimmed.strip_prefix("run").unwrap_or("").trim();
        return handle_scheduler_run_command(project_root, args, tool_context).await;
    }
    if trimmed == "background" || trimmed.starts_with("background ") {
        let args = trimmed.strip_prefix("background").unwrap_or("").trim();
        return handle_background_command(project_root, args, tool_context).await;
    }
    handle_lab_command(project_root, current_session_id, args)
}

fn handle_provider_command(project_root: &Path, tool_context: ToolContext) -> String {
    let report = provider_certification_report(&tool_context);
    let mut lines = vec![
        "Lab provider diagnostics:".to_string(),
        format!("Provider: {}", report.provider_id),
        format!("Model: {}", report.model),
        format!(
            "Graduate diagnostic status: {}",
            report.graduate_certification.as_str()
        ),
        format!(
            "Graduate dispatch policy: {}",
            if report.graduate_execution_allowed {
                "provider_neutral_task_evidence"
            } else {
                "blocked"
            }
        ),
        format!("Diagnostic override enabled: {}", report.override_enabled),
        format!("Legacy override env enabled: {}", report.override_enabled),
        format!("Control-plane diagnostic: {}", report.control_plane_command),
        format!("Graduate diagnostic: {}", report.graduate_command),
        format!("Recommendation: {}", report.recommendation),
    ];
    lines.push(provider_record_line(
        "Latest control-plane record",
        report.latest_control_plane_record.as_ref(),
    ));
    lines.push(provider_record_line(
        "Latest graduate record",
        report.latest_graduate_record.as_ref(),
    ));
    lines.push(format!(
        "Diagnostics store: {}",
        LabStore::for_project(project_root)
            .root()
            .join("provider_certifications.jsonl")
            .display()
    ));
    lines.join("\n")
}

fn provider_record_line(label: &str, record: Option<&LabProviderCertificationRecord>) -> String {
    let Some(record) = record else {
        return format!("{label}: none");
    };
    format!(
        "{}: {} {} at {} evidence={} summary={}",
        label,
        record.kind.as_str(),
        record.outcome.as_str(),
        record.recorded_at,
        record.evidence_path,
        truncate_single_line(&record.summary, 160)
    )
}

fn handle_provider_record_command(
    project_root: &Path,
    rest: &str,
    tool_context: ToolContext,
) -> String {
    let (kind_raw, rest) = split_once(rest);
    let (outcome_raw, rest) = split_once(rest);
    let (evidence_path, summary) = split_once(rest);
    if kind_raw.trim().is_empty()
        || outcome_raw.trim().is_empty()
        || evidence_path.trim().is_empty()
    {
        return "Usage: /lab provider record <control-plane|graduate> <passed|failed> <evidence_path> [summary]".to_string();
    }
    let kind = match parse_provider_certification_kind(kind_raw) {
        Ok(kind) => kind,
        Err(err) => return err,
    };
    let outcome = match parse_provider_certification_outcome(outcome_raw) {
        Ok(outcome) => outcome,
        Err(err) => return err,
    };
    let report = provider_certification_report(&tool_context);
    if report.provider_id == "unknown" || report.model == "unknown" {
        return "Failed to record provider diagnostic: active provider/model is unknown"
            .to_string();
    }
    let summary = if summary.trim().is_empty() {
        format!(
            "{} {} validation {}",
            report.provider_id,
            report.model,
            outcome.as_str()
        )
    } else {
        summary.trim().to_string()
    };
    let store = LabStore::for_project(project_root);
    match store.record_provider_certification(
        &report.provider_id,
        &report.model,
        kind,
        outcome,
        evidence_path,
        &summary,
    ) {
        Ok(record) => format!(
            "Recorded provider diagnostic: {}\nProvider: {}\nModel: {}\nKind: {}\nOutcome: {}\nEvidence: {}\nStore: {}",
            record.record_id,
            record.provider_id,
            record.model,
            record.kind.as_str(),
            record.outcome.as_str(),
            record.evidence_path,
            store.root().join("provider_certifications.jsonl").display()
        ),
        Err(err) => format!("Failed to record provider diagnostic: {err}"),
    }
}

fn parse_provider_certification_kind(value: &str) -> Result<LabProviderCertificationKind, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "control-plane" | "control_plane" | "control" | "professor" => {
            Ok(LabProviderCertificationKind::ControlPlane)
        }
        "graduate" | "lab-graduate" | "worker" => Ok(LabProviderCertificationKind::Graduate),
        _ => {
            Err("Invalid provider diagnostic kind. Expected control-plane or graduate.".to_string())
        }
    }
}

fn parse_provider_certification_outcome(
    value: &str,
) -> Result<LabProviderCertificationOutcome, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "passed" | "pass" | "ok" | "success" | "succeeded" => {
            Ok(LabProviderCertificationOutcome::Passed)
        }
        "failed" | "fail" | "error" | "blocked" => Ok(LabProviderCertificationOutcome::Failed),
        _ => Err("Invalid provider diagnostic outcome. Expected passed or failed.".to_string()),
    }
}

async fn handle_provider_compare_command(project_root: &Path, tool_context: ToolContext) -> String {
    let report = provider_certification_report(&tool_context);
    let generic_result =
        run_generic_subagent_provider_smoke(project_root, tool_context.clone()).await;
    let background_result =
        run_generic_background_subagent_provider_smoke(project_root, tool_context.clone()).await;
    let lab_result =
        run_lab_graduate_provider_smoke(project_root, tool_context, &report.provider_id).await;
    [
        "Provider subagent comparison:".to_string(),
        format!("Provider: {}", report.provider_id),
        format!("Model: {}", report.model),
        format!(
            "Graduate diagnostic status: {}",
            report.graduate_certification.as_str()
        ),
        format!(
            "Graduate dispatch policy: {}",
            if report.graduate_execution_allowed {
                "provider_neutral_task_evidence"
            } else {
                "blocked"
            }
        ),
        String::new(),
        generic_result.summary.clone(),
        String::new(),
        background_result.summary.clone(),
        String::new(),
        lab_result.summary.clone(),
        String::new(),
        provider_compare_conclusion(&generic_result, &lab_result),
    ]
    .join("\n")
}

async fn handle_provider_tool_diagnostics_command(tool_context: ToolContext) -> String {
    let report = provider_certification_report(&tool_context);
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab provider diagnose-tools requires the Lab Mode shell provider. In non-interactive mode use `pa lab --command \"provider diagnose-tools\" --with-provider`."
            .to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab provider diagnose-tools requires an active model.".to_string();
    }

    let probes = vec![
        ProviderToolProbe {
            label: "minimal_auto".to_string(),
            tool_choice: ToolChoice::Auto,
            tools: vec![provider_echo_tool()],
            user_prompt: "Call lab_provider_echo with message exactly `tool probe ok`."
                .to_string(),
        },
        ProviderToolProbe {
            label: "minimal_required".to_string(),
            tool_choice: ToolChoice::Required,
            tools: vec![provider_echo_tool()],
            user_prompt: "Call lab_provider_echo with message exactly `tool probe ok`."
                .to_string(),
        },
        ProviderToolProbe {
            label: "minimal_forced".to_string(),
            tool_choice: ToolChoice::Function("lab_provider_echo".to_string()),
            tools: vec![provider_echo_tool()],
            user_prompt: "Call lab_provider_echo with message exactly `tool probe ok`."
                .to_string(),
        },
        ProviderToolProbe {
            label: "runtime_file_write_auto".to_string(),
            tool_choice: ToolChoice::Auto,
            tools: provider_runtime_tools(&["file_write"]),
            user_prompt: "Use file_write to create `lab-provider-schema-probe.txt` with content exactly `schema probe ok`."
                .to_string(),
        },
        ProviderToolProbe {
            label: "runtime_file_write_bash_auto".to_string(),
            tool_choice: ToolChoice::Auto,
            tools: provider_runtime_tools(&["file_write", "bash"]),
            user_prompt: "Use file_write to create `lab-provider-schema-probe.txt` with content exactly `schema probe ok`; do not answer in prose."
                .to_string(),
        },
        ProviderToolProbe {
            label: "runtime_subagent_allowed_auto".to_string(),
            tool_choice: ToolChoice::Auto,
            tools: provider_runtime_tools(&["file_write", "file_edit", "bash", "diff"]),
            user_prompt: "Use file_write to create `lab-provider-schema-probe.txt` with content exactly `schema probe ok`; do not answer in prose."
                .to_string(),
        },
    ];
    let mut lines = vec![
        "Provider tool-call diagnostics:".to_string(),
        format!("Provider: {}", report.provider_id),
        format!("Model: {}", report.model),
        format!(
            "Graduate diagnostic status: {}",
            report.graduate_certification.as_str()
        ),
        format!(
            "Graduate dispatch policy: {}",
            if report.graduate_execution_allowed {
                "provider_neutral_task_evidence"
            } else {
                "blocked"
            }
        ),
    ];

    for probe in probes {
        let label = probe.label.clone();
        let request = provider_tool_probe_request(&tool_context.model, probe);
        let request_tool_names = request
            .tools
            .as_ref()
            .map(|tools| {
                tools
                    .iter()
                    .map(|tool| tool.name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_else(|| "none".to_string());
        let request_tool_count = request.tools.as_ref().map_or(0, Vec::len);
        let request_tool_choice = format!("{:?}", request.tool_choice);
        let response = provider.chat(request).await;
        lines.push(String::new());
        lines.push(format!("Probe: {label}"));
        lines.push(format!("  request_tools_count: {request_tool_count}"));
        lines.push(format!("  request_tools: {request_tool_names}"));
        lines.push(format!("  request_tool_choice: {request_tool_choice}"));
        match response {
            Ok(response) => append_provider_tool_probe_response(&mut lines, response),
            Err(err) => {
                lines.push("  success: false".to_string());
                lines.push(format!("  error: {err:#}"));
            }
        }
    }

    lines.join("\n")
}

struct ProviderToolProbe {
    label: String,
    tool_choice: ToolChoice,
    tools: Vec<ApiTool>,
    user_prompt: String,
}

fn provider_tool_probe_request(model: &str, probe: ProviderToolProbe) -> ChatRequest {
    ChatRequest::new(model)
        .with_messages(vec![
            ApiMessage::system(
                "You are a function-calling probe. When a tool is available, respond by calling the tool. Do not describe the tool call in prose.",
            ),
            ApiMessage::user(probe.user_prompt),
        ])
        .with_tools(probe.tools)
        .with_tool_choice(probe.tool_choice)
        .with_temperature(0.0)
        .with_output_cap(Some(256))
}

fn provider_runtime_tools(names: &[&str]) -> Vec<ApiTool> {
    let registry = crate::tools::ToolRegistry::full_registry();
    names
        .iter()
        .filter_map(|name| registry.get(name).map(provider_tool_from_runtime_tool))
        .collect()
}

fn provider_tool_from_runtime_tool(tool: &dyn Tool) -> ApiTool {
    ApiTool {
        name: tool.name().to_string(),
        description: tool.description().to_string(),
        parameters: tool.parameters(),
        strict_schema: tool.strict_schema(),
    }
}

fn provider_echo_tool() -> ApiTool {
    ApiTool {
        name: "lab_provider_echo".to_string(),
        description: "Echo a short diagnostic message.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Diagnostic message to echo."
                }
            },
            "required": ["message"],
            "additionalProperties": false
        }),
        strict_schema: true,
    }
}

fn append_provider_tool_probe_response(lines: &mut Vec<String>, response: ChatResponse) {
    let tool_calls = response.tool_calls.unwrap_or_default();
    let tool_names = if tool_calls.is_empty() {
        "none".to_string()
    } else {
        tool_calls
            .iter()
            .map(|call| call.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };
    let repair_summary = response
        .tool_call_repair
        .as_ref()
        .map(provider_tool_repair_summary)
        .unwrap_or_else(|| "none".to_string());
    lines.push("  success: true".to_string());
    lines.push(format!("  response_tool_calls_count: {}", tool_calls.len()));
    lines.push(format!("  response_tool_calls: {tool_names}"));
    lines.push(format!(
        "  finish_reason: {}",
        response.finish_reason.unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!("  tool_call_repair: {repair_summary}"));
    lines.push(format!(
        "  content_preview: {}",
        truncate_single_line(&response.content, 240)
    ));
}

fn provider_tool_repair_summary(
    report: &crate::services::api::tool_call_repair::ToolCallRepairReport,
) -> String {
    format!(
        "family={} flattened_tools={} scavenged={} argument_repairs={} unflattened={} duplicates_dropped={} malformed={} warnings={}",
        report.provider_family,
        report.schema_flattened_tools,
        report.scavenged_tool_calls,
        report.argument_repairs,
        report.unflattened_arguments,
        report.dropped_duplicate_calls,
        report.malformed_tool_calls,
        report.warnings.len()
    )
}

struct ProviderComparePathResult {
    summary: String,
    success: bool,
    used_mutating_tool: bool,
    blocked_by_certification: bool,
}

async fn run_generic_subagent_provider_smoke(
    project_root: &Path,
    tool_context: ToolContext,
) -> ProviderComparePathResult {
    let session_store = tool_context.session_store.clone();
    let session_id = tool_context.session_id.clone();
    let task_id = "provider-compare-generic";
    let params = serde_json::json!({
        "description": "Provider comparison generic implementer smoke",
        "prompt": "Inside the current isolated worktree only, create or overwrite the relative path exactly `lab-provider-compare-generic.txt` with exactly one line: generic subagent tool smoke. Do not use an absolute path and do not write in the parent workspace. Then verify by reading the same relative path with file_read; if bash validation is allowed, also run `test -f lab-provider-compare-generic.txt`. Use real tools only, then summarize the tools used.",
        "files": ["lab-provider-compare-generic.txt"],
        "profile": "implementer",
        "context_mode": "isolated_worktree_fork",
        "allowed_tools": ["file_read", "file_write", "file_edit", "bash", "diff"],
        "timeout_secs": 90,
        "max_turns": 3,
        "task_id": task_id
    });
    let result = AgentTool::with_working_dir(project_root)
        .execute(params, tool_context)
        .await;
    if !result.success {
        if let Some(store) = session_store.as_ref() {
            if let Some(recovered) = recover_provider_compare_durable_subagent(
                "Generic subagent",
                store,
                &session_id,
                task_id,
                "lab-provider-compare-generic.txt",
                result.error.as_deref().unwrap_or("none"),
            )
            .await
            {
                return recovered;
            }
        }
    }
    let data = result.data.as_ref();
    let tools_used = data
        .and_then(|value| value.get("tools_used"))
        .map(format_json_value)
        .unwrap_or_else(|| "none".to_string());
    let allowed_tools = data
        .and_then(|value| value.get("allowed_tools"))
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let status = data
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or(if result.success { "success" } else { "failed" });
    let agent_id = data
        .and_then(|value| value.get("agent_id"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    let result_preview = data
        .and_then(|value| value.get("result"))
        .and_then(Value::as_str)
        .map(|value| truncate_single_line(value, 240))
        .unwrap_or_else(|| "none".to_string());
    let profile = data
        .and_then(|value| value.get("profile"))
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let context_mode = data
        .and_then(|value| value.get("context_mode"))
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let error = result.error.unwrap_or_else(|| "none".to_string());
    let attempted_mutating_tool = data
        .and_then(|value| value.get("tools_used"))
        .is_some_and(value_contains_mutating_tool);
    let (file_proof, file_exists) = subagent_smoke_file_proof(
        session_store.as_deref(),
        &session_id,
        agent_id,
        "lab-provider-compare-generic.txt",
    );
    ProviderComparePathResult {
        summary: [
            "Generic subagent:".to_string(),
            format!("  success: {}", result.success),
            format!("  status: {status}"),
            format!("  agent_id: {agent_id}"),
            format!("  profile: {profile}"),
            format!("  context_mode: {context_mode}"),
            format!("  allowed_tools: {allowed_tools}"),
            format!("  tools_used: {tools_used}"),
            format!("  file_proof: {file_proof}"),
            format!("  result_preview: {result_preview}"),
            format!("  error: {error}"),
        ]
        .join("\n"),
        success: result.success,
        used_mutating_tool: hard_subagent_mutation_proof(
            attempted_mutating_tool,
            file_exists,
            &result_preview,
        ),
        blocked_by_certification: false,
    }
}

async fn recover_provider_compare_durable_subagent(
    label: &str,
    store: &std::sync::Arc<crate::session_store::SessionStore>,
    session_id: &str,
    task_id: &str,
    proof_file: &str,
    foreground_error: &str,
) -> Option<ProviderComparePathResult> {
    let mut final_state = None;
    for _ in 0..60 {
        match store.agent_task_state(session_id, task_id) {
            Ok(Some(state)) if state.result_artifact_id.is_some() || state.status != "running" => {
                final_state = Some(state);
                if final_state
                    .as_ref()
                    .is_some_and(|state| state.result_artifact_id.is_some())
                {
                    break;
                }
            }
            Ok(Some(state)) => {
                final_state = Some(state);
            }
            Ok(None) | Err(_) => {}
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let state = final_state?;
    let artifact = state
        .result_artifact_id
        .and_then(|id| store.agent_artifact(session_id, id).ok().flatten());
    let tools_used = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("tools_used"))
        .map(format_json_value)
        .or_else(|| state.payload.get("tools_used").map(format_json_value))
        .unwrap_or_else(|| "none".to_string());
    let allowed_tools = state
        .payload
        .get("allowed_tools")
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let profile = state
        .profile
        .clone()
        .or_else(|| {
            state
                .payload
                .pointer("/agent_definition/name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string());
    let context_mode = state
        .payload
        .get("context_mode")
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let completion_sink = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("completion_sink"))
        .and_then(Value::as_str)
        .or_else(|| state.payload.get("completion_sink").and_then(Value::as_str))
        .unwrap_or("none");
    let result_preview = artifact
        .as_ref()
        .map(|artifact| truncate_single_line(&artifact.output, 240))
        .unwrap_or_else(|| "none".to_string());
    let (file_proof, file_exists) = subagent_smoke_file_proof(
        Some(store.as_ref()),
        session_id,
        &state.agent_id,
        proof_file,
    );
    let attempted_mutating_tool = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("tools_used"))
        .is_some_and(value_contains_mutating_tool)
        || state
            .payload
            .get("tools_used")
            .is_some_and(value_contains_mutating_tool);
    let hard_mutation_proof =
        hard_subagent_mutation_proof(attempted_mutating_tool, file_exists, &result_preview);
    let success = state.status == "completed" && artifact.is_some() && hard_mutation_proof;

    Some(ProviderComparePathResult {
        summary: [
            format!("{label}:"),
            format!("  success: {success}"),
            format!("  status: {}", state.status),
            format!("  agent_id: {}", state.agent_id),
            format!("  task_id: {}", state.task_id),
            format!("  profile: {profile}"),
            format!("  context_mode: {context_mode}"),
            format!("  allowed_tools: {allowed_tools}"),
            format!("  tools_used: {tools_used}"),
            format!(
                "  result_artifact_id: {}",
                state
                    .result_artifact_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!("  completion_sink: {completion_sink}"),
            format!("  file_proof: {file_proof}"),
            format!("  hard_file_proof: {file_exists}"),
            "  recovered_from_durable_sink: true".to_string(),
            format!("  result_preview: {result_preview}"),
            format!("  foreground_error: {foreground_error}"),
        ]
        .join("\n"),
        success,
        used_mutating_tool: hard_mutation_proof,
        blocked_by_certification: false,
    })
}

async fn run_generic_background_subagent_provider_smoke(
    project_root: &Path,
    tool_context: ToolContext,
) -> ProviderComparePathResult {
    let Some(store) = tool_context.session_store.clone() else {
        return ProviderComparePathResult {
            summary: "Background subagent:\n  skipped: no session store available for completion sink validation".to_string(),
            success: false,
            used_mutating_tool: false,
            blocked_by_certification: false,
        };
    };
    let session_id = tool_context.session_id.clone();
    let task_id = "provider-compare-background";
    let params = serde_json::json!({
        "description": "Provider comparison background implementer smoke",
        "prompt": "Inside the current isolated worktree only, create or overwrite the relative path exactly `lab-provider-compare-background.txt` with exactly one line: background subagent tool smoke. Do not use an absolute path and do not write in the parent workspace. Then verify by reading the same relative path with file_read; if bash validation is allowed, also run `test -f lab-provider-compare-background.txt`. Use real tools only, then summarize the tools used.",
        "files": ["lab-provider-compare-background.txt"],
        "profile": "implementer",
        "context_mode": "isolated_worktree_fork",
        "allowed_tools": ["file_read", "file_write", "file_edit", "bash", "diff"],
        "timeout_secs": 90,
        "max_turns": 3,
        "task_id": task_id,
        "background": true
    });
    let launch = AgentTool::with_working_dir(project_root)
        .execute(params, tool_context.clone())
        .await;
    if !launch.success {
        return ProviderComparePathResult {
            summary: [
                "Background subagent:".to_string(),
                "  success: false".to_string(),
                "  launch_status: failed".to_string(),
                format!("  task_id: {task_id}"),
                format!(
                    "  error: {}",
                    launch.error.unwrap_or_else(|| "none".to_string())
                ),
            ]
            .join("\n"),
            success: false,
            used_mutating_tool: false,
            blocked_by_certification: false,
        };
    }

    if let (Some(manager), Some(agent_id)) = (
        tool_context.agent_manager.as_ref(),
        launch
            .data
            .as_ref()
            .and_then(|data| data.get("agent_id"))
            .and_then(Value::as_str),
    ) {
        let _ = manager
            .wait_for_result(&AgentId(agent_id.to_string()), 180)
            .await;
    }

    let mut final_state = None;
    for _ in 0..120 {
        match store.agent_task_state(&session_id, task_id) {
            Ok(Some(state)) if state.status != "running" => {
                final_state = Some(state);
                break;
            }
            Ok(Some(state)) if state.result_artifact_id.is_some() => {
                final_state = Some(state);
                break;
            }
            Ok(Some(state)) => {
                final_state = Some(state);
            }
            Ok(None) | Err(_) => {}
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let Some(state) = final_state else {
        return ProviderComparePathResult {
            summary: [
                "Background subagent:".to_string(),
                "  success: false".to_string(),
                "  launch_status: launched".to_string(),
                format!("  task_id: {task_id}"),
                "  durable_state: missing".to_string(),
            ]
            .join("\n"),
            success: false,
            used_mutating_tool: false,
            blocked_by_certification: false,
        };
    };

    let artifact = state
        .result_artifact_id
        .and_then(|id| store.agent_artifact(&session_id, id).ok().flatten());
    let tools_used = state
        .payload
        .get("tools_used")
        .map(format_json_value)
        .unwrap_or_else(|| "none".to_string());
    let completion_sink = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("completion_sink"))
        .and_then(Value::as_str)
        .or_else(|| state.payload.get("completion_sink").and_then(Value::as_str))
        .unwrap_or("none");
    let result_preview = artifact
        .as_ref()
        .map(|artifact| truncate_single_line(&artifact.output, 240))
        .unwrap_or_else(|| "none".to_string());
    let (file_proof, file_exists) = subagent_smoke_file_proof(
        Some(store.as_ref()),
        &session_id,
        &state.agent_id,
        "lab-provider-compare-background.txt",
    );
    let attempted_mutating_tool = value_contains_mutating_tool(&state.payload["tools_used"]);
    let hard_mutation_proof =
        hard_subagent_mutation_proof(attempted_mutating_tool, file_exists, &result_preview);
    let permission_denied = subagent_runtime_denied(&result_preview);
    let success = state.status == "completed"
        && artifact.is_some()
        && completion_sink == "agent_manager"
        && hard_mutation_proof;

    ProviderComparePathResult {
        summary: [
            "Background subagent:".to_string(),
            format!("  success: {success}"),
            "  launch_status: launched".to_string(),
            format!("  task_id: {}", state.task_id),
            format!("  status: {}", state.status),
            format!("  agent_id: {}", state.agent_id),
            format!(
                "  result_artifact_id: {}",
                state
                    .result_artifact_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!("  completion_sink: {completion_sink}"),
            format!("  tools_used: {tools_used}"),
            format!("  file_proof: {file_proof}"),
            format!("  hard_file_proof: {file_exists}"),
            format!("  permission_denied: {permission_denied}"),
            format!("  result_preview: {result_preview}"),
        ]
        .join("\n"),
        success,
        used_mutating_tool: hard_mutation_proof,
        blocked_by_certification: false,
    }
}

async fn run_lab_graduate_provider_smoke(
    project_root: &Path,
    tool_context: ToolContext,
    provider_id: &str,
) -> ProviderComparePathResult {
    let orchestrator = LabOrchestrator::for_project(project_root);
    let store = orchestrator.store();
    let run = match store.latest_run() {
        Ok(Some(run)) if matches!(run.status, LabRunStatus::Active) => run,
        Ok(Some(run)) => {
            return ProviderComparePathResult {
                summary: format!(
                    "Lab graduate:\n  skipped: latest LabRun {} is not active ({:?})",
                    run.lab_run_id, run.status
                ),
                success: false,
                used_mutating_tool: false,
                blocked_by_certification: false,
            };
        }
        Ok(None) => {
            return ProviderComparePathResult {
                summary: "Lab graduate:\n  skipped: no active LabRun; create and approve a LabRun before comparing the Lab graduate path.".to_string(),
                success: false,
                used_mutating_tool: false,
                blocked_by_certification: false,
            };
        }
        Err(err) => {
            return ProviderComparePathResult {
                summary: format!("Lab graduate:\n  failed to read latest LabRun: {err}"),
                success: false,
                used_mutating_tool: false,
                blocked_by_certification: false,
            };
        }
    };
    let task = match store.create_graduate_task(
        &run.lab_run_id,
        "Provider comparison Lab graduate smoke",
        &format!(
            "Provider comparison smoke for {provider_id}. Create or overwrite lab-provider-compare-lab.txt with exactly one line: lab graduate tool smoke. Then run `test -f lab-provider-compare-lab.txt`. Return the required graduate_result JSON with changed_files containing lab-provider-compare-lab.txt and validation_results containing the test command."
        ),
        vec!["lab-provider-compare-lab.txt".to_string()],
        vec!["test -f lab-provider-compare-lab.txt".to_string()],
    ) {
        Ok(task) => task,
        Err(err) => {
            return ProviderComparePathResult {
                summary: format!("Lab graduate:\n  failed to create smoke task: {err}"),
                success: false,
                used_mutating_tool: false,
                blocked_by_certification: false,
            };
        }
    };
    match orchestrator
        .execute_graduate_task_latest_with_context(&task.task_id, tool_context.clone())
        .await
    {
        Ok(record) => {
            let error = record.error.as_deref().unwrap_or("none");
            let blocked_by_certification = error.contains("graduate provider")
                || error.contains("not certified")
                || error.contains("certification");
            let agent_task_id = graduate_agent_task_id(&task);
            let (durable_lines, durable_mutation_proof) = lab_graduate_durable_smoke_details(
                &tool_context,
                &agent_task_id,
                "lab-provider-compare-lab.txt",
            );
            let success = matches!(
                record.status,
                crate::lab::model::GraduateDispatchStatus::Succeeded
            );
            let mut summary = vec![
                "Lab graduate:".to_string(),
                format!("  success: {success}"),
                format!("  status: {:?}", record.status),
                format!("  dispatch_id: {}", record.dispatch_id),
                format!("  task_id: {}", record.task_id),
                format!("  durable_task_id: {agent_task_id}"),
                format!(
                    "  agent_id: {}",
                    record.agent_id.as_deref().unwrap_or("none")
                ),
                format!(
                    "  result_artifact_id: {}",
                    record.result_artifact_id.as_deref().unwrap_or("none")
                ),
                format!("  error: {error}"),
            ];
            summary.extend(durable_lines);
            ProviderComparePathResult {
                summary: summary.join("\n"),
                success,
                used_mutating_tool: success && durable_mutation_proof,
                blocked_by_certification,
            }
        }
        Err(err) => ProviderComparePathResult {
            summary: format!("Lab graduate:\n  failed before/while dispatching: {err}"),
            success: false,
            used_mutating_tool: false,
            blocked_by_certification: err.to_string().contains("certification"),
        },
    }
}

fn lab_graduate_durable_smoke_details(
    tool_context: &ToolContext,
    agent_task_id: &str,
    file_name: &str,
) -> (Vec<String>, bool) {
    let Some(store) = tool_context.session_store.as_deref() else {
        return (
            vec!["  durable_state: unavailable: no session store".to_string()],
            false,
        );
    };
    let state = match store.agent_task_state(&tool_context.session_id, agent_task_id) {
        Ok(Some(state)) => state,
        Ok(None) => {
            return (
                vec![format!(
                    "  durable_state: missing for task_id {agent_task_id}"
                )],
                false,
            );
        }
        Err(err) => {
            return (vec![format!("  durable_state: error: {err}")], false);
        }
    };

    let artifact = state.result_artifact_id.and_then(|id| {
        store
            .agent_artifact(&tool_context.session_id, id)
            .ok()
            .flatten()
    });
    let tools_used = state
        .payload
        .get("tools_used")
        .map(format_json_value)
        .or_else(|| {
            artifact
                .as_ref()
                .and_then(|artifact| artifact.payload.get("tools_used"))
                .map(format_json_value)
        })
        .unwrap_or_else(|| "none".to_string());
    let completion_sink = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("completion_sink"))
        .and_then(Value::as_str)
        .or_else(|| state.payload.get("completion_sink").and_then(Value::as_str))
        .unwrap_or("none");
    let result_preview = artifact
        .as_ref()
        .map(|artifact| truncate_single_line(&artifact.output, 240))
        .unwrap_or_else(|| "none".to_string());
    let (file_proof, file_exists) = subagent_smoke_file_proof(
        Some(store),
        &tool_context.session_id,
        &state.agent_id,
        file_name,
    );
    let attempted_mutating_tool = state
        .payload
        .get("tools_used")
        .is_some_and(value_contains_mutating_tool);
    let hard_mutation_proof =
        hard_subagent_mutation_proof(attempted_mutating_tool, file_exists, &result_preview);
    let permission_denied = subagent_runtime_denied(&result_preview);
    let context_mode = state
        .payload
        .get("context_mode")
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let result_artifact_id = state
        .result_artifact_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());

    (
        vec![
            "  durable_state: present".to_string(),
            format!("  durable_status: {}", state.status),
            format!("  durable_agent_id: {}", state.agent_id),
            format!(
                "  durable_profile: {}",
                state.profile.as_deref().unwrap_or("none")
            ),
            format!("  durable_context_mode: {context_mode}"),
            format!("  durable_result_artifact_id: {result_artifact_id}"),
            format!("  completion_sink: {completion_sink}"),
            format!("  tools_used: {tools_used}"),
            format!("  file_proof: {file_proof}"),
            format!("  hard_file_proof: {file_exists}"),
            format!("  permission_denied: {permission_denied}"),
            format!("  durable_result_preview: {result_preview}"),
        ],
        hard_mutation_proof,
    )
}

fn provider_compare_conclusion(
    generic: &ProviderComparePathResult,
    lab: &ProviderComparePathResult,
) -> String {
    let conclusion = if lab.blocked_by_certification {
        if generic.used_mutating_tool {
            "Generic subagent demonstrated tool use; Lab graduate reported a legacy certification block, which should now be treated as a diagnostics bug rather than provider policy."
        } else if generic.success {
            "Generic subagent completed but produced no hard mutating-tool proof through tools_used or isolated-worktree file evidence; inspect task-level evidence before trusting graduate output."
        } else {
            "Generic subagent also failed; this points first at provider/subagent tool-call capability or runtime provider wiring, not only Lab graduate prompting."
        }
    } else if generic.used_mutating_tool && !lab.success {
        "Generic subagent demonstrated tool use but Lab graduate failed; inspect Lab graduate envelope, prompt contract, scope validation, JSON binding, and task evidence."
    } else if generic.success && lab.success {
        "Both paths completed; provider tool use is available through generic and Lab graduate routing."
    } else if !generic.success && lab.success {
        "Lab graduate completed while generic failed; compare profile/tool exposure differences before changing provider policy."
    } else {
        "Both paths failed or lacked tool-use proof; treat this run as weak task evidence and rely on postdoc review, not provider allowlists."
    };
    format!("Conclusion: {conclusion}")
}

fn value_contains_mutating_tool(value: &Value) -> bool {
    match value {
        Value::String(tool) => is_mutating_tool_name(tool),
        Value::Array(items) => items.iter().any(value_contains_mutating_tool),
        Value::Object(map) => map.values().any(value_contains_mutating_tool),
        _ => false,
    }
}

fn is_mutating_tool_name(tool: &str) -> bool {
    matches!(tool, "file_write" | "file_edit" | "bash" | "format")
}

fn hard_subagent_mutation_proof(
    attempted_mutating_tool: bool,
    file_exists: bool,
    result_preview: &str,
) -> bool {
    attempted_mutating_tool && file_exists && !subagent_runtime_denied(result_preview)
}

fn subagent_runtime_denied(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "permission denied",
        "requires user confirmation",
        "blocked by the runtime permission system",
        "checkpoint_required",
        "action rejected before execution",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

fn format_json_value(value: &Value) -> String {
    match value {
        Value::Null => "none".to_string(),
        Value::String(value) => value.to_string(),
        Value::Array(items) if items.is_empty() => "none".to_string(),
        Value::Array(items) => items
            .iter()
            .map(format_json_value)
            .collect::<Vec<_>>()
            .join(","),
        other => other.to_string(),
    }
}

fn subagent_smoke_file_proof(
    store: Option<&crate::session_store::SessionStore>,
    session_id: &str,
    agent_id: &str,
    file_name: &str,
) -> (String, bool) {
    if agent_id == "none" {
        return ("unavailable: no agent_id".to_string(), false);
    }
    let Some(store) = store else {
        return ("unavailable: no session store".to_string(), false);
    };
    let state = match store.agent_task_state(session_id, agent_id) {
        Ok(Some(state)) => state,
        Ok(None) => return ("unavailable: no durable agent state".to_string(), false),
        Err(err) => return (format!("unavailable: durable state error: {err}"), false),
    };
    let Some(worktree_path) = state
        .payload
        .pointer("/isolated_worktree/path")
        .and_then(Value::as_str)
    else {
        return (
            "unavailable: no isolated_worktree path in durable state".to_string(),
            false,
        );
    };
    let proof_path = Path::new(worktree_path).join(file_name);
    let exists = proof_path.is_file();
    (
        format!("{} exists={}", proof_path.display(), exists),
        exists,
    )
}

fn truncate_single_line(value: &str, max_chars: usize) -> String {
    let single_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut truncated = single_line.chars().take(max_chars).collect::<String>();
    if single_line.chars().count() > max_chars {
        truncated.push_str("...");
    }
    truncated
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

async fn handle_sponsor_message_classify_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let (message_id, instructions) = split_once(args);
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return "Usage: /lab messages classify <message_id|latest> [instructions]".to_string();
    }
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab messages classify <message_id|latest> [instructions] requires an active Lab Mode provider."
            .to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab messages classify <message_id|latest> [instructions] requires an active model."
            .to_string();
    }
    match crate::lab::draft::classify_sponsor_message_with_provider(
        project_root,
        provider,
        tool_context.model,
        message_id,
        instructions,
    )
    .await
    {
        Ok(outcome) => format!(
            "Professor classified sponsor message: {}\nDecision: {}\nStatus: {:?}\nNote: {}",
            outcome.message.message_id, outcome.decision, outcome.message.status, outcome.note
        ),
        Err(err) => format!(
            "Failed to classify professor side-channel message: {}",
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

async fn handle_task_run_command(
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

async fn handle_task_sync_command(
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

async fn handle_task_worktree_command(
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

async fn handle_scheduler_step_command(project_root: &Path, tool_context: ToolContext) -> String {
    let orchestrator = LabOrchestrator::for_project(project_root);
    match orchestrator
        .run_scheduler_step_latest_with_context(tool_context)
        .await
    {
        Ok(step) => {
            let mut lines = vec![
                format!("Lab scheduler step: {:?}", step.action),
                format!("LabRun: {}", step.lab_run_id),
                format!("Stage: {}", step.stage),
                step.message,
            ];
            if let Some(task_id) = step.task_id {
                lines.push(format!("Task: {task_id}"));
            }
            if let Some(dispatch_id) = step.dispatch_id {
                lines.push(format!("Dispatch: {dispatch_id}"));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to run Lab scheduler step: {err}"),
    }
}

async fn handle_scheduler_run_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let (mode, rest) = split_once(args.trim());
    if mode == "llm" {
        return handle_provider_stage_run_command(project_root, rest, tool_context).await;
    }
    if mode == "hybrid" {
        return handle_hybrid_run_command(project_root, rest, tool_context).await;
    }
    if mode == "hybrid-cycles" {
        return handle_hybrid_cycle_run_command(project_root, rest, tool_context).await;
    }

    let max_steps = if args.trim().is_empty() {
        5
    } else {
        match args.trim().parse::<usize>() {
            Ok(value) if value > 0 => value,
            _ => return "Usage: /lab run [max_steps]".to_string(),
        }
    };
    let orchestrator = LabOrchestrator::for_project(project_root);
    match orchestrator
        .run_scheduler_steps_latest_with_context(max_steps, tool_context)
        .await
    {
        Ok(steps) => {
            if steps.is_empty() {
                return "Lab scheduler run completed no steps.".to_string();
            }
            let mut lines = vec![format!("Lab scheduler run: {} step(s)", steps.len())];
            for (idx, step) in steps.iter().enumerate() {
                lines.push(format!(
                    "{}. {:?} stage={} task={} dispatch={} - {}",
                    idx + 1,
                    step.action,
                    step.stage,
                    step.task_id.as_deref().unwrap_or("none"),
                    step.dispatch_id.as_deref().unwrap_or("none"),
                    step.message
                ));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to run Lab scheduler: {err}"),
    }
}

async fn handle_provider_stage_run_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab run llm [max_steps] [instructions] requires an active Lab Mode provider."
            .to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab run llm [max_steps] [instructions] requires an active model."
            .to_string();
    }
    let (max_steps, instructions) = match parse_run_limit_and_instructions(args, 5) {
        Ok(parsed) => parsed,
        Err(usage) => return usage,
    };
    match crate::lab::draft::run_provider_stage_steps_until_boundary(
        project_root,
        provider,
        tool_context.model,
        max_steps,
        &instructions,
    )
    .await
    {
        Ok(outcome) => {
            let mut lines = vec![
                format!("Provider Lab run: {} step(s)", outcome.steps.len()),
                format!("LabRun: {}", outcome.lab_run_id),
                format!("Final stage: {}", outcome.final_stage),
                format!("Stop reason: {:?}", outcome.stop_reason),
            ];
            for (idx, step) in outcome.steps.iter().enumerate() {
                lines.push(format!(
                    "{}. provider {} -> {} artifact={} review={:?} advanced={}",
                    idx + 1,
                    step.from_stage,
                    step.to_stage,
                    step.artifact_id,
                    step.review_decision,
                    step.advanced
                ));
            }
            lines.join("\n")
        }
        Err(err) => format!(
            "Failed to run provider Lab stages: {}",
            format_error_chain(&err)
        ),
    }
}

async fn handle_hybrid_run_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab run hybrid [max_steps] [instructions] requires an active Lab Mode provider."
            .to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab run hybrid [max_steps] [instructions] requires an active model."
            .to_string();
    }
    let (max_steps, instructions) = match parse_run_limit_and_instructions(args, 5) {
        Ok(parsed) => parsed,
        Err(usage) => return usage,
    };
    match crate::lab::draft::run_hybrid_lab_steps_until_boundary(
        project_root,
        provider,
        tool_context.model.clone(),
        max_steps,
        &instructions,
        tool_context,
    )
    .await
    {
        Ok(outcome) => {
            let mut lines = vec![
                format!("Hybrid Lab run: {} step(s)", outcome.steps.len()),
                format!("LabRun: {}", outcome.lab_run_id),
                format!("Final stage: {}", outcome.final_stage),
                format!("Stop reason: {:?}", outcome.stop_reason),
            ];
            for (idx, step) in outcome.steps.iter().enumerate() {
                lines.push(format!("{}. {}", idx + 1, render_hybrid_run_step(step)));
            }
            lines.join("\n")
        }
        Err(err) => format!(
            "Failed to run hybrid Lab stages: {}",
            format_error_chain(&err)
        ),
    }
}

async fn handle_hybrid_cycle_run_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab run hybrid-cycles [max_cycles] [max_steps_per_cycle] [instructions] requires an active Lab Mode provider."
            .to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab run hybrid-cycles [max_cycles] [max_steps_per_cycle] [instructions] requires an active model."
            .to_string();
    }
    let (max_cycles, max_steps_per_cycle, instructions) =
        match parse_cycle_run_limits_and_instructions(args, 2, 5) {
            Ok(parsed) => parsed,
            Err(usage) => return usage,
        };
    match crate::lab::draft::run_hybrid_lab_cycles_until_boundary(
        project_root,
        provider,
        tool_context.model.clone(),
        max_cycles,
        max_steps_per_cycle,
        &instructions,
        tool_context,
    )
    .await
    {
        Ok(outcome) => {
            let mut lines = vec![
                format!("Hybrid Lab cycle run: {} cycle(s)", outcome.cycles.len()),
                format!("LabRun: {}", outcome.lab_run_id),
                format!("Final stage: {}", outcome.final_stage),
                format!("Final cycle count: {}", outcome.final_cycle_count),
                format!("Stop reason: {:?}", outcome.stop_reason),
            ];
            for cycle in &outcome.cycles {
                lines.push(format!(
                    "Cycle {} started_at={} steps={} final_stage={} stop={:?} continued_to_next_cycle={} compression_artifacts={}",
                    cycle.cycle_index,
                    cycle.cycle_count_at_start,
                    cycle.outcome.steps.len(),
                    cycle.outcome.final_stage,
                    cycle.outcome.stop_reason,
                    cycle.continued_to_next_cycle,
                    if cycle.compression_artifact_ids.is_empty() {
                        "none".to_string()
                    } else {
                        cycle.compression_artifact_ids.join(",")
                    }
                ));
                for (idx, step) in cycle.outcome.steps.iter().enumerate() {
                    lines.push(format!(
                        "  {}.{} {}",
                        cycle.cycle_index,
                        idx + 1,
                        render_hybrid_run_step(step)
                    ));
                }
            }
            lines.join("\n")
        }
        Err(err) => format!(
            "Failed to run hybrid Lab cycles: {}",
            format_error_chain(&err)
        ),
    }
}

fn parse_run_limit_and_instructions(
    args: &str,
    default_limit: usize,
) -> Result<(usize, String), String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return Ok((default_limit, String::new()));
    }
    let (first, rest) = split_once(trimmed);
    if let Ok(value) = first.parse::<usize>() {
        if value == 0 {
            return Err("Usage: /lab run <llm|hybrid> [max_steps] [instructions]".to_string());
        }
        return Ok((value, rest.trim().to_string()));
    }
    Ok((default_limit, trimmed.to_string()))
}

fn parse_cycle_run_limits_and_instructions(
    args: &str,
    default_cycles: usize,
    default_steps: usize,
) -> Result<(usize, usize, String), String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return Ok((default_cycles, default_steps, String::new()));
    }
    let (first, rest) = split_once(trimmed);
    let Ok(max_cycles) = first.parse::<usize>() else {
        return Ok((default_cycles, default_steps, trimmed.to_string()));
    };
    if max_cycles == 0 {
        return Err(
            "Usage: /lab run hybrid-cycles [max_cycles] [max_steps_per_cycle] [instructions]"
                .to_string(),
        );
    }
    let rest = rest.trim();
    if rest.is_empty() {
        return Ok((max_cycles, default_steps, String::new()));
    }
    let (second, instructions) = split_once(rest);
    if let Ok(max_steps) = second.parse::<usize>() {
        if max_steps == 0 {
            return Err(
                "Usage: /lab run hybrid-cycles [max_cycles] [max_steps_per_cycle] [instructions]"
                    .to_string(),
            );
        }
        return Ok((max_cycles, max_steps, instructions.trim().to_string()));
    }
    Ok((max_cycles, default_steps, rest.to_string()))
}

fn render_hybrid_run_step(step: &crate::lab::draft::LabHybridRunStep) -> String {
    match step {
        crate::lab::draft::LabHybridRunStep::Provider(step) => format!(
            "provider {} -> {} artifact={} review={:?} advanced={}",
            step.from_stage, step.to_stage, step.artifact_id, step.review_decision, step.advanced
        ),
        crate::lab::draft::LabHybridRunStep::Scheduler(step) => format!(
            "scheduler {:?} stage={} task={} dispatch={} - {}",
            step.action,
            step.stage,
            step.task_id.as_deref().unwrap_or("none"),
            step.dispatch_id.as_deref().unwrap_or("none"),
            step.message
        ),
        crate::lab::draft::LabHybridRunStep::Deterministic(step) => format!(
            "deterministic {} -> {} artifact={} gate_satisfied={}",
            step.from_stage, step.to_stage, step.artifact_id, step.gate_satisfied
        ),
    }
}

async fn handle_background_command(
    project_root: &Path,
    args: &str,
    tool_context: ToolContext,
) -> String {
    let (action, rest) = split_once(args);
    match action {
        "" | "status" => match background_scheduler_status(project_root) {
            Ok(status) => {
                let mut lines = vec![
                    format!("Lab background scheduler: {}", status.lab_run_id),
                    format!("Running in process: {}", status.running_in_process),
                ];
                if let Some(state) = status.persisted {
                    lines.push(format!("Persisted status: {:?}", state.status));
                    lines.push(format!(
                        "Steps: {}/{}",
                        state.steps_completed, state.max_steps
                    ));
                    if let Some(message) = state.last_message {
                        lines.push(format!("Last message: {message}"));
                    }
                } else {
                    lines.push("Persisted status: none".to_string());
                }
                lines.join("\n")
            }
            Err(err) => format!("Failed to read Lab background scheduler: {err}"),
        },
        "start" => {
            let parts = rest.split_whitespace().collect::<Vec<_>>();
            let max_steps = match parts.first() {
                Some(value) => match value.parse::<usize>() {
                    Ok(value) => value,
                    Err(_) => {
                        return "Usage: /lab background start [max_steps] [interval_ms]".to_string()
                    }
                },
                None => default_background_max_steps(),
            };
            let interval_ms = match parts.get(1) {
                Some(value) => match value.parse::<u64>() {
                    Ok(value) => value,
                    Err(_) => {
                        return "Usage: /lab background start [max_steps] [interval_ms]".to_string()
                    }
                },
                None => default_background_interval_ms(),
            };
            if parts.len() > 2 {
                return "Usage: /lab background start [max_steps] [interval_ms]".to_string();
            }
            match start_background_scheduler(project_root, tool_context, max_steps, interval_ms) {
                Ok(started) => format!(
                    "Started Lab background scheduler for {}.\nMax steps: {}\nInterval ms: {}",
                    started.lab_run_id, started.max_steps, started.interval_ms
                ),
                Err(err) => format!("Failed to start Lab background scheduler: {err}"),
            }
        }
        "hybrid" => {
            let Some(provider) = tool_context.llm_provider.clone() else {
                return "Usage: /lab background hybrid [max_steps] [interval_ms] [instructions] requires an active Lab Mode provider."
                    .to_string();
            };
            if tool_context.model.trim().is_empty() {
                return "Usage: /lab background hybrid [max_steps] [interval_ms] [instructions] requires an active model."
                    .to_string();
            }
            let (max_steps, interval_ms, instructions) =
                match parse_background_hybrid_args(rest) {
                    Ok(parsed) => parsed,
                    Err(usage) => return usage,
                };
            match start_background_hybrid_scheduler(
                project_root,
                tool_context.clone(),
                provider,
                tool_context.model.clone(),
                max_steps,
                interval_ms,
                instructions,
            ) {
                Ok(started) => format!(
                    "Started Lab hybrid background scheduler for {}.\nMax steps: {}\nInterval ms: {}",
                    started.lab_run_id, started.max_steps, started.interval_ms
                ),
                Err(err) => format!("Failed to start Lab hybrid background scheduler: {err}"),
            }
        }
        "hybrid-cycles" => {
            let Some(provider) = tool_context.llm_provider.clone() else {
                return "Usage: /lab background hybrid-cycles [max_cycles] [max_steps_per_cycle] [interval_ms] [instructions] requires an active Lab Mode provider."
                    .to_string();
            };
            if tool_context.model.trim().is_empty() {
                return "Usage: /lab background hybrid-cycles [max_cycles] [max_steps_per_cycle] [interval_ms] [instructions] requires an active model."
                    .to_string();
            }
            let (max_cycles, max_steps_per_cycle, interval_ms, instructions) =
                match parse_background_hybrid_cycle_args(rest) {
                    Ok(parsed) => parsed,
                    Err(usage) => return usage,
                };
            match start_background_hybrid_cycle_scheduler(
                project_root,
                tool_context.clone(),
                provider,
                tool_context.model.clone(),
                max_cycles,
                max_steps_per_cycle,
                interval_ms,
                instructions,
            ) {
                Ok(started) => format!(
                    "Started Lab hybrid-cycle background scheduler for {}.\nMax cycles: {}\nInterval ms: {}",
                    started.lab_run_id, started.max_steps, started.interval_ms
                ),
                Err(err) => {
                    format!("Failed to start Lab hybrid-cycle background scheduler: {err}")
                }
            }
        }
        "stop" => match stop_background_scheduler(project_root) {
            Ok(state) => format!(
                "Stopped Lab background scheduler for {}.\nStatus: {:?}",
                state.lab_run_id, state.status
            ),
            Err(err) => format!("Failed to stop Lab background scheduler: {err}"),
        },
        "recover" => match LabStore::for_project(project_root).recover_interrupted_scheduler() {
            Ok(Some(state)) => format!(
                "Recovered interrupted Lab background scheduler for {}.\nStatus: {:?}\nStop reason: {}",
                state.lab_run_id,
                state.status,
                state.stop_reason.as_deref().unwrap_or("none")
            ),
            Ok(None) => "No interrupted Lab background scheduler found.".to_string(),
            Err(err) => format!("Failed to recover Lab background scheduler: {err}"),
        },
        _ => {
            "Usage: /lab background [status|start [max_steps] [interval_ms]|hybrid [max_steps] [interval_ms] [instructions]|hybrid-cycles [max_cycles] [max_steps_per_cycle] [interval_ms] [instructions]|stop|recover]"
                .to_string()
        }
    }
}

fn parse_background_hybrid_args(rest: &str) -> Result<(usize, u64, String), String> {
    let usage =
        "Usage: /lab background hybrid [max_steps] [interval_ms] [instructions]".to_string();
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return Ok((
            default_background_max_steps(),
            default_background_interval_ms(),
            String::new(),
        ));
    }
    let mut parts = trimmed.split_whitespace().collect::<Vec<_>>();
    let first = parts[0];
    let Ok(max_steps) = first.parse::<usize>() else {
        return Ok((
            default_background_max_steps(),
            default_background_interval_ms(),
            trimmed.to_string(),
        ));
    };
    parts.remove(0);
    let mut interval_ms = default_background_interval_ms();
    if let Some(next) = parts.first().copied() {
        if let Ok(parsed) = next.parse::<u64>() {
            interval_ms = parsed;
            parts.remove(0);
        }
    }
    if max_steps == 0 || interval_ms == 0 {
        return Err(usage);
    }
    Ok((max_steps, interval_ms, parts.join(" ")))
}

fn parse_background_hybrid_cycle_args(rest: &str) -> Result<(usize, usize, u64, String), String> {
    let usage =
        "Usage: /lab background hybrid-cycles [max_cycles] [max_steps_per_cycle] [interval_ms] [instructions]"
            .to_string();
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return Ok((2, 5, default_background_interval_ms(), String::new()));
    }
    let mut parts = trimmed.split_whitespace().collect::<Vec<_>>();
    let first = parts[0];
    let Ok(max_cycles) = first.parse::<usize>() else {
        return Ok((2, 5, default_background_interval_ms(), trimmed.to_string()));
    };
    if max_cycles == 0 {
        return Err(usage);
    }
    parts.remove(0);
    let mut max_steps_per_cycle = 5usize;
    if let Some(next) = parts.first().copied() {
        if let Ok(parsed) = next.parse::<usize>() {
            if parsed == 0 {
                return Err(usage);
            }
            max_steps_per_cycle = parsed;
            parts.remove(0);
        }
    }
    let mut interval_ms = default_background_interval_ms();
    if let Some(next) = parts.first().copied() {
        if let Ok(parsed) = next.parse::<u64>() {
            if parsed == 0 {
                return Err(usage);
            }
            interval_ms = parsed;
            parts.remove(0);
        }
    }
    Ok((
        max_cycles,
        max_steps_per_cycle,
        interval_ms,
        parts.join(" "),
    ))
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
    lines.extend(graduate_cleanup_state_lines(&dispatches, 5));
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

fn handle_daemon_command(project_root: &Path, store: &LabStore, args: &str) -> String {
    let (action, rest) = split_once(args);
    match action {
        "" | "status" => match store.load_daemon_state() {
            Ok(Some(state)) => [
                format!("Lab daemon policy: {}", state.project_root),
                format!("Enabled: {}", state.enabled),
                format!("Mode: {:?}", state.mode),
                format!("Max steps: {}", state.max_steps),
                format!("Max steps per cycle: {}", state.max_steps_per_cycle),
                format!("Interval ms: {}", state.interval_ms),
                format!(
                    "Instructions: {}",
                    if state.instructions.is_empty() {
                        "none"
                    } else {
                        state.instructions.as_str()
                    }
                ),
                format!(
                    "Last enabled: {}",
                    state
                        .last_enabled_at
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_else(|| "none".to_string())
                ),
                format!(
                    "Last disabled: {}",
                    state
                        .last_disabled_at
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_else(|| "none".to_string())
                ),
                format!(
                    "Last started: {}",
                    state
                        .last_started_at
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_else(|| "none".to_string())
                ),
                format!(
                    "Last started LabRun: {}",
                    state
                        .last_started_lab_run_id
                        .as_deref()
                        .unwrap_or("none")
                ),
                format!(
                    "Last start error: {}",
                    state.last_start_error.as_deref().unwrap_or("none")
                ),
                format!(
                    "Last message: {}",
                    state.last_message.unwrap_or_else(|| "none".to_string())
                ),
            ]
            .join("\n"),
            Ok(None) => "No Lab daemon policy found.".to_string(),
            Err(err) => format!("Failed to read Lab daemon policy: {err}"),
        },
        "enable" => {
            let (mode, max_steps, max_steps_per_cycle, interval_ms, instructions) =
                match parse_daemon_enable_args(rest) {
                Ok(parsed) => parsed,
                Err(message) => return message,
            };
            match store.enable_daemon_with_cycle_bound(
                mode,
                max_steps,
                max_steps_per_cycle,
                interval_ms,
                instructions,
            ) {
                Ok(state) => format!(
                    "Enabled Lab daemon policy.\nMode: {:?}\nMax steps: {}\nMax steps per cycle: {}\nInterval ms: {}\nInstructions: {}",
                    state.mode,
                    state.max_steps,
                    state.max_steps_per_cycle,
                    state.interval_ms,
                    if state.instructions.is_empty() {
                        "none"
                    } else {
                        state.instructions.as_str()
                    }
                ),
                Err(err) => format!("Failed to enable Lab daemon policy: {err}"),
            }
        }
        "start" => {
            "Use /lab daemon start from the interactive shell so the daemon can access the active provider and ToolContext."
                .to_string()
        }
        "health" => handle_daemon_health_command(project_root, store),
        "launchd" => handle_daemon_launchd_command(store, rest),
        "service" => handle_daemon_service_command(store, rest),
        "disable" => match store.disable_daemon(rest) {
            Ok(state) => format!(
                "Disabled Lab daemon policy.\nLast message: {}",
                state.last_message.unwrap_or_else(|| "none".to_string())
            ),
            Err(err) => format!("Failed to disable Lab daemon policy: {err}"),
        },
        _ => {
            "Usage: /lab daemon [status|health|enable [strict|hybrid|hybrid-cycles] [max_steps] [max_steps_per_cycle] [interval_ms] [instructions]|start|launchd [label]|service [status|install|uninstall|load|unload|restart|supervise|commands] [label]|disable [reason]]"
                .to_string()
        }
    }
}

fn handle_daemon_health_command(project_root: &Path, store: &LabStore) -> String {
    let daemon = match store.load_daemon_state() {
        Ok(Some(state)) => state,
        Ok(None) => return "Lab daemon health: no_policy\nNo Lab daemon policy found.".to_string(),
        Err(err) => return format!("Failed to read Lab daemon health: {err}"),
    };
    let scheduler_status = background_scheduler_status(project_root).ok();
    let persisted_scheduler = scheduler_status
        .as_ref()
        .and_then(|status| status.persisted.as_ref());
    let scheduler_label = persisted_scheduler
        .map(|state| format!("{:?}", state.status))
        .unwrap_or_else(|| "none".to_string());
    let running_in_process = scheduler_status
        .as_ref()
        .map(|status| status.running_in_process)
        .unwrap_or(false);
    let health = if !daemon.enabled {
        "disabled"
    } else if daemon.last_start_error.is_some() {
        "unhealthy_start_error"
    } else if running_in_process {
        "running_in_process"
    } else if let Some(state) = persisted_scheduler {
        match state.status {
            crate::lab::model::LabSchedulerStatus::Running => "running_persisted",
            crate::lab::model::LabSchedulerStatus::Stopping => "stopping",
            crate::lab::model::LabSchedulerStatus::PausedRestart => "paused_restart",
            crate::lab::model::LabSchedulerStatus::Blocked => "attention_blocked",
            crate::lab::model::LabSchedulerStatus::NeedsUser => "needs_user",
            crate::lab::model::LabSchedulerStatus::Failed => "unhealthy_failed",
            crate::lab::model::LabSchedulerStatus::Completed => "completed",
            crate::lab::model::LabSchedulerStatus::Stopped => "stopped",
            crate::lab::model::LabSchedulerStatus::Idle => "idle",
        }
    } else if daemon.last_started_at.is_none() {
        "enabled_not_started"
    } else {
        "enabled_no_scheduler_state"
    };
    let lifecycle = match store.load_app_lifecycle_state() {
        Ok(Some(state)) => state
            .last_message
            .unwrap_or_else(|| "lifecycle checkpoint recorded".to_string()),
        Ok(None) => "none".to_string(),
        Err(err) => format!("unavailable ({err})"),
    };
    let launchd_label = default_launchd_label(store);
    let launchd_plist = store.root().join("launchd").join(format!(
        "{}.plist",
        safe_launchd_label_component(&launchd_label)
    ));
    [
        format!("Lab daemon health: {health}"),
        format!(
            "Policy: enabled={} mode={:?} max_steps={} max_steps_per_cycle={} interval_ms={}",
            daemon.enabled,
            daemon.mode,
            daemon.max_steps,
            daemon.max_steps_per_cycle,
            daemon.interval_ms
        ),
        format!("Scheduler: running_in_process={running_in_process} persisted={scheduler_label}"),
        format!(
            "Last started: {}",
            daemon
                .last_started_at
                .map(|time| time.to_rfc3339())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "Last started LabRun: {}",
            daemon.last_started_lab_run_id.as_deref().unwrap_or("none")
        ),
        format!(
            "Last start error: {}",
            daemon.last_start_error.as_deref().unwrap_or("none")
        ),
        format!(
            "Last message: {}",
            daemon.last_message.as_deref().unwrap_or("none")
        ),
        format!("Lifecycle: {lifecycle}"),
        format!("LaunchAgent plist: {}", launchd_plist.display()),
        format!("LaunchAgent exists: {}", launchd_plist.exists()),
    ]
    .join("\n")
}

fn handle_daemon_launchd_command(store: &LabStore, args: &str) -> String {
    let label = if args.trim().is_empty() {
        default_launchd_label(store)
    } else {
        safe_launchd_label_component(args.trim())
    };
    match write_daemon_launchd_plist(store, &label) {
        Ok(path) => format!(
            "Wrote Lab daemon LaunchAgent plist.\nLabel: {}\nPlist: {}\nInstall hint: launchctl bootstrap gui/$(id -u) {}\nRun hint: launchctl kickstart -k gui/$(id -u)/{}",
            label,
            path.display(),
            path.display(),
            label
        ),
        Err(err) => format!("Failed to write Lab daemon LaunchAgent plist: {err}"),
    }
}

fn handle_daemon_service_command(store: &LabStore, args: &str) -> String {
    let (action, rest) = split_once(args.trim());
    let action = if action.is_empty() { "status" } else { action };
    let label = if rest.trim().is_empty() {
        default_launchd_label(store)
    } else {
        safe_launchd_label_component(rest.trim())
    };

    match action {
        "status" => daemon_service_status(store, &label),
        "commands" => daemon_service_commands(store, &label),
        "install" => match install_daemon_service_plist(store, &label) {
            Ok(paths) => format!(
                "Installed Lab daemon LaunchAgent plist.\n{}",
                daemon_service_lines(
                    store,
                    &label,
                    &paths.generated_plist,
                    &paths.installed_plist
                )
                .join("\n")
            ),
            Err(err) => format!("Failed to install Lab daemon service plist: {err}"),
        },
        "uninstall" => match uninstall_daemon_service_plist(&label) {
            Ok(removed) => {
                let paths = daemon_service_paths(store, &label);
                format!(
                    "Uninstalled Lab daemon LaunchAgent plist.\nRemoved: {}\n{}",
                    removed,
                    daemon_service_lines(
                        store,
                        &label,
                        &paths.generated_plist,
                        &paths.installed_plist
                    )
                    .join("\n")
                )
            }
            Err(err) => format!("Failed to uninstall Lab daemon service plist: {err}"),
        },
        "load" => match load_daemon_service(store, &label) {
            Ok(result) => format!("Loaded Lab daemon service.\n{}", result.format()),
            Err(err) => format!("Failed to load Lab daemon service: {err}"),
        },
        "unload" => match unload_daemon_service(&label) {
            Ok(result) => format!("Unloaded Lab daemon service.\n{}", result.format()),
            Err(err) => format!("Failed to unload Lab daemon service: {err}"),
        },
        "restart" | "kickstart" => match restart_daemon_service(&label) {
            Ok(result) => format!("Restarted Lab daemon service.\n{}", result.format()),
            Err(err) => format!("Failed to restart Lab daemon service: {err}"),
        },
        "supervise" => match supervise_daemon_service(store, &label) {
            Ok(report) => report,
            Err(err) => format!("Failed to supervise Lab daemon service: {err}"),
        },
        _ => "Usage: /lab daemon service [status|install|uninstall|load|unload|restart|supervise|commands] [label]".to_string(),
    }
}

struct DaemonServicePaths {
    generated_plist: PathBuf,
    installed_plist: PathBuf,
}

fn daemon_service_status(store: &LabStore, label: &str) -> String {
    let paths = daemon_service_paths(store, label);
    [
        vec!["Lab daemon service status.".to_string()],
        daemon_service_lines(store, label, &paths.generated_plist, &paths.installed_plist),
    ]
    .concat()
    .join("\n")
}

fn daemon_service_commands(store: &LabStore, label: &str) -> String {
    let paths = daemon_service_paths(store, label);
    daemon_service_lines(store, label, &paths.generated_plist, &paths.installed_plist).join("\n")
}

fn install_daemon_service_plist(
    store: &LabStore,
    label: &str,
) -> anyhow::Result<DaemonServicePaths> {
    let generated_plist = write_daemon_launchd_plist(store, label)?;
    let installed_plist = launch_agent_install_path(label)?;
    if let Some(parent) = installed_plist.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&generated_plist, &installed_plist)?;
    Ok(DaemonServicePaths {
        generated_plist,
        installed_plist,
    })
}

fn uninstall_daemon_service_plist(label: &str) -> anyhow::Result<bool> {
    let installed_plist = launch_agent_install_path(label)?;
    if installed_plist.exists() {
        fs::remove_file(installed_plist)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn load_daemon_service(store: &LabStore, label: &str) -> anyhow::Result<LaunchctlResult> {
    let paths = install_daemon_service_plist(store, label)?;
    let domain = launchctl_gui_domain()?;
    run_launchctl(&[
        "bootstrap".to_string(),
        domain,
        paths.installed_plist.display().to_string(),
    ])
}

fn unload_daemon_service(label: &str) -> anyhow::Result<LaunchctlResult> {
    let target = launchctl_label_target(label)?;
    run_launchctl(&["bootout".to_string(), target])
}

fn restart_daemon_service(label: &str) -> anyhow::Result<LaunchctlResult> {
    let target = launchctl_label_target(label)?;
    run_launchctl(&["kickstart".to_string(), "-k".to_string(), target])
}

fn print_daemon_service(label: &str) -> anyhow::Result<LaunchctlResult> {
    let target = launchctl_label_target(label)?;
    run_launchctl_status(&["print".to_string(), target])
}

fn supervise_daemon_service(store: &LabStore, label: &str) -> anyhow::Result<String> {
    let Some(policy) = store.load_daemon_state()? else {
        return Ok("Lab daemon service supervision skipped: no daemon policy.".to_string());
    };
    if !policy.enabled {
        return Ok("Lab daemon service supervision skipped: daemon policy disabled.".to_string());
    }
    let print = print_daemon_service(label)?;
    if print.success {
        return Ok(format!(
            "Lab daemon service supervision healthy.\n{}",
            print.format()
        ));
    }
    let load = load_daemon_service(store, label)?;
    Ok(format!(
        "Lab daemon service supervision repaired missing service.\nPrint check:\n{}\nRepair:\n{}",
        print.format(),
        load.format()
    ))
}

struct LaunchctlResult {
    command: String,
    success: bool,
    status_code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl LaunchctlResult {
    fn format(&self) -> String {
        [
            format!("Command: {}", self.command),
            format!(
                "Exit status: {}",
                self.status_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ),
            format!("Stdout: {}", compact_command_output(&self.stdout)),
            format!("Stderr: {}", compact_command_output(&self.stderr)),
        ]
        .join("\n")
    }
}

fn run_launchctl(args: &[String]) -> anyhow::Result<LaunchctlResult> {
    let result = run_launchctl_status(args)?;
    if result.success {
        Ok(result)
    } else {
        anyhow::bail!("{}", result.format())
    }
}

fn run_launchctl_status(args: &[String]) -> anyhow::Result<LaunchctlResult> {
    let bin = launchctl_bin();
    let output = Command::new(&bin).args(args).output()?;
    let command = format!(
        "{} {}",
        bin.display(),
        args.iter()
            .map(|arg| shell_display_arg(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );
    Ok(LaunchctlResult {
        command,
        success: output.status.success(),
        status_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn launchctl_bin() -> PathBuf {
    std::env::var_os("PRIORITY_AGENT_LAUNCHCTL_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("launchctl"))
}

fn launchctl_gui_domain() -> anyhow::Result<String> {
    if let Ok(domain) = std::env::var("PRIORITY_AGENT_LAUNCHCTL_DOMAIN") {
        let trimmed = domain.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    #[cfg(unix)]
    {
        Ok(format!("gui/{}", unsafe { libc::getuid() }))
    }
    #[cfg(not(unix))]
    {
        let uid = std::env::var("UID")
            .map_err(|_| anyhow::anyhow!("UID is not set; cannot build launchctl gui domain"))?;
        Ok(format!("gui/{uid}"))
    }
}

fn launchctl_label_target(label: &str) -> anyhow::Result<String> {
    Ok(format!(
        "{}/{}",
        launchctl_gui_domain()?,
        safe_launchd_label_component(label)
    ))
}

fn daemon_service_paths(store: &LabStore, label: &str) -> DaemonServicePaths {
    DaemonServicePaths {
        generated_plist: store
            .root()
            .join("launchd")
            .join(format!("{}.plist", safe_launchd_label_component(label))),
        installed_plist: launch_agent_install_path(label).unwrap_or_else(|_| {
            PathBuf::from("~/Library/LaunchAgents")
                .join(format!("{}.plist", safe_launchd_label_component(label)))
        }),
    }
}

fn launch_agent_install_path(label: &str) -> anyhow::Result<PathBuf> {
    let dir = launch_agents_dir()?;
    Ok(dir.join(format!("{}.plist", safe_launchd_label_component(label))))
}

fn launch_agents_dir() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("PRIORITY_AGENT_LAUNCH_AGENTS_DIR") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| anyhow::anyhow!("HOME is not set; cannot resolve ~/Library/LaunchAgents"))?;
    Ok(PathBuf::from(home).join("Library").join("LaunchAgents"))
}

fn daemon_service_lines(
    store: &LabStore,
    label: &str,
    generated_plist: &Path,
    installed_plist: &Path,
) -> Vec<String> {
    vec![
        format!("Label: {label}"),
        format!("Generated plist: {}", generated_plist.display()),
        format!("Generated exists: {}", generated_plist.exists()),
        format!("Installed plist: {}", installed_plist.display()),
        format!("Installed exists: {}", installed_plist.exists()),
        format!(
            "Install command: /lab daemon service install {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Uninstall command: launchctl bootout gui/$(id -u)/{} && /lab daemon service uninstall {}",
            safe_launchd_label_component(label),
            safe_launchd_label_component(label)
        ),
        format!(
            "Bootstrap command: launchctl bootstrap gui/$(id -u) {}",
            installed_plist.display()
        ),
        format!(
            "Kickstart command: launchctl kickstart -k gui/$(id -u)/{}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Load command: /lab daemon service load {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Unload command: /lab daemon service unload {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Restart command: /lab daemon service restart {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Supervise command: /lab daemon service supervise {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Print command: launchctl print gui/$(id -u)/{}",
            safe_launchd_label_component(label)
        ),
        format!("Health command: /lab daemon health"),
        format!("Project root: {}", store.project_root().display()),
    ]
}

fn compact_command_output(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "none".to_string()
    } else {
        compact_message_line(trimmed, 240)
    }
}

fn shell_display_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn write_daemon_launchd_plist(store: &LabStore, label: &str) -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let launchd_dir = store.root().join("launchd");
    fs::create_dir_all(&launchd_dir)?;
    let plist_path = launchd_dir.join(format!("{}.plist", safe_launchd_label_component(label)));
    let stdout_path = store.root().join("daemon.out.log");
    let stderr_path = store.root().join("daemon.err.log");
    let plist = render_launchd_plist(
        label,
        &exe,
        store.project_root(),
        &stdout_path,
        &stderr_path,
    );
    fs::write(&plist_path, plist)?;
    Ok(plist_path)
}

fn default_launchd_label(store: &LabStore) -> String {
    let project = store
        .project_root()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    format!(
        "com.priority-agent.lab.{}",
        safe_launchd_label_component(project)
    )
}

fn safe_launchd_label_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_was_dash = false;
    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' {
            last_was_dash = false;
            Some(ch.to_ascii_lowercase())
        } else if !last_was_dash {
            last_was_dash = true;
            Some('-')
        } else {
            None
        };
        if let Some(ch) = normalized {
            out.push(ch);
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}

fn render_launchd_plist(
    label: &str,
    executable: &Path,
    working_directory: &Path,
    stdout_path: &Path,
    stderr_path: &Path,
) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>lab-daemon</string>
  </array>
  <key>WorkingDirectory</key>
  <string>{}</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
  <key>StandardOutPath</key>
  <string>{}</string>
  <key>StandardErrorPath</key>
  <string>{}</string>
</dict>
</plist>
"#,
        xml_escape(label),
        xml_escape(&executable.display().to_string()),
        xml_escape(&working_directory.display().to_string()),
        xml_escape(&stdout_path.display().to_string()),
        xml_escape(&stderr_path.display().to_string())
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn parse_daemon_enable_args(
    args: &str,
) -> Result<(LabDaemonMode, usize, usize, u64, &str), String> {
    let trimmed = args.trim();
    let default_max_steps = default_background_max_steps();
    let default_max_steps_per_cycle = 5usize;
    let default_interval_ms = default_background_interval_ms();
    if trimmed.is_empty() {
        return Ok((
            LabDaemonMode::Strict,
            default_max_steps,
            default_max_steps_per_cycle,
            default_interval_ms,
            "",
        ));
    }

    let (first, rest) = split_once(trimmed);
    let (mode, rest) = match first.to_ascii_lowercase().as_str() {
        "strict" => (LabDaemonMode::Strict, rest.trim()),
        "hybrid" => (LabDaemonMode::Hybrid, rest.trim()),
        "hybrid-cycles" | "hybrid_cycles" | "cycles" => (LabDaemonMode::HybridCycles, rest.trim()),
        _ => (LabDaemonMode::Strict, trimmed),
    };
    if rest.is_empty() {
        return Ok((
            mode,
            default_max_steps,
            default_max_steps_per_cycle,
            default_interval_ms,
            "",
        ));
    }
    let (first_numeric, after_steps) = split_once(rest);
    let max_steps = match first_numeric.parse::<usize>() {
        Ok(value) if value > 0 => value,
        Ok(_) => return Err(
            "Usage: /lab daemon enable [strict|hybrid|hybrid-cycles] [max_steps] [max_steps_per_cycle] [interval_ms] [instructions]"
                .to_string(),
        ),
        Err(_) => {
            return Ok((
                mode,
                default_max_steps,
                default_max_steps_per_cycle,
                default_interval_ms,
                rest,
            ))
        }
    };
    let after_steps = after_steps.trim();
    if after_steps.is_empty() {
        return Ok((
            mode,
            max_steps,
            default_max_steps_per_cycle,
            default_interval_ms,
            "",
        ));
    }
    if mode == LabDaemonMode::HybridCycles {
        let (second_numeric, after_cycle_steps) = split_once(after_steps);
        let max_steps_per_cycle = match second_numeric.parse::<usize>() {
            Ok(value) if value > 0 => value,
            Ok(_) => return Err(
                "Usage: /lab daemon enable hybrid-cycles [max_cycles] [max_steps_per_cycle] [interval_ms] [instructions]"
                    .to_string(),
            ),
            Err(_) => {
                return Ok((
                    mode,
                    max_steps,
                    default_max_steps_per_cycle,
                    default_interval_ms,
                    after_steps,
                ))
            }
        };
        let after_cycle_steps = after_cycle_steps.trim();
        if after_cycle_steps.is_empty() {
            return Ok((
                mode,
                max_steps,
                max_steps_per_cycle,
                default_interval_ms,
                "",
            ));
        }
        let (third_numeric, instructions) = split_once(after_cycle_steps);
        let interval_ms = match third_numeric.parse::<u64>() {
            Ok(value) if value > 0 => value,
            Ok(_) => return Err(
                "Usage: /lab daemon enable hybrid-cycles [max_cycles] [max_steps_per_cycle] [interval_ms] [instructions]"
                    .to_string(),
            ),
            Err(_) => {
                return Ok((
                    mode,
                    max_steps,
                    max_steps_per_cycle,
                    default_interval_ms,
                    after_cycle_steps,
                ))
            }
        };
        return Ok((
            mode,
            max_steps,
            max_steps_per_cycle,
            interval_ms,
            instructions,
        ));
    }
    let (second_numeric, instructions) = split_once(after_steps);
    let interval_ms = match second_numeric.parse::<u64>() {
        Ok(value) if value > 0 => value,
        Ok(_) => return Err(
            "Usage: /lab daemon enable [strict|hybrid|hybrid-cycles] [max_steps] [interval_ms] [instructions]"
                .to_string(),
        ),
        Err(_) => {
            return Ok((
                mode,
                max_steps,
                default_max_steps_per_cycle,
                default_interval_ms,
                after_steps,
            ))
        }
    };
    Ok((
        mode,
        max_steps,
        default_max_steps_per_cycle,
        interval_ms,
        instructions,
    ))
}

fn handle_gate_command(orchestrator: &LabOrchestrator, args: &str) -> String {
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

fn handle_artifact_accept_command(orchestrator: &LabOrchestrator, args: &str) -> String {
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

fn handle_artifact_revise_command(orchestrator: &LabOrchestrator, args: &str) -> String {
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

fn handle_review_command(orchestrator: &LabOrchestrator, store: &LabStore, args: &str) -> String {
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

fn graduate_cleanup_state_lines(
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

fn lab_status(store: &LabStore) -> String {
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

fn handle_meeting_command(
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

fn handle_runs_command(store: &LabStore) -> String {
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

fn handle_report_command(store: &LabStore, args: &str) -> String {
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

fn handle_dashboard_command(
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

fn handle_task_command(
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

fn format_error_chain(err: &anyhow::Error) -> String {
    err.chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
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

fn handle_sponsor_messages_command(
    orchestrator: &LabOrchestrator,
    store: &LabStore,
    args: &str,
) -> String {
    let (action, rest) = split_once(args);
    match action {
        "" | "list" | "messages" => list_sponsor_messages(store),
        "review" | "reviewed" => {
            update_sponsor_message_status(store, rest, SponsorMessageStatus::Reviewed, "review")
        }
        "meeting" => update_sponsor_message_status(
            store,
            rest,
            SponsorMessageStatus::ConvertedToMeeting,
            "meeting",
        ),
        "task" => update_sponsor_message_status(
            store,
            rest,
            SponsorMessageStatus::ConvertedToTask,
            "task",
        ),
        "reject" | "rejected" => {
            update_sponsor_message_status(store, rest, SponsorMessageStatus::Rejected, "reject")
        }
        "decision" | "decide" => render_sponsor_message_decision(store, rest),
        "classify" => {
            "Usage: /lab messages classify <message_id|latest> [instructions] requires the Lab Mode shell provider."
                .to_string()
        }
        "apply" => apply_sponsor_message(orchestrator, store, rest),
        _ => "Usage: /lab messages [list|classify|decision|review|meeting|task|reject|apply <message_id> [note]]"
            .to_string(),
    }
}

fn list_sponsor_messages(store: &LabStore) -> String {
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found.".to_string(),
        Err(err) => return format!("Failed to load LabRun: {err}"),
    };
    match store.list_sponsor_messages(&run.lab_run_id) {
        Ok(messages) if messages.is_empty() => {
            format!(
                "Professor side-channel inbox is empty for {}.",
                run.lab_run_id
            )
        }
        Ok(messages) => {
            let mut lines = vec![
                format!("Professor side-channel inbox: {}", run.lab_run_id),
                format!("Messages: {}", messages.len()),
            ];
            for message in messages.iter().rev().take(10) {
                lines.push(format!(
                    "- {} [{:?}/{:?}/{}] {}",
                    message.message_id,
                    message.message_type,
                    message.status,
                    message.urgency,
                    compact_message_line(&message.body, 160)
                ));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to list professor side-channel messages: {err}"),
    }
}

fn render_sponsor_message_decision(store: &LabStore, args: &str) -> String {
    let message_id = args.trim();
    if message_id.is_empty() {
        return "Usage: /lab messages decision <message_id|latest>".to_string();
    }
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found.".to_string(),
        Err(err) => return format!("Failed to load LabRun: {err}"),
    };
    let messages = match store.list_sponsor_messages(&run.lab_run_id) {
        Ok(messages) => messages,
        Err(err) => return format!("Failed to list professor side-channel messages: {err}"),
    };
    let Some(message) = select_sponsor_message(&messages, message_id) else {
        return format!("Professor side-channel message not found: {message_id}");
    };
    let (decision, next_action) = sponsor_message_decision_label(message.status);
    let artifact =
        build_professor_steering_decision_artifact(&run.lab_run_id, message, decision, next_action);
    let artifact_id = artifact.artifact_id().to_string();
    let report_path = match store
        .write_stage_artifact(&artifact)
        .and_then(|_| store.write_stage_artifact_report(&artifact))
    {
        Ok(path) => path,
        Err(err) => return format!("Failed to write Professor steering decision: {err}"),
    };
    [
        format!("Professor steering decision: {}", artifact_id),
        format!("Source message: {}", message.message_id),
        format!("Decision: {decision}"),
        format!("Status: {:?}", message.status),
        format!("Type: {:?}", message.message_type),
        format!("Urgency: {}", message.urgency),
        format!("Next action: {next_action}"),
        format!("Report: {}", report_path.display()),
        format!("Message: {}", compact_message_line(&message.body, 240)),
    ]
    .join("\n")
}

fn build_professor_steering_decision_artifact(
    lab_run_id: &str,
    message: &crate::lab::model::SponsorMessage,
    decision: &str,
    next_action: &str,
) -> StageArtifact {
    let decision_id = format!("profsteer_{}", Uuid::new_v4().simple());
    let artifact_id = format!(
        "artifact_professorsteeringdecision_{}",
        Uuid::new_v4().simple()
    );
    let mut artifact = StageArtifact::ProfessorSteeringDecision(LabArtifactEnvelope::new(
        artifact_id,
        lab_run_id.to_string(),
        LabArtifactType::ProfessorSteeringDecision,
        format!("Professor steering decision for {}", message.message_id),
        Utc::now(),
        ProfessorSteeringDecision {
            decision_id,
            source_message_id: message.message_id.clone(),
            decision: decision.to_string(),
            status: message.status,
            message_type: message.message_type,
            urgency: message.urgency.clone(),
            rationale: sponsor_message_decision_rationale(message.status).to_string(),
            next_action: next_action.to_string(),
            message_summary: compact_message_line(&message.body, 240),
        },
    ));
    artifact.set_review_state(
        LabArtifactStatus::ReadyForHandoff,
        Some("decision_recorded_not_applied".to_string()),
    );
    artifact
}

fn sponsor_message_decision_rationale(status: SponsorMessageStatus) -> &'static str {
    match status {
        SponsorMessageStatus::Queued => "Professor has not classified this message yet.",
        SponsorMessageStatus::Reviewed => {
            "Professor reviewed the message and found no workflow mutation pending."
        }
        SponsorMessageStatus::ConvertedToMeeting => {
            "Professor classified the message as a meeting-worthy topic."
        }
        SponsorMessageStatus::ConvertedToTask => {
            "Professor classified the message as a follow-up implementation task."
        }
        SponsorMessageStatus::Applied => "The steering decision has already been applied.",
        SponsorMessageStatus::Rejected => "Professor rejected the message for LabRun action.",
        SponsorMessageStatus::Superseded => "A newer message or decision superseded this one.",
    }
}

fn sponsor_message_decision_label(status: SponsorMessageStatus) -> (&'static str, &'static str) {
    match status {
        SponsorMessageStatus::Queued => (
            "pending_professor_review",
            "Classify with /lab messages classify <message_id> or mark review/meeting/task/reject.",
        ),
        SponsorMessageStatus::Reviewed => (
            "no_change",
            "No LabRun state change is pending; current loop can continue.",
        ),
        SponsorMessageStatus::ConvertedToMeeting => (
            "open_lab_meeting",
            "Apply with /lab messages apply <message_id> to write a read-only meeting report.",
        ),
        SponsorMessageStatus::ConvertedToTask => (
            "create_postdoc_task",
            "Apply with /lab messages apply <message_id> to create a blocked task for postdoc scoping.",
        ),
        SponsorMessageStatus::Applied => (
            "applied",
            "Decision has already been applied; inspect reports or tasks for resulting artifacts.",
        ),
        SponsorMessageStatus::Rejected => (
            "reject",
            "No LabRun action should be taken from this message.",
        ),
        SponsorMessageStatus::Superseded => (
            "superseded",
            "A newer sponsor message or decision replaced this one.",
        ),
    }
}

fn select_sponsor_message<'a>(
    messages: &'a [crate::lab::model::SponsorMessage],
    message_id: &str,
) -> Option<&'a crate::lab::model::SponsorMessage> {
    if message_id == "latest" {
        return messages.iter().rev().next();
    }
    messages
        .iter()
        .find(|message| message.message_id == message_id)
}

fn update_sponsor_message_status(
    store: &LabStore,
    args: &str,
    status: SponsorMessageStatus,
    action: &str,
) -> String {
    let (message_id, note) = split_once(args);
    if message_id.trim().is_empty() {
        return format!("Usage: /lab messages {action} <message_id> [note]");
    }
    match store.update_latest_sponsor_message_status(message_id, status, note) {
        Ok(message) => format!(
            "Professor side-channel message updated: {}\nStatus: {:?}\nType: {:?}",
            message.message_id, message.status, message.message_type
        ),
        Err(err) => format!("Failed to update professor side-channel message: {err}"),
    }
}

fn apply_sponsor_message(orchestrator: &LabOrchestrator, store: &LabStore, args: &str) -> String {
    let (message_id, note) = split_once(args);
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return "Usage: /lab messages apply <message_id> [note]".to_string();
    }
    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => return "No LabRun found.".to_string(),
        Err(err) => return format!("Failed to load LabRun: {err}"),
    };
    let messages = match store.list_sponsor_messages(&run.lab_run_id) {
        Ok(messages) => messages,
        Err(err) => return format!("Failed to list professor side-channel messages: {err}"),
    };
    let Some(message) = select_sponsor_message(&messages, message_id) else {
        return format!("Professor side-channel message not found: {message_id}");
    };

    match message.status {
        SponsorMessageStatus::ConvertedToMeeting => {
            let topic = if note.trim().is_empty() {
                format!("Sponsor message {}: {}", message.message_id, message.body)
            } else {
                note.trim().to_string()
            };
            match orchestrator.create_meeting_summary_for_latest(Some(&topic)) {
                Ok(created) => {
                    let _ = store.update_latest_sponsor_message_status(
                        &message.message_id,
                        SponsorMessageStatus::Applied,
                        &format!("meeting_artifact={}", created.artifact.artifact_id()),
                    );
                    format!(
                        "Professor side-channel message applied as meeting: {}\nArtifact: {}\nReport: {}",
                        message.message_id,
                        created.artifact.artifact_id(),
                        created.report_path.display()
                    )
                }
                Err(err) => format!("Failed to apply professor message as meeting: {err}"),
            }
        }
        SponsorMessageStatus::ConvertedToTask => {
            let title = if note.trim().is_empty() {
                compact_message_line(&message.body, 80)
            } else {
                note.trim().to_string()
            };
            match store.create_graduate_task(
                &run.lab_run_id,
                &title,
                &format!(
                    "Sponsor-requested task from professor side channel {}: {}",
                    message.message_id, message.body
                ),
                Vec::new(),
                vec!["Postdoc must set allowed_scope before graduate execution.".to_string()],
            ) {
                Ok(task) => {
                    if let Err(err) = store.block_graduate_task(
                        &run.lab_run_id,
                        &task.task_id,
                        "Converted from sponsor message; postdoc must assign allowed_scope before execution.",
                    ) {
                        return format!("Failed to block converted graduate task: {err}");
                    }
                    let _ = store.update_latest_sponsor_message_status(
                        &message.message_id,
                        SponsorMessageStatus::Applied,
                        &format!("graduate_task={}", task.task_id),
                    );
                    format!(
                        "Professor side-channel message applied as blocked graduate task: {}\nTask: {}\nReason: postdoc must assign allowed_scope before execution",
                        message.message_id, task.task_id
                    )
                }
                Err(err) => format!("Failed to apply professor message as task: {err}"),
            }
        }
        other => format!(
            "Professor side-channel message {} is {:?}; mark it as meeting or task before apply.",
            message.message_id, other
        ),
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
    match store.record_evidence_ref(
        &run.lab_run_id,
        kind,
        run.internal_owner,
        reference,
        summary,
        run.resume_cursor.active_artifact_id.as_deref(),
        Some(&run.cycle_count.to_string()),
    ) {
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
mod tests {
    use super::*;
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, ToolCall, Usage};
    use async_openai::types::ChatCompletionResponseStream;
    use async_trait::async_trait;
    use std::path::Path;
    use std::sync::Arc;

    fn lab_command_git(cwd: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .unwrap_or_else(|err| panic!("failed to run git {}: {}", args.join(" "), err));
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn init_lab_command_git_repo(path: &Path) {
        lab_command_git(path, &["init", "-q"]);
        lab_command_git(path, &["config", "user.email", "lab@example.test"]);
        lab_command_git(path, &["config", "user.name", "Lab Test"]);
        std::fs::write(path.join("hello.txt"), "base\n").expect("seed repo file");
        lab_command_git(path, &["add", "hello.txt"]);
        lab_command_git(path, &["commit", "-q", "-m", "initial"]);
    }

    fn drive_lab_command_to_user_report(path: &Path) {
        for stage in [
            "professor_discussion",
            "postdoc_plan",
            "graduate_work",
            "postdoc_review",
            "professor_review",
        ] {
            let planned = handle_lab_command(
                path,
                Some("session".to_string()),
                &format!("plan explicit artifact for {stage}"),
            );
            assert!(
                planned.contains("Gate satisfied"),
                "plan failed at {stage}: {planned}"
            );
            let advanced = handle_lab_command(path, Some("session".to_string()), "advance");
            assert!(
                advanced.contains("Advanced LabRun") || advanced.contains("needs user review"),
                "advance failed at {stage}: {advanced}"
            );
        }
    }

    struct ProposalProvider {
        response: String,
    }

    #[async_trait]
    impl LlmProvider for ProposalProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Ok(ChatResponse {
                content: self.response.clone(),
                tool_calls: None::<Vec<ToolCall>>,
                usage: Some(Usage {
                    prompt_tokens: 12,
                    completion_tokens: 8,
                    total_tokens: 20,
                    reasoning_tokens: None,
                    cached_tokens: None,
                    cache_write_tokens: None,
                }),
                tool_call_repair: None,
                finish_reason: Some("stop".to_string()),
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            unimplemented!("not needed for Lab command tests")
        }

        fn base_url(&self) -> &str {
            "mock://proposal-provider"
        }

        fn default_model(&self) -> &str {
            "mock-proposal"
        }
    }

    struct SequenceCommandProvider {
        responses: parking_lot::Mutex<std::collections::VecDeque<String>>,
    }

    #[async_trait]
    impl LlmProvider for SequenceCommandProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let response = self
                .responses
                .lock()
                .pop_front()
                .unwrap_or_else(|| r#"{"decision":"accept","note":"ok"}"#.to_string());
            Ok(ChatResponse {
                content: response,
                tool_calls: None::<Vec<ToolCall>>,
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 6,
                    total_tokens: 16,
                    reasoning_tokens: None,
                    cached_tokens: None,
                    cache_write_tokens: None,
                }),
                tool_call_repair: None,
                finish_reason: Some("stop".to_string()),
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            unimplemented!("not needed for Lab command tests")
        }

        fn base_url(&self) -> &str {
            "mock://sequence-command-provider"
        }

        fn default_model(&self) -> &str {
            "mock-sequence"
        }
    }

    struct ToolProbeProvider;

    #[async_trait]
    impl LlmProvider for ToolProbeProvider {
        async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let tool_name = request
                .tools
                .as_ref()
                .and_then(|tools| tools.first())
                .map(|tool| tool.name.clone())
                .unwrap_or_else(|| "missing_tool".to_string());
            Ok(ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_probe".to_string(),
                    name: tool_name,
                    arguments: serde_json::json!({"message": "tool probe ok"}),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 4,
                    total_tokens: 14,
                    reasoning_tokens: None,
                    cached_tokens: None,
                    cache_write_tokens: None,
                }),
                tool_call_repair: None,
                finish_reason: Some("tool_calls".to_string()),
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            anyhow::bail!("streaming not supported in ToolProbeProvider")
        }

        fn base_url(&self) -> &str {
            "mock://tool-probe"
        }

        fn default_model(&self) -> &str {
            "mock-tool-probe"
        }
    }

    #[test]
    fn daemon_launchd_plist_renders_worker_entrypoint() {
        let plist = render_launchd_plist(
            "com.priority-agent.lab.demo&one",
            Path::new("/tmp/priority-agent"),
            Path::new("/tmp/project <root>"),
            Path::new("/tmp/lab/out.log"),
            Path::new("/tmp/lab/err.log"),
        );

        assert!(plist.contains("<string>com.priority-agent.lab.demo&amp;one</string>"));
        assert!(plist.contains("<string>/tmp/priority-agent</string>"));
        assert!(plist.contains("<string>lab-daemon</string>"));
        assert!(plist.contains("<key>WorkingDirectory</key>"));
        assert!(plist.contains("<string>/tmp/project &lt;root&gt;</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("<key>KeepAlive</key>"));
        assert!(plist.contains("<string>/tmp/lab/out.log</string>"));
        assert!(plist.contains("<string>/tmp/lab/err.log</string>"));
    }

    #[test]
    fn daemon_enable_accepts_hybrid_cycles_mode() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let enabled = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon enable hybrid-cycles 4 6 500 continue bounded cycles",
        );
        assert!(enabled.contains("Enabled Lab daemon policy"));
        assert!(enabled.contains("Mode: HybridCycles"));
        assert!(enabled.contains("Max steps: 4"));
        assert!(enabled.contains("Max steps per cycle: 6"));
        assert!(enabled.contains("Interval ms: 500"));

        let status = handle_lab_command(temp.path(), Some("session".to_string()), "daemon status");
        assert!(status.contains("Mode: HybridCycles"));
        assert!(status.contains("Max steps per cycle: 6"));
        assert!(status.contains("Instructions: continue bounded cycles"));
    }

    #[test]
    fn daemon_health_reports_policy_scheduler_and_start_errors() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let enabled = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon enable strict 3 250",
        );
        assert!(enabled.contains("Enabled Lab daemon policy"));

        let health = handle_lab_command(temp.path(), Some("session".to_string()), "daemon health");

        assert!(health.contains("Lab daemon health: enabled_not_started"));
        assert!(health.contains("Policy: enabled=true mode=Strict"));
        assert!(health.contains("Scheduler: running_in_process=false persisted=none"));
        assert!(health.contains("Last start error: none"));
        assert!(health.contains("LaunchAgent exists: false"));

        let store = LabStore::for_project(temp.path());
        store
            .record_daemon_start_result(None, Some("provider unavailable"))
            .unwrap();
        let unhealthy =
            handle_lab_command(temp.path(), Some("session".to_string()), "daemon health");

        assert!(unhealthy.contains("Lab daemon health: unhealthy_start_error"));
        assert!(unhealthy.contains("Last start error: provider unavailable"));
    }

    #[test]
    fn daemon_service_status_reports_install_plan() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        let temp = tempfile::tempdir().unwrap();
        let launch_agents = tempfile::tempdir().unwrap();
        env.set(
            "PRIORITY_AGENT_LAUNCH_AGENTS_DIR",
            launch_agents.path().to_str().unwrap(),
        );

        let status = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon service status com.example.Lab Service",
        );

        assert!(status.contains("Lab daemon service status."));
        assert!(status.contains("Label: com.example.lab-service"));
        assert!(status.contains("Generated exists: false"));
        assert!(status.contains("Installed exists: false"));
        assert!(status.contains("Bootstrap command: launchctl bootstrap gui/$(id -u)"));
        assert!(status.contains(
            "Kickstart command: launchctl kickstart -k gui/$(id -u)/com.example.lab-service"
        ));
        assert!(status.contains("Health command: /lab daemon health"));
        assert!(status.contains(&launch_agents.path().display().to_string()));
    }

    #[test]
    fn daemon_service_install_and_uninstall_manage_launchagent_plist() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        let temp = tempfile::tempdir().unwrap();
        let launch_agents = tempfile::tempdir().unwrap();
        env.set(
            "PRIORITY_AGENT_LAUNCH_AGENTS_DIR",
            launch_agents.path().to_str().unwrap(),
        );

        let install = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon service install com.example.lab.demo",
        );

        let installed = launch_agents.path().join("com.example.lab.demo.plist");
        assert!(install.contains("Installed Lab daemon LaunchAgent plist."));
        assert!(install.contains("Generated exists: true"));
        assert!(install.contains("Installed exists: true"));
        assert!(installed.exists());
        let installed_plist = fs::read_to_string(&installed).unwrap();
        assert!(installed_plist.contains("<string>com.example.lab.demo</string>"));
        assert!(installed_plist.contains("<string>lab-daemon</string>"));

        let uninstall = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon service uninstall com.example.lab.demo",
        );

        assert!(uninstall.contains("Uninstalled Lab daemon LaunchAgent plist."));
        assert!(uninstall.contains("Removed: true"));
        assert!(uninstall.contains("Installed exists: false"));
        assert!(!installed.exists());
    }

    #[test]
    #[cfg(unix)]
    fn daemon_service_load_unload_and_restart_call_launchctl() {
        use std::os::unix::fs::PermissionsExt;

        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        let temp = tempfile::tempdir().unwrap();
        let launch_agents = tempfile::tempdir().unwrap();
        let bin_dir = tempfile::tempdir().unwrap();
        let fake_launchctl = bin_dir.path().join("launchctl");
        let launchctl_log = bin_dir.path().join("launchctl.log");
        fs::write(
            &fake_launchctl,
            r#"#!/bin/sh
printf '%s' "$1" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
shift
for arg in "$@"; do
  printf '|%s' "$arg" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
done
printf '\n' >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_launchctl).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_launchctl, permissions).unwrap();
        env.set(
            "PRIORITY_AGENT_LAUNCH_AGENTS_DIR",
            launch_agents.path().to_str().unwrap(),
        );
        env.set(
            "PRIORITY_AGENT_LAUNCHCTL_BIN",
            fake_launchctl.to_str().unwrap(),
        );
        env.set("PRIORITY_AGENT_LAUNCHCTL_DOMAIN", "gui/test");
        env.set(
            "PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG",
            launchctl_log.to_str().unwrap(),
        );

        let load = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon service load com.example.lab.demo",
        );
        let restart = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon service restart com.example.lab.demo",
        );
        let unload = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon service unload com.example.lab.demo",
        );

        assert!(load.contains("Loaded Lab daemon service."));
        assert!(restart.contains("Restarted Lab daemon service."));
        assert!(unload.contains("Unloaded Lab daemon service."));
        let installed = launch_agents.path().join("com.example.lab.demo.plist");
        assert!(installed.exists());
        let log = fs::read_to_string(launchctl_log).unwrap();
        assert!(log.contains(&format!("bootstrap|gui/test|{}", installed.display())));
        assert!(log.contains("kickstart|-k|gui/test/com.example.lab.demo"));
        assert!(log.contains("bootout|gui/test/com.example.lab.demo"));
    }

    #[test]
    #[cfg(unix)]
    fn daemon_service_supervise_skips_disabled_and_repairs_missing_service() {
        use std::os::unix::fs::PermissionsExt;

        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        let temp = tempfile::tempdir().unwrap();
        let launch_agents = tempfile::tempdir().unwrap();
        let bin_dir = tempfile::tempdir().unwrap();
        let fake_launchctl = bin_dir.path().join("launchctl");
        let launchctl_log = bin_dir.path().join("launchctl.log");
        fs::write(
            &fake_launchctl,
            r#"#!/bin/sh
printf '%s' "$1" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
shift
for arg in "$@"; do
  printf '|%s' "$arg" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
done
printf '\n' >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
if [ "$1" = "gui/test/com.example.lab.demo" ]; then
  printf 'missing service\n' >&2
  exit 113
fi
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_launchctl).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_launchctl, permissions).unwrap();
        env.set(
            "PRIORITY_AGENT_LAUNCH_AGENTS_DIR",
            launch_agents.path().to_str().unwrap(),
        );
        env.set(
            "PRIORITY_AGENT_LAUNCHCTL_BIN",
            fake_launchctl.to_str().unwrap(),
        );
        env.set("PRIORITY_AGENT_LAUNCHCTL_DOMAIN", "gui/test");
        env.set(
            "PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG",
            launchctl_log.to_str().unwrap(),
        );

        let skipped = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon service supervise com.example.lab.demo",
        );

        assert!(skipped.contains("supervision skipped: no daemon policy"));

        let enabled = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon enable strict 3 250",
        );
        assert!(enabled.contains("Enabled Lab daemon policy"));

        let repaired = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "daemon service supervise com.example.lab.demo",
        );

        let installed = launch_agents.path().join("com.example.lab.demo.plist");
        assert!(repaired.contains("supervision repaired missing service"));
        assert!(repaired.contains("Exit status: 113"));
        assert!(repaired.contains("Repair:"));
        assert!(installed.exists());
        let log = fs::read_to_string(launchctl_log).unwrap();
        assert!(log.contains("print|gui/test/com.example.lab.demo"));
        assert!(log.contains(&format!("bootstrap|gui/test|{}", installed.display())));
    }

    #[test]
    fn start_drafts_proposal_without_creating_run() {
        let temp = tempfile::tempdir().unwrap();
        let output = handle_lab_command(temp.path(), Some("session".to_string()), "start Build it");

        assert!(output.contains("Lab proposal drafted"));
        assert!(temp.path().join(".priority-agent/lab/proposals").exists());
        assert!(!temp.path().join(".priority-agent/lab/runs").exists());
    }

    #[tokio::test]
    async fn proposal_llm_command_structures_intake_without_creating_run() {
        let temp = tempfile::tempdir().unwrap();
        let provider = Arc::new(ProposalProvider {
            response: serde_json::json!({
                "problem_statement": "Need a formal LabRun intake.",
                "desired_outcome": "A proposal that can be approved explicitly.",
                "scope": ["proposal drafting", "approval boundary"],
                "non_goals": ["auto-approve"],
                "constraints": ["do not mutate code before approval"],
                "risks": ["unclear scope"],
                "success_criteria": ["structured proposal exists"],
                "recommended_mode": "labrun",
                "professor_rationale": "This should use LabRun because it spans planning and implementation."
            })
            .to_string(),
        });
        let context = ToolContext::new(temp.path(), "lab-command-test")
            .with_llm_provider(provider)
            .with_model("mock-proposal".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "propose llm Build a safer Lab Mode",
            context,
        )
        .await;

        assert!(output.contains("Professor drafted Lab proposal:"));
        assert!(output.contains("Recommended mode: Labrun"));
        assert!(output.contains("Formal approval is required"));
        let store = LabStore::for_project(temp.path());
        let proposal = store.latest_proposal().unwrap().unwrap();
        assert_eq!(proposal.problem_statement, "Need a formal LabRun intake.");
        assert_eq!(
            proposal.success_criteria,
            vec!["structured proposal exists".to_string()]
        );
        assert!(store.latest_run().unwrap().is_none());
    }

    #[tokio::test]
    async fn meeting_llm_command_writes_provider_meeting_summary() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let provider = Arc::new(ProposalProvider {
            response: serde_json::json!({
                "professor_view": "Keep the project scope narrow.",
                "postdoc_view": "One implementation blocker needs repair.",
                "decision": "revise_plan",
                "next_actions": ["revise the next postdoc slice"],
                "evidence_ids": []
            })
            .to_string(),
        });
        let context = ToolContext::new(temp.path(), "lab-command-test")
            .with_llm_provider(provider)
            .with_model("mock-meeting".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "meeting llm discuss blocker",
            context,
        )
        .await;

        assert!(output.contains("Provider Lab meeting summary created:"));
        assert!(output.contains("This meeting is read-only and does not mutate code."));
        assert!(output.contains("Usage recorded: true"));
        let store = LabStore::for_project(temp.path());
        let run = store.latest_run().unwrap().unwrap();
        assert_eq!(run.meeting_ids.len(), 1);
        let artifact = store
            .load_stage_artifact(&run.lab_run_id, run.artifact_ids.last().unwrap())
            .unwrap();
        match artifact {
            StageArtifact::LabMeetingSummary(envelope) => {
                assert_eq!(envelope.body.topic, "discuss blocker");
                assert_eq!(envelope.body.decision, "revise_plan");
                assert_eq!(
                    envelope.validation_status.as_deref(),
                    Some("read_only_provider_summary")
                );
            }
            other => panic!(
                "expected LabMeetingSummary, got {:?}",
                other.artifact_type()
            ),
        }
    }

    #[test]
    fn provider_command_without_context_points_to_provider_shell() {
        let temp = tempfile::tempdir().unwrap();

        let output = handle_lab_command(temp.path(), Some("session".to_string()), "provider");

        assert!(output.contains("requires the Lab Mode shell provider"));
        assert!(output.contains("--with-provider"));
    }

    #[test]
    fn provider_compare_without_context_points_to_provider_shell() {
        let temp = tempfile::tempdir().unwrap();

        let output =
            handle_lab_command(temp.path(), Some("session".to_string()), "provider compare");

        assert!(output.contains("provider compare requires the Lab Mode shell provider"));
        assert!(output.contains("--with-provider"));
    }

    #[test]
    fn provider_tool_diagnostics_without_context_points_to_provider_shell() {
        let temp = tempfile::tempdir().unwrap();

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "provider diagnose-tools",
        );

        assert!(output.contains("provider diagnose-tools requires the Lab Mode shell provider"));
        assert!(output.contains("--with-provider"));
    }

    #[test]
    fn meeting_llm_without_context_points_to_provider_shell() {
        let temp = tempfile::tempdir().unwrap();

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "meeting llm validate repair plan",
        );

        assert!(output
            .contains("meeting llm validate repair plan requires the Lab Mode shell provider"));
        assert!(output.contains("--with-provider"));
    }

    #[test]
    fn provider_run_commands_without_context_point_to_provider_shell() {
        let temp = tempfile::tempdir().unwrap();

        let step = handle_lab_command(temp.path(), Some("session".to_string()), "step llm focus");
        let run_llm =
            handle_lab_command(temp.path(), Some("session".to_string()), "run llm 2 focus");
        let run_hybrid = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "run hybrid 2 focus",
        );
        let run_hybrid_cycles = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "run hybrid-cycles 2 1 focus",
        );

        assert!(step.contains("step llm"));
        assert!(step.contains("--with-provider"));
        assert!(run_llm.contains("run <llm|hybrid|hybrid-cycles>"));
        assert!(run_llm.contains("--with-provider"));
        assert!(run_hybrid.contains("run <llm|hybrid|hybrid-cycles>"));
        assert!(run_hybrid.contains("--with-provider"));
        assert!(run_hybrid_cycles.contains("run <llm|hybrid|hybrid-cycles>"));
        assert!(run_hybrid_cycles.contains("--with-provider"));
    }

    #[tokio::test]
    async fn step_llm_command_runs_provider_stage_step() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let provider = Arc::new(SequenceCommandProvider {
            responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Build LabRun",
                        "strategic_direction": "Keep gates strict.",
                        "success_criteria": ["advance"],
                        "constraints": ["no overclaiming"],
                        "risks": ["weak evidence"],
                        "handoff_to_postdoc": "Plan the implementation."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready"}"#.to_string(),
            ])),
        });
        let context = ToolContext::new(temp.path(), "lab-step-llm-command")
            .with_llm_provider(provider)
            .with_model("mock-sequence".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "step llm advance professor plan",
            context,
        )
        .await;

        assert!(output.contains("Provider Lab step:"));
        assert!(output.contains("From: professor_discussion"));
        assert!(output.contains("To: postdoc_plan"));
        assert!(output.contains("Advanced: true"));
        let saved = LabStore::for_project(temp.path())
            .latest_run()
            .unwrap()
            .unwrap();
        assert_eq!(saved.current_stage, "postdoc_plan");
    }

    #[tokio::test]
    async fn run_llm_command_reaches_graduate_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        assert!(handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}")
        )
        .contains("LabRun created"));
        let provider = Arc::new(SequenceCommandProvider {
            responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Build LabRun",
                        "strategic_direction": "Keep runtime gates strict.",
                        "success_criteria": ["reach graduate boundary"],
                        "constraints": ["no hidden mutation"],
                        "risks": ["missing task scope"],
                        "handoff_to_postdoc": "Create a scoped plan."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready for postdoc"}"#.to_string(),
                serde_json::json!({
                    "postdoc_plan": {
                        "implementation_summary": "Prepare one implementation slice.",
                        "slices": ["runtime command route"],
                        "files_expected": ["src/lab/commands.rs"],
                        "validation_plan": ["cargo check -q"],
                        "graduate_handoff": "Implement the command route."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready for graduate"}"#.to_string(),
            ])),
        });
        let context = ToolContext::new(temp.path(), "lab-run-llm-command")
            .with_llm_provider(provider)
            .with_model("mock-sequence".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "run llm 5 command routing",
            context,
        )
        .await;

        assert!(output.contains("Provider Lab run: 2 step(s)"));
        assert!(output.contains("Stop reason: GraduateBoundary"));
        let saved = LabStore::for_project(temp.path())
            .latest_run()
            .unwrap()
            .unwrap();
        assert_eq!(saved.current_stage, "graduate_work");
    }

    #[tokio::test]
    async fn run_hybrid_command_enters_strict_graduate_scheduler_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        assert!(handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}")
        )
        .contains("LabRun created"));
        let provider = Arc::new(SequenceCommandProvider {
            responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Build LabRun",
                        "strategic_direction": "Keep runtime gates strict.",
                        "success_criteria": ["hit strict scheduler"],
                        "constraints": ["no provider-only graduate work"],
                        "risks": ["weak tool evidence"],
                        "handoff_to_postdoc": "Create a plan with no scoped graduate work."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready"}"#.to_string(),
                serde_json::json!({
                    "postdoc_plan": {
                        "implementation_summary": "Reach the strict scheduler boundary.",
                        "slices": ["boundary"],
                        "files_expected": [],
                        "validation_plan": ["cargo check -q"],
                        "graduate_handoff": "No scoped graduate task is available."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready"}"#.to_string(),
            ])),
        });
        let context = ToolContext::new(temp.path(), "lab-run-hybrid-command")
            .with_llm_provider(provider)
            .with_model("mock-sequence".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "run hybrid 5 command routing",
            context,
        )
        .await;

        assert!(output.contains("Hybrid Lab run:"));
        assert!(output.contains("Stop reason: SchedulerStopped(Blocked)"));
        assert!(output.contains("scheduler Blocked"));
        let saved = LabStore::for_project(temp.path())
            .latest_run()
            .unwrap()
            .unwrap();
        assert_eq!(saved.current_stage, "graduate_work");
    }

    #[tokio::test]
    async fn run_hybrid_cycles_command_stops_at_professor_gate_without_explicit_review() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Complete previous cycle",
                "Provide accepted graduate evidence.",
                vec!["src/lab/commands.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Previous cycle implementation complete.",
                vec!["src/lab/commands.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let mut saved = store.load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        store.save_run(&saved).unwrap();
        orchestrator
            .create_postdoc_integration_summary_for_latest(None)
            .unwrap();
        let mut saved = store.load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "professor_review".to_string();
        saved.internal_owner = LabRole::Professor;
        store.save_run(&saved).unwrap();

        let provider = Arc::new(SequenceCommandProvider {
            responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Continue LabRun",
                        "strategic_direction": "Start the next bounded cycle.",
                        "success_criteria": ["next cycle starts"],
                        "constraints": ["bounded only"],
                        "risks": ["unbounded autonomy"],
                        "handoff_to_postdoc": "Prepare the next implementation plan."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"next cycle ready"}"#.to_string(),
            ])),
        });
        let context = ToolContext::new(temp.path(), "lab-run-hybrid-cycles-command")
            .with_llm_provider(provider)
            .with_model("mock-sequence".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "run hybrid-cycles 2 1 continue bounded cycle",
            context,
        )
        .await;

        assert!(output.contains("Hybrid Lab cycle run: 1 cycle(s)"));
        assert!(output.contains("Final stage: professor_review"));
        assert!(output.contains("Stop reason: Stopped(DeterministicGateBlocked)"));
        assert!(output.contains("continued_to_next_cycle=false"));
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.cycle_count, 0);
        assert_eq!(saved.current_stage, "professor_review");
    }

    #[tokio::test]
    async fn run_hybrid_cycles_command_stops_when_cycle_token_budget_is_exceeded() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.cost_policy.max_cycle_tokens = 10;
        store.save_run(&run).unwrap();
        store
            .record_cost_usage(
                &run.lab_run_id,
                LabRole::Professor,
                "mock-sequence",
                LabCostTokens {
                    prompt_tokens: 12,
                    completion_tokens: 2,
                    reasoning_tokens: 0,
                    cached_tokens: 0,
                    cache_write_tokens: 0,
                    cycle_id: Some("0".to_string()),
                    meeting_id: None,
                },
                0.0,
                Some("budget test"),
            )
            .unwrap();
        let provider = Arc::new(SequenceCommandProvider {
            responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
                r#"{"decision":"accept","note":"should not be called"}"#.to_string(),
            ])),
        });
        let context = ToolContext::new(temp.path(), "lab-run-hybrid-cycle-budget")
            .with_llm_provider(provider.clone())
            .with_model("mock-sequence".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "run hybrid-cycles 1 5 budget check",
            context,
        )
        .await;

        assert!(output.contains("Hybrid Lab cycle run: 0 cycle(s)"));
        assert!(output.contains("CostBudgetExceeded"));
        assert!(output.contains("total_tokens: 14"));
        assert_eq!(provider.responses.lock().len(), 1);
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.current_stage, "professor_discussion");
    }

    #[tokio::test]
    async fn run_hybrid_cycles_command_does_not_compress_blocked_professor_gate() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Complete previous cycle",
                "Provide accepted graduate evidence.",
                vec!["src/lab/commands.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Previous cycle implementation complete.",
                vec!["src/lab/commands.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let mut saved = store.load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = LabRole::Postdoc;
        saved.cost_policy.professor_context_budget = 10;
        saved.cost_policy.postdoc_context_budget = 10;
        saved.cost_policy.auto_compress_after_cycle = true;
        store.save_run(&saved).unwrap();
        orchestrator
            .create_postdoc_integration_summary_for_latest(None)
            .unwrap();
        let mut saved = store.load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "professor_review".to_string();
        saved.internal_owner = LabRole::Professor;
        saved.cost_policy.professor_context_budget = 10;
        saved.cost_policy.postdoc_context_budget = 10;
        saved.cost_policy.auto_compress_after_cycle = true;
        store.save_run(&saved).unwrap();
        let provider = Arc::new(SequenceCommandProvider {
            responses: parking_lot::Mutex::new(std::collections::VecDeque::new()),
        });
        let context = ToolContext::new(temp.path(), "lab-run-hybrid-cycle-compress")
            .with_llm_provider(provider)
            .with_model("mock-sequence".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "run hybrid-cycles 1 1 compression check",
            context,
        )
        .await;

        assert!(output.contains("Hybrid Lab cycle run: 1 cycle(s)"));
        assert!(output.contains("Stop reason: Stopped(DeterministicGateBlocked)"));
        assert!(output.contains("compression_artifacts=none"));
        let saved = store.latest_run().unwrap().unwrap();
        assert!(!store
            .list_stage_artifacts(&saved.lab_run_id)
            .unwrap()
            .iter()
            .any(|artifact| matches!(artifact, StageArtifact::CompressionSummary(_))));
    }

    #[tokio::test]
    async fn provider_command_reports_provider_neutral_graduate_diagnostics() {
        let temp = tempfile::tempdir().unwrap();
        let mut context =
            ToolContext::new(temp.path(), "lab-provider-test").with_model("deepseek-v4-flash");
        context
            .metadata
            .insert("provider_id".to_string(), "deepseek".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "provider",
            context,
        )
        .await;

        assert!(output.contains("Lab provider diagnostics:"));
        assert!(output.contains("Provider: deepseek"));
        assert!(output.contains("Model: deepseek-v4-flash"));
        assert!(output.contains("Graduate diagnostic status: unverified"));
        assert!(output.contains("Graduate dispatch policy: provider_neutral_task_evidence"));
        assert!(output.contains("scripts/lab-live-validation.sh --live-control-plane"));
        assert!(output.contains("scripts/lab-live-validation.sh --live-graduate"));
        assert!(output.contains("Latest graduate record: none"));
    }

    #[tokio::test]
    async fn provider_record_command_certifies_graduate_provider_locally() {
        let temp = tempfile::tempdir().unwrap();
        let mut context =
            ToolContext::new(temp.path(), "lab-provider-test").with_model("deepseek-v4-flash");
        context
            .metadata
            .insert("provider_id".to_string(), "deepseek".to_string());

        let recorded = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "provider record graduate passed target/lab-live-validation/pass/report.md full live graduate validation passed",
            context.clone(),
        )
        .await;

        assert!(recorded.contains("Recorded provider diagnostic:"));
        assert!(recorded.contains("Kind: graduate"));
        assert!(recorded.contains("Outcome: passed"));

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "provider",
            context,
        )
        .await;

        assert!(output.contains("Graduate diagnostic status: certified"));
        assert!(output.contains("Graduate dispatch policy: provider_neutral_task_evidence"));
        assert!(output.contains("Latest graduate record: graduate passed"));
        assert!(output.contains("target/lab-live-validation/pass/report.md"));
        let store = LabStore::for_project(temp.path());
        let latest = store
            .latest_provider_certification(
                "deepseek",
                "deepseek-v4-flash",
                LabProviderCertificationKind::Graduate,
            )
            .unwrap()
            .unwrap();
        assert_eq!(latest.outcome, LabProviderCertificationOutcome::Passed);
    }

    #[tokio::test]
    async fn provider_compare_recovers_generic_foreground_from_durable_sink() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = "lab-provider-command";
        let task_id = "provider-compare-generic";
        let agent_id = "agent_generic";
        let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        store
            .create_session(session_id, "Lab provider command", "mock", Some("/repo"))
            .unwrap();
        let worktree = temp.path().join("generic-worktree");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(
            worktree.join("lab-provider-compare-generic.txt"),
            "generic subagent tool smoke\n",
        )
        .unwrap();
        let artifact_id = store
            .add_agent_artifact(
                session_id,
                agent_id,
                Some("implementer"),
                "Specialist",
                "completed",
                "Provider comparison generic implementer smoke",
                "completed generic compare",
                &serde_json::json!({
                    "completion_sink": "agent_manager",
                    "tools_used": ["file_write", "file_read", "bash"],
                    "confidence": 1.0,
                    "has_conflict": false
                }),
            )
            .unwrap();
        store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: session_id.to_string(),
                task_id: task_id.to_string(),
                agent_id: agent_id.to_string(),
                profile: Some("implementer".to_string()),
                role: "Specialist".to_string(),
                status: "completed".to_string(),
                description: "Provider comparison generic implementer smoke".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: Some(artifact_id),
                cleanup_hooks: Vec::new(),
                payload: serde_json::json!({
                    "allowed_tools": ["file_read", "file_write", "file_edit", "bash", "diff"],
                    "context_mode": "isolated_worktree_fork",
                    "isolated_worktree": {
                        "path": worktree.to_string_lossy(),
                        "branch": "codex/agent-generic"
                    },
                    "tools_used": ["file_write", "file_read", "bash"],
                    "completion_sink": "agent_manager"
                }),
            })
            .unwrap();

        let recovered = recover_provider_compare_durable_subagent(
            "Generic subagent",
            &store,
            session_id,
            task_id,
            "lab-provider-compare-generic.txt",
            "Timeout waiting for agent agent_generic result after 90s",
        )
        .await
        .expect("durable compare state should recover");

        assert!(recovered.success);
        assert!(recovered.used_mutating_tool);
        assert!(recovered
            .summary
            .contains("recovered_from_durable_sink: true"));
        assert!(recovered.summary.contains("hard_file_proof: true"));
        assert!(recovered.summary.contains("completion_sink: agent_manager"));
    }

    #[tokio::test]
    async fn provider_failed_record_is_visible_but_does_not_certify() {
        let temp = tempfile::tempdir().unwrap();
        let mut context =
            ToolContext::new(temp.path(), "lab-provider-test").with_model("deepseek-v4-flash");
        context
            .metadata
            .insert("provider_id".to_string(), "deepseek".to_string());

        let recorded = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "provider record graduate failed target/lab-live-validation/fail/report.md full live graduate validation failed",
            context.clone(),
        )
        .await;

        assert!(recorded.contains("Recorded provider diagnostic:"));
        assert!(recorded.contains("Kind: graduate"));
        assert!(recorded.contains("Outcome: failed"));

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "provider",
            context,
        )
        .await;

        assert!(output.contains("Graduate diagnostic status: unverified"));
        assert!(output.contains("Graduate dispatch policy: provider_neutral_task_evidence"));
        assert!(output.contains("Latest graduate record: graduate failed"));
        assert!(output.contains("target/lab-live-validation/fail/report.md"));
    }

    #[tokio::test]
    async fn provider_compare_reports_generic_and_lab_paths() {
        let temp = tempfile::tempdir().unwrap();
        let proposal = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "propose Compare provider paths",
        );
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let mut context = ToolContext::new(temp.path(), "lab-provider-compare-test")
            .with_model("deepseek-v4-flash");
        context
            .metadata
            .insert("provider_id".to_string(), "deepseek".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "provider compare",
            context,
        )
        .await;

        assert!(output.contains("Provider subagent comparison:"));
        assert!(output.contains("Provider: deepseek"));
        assert!(output.contains("Generic subagent:"));
        assert!(output.contains("AgentManager not available"));
        assert!(output.contains("Lab graduate:"));
        assert!(output.contains("status: Failed"));
        assert!(output.contains("Conclusion:"));
    }

    #[test]
    fn provider_compare_does_not_treat_denied_tool_attempt_as_mutation_proof() {
        assert!(!hard_subagent_mutation_proof(
            true,
            false,
            "file_write returns Permission denied: 'file_write' requires user confirmation"
        ));
        assert!(!hard_subagent_mutation_proof(
            true,
            true,
            "Action rejected before execution: checkpoint_required"
        ));
        assert!(hard_subagent_mutation_proof(
            true,
            true,
            "Created lab-provider-compare-background.txt"
        ));
    }

    #[test]
    fn lab_graduate_provider_compare_reports_durable_subagent_proof() {
        let temp = tempfile::tempdir().unwrap();
        let worktree = temp.path().join("lab-graduate-worktree");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(
            worktree.join("lab-provider-compare-lab.txt"),
            "lab graduate tool smoke\n",
        )
        .unwrap();
        let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        session_store
            .create_session("lab-test", "lab durable proof", "test-model", None)
            .unwrap();
        let artifact_id = session_store
            .add_agent_artifact(
                "lab-test",
                "agent_lab",
                Some("lab-graduate"),
                "implementation",
                "completed",
                "lab graduate durable proof",
                "Created lab-provider-compare-lab.txt",
                &serde_json::json!({"completion_sink": "agent_manager"}),
            )
            .unwrap();
        session_store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: "lab-test".to_string(),
                task_id: "lab-graduate-gradtask_compare".to_string(),
                agent_id: "agent_lab".to_string(),
                profile: Some("lab-graduate".to_string()),
                role: "implementation".to_string(),
                status: "completed".to_string(),
                description: "lab graduate durable proof".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: Some(artifact_id),
                cleanup_hooks: Vec::new(),
                payload: serde_json::json!({
                    "completion_sink": "agent_manager",
                    "context_mode": "isolated_worktree_fork",
                    "tools_used": ["file_write", "bash"],
                    "isolated_worktree": {
                        "path": worktree.to_string_lossy().to_string(),
                        "branch": "codex/lab-graduate-proof"
                    }
                }),
            })
            .unwrap();
        let context = ToolContext::new(temp.path(), "lab-test").with_session_store(session_store);

        let (lines, hard_proof) = lab_graduate_durable_smoke_details(
            &context,
            "lab-graduate-gradtask_compare",
            "lab-provider-compare-lab.txt",
        );
        let rendered = lines.join("\n");

        assert!(hard_proof);
        assert!(rendered.contains("durable_state: present"));
        assert!(rendered.contains("durable_profile: lab-graduate"));
        assert!(rendered.contains("durable_context_mode: isolated_worktree_fork"));
        assert!(rendered.contains("tools_used: file_write,bash"));
        assert!(rendered.contains("hard_file_proof: true"));
        assert!(rendered.contains("permission_denied: false"));
    }

    #[tokio::test]
    async fn provider_tool_diagnostics_reports_request_and_response_tool_calls() {
        let temp = tempfile::tempdir().unwrap();
        let mut context = ToolContext::new(temp.path(), "lab-provider-tool-diagnostics-test")
            .with_llm_provider(Arc::new(ToolProbeProvider))
            .with_model("mock-tool-probe".to_string());
        context
            .metadata
            .insert("provider_id".to_string(), "mock".to_string());

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "provider diagnose-tools",
            context,
        )
        .await;

        assert!(output.contains("Provider tool-call diagnostics:"));
        assert!(output.contains("Provider: mock"));
        assert!(output.contains("Probe: minimal_auto"));
        assert!(output.contains("Probe: minimal_required"));
        assert!(output.contains("Probe: minimal_forced"));
        assert!(output.contains("Probe: runtime_file_write_auto"));
        assert!(output.contains("Probe: runtime_file_write_bash_auto"));
        assert!(output.contains("Probe: runtime_subagent_allowed_auto"));
        assert!(output.contains("request_tools_count: 1"));
        assert!(output.contains("request_tools: lab_provider_echo"));
        assert!(output.contains("request_tools: file_write"));
        assert!(output.contains("request_tools: file_write,bash"));
        assert!(output.contains("request_tools: file_write,file_edit,bash,diff"));
        assert!(output.contains("response_tool_calls_count: 1"));
        assert!(output.contains("response_tool_calls: lab_provider_echo"));
        assert!(output.contains("response_tool_calls: file_write"));
        assert!(output.contains("finish_reason: tool_calls"));
    }

    #[test]
    fn advance_requires_gate_satisfaction() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let blocked = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
        assert!(blocked.contains("Failed to advance LabRun"));
        assert!(blocked.contains("artifact_id"));

        let gate = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "gate satisfy artifact_professor_plan_001 not_verified",
        );
        assert!(gate.contains("Failed to satisfy artifact gate"));
        assert!(gate.contains("missing or malformed artifact"));

        let planned = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "plan Professor direction",
        );
        assert!(planned.contains("Created ProfessorPlan artifact"));
        assert!(planned.contains("Gate satisfied"));

        let advanced = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
        assert!(advanced.contains("postdoc_plan"));
    }

    #[test]
    fn plan_command_creates_artifact_and_allows_advance() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let planned = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "plan Professor direction",
        );
        assert!(planned.contains("Created ProfessorPlan artifact"));
        assert!(planned.contains("Gate satisfied"));
        assert!(planned.contains("Report: "));

        let advanced = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
        assert!(advanced.contains("postdoc_plan"));
        assert!(temp.path().join(".priority-agent/lab/runs").exists());
    }

    #[test]
    fn report_command_shows_latest_generated_report() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let planned = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "plan Professor direction",
        );
        assert!(planned.contains("Report: "));

        let report = handle_lab_command(temp.path(), Some("session".to_string()), "report");
        let list = handle_lab_command(temp.path(), Some("session".to_string()), "report list");

        assert!(report.contains("Lab report:"));
        assert!(report.contains("Artifact: artifact_professorplan_"));
        assert!(report.contains("Path:"));
        assert!(report.contains("Preview:"));
        assert!(list.contains("Lab reports:"));
        assert!(list.contains("artifact_professorplan_"));
    }

    #[test]
    fn review_command_summarizes_current_review_state() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let planned = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "plan Professor direction",
        );
        assert!(planned.contains("Created ProfessorPlan artifact"));

        let review = handle_lab_command(temp.path(), Some("session".to_string()), "review");

        assert!(review.contains("Lab review:"));
        assert!(review.contains("Run: status=Active"));
        assert!(review.contains("Artifacts: 1 latest=artifact_professorplan_"));
        assert!(review.contains("Reports: 1 latest="));
        assert!(review.contains("Current gate: stage=professor_discussion"));
        assert!(review.contains("satisfied=true"));
        assert!(review.contains("Graduate worktree proof: none"));
        assert!(review.contains("Graduate workspace snapshots: none"));
        assert!(review
            .contains("Provider artifact review: /lab review artifact artifact_professorplan_"));
        assert!(!review.contains("planned for a later orchestration slice"));
    }

    #[test]
    fn artifact_revise_blocks_advance_until_acceptance() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let planned = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "plan Professor direction",
        );
        let artifact_id = planned
            .lines()
            .find_map(|line| line.strip_prefix("Created ProfessorPlan artifact: "))
            .unwrap()
            .to_string();

        let revised = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("revise {artifact_id} needs clearer constraints"),
        );
        assert!(revised.contains("Revision requested"));
        assert!(revised.contains("needs_revision"));
        let blocked = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
        assert!(blocked.contains("Failed to advance LabRun"));
        assert!(blocked.contains("blocked") || blocked.contains("needs revision"));

        let accepted = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("accept {artifact_id} revised offline"),
        );
        assert!(accepted.contains("Accepted artifact"));
        assert!(accepted.contains("accepted"));
        let advanced = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
        assert!(advanced.contains("postdoc_plan"));
    }

    #[test]
    fn cost_command_records_and_summarizes_lab_usage() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let recorded = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "cost record professor test-model 1000 200 50 700 120 0.0123 draft",
        );
        assert!(recorded.contains("Recorded Lab cost usage"));
        assert!(recorded.contains("cached=700"));
        assert!(recorded.contains("cache_write=120"));
        assert!(recorded.contains("miss=300"));

        let summary = handle_lab_command(temp.path(), Some("session".to_string()), "cost");
        assert!(summary.contains("Requests: 1"));
        assert!(summary.contains("total=1250"));
        assert!(summary.contains("hit_rate=70.0%"));
        assert!(summary.contains("Professor"));
    }

    #[test]
    fn closeout_command_marks_latest_run_verified_completed() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "closeout verified validation passed",
        );

        assert!(output.contains("LabRun closeout recorded"));
        assert!(output.contains("Status: Completed"));
        assert!(output.contains("CompletedVerified"));
        let store = LabStore::for_project(temp.path());
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.status, crate::lab::model::LabRunStatus::Completed);
        assert_eq!(
            saved.closeout_status,
            Some(LabCloseoutStatus::CompletedVerified)
        );
        assert!(!store.root().join("active_lease.json").exists());
    }

    #[test]
    fn auto_closeout_command_uses_final_professor_gate() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        drive_lab_command_to_user_report(temp.path());

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "closeout auto final report shown",
        );

        assert!(output.contains("LabRun closeout recorded from final evidence"));
        assert!(output.contains("Status: Completed"));
        assert!(output.contains("CompletedNotVerified"));
        let store = LabStore::for_project(temp.path());
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.status, crate::lab::model::LabRunStatus::Completed);
        assert_eq!(
            saved.closeout_status,
            Some(LabCloseoutStatus::CompletedNotVerified)
        );
    }

    #[test]
    fn continue_command_starts_next_cycle_from_user_report() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        drive_lab_command_to_user_report(temp.path());

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "continue next cycle approved",
        );

        assert!(output.contains("Continued LabRun"));
        assert!(output.contains("cycle 1"));
        assert!(output.contains("professor_discussion"));
        let store = LabStore::for_project(temp.path());
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.status, crate::lab::model::LabRunStatus::Active);
        assert_eq!(saved.current_stage, "professor_discussion");
        assert_eq!(saved.cycle_count, 1);
    }

    #[test]
    fn intervene_command_pauses_run_without_creating_graduate_task() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "intervene Reconsider whether this is still in scope",
        );

        assert!(output.contains("LabRun intervention queued"));
        assert!(output.contains("Run status: NeedsUser"));
        let store = LabStore::for_project(temp.path());
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.status, crate::lab::model::LabRunStatus::NeedsUser);
        assert_eq!(store.latest_graduate_tasks().unwrap().len(), 0);
        assert!(!store.root().join("active_lease.json").exists());
    }

    #[test]
    fn recovery_command_shows_paused_run_options_without_resuming() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let paused = handle_lab_command(temp.path(), Some("session".to_string()), "pause user");
        assert!(paused.contains("Paused LabRun"));

        let recovery = handle_lab_command(temp.path(), Some("session".to_string()), "recovery");

        assert!(recovery.contains("Lab recovery:"));
        assert!(recovery.contains("Recovery: available"));
        assert!(recovery.contains("Resume cursor:"));
        assert!(recovery.contains("Continue: /lab resume"));
        assert!(recovery.contains("Inspect: /lab dashboard"));
        assert!(recovery.contains("Keep paused: no action"));
        assert!(recovery.contains("Close/cancel: /lab close"));
        let store = LabStore::for_project(temp.path());
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.status, crate::lab::model::LabRunStatus::Paused);
        assert!(!store.root().join("active_lease.json").exists());
    }

    #[test]
    fn open_command_switches_active_labrun_without_resuming() {
        let temp = tempfile::tempdir().unwrap();
        let first = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "propose First run",
        );
        let first_proposal_id = first
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let first_approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {first_proposal_id}"),
        );
        assert!(first_approved.contains("LabRun created"));
        let store = LabStore::for_project(temp.path());
        let first_run_id = store.latest_run().unwrap().unwrap().lab_run_id;
        let paused_first =
            handle_lab_command(temp.path(), Some("session".to_string()), "pause user");
        assert!(paused_first.contains("Paused LabRun"));

        let second = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "propose Second run",
        );
        let second_proposal_id = second
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let second_approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {second_proposal_id}"),
        );
        assert!(second_approved.contains("LabRun created"));
        let paused_second =
            handle_lab_command(temp.path(), Some("session".to_string()), "pause user");
        assert!(paused_second.contains("Paused LabRun"));

        let opened = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("open {first_run_id}"),
        );

        assert!(opened.contains("Opened LabRun"));
        assert!(opened.contains("for inspection"));
        let latest = store.latest_run().unwrap().unwrap();
        assert_eq!(latest.lab_run_id, first_run_id);
        assert_eq!(latest.status, crate::lab::model::LabRunStatus::Paused);
        assert!(!store.root().join("active_lease.json").exists());
    }

    #[test]
    fn runs_command_lists_recent_lab_runs() {
        let temp = tempfile::tempdir().unwrap();
        for goal in ["First run", "Second run"] {
            let proposal = handle_lab_command(
                temp.path(),
                Some("session".to_string()),
                &format!("propose {goal}"),
            );
            let proposal_id = proposal
                .lines()
                .find_map(|line| line.strip_prefix("Lab proposal created: "))
                .unwrap()
                .to_string();
            let approved = handle_lab_command(
                temp.path(),
                Some("session".to_string()),
                &format!("approve {proposal_id}"),
            );
            assert!(approved.contains("LabRun created"));
            let paused = handle_lab_command(temp.path(), Some("session".to_string()), "pause user");
            assert!(paused.contains("Paused LabRun"));
        }

        let runs = handle_lab_command(temp.path(), Some("session".to_string()), "runs");

        assert!(runs.contains("Lab runs:"));
        assert!(runs.contains("Total: 2"));
        assert!(runs.contains("Index:"));
        assert!(runs.contains("runs_index.json"));
        assert!(runs.contains("Open one with /lab open <lab_run_id>"));
        assert!(runs.matches("status=Paused").count() >= 2);
        assert!(runs.contains("tasks=0 artifacts=0"));
        assert!(runs.contains("* labrun_"));
    }

    #[test]
    fn status_reports_file_and_sqlite_index_summaries() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let runs = handle_lab_command(temp.path(), Some("session".to_string()), "runs");
        assert!(runs.contains("Lab runs:"));

        let status = handle_lab_command(temp.path(), Some("session".to_string()), "status");

        assert!(status.contains("Latest LabRun:"));
        assert!(status.contains("Index:"));
        assert!(status.contains("runs_index.json"));
        assert!(status.contains("latest=matched"));
        assert!(status.contains("SQLite index:"));
        assert!(status.contains("lab_index.sqlite3"));
        assert!(status.contains("runs=1"));
    }

    #[test]
    fn messages_command_lists_professor_side_channel_inbox() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let queued = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "professor This needs a tighter product boundary",
        );
        assert!(queued.contains("Message queued for professor"));
        let inbox = handle_lab_command(temp.path(), Some("session".to_string()), "messages");

        assert!(inbox.contains("Professor side-channel inbox"));
        assert!(inbox.contains("Messages: 1"));
        assert!(inbox.contains("Concern/Queued/normal"));
        assert!(inbox.contains("tighter product boundary"));
    }

    #[test]
    fn messages_command_updates_professor_side_channel_status() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let queued = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "professor Convert this into a group meeting",
        );
        let message_id = queued
            .lines()
            .find_map(|line| line.strip_prefix("Message queued for professor: "))
            .unwrap()
            .to_string();

        let updated = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("messages meeting {message_id} schedule it"),
        );
        let inbox = handle_lab_command(temp.path(), Some("session".to_string()), "messages");

        assert!(updated.contains("Professor side-channel message updated"));
        assert!(updated.contains("ConvertedToMeeting"));
        assert!(inbox.contains("Concern/ConvertedToMeeting/normal"));
    }

    #[test]
    fn messages_decision_renders_professor_steering_state_without_applying() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let queued = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "professor Schedule a product review meeting",
        );
        let message_id = queued
            .lines()
            .find_map(|line| line.strip_prefix("Message queued for professor: "))
            .unwrap()
            .to_string();
        let converted = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("messages meeting {message_id}"),
        );
        assert!(converted.contains("ConvertedToMeeting"));

        let decision = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "messages decision latest",
        );

        assert!(decision.contains("Professor steering decision:"));
        assert!(decision.contains("Decision: open_lab_meeting"));
        assert!(decision.contains("Status: ConvertedToMeeting"));
        assert!(decision.contains("Next action: Apply with /lab messages apply"));
        assert!(decision.contains("Report: "));
        assert!(decision.contains("product review meeting"));
        let store = LabStore::for_project(temp.path());
        let run = store.latest_run().unwrap().unwrap();
        let messages = store.list_sponsor_messages(&run.lab_run_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].status, SponsorMessageStatus::ConvertedToMeeting);
        let artifacts = store.list_stage_artifacts(&run.lab_run_id).unwrap();
        let steering = artifacts
            .iter()
            .find_map(|artifact| match artifact {
                StageArtifact::ProfessorSteeringDecision(decision) => Some(decision),
                _ => None,
            })
            .expect("professor steering decision artifact");
        assert_eq!(steering.body.source_message_id, message_id);
        assert_eq!(steering.body.decision, "open_lab_meeting");
        assert_eq!(
            steering.validation_status.as_deref(),
            Some("decision_recorded_not_applied")
        );
        let reports = store
            .list_stage_artifact_report_paths(&run.lab_run_id)
            .unwrap();
        assert!(reports
            .iter()
            .any(|(artifact_id, path)| { artifact_id == &steering.artifact_id && path.exists() }));
    }

    #[test]
    fn messages_apply_meeting_creates_report_and_marks_message_applied() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let queued = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "professor Schedule a lab meeting about scope",
        );
        let message_id = queued
            .lines()
            .find_map(|line| line.strip_prefix("Message queued for professor: "))
            .unwrap()
            .to_string();
        let converted = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("messages meeting {message_id}"),
        );
        assert!(converted.contains("ConvertedToMeeting"));

        let applied = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("messages apply {message_id} scope meeting"),
        );
        let inbox = handle_lab_command(temp.path(), Some("session".to_string()), "messages");

        assert!(applied.contains("applied as meeting"));
        assert!(applied.contains("Report:"));
        assert!(inbox.contains("Concern/Applied/normal"));
    }

    #[test]
    fn messages_apply_task_creates_blocked_graduate_task() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let queued = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "professor Turn this concern into a scoped implementation task",
        );
        let message_id = queued
            .lines()
            .find_map(|line| line.strip_prefix("Message queued for professor: "))
            .unwrap()
            .to_string();
        let converted = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("messages task {message_id}"),
        );
        assert!(converted.contains("ConvertedToTask"));

        let applied = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("messages apply {message_id} implementation task"),
        );

        assert!(applied.contains("applied as blocked graduate task"));
        let store = LabStore::for_project(temp.path());
        let tasks = store.latest_graduate_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, crate::lab::model::LabTaskStatus::Blocked);
        assert!(tasks[0]
            .blocker
            .as_deref()
            .unwrap_or_default()
            .contains("allowed_scope"));
        let inbox = handle_lab_command(temp.path(), Some("session".to_string()), "messages");
        assert!(inbox.contains("Concern/Applied/normal"));
    }

    #[test]
    fn context_command_renders_packet_fingerprints() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let context =
            handle_lab_command(temp.path(), Some("session".to_string()), "context postdoc");

        assert!(context.contains("Lab context packet"));
        assert!(context.contains("Role: Postdoc"));
        assert!(context.contains("Stable prefix: hash="));
        assert!(context.contains("Dynamic tail: hash="));
        assert!(context.contains("L0 role-profile-and-project-charter"));
        assert!(context.contains("L3 cost-and-cache-summary"));
        assert!(context.contains("L5 validation-retry-history"));
        assert!(context.contains("L6 artifact-and-gate-evidence-refs"));
    }

    #[test]
    fn dashboard_command_renders_status_panel_summary() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let dashboard = handle_lab_command(temp.path(), Some("session".to_string()), "dashboard");

        assert!(dashboard.contains("Lab dashboard:"));
        assert!(dashboard.contains("Run: status=Active"));
        assert!(dashboard.contains("Tasks: total=0 open=0 blocked=0"));
        assert!(dashboard.contains("Validation retries: total=0 escalated=0"));
        assert!(dashboard.contains("Cost: requests=0"));
        assert!(dashboard.contains("Runtime escalation signals: suggested_meeting=false"));
        assert!(dashboard.contains("Scheduler:"));
        assert!(dashboard.contains("Indexed dashboard: missing"));
        assert!(dashboard.contains("Graduate worktree proof: none"));
        assert!(dashboard.contains("Graduate workspace snapshots: none"));
    }

    #[test]
    fn review_and_dashboard_render_graduate_workspace_snapshots() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let store = LabStore::for_project(temp.path());
        let run = store.latest_run().unwrap().unwrap();
        store
            .record_run_event(
                &run.lab_run_id,
                "lab_graduate_workspace_snapshot",
                serde_json::json!({
                    "task_id": "gradtask_snapshot",
                    "dispatch_id": "graddispatch_snapshot",
                    "phase": "before",
                    "dirty_path_count": 2,
                    "dirty_paths": ["preexisting-user-change.txt", "src/lib.rs"],
                    "changed_path_count": 0,
                    "changed_paths": [],
                }),
            )
            .unwrap();
        store
            .record_run_event(
                &run.lab_run_id,
                "lab_graduate_workspace_snapshot",
                serde_json::json!({
                    "task_id": "gradtask_snapshot",
                    "dispatch_id": "graddispatch_snapshot",
                    "phase": "after",
                    "dirty_path_count": 3,
                    "dirty_paths": ["preexisting-user-change.txt", "src/lib.rs", "src/lab/model.rs"],
                    "changed_path_count": 1,
                    "changed_paths": ["src/lab/model.rs"],
                }),
            )
            .unwrap();

        let review = handle_lab_command(temp.path(), Some("session".to_string()), "review");
        assert!(review.contains("Graduate workspace snapshots:"));
        assert!(review.contains("before task=gradtask_snapshot"));
        assert!(review.contains("dirty=2 [preexisting-user-change.txt,src/lib.rs]"));
        assert!(review.contains("after task=gradtask_snapshot"));
        assert!(review.contains("changed=1 [src/lab/model.rs]"));

        let dashboard = handle_lab_command(temp.path(), Some("session".to_string()), "dashboard");
        assert!(dashboard.contains("Graduate workspace snapshots:"));
        assert!(dashboard.contains("after task=gradtask_snapshot"));
        assert!(dashboard.contains("changed=1 [src/lab/model.rs]"));
    }

    #[test]
    fn dashboard_consumes_sqlite_index_for_professor_postdoc_state() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let planned = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "plan Professor plan",
        );
        assert!(planned.contains("Created ProfessorPlan artifact"));
        let advanced = handle_lab_command(temp.path(), Some("session".to_string()), "advance");
        assert!(advanced.contains("postdoc_plan"));
        let postdoc = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "plan Postdoc plan",
        );
        assert!(postdoc.contains("Created PostdocPlan artifact"));
        let runs = handle_lab_command(temp.path(), Some("session".to_string()), "runs");
        assert!(runs.contains("Lab runs:"));

        let dashboard = handle_lab_command(temp.path(), Some("session".to_string()), "dashboard");

        assert!(dashboard.contains("Indexed dashboard: sqlite="));
        assert!(dashboard.contains("lab_index.sqlite3"));
        assert!(dashboard.contains("ProfessorPlan:"));
        assert!(dashboard.contains("PostdocPlan:"));
        assert!(dashboard.contains("artifacts=2"));
    }

    #[test]
    fn evidence_command_records_refs_only_index() {
        let temp = tempfile::tempdir().unwrap();
        let evidence_path = temp.path().join("proof.log");
        std::fs::write(&evidence_path, "proof").unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let recorded = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!(
                "evidence add file {} cargo check passed",
                evidence_path.display()
            ),
        );
        assert!(recorded.contains("Recorded Lab evidence ref"));
        assert!(recorded.contains("kind=File"));

        let listed = handle_lab_command(temp.path(), Some("session".to_string()), "evidence list");
        assert!(listed.contains("Lab evidence refs: 1"));
        assert!(listed.contains("cargo check passed"));

        let context = handle_lab_command(temp.path(), Some("session".to_string()), "context");
        assert!(context.contains("L4 refs-only-evidence-index"));
    }

    #[test]
    fn task_command_manages_graduate_task_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let created = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "task create Implement task queue | src/lab/model.rs,src/lab/store.rs | cargo check -q | Add graduate task persistence and tests",
        );
        assert!(created.contains("Created graduate task"));
        assert!(created.contains("Status: Queued"));
        let task_id = created
            .lines()
            .find_map(|line| line.strip_prefix("Created graduate task: "))
            .unwrap()
            .to_string();

        let listed = handle_lab_command(temp.path(), Some("session".to_string()), "task list");
        assert!(listed.contains("Graduate tasks: 1 total, 1 open"));
        assert!(listed.contains("Implement task queue"));

        let envelope = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task envelope {task_id}"),
        );
        assert!(envelope.contains("Graduate task envelope"));
        assert!(envelope.contains("\"profile\": \"lab-graduate\""));
        assert!(envelope.contains("\"context_mode\": \"isolated_worktree_fork\""));
        assert!(envelope.contains("GraduateResult"));

        let dispatch = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task dispatch {task_id}"),
        );
        assert!(dispatch.contains("Prepared graduate dispatch"));
        assert!(dispatch.contains("Status: Prepared"));
        assert!(dispatch.contains("Dispatch: "));

        let started = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task start {task_id}"),
        );
        assert!(started.contains("Status: InProgress"));

        let completed = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!(
                "task result {task_id} | src/lab/model.rs | cargo check -q | | labevidence_001 | Implemented task queue"
            ),
        );
        assert!(completed.contains("Created graduate result artifact"));
        assert!(completed.contains("Report: "));
        assert!(completed.contains("Gate status: satisfied"));

        let listed = handle_lab_command(temp.path(), Some("session".to_string()), "tasks");
        assert!(listed.contains("Graduate tasks: 1 total, 0 open"));
        assert!(listed.contains("Completed"));
    }

    #[test]
    fn task_bind_json_command_binds_agent_contract_output() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let created = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "task create Bind graduate JSON | src/lab/model.rs | cargo check -q | Verify structured graduate output binding",
        );
        let task_id = created
            .lines()
            .find_map(|line| line.strip_prefix("Created graduate task: "))
            .unwrap()
            .to_string();
        let json_path = temp.path().join("graduate-result.json");
        std::fs::write(
            &json_path,
            serde_json::json!({
                "result": serde_json::json!({
                    "graduate_result": {
                        "summary": "Bound structured graduate output.",
                        "changed_files": ["src/lab/model.rs"],
                        "validation_results": ["cargo check -q passed"],
                        "blockers": [],
                        "evidence_ids": ["labevidence_bind_json"]
                    }
                })
                .to_string()
            })
            .to_string(),
        )
        .unwrap();

        let bound = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task bind-json {task_id} {}", json_path.display()),
        );

        assert!(bound.contains("Bound graduate agent JSON result"));
        assert!(bound.contains("Gate status: satisfied"));
        let listed = handle_lab_command(temp.path(), Some("session".to_string()), "tasks");
        assert!(listed.contains("Graduate tasks: 1 total, 0 open"));
        assert!(listed.contains("Completed"));
    }

    #[tokio::test]
    async fn task_sync_command_binds_completed_durable_graduate_result() {
        let temp = tempfile::tempdir().unwrap();
        init_lab_command_git_repo(temp.path());
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build it", None)
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
                "Sync durable result",
                "Update scoped file.",
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

        let worktree = temp.path().join("graduate-sync-worktree");
        std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
        lab_command_git(&worktree, &["init", "-q"]);
        lab_command_git(&worktree, &["config", "user.email", "lab@example.test"]);
        lab_command_git(&worktree, &["config", "user.name", "Lab Test"]);
        std::fs::write(
            worktree.join("src/lab/orchestrator.rs"),
            "durable graduate command sync\n",
        )
        .unwrap();

        let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        session_store
            .create_session("lab-test", "lab command sync", "test-model", None)
            .unwrap();
        let agent_task_id = crate::lab::delegation::graduate_agent_task_id(&task);
        let agent_artifact_id = session_store
            .add_agent_artifact(
                "lab-test",
                "agent_sync",
                Some("lab-graduate"),
                "implementation",
                "completed",
                "graduate durable sync result",
                r#"{"graduate_result":{"summary":"Synced command result.","changed_files":["src/lab/orchestrator.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
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

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("lab-test".to_string()),
            &format!("task sync {}", task.task_id),
            ToolContext::new(temp.path(), "lab-test").with_session_store(session_store),
        )
        .await;

        assert!(output.contains("Synced graduate durable subagent result"));
        assert!(output.contains("Gate status: satisfied"));
        let saved_task = orchestrator
            .store()
            .load_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();
        assert_eq!(
            saved_task.status,
            crate::lab::model::LabTaskStatus::Completed
        );
        assert!(saved_task
            .evidence_ids
            .contains(&format!("agent_task:{agent_task_id}")));
        let saved_dispatch = orchestrator
            .store()
            .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
            .unwrap();
        assert_eq!(
            saved_dispatch.status,
            crate::lab::model::GraduateDispatchStatus::Succeeded
        );
        assert_eq!(saved_dispatch.agent_id.as_deref(), Some("agent_sync"));
    }

    #[tokio::test]
    async fn task_run_command_uses_runtime_context_and_records_failure() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let created = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "task create Implement task queue | src/lab/model.rs | cargo check -q | Add graduate task persistence and tests",
        );
        let task_id = created
            .lines()
            .find_map(|line| line.strip_prefix("Created graduate task: "))
            .unwrap()
            .to_string();

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            &format!("task run {task_id}"),
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;

        assert!(output.contains("Graduate task run dispatched"));
        assert!(output.contains("Status: Failed"));
        assert!(output.contains("AgentManager not available"));
        let store = LabStore::for_project(temp.path());
        let run = store.latest_run().unwrap().unwrap();
        let task = store.load_graduate_task(&run.lab_run_id, &task_id).unwrap();
        assert_eq!(task.status, crate::lab::model::LabTaskStatus::Blocked);
    }

    #[tokio::test]
    async fn task_worktree_command_falls_back_to_durable_task_id() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let created = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "task create Fix lab model | src/lab/model.rs | cargo check -q | update model",
        );
        let task_id = created
            .lines()
            .find_map(|line| line.strip_prefix("Created graduate task: "))
            .unwrap()
            .to_string();
        let dispatch = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task dispatch {task_id}"),
        );
        assert!(dispatch.contains("Prepared graduate dispatch"));

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            &format!("task worktree review {task_id}"),
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;

        assert!(output.contains("via task_id"));
        assert!(output.contains("lab-graduate-"));
        assert!(output.contains("Worktree manager not available"));
    }

    #[tokio::test]
    async fn task_worktree_command_reviews_durable_task_id_worktree() {
        let temp = tempfile::tempdir().unwrap();
        init_lab_command_git_repo(temp.path());
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let created = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "task create Fix hello | hello.txt | test -f hello.txt | update hello",
        );
        let task_id = created
            .lines()
            .find_map(|line| line.strip_prefix("Created graduate task: "))
            .unwrap()
            .to_string();
        let dispatch = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task dispatch {task_id}"),
        );
        assert!(dispatch.contains("Prepared graduate dispatch"));

        let lab_store = LabStore::for_project(temp.path());
        let run = lab_store.latest_run().unwrap().unwrap();
        let dispatch = lab_store
            .list_graduate_dispatches(&run.lab_run_id)
            .unwrap()
            .into_iter()
            .find(|dispatch| dispatch.task_id == task_id)
            .unwrap();
        assert!(dispatch.agent_id.is_none());
        let durable_task_id = dispatch.agent_tool_params["task_id"]
            .as_str()
            .unwrap()
            .to_string();

        let manager = Arc::new(crate::engine::worktree::WorktreeManager::for_root(
            temp.path().to_path_buf(),
        ));
        let branch = "codex/lab-command-durable-review";
        let worktree_path = manager
            .create("lab-command-durable-review", Some(branch))
            .await
            .unwrap();
        std::fs::write(worktree_path.join("hello.txt"), "agent edit\n").unwrap();

        let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        session_store
            .create_session(
                "lab-test",
                "lab command durable worktree",
                "test-model",
                None,
            )
            .unwrap();
        session_store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: "lab-test".to_string(),
                task_id: durable_task_id.clone(),
                agent_id: "agent_runtime_1".to_string(),
                profile: Some("lab-graduate".to_string()),
                role: "implementation".to_string(),
                status: "completed".to_string(),
                description: "durable graduate worktree".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: None,
                cleanup_hooks: vec!["worktree_cleanup".to_string()],
                payload: serde_json::json!({
                    "isolated_worktree": {
                        "path": worktree_path.to_string_lossy().to_string(),
                        "branch": branch
                    }
                }),
            })
            .unwrap();

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            &format!("task worktree review {task_id}"),
            ToolContext::new(temp.path(), "lab-test")
                .with_session_store(session_store)
                .with_worktree_manager(manager),
        )
        .await;

        assert!(output.contains("Lab graduate worktree review succeeded"));
        assert!(output.contains("via task_id"));
        assert!(output.contains(&durable_task_id));
        assert!(output.contains("Agent worktree review: agent_runtime_1"));
        assert!(output.contains("hello.txt"));
    }

    #[tokio::test]
    async fn task_worktree_command_merges_and_cleans_durable_task_id_worktree() {
        let temp = tempfile::tempdir().unwrap();
        init_lab_command_git_repo(temp.path());
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let created = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "task create Merge hello | hello.txt | test -f hello.txt | merge hello edit",
        );
        let task_id = created
            .lines()
            .find_map(|line| line.strip_prefix("Created graduate task: "))
            .unwrap()
            .to_string();
        let dispatch = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task dispatch {task_id}"),
        );
        assert!(dispatch.contains("Prepared graduate dispatch"));

        let lab_store = LabStore::for_project(temp.path());
        let run = lab_store.latest_run().unwrap().unwrap();
        let dispatch = lab_store
            .list_graduate_dispatches(&run.lab_run_id)
            .unwrap()
            .into_iter()
            .find(|dispatch| dispatch.task_id == task_id)
            .unwrap();
        let durable_task_id = dispatch.agent_tool_params["task_id"]
            .as_str()
            .unwrap()
            .to_string();

        let manager = Arc::new(crate::engine::worktree::WorktreeManager::for_root(
            temp.path().to_path_buf(),
        ));
        let branch = "codex/lab-command-durable-merge";
        let worktree_path = manager
            .create("lab-command-durable-merge", Some(branch))
            .await
            .unwrap();
        std::fs::write(worktree_path.join("hello.txt"), "agent merged edit\n").unwrap();

        let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        session_store
            .create_session("lab-test", "lab command durable merge", "test-model", None)
            .unwrap();
        session_store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: "lab-test".to_string(),
                task_id: durable_task_id.clone(),
                agent_id: "agent_runtime_2".to_string(),
                profile: Some("lab-graduate".to_string()),
                role: "implementation".to_string(),
                status: "completed".to_string(),
                description: "durable graduate merge worktree".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: None,
                cleanup_hooks: vec!["worktree_cleanup".to_string()],
                payload: serde_json::json!({
                    "isolated_worktree": {
                        "path": worktree_path.to_string_lossy().to_string(),
                        "branch": branch
                    }
                }),
            })
            .unwrap();

        let context = ToolContext::new(temp.path(), "lab-test")
            .with_session_store(session_store)
            .with_worktree_manager(manager);
        let merge = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            &format!("task worktree merge {task_id}"),
            context.clone(),
        )
        .await;

        assert!(merge.contains("Lab graduate worktree merge succeeded"));
        assert!(merge.contains("via task_id"));
        assert!(merge.contains(&durable_task_id));
        assert_eq!(
            std::fs::read_to_string(temp.path().join("hello.txt")).unwrap(),
            "agent merged edit\n"
        );
        assert!(
            worktree_path.exists(),
            "merge should not remove dirty graduate worktree without cleanup"
        );

        let cleanup = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            &format!("task worktree cleanup {task_id} force"),
            context,
        )
        .await;

        assert!(cleanup.contains("Lab graduate worktree cleanup succeeded"));
        assert!(cleanup.contains("via task_id"));
        let cleaned_dispatch = lab_store
            .load_graduate_dispatch(&run.lab_run_id, &dispatch.dispatch_id)
            .unwrap();
        assert_eq!(
            cleaned_dispatch.cleanup_status,
            GraduateCleanupStatus::CleanupDone
        );
        assert!(cleaned_dispatch
            .cleanup_message
            .as_deref()
            .unwrap_or_default()
            .contains("cleanup succeeded"));
        assert!(
            !worktree_path.exists(),
            "force cleanup should remove the graduate worktree"
        );

        let events = std::fs::read_to_string(
            lab_store
                .root()
                .join("runs")
                .join(&run.lab_run_id)
                .join("events.jsonl"),
        )
        .unwrap();
        assert!(events.contains("\"event_type\":\"lab_graduate_worktree_action\""));
        assert!(events.contains("\"agent_ref_kind\":\"task_id\""));
        assert!(events.contains(&durable_task_id));
        assert!(events.contains("\"result_data\""));
        assert!(events.contains("\"merge_kind\":\"tracked_diff\""));
        assert!(events.contains("\"result_content_preview\""));

        let review = handle_lab_command(temp.path(), Some("session".to_string()), "review");
        assert!(review.contains("Graduate cleanup states:"));
        assert!(review.contains("cleanup_done"));
        assert!(review.contains("Graduate worktree proof:"));
        assert!(review.contains("agent_merge"));
        assert!(review.contains("agent_cleanup"));
        assert!(review.contains("ref=task_id:lab-graduate-"));
        assert!(review.contains("merge_kind=tracked_diff"));

        let dashboard = handle_lab_command(temp.path(), Some("session".to_string()), "dashboard");
        assert!(dashboard.contains("Graduate cleanup states:"));
        assert!(dashboard.contains("cleanup_done"));
        assert!(dashboard.contains("Graduate worktree proof:"));
        assert!(dashboard.contains("agent_merge"));
        assert!(dashboard.contains("ref=task_id:lab-graduate-"));
        assert!(dashboard.contains("merge_kind=tracked_diff"));
        let recovery = handle_lab_command(temp.path(), Some("session".to_string()), "recovery");
        assert!(recovery.contains("Graduate cleanup states:"));
        assert!(recovery.contains("cleanup_done"));
    }

    #[tokio::test]
    async fn step_command_blocks_graduate_stage_without_task() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let store = LabStore::for_project(temp.path());
        let mut run = store.latest_run().unwrap().unwrap();
        run.current_stage = "graduate_work".to_string();
        run.internal_owner = LabRole::Graduate;
        store.save_run(&run).unwrap();

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "step",
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;

        assert!(output.contains("Lab scheduler step: Blocked"));
        assert!(output.contains("requires a queued GraduateTask"));
    }

    #[tokio::test]
    async fn run_command_stops_when_scheduler_blocks() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "run 5",
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;

        assert!(output.contains("Blocked"));
        assert!(output.contains("Scheduler blocked at professor_discussion"));
    }

    #[tokio::test]
    async fn background_command_starts_reports_and_stops_scheduler() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let started = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background start 3 100",
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;
        assert!(started.contains("Started Lab background scheduler"));

        let status = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background status",
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;
        assert!(status.contains("Running in process: true"));

        let stopped = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background stop",
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;
        assert!(stopped.contains("Stopped Lab background scheduler"));
    }

    #[tokio::test]
    async fn background_hybrid_command_requires_provider_context() {
        let temp = tempfile::tempdir().unwrap();
        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background hybrid 3 100 focus",
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;

        assert!(output.contains("requires an active Lab Mode provider"));
    }

    #[tokio::test]
    async fn background_hybrid_command_starts_reports_and_stops_scheduler() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let provider = Arc::new(SequenceCommandProvider {
            responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Build LabRun",
                        "strategic_direction": "Keep background hybrid bounded.",
                        "success_criteria": ["hybrid background starts"],
                        "constraints": ["do not bypass runtime gates"],
                        "risks": ["weak provider evidence"],
                        "handoff_to_postdoc": "Create a small plan."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready"}"#.to_string(),
            ])),
        });
        let context = ToolContext::new(temp.path(), "lab-background-hybrid-command")
            .with_llm_provider(provider)
            .with_model("mock-sequence".to_string());

        let started = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background hybrid 3 100 background focus",
            context.clone(),
        )
        .await;
        assert!(started.contains("Started Lab hybrid background scheduler"));

        let status = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background status",
            context.clone(),
        )
        .await;
        assert!(status.contains("Running in process: true"));
        assert!(status.contains("Persisted status: Running"));

        let stopped = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background stop",
            context,
        )
        .await;
        assert!(stopped.contains("Stopped Lab background scheduler"));
    }

    #[tokio::test]
    async fn background_hybrid_cycles_command_requires_provider_context() {
        let temp = tempfile::tempdir().unwrap();
        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background hybrid-cycles 2 5 100 focus",
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;

        assert!(output.contains("requires an active Lab Mode provider"));
    }

    #[tokio::test]
    async fn background_hybrid_cycles_command_starts_reports_and_stops_scheduler() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let provider = Arc::new(SequenceCommandProvider {
            responses: parking_lot::Mutex::new(std::collections::VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Build LabRun",
                        "strategic_direction": "Keep background cycles bounded.",
                        "success_criteria": ["hybrid-cycle background starts"],
                        "constraints": ["do not bypass runtime gates"],
                        "risks": ["weak provider evidence"],
                        "handoff_to_postdoc": "Create a small plan."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready"}"#.to_string(),
            ])),
        });
        let context = ToolContext::new(temp.path(), "lab-background-hybrid-cycles-command")
            .with_llm_provider(provider)
            .with_model("mock-sequence".to_string());

        let started = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background hybrid-cycles 2 5 100 background cycles",
            context.clone(),
        )
        .await;
        assert!(started.contains("Started Lab hybrid-cycle background scheduler"));
        assert!(started.contains("Max cycles: 2"));

        let status = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background status",
            context.clone(),
        )
        .await;
        assert!(status.contains("Running in process: true"));
        assert!(status.contains("Persisted status: Running"));

        let stopped = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background stop",
            context,
        )
        .await;
        assert!(stopped.contains("Stopped Lab background scheduler"));
    }

    #[tokio::test]
    async fn background_start_refuses_missing_active_lease() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let store = LabStore::for_project(temp.path());
        let run = store.latest_run().unwrap().unwrap();
        std::fs::remove_file(store.root().join("active_lease.json")).unwrap();

        let output = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background start 3 100",
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;

        assert!(output.contains("Failed to start Lab background scheduler"));
        assert!(output.contains("active lease is missing"));
        assert!(store
            .load_scheduler_state(&run.lab_run_id)
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn background_recover_marks_interrupted_scheduler_resumable() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let store = LabStore::for_project(temp.path());
        let run = store.latest_run().unwrap().unwrap();
        let now = chrono::Utc::now();
        store
            .write_scheduler_state(&crate::lab::model::LabSchedulerState {
                schema_version: crate::lab::model::LAB_SCHEMA_VERSION,
                lab_run_id: run.lab_run_id.clone(),
                status: crate::lab::model::LabSchedulerStatus::Running,
                updated_at: now,
                started_at: Some(now),
                stopped_at: None,
                max_steps: 10,
                steps_completed: 2,
                interval_ms: 250,
                last_action: None,
                last_message: None,
                stop_reason: None,
            })
            .unwrap();

        let recovered = handle_lab_command_with_context(
            temp.path(),
            Some("session".to_string()),
            "background recover",
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await;

        assert!(recovered.contains("Recovered interrupted Lab background scheduler"));
        assert!(recovered.contains("Status: PausedRestart"));
        assert!(recovered.contains("Stop reason: process_restart"));
    }

    #[test]
    fn cycle_summary_command_writes_artifact_and_report() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "cycle summary Finished initial planning slice",
        );

        assert!(output.contains("Created cycle summary"));
        assert!(output.contains("Artifact: "));
        assert!(output.contains("Report: "));
        let status = handle_lab_command(temp.path(), Some("session".to_string()), "status");
        assert!(status.contains("Cycles: 1"));
    }

    #[test]
    fn meeting_recommend_command_reports_no_signal_by_default() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "meeting recommend",
        );

        assert!(output.contains("Suggested meeting: false"));
        assert!(output.contains("Signals: none"));
    }

    #[test]
    fn meeting_open_refuses_without_recommendation_signal() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let output = handle_lab_command(temp.path(), Some("session".to_string()), "meeting open");

        assert!(output.contains("No runtime escalation signal is open"));
        assert!(output.contains("Use /lab meeting <topic>"));
        let store = LabStore::for_project(temp.path());
        let run = store.latest_run().unwrap().unwrap();
        assert!(run.meeting_ids.is_empty());
    }

    #[test]
    fn meeting_open_creates_read_only_report_from_recommendation_signal() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let queued = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "professor Turn this concern into a scoped implementation task",
        );
        let message_id = queued
            .lines()
            .find_map(|line| line.strip_prefix("Message queued for professor: "))
            .unwrap()
            .to_string();
        let converted = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("messages task {message_id}"),
        );
        assert!(converted.contains("ConvertedToTask"));
        let applied = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("messages apply {message_id} implementation task"),
        );
        assert!(applied.contains("applied as blocked graduate task"));
        let recommendation = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "meeting recommend",
        );
        assert!(recommendation.contains("Suggested meeting: true"));
        assert!(recommendation.contains("Open meeting with /lab meeting open"));

        let opened = handle_lab_command(temp.path(), Some("session".to_string()), "meeting open");

        assert!(opened.contains("Lab meeting opened from runtime escalation signal"));
        assert!(opened.contains("This meeting is read-only and does not mutate code."));
        assert!(opened.contains("Topic: resolve 1 blocked graduate task(s)"));
        assert!(opened.contains("Request: "));
        assert!(opened.contains("Request report: "));
        assert!(opened.contains("Artifact: "));
        assert!(opened.contains("Report: "));
        let store = LabStore::for_project(temp.path());
        let run = store.latest_run().unwrap().unwrap();
        assert_eq!(run.meeting_ids.len(), 1);
        let artifacts = store.list_stage_artifacts(&run.lab_run_id).unwrap();
        assert!(artifacts.iter().any(|artifact| matches!(
            artifact,
            StageArtifact::LabMeetingRequest(request)
                if request.body.reason == "runtime_escalation_signals_present"
                    && request.body.topic.starts_with("resolve 1 blocked graduate task")
        )));
        assert!(artifacts
            .iter()
            .any(|artifact| matches!(artifact, StageArtifact::LabMeetingSummary(_))));
        assert!(store.root().join("active_lease.json").exists());
    }

    #[test]
    fn blocker_report_command_writes_artifact_and_report() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let created = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "task create Fix lab model | src/lab/model.rs | cargo check -q | update model",
        );
        let task_id = created
            .lines()
            .find_map(|line| line.strip_prefix("Created graduate task: "))
            .unwrap()
            .to_string();
        let blocked = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task block {task_id} validation failed"),
        );
        assert!(blocked.contains("Blocked graduate task"));

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "blocker report Need professor decision",
        );

        assert!(output.contains("Lab blocker report created"));
        assert!(output.contains("Artifact: "));
        assert!(output.contains("Report: "));

        let escalated =
            handle_lab_command(temp.path(), Some("session".to_string()), "blocker escalate");
        assert!(escalated.contains("Escalated Lab blocker to professor review"));
        assert!(escalated.contains("Stage: professor_review"));
    }

    #[test]
    fn task_revise_command_requeues_blocked_task() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let created = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "task create Fix lab model | | cargo check -q | update model",
        );
        let task_id = created
            .lines()
            .find_map(|line| line.strip_prefix("Created graduate task: "))
            .unwrap()
            .to_string();
        let blocked = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task block {task_id} missing scope"),
        );
        assert!(blocked.contains("Blocked graduate task"));

        let revised = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!(
                "task revise {task_id} | src/lab/commands.rs | cargo check -q --tests | narrow command repair"
            ),
        );

        assert!(revised.contains("Revised graduate task"));
        assert!(revised.contains("Status: Queued"));
        assert!(revised.contains("src/lab/commands.rs"));
        assert!(revised.contains("cargo check -q --tests"));
        assert!(revised.contains("Blocker: none"));
    }

    #[test]
    fn integrate_command_writes_postdoc_summary() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", Some("session".to_string()))
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab commands.",
                vec!["src/lab/commands.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented command path.",
                vec!["src/lab/commands.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = crate::lab::model::LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "integrate Ready for professor review",
        );

        assert!(output.contains("Created postdoc integration summary"));
        assert!(output.contains("Gate: postdoc_review (satisfied)"));
        assert!(output.contains("Artifact: "));
        assert!(output.contains("Report: "));
    }

    #[test]
    fn professor_review_command_writes_final_review() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = LabOrchestrator::for_project(temp.path());
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", Some("session".to_string()))
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update lab commands.",
                vec!["src/lab/commands.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented command path.",
                vec!["src/lab/commands.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = crate::lab::model::LabRole::Postdoc;
        orchestrator.store().save_run(&saved).unwrap();
        orchestrator
            .create_postdoc_integration_summary_for_latest(Some("Ready for professor."))
            .unwrap();
        let advanced = orchestrator.advance_latest().unwrap();
        assert_eq!(advanced.current_stage, "professor_review");

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "professor-review Final professor review",
        );

        assert!(output.contains("Created professor review"));
        assert!(output.contains("Gate: professor_review (blocked)"));
        assert!(output.contains("Artifact: "));
        assert!(output.contains("Report: "));
    }

    #[test]
    fn task_retry_command_creates_repair_task() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let created = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "task create Fix lab model | src/lab/model.rs | cargo check -q | update model",
        );
        let task_id = created
            .lines()
            .find_map(|line| line.strip_prefix("Created graduate task: "))
            .unwrap()
            .to_string();

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("task retry {task_id} | cargo check failed"),
        );

        assert!(output.contains("Recorded validation retry"));
        assert!(output.contains("Attempt: 1"));
        assert!(output.contains("Repair task: gradtask_"));
        assert!(output.contains("Escalated: false"));

        let blocker_status =
            handle_lab_command(temp.path(), Some("session".to_string()), "blocker status");
        assert!(blocker_status.contains("validation_retries=1"));
        assert!(blocker_status.contains("escalated_retries=0"));
    }

    #[test]
    fn compression_command_records_context_decision() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "compression professor",
        );

        assert!(output.contains("Lab compression decision"));
        assert!(output.contains("role=Professor"));
        assert!(output.contains("action="));
        assert!(output.contains("stable_hash="));
        assert!(temp.path().join(".priority-agent/lab/runs").exists());
    }

    #[test]
    fn compress_command_writes_summary_when_budget_requires_it() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));
        let store = LabStore::for_project(temp.path());
        let mut run = store.latest_run().unwrap().unwrap();
        run.cost_policy.professor_context_budget = 10;
        store.save_run(&run).unwrap();

        let output = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            "compress professor",
        );

        assert!(output.contains("Created compression summary"));
        assert!(output.contains("Artifact: "));
        assert!(output.contains("Report: "));
    }

    #[test]
    fn tick_command_runs_one_orchestration_step() {
        let temp = tempfile::tempdir().unwrap();
        let proposal =
            handle_lab_command(temp.path(), Some("session".to_string()), "propose Build it");
        let proposal_id = proposal
            .lines()
            .find_map(|line| line.strip_prefix("Lab proposal created: "))
            .unwrap()
            .to_string();
        let approved = handle_lab_command(
            temp.path(),
            Some("session".to_string()),
            &format!("approve {proposal_id}"),
        );
        assert!(approved.contains("LabRun created"));

        let output = handle_lab_command(temp.path(), Some("session".to_string()), "tick");

        assert!(output.contains("Lab tick: Blocked"));
        assert!(output.contains("Stage: professor_discussion -> professor_discussion"));
    }
}
