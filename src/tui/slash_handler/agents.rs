// Agent, system, and integration slash command handlers

use super::utils::*;
use crate::engine::agent_mode::AgentMode;
use crate::tools::Tool;
use crate::tui::app::{AppMode, TuiApp};

mod agent_listing;
mod auth_status;
mod doctor_formatting;
mod history_mode;
mod launchers;
mod lsp;
mod npm;
#[cfg(test)]
use agent_listing::format_agent_task_state_lines;
pub use agent_listing::handle_agents;
pub use auth_status::*;
use doctor_formatting::{
    evaluate_product_readiness, exposure_label, format_effective_config_summary,
    format_prompt_cache_doctor_line, format_provider_status_summary, format_terminal_bash_exposure,
    short_hash,
};
pub use history_mode::*;
pub use launchers::*;
pub use lsp::*;
pub use npm::*;

pub async fn handle_status(app: &TuiApp) -> String {
    let msg_count = app.messages.len();
    let runtime = app.runtime_status_snapshot().await;
    let mut lines = vec![];

    // 基本信息
    lines.push(format!("Messages: {}", msg_count));
    lines.push(format!("Agent mode: {}", app.current_agent_mode_label()));
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

    if let Some(ref engine) = app.streaming_engine {
        let history_len = engine.get_history().await.len();
        lines.push(format!("History: {} turns", history_len));

        // 模型信息
        lines.push(format!(
            "Model: {} (via {})",
            app.current_model_label(),
            app.current_provider_label()
        ));

        // 工具统计
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

        // MCP 状态
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

        let profiles = crate::agent::profiles::load_profiles(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
        lines.push(format!("Agent profiles: {}", profiles.len()));
        lines.push(format!("Skills: {}", app.skill_runtime.len()));

        // 权限模式
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

    let bash_exposure = terminal_bash_exposure_report(app).await;
    lines.push(format!(
        "Bash exposure: {}",
        format_terminal_bash_exposure(&bash_exposure)
    ));
    lines.push(terminal_task_status_line(app).await);

    // 查询状态
    lines.push(format!("Querying: {}", app.is_querying));

    lines.join("\n")
}

const TERMINAL_EXPOSURE_PROMPT: &str = "帮我看看我电脑默认的python有没有安装pygame，帮我安装一下吧";

async fn terminal_bash_exposure_report(
    app: &TuiApp,
) -> crate::engine::tool_exposure::ToolExposureReport {
    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut registry = crate::tools::ToolRegistry::default_registry();
    let _ = crate::tools::plugin_tool::register_enabled_plugin_tools(&mut registry, &working_dir);
    let context = app.build_tool_context().await;
    let learning_events = recent_route_learning_events(app);
    let route = route_for_agent_mode_with_learning(
        TERMINAL_EXPOSURE_PROMPT,
        app.agent_mode,
        &learning_events,
    );
    crate::engine::tool_exposure::diagnose_tool_exposure(&registry, &context, &route, "bash")
}

#[cfg(test)]
fn route_for_agent_mode(
    prompt: &str,
    mode: AgentMode,
) -> crate::engine::intent_router::IntentRoute {
    route_for_agent_mode_with_learning(prompt, mode, &[])
}

fn route_for_agent_mode_with_learning(
    prompt: &str,
    mode: AgentMode,
    learning_events: &[crate::session_store::LearningEventRecord],
) -> crate::engine::intent_router::IntentRoute {
    let mut route = crate::engine::intent_router::IntentRouter::new()
        .route_with_learning(prompt, learning_events);
    mode.apply_to_route(&mut route);
    route
}

fn recent_route_learning_events(app: &TuiApp) -> Vec<crate::session_store::LearningEventRecord> {
    app.session_manager
        .recent_learning_events(20)
        .unwrap_or_default()
}

fn format_current_mode_route_exposure(
    app: &TuiApp,
    registry: &crate::tools::ToolRegistry,
    context: &crate::tools::ToolContext,
) -> String {
    let learning_events = recent_route_learning_events(app);
    let route = route_for_agent_mode_with_learning(
        TERMINAL_EXPOSURE_PROMPT,
        app.agent_mode,
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
        app.current_agent_mode_label(),
        route.compact_label(),
        crate::engine::conversation_loop::ConversationLoop::route_scoped_tools_enabled(),
        exposure_label(&bash),
        exposure_label(&file_edit),
        exposure_label(&file_write)
    )
}

fn format_agent_mode_exposure_matrix(
    registry: &crate::tools::ToolRegistry,
    context: &crate::tools::ToolContext,
    learning_events: &[crate::session_store::LearningEventRecord],
) -> String {
    [
        AgentMode::Auto,
        AgentMode::Build,
        AgentMode::Plan,
        AgentMode::Explore,
        AgentMode::Review,
    ]
    .into_iter()
    .map(|mode| {
        let route =
            route_for_agent_mode_with_learning(TERMINAL_EXPOSURE_PROMPT, mode, learning_events);
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
            exposure_label(&bash),
            exposure_label(&file_edit)
        )
    })
    .collect::<Vec<_>>()
    .join("; ")
}

fn format_route_tool_schema_cache_matrix(
    app: &TuiApp,
    registry: &crate::tools::ToolRegistry,
    context: &crate::tools::ToolContext,
    learning_events: &[crate::session_store::LearningEventRecord],
) -> String {
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
        let route =
            route_for_agent_mode_with_learning(TERMINAL_EXPOSURE_PROMPT, mode, learning_events);
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
            if mode == app.agent_mode { "*" } else { "" },
            mode.label(),
            manifest.tool_count,
            short_hash(&manifest.fingerprint),
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

async fn terminal_task_status_line(app: &TuiApp) -> String {
    let context = app.build_tool_context().await;
    let result = crate::tools::bash_tool::BashTasksTool
        .execute(serde_json::json!({}), context)
        .await;
    if !result.success {
        return format!(
            "Terminal tasks: unavailable ({})",
            result
                .error
                .as_deref()
                .filter(|error| !error.trim().is_empty())
                .unwrap_or("bash_tasks failed")
        );
    }
    let tasks = result
        .data
        .as_ref()
        .and_then(|data| data.get("terminal_tasks"))
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    format_terminal_task_status_counts(tasks)
}

fn format_terminal_task_status_counts(tasks: &[serde_json::Value]) -> String {
    if tasks.is_empty() {
        return "Terminal tasks: none".to_string();
    }
    let count_status = |status: &str| -> usize {
        tasks
            .iter()
            .filter(|task| {
                task.get("status")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| value == status)
            })
            .count()
    };
    format!(
        "Terminal tasks: {} known ({} running, {} completed, {} failed, {} cancelled, {} timed out)",
        tasks.len(),
        count_status("running"),
        count_status("completed"),
        count_status("failed"),
        count_status("cancelled"),
        count_status("timed_out")
    )
}

pub async fn handle_tasks(app: &TuiApp) -> String {
    crate::tui::runtime_panels::render_runtime_panel(
        app,
        crate::tui::runtime_panels::RuntimePanelKind::Tasks,
    )
    .await
}
pub async fn handle_doctor(app: &TuiApp, args: &str) -> String {
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

    // Tool availability check
    let context = app.build_tool_context().await;
    let route = crate::engine::intent_router::IntentRouter::new().route("general coding task");
    let mut available_count = 0;
    let mut hidden_by_route = 0;
    let mut hidden_by_permission = 0;
    let mut unavailable_count = 0;
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

    let bash_exposure = terminal_bash_exposure_report(app).await;
    let bash_message = format_terminal_bash_exposure(&bash_exposure);
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
    let context = app.build_tool_context().await;
    let learning_events = recent_route_learning_events(app);
    report.checks.push(crate::diagnostics::CheckResult::info(
        "agent_mode_route",
        format_current_mode_route_exposure(app, &registry, &context),
    ));
    report.checks.push(crate::diagnostics::CheckResult::info(
        "agent_mode_matrix",
        format_agent_mode_exposure_matrix(&registry, &context, &learning_events),
    ));
    report.checks.push(crate::diagnostics::CheckResult::info(
        "route_tool_schema_cache",
        format_route_tool_schema_cache_matrix(app, &registry, &context, &learning_events),
    ));

    if let Some(ref engine) = app.streaming_engine {
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

        if let Some(ref am) = engine.agent_manager() {
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
            format_prompt_cache_doctor_line(&tracker),
        ));

        // W4-2: Performance panel - tool P95 latency
        report.checks.push(crate::diagnostics::CheckResult::info(
            "tool_latency",
            tracker.slowest_tools_line(5),
        ));

        // W4-2: Failure reasons
        report.checks.push(crate::diagnostics::CheckResult::info(
            "tool_failures",
            tracker.top_failure_reasons_line(5),
        ));

        // W4-2: Cache hit rate (tool result cache - if available via executor)
        // Note: ToolRegistry doesn't have cache by default, only CachedToolExecutor does
        // Report tool call efficiency from cost_tracker instead
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

        // W4-2: Coding quality metrics
        report.checks.push(crate::diagnostics::CheckResult::info(
            "coding_quality",
            tracker.coding_quality_detail(),
        ));

        // W4-2: Model usage
        report.checks.push(crate::diagnostics::CheckResult::info(
            "model_usage",
            tracker.model_usage_summary(),
        ));

        // W4-2: Token summary
        report.checks.push(crate::diagnostics::CheckResult::info(
            "token_usage",
            tracker.token_summary(),
        ));

        // W4-2: Tool latency percentiles (P95)
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

        // W4-2: Tool quality ranking
        report.checks.push(crate::diagnostics::CheckResult::info(
            "tool_quality",
            tracker.tool_quality_ranking(5),
        ));

        // W4-2: Memory extraction stats (if available)
        if let Some(ref mem_mgr) = engine.memory_manager() {
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

        // W4-2: Context compression stats
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

    // Provider status summary
    report.checks.push(crate::diagnostics::CheckResult::info(
        "provider_status",
        format_provider_status_summary(),
    ));

    // Effective config summary
    report.checks.push(crate::diagnostics::CheckResult::info(
        "effective_config",
        format_effective_config_summary(),
    ));

    let runtime = app.runtime_status_snapshot().await;
    let readiness = evaluate_product_readiness(&report, &runtime);
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
        // W4-3: Generate a live gap snapshot based on current implementation
        generate_gap_snapshot(app, &report).await
    } else {
        format!("{}\n\n{}", readiness.format_text(), report.format_text())
    }
}
/// Generate a live gap snapshot (W4-3)
async fn generate_gap_snapshot(
    app: &TuiApp,
    report: &crate::diagnostics::DiagnosticReport,
) -> String {
    let mut lines = vec![
        "=== Claude Code Gap Snapshot ===".to_string(),
        format!(
            "Generated: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
        "".to_string(),
    ];

    // Count tools from registry
    let mut registry = crate::tools::ToolRegistry::default_registry();
    let _injected = crate::tools::plugin_tool::register_enabled_plugin_tools(
        &mut registry,
        &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    );
    let tool_count = registry.tool_names().len();

    // Count commands
    let cmd_count = crate::tui::commands::ALL_COMMANDS.len();

    // Engine status
    let engine_ok = app.streaming_engine.is_some();
    let model_name = app
        .streaming_engine
        .as_ref()
        .map(|e| e.model_name())
        .unwrap_or_default();

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
    lines.push("| Agents    | 7    | 7      | 0   |".to_string());
    lines.push("| Transport | 3    | 3      | 0   |".to_string());
    lines.push("| Frontend  | 2    | 4      | -2  |".to_string());
    lines.push("".to_string());

    // Performance snapshot
    lines.push("## Performance Snapshot".to_string());
    for check in &report.checks {
        if matches!(
            check.name.as_str(),
            "tool_latency_p95"
                | "tool_success_rate"
                | "coding_quality"
                | "context_compression"
                | "memory_cache"
        ) {
            lines.push(format!("- {}: {}", check.name, check.message));
        }
    }
    lines.push("".to_string());

    // Quick assessment
    lines.push("## Quick Assessment".to_string());
    if engine_ok {
        lines.push(format!("- Engine: OK (model={})", model_name));
    } else {
        lines.push("- Engine: NOT AVAILABLE".to_string());
    }
    lines.push(format!("- Overall diagnostics: {:?}", report.overall));
    lines.push("".to_string());

    lines.push("Run `/doctor json` for full JSON report.".to_string());
    lines.join("\n")
}
pub async fn handle_audit(app: &TuiApp, args: &str) -> String {
    if let Some(engine) = app.streaming_engine.as_ref() {
        let mut parts = args.split_whitespace();
        let sub = parts.next().unwrap_or("summary");

        if matches!(sub, "tools" | "tool-contracts" | "contracts") {
            let show_all = parts.any(|part| matches!(part, "all" | "--all"));
            let profiles = engine.tool_registry().reliability_audit();
            let total = profiles.len();
            let errors = profiles
                .iter()
                .flat_map(|profile| profile.issues.iter())
                .filter(|issue| {
                    issue.severity == crate::tools::reliability::ToolReliabilityIssueSeverity::Error
                })
                .count();
            let warnings = profiles
                .iter()
                .flat_map(|profile| profile.issues.iter())
                .filter(|issue| {
                    issue.severity
                        == crate::tools::reliability::ToolReliabilityIssueSeverity::Warning
                })
                .count();
            let mut lines = vec![
                format!(
                    "Tool reliability audit: profiles={} errors={} warnings={}",
                    total, errors, warnings
                ),
                "Release gate: hard errors must be zero.".to_string(),
            ];

            for profile in profiles
                .iter()
                .filter(|profile| show_all || !profile.issues.is_empty())
            {
                let issue_summary = if profile.issues.is_empty() {
                    "ok".to_string()
                } else {
                    profile
                        .issues
                        .iter()
                        .map(|issue| {
                            let severity = match issue.severity {
                                crate::tools::reliability::ToolReliabilityIssueSeverity::Warning => {
                                    "warn"
                                }
                                crate::tools::reliability::ToolReliabilityIssueSeverity::Error => {
                                    "error"
                                }
                            };
                            format!("{}:{}={}", severity, issue.field, issue.message)
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                };
                lines.push(format!(
                    "- {}:{} kind={:?} read_only={} concurrent={} ui={:?} {}",
                    profile.tool_name,
                    profile.sample_label,
                    profile.operation_kind,
                    profile.read_only,
                    profile.concurrency_safe,
                    profile.ui_render_kind,
                    issue_summary
                ));
            }

            if lines.len() == 2 {
                lines.push("No tool reliability issues found.".to_string());
            } else if !show_all {
                lines.push("Use `/audit tools all` to include clean profiles.".to_string());
            }

            return lines.join("\n");
        }

        let tracker = engine.cost_tracker().lock().await;

        match sub {
            "summary" => {
                let lines = [
                    tracker.tool_diagnostics_line(),
                    tracker.slowest_tools_line(5),
                    tracker.top_failure_reasons_line(5),
                    tracker.coding_quality_line(),
                    format!("tool_recent_events: {}", tracker.recent_tool_event_count()),
                ];
                lines.join("\n")
            }
            "recent" => {
                let limit = parts
                    .next()
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(20)
                    .clamp(1, 200);
                let events = tracker.recent_tool_events(limit);
                if events.is_empty() {
                    "No recent tool events.".to_string()
                } else {
                    let mut lines = vec![format!("Recent tool events ({}):", events.len())];
                    for e in events {
                        lines.push(format!(
                            "- ts={} tool={} ok={} duration_ms={} reason={}",
                            e.timestamp_ms,
                            e.tool_name,
                            e.success,
                            e.duration_ms,
                            e.failure_reason.unwrap_or_else(|| "-".to_string())
                        ));
                    }
                    lines.join("\n")
                }
            }
            "export" => {
                let session_id = app
                    .session_manager
                    .current_session_id()
                    .map(|s| s.to_string());
                let content = tracker.export_audit_snapshot_json(session_id.as_deref(), 200);
                drop(tracker);

                let path = if let Some(arg_path) = parts.next() {
                    std::path::PathBuf::from(arg_path)
                } else {
                    let sid = session_id.unwrap_or_else(|| "unknown".to_string());
                    let sid_short = &sid[..8.min(sid.len())];
                    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
                    dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join(".priority-agent")
                        .join(format!("audit_{}_{}.json", sid_short, ts))
                };

                if let Some(parent) = path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                match tokio::fs::write(&path, content).await {
                    Ok(_) => format!("Audit snapshot exported: {}", path.display()),
                    Err(e) => format!("Failed to export audit snapshot: {}", e),
                }
            }
            _ => "Usage: /audit [summary|recent <n>|tools [all]|export [path]]".to_string(),
        }
    } else {
        "Audit unavailable (no engine connected).".to_string()
    }
}

fn format_mcp_repair_plan(diagnostics: &[crate::engine::mcp::McpServerHealth]) -> String {
    if diagnostics.is_empty() {
        return "MCP Repair Plan\n- no servers configured".to_string();
    }

    let mut lines = vec!["MCP Repair Plan".to_string()];
    let mut actionable = 0usize;
    for diag in diagnostics {
        if diag.repair_hint == "none" {
            lines.push(format!(
                "- {} [{:?}] healthy enough; no repair needed",
                diag.name, diag.health
            ));
            continue;
        }
        actionable += 1;
        let kind = if diag.repair_hint.starts_with("/mcp approve ") {
            "approval"
        } else if diag.repair_hint.starts_with("/mcp auth ") {
            "auth"
        } else if diag.repair_hint.starts_with("/mcp repair ") {
            "circuit"
        } else {
            "manual"
        };
        lines.push(format!(
            "- {} [{:?}] {} repair: {}",
            diag.name, diag.health, kind, diag.repair_hint
        ));
    }

    if actionable == 0 {
        lines.push(
            "All configured MCP servers are healthy or have no known repair action.".to_string(),
        );
    } else {
        lines.push("Use /mcp repair --all to apply only circuit-breaker repairs; approvals and OAuth remain explicit.".to_string());
    }
    lines.join("\n")
}

fn mcp_circuit_repair_targets(diagnostics: &[crate::engine::mcp::McpServerHealth]) -> Vec<String> {
    diagnostics
        .iter()
        .filter(|diag| diag.repair_hint == format!("/mcp repair {}", diag.name))
        .map(|diag| diag.name.clone())
        .collect()
}

fn format_mcp_repair_all_result(
    diagnostics: &[crate::engine::mcp::McpServerHealth],
    manager: &crate::engine::mcp::McpManager,
) -> String {
    let targets = mcp_circuit_repair_targets(diagnostics);
    let mut lines = vec!["MCP repair --all".to_string()];
    if targets.is_empty() {
        lines.push("No circuit-breaker repairs to apply.".to_string());
    } else {
        lines.push("Applied circuit-breaker repairs:".to_string());
        for target in targets {
            match manager.repair_server(&target) {
                Ok(message) => lines.push(format!("- {}", message)),
                Err(err) => lines.push(format!("- {}: failed: {}", target, err)),
            }
        }
    }

    let skipped = diagnostics
        .iter()
        .filter(|diag| {
            diag.repair_hint != "none" && diag.repair_hint != format!("/mcp repair {}", diag.name)
        })
        .collect::<Vec<_>>();
    if !skipped.is_empty() {
        lines.push("Skipped explicit repairs:".to_string());
        for diag in skipped {
            lines.push(format!("- {} -> {}", diag.name, diag.repair_hint));
        }
    }
    lines.join("\n")
}
pub async fn handle_mcp(app: &TuiApp, args: &str) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts
        .first()
        .is_some_and(|part| matches!(*part, "status" | "health"))
    {
        return crate::tui::runtime_panels::render_runtime_panel(
            app,
            crate::tui::runtime_panels::RuntimePanelKind::Mcp,
        )
        .await;
    }

    if let Some(ref engine) = app.streaming_engine {
        if let Some(mgr) = engine.mcp_manager() {
            if parts.is_empty() || parts[0] == "list" {
                let servers = mgr.server_summaries();
                let approved = mgr.approved_server_names();
                if servers.is_empty() {
                    "No MCP servers configured.".to_string()
                } else {
                    format!(
                        "MCP servers ({}):\n{}\n\nApproved: {}\n\nUsage:\n  /mcp status\n  /mcp prompts\n  /mcp resources [server]\n  /mcp read <server> <uri>\n  /mcp auth <server>\n  /mcp repair [server|--all]\n  /mcp approve <server>\n  /mcp revoke <server>",
                        servers.len(),
                        servers.join("\n"),
                        if approved.is_empty() {
                            "none".to_string()
                        } else {
                            approved.join(", ")
                        }
                    )
                }
            } else if parts[0] == "prompts" {
                let prompts = mgr.discover_all_prompts().await;
                if prompts.is_empty() {
                    "No MCP prompts available from approved servers.".to_string()
                } else {
                    let lines = prompts
                        .iter()
                        .map(|prompt| {
                            let desc = if prompt.description.is_empty() {
                                "(no description)"
                            } else {
                                &prompt.description
                            };
                            format!("- /mcp__{}__{}: {}", prompt.server_name, prompt.name, desc)
                        })
                        .collect::<Vec<_>>();
                    format!("MCP prompts ({}):\n{}", lines.len(), lines.join("\n"))
                }
            } else if matches!(parts[0], "resources" | "list_resources" | "list-resources") {
                let params = if parts.len() >= 2 {
                    serde_json::json!({ "server_name": parts[1] })
                } else {
                    serde_json::json!({})
                };
                let result = crate::tools::ListMcpResourcesTool
                    .execute(params, app.build_tool_context().await)
                    .await;
                if result.success {
                    result.content
                } else {
                    result
                        .error
                        .unwrap_or_else(|| "Failed to list MCP resources.".to_string())
                }
            } else if parts[0] == "read" && parts.len() >= 3 {
                let params = serde_json::json!({
                    "server_name": parts[1],
                    "uri": parts[2],
                });
                let result = crate::tools::ReadMcpResourceTool
                    .execute(params, app.build_tool_context().await)
                    .await;
                if result.success {
                    result.content
                } else {
                    result
                        .error
                        .unwrap_or_else(|| "Failed to read MCP resource.".to_string())
                }
            } else if parts[0] == "approve" && parts.len() >= 2 {
                let name = parts[1];
                if mgr.server_names().contains(&name.to_string()) {
                    mgr.approve_server(name);
                    format!("MCP server '{}' approved.", name)
                } else {
                    format!(
                        "MCP server '{}' not found. Configured servers: {}",
                        name,
                        mgr.server_names().join(", ")
                    )
                }
            } else if parts[0] == "revoke" && parts.len() >= 2 {
                let name = parts[1];
                mgr.revoke_server(name);
                format!("MCP server '{}' approval revoked.", name)
            } else if parts[0] == "auth" && parts.len() >= 2 {
                let name = parts[1];
                match mgr.authenticate_server(name).await {
                    Ok(()) => format!("MCP server '{}' authenticated.", name),
                    Err(e) => format!(
                        "MCP auth failed for '{}': {}\nCheck OAuth config, then retry with /mcp auth {}.",
                        name, e, name
                    ),
                }
            } else if parts[0] == "repair" {
                if parts.len() == 1 {
                    format_mcp_repair_plan(&mgr.health_diagnostics())
                } else if matches!(parts[1], "--all" | "all") {
                    format_mcp_repair_all_result(&mgr.health_diagnostics(), &mgr)
                } else {
                    let name = parts[1];
                    match mgr.repair_server(name) {
                        Ok(msg) => msg,
                        Err(e) => format!("MCP repair failed for '{}': {}", name, e),
                    }
                }
            } else {
                "Usage: /mcp [list|status|prompts|resources [server]|read <server> <uri>|auth <server>|repair [server|--all]|approve <server>|revoke <server>]".to_string()
            }
        } else {
            "No MCP manager configured.".to_string()
        }
    } else {
        "Engine not initialized.".to_string()
    }
}
pub fn handle_voice() -> String {
    #[cfg(not(feature = "voice"))]
    {
        "Voice module is not enabled in this build. Rebuild with `--features voice` to use TTS/STT."
            .to_string()
    }

    #[cfg(feature = "voice")]
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let vm = crate::voice::VoiceManager::new();
        let tts = handle.block_on(vm.tts_available());
        let stt = handle.block_on(vm.stt_available());
        let tts_cmd = if cfg!(target_os = "macos") {
            "say"
        } else if cfg!(target_os = "linux") {
            "espeak/spd-say"
        } else {
            "PowerShell"
        };
        let stt_cmd = "whisper";
        format!(
            "Voice Module Status:\n  TTS ({}): {} — {}\n  STT ({}): {} — {}\n\nUse the `voice` tool with action=speak/transcribe/status.",
            vm.tts_name(),
            if tts { "available" } else { "not available" },
            tts_cmd,
            vm.stt_name(),
            if stt { "available" } else { "not available" },
            stt_cmd,
        )
    } else {
        "Voice module loaded. Run with tokio runtime for status check.".to_string()
    }
}
pub fn handle_telemetry() -> String {
    let collector = crate::telemetry::TelemetryCollector::new();
    let consent = collector.consent();
    let enabled = collector.is_enabled();
    let data = collector.summary();
    format!(
        "Telemetry Status:\n  Consent: {:?}\n  Enabled: {}\n  Recorded sessions: {}\n\nSet PRIORITY_AGENT_TELEMETRY=enabled to start collecting.\nUse the `telemetry` tool for detailed summary/export.",
        consent, enabled, data.total_sessions
    )
}
/// /btw -随口说一句（one-off 注释，不影响对话）
pub async fn handle_btw(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /btw <message> - Add a side note without disrupting the conversation"
            .to_string();
    }
    let note = format!("[btw] {}", args);
    app.add_system_message(note.clone());
    String::new()
}
/// /context - 显示当前上下文状态
pub async fn handle_context(app: &TuiApp) -> String {
    let mut lines = vec![crate::tui::runtime_panels::render_context_panel(app).await];

    if let Some(ref engine) = app.streaming_engine {
        let usage = engine.context_usage_report().await;

        lines.push("".to_string());
        lines.push("## Request Budget Detail".to_string());
        lines.push(format!("History turns: {}", usage.history_messages));
        lines.push(format!(
            "  System prompt: {} tokens ({} chars, hash {})",
            usage.prompt.total_tokens, usage.prompt.total_chars, usage.prompt.fingerprint
        ));
        for layer in &usage.prompt.layers {
            lines.push(format!(
                "    - {}: {} tokens, {} chars",
                layer.name, layer.tokens, layer.chars
            ));
        }
        lines.push(format!(
            "  Conversation history: {} tokens ({} messages)",
            usage.history_tokens, usage.history_messages
        ));
        lines.push(format!(
            "  Tool schemas: {} tokens ({} tools)",
            usage.tool_schema_tokens, usage.tool_count
        ));
        lines.push(format!(
            "  Memory snapshot: {} tokens",
            usage.memory_snapshot_tokens
        ));
        if !usage.relevant_memories.is_empty() {
            lines.push("".to_string());
            lines.push("## Relevant Memory Preview".to_string());
            for memory in &usage.relevant_memories {
                let snippet = memory
                    .snippet
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" ");
                let snippet = snippet.chars().take(180).collect::<String>();
                lines.push(format!(
                    "  - {} (score {}): {}",
                    memory.source, memory.score, snippet
                ));
            }
        }

        // 压缩器状态
        if let Some(compressor_arc) = engine.compressor() {
            let comp = compressor_arc.lock().await;
            let stats = comp.stats();

            lines.push("".to_string());
            lines.push("## Compression".to_string());
            lines.push(format!("  Compression count: {}", stats.compression_count));
            lines.push(format!(
                "  Total tokens before: {}",
                stats.total_tokens_before
            ));
            lines.push(format!(
                "  Total tokens after: {}",
                stats.total_tokens_after
            ));
            if stats.total_tokens_before > 0 {
                let savings = (stats.total_tokens_before - stats.total_tokens_after) * 100
                    / stats.total_tokens_before;
                lines.push(format!("  Overall savings: {}%", savings));
            }
            lines.push(format!(
                "  LLM attempts: {} (failures: {})",
                stats.llm_compression_attempts, stats.llm_compression_failures
            ));

            // 压缩历史
            let history = comp.compact_metadata_history();
            if !history.is_empty() {
                lines.push("".to_string());
                lines.push("## Compression History".to_string());
                for meta in history.iter().rev().take(5) {
                    lines.push(format!(
                        "  #{}: {} msgs -> {} msgs ({} -> {} tokens)",
                        meta.sequence,
                        meta.messages_before,
                        meta.messages_after,
                        meta.tokens_before,
                        meta.tokens_after
                    ));
                }
            }

            // 累积摘要
            if let Some(summary) = comp.accumulated_summary() {
                if !summary.is_empty() {
                    lines.push("".to_string());
                    lines.push("## Accumulated Summary".to_string());
                    if !summary.goal.is_empty() {
                        lines.push(format!("  Goal: {}", summary.goal));
                    }
                    if !summary.progress_done.is_empty() {
                        lines.push(format!("  Done: {}", summary.progress_done.join(", ")));
                    }
                    if !summary.files_modified.is_empty() {
                        lines.push(format!("  Files: {}", summary.files_modified.join(", ")));
                    }
                    if !summary.next_steps.is_empty() {
                        lines.push(format!("  Next: {}", summary.next_steps.join(", ")));
                    }
                }
            }
        }
    } else {
        lines.push("Engine not initialized".to_string());
    }

    lines.join("\n")
}
/// /git - 内联 Git 操作
pub async fn handle_git(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::GitTool;

    // Validate git action to prevent arbitrary command injection
    let allowed_actions = [
        "status", "diff", "log", "branch", "checkout", "stash", "tag",
    ];
    let action = if args.is_empty() {
        "status".to_string()
    } else {
        let first_word = args.split_whitespace().next().unwrap_or("");
        if !allowed_actions.contains(&first_word) {
            return format!(
                "Git action '{}' is not allowed via /git command.\nAllowed actions: {}\nUse /bash for other git commands.",
                first_word,
                allowed_actions.join(", ")
            );
        }
        args.to_string()
    };

    let params = serde_json::json!({ "action": action });
    let result = tool.execute(params, app.build_tool_context().await).await;
    if result.success {
        result.content
    } else {
        result
            .error
            .unwrap_or_else(|| "Git command failed".to_string())
    }
}

