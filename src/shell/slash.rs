//! CLI wrappers for slash command handlers that only need the session manager.
//!
//! These functions extract the session manager from a `ShellHost` and call the
//! underlying TUI handler logic. They exist because some handlers only touch
//! session state and do not require Ratatui widgets.

use crate::engine::streaming::StreamingQueryEngine;
use crate::shell::host::ShellHost;

pub fn handle_undo(host: &mut dyn ShellHost, args: &str) -> String {
    let session_id = match host.session_manager().current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let n = match crate::tui::slash_handler::utils::parse_optional_count(args, "/undo") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut successes = 0usize;
    let mut last_error = None::<String>;
    for _ in 0..n {
        match host.session_manager().rewind_last_edit(session_id) {
            Ok(_) => successes += 1,
            Err(e) => {
                last_error = Some(e.to_string());
                break;
            }
        }
    }

    if successes == 0 {
        format!(
            "Nothing to undo or undo failed{}",
            last_error.map(|e| format!(": {e}")).unwrap_or_default()
        )
    } else {
        format!(
            "Undid {successes} edit{}.",
            if successes > 1 { "s" } else { "" }
        )
    }
}

pub fn handle_redo(host: &mut dyn ShellHost, args: &str) -> String {
    let session_id = match host.session_manager().current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let n = match crate::tui::slash_handler::utils::parse_optional_count(args, "/redo") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut successes = 0usize;
    let mut last_error = None::<String>;
    for _ in 0..n {
        match host.session_manager().redo_last_edit(session_id) {
            Ok(_) => successes += 1,
            Err(e) => {
                last_error = Some(e.to_string());
                break;
            }
        }
    }

    if successes == 0 {
        format!(
            "Nothing to redo or redo failed{}",
            last_error.map(|e| format!(": {e}")).unwrap_or_default()
        )
    } else {
        format!(
            "Redone {successes} edit{}.",
            if successes > 1 { "s" } else { "" }
        )
    }
}

pub async fn handle_diff(host: &mut dyn ShellHost, args: &str) -> String {
    let trimmed = args.trim();
    let Some(session_id) = host.session_manager().current_session_id() else {
        return "No active session.".to_string();
    };

    if trimmed.is_empty() {
        match host.session_manager().list_edits(session_id) {
            Ok(edits) if edits.is_empty() => {
                return "No edits to diff. Use /diff <file_path> for a specific file.".to_string();
            }
            Ok(edits) => {
                let mut lines = vec!["Recent edits:".to_string()];
                for edit in edits.iter().take(10) {
                    lines.push(format!(
                        "  {} · {} · {}",
                        edit.timestamp, edit.tool_name, edit.file_path
                    ));
                }
                return lines.join("\n");
            }
            Err(e) => return format!("Failed to list edits: {e}"),
        }
    }

    // Try to build a checkpoint diff for the target file.
    match checkpoint_diff_for_target(host, trimmed).await {
        Some((title, content)) => format!("{title}\n{content}"),
        None => "No checkpoint diff available for this file.".to_string(),
    }
}

async fn checkpoint_diff_for_target(
    host: &mut dyn ShellHost,
    target: &str,
) -> Option<(String, String)> {
    let session_id = host.session_manager().current_session_id()?;
    let edits = host.session_manager().list_edits(session_id).ok()?;
    let edit = edits
        .iter()
        .find(|e| e.file_path == target || e.file_path.ends_with(target))?;
    let snapshot = edit.snapshot_path();
    let current = std::fs::read_to_string(&edit.file_path).ok()?;
    let previous = std::fs::read_to_string(snapshot).ok()?;

    let title = format!("Diff for {}", edit.file_path);
    let diff =
        crate::shell::permission_diff::generate_unified_diff(&previous, &current, &edit.file_path)
            .unwrap_or_else(|| "No differences.".to_string());
    Some((title, diff))
}

pub async fn handle_export_data(host: &dyn ShellHost, args: &str) -> String {
    let session_id = match host.session_manager().current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let parts: Vec<&str> = args.split_whitespace().collect();
    let format = parts
        .iter()
        .find(|p| matches!(**p, "json" | "markdown" | "md"))
        .map(|p| match *p {
            "markdown" | "md" => crate::session_store::export::SessionExportFormat::Markdown,
            _ => crate::session_store::export::SessionExportFormat::Json,
        })
        .unwrap_or(crate::session_store::export::SessionExportFormat::Json);
    let privacy = parts
        .iter()
        .find(|p| **p == "&public")
        .map(|_| crate::session_store::export::SessionExportPrivacy::Redacted)
        .unwrap_or(crate::session_store::export::SessionExportPrivacy::Full);

    match host
        .session_manager()
        .write_session_export(session_id, format, privacy)
    {
        Ok(path) => format!("Session exported to {}", path.display()),
        Err(e) => format!("Failed to export session: {e}"),
    }
}

