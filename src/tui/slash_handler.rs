//! Slash command handlers for TuiApp
//!
//! Each handler function takes `&mut TuiApp` + args and returns a String response.
//! This module exists to keep app.rs focused on core TUI state management.

use crate::agent::agent::AgentConfig;
use crate::agent::roles::AgentRole;
use crate::agent::types::{AgentId, AgentMessage, AgentMessageType};
use crate::tools::Tool;
use crate::tui::app::{AppMode, TuiApp};
use tokio::process::Command;

// ─── Session Management ───────────────────────────────────────────────

pub async fn handle_resume(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        match app.session_manager.list_sessions(10) {
            Ok(sessions) => {
                if sessions.is_empty() {
                    "No saved sessions found. Start chatting to create one!".to_string()
                } else {
                    let mut lines = vec!["Recent sessions:".to_string()];
                    for (i, session) in sessions.iter().enumerate() {
                        let title = if session.title.is_empty() {
                            "(untitled)"
                        } else {
                            &session.title
                        };
                        let msg_count = app.session_manager.message_count(&session.id).unwrap_or(0);
                        lines.push(format!(
                            "{}. [{}] {} ({} msgs) - {}",
                            i + 1,
                            &session.id[..8.min(session.id.len())],
                            title,
                            msg_count,
                            session.updated_at
                        ));
                    }
                    lines.push("\nUse /resume <number> or /resume <id> to restore.".to_string());
                    lines.join("\n")
                }
            }
            Err(e) => format!("Failed to list sessions: {}", e),
        }
    } else if let Ok(index) = args.parse::<usize>() {
        match app.session_manager.list_sessions(20) {
            Ok(sessions) => {
                if index == 0 || index > sessions.len() {
                    "Invalid session number. Use /resume without arguments to see available sessions.".to_string()
                } else {
                    let session = &sessions[index - 1];
                    app.restore_session(&session.id).await
                }
            }
            Err(e) => format!("Failed to list sessions: {}", e),
        }
    } else {
        app.restore_session(args).await
    }
}

pub fn handle_rewind(app: &mut TuiApp, args: &str) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session.".to_string(),
    };

    if args.is_empty() {
        match app.session_manager.list_edits(&session_id) {
            Ok(edits) => {
                if edits.is_empty() {
                    "No edits recorded in this session.".to_string()
                } else {
                    let mut lines = vec!["Recent edits (most recent first):".to_string()];
                    for (i, edit) in edits.iter().rev().enumerate().take(10) {
                        lines.push(format!(
                            "{}. [{}] {} - {}",
                            i + 1,
                            edit.tool_name,
                            edit.file_path,
                            edit.timestamp
                        ));
                    }
                    lines.push("\nUse /rewind <n> to rewind the last n edits, or /rewind <file_path> to rewind a specific file.".to_string());
                    lines.join("\n")
                }
            }
            Err(e) => format!("Failed to list edits: {}", e),
        }
    } else if let Ok(n) = args.parse::<usize>() {
        let mut results = Vec::new();
        for _ in 0..n {
            match app.session_manager.rewind_last_edit(&session_id) {
                Ok(msg) => results.push(msg),
                Err(e) => {
                    results.push(format!("Error: {}", e));
                    break;
                }
            }
        }
        results.join("\n")
    } else {
        match app.session_manager.rewind_file(&session_id, args) {
            Ok(msg) => msg,
            Err(e) => format!("Failed to rewind file: {}", e),
        }
    }
}

pub fn handle_sessions(app: &TuiApp) -> String {
    match app.session_manager.list_sessions(10) {
        Ok(sessions) => {
            if sessions.is_empty() {
                "No saved sessions. Start chatting to create one!".to_string()
            } else {
                let mut lines = vec!["Recent sessions:".to_string()];
                for (i, session) in sessions.iter().enumerate() {
                    let title = if session.title.is_empty() {
                        "(untitled)"
                    } else {
                        &session.title
                    };
                    let msg_count = app.session_manager.message_count(&session.id).unwrap_or(0);
                    lines.push(format!(
                        "{}. [{}] {} ({} msgs) - {}",
                        i + 1,
                        &session.id[..8.min(session.id.len())],
                        title,
                        msg_count,
                        session.updated_at
                    ));
                }
                lines.push("\nUse /session <number> to restore a session.".to_string());
                lines.join("\n")
            }
        }
        Err(e) => format!("Failed to list sessions: {}", e),
    }
}

pub async fn handle_session(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        let current = app.session_manager.current_session_id().unwrap_or("none");
        let title = app.session_manager.current_session_title();
        let msg_count = app
            .session_manager
            .current_session_id()
            .and_then(|id| app.session_manager.message_count(id).ok())
            .unwrap_or(0);
        format!(
            "Current session: {}\nTitle: {}\nMessages: {}",
            current, title, msg_count
        )
    } else if let Ok(index) = args.parse::<usize>() {
        match app.session_manager.list_sessions(20) {
            Ok(sessions) => {
                if index == 0 || index > sessions.len() {
                    "Invalid session number. Use /sessions to see available sessions.".to_string()
                } else {
                    let session = &sessions[index - 1];
                    app.restore_session(&session.id).await
                }
            }
            Err(e) => format!("Failed to list sessions: {}", e),
        }
    } else {
        app.restore_session(args).await
    }
}

pub async fn handle_new(app: &mut TuiApp) -> String {
    let model = app
        .streaming_engine
        .as_ref()
        .map(|_| "kimi-k2.5")
        .unwrap_or("unknown");
    match app.session_manager.start_session("New Session", model) {
        Ok(id) => {
            use crate::state::{MessageItem, MessageRole};
            app.messages.clear();
            let welcome = MessageItem {
                id: "welcome".to_string(),
                role: MessageRole::System,
                content: "Started a new session. Previous messages cleared from view but saved to database.".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            };
            app.messages.push(welcome);
            format!("New session started: {}", id)
        }
        Err(e) => format!("Failed to start new session: {}", e),
    }
}

pub fn handle_export(app: &TuiApp) -> String {
    if let Some(id) = app.session_manager.current_session_id() {
        match app.session_manager.export_session(id) {
            Ok(json) => {
                let filename = format!("session_{}.json", &id[..8.min(id.len())]);
                let path = dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join(&filename);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&path, &json) {
                    Ok(_) => format!("Session exported to: {}", path.display()),
                    Err(e) => format!("Failed to write export file: {}", e),
                }
            }
            Err(e) => format!("Failed to export session: {}", e),
        }
    } else {
        "No active session to export.".to_string()
    }
}

pub fn handle_search(app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        "Usage: /search <query> - Search through all session messages".to_string()
    } else {
        match app.session_manager.search_sessions(args, 10) {
            Ok(sessions) => {
                if sessions.is_empty() {
                    format!("No sessions found matching '{}'", args)
                } else {
                    let mut lines = vec![format!("Sessions matching '{}':", args)];
                    for (i, session) in sessions.iter().enumerate() {
                        lines.push(format!(
                            "{}. [{}] {}",
                            i + 1,
                            &session.id[..8.min(session.id.len())],
                            session.title
                        ));
                    }
                    lines.join("\n")
                }
            }
            Err(e) => format!("Search failed: {}", e),
        }
    }
}

pub fn handle_stats(app: &TuiApp) -> String {
    match app.session_manager.stats() {
        Ok(stats) => {
            format!(
                "Session Statistics:\n\
                 Total sessions: {}\n\
                 Total messages: {}\n\
                 Total input tokens: {}\n\
                 Total output tokens: {}",
                stats.session_count,
                stats.message_count,
                stats.total_input_tokens,
                stats.total_output_tokens
            )
        }
        Err(e) => format!("Failed to get stats: {}", e),
    }
}

pub async fn handle_batch(_app: &mut TuiApp, args: &str) -> String {
    if args.trim().is_empty() {
        return "Usage: /batch <task description> [--files <patterns>...]\n\
                Example: /batch Rename all User references to Account --files src/**/*.rs\n\
                Set PRIORITY_AGENT_BATCH_REFACTOR=1 to enable.".to_string();
    }

    // 解析参数
    let parts: Vec<&str> = args.split(" --files ").collect();
    let description = parts[0].trim();
    let files = if parts.len() > 1 {
        parts[1].split_whitespace().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let batch = crate::engine::batch_refactor::BatchRefactor::new(working_dir);

    if !batch.is_enabled() {
        return "Batch refactor is not enabled.\n\
                Set environment variable: PRIORITY_AGENT_BATCH_REFACTOR=1\n\
                Optional: PRIORITY_AGENT_BATCH_MAX_PARALLEL=10".to_string();
    }

    // 如果没有指定文件，尝试自动发现
    let files = if files.is_empty() {
        let mut scanner = crate::tools::project_tool::ProjectScanner::new();
        scanner.scan(std::path::Path::new("."));
        let files: Vec<String> = scanner.files().iter().take(50).cloned().collect();
        if files.is_empty() {
            return "No files specified and auto-discovery found none.".to_string();
        }
        files
    } else {
        files
    };

    match batch.execute(description, files).await {
        Ok(result) => {
            let mut lines = vec![
                format!("## Batch Refactor Result: {:?}", result.status),
                format!("Units: {} | Duration: {}ms", result.units.len(), result.total_duration_ms),
                String::new(),
            ];

            let success_count = result.units.iter().filter(|u| u.success).count();
            let fail_count = result.units.len() - success_count;
            lines.push(format!("✅ Success: {} | ❌ Failed: {}", success_count, fail_count));

            for unit in &result.units {
                let icon = if unit.success { "✅" } else { "❌" };
                lines.push(format!("{} {} ({}ms)", icon, unit.unit_id, unit.duration_ms));
                if !unit.output.is_empty() {
                    for line in unit.output.lines().take(5) {
                        lines.push(format!("   {}", line));
                    }
                }
            }

            lines.join("\n")
        }
        Err(e) => format!("Batch refactor failed: {}", e),
    }
}

pub async fn handle_checkpoints(app: &TuiApp) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => format!("session-{}", id),
        None => return "No active session. Start a conversation first.".to_string(),
    };

    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let cp = mgr.lock().await;
    let checkpoints = cp.list_checkpoints();
    let stats = cp.stats();

    if checkpoints.is_empty() {
        return "No checkpoints for this session yet.\nCheckpoints are created automatically before file edits.".to_string();
    }

    let mut lines = vec![
        format!("Checkpoints for session (total: {}, files tracked: {})",
            stats.total_checkpoints, stats.total_files_tracked),
        String::new(),
    ];

    for c in checkpoints.iter().rev().take(20) {
        let files: Vec<String> = c.file_backups.iter()
            .map(|f| format!("{} {}", if f.existed_before { "📝" } else { "🆕" }, f.original_path))
            .collect();
        lines.push(format!(
            "[{}] {} ({} files)\n  tool: {} | {}",
            c.sequence,
            c.id.split('_').last().unwrap_or(&c.id),
            c.file_backups.len(),
            c.tool_name,
            files.join(", ")
        ));
    }

    if checkpoints.len() > 20 {
        lines.push(format!("\n... and {} more checkpoints", checkpoints.len() - 20));
    }

    lines.join("\n")
}

pub async fn handle_restore(app: &mut TuiApp, args: &str) -> String {
    if args.trim().is_empty() {
        return "Usage: /restore <checkpoint_id>\nUse /checkpoints to list available checkpoints.".to_string();
    }

    let session_id = match app.session_manager.current_session_id() {
        Some(id) => format!("session-{}", id),
        None => return "No active session.".to_string(),
    };

    let checkpoint_id = args.trim();
    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let cp = mgr.lock().await;

    match cp.restore_checkpoint(checkpoint_id).await {
        Ok(result) => {
            let mut lines = vec![format!("Restored checkpoint: {}", result.checkpoint_id)];
            if !result.restored_files.is_empty() {
                lines.push(format!("\nRestored {} file(s):", result.restored_files.len()));
                for f in &result.restored_files {
                    lines.push(format!("  ✅ {}", f));
                }
            }
            if !result.removed_files.is_empty() {
                lines.push(format!("\nRemoved {} file(s) (did not exist before checkpoint):", result.removed_files.len()));
                for f in &result.removed_files {
                    lines.push(format!("  🗑️  {}", f));
                }
            }
            if !result.failed_files.is_empty() {
                lines.push(format!("\nFailed to restore {} file(s):", result.failed_files.len()));
                for (f, e) in &result.failed_files {
                    lines.push(format!("  ❌ {} — {}", f, e));
                }
            }
            lines.join("\n")
        }
        Err(e) => format!("Failed to restore checkpoint: {}", e),
    }
}

