//! Observability and runtime inspection slash command handlers.

use super::utils::*;

use crate::tui::app::TuiApp;
use std::collections::HashSet;

const RECENT_TRACE_LIMIT: usize = 10;

/// /hooks - Show hook configuration status
pub fn handle_hooks(app: &TuiApp) -> String {
    use std::env;

    let pre_hook = env::var("PRIORITY_AGENT_PRE_TOOL_HOOK").ok();
    let post_hook = env::var("PRIORITY_AGENT_POST_TOOL_HOOK").ok();
    let mut tool_before = Vec::new();
    let mut tool_after = Vec::new();
    for (key, value) in env::vars() {
        if key.starts_with("PRIORITY_AGENT_TOOL_HOOK_BEFORE_") && !value.trim().is_empty() {
            tool_before.push(format!("{}={}", key, value));
        } else if key.starts_with("PRIORITY_AGENT_TOOL_HOOK_AFTER_") && !value.trim().is_empty() {
            tool_after.push(format!("{}={}", key, value));
        }
    }
    tool_before.sort();
    tool_after.sort();
    let timeout = env::var("PRIORITY_AGENT_HOOK_TIMEOUT_MS").ok();
    let fail_closed = env::var("PRIORITY_AGENT_HOOK_FAIL_CLOSED").ok();

    let mut lines = vec!["Hook Configuration:".to_string()];

    if let Some(ref h) = pre_hook {
        lines.push(format!("  PRE_TOOL_HOOK: {}", h));
    } else {
        lines.push("  PRE_TOOL_HOOK: not set".to_string());
    }
    if let Some(ref h) = post_hook {
        lines.push(format!("  POST_TOOL_HOOK: {}", h));
    } else {
        lines.push("  POST_TOOL_HOOK: not set".to_string());
    }
    if !tool_before.is_empty() {
        lines.push("  TOOL_HOOK_BEFORE:".to_string());
        for hook in &tool_before {
            lines.push(format!("    {}", hook));
        }
    } else {
        lines.push("  TOOL_HOOK_BEFORE: not set".to_string());
    }
    if !tool_after.is_empty() {
        lines.push("  TOOL_HOOK_AFTER:".to_string());
        for hook in &tool_after {
            lines.push(format!("    {}", hook));
        }
    } else {
        lines.push("  TOOL_HOOK_AFTER: not set".to_string());
    }
    lines.push(format!(
        "  HOOK_TIMEOUT_MS: {}",
        timeout.unwrap_or_else(|| "1000".to_string())
    ));
    lines.push(format!(
        "  HOOK_FAIL_CLOSED: {}",
        fail_closed.unwrap_or_else(|| "false".to_string())
    ));
    lines.push(
        "\nTyped lifecycle events: PromptSubmit, PreToolUse, PostToolUse, PermissionRequest, ValidationStart, ValidationEnd, SubagentStart, SubagentEnd, FileChange, Compact, SessionEnd"
            .to_string(),
    );

    if let Some(trace) = latest_trace_for_app(app) {
        let hook_events: Vec<_> = trace
            .events
            .iter()
            .filter_map(|event| {
                if let crate::engine::trace::TraceEvent::HookCompleted {
                    event,
                    provider,
                    hook_name,
                    call_id,
                    tool,
                    success,
                    blocked,
                    duration_ms,
                    error,
                    output_preview,
                } = event
                {
                    Some(format!(
                        "  - {} '{}' provider={} tool={} call={} {}{} in {}ms{}",
                        event,
                        hook_name,
                        provider,
                        tool.as_deref().unwrap_or("lifecycle"),
                        call_id,
                        if *success { "ok" } else { "failed" },
                        if *blocked { " blocked" } else { "" },
                        duration_ms,
                        error
                            .as_deref()
                            .or(output_preview.as_deref())
                            .map(|detail| format!(
                                ": {}",
                                detail.chars().take(120).collect::<String>()
                            ))
                            .unwrap_or_default()
                    ))
                } else {
                    None
                }
            })
            .collect();
        if hook_events.is_empty() {
            lines.push("\nRecent hook executions: none in latest trace".to_string());
        } else {
            lines.push("\nRecent hook executions from latest trace:".to_string());
            lines.extend(hook_events.into_iter().rev().take(8));
        }
    } else {
        lines.push("\nRecent hook executions: no trace recorded yet".to_string());
    }

    if pre_hook.is_none() && post_hook.is_none() && tool_before.is_empty() && tool_after.is_empty()
    {
        lines.push(
            "\nNo hooks configured. Set PRIORITY_AGENT_*_HOOK environment variables.".to_string(),
        );
    }

    lines.join("\n")
}

