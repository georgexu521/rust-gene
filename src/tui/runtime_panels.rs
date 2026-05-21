use crate::state::{RuntimeTerminalTask, TaskItem, TaskStatus};
use crate::tui::app::TuiApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePanelKind {
    All,
    Diff,
    Approval,
    Hooks,
    Context,
    Tasks,
    Mcp,
    Bridge,
}

impl RuntimePanelKind {
    pub fn parse(args: &str) -> Option<Self> {
        let name = args.split_whitespace().next().unwrap_or("all");
        match name.to_ascii_lowercase().as_str() {
            "" | "all" | "runtime" | "status" => Some(Self::All),
            "diff" | "changes" => Some(Self::Diff),
            "approval" | "approvals" | "permission" | "permissions" => Some(Self::Approval),
            "hook" | "hooks" => Some(Self::Hooks),
            "context" | "memory" | "tokens" => Some(Self::Context),
            "task" | "tasks" | "agents" => Some(Self::Tasks),
            "mcp" | "servers" => Some(Self::Mcp),
            "bridge" | "remote" | "remotes" => Some(Self::Bridge),
            _ => None,
        }
    }

    pub const fn usage() -> &'static str {
        "Usage: /panel [all|diff|approval|hooks|context|tasks|mcp|bridge]"
    }
}

pub async fn render_runtime_panel(app: &TuiApp, kind: RuntimePanelKind) -> String {
    match kind {
        RuntimePanelKind::All => {
            let mut sections = vec![
                render_context_panel(app).await,
                render_approval_panel(app),
                render_hooks_panel(app),
                render_tasks_panel(app).await,
                render_mcp_panel(app).await,
                render_bridge_panel(app),
                render_diff_panel(app),
            ];
            sections.retain(|section| !section.trim().is_empty());
            sections.join("\n\n")
        }
        RuntimePanelKind::Diff => render_diff_panel(app),
        RuntimePanelKind::Approval => render_approval_panel(app),
        RuntimePanelKind::Hooks => render_hooks_panel(app),
        RuntimePanelKind::Context => render_context_panel(app).await,
        RuntimePanelKind::Tasks => render_tasks_panel(app).await,
        RuntimePanelKind::Mcp => render_mcp_panel(app).await,
        RuntimePanelKind::Bridge => render_bridge_panel(app),
    }
}

fn render_diff_panel(app: &TuiApp) -> String {
    let mut lines = vec!["# Diff Panel".to_string()];
    if app.diff_content.trim().is_empty() {
        lines.push("Cached diff viewer: none".to_string());
        lines.push("Open /diff for checkpoint-backed file changes or git diff ranges.".to_string());
        return lines.join("\n");
    }

    lines.push(format!("Cached diff viewer: {}", app.diff_title));
    lines.push(format!("Lines: {}", app.diff_content.lines().count()));
    lines.push("Preview:".to_string());
    for line in app.diff_content.lines().take(16) {
        lines.push(format!("  {}", compact_panel_line(line, 140)));
    }
    if app.diff_content.lines().count() > 16 {
        lines.push("  ...".to_string());
    }
    lines.join("\n")
}

pub fn render_approval_panel(app: &TuiApp) -> String {
    let mut lines = vec![
        "# Approval Panel".to_string(),
        format!("Permission mode: {}", app.current_permission_label()),
    ];

    let Some(request) = app.pending_permission_request.as_ref() else {
        lines.push("Pending approval: none".to_string());
        return lines.join("\n");
    };

    let review = request.human_review_request();
    let permission = request.permission_review();
    lines.push(format!(
        "Pending approval: {} [{}]",
        review.title,
        review.risk.as_str()
    ));
    lines.push(format!(
        "Tool: {} ({})",
        request.tool_call.name, request.tool_call.id
    ));
    lines.push(format!(
        "Subject: {}",
        compact_panel_line(&review.subject, 180)
    ));
    lines.push(format!("Rule: {}", permission.rule_pattern));
    lines.push(format!(
        "Reason: {}",
        compact_panel_line(&review.reason, 220)
    ));

    let choices = permission
        .options
        .iter()
        .map(|option| format!("{}={}", option.key, option.label))
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(format!("Choices: {}", choices));

    if let Some((title, preview)) = app.compute_permission_diff() {
        lines.push("Preview:".to_string());
        lines.push(format!("  {}", title));
        for line in preview.lines().take(10) {
            lines.push(format!("  {}", compact_panel_line(line, 140)));
        }
        if preview.lines().count() > 10 {
            lines.push("  ...".to_string());
        }
    }

    lines.join("\n")
}