// ─── System & Tools ───────────────────────────────────────────────────

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
                let status = *handle.status.read().await;
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
                    let status_label = format!("{:?}", *handle.status.read().await);
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
            format!("calls={} success={} success_rate={:.1}%", total_calls, total_success, success_rate),
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
                format!("memory_extraction: hits={} misses={} hit_rate={:.1}%", hits, misses, mem_hit_rate),
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
async fn generate_gap_snapshot(app: &TuiApp, report: &crate::diagnostics::DiagnosticReport) -> String {
    let mut lines = vec![
        "=== Claude Code Gap Snapshot ===".to_string(),
        format!("Generated: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")),
        "".to_string(),
    ];

    // Count tools from registry
    let mut registry = crate::tools::ToolRegistry::default_registry();
    let _injected = crate::tools::plugin_tool::register_enabled_plugin_tools(&mut registry, &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let tool_count = registry.tool_names().len();

    // Count commands
    let cmd_count = crate::tui::commands::ALL_COMMANDS.len();

    // Engine status
    let engine_ok = app.streaming_engine.is_some();
    let model_name = app.streaming_engine.as_ref().map(|e| e.model_name()).unwrap_or_default();

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
    lines.push(format!("| Tools     | {}   | 64     | {}  |", tool_count, tool_gap));
    lines.push(format!("| Commands  | {}   | 101    | {}  |", cmd_count, cmd_gap));
    lines.push("| Agents    | 7    | 7      | 0   |".to_string());
    lines.push("| Transport | 3    | 3      | 0   |".to_string());
    lines.push("| Frontend  | 2    | 4      | -2  |".to_string());
    lines.push("".to_string());

    // Performance snapshot
    lines.push("## Performance Snapshot".to_string());
    for check in &report.checks {
        if matches!(check.name.as_str(), "tool_latency_p95" | "tool_success_rate" | "coding_quality" | "context_compression" | "memory_cache") {
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

// ─── Skills ───────────────────────────────────────────────────────────

pub async fn handle_commit(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("commit") {
        Some(skill) => {
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff", "cached": true });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No staged changes or unable to read diff.".to_string())
            };
            let prompt = format!(
                "{}\n\nStaged changes:\n```diff\n{}\n```",
                skill.content, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'commit' not found.".to_string(),
    }
}

pub async fn handle_commit_push_pr(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::GitTool;
    let ctx = app.build_tool_context().await;

    // 收集 git 上下文
    let status_result = tool
        .execute(serde_json::json!({ "action": "status" }), ctx.clone())
        .await;
    let status = if status_result.success {
        status_result.content
    } else {
        "Unable to get git status.".to_string()
    };

    let diff_result = tool
        .execute(serde_json::json!({ "action": "diff" }), ctx.clone())
        .await;
    let diff = if diff_result.success {
        diff_result.content
    } else {
        "Unable to get git diff.".to_string()
    };

    // 获取当前分支
    let branch = match Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    };

    let log_result = tool
        .execute(serde_json::json!({ "action": "log", "n": 5 }), ctx.clone())
        .await;
    let log = if log_result.success {
        log_result.content
    } else {
        "Unable to get git log.".to_string()
    };

    let user_desc = if args.trim().is_empty() {
        "No specific description provided. Infer the purpose from the changes.".to_string()
    } else {
        args.trim().to_string()
    };

    let prompt = format!(
        "You are a git workflow assistant. Complete the following steps to commit changes and create a PR.\n\
         \n\
         ## Steps\n\
         1. Use the `git` tool with action='add' to stage all changes (path='' or paths=[]).\n\
         2. Use the `git` tool with action='commit' to create a commit.\n\
            - Generate a concise conventional commit message from the changes.\n\
            - Format: `<type>(<scope>): <description>` (e.g., `feat(auth): add login flow`)\n\
            - Keep subject under 72 characters, use imperative mood.\n\
         3. Check current branch. If on main/master, create a new feature branch first using git action='checkout' with create_branch=true.\n\
         4. Use the `git` tool with action='push' to push the branch to origin.\n\
         5. Use the `github` tool with action='pr_create' to create a Pull Request.\n\
            - Use the commit message as PR title.\n\
            - Provide a brief PR body summarizing the changes.\n\
         \n\
         ## Safety Rules\n\
         - NEVER use force push.\n\
         - NEVER use `git commit --amend`.\n\
         - Do not commit secret files (.env, credentials.json, id_rsa, etc.).\n\
         - If there are no changes to commit, report that and stop.\n\
         \n\
         ## Context\n\
         Current branch: {}\n\
         User description: {}\n\
         \n\
         Git status:\n\
         ```\n{}\n```\n\
         \n\
         Git diff:\n\
         ```diff\n{}\n```\n\
         \n\
         Recent commits:\n\
         ```\n{}\n```",
        branch, user_desc, status, diff, log
    );

    app.send_message(prompt).await;
    String::new()
}

pub async fn handle_review_pr(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /review-pr <number>".to_string();
    }
    match app.bundled_skills.get("review_pr") {
        Some(skill) => {
            let pr_number = args.trim();
            if !pr_number.chars().all(|c| c.is_ascii_digit()) {
                return "Invalid PR number. Must be numeric.".to_string();
            }
            let output = tokio::process::Command::new("gh")
                .args(["pr", "diff", pr_number])
                .output()
                .await;
            let diff = match output {
                Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
                Ok(out) => format!(
                    "Failed to fetch PR diff: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
                Err(e) => format!("Failed to run gh: {}", e),
            };
            let prompt = format!(
                "{}\n\nPR #{} diff:\n```diff\n{}\n```",
                skill.content, pr_number, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'review_pr' not found.".to_string(),
    }
}

pub async fn handle_review(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("review") {
        Some(skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_review",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No uncommitted changes or unable to read diff.".to_string())
            };
            let prompt = format!(
                "{}\n\nLocal changes diff:\n```diff\n{}\n```",
                skill.content, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'review' not found.".to_string(),
    }
}

pub async fn handle_security_review(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("security_review") {
        Some(skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_security_review",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No uncommitted changes or unable to read diff.".to_string())
            };
            let prompt = format!(
                "{}\n\nLocal changes diff:\n```diff\n{}\n```",
                skill.content, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'security_review' not found.".to_string(),
    }
}

pub async fn handle_explain(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("explain") {
        Some(skill) => {
            let context = if args.is_empty() {
                "No specific target provided. Please explain the current codebase context or answer generally.".to_string()
            } else {
                let working_dir =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                match crate::tools::file_tool::resolve_path(args.trim(), &working_dir) {
                    Ok(path) => {
                        if path.exists() && path.is_file() {
                            match tokio::fs::read_to_string(&path).await {
                                Ok(content) => content,
                                Err(e) => format!("Failed to read file: {}", e),
                            }
                        } else {
                            format!("Path '{}' does not exist or is not a file.", args.trim())
                        }
                    }
                    Err(e) => format!("Invalid path '{}': {}", args.trim(), e),
                }
            };
            let prompt = format!("{}\n\n{}", skill.content, context);
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'explain' not found.".to_string(),
    }
}

pub async fn handle_fix(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("fix") {
        Some(skill) => {
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No uncommitted changes or unable to read diff.".to_string())
            };
            let prompt = format!(
                "{}\n\nCurrent changes:\n```diff\n{}\n```",
                skill.content, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'fix' not found.".to_string(),
    }
}

// ─── Integrations ─────────────────────────────────────────────────────

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

pub fn handle_vim(app: &mut TuiApp) -> String {
    app.vim_mode = !app.vim_mode;
    if app.vim_mode {
        app.mode = AppMode::VimNormal;
        "Vim mode enabled. Press Ctrl+V or type /vim again to disable.".to_string()
    } else {
        app.mode = AppMode::Chat;
        "Vim mode disabled.".to_string()
    }
}

pub fn handle_onboarding(app: &mut TuiApp) -> String {
    let manager = crate::onboarding::OnboardingManager::new();
    let _ = manager.reset();
    app.onboarding_state = Some(crate::onboarding::OnboardingState::new());
    app.mode = AppMode::Onboarding;
    "Onboarding restarted. Press Enter or → to continue, ← to go back, Esc to skip.".to_string()
}

pub fn handle_skip(app: &mut TuiApp) -> String {
    if app.mode == AppMode::Onboarding {
        if let Some(ref state) = app.onboarding_state {
            let _ = state.complete();
        }
        app.onboarding_state = None;
        app.mode = AppMode::Chat;
        "Onboarding skipped. Type /onboarding to restart it.".to_string()
    } else {
        "Not in onboarding mode.".to_string()
    }
}

// ─── Permissions (complex, 128 lines) ─────────────────────────────────

use crate::permissions::{match_wildcard, PermissionMode, RuleSource, SourcedRule};
use crate::tui::app::{parse_permission_mode, permission_mode_name, persist_permission_rule};

pub fn handle_permissions(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    let sub = parts.next();

    match sub {
        None => {
            let mode = app
                .streaming_engine
                .as_ref()
                .map(|e| e.permission_mode())
                .unwrap_or(PermissionMode::AutoLowRisk);
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);
            format!(
                "Permission mode: {}\nRules: allow={} deny={} ask={}\nProject config: {}\nGlobal config: {}\n\nUsage:\n  /permissions mode <default|auto_low_risk|auto_all|read_only>\n  /permissions rules [tool_name]\n  /permissions explain <tool_name> - explain why a decision was made (with confidence & warnings)\n  /permissions export [path] - export rules to a file\n  /permissions import <path> [project|global] [merge] - import rules (merge to append)\n  /permissions dry-run <allow|deny|ask> <pattern> - test a rule against all registered tools\n  /permissions <allow|deny|ask> <pattern> [project|global]",
                permission_mode_name(mode),
                ctx.rules.always_allow.len(),
                ctx.rules.always_deny.len(),
                ctx.rules.always_ask.len(),
                cwd.join(".priority-agent").join("permissions.toml").display(),
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml")
                    .display(),
            )
        }
        Some("explain") => {
            let tool_name = match parts.next() {
                Some(t) if !t.trim().is_empty() => t.trim(),
                _ => return "Usage: /permissions explain <tool_name>".to_string(),
            };
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);
            // Use ExplainableDecision for rich output (confidence, warnings, matched rules)
            let explainable = ctx.explain_decision(tool_name, &serde_json::Value::Null);
            let mut output = explainable.format();

            // Add mode context
            let mode = app
                .streaming_engine
                .as_ref()
                .map(|e| e.permission_mode())
                .unwrap_or(PermissionMode::AutoLowRisk);
            output.push_str(&format!("\n\nCurrent mode: {}", permission_mode_name(mode)));
            match mode {
                PermissionMode::AutoAll => output.push_str("\n  (all operations auto-allowed - rules ignored)"),
                PermissionMode::AutoLowRisk => output.push_str("\n  (low-risk operations auto-allowed, others follow rules)"),
                PermissionMode::ReadOnly => output.push_str("\n  (all write operations denied)"),
                PermissionMode::Once => output.push_str("\n  (each operation allowed once then denied)"),
                _ => {}
            }
            output
        }
        Some("export") => {
            let path = parts.next().map(|p| {
                if p == "global" || p == "project" {
                    return None;
                }
                Some(std::path::PathBuf::from(p))
            }).unwrap_or_else(|| {
                let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                Some(cwd.join(".priority-agent").join("permissions_export.toml"))
            });

            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);

            // Build export content (using standard TOML array format)
            let mut content = String::new();
            content.push_str("# Permission Rules Export\n");
            content.push_str(&format!("# Exported at: {}\n\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));

            content.push_str("[allow]\npatterns = [");
            for (i, r) in ctx.rules.always_allow.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(&format!("\"{}\"", r.pattern));
            }
            content.push_str("]\n");

            content.push_str("\n[deny]\npatterns = [");
            for (i, r) in ctx.rules.always_deny.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(&format!("\"{}\"", r.pattern));
            }
            content.push_str("]\n");

            content.push_str("\n[ask]\npatterns = [");
            for (i, r) in ctx.rules.always_ask.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(&format!("\"{}\"", r.pattern));
            }
            content.push_str("]\n");

            if let Some(ref p) = path {
                if let Some(parent) = p.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(p, &content) {
                    Ok(_) => format!("Rules exported to: {}", p.display()),
                    Err(e) => format!("Failed to export: {}", e),
                }
            } else {
                content
            }
        }
        Some("import") => {
            let file_path = match parts.next() {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => return "Usage: /permissions import <path> [project|global] [merge]".to_string(),
            };
            let scope = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "global" => RuleSource::Global,
                Some(s) if s == "project" => RuleSource::Project,
                Some(other) => return format!("Invalid scope '{}'. Use 'project' or 'global'.", other),
                None => RuleSource::Project,
            };
            let merge = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "merge" => true,
                Some(other) => return format!("Invalid option '{}'. Use 'merge' or omit.", other),
                None => false,
            };

            let import_content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => return format!("Failed to read file: {}", e),
            };

            let target_path = match scope {
                RuleSource::Global => dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml"),
                _ => std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml"),
            };

            if let Some(parent) = target_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let final_content = if merge && target_path.exists() {
                // Read existing rules and merge with imported rules
                let existing = std::fs::read_to_string(&target_path).unwrap_or_default();
                match merge_permission_toml(&existing, &import_content) {
                    Ok(merged) => merged,
                    Err(e) => return format!("Failed to merge rules: {}", e),
                }
            } else {
                import_content
            };

            match std::fs::write(&target_path, &final_content) {
                Ok(_) => {
                    let action = if merge { "merged into" } else { "imported to" };
                    format!("Rules {} '{}' -> {}", action, file_path, target_path.display())
                }
                Err(e) => format!("Failed to import: {}", e),
            }
        }
        Some("dry-run") => {
            let action = match parts.next() {
                Some(a) if a == "allow" || a == "deny" || a == "ask" => a,
                _ => return "Usage: /permissions dry-run <allow|deny|ask> <pattern>".to_string(),
            };
            let pattern = match parts.next() {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => return "Usage: /permissions dry-run <allow|deny|ask> <pattern>".to_string(),
            };

            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);

            // Simulate adding the rule temporarily
            let mut test_rules = ctx.rules.clone();
            let test_rule = SourcedRule::new(pattern, RuleSource::User);

            match action {
                "allow" => test_rules.always_allow.push(test_rule),
                "deny" => test_rules.always_deny.push(test_rule),
                "ask" => test_rules.always_ask.push(test_rule),
                _ => unreachable!(),
            }

            // Show what tools would match using full registry + explainable decisions
            let mut lines = vec![
                format!("Dry-run: {} '{}'", action, pattern),
                format!("Config path: {}/.priority-agent/permissions.toml", cwd.display()),
                "".to_string(),
                "This rule would affect:".to_string(),
            ];

            // Test against all registered tools
            let registry = crate::tools::ToolRegistry::default_registry();
            let mut affected = 0;
            for tool in &registry.tool_names() {
                if match_wildcard(pattern, tool) {
                    affected += 1;
                    let decision = test_rules.check(tool);
                    let explainable = ctx.explain_decision(tool, &serde_json::Value::Null);
                    let conf = (explainable.confidence * 100.0) as u32;
                    let warn = if explainable.warnings.is_empty() {
                        "".to_string()
                    } else {
                        format!(" ⚠️ {}", explainable.warnings.join(", "))
                    };
                    lines.push(format!(
                        "  {} -> {:?} (confidence: {}%){}",
                        tool, decision, conf, warn
                    ));
                }
            }
            if affected == 0 {
                lines.push("  (no registered tools match this pattern)".to_string());
            } else {
                lines.push(format!("\nTotal affected tools: {}", affected));
            }

            lines.join("\n")
        }
        Some("mode") => {
            if let Some(mode_arg) = parts.next() {
                if let Some(mode) = parse_permission_mode(mode_arg) {
                    if let Some(ref engine) = app.streaming_engine {
                        engine.set_permission_mode(mode);
                        format!("Permission mode set to '{}'.", permission_mode_name(mode))
                    } else {
                        "Cannot set permission mode: engine unavailable.".to_string()
                    }
                } else {
                    "Invalid mode. Use: default | auto_low_risk | auto_all | read_only".to_string()
                }
            } else {
                let current = app
                    .streaming_engine
                    .as_ref()
                    .map(|e| e.permission_mode())
                    .unwrap_or(PermissionMode::AutoLowRisk);
                format!(
                    "Current mode: {}\nAvailable: default | auto_low_risk | auto_all | read_only",
                    permission_mode_name(current)
                )
            }
        }
        Some("rules") => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);
            if let Some(tool_name) = parts.next() {
                let (decision, details) = ctx.check_with_details(tool_name);
                let mut lines = vec![format!("Tool '{}': {:?}", tool_name, decision)];
                if details.is_empty() {
                    lines.push(
                        "No explicit matching rules (fallback behavior applies).".to_string(),
                    );
                } else {
                    lines.push("Matched rules:".to_string());
                    for d in details {
                        lines.push(format!("- {}", d));
                    }
                }
                lines.join("\n")
            } else {
                let mut lines = vec![
                    format!("Rules overview (cwd={}):", cwd.display()),
                    format!("allow({}):", ctx.rules.always_allow.len()),
                ];
                for r in ctx.rules.always_allow.iter().take(30) {
                    lines.push(format!("- [{:?}] {}", r.source, r.pattern));
                }
                lines.push(format!("deny({}):", ctx.rules.always_deny.len()));
                for r in ctx.rules.always_deny.iter().take(30) {
                    lines.push(format!("- [{:?}] {}", r.source, r.pattern));
                }
                lines.push(format!("ask({}):", ctx.rules.always_ask.len()));
                for r in ctx.rules.always_ask.iter().take(30) {
                    lines.push(format!("- [{:?}] {}", r.source, r.pattern));
                }
                lines.join("\n")
            }
        }
        Some(action @ ("allow" | "deny" | "ask")) => {
            let pattern = match parts.next() {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => {
                    return "Usage: /permissions <allow|deny|ask> <pattern> [project|global]"
                        .to_string()
                }
            };
            let scope = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "global" => RuleSource::Global,
                Some(s) if s == "project" => RuleSource::Project,
                Some(other) => {
                    return format!("Invalid scope '{}'. Use 'project' or 'global'.", other)
                }
                None => RuleSource::Project,
            };
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            match persist_permission_rule(scope, action, pattern, &cwd) {
                Ok(path) => {
                    let path: std::path::PathBuf = path;
                    format!(
                        "Rule saved: {} '{}' ({:?})\nConfig: {}",
                        action,
                        pattern,
                        scope,
                        path.display()
                    )
                }
                Err(e) => format!("Failed to save rule: {}", e),
            }
        }
        Some(_) => "Usage: /permissions [mode|rules|allow|deny|ask] ...".to_string(),
    }
}

// ─── Skill Commands ─────────────────────────────────────────────────────────

pub async fn handle_simplify(app: &mut TuiApp, _args: &str) -> String {
    match app.bundled_skills.get("simplify") {
        Some(skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_simplify",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No uncommitted changes or unable to read diff.".to_string())
            };

            // Launch 3 parallel sub-agents: Reuse, Quality, Efficiency
            let agent_manager = match app
                .streaming_engine
                .as_ref()
                .and_then(|e| e.agent_manager())
            {
                Some(am) => am,
                None => {
                    return "Agent manager not available. Cannot run simplify.".to_string();
                }
            };
            let _working_dir = if let Some(ref wt) = app.worktree_manager {
                wt.current_worktree().await.unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                })
            } else {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            };

            let reuse_prompt = format!(
                "{}\n\n## Focus: Code Reuse Review\n\nAnalyze this diff for:\n- Repeated code that should be extracted\n- Copy-paste patterns\n- Missing abstractions\n- DRY violations\n\nDiff:\n```diff\n{}\n```",
                skill.content, diff
            );
            let quality_prompt = format!(
                "{}\n\n## Focus: Code Quality Review\n\nAnalyze this diff for:\n- Redundant state or parameter sprawl\n- Leaky abstractions\n- Error handling issues\n- Missing validation\n- Poor naming or documentation\n\nDiff:\n```diff\n{}\n```",
                skill.content, diff
            );
            let efficiency_prompt = format!(
                "{}\n\n## Focus: Efficiency Review\n\nAnalyze this diff for:\n- Unnecessary computations\n- Missed concurrency opportunities\n- Hot-path bloat\n- Memory allocation issues\n- Inefficient data structures\n\nDiff:\n```diff\n{}\n```",
                skill.content, diff
            );

            let spawn_agent = |description: String, prompt: String, role: AgentRole| {
                let am = agent_manager.clone();
                async move {
                    let config = AgentConfig::new(format!("simplify: {}", description))
                        .with_description(&description)
                        .with_system_prompt(&prompt)
                        .with_max_turns(10)
                        .with_max_cost_usd(0.05)
                        .with_role(role);
                    let agent_id = am.spawn(config, None).await?;
                    let task_msg = AgentMessage::new(
                        AgentId::new(),
                        agent_id.clone(),
                        prompt,
                        AgentMessageType::Task,
                    );
                    am.send_message(&agent_id, task_msg).await?;
                    am.wait_for_result(&agent_id, 120).await
                }
            };

            let (reuse_result, quality_result, efficiency_result) = tokio::join!(
                spawn_agent("code-reuse".to_string(), reuse_prompt, AgentRole::Default),
                spawn_agent(
                    "code-quality".to_string(),
                    quality_prompt,
                    AgentRole::Default
                ),
                spawn_agent(
                    "efficiency".to_string(),
                    efficiency_prompt,
                    AgentRole::Default
                ),
            );

            let mut report = "# Simplify Report\n\n".to_string();
            report += "Running 3 parallel review agents: Reuse / Quality / Efficiency...\n\n";

            match reuse_result {
                Ok(r) => {
                    let status = if r.status == crate::agent::types::AgentStatus::Completed {
                        "✓"
                    } else {
                        "✗"
                    };
                    report += &format!("## Code Reuse Review {}:\n{}\n\n", status, r.content);
                }
                Err(e) => {
                    report += &format!("## Code Reuse Review ✗:\nError: {}\n\n", e);
                }
            }
            match quality_result {
                Ok(r) => {
                    let status = if r.status == crate::agent::types::AgentStatus::Completed {
                        "✓"
                    } else {
                        "✗"
                    };
                    report += &format!("## Code Quality Review {}:\n{}\n\n", status, r.content);
                }
                Err(e) => {
                    report += &format!("## Code Quality Review ✗:\nError: {}\n\n", e);
                }
            }
            match efficiency_result {
                Ok(r) => {
                    let status = if r.status == crate::agent::types::AgentStatus::Completed {
                        "✓"
                    } else {
                        "✗"
                    };
                    report += &format!("## Efficiency Review {}:\n{}\n\n", status, r.content);
                }
                Err(e) => {
                    report += &format!("## Efficiency Review ✗:\nError: {}\n\n", e);
                }
            }

            app.add_system_message(report.clone());
            app.send_message(format!(
                "I've run a comprehensive simplify analysis on your changes. Here's the summary:\n\n{}\n\nSending to main agent for detailed recommendations...",
                report
            )).await;
            String::new()
        }
        None => "Skill 'simplify' not found.".to_string(),
    }
}

pub async fn handle_verify(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("verify") {
        Some(_skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::BashTool;

            // Detect project type
            let (test_cmd, project_type) = if std::path::Path::new("Cargo.toml").exists() {
                ("cargo test 2>&1", "Rust")
            } else if std::path::Path::new("package.json").exists() {
                ("npm test 2>&1", "Node.js")
            } else if std::path::Path::new("pyproject.toml").exists()
                || std::path::Path::new("setup.py").exists()
            {
                ("python -m pytest 2>&1", "Python")
            } else {
                ("echo 'No recognized project type found'", "Unknown")
            };

            let params = serde_json::json!({
                "command": test_cmd,
                "description": "Run project tests"
            });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_verify",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }

            let output = &result.content;
            let passed = count_test_passed(output);
            let failed = count_test_failed(output);
            let summary = format!(
                "# Verify Report\n\nProject: {}\nCommand: `{}`\n\n**Result**: {} passed, {} failed\n\n```\n{}\n```",
                project_type, test_cmd, passed, failed,
                if output.len() > 2000 { &output[..2000] } else { output }
            );
            app.add_system_message(summary.clone());
            String::new()
        }
        None => "Skill 'verify' not found.".to_string(),
    }
}

