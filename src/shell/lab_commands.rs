//! Shell-facing LabRun command handlers.
//!
//! Adapts LabRun command execution to the shell entrypoint while keeping orchestration logic in the lab modules.

use super::*;

pub(super) async fn handle_lab_provider_professor_review_command(
    engine: &StreamingQueryEngine,
    project_root: &std::path::Path,
    instructions: &str,
) -> String {
    match crate::lab::draft::draft_professor_review_with_provider(
        project_root,
        engine.provider(),
        engine.model_name(),
        instructions,
    )
    .await
    {
        Ok(outcome) => {
            let mut lines = vec![
                format!(
                    "Drafted provider ProfessorReview: {}",
                    outcome.created.artifact.artifact_id()
                ),
                format!("Gate: {}", outcome.created.gate.stage),
                format!(
                    "Validation: {}",
                    outcome
                        .created
                        .gate
                        .validation_status
                        .as_deref()
                        .unwrap_or("none")
                ),
                format!("Artifact: {}", outcome.created.path.display()),
                format!("Report: {}", outcome.created.report_path.display()),
            ];
            if let Some(usage) = outcome.usage {
                lines.push(format!(
                    "Usage: prompt={} completion={} total={} cached={} cache_write={}",
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens,
                    usage.cached_tokens.unwrap_or(0),
                    usage.cache_write_tokens.unwrap_or(0)
                ));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to draft provider ProfessorReview: {err}"),
    }
}

pub(super) fn start_lab_daemon_from_policy(
    engine: &StreamingQueryEngine,
    host: &dyn ShellHost,
    project_root: &std::path::Path,
    prefix: Option<String>,
) -> String {
    match crate::lab::scheduler::start_daemon_scheduler_from_policy(
        project_root,
        host.build_tool_context(),
        engine.provider(),
        engine.model_name(),
    ) {
        Ok(started) => format!(
            "{}Lab daemon started for {}.\nMode: {:?}\nMax steps: {}\nInterval ms: {}",
            prefix
                .as_deref()
                .map(|value| format!("{value}: "))
                .unwrap_or_default(),
            started.lab_run_id,
            started.mode,
            started.max_steps,
            started.interval_ms
        ),
        Err(err) => {
            let _ = LabStore::for_project(project_root)
                .record_daemon_start_result(None, Some(&err.to_string()));
            format!("Failed to start Lab daemon from policy: {err}")
        }
    }
}

pub(super) async fn handle_lab_background_hybrid_command(
    engine: &StreamingQueryEngine,
    project_root: &std::path::Path,
    args: &str,
    tool_context: crate::tools::ToolContext,
) -> String {
    let (max_steps, interval_ms, instructions) = match parse_lab_background_hybrid_args(args) {
        Ok(parsed) => parsed,
        Err(message) => return message,
    };
    match crate::lab::scheduler::start_background_hybrid_scheduler(
        project_root,
        tool_context,
        engine.provider(),
        engine.model_name(),
        max_steps,
        interval_ms,
        instructions.to_string(),
    ) {
        Ok(started) => format!(
            "Started Lab hybrid background scheduler for {}.\nMax steps: {}\nInterval ms: {}",
            started.lab_run_id, started.max_steps, started.interval_ms
        ),
        Err(err) => format!("Failed to start Lab hybrid background scheduler: {err}"),
    }
}

pub(super) async fn handle_lab_hybrid_run_command(
    engine: &StreamingQueryEngine,
    project_root: &std::path::Path,
    args: &str,
    tool_context: crate::tools::ToolContext,
) -> String {
    let (max_steps, instructions) = match parse_lab_run_args(args, "/lab run hybrid") {
        Ok(parsed) => parsed,
        Err(message) => return message,
    };
    match crate::lab::draft::run_hybrid_lab_steps_until_boundary(
        project_root,
        engine.provider(),
        engine.model_name(),
        max_steps,
        instructions,
        tool_context,
    )
    .await
    {
        Ok(outcome) => {
            let mut lines = vec![
                format!("Lab hybrid run: {} step(s)", outcome.steps.len()),
                format!("LabRun: {}", outcome.lab_run_id),
                format!("Final stage: {}", outcome.final_stage),
                format!("Stop reason: {:?}", outcome.stop_reason),
            ];
            for (idx, step) in outcome.steps.iter().enumerate() {
                match step {
                    crate::lab::draft::LabHybridRunStep::Provider(step) => {
                        lines.push(format!(
                            "{}. provider {} -> {} artifact={} review={:?} advanced={} note={}",
                            idx + 1,
                            step.from_stage,
                            step.to_stage,
                            step.artifact_id,
                            step.review_decision,
                            step.advanced,
                            step.review_note
                        ));
                    }
                    crate::lab::draft::LabHybridRunStep::Scheduler(step) => {
                        lines.push(format!(
                            "{}. scheduler {:?} stage={} task={} dispatch={} - {}",
                            idx + 1,
                            step.action,
                            step.stage,
                            step.task_id.as_deref().unwrap_or("none"),
                            step.dispatch_id.as_deref().unwrap_or("none"),
                            step.message
                        ));
                    }
                    crate::lab::draft::LabHybridRunStep::Deterministic(step) => {
                        lines.push(format!(
                            "{}. deterministic {} -> {} artifact={} gate_satisfied={} - {}",
                            idx + 1,
                            step.from_stage,
                            step.to_stage,
                            step.artifact_id,
                            step.gate_satisfied,
                            step.message
                        ));
                    }
                }
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to run hybrid Lab stages: {err}"),
    }
}

pub(super) async fn handle_lab_provider_run_command(
    engine: &StreamingQueryEngine,
    project_root: &std::path::Path,
    args: &str,
) -> String {
    let (max_steps, instructions) = match parse_lab_run_args(args, "/lab run llm") {
        Ok(parsed) => parsed,
        Err(message) => return message,
    };
    match crate::lab::draft::run_provider_stage_steps_until_boundary(
        project_root,
        engine.provider(),
        engine.model_name(),
        max_steps,
        instructions,
    )
    .await
    {
        Ok(outcome) => {
            let mut lines = vec![
                format!("Lab provider run: {} step(s)", outcome.steps.len()),
                format!("LabRun: {}", outcome.lab_run_id),
                format!("Final stage: {}", outcome.final_stage),
                format!("Stop reason: {:?}", outcome.stop_reason),
            ];
            for (idx, step) in outcome.steps.iter().enumerate() {
                lines.push(format!(
                    "{}. {} -> {} artifact={} review={:?} advanced={} note={}",
                    idx + 1,
                    step.from_stage,
                    step.to_stage,
                    step.artifact_id,
                    step.review_decision,
                    step.advanced,
                    step.review_note
                ));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to run Lab provider stages: {err}"),
    }
}

pub(super) async fn handle_lab_provider_step_command(
    engine: &StreamingQueryEngine,
    project_root: &std::path::Path,
    instructions: &str,
) -> String {
    match crate::lab::draft::run_provider_stage_step(
        project_root,
        engine.provider(),
        engine.model_name(),
        instructions,
    )
    .await
    {
        Ok(outcome) => [
            "Lab provider stage step".to_string(),
            format!("LabRun: {}", outcome.lab_run_id),
            format!("Stage: {} -> {}", outcome.from_stage, outcome.to_stage),
            format!("Artifact: {}", outcome.artifact_id),
            format!("Review: {:?}", outcome.review_decision),
            format!("Advanced: {}", outcome.advanced),
            format!("Note: {}", outcome.review_note),
        ]
        .join("\n"),
        Err(err) => format!("Failed to run Lab provider stage step: {err}"),
    }
}

pub(super) async fn handle_lab_artifact_review_command(
    engine: &StreamingQueryEngine,
    project_root: &std::path::Path,
    args: &str,
) -> String {
    let (artifact_id, instructions) = split_once_local(args);
    if artifact_id.trim().is_empty() {
        return "Usage: /lab review artifact <artifact_id> [instructions]".to_string();
    }
    match crate::lab::draft::review_stage_artifact_with_provider(
        project_root,
        engine.provider(),
        engine.model_name(),
        artifact_id,
        instructions,
    )
    .await
    {
        Ok(outcome) => {
            let mut lines = vec![
                format!("Reviewed artifact: {}", outcome.artifact_id),
                format!("Decision: {:?}", outcome.decision),
                format!("Gate: {}", outcome.gate.stage),
                format!("Note: {}", outcome.note),
            ];
            if let Some(usage) = outcome.usage {
                lines.push(format!(
                    "Usage: prompt={} completion={} total={} cached={} cache_write={}",
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens,
                    usage.cached_tokens.unwrap_or(0),
                    usage.cache_write_tokens.unwrap_or(0)
                ));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to review Lab artifact: {err}"),
    }
}

pub(super) async fn handle_lab_draft_command(
    engine: &StreamingQueryEngine,
    project_root: &std::path::Path,
    instructions: &str,
) -> String {
    match crate::lab::draft::draft_current_stage_artifact(
        project_root,
        engine.provider(),
        engine.model_name(),
        instructions,
    )
    .await
    {
        Ok(outcome) => {
            let mut lines = vec![
                format!(
                    "Drafted {} artifact: {}",
                    outcome.created.artifact.artifact_type().as_str(),
                    outcome.created.artifact.artifact_id()
                ),
                format!("Gate satisfied for stage '{}'.", outcome.created.gate.stage),
                format!("Artifact: {}", outcome.created.path.display()),
                format!("Report: {}", outcome.created.report_path.display()),
            ];
            if let Some(usage) = outcome.usage {
                lines.push(format!(
                    "Usage: prompt={} completion={} total={} cached={} cache_write={}",
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens,
                    usage.cached_tokens.unwrap_or(0),
                    usage.cache_write_tokens.unwrap_or(0)
                ));
            }
            lines.join("\n")
        }
        Err(err) => format!("Failed to draft Lab artifact: {err}"),
    }
}

pub(super) fn split_once_local(input: &str) -> (&str, &str) {
    let trimmed = input.trim();
    match trimmed.find(char::is_whitespace) {
        Some(idx) => (&trimmed[..idx], trimmed[idx..].trim()),
        None => (trimmed, ""),
    }
}

pub(super) fn parse_lab_run_args<'a>(
    args: &'a str,
    command: &str,
) -> Result<(usize, &'a str), String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return Ok((5, ""));
    }
    let (first, rest) = split_once_local(trimmed);
    match first.parse::<usize>() {
        Ok(max_steps) if max_steps > 0 => Ok((max_steps, rest)),
        Ok(_) => Err(format!("Usage: {command} [max_steps] [instructions]")),
        Err(_) => Ok((5, trimmed)),
    }
}

pub(super) fn parse_lab_background_hybrid_args(args: &str) -> Result<(usize, u64, &str), String> {
    let trimmed = args.trim();
    let default_max_steps = crate::lab::scheduler::default_background_max_steps();
    let default_interval_ms = crate::lab::scheduler::default_background_interval_ms();
    if trimmed.is_empty() {
        return Ok((default_max_steps, default_interval_ms, ""));
    }

    let (first, rest) = split_once_local(trimmed);
    let max_steps = match first.parse::<usize>() {
        Ok(value) if value > 0 => value,
        Ok(_) => {
            return Err(
                "Usage: /lab background hybrid [max_steps] [interval_ms] [instructions]"
                    .to_string(),
            );
        }
        Err(_) => return Ok((default_max_steps, default_interval_ms, trimmed)),
    };

    let rest = rest.trim();
    if rest.is_empty() {
        return Ok((max_steps, default_interval_ms, ""));
    }
    let (second, instructions) = split_once_local(rest);
    let interval_ms = match second.parse::<u64>() {
        Ok(value) if value > 0 => value,
        Ok(_) => {
            return Err(
                "Usage: /lab background hybrid [max_steps] [interval_ms] [instructions]"
                    .to_string(),
            );
        }
        Err(_) => return Ok((max_steps, default_interval_ms, rest)),
    };
    Ok((max_steps, interval_ms, instructions))
}