pub async fn handle_save_session(host: &dyn ShellHost) -> String {
    let Some(session_id) = host.session_manager().current_session_id() else {
        return "No active session.".to_string();
    };
    // Session messages and edit history are persisted continuously. /save
    // forces a checkpoint snapshot of the current workspace files so the
    // session can be rewound to this exact state later.
    let mgr = crate::engine::checkpoint::get_checkpoint_manager(session_id).await;
    let mut guard = mgr.lock().await;
    let result = guard
        .create_checkpoint("manual_save", None, None, &[])
        .await;
    drop(guard);
    match result {
        Ok(_) => format!("Session {} saved.", &session_id[..8.min(session_id.len())]),
        Err(e) => format!("Failed to save session: {e}"),
    }
}

pub async fn handle_doctor(host: &dyn ShellHost, args: &str) -> String {
    if args.trim() == "product" {
        return crate::engine::product_readiness::readiness_report();
    }
    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut report = crate::diagnostics::run_full_diagnostics(&working_dir).await;

    let mut registry = crate::tools::ToolRegistry::default_registry();
    let injected =
        crate::tools::plugin_tool::register_enabled_plugin_tools(&mut registry, &working_dir);
    let total_tools = registry.tool_names().len();
    report.checks.push(crate::diagnostics::CheckResult::info(
        "tools",
        format!(
            "{} tools registered ({} plugin runtime injected)",
            total_tools, injected
        ),
    ));

    let context = host.build_tool_context();
    let route = crate::engine::intent_router::IntentRouter::new().route("general coding task");
    let mut available_count = 0usize;
    let mut hidden_by_route = 0usize;
    let mut hidden_by_permission = 0usize;
    let mut unavailable_count = 0usize;
    for tool_name in registry.tool_names() {
        let exposure = crate::engine::tool_exposure::diagnose_tool_exposure(
            &registry, &context, &route, tool_name,
        );
        if exposure.model_exposed {
            available_count += 1;
        } else if !exposure.route_exposed {
            hidden_by_route += 1;
        } else if !exposure.permission_exposed {
            hidden_by_permission += 1;
        } else {
            unavailable_count += 1;
        }
    }
    report.checks.push(crate::diagnostics::CheckResult::info(
        "tool_availability",
        format!(
            "available={} hidden_by_route={} hidden_by_permission={} unavailable={}",
            available_count, hidden_by_route, hidden_by_permission, unavailable_count
        ),
    ));

    let bash_exposure = terminal_bash_exposure_report(host, &registry, &context).await;
    let bash_message =
        crate::tui::slash_handler::agents::doctor_formatting::format_terminal_bash_exposure(
            &bash_exposure,
        );
    if bash_exposure.model_exposed {
        report.checks.push(crate::diagnostics::CheckResult::ok(
            "bash_model_exposure",
            bash_message,
        ));
    } else if !bash_exposure.registered || !bash_exposure.available {
        report.checks.push(crate::diagnostics::CheckResult::error(
            "bash_model_exposure",
            bash_message,
            "Register the bash tool or fix its runtime availability before terminal tasks.",
        ));
    } else {
        report.checks.push(crate::diagnostics::CheckResult::warn(
            "bash_model_exposure",
            bash_message,
            "Check /mode, /permissions mode and rules, or disable route scoped tools only for debugging.",
        ));
    }

    let learning_events = recent_route_learning_events(host);
    report.checks.push(crate::diagnostics::CheckResult::info(
        "agent_mode_route",
        format_current_mode_route_exposure(host, &registry, &context),
    ));
    report.checks.push(crate::diagnostics::CheckResult::info(
        "agent_mode_matrix",
        format_agent_mode_exposure_matrix(&registry, &context, &learning_events),
    ));
    report.checks.push(crate::diagnostics::CheckResult::info(
        "route_tool_schema_cache",
        format_route_tool_schema_cache_matrix(host, &registry, &context, &learning_events),
    ));

    if let Some(engine) = host.engine() {
        report.checks.push(crate::diagnostics::CheckResult::ok(
            "engine",
            format!("model={}", engine.model_name()),
        ));
        report.checks.push(crate::diagnostics::CheckResult::ok(
            "task_manager",
            if engine.task_manager().is_some() {
                "connected"
            } else {
                "missing"
            },
        ));
        report.checks.push(crate::diagnostics::CheckResult::ok(
            "agent_manager",
            if engine.agent_manager().is_some() {
                "connected"
            } else {
                "missing"
            },
        ));

        if let Some(am) = engine.agent_manager() {
            let agents = am.list_agents().await;
            if !agents.is_empty() {
                use std::collections::HashMap;
                let mut role_counts: HashMap<String, usize> = HashMap::new();
                let mut status_counts: HashMap<String, usize> = HashMap::new();
                for handle in &agents {
                    *role_counts
                        .entry(handle.config.role.display_name().to_string())
                        .or_insert(0) += 1;
                    let status_label = format!("{:?}", *handle.status.borrow());
                    *status_counts.entry(status_label).or_insert(0) += 1;
                }
                let role_line = role_counts
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                let status_line = status_counts
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                report.checks.push(crate::diagnostics::CheckResult::info(
                    "agent_roles",
                    format!(
                        "{} agents | roles [{}] | status [{}]",
                        agents.len(),
                        role_line,
                        status_line
                    ),
                ));
            } else {
                report.checks.push(crate::diagnostics::CheckResult::info(
                    "agent_roles",
                    "0 agents active".to_string(),
                ));
            }
        }

        let tracker = engine.cost_tracker().lock().await;
        report.checks.push(crate::diagnostics::CheckResult::info(
            "cost_tracker",
            tracker.tool_diagnostics_line(),
        ));
        report.checks.push(crate::diagnostics::CheckResult::info(
            "prompt_cache",
            crate::tui::slash_handler::agents::doctor_formatting::format_prompt_cache_doctor_line(
                &tracker,
            ),
        ));
        report.checks.push(crate::diagnostics::CheckResult::info(
            "tool_latency",
            tracker.slowest_tools_line(5),
        ));
        report.checks.push(crate::diagnostics::CheckResult::info(
            "tool_failures",
            tracker.top_failure_reasons_line(5),
        ));

        let total_calls: u64 = tracker.tool_metrics.values().map(|s| s.calls).sum();
        let total_success: u64 = tracker.tool_metrics.values().map(|s| s.success).sum();
        let success_rate = if total_calls > 0 {
            (total_success as f64 / total_calls as f64) * 100.0
        } else {
            0.0
        };
        report.checks.push(crate::diagnostics::CheckResult::info(
            "tool_success_rate",
            format!(
                "calls={} success={} success_rate={:.1}%",
                total_calls, total_success, success_rate
            ),
        ));
        report.checks.push(crate::diagnostics::CheckResult::info(
            "coding_quality",
            tracker.coding_quality_detail(),
        ));
        report.checks.push(crate::diagnostics::CheckResult::info(
            "model_usage",
            tracker.model_usage_summary(),
        ));
        report.checks.push(crate::diagnostics::CheckResult::info(
            "token_usage",
            tracker.token_summary(),
        ));

        let p95_lines: Vec<String> = tracker
            .tool_latency_percentiles(5)
            .into_iter()
            .map(|(name, p50, p95, _p99, n)| {
                format!("{}: p50={:.0}ms p95={:.0}ms (n={})", name, p50, p95, n)
            })
            .collect();
        if !p95_lines.is_empty() {
            report.checks.push(crate::diagnostics::CheckResult::info(
                "tool_latency_p95",
                p95_lines.join(", "),
            ));
        }
        report.checks.push(crate::diagnostics::CheckResult::info(
            "tool_quality",
            tracker.tool_quality_ranking(5),
        ));

        if let Some(mem_mgr) = engine.memory_manager() {
            let mem = mem_mgr.lock().await;
            let (hits, misses) = mem.cache_stats();
            let mem_hit_rate = if hits + misses > 0 {
                ((hits as f64) / ((hits + misses) as f64)) * 100.0
            } else {
                0.0
            };
            report.checks.push(crate::diagnostics::CheckResult::info(
                "memory_cache",
                format!(
                    "memory_extraction: hits={} misses={} hit_rate={:.1}%",
                    hits, misses, mem_hit_rate
                ),
            ));
        }

        if let Some(compressor) = engine.compressor() {
            let comp = compressor.lock().await;
            let stats = comp.stats();
            let savings = if stats.total_tokens_before > 0 {
                ((stats.total_tokens_before - stats.total_tokens_after) as f64
                    / stats.total_tokens_before as f64)
                    * 100.0
            } else {
                0.0
            };
            report.checks.push(crate::diagnostics::CheckResult::info(
                "context_compression",
                format!(
                    "compressions={} before={} after={} savings={:.1}% session={}s",
                    stats.compression_count,
                    stats.total_tokens_before,
                    stats.total_tokens_after,
                    savings,
                    stats.session_duration_secs
                ),
            ));
        }
    } else {
        report.checks.push(crate::diagnostics::CheckResult::error(
            "engine",
            "Streaming engine not available",
            "Restart the application or check bootstrap logs",
        ));
    }

    report.checks.push(crate::diagnostics::CheckResult::info(
        "provider_status",
        crate::tui::slash_handler::agents::doctor_formatting::format_provider_status_summary(),
    ));
    report.checks.push(crate::diagnostics::CheckResult::info(
        "effective_config",
        crate::tui::slash_handler::agents::doctor_formatting::format_effective_config_summary(),
    ));

    let runtime = host.runtime_status_snapshot();
    let readiness =
        crate::tui::slash_handler::agents::doctor_formatting::evaluate_product_readiness(
            &report, &runtime,
        );
    report
        .metadata
        .insert("product_ready".to_string(), readiness.ready.to_string());
    report
        .metadata
        .insert("product_readiness".to_string(), readiness.label.to_string());
    report.metadata.insert(
        "product_blockers".to_string(),
        readiness.blockers.len().to_string(),
    );
    report.metadata.insert(
        "product_warnings".to_string(),
        readiness.warnings.len().to_string(),
    );
    report.checks.push(readiness.to_check_result());

    report.overall = if report
        .checks
        .iter()
        .any(|c| c.status == crate::diagnostics::CheckStatus::Error)
    {
        crate::diagnostics::CheckStatus::Error
    } else if report
        .checks
        .iter()
        .any(|c| c.status == crate::diagnostics::CheckStatus::Warning)
    {
        crate::diagnostics::CheckStatus::Warning
    } else {
        crate::diagnostics::CheckStatus::Ok
    };

    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.first() == Some(&"json") {
        report.to_json()
    } else if parts.first() == Some(&"gap") {
        generate_gap_snapshot(host, &report, &registry).await
    } else {
        format!("{}\n\n{}", readiness.format_text(), report.format_text())
    }
}

