//! Learning, evolution and recovery slash command handlers.

use super::utils::*;

use crate::engine::task_contract::{MemoryProposalBatchFilter, MemoryProposalStatus};
use crate::tui::app::TuiApp;

mod skills;
use skills::*;

mod improvements;
mod memory_proposals;
pub use improvements::*;
pub use memory_proposals::*;

/// /quick - Quick actions menu
pub fn handle_quick(app: &mut TuiApp) -> String {
    let session = app
        .session_manager
        .current_session_id()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "none".to_string());
    let pending = [
        app.pending_plan.is_some(),
        app.pending_permission_request.is_some(),
        app.pending_question.is_some(),
    ]
    .into_iter()
    .filter(|b| *b)
    .count();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let workspace = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace");
    let recent_commands = if app.recent_palette_commands.is_empty() {
        "none yet".to_string()
    } else {
        app.recent_palette_commands
            .iter()
            .rev()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let goal_line = app
        .streaming_engine
        .as_ref()
        .and_then(|engine| engine.goal_manager().current())
        .map(|goal| goal.compact_status())
        .unwrap_or_else(|| "none".to_string());
    let drift_line = latest_trace_for_app(app)
        .map(|trace| goal_drift_count_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let resource_line = latest_trace_for_app(app)
        .and_then(|trace| latest_resource_policy_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let runtime_diet_line = latest_trace_for_app(app)
        .and_then(|trace| crate::engine::trace::latest_runtime_diet_summary(&trace))
        .map(|line| compact_inline(&line, 120))
        .unwrap_or_else(|| "none".to_string());
    let contract_line = latest_trace_for_app(app)
        .map(|trace| latest_contract_state_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let retrieval_line = latest_trace_for_app(app)
        .and_then(|trace| latest_retrieval_context_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let reflection_line = latest_trace_for_app(app)
        .and_then(|trace| latest_reflection_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let stage_line = latest_trace_for_app(app)
        .and_then(|trace| latest_stage_validation_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let acceptance_line = latest_trace_for_app(app)
        .and_then(|trace| latest_acceptance_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let debugging_line = latest_trace_for_app(app)
        .and_then(|trace| latest_guided_debugging_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let plan_line = latest_trace_for_app(app)
        .and_then(|trace| latest_workflow_plan_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let closeout_line = latest_trace_for_app(app)
        .and_then(|trace| latest_closeout_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let memory_proposal_line = latest_trace_for_app(app)
        .and_then(|trace| crate::engine::trace::latest_memory_proposal_summary(&trace))
        .map(|line| compact_inline(&line, 120))
        .unwrap_or_else(|| "none".to_string());
    let active_task_plan = active_task_plan_for_app(app);
    let a2a_line = latest_a2a_transcript_label();

    format!(
        "Quick Panel\n\nStatus:\n- Agent mode: {}\n- UI mode: {:?}\n- Querying: {}\n- Pending prompts: {}\n- Messages: {}\n- Session: {}\n- Goal: {}\n- Goal drift: {}\n\nActive task:\n{}\n\nRuntime:\n- Provider: {}\n- Model: {}\n- Permissions: {}\n- Resource policy: {}\n- Runtime diet: {}\n- Recent commands: {}\n\nContracts:\n- State: {}\n- Plan: {}\n- Stage: {}\n- Retrieval: {}\n- Reflection: {}\n- Acceptance: {}\n- Guided debug: {}\n- Closeout: {}\n- Memory proposal: {}\n- A2A: {}\n\nWorkspace:\n- Project: {}\n- Path: {}\n- {}\n\nNext actions:\n1. /active-task   inspect unified task/progress state\n2. /memory-proposals review memory candidates\n3. /mode          switch auto/build/plan/explore/review\n4. /resource      inspect latest resource budget\n5. /goal          inspect or pin the active goal\n6. /project pulse inspect the next project step\n7. /doctor        run environment diagnostics",
        app.current_agent_mode_label(),
        app.mode,
        app.is_querying,
        pending,
        app.messages.len(),
        &session[..8.min(session.len())],
        goal_line,
        drift_line,
        active_task_plan.format(),
        app.current_provider_label(),
        app.current_model_label(),
        app.current_permission_label(),
        resource_line,
        runtime_diet_line,
        recent_commands,
        contract_line,
        plan_line,
        stage_line,
        retrieval_line,
        reflection_line,
        acceptance_line,
        debugging_line,
        closeout_line,
        memory_proposal_line,
        a2a_line,
        workspace,
        cwd.display(),
        quick_git_line(&cwd)
    )
}

/// /active-task - Unified current task/progress panel
pub fn handle_active_task(app: &mut TuiApp) -> String {
    active_task_plan_for_app(app).format()
}

fn active_task_plan_for_app(app: &mut TuiApp) -> crate::engine::active_task_plan::ActiveTaskPlan {
    let goal = app
        .streaming_engine
        .as_ref()
        .and_then(|engine| engine.goal_manager().current());
    let trace = latest_trace_for_app(app);
    let project_progress =
        crate::engine::project_progress::ProjectProgressLedger::default().latest_summary();
    crate::engine::active_task_plan::ActiveTaskPlan::from_goal_trace_and_project_progress(
        goal.as_ref(),
        trace.as_ref(),
        project_progress,
    )
}

fn latest_resource_policy_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::ResourcePolicySelected {
            latency,
            cost_ceiling_usd,
            reasoning,
            parallelism_limit,
            max_tool_calls,
            ..
        } = event
        {
            Some(format!(
                "{} ${:.2} {} p{} tools{}",
                latency, cost_ceiling_usd, reasoning, parallelism_limit, max_tool_calls
            ))
        } else {
            None
        }
    })
}

fn latest_contract_state_label(trace: &crate::engine::trace::TurnTrace) -> String {
    let mut task = false;
    let mut judgment = false;
    let mut plan = false;
    let mut retrieval = false;
    let mut reflection = false;
    let mut verification = false;
    let mut acceptance = false;
    let mut debugging = false;
    let mut stage = false;
    let mut closeout = false;
    for event in &trace.events {
        match event {
            crate::engine::trace::TraceEvent::TaskContextBuilt { .. } => task = true,
            crate::engine::trace::TraceEvent::WorkflowJudgmentCompleted { .. } => judgment = true,
            crate::engine::trace::TraceEvent::WorkflowPlanProgress { .. } => plan = true,
            crate::engine::trace::TraceEvent::StageValidationCompleted { .. } => stage = true,
            crate::engine::trace::TraceEvent::RetrievalContextBuilt { .. } => retrieval = true,
            crate::engine::trace::TraceEvent::ReflectionPassCompleted { .. } => reflection = true,
            crate::engine::trace::TraceEvent::VerificationCompleted { .. } => verification = true,
            crate::engine::trace::TraceEvent::AcceptanceReviewCompleted { .. } => acceptance = true,
            crate::engine::trace::TraceEvent::GuidedDebuggingCompleted { .. } => debugging = true,
            crate::engine::trace::TraceEvent::FinalCloseoutPrepared { .. } => closeout = true,
            _ => {}
        }
    }
    let mut parts = Vec::new();
    if task {
        parts.push("task");
    }
    if judgment {
        parts.push("judgment");
    }
    if plan {
        parts.push("plan");
    }
    if stage {
        parts.push("stage");
    }
    if retrieval {
        parts.push("retrieval");
    }
    if reflection {
        parts.push("reflection");
    }
    if verification {
        parts.push("verification");
    }
    if acceptance {
        parts.push("acceptance");
    }
    if debugging {
        parts.push("debug");
    }
    if closeout {
        parts.push("closeout");
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

fn latest_retrieval_context_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::RetrievalContextBuilt {
            policy,
            sources,
            items,
            estimated_tokens,
            conflicts,
            ..
        } = event
        {
            Some(format!(
                "{} {} item(s) from {} tokens~{} conflicts={}",
                policy,
                items,
                sources.join("+"),
                estimated_tokens,
                conflicts
            ))
        } else {
            None
        }
    })
}

fn latest_reflection_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::ReflectionPassCompleted {
            status,
            findings,
            unresolved,
            ..
        } = event
        {
            Some(format!(
                "{} findings={} unresolved={}",
                status, findings, unresolved
            ))
        } else {
            None
        }
    })
}

fn latest_stage_validation_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::StageValidationCompleted {
            step,
            status,
            changed_files,
            evidence_items,
        } = event
        {
            Some(format!(
                "{} step={} files={} evidence={}",
                status,
                step.as_deref()
                    .map(|step| compact_inline(step, 60))
                    .unwrap_or_else(|| "none".to_string()),
                changed_files,
                evidence_items
            ))
        } else {
            None
        }
    })
}

