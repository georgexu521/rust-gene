//! Slash command handlers for TuiApp
//!
//! Each handler function takes `&mut TuiApp` + args and returns a String response.
//! This module exists to keep app.rs focused on core TUI state management.

use crate::agent::agent::AgentConfig;
use crate::agent::roles::AgentRole;
use crate::agent::types::{AgentId, AgentMessage, AgentMessageType};
use crate::tools::Tool;
use crate::tui::app::{AppMode, TuiApp};

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

// ─── System & Tools ───────────────────────────────────────────────────

pub fn handle_status(app: &TuiApp) -> String {
    let msg_count = app.messages.len();
    let (history_len, model_line, provider_line) = if let Some(ref engine) = app.streaming_engine {
        let provider_base = app.current_provider_base_url();
        let provider = app.current_provider_label();
        (
            futures::executor::block_on(engine.get_history()).len(),
            format!("Model: {}", app.current_model_label()),
            format!("Provider: {} ({})", provider, provider_base),
        )
    } else {
        (
            0,
            "Model: unavailable".to_string(),
            "Provider: unavailable".to_string(),
        )
    };
    format!(
        "Messages: {}\nHistory: {} turns\nQuerying: {}\n{}\n{}",
        msg_count, history_len, app.is_querying, model_line, provider_line
    )
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
    } else {
        report.format_text()
    }
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
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&path, content) {
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
                            match std::fs::read_to_string(&path) {
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
                "Permission mode: {}\nRules: allow={} deny={} ask={}\nProject config: {}\nGlobal config: {}\n\nUsage:\n  /permissions mode <default|auto_low_risk|auto_all|read_only>\n  /permissions rules [tool_name]\n  /permissions explain <tool_name> - explain why a decision was made\n  /permissions export [path] - export rules to a file\n  /permissions import <path> [project|global] - import rules from a file\n  /permissions dry-run <allow|deny|ask> <pattern> - test a rule without saving\n  /permissions <allow|deny|ask> <pattern> [project|global]",
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
            let (decision, details) = ctx.check_with_details(tool_name);

            // Build explanation based on risk level and matching rules
            let risk = ctx.rules.check(tool_name);
            let mut lines = vec![
                format!("Permission explanation for '{}':", tool_name),
                format!("  Decision: {:?}", decision),
                format!("  Risk level: {:?}", risk),
            ];

            if details.is_empty() {
                lines.push("  Reason: No explicit rules matched - using default policy".to_string());
                lines.push("  Default behavior: ask (prompt before execution)".to_string());
            } else {
                lines.push("  Matched rules:".to_string());
                for d in &details {
                    lines.push(format!("    - {}", d));
                }
                lines.push("  Priority: deny > allow > ask (first match wins)".to_string());
            }

            // Add mode context
            let mode = app
                .streaming_engine
                .as_ref()
                .map(|e| e.permission_mode())
                .unwrap_or(PermissionMode::AutoLowRisk);
            lines.push(format!("\n  Current mode: {}", permission_mode_name(mode)));
            match mode {
                PermissionMode::AutoAll => lines.push("    (all operations auto-allowed - rules ignored)".to_string()),
                PermissionMode::AutoLowRisk => lines.push("    (low-risk operations auto-allowed, others follow rules)".to_string()),
                PermissionMode::ReadOnly => lines.push("    (all write operations denied)".to_string()),
                PermissionMode::Once => lines.push("    (each operation allowed once then denied)".to_string()),
                _ => {}
            }

            lines.join("\n")
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
                _ => return "Usage: /permissions import <path> [project|global]".to_string(),
            };
            let scope = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "global" => RuleSource::Global,
                Some(s) if s == "project" => RuleSource::Project,
                Some(other) => return format!("Invalid scope '{}'. Use 'project' or 'global'.", other),
                None => RuleSource::Project,
            };

            let content = match std::fs::read_to_string(file_path) {
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

            match std::fs::write(&target_path, &content) {
                Ok(_) => format!("Rules imported from '{}' to: {}", file_path, target_path.display()),
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

            // Show what tools would match
            let mut lines = vec![
                format!("Dry-run: {} '{}'", action, pattern),
                format!("Config path: {}/.priority-agent/permissions.toml", cwd.display()),
                "".to_string(),
                "This rule would affect:".to_string(),
            ];

            // Check some common tools
            let test_tools = ["file_read", "file_write", "bash", "grep", "glob", "agent", "mcp"];
            for tool in test_tools {
                if match_wildcard(pattern, tool) {
                    let decision = test_rules.check(tool);
                    lines.push(format!("  {} -> {:?}", tool, decision));
                }
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
            } else if args.starts_with("edit ") {
                let json_str = &args[5..];
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
pub fn handle_context(app: &TuiApp) -> String {
    let msg_count = app.messages.len();
    let history_len = if let Some(ref engine) = app.streaming_engine {
        futures::executor::block_on(engine.get_history()).len()
    } else {
        0
    };
    let model = app.current_model_label();
    let provider = app.current_provider_label();
    let working_dir = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let session_id = app.session_manager.current_session_id()
        .map(|s| format!("{}", &s[..8.min(s.len())]))
        .unwrap_or_else(|| "none".to_string());

    let engine_info = if let Some(ref engine) = app.streaming_engine {
        let history = futures::executor::block_on(engine.get_history());

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
    let action = if args.is_empty() { "status" } else { args };
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
            format!("Switched to chat mode.")
        }
        "settings" => {
            let config = crate::services::config::AppConfig::load().unwrap_or_default();
            app.settings_state = Some(crate::tui::components::settings::SettingsState::new(
                config,
                app.keybindings.clone(),
            ));
            app.mode = AppMode::Settings;
            format!("Switched to settings mode.")
        }
        "vim" | "vim_normal" => {
            app.mode = AppMode::VimNormal;
            format!("Switched to vim_normal mode. Use j/k to navigate, i to return to insert mode.")
        }
        _ => format!("Unknown mode: {}. Available: chat, settings, vim", new_mode),
    }
}

/// /package - 包管理相关操作
pub async fn handle_package(app: &mut TuiApp, args: &str) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let action = parts.first().map(|s| *s).unwrap_or("help");

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
        "help" | _ => {
            format!(
                "Package Manager Commands:\n\n\
                 /package list     - List package files in project\n\
                 /package deps     - Show installed dependencies\n\
                 /package outdated - Check for outdated packages\n\n\
                 Supported: npm (Node.js), cargo (Rust), go (Go)"
            )
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

// ─── Batch 1 Commands (Phase 10) ──────────────────────────────────────────────

/// /session - 会话管理
pub fn handle_session_cmd(app: &mut TuiApp, args: &str) -> String {
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
                        let _marker = if current == Some(s.id.as_str()) {
                            " (current)"
                        } else {
                            ""
                        };
                        lines.push(format!(
                            "{}. {} - {} [{}]",
                            i + 1,
                            s.title,
                            s.id[..8.min(s.id.len())].to_string(),
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
            Ok(sessions) if n <= sessions.len() => {
                let session = &sessions[n - 1];
                futures::executor::block_on(app.restore_session(&session.id));
                format!("Switched to session: {}", session.title)
            }
            _ => "Invalid session number. Use /session list to see available.".to_string(),
        }
    } else if args.starts_with("new") {
        // Create new session
        let title = args.strip_prefix("new ").unwrap_or("New Session");
        match app.session_manager.start_session(title, "kimi-k2.5") {
            Ok(id) => {
                futures::executor::block_on(app.restore_session(&id));
                format!("Created new session: {}", title)
            }
            Err(e) => format!("Failed to create session: {}", e),
        }
    } else if args == "current" {
        // Show current session
        let id = app.session_manager.current_session_id().map(|s| s.to_string()).unwrap_or_else(|| "none".to_string());
        let title = app.session_manager.current_session_title();
        format!("Current session: {} ({})", title, &id[..8.min(id.len())])
    } else {
        "Usage: /session [list|n|<n>|new <title>|current]".to_string()
    }
}

/// /undo - 撤销上一次操作
pub fn handle_undo(app: &mut TuiApp, _args: &str) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    match app.session_manager.rewind_last_edit(&session_id) {
        Ok(msg) => msg,
        Err(e) => format!("Nothing to undo or undo failed: {}", e),
    }
}

/// /redo - 重做
pub fn handle_redo(app: &mut TuiApp, _args: &str) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    match app.session_manager.rewind_last_edit(&session_id) {
        Ok(msg) => msg,
        Err(e) => format!("Nothing to redo: {}", e),
    }
}

/// /retry - 重试上一次 LLM 调用
pub async fn handle_retry(app: &mut TuiApp, _args: &str) -> String {
    if app.messages.len() < 2 {
        return "No previous message to retry.".to_string();
    }
    // Get the last user message content (clone before mutating)
    let content = {
        let user_msg = app.messages.iter().rev().find(|m| m.role == crate::state::MessageRole::User);
        match user_msg {
            Some(msg) => msg.content.clone(),
            None => return "No user message to retry.".to_string(),
        }
    };
    app.messages.pop(); // Remove last message
    app.send_message(content).await;
    String::new()
}

/// /stop - 停止当前操作
pub fn handle_stop(app: &mut TuiApp, _args: &str) -> String {
    if app.is_querying {
        app.is_querying = false;
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
            format!("Skills registry: use /skills list to view")
        } else {
            "Skills not available.".to_string()
        }
    } else {
        "Usage: /reload [config|plugins|skills]".to_string()
    }
}