/// /profiling - Show runtime profiling info
pub fn handle_profiling(app: &TuiApp) -> String {
    let mut lines = vec!["Profiling Info:".to_string()];

    // Session info
    if let Some(id) = app.session_manager.current_session_id() {
        lines.push(format!("  Session: {}...", &id[..8.min(id.len())]));
    }
    lines.push(format!("  Messages: {}", app.messages.len()));

    // Engine info
    if app.streaming_engine.is_some() {
        lines.push("  Engine: StreamingQueryEngine".to_string());
    } else {
        lines.push("  Engine: not initialized".to_string());
    }

    // Memory
    if let Some(ref engine) = app.streaming_engine {
        if engine.memory_manager().is_some() {
            lines.push("  Memory: active (use /memory to view)".to_string());
        }
    }

    lines.join("\n")
}

/// /debug - Toggle debug mode
pub fn handle_debug_cmd(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "on" {
        std::env::set_var("RUST_LOG", "debug");
        "Debug mode enabled (RUST_LOG=debug)".to_string()
    } else if args == "off" {
        std::env::set_var("RUST_LOG", "info");
        "Debug mode disabled (RUST_LOG=info)".to_string()
    } else {
        "Usage: /debug [on|off]".to_string()
    }
}

/// /trace - Runtime trace viewer and tracing controls
pub fn handle_trace(app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "last" {
        if let Some(engine) = &app.streaming_engine {
            if let Some(trace) = engine.trace_store().latest() {
                return crate::engine::trace::format_trace_summary(&trace, 80);
            }
        }
        return match app.session_manager.latest_trace() {
            Ok(Some(trace)) => crate::engine::trace::format_trace_summary(&trace, 80),
            Ok(None) => "No turn trace recorded yet.".to_string(),
            Err(e) => format!("Failed to load latest trace: {}", e),
        };
    } else if arg == "recent" {
        let mut traces = Vec::new();
        if let Some(engine) = &app.streaming_engine {
            traces = engine.trace_store().recent(RECENT_TRACE_LIMIT);
        }
        match app.session_manager.recent_traces(RECENT_TRACE_LIMIT as i64) {
            Ok(persisted) => {
                traces = merge_recent_traces(traces, persisted, RECENT_TRACE_LIMIT);
            }
            Err(e) if traces.is_empty() => return format!("Failed to load recent traces: {}", e),
            _ => {}
        }
        if !traces.is_empty() {
            return format_trace_recent_lines(traces);
        } else {
            return "No recent traces recorded.".to_string();
        }
    } else if arg == "status" {
        return format!(
            "Log tracing: {}\nRuntime traces: {}",
            if prefs.trace { "enabled" } else { "disabled" },
            app.streaming_engine
                .as_ref()
                .map(|engine| engine.trace_store().len().to_string())
                .unwrap_or_else(|| "unavailable".to_string())
        );
    }

    match arg {
        "on" | "enable" => prefs.trace = true,
        "off" | "disable" => prefs.trace = false,
        "toggle" => prefs.trace = !prefs.trace,
        _ => return "Usage: /trace [last|recent|on|off|toggle|status]".to_string(),
    }
    std::env::set_var(
        "RUST_LOG",
        if prefs.trace {
            "trace"
        } else if prefs.verbose {
            "debug"
        } else {
            "info"
        },
    );
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Tracing changed but failed to persist: {}", e);
    }
    format!(
        "Tracing {}.",
        if prefs.trace { "enabled" } else { "disabled" }
    )
}

fn format_trace_recent_lines(traces: Vec<crate::engine::trace::TurnTrace>) -> String {
    let mut lines = vec!["Recent traces:".to_string()];
    for trace in traces {
        lines.push(crate::engine::trace::format_trace_recent_line(&trace));
    }
    lines.join("\n")
}

fn merge_recent_traces(
    memory_traces: Vec<crate::engine::trace::TurnTrace>,
    persisted_traces: Vec<crate::engine::trace::TurnTrace>,
    limit: usize,
) -> Vec<crate::engine::trace::TurnTrace> {
    let mut seen = HashSet::new();
    let mut traces = Vec::new();
    for trace in memory_traces.into_iter().chain(persisted_traces) {
        if seen.insert(trace.trace_id.clone()) {
            traces.push(trace);
        }
    }
    traces.sort_by(|left, right| {
        right
            .turn_index
            .cmp(&left.turn_index)
            .then_with(|| right.started_at.cmp(&left.started_at))
    });
    traces.truncate(limit);
    traces
}