fn latest_workflow_plan_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::WorkflowPlanProgress {
            total_steps,
            completed_steps,
            active_step,
            top_priority,
            top_importance_score: _,
            top_weight_share: _,
            weight_source: _,
            reweighted,
        } = event
        {
            let reweighted_suffix = if *reweighted { " reweighted" } else { "" };
            Some(format!(
                "{}/{} {} ({}){}",
                completed_steps,
                total_steps,
                active_step
                    .as_deref()
                    .map(|step| compact_inline(step, 60))
                    .unwrap_or_else(|| "none".to_string()),
                top_priority.as_deref().unwrap_or("none"),
                reweighted_suffix
            ))
        } else {
            None
        }
    })
}

fn latest_closeout_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::FinalCloseoutPrepared {
            status,
            changed_files,
            validation_items,
            tool_records,
            tool_evidence,
            verification_proof_status,
            verification_proof_summary,
            acceptance_items,
            residual_risks,
            ..
        } = event
        {
            Some(format!(
                "{} files={} validation={} tool_records={} tool_evidence={} proof={} proof_summary={} acceptance={} risks={}",
                status,
                changed_files,
                validation_items,
                tool_records,
                tool_evidence.as_deref().unwrap_or("none"),
                verification_proof_status.as_deref().unwrap_or("none"),
                verification_proof_summary.as_deref().unwrap_or("none"),
                acceptance_items,
                residual_risks
            ))
        } else {
            None
        }
    })
}

fn latest_acceptance_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::AcceptanceReviewCompleted {
            accepted,
            confidence,
            criteria,
            unresolved,
            next_action,
        } = event
        {
            Some(format!(
                "{} confidence={} criteria={} unresolved={} next={}",
                if *accepted {
                    "accepted"
                } else {
                    "not accepted"
                },
                confidence,
                criteria,
                unresolved,
                next_action
            ))
        } else {
            None
        }
    })
}

fn latest_guided_debugging_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::GuidedDebuggingCompleted {
            blocker,
            next_action,
            causes,
            evidence_items,
            ask_user,
        } = event
        {
            Some(format!(
                "blocker={} next={} causes={} evidence={} ask_user={}",
                blocker, next_action, causes, evidence_items, ask_user
            ))
        } else {
            None
        }
    })
}

fn latest_a2a_transcript_label() -> String {
    match crate::agent::a2a_transcript::read_recent(1) {
        Ok(records) if !records.is_empty() => {
            let record = records.last().expect("checked non-empty");
            format!(
                "{:?} {} -> {} artifacts={} goal={}",
                record.status,
                record.from,
                record.to.as_deref().unwrap_or("unassigned"),
                record.artifacts,
                compact_inline(&record.goal, 60)
            )
        }
        _ => "none".to_string(),
    }
}