async fn terminal_bash_exposure_report(
    host: &dyn ShellHost,
    registry: &crate::tools::ToolRegistry,
    context: &crate::tools::ToolContext,
) -> crate::engine::tool_exposure::ToolExposureReport {
    let learning_events = recent_route_learning_events(host);
    let route = route_for_agent_mode_with_learning(
        crate::tui::slash_handler::agents::TERMINAL_EXPOSURE_PROMPT,
        host.agent_mode(),
        &learning_events,
    );
    crate::engine::tool_exposure::diagnose_tool_exposure(registry, context, &route, "bash")
}

fn recent_route_learning_events(
    host: &dyn ShellHost,
) -> Vec<crate::session_store::LearningEventRecord> {
    host.session_manager()
        .recent_learning_events(20)
        .unwrap_or_default()
}

fn route_for_agent_mode_with_learning(
    prompt: &str,
    mode: crate::engine::agent_mode::AgentMode,
    learning_events: &[crate::session_store::LearningEventRecord],
) -> crate::engine::intent_router::IntentRoute {
    let mut route = crate::engine::intent_router::IntentRouter::new()
        .route_with_learning(prompt, learning_events);
    mode.apply_to_route(&mut route);
    route
}

fn format_current_mode_route_exposure(
    host: &dyn ShellHost,
    registry: &crate::tools::ToolRegistry,
    context: &crate::tools::ToolContext,
) -> String {
    let learning_events = recent_route_learning_events(host);
    let route = route_for_agent_mode_with_learning(
        crate::tui::slash_handler::agents::TERMINAL_EXPOSURE_PROMPT,
        host.agent_mode(),
        &learning_events,
    );
    let bash =
        crate::engine::tool_exposure::diagnose_tool_exposure(registry, context, &route, "bash");
    let file_edit = crate::engine::tool_exposure::diagnose_tool_exposure(
        registry,
        context,
        &route,
        "file_edit",
    );
    let file_write = crate::engine::tool_exposure::diagnose_tool_exposure(
        registry,
        context,
        &route,
        "file_write",
    );
    format!(
        "mode={} route={} route_scoped={} bash={} file_edit={} file_write={}",
        host.current_agent_mode_label(),
        route.compact_label(),
        crate::engine::conversation_loop::ConversationLoop::route_scoped_tools_enabled(),
        crate::tui::slash_handler::agents::doctor_formatting::exposure_label(&bash),
        crate::tui::slash_handler::agents::doctor_formatting::exposure_label(&file_edit),
        crate::tui::slash_handler::agents::doctor_formatting::exposure_label(&file_write)
    )
}