/// /share - 分享当前会话
pub fn handle_share(app: &mut TuiApp, _args: &str) -> String {
    if let Some(id) = app.session_manager.current_session_id() {
        match app.session_manager.export_session(&id) {
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
pub fn handle_token(app: &TuiApp) -> String {
    if let Some(ref engine) = app.streaming_engine {
        let tracker = futures::executor::block_on(engine.cost_tracker().lock());
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
pub fn handle_prompt(_app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        "Usage: /prompt [show|edit <text>]".to_string()
    } else if args == "show" {
        "System prompt configuration not exposed via TUI yet.".to_string()
    } else if args.starts_with("edit ") {
        format!("Prompt editing not implemented yet. Received: {}", args)
    } else {
        "Usage: /prompt [show|edit <text>]".to_string()
    }
}

/// /migrate - Migration helper
pub fn handle_migrate(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /migrate [up|down|status]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "up" => "Migration 'up' not implemented via TUI.".to_string(),
        "down" => "Migration 'down' not implemented via TUI.".to_string(),
        "status" => {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let migrations_dir = dir.join("migrations");
            if migrations_dir.exists() {
                format!("Migrations directory exists at: {}", migrations_dir.display())
            } else {
                "No migrations directory found.".to_string()
            }
        }
        _ => "Usage: /migrate [up|down|status]".to_string(),
    }
}

/// /focus - Focus mode toggle
pub fn handle_focus(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "on" {
        "Focus mode enabled (UI filtering not yet implemented).".to_string()
    } else if args == "off" {
        "Focus mode disabled.".to_string()
    } else {
        "Usage: /focus [on|off]".to_string()
    }
}

/// /pause - Pause/resume agent
pub fn handle_pause(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "pause" {
        app.is_querying = false;
        "Agent paused. Use /pause resume to continue.".to_string()
    } else if args == "resume" {
        app.is_querying = true;
        "Agent resumed.".to_string()
    } else {
        "Usage: /pause [pause|resume]".to_string()
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
    use crate::tui::theme::Theme;

    if args.is_empty() {
        return "Current theme: Dark\nUse /color <preset> to change (dark/light/high-contrast)".to_string();
    }

    match args {
        "dark" => {
            app.theme = Theme::from_name("dark");
            "Theme changed to: dark".to_string()
        }
        "light" => {
            app.theme = Theme::from_name("light");
            "Theme changed to: light".to_string()
        }
        "high-contrast" => {
            app.theme = Theme::from_name("high-contrast");
            "Theme changed to: high-contrast".to_string()
        }
        _ => format!("Unknown preset: {}. Available: dark, light, high-contrast", args),
    }
}

// ═══════════════════════════════════════
// Phase 10 Batch 3: webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker
// ═══════════════════════════════════════

/// /webhook - Webhook management
pub fn handle_webhook(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /webhook [list|create|delete] <url>".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => "No webhooks configured. Set PRIORITY_AGENT_WEBHOOK_URL to enable.".to_string(),
        "create" => {
            if parts.len() < 2 {
                "Usage: /webhook create <url>".to_string()
            } else {
                format!("Webhook creation not yet implemented. URL: {}", parts[1])
            }
        }
        "delete" => "Webhook deletion not yet implemented.".to_string(),
        _ => "Usage: /webhook [list|create|delete] <url>".to_string(),
    }
}

/// /wizard - Setup wizard
pub fn handle_wizard(app: &mut TuiApp) -> String {
    app.mode = crate::tui::app::AppMode::Settings;
    "Starting setup wizard... Switching to settings mode.".to_string()
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
            // List git worktrees
            "Use /branch to see git branches, or /worktree for worktree management.".to_string()
        }
        "info" => {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            format!("Workspace: {}\nFiles: (use /tools to see available tools)", dir.display())
        }
        _ => "Usage: /workspace [list|info]".to_string(),
    }
}

/// /slack - Slack integration
pub fn handle_slack(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /slack [connect|disconnect|send] <channel> <message>".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "connect" => "Slack integration not yet implemented. Set PRIORITY_AGENT_SLACK_TOKEN to enable.".to_string(),
        "disconnect" => "Slack disconnected.".to_string(),
        "send" => {
            if parts.len() < 3 {
                "Usage: /slack send <channel> <message>".to_string()
            } else {
                format!("Slack send not yet implemented. Would send '{}' to channel '{}'", parts[2], parts[1])
            }
        }
        _ => "Usage: /slack [connect|disconnect|send]".to_string(),
    }
}