/// /goal - Show or pin the current session goal
pub fn handle_goal(app: &mut TuiApp, args: &str) -> String {
    let trimmed = args.trim();
    if trimmed.starts_with("drift") {
        let limit = trimmed
            .strip_prefix("drift")
            .unwrap_or_default()
            .trim()
            .parse::<usize>()
            .unwrap_or(8)
            .clamp(1, 50);
        return match latest_trace_for_app(app) {
            Some(trace) => format_goal_drift_report(&trace, limit),
            None => "Goal Drift\n- none yet".to_string(),
        };
    }

    let Some(engine) = app.streaming_engine.as_ref() else {
        return "Current Goal\n- unavailable (no engine connected)".to_string();
    };
    let manager = engine.goal_manager();

    // Ensure the runner is initialized before any operation that needs it
    let has_runner = app.lazy_goal_runner().is_some();

    // Explicit subcommand dispatch
    match trimmed {
        "" | "status" | "show" => {
            if has_runner {
                if let Some(session_id) = app.session_manager.current_session_id() {
                    if let Some(ref runner) = app.goal_runner {
                        match runner.status(session_id) {
                            Ok(info) => {
                                if let Some(ref goal) = info.goal {
                                    return format!(
                                        "Current Goal\n- Id: {}\n- Objective: {}\n- Status: {:?}\n- Turn: {}/{}\n- Steps: {}\n- Updated: {}",
                                        goal.id,
                                        goal.objective,
                                        goal.status,
                                        goal.turn_count,
                                        goal.budget.max_turns,
                                        info.steps.len(),
                                        goal.updated_at
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Goal runner status error: {}", e);
                            }
                        }
                    }
                }
            }
            return manager.format_current();
        }
        "clear" | "reset" => {
            if has_runner {
                if let Some(session_id) = app.session_manager.current_session_id() {
                    if let Some(ref runner) = app.goal_runner {
                        match runner.clear(session_id) {
                            Ok(true) => return "Goal cleared (durable run cancelled).".to_string(),
                            Ok(false) => {}
                            Err(e) => {
                                return format!("Goal clear error: {}", e);
                            }
                        }
                    }
                }
            }
            manager.clear();
            return "Current Goal\n- cleared".to_string();
        }
        "pause" => {
            if has_runner {
                if let Some(session_id) = app.session_manager.current_session_id() {
                    if let Some(ref runner) = app.goal_runner {
                        match runner.pause(session_id) {
                            Ok(true) => {
                                return "Goal paused. Use /goal resume to continue automatic turns.".to_string();
                            }
                            Ok(false) => {
                                return "No active goal to pause.".to_string();
                            }
                            Err(e) => {
                                return format!("Goal pause error: {}", e);
                            }
                        }
                    }
                }
            }
            return goal_not_implemented("pause", "Pause automatic goal continuation. When the runner is active this will pause automatic turn scheduling.");
        }
        "resume" => {
            if has_runner {
                if let Some(session_id) = app.session_manager.current_session_id() {
                    if let Some(ref runner) = app.goal_runner {
                        match runner.resume(session_id) {
                            Ok(true) => {
                                app.pending_goal_prompt = Some(
                                    "Continue working toward the active goal.".to_string(),
                                );
                                return "Goal resumed. The next turn will continue automatically."
                                    .to_string();
                            }
                            Ok(false) => {
                                return "No active goal to resume. Use /goal <objective> to start one.".to_string();
                            }
                            Err(e) => {
                                return format!("Goal resume error: {}", e);
                            }
                        }
                    }
                }
            }
            return goal_not_implemented("resume", "Resume automatic goal continuation. When paused this will restart turn scheduling if the goal is active.");
        }
        _ => {}
    }

    // /goal log [limit]
    if trimmed == "log" || trimmed.starts_with("log ") {
        let limit = trimmed
            .strip_prefix("log")
            .unwrap_or_default()
            .trim()
            .parse::<usize>()
            .unwrap_or(10);
        if has_runner {
            if let Some(session_id) = app.session_manager.current_session_id() {
                if let Some(ref runner) = app.goal_runner {
                    match runner.status(session_id) {
                        Ok(info) => {
                            if info.steps.is_empty() {
                                return "Goal Log\n- no steps recorded yet".to_string();
                            }
                            let mut lines = vec![format!("Goal Log ({} steps)", info.steps.len())];
                            for step in info.steps.iter().rev().take(limit) {
                                lines.push(format!(
                                    "- turn {} [{}] {:?}: {}",
                                    step.turn_index,
                                    step.closeout_status.as_deref().unwrap_or("?"),
                                    step.decision,
                                    step.summary
                                ));
                            }
                            return lines.join("\n");
                        }
                        Err(e) => {
                            return format!("Goal log error: {}", e);
                        }
                    }
                }
            }
        }
        return goal_not_implemented(
            "log",
            "Show recent goal steps. Once the durable goal store is implemented, this will display turn-by-turn progress.",
        );
    }

    // /goal set <text> — compatibility alias
    if let Some(title) = trimmed.strip_prefix("set ") {
        return start_goal_with_runner(app, title);
    }

    // /goal edit <text>
    if let Some(text) = trimmed.strip_prefix("edit ") {
        let objective = text.trim();
        if objective.is_empty() {
            return "Usage: /goal edit <text>".to_string();
        }
        return goal_not_implemented(
            "edit",
            "Replace the active goal objective while preserving run history.",
        );
    }

    // /goal <objective> — preferred start command (non-empty, non-subcommand text)
    if !trimmed.is_empty() {
        return start_goal_with_runner(app, trimmed);
    }

    "Usage: /goal [<objective>|set|pause|resume|clear|edit|log|drift]".to_string()
}

fn start_goal_with_runner(app: &mut TuiApp, title: &str) -> String {
    if title.is_empty() {
        return "Goal Error\n- objective must be non-empty".to_string();
    }
    if title.chars().count() > 4000 {
        return format!(
            "Goal Error\n- objective is {} characters, maximum is 4000. Consider putting longer instructions in a file and referencing it from the goal.",
            title.chars().count()
        );
    }

    if let Some(session_id) = app.session_manager.current_session_id() {
        let sid = session_id.to_string();
        if let Some(ref runner) = app.goal_runner {
            match runner.start(&sid, title) {
                Ok(result) => {
                    app.pending_goal_prompt = Some(result.first_prompt.clone());

                    let engine = app.streaming_engine.as_ref().unwrap();
                    let manager = engine.goal_manager();
                    let status = manager
                        .current()
                        .map(|g| g.compact_status())
                        .unwrap_or_else(|| "none".to_string());

                    return format!(
                        "Goal started\n- Id: {}\n- Objective: {}\n- Status: Active\n- Session goal: {}\n\nThe first turn will start automatically.",
                        result.goal_id, title, status
                    );
                }
                Err(e) => {
                    return format!("Goal Error\n- {}", e);
                }
            }
        }
    }

    // Fallback: use SessionGoalManager directly (no runner available)
    let Some(engine) = app.streaming_engine.as_ref() else {
        return "Goal Error\n- no engine available".to_string();
    };
    let manager = engine.goal_manager();
    set_goal_objective(&manager, title)
}

fn goal_not_implemented(subcommand: &str, detail: &str) -> String {
    format!(
        "Goal {}\n- not implemented yet (Phase 1+)\n\n{}",
        subcommand, detail
    )
}

fn set_goal_objective(
    manager: &crate::engine::session_goal::SessionGoalManager,
    title: &str,
) -> String {
    if title.is_empty() {
        return "Goal Error\n- objective must be non-empty".to_string();
    }
    if title.chars().count() > 4000 {
        return format!(
            "Goal Error\n- objective is {} characters, maximum is 4000. Consider putting longer instructions in a file and referencing it from the goal.",
            title.chars().count()
        );
    }
    manager
        .set_manual(title)
        .map(|goal| format!("Current Goal\n- pinned: {}", goal.compact_status()))
        .unwrap_or_else(|| "Usage: /goal <objective>".to_string())
}

pub(crate) fn goal_drift_count_label(trace: &crate::engine::trace::TurnTrace) -> String {
    let mut medium = 0usize;
    let mut high = 0usize;
    for event in &trace.events {
        if let crate::engine::trace::TraceEvent::GoalDriftDetected { level, .. } = event {
            if level.eq_ignore_ascii_case("high") {
                high += 1;
            } else {
                medium += 1;
            }
        }
    }
    match (high, medium) {
        (0, 0) => "none".to_string(),
        (0, medium) => format!("{} advisory", medium),
        (high, 0) => format!("{} high", high),
        (high, medium) => format!("{} high, {} advisory", high, medium),
    }
}

pub(crate) fn format_goal_drift_report(
    trace: &crate::engine::trace::TurnTrace,
    limit: usize,
) -> String {
    let lines = trace
        .events
        .iter()
        .filter_map(|event| match event {
            crate::engine::trace::TraceEvent::GoalDriftDetected {
                goal_id,
                tool,
                call_id,
                level,
                reason,
                suggested_action,
            } => Some(format!(
                "- {} drift via {} {} goal={} reason={} suggested={}",
                level,
                tool,
                call_id.chars().take(8).collect::<String>(),
                goal_id.chars().take(8).collect::<String>(),
                compact_inline(reason, 120),
                suggested_action.as_deref().unwrap_or("none")
            )),
            _ => None,
        })
        .take(limit)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        format!(
            "Goal Drift\n- none in latest trace {}\n\nUse /trace last for the full turn timeline.",
            trace.trace_id.chars().take(8).collect::<String>()
        )
    } else {
        format!(
            "Goal Drift from trace {} ({})\n{}",
            trace.trace_id.chars().take(8).collect::<String>(),
            goal_drift_count_label(trace),
            lines.join("\n")
        )
    }
}

fn compact_inline(text: &str, max_chars: usize) -> String {
    let mut value = text.replace('\n', " ");
    if value.chars().count() > max_chars {
        value = value.chars().take(max_chars).collect::<String>();
        value.push_str("...");
    }
    value
}

/// /learn - Show recent runtime learning events
pub fn handle_learn(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    if matches!(parts.next(), Some("show")) {
        let Some(id) = parts.next().and_then(|value| value.parse::<i64>().ok()) else {
            return "Usage: /learn show <id>".to_string();
        };
        return match app.session_manager.learning_event(id) {
            Ok(Some(event)) => format_learning_event_detail(&event),
            Ok(None) => format!("Learning event #{} not found in current session.", id),
            Err(e) => format!("Learning event unavailable: {}", e),
        };
    }

    let limit = args.trim().parse::<i64>().unwrap_or(8).clamp(1, 50);
    let events = match app.session_manager.recent_learning_events(limit) {
        Ok(events) => events,
        Err(e) => return format!("Learning events unavailable: {}", e),
    };
    if events.is_empty() {
        return "Learning Events\n- none yet".to_string();
    }

    let mut lines = vec![format!("Learning Events ({} recent)", events.len())];
    for event in events {
        lines.push(format!(
            "- #{} {} [{}] conf={:.2}: {}",
            event.id, event.kind, event.source, event.confidence, event.summary
        ));
    }
    lines.join("\n")
}

fn format_learning_event_detail(event: &crate::session_store::LearningEventRecord) -> String {
    let pretty_payload =
        serde_json::to_string_pretty(&event.payload).unwrap_or_else(|_| event.payload.to_string());
    format!(
        "Learning Event #{}\nKind: {}\nSource: {}\nConfidence: {:.2}\nCreated: {}\nSummary: {}\n\nPayload:\n{}",
        event.id,
        event.kind,
        event.source,
        event.confidence,
        event.created_at,
        event.summary,
        pretty_payload
    )
}

/// /experience - Inspect typed ExperienceRecord payloads.
pub fn handle_experience(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("last");
    match action {
        "last" | "" => {
            let events = match app.session_manager.recent_learning_events(30) {
                Ok(events) => events,
                Err(e) => return format!("Experience ledger unavailable: {}", e),
            };
            match events
                .iter()
                .find(|event| event.payload.get("experience").is_some())
            {
                Some(event) => format_experience_event(event),
                None => "Experience Ledger\n- no structured experience records yet".to_string(),
            }
        }
        "list" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(10)
                .clamp(1, 50);
            let events = match app.session_manager.recent_learning_events(limit * 3) {
                Ok(events) => events,
                Err(e) => return format!("Experience ledger unavailable: {}", e),
            };
            let lines = events
                .iter()
                .filter(|event| event.payload.get("experience").is_some())
                .take(limit as usize)
                .map(|event| {
                    let experience = &event.payload["experience"];
                    format!(
                        "- #{} {} workflow={} outcome={} tools={}",
                        event.id,
                        event.kind,
                        experience["workflow"].as_str().unwrap_or("unknown"),
                        experience["final_outcome"].as_str().unwrap_or("unknown"),
                        experience["cost"]["tool_calls"].as_u64().unwrap_or(0)
                    )
                })
                .collect::<Vec<_>>();
            if lines.is_empty() {
                "Experience Ledger\n- no structured experience records yet".to_string()
            } else {
                format!("Experience Ledger\n{}", lines.join("\n"))
            }
        }
        "show" => {
            let Some(id) = parts.next().and_then(|value| value.parse::<i64>().ok()) else {
                return "Usage: /experience show <id>".to_string();
            };
            match app.session_manager.learning_event(id) {
                Ok(Some(event)) if event.payload.get("experience").is_some() => {
                    format_experience_event(&event)
                }
                Ok(Some(_)) => format!(
                    "Learning event #{} has no structured experience payload.",
                    id
                ),
                Ok(None) => format!("Experience event #{} not found in current session.", id),
                Err(e) => format!("Experience event unavailable: {}", e),
            }
        }
        _ => "Usage: /experience [last|list [limit]|show <id>]".to_string(),
    }
}