fn format_agent_mode_exposure_matrix(
    registry: &crate::tools::ToolRegistry,
    context: &crate::tools::ToolContext,
    learning_events: &[crate::session_store::LearningEventRecord],
) -> String {
    use crate::engine::agent_mode::AgentMode;
    [
        AgentMode::Auto,
        AgentMode::Build,
        AgentMode::Plan,
        AgentMode::Explore,
        AgentMode::Review,
    ]
    .into_iter()
    .map(|mode| {
        let route = route_for_agent_mode_with_learning(
            crate::tui::slash_handler::agents::TERMINAL_EXPOSURE_PROMPT,
            mode,
            learning_events,
        );
        let bash =
            crate::engine::tool_exposure::diagnose_tool_exposure(registry, context, &route, "bash");
        let file_edit = crate::engine::tool_exposure::diagnose_tool_exposure(
            registry,
            context,
            &route,
            "file_edit",
        );
        format!(
            "{}: route={} bash={} file_edit={}",
            mode.label(),
            route.compact_label(),
            crate::tui::slash_handler::agents::doctor_formatting::exposure_label(&bash),
            crate::tui::slash_handler::agents::doctor_formatting::exposure_label(&file_edit)
        )
    })
    .collect::<Vec<_>>()
    .join("; ")
}