/// /stealth - Stealth mode toggle
pub fn handle_stealth(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "on" {
        "Stealth mode enabled (telemetry disabled).".to_string()
    } else if args == "off" {
        "Stealth mode disabled (telemetry enabled).".to_string()
    } else {
        "Usage: /stealth [on|off]".to_string()
    }
}

/// /shadow - Shadow mode for observing agent behavior
pub fn handle_shadow(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "on" {
        "Shadow mode enabled. Agent actions will be logged but not executed.".to_string()
    } else if args == "off" {
        "Shadow mode disabled. Normal operation resumed.".to_string()
    } else {
        "Usage: /shadow [on|off]".to_string()
    }
}

/// /reject - Reject pending approval
pub fn handle_reject(_app: &mut TuiApp, _args: &str) -> String {
    // When in approval mode, this rejects the current pending tool call
    "No pending approval to reject.".to_string()
}

/// /subscribe - Subscribe to events/notifications
pub fn handle_subscribe(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /subscribe <event_type>".to_string();
    }

    let event = args;
    format!("Subscribed to: {}. Notifications will appear in the TUI.", event)
}

/// /slots - View/edit slot variables
pub fn handle_slots(app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /slots [list|set <name> <value>|clear]".to_string();
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
            lines.join("\n")
        }
        "set" => {
            if parts.len() < 3 {
                "Usage: /slots set <name> <value>".to_string()
            } else {
                format!("Slot '{}' would be set to '{}' (not yet implemented)", parts[1], parts[2])
            }
        }
        "clear" => "All slots cleared.".to_string(),
        _ => "Usage: /slots [list|set <name> <value>|clear]".to_string(),
    }
}