/// /evolution - Inspect controlled self-evolution audit events.
pub fn handle_evolution(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("audit");
    match action {
        "status" | "panel" => format_evolution_status_panel(app),
        "audit" | "list" | "" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(20)
                .clamp(1, 100);
            let events = match app.session_manager.recent_learning_events(limit * 4) {
                Ok(events) => events,
                Err(e) => return format!("Evolution audit unavailable: {}", e),
            };
            let events = events
                .into_iter()
                .filter(is_evolution_learning_event)
                .take(limit as usize)
                .collect::<Vec<_>>();
            if events.is_empty() {
                return "Evolution Audit\n- no evolution events yet".to_string();
            }
            let mut lines = vec![format!("Evolution Audit ({} recent)", events.len())];
            for event in events {
                lines.push(format!(
                    "- #{} {} [{}] conf={:.2} at {}: {}",
                    event.id,
                    event.kind,
                    event.source,
                    event.confidence,
                    event.created_at,
                    event.summary
                ));
            }
            lines.push("Use /learn show <id> for full payload.".to_string());
            lines.join("\n")
        }
        "json" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(50)
                .clamp(1, 200);
            let events = match app.session_manager.recent_learning_events(limit * 4) {
                Ok(events) => events,
                Err(e) => return format!("Evolution audit unavailable: {}", e),
            };
            let events = events
                .into_iter()
                .filter(is_evolution_learning_event)
                .take(limit as usize)
                .collect::<Vec<_>>();
            serde_json::to_string_pretty(&events).unwrap_or_else(|_| "[]".to_string())
        }
        "show" => {
            let Some(id) = parts.next().and_then(|value| value.parse::<i64>().ok()) else {
                return "Usage: /evolution show <id>".to_string();
            };
            match app.session_manager.learning_event(id) {
                Ok(Some(event)) if is_evolution_learning_event(&event) => {
                    format_learning_event_detail(&event)
                }
                Ok(Some(_)) => format!("Learning event #{} is not an evolution audit event.", id),
                Ok(None) => format!("Evolution event #{} not found in current session.", id),
                Err(e) => format!("Evolution event unavailable: {}", e),
            }
        }
        _ => "Usage: /evolution [status|audit [limit]|json [limit]|show <id>]".to_string(),
    }
}

