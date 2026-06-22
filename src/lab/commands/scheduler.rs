//! LabRun scheduler command handlers.
//!
//! Scheduler commands run foreground or background stage steps through
//! `LabOrchestrator`, preserving persisted state and explicit validation gates.

use super::*;

pub(super) async fn handle_scheduler_step_command(
    project_root: &Path,
    tool_context: ToolContext,
) -> String {
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

pub(super) async fn handle_scheduler_run_command(
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

pub(super) async fn handle_background_command(
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
                LabHybridCycleBackgroundRequest {
                    context: tool_context.clone(),
                    provider,
                    model: tool_context.model.clone(),
                    max_cycles,
                    max_steps_per_cycle,
                    interval_ms,
                    instructions,
                },
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