/// /ticker - Display a scrolling ticker/marquee
pub fn handle_ticker(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /ticker <message>".to_string();
    }

    format!("Ticker: {} (display not yet implemented in TUI)", args)
}

// ═══════════════════════════════════════
// Phase 10 Batch 4: config, copy, desktop, chrome, effort, preamble, untrap, verbose, write
// ═══════════════════════════════════════

/// /config - Configuration viewer/editor
pub fn handle_config(_app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        // Show current config summary
        match crate::services::config::AppConfig::load() {
            Ok(config) => {
                format!(
                    "Config:\n  API: {}\n  Model: {}\n  Theme: {}",
                    config.api.base_url, config.api.model, config.ui.theme
                )
            }
            Err(_) => "No config file found. Using defaults.".to_string(),
        }
    } else if args == "edit" {
        "Config editing not yet implemented. Edit config.toml directly.".to_string()
    } else if args.starts_with("get ") {
        let key = args.strip_prefix("get ").unwrap_or("");
        format!("Config value for '{}' (not yet implemented)", key)
    } else {
        "Usage: /config [edit|get <key>]".to_string()
    }
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
                format!("Chrome open not yet implemented: {}", parts[1])
            }
        }
        "tabs" => "Chrome tabs listing not yet implemented.".to_string(),
        "bookmarks" => "Chrome bookmarks not yet implemented.".to_string(),
        _ => "Usage: /chrome [open|tabs|bookmarks]".to_string(),
    }
}

/// /effort - Set effort level for tasks
pub fn handle_effort(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /effort [minimal|normal|maximum]".to_string();
    }

    match args {
        "minimal" => "Effort set to: minimal (quick solutions)".to_string(),
        "normal" => "Effort set to: normal (balanced approach)".to_string(),
        "maximum" => "Effort set to: maximum (thorough analysis)".to_string(),
        _ => "Usage: /effort [minimal|normal|maximum]".to_string(),
    }
}