fn is_evolution_learning_event(event: &crate::session_store::LearningEventRecord) -> bool {
    let kind = event.kind.as_str();
    let source = event.source.as_str();
    kind.contains("improvement")
        || kind.contains("skill_")
        || kind.contains("evolution")
        || source.contains("improvement")
        || source.contains("skill_evolution")
        || source.contains("skill_proposals")
}

fn format_evolution_status_panel(app: &mut TuiApp) -> String {
    let improvement_store = crate::engine::improvement::ImprovementStore::default();
    let improvements = improvement_store.list();
    let active_guidance = improvement_store.applied_guidance_store().active();
    let blocked_missing_evalsets = improvements
        .iter()
        .filter(|proposal| {
            proposal.status == crate::engine::improvement::ProposalStatus::Accepted
                && proposal.evalset_bindings.is_empty()
        })
        .count();
    let failed_eval = improvements
        .iter()
        .filter(|proposal| {
            proposal.eval_status == crate::engine::improvement::ProposalEvalStatus::Failed
        })
        .count();
    let rollback_recommended = improvements
        .iter()
        .filter(|proposal| {
            improvement_store
                .effect_store()
                .summary(&proposal.id)
                .rollback_recommended
        })
        .count();
    let skill_store = crate::engine::skill_evolution::SkillProposalStore::default();
    let skill_proposals = skill_store.list();
    let backups = disabled_skill_backups(&user_skill_root(), None);
    let audit_events = app
        .session_manager
        .recent_learning_events(200)
        .map(|events| {
            events
                .into_iter()
                .filter(is_evolution_learning_event)
                .count()
        })
        .unwrap_or(0);

    let improvement_status =
        count_debug_values(improvements.iter().map(|proposal| proposal.status));
    let improvement_eval =
        count_debug_values(improvements.iter().map(|proposal| proposal.eval_status));
    let skill_status = count_debug_values(skill_proposals.iter().map(|proposal| proposal.status));
    let skill_trust = count_debug_values(skill_proposals.iter().map(|proposal| proposal.trust));
    let memory_write_policy = crate::services::config::runtime_config()
        .auto_memory_write_policy()
        .to_string();

    let mut lines = vec![
        "Evolution Status".to_string(),
        "Flow: proposal -> eval -> accept/apply -> rollback".to_string(),
        format!(
            "Memory: provider lifecycle visible via memory_load doctor_json; auto-write policy={}",
            memory_write_policy
        ),
        format!(
            "Improvements: total={} status={} eval={} active_guidance={} blocked_missing_evalsets={} failed_eval={} rollback_recommended={}",
            improvements.len(),
            format_counts(&improvement_status),
            format_counts(&improvement_eval),
            active_guidance.len(),
            blocked_missing_evalsets,
            failed_eval,
            rollback_recommended
        ),
        format!(
            "Skills: total={} status={} trust={} rollback_backups={}",
            skill_proposals.len(),
            format_counts(&skill_status),
            format_counts(&skill_trust),
            backups.len()
        ),
        format!("Audit events: {}", audit_events),
        "".to_string(),
        "Commands:".to_string(),
        "- /improvements scan | bind-eval <id> <evalset> | eval <id> | accept <id> | apply <id> | active | effect <id> | rollback <id>".to_string(),
        "- /skill-proposals scan | eval <id> | accept <id> | apply <id> | rollback <name> --yes"
            .to_string(),
        "- memory_load {\"action\":\"doctor_json\"}".to_string(),
    ];

    if let Some(proposal) = improvements.first() {
        lines.push("".to_string());
        lines.push(format!(
            "Latest improvement: {}",
            format_improvement_line(proposal)
        ));
    }
    if let Some(proposal) = skill_proposals.first() {
        lines.push(format!(
            "Latest skill proposal: {}",
            format_skill_proposal_line(proposal)
        ));
    }
    lines.join("\n")
}

fn count_debug_values<T: std::fmt::Debug>(values: impl Iterator<Item = T>) -> Vec<(String, usize)> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(format!("{:?}", value)).or_default() += 1;
    }
    counts.into_iter().collect()
}