fn format_route_tool_schema_cache_matrix(
    host: &dyn ShellHost,
    registry: &crate::tools::ToolRegistry,
    context: &crate::tools::ToolContext,
    learning_events: &[crate::session_store::LearningEventRecord],
) -> String {
    use crate::engine::agent_mode::AgentMode;
    let available_tools = available_provider_tools(registry, context);
    [
        AgentMode::Auto,
        AgentMode::Build,
        AgentMode::Plan,
        AgentMode::Explore,
        AgentMode::Review,
    ]
    .into_iter()
    .map(|mode| {
        let route = route_for_agent_mode_with_learning(
            crate::tui::slash_handler::agents::TERMINAL_EXPOSURE_PROMPT,
            mode,
            learning_events,
        );
        let scoped_tools =
            if crate::engine::conversation_loop::ConversationLoop::route_scoped_tools_enabled() {
                let allowlist =
                    crate::engine::conversation_loop::ConversationLoop::route_tool_allowlist(
                        &route,
                    );
                available_tools
                    .iter()
                    .filter(|tool| allowlist.contains(tool.name.as_str()))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                available_tools.clone()
            };
        let manifest = crate::engine::cache_stability::provider_tool_schema_manifest(&scoped_tools);
        format!(
            "{}:{} tools={} tool_fp={} route={}",
            if mode == host.agent_mode() { "*" } else { "" },
            mode.label(),
            manifest.tool_count,
            crate::tui::slash_handler::agents::doctor_formatting::short_hash(&manifest.fingerprint),
            route.compact_label()
        )
    })
    .collect::<Vec<_>>()
    .join("; ")
}

fn available_provider_tools(
    registry: &crate::tools::ToolRegistry,
    context: &crate::tools::ToolContext,
) -> Vec<crate::services::api::Tool> {
    registry
        .iter_tools()
        .filter(|tool| {
            tool.is_available(context) && context.permission_context.should_expose_tool(tool.name())
        })
        .map(|tool| crate::services::api::Tool {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            parameters: tool.parameters(),
            strict_schema: tool.strict_schema(),
        })
        .collect()
}

async fn generate_gap_snapshot(
    host: &dyn ShellHost,
    report: &crate::diagnostics::DiagnosticReport,
    registry: &crate::tools::ToolRegistry,
) -> String {
    let mut lines = vec![
        "=== Claude Code Gap Snapshot ===".to_string(),
        format!(
            "Generated: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
        "".to_string(),
    ];

    let tool_count = registry.tool_names().len();
    let cmd_count = crate::tui::commands::ALL_COMMANDS.len();
    let engine_ok = host.engine().is_some();
    let model_name = host.engine().map(|e| e.model_name()).unwrap_or_default();

    let tool_gap = if tool_count >= 64 {
        "0".to_string()
    } else {
        format!("-{}", 64i32 - tool_count as i32)
    };
    let cmd_gap = if cmd_count >= 101 {
        "0".to_string()
    } else {
        format!("-{}", 101i32 - cmd_count as i32)
    };

    lines.push("## Dimensions".to_string());
    lines.push("| Dimension | Ours | Claude | Gap |".to_string());
    lines.push("|-----------|------|--------|-----|".to_string());
    lines.push(format!(
        "| Tools     | {}   | 64     | {}  |",
        tool_count, tool_gap
    ));
    lines.push(format!(
        "| Commands  | {}   | 101    | {}  |",
        cmd_count, cmd_gap
    ));
    lines.push(format!(
        "| Engine    | {}   | true   | {}  |",
        engine_ok,
        if engine_ok { "0" } else { "-1" }
    ));
    lines.push(format!(
        "| Model     | {:<4} | any    | 0   |",
        model_name.split('/').next().unwrap_or("none")
    ));
    lines.push("".to_string());
    lines.push("## Diagnostics".to_string());
    for check in &report.checks {
        let icon = match check.status {
            crate::diagnostics::CheckStatus::Ok => "+",
            crate::diagnostics::CheckStatus::Warning => "~",
            crate::diagnostics::CheckStatus::Error => "-",
            crate::diagnostics::CheckStatus::Info => "·",
        };
        lines.push(format!("{} {}: {}", icon, check.name, check.message));
    }
    lines.join("\n")
}

pub async fn handle_audit(host: &dyn ShellHost, args: &str) -> String {
    let Some(engine) = host.engine() else {
        return "No engine available.".to_string();
    };
    let parts: Vec<&str> = args.split_whitespace().collect();
    let sub = parts.first().copied().unwrap_or("summary");

    match sub {
        "summary" => {
            let tracker = engine.cost_tracker().lock().await;
            format!("Token usage summary:\n{}", tracker.generate_report())
        }
        "tools" => {
            let names: Vec<String> = engine
                .tool_registry()
                .tool_names()
                .into_iter()
                .map(|n| n.to_string())
                .collect();
            format!("Registered tools:\n{}", names.join("\n"))
        }
        _ => "Usage: /audit [summary|tools]".to_string(),
    }
}

pub async fn handle_provider(host: &dyn ShellHost, args: &str) -> String {
    let registry = crate::services::api::provider::ProviderRegistry::from_env();
    let trimmed = args.trim();

    if trimmed.is_empty()
        || trimmed == "status"
        || trimmed == "status --json"
        || trimmed == "status json"
    {
        if let Some(engine) = host.engine() {
            format!(
                "Provider: {}\nModel: {}\nBase URL: {}\n\nUse /provider list or /provider switch <name>.",
                provider_label_for_base_url(&engine.provider_base_url()),
                engine.model_name(),
                engine.provider_base_url(),
            )
        } else {
            "No engine available.".to_string()
        }
    } else if trimmed == "list" {
        let statuses = crate::services::api::provider_catalog::provider_status_list();
        if statuses.is_empty() {
            return "No providers configured.".to_string();
        }
        let mut lines = vec!["Providers:".to_string()];
        for s in statuses {
            let marker = if s.configured { "*" } else { "-" };
            lines.push(format!(
                "{} {:<12} {:<12} {}",
                marker,
                s.id,
                s.default_model,
                if s.configured {
                    "configured"
                } else {
                    "not configured"
                }
            ));
        }
        lines.join("\n")
    } else if let Some(name) = trimmed
        .strip_prefix("switch ")
        .or_else(|| trimmed.strip_prefix("set "))
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let name_lower = name.to_ascii_lowercase();
        let provider = registry.get(&name_lower);
        let config = registry.get_config(&name_lower).cloned();
        match (provider, config) {
            (Some(provider), Some(config)) => {
                if let Some(engine) = host.engine() {
                    engine.set_provider(provider, config.default_model.clone());
                }
                if let Ok(mut app_config) = crate::services::config::AppConfig::load() {
                    app_config.api.provider_name = Some(name_lower.clone());
                    app_config.api.model = config.default_model.clone();
                    app_config.api.base_url = config.base_url.clone().unwrap_or_default();
                    if app_config.save().is_ok() {
                        crate::services::config::init_runtime_config(app_config);
                    }
                }
                format!(
                    "Provider switched to {}\nModel: {}\nBase URL: {}",
                    config.name,
                    config.default_model,
                    config.base_url.as_deref().unwrap_or("default")
                )
            }
            _ => format!(
                "Provider '{}' is not configured. Use /provider list to see available providers.",
                name
            ),
        }
    } else {
        "Usage: /provider [list|switch <name>|status]".to_string()
    }
}