pub fn render_hooks_panel(app: &TuiApp) -> String {
    let snapshot = crate::engine::hooks::ToolHookManager::lifecycle_snapshot_from_env();
    let mut lines = vec![
        "# Hook Panel".to_string(),
        format!(
            "Configured: {}",
            if snapshot.configured { "yes" } else { "no" }
        ),
        format!(
            "Policy: timeout={}ms, {}",
            snapshot.default_timeout_ms,
            if snapshot.fail_closed {
                "fail-closed"
            } else {
                "fail-open"
            }
        ),
    ];

    if snapshot.registrations.is_empty() {
        lines.push("Registrations: none".to_string());
        lines.push(
            "Set PRIORITY_AGENT_PRE_TOOL_HOOK, PRIORITY_AGENT_POST_TOOL_HOOK, or tool-specific hook env vars."
                .to_string(),
        );
    } else {
        lines.push(format!(
            "Registrations: {} hook(s)",
            snapshot.registrations.len()
        ));
        for registration in snapshot.registrations.iter().take(16) {
            lines.push(format!(
                "- {} {} provider={} scope={} timeout={}ms block_on_error={} command={}",
                registration.event,
                registration.hook_name,
                registration.provider.as_str(),
                registration.scope,
                registration.timeout_ms,
                registration.block_on_error,
                compact_panel_line(&registration.command_preview, 120)
            ));
        }
    }

    let latest_trace = crate::tui::slash_handler::utils::latest_trace_for_app(app);
    let trace_records = latest_trace
        .as_ref()
        .map(hook_trace_lines)
        .unwrap_or_default();
    if trace_records.is_empty() {
        lines.push("Recent executions: none in latest trace".to_string());
    } else {
        let success_count = trace_records
            .iter()
            .filter(|record| record.contains(" ok "))
            .count();
        let failed_count = trace_records
            .iter()
            .filter(|record| record.contains(" failed "))
            .count();
        let blocked_count = trace_records
            .iter()
            .filter(|record| record.contains(" blocked"))
            .count();
        lines.push(format!(
            "Recent executions: ok={} failed={} blocked={}",
            success_count, failed_count, blocked_count
        ));
        for record in trace_records.into_iter().rev().take(8) {
            lines.push(record);
        }
    }

    lines.join("\n")
}

pub async fn render_context_panel(app: &TuiApp) -> String {
    let runtime = app.runtime_status_snapshot().await;
    let working_dir = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let session = app
        .session_manager
        .current_session_id()
        .map(abbreviate_id)
        .unwrap_or_else(|| "none".to_string());

    let mut lines = vec![
        "# Context Panel".to_string(),
        format!("Session: {}", session),
        format!(
            "Model: {} ({})",
            app.current_model_label(),
            app.current_provider_label()
        ),
        format!("Agent mode: {}", app.current_agent_mode_label()),
        format!("Working dir: {}", working_dir),
        format!(
            "Messages: {} visible, querying={}",
            runtime.messages, runtime.is_querying
        ),
        format!(
            "Tools: {} active / {} total, failed={}, backgrounded={}",
            runtime.active_tool_count,
            runtime.total_tools,
            runtime.failed_tool_count,
            runtime.backgrounded_tool_count
        ),
        format!(
            "Permission: {}{}",
            runtime.permission_mode,
            runtime
                .pending_permission
                .as_ref()
                .map(|pending| format!(" (pending: {})", pending))
                .unwrap_or_default()
        ),
    ];

    if let Some(error) = runtime.last_error.as_ref() {
        lines.push(format!("Last error: {}", compact_panel_line(error, 180)));
    }

    if let Some(engine) = app.streaming_engine.as_ref() {
        let usage = engine.context_usage_report().await;
        let usage_pct = if usage.max_context_tokens > 0 {
            usage.total_estimated_tokens.saturating_mul(100) / usage.max_context_tokens
        } else {
            0
        };
        lines.push(format!(
            "Estimated request tokens: {} / {} ({}%)",
            usage.total_estimated_tokens, usage.max_context_tokens, usage_pct
        ));
        lines.push(format!(
            "Budget: prompt={} history={} tool_schemas={} memory={}",
            usage.prompt.total_tokens,
            usage.history_tokens,
            usage.tool_schema_tokens,
            usage.memory_snapshot_tokens
        ));
        lines.push(format!(
            "Stable prefix: {}",
            usage.stable_prefix_fingerprint
        ));
    } else {
        lines.push("Context budget: engine unavailable".to_string());
    }

    lines.join("\n")
}