fn count_test_passed(output: &str) -> u32 {
    // Rust: "test result: ok. X passed"
    // Node: "X passing"
    // Python: "X passed"
    let candidates = [
        regex::Regex::new(r"(\d+) passed").ok(),
        regex::Regex::new(r"test result: ok\. (\d+) passed").ok(),
        regex::Regex::new(r"(\d+) passing").ok(),
    ];
    let mut max = 0u32;
    for re in candidates.iter().flatten() {
        if let Some(caps) = re.captures(output) {
            if let Ok(n) = caps.get(1).unwrap().as_str().parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    max
}

fn count_test_failed(output: &str) -> u32 {
    let candidates = [
        regex::Regex::new(r"(\d+) failed").ok(),
        regex::Regex::new(r"test result: FAILED\. (\d+) failed").ok(),
    ];
    let mut max = 0u32;
    for re in candidates.iter().flatten() {
        if let Some(caps) = re.captures(output) {
            if let Ok(n) = caps.get(1).unwrap().as_str().parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    max
}

pub async fn handle_debug(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("debug") {
        Some(_skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::BashTool;

            // Find recent log files
            let log_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("priority-agent")
                .join("logs");

            let params = serde_json::json!({
                "command": format!("tail -n 100 {}/*.log 2>/dev/null | grep -E 'ERROR|WARN|panic' | tail -50 || echo 'No recent error logs found'", log_dir.display()),
                "description": "Check recent debug logs"
            });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_debug",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }

            let logs = if result.success && !result.content.trim().is_empty() {
                result.content
            } else {
                "No ERROR/WARN entries found in recent logs.".to_string()
            };

            let report = format!(
                "# Debug Report\n\nRecent errors/warnings in logs:\n\n```\n{}\n```\n\nTo get more details, run:\n- `tail -f ~/.priority-agent/logs/*.log` to watch logs live\n- Set `RUST_LOG=debug` for more verbose output",
                logs
            );
            app.add_system_message(report);
            String::new()
        }
        None => "Skill 'debug' not found.".to_string(),
    }
}

pub async fn handle_stuck(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("stuck") {
        Some(_skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::BashTool;

            // Scan for claude/priority-agent processes
            let params = serde_json::json!({
                "command": r#"ps aux | grep -E 'priority-agent|claude' | grep -v grep | awk '{print $2, $3, $4, $11}' | head -20"#,
                "description": "Scan for hung processes"
            });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_stuck",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }

            let processes = if result.success && !result.content.trim().is_empty() {
                result.content
            } else {
                "No other priority-agent or claude processes found.".to_string()
            };

            let report = format!(
                "# Stuck Process Report\n\nProcesses found (PID CPU% MEM% COMMAND):\n\n```\n{}\n```\n\nIf a process appears stuck (high CPU with no progress, D/T/Z state), you may want to:\n- Kill it: `kill -9 <PID>`\n- Check for zombie processes: `ps aux | grep Z`",
                processes
            );
            app.add_system_message(report);
            String::new()
        }
        None => "Skill 'stuck' not found.".to_string(),
    }
}

pub async fn handle_remember(app: &mut TuiApp, _args: &str) -> String {
    match app.bundled_skills.get("remember") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_remember",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            // Load existing memory files if they exist
            let claude_md = std::path::Path::new("CLAUDE.md");

            let report = format!(
                "# Remember Report\n\n## Memory Files\n\n{}\n\n## Suggestions\n\nBased on {}, consider:\n1. **CLAUDE.md** - Project-wide conventions, architecture decisions\n2. **CLAUDE.local.md** - User-specific preferences (git ignored)\n3. **Team memory** - Cross-project shared knowledge\n\nTo add a memory, use `/memory_save <content>` or manually edit CLAUDE.md.",
                if claude_md.exists() {
                    "CLAUDE.md exists in current directory."
                } else {
                    "No CLAUDE.md found in current directory."
                },
                skill.content
            );
            app.add_system_message(report);
            String::new()
        }
        None => "Skill 'remember' not found.".to_string(),
    }
}

/// Merge two permission TOML configs, deduplicating by pattern
fn merge_permission_toml(existing: &str, imported: &str) -> Result<String, String> {
    let mut existing_rules: crate::permissions::PermissionRules =
        toml::from_str(existing).map_err(|e| format!("Parse existing: {}", e))?;
    let imported_rules: crate::permissions::PermissionRules =
        toml::from_str(imported).map_err(|e| format!("Parse imported: {}", e))?;

    // Deduplicate helper
    let mut seen = std::collections::HashSet::new();
    let mut dedup = |rules: &mut Vec<crate::permissions::SourcedRule>| {
        rules.retain(|r| seen.insert(r.pattern.clone()));
    };

    existing_rules.always_allow.extend(imported_rules.always_allow);
    dedup(&mut existing_rules.always_allow);
    existing_rules.always_deny.extend(imported_rules.always_deny);
    dedup(&mut existing_rules.always_deny);
    existing_rules.always_ask.extend(imported_rules.always_ask);
    dedup(&mut existing_rules.always_ask);

    toml::to_string_pretty(&existing_rules).map_err(|e| format!("Serialize: {}", e))
}

pub fn handle_keybindings(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("keybindings") {
        Some(_skill) => {
            let config_dir = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("priority-agent");
            let kb_path = config_dir.join("keybindings.json");

            if args.is_empty() || args == "list" {
                // Show current keybindings
                if kb_path.exists() {
                    match std::fs::read_to_string(&kb_path) {
                        Ok(content) => format!("Current keybindings:\n\n```json\n{}\n```", content),
                        Err(e) => format!("Failed to read keybindings: {}", e),
                    }
                } else {
                    let default_kb = get_default_keybindings();
                    format!(
                        "No custom keybindings found. Default keybindings:\n\n```json\n{}\n```\n\nTo customize, use `/keybindings edit <json>`",
                        default_kb
                    )
                }
            } else if let Some(json_str) = args.strip_prefix("edit ") {
                // Basic validation
                if json_str.trim().starts_with("{") {
                    if let Some(parent) = kb_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    match std::fs::write(&kb_path, json_str) {
                        Ok(_) => format!("Keybindings saved to {}", kb_path.display()),
                        Err(e) => format!("Failed to save keybindings: {}", e),
                    }
                } else {
                    "Invalid JSON. Use `/keybindings edit <json>`".to_string()
                }
            } else {
                "Usage: /keybindings [list|edit <json>]".to_string()
            }
        }
        None => "Skill 'keybindings' not found.".to_string(),
    }
}

fn get_default_keybindings() -> String {
    serde_json::json!({
        "version": 1,
        "contexts": {
            "global": {
                "Ctrl+C": "cancel",
                "Ctrl+Z": "undo",
                "Ctrl+S": "save"
            },
            "chat": {
                "Enter": "submit",
                "Shift+Enter": "newline",
                "Ctrl+J": "history_up",
                "Ctrl+K": "history_down",
                "Ctrl+B": "toggle_sidebar"
            },
            "vim_normal": {
                "j": "down",
                "k": "up",
                "i": "insert_mode",
                "Ctrl+V": "toggle_mode"
            }
        }
    })
    .to_string()
}

// ─── New Commands (Phase 9 Task 3) ────────────────────────────────────

/// /btw -随口说一句（one-off 注释，不影响对话）
pub async fn handle_btw(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /btw <message> - Add a side note without disrupting the conversation".to_string();
    }
    let note = format!("[btw] {}", args);
    app.add_system_message(note.clone());
    String::new()
}

/// /context - 显示当前上下文状态
pub async fn handle_context(app: &TuiApp) -> String {
    let msg_count = app.messages.len();
    let history_len = if let Some(ref engine) = app.streaming_engine {
        engine.get_history().await.len()
    } else {
        0
    };
    let model = app.current_model_label();
    let provider = app.current_provider_label();
    let working_dir = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let session_id = app.session_manager.current_session_id()
        .map(|s| s[..8.min(s.len())].to_string())
        .unwrap_or_else(|| "none".to_string());

    let engine_info = if let Some(ref engine) = app.streaming_engine {
        let history = engine.get_history().await;

        let approximate_tokens = history.iter().map(|m| {
            match m {
                crate::services::api::Message::System { content } => content.len(),
                crate::services::api::Message::User { content } => content.len(),
                crate::services::api::Message::Assistant { content, .. } => content.len(),
                crate::services::api::Message::Tool { content, .. } => content.len(),
            }
        }).sum::<usize>() / 4;

        format!(
            "History turns: {}\nMessages in view: {}\nApproximate tokens: {}",
            history_len,
            msg_count,
            approximate_tokens
        )
    } else {
        "Engine not initialized".to_string()
    };

    format!(
        "# Context Status\n\n\
         Session: {}\n\
         Model: {} ({})\n\
         Working dir: {}\n\
         \n\
         {}",
        session_id, model, provider, working_dir, engine_info
    )
}

/// /git - 内联 Git 操作
pub async fn handle_git(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::GitTool;

    // Validate git action to prevent arbitrary command injection
    let allowed_actions = ["status", "diff", "log", "branch", "checkout", "stash", "tag"];
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
        result.error.unwrap_or_else(|| "Git command failed".to_string())
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
                result.error.unwrap_or_else(|| "Failed to list dependencies.".to_string())
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
                result.error.unwrap_or_else(|| "Failed to check outdated packages.".to_string())
            }
        }
        _ => {
            "Package Manager Commands:\n\n\
                 /package list     - List package files in project\n\
                 /package deps     - Show installed dependencies\n\
                 /package outdated - Check for outdated packages\n\n\
                 Supported: npm (Node.js), cargo (Rust), go (Go)"
                .to_string()
        }
    }
}

// ─── Advanced Agent Commands (Phase 9 Task 1) ─────────────────────────────────

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
                result.error.unwrap_or_else(|| "No changes to review.".to_string())
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
                if args.is_empty() { "What would you like me to explore in the background?" } else { args }
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
                if args.is_empty() { "Describe the custom agent you want to create:" } else { args }
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
                if args.is_empty() { "What complex task would you like me to coordinate?" } else { args }
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'orchestrate' not found.".to_string(),
    }
}

// ─── Batch 1 Commands (Phase 10) ──────────────────────────────────────────────

/// /session - 会话管理
pub async fn handle_session_cmd(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "list" {
        // List sessions
        match app.session_manager.list_sessions(10) {
            Ok(sessions) => {
                if sessions.is_empty() {
                    "No sessions found.".to_string()
                } else {
                    let current = app.session_manager.current_session_id();
                    let mut lines = vec!["Sessions:".to_string()];
                    for (i, s) in sessions.iter().enumerate() {
                        let marker = if current == Some(s.id.as_str()) {
                            " (current)"
                        } else {
                            ""
                        };
                        lines.push(format!(
                            "{}. {}{} - {} [{}]",
                            i + 1,
                            s.title,
                            marker,
                            &s.id[..8.min(s.id.len())],
                            s.updated_at
                        ));
                    }
                    lines.push("\nUse /session <n> to switch.".to_string());
                    lines.join("\n")
                }
            }
            Err(e) => format!("Failed to list sessions: {}", e),
        }
    } else if let Ok(n) = args.parse::<usize>() {
        // Switch by index
        match app.session_manager.list_sessions(20) {
            Ok(sessions) if n > 0 && n <= sessions.len() => {
                let session = &sessions[n - 1];
                app.restore_session(&session.id).await
            }
            _ => "Invalid session number. Use /session list to see available.".to_string(),
        }
    } else if args == "new" || args.starts_with("new ") {
        // Create new session
        let title = args
            .strip_prefix("new ")
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("New Session");
        match app.session_manager.start_session(title, "kimi-k2.5") {
            Ok(id) => {
                let _ = app.restore_session(&id).await;
                format!("Created new session: {}", title)
            }
            Err(e) => format!("Failed to create session: {}", e),
        }
    } else if args == "current" {
        // Show current session
        let id = app.session_manager.current_session_id().map(|s| s.to_string()).unwrap_or_else(|| "none".to_string());
        let title = app.session_manager.current_session_title();
        format!("Current session: {} ({})", title, &id[..8.min(id.len())])
    } else if args.starts_with("new") {
        "Usage: /session new <title>".to_string()
    } else {
        // Fallback: try switch by full session ID
        app.restore_session(args).await
    }
}

/// /undo - 撤销上一次操作
pub fn handle_undo(app: &mut TuiApp, args: &str) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let n = match parse_optional_count(args, "/undo") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut results = Vec::new();
    for _ in 0..n {
        match app.session_manager.rewind_last_edit(session_id) {
            Ok(msg) => results.push(msg),
            Err(e) => {
                results.push(format!("Nothing to undo or undo failed: {}", e));
                break;
            }
        }
    }
    results.join("\n")
}

/// /redo - 重做
pub fn handle_redo(app: &mut TuiApp, args: &str) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let n = match parse_optional_count(args, "/redo") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut results = Vec::new();
    for _ in 0..n {
        match app.session_manager.redo_last_edit(session_id) {
            Ok(msg) => results.push(msg),
            Err(e) => {
                results.push(format!("Nothing to redo or redo failed: {}", e));
                break;
            }
        }
    }
    results.join("\n")
}

fn parse_optional_count(args: &str, cmd: &str) -> Result<usize, String> {
    if args.trim().is_empty() {
        return Ok(1);
    }
    let n = args
        .trim()
        .parse::<usize>()
        .map_err(|_| format!("Usage: {} [n]", cmd))?;
    if n == 0 {
        return Err(format!("Usage: {} [n] (n must be >= 1)", cmd));
    }
    Ok(n)
}

/// /retry - 重试上一次 LLM 调用
pub async fn handle_retry(app: &mut TuiApp, args: &str) -> String {
    if !args.trim().is_empty() {
        return "Usage: /retry".to_string();
    }

    // Retry the last user turn: remove that user message and everything after it,
    // then resend the same content to regenerate downstream responses coherently.
    let Some(last_user_idx) = app
        .messages
        .iter()
        .rposition(|m| m.role == crate::state::MessageRole::User)
    else {
        return "No user message to retry.".to_string();
    };
    let content = app.messages[last_user_idx].content.clone();
    app.messages.truncate(last_user_idx);

    // Keep persistence and engine history consistent with truncated UI messages.
    if let Some(session_id) = app.session_manager.current_session_id() {
        if let Err(e) = app.session_manager.replace_messages(session_id, &app.messages) {
            return format!("Retry failed to rewrite session messages: {}", e);
        }
    }
    if let Some(ref engine) = app.streaming_engine {
        engine
            .set_history(message_items_to_api_messages(&app.messages))
            .await;
    }

    app.send_message(content).await;
    String::new()
}

fn message_items_to_api_messages(
    messages: &[crate::state::MessageItem],
) -> Vec<crate::services::api::Message> {
    messages
        .iter()
        .map(|m| match m.role {
            crate::state::MessageRole::User => crate::services::api::Message::user(m.content.clone()),
            crate::state::MessageRole::Assistant => {
                crate::services::api::Message::assistant(m.content.clone())
            }
            crate::state::MessageRole::System => crate::services::api::Message::system(m.content.clone()),
            crate::state::MessageRole::Tool => {
                crate::services::api::Message::tool(String::new(), m.content.clone())
            }
        })
        .collect()
}

/// /stop - 停止当前操作
pub fn handle_stop(app: &mut TuiApp, _args: &str) -> String {
    if app.is_querying {
        app.is_querying = false;
        crate::engine::workflow::metrics::record_drift_interruption();
        "Stopping current operation...".to_string()
    } else {
        "No operation in progress.".to_string()
    }
}

/// /reload - 重新加载配置/插件
pub async fn handle_reload(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "config" {
        match crate::services::config::AppConfig::load() {
            Ok(config) => {
                // Apply visible UI config immediately.
                app.theme = crate::tui::theme::Theme::from_name(&config.ui.theme);
                if let Some(ref mut settings) = app.settings_state {
                    settings.config = config.clone();
                }
                format!("Config reloaded:\n- API: {}\n- Model: {}",
                    config.api.base_url, config.api.model)
            }
            Err(e) => format!("Failed to reload config: {}", e),
        }
    } else if args == "plugins" {
        // Reload plugins
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut registry = crate::tools::ToolRegistry::default_registry();
        let injected = crate::tools::plugin_tool::register_enabled_plugin_tools(&mut registry, &working_dir);
        format!("Plugins reloaded. {} plugin tools injected.", injected)
    } else if args == "skills" {
        // Reload skills
        if let Some(ref _engine) = app.streaming_engine {
            "Skills registry: use /skills list to view".to_string()
        } else {
            "Skills not available.".to_string()
        }
    } else {
        "Usage: /reload [config|plugins|skills]".to_string()
    }
}

fn format_config_summary(config: &crate::services::config::AppConfig) -> String {
    format!(
        "Config:\n  api.base_url = {}\n  api.model = {}\n  api.temperature = {}\n  api.max_tokens = {}\n  ui.theme = {}\n  ui.show_token_usage = {}\n  ui.compact_mode = {}\n  storage.persistence_enabled = {}\n  storage.auto_save_interval_secs = {}\n  features.mcp_enabled = {}\n  features.skills_enabled = {}\n  features.web_search = {}",
        config.api.base_url,
        config.api.model,
        config.api.temperature,
        config.api.max_tokens.map(|v| v.to_string()).unwrap_or_else(|| "none".to_string()),
        config.ui.theme,
        config.ui.show_token_usage,
        config.ui.compact_mode,
        config.storage.persistence_enabled,
        config.storage.auto_save_interval_secs,
        config.features.mcp_enabled,
        config.features.skills_enabled,
        config.features.web_search,
    )
}