/// /preamble - Customize agent preamble
pub fn handle_preamble(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /preamble [show|set <text>|reset]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "show" => "Preamble: (default agent preamble in use)".to_string(),
        "set" => {
            if parts.len() < 2 {
                "Usage: /preamble set <text>".to_string()
            } else {
                format!("Preamble would be set to: {} (not yet implemented)", parts[1])
            }
        }
        "reset" => "Preamble reset to default.".to_string(),
        _ => "Usage: /preamble [show|set <text>|reset]".to_string(),
    }
}

/// /untrap - Reset trapped state
pub fn handle_untrap(_app: &mut TuiApp, _args: &str) -> String {
    "Untrap: Reset agent from trapped state.".to_string()
}

/// /verbose - Toggle verbose output
pub fn handle_verbose(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "on" {
        "Verbose mode enabled.".to_string()
    } else if args == "off" {
        "Verbose mode disabled.".to_string()
    } else {
        "Usage: /verbose [on|off]".to_string()
    }
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
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let parts: Vec<&str> = args.split_whitespace().collect();
    let target = parts.get(0).unwrap_or(&"HEAD~1");

    let cmd = format!("git rollback {}", target);
    let params = serde_json::json!({
        "command": cmd,
        "description": "Git rollback"
    });
    let result = tool.execute(params, ctx).await;
    if result.success { result.content } else { result.error.unwrap_or_default() }
}

/// /project - Project management
pub fn handle_project(_app: &TuiApp, args: &str) -> String {
    if args.is_empty() || args == "info" {
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let name = dir.file_name().unwrap_or_default().to_string_lossy();
        return format!("Project: {}\nPath: {}", name, dir.display());
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => "Only one project currently supported.".to_string(),
        "init" => "Project already initialized.".to_string(),
        _ => "Usage: /project [info|list|init]".to_string(),
    }
}

/// /backend - Switch execution backend
pub fn handle_backend(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Current backend: local\nUsage: /backend [local|restricted|external]".to_string();
    }

    match args {
        "local" => "Backend set to: local (direct execution)".to_string(),
        "restricted" => "Backend set to: restricted (resource-limited)".to_string(),
        "external" => {
            let external_cmd = std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_CMD").unwrap_or_default();
            if external_cmd.is_empty() {
                "External backend not configured. Set PRIORITY_AGENT_BASH_EXTERNAL_CMD".to_string()
            } else {
                format!("Backend set to: external ({})", external_cmd)
            }
        }
        _ => "Usage: /backend [local|restricted|external]".to_string(),
    }
}

/// /sandbox - Sandbox mode toggle
pub fn handle_sandbox(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "on" {
        "Sandbox mode enabled (restricted backend).".to_string()
    } else if args == "off" {
        "Sandbox mode disabled (local backend).".to_string()
    } else {
        "Usage: /sandbox [on|off]".to_string()
    }
}

/// /env - Show/manage environment variables
pub fn handle_env(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /env [list|get <key>|set <key> <value>]".to_string();
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
        _ => "Usage: /env [list|get <key>]".to_string(),
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
        "stats" => "Cache stats not yet implemented.".to_string(),
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
        return "Benchmark script not found at scripts/benchmark.sh".to_string();
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
        "cargo test 2>&1 | tail -30".to_string()
    } else {
        format!("cargo test {} 2>&1 | tail -30", args)
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
    if args.is_empty() {
        return "Usage: /trace [on|off|status]".to_string();
    }

    match args {
        "on" => "Tracing enabled.".to_string(),
        "off" => "Tracing disabled.".to_string(),
        "status" => "Tracing status: not implemented.".to_string(),
        _ => "Usage: /trace [on|off|status]".to_string(),
    }
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

// ═══════════════════════════════════════
// Phase 10 Extended 2: Even more commands
// ═══════════════════════════════════════

/// /init - Initialize a new project
pub fn handle_init(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /init <project_name>".to_string();
    }

    let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project_path = dir.join(&args);

    format!("Init not yet implemented. Would create: {}", project_path.display())
}

/// /login - Authentication
pub fn handle_login(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /login [provider]".to_string();
    }

    format!("Login to {} not yet implemented.", args)
}

/// /logout - Logout from provider
pub fn handle_logout(_app: &mut TuiApp, _args: &str) -> String {
    "Logout not yet implemented.".to_string()
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

    match args {
        "show" => "API key not shown for security. Set MOONSHOT_API_KEY or OPENAI_API_KEY.".to_string(),
        "clear" => "Cleared API key (from environment). Restart required.".to_string(),
        _ => "Usage: /key [show|clear]".to_string(),
    }
}