/// /eval - Deterministic behavior evalsets
pub fn handle_eval(_app: &mut TuiApp, args: &str) -> String {
    let eval_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("evalsets");
    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");

    match action {
        "list" => match crate::engine::evalset::load_evalsets_from_dir(&eval_dir) {
            Ok(sets) if sets.is_empty() => {
                format!("No evalsets found in {}.", eval_dir.display())
            }
            Ok(sets) => {
                let mut lines = vec![format!("Evalsets in {}:", eval_dir.display())];
                for (path, set) in sets {
                    lines.push(format!(
                        "- {} [{} scenarios] {}",
                        set.name,
                        set.scenarios.len(),
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("unknown")
                    ));
                }
                lines.push("Run with /eval run <name|all>.".to_string());
                lines.join("\n")
            }
            Err(e) => format!("Failed to list evalsets: {}", e),
        },
        "run" => {
            let target = parts.next().unwrap_or("all");
            match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(target)) {
                Ok(reports) => crate::engine::evalset::format_reports(&reports),
                Err(e) => format!("Eval run failed: {}", e),
            }
        }
        "json" => {
            let target = parts.next().unwrap_or("all");
            match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(target)) {
                Ok(reports) => crate::engine::evalset::format_reports_json(&reports)
                    .unwrap_or_else(|e| format!("Eval JSON failed: {}", e)),
                Err(e) => format!("Eval run failed: {}", e),
            }
        }
        "record" => {
            let target = parts.next().unwrap_or("all");
            match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(target)) {
                Ok(reports) => {
                    let report_dir = std::env::current_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        .join("target")
                        .join("eval-reports");
                    match crate::engine::evalset::write_reports_json(&reports, &report_dir, target)
                    {
                        Ok(path) => format!(
                            "{}\n\nRecorded JSON report: {}",
                            crate::engine::evalset::format_reports(&reports),
                            path.display()
                        ),
                        Err(e) => format!("Eval record failed: {}", e),
                    }
                }
                Err(e) => format!("Eval run failed: {}", e),
            }
        }
        "trend" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(10)
                .clamp(1, 50);
            let report_dir = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("target")
                .join("eval-reports");
            match crate::engine::evalset::load_eval_report_bundles(&report_dir, limit) {
                Ok(entries) => crate::engine::evalset::format_eval_trend(&entries),
                Err(e) => format!("Eval trend failed: {}", e),
            }
        }
        _ => "Usage: /eval [list|run <name|all>|json <name|all>|record <name|all>|trend [limit]]"
            .to_string(),
    }
}

/// /resource - Show the latest selected resource policy
pub fn handle_resource(app: &mut TuiApp) -> String {
    let Some(trace) = latest_trace_for_app(app) else {
        return "No resource policy recorded yet. Send a message, then run /resource or /trace last."
            .to_string();
    };
    let policy = trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::ResourcePolicySelected {
            latency,
            target_ms,
            cost_ceiling_usd,
            reasoning,
            parallelism_limit,
            max_tool_calls,
            context_budget_tokens,
            reason,
        } = event
        {
            Some((
                latency,
                target_ms,
                cost_ceiling_usd,
                reasoning,
                parallelism_limit,
                max_tool_calls,
                context_budget_tokens,
                reason,
            ))
        } else {
            None
        }
    });

    let Some((
        latency,
        target_ms,
        cost_ceiling_usd,
        reasoning,
        parallelism_limit,
        max_tool_calls,
        context_budget_tokens,
        reason,
    )) = policy
    else {
        return format!(
            "No resource policy in latest trace {}. Use /trace last for the full timeline.",
            &trace.trace_id[..8.min(trace.trace_id.len())]
        );
    };

    format!(
        "Resource Policy\n- trace: {}\n- latency: {} ({} ms)\n- cost ceiling: ${:.2}\n- reasoning: {}\n- parallelism: {}\n- max tool calls: {}\n- context budget: {} tokens\n- reason: {}\n\nRuntime Inventory\n- skills: {}\n- agent definitions: {}\n- mcp servers: {}\n- evalsets: {}",
        &trace.trace_id[..8.min(trace.trace_id.len())],
        latency,
        target_ms,
        cost_ceiling_usd,
        reasoning,
        parallelism_limit,
        max_tool_calls,
        context_budget_tokens,
        reason,
        app.skill_runtime.len(),
        crate::agent::profiles::load_definitions(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        )
        .len(),
        app.streaming_engine
            .as_ref()
            .and_then(|engine| engine.mcp_manager())
            .map(|mcp| mcp.health_diagnostics().len())
            .unwrap_or(0),
        crate::engine::evalset::load_evalsets_from_dir(
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("evalsets")
        )
        .map(|sets| sets.len())
        .unwrap_or(0)
    )
}