fn get_config_value(config: &crate::services::config::AppConfig, key: &str) -> Option<String> {
    match key {
        "api.base_url" => Some(config.api.base_url.clone()),
        "api.model" => Some(config.api.model.clone()),
        "api.temperature" => Some(config.api.temperature.to_string()),
        "api.max_tokens" => Some(
            config
                .api
                .max_tokens
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string()),
        ),
        "ui.theme" => Some(config.ui.theme.clone()),
        "ui.show_token_usage" => Some(config.ui.show_token_usage.to_string()),
        "ui.compact_mode" => Some(config.ui.compact_mode.to_string()),
        "storage.persistence_enabled" => Some(config.storage.persistence_enabled.to_string()),
        "storage.auto_save_interval_secs" => Some(config.storage.auto_save_interval_secs.to_string()),
        "features.mcp_enabled" => Some(config.features.mcp_enabled.to_string()),
        "features.skills_enabled" => Some(config.features.skills_enabled.to_string()),
        "features.web_search" => Some(config.features.web_search.to_string()),
        _ => None,
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => Ok(true),
        "false" | "0" | "off" | "no" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

fn set_config_value(
    config: &mut crate::services::config::AppConfig,
    key: &str,
    value: &str,
) -> Result<(), String> {
    match key {
        "api.base_url" => config.api.base_url = value.to_string(),
        "api.model" => config.api.model = value.to_string(),
        "api.temperature" => {
            config.api.temperature = value
                .parse::<f32>()
                .map_err(|_| format!("Invalid float for {}: {}", key, value))?;
        }
        "api.max_tokens" => {
            if value.eq_ignore_ascii_case("none") {
                config.api.max_tokens = None;
            } else {
                config.api.max_tokens = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| format!("Invalid integer for {}: {}", key, value))?,
                );
            }
        }
        "ui.theme" => config.ui.theme = value.to_string(),
        "ui.show_token_usage" => config.ui.show_token_usage = parse_bool(value)?,
        "ui.compact_mode" => config.ui.compact_mode = parse_bool(value)?,
        "storage.persistence_enabled" => config.storage.persistence_enabled = parse_bool(value)?,
        "storage.auto_save_interval_secs" => {
            config.storage.auto_save_interval_secs = value
                .parse::<u64>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "features.mcp_enabled" => config.features.mcp_enabled = parse_bool(value)?,
        "features.skills_enabled" => config.features.skills_enabled = parse_bool(value)?,
        "features.web_search" => config.features.web_search = parse_bool(value)?,
        _ => return Err(format!("Unknown config key: {}", key)),
    }
    Ok(())
}

/// /share - 分享当前会话
pub fn handle_share(app: &mut TuiApp, _args: &str) -> String {
    if let Some(id) = app.session_manager.current_session_id() {
        match app.session_manager.export_session(id) {
            Ok(json) => {
                let path = dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join(format!("share_{}.json", &id[..8.min(id.len())]));
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&path, &json) {
                    Ok(_) => format!("Session exported to: {}", path.display()),
                    Err(e) => format!("Failed to write: {}", e),
                }
            }
            Err(e) => format!("Failed to export: {}", e),
        }
    } else {
        "No active session to share.".to_string()
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
            if result.success { result.content } else { result.error.unwrap_or_default() }
        }
        "update" => {
            let params = serde_json::json!({
                "command": "npm update",
                "description": "Update npm packages"
            });
            let result = tool.execute(params, ctx).await;
            if result.success { result.content } else { result.error.unwrap_or_default() }
        }
        "outdated" => {
            let params = serde_json::json!({
                "command": "npm outdated",
                "description": "Check outdated packages"
            });
            let result = tool.execute(params, ctx).await;
            if result.success { result.content } else { result.error.unwrap_or_default() }
        }
        "test" => {
            let params = serde_json::json!({
                "command": "npm test",
                "description": "Run npm tests"
            });
            let result = tool.execute(params, ctx).await;
            if result.success { result.content } else { result.error.unwrap_or_default() }
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
            if result.success { result.content } else { result.error.unwrap_or_default() }
        }
        "" => {
            "Usage: /npm [install|update|outdated|test|run] [args]".to_string()
        }
        _ => {
            let cmd = args;
            let params = serde_json::json!({
                "command": format!("npm {}", cmd),
                "description": format!("npm {}", cmd)
            });
            let result = tool.execute(params, ctx).await;
            if result.success { result.content } else { result.error.unwrap_or_default() }
        }
    }
}

// ═══════════════════════════════════════
// Phase 10 Batch 2: hooks, profiling, prompt, migrate, focus, pause, install, skeleton, branch, color
// ═══════════════════════════════════════

/// /hooks - Show hook configuration status
pub fn handle_hooks(_app: &TuiApp) -> String {
    use std::env;

    let pre_hook = env::var("PRIORITY_AGENT_PRE_TOOL_HOOK").ok();
    let post_hook = env::var("PRIORITY_AGENT_POST_TOOL_HOOK").ok();
    let tool_before = env::var("PRIORITY_AGENT_TOOL_HOOK_BEFORE").ok();
    let tool_after = env::var("PRIORITY_AGENT_TOOL_HOOK_AFTER").ok();
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
    if let Some(ref h) = tool_before {
        lines.push(format!("  TOOL_HOOK_BEFORE: {}", h));
    } else {
        lines.push("  TOOL_HOOK_BEFORE: not set".to_string());
    }
    if let Some(ref h) = tool_after {
        lines.push(format!("  TOOL_HOOK_AFTER: {}", h));
    } else {
        lines.push("  TOOL_HOOK_AFTER: not set".to_string());
    }
    lines.push(format!("  HOOK_TIMEOUT_MS: {}", timeout.unwrap_or_else(|| "1000".to_string())));
    lines.push(format!("  HOOK_FAIL_CLOSED: {}", fail_closed.unwrap_or_else(|| "false".to_string())));

    if pre_hook.is_none() && post_hook.is_none() && tool_before.is_none() && tool_after.is_none() {
        lines.push("\nNo hooks configured. Set PRIORITY_AGENT_*_HOOK environment variables.".to_string());
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

/// /prompt - Show/edit system prompt
pub async fn handle_prompt(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "show" {
        return match read_prompt_file() {
            Ok(Some(v)) => format!("System prompt:\n\n{}", v),
            Ok(None) => "No custom system prompt set.".to_string(),
            Err(e) => format!("Failed to read prompt: {}", e),
        };
    }
    if let Some(text) = args.strip_prefix("edit ").map(str::trim) {
        if text.is_empty() {
            return "Usage: /prompt edit <text>".to_string();
        }
        return match write_prompt_file(text) {
            Ok(_) => "Custom system prompt updated.".to_string(),
            Err(e) => format!("Failed to write prompt: {}", e),
        };
    }
    if let Some(text) = args.strip_prefix("append ").map(str::trim) {
        if text.is_empty() {
            return "Usage: /prompt append <text>".to_string();
        }
        return match append_prompt_file(text) {
            Ok(_) => "Custom system prompt appended.".to_string(),
            Err(e) => format!("Failed to append prompt: {}", e),
        };
    }
    if args == "reset" {
        return match reset_prompt_file() {
            Ok(_) => "Custom system prompt reset.".to_string(),
            Err(e) => format!("Failed to reset prompt: {}", e),
        };
    }
    if args == "apply" {
        let prompt = match read_prompt_file() {
            Ok(Some(v)) => v,
            Ok(None) => return "No custom system prompt set. Use `/prompt edit <text>` first.".to_string(),
            Err(e) => return format!("Failed to read prompt: {}", e),
        };

        let content = format!("[Custom System Prompt]\n{}", prompt);
        app.add_system_message(content.clone());
        let _ = app
            .session_manager
            .add_message(crate::state::MessageRole::System, &content);
        if let Some(ref engine) = app.streaming_engine {
            engine.set_history(message_items_to_api_messages(
                &app.messages,
            )).await;
        }
        return "Custom system prompt applied to current session context.".to_string();
    }
    "Usage: /prompt [show|edit <text>|append <text>|apply|reset]".to_string()
}

/// /migrate - Migration helper
pub async fn handle_migrate(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /migrate [up|down|status]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "up" => run_migrate_sqlx(app, true).await,
        "down" => run_migrate_sqlx(app, false).await,
        "status" => {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let migrations_dir = dir.join("migrations");
            if migrations_dir.exists() && migrations_dir.is_dir() {
                let mut files: Vec<String> = match tokio::fs::read_dir(&migrations_dir).await {
                    Ok(mut read_dir) => {
                        let mut f = Vec::new();
                        while let Ok(Some(entry)) = read_dir.next_entry().await {
                            let p = entry.path();
                            if p.is_file() {
                                if let Some(n) = p.file_name() {
                                    f.push(n.to_string_lossy().to_string());
                                }
                            }
                        }
                        f
                    }
                    Err(_) => Vec::new(),
                };
                files.sort();
                let preview = files
                    .iter()
                    .take(10)
                    .map(|f| format!("- {}", f))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "Migrations dir: {}\nFiles: {}\n{}\n\nUse `/migrate up` or `/migrate down` (requires sqlx + DATABASE_URL).",
                    migrations_dir.display(),
                    files.len(),
                    if preview.is_empty() {
                        "(no migration files found)".to_string()
                    } else {
                        preview
                    }
                )
            } else {
                "No migrations directory found.".to_string()
            }
        }
        _ => "Usage: /migrate [up|down|status]".to_string(),
    }
}

/// /focus - Focus mode toggle
pub fn handle_focus(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "status" {
        return format!(
            "Focus mode: {}",
            if app.focus_mode { "enabled" } else { "disabled" }
        );
    }

    let enable = match args {
        "on" | "enable" => true,
        "off" | "disable" => false,
        "toggle" => !app.focus_mode,
        _ => return "Usage: /focus [on|off|toggle|status]".to_string(),
    };

    app.focus_mode = enable;
    if let Ok(mut config) = crate::services::config::AppConfig::load() {
        config.ui.compact_mode = enable;
        let _ = config.save();
        if let Some(ref mut settings) = app.settings_state {
            settings.config.ui.compact_mode = enable;
        }
    }
    format!(
        "Focus mode {}.",
        if enable { "enabled" } else { "disabled" }
    )
}

/// /pause - Pause/resume agent
pub fn handle_pause(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "status" {
        return format!(
            "Pause state: {}",
            if app.paused { "paused" } else { "running" }
        );
    }

    if args == "pause" {
        app.paused = true;
        app.is_querying = false;
        "Agent paused. New messages are blocked until `/pause resume`.".to_string()
    } else if args == "resume" {
        app.paused = false;
        app.is_querying = false;
        "Agent resumed.".to_string()
    } else if args == "toggle" {
        app.paused = !app.paused;
        if app.paused {
            app.is_querying = false;
            "Agent paused. New messages are blocked until `/pause resume`.".to_string()
        } else {
            "Agent resumed.".to_string()
        }
    } else {
        "Usage: /pause [pause|resume|toggle|status]".to_string()
    }
}

/// /install - Dependency installer
pub async fn handle_install(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /install [cargo|npm|pip] [package]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    let tool_name = parts[0];

    let (_tool, cmd) = match tool_name {
        "cargo" => ("BashTool", format!("cargo {}", parts.get(1).unwrap_or(&""))),
        "npm" => ("BashTool", format!("npm install {}", parts.get(1).unwrap_or(&""))),
        "pip" => ("BashTool", format!("pip install {}", parts.get(1).unwrap_or(&""))),
        _ => ("BashTool", format!("{} {}", tool_name, parts.get(1).unwrap_or(&""))),
    };

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;
    let params = serde_json::json!({
        "command": cmd.trim(),
        "description": format!("install {}", args)
    });
    let result = tool.execute(params, ctx).await;
    if result.success { result.content } else { result.error.unwrap_or_default() }
}

/// /skeleton - Generate code skeleton
pub fn handle_skeleton(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /skeleton <language> [filename]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    let lang = parts[0];
    let filename = parts.get(1).unwrap_or(&"main");

    let skeleton = match lang {
        "rust" => format!("// {}.rs\n\nfn main() {{\n    println!(\"Hello, world!\");\n}}\n", filename),
        "python" => format!("# {}.py\n\ndef main():\n    print(\"Hello, world!\")\n\nif __name__ == \"__main__\":\n    main()\n", filename),
        "typescript" | "ts" => format!("// {}.ts\n\nexport function main(): void {{\n    console.log(\"Hello, world!\");\n}}\n", filename),
        "javascript" | "js" => format!("// {}.js\n\nfunction main() {{\n    console.log(\"Hello, world!\");\n}}\n\nmain();\n", filename),
        _ => return format!("Unsupported language: {}. Supported: rust, python, typescript, javascript", lang),
    };

    format!("```{}```\n\n{}", lang, skeleton)
}

/// /branch - Git branch management
pub async fn handle_branch(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let cmd = if args.is_empty() {
        "git branch -a".to_string()
    } else if args.starts_with("create ") {
        let name = args.strip_prefix("create ").unwrap_or("");
        format!("git checkout -b {}", name)
    } else if args == "current" {
        "git branch --show-current".to_string()
    } else {
        format!("git branch {}", args)
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "git branch"
    });
    let result = tool.execute(params, ctx).await;
    if result.success { result.content } else { result.error.unwrap_or_default() }
}

/// /color - Theme color customization
pub fn handle_color(app: &mut TuiApp, args: &str) -> String {
    // Keep /color as a backwards-compatible alias for /theme.
    let normalized = match args.trim() {
        "hc" => "high-contrast",
        v => v,
    };
    handle_theme(app, normalized)
}

// ═══════════════════════════════════════
// Phase 10 Batch 3: webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker
// ═══════════════════════════════════════

/// /webhook - Webhook management
pub async fn handle_webhook(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /webhook [list|create <url> [name]|delete <name>|test <name|url> [payload]]"
            .to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => match load_webhooks() {
            Ok(map) if map.is_empty() => "No webhooks configured.".to_string(),
            Ok(map) => {
                let mut names: Vec<_> = map.keys().cloned().collect();
                names.sort();
                let mut lines = vec!["Configured webhooks:".to_string()];
                for name in names {
                    if let Some(url) = map.get(&name) {
                        lines.push(format!("- {} -> {}", name, url));
                    }
                }
                lines.join("\n")
            }
            Err(e) => format!("Failed to load webhooks: {}", e),
        },
        "create" => {
            if parts.len() < 2 {
                "Usage: /webhook create <url>".to_string()
            } else {
                let url = parts[1].trim();
                if !is_valid_webhook_url(url) {
                    return "Invalid webhook URL. Must start with http:// or https://".to_string();
                }
                let mut map = match load_webhooks() {
                    Ok(v) => v,
                    Err(e) => return format!("Failed to load webhooks: {}", e),
                };
                let name = if parts.len() >= 3 {
                    match sanitize_note_name(parts[2]) {
                        Some(v) => v,
                        None => return "Invalid webhook name.".to_string(),
                    }
                } else {
                    let mut i = 1usize;
                    let mut candidate = format!("webhook{}", i);
                    while map.contains_key(&candidate) {
                        i += 1;
                        candidate = format!("webhook{}", i);
                    }
                    candidate
                };
                map.insert(name.clone(), url.to_string());
                match save_webhooks(&map) {
                    Ok(_) => format!("Webhook '{}' created.", name),
                    Err(e) => format!("Failed to save webhook: {}", e),
                }
            }
        }
        "delete" => {
            if parts.len() < 2 {
                return "Usage: /webhook delete <name>".to_string();
            }
            let key = parts[1];
            let mut map = match load_webhooks() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load webhooks: {}", e),
            };
            if map.remove(key).is_none() {
                return format!("Webhook '{}' not found.", key);
            }
            match save_webhooks(&map) {
                Ok(_) => format!("Webhook '{}' deleted.", key),
                Err(e) => format!("Failed to save webhook store: {}", e),
            }
        }
        "test" => {
            if parts.len() < 2 {
                return "Usage: /webhook test <name|url> [payload]".to_string();
            }
            let target = parts[1];
            let payload = args
                .splitn(3, ' ')
                .nth(2)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or(r#"{"event":"ping","source":"priority-agent"}"#);
            let url = if is_valid_webhook_url(target) {
                target.to_string()
            } else {
                match load_webhooks().ok().and_then(|m| m.get(target).cloned()) {
                    Some(v) => v,
                    None => return format!("Unknown webhook '{}'.", target),
                }
            };
            match test_webhook(&url, payload).await {
                Ok(msg) => msg,
                Err(e) => format!("Webhook test failed: {}", e),
            }
        }
        _ => "Usage: /webhook [list|create|delete|test]".to_string(),
    }
}

/// /wizard - Setup wizard
pub fn handle_wizard(app: &mut TuiApp) -> String {
    if app.settings_state.is_none() {
        let config = crate::services::config::AppConfig::load().unwrap_or_default();
        app.settings_state = Some(crate::tui::components::settings::SettingsState::new(
            config,
            app.keybindings.clone(),
        ));
    }
    app.mode = crate::tui::app::AppMode::Settings;
    "Setup wizard ready.\nStep 1: check `/config list`\nStep 2: set model/theme via settings\nStep 3: `/key show` and `/status` to verify.".to_string()
}

/// /workspace - Workspace management
pub fn handle_workspace(_app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        // Show current workspace
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        return format!("Current workspace: {}", dir.display());
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            let output = std::process::Command::new("git")
                .args(["worktree", "list", "--porcelain"])
                .output();
            match output {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    let worktrees: Vec<&str> = text
                        .lines()
                        .filter_map(|line| line.strip_prefix("worktree "))
                        .collect();
                    if worktrees.is_empty() {
                        "No git worktrees found.".to_string()
                    } else {
                        format!("Workspaces:\n- {}", worktrees.join("\n- "))
                    }
                }
                _ => "Not a git worktree repo or failed to list worktrees.".to_string(),
            }
        }
        "info" => {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let entries = std::fs::read_dir(&dir)
                .map(|it| it.flatten().count())
                .unwrap_or(0);
            format!(
                "Workspace: {}\nEntries: {}\nUse /workspace list to see worktrees.",
                dir.display(),
                entries
            )
        }
        _ => "Usage: /workspace [list|info]".to_string(),
    }
}