fn format_counts(counts: &[(String, usize)]) -> String {
    if counts.is_empty() {
        return "none".to_string();
    }
    counts
        .iter()
        .map(|(label, count)| format!("{}={}", label, count))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_experience_event(event: &crate::session_store::LearningEventRecord) -> String {
    let experience = &event.payload["experience"];
    let pretty =
        serde_json::to_string_pretty(experience).unwrap_or_else(|_| experience.to_string());
    format!(
        "Experience #{}\nKind: {}\nSource: {}\nCreated: {}\nWorkflow: {}\nOutcome: {}\nTool calls: {}\n\n{}",
        event.id,
        event.kind,
        event.source,
        event.created_at,
        experience["workflow"].as_str().unwrap_or("unknown"),
        experience["final_outcome"].as_str().unwrap_or("unknown"),
        experience["cost"]["tool_calls"].as_u64().unwrap_or(0),
        pretty
    )
}

/// /skill-proposals - Review generated skill candidates before activation
pub fn handle_skill_proposals(app: &mut TuiApp, args: &str) -> String {
    use crate::engine::skill_evolution::{
        evaluate_skill_proposal, write_active_skill, SkillProposalStatus, SkillProposalStore,
    };

    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");
    let store = SkillProposalStore::default();

    match action {
        "scan" | "propose" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(80)
                .clamp(5, 300);
            let events = match app.session_manager.recent_learning_events(limit) {
                Ok(events) => events,
                Err(e) => return format!("Skill proposal scan failed: {}", e),
            };
            match store.propose_from_learning_events(&events) {
                Ok(proposals) if proposals.is_empty() => {
                    "Skill proposal scan complete: no repeated successful procedures found."
                        .to_string()
                }
                Ok(proposals) => {
                    let mut lines = vec![format!(
                        "Skill proposal scan complete: {} new candidate(s)",
                        proposals.len()
                    )];
                    for proposal in proposals {
                        lines.push(format_skill_proposal_line(&proposal));
                    }
                    lines.join("\n")
                }
                Err(e) => format!("Skill proposal scan failed: {}", e),
            }
        }
        "list" | "" => {
            let proposals = store.list();
            if proposals.is_empty() {
                "Skill Proposals\n- none yet\n\nRun /skill-proposals scan to generate candidates from repeated successful workflows.".to_string()
            } else {
                let mut lines = vec![format!("Skill Proposals ({} total)", proposals.len())];
                for proposal in proposals.iter().take(20) {
                    lines.push(format_skill_proposal_line(proposal));
                }
                lines.join("\n")
            }
        }
        "show" => {
            let Some(id) = parts.next() else {
                return "Usage: /skill-proposals show <id|name>".to_string();
            };
            match store.get(id) {
                Some(proposal) => format_skill_proposal_detail(&proposal),
                None => format!("No skill proposal matching '{}'.", id),
            }
        }
        "eval" => {
            let Some(id) = parts.next() else {
                return "Usage: /skill-proposals eval <id|name>".to_string();
            };
            match store.get(id) {
                Some(proposal) => format_skill_eval(&evaluate_skill_proposal(&proposal)),
                None => format!("No skill proposal matching '{}'.", id),
            }
        }
        "fitness" | "stats" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals fitness <skill-name>".to_string();
            };
            match store.fitness_snapshot(name) {
                Some(snapshot) => format_skill_fitness(&snapshot),
                None => format!("No skill usage events found for '{}'.", name),
            }
        }
        "gate" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals gate <skill-name> [old-fitness]".to_string();
            };
            let old_fitness = parts
                .next()
                .and_then(|value| value.parse::<f32>().ok())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            match store.fitness_snapshot(name) {
                Some(snapshot) => {
                    let gate = crate::engine::skill_evolution::compare_skill_versions_for_promotion(
                        old_fitness,
                        &snapshot,
                        0.0,
                        0.0,
                    );
                    format_skill_promotion_gate(&gate)
                }
                None => format!("No skill usage events found for '{}'.", name),
            }
        }
        "versions" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals versions <skill-name>".to_string();
            };
            let records = store.version_records(name);
            if records.is_empty() {
                format!("No applied versions recorded for '{}'.", name)
            } else {
                let mut lines = vec![format!("Skill Versions /{}", name)];
                for record in records.iter().rev().take(10) {
                    lines.push(format!(
                        "- {} path={} rollback_to={} evalsets={}",
                        record.version,
                        record.applied_path,
                        record.rollback_to.as_deref().unwrap_or("none"),
                        if record.evalset_bindings.is_empty() {
                            "none".to_string()
                        } else {
                            record.evalset_bindings.join(",")
                        }
                    ));
                }
                lines.join("\n")
            }
        }
        "rollback-list" | "disabled" => {
            let filter = parts.next();
            let backups = disabled_skill_backups(&user_skill_root(), filter);
            if backups.is_empty() {
                match filter {
                    Some(name) => format!("No disabled rollback backups found for /{}.", name),
                    None => "No disabled rollback backups found.".to_string(),
                }
            } else {
                let mut lines = vec![format!("Disabled Skill Backups ({} total)", backups.len())];
                for backup in backups.iter().take(20) {
                    lines.push(format!(
                        "- /{} backup={} path={}",
                        backup.skill_name,
                        backup.backup_name,
                        backup.path.display()
                    ));
                }
                lines.push(
                    "Restore with: /skill-proposals restore <skill-name> [backup-name] --yes"
                        .to_string(),
                );
                lines.join("\n")
            }
        }
        "restore" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals restore <skill-name> [backup-name] --yes"
                    .to_string();
            };
            if !is_safe_skill_dir_name(name) {
                return "Invalid skill name. Use only the skill directory name, not a path."
                    .to_string();
            }
            let mut backup_name: Option<&str> = None;
            let mut confirmed = false;
            for part in parts {
                if part == "--yes" {
                    confirmed = true;
                } else {
                    backup_name = Some(part);
                }
            }
            if !confirmed {
                return format!(
                    "Restore reactivates a disabled /{} skill backup.\nUsage: /skill-proposals restore {} [backup-name] --yes",
                    name, name
                );
            }
            if let Some(backup_name) = backup_name {
                if !is_safe_skill_dir_name(backup_name) {
                    return "Invalid backup name. Use the basename shown by /skill-proposals rollback-list."
                        .to_string();
                }
            }
            let root = user_skill_root();
            let active_dir = root.join(name);
            if active_dir.exists() {
                return format!(
                    "Refusing restore: active skill directory already exists: {}",
                    active_dir.display()
                );
            }
            let Some(backup) = resolve_disabled_skill_backup(&root, name, backup_name) else {
                return format!(
                    "No disabled backup found for /{}.\nUse /skill-proposals rollback-list {} to inspect backups.",
                    name, name
                );
            };
            if !backup.path.starts_with(&root) || !active_dir.starts_with(&root) {
                return "Refusing restore outside user skill root.".to_string();
            }
            match std::fs::rename(&backup.path, &active_dir) {
                Ok(()) => {
                    record_evolution_update(
                        crate::engine::evolution_controller::EvolutionTarget::Skill,
                    );
                    let loaded = app.skill_runtime.reload();
                    let payload = serde_json::json!({
                        "skill_name": name,
                        "backup_name": backup.backup_name,
                        "restored_path": active_dir,
                        "source_path": backup.path,
                    });
                    let _ = app.session_manager.add_learning_event(
                        "skill_rollback_restore",
                        "skill_evolution",
                        &format!("Restored disabled skill /{}", name),
                        0.9,
                        &payload,
                    );
                    format!(
                        "Restored /{}\n- from: {}\n- active: {}\n- reloaded skills: {}",
                        name,
                        backup.backup_name,
                        active_dir.display(),
                        loaded
                    )
                }
                Err(e) => format!("Failed to restore /{}: {}", name, e),
            }
        }
        "rollback" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals rollback <skill-name> --yes".to_string();
            };
            if !is_safe_skill_dir_name(name) {
                return "Invalid skill name. Use only the skill directory name, not a path."
                    .to_string();
            }
            let confirmed = parts.any(|part| part == "--yes");
            if !confirmed {
                return format!(
                    "Rollback disables the active /{} skill by moving its directory aside.\nUsage: /skill-proposals rollback {} --yes",
                    name, name
                );
            }
            let records = store.version_records(name);
            let Some(latest) = records.last() else {
                return format!("No applied versions recorded for '{}'.", name);
            };
            let root = user_skill_root();
            let skill_dir = root.join(name);
            if !skill_dir.exists() {
                return format!("Active skill directory does not exist: {}", skill_dir.display());
            }
            if !skill_dir.starts_with(&root) {
                return format!("Refusing rollback outside user skill root: {}", skill_dir.display());
            }
            let disabled_dir = root.join(format!(
                "{}.disabled-{}",
                name,
                chrono::Utc::now().format("%Y%m%d%H%M%S")
            ));
            match std::fs::rename(&skill_dir, &disabled_dir) {
                Ok(()) => {
                    let _ = store.update_status(&latest.proposal_id, SkillProposalStatus::Accepted);
                    record_evolution_update(
                        crate::engine::evolution_controller::EvolutionTarget::Skill,
                    );
                    let loaded = app.skill_runtime.reload();
                    let payload = serde_json::json!({
                        "skill_name": name,
                        "disabled_path": disabled_dir,
                        "previous_path": skill_dir,
                        "version": latest.version,
                        "proposal_id": latest.proposal_id,
                    });
                    let _ = app.session_manager.add_learning_event(
                        "skill_rollback",
                        "skill_evolution",
                        &format!("Rolled back active skill /{}", name),
                        0.9,
                        &payload,
                    );
                    format!(
                        "Rolled back /{}\n- moved: {}\n- disabled: {}\n- proposal returned to Accepted\n- reloaded skills: {}",
                        name,
                        skill_dir.display(),
                        disabled_dir.display(),
                        loaded
                    )
                }
                Err(e) => format!("Failed to rollback /{}: {}", name, e),
            }
        }
        "bind-eval" => {
            let Some(id) = parts.next() else {
                return "Usage: /skill-proposals bind-eval <id|name> <evalset-name>".to_string();
            };
            let Some(evalset) = parts.next() else {
                return "Usage: /skill-proposals bind-eval <id|name> <evalset-name>".to_string();
            };
            match store.bind_evalset(id, evalset) {
                Ok(Some(updated)) => format!(
                    "Bound evalset '{}' to skill proposal {}\n{}",
                    evalset,
                    updated.id,
                    format_skill_proposal_line(&updated)
                ),
                Ok(None) => format!("No skill proposal matching '{}'.", id),
                Err(e) => format!("Failed to bind evalset: {}", e),
            }
        }
        "record" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals record <skill-name> <success|fail> [version]"
                    .to_string();
            };
            let Some(outcome) = parts.next() else {
                return "Usage: /skill-proposals record <skill-name> <success|fail> [version]"
                    .to_string();
            };
            let success = match outcome {
                "success" | "pass" | "passed" => true,
                "fail" | "failed" | "failure" => false,
                _ => return "Outcome must be success or fail.".to_string(),
            };
            let version = parts.next().unwrap_or("manual");
            let event = crate::engine::skill_evolution::SkillUsageEvent {
                skill_name: name.to_string(),
                skill_version: version.to_string(),
                provisional: false,
                success,
                acceptance_passed: Some(success),
                tests_passed: None,
                user_satisfaction: if success { Some(0.80) } else { Some(0.25) },
                duration_ms: None,
                tool_calls: 0,
                risk_penalty: if success { 0.05 } else { 0.25 },
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            match store.record_usage(&event) {
                Ok(()) => {
                    let _ = app.session_manager.add_learning_event(
                        "skill_usage",
                        "skill_proposals",
                        &format!("Skill /{} outcome recorded: {}", name, outcome),
                        0.85,
                        &serde_json::to_value(&event).unwrap_or_else(|_| serde_json::json!({})),
                    );
                    match store.fitness_snapshot(name) {
                        Some(snapshot) => format!(
                            "Recorded skill usage for /{}\n{}",
                            name,
                            format_skill_fitness(&snapshot)
                        ),
                        None => format!("Recorded skill usage for /{}.", name),
                    }
                }
                Err(e) => format!("Failed to record skill usage: {}", e),
            }
        }
        "accept" | "reject" => {
            let Some(id) = parts.next() else {
                return format!("Usage: /skill-proposals {} <id|name>", action);
            };
            let desired = if action == "accept" {
                SkillProposalStatus::Accepted
            } else {
                SkillProposalStatus::Rejected
            };
            match store.update_status(id, desired) {
                Ok(Some(updated)) => {
                    persist_skill_proposal_learning_event(app, &updated, action, None);
                    format!(
                        "Updated skill proposal {}\n{}",
                        updated.id,
                        format_skill_proposal_line(&updated)
                    )
                }
                Ok(None) => format!("No skill proposal matching '{}'.", id),
                Err(e) => format!("Failed to update skill proposal: {}", e),
            }
        }
        "apply" => {
            let Some(id) = parts.next() else {
                return "Usage: /skill-proposals apply <id|name>".to_string();
            };
            let Some(current) = store.get(id) else {
                return format!("No skill proposal matching '{}'.", id);
            };
            if current.status != SkillProposalStatus::Accepted {
                return format!(
                    "Skill proposal {} is {:?}. Accept it before applying; generated skills are not activated automatically.",
                    current.id, current.status
                );
            }
            let eval = evaluate_skill_proposal(&current);
            if !eval.passed {
                return format!(
                    "Skill proposal {} failed eval and was not applied.\n{}",
                    current.id,
                    format_skill_eval(&eval)
                );
            }
            let bound_report = run_bound_skill_evalsets(&current);
            if let Some(ref report) = bound_report {
                if !report.ok {
                    return format!(
                        "Skill proposal {} failed bound evalsets and was not applied.\n{}",
                        current.id, report.summary
                    );
                }
            }
            let gate = skill_evolution_gate(&current);
            if matches!(
                gate.action,
                crate::engine::evolution_controller::EvolutionAction::Reject
                    | crate::engine::evolution_controller::EvolutionAction::Monitor
            ) {
                return format!(
                    "Skill proposal {} was not applied by evolution gate.\n{}",
                    current.id,
                    format_evolution_gate(&gate)
                );
            }
            if let Err(report) = validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref()) {
                return format!(
                    "Skill proposal {} was not applied by promotion gate.\n{}",
                    current.id, report
                );
            }
            let root = user_skill_root();
            match write_active_skill(&current, &root) {
                Ok(path) => match store.record_applied_version(id, &path) {
                    Ok(Some((updated, _version))) => {
                        record_evolution_update(
                            crate::engine::evolution_controller::EvolutionTarget::Skill,
                        );
                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,
                            "apply",
                            Some(path.display().to_string()),
                        );
                        format!(
                            "Applied skill proposal {}\n- wrote: {}\n- trust: {:?}\n- reloaded skills: {}\n\nInvoke with /{} <task>",
                            updated.id,
                            path.display(),
                            updated.trust,
                            loaded,
                            updated.name
                        )
                    }
                    Ok(None) => format!(
                        "Skill file written, but version record update failed for '{}'.",
                        id
                    ),
                    Err(e) => format!("Skill file written, but status update failed: {}", e),
                },
                Err(e) => format!("Failed to apply skill proposal: {}", e),
            }
        }
        _ => "Usage: /skill-proposals [list|scan [limit]|show <id>|eval <id>|fitness <name>|gate <name>|versions <name>|rollback-list [name]|rollback <name> --yes|restore <name> [backup] --yes|bind-eval <id> <evalset>|record <name> <success|fail>|accept <id>|reject <id>|apply <id>]".to_string(),
    }
}