async fn render_tasks_panel(app: &TuiApp) -> String {
    let runtime = app.runtime_status_snapshot().await;
    let tasks = if let Some(manager) = app.streaming_engine.as_ref().and_then(|e| e.task_manager())
    {
        manager.list_tasks(None).await
    } else {
        app.tasks.clone()
    };

    let mut lines = vec![
        "# Task Panel".to_string(),
        task_summary_line("Tracked tasks", &tasks),
        format!(
            "Runtime tasks: {} total, {} running",
            runtime.task_count, runtime.running_task_count
        ),
        format!(
            "Terminal tasks: {} known, {} running, {} pty",
            runtime.terminal_task_count,
            runtime.running_terminal_task_count,
            runtime.pty_terminal_task_count
        ),
        format!("Backgrounded tools: {}", runtime.backgrounded_tool_count),
    ];

    if tasks.is_empty() {
        lines.push("Recent tasks: none".to_string());
    } else {
        lines.push("Recent tasks:".to_string());
        for task in tasks.iter().take(12) {
            lines.push(format!(
                "- {} [{}] {}",
                task.id,
                task_status_label(task.status),
                compact_panel_line(&task.name, 120)
            ));
        }
    }

    if app.runtime_state_snapshot.terminal_tasks.is_empty() {
        lines.push("Terminal handles: none".to_string());
    } else {
        lines.push("Terminal handles:".to_string());
        for task in app.runtime_state_snapshot.terminal_tasks.iter().take(8) {
            lines.push(format_terminal_task_line(task));
        }
    }

    let tools = app
        .runtime_state_snapshot
        .tool_uses
        .iter()
        .rev()
        .take(8)
        .map(|tool| {
            format!(
                "- {} [{}] {}",
                tool.id,
                tool.status.label(),
                compact_panel_line(&tool.summary, 120)
            )
        })
        .collect::<Vec<_>>();
    if tools.is_empty() {
        lines.push("Runtime tools: none".to_string());
    } else {
        lines.push("Recent runtime tools:".to_string());
        lines.extend(tools);
    }

    lines.join("\n")
}

async fn render_mcp_panel(app: &TuiApp) -> String {
    let runtime = app.runtime_status_snapshot().await;
    let mut lines = vec![
        "# MCP Panel".to_string(),
        format!(
            "Runtime MCP: {} servers, {} available",
            runtime.mcp_server_count, runtime.mcp_available_count
        ),
    ];
    if !runtime.mcp_repair_hints.is_empty() {
        lines.push(format!(
            "Repair hints: {}",
            runtime.mcp_repair_hints.join(", ")
        ));
    }

    let Some(engine) = app.streaming_engine.as_ref() else {
        lines.push("Manager: engine unavailable".to_string());
        return lines.join("\n");
    };
    let Some(manager) = engine.mcp_manager() else {
        lines.push("Manager: not configured".to_string());
        return lines.join("\n");
    };

    let diagnostics = manager.health_diagnostics();
    if diagnostics.is_empty() {
        lines.push("Servers: none configured".to_string());
        return lines.join("\n");
    }

    let available = manager.available_servers();
    lines.push(format!(
        "Available: {}",
        if available.is_empty() {
            "none".to_string()
        } else {
            available.join(", ")
        }
    ));
    lines.push("Servers:".to_string());
    for diag in diagnostics.iter().take(16) {
        lines.push(format!(
            "- {} [{}] health={:?} approved={} oauth={} token={} circuit={} repair={}",
            diag.name,
            diag.transport,
            diag.health,
            diag.approved,
            if diag.oauth_configured {
                "configured"
            } else {
                "none"
            },
            if diag.oauth_token_present {
                "present"
            } else {
                "missing"
            },
            diag.circuit_breaker,
            diag.repair_hint
        ));
    }

    lines.join("\n")
}

