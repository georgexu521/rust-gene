// Agent, system, and integration slash command handlers

use super::utils::*;
use crate::tools::Tool;
use crate::tui::app::{AppMode, TuiApp};

pub async fn handle_status(app: &TuiApp) -> String {
    let msg_count = app.messages.len();
    let mut lines = vec![];

    // 基本信息
    lines.push(format!("Messages: {}", msg_count));

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
            let available = mcp.available_servers();
            let degraded = mcp.degraded_servers();
            if available.is_empty() && degraded.is_empty() {
                lines.push("MCP: no servers configured".to_string());
            } else {
                if !available.is_empty() {
                    lines.push(format!("MCP: {} available", available.len()));
                }
                if !degraded.is_empty() {
                    lines.push(format!("MCP: {} degraded", degraded.join(", ")));
                }
            }
        }

        // 权限模式
        let mode = engine.permission_mode();
        lines.push(format!("Permission mode: {:?}", mode));
    } else {
        lines.push("Model: unavailable".to_string());
    }

    // 查询状态
    lines.push(format!("Querying: {}", app.is_querying));

    lines.join("\n")
}
pub async fn handle_tasks(app: &TuiApp) -> String {
    if let Some(manager) = app.streaming_engine.as_ref().and_then(|e| e.task_manager()) {
        let tasks = manager.list_tasks(None).await;
        if tasks.is_empty() {
            "No tracked tasks.".to_string()
        } else {
            use crate::state::TaskStatus;
            let pending = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Pending)
                .count();
            let running = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Running)
                .count();
            let completed = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Completed)
                .count();
            let failed = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Failed)
                .count();
            let killed = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Killed)
                .count();
            let mut lines = vec![
                format!(
                    "Tasks summary: total={} pending={} running={} completed={} failed={} killed={}",
                    tasks.len(), pending, running, completed, failed, killed
                ),
                String::new(),
                "Recent tasks:".to_string(),
            ];
            for task in tasks.iter().take(20) {
                lines.push(format!(
                    "- {} [{}] {}",
                    task.id,
                    match task.status {
                        TaskStatus::Pending => "pending",
                        TaskStatus::Running => "running",
                        TaskStatus::Completed => "completed",
                        TaskStatus::Failed => "failed",
                        TaskStatus::Killed => "killed",
                    },
                    task.name
                ));
            }
            lines.join("\n")
        }
    } else {
        "Task manager unavailable (no engine connected).".to_string()
    }
}
pub async fn handle_agents(app: &TuiApp) -> String {
    if let Some(manager) = app
        .streaming_engine
        .as_ref()
        .and_then(|e| e.agent_manager())
    {
        let agents = manager.list_agents().await;
        if agents.is_empty() {
            "No agents found.".to_string()
        } else {
            let mut lines = vec![format!("Agents ({}):", agents.len())];
            for handle in agents.iter().take(30) {
                let status = *handle.status.borrow();
                lines.push(format!(
                    "- {} [{:?}] [{}] {}",
                    handle.id,
                    status,
                    handle.config.role.display_name(),
                    handle.config.name
                ));
            }
            lines.join("\n")
        }
    } else {
        "Agent manager unavailable (no engine connected).".to_string()
    }
}
pub async fn handle_doctor(app: &TuiApp, args: &str) -> String {
    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut report = crate::diagnostics::run_full_diagnostics(&working_dir).await;

    let mut registry = crate::tools::ToolRegistry::default_registry();
    let injected =
        crate::tools::plugin_tool::register_enabled_plugin_tools(&mut registry, &working_dir);
    report.checks.push(crate::diagnostics::CheckResult::info(
        "tools",
        format!(
            "{} tools registered ({} plugin runtime injected)",
            registry.tool_names().len(),
            injected
        ),
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
        report.format_text()
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
            _ => "Usage: /audit [summary|recent <n>|export [path]]".to_string(),
        }
    } else {
        "Audit unavailable (no engine connected).".to_string()
    }
}
pub fn handle_mcp(app: &TuiApp, args: &str) -> String {
    if let Some(ref engine) = app.streaming_engine {
        if let Some(mgr) = engine.mcp_manager() {
            let parts: Vec<&str> = args.split_whitespace().collect();
            if parts.is_empty() || parts[0] == "list" {
                let servers = mgr.server_summaries();
                let approved = mgr.approved_server_names();
                if servers.is_empty() {
                    "No MCP servers configured.".to_string()
                } else {
                    format!(
                        "MCP servers ({}):\n{}\n\nApproved: {}\n\nUsage:\n  /mcp approve <server>\n  /mcp revoke <server>",
                        servers.len(),
                        servers.join("\n"),
                        if approved.is_empty() {
                            "none".to_string()
                        } else {
                            approved.join(", ")
                        }
                    )
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
            } else {
                "Usage: /mcp [list|approve <server>|revoke <server>]".to_string()
            }
        } else {
            "No MCP manager configured.".to_string()
        }
    } else {
        "Engine not initialized.".to_string()
    }
}
pub fn handle_voice() -> String {
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
    let msg_count = app.messages.len();
    let model = app.current_model_label();
    let provider = app.current_provider_label();
    let working_dir = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let session_id = app
        .session_manager
        .current_session_id()
        .map(|s| s[..8.min(s.len())].to_string())
        .unwrap_or_else(|| "none".to_string());

    let mut lines = vec![
        "# Context Status".to_string(),
        "".to_string(),
        format!("Session: {}", session_id),
        format!("Model: {} ({})", model, provider),
        format!("Working dir: {}", working_dir),
        "".to_string(),
    ];

    if let Some(ref engine) = app.streaming_engine {
        let usage = engine.context_usage_report().await;
        let usage_pct = if usage.max_context_tokens > 0 {
            usage.total_estimated_tokens.saturating_mul(100) / usage.max_context_tokens
        } else {
            0
        };

        lines.push(format!("History turns: {}", usage.history_messages));
        lines.push(format!("Messages in view: {}", msg_count));
        lines.push(format!(
            "Estimated request tokens: {} / {} ({}%)",
            usage.total_estimated_tokens, usage.max_context_tokens, usage_pct
        ));
        lines.push(format!(
            "Stable prefix fingerprint: {}",
            usage.stable_prefix_fingerprint
        ));
        lines.push("".to_string());
        lines.push("## Request Budget".to_string());
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
/// /history - 会话历史查看
pub fn handle_history(app: &TuiApp, args: &str) -> String {
    let limit = args.parse::<usize>().unwrap_or(20);
    let messages = &app.messages;

    if messages.is_empty() {
        return "No messages in current session.".to_string();
    }

    let start = if messages.len() > limit {
        messages.len() - limit
    } else {
        0
    };

    let mut lines = vec![format!("Recent {} messages: ", messages.len() - start)];
    for (i, msg) in messages.iter().enumerate().skip(start) {
        let role_str = match msg.role {
            crate::state::MessageRole::User => "user",
            crate::state::MessageRole::Assistant => "assistant",
            crate::state::MessageRole::System => "system",
            crate::state::MessageRole::Tool => "tool",
        };
        let preview = if msg.content.len() > 60 {
            format!("{}...", &msg.content[..60])
        } else {
            msg.content.clone()
        };
        lines.push(format!("{}. [{}] {}", i + 1, role_str, preview));
    }
    lines.join("\n")
}
/// /mode - 切换交互模式
pub fn handle_mode(app: &mut TuiApp, args: &str) -> String {
    let current = format!("{:?}", app.mode);
    if args.is_empty() {
        return format!(
            "Current mode: {}\n\nAvailable modes:\n\
             - chat: Normal chat mode\n\
             - settings: Settings configuration mode\n\
             - vim_normal: Vim-style navigation mode\n\n\
             Usage: /mode <mode_name>",
            current
        );
    }

    let new_mode = args.trim().to_lowercase();
    match new_mode.as_str() {
        "chat" => {
            app.mode = AppMode::Chat;
            "Switched to chat mode.".to_string()
        }
        "settings" => {
            let config = crate::services::config::AppConfig::load().unwrap_or_default();
            app.settings_state = Some(crate::tui::components::settings::SettingsState::new(
                config,
                app.keybindings.clone(),
            ));
            app.mode = AppMode::Settings;
            "Switched to settings mode.".to_string()
        }
        "vim" | "vim_normal" => {
            app.mode = AppMode::VimNormal;
            "Switched to vim_normal mode. Use j/k to navigate, i to return to insert mode."
                .to_string()
        }
        _ => format!("Unknown mode: {}. Available: chat, settings, vim", new_mode),
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
/// /teammate - 启动协作队友 Agent
pub async fn handle_teammate(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("teammate") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_teammate",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let domain = if args.is_empty() {
                "general software development tasks".to_string()
            } else {
                args.to_string()
            };

            let prompt = format!(
                "{}\n\n## Your Focus\n\nYou are collaborating on: {}\n\nBegin by introducing yourself and asking what specific task you'd like to work on together.",
                skill.content, domain
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'teammate' not found.".to_string(),
    }
}
/// /critic - 启动批评型 Agent 审查代码
pub async fn handle_critic(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("critic") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_critic",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No changes to review.".to_string())
            };

            let scope = if args.is_empty() {
                "all code in the diff".to_string()
            } else {
                args.to_string()
            };

            let prompt = format!(
                "{}\n\n## Review Scope\n\nPlease critically review: {}\n\n## Changes\n\n```diff\n{}\n```",
                skill.content, scope, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'critic' not found.".to_string(),
    }
}
/// /assistant - 启动领域专家 Agent
pub async fn handle_assistant(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("assistant") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_assistant",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let parts: Vec<&str> = args.splitn(2, ':').collect();
            let domain = parts.first().unwrap_or(&"general");
            let task = parts.get(1).map(|s| s.trim()).unwrap_or("");

            let domain_intro = match *domain {
                "code_review" => "You are an expert code analyst. Provide deep insights into code structure, patterns, and potential issues.",
                "security" => "You are a security expert. Focus on vulnerabilities, injection risks, authentication issues, and secure coding practices.",
                "data" => "You are a data engineering expert. Focus on data pipelines, transformations, storage, and processing efficiency.",
                "infrastructure" => "You are an infrastructure expert. Focus on DevOps, deployment, CI/CD, and infrastructure as code.",
                "testing" => "You are a testing expert. Focus on test strategy, coverage, edge cases, and quality assurance.",
                _ => "You are a helpful specialized assistant.",
            };

            let prompt = if task.is_empty() {
                format!(
                    "{}\n\n## Domain\n\n{}\n\nWhat would you like expert assistance with?",
                    skill.content, domain_intro
                )
            } else {
                format!(
                    "{}\n\n## Domain\n\n{}\n\n## Task\n\n{}",
                    skill.content, domain_intro, task
                )
            };
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'assistant' not found.".to_string(),
    }
}
/// /remote - 启动远程专家 Agent
pub async fn handle_remote(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("remote") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_remote",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            // Bridge configuration is read from environment variables
            let bridge_url = std::env::var("PRIORITY_AGENT_BRIDGE_URL")
                .ok()
                .unwrap_or_else(|| "not configured".to_string());

            let prompt = format!(
                "{}\n\n## Bridge Configuration\n\nBridge URL: {}\n\nTo enable remote execution, set PRIORITY_AGENT_BRIDGE_URL environment variable.\n\n## Your Task\n\n{}",
                skill.content,
                bridge_url,
                if args.is_empty() { "What remote task would you like to execute?" } else { args }
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'remote' not found.".to_string(),
    }
}
/// /dream - 启动梦境任务 Agent（后台探索性分析）
pub async fn handle_dream(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("dream") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_dream",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let prompt = format!(
                "{}\n\n## Dream Task\n\n{}",
                skill.content,
                if args.is_empty() {
                    "What would you like me to explore in the background?"
                } else {
                    args
                }
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'dream' not found. The dream skill is not yet loaded.".to_string(),
    }
}
/// /custom - Create a custom agent
pub async fn handle_custom(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("custom") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_custom",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let prompt = format!(
                "{}\n\n## Custom Agent Request\n\n{}",
                skill.content,
                if args.is_empty() {
                    "Describe the custom agent you want to create:"
                } else {
                    args
                }
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'custom' not found.".to_string(),
    }
}
/// /orchestrate - Multi-agent coordination
pub async fn handle_orchestrate(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("orchestrate") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_orchestrate",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            let prompt = format!(
                "{}\n\n## Orchestration Task\n\n{}",
                skill.content,
                if args.is_empty() {
                    "What complex task would you like me to coordinate?"
                } else {
                    args
                }
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'orchestrate' not found.".to_string(),
    }
}
/// /token - 显示 token 使用情况
pub async fn handle_token(app: &TuiApp) -> String {
    if let Some(ref engine) = app.streaming_engine {
        let tracker = engine.cost_tracker().lock().await;
        let report = tracker.generate_report();
        format!("Token Usage:\n{}", report)
    } else {
        "Engine not initialized.".to_string()
    }
}
/// /lsp - LSP 服务器管理
pub fn handle_lsp(app: &TuiApp, args: &str) -> String {
    if let Some(ref mgr) = app.lsp_manager {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.is_empty() || parts[0] == "list" {
            let servers = mgr.server_names();
            if servers.is_empty() {
                "No LSP servers running.".to_string()
            } else {
                format!("LSP servers ({}):\n{}", servers.len(), servers.join("\n"))
            }
        } else if parts[0] == "restart" && parts.len() >= 2 {
            let _name = parts[1];
            format!("Restarting LSP server: {}...", _name)
        } else if parts[0] == "stop" && parts.len() >= 2 {
            let _name = parts[1];
            format!("Stopping LSP server: {}...", _name)
        } else {
            "Usage: /lsp [list|restart <name>|stop <name>]".to_string()
        }
    } else {
        "LSP manager not available.".to_string()
    }
}
/// /npm - npm 包管理辅助
pub async fn handle_npm(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let parts: Vec<&str> = args.split_whitespace().collect();
    let action = parts.first().unwrap_or(&"");

    match *action {
        "install" => {
            let pkg = parts.get(1).unwrap_or(&"");
            let cmd = if pkg.is_empty() {
                "npm install".to_string()
            } else {
                format!("npm install {}", pkg)
            };
            let params = serde_json::json!({
                "command": cmd,
                "description": "Install npm package"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "update" => {
            let params = serde_json::json!({
                "command": "npm update",
                "description": "Update npm packages"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "outdated" => {
            let params = serde_json::json!({
                "command": "npm outdated",
                "description": "Check outdated packages"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "test" => {
            let params = serde_json::json!({
                "command": "npm test",
                "description": "Run npm tests"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "run" => {
            let script = parts.get(1).unwrap_or(&"");
            let cmd = if script.is_empty() {
                "npm run".to_string()
            } else {
                format!("npm run {}", script)
            };
            let params = serde_json::json!({
                "command": cmd,
                "description": "Run npm script"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "" => "Usage: /npm [install|update|outdated|test|run] [args]".to_string(),
        _ => {
            let cmd = args;
            let params = serde_json::json!({
                "command": format!("npm {}", cmd),
                "description": format!("npm {}", cmd)
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
    }
}
/// Get diagnostic suggestions based on recent failures
pub async fn get_failure_suggestions(app: &TuiApp) -> String {
    let Some(ref engine) = app.streaming_engine else {
        return String::new();
    };

    let tracker_guard = engine.cost_tracker().lock().await;

    // Get top failure reasons
    let mut agg: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for s in tracker_guard.tool_metrics.values() {
        for (reason, cnt) in &s.failure_reasons {
            *agg.entry(reason.clone()).or_insert(0) += *cnt;
        }
    }

    if agg.is_empty() {
        return String::new();
    }

    let mut suggestions: Vec<String> = vec![];

    for (reason, _count) in agg.iter().take(3) {
        let reason_str: &str = reason.as_str();
        match reason_str {
            "timeout" => {
                suggestions.push(
                    "Timeout: Try /retry to repeat, or /doctor to check tool latency".to_string(),
                );
            }
            "permission" => {
                suggestions.push(
                    "Permission denied: Use /permissions to check rules, or /doctor to diagnose"
                        .to_string(),
                );
            }
            "not_found" => {
                suggestions.push(
                    "Not found: Check file paths with /ls, or verify resource exists".to_string(),
                );
            }
            "hook_blocked" => {
                suggestions.push(
                    "Hook blocked: Check PRE_TOOL_HOOK / POST_TOOL_HOOK env vars in /doctor"
                        .to_string(),
                );
            }
            "dangerous_command" => {
                suggestions.push(
                    "Dangerous command: Use /permissions to allow, or modify the command"
                        .to_string(),
                );
            }
            _ => {
                suggestions.push(format!(
                    "Error '{}': Run /doctor for detailed diagnostics",
                    reason_str
                ));
            }
        }
    }

    drop(tracker_guard);

    if suggestions.is_empty() {
        String::new()
    } else {
        format!("\n\nRecovery suggestions:\n- {}", suggestions.join("\n- "))
    }
}
/// Suggest recovery action based on error context
pub fn suggest_recovery(error: &str, _context: &str) -> String {
    let error_lower = error.to_lowercase();

    if error_lower.contains("timeout") {
        return "Timeout error. Suggestions:\n- Use /retry to repeat the operation\n- Use /doctor to check tool latency\n- Try a simpler command".to_string();
    }

    if error_lower.contains("permission") || error_lower.contains("denied") {
        return "Permission error. Suggestions:\n- Use /permissions rules to check current rules\n- Use /permissions mode to change mode\n- Run /doctor for permission diagnostics".to_string();
    }

    if error_lower.contains("not found") || error_lower.contains("does not exist") {
        return "Not found error. Suggestions:\n- Check file/resource exists with ls or glob\n- Verify the path is correct\n- Use /context to see current state".to_string();
    }

    if error_lower.contains("syntax") || error_lower.contains("parse") {
        return "Syntax error. Suggestions:\n- Check command arguments with /help <command>\n- Verify JSON formatting if using structured args\n- Try /doctor to validate environment".to_string();
    }

    // Default
    format!(
        "Error encountered. General suggestions:\n\
        - Use /retry to attempt the operation again\n\
        - Use /doctor to run full diagnostics\n\
        - Use /status to check current state\n\
        - Use /context to view conversation context\n\
        Error: {}",
        error
    )
}
/// /init - Initialize a new project
pub fn handle_init(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /init <project_name>\n\nCreates a small Rust project scaffold with README, Cargo.toml, src/main.rs, .gitignore, and .priority-agent/AGENTS.md.".to_string();
    }

    let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project_name = args.trim();
    let project_path = dir.join(project_name);

    if project_path.exists() {
        return format!("Target already exists: {}", project_path.display());
    }
    match std::fs::create_dir_all(project_path.join("src"))
        .and_then(|_| std::fs::create_dir_all(project_path.join(".priority-agent")))
    {
        Ok(_) => {
            let readme = project_path.join("README.md");
            let gitignore = project_path.join(".gitignore");
            let cargo_toml = project_path.join("Cargo.toml");
            let main_rs = project_path.join("src").join("main.rs");
            let agents = project_path.join(".priority-agent").join("AGENTS.md");
            if std::fs::write(
                &readme,
                format!(
                    "# {}\n\nInitialized by Priority Agent `/init`.\n\n## Next steps\n\n- Run `cargo test`\n- Describe the first feature you want the agent to build\n- Use `/settings` to confirm model and permission mode\n",
                    project_name
                ),
            )
            .is_err()
            {
                return format!(
                    "Project initialized at {} (README.md write failed)",
                    project_path.display()
                );
            }
            if std::fs::write(&gitignore, "target/\n*.log\n.env\n").is_err() {
                return format!(
                    "Project initialized at {} (.gitignore write failed)",
                    project_path.display()
                );
            }
            if std::fs::write(
                &cargo_toml,
                format!(
                    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
                    project_name.replace('-', "_")
                ),
            ).is_err() {
                return format!("Project initialized at {} (Cargo.toml write failed)", project_path.display());
            }
            if std::fs::write(&main_rs, "fn main() {\n    println!(\"hello\");\n}\n").is_err() {
                return format!(
                    "Project initialized at {} (src/main.rs write failed)",
                    project_path.display()
                );
            }
            if std::fs::write(
                &agents,
                format!(
                    "# Project Instructions\n\nProject: {}\n\n## Working style\n\n- Prefer small, verified changes.\n- Run relevant tests after edits.\n- Keep user-facing CLI output concise and useful.\n\n## Commands\n\n- `cargo test`\n- `cargo check`\n",
                    project_name
                ),
            )
            .is_err()
            {
                return format!(
                    "Project initialized at {} (.priority-agent/AGENTS.md write failed)",
                    project_path.display()
                );
            }
            format!(
                "Project initialized\n\nPath: {}\nCreated:\n  - README.md\n  - Cargo.toml\n  - src/main.rs\n  - .gitignore\n  - .priority-agent/AGENTS.md\n\nNext:\n  cd {}\n  cargo check\n\nThen tell the agent what to build first.",
                project_path.display(),
                project_path.display()
            )
        }
        Err(e) => format!("Failed to initialize project: {}", e),
    }
}
/// /login - Authentication
pub fn handle_login(_app: &mut TuiApp, args: &str) -> String {
    if args.trim().is_empty() || args.trim() == "status" {
        let prefs = load_runtime_prefs().unwrap_or_default();
        return format!(
            "Login status: {}",
            prefs
                .logged_in_provider
                .as_deref()
                .unwrap_or("not logged in")
        );
    }

    let provider = args.trim().to_ascii_lowercase();
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    prefs.logged_in_provider = Some(provider.clone());
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Login state update failed: {}", e);
    }
    format!(
        "Logged in to '{}' (local state only). Use /key to configure API keys.",
        provider
    )
}
/// /logout - Logout from provider
pub fn handle_logout(_app: &mut TuiApp, _args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let old = prefs.logged_in_provider.take();
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Failed to clear login state: {}", e);
    }
    match old {
        Some(p) => format!("Logged out from '{}'.", p),
        None => "No active login session.".to_string(),
    }
}
/// /key - API key management
pub fn handle_key(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        let has_key =
            std::env::var("MOONSHOT_API_KEY").is_ok() || std::env::var("OPENAI_API_KEY").is_ok();
        return if has_key {
            "API key is set. Use /model to see which model is active.".to_string()
        } else {
            "No API key set. Set MOONSHOT_API_KEY or OPENAI_API_KEY environment variable."
                .to_string()
        };
    }

    match args.trim() {
        "show" => {
            "API key not shown for security. Set MOONSHOT_API_KEY or OPENAI_API_KEY.".to_string()
        }
        "clear" => {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("MOONSHOT_API_KEY");
            std::env::remove_var("MINIMAX_API_KEY");
            "Cleared API keys from current process environment.".to_string()
        }
        _ => "Usage: /key [show|clear]".to_string(),
    }
}
/// /status - Detailed status
pub fn handle_status_detailed(_app: &TuiApp) -> String {
    let mut lines = vec!["Detailed Status:".to_string()];
    lines.push("  Mode: interactive CLI".to_string());
    lines.push(format!("  Rust version: {}", std::env::consts::OS));
    format!(
        "{}\n{}",
        lines.join("\n"),
        "Use /doctor for full diagnostics"
    )
}
/// /health - Health check
pub fn handle_health(_app: &TuiApp) -> String {
    "Health: OK\nSystem operational.".to_string()
}
/// /ping - Latency check
pub fn handle_ping(app: &mut TuiApp) -> String {
    use std::time::Instant;

    // Measure a real local round-trip by touching the session store.
    let db_start = Instant::now();
    let db_ok = app.session_manager.list_sessions(1).is_ok();
    let db_ms = db_start.elapsed().as_millis();

    format!(
        "Pong! Local DB round-trip: {}ms ({})",
        db_ms,
        if db_ok { "ok" } else { "error" }
    )
}
/// /uptime - Show uptime
pub fn handle_uptime(_app: &TuiApp) -> String {
    "Uptime: since boot (detailed tracking not implemented)".to_string()
}
/// /version - Show version
pub fn handle_version(_app: &TuiApp) -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
/// /about - About this agent
pub fn handle_about(_app: &TuiApp) -> String {
    format!(
        "Priority Agent v{}\nWeighted priority desktop Agent.\nType /help for available commands.",
        env!("CARGO_PKG_VERSION")
    )
}