pub async fn handle_resume(host: &mut dyn ShellHost, args: &str) -> String {
    if args.is_empty() {
        match host.session_manager().list_resumable_sessions(10) {
            Ok(sessions) => {
                if sessions.is_empty() {
                    "No saved sessions found. Start chatting to create one!".to_string()
                } else {
                    let mut lines = vec!["Recent resumable sessions:".to_string()];
                    for (i, session) in sessions.iter().enumerate() {
                        let title = if session.title.is_empty() {
                            "(untitled)"
                        } else {
                            &session.title
                        };
                        let msg_count = host
                            .session_manager()
                            .message_count(&session.id)
                            .unwrap_or(0);
                        lines.push(format!(
                            "{}. [{}] {} ({} msgs) - {}",
                            i + 1,
                            &session.id[..8.min(session.id.len())],
                            title,
                            msg_count,
                            session.updated_at
                        ));
                    }
                    lines.push(
                        "\nUse /resume <number>, /resume <id>, /resume <search>, or /resume latest."
                            .to_string(),
                    );
                    lines.join("\n")
                }
            }
            Err(e) => format!("Failed to list sessions: {}", e),
        }
    } else {
        match host.session_manager().resolve_resume_selection(args, 40) {
            Ok(Some(session)) => host.restore_session(&session.id).await,
            Ok(None) => {
                "No matching session found. Use /resume without arguments to see recent sessions."
                    .to_string()
            }
            Err(e) => format!("Failed to resolve session: {}", e),
        }
    }
}

pub async fn handle_validate(host: &dyn ShellHost) -> String {
    let Some(session_id) = host.session_manager().current_session_id() else {
        return "No active session.".to_string();
    };
    let sid = session_id.to_string();
    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&sid).await;
    let cp = mgr.lock().await;
    let changes = cp.list_file_changes();
    let rounds = cp.list_file_change_rounds();
    let mut lines = vec![
        "Validation Summary".to_string(),
        "==================".to_string(),
        String::new(),
        format!("File changes: {}", changes.len()),
        format!("Tool rounds: {}", rounds.len()),
        String::new(),
    ];
    if changes.is_empty() {
        lines.push("No file changes to validate.".to_string());
    } else {
        lines.push("Changed files:".to_string());
        for c in changes.iter().rev().take(10) {
            lines.push(format!("  {} ({})", c.path, c.tool_name));
        }
        lines.push(String::new());
        lines.push("Run your test suite to validate changes.".to_string());
        lines.push(
            "Use /diff for details or /changes in --tui for a round-by-round breakdown."
                .to_string(),
        );
    }
    lines.join(
        "
",
    )
}