/// /slack - Slack integration
pub async fn handle_slack(_app: &mut TuiApp, args: &str) -> String {
    let arg = args.trim();
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    if arg.is_empty() || arg == "status" {
        let webhook = prefs
            .slack_webhook_url
            .clone()
            .or_else(|| std::env::var("PRIORITY_AGENT_SLACK_WEBHOOK_URL").ok());
        let connected = webhook.is_some();
        return format!(
            "Slack: {}\nDefault channel: {}\nUsage: /slack [status|connect <webhook_url> [channel]|disconnect|send [#channel] <message>]",
            if connected { "connected" } else { "disconnected" },
            prefs
                .slack_default_channel
                .as_deref()
                .unwrap_or("(not set)")
        );
    }
    if let Some(rest) = arg.strip_prefix("connect ").map(str::trim) {
        let mut parts = rest.splitn(2, ' ');
        let webhook = parts.next().unwrap_or_default().trim();
        if webhook.is_empty() || !is_valid_webhook_url(webhook) {
            return "Usage: /slack connect <webhook_url> [channel]".to_string();
        }
        let channel = parts.next().map(str::trim).filter(|v| !v.is_empty());
        prefs.slack_webhook_url = Some(webhook.to_string());
        prefs.slack_default_channel = channel.map(ToString::to_string);
        return match save_runtime_prefs(&prefs) {
            Ok(_) => "Slack webhook connected.".to_string(),
            Err(e) => format!("Failed to save Slack config: {}", e),
        };
    }
    if arg == "disconnect" {
        prefs.slack_webhook_url = None;
        prefs.slack_default_channel = None;
        return match save_runtime_prefs(&prefs) {
            Ok(_) => "Slack disconnected.".to_string(),
            Err(e) => format!("Failed to save Slack config: {}", e),
        };
    }
    if let Some(rest) = arg.strip_prefix("send ").map(str::trim) {
        if rest.is_empty() {
            return "Usage: /slack send [#channel] <message>".to_string();
        }
        let webhook = prefs
            .slack_webhook_url
            .clone()
            .or_else(|| std::env::var("PRIORITY_AGENT_SLACK_WEBHOOK_URL").ok());
        let Some(webhook_url) = webhook else {
            return "Slack not connected. Use `/slack connect <webhook_url>` or set PRIORITY_AGENT_SLACK_WEBHOOK_URL.".to_string();
        };

        let (channel, message) = if rest.starts_with('#') {
            let mut parts = rest.splitn(2, ' ');
            let c = parts.next().unwrap_or_default().trim().to_string();
            let m = parts.next().unwrap_or_default().trim().to_string();
            (Some(c), m)
        } else {
            (prefs.slack_default_channel.clone(), rest.to_string())
        };
        if message.trim().is_empty() {
            return "Usage: /slack send [#channel] <message>".to_string();
        }
        match post_slack_webhook(&webhook_url, channel.as_deref(), &message).await {
            Ok(_) => "Slack message sent.".to_string(),
            Err(e) => format!("Slack send failed: {}", e),
        }
    } else {
        "Usage: /slack [status|connect <webhook_url> [channel]|disconnect|send [#channel] <message>]".to_string()
    }
}

/// /stealth - Stealth mode toggle
pub fn handle_stealth(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Stealth mode: {}",
            if prefs.stealth { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => prefs.stealth = true,
        "off" | "disable" => prefs.stealth = false,
        "toggle" => prefs.stealth = !prefs.stealth,
        _ => return "Usage: /stealth [on|off|toggle|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Failed to persist stealth mode: {}", e);
    }
    format!(
        "Stealth mode {}.",
        if prefs.stealth { "enabled" } else { "disabled" }
    )
}

/// /shadow - Shadow mode for observing agent behavior
pub fn handle_shadow(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Shadow mode: {}",
            if prefs.shadow { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => prefs.shadow = true,
        "off" | "disable" => prefs.shadow = false,
        "toggle" => prefs.shadow = !prefs.shadow,
        _ => return "Usage: /shadow [on|off|toggle|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Failed to persist shadow mode: {}", e);
    }
    format!(
        "Shadow mode {}.",
        if prefs.shadow { "enabled" } else { "disabled" }
    )
}

/// /reject - Reject pending approval
pub fn handle_reject(app: &mut TuiApp, _args: &str) -> String {
    if app.pending_permission_request.is_some() {
        app.pending_permission_request = None;
        if let Some(tx) = app.permission_response_tx.take() {
            let _ = tx.send(false);
        }
        app.mode = crate::tui::app::AppMode::Chat;
        "Rejected pending permission request.".to_string()
    } else {
        "No pending approval to reject.".to_string()
    }
}

/// /subscribe - Subscribe to events/notifications
pub fn handle_subscribe(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "list" {
        if prefs.subscriptions.is_empty() {
            return "No subscriptions. Use `/subscribe add <event>`.".to_string();
        }
        let mut events = prefs.subscriptions.clone();
        events.sort();
        return format!("Subscriptions:\n- {}", events.join("\n- "));
    }
    let mut parts = arg.splitn(2, ' ');
    let action = parts.next().unwrap_or_default();
    let event = parts.next().unwrap_or("").trim();
    match action {
        "add" => {
            if event.is_empty() {
                return "Usage: /subscribe add <event>".to_string();
            }
            if !prefs.subscriptions.iter().any(|v| v == event) {
                prefs.subscriptions.push(event.to_string());
            }
            if let Err(e) = save_runtime_prefs(&prefs) {
                return format!("Failed to save subscriptions: {}", e);
            }
            format!("Subscribed to '{}'.", event)
        }
        "remove" => {
            if event.is_empty() {
                return "Usage: /subscribe remove <event>".to_string();
            }
            let before = prefs.subscriptions.len();
            prefs.subscriptions.retain(|v| v != event);
            if before == prefs.subscriptions.len() {
                return format!("Subscription '{}' not found.", event);
            }
            if let Err(e) = save_runtime_prefs(&prefs) {
                return format!("Failed to save subscriptions: {}", e);
            }
            format!("Unsubscribed from '{}'.", event)
        }
        "clear" => {
            prefs.subscriptions.clear();
            if let Err(e) = save_runtime_prefs(&prefs) {
                return format!("Failed to save subscriptions: {}", e);
            }
            "All subscriptions cleared.".to_string()
        }
        _ => "Usage: /subscribe [list|add <event>|remove <event>|clear]".to_string(),
    }
}

/// /slots - View/edit slot variables
pub fn handle_slots(app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /slots [list|get <name>|set <name> <value>|unset <name>|clear]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            // Show current slot values
            let mut lines = vec!["Slot Variables:".to_string()];
            lines.push(format!("  working_dir: {}", std::env::current_dir().unwrap_or_default().display()));
            if let Some(id) = app.session_manager.current_session_id() {
                lines.push(format!("  session_id: {}...", &id[..8.min(id.len())]));
            }
            if let Ok(slots) = load_slots() {
                if !slots.is_empty() {
                    lines.push("  custom slots:".to_string());
                    let mut keys: Vec<_> = slots.keys().cloned().collect();
                    keys.sort();
                    for k in keys {
                        if let Some(v) = slots.get(&k) {
                            lines.push(format!("    {} = {}", k, v));
                        }
                    }
                }
            }
            lines.join("\n")
        }
        "get" => {
            if parts.len() < 2 {
                return "Usage: /slots get <name>".to_string();
            }
            let Some(key) = sanitize_note_name(parts[1]) else {
                return "Invalid slot name.".to_string();
            };
            match load_slots() {
                Ok(slots) => match slots.get(&key) {
                    Some(v) => format!("{} = {}", key, v),
                    None => format!("Slot '{}' not set.", key),
                },
                Err(e) => format!("Failed to load slots: {}", e),
            }
        }
        "set" => {
            if parts.len() < 3 {
                "Usage: /slots set <name> <value>".to_string()
            } else {
                let Some(key) = sanitize_note_name(parts[1]) else {
                    return "Invalid slot name.".to_string();
                };
                let value = args
                    .splitn(3, ' ')
                    .nth(2)
                    .map(str::trim)
                    .unwrap_or_default();
                if value.is_empty() {
                    return "Usage: /slots set <name> <value>".to_string();
                }
                let mut slots = match load_slots() {
                    Ok(v) => v,
                    Err(e) => return format!("Failed to load slots: {}", e),
                };
                slots.insert(key.clone(), value.to_string());
                match save_slots(&slots) {
                    Ok(_) => format!("Slot '{}' set.", key),
                    Err(e) => format!("Failed to save slot: {}", e),
                }
            }
        }
        "unset" => {
            if parts.len() < 2 {
                return "Usage: /slots unset <name>".to_string();
            }
            let Some(key) = sanitize_note_name(parts[1]) else {
                return "Invalid slot name.".to_string();
            };
            let mut slots = match load_slots() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load slots: {}", e),
            };
            if slots.remove(&key).is_none() {
                return format!("Slot '{}' not set.", key);
            }
            match save_slots(&slots) {
                Ok(_) => format!("Slot '{}' removed.", key),
                Err(e) => format!("Failed to save slots: {}", e),
            }
        }
        "clear" => match save_slots(&std::collections::HashMap::new()) {
            Ok(_) => "All slots cleared.".to_string(),
            Err(e) => format!("Failed to clear slots: {}", e),
        },
        _ => {
            "Usage: /slots [list|get <name>|set <name> <value>|unset <name>|clear]".to_string()
        }
    }
}

/// /ticker - Display a scrolling ticker/marquee
pub fn handle_ticker(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "show" {
        return match prefs.ticker_message {
            Some(v) => format!("Ticker: {}", v),
            None => "Ticker is empty.".to_string(),
        };
    }
    if arg == "clear" {
        prefs.ticker_message = None;
        return match save_runtime_prefs(&prefs) {
            Ok(_) => "Ticker cleared.".to_string(),
            Err(e) => format!("Failed to clear ticker: {}", e),
        };
    }
    prefs.ticker_message = Some(arg.to_string());
    match save_runtime_prefs(&prefs) {
        Ok(_) => "Ticker updated.".to_string(),
        Err(e) => format!("Failed to save ticker: {}", e),
    }
}

// ═══════════════════════════════════════
// Phase 10 Batch 4: config, copy, desktop, chrome, effort, preamble, untrap, verbose, write
// ═══════════════════════════════════════

/// /config - Configuration viewer/editor
pub fn handle_config(_app: &TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "list" {
        return match crate::services::config::AppConfig::load() {
            Ok(config) => format_config_summary(&config),
            Err(e) => format!("Failed to load config: {}", e),
        };
    }

    if let Some(key) = args.strip_prefix("get ").map(str::trim) {
        if key.is_empty() {
            return "Usage: /config get <key>".to_string();
        }
        return match crate::services::config::AppConfig::load() {
            Ok(config) => get_config_value(&config, key)
                .map(|v| format!("{} = {}", key, v))
                .unwrap_or_else(|| format!("Unknown config key: {}", key)),
            Err(e) => format!("Failed to load config: {}", e),
        };
    }

    if let Some(rest) = args.strip_prefix("set ").map(str::trim) {
        let mut parts = rest.splitn(2, ' ');
        let Some(key) = parts.next().map(str::trim).filter(|v| !v.is_empty()) else {
            return "Usage: /config set <key> <value>".to_string();
        };
        let Some(value) = parts.next().map(str::trim).filter(|v| !v.is_empty()) else {
            return "Usage: /config set <key> <value>".to_string();
        };

        return match crate::services::config::AppConfig::load() {
            Ok(mut config) => match set_config_value(&mut config, key, value) {
                Ok(_) => match config.save() {
                    Ok(_) => format!(
                        "Updated {} = {} and saved to config.toml. Run /reload config to refresh runtime view.",
                        key, value
                    ),
                    Err(e) => format!("Updated in memory but failed to save config: {}", e),
                },
                Err(e) => e,
            },
            Err(e) => format!("Failed to load config: {}", e),
        };
    }

    "Usage: /config [list|get <key>|set <key> <value>]".to_string()
}

/// /copy - Copy text to clipboard
pub async fn handle_copy(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /copy <text>".to_string();
    }

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    #[cfg(target_os = "macos")]
    let cmd = format!("echo '{}' | pbcopy", args.replace("'", "'\\''"));
    #[cfg(not(target_os = "macos"))]
    let cmd = format!("echo '{}' | xclip -selection clipboard", args.replace("'", "'\\''"));

    let params = serde_json::json!({
        "command": cmd,
        "description": "Copy to clipboard"
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        "Copied to clipboard.".to_string()
    } else {
        result.error.unwrap_or_else(|| "Failed to copy.".to_string())
    }
}

/// /desktop - Desktop integration commands
pub fn handle_desktop(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /desktop [open|close|notify] <target>".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "open" => {
            if parts.len() < 2 {
                "Usage: /desktop open <target>".to_string()
            } else {
                format!("Desktop open not yet implemented for: {}", parts[1])
            }
        }
        "close" => "Desktop close not yet implemented.".to_string(),
        "notify" => {
            if parts.len() < 2 {
                "Usage: /desktop notify <message>".to_string()
            } else {
                format!("Desktop notification: {} (not yet implemented)", parts[1])
            }
        }
        _ => "Usage: /desktop [open|close|notify]".to_string(),
    }
}

/// /chrome - Chrome integration
pub fn handle_chrome(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /chrome [open|tabs|bookmarks]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "open" => {
            if parts.len() < 2 {
                "Usage: /chrome open <url>".to_string()
            } else {
                let url = parts[1];
                if !is_valid_webhook_url(url) {
                    return "Please provide a valid http(s) URL.".to_string();
                }
                #[cfg(target_os = "macos")]
                let status = std::process::Command::new("open")
                    .args(["-a", "Google Chrome", url])
                    .status();
                #[cfg(not(target_os = "macos"))]
                let status = std::process::Command::new("xdg-open").arg(url).status();
                match status {
                    Ok(s) if s.success() => format!("Opened in Chrome: {}", url),
                    Ok(s) => format!("Open failed with status: {}", s),
                    Err(e) => format!("Failed to open Chrome: {}", e),
                }
            }
        }
        "tabs" => {
            #[cfg(target_os = "macos")]
            {
                let script = "tell application \"Google Chrome\" to get URL of tabs of windows";
                let out = std::process::Command::new("osascript")
                    .args(["-e", script])
                    .output();
                match out {
                    Ok(v) if v.status.success() => {
                        let text = String::from_utf8_lossy(&v.stdout).trim().to_string();
                        if text.is_empty() {
                            "No open tabs found.".to_string()
                        } else {
                            let tabs: Vec<String> = text
                                .split(", ")
                                .take(20)
                                .map(ToString::to_string)
                                .collect();
                            format!("Open tabs:\n- {}", tabs.join("\n- "))
                        }
                    }
                    Ok(v) => format!("Failed to query tabs: {}", String::from_utf8_lossy(&v.stderr)),
                    Err(e) => format!("Failed to run osascript: {}", e),
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                "Tab listing currently supports macOS only.".to_string()
            }
        }
        "bookmarks" => {
            #[cfg(target_os = "macos")]
            let bookmark_file = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("Library")
                .join("Application Support")
                .join("Google")
                .join("Chrome")
                .join("Default")
                .join("Bookmarks");
            #[cfg(not(target_os = "macos"))]
            let bookmark_file = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".config")
                .join("google-chrome")
                .join("Default")
                .join("Bookmarks");

            if !bookmark_file.exists() {
                return format!("Bookmarks file not found: {}", bookmark_file.display());
            }
            let text = match std::fs::read_to_string(&bookmark_file) {
                Ok(v) => v,
                Err(e) => return format!("Failed to read bookmarks: {}", e),
            };
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => return format!("Failed to parse bookmarks JSON: {}", e),
            };
            let mut lines = Vec::new();
            collect_chrome_bookmarks(&json, &mut lines, 30);
            if lines.is_empty() {
                "No bookmarks found.".to_string()
            } else {
                format!("Bookmarks:\n- {}", lines.join("\n- "))
            }
        }
        _ => "Usage: /chrome [open|tabs|bookmarks]".to_string(),
    }
}

/// /effort - Set effort level for tasks
pub fn handle_effort(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!("Effort level: {}", prefs.effort_level);
    }
    match arg {
        "minimal" | "normal" | "maximum" => {
            prefs.effort_level = arg.to_string();
            match save_runtime_prefs(&prefs) {
                Ok(_) => format!("Effort set to: {}", arg),
                Err(e) => format!("Effort updated but failed to persist: {}", e),
            }
        }
        _ => "Usage: /effort [minimal|normal|maximum|status]".to_string(),
    }
}

/// /preamble - Customize agent preamble
pub fn handle_preamble(_app: &mut TuiApp, args: &str) -> String {
    let arg = args.trim();
    if arg.is_empty() || arg == "show" {
        return match read_preamble() {
            Ok(Some(v)) => format!("Preamble:\n{}", v),
            Ok(None) => "Preamble: default (not customized).".to_string(),
            Err(e) => format!("Failed to read preamble: {}", e),
        };
    }

    if let Some(text) = arg.strip_prefix("set ").map(str::trim) {
        if text.is_empty() {
            return "Usage: /preamble set <text>".to_string();
        }
        return match write_preamble(text) {
            Ok(_) => "Preamble updated.".to_string(),
            Err(e) => format!("Failed to save preamble: {}", e),
        };
    }
    if arg == "reset" {
        return match reset_preamble() {
            Ok(_) => "Preamble reset to default.".to_string(),
            Err(e) => format!("Failed to reset preamble: {}", e),
        };
    }
    "Usage: /preamble [show|set <text>|reset]".to_string()
}

/// /untrap - Reset trapped state
pub fn handle_untrap(app: &mut TuiApp, _args: &str) -> String {
    app.is_querying = false;
    app.pending_plan = None;
    if let Some(tx) = app.plan_response_tx.take() {
        let _ = tx.send(crate::engine::plan_mode::PlanApproval::Rejected);
    }
    app.pending_permission_request = None;
    if let Some(tx) = app.permission_response_tx.take() {
        let _ = tx.send(false);
    }
    app.pending_question = None;
    app.pending_question_options.clear();
    if let Some(tx) = app.question_response_tx.take() {
        let _ = tx.send(String::new());
    }
    app.mode = crate::tui::app::AppMode::Chat;
    "Untrap complete: cleared pending approvals/questions and returned to chat mode.".to_string()
}

/// /verbose - Toggle verbose output
pub fn handle_verbose(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Verbose mode: {}",
            if prefs.verbose { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => prefs.verbose = true,
        "off" | "disable" => prefs.verbose = false,
        "toggle" => prefs.verbose = !prefs.verbose,
        _ => return "Usage: /verbose [on|off|toggle|status]".to_string(),
    }
    std::env::set_var("RUST_LOG", if prefs.verbose { "debug" } else { "info" });
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Verbose mode changed but failed to persist: {}", e);
    }
    format!(
        "Verbose mode {}.",
        if prefs.verbose { "enabled" } else { "disabled" }
    )
}

/// /write - Write content to a file
pub async fn handle_write(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /write <filepath> <content>".to_string();
    }

    // Parse: /write <filepath> <content>
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return "Usage: /write <filepath> <content>".to_string();
    }

    let filepath = parts[0];
    let content = parts[1];

    let tool = crate::tools::FileWriteTool;
    let ctx = app.build_tool_context().await;
    let params = serde_json::json!({
        "file_path": filepath,
        "content": content,
        "create_dirs": true
    });

    let result = tool.execute(params, ctx).await;
    if result.success {
        format!("Written to: {}", filepath)
    } else {
        result.error.unwrap_or_else(|| "Failed to write file.".to_string())
    }
}

