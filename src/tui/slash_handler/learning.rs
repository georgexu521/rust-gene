//! Learning, evolution and recovery slash command handlers.

use super::utils::*;

use crate::engine::task_contract::{MemoryProposalBatchFilter, MemoryProposalStatus};
use crate::tui::app::TuiApp;

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
    if trimmed.is_empty() || trimmed == "status" || trimmed == "show" {
        return manager.format_current();
    }

    if trimmed == "clear" || trimmed == "reset" {
        manager.clear();
        return "Current Goal\n- cleared".to_string();
    }

    if let Some(title) = trimmed.strip_prefix("set ") {
        return manager
            .set_manual(title)
            .map(|goal| format!("Current Goal\n- pinned: {}", goal.compact_status()))
            .unwrap_or_else(|| "Usage: /goal set <text>".to_string());
    }

    "Usage: /goal [set <text>|clear|drift [limit]]".to_string()
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
    let memory_write_policy = std::env::var("PRIORITY_AGENT_AUTO_MEMORY_WRITE")
        .unwrap_or_else(|_| "review_only".to_string());

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

/// /memory-proposals - Review closeout-generated memory candidates
pub fn handle_memory_proposals(app: &mut TuiApp, args: &str) -> String {
    use crate::engine::task_contract::MemoryProposalReviewStore;

    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");
    let store = MemoryProposalReviewStore::default();

    match action {
        "list" | "" => {
            let remaining = parts.collect::<Vec<_>>();
            let filter = parse_memory_proposal_batch_filter(&remaining);
            let mut records = store.list_records();
            if let Some(source) = filter.source.as_deref() {
                records.retain(|record| record.source == source);
            }
            if let Some(scope) = filter.scope.as_deref() {
                records.retain(|record| {
                    record
                        .proposal
                        .candidates
                        .iter()
                        .any(|candidate| candidate.scope == scope)
                        || record.active_scope.split(',').any(|item| item.trim() == scope)
                });
            }
            if let Some(status) = filter.status {
                records.retain(|record| record.proposal.status == status);
            }
            if records.is_empty() {
                return "Memory Proposals\n- none yet".to_string();
            }
            let mut lines = vec![format!("Memory Proposals ({} total)", records.len())];
            for record in records.iter().take(20) {
                lines.push(format_memory_proposal_record_line(record));
            }
            lines.join("\n")
        }
        "show" => {
            let Some(id) = parts.next() else {
                return "Usage: /memory-proposals show <task-id>".to_string();
            };
            match store.get_record(id) {
                Some(record) => format_memory_proposal_detail(&record),
                None => format!("No memory proposal matching '{}'.", id),
            }
        }
        "conflicts" | "conflict-groups" => {
            let records = store.list_records();
            format_memory_proposal_conflict_panel(&records)
        }
        "resolve-conflict" | "resolve" => {
            let Some(id) = parts.next() else {
                return "Usage: /memory-proposals resolve-conflict <keep-task-id>".to_string();
            };
            match store.resolve_conflict_keep(id) {
                Ok(Some(result)) => format_memory_proposal_conflict_resolution(&result),
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(error) => format!("Failed to resolve memory proposal conflict: {}", error),
            }
        }
        "accept" | "reject" => {
            let Some(id) = parts.next() else {
                return format!("Usage: /memory-proposals {} <task-id>", action);
            };
            let status = if action == "accept" {
                MemoryProposalStatus::Accepted
            } else {
                MemoryProposalStatus::Rejected
            };
            match store.update_status(id, status) {
                Ok(Some(proposal)) => {
                    format!("Updated memory proposal\n{}", format_memory_proposal_line(&proposal))
                }
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(e) => format!("Failed to update memory proposal: {}", e),
            }
        }
        "batch-accept" | "accept-batch" => {
            let remaining = parts.collect::<Vec<_>>();
            let mut filter = parse_memory_proposal_batch_filter(&remaining);
            if filter.status.is_none() {
                filter.status = Some(MemoryProposalStatus::Proposed);
            }
            match store.batch_update_status(
                filter,
                MemoryProposalStatus::Accepted,
                "batch accepted for memory apply",
            ) {
                Ok(result) => format_memory_proposal_batch_result("Batch accepted", &result),
                Err(error) => format!("Failed to batch accept memory proposals: {}", error),
            }
        }
        "batch-reject" | "reject-batch" => {
            let remaining = parts.collect::<Vec<_>>();
            let mut filter = parse_memory_proposal_batch_filter(&remaining);
            let mut reason = "batch rejected by review".to_string();
            if remaining.iter().any(|part| *part == "duplicate" || *part == "--duplicate") {
                filter.duplicate_only = true;
                reason = "batch rejected as duplicate/conflicting".to_string();
            }
            if filter.status.is_none() {
                filter.status = Some(MemoryProposalStatus::Proposed);
            }
            match store.batch_update_status(filter, MemoryProposalStatus::Rejected, reason) {
                Ok(result) => format_memory_proposal_batch_result("Batch rejected", &result),
                Err(error) => format!("Failed to batch reject memory proposals: {}", error),
            }
        }
        "cleanup-stale" => {
            let remaining = parts.collect::<Vec<_>>();
            let mut filter = parse_memory_proposal_batch_filter(&remaining);
            if filter.stale_days.is_none() {
                filter.stale_days = Some(30);
            }
            if filter.status.is_none() {
                filter.status = Some(MemoryProposalStatus::Proposed);
            }
            match store.batch_update_status(
                filter,
                MemoryProposalStatus::Rejected,
                "batch rejected as stale proposal",
            ) {
                Ok(result) => format_memory_proposal_batch_result("Stale cleanup", &result),
                Err(error) => format!("Failed to cleanup stale memory proposals: {}", error),
            }
        }
        "supersede" => {
            let Some(old_id) = parts.next() else {
                return "Usage: /memory-proposals supersede <old-id> <new-id>".to_string();
            };
            let Some(new_id) = parts.next() else {
                return "Usage: /memory-proposals supersede <old-id> <new-id>".to_string();
            };
            match store.supersede(old_id, new_id) {
                Ok(Some(proposal)) => format!(
                    "Superseded memory proposal\n{}",
                    format_memory_proposal_line(&proposal)
                ),
                Ok(None) => format!("No memory proposal matching '{}'.", old_id),
                Err(error) => format!("Failed to supersede memory proposal: {}", error),
            }
        }
        "edit" => {
            let Some(id) = parts.next() else {
                return "Usage: /memory-proposals edit <task-id> <content>".to_string();
            };
            let content = parts.collect::<Vec<_>>().join(" ");
            if content.trim().is_empty() {
                return "Usage: /memory-proposals edit <task-id> <content>".to_string();
            }
            match store.edit_first_candidate(id, content) {
                Ok(Some(proposal)) => {
                    format!("Edited memory proposal\n{}", format_memory_proposal_line(&proposal))
                }
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(e) => format!("Failed to edit memory proposal: {}", e),
            }
        }
        "apply" => {
            let remaining = parts.collect::<Vec<_>>();
            if remaining.iter().any(|part| {
                matches!(
                    *part,
                    "--accepted"
                        | "accepted"
                        | "--scope"
                        | "--source"
                        | "--status"
                        | "--pending"
                        | "--rejected"
                        | "--applied"
                ) || part.starts_with("--scope=")
                    || part.starts_with("--source=")
                    || part.starts_with("--status=")
            }) {
                let mut filter = parse_memory_proposal_batch_filter(&remaining);
                if filter.status.is_none() {
                    filter.status = Some(MemoryProposalStatus::Accepted);
                }
                let mut memory = crate::memory::MemoryManager::new();
                return match store.batch_apply(filter, &mut memory) {
                    Ok(result) => format_memory_proposal_batch_apply_result(&result),
                    Err(e) => format!("Failed to batch apply memory proposals: {}", e),
                };
            }
            let Some(id) = remaining.first().copied() else {
                return "Usage: /memory-proposals apply <task-id>".to_string();
            };
            let mut memory = crate::memory::MemoryManager::new();
            match store.apply(id, &mut memory) {
                Ok(Some((proposal, applied))) => format!(
                    "Applied memory proposal {}\n- candidates applied: {}\n{}",
                    proposal.task_id,
                    applied,
                    format_memory_proposal_line(&proposal)
                ),
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(e) => format!("Failed to apply memory proposal: {}", e),
            }
        }
        "edit-and-apply" | "edit-apply" => {
            let Some(id) = parts.next() else {
                return "Usage: /memory-proposals edit-and-apply <task-id> <content>".to_string();
            };
            let content = parts.collect::<Vec<_>>().join(" ");
            if content.trim().is_empty() {
                return "Usage: /memory-proposals edit-and-apply <task-id> <content>".to_string();
            }
            let mut memory = crate::memory::MemoryManager::new();
            match store.edit_and_apply(id, content, &mut memory) {
                Ok(Some((proposal, applied))) => format!(
                    "Edited and applied memory proposal {}\n- candidates applied: {}\n{}",
                    proposal.task_id,
                    applied,
                    format_memory_proposal_line(&proposal)
                ),
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(e) => format!("Failed to edit and apply memory proposal: {}", e),
            }
        }
        "repair-drift" | "repair-proposals" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(20)
                .clamp(1, 200);
            let created = if let Some(manager) = app
                .streaming_engine
                .as_ref()
                .and_then(|engine| engine.memory_manager_or_init())
            {
                match manager.try_lock() {
                    Ok(memory) => memory.upsert_projection_repair_proposals(limit),
                    Err(_) => {
                        return "Memory manager is busy; retry repair proposal scan later."
                            .to_string();
                    }
                }
            } else {
                crate::memory::MemoryManager::new().upsert_projection_repair_proposals(limit)
            };
            format!(
                "Memory repair proposal scan complete\n- projection drift proposals: {}\n- review: /memory-proposals list --source repair",
                created
            )
        }
        _ => {
            "Usage: /memory-proposals [list [--status proposed|accepted|rejected|applied] [--blocked] [--scope user|project|topic] [--project <id|label>] [--source background|repair]|show <task-id>|conflicts|resolve-conflict <keep-task-id>|accept <task-id>|reject <task-id>|batch-accept [filters]|batch-reject [duplicate] [filters]|cleanup-stale [--days N]|supersede <old> <new>|edit <task-id> <content>|apply <task-id>|apply --accepted [--scope project|user|topic] [--project <id|label>] [--source closeout|background|repair]|edit-and-apply <task-id> <content>|repair-drift [limit]]"
                .to_string()
        }
    }
}