/// /package - 包管理相关操作
pub async fn handle_package(app: &mut TuiApp, args: &str) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let action = parts.first().copied().unwrap_or("help");

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    match action {
        "list" => {
            // List available package files
            let params = serde_json::json!({
                "command": r#"find . -name "package.json" -o -name "Cargo.toml" -o -name "go.mod" -o -name "pyproject.toml" -o -name "Gemfile" 2>/dev/null | head -20"#,
                "description": "Find package files"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                format!("Found package files:\n\n{}", result.content)
            } else {
                "No package files found in current directory.".to_string()
            }
        }
        "deps" => {
            // Show dependencies for detected package manager
            let params = serde_json::json!({
                "command": r#"if [ -f "package.json" ]; then npm ls --depth=0 2>/dev/null || echo "npm not available"; elif [ -f "Cargo.toml" ]; then cargo tree --depth=1 2>/dev/null || echo "cargo tree not available"; elif [ -f "go.mod" ]; then go list -m all 2>/dev/null || echo "go not available"; else echo "No recognized package file found"; fi"#,
                "description": "List dependencies"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                format!("Dependencies:\n\n{}", result.content)
            } else {
                result
                    .error
                    .unwrap_or_else(|| "Failed to list dependencies.".to_string())
            }
        }
        "outdated" => {
            let params = serde_json::json!({
                "command": r#"if [ -f "package.json" ]; then npm outdated 2>/dev/null || echo "npm outdated not available"; elif [ -f "Cargo.toml" ]; then cargo outdated --depth=1 2>/dev/null || echo "cargo outdated not available"; else echo "No recognized package file with outdated check"; fi"#,
                "description": "Check outdated packages"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                format!("Outdated packages:\n\n{}", result.content)
            } else {
                result
                    .error
                    .unwrap_or_else(|| "Failed to check outdated packages.".to_string())
            }
        }
        _ => "Package Manager Commands:\n\n\
                 /package list     - List package files in project\n\
                 /package deps     - Show installed dependencies\n\
                 /package outdated - Check for outdated packages\n\n\
                 Supported: npm (Node.js), cargo (Rust), go (Go)"
            .to_string(),
    }
}

#[cfg(test)]
mod tests;