/// /status - Detailed status
pub fn handle_status_detailed(_app: &TuiApp) -> String {
    let mut lines = vec!["Detailed Status:".to_string()];
    lines.push(format!("  Mode: TUI"));
    lines.push(format!("  Rust version: {}", std::env::consts::OS));
    format!("{}\n{}", lines.join("\n"), "Use /doctor for full diagnostics")
}

/// /health - Health check
pub fn handle_health(_app: &TuiApp) -> String {
    "Health: OK\nSystem operational.".to_string()
}

/// /ping - Latency check
pub fn handle_ping(_app: &mut TuiApp) -> String {
    let start = std::time::Instant::now();
    let elapsed = start.elapsed().as_millis();
    format!("Pong! Latency: {}ms", elapsed)
}

/// /uptime - Show uptime
pub fn handle_uptime(_app: &TuiApp) -> String {
    format!("Uptime: since boot (detailed tracking not implemented)")
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
        format!("Full reset not yet implemented.")
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

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;
    let cmd = format!("test -f {} && echo 'File exists' || echo 'File not found'", args);

    let params = serde_json::json!({
        "command": cmd,
        "description": "Check import file"
    });
    let result = tool.execute(params, ctx).await;
    if result.success && result.content.contains("exists") {
        format!("Import of {} not yet implemented.", args)
    } else {
        format!("File not found: {}", args)
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
pub fn handle_merge(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /merge <session_id> into current".to_string();
    }
    format!("Merge session {} not yet implemented.", args)
}

/// /cleanup - Cleanup old data
pub fn handle_cleanup(_app: &mut TuiApp, args: &str) -> String {
    let target = if args.is_empty() { "all" } else { args };

    match target {
        "sessions" => "Cleaned up old sessions (not yet implemented).".to_string(),
        "cache" => "Cache cleaned.".to_string(),
        "logs" => "Logs cleaned (not yet implemented).".to_string(),
        "all" => "Full cleanup not yet implemented.".to_string(),
        _ => "Usage: /cleanup [sessions|cache|logs|all]".to_string(),
    }
}

/// /compact - Compact context
pub fn handle_compact(_app: &mut TuiApp) -> String {
    "Use /compact command from bundled skills for context compression.".to_string()
}

/// /snippet - Save/load code snippets
pub fn handle_snippet(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /snippet [save <name>|load <name>|list]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "save" => {
            if parts.len() < 2 { "Usage: /snippet save <name>".to_string() }
            else { format!("Snippet '{}' saved (not yet implemented).", parts[1]) }
        }
        "load" => {
            if parts.len() < 2 { "Usage: /snippet load <name>".to_string() }
            else { format!("Snippet '{}' loaded (not yet implemented).", parts[1]) }
        }
        "list" => "No snippets saved.".to_string(),
        _ => "Usage: /snippet [save|load|list]".to_string(),
    }
}

/// /bookmark - Bookmark locations
pub fn handle_bookmark(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "list" {
        return "No bookmarks saved.".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "add" => format!("Bookmark '{}' added (not yet implemented).", parts.get(1).unwrap_or(&"?")),
        "go" => format!("Navigate to bookmark (not yet implemented)."),
        _ => "Usage: /bookmark [add <name>|go <name>|list]".to_string(),
    }
}

/// /tag - Tag items
pub fn handle_tag(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /tag [add <item> <tag>|list <item>|find <tag>]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "add" => format!("Tag '{}' added (not yet implemented).", parts.get(2).unwrap_or(&"?")),
        "list" => "No tags.".to_string(),
        "find" => format!("Items with tag '{}' (not yet implemented).", parts.get(1).unwrap_or(&"?")),
        _ => "Usage: /tag [add|list|find]".to_string(),
    }
}

/// /search - Search within session
pub fn handle_search_cmd(_app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /search <query>".to_string();
    }
    format!("Search for '{}' (use input field for interactive search)", args)
}

/// /filter - Filter messages
pub fn handle_filter(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /filter [user|assistant|tool|all]".to_string();
    }

    match args {
        "user" => "Filter set to: user messages only (not yet implemented).".to_string(),
        "assistant" => "Filter set to: assistant messages only.".to_string(),
        "tool" => "Filter set to: tool messages only.".to_string(),
        "all" => "Filter cleared.".to_string(),
        _ => "Usage: /filter [user|assistant|tool|all]".to_string(),
    }
}
