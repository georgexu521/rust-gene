use super::*;

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
            return goal_not_implemented(
                "pause",
                "Pause automatic goal continuation. When the runner is active this will pause automatic turn scheduling.",
            );
        }
        "resume" => {
            if has_runner {
                if let Some(session_id) = app.session_manager.current_session_id() {
                    if let Some(ref runner) = app.goal_runner {
                        match runner.resume(session_id) {
                            Ok(true) => {
                                app.pending_goal_prompt =
                                    Some("Continue working toward the active goal.".to_string());
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
            return goal_not_implemented(
                "resume",
                "Resume automatic goal continuation. When paused this will restart turn scheduling if the goal is active.",
            );
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
        if objective.chars().count() > 4000 {
            return format!(
                "Goal Error\n- objective is {} characters, maximum is 4000",
                objective.chars().count()
            );
        }
        if has_runner {
            if let Some(session_id) = app.session_manager.current_session_id() {
                if let Some(ref runner) = app.goal_runner {
                    match runner.edit_objective(session_id, objective) {
                        Ok(Some(goal)) => {
                            return format!(
                                "Goal edited\n- Id: {}\n- New objective: {}\n- Status: {:?}\n- Turn: {}/{}",
                                goal.id,
                                goal.objective,
                                goal.status,
                                goal.turn_count,
                                goal.budget.max_turns
                            );
                        }
                        Ok(None) => {
                            return "Goal edit: no active goal to edit. Use /goal <objective> to start one.".to_string();
                        }
                        Err(e) => {
                            return format!("Goal edit error: {}", e);
                        }
                    }
                }
            }
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

pub(super) fn goal_not_implemented(subcommand: &str, detail: &str) -> String {
    format!(
        "Goal {}\n- not implemented yet (Phase 1+)\n\n{}",
        subcommand, detail
    )
}

pub(super) fn set_goal_objective(
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