fn parse_memory_proposal_batch_filter(parts: &[&str]) -> MemoryProposalBatchFilter {
    let mut filter = MemoryProposalBatchFilter::default();
    for (idx, part) in parts.iter().enumerate() {
        if let Some(source) = part.strip_prefix("--source=") {
            filter.source = Some(source.to_string());
        } else if *part == "--source" {
            filter.source = parts.get(idx + 1).map(|value| (*value).to_string());
        } else if let Some(scope) = part.strip_prefix("--scope=") {
            filter.scope = Some(scope.to_string());
        } else if *part == "--scope" {
            filter.scope = parts.get(idx + 1).map(|value| (*value).to_string());
        } else if let Some(project) = part.strip_prefix("--project=") {
            filter.project = Some(project.to_string());
        } else if *part == "--project" {
            filter.project = parts.get(idx + 1).map(|value| (*value).to_string());
        } else if let Some(status) = part.strip_prefix("--status=") {
            filter.status = parse_memory_proposal_status(status);
        } else if *part == "--status" {
            filter.status = parts
                .get(idx + 1)
                .and_then(|value| parse_memory_proposal_status(value));
        } else if *part == "--pending" || *part == "pending" {
            filter.status = Some(MemoryProposalStatus::Proposed);
        } else if *part == "--accepted" || *part == "accepted" {
            filter.status = Some(MemoryProposalStatus::Accepted);
        } else if *part == "--rejected" || *part == "rejected" {
            filter.status = Some(MemoryProposalStatus::Rejected);
        } else if *part == "--applied" || *part == "applied" {
            filter.status = Some(MemoryProposalStatus::Applied);
        } else if *part == "--blocked" || *part == "blocked" {
            filter.blocked_only = true;
        } else if let Some(days) = part.strip_prefix("--days=") {
            filter.stale_days = days.parse::<i64>().ok();
        } else if *part == "--days" {
            filter.stale_days = parts
                .get(idx + 1)
                .and_then(|value| value.parse::<i64>().ok());
        } else if *part == "--duplicate" || *part == "duplicate" {
            filter.duplicate_only = true;
        }
    }
    filter
}

fn parse_memory_proposal_status(value: &str) -> Option<MemoryProposalStatus> {
    match value {
        "pending" | "proposed" => Some(MemoryProposalStatus::Proposed),
        "accepted" => Some(MemoryProposalStatus::Accepted),
        "rejected" => Some(MemoryProposalStatus::Rejected),
        "applied" => Some(MemoryProposalStatus::Applied),
        "not_applicable" => Some(MemoryProposalStatus::NotApplicable),
        _ => None,
    }
}

fn format_memory_proposal_batch_result(
    title: &str,
    result: &crate::engine::task_contract::MemoryProposalBatchUpdate,
) -> String {
    let ids = if result.proposal_ids.is_empty() {
        "none".to_string()
    } else {
        result
            .proposal_ids
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "{}\n- matched: {}\n- updated: {}\n- ids: {}",
        title, result.matched, result.updated, ids
    )
}