/// /recover - Show recent recovery plans
pub fn handle_recover(app: &mut TuiApp, args: &str) -> String {
    let limit = args.trim().parse::<usize>().unwrap_or(8).clamp(1, 50);
    let trace = if let Some(engine) = app.streaming_engine.as_ref() {
        engine
            .trace_store()
            .latest()
            .or_else(|| app.session_manager.latest_trace().ok().flatten())
    } else {
        app.session_manager.latest_trace().ok().flatten()
    };

    let Some(trace) = trace else {
        return "Recovery Plans\n- none yet".to_string();
    };

    let plans = trace
        .events
        .iter()
        .filter_map(|event| match event {
            crate::engine::trace::TraceEvent::RecoveryPlan {
                plan_id,
                source,
                category,
                failure_type,
                recovery_kind,
                action,
                retryable,
                safe_retry,
                retry_budget,
                side_effect_uncertain,
                requires_user_decision,
                suggested_command,
                status,
                ..
            } => Some(format!(
                "- {} [{}:{}] failure={} recovery_kind={} status={} retryable={} safe_retry={} retry_budget={} side_effect_uncertain={} requires_user={} suggested={} action={}",
                &plan_id[..8.min(plan_id.len())],
                source,
                category,
                if failure_type.is_empty() {
                    "none"
                } else {
                    failure_type.as_str()
                },
                if recovery_kind.is_empty() {
                    "none"
                } else {
                    recovery_kind.as_str()
                },
                status,
                retryable,
                safe_retry,
                retry_budget
                    .as_ref()
                    .map(|budget| budget.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                side_effect_uncertain,
                requires_user_decision,
                suggested_command.as_deref().unwrap_or("none"),
                action
            )),
            _ => None,
        })
        .take(limit)
        .collect::<Vec<_>>();

    if plans.is_empty() {
        format!(
            "Recovery Plans\n- none in latest trace {}\n\nUse /trace last for the full turn timeline.",
            &trace.trace_id[..8.min(trace.trace_id.len())]
        )
    } else {
        format!(
            "Recovery Plans from trace {}\n{}",
            &trace.trace_id[..8.min(trace.trace_id.len())],
            plans.join("\n")
        )
    }
}

fn quick_git_line(cwd: &std::path::Path) -> String {
    let branch = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|branch| !branch.is_empty());

    let changes = std::process::Command::new("git")
        .args(["status", "--short"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count()
        });

    match (branch, changes) {
        (Some(branch), Some(0)) => format!("Git: {} clean", branch),
        (Some(branch), Some(count)) => format!("Git: {} with {} changed files", branch, count),
        (Some(branch), None) => format!("Git: {}", branch),
        _ => "Git: not a repository".to_string(),
    }
}

/// /feedback - Send feedback
pub fn handle_feedback(app: &mut TuiApp, args: &str) -> String {
    let message = args.trim();
    if message.is_empty() {
        return "Usage: /feedback <message>".to_string();
    }
    let session_id = app
        .session_manager
        .current_session_id()
        .unwrap_or("none")
        .to_string();
    match append_feedback(&session_id, message) {
        Ok(path) => format!("Feedback recorded to {}.", path.display()),
        Err(e) => format!("Failed to record feedback: {}", e),
    }
}

#[cfg(test)]
mod tests;