pub async fn handle_token_cost(engine: &StreamingQueryEngine) -> String {
    let tracker = engine.cost_tracker().lock().await;
    tracker.generate_report()
}

fn provider_label_for_base_url(base_url: &str) -> String {
    let u = base_url.to_ascii_lowercase();
    if u.contains("minimax") {
        "MiniMax".to_string()
    } else if u.contains("api.kimi.com") {
        "Kimi Code".to_string()
    } else if u.contains("moonshot") {
        "Kimi".to_string()
    } else if u.contains("deepseek") {
        "DeepSeek".to_string()
    } else if u.contains("bigmodel") || u.contains("z.ai") {
        "GLM".to_string()
    } else if u.contains("openai.com") {
        "OpenAI".to_string()
    } else {
        "Custom".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_session_returns_message() {
        struct DummyHost;
        impl ShellHost for DummyHost {
            fn engine(
                &self,
            ) -> Option<std::sync::Arc<crate::engine::streaming::StreamingQueryEngine>>
            {
                None
            }
            fn session_manager(&self) -> &crate::tui::session_manager::TuiSessionManager {
                static MANAGER: std::sync::OnceLock<
                    crate::tui::session_manager::TuiSessionManager,
                > = std::sync::OnceLock::new();
                MANAGER.get_or_init(|| {
                    crate::tui::session_manager::TuiSessionManager::in_memory().unwrap()
                })
            }
            fn build_tool_context(&self) -> crate::tools::ToolContext {
                crate::tools::ToolContext::new(std::path::PathBuf::from("."), "test")
            }
            fn restore_session(
                &mut self,
                _session_id: &str,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + '_>>
            {
                Box::pin(async move { String::new() })
            }
            fn show_message(&mut self, _message: String) {}
            fn memory_use(&self) -> bool {
                false
            }
            fn set_memory_use(&mut self, _value: bool) {}
            fn memory_generate(&self) -> bool {
                false
            }
            fn set_memory_generate(&mut self, _value: bool) {}
            fn memory_recall_mode(&self) -> &str {
                ""
            }
            fn set_memory_recall_mode(&mut self, _value: String) {}
        }

        let host = DummyHost;
        assert_eq!(
            futures::executor::block_on(handle_save_session(&host)),
            "No active session."
        );
    }
}

pub async fn handle_status(host: &dyn ShellHost) -> String {
    let mut lines = vec![];
    let msg_count = host.message_count();
    let runtime = host.runtime_status_snapshot();

    lines.push(format!("Messages: {}", msg_count));
    lines.push(format!("Agent mode: {}", host.current_agent_mode_label()));
    lines.push(format!(
        "Runtime tools: {} active / {} total ({} failed)",
        runtime.active_tool_count, runtime.total_tools, runtime.failed_tool_count
    ));
    if let Some(label) = runtime.current_tool_label.as_ref() {
        lines.push(format!("Active tool: {}", label));
    }
    if let Some(pending) = runtime.pending_permission.as_ref() {
        lines.push(format!("Permission pending: {}", pending));
    }

    if let Some(engine) = host.engine() {
        let history_len = engine.get_history().await.len();
        lines.push(format!("History: {} turns", history_len));
        lines.push(format!(
            "Model: {} (via {})",
            engine.model_name(),
            engine.provider_base_url()
        ));

        let tracker = engine.cost_tracker();
        let tracker_guard = tracker.lock().await;
        lines.push(format!(
            "Cost: ${:.4} ({} tokens)",
            tracker_guard.estimated_cost_usd, tracker_guard.total_tokens.total
        ));
        let total_calls: u64 = tracker_guard.tool_metrics.values().map(|s| s.calls).sum();
        let total_failed: u64 = tracker_guard.tool_metrics.values().map(|s| s.failed).sum();
        lines.push(format!(
            "Tools: {} calls ({} failed)",
            total_calls, total_failed
        ));
        drop(tracker_guard);

        if let Some(mcp) = engine.mcp_manager() {
            let diagnostics = mcp.health_diagnostics();
            let available = diagnostics
                .iter()
                .filter(|diag| {
                    diag.approved && diag.health == crate::engine::mcp::McpHealthStatus::Healthy
                })
                .count();
            let needs_repair = diagnostics
                .iter()
                .filter(|diag| diag.repair_hint != "none")
                .map(|diag| format!("{}=>{}", diag.name, diag.repair_hint))
                .collect::<Vec<_>>();
            if diagnostics.is_empty() {
                lines.push("MCP: no servers configured".to_string());
            } else {
                lines.push(format!(
                    "MCP: {} servers, {} available",
                    diagnostics.len(),
                    available
                ));
                if !needs_repair.is_empty() {
                    lines.push(format!("MCP repair: {}", needs_repair.join(", ")));
                }
            }
        }

        let profiles = crate::agent::profiles::load_profiles(host.workspace_root());
        lines.push(format!("Agent profiles: {}", profiles.len()));
        if let Some(skill_runtime) = host.skill_runtime() {
            lines.push(format!("Skills: {}", skill_runtime.len()));
        }

        let mode = engine.permission_mode();
        lines.push(format!("Permission mode: {:?}", mode));
    } else {
        lines.push("Model: unavailable".to_string());
    }

    if runtime.mcp_server_count > 0 {
        lines.push(format!(
            "Runtime MCP: {} servers, {} available",
            runtime.mcp_server_count, runtime.mcp_available_count
        ));
        if !runtime.mcp_repair_hints.is_empty() {
            lines.push(format!(
                "Runtime MCP repair: {}",
                runtime.mcp_repair_hints.join(", ")
            ));
        }
    }
    if runtime.running_task_count > 0 {
        lines.push(format!(
            "Runtime tasks: {} running / {} total",
            runtime.running_task_count, runtime.task_count
        ));
    }
    if runtime.terminal_task_count > 0 || runtime.backgrounded_tool_count > 0 {
        lines.push(format!(
            "Runtime terminal tasks: {} known ({} running, {} pty, {} backgrounded tools)",
            runtime
                .terminal_task_count
                .max(runtime.backgrounded_tool_count),
            runtime
                .running_terminal_task_count
                .max(runtime.backgrounded_tool_count),
            runtime.pty_terminal_task_count,
            runtime.backgrounded_tool_count
        ));
    }

    let registry = crate::tools::ToolRegistry::default_registry();
    let context = host.build_tool_context();
    let bash_exposure = crate::engine::tool_exposure::diagnose_tool_exposure(
        &registry,
        &context,
        &crate::engine::intent_router::IntentRouter::new().route("general coding task"),
        "bash",
    );
    lines.push(format!(
        "Bash exposure: {}",
        crate::tui::slash_handler::agents::doctor_formatting::format_terminal_bash_exposure(
            &bash_exposure
        )
    ));

    lines.push(format!("Querying: {}", host.is_querying()));
    lines.join(
        "
",
    )
}

pub async fn handle_model(host: &dyn ShellHost, args: &str) -> String {
    let Some(engine) = host.engine() else {
        return "Model: unavailable (no engine connected)".to_string();
    };
    let args = args.trim();
    if let Some(model) = args
        .strip_prefix("set ")
        .or_else(|| args.strip_prefix("switch "))
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        engine.set_model(model.to_string());
        if let Ok(mut config) = crate::services::config::AppConfig::load() {
            config.api.model = model.to_string();
            if config.save().is_ok() {
                crate::services::config::init_runtime_config(config);
            }
        }
        format!("Model switched to {}. Next request will use it.", model)
    } else if args == "list" {
        let choices = model_choices(&engine).await;
        let lines = choices
            .into_iter()
            .map(|choice| {
                format!(
                    "{} {} ({})",
                    if choice.active { "*" } else { "-" },
                    choice.model,
                    choice.note
                )
            })
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        format!(
            "Models for {}:
{}",
            engine.provider_base_url(),
            lines
        )
    } else {
        format!(
            "Model: {}
Provider: {}
Base URL: {}

Use /model list or /model switch <name>.",
            engine.model_name(),
            engine.provider_base_url(),
            engine.provider_base_url()
        )
    }
}

#[derive(Debug, Clone)]
struct ModelChoice {
    model: String,
    note: String,
    active: bool,
}

async fn model_choices(
    engine: &crate::engine::streaming::StreamingQueryEngine,
) -> Vec<ModelChoice> {
    let current = engine.model_name();
    let provider_label = provider_label_for_base_url(&engine.provider_base_url());
    let provider_id =
        crate::services::api::provider_catalog::provider_id_for_label(&provider_label)
            .unwrap_or_default();

    let mut model_names: Vec<String> = Vec::new();
    if !provider_id.is_empty() {
        let manifest =
            crate::services::api::provider_manifest::ProviderManifestLoader::load_merged();
        if let Some(entry) = manifest.provider.iter().find(|e| e.id == provider_id) {
            if let Some(api_key) = entry.resolve_api_key() {
                let discovery = crate::services::api::model_discovery::ModelDiscovery::new();
                model_names = discovery
                    .list(&provider_id, entry, Some(&api_key))
                    .await
                    .into_iter()
                    .map(|m| m.id)
                    .collect();
            }
        }
    }

    if model_names.is_empty() {
        model_names = crate::services::api::provider_catalog::supported_models(&provider_id);
        if model_names.is_empty() {
            model_names.push(current.clone());
        }
    }

    let mut models: Vec<&str> = model_names.iter().map(|s| s.as_str()).collect();
    if !models.iter().any(|m| *m == current) {
        models.insert(0, current.as_str());
    }
    models
        .into_iter()
        .map(|model| ModelChoice {
            model: model.to_string(),
            note: if model == current {
                "current".to_string()
            } else {
                "same provider, takes effect next request".to_string()
            },
            active: model == current,
        })
        .collect()
}