fn format_memory_proposal_batch_apply_result(
    result: &crate::engine::task_contract::MemoryProposalBatchApply,
) -> String {
    let ids = if result.proposal_ids.is_empty() {
        "none".to_string()
    } else {
        result
            .proposal_ids
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let failures = if result.failures.is_empty() {
        "none".to_string()
    } else {
        result
            .failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ")
    };
    format!(
        "Batch applied memory proposals\n- matched: {}\n- applied: {}\n- candidates applied: {}\n- failed: {}\n- ids: {}\n- failures: {}",
        result.matched,
        result.applied,
        result.applied_candidates,
        result.failed,
        ids,
        failures
    )
}

fn format_memory_proposal_conflict_resolution(
    result: &crate::engine::task_contract::MemoryProposalConflictResolution,
) -> String {
    let rejected = if result.rejected_ids.is_empty() {
        "none".to_string()
    } else {
        result.rejected_ids.join(", ")
    };
    format!(
        "Memory proposal conflict resolved\n- kept: {}\n- accepted kept proposal: {}\n- conflict groups: {}\n- rejected: {}\n- next: /memory-proposals apply {}",
        result.kept_id, result.accepted_keep, result.conflict_groups, rejected, result.kept_id
    )
}

fn format_memory_proposal_conflict_panel(
    records: &[crate::engine::task_contract::MemoryProposalReviewRecord],
) -> String {
    let mut lines = vec!["Memory Proposal Conflicts".to_string()];
    let mut seen = std::collections::HashSet::<String>::new();
    let records_by_id = records
        .iter()
        .map(|record| (record.proposal.task_id.as_str(), record))
        .collect::<std::collections::HashMap<_, _>>();
    for record in records {
        for group in &record.conflict_groups {
            let key = format!(
                "{}:{}:{}:{}:{}",
                group.group_type,
                group.scope,
                group.kind,
                group.key,
                group
                    .matches
                    .iter()
                    .map(|item| format!("{}#{}", item.proposal_id, item.candidate_index))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            if !seen.insert(key) {
                continue;
            }
            let ids = group
                .matches
                .iter()
                .map(|item| {
                    format!(
                        "{}#{}:{}",
                        item.proposal_id,
                        item.candidate_index + 1,
                        item.status.label()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "- {} scope={} kind={} key={} proposals={} hint={}",
                group.group_type, group.scope, group.kind, group.key, ids, group.resolution_hint
            ));
            for matched in group.matches.iter().take(6) {
                let evidence = records_by_id
                    .get(matched.proposal_id.as_str())
                    .and_then(|matched_record| {
                        matched_record
                            .proposal
                            .candidates
                            .get(matched.candidate_index)
                    })
                    .map(|candidate| candidate.evidence.len())
                    .unwrap_or(0);
                lines.push(format!(
                    "  - {}#{} status={} source={} evidence={} value={} content={}",
                    matched.proposal_id,
                    matched.candidate_index + 1,
                    matched.status.label(),
                    matched.source,
                    evidence,
                    compact_inline(&matched.value, 80),
                    compact_inline(&matched.content, 140)
                ));
            }
            lines.push(format!(
                "  next: keep one with /memory-proposals resolve-conflict <task-id>; inspect with /memory-proposals show {}",
                record.proposal.task_id
            ));
        }
    }
    if lines.len() == 1 {
        lines.push("- none".to_string());
    }
    lines.push("Resolve: /memory-proposals resolve-conflict <keep-task-id>".to_string());
    lines.join("\n")
}

fn format_memory_proposal_line(proposal: &crate::engine::task_contract::MemoryProposal) -> String {
    format!(
        "- task={} [{}] source={} candidates={} kinds={} evidence={} wrote={} reason={}",
        proposal.task_id,
        proposal.status.label(),
        proposal.source,
        proposal.candidates.len(),
        if proposal.candidates.is_empty() {
            "none".to_string()
        } else {
            proposal.candidate_kinds().join("+")
        },
        proposal.evidence_items(),
        proposal.write_performed,
        compact_inline(&proposal.reason, 80)
    )
}

fn format_memory_proposal_record_line(
    record: &crate::engine::task_contract::MemoryProposalReviewRecord,
) -> String {
    format!(
        "- id={} task={} [{}] source={} project={} candidates={} kinds={} evidence={} conflicts={} wrote={} reason={}",
        record.id,
        record.proposal.task_id,
        record.proposal.status.label(),
        record.source,
        record.project_id.as_deref().unwrap_or("unknown"),
        record.proposal.candidates.len(),
        if record.proposal.candidates.is_empty() {
            "none".to_string()
        } else {
            record.proposal.candidate_kinds().join("+")
        },
        record.proposal.evidence_items(),
        record.conflict_groups.len(),
        record.proposal.write_performed,
        compact_inline(&record.proposal.reason, 80)
    )
}

fn format_memory_proposal_detail(
    record: &crate::engine::task_contract::MemoryProposalReviewRecord,
) -> String {
    let proposal = &record.proposal;
    let readiness = memory_proposal_review_readiness(record);
    let candidates = if proposal.candidates.is_empty() {
        "- none".to_string()
    } else {
        proposal
            .candidates
            .iter()
            .enumerate()
            .map(|(idx, candidate)| {
                let evidence = if candidate.evidence.is_empty() {
                    "   evidence: none".to_string()
                } else {
                    candidate
                        .evidence
                        .iter()
                        .enumerate()
                        .map(|(evidence_idx, evidence)| {
                            format!(
                                "   evidence {}: {}",
                                evidence_idx + 1,
                                compact_inline(evidence, 180)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                format!(
                    "{}. kind={} scope={} evidence={}\n   {}\n{}",
                    idx + 1,
                    candidate.kind,
                    candidate.scope,
                    candidate.evidence.len(),
                    compact_inline(&candidate.content, 220),
                    evidence
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let gates = if record.gate_report.is_empty() {
        "- none".to_string()
    } else {
        record
            .gate_report
            .iter()
            .map(|gate| {
                let target = gate
                    .candidate_index
                    .map(|idx| format!("candidate {}", idx + 1))
                    .unwrap_or_else(|| "proposal".to_string());
                format!(
                    "- {} [{}]: {} ({})",
                    gate.gate, target, gate.status, gate.reason
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let history = if record.status_history.is_empty() {
        "- none".to_string()
    } else {
        record
            .status_history
            .iter()
            .map(|entry| {
                format!(
                    "- {}: {} ({})",
                    entry.at,
                    entry.status.label(),
                    compact_inline(&entry.reason, 120)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let conflicts = if record.conflict_groups.is_empty() {
        "- none".to_string()
    } else {
        record
            .conflict_groups
            .iter()
            .map(|group| {
                let matches = group
                    .matches
                    .iter()
                    .map(|item| {
                        format!(
                            "  - {}#{} [{} source={}] value={} content={}",
                            item.proposal_id,
                            item.candidate_index + 1,
                            item.status.label(),
                            item.source,
                            compact_inline(&item.value, 120),
                            compact_inline(&item.content, 160)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "- {} key={} scope={} kind={} hint={}\n{}",
                    group.group_type,
                    group.key,
                    group.scope,
                    group.kind,
                    group.resolution_hint,
                    matches
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "Memory Proposal {}\nID: {}\nStatus: {}\nReview state: {}\nAffects future sessions: {}\nWhy this was suggested: {}\nCreated: {}\nUpdated: {}\nSource session: {}\nSource task: {}\nSource: {}\nActive scope: {}\nProject: {}\nProject labels: {}\nWrite policy: {}\nWrite performed: {}\nReason: {}\nDuplicate/conflict: {}\n\nConflict groups:\n{}\n\nGate report:\n{}\n\nStatus history:\n{}\n\nCandidates:\n{}",
        proposal.task_id,
        record.id,
        proposal.status.label(),
        readiness,
        if proposal.status == crate::engine::task_contract::MemoryProposalStatus::Applied {
            "yes"
        } else {
            "after accept/apply only"
        },
        compact_inline(&proposal.reason, 180),
        record.created_at,
        record.updated_at,
        record.source_session.as_deref().unwrap_or("unknown"),
        record.source_task,
        record.source,
        record.active_scope,
        record.project_id.as_deref().unwrap_or("unknown"),
        if record.project_labels.is_empty() {
            "none".to_string()
        } else {
            record.project_labels.join(", ")
        },
        proposal.write_policy,
        proposal.write_performed,
        proposal.reason,
        record.duplicate_conflict_summary,
        conflicts,
        gates,
        history,
        candidates
    )
}

fn memory_proposal_review_readiness(
    record: &crate::engine::task_contract::MemoryProposalReviewRecord,
) -> String {
    use crate::engine::task_contract::MemoryProposalStatus;

    match record.proposal.status {
        MemoryProposalStatus::Applied => "already applied".to_string(),
        MemoryProposalStatus::Rejected => "rejected; preserved for audit".to_string(),
        MemoryProposalStatus::NotApplicable => "not applicable".to_string(),
        MemoryProposalStatus::Accepted => {
            if let Some(blocking) = record
                .gate_report
                .iter()
                .find(|gate| gate.status == "blocked" || gate.status == "missing")
            {
                return format!("blocked by {}: {}", blocking.gate, blocking.reason);
            }
            if let Some(review) = record
                .gate_report
                .iter()
                .find(|gate| gate.status == "review_required")
            {
                return format!(
                    "accepted, review needed for {}: {}",
                    review.gate, review.reason
                );
            }
            "accepted; ready to apply".to_string()
        }
        MemoryProposalStatus::Proposed => {
            if let Some(blocking) = record
                .gate_report
                .iter()
                .find(|gate| gate.status == "blocked" || gate.status == "missing")
            {
                return format!("not ready; {} says {}", blocking.gate, blocking.reason);
            }
            "pending user review; accept before apply".to_string()
        }
    }
}

/// /improvements - Controlled self-evolution proposals
pub fn handle_improvements(app: &mut TuiApp, args: &str) -> String {
    use crate::engine::improvement::{
        ImprovementEffectOutcome, ImprovementStore, ProposalEvalStatus, ProposalStatus,
    };

    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");
    let store = ImprovementStore::default();

    match action {
        "scan" | "propose" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(50)
                .clamp(5, 200);
            let events = match app.session_manager.recent_learning_events(limit) {
                Ok(events) => events,
                Err(e) => return format!("Improvement scan failed: {}", e),
            };
            match store.propose_from_learning_events(&events) {
                Ok(proposals) if proposals.is_empty() => {
                    "Improvement scan complete: no new proposals.".to_string()
                }
                Ok(proposals) => {
                    let mut lines = vec![format!(
                        "Improvement scan complete: {} new proposal(s)",
                        proposals.len()
                    )];
                    for proposal in proposals {
                        lines.push(format_improvement_line(&proposal));
                    }
                    lines.join("\n")
                }
                Err(e) => format!("Improvement scan failed: {}", e),
            }
        }
        "list" | "" => {
            let proposals = store.list();
            if proposals.is_empty() {
                "Improvements\n- none yet\n\nRun /improvements scan to generate proposals from recent learning events.".to_string()
            } else {
                let mut lines = vec![format!("Improvements ({} total)", proposals.len())];
                for proposal in proposals.iter().take(20) {
                    lines.push(format_improvement_line(proposal));
                }
                lines.join("\n")
            }
        }
        "active" => format_applied_guidance_list(&store.applied_guidance_store().active()),
        "doctor" => format_improvement_doctor(&store),
        "show" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements show <id>".to_string();
            };
            match store.get(id) {
                Some(proposal) => format_improvement_detail_with_state(&proposal, &store),
                None => format!("No improvement proposal matching '{}'.", id),
            }
        }
        "effect" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements effect <id>".to_string();
            };
            let Some(proposal) = store.get(id) else {
                return format!("No improvement proposal matching '{}'.", id);
            };
            format_improvement_effect_summary(&store.effect_store().summary(&proposal.id))
        }
        "record-effect" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements record-effect <id> <positive|neutral|negative> <evalset> [reason]".to_string();
            };
            let Some(outcome) = parts.next().and_then(parse_improvement_effect_outcome) else {
                return "Usage: /improvements record-effect <id> <positive|neutral|negative> <evalset> [reason]".to_string();
            };
            let Some(evalset) = parts.next() else {
                return "Usage: /improvements record-effect <id> <positive|neutral|negative> <evalset> [reason]".to_string();
            };
            let Some(proposal) = store.get(id) else {
                return format!("No improvement proposal matching '{}'.", id);
            };
            let reason = parts.collect::<Vec<_>>().join(" ");
            let reason = if reason.trim().is_empty() {
                "manual effect record".to_string()
            } else {
                reason
            };
            match store.effect_store().record(
                proposal.id.clone(),
                evalset,
                format!("manual-{}", chrono::Utc::now().timestamp()),
                outcome,
                if outcome == ImprovementEffectOutcome::Negative {
                    "framework"
                } else {
                    "none"
                },
                reason,
            ) {
                Ok(record) => format!(
                    "Recorded improvement effect {}\n{}",
                    record.id,
                    format_improvement_effect_summary(&store.effect_store().summary(&proposal.id))
                ),
                Err(e) => format!("Failed to record improvement effect: {}", e),
            }
        }
        "deactivate" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements deactivate <id>".to_string();
            };
            match store.applied_guidance_store().deactivate(id) {
                Ok(Some(record)) => format!(
                    "Deactivated applied guidance {}\nproposal={} status={:?}",
                    record.id, record.proposal_id, record.status
                ),
                Ok(None) => format!("No applied guidance matching '{}'.", id),
                Err(e) => format!("Failed to deactivate applied guidance: {}", e),
            }
        }
        "bind-eval" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements bind-eval <id> <evalset-name>".to_string();
            };
            let Some(evalset) = parts.next() else {
                return "Usage: /improvements bind-eval <id> <evalset-name>".to_string();
            };
            match store.bind_evalset(id, evalset) {
                Ok(Some(updated)) => format!(
                    "Bound evalset '{}' to improvement proposal {}\n{}",
                    evalset,
                    updated.id,
                    format_improvement_line(&updated)
                ),
                Ok(None) => format!("No improvement proposal matching '{}'.", id),
                Err(e) => format!("Failed to bind evalset: {}", e),
            }
        }
        "eval" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements eval <id>".to_string();
            };
            let Some(current) = store.get(id) else {
                return format!("No improvement proposal matching '{}'.", id);
            };
            let eval = evaluate_improvement_proposal_for_apply(&current);
            match store.record_eval(
                id,
                if eval.passed {
                    ProposalEvalStatus::Passed
                } else {
                    ProposalEvalStatus::Failed
                },
                eval.summary.clone(),
            ) {
                Ok(Some(updated)) => {
                    persist_improvement_learning_event(app, &updated, "eval");
                    format!("{}\n\n{}", eval.summary, format_improvement_line(&updated))
                }
                Ok(None) => format!("No improvement proposal matching '{}'.", id),
                Err(e) => format!("Failed to record improvement eval: {}", e),
            }
        }
        "accept" | "reject" | "apply" | "rollback" => {
            let Some(id) = parts.next() else {
                return format!("Usage: /improvements {} <id>", action);
            };
            let desired = match action {
                "accept" => ProposalStatus::Accepted,
                "reject" => ProposalStatus::Rejected,
                "apply" => ProposalStatus::Applied,
                "rollback" => ProposalStatus::RolledBack,
                _ => unreachable!(),
            };
            let Some(current) = store.get(id) else {
                return format!("No improvement proposal matching '{}'.", id);
            };
            if desired == ProposalStatus::RolledBack && current.status != ProposalStatus::Applied {
                return format!(
                    "Proposal {} is {:?}. Only applied proposals can be rolled back.",
                    current.id, current.status
                );
            }
            if desired == ProposalStatus::Applied && current.status != ProposalStatus::Accepted {
                return format!(
                    "Proposal {} is {:?}. Accept it before applying. High-risk and behavior-changing proposals require explicit approval.",
                    current.id, current.status
                );
            }
            if desired == ProposalStatus::Applied && current.evalset_bindings.is_empty() {
                return format!(
                    "Proposal {} has no bound evalset. Run /improvements bind-eval {} <evalset> before eval/apply.",
                    current.id, current.id
                );
            }
            if desired == ProposalStatus::Applied
                && current.eval_status != ProposalEvalStatus::Passed
            {
                return format!(
                    "Proposal {} has eval={:?}. Run /improvements eval {} before applying.",
                    current.id, current.eval_status, current.id
                );
            }
            if desired == ProposalStatus::Applied {
                let gate = improvement_evolution_gate(&current);
                if matches!(
                    gate.action,
                    crate::engine::evolution_controller::EvolutionAction::Reject
                        | crate::engine::evolution_controller::EvolutionAction::Monitor
                ) {
                    return format!(
                        "Proposal {} was not applied by evolution gate.\n{}",
                        current.id,
                        format_evolution_gate(&gate)
                    );
                }
            }
            match store.update_status(id, desired) {
                Ok(Some(updated)) => {
                    if desired == ProposalStatus::Applied {
                        record_evolution_update(improvement_target(&updated));
                    }
                    if desired == ProposalStatus::RolledBack {
                        record_evolution_update(improvement_target(&updated));
                    }
                    persist_improvement_learning_event(app, &updated, action);
                    format!(
                        "Updated proposal {}\n{}",
                        updated.id,
                        format_improvement_line(&updated)
                    )
                }
                Ok(None) => format!("No improvement proposal matching '{}'.", id),
                Err(e) => format!("Failed to update proposal: {}", e),
            }
        }
        _ => {
            "Usage: /improvements [list|scan [limit]|active|doctor|show <id>|bind-eval <id> <evalset>|eval <id>|accept <id>|reject <id>|apply <id>|rollback <id>|effect <id>|record-effect <id> <positive|neutral|negative> <evalset> [reason]|deactivate <id>]"
                .to_string()
        }
    }
}

fn format_improvement_line(proposal: &crate::engine::improvement::ImprovementProposal) -> String {
    format!(
        "- {} [{:?}/{:?}/{:?}] eval={:?} evalsets={} stage={} events={}: {}",
        proposal.id,
        proposal.status,
        proposal.target,
        proposal.risk,
        proposal.eval_status,
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(",")
        },
        proposal.lifecycle_stage(),
        proposal.trigger_event_ids.len(),
        proposal.proposed_change
    )
}

fn format_improvement_detail(proposal: &crate::engine::improvement::ImprovementProposal) -> String {
    format!(
        "Improvement Proposal {}\n\nStatus: {:?}\nStage: {}\nTarget: {:?}\nRisk: {:?}\nEval: {:?}\nEvalSets: {}\nEval summary: {}\nApplied ref: {}\nRollback ref: {}\nEvents: {:?}\n\nProposed change:\n{}\n\nExpected benefit:\n{}\n\nValidation plan:\n{}\n\nRollback plan:\n{}\n\nEvidence:\n{}",
        proposal.id,
        proposal.status,
        proposal.lifecycle_stage(),
        proposal.target,
        proposal.risk,
        proposal.eval_status,
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(", ")
        },
        proposal.eval_summary.as_deref().unwrap_or("none"),
        proposal.applied_ref.as_deref().unwrap_or("none"),
        proposal.rollback_ref.as_deref().unwrap_or("none"),
        proposal.trigger_event_ids,
        proposal.proposed_change,
        proposal.expected_benefit,
        proposal
            .validation
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal.rollback_plan,
        proposal
            .evidence
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn format_improvement_detail_with_state(
    proposal: &crate::engine::improvement::ImprovementProposal,
    store: &crate::engine::improvement::ImprovementStore,
) -> String {
    let mut detail = format_improvement_detail(proposal);
    let guidance = store.applied_guidance_store().get(&proposal.id);
    let effect = store.effect_store().summary(&proposal.id);
    detail.push_str("\n\nApplied guidance:\n");
    match guidance {
        Some(record) => detail.push_str(&format!(
            "- {} status={:?} activation={:?} scope={}:{} rollback={}",
            record.id,
            record.status,
            record.activation,
            record.scope.kind,
            record.scope.label,
            record.rollback_ref.as_deref().unwrap_or("none")
        )),
        None => detail.push_str("- none"),
    }
    detail.push_str("\n\nEffect summary:\n");
    detail.push_str(&format_improvement_effect_summary(&effect));
    detail
}

fn format_applied_guidance_list(
    records: &[crate::engine::improvement::AppliedGuidanceRecord],
) -> String {
    if records.is_empty() {
        return "Active Applied Guidance\n- none".to_string();
    }
    let mut lines = vec![format!("Active Applied Guidance ({} total)", records.len())];
    for record in records.iter().take(20) {
        lines.push(format!(
            "- {} proposal={} target={:?} activation={:?} scope={}:{} evalsets={} updated={}",
            record.id,
            record.proposal_id,
            record.target,
            record.activation,
            record.scope.kind,
            record.scope.label,
            if record.evalsets.is_empty() {
                "none".to_string()
            } else {
                record.evalsets.join(",")
            },
            record.updated_at
        ));
        lines.push(format!(
            "  {}",
            record
                .content
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(220)
                .collect::<String>()
        ));
    }
    lines.join("\n")
}

fn format_improvement_effect_summary(
    summary: &crate::engine::improvement::ImprovementEffectSummary,
) -> String {
    let mut lines = vec![format!(
        "Improvement Effect {}\n- total={} positive={} neutral={} negative={} rollback_recommended={}",
        summary.proposal_id,
        summary.total,
        summary.positive,
        summary.neutral,
        summary.negative,
        summary.rollback_recommended
    )];
    for record in &summary.recent {
        lines.push(format!(
            "- {} {:?} evalset={} run={} owner={} reason={}",
            record.created_at,
            record.outcome,
            record.evalset,
            record.run_id,
            record.failure_owner,
            record.reason
        ));
    }
    lines.join("\n")
}

fn parse_improvement_effect_outcome(
    value: &str,
) -> Option<crate::engine::improvement::ImprovementEffectOutcome> {
    match value.to_ascii_lowercase().as_str() {
        "positive" | "pass" | "passed" | "improved" => {
            Some(crate::engine::improvement::ImprovementEffectOutcome::Positive)
        }
        "neutral" | "same" => Some(crate::engine::improvement::ImprovementEffectOutcome::Neutral),
        "negative" | "fail" | "failed" | "regressed" => {
            Some(crate::engine::improvement::ImprovementEffectOutcome::Negative)
        }
        _ => None,
    }
}

fn format_improvement_doctor(store: &crate::engine::improvement::ImprovementStore) -> String {
    let proposals = store.list();
    let active = store.applied_guidance_store().active();
    let missing_evalsets = proposals
        .iter()
        .filter(|proposal| {
            proposal.status == crate::engine::improvement::ProposalStatus::Accepted
                && proposal.evalset_bindings.is_empty()
        })
        .count();
    let failed_eval = proposals
        .iter()
        .filter(|proposal| {
            proposal.eval_status == crate::engine::improvement::ProposalEvalStatus::Failed
        })
        .count();
    let rollback_recommended = proposals
        .iter()
        .filter(|proposal| {
            store
                .effect_store()
                .summary(&proposal.id)
                .rollback_recommended
        })
        .count();
    let last_eval = proposals
        .iter()
        .filter(|proposal| proposal.eval_summary.is_some())
        .max_by(|left, right| left.updated_at.cmp(&right.updated_at));
    format!(
        "Improvement Doctor\n- proposals={}\n- active_guidance={}\n- blocked_missing_evalsets={}\n- failed_eval={}\n- rollback_recommended={}\n- last_eval={}",
        proposals.len(),
        active.len(),
        missing_evalsets,
        failed_eval,
        rollback_recommended,
        last_eval
            .map(|proposal| format!("{} {:?}", proposal.id, proposal.eval_status))
            .unwrap_or_else(|| "none".to_string())
    )
}

struct ImprovementEvalSummary {
    passed: bool,
    summary: String,
}

fn evaluate_improvement_proposal_for_apply(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> ImprovementEvalSummary {
    let has_validation = !proposal.validation.is_empty();
    let has_evidence = !proposal.evidence.is_empty();
    let gate = improvement_evolution_gate(proposal);
    let gate_allows = !matches!(
        gate.action,
        crate::engine::evolution_controller::EvolutionAction::Reject
            | crate::engine::evolution_controller::EvolutionAction::Monitor
    );
    let bound_report = run_bound_improvement_evalsets(proposal);
    let has_bound_evalset = !proposal.evalset_bindings.is_empty();
    let bound_ok = bound_report
        .as_ref()
        .map(|report| report.ok)
        .unwrap_or(false);
    let passed = has_validation && has_evidence && gate_allows && bound_ok;
    let mut lines = vec![format!(
        "Improvement Eval {}: {}",
        proposal.id,
        if passed { "passed" } else { "failed" }
    )];
    lines.push(format!(
        "- validation_plan={} evidence={} gate={:?} score={:.2}",
        proposal.validation.len(),
        proposal.evidence.len(),
        gate.action,
        gate.score
    ));
    if !has_validation {
        lines.push("- missing validation plan".to_string());
    }
    if !has_evidence {
        lines.push("- missing evidence".to_string());
    }
    if !gate_allows {
        lines.push("- evolution gate did not allow apply".to_string());
    }
    if !has_bound_evalset {
        lines.push("- missing bound evalset; bind at least one evalset before apply".to_string());
        lines.push("- failure_owner=framework".to_string());
    }
    if let Some(report) = bound_report {
        lines.push(format!(
            "- bound_evalsets: {}/{} passed, failed={}, run_id={}, failure_owner={}",
            report.passed, report.total, report.failed, report.run_id, report.failure_owner
        ));
        if !report.ok {
            lines.push(report.summary);
        }
    }
    for reason in gate.reasons.iter().take(3) {
        lines.push(format!("- gate: {}", reason));
    }
    ImprovementEvalSummary {
        passed,
        summary: lines.join("\n"),
    }
}

fn run_bound_improvement_evalsets(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> Option<BoundSkillEvalReport> {
    if proposal.evalset_bindings.is_empty() {
        return None;
    }
    let eval_dir = std::env::current_dir().ok()?.join("evalsets");
    let mut summaries = Vec::new();
    let mut ok = true;
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failure_owner = "none".to_string();
    let run_id = format!("eval-{}", chrono::Utc::now().timestamp());
    for binding in &proposal.evalset_bindings {
        match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(binding)) {
            Ok(reports) if reports.is_empty() => {
                ok = false;
                failure_owner = "test_harness".to_string();
                summaries.push(format!("- {}: no matching evalset", binding));
            }
            Ok(reports) => {
                let binding_ok = reports.iter().all(|report| report.ok());
                ok &= binding_ok;
                for report in &reports {
                    total += report.total;
                    passed += report.passed;
                    failed += report.failed;
                }
                if !binding_ok {
                    failure_owner = "framework".to_string();
                }
                summaries.push(crate::engine::evalset::format_reports(&reports));
            }
            Err(e) => {
                ok = false;
                failure_owner = "test_harness".to_string();
                summaries.push(format!("- {}: {}", binding, e));
            }
        }
    }
    Some(BoundSkillEvalReport {
        ok,
        summary: summaries.join("\n\n"),
        total,
        passed,
        failed,
        run_id,
        failure_owner,
    })
}

fn persist_improvement_learning_event(
    app: &mut TuiApp,
    proposal: &crate::engine::improvement::ImprovementProposal,
    action: &str,
) {
    let mut payload = serde_json::to_value(proposal).unwrap_or_else(|_| serde_json::json!({}));
    if action == "apply" {
        payload["evolution_gate"] =
            serde_json::to_value(improvement_evolution_gate(proposal)).unwrap_or_default();
    }
    let _ = app.session_manager.add_learning_event(
        "improvement_proposal",
        "improvements",
        &format!("Improvement proposal {} {}", proposal.id, action),
        0.9,
        &payload,
    );
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

fn user_skill_root() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
        .join("skills")
}

#[derive(Debug, Clone)]
struct DisabledSkillBackup {
    skill_name: String,
    backup_name: String,
    path: std::path::PathBuf,
}

fn is_safe_skill_dir_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn disabled_skill_backups(
    root: &std::path::Path,
    filter: Option<&str>,
) -> Vec<DisabledSkillBackup> {
    let mut backups = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return backups;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(backup_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((skill_name, _suffix)) = backup_name.split_once(".disabled-") else {
            continue;
        };
        if filter.is_some_and(|needle| needle != skill_name) {
            continue;
        }
        backups.push(DisabledSkillBackup {
            skill_name: skill_name.to_string(),
            backup_name: backup_name.to_string(),
            path,
        });
    }
    backups.sort_by(|a, b| b.backup_name.cmp(&a.backup_name));
    backups
}

fn resolve_disabled_skill_backup(
    root: &std::path::Path,
    skill_name: &str,
    backup_name: Option<&str>,
) -> Option<DisabledSkillBackup> {
    let backups = disabled_skill_backups(root, Some(skill_name));
    match backup_name {
        Some(name) => backups
            .into_iter()
            .find(|backup| backup.backup_name == name),
        None => backups.into_iter().next(),
    }
}

fn format_skill_proposal_line(proposal: &crate::engine::skill_evolution::SkillProposal) -> String {
    format!(
        "- {} /{} v{} [{:?}/{:?}] score={:.2} events={} evalsets={}: {}",
        proposal.id,
        proposal.name,
        proposal.skill_version(),
        proposal.status,
        proposal.trust,
        proposal.creation_score,
        proposal.trigger_event_ids.len(),
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(",")
        },
        proposal.procedure
    )
}

fn format_skill_proposal_detail(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> String {
    format!(
        "Skill Proposal {}\n\nName: /{}\nVersion: {}\nStatus: {:?}\nTrust: {:?}\nScope: {}\nCreation score: {:.2}\nEvidence count: {}\nScope confidence: {:.2}\nEvalSets: {}\nRollback to: {}\nApplied path: {}\nEvents: {:?}\n\nProcedure:\n{}\n\nTriggers:\n{}\n\nWorkflow:\n{}\n\nValidation:\n{}\n\nTools:\n{}\n\nEvidence:\n{}",
        proposal.id,
        proposal.name,
        proposal.skill_version(),
        proposal.status,
        proposal.trust,
        proposal.scope,
        proposal.creation_score,
        proposal.evidence_count,
        proposal.scope_confidence,
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(", ")
        },
        proposal.rollback_to.as_deref().unwrap_or("none"),
        proposal.applied_path.as_deref().unwrap_or("none"),
        proposal.trigger_event_ids,
        proposal.procedure,
        proposal
            .trigger_conditions
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal
            .workflow_steps
            .iter()
            .enumerate()
            .map(|(idx, item)| format!("{}. {}", idx + 1, item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal
            .validation
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal.allowed_tools.join(", "),
        proposal
            .evidence
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

struct BoundSkillEvalReport {
    ok: bool,
    summary: String,
    total: usize,
    passed: usize,
    failed: usize,
    run_id: String,
    failure_owner: String,
}

fn run_bound_skill_evalsets(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> Option<BoundSkillEvalReport> {
    if proposal.evalset_bindings.is_empty() {
        return None;
    }
    let eval_dir = std::env::current_dir().ok()?.join("evalsets");
    let mut summaries = Vec::new();
    let mut ok = true;
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failure_owner = "none".to_string();
    let run_id = format!("eval-{}", chrono::Utc::now().timestamp());
    for binding in &proposal.evalset_bindings {
        match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(binding)) {
            Ok(reports) if reports.is_empty() => {
                ok = false;
                failure_owner = "test_harness".to_string();
                summaries.push(format!("- {}: no matching evalset", binding));
            }
            Ok(reports) => {
                let binding_ok = reports.iter().all(|report| report.ok());
                ok &= binding_ok;
                for report in &reports {
                    total += report.total;
                    passed += report.passed;
                    failed += report.failed;
                }
                if !binding_ok {
                    failure_owner = "framework".to_string();
                }
                summaries.push(crate::engine::evalset::format_reports(&reports));
            }
            Err(e) => {
                ok = false;
                failure_owner = "test_harness".to_string();
                summaries.push(format!("- {}: {}", binding, e));
            }
        }
    }
    Some(BoundSkillEvalReport {
        ok,
        summary: summaries.join("\n\n"),
        total,
        passed,
        failed,
        run_id,
        failure_owner,
    })
}

fn validate_skill_promotion_for_apply(
    store: &crate::engine::skill_evolution::SkillProposalStore,
    proposal: &crate::engine::skill_evolution::SkillProposal,
    bound_report: Option<&BoundSkillEvalReport>,
) -> Result<Option<crate::engine::skill_evolution::SkillPromotionGate>, String> {
    let records = store.version_records(&proposal.name);
    let active_exists = user_skill_root().join(&proposal.name).exists();
    if records.is_empty() && !active_exists {
        return Ok(None);
    }

    let latest = records.last().ok_or_else(|| {
        format!(
            "Active /{} exists but no version baseline is recorded; rollback or record a baseline before replacing it.",
            proposal.name
        )
    })?;
    let candidate_version = proposal.skill_version();
    if latest.version == candidate_version {
        return Err(format!(
            "Candidate version '{}' matches the active /{} version; cannot compare promotion fitness. Regenerate the proposal or record candidate usage under a distinct version.",
            candidate_version, proposal.name
        ));
    }

    let all_events = store.usage_events(&proposal.name);
    let old_events = all_events
        .iter()
        .filter(|event| event.skill_version == latest.version && !event.provisional)
        .cloned()
        .collect::<Vec<_>>();
    let old_snapshot =
        crate::engine::skill_evolution::skill_fitness_snapshot(&proposal.name, &old_events)
            .ok_or_else(|| {
                format!(
                    "Existing /{} version '{}' has no confirmed fitness baseline. Record usage before replacing it.",
                    proposal.name, latest.version
                )
            })?;

    let new_events = all_events
        .iter()
        .filter(|event| event.skill_version == candidate_version && !event.provisional)
        .cloned()
        .collect::<Vec<_>>();
    let new_snapshot =
        crate::engine::skill_evolution::skill_fitness_snapshot(&proposal.name, &new_events)
            .or_else(|| bound_report.and_then(|report| skill_fitness_from_bound_eval(proposal, report)))
            .ok_or_else(|| {
                format!(
                    "Candidate /{} version '{}' has no promotion evidence. Record at least 3 candidate outcomes or bind passing evalsets before replacing an active skill.",
                    proposal.name, candidate_version
                )
            })?;
    let regression_rate = if new_snapshot.events == 0 {
        1.0
    } else {
        new_snapshot.stats.failure_rate
    };
    let semantic_drift = estimate_skill_semantic_drift(proposal);
    let gate = crate::engine::skill_evolution::compare_skill_versions_for_promotion(
        old_snapshot.fitness,
        &new_snapshot,
        regression_rate,
        semantic_drift,
    );
    if gate.passed {
        Ok(Some(gate))
    } else {
        Err(format_skill_promotion_gate(&gate))
    }
}

fn skill_fitness_from_bound_eval(
    proposal: &crate::engine::skill_evolution::SkillProposal,
    report: &BoundSkillEvalReport,
) -> Option<crate::engine::skill_evolution::SkillFitnessSnapshot> {
    if report.total == 0 {
        return None;
    }
    let pass_rate = report.passed as f32 / report.total as f32;
    let failure_rate = report.failed as f32 / report.total as f32;
    let stats = crate::engine::skill_evolution::SkillFitnessStats {
        task_success: pass_rate,
        acceptance_pass_rate: pass_rate,
        test_pass_rate: pass_rate,
        user_satisfaction: if report.ok { 0.75 } else { 0.35 },
        reuse_rate: (proposal.evidence_count as f32 / 10.0).clamp(0.0, 1.0),
        time_saved: 0.55,
        tool_efficiency: 0.55,
        failure_rate,
        cost: 0.20,
        risk_penalty: if report.ok { 0.05 } else { 0.30 },
    };
    Some(crate::engine::skill_evolution::SkillFitnessSnapshot {
        skill_name: proposal.name.clone(),
        skill_version: proposal.skill_version(),
        events: report.total,
        fitness: crate::engine::skill_evolution::compute_skill_fitness(stats),
        stats,
    })
}

fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
    let step_count = proposal.workflow_steps.len() as f32;
    let validation_count = proposal.validation.len() as f32;
    let evidence_count = proposal.evidence_count.max(1) as f32;
    let shape_risk =
        ((step_count - validation_count).abs() / (step_count.max(1.0) + 2.0)).clamp(0.0, 0.25);
    let specificity_risk = proposal.creation_factors.over_specificity * 0.50;
    let evidence_risk = (1.0 / evidence_count).min(0.25);
    (shape_risk + specificity_risk + evidence_risk).clamp(0.0, 1.0)
}

fn format_skill_eval(eval: &crate::engine::skill_evolution::SkillEvalResult) -> String {
    let mut lines = vec![format!(
        "Skill Eval {}\nResult: {}",
        eval.proposal_id,
        if eval.passed { "pass" } else { "fail" }
    )];
    for check in &eval.quality.checks {
        lines.push(format!(
            "- {} {}: {}",
            if check.passed { "ok" } else { "fail" },
            check.name,
            check.detail
        ));
    }
    for note in &eval.notes {
        lines.push(format!("- note: {}", note));
    }
    lines.join("\n")
}

fn format_skill_fitness(snapshot: &crate::engine::skill_evolution::SkillFitnessSnapshot) -> String {
    format!(
        "Skill Fitness /{}\nVersion: {}\nEvents: {}\nFitness: {:.2}\n\nFactors:\n- task_success: {:.2}\n- acceptance_pass_rate: {:.2}\n- test_pass_rate: {:.2}\n- user_satisfaction: {:.2}\n- reuse_rate: {:.2}\n- time_saved: {:.2}\n- tool_efficiency: {:.2}\n- failure_rate: {:.2}\n- cost: {:.2}\n- risk_penalty: {:.2}",
        snapshot.skill_name,
        snapshot.skill_version,
        snapshot.events,
        snapshot.fitness,
        snapshot.stats.task_success,
        snapshot.stats.acceptance_pass_rate,
        snapshot.stats.test_pass_rate,
        snapshot.stats.user_satisfaction,
        snapshot.stats.reuse_rate,
        snapshot.stats.time_saved,
        snapshot.stats.tool_efficiency,
        snapshot.stats.failure_rate,
        snapshot.stats.cost,
        snapshot.stats.risk_penalty
    )
}

fn format_skill_promotion_gate(
    gate: &crate::engine::skill_evolution::SkillPromotionGate,
) -> String {
    let mut lines = vec![format!(
        "Skill Promotion Gate\nResult: {}\nOld fitness: {:.2}\nNew fitness: {:.2}\nDelta: {:.2}\nEval count: {}\nRegression rate: {:.2}\nRisk penalty: {:.2}\nSemantic drift: {:.2}",
        if gate.passed { "pass" } else { "blocked" },
        gate.old_fitness,
        gate.new_fitness,
        gate.delta,
        gate.eval_count,
        gate.regression_rate,
        gate.risk_penalty,
        gate.semantic_drift
    )];
    if !gate.reasons.is_empty() {
        lines.push("Reasons:".to_string());
        for reason in &gate.reasons {
            lines.push(format!("- {}", reason));
        }
    }
    lines.join("\n")
}

fn persist_skill_proposal_learning_event(
    app: &mut TuiApp,
    proposal: &crate::engine::skill_evolution::SkillProposal,
    action: &str,
    applied_path: Option<String>,
) {
    let mut payload = serde_json::to_value(proposal).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(path) = applied_path {
        payload["applied_path"] = serde_json::json!(path);
    }
    if action == "apply" {
        payload["evolution_gate"] =
            serde_json::to_value(skill_evolution_gate(proposal)).unwrap_or_default();
    }
    let _ = app.session_manager.add_learning_event(
        "skill_proposal",
        "skill_evolution",
        &format!("Skill proposal {} {}", proposal.id, action),
        0.9,
        &payload,
    );
}

const EVOLUTION_COOLDOWN_SECS: u64 = 300;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct PersistentEvolutionState {
    #[serde(default)]
    last_update_turn:
        std::collections::HashMap<crate::engine::evolution_controller::EvolutionTarget, u64>,
}

fn evolution_state_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
        .join("evolution_state.json")
}

fn now_evolution_turn() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn load_evolution_state() -> PersistentEvolutionState {
    std::fs::read_to_string(evolution_state_path())
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_evolution_state(state: &PersistentEvolutionState) {
    let path = evolution_state_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, content);
    }
}

fn load_evolution_controller() -> crate::engine::evolution_controller::EvolutionController {
    crate::engine::evolution_controller::EvolutionController::new()
        .with_cooldown_turns(EVOLUTION_COOLDOWN_SECS)
        .with_last_updates(load_evolution_state().last_update_turn)
}

fn record_evolution_update(target: crate::engine::evolution_controller::EvolutionTarget) {
    let mut state = load_evolution_state();
    state.last_update_turn.insert(target, now_evolution_turn());
    save_evolution_state(&state);
}

fn improvement_target(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> crate::engine::evolution_controller::EvolutionTarget {
    match proposal.target {
        crate::engine::improvement::ImprovementTarget::Memory => {
            crate::engine::evolution_controller::EvolutionTarget::Memory
        }
        crate::engine::improvement::ImprovementTarget::Skill => {
            crate::engine::evolution_controller::EvolutionTarget::Skill
        }
        crate::engine::improvement::ImprovementTarget::Prompt => {
            crate::engine::evolution_controller::EvolutionTarget::PromptSection
        }
        crate::engine::improvement::ImprovementTarget::Routing => {
            crate::engine::evolution_controller::EvolutionTarget::WorkflowPolicy
        }
        crate::engine::improvement::ImprovementTarget::ToolGuidance => {
            crate::engine::evolution_controller::EvolutionTarget::ToolDescription
        }
    }
}

fn improvement_evolution_gate(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> crate::engine::evolution_controller::EvolutionGateDecision {
    use crate::engine::evolution_controller::EvolutionTriggerFactors;
    let risk = risk_value(proposal.risk);
    let target = improvement_target(proposal);
    load_evolution_controller().gate(
        target,
        EvolutionTriggerFactors {
            repeated_failure: (proposal.trigger_event_ids.len() as f32 / 4.0).clamp(0.0, 1.0),
            reuse_frequency: 0.55,
            user_correction_frequency: if proposal
                .evidence
                .iter()
                .any(|item| item.to_lowercase().contains("correction"))
            {
                0.80
            } else {
                0.35
            },
            task_impact: if proposal.target == crate::engine::improvement::ImprovementTarget::Memory
            {
                0.55
            } else {
                0.75
            },
            optimization_potential: 0.70,
            evolution_cost: if matches!(
                proposal.target,
                crate::engine::improvement::ImprovementTarget::Prompt
                    | crate::engine::improvement::ImprovementTarget::Routing
            ) {
                0.65
            } else {
                0.35
            },
            risk,
        },
        now_evolution_turn(),
    )
}

fn skill_evolution_gate(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> crate::engine::evolution_controller::EvolutionGateDecision {
    use crate::engine::evolution_controller::{EvolutionTarget, EvolutionTriggerFactors};
    load_evolution_controller().gate(
        EvolutionTarget::Skill,
        EvolutionTriggerFactors {
            repeated_failure: 0.0,
            reuse_frequency: (proposal.evidence_count as f32 / 6.0).clamp(0.0, 1.0),
            user_correction_frequency: proposal.creation_factors.user_correction_value,
            task_impact: proposal.creation_factors.future_utility,
            optimization_potential: proposal.creation_score,
            evolution_cost: proposal.creation_factors.over_specificity.max(0.20),
            risk: 1.0 - proposal.scope_confidence,
        },
        now_evolution_turn(),
    )
}

fn risk_value(risk: crate::engine::intent_router::RiskLevel) -> f32 {
    match risk {
        crate::engine::intent_router::RiskLevel::Low => 0.20,
        crate::engine::intent_router::RiskLevel::Medium => 0.50,
        crate::engine::intent_router::RiskLevel::High => 0.85,
    }
}

fn format_evolution_gate(
    gate: &crate::engine::evolution_controller::EvolutionGateDecision,
) -> String {
    let mut lines = vec![format!(
        "Evolution gate: {:?} target={:?} score={:.2} auto_apply={}",
        gate.action, gate.target, gate.score, gate.auto_apply_allowed
    )];
    for reason in &gate.reasons {
        lines.push(format!("- {}", reason));
    }
    lines.join("\n")
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
mod tests {
    use super::*;

    #[test]
    fn safe_skill_dir_name_rejects_paths() {
        assert!(is_safe_skill_dir_name("rust-debug"));
        assert!(is_safe_skill_dir_name("rust_debug.v1"));
        assert!(!is_safe_skill_dir_name("../rust-debug"));
        assert!(!is_safe_skill_dir_name("rust/debug"));
        assert!(!is_safe_skill_dir_name(".."));
    }

    #[test]
    fn disabled_skill_backups_filters_and_sorts_latest_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lint.disabled-20260101000000")).unwrap();
        std::fs::create_dir_all(dir.path().join("lint.disabled-20260201000000")).unwrap();
        std::fs::create_dir_all(dir.path().join("other.disabled-20260101000000")).unwrap();
        std::fs::create_dir_all(dir.path().join("lint")).unwrap();

        let backups = disabled_skill_backups(dir.path(), Some("lint"));
        assert_eq!(backups.len(), 2);
        assert_eq!(backups[0].backup_name, "lint.disabled-20260201000000");
        assert_eq!(backups[0].skill_name, "lint");

        let latest = resolve_disabled_skill_backup(dir.path(), "lint", None).unwrap();
        assert_eq!(latest.backup_name, "lint.disabled-20260201000000");
    }

    #[test]
    fn memory_proposal_detail_shows_review_fields() {
        use crate::engine::task_contract::{
            MemoryProposal, MemoryProposalCandidate, MemoryProposalConflictGroup,
            MemoryProposalConflictMatch, MemoryProposalGateDecision, MemoryProposalReviewRecord,
            MemoryProposalStatus, MemoryProposalStatusHistoryEntry,
        };

        let proposal = MemoryProposal {
            task_id: "task-memory-review".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "successful_fix".to_string(),
                scope: "project".to_string(),
                content: "Completed parser fix with cargo test parser passing".to_string(),
                evidence: vec!["validation: cargo test parser passed".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        let record = MemoryProposalReviewRecord {
            id: proposal.task_id.clone(),
            proposal,
            created_at: "2026-05-27T00:00:00Z".to_string(),
            updated_at: "2026-05-27T00:01:00Z".to_string(),
            source_session: Some("session-1".to_string()),
            source_task: "task-memory-review".to_string(),
            source: "closeout".to_string(),
            active_scope: "project".to_string(),
            project_id: Some("project:rust-agent".to_string()),
            project_labels: vec!["project_root:/tmp/rust-agent".to_string()],
            gate_report: vec![MemoryProposalGateDecision {
                gate: "write_policy".to_string(),
                candidate_index: None,
                status: "passed".to_string(),
                reason: "write_policy=review_required".to_string(),
            }],
            duplicate_conflict_summary: "not_checked".to_string(),
            conflict_groups: vec![MemoryProposalConflictGroup {
                group_type: "conflict".to_string(),
                key: "language".to_string(),
                scope: "user".to_string(),
                kind: "user_preference".to_string(),
                matches: vec![MemoryProposalConflictMatch {
                    proposal_id: "task-memory-review".to_string(),
                    candidate_index: 0,
                    status: MemoryProposalStatus::Proposed,
                    source: "closeout".to_string(),
                    value: "Chinese".to_string(),
                    content: "language: Chinese".to_string(),
                }],
                resolution_hint: "prefer newer explicit user correction".to_string(),
            }],
            status_history: vec![MemoryProposalStatusHistoryEntry {
                at: "2026-05-27T00:00:00Z".to_string(),
                status: MemoryProposalStatus::Proposed,
                reason: "created".to_string(),
            }],
        };

        let detail = format_memory_proposal_detail(&record);

        assert!(detail.contains("Review state: pending user review; accept before apply"));
        assert!(detail.contains("ID: task-memory-review"));
        assert!(detail.contains("Affects future sessions: after accept/apply only"));
        assert!(detail.contains("Why this was suggested: candidate memory requires review"));
        assert!(detail.contains("Source session: session-1"));
        assert!(detail.contains("Active scope: project"));
        assert!(detail.contains("Project: project:rust-agent"));
        assert!(detail.contains("evidence 1: validation: cargo test parser passed"));
        assert!(detail.contains("Gate report:"));
        assert!(detail.contains("write_policy [proposal]: passed"));
        assert!(detail.contains("Duplicate/conflict: not_checked"));
        assert!(detail.contains("Conflict groups:"));
        assert!(detail.contains("key=language"));
        assert!(detail.contains("Status history:"));
    }

    #[test]
    fn memory_proposal_filter_parses_blocked_flag() {
        let filter = parse_memory_proposal_batch_filter(&[
            "--blocked",
            "--scope",
            "project",
            "--project",
            "rust-agent",
        ]);

        assert!(filter.blocked_only);
        assert_eq!(filter.scope.as_deref(), Some("project"));
        assert_eq!(filter.project.as_deref(), Some("rust-agent"));
    }

    #[test]
    fn memory_proposal_conflict_panel_shows_resolution_command() {
        use crate::engine::task_contract::{
            MemoryProposal, MemoryProposalCandidate, MemoryProposalConflictGroup,
            MemoryProposalConflictMatch, MemoryProposalReviewRecord, MemoryProposalStatus,
        };

        let proposal = MemoryProposal {
            task_id: "pref-keep".to_string(),
            source: "closeout".to_string(),
            status: MemoryProposalStatus::Proposed,
            candidates: vec![MemoryProposalCandidate {
                kind: "user_preference".to_string(),
                scope: "user".to_string(),
                content: "language: Chinese".to_string(),
                evidence: vec!["user: Chinese".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "candidate memory requires review before persistence".to_string(),
        };
        let record = MemoryProposalReviewRecord {
            id: proposal.task_id.clone(),
            proposal,
            created_at: "2026-05-27T00:00:00Z".to_string(),
            updated_at: "2026-05-27T00:01:00Z".to_string(),
            source_session: None,
            source_task: "pref-keep".to_string(),
            source: "closeout".to_string(),
            active_scope: "user".to_string(),
            project_id: Some("project:rust-agent".to_string()),
            project_labels: vec!["project_root:/tmp/rust-agent".to_string()],
            gate_report: Vec::new(),
            duplicate_conflict_summary: "conflicts=1".to_string(),
            conflict_groups: vec![MemoryProposalConflictGroup {
                group_type: "conflict".to_string(),
                key: "language".to_string(),
                scope: "user".to_string(),
                kind: "user_preference".to_string(),
                matches: vec![MemoryProposalConflictMatch {
                    proposal_id: "pref-keep".to_string(),
                    candidate_index: 0,
                    status: MemoryProposalStatus::Proposed,
                    source: "closeout".to_string(),
                    value: "Chinese".to_string(),
                    content: "language: Chinese".to_string(),
                }],
                resolution_hint: "prefer newer explicit user correction".to_string(),
            }],
            status_history: Vec::new(),
        };

        let panel = format_memory_proposal_conflict_panel(&[record]);

        assert!(panel.contains("Memory Proposal Conflicts"));
        assert!(panel.contains("key=language"));
        assert!(panel.contains("status=proposed source=closeout evidence=1"));
        assert!(panel.contains("content=language: Chinese"));
        assert!(panel.contains("/memory-proposals show pref-keep"));
        assert!(panel.contains("/memory-proposals resolve-conflict <keep-task-id>"));
    }

    #[test]
    fn memory_proposal_batch_apply_result_shows_applied_candidates_and_failures() {
        let result = crate::engine::task_contract::MemoryProposalBatchApply {
            matched: 3,
            applied: 2,
            applied_candidates: 4,
            failed: 1,
            proposal_ids: vec!["proposal-a".to_string(), "proposal-b".to_string()],
            failures: vec!["proposal-c: missing evidence".to_string()],
        };

        let output = format_memory_proposal_batch_apply_result(&result);

        assert!(output.contains("Batch applied memory proposals"));
        assert!(output.contains("- matched: 3"));
        assert!(output.contains("- applied: 2"));
        assert!(output.contains("- candidates applied: 4"));
        assert!(output.contains("- failed: 1"));
        assert!(output.contains("proposal-a, proposal-b"));
        assert!(output.contains("proposal-c: missing evidence"));
    }

    fn test_improvement_proposal() -> crate::engine::improvement::ImprovementProposal {
        crate::engine::improvement::ImprovementProposal {
            id: "imp_learning_test".to_string(),
            trigger_event_ids: vec![1, 2],
            target: crate::engine::improvement::ImprovementTarget::ToolGuidance,
            proposed_change:
                "Add guidance for repeated bash failures: inspect arguments before retrying."
                    .to_string(),
            expected_benefit: "Reduce repeated tool failures.".to_string(),
            risk: crate::engine::intent_router::RiskLevel::Medium,
            validation: vec!["Run tool guidance evalset.".to_string()],
            eval_status: crate::engine::improvement::ProposalEvalStatus::Pending,
            eval_summary: None,
            evalset_bindings: Vec::new(),
            status: crate::engine::improvement::ProposalStatus::Accepted,
            evidence: vec!["learning event showed repeated bash failures".to_string()],
            rollback_plan: "Deactivate applied guidance.".to_string(),
            applied_ref: None,
            rollback_ref: None,
            created_at: "2026-05-28T00:00:00Z".to_string(),
            updated_at: "2026-05-28T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn improvement_eval_blocks_apply_without_bound_evalset() {
        let proposal = test_improvement_proposal();

        let eval = evaluate_improvement_proposal_for_apply(&proposal);

        assert!(!eval.passed);
        assert!(eval.summary.contains("missing bound evalset"));
        assert!(eval.summary.contains("failure_owner=framework"));
    }

    #[test]
    fn improvement_detail_shows_applied_guidance_and_effect_summary() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::engine::improvement::ImprovementStore::new(
            dir.path().join("improvements.jsonl"),
        );
        let mut proposal = test_improvement_proposal();
        proposal.evalset_bindings = vec!["tool-guidance-smoke".to_string()];
        proposal.eval_status = crate::engine::improvement::ProposalEvalStatus::Passed;
        proposal.eval_summary = Some("eval passed".to_string());
        store.upsert(&proposal).unwrap();
        store
            .update_status(
                &proposal.id,
                crate::engine::improvement::ProposalStatus::Applied,
            )
            .unwrap();
        store
            .effect_store()
            .record(
                &proposal.id,
                "tool-guidance-smoke",
                "run-1",
                crate::engine::improvement::ImprovementEffectOutcome::Positive,
                "none",
                "reduced repeated tool failures",
            )
            .unwrap();
        let applied = store.get(&proposal.id).unwrap();

        let detail = format_improvement_detail_with_state(&applied, &store);

        assert!(detail.contains("Applied guidance:"));
        assert!(detail.contains("status=Active"));
        assert!(detail.contains("Effect summary:"));
        assert!(detail.contains("positive=1"));
    }

    #[test]
    fn applied_guidance_panel_and_effect_panel_show_operational_state() {
        let mut proposal = test_improvement_proposal();
        proposal.evalset_bindings = vec!["tool-guidance-smoke".to_string()];
        proposal.eval_status = crate::engine::improvement::ProposalEvalStatus::Passed;
        let guidance = crate::engine::improvement::AppliedGuidanceRecord::from_proposal(
            &proposal,
            "2026-05-28T00:00:00Z".to_string(),
        );
        let list = format_applied_guidance_list(&[guidance]);

        assert!(list.contains("Active Applied Guidance (1 total)"));
        assert!(list.contains("activation=ToolContractHint"));
        assert!(list.contains("evalsets=tool-guidance-smoke"));

        let summary = crate::engine::improvement::ImprovementEffectSummary {
            proposal_id: proposal.id.clone(),
            total: 1,
            positive: 0,
            neutral: 0,
            negative: 1,
            rollback_recommended: false,
            recent: vec![crate::engine::improvement::ImprovementEffectRecord {
                id: "effect-1".to_string(),
                proposal_id: proposal.id,
                evalset: "tool-guidance-smoke".to_string(),
                run_id: "run-1".to_string(),
                outcome: crate::engine::improvement::ImprovementEffectOutcome::Negative,
                failure_owner: "framework".to_string(),
                reason: "regressed validation".to_string(),
                created_at: "2026-05-28T00:01:00Z".to_string(),
            }],
        };
        let effect = format_improvement_effect_summary(&summary);

        assert!(effect.contains("negative=1"));
        assert!(effect.contains("owner=framework"));
        assert!(effect.contains("regressed validation"));
    }
}