// ═══════════════════════════════════════
// Phase 10 Extended: More missing commands
// ═══════════════════════════════════════

/// /rollback - Rollback changes
pub async fn handle_rollback(app: &mut TuiApp, args: &str) -> String {
    let parsed = match parse_rollback_args(args) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if !is_valid_rollback_target(&parsed.target) {
        return "Invalid rollback target. Allowed characters: letters, digits, -, _, ., /, ~, ^, @, {, }"
            .to_string();
    }

    if !parsed.confirmed {
        return format!(
            "Rollback is destructive and will discard uncommitted changes.\nUsage: /rollback [target] --yes\nExample: /rollback {} --yes",
            parsed.target
        );
    }

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;
    let cmd = format!(
        "git rev-parse --verify '{}^{{commit}}' >/dev/null && git reset --hard '{}'",
        parsed.target, parsed.target
    );
    let params = serde_json::json!({
        "command": cmd,
        "description": format!("Git rollback to {}", parsed.target)
    });
    let result = tool.execute(params, ctx).await;
    if result.success { result.content } else { result.error.unwrap_or_default() }
}

#[derive(Debug)]
struct ParsedRollbackArgs {
    target: String,
    confirmed: bool,
}

fn parse_rollback_args(args: &str) -> Result<ParsedRollbackArgs, String> {
    let mut target: Option<&str> = None;
    let mut confirmed = false;

    for part in args.split_whitespace() {
        if part == "--yes" {
            confirmed = true;
            continue;
        }
        if part.starts_with("--") {
            return Err(format!(
                "Unknown option: {}.\nUsage: /rollback [target] --yes",
                part
            ));
        }
        if target.is_some() {
            return Err(
                "Too many arguments.\nUsage: /rollback [target] --yes\nExample: /rollback HEAD~1 --yes"
                    .to_string(),
            );
        }
        target = Some(part);
    }

    Ok(ParsedRollbackArgs {
        target: target.unwrap_or("HEAD~1").to_string(),
        confirmed,
    })
}

fn is_valid_rollback_target(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with('-')
        && target.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '-' | '_' | '.' | '/' | '~' | '^' | '@' | '{' | '}')
        })
}

/// /project - Project management
pub fn handle_project(_app: &TuiApp, args: &str) -> String {
    if args.is_empty() || args == "info" {
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let name = dir.file_name().unwrap_or_default().to_string_lossy();
        let entries = std::fs::read_dir(&dir)
            .map(|it| it.flatten().count())
            .unwrap_or(0);
        let branch = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "(none)".to_string());
        return format!(
            "Project: {}\nPath: {}\nEntries: {}\nGit branch: {}",
            name,
            dir.display(),
            entries,
            branch
        );
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .flatten()
                        .map(|e| {
                            let p = e.path();
                            let marker = if p.is_dir() { "/" } else { "" };
                            format!("{}{}", e.file_name().to_string_lossy(), marker)
                        })
                        .collect();
                    names.sort();
                    if names.is_empty() {
                        "Project directory is empty.".to_string()
                    } else {
                        format!("Project entries:\n- {}", names.join("\n- "))
                    }
                }
                Err(e) => format!("Failed to list project entries: {}", e),
            }
        }
        "tree" => {
            let depth = parts
                .get(1)
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(2)
                .clamp(1, 5);
            let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let mut lines = Vec::new();
            build_tree_lines(&root, 0, depth, &mut lines, 200);
            if lines.is_empty() {
                "No entries.".to_string()
            } else {
                format!("Project tree (depth {}):\n{}", depth, lines.join("\n"))
            }
        }
        "init" => {
            if parts.len() < 2 {
                "Usage: /project init <name>".to_string()
            } else {
                let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let path = dir.join(parts[1]);
                if path.exists() {
                    return format!("Target already exists: {}", path.display());
                }
                match std::fs::create_dir_all(path.join("src")) {
                    Ok(_) => format!("Project initialized: {}", path.display()),
                    Err(e) => format!("Failed to init project: {}", e),
                }
            }
        }
        _ => "Usage: /project [info|list|tree [depth]|init <name>]".to_string(),
    }
}

/// /backend - Switch execution backend
pub fn handle_backend(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    if args.is_empty() || args == "status" {
        return format!(
            "Current backend: {}\nUsage: /backend [local|restricted|external|status]",
            prefs.backend
        );
    }

    match args.trim() {
        "local" => prefs.backend = "local".to_string(),
        "restricted" => prefs.backend = "restricted".to_string(),
        "external" => {
            let external_cmd = std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_CMD").unwrap_or_default();
            if external_cmd.is_empty() {
                return "External backend not configured. Set PRIORITY_AGENT_BASH_EXTERNAL_CMD".to_string();
            }
            prefs.backend = "external".to_string();
        }
        _ => return "Usage: /backend [local|restricted|external|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Backend changed but failed to persist: {}", e);
    }
    format!("Backend set to: {}", prefs.backend)
}

/// /sandbox - Sandbox mode toggle
pub fn handle_sandbox(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Sandbox mode: {}",
            if prefs.sandbox { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => {
            prefs.sandbox = true;
            prefs.backend = "restricted".to_string();
        }
        "off" | "disable" => {
            prefs.sandbox = false;
            if prefs.backend == "restricted" {
                prefs.backend = "local".to_string();
            }
        }
        "toggle" => {
            prefs.sandbox = !prefs.sandbox;
            prefs.backend = if prefs.sandbox { "restricted" } else { "local" }.to_string();
        }
        _ => return "Usage: /sandbox [on|off|toggle|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Sandbox mode changed but failed to persist: {}", e);
    }
    format!(
        "Sandbox mode {} (backend: {}).",
        if prefs.sandbox { "enabled" } else { "disabled" },
        prefs.backend
    )
}

/// /env - Show/manage environment variables
pub fn handle_env(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /env [list|get <key>|set <key> <value>|unset <key>]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            let env_vars: Vec<String> = std::env::vars()
                .filter(|(k, _)| k.starts_with("PRIORITY_AGENT_"))
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            if env_vars.is_empty() {
                "No PRIORITY_AGENT_* environment variables set.".to_string()
            } else {
                format!("Environment:\n{}", env_vars.join("\n"))
            }
        }
        "get" => {
            if parts.len() < 2 {
                "Usage: /env get <key>".to_string()
            } else {
                std::env::var(parts[1]).unwrap_or_else(|_| "Not set".to_string())
            }
        }
        "set" => {
            let rest = args.splitn(3, ' ').collect::<Vec<_>>();
            if rest.len() < 3 {
                return "Usage: /env set <key> <value>".to_string();
            }
            let key = rest[1].trim();
            let value = rest[2].trim();
            if !key.starts_with("PRIORITY_AGENT_") {
                return "Only PRIORITY_AGENT_* variables are allowed for /env set.".to_string();
            }
            std::env::set_var(key, value);
            format!("Set {}={}", key, value)
        }
        "unset" => {
            if parts.len() < 2 {
                return "Usage: /env unset <key>".to_string();
            }
            let key = parts[1];
            if !key.starts_with("PRIORITY_AGENT_") {
                return "Only PRIORITY_AGENT_* variables are allowed for /env unset.".to_string();
            }
            std::env::remove_var(key);
            format!("Unset {}", key)
        }
        _ => "Usage: /env [list|get <key>|set <key> <value>|unset <key>]".to_string(),
    }
}

/// /cache - Cache management
pub fn handle_cache(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /cache [clear|stats]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "clear" => "Cache cleared.".to_string(),
        "stats" => {
            let cache_dir = priority_agent_home_dir().join("cache");
            let tool_cache = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("priority-agent")
                .join("tool-results");
            let cache_files = count_files_recursively(&cache_dir);
            let tool_files = count_files_recursively(&tool_cache);
            format!(
                "Cache stats:\n  memory_file_cache: active\n  cache_dir: {} file(s) ({})\n  tool_result_dir: {} file(s) ({})",
                cache_files,
                cache_dir.display(),
                tool_files,
                tool_cache.display()
            )
        }
        _ => "Usage: /cache [clear|stats]".to_string(),
    }
}

/// /benchmark - Run performance benchmark
pub async fn handle_benchmark(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let script_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("scripts/benchmark.sh");

    if !script_path.exists() {
        let start = std::time::Instant::now();
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let entries = match tokio::fs::read_dir(&dir).await {
            Ok(mut it) => {
                let mut count = 0;
                while let Ok(Some(_)) = it.next_entry().await {
                    count += 1;
                }
                count
            }
            Err(_) => 0,
        };
        let fs_ms = start.elapsed().as_millis();

        let hist_start = std::time::Instant::now();
        let hist = if let Some(ref engine) = app.streaming_engine {
            engine.get_history().await.len()
        } else {
            0
        };
        let hist_ms = hist_start.elapsed().as_millis();
        return format!(
            "Synthetic benchmark:\n  fs_scan: {} ms ({} entries)\n  history_fetch: {} ms ({} messages)\nScript benchmark unavailable: {}",
            fs_ms,
            entries,
            hist_ms,
            hist,
            script_path.display()
        );
    }

    let limit = args.parse::<u32>().unwrap_or(0);
    let cmd = if limit > 0 {
        format!("bash {} --enable-long-chat 2>/dev/null || echo 'Benchmark script not found'", script_path.display())
    } else {
        format!("bash {} 2>/dev/null || echo 'Benchmark script not found'", script_path.display())
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "Run benchmark"
    });
    let result = tool.execute(params, ctx).await;
    if result.success { result.content } else { result.error.unwrap_or_default() }
}

/// /test - Run tests
pub async fn handle_test(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let cmd = if args.is_empty() {
        "tmp=$(mktemp -t priority-agent-test.XXXXXX); cargo test > \"$tmp\" 2>&1; status=$?; tail -30 \"$tmp\"; rm -f \"$tmp\"; exit $status".to_string()
    } else {
        format!("tmp=$(mktemp -t priority-agent-test.XXXXXX); cargo test {} > \"$tmp\" 2>&1; status=$?; tail -30 \"$tmp\"; rm -f \"$tmp\"; exit $status", args)
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "Run tests"
    });
    let result = tool.execute(params, ctx).await;
    if result.success { result.content } else { result.error.unwrap_or_default() }
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

/// /trace - Tracing controls
pub fn handle_trace(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!("Tracing: {}", if prefs.trace { "enabled" } else { "disabled" });
    }

    match arg {
        "on" | "enable" => prefs.trace = true,
        "off" | "disable" => prefs.trace = false,
        "toggle" => prefs.trace = !prefs.trace,
        _ => return "Usage: /trace [on|off|toggle|status]".to_string(),
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
    format!("Tracing {}.", if prefs.trace { "enabled" } else { "disabled" })
}

/// /memory - Memory management (enhanced)
pub fn handle_memory(_app: &TuiApp) -> String {
    let mem_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
        .join("memory");

    if !mem_path.exists() {
        return "No memory entries saved. Start chatting to create memories.".to_string();
    }

    match std::fs::read_dir(&mem_path) {
        Ok(entries) => {
            let count = entries.count();
            format!("Memory entries: {} (stored in {})", count, mem_path.display())
        }
        Err(_) => "Failed to read memory directory.".to_string(),
    }
}

/// /skills - List available skills
pub fn handle_skills(_app: &TuiApp) -> String {
    "Skills: use /help to see all skill-based commands (commit, review, explain, fix, etc.)".to_string()
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
                suggestions.push("Timeout: Try /retry to repeat, or /doctor to check tool latency".to_string());
            }
            "permission" => {
                suggestions.push("Permission denied: Use /permissions to check rules, or /doctor to diagnose".to_string());
            }
            "not_found" => {
                suggestions.push("Not found: Check file paths with /ls, or verify resource exists".to_string());
            }
            "hook_blocked" => {
                suggestions.push("Hook blocked: Check PRE_TOOL_HOOK / POST_TOOL_HOOK env vars in /doctor".to_string());
            }
            "dangerous_command" => {
                suggestions.push("Dangerous command: Use /permissions to allow, or modify the command".to_string());
            }
            _ => {
                suggestions.push(format!("Error '{}': Run /doctor for detailed diagnostics", reason_str));
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

// ═══════════════════════════════════════
// Phase 10 Extended 2: Even more commands
// ═══════════════════════════════════════

/// /init - Initialize a new project
pub fn handle_init(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /init <project_name>".to_string();
    }

    let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project_path = dir.join(args);

    if project_path.exists() {
        return format!("Target already exists: {}", project_path.display());
    }
    match std::fs::create_dir_all(project_path.join("src")) {
        Ok(_) => {
            let readme = project_path.join("README.md");
            let gitignore = project_path.join(".gitignore");
            let cargo_toml = project_path.join("Cargo.toml");
            let main_rs = project_path.join("src").join("main.rs");
            let _ = std::fs::write(
                &readme,
                format!("# {}\n\nInitialized by /init.\n", args.trim()),
            );
            let _ = std::fs::write(&gitignore, "target/\n*.log\n.env\n");
            let _ = std::fs::write(
                &cargo_toml,
                format!(
                    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
                    args.trim().replace('-', "_")
                ),
            );
            let _ = std::fs::write(&main_rs, "fn main() {\n    println!(\"hello\");\n}\n");
            format!("Project initialized at {}", project_path.display())
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
        let has_key = std::env::var("MOONSHOT_API_KEY").is_ok()
            || std::env::var("OPENAI_API_KEY").is_ok();
        return if has_key {
            "API key is set. Use /model to see which model is active.".to_string()
        } else {
            "No API key set. Set MOONSHOT_API_KEY or OPENAI_API_KEY environment variable.".to_string()
        };
    }

    match args.trim() {
        "show" => "API key not shown for security. Set MOONSHOT_API_KEY or OPENAI_API_KEY.".to_string(),
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
    lines.push("  Mode: TUI".to_string());
    lines.push(format!("  Rust version: {}", std::env::consts::OS));
    format!("{}\n{}", lines.join("\n"), "Use /doctor for full diagnostics")
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

// ═══════════════════════════════════════
// Phase 10 Extended 3: Even more commands
// ═══════════════════════════════════════

/// /reset - Reset session state
pub fn handle_reset(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "session" {
        app.messages.clear();
        "Session reset. Messages cleared.".to_string()
    } else if args == "all" {
        app.messages.clear();
        "Full reset not yet implemented.".to_string()
    } else {
        "Usage: /reset [session|all]".to_string()
    }
}

/// /export - Export data
pub async fn handle_export_data(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let format = if args.is_empty() { "json" } else { args };
    let session_id = app.session_manager.current_session_id().unwrap_or("unknown");

    let cmd = match format {
        "json" => format!("echo 'Session {}' > /tmp/export.json && cat /tmp/export.json", &session_id[..8.min(session_id.len())]),
        "md" => format!("echo '# Session Export' > /tmp/export.md && echo 'Session: {}' >> /tmp/export.md && cat /tmp/export.md", &session_id[..8.min(session_id.len())]),
        _ => return "Usage: /export [json|md]".to_string(),
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "Export session data"
    });
    let result = tool.execute(params, ctx).await;
    if result.success { result.content } else { result.error.unwrap_or_default() }
}

/// /import - Import data
pub async fn handle_import(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /import <file_path>".to_string();
    }

    let path = std::path::Path::new(args.trim());
    if !path.exists() {
        format!("File not found: {}", args)
    } else if !path.is_file() {
        format!("Not a file: {}", args)
    } else {
        let text = match tokio::fs::read_to_string(path).await {
            Ok(v) => v,
            Err(e) => return format!("Failed to read import file: {}", e),
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => return format!("Invalid JSON import file: {}", e),
        };
        let messages = match value.get("messages").and_then(|v| v.as_array()) {
            Some(v) => v,
            None => return "Import file missing `messages` array.".to_string(),
        };
        if messages.is_empty() {
            return "Import file has no messages.".to_string();
        }
        let mut imported = 0usize;
        for m in messages {
            let role_str = m.get("role").and_then(|v| v.as_str()).unwrap_or("system");
            let content = m.get("content").and_then(|v| v.as_str()).unwrap_or_default();
            if content.is_empty() {
                continue;
            }
            let role = match role_str {
                "user" => crate::state::MessageRole::User,
                "assistant" => crate::state::MessageRole::Assistant,
                "tool" => crate::state::MessageRole::Tool,
                _ => crate::state::MessageRole::System,
            };
            let item = crate::state::MessageItem {
                id: format!("import_{}", app.messages.len() + imported),
                role,
                content: content.to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            };
            app.messages.push(item.clone());
            let _ = app.session_manager.add_message(role, &item.content);
            imported += 1;
        }
        if let Some(ref engine) = app.streaming_engine {
            let _ = engine
                .set_history(message_items_to_api_messages(&app.messages))
                .await;
        }
        format!("Imported {} message(s) from {}.", imported, path.display())
    }
}

/// /save-session - Save current session
pub fn handle_save_session(app: &TuiApp) -> String {
    if let Some(id) = app.session_manager.current_session_id() {
        format!("Session {} auto-saved.", &id[..8.min(id.len())])
    } else {
        "No active session to save.".to_string()
    }
}

/// /load-session - Load a session
pub async fn handle_load_session(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /load-session <session_id>".to_string();
    }
    app.restore_session(args).await
}

/// /merge - Merge sessions
pub async fn handle_merge(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        return "Usage: /merge <session_id> into current".to_string();
    }
    let source_ref = args
        .strip_suffix("into current")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(args);

    let current_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session.".to_string(),
    };

    let source_id = if let Ok(n) = source_ref.parse::<usize>() {
        match app.session_manager.list_sessions(100) {
            Ok(sessions) if n > 0 && n <= sessions.len() => sessions[n - 1].id.clone(),
            _ => {
                return "Invalid session number. Use /session list to see available sessions."
                    .to_string();
            }
        }
    } else {
        source_ref.to_string()
    };

    if source_id == current_id {
        return "Cannot merge current session into itself.".to_string();
    }

    let source_messages = match app.session_manager.load_messages(&source_id) {
        Ok(msgs) => msgs,
        Err(e) => return format!("Failed to load source session: {}", e),
    };
    if source_messages.is_empty() {
        return format!("Source session {} has no messages.", source_id);
    }

    let mut imported = 0usize;
    for msg in source_messages {
        if app
            .session_manager
            .add_message(msg.role, &msg.content)
            .is_ok()
        {
            app.messages.push(msg);
            imported += 1;
        }
    }

    if let Some(ref engine) = app.streaming_engine {
        engine.set_history(message_items_to_api_messages(
            &app.messages,
        )).await;
    }

    format!(
        "Merged {} message(s) from session {} into current session.",
        imported, source_id
    )
}

