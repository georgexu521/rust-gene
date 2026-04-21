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

pub fn handle_share(app: &TuiApp) -> String {
    let share_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
        .join("shared");
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("session_{}.md", timestamp);
    let output_path = share_dir.join(&filename);
    let _ = std::fs::create_dir_all(&share_dir);

    let mut lines = vec![
        "# Session Export\n".to_string(),
        format!(
            "**Session ID**: {}\n",
            app.session_manager
                .current_session_id()
                .unwrap_or("unknown")
        ),
        format!(
            "**Exported at**: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
        format!("**Messages**: {}\n", app.messages.len()),
        "---\n".to_string(),
    ];
    for msg in &app.messages {
        use crate::state::MessageRole;
        let role_label = match msg.role {
            MessageRole::User => "**User**",
            MessageRole::Assistant => "**Assistant**",
            MessageRole::System => "**System**",
            MessageRole::Tool => "**Tool**",
        };
        lines.push(format!("\n{}\n\n{}\n", role_label, msg.content));
    }
    let markdown = lines.join("\n");

    match std::fs::write(&output_path, markdown) {
        Ok(()) => format!(
            "Session exported to {} ({} messages)",
            output_path.display(),
            app.messages.len()
        ),
        Err(e) => format!("Failed to export session: {}", e),
    }
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

use crate::permissions::{PermissionMode, RuleSource};
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
                "Permission mode: {}\nRules: allow={} deny={} ask={}\nProject config: {}\nGlobal config: {}\n\nUsage:\n  /permissions mode <default|auto_low_risk|auto_all|read_only>\n  /permissions rules [tool_name]\n  /permissions <allow|deny|ask> <pattern> [project|global]",
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