pub fn render_bridge_panel(app: &TuiApp) -> String {
    let bridge = crate::bridge::runtime_snapshot();
    let remote_env = crate::remote::RemoteEnvDetector::detect();
    let sessions = crate::remote::RemoteSessionManager::new().list_sessions();
    let remote_trigger_available = app
        .streaming_engine
        .as_ref()
        .map(|engine| engine.tool_registry().has("remote_trigger"))
        .unwrap_or(false);
    let remote_dev_available = app
        .streaming_engine
        .as_ref()
        .map(|engine| engine.tool_registry().has("remote_dev"))
        .unwrap_or(false);

    let mut lines = vec![
        "# Bridge Panel".to_string(),
        format!(
            "Bridge URL: {}",
            bridge
                .bridge_url
                .as_deref()
                .map(|url| compact_panel_line(url, 140))
                .unwrap_or_else(|| "not configured".to_string())
        ),
        format!(
            "Bridge source: {}",
            bridge.bridge_url_source.as_deref().unwrap_or("none")
        ),
        format!(
            "Auth token: {}{}",
            if bridge.auth_token_configured {
                "configured"
            } else {
                "not configured"
            },
            bridge
                .auth_token_source
                .as_deref()
                .map(|source| format!(" ({})", source))
                .unwrap_or_default()
        ),
        format!("Tenant: {}", bridge.tenant_id.as_deref().unwrap_or("none")),
        format!(
            "Replay cursors: {} at {}",
            bridge.cursor_count,
            bridge.cursor_path.display()
        ),
        format!(
            "Tools: remote_trigger={} remote_dev={}",
            remote_trigger_available, remote_dev_available
        ),
        format!(
            "Remote env: {} remote={} host={} user={} cwd={}",
            remote_env.env_type,
            remote_env.is_remote,
            empty_as_unknown(&remote_env.hostname),
            empty_as_unknown(&remote_env.username),
            remote_env.working_dir.display()
        ),
        format!("Saved SSH sessions: {}", sessions.len()),
    ];

    if !bridge.cursor_session_ids.is_empty() {
        lines.push(format!(
            "Cursor sessions: {}",
            bridge
                .cursor_session_ids
                .iter()
                .take(8)
                .map(|id| compact_panel_line(id, 48))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !remote_env.detected_env_vars.is_empty() {
        lines.push(format!(
            "Detected remote vars: {}",
            remote_env.detected_env_vars.join(", ")
        ));
    }
    if !sessions.is_empty() {
        lines.push("Sessions:".to_string());
        for session in sessions.iter().take(8) {
            lines.push(format!(
                "- {} [{}] {}@{}:{}",
                session.id,
                serde_json::to_string(&session.status)
                    .unwrap_or_else(|_| "unknown".to_string())
                    .trim_matches('"')
                    .to_string(),
                session.config.username,
                session.config.host,
                session.config.port
            ));
        }
    }

    lines.join("\n")
}

fn task_summary_line(label: &str, tasks: &[TaskItem]) -> String {
    let count_status =
        |status: TaskStatus| -> usize { tasks.iter().filter(|task| task.status == status).count() };
    format!(
        "{}: total={} pending={} running={} completed={} failed={} killed={}",
        label,
        tasks.len(),
        count_status(TaskStatus::Pending),
        count_status(TaskStatus::Running),
        count_status(TaskStatus::Completed),
        count_status(TaskStatus::Failed),
        count_status(TaskStatus::Killed)
    )
}

fn task_status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Killed => "killed",
    }
}

fn format_terminal_task_line(task: &RuntimeTerminalTask) -> String {
    let mut parts = vec![format!("- {} [{}]", task.id, task.status)];
    if let Some(kind) = task.terminal_kind.as_deref() {
        parts.push(format!("kind={}", kind));
    }
    if let Some(handle) = task.handle.as_deref() {
        parts.push(format!("handle={}", handle));
    }
    if let Some(command) = task.command.as_deref() {
        parts.push(format!("cmd={}", compact_panel_line(command, 96)));
    }
    parts.join(" ")
}

fn empty_as_unknown(value: &str) -> &str {
    if value.trim().is_empty() {
        "unknown"
    } else {
        value
    }
}

fn hook_trace_lines(trace: &crate::engine::trace::TurnTrace) -> Vec<String> {
    trace
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
                let detail = error
                    .as_deref()
                    .or(output_preview.as_deref())
                    .map(|detail| format!(": {}", compact_panel_line(detail, 120)))
                    .unwrap_or_default();
                Some(format!(
                    "- {} {} provider={} tool={} call={} {}{} duration={}ms{}",
                    event,
                    hook_name,
                    provider,
                    tool.as_deref().unwrap_or("lifecycle"),
                    call_id,
                    if *success { "ok" } else { "failed" },
                    if *blocked { " blocked" } else { "" },
                    duration_ms,
                    detail
                ))
            } else {
                None
            }
        })
        .collect()
}

fn compact_panel_line(text: &str, max_chars: usize) -> String {
    let one_line = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if one_line.chars().count() <= max_chars {
        return one_line;
    }
    let take = max_chars.saturating_sub(3);
    format!("{}...", one_line.chars().take(take).collect::<String>())
}