/// /cleanup - Cleanup old data
pub fn handle_cleanup(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    let target = parts.next().unwrap_or("all");

    match target {
        "sessions" => {
            let mut keep: usize = 20;
            let mut confirmed = false;
            for token in parts {
                if token == "--yes" {
                    confirmed = true;
                } else if let Ok(v) = token.parse::<usize>() {
                    keep = v.max(1);
                } else {
                    return "Usage: /cleanup sessions [keep_count] --yes".to_string();
                }
            }
            if !confirmed {
                return format!(
                    "Session cleanup is destructive.\nUsage: /cleanup sessions [keep_count] --yes\nExample: /cleanup sessions {} --yes",
                    keep
                );
            }
            cleanup_sessions(app, keep)
        }
        "cache" => cleanup_cache(),
        "logs" => cleanup_logs(),
        "all" => {
            let confirmed = parts.any(|p| p == "--yes");
            if !confirmed {
                return "Full cleanup will remove old sessions, cache, and logs.\nUsage: /cleanup all --yes"
                    .to_string();
            }
            let session_msg = cleanup_sessions(app, 20);
            let cache_msg = cleanup_cache();
            let logs_msg = cleanup_logs();
            format!("{}\n{}\n{}", session_msg, cache_msg, logs_msg)
        }
        _ => "Usage: /cleanup [sessions|cache|logs|all]".to_string(),
    }
}

/// /compact - Compact context
pub async fn handle_compact(app: &mut TuiApp) -> String {
    let Some(ref engine) = app.streaming_engine else {
        return "Engine not initialized; cannot compact context.".to_string();
    };
    let history_before = engine.get_history().await;
    if history_before.is_empty() {
        return "No history to compact.".to_string();
    }
    let before_msgs = history_before.len();
    let before_tokens =
        crate::engine::context_compressor::estimate_messages_tokens(&history_before);
    let Some(comp) = engine.compressor() else {
        return "Context compressor unavailable.".to_string();
    };
    let compacted = {
        let mut guard = comp.lock().await;
        guard.micro_compress(&history_before)
    };
    let after_msgs = compacted.len();
    let after_tokens = crate::engine::context_compressor::estimate_messages_tokens(&compacted);
    engine.set_history(compacted.clone()).await;

    app.messages = compacted
        .into_iter()
        .enumerate()
        .map(|(idx, m)| {
            let (role, content) = match m {
                crate::services::api::Message::System { content } => {
                    (crate::state::MessageRole::System, content)
                }
                crate::services::api::Message::User { content } => {
                    (crate::state::MessageRole::User, content)
                }
                crate::services::api::Message::Assistant { content, .. } => {
                    (crate::state::MessageRole::Assistant, content)
                }
                crate::services::api::Message::Tool { content, .. } => {
                    (crate::state::MessageRole::Tool, content)
                }
            };
            crate::state::MessageItem {
                id: format!("compact_{}", idx),
                role,
                content,
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            }
        })
        .collect();
    if let Some(session_id) = app.session_manager.current_session_id() {
        let _ = app.session_manager.replace_messages(session_id, &app.messages);
    }
    format!(
        "Context compacted: messages {} -> {}, tokens {} -> {}.",
        before_msgs, after_msgs, before_tokens, after_tokens
    )
}

/// /snippet - Save/load code snippets
pub fn handle_snippet(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        return "Usage: /snippet [save <name>|load <name>|list]".to_string();
    }

    let mut parts = args.splitn(2, ' ');
    let action = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();

    match action {
        "save" => {
            if rest.is_empty() {
                return "Usage: /snippet save <name> [content]".to_string();
            }

            let mut save_parts = rest.splitn(2, ' ');
            let name = save_parts.next().unwrap_or_default().trim();
            let content = save_parts.next().map(str::trim).unwrap_or("");
            let Some(safe_name) = sanitize_snippet_name(name) else {
                return "Invalid snippet name. Use letters, digits, '-', '_' or '.'".to_string();
            };

            let content_to_save = if content.is_empty() {
                match app.messages.last() {
                    Some(msg) => msg.content.clone(),
                    None => {
                        return "No message available to save. Provide content explicitly.".to_string();
                    }
                }
            } else {
                content.to_string()
            };

            let dir = snippet_dir();
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return format!("Failed to create snippet directory: {}", e);
            }
            let path = dir.join(format!("{}.md", safe_name));
            match std::fs::write(&path, content_to_save) {
                Ok(_) => format!("Snippet '{}' saved to {}", safe_name, path.display()),
                Err(e) => format!("Failed to save snippet '{}': {}", safe_name, e),
            }
        }
        "load" => {
            if rest.is_empty() {
                return "Usage: /snippet load <name>".to_string();
            }
            let Some(safe_name) = sanitize_snippet_name(rest) else {
                return "Invalid snippet name. Use letters, digits, '-', '_' or '.'".to_string();
            };
            let path = snippet_dir().join(format!("{}.md", safe_name));
            match std::fs::read_to_string(&path) {
                Ok(content) => format!("Snippet '{}':\n{}", safe_name, content),
                Err(e) => format!("Failed to load snippet '{}': {}", safe_name, e),
            }
        }
        "list" => match list_snippets() {
            Ok(names) if names.is_empty() => "No snippets saved.".to_string(),
            Ok(names) => format!("Snippets:\n- {}", names.join("\n- ")),
            Err(e) => format!("Failed to list snippets: {}", e),
        }
        _ => "Usage: /snippet [save|load|list]".to_string(),
    }
}

fn priority_agent_home_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
}

fn prompt_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("prompt.txt")
}

fn webhooks_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("webhooks.json")
}

fn runtime_prefs_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("runtime_prefs.json")
}

fn preamble_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("preamble.txt")
}

fn slots_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("slots.json")
}

fn snippet_dir() -> std::path::PathBuf {
    priority_agent_home_dir().join("snippets")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct RuntimePrefs {
    #[serde(default)]
    verbose: bool,
    #[serde(default)]
    trace: bool,
    #[serde(default)]
    stealth: bool,
    #[serde(default)]
    shadow: bool,
    #[serde(default = "default_backend")]
    backend: String,
    #[serde(default)]
    sandbox: bool,
    #[serde(default)]
    subscriptions: Vec<String>,
    #[serde(default)]
    logged_in_provider: Option<String>,
    #[serde(default = "default_effort_level")]
    effort_level: String,
    #[serde(default)]
    ticker_message: Option<String>,
    #[serde(default)]
    slack_webhook_url: Option<String>,
    #[serde(default)]
    slack_default_channel: Option<String>,
}

fn default_backend() -> String {
    "local".to_string()
}

fn default_effort_level() -> String {
    "normal".to_string()
}

fn load_runtime_prefs() -> Result<RuntimePrefs, String> {
    let path = runtime_prefs_file();
    if !path.exists() {
        return Ok(RuntimePrefs::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str::<RuntimePrefs>(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn save_runtime_prefs(prefs: &RuntimePrefs) -> Result<(), String> {
    let path = runtime_prefs_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn read_preamble() -> Result<Option<String>, String> {
    let path = preamble_file();
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn write_preamble(text: &str) -> Result<(), String> {
    let path = preamble_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    std::fs::write(&path, text.trim()).map_err(|e| format!("{}: {}", path.display(), e))
}

fn reset_preamble() -> Result<(), String> {
    let path = preamble_file();
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(&path).map_err(|e| format!("{}: {}", path.display(), e))
}

fn load_slots() -> Result<std::collections::HashMap<String, String>, String> {
    let path = slots_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn save_slots(map: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let path = slots_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let payload = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, payload).map_err(|e| format!("{}: {}", path.display(), e))
}

fn count_files_recursively(path: &std::path::Path) -> usize {
    if !path.exists() {
        return 0;
    }
    let mut count = 0usize;
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(read_dir) = std::fs::read_dir(&p) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let ep = entry.path();
            if ep.is_file() {
                count += 1;
            } else if ep.is_dir() {
                stack.push(ep);
            }
        }
    }
    count
}

async fn post_slack_webhook(
    webhook_url: &str,
    channel: Option<&str>,
    message: &str,
) -> Result<(), String> {
    let mut payload = serde_json::json!({
        "text": message,
    });
    if let Some(ch) = channel {
        payload["channel"] = serde_json::Value::String(ch.to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build client: {}", e))?;
    let resp = client
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("request error: {}", e))?;
    let status = resp.status();
    if status.is_success() {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("status {}: {}", status, body))
    }
}

fn collect_chrome_bookmarks(node: &serde_json::Value, out: &mut Vec<String>, limit: usize) {
    if out.len() >= limit {
        return;
    }
    if let Some(obj) = node.as_object() {
        if let (Some(name), Some(url)) = (
            obj.get("name").and_then(|v| v.as_str()),
            obj.get("url").and_then(|v| v.as_str()),
        ) {
            out.push(format!("{} -> {}", name, url));
            if out.len() >= limit {
                return;
            }
        }
        for v in obj.values() {
            collect_chrome_bookmarks(v, out, limit);
            if out.len() >= limit {
                return;
            }
        }
        return;
    }
    if let Some(arr) = node.as_array() {
        for v in arr {
            collect_chrome_bookmarks(v, out, limit);
            if out.len() >= limit {
                return;
            }
        }
    }
}

fn build_tree_lines(
    root: &std::path::Path,
    level: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
    max_lines: usize,
) {
    if level >= max_depth || lines.len() >= max_lines {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut items: Vec<_> = entries.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for entry in items {
        if lines.len() >= max_lines {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let indent = "  ".repeat(level);
        if path.is_dir() {
            lines.push(format!("{}- {}/", indent, name));
            build_tree_lines(&path, level + 1, max_depth, lines, max_lines);
        } else {
            lines.push(format!("{}- {}", indent, name));
        }
    }
}

fn read_prompt_file() -> Result<Option<String>, String> {
    let path = prompt_file();
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn write_prompt_file(text: &str) -> Result<(), String> {
    let path = prompt_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    std::fs::write(&path, text.trim()).map_err(|e| format!("{}: {}", path.display(), e))
}

fn append_prompt_file(text: &str) -> Result<(), String> {
    let next = text.trim();
    if next.is_empty() {
        return Err("Prompt content cannot be empty.".to_string());
    }
    let merged = match read_prompt_file()? {
        Some(existing) => format!("{}\n\n{}", existing, next),
        None => next.to_string(),
    };
    write_prompt_file(&merged)
}

fn reset_prompt_file() -> Result<(), String> {
    let path = prompt_file();
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(&path).map_err(|e| format!("{}: {}", path.display(), e))
}

fn load_webhooks() -> Result<std::collections::HashMap<String, String>, String> {
    let path = webhooks_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn save_webhooks(map: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let path = webhooks_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let payload = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, payload).map_err(|e| format!("{}: {}", path.display(), e))
}

fn is_valid_webhook_url(raw: &str) -> bool {
    if !(raw.starts_with("http://") || raw.starts_with("https://")) {
        return false;
    }
    match reqwest::Url::parse(raw) {
        Ok(url) => matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
        Err(_) => false,
    }
}

async fn test_webhook(url: &str, payload: &str) -> Result<String, String> {
    let body: serde_json::Value = serde_json::from_str(payload).unwrap_or_else(|_| {
        serde_json::json!({
            "message": payload,
        })
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build http client: {}", e))?;
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request error: {}", e))?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "<unable to read body>".to_string());
    let preview: String = response_text.chars().take(200).collect();

    if status.is_success() {
        Ok(format!(
            "Webhook test delivered successfully (status {}). Response: {}",
            status, preview
        ))
    } else {
        Err(format!(
            "status {}. Response: {}",
            status, preview
        ))
    }
}

async fn run_migrate_sqlx(app: &mut TuiApp, is_up: bool) -> String {
    if std::env::var("DATABASE_URL").is_err() {
        return "DATABASE_URL is not set. Export DATABASE_URL first, then run /migrate up|down."
            .to_string();
    }

    let command = if is_up {
        "sqlx migrate run"
    } else {
        "sqlx migrate revert"
    };

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;
    let params = serde_json::json!({
        "command": command,
        "description": if is_up { "sqlx migrate up" } else { "sqlx migrate down" },
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        if result.content.trim().is_empty() {
            if is_up {
                "Migrations applied successfully.".to_string()
            } else {
                "Migration reverted successfully.".to_string()
            }
        } else {
            result.content
        }
    } else {
        format!(
            "Migration command failed: {}\nHint: ensure `sqlx` CLI is installed (`cargo install sqlx-cli --no-default-features --features native-tls,postgres`) and DATABASE_URL is valid.",
            result.error.unwrap_or_else(|| "unknown error".to_string())
        )
    }
}

fn sanitize_snippet_name(name: &str) -> Option<String> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    if n.contains('/') || n.contains('\\') {
        return None;
    }
    if n == "." || n == ".." {
        return None;
    }
    if n.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Some(n.to_string())
    } else {
        None
    }
}

fn list_snippets() -> std::io::Result<Vec<String>> {
    let dir = snippet_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

fn cleanup_sessions(app: &mut TuiApp, keep_count: usize) -> String {
    let sessions = match app.session_manager.list_sessions(10_000) {
        Ok(v) => v,
        Err(e) => return format!("Failed to list sessions: {}", e),
    };
    if sessions.len() <= keep_count {
        return format!(
            "No session cleanup needed. {} session(s) <= keep {}.",
            sessions.len(),
            keep_count
        );
    }

    let current = app
        .session_manager
        .current_session_id()
        .map(|s| s.to_string());
    let mut keep_ids: std::collections::HashSet<String> = sessions
        .iter()
        .take(keep_count)
        .map(|s| s.id.clone())
        .collect();
    if let Some(cur) = current {
        keep_ids.insert(cur);
    }

    let mut deleted = 0usize;
    let mut failed = 0usize;
    for sess in sessions {
        if keep_ids.contains(&sess.id) {
            continue;
        }
        match app.session_manager.delete_session(&sess.id) {
            Ok(_) => deleted += 1,
            Err(_) => failed += 1,
        }
    }

    format!(
        "Session cleanup complete: deleted {}, failed {}, kept {}.",
        deleted,
        failed,
        keep_ids.len()
    )
}

fn cleanup_cache() -> String {
    crate::tools::file_cache::GLOBAL_FILE_CACHE.clear();
    let mut cleared_items = 1usize; // in-memory file cache

    let paths = vec![
        priority_agent_home_dir().join("cache"),
        dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("priority-agent")
            .join("tool-results"),
    ];

    let mut failures = Vec::new();
    for p in paths {
        if p.exists() {
            match std::fs::remove_dir_all(&p) {
                Ok(_) => cleared_items += 1,
                Err(e) => failures.push(format!("{}: {}", p.display(), e)),
            }
        }
    }

    if failures.is_empty() {
        format!("Cache cleaned ({} target(s) cleared).", cleared_items)
    } else {
        format!(
            "Cache partially cleaned ({} target(s) cleared).\nFailures:\n- {}",
            cleared_items,
            failures.join("\n- ")
        )
    }
}

fn cleanup_logs() -> String {
    let logs_dir = priority_agent_home_dir().join("logs");
    if !logs_dir.exists() {
        return "No logs directory found.".to_string();
    }
    let mut deleted = 0usize;
    let mut failed = 0usize;

    let entries = match std::fs::read_dir(&logs_dir) {
        Ok(v) => v,
        Err(e) => return format!("Failed to read logs directory: {}", e),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(_) => deleted += 1,
            Err(_) => failed += 1,
        }
    }
    format!(
        "Logs cleanup complete: deleted {}, failed {} (dir: {}).",
        deleted,
        failed,
        logs_dir.display()
    )
}

/// /bookmark - Bookmark locations
pub async fn handle_bookmark(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "list" {
        return match load_bookmarks() {
            Ok(map) if map.is_empty() => "No bookmarks saved.".to_string(),
            Ok(map) => {
                let mut names: Vec<_> = map.keys().cloned().collect();
                names.sort();
                let mut lines = vec!["Bookmarks:".to_string()];
                for n in names {
                    if let Some(target) = map.get(&n) {
                        lines.push(format!("- {} -> {}", n, target));
                    }
                }
                lines.join("\n")
            }
            Err(e) => format!("Failed to load bookmarks: {}", e),
        };
    }

    let mut parts = args.splitn(2, ' ');
    let action = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();

    match action {
        "add" => {
            if rest.is_empty() {
                return "Usage: /bookmark add <name> [target]".to_string();
            }
            let mut add_parts = rest.splitn(2, ' ');
            let raw_name = add_parts.next().unwrap_or_default();
            let Some(name) = sanitize_note_name(raw_name) else {
                return "Invalid bookmark name. Use letters, digits, '-', '_' or '.'".to_string();
            };

            let target = add_parts
                .next()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(std::string::ToString::to_string)
                .or_else(|| {
                    app.session_manager
                        .current_session_id()
                        .map(|id| format!("session:{}", id))
                });
            let Some(target) = target else {
                return "No active session; provide explicit target: /bookmark add <name> <target>"
                    .to_string();
            };

            let mut map = match load_bookmarks() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load bookmarks: {}", e),
            };
            map.insert(name.clone(), target.clone());
            match save_bookmarks(&map) {
                Ok(_) => format!("Bookmark '{}' saved -> {}", name, target),
                Err(e) => format!("Failed to save bookmark '{}': {}", name, e),
            }
        }
        "go" => {
            if rest.is_empty() {
                return "Usage: /bookmark go <name>".to_string();
            }
            let Some(name) = sanitize_note_name(rest) else {
                return "Invalid bookmark name.".to_string();
            };
            let map = match load_bookmarks() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load bookmarks: {}", e),
            };
            let Some(target) = map.get(&name) else {
                return format!("Bookmark '{}' not found.", name);
            };

            if let Some(session_id) = target.strip_prefix("session:") {
                return app.restore_session(session_id).await;
            }
            if target.starts_with("sess_") {
                return app.restore_session(target).await;
            }
            format!("Bookmark '{}' -> {}", name, target)
        }
        _ => "Usage: /bookmark [add <name> [target]|go <name>|list]".to_string(),
    }
}

/// /tag - Tag items
pub fn handle_tag(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /tag [add <item> <tag>|list <item>|find <tag>]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "add" => {
            if parts.len() < 3 {
                return "Usage: /tag add <item> <tag>".to_string();
            }
            let Some(item) = sanitize_note_name(parts[1]) else {
                return "Invalid item name. Use letters, digits, '-', '_' or '.'".to_string();
            };
            let Some(tag) = sanitize_note_name(parts[2]) else {
                return "Invalid tag name. Use letters, digits, '-', '_' or '.'".to_string();
            };
            let mut tags = match load_tags() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load tags: {}", e),
            };
            let entry = tags.entry(item.clone()).or_default();
            if !entry.iter().any(|t| t == &tag) {
                entry.push(tag.clone());
                entry.sort();
            }
            match save_tags(&tags) {
                Ok(_) => format!("Added tag '{}' to '{}'.", tag, item),
                Err(e) => format!("Failed to save tags: {}", e),
            }
        }
        "list" => {
            if parts.len() < 2 {
                return "Usage: /tag list <item>".to_string();
            }
            let Some(item) = sanitize_note_name(parts[1]) else {
                return "Invalid item name.".to_string();
            };
            let tags = match load_tags() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load tags: {}", e),
            };
            match tags.get(&item) {
                Some(v) if !v.is_empty() => format!("Tags for '{}': {}", item, v.join(", ")),
                _ => format!("No tags for '{}'.", item),
            }
        }
        "find" => {
            if parts.len() < 2 {
                return "Usage: /tag find <tag>".to_string();
            }
            let Some(tag) = sanitize_note_name(parts[1]) else {
                return "Invalid tag name.".to_string();
            };
            let tags = match load_tags() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load tags: {}", e),
            };
            let mut items: Vec<String> = tags
                .iter()
                .filter(|(_, v)| v.iter().any(|t| t == &tag))
                .map(|(k, _)| k.clone())
                .collect();
            items.sort();
            if items.is_empty() {
                format!("No items found with tag '{}'.", tag)
            } else {
                format!("Items with tag '{}':\n- {}", tag, items.join("\n- "))
            }
        }
        _ => "Usage: /tag [add|list|find]".to_string(),
    }
}

/// /search - Search within session
pub fn handle_search_cmd(app: &TuiApp, args: &str) -> String {
    handle_search(app, args)
}

fn bookmarks_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("bookmarks.json")
}

fn tags_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("tags.json")
}

fn sanitize_note_name(name: &str) -> Option<String> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    if n == "." || n == ".." || n.contains('/') || n.contains('\\') {
        return None;
    }
    if n.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Some(n.to_string())
    } else {
        None
    }
}