/// /memory - Memory management (enhanced)
pub fn handle_memory(_app: &TuiApp) -> String {
    let root = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent");
    let mem_path = root.join("memory");
    let project_exists = root.join("MEMORY.md").exists();
    let user_exists = root.join("USER.md").exists();
    let topic_files = count_files_with_ext(&mem_path, "md");
    let agent_files = count_files_with_ext(&mem_path.join("agents"), "json");

    if !project_exists && !user_exists && topic_files == 0 && agent_files == 0 {
        return "No memory entries saved. Start chatting to create memories.".to_string();
    }

    format!(
        "Memory namespaces:\n- project: {}\n- user: {}\n- topic files: {}\n- agent files: {}\n\nUse memory_load with a query to search across namespaces and show conflict hints.\nStored in: {}",
        if project_exists { "MEMORY.md" } else { "none" },
        if user_exists { "USER.md" } else { "none" },
        topic_files,
        agent_files,
        root.display()
    )
}

fn count_files_with_ext(dir: &std::path::Path, ext: &str) -> usize {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    entries
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some(ext))
        .count()
}

/// /skills - List available skills
pub fn handle_skills(app: &TuiApp) -> String {
    let names = app
        .skill_runtime
        .names()
        .into_iter()
        .map(|name| format!("/{}", name))
        .collect::<Vec<_>>();
    format!(
        "Skills ({} available):\n{}\n\nInvoke directly with /<skill-name> <task>, or use skill_view for full skill content.",
        app.skill_runtime.len(),
        names.join(", ")
    )
}

// ═══════════════════════════════════════
// Phase 10 Final: profile, theme, shortcuts, quick, feedback
// ═══════════════════════════════════════

/// /profile - Edit user profile
pub fn handle_profile(_app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "show" {
        return match load_profile() {
            Ok(map) if map.is_empty() => "Profile is empty.".to_string(),
            Ok(map) => {
                let mut keys: Vec<_> = map.keys().cloned().collect();
                keys.sort();
                let mut lines = vec!["Profile:".to_string()];
                for k in keys {
                    if let Some(v) = map.get(&k) {
                        lines.push(format!("- {} = {}", k, v));
                    }
                }
                lines.join("\n")
            }
            Err(e) => format!("Failed to load profile: {}", e),
        };
    }

    let mut parts = args.splitn(2, ' ');
    let action = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();
    match action {
        "show" => {
            let Some(key) = sanitize_profile_key(rest) else {
                return "Usage: /profile show <key>".to_string();
            };
            match load_profile() {
                Ok(map) => match map.get(&key) {
                    Some(v) => format!("{} = {}", key, v),
                    None => format!("Profile key '{}' not found.", key),
                },
                Err(e) => format!("Failed to load profile: {}", e),
            }
        }
        "set" => {
            let mut kv = rest.splitn(2, ' ');
            let raw_key = kv.next().unwrap_or_default();
            let value = kv.next().unwrap_or("").trim();
            let Some(key) = sanitize_profile_key(raw_key) else {
                return "Usage: /profile set <key> <value>".to_string();
            };
            if value.is_empty() {
                return "Usage: /profile set <key> <value>".to_string();
            }
            let mut map = match load_profile() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load profile: {}", e),
            };
            map.insert(key.clone(), value.to_string());
            match save_profile(&map) {
                Ok(_) => format!("Profile updated: {} = {}", key, value),
                Err(e) => format!("Failed to save profile: {}", e),
            }
        }
        "unset" => {
            let Some(key) = sanitize_profile_key(rest) else {
                return "Usage: /profile unset <key>".to_string();
            };
            let mut map = match load_profile() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load profile: {}", e),
            };
            if map.remove(&key).is_none() {
                return format!("Profile key '{}' not found.", key);
            }
            match save_profile(&map) {
                Ok(_) => format!("Profile key '{}' removed.", key),
                Err(e) => format!("Failed to save profile: {}", e),
            }
        }
        _ => "Usage: /profile [show [key]|set <key> <value>|unset <key>]".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace(id: &str, turn_index: u64) -> crate::engine::trace::TurnTrace {
        let mut trace =
            crate::engine::trace::TurnTrace::new("session", turn_index, "inspect workspace");
        trace.trace_id = id.to_string();
        trace.finish(crate::engine::trace::TurnStatus::Completed);
        trace
    }

    #[test]
    fn merge_recent_traces_keeps_persisted_history_after_memory_entries() {
        let traces = merge_recent_traces(
            vec![trace("trace-3", 3), trace("trace-2", 2)],
            vec![trace("trace-3", 3), trace("trace-1", 1)],
            10,
        );

        assert_eq!(traces.len(), 3);
        assert_eq!(
            traces
                .iter()
                .map(|trace| trace.turn_index)
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
    }

    #[test]
    fn merge_recent_traces_applies_recent_limit_after_deduping() {
        let traces = merge_recent_traces(
            vec![trace("trace-4", 4)],
            vec![
                trace("trace-3", 3),
                trace("trace-2", 2),
                trace("trace-1", 1),
            ],
            2,
        );

        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].turn_index, 4);
        assert_eq!(traces[1].turn_index, 3);
    }
}