fn abbreviate_id(id: &str) -> String {
    id.chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{RuntimeTerminalTask, RuntimeToolStatus, RuntimeToolUse};

    #[test]
    fn parses_panel_kind_aliases() {
        assert_eq!(RuntimePanelKind::parse(""), Some(RuntimePanelKind::All));
        assert_eq!(
            RuntimePanelKind::parse("permissions"),
            Some(RuntimePanelKind::Approval)
        );
        assert_eq!(
            RuntimePanelKind::parse("hooks"),
            Some(RuntimePanelKind::Hooks)
        );
        assert_eq!(
            RuntimePanelKind::parse("tokens"),
            Some(RuntimePanelKind::Context)
        );
        assert_eq!(
            RuntimePanelKind::parse("servers"),
            Some(RuntimePanelKind::Mcp)
        );
        assert_eq!(
            RuntimePanelKind::parse("remote"),
            Some(RuntimePanelKind::Bridge)
        );
        assert_eq!(RuntimePanelKind::parse("unknown"), None);
    }

    #[test]
    fn renders_hook_panel_from_env_snapshot() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_PRE_TOOL_HOOK");
        env.remove("PRIORITY_AGENT_POST_TOOL_HOOK");
        for (key, _) in std::env::vars() {
            if key.starts_with("PRIORITY_AGENT_TOOL_HOOK_BEFORE_")
                || key.starts_with("PRIORITY_AGENT_TOOL_HOOK_AFTER_")
            {
                env.remove(&key);
            }
        }
        env.set("PRIORITY_AGENT_PRE_TOOL_HOOK", "echo ok");
        env.set("PRIORITY_AGENT_HOOK_TIMEOUT_MS", "1500");
        let app = TuiApp::new();

        let panel = render_hooks_panel(&app);

        assert!(panel.contains("# Hook Panel"));
        assert!(panel.contains("Configured: yes"));
        assert!(panel.contains("timeout=1500ms"));
        assert!(panel.contains("env_pre_tool_hook"));
    }

    #[tokio::test]
    async fn renders_pending_permission_approval_panel() {
        let mut app = TuiApp::new();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({ "command": "cargo check -q" }),
                },
                prompt: "Approve shell command?".to_string(),
                review: None,
            });

        let panel = render_runtime_panel(&app, RuntimePanelKind::Approval).await;

        assert!(panel.contains("# Approval Panel"));
        assert!(panel.contains("Pending approval: Tool approval"));
        assert!(panel.contains("Rule: bash"));
        assert!(panel.contains("Choices: y=allow once"));
        assert!(panel.contains("Preview:"));
    }

    #[tokio::test]
    async fn renders_task_panel_from_app_and_runtime_state() {
        let mut app = TuiApp::new();
        app.tasks.push(TaskItem {
            status: TaskStatus::Running,
            ..TaskItem::new("task_1", "run validation")
        });
        app.runtime_state_snapshot.tool_uses.push(RuntimeToolUse {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            summary: "Running cargo test".to_string(),
            status: RuntimeToolStatus::Running,
            active: true,
            arguments: None,
            latest_progress: None,
            result_preview: None,
            elapsed_ms: Some(1500),
        });
        app.runtime_state_snapshot
            .terminal_tasks
            .push(RuntimeTerminalTask {
                id: "shell_1".to_string(),
                status: "running".to_string(),
                terminal_kind: Some("background_shell".to_string()),
                command: Some("npm run dev".to_string()),
                handle: Some("shell_1".to_string()),
                output_path: None,
                read_tool: Some("bash_output".to_string()),
                cancel_handle: Some("shell_1".to_string()),
            });

        let panel = render_runtime_panel(&app, RuntimePanelKind::Tasks).await;

        assert!(panel.contains("# Task Panel"));
        assert!(panel.contains("Tracked tasks: total=1"));
        assert!(panel.contains("Terminal handles:"));
        assert!(panel.contains("tool_1 [running] Running cargo test"));
    }

    #[tokio::test]
    async fn renders_bridge_panel_without_engine() {
        let app = TuiApp::new();

        let panel = render_runtime_panel(&app, RuntimePanelKind::Bridge).await;

        assert!(panel.contains("# Bridge Panel"));
        assert!(panel.contains("Bridge URL:"));
        assert!(panel.contains("Remote env:"));
        assert!(panel.contains("Saved SSH sessions:"));
    }
}