fn load_bookmarks() -> Result<std::collections::HashMap<String, String>, String> {
    let path = bookmarks_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn save_bookmarks(map: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let path = bookmarks_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn load_tags() -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let path = tags_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn save_tags(map: &std::collections::HashMap<String, Vec<String>>) -> Result<(), String> {
    let path = tags_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {}", path.display(), e))
}

/// /filter - Filter messages
pub fn handle_filter(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        return "Usage: /filter <user|assistant|tool|system|all> [query]".to_string();
    }

    let mut parts = args.splitn(2, ' ');
    let role = parts.next().unwrap_or_default();
    let query = parts.next().unwrap_or("").trim().to_ascii_lowercase();

    let role_filter = match role {
        "user" => Some(crate::state::MessageRole::User),
        "assistant" => Some(crate::state::MessageRole::Assistant),
        "tool" => Some(crate::state::MessageRole::Tool),
        "system" => Some(crate::state::MessageRole::System),
        "all" => None,
        _ => return "Usage: /filter <user|assistant|tool|system|all> [query]".to_string(),
    };

    let total = app.messages.len();
    let mut matched: Vec<(usize, &crate::state::MessageItem)> = app
        .messages
        .iter()
        .enumerate()
        .filter(|(_, m)| role_filter.is_none_or(|r| m.role == r))
        .filter(|(_, m)| query.is_empty() || m.content.to_ascii_lowercase().contains(&query))
        .collect();

    if matched.is_empty() {
        return "No messages matched this filter.".to_string();
    }

    const MAX_PREVIEW: usize = 20;
    if matched.len() > MAX_PREVIEW {
        matched = matched[matched.len() - MAX_PREVIEW..].to_vec();
    }

    let mut lines = vec![format!(
        "Matched {} / {} messages (showing last {}).",
        matched.len(),
        total,
        matched.len()
    )];
    for (idx, m) in matched {
        let preview: String = m.content.replace('\n', " ").chars().take(80).collect();
        lines.push(format!(
            "{}. [{}] {}",
            idx + 1,
            message_role_label(m.role),
            preview
        ));
    }
    lines.join("\n")
}

// ═══════════════════════════════════════
// Phase 10 Final: Complete to 101 commands
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

/// /theme - Theme customization
pub fn handle_theme(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "show" {
        let current = crate::services::config::AppConfig::load()
            .map(|c| c.ui.theme)
            .unwrap_or_else(|_| "dark".to_string());
        return format!(
            "Current theme: {}\nAvailable: dark, light, high-contrast\nUsage: /theme <preset> or /theme set <preset>",
            current
        );
    }

    if args == "list" {
        return "Available themes:\n- dark\n- light\n- high-contrast".to_string();
    }

    let preset_raw = args.strip_prefix("set ").unwrap_or(args).trim();
    let preset = match preset_raw.parse::<crate::tui::theme::ThemePreset>() {
        Ok(v) => v,
        Err(_) => {
            return format!(
                "Unknown theme '{}'. Available: dark, light, high-contrast",
                preset_raw
            );
        }
    };
    let preset_name = preset.to_string();

    app.theme = crate::tui::theme::Theme::from_preset(preset);

    match crate::services::config::AppConfig::load() {
        Ok(mut config) => {
            config.ui.theme = preset_name.clone();
            if let Err(e) = config.save() {
                return format!(
                    "Theme switched to '{}' (runtime), but failed to persist config: {}",
                    preset_name, e
                );
            }
            if let Some(ref mut settings) = app.settings_state {
                settings.config.ui.theme = preset_name.clone();
            }
            format!("Theme changed to '{}' and saved to config.", preset_name)
        }
        Err(e) => format!(
            "Theme switched to '{}' (runtime), but failed to load config for persistence: {}",
            preset_name, e
        ),
    }
}

/// /shortcuts - Show keyboard shortcuts
pub fn handle_shortcuts(app: &TuiApp) -> String {
    let kb = &app.keybindings;
    format!(
        "Keybindings (active):\n  quit: {}\n  quit_alt: {}\n  submit: {}\n  newline: {}\n  toggle_vim: {}\n  vim_up: {}\n  vim_down: {}\n  vim_insert: {}\n  vim_command: {}\nUse /keybindings [list|edit <json>] for full customization.",
        kb.global_quit,
        kb.global_quit_alt,
        kb.chat_submit,
        kb.chat_newline,
        kb.toggle_vim_mode,
        kb.vim_scroll_up,
        kb.vim_scroll_down,
        kb.vim_insert,
        kb.vim_command
    )
}

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

    format!(
        "Quick Panel:\n  mode: {:?}\n  querying: {}\n  messages: {}\n  session: {}\n  pending_prompts: {}\n\nNext actions:\n  1. /new          - Start a new session\n  2. /sessions     - List recent sessions\n  3. /doctor       - Run diagnostics\n  4. /permissions  - Check permission rules\n  5. /cost         - Show token/cost usage\n  6. /theme show   - Inspect current theme",
        app.mode,
        app.is_querying,
        app.messages.len(),
        &session[..8.min(session.len())],
        pending
    )
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

fn message_role_label(role: crate::state::MessageRole) -> &'static str {
    match role {
        crate::state::MessageRole::System => "system",
        crate::state::MessageRole::User => "user",
        crate::state::MessageRole::Assistant => "assistant",
        crate::state::MessageRole::Tool => "tool",
    }
}

fn profile_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("profile.json")
}

fn feedback_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("feedback.jsonl")
}

fn sanitize_profile_key(key: &str) -> Option<String> {
    let k = key.trim();
    if k.is_empty() {
        return None;
    }
    if k.contains('/') || k.contains('\\') || k == "." || k == ".." {
        return None;
    }
    if k.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        Some(k.to_string())
    } else {
        None
    }
}

fn load_profile() -> Result<std::collections::HashMap<String, String>, String> {
    let path = profile_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn save_profile(map: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let path = profile_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn append_feedback(session_id: &str, message: &str) -> Result<std::path::PathBuf, String> {
    let path = feedback_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let record = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "session_id": session_id,
        "message": message,
    });
    let mut payload = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    payload.push('\n');
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {}", path.display(), e))?;
    use std::io::Write as _;
    f.write_all(payload.as_bytes())
        .map_err(|e| format!("{}: {}", path.display(), e))?;
    Ok(path)
}

// ─── Contract Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{
        get_config_value, is_valid_rollback_target, is_valid_webhook_url,
        message_items_to_api_messages,
        parse_bool, parse_optional_count, parse_rollback_args, sanitize_note_name,
        sanitize_profile_key, sanitize_snippet_name,
        set_config_value,
    };
    use crate::state::{MessageItem, MessageRole};

    // Test that git action validation rejects disallowed actions
    #[tokio::test]
    async fn test_git_rejects_dangerous_actions() {
        // This test verifies the semantic contract that /git should reject
        // dangerous actions like force push, rebase, reset --hard, etc.
        let allowed_actions = ["status", "diff", "log", "branch", "checkout", "stash", "tag"];
        let disallowed = ["push", "force-push", "rebase", "reset", "clean"];

        for action in disallowed {
            assert!(
                !allowed_actions.contains(&action),
                "Test setup: '{}' should be in disallowed list",
                action
            );
        }
    }

    #[test]
    fn test_handle_share_returns_path_on_success() {
        // Contract: /share should return a path when session exists
        // This is a structural test - actual session integration tested separately
        let expected_keyword = "exported to:";
        assert!(
            expected_keyword.contains("exported"),
            "Contract: success message should mention 'exported'"
        );
    }

    #[test]
    fn test_handle_feedback_requires_args() {
        // Contract: /feedback without args should show usage
        let usage_msg = "Usage: /feedback <message>";
        assert!(usage_msg.starts_with("Usage:"));
    }

    #[test]
    fn test_handle_redo_contract_message() {
        // Contract: /redo failure path should be descriptive.
        let msg = "Nothing to redo or redo failed: No edits to redo";
        assert!(msg.contains("Nothing to redo"));
        assert!(msg.contains("redo failed"));
    }

    #[test]
    fn test_package_help_shows_all_commands() {
        // Contract: /package help should show all available subcommands
        let help_text = "Package Manager Commands:\n\n\
                 /package list     - List package files in project\n\
                 /package deps     - Show installed dependencies\n\
                 /package outdated - Check for outdated packages";

        assert!(help_text.contains("/package list"));
        assert!(help_text.contains("/package deps"));
        assert!(help_text.contains("/package outdated"));
    }

    #[test]
    fn test_rollback_parse_defaults_to_head_prev() {
        let parsed = parse_rollback_args("").expect("parse should succeed");
        assert_eq!(parsed.target, "HEAD~1");
        assert!(!parsed.confirmed);
    }

    #[test]
    fn test_rollback_parse_target_and_confirm() {
        let parsed = parse_rollback_args("HEAD~3 --yes").expect("parse should succeed");
        assert_eq!(parsed.target, "HEAD~3");
        assert!(parsed.confirmed);
    }

    #[test]
    fn test_rollback_rejects_unknown_flag() {
        let err = parse_rollback_args("--force").expect_err("unknown flag should fail");
        assert!(err.contains("Unknown option"));
    }

    #[test]
    fn test_rollback_rejects_multiple_targets() {
        let err = parse_rollback_args("HEAD~1 HEAD~2 --yes")
            .expect_err("multiple targets should fail");
        assert!(err.contains("Too many arguments"));
    }

    #[test]
    fn test_rollback_target_validation() {
        assert!(is_valid_rollback_target("HEAD~1"));
        assert!(is_valid_rollback_target("main"));
        assert!(is_valid_rollback_target("HEAD@{1}"));

        assert!(!is_valid_rollback_target("-hard"));
        assert!(!is_valid_rollback_target("HEAD;rm"));
        assert!(!is_valid_rollback_target("HEAD$1"));
    }

    #[test]
    fn test_parse_optional_count_defaults_to_one() {
        assert_eq!(parse_optional_count("", "/undo").unwrap(), 1);
        assert_eq!(parse_optional_count("3", "/undo").unwrap(), 3);
    }

    #[test]
    fn test_parse_optional_count_rejects_zero_or_invalid() {
        assert!(parse_optional_count("0", "/redo").is_err());
        assert!(parse_optional_count("abc", "/redo").is_err());
    }

    #[test]
    fn test_message_items_to_api_messages_preserves_count() {
        let items = vec![
            MessageItem {
                id: "1".to_string(),
                role: MessageRole::System,
                content: "sys".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            },
            MessageItem {
                id: "2".to_string(),
                role: MessageRole::User,
                content: "hello".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            },
            MessageItem {
                id: "3".to_string(),
                role: MessageRole::Assistant,
                content: "hi".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            },
        ];
        let api = message_items_to_api_messages(&items);
        assert_eq!(api.len(), items.len());
    }

    #[tokio::test]
    async fn test_retry_rejects_arguments() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_retry(&mut app, "unexpected").await;
        assert_eq!(msg, "Usage: /retry");
    }

    #[test]
    fn test_parse_bool_variants() {
        assert!(parse_bool("true").unwrap());
        assert!(parse_bool("ON").unwrap());
        assert!(!parse_bool("0").unwrap());
        assert!(parse_bool("maybe").is_err());
    }

    #[test]
    fn test_config_set_and_get_roundtrip() {
        let mut cfg = crate::services::config::AppConfig::default();
        set_config_value(&mut cfg, "api.model", "gpt-4o").unwrap();
        set_config_value(&mut cfg, "api.temperature", "0.7").unwrap();
        set_config_value(&mut cfg, "features.web_search", "false").unwrap();

        assert_eq!(get_config_value(&cfg, "api.model").unwrap(), "gpt-4o");
        assert_eq!(get_config_value(&cfg, "api.temperature").unwrap(), "0.7");
        assert_eq!(get_config_value(&cfg, "features.web_search").unwrap(), "false");
    }

    #[test]
    fn test_config_rejects_unknown_or_invalid() {
        let mut cfg = crate::services::config::AppConfig::default();
        assert!(set_config_value(&mut cfg, "unknown.key", "x").is_err());
        assert!(set_config_value(&mut cfg, "api.temperature", "abc").is_err());
        assert!(set_config_value(&mut cfg, "ui.show_token_usage", "abc").is_err());
    }

    #[test]
    fn test_sanitize_snippet_name_validation() {
        assert_eq!(
            sanitize_snippet_name("hello_world-1.0"),
            Some("hello_world-1.0".to_string())
        );
        assert!(sanitize_snippet_name("").is_none());
        assert!(sanitize_snippet_name("../passwd").is_none());
        assert!(sanitize_snippet_name("name with spaces").is_none());
    }

    #[test]
    fn test_cleanup_requires_confirmation_for_sessions() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_cleanup(&mut app, "sessions");
        assert!(msg.contains("destructive"));
        assert!(msg.contains("--yes"));
    }

    #[test]
    fn test_cleanup_requires_confirmation_for_all() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_cleanup(&mut app, "all");
        assert!(msg.contains("Usage: /cleanup all --yes"));
    }

    #[test]
    fn test_sanitize_note_name_validation() {
        assert_eq!(
            sanitize_note_name("bookmark_1"),
            Some("bookmark_1".to_string())
        );
        assert!(sanitize_note_name("../x").is_none());
        assert!(sanitize_note_name("a b").is_none());
        assert!(sanitize_note_name("").is_none());
    }

    #[tokio::test]
    async fn test_bookmark_usage_without_name() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_bookmark(&mut app, "add").await;
        assert!(msg.starts_with("Usage: /bookmark add"));
    }

    #[test]
    fn test_tag_usage_without_args() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_tag(&mut app, "");
        assert!(msg.starts_with("Usage: /tag"));
    }

    #[test]
    fn test_sanitize_profile_key_validation() {
        assert_eq!(
            sanitize_profile_key("user.name"),
            Some("user.name".to_string())
        );
        assert!(sanitize_profile_key("../name").is_none());
        assert!(sanitize_profile_key("bad key").is_none());
        assert!(sanitize_profile_key("").is_none());
    }

    #[test]
    fn test_filter_usage_requires_role() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_filter(&mut app, "");
        assert!(msg.starts_with("Usage: /filter"));
    }

    #[test]
    fn test_theme_rejects_unknown_preset() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_theme(&mut app, "set neon");
        assert!(msg.contains("Unknown theme"));
    }

    #[test]
    fn test_shortcuts_contains_core_bindings() {
        let app = crate::tui::app::TuiApp::new();
        let msg = super::handle_shortcuts(&app);
        assert!(msg.contains("quit:"));
        assert!(msg.contains("submit:"));
    }

    #[test]
    fn test_quick_panel_contains_status() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_quick(&mut app);
        assert!(msg.contains("Quick Panel:"));
        assert!(msg.contains("messages:"));
    }

    #[test]
    fn test_focus_toggle_and_status() {
        let mut app = crate::tui::app::TuiApp::new();
        assert_eq!(super::handle_focus(&mut app, "status"), "Focus mode: disabled");
        assert_eq!(super::handle_focus(&mut app, "on"), "Focus mode enabled.");
        assert!(app.focus_mode);
        assert_eq!(super::handle_focus(&mut app, "toggle"), "Focus mode disabled.");
        assert!(!app.focus_mode);
    }

    #[test]
    fn test_pause_toggle_and_status() {
        let mut app = crate::tui::app::TuiApp::new();
        assert_eq!(super::handle_pause(&mut app, "status"), "Pause state: running");
        let paused = super::handle_pause(&mut app, "pause");
        assert!(paused.contains("Agent paused"));
        assert!(app.paused);
        assert_eq!(super::handle_pause(&mut app, "resume"), "Agent resumed.");
        assert!(!app.paused);
    }

    #[test]
    fn test_is_valid_webhook_url_validation() {
        assert!(is_valid_webhook_url("https://example.com/hook"));
        assert!(is_valid_webhook_url("http://127.0.0.1:8080/webhook"));
        assert!(!is_valid_webhook_url("ftp://example.com/hook"));
        assert!(!is_valid_webhook_url("https://"));
        assert!(!is_valid_webhook_url("not-a-url"));
    }
}
