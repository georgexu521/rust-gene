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
    Agents,
    Mcp,
    Bridge,
    Trace,
    Skills,
    Lab,
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
            "task" | "tasks" => Some(Self::Tasks),
            "agent" | "agents" | "agent-panel" | "subagent" | "subagents" => Some(Self::Agents),
            "mcp" | "servers" => Some(Self::Mcp),
            "bridge" | "remote" | "remotes" => Some(Self::Bridge),
            "trace" | "traces" | "replay" => Some(Self::Trace),
            "skill" | "skills" => Some(Self::Skills),
            "lab" | "labrun" | "lab-mode" => Some(Self::Lab),
            _ => None,
        }
    }

    pub const fn usage() -> &'static str {
        "Usage: /panel [all|diff|approval|hooks|context|tasks|agents|mcp|bridge|trace|skills|lab]"
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
                render_agent_panel(app).await,
                render_mcp_panel(app).await,
                render_bridge_panel(app),
                render_trace_panel(app),
                render_diff_panel(app),
                render_skills_panel(app),
                render_lab_panel(app),
            ];
            sections.retain(|section| !section.trim().is_empty());
            sections.join("\n\n")
        }
        RuntimePanelKind::Diff => render_diff_panel(app),
        RuntimePanelKind::Approval => render_approval_panel(app),
        RuntimePanelKind::Hooks => render_hooks_panel(app),
        RuntimePanelKind::Context => render_context_panel(app).await,
        RuntimePanelKind::Tasks => render_tasks_panel(app).await,
        RuntimePanelKind::Agents => render_agent_panel(app).await,
        RuntimePanelKind::Mcp => render_mcp_panel(app).await,
        RuntimePanelKind::Bridge => render_bridge_panel(app),
        RuntimePanelKind::Trace => render_trace_panel(app),
        RuntimePanelKind::Skills => render_skills_panel(app),
        RuntimePanelKind::Lab => render_lab_panel(app),
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
        format!("Mode: {}", app.current_permission_label()),
    ];

    let Some(request) = app.pending_permission_request.as_ref() else {
        lines.push("Pending approval: none".to_string());
        return lines.join("\n");
    };

    let review = request.human_review_request();
    let permission = request.permission_review();

    // Zone 1: Tool Semantic
    lines.push("\n═══ Tool ═══".to_string());
    lines.push(format!(
        "Name: {} | ID: {}",
        request.tool_call.name, request.tool_call.id
    ));
    lines.push(format!("Description: {}", review.title));

    // Zone 2: Risk & Rules
    lines.push("\n═══ Risk & Rules ═══".to_string());
    lines.push(format!(
        "Risk: {} | Subject: {}",
        review.risk.as_str(),
        compact_panel_line(&review.subject, 140)
    ));
    lines.push(format!(
        "Reason: {}",
        compact_panel_line(&review.reason, 200)
    ));
    lines.push(format!("Match pattern: {}", permission.rule_pattern));
    if let Some(audit) = request.audit.as_ref() {
        if !audit.risk_facts.is_empty() {
            lines.push(format!(
                "Risk facts: {}",
                compact_panel_line(&audit.risk_facts.join(", "), 200)
            ));
        }
        if !audit.matched_rules.is_empty() {
            lines.push(format!(
                "Matched rules: {}",
                compact_panel_line(&audit.matched_rules.join(", "), 160)
            ));
        }
        if let Some(hint) = audit.recovery_hint.as_deref() {
            lines.push(format!("Recovery: {}", compact_panel_line(hint, 200)));
        }
    }

    // Zone 3: Diff / Command Preview
    lines.push("\n═══ Preview ═══".to_string());
    if let Some((title, preview)) = app.compute_permission_diff() {
        lines.push(format!("  {}", title));
        for preview_line in preview.lines().take(30) {
            lines.push(format!("  {}", preview_line));
        }
    } else {
        lines.push(format!("  {}", compact_panel_line(&review.subject, 160)));
    }

    // Zone 4: Actions
    lines.push("\n═══ Actions ═══".to_string());
    let choices = permission
        .options
        .iter()
        .map(|option| format!("{}={}", option.key, option.label))
        .collect::<Vec<_>>()
        .join(" | ");
    lines.push(choices);
    lines.push(
        "  1=approve once · 2=approve session · 3=approve project · 4=deny · esc=cancel"
            .to_string(),
    );

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
            "Projection: seq={} event={}",
            app.sync_snapshot.last_projection_seq,
            app.sync_snapshot
                .last_projection_event_id
                .as_deref()
                .map(|id| compact_panel_line(id, 80))
                .unwrap_or_else(|| "none".to_string())
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
            "Budget: prompt={} history={} tool_schemas={} pinned_memory={}",
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

async fn render_agent_panel(app: &TuiApp) -> String {
    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let definitions = crate::agent::profiles::load_definitions(&working_dir);
    let mut lines = vec![
        "# Agent Panel".to_string(),
        format!("Definitions: {}", definitions.len()),
    ];
    if !definitions.is_empty() {
        lines.push(format!(
            "Profiles: {}",
            definitions
                .iter()
                .take(8)
                .map(|definition| compact_panel_line(&definition.summary_line(), 120))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if let Some(manager) = app
        .streaming_engine
        .as_ref()
        .and_then(|engine| engine.agent_manager())
    {
        let agents = manager.list_agents().await;
        lines.push(format!("Running agents: {}", agents.len()));
        for handle in agents.iter().take(8) {
            let status = *handle.status.borrow();
            lines.push(format!(
                "- {} [{:?}] role={} {}",
                handle.id,
                status,
                handle.config.role.display_name(),
                compact_panel_line(&handle.config.name, 96)
            ));
        }
    } else {
        lines.push("Running agents: manager unavailable".to_string());
    }

    match app.session_manager.recent_agent_task_states(8) {
        Ok(states) if states.is_empty() => {
            lines.push("Durable task states: none for current session".to_string());
        }
        Ok(states) => {
            lines.push(format!("Durable task states: {}", states.len()));
            for state in states.iter().take(8) {
                let recovery = state
                    .payload
                    .get("recovery_status")
                    .and_then(|value| value.as_str())
                    .map(|status| format!(" recovery={status}"))
                    .unwrap_or_default();
                let child_session = state
                    .payload
                    .get("child_session_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("none");
                lines.push(format!(
                    "- task={} agent={} [{}] artifact={} child={} profile={} role={} tools={} permissions={}{} {}",
                    state.task_id,
                    state.agent_id,
                    state.status,
                    state
                        .result_artifact_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    child_session,
                    state.profile.as_deref().unwrap_or("none"),
                    state.role,
                    state.tool_ids_in_progress.len(),
                    state.permission_requests.len(),
                    recovery,
                    compact_panel_line(&state.description, 100)
                ));
            }
        }
        Err(error) => lines.push(format!("Durable task states: unavailable ({})", error)),
    }

    match app.session_manager.recent_agent_artifacts(8) {
        Ok(artifacts) if artifacts.is_empty() => {
            lines.push("Recent artifacts: none for current session".to_string());
        }
        Ok(artifacts) => {
            lines.push(format!("Recent artifacts: {}", artifacts.len()));
            for artifact in artifacts.iter().take(8) {
                let preview = artifact.output.lines().next().unwrap_or("");
                let detail = if preview.trim().is_empty() {
                    artifact.description.as_str()
                } else {
                    preview
                };
                let task_id = artifact
                    .payload
                    .get("task_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("none");
                let sink = artifact
                    .payload
                    .get("completion_sink")
                    .and_then(|value| value.as_str())
                    .unwrap_or("legacy");
                lines.push(format!(
                    "- artifact={} task={} agent={} [{}] sink={} profile={} role={} {}",
                    artifact.id,
                    task_id,
                    artifact.agent_id,
                    artifact.status,
                    sink,
                    artifact.profile.as_deref().unwrap_or("none"),
                    artifact.role,
                    compact_panel_line(detail, 100)
                ));
            }
        }
        Err(error) => lines.push(format!("Recent artifacts: unavailable ({})", error)),
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
                    .trim_matches('"'),
                session.config.username,
                session.config.host,
                session.config.port
            ));
        }
    }

    lines.join("\n")
}

fn render_trace_panel(app: &TuiApp) -> String {
    let mut lines = vec!["# Trace Panel".to_string()];
    let mut traces = Vec::new();
    if let Some(engine) = app.streaming_engine.as_ref() {
        lines.push(format!("In-memory traces: {}", engine.trace_store().len()));
        if let Some(trace) = engine.trace_store().latest() {
            lines.push(format!(
                "Latest: {}",
                crate::engine::trace::format_trace_recent_line(&trace)
            ));
        }
        traces.extend(engine.trace_store().recent(5));
    } else {
        lines.push("In-memory traces: engine unavailable".to_string());
    }

    match app.session_manager.recent_traces(5) {
        Ok(persisted) => traces.extend(persisted),
        Err(error) => lines.push(format!("Persisted traces: unavailable ({})", error)),
    }
    traces = dedupe_trace_panel_traces(traces, 5);

    if traces.is_empty() {
        lines.push("Recent traces: none recorded".to_string());
    } else {
        lines.push("Recent traces:".to_string());
        for trace in traces {
            lines.push(format!(
                "- {}",
                crate::engine::trace::format_trace_recent_line(&trace)
            ));
        }
    }
    lines.push(
        "Replay: use /eval matrix or /eval parity for deterministic replay status".to_string(),
    );
    lines.join("\n")
}

fn dedupe_trace_panel_traces(
    traces: Vec<crate::engine::trace::TurnTrace>,
    limit: usize,
) -> Vec<crate::engine::trace::TurnTrace> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for trace in traces {
        if seen.insert(trace.trace_id.clone()) {
            deduped.push(trace);
        }
    }
    deduped.sort_by(|left, right| {
        right
            .turn_index
            .cmp(&left.turn_index)
            .then_with(|| right.started_at.cmp(&left.started_at))
    });
    deduped.truncate(limit);
    deduped
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

fn render_skills_panel(app: &TuiApp) -> String {
    let mut lines = vec!["# Skills Panel".to_string()];
    let skills: Vec<_> = app
        .skill_runtime
        .list()
        .into_iter()
        .filter(|s| s.meta.panel.is_some())
        .collect();
    if skills.is_empty() {
        lines.push("No skills declare a panel.".to_string());
        lines.push(
            "Add `panel = \"name\"` to a skill's frontmatter to contribute here.".to_string(),
        );
        return lines.join("\n");
    }

    for skill in skills {
        let panel = skill.meta.panel.as_deref().unwrap_or("");
        lines.push(format!("\n## {} (panel: {})", skill.meta.name, panel));
        let content = skill.content.trim();
        for line in content.lines().take(24) {
            lines.push(format!("  {}", compact_panel_line(line, 140)));
        }
        if content.lines().count() > 24 {
            lines.push("  ...".to_string());
        }
    }

    lines.join("\n")
}

fn render_lab_panel(app: &TuiApp) -> String {
    let project_root = &app.workspace.root;
    let store = crate::lab::store::LabStore::for_project(project_root);
    let mut lines = vec![
        "# Lab Panel".to_string(),
        format!(
            "Workspace: {}",
            compact_panel_line(&project_root.display().to_string(), 140)
        ),
    ];

    let run = match store.latest_run() {
        Ok(Some(run)) => run,
        Ok(None) => {
            match store.latest_proposal() {
                Ok(Some(proposal)) => {
                    lines.push(format!(
                        "Proposal: {} status={:?}",
                        proposal.proposal_id, proposal.status
                    ));
                    lines.push(format!(
                        "Goal: {}",
                        compact_panel_line(&proposal.user_goal, 180)
                    ));
                    lines.push("Actions:".to_string());
                    lines.push(format!("  /lab approve {}", proposal.proposal_id));
                    lines.push("  /lab propose <goal>".to_string());
                }
                Ok(None) => {
                    lines.push("No LabRun or proposal found.".to_string());
                    lines.push("Actions:".to_string());
                    lines.push("  /lab propose <goal>".to_string());
                    lines.push("  /lab start <goal>".to_string());
                }
                Err(err) => {
                    lines.push(format!("Failed to read Lab proposal: {err}"));
                }
            }
            return lines.join("\n");
        }
        Err(err) => {
            lines.push(format!("Failed to read LabRun: {err}"));
            return lines.join("\n");
        }
    };

    let tasks = match store.list_graduate_tasks(&run.lab_run_id) {
        Ok(tasks) => tasks,
        Err(err) => {
            lines.push(format!("Failed to read Lab tasks: {err}"));
            Vec::new()
        }
    };
    let open_tasks = tasks.iter().filter(|task| task.status.is_open()).count();
    let blocked_tasks = tasks
        .iter()
        .filter(|task| matches!(task.status, crate::lab::model::LabTaskStatus::Blocked))
        .count();
    let retries = match store.list_validation_retries(&run.lab_run_id) {
        Ok(retries) => retries,
        Err(err) => {
            lines.push(format!("Failed to read validation retries: {err}"));
            Vec::new()
        }
    };
    let escalated_retries = retries.iter().filter(|retry| retry.escalated).count();
    let cost = store.cost_summary(&run.lab_run_id).ok();
    let meeting = crate::lab::orchestrator::LabOrchestrator::for_project(project_root)
        .meeting_recommendation_for_latest()
        .ok();
    let recommended_meeting_topic = meeting
        .as_ref()
        .filter(|meeting| meeting.recommended)
        .map(|meeting| meeting.topic.clone());
    let latest_report = store
        .list_stage_artifact_report_paths(&run.lab_run_id)
        .ok()
        .and_then(|reports| reports.last().map(|(_, path)| path.display().to_string()));

    lines.push(format!("LabRun: {}", run.lab_run_id));
    lines.push(format!(
        "Run: status={:?} stage={} owner={:?} needs_user={}",
        run.status, run.current_stage, run.internal_owner, run.needs_user
    ));
    lines.push(format!(
        "Progress: cycles={} failures={} artifacts={} meetings={}",
        run.cycle_count,
        run.failure_count,
        run.artifact_ids.len(),
        run.meeting_ids.len()
    ));
    lines.push(format!(
        "Tasks: total={} open={} blocked={}",
        tasks.len(),
        open_tasks,
        blocked_tasks
    ));
    lines.push(format!(
        "Validation retries: total={} escalated={}",
        retries.len(),
        escalated_retries
    ));
    if let Some(cost) = cost {
        lines.push(format!(
            "Cost: requests={} total_tokens={} cache_hit_rate={:.1}% estimated_cost_usd={:.6}",
            cost.requests,
            cost.total_tokens,
            cost.cache_hit_rate_percent(),
            cost.estimated_cost_usd
        ));
    }
    if let Some(meeting) = meeting {
        lines.push(format!(
            "Meeting recommendation: recommended={} topic={} reason={}",
            meeting.recommended,
            compact_panel_line(&meeting.topic, 96),
            compact_panel_line(&meeting.reason, 120)
        ));
    }
    if let Some(report) = latest_report {
        lines.push(format!(
            "Latest report: {}",
            compact_panel_line(&report, 140)
        ));
    }

    let mut blockers = tasks
        .iter()
        .filter_map(|task| {
            task.blocker.as_ref().map(|blocker| {
                format!(
                    "{}: {}",
                    compact_panel_line(&task.title, 80),
                    compact_panel_line(blocker, 120)
                )
            })
        })
        .collect::<Vec<_>>();
    if let Some(reason) = run.blocked_reason.as_ref() {
        blockers.push(format!("Run: {}", compact_panel_line(reason, 120)));
    }
    if blockers.is_empty() {
        lines.push("Blockers: none".to_string());
    } else {
        lines.push("Blockers:".to_string());
        for blocker in blockers.into_iter().take(5) {
            lines.push(format!("  {blocker}"));
        }
    }
    if let Some(retry) = retries.last() {
        lines.push(format!(
            "Latest retry: task={} attempt={} escalated={} summary={}",
            retry.task_id,
            retry.attempt,
            retry.escalated,
            compact_panel_line(&retry.validation_summary, 140)
        ));
    }

    lines.push("Actions:".to_string());
    lines.push("  /lab dashboard".to_string());
    if let Some(topic) = recommended_meeting_topic {
        lines.push(format!(
            "  recommended: /lab meeting open {}",
            compact_panel_line(&topic, 120)
        ));
    }
    lines.push("  /lab meeting open [topic]".to_string());
    lines.push("  /lab intervene <message>".to_string());
    lines.push("  /lab continue <note>".to_string());
    lines.push("  /lab closeout auto".to_string());
    lines.join("\n")
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
            RuntimePanelKind::parse("agents"),
            Some(RuntimePanelKind::Agents)
        );
        assert_eq!(
            RuntimePanelKind::parse("remote"),
            Some(RuntimePanelKind::Bridge)
        );
        assert_eq!(
            RuntimePanelKind::parse("trace"),
            Some(RuntimePanelKind::Trace)
        );
        assert_eq!(RuntimePanelKind::parse("lab"), Some(RuntimePanelKind::Lab));
        assert_eq!(RuntimePanelKind::parse("unknown"), None);
    }

    #[tokio::test]
    async fn renders_context_panel_projection_cursor() {
        let app = TuiApp::new();

        let panel = render_runtime_panel(&app, RuntimePanelKind::Context).await;

        assert!(panel.contains("# Context Panel"));
        assert!(panel.contains("Projection: seq=0 event=none"));
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
                audit: Some(crate::engine::human_review::HumanReviewAuditRecord {
                    kind: crate::engine::human_review::HumanReviewKind::ToolPermission,
                    title: "Tool approval".to_string(),
                    risk: crate::engine::human_review::HumanReviewRisk::Medium,
                    subject: "bash: cargo check -q".to_string(),
                    reason: "Approve shell command?".to_string(),
                    tool_call_id: Some("call_1".to_string()),
                    tool_name: Some("bash".to_string()),
                    input_summary: "bash: cargo check -q".to_string(),
                    risk_facts: vec!["family:shell".to_string()],
                    matched_rules: vec!["bash:cargo check*".to_string()],
                    classifier_result: None,
                    hook_decision: None,
                    user_decision: None,
                    persistence_scope: Some("this_call".to_string()),
                    saved_config_path: None,
                    recovery_hint: Some("Ask the user before retrying.".to_string()),
                }),
                diff_preview: None,
            });

        let panel = render_runtime_panel(&app, RuntimePanelKind::Approval).await;

        assert!(panel.contains("# Approval Panel"));
        assert!(panel.contains("Name: bash"));
        assert!(panel.contains("Match pattern: bash"));
        assert!(panel.contains("Risk facts: family:shell"));
        assert!(panel.contains("Matched rules: bash:cargo check*"));
        assert!(panel.contains("Recovery: Ask the user"));
        assert!(panel.contains("y=allow once"));
        assert!(panel.contains("═══ Preview ═══"));
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
            operation_kind: Some("shell".to_string()),
            ui_render_kind: Some("shell".to_string()),
            read_only: Some(false),
            concurrency_safe: Some(false),
            destructive: Some(false),
            input_paths: Vec::new(),
            transcript_summary: Some("cargo test".to_string()),
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

    #[tokio::test]
    async fn renders_agent_panel_without_engine() {
        let app = TuiApp::new();

        let panel = render_runtime_panel(&app, RuntimePanelKind::Agents).await;

        assert!(panel.contains("# Agent Panel"));
        assert!(panel.contains("Definitions:"));
        assert!(panel.contains("Running agents: manager unavailable"));
        assert!(panel.contains("Durable task states:"));
    }

    #[tokio::test]
    async fn renders_agent_panel_with_durable_subagent_task_and_artifact_details() {
        let mut app = TuiApp::new();
        let store = std::sync::Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        app.session_manager = crate::tui::session_manager::TuiSessionManager::from_store(
            store,
            "panel-session",
            "Panel Session",
            "mock-model",
            None,
        )
        .unwrap();
        let store = app.session_manager.store();
        let session_id = app
            .session_manager
            .current_session_id()
            .expect("current session")
            .to_string();
        let artifact_id = store
            .add_agent_artifact(
                &session_id,
                "agent_1",
                Some("implementer"),
                "Specialist",
                "completed",
                "edit code",
                "completed result preview",
                &serde_json::json!({
                    "task_id": "task_1",
                    "completion_sink": "agent_manager"
                }),
            )
            .unwrap();
        store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id,
                task_id: "task_1".to_string(),
                agent_id: "agent_1".to_string(),
                profile: Some("implementer".to_string()),
                role: "Specialist".to_string(),
                status: "paused_restart".to_string(),
                description: "edit code".to_string(),
                transcript_path: Some("/tmp/a2a.jsonl".to_string()),
                tool_ids_in_progress: Vec::new(),
                permission_requests: vec!["file_write".to_string()],
                result_artifact_id: Some(artifact_id),
                cleanup_hooks: vec!["cleanup".to_string()],
                payload: serde_json::json!({
                    "child_session_id": "parent:subagent:task_1",
                    "recovery_status": "paused_restart"
                }),
            })
            .unwrap();

        let panel = render_runtime_panel(&app, RuntimePanelKind::Agents).await;

        assert!(panel.contains("task=task_1"));
        assert!(panel.contains("artifact=1"));
        assert!(panel.contains("child=parent:subagent:task_1"));
        assert!(panel.contains("recovery=paused_restart"));
        assert!(panel.contains("sink=agent_manager"));
        assert!(panel.contains("completed result preview"));
    }

    #[tokio::test]
    async fn renders_skills_panel_without_engine() {
        let app = TuiApp::new();

        let panel = render_runtime_panel(&app, RuntimePanelKind::Skills).await;

        assert!(panel.contains("# Skills Panel"));
        assert!(panel.contains("No skills declare a panel"));
    }

    #[tokio::test]
    async fn renders_lab_panel_from_workspace_labrun_state() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = TuiApp::new();
        app.workspace = crate::workspace::Workspace::detect(temp.path());

        let store = crate::lab::store::LabStore::for_project(temp.path());
        let proposal = store
            .create_proposal("Build the TUI Lab panel", Some("session".to_string()))
            .unwrap();
        let run = store.approve_proposal(&proposal.proposal_id).unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Render Lab panel",
                "Render a TUI Lab status panel from file-backed LabRun state.",
                vec!["src/tui/runtime_panels.rs".to_string()],
                vec!["cargo test -q runtime_panels".to_string()],
            )
            .unwrap();
        store
            .record_validation_retry_and_repair_task(
                &run.lab_run_id,
                &task.task_id,
                "TUI Lab panel snapshot was missing retry state",
            )
            .unwrap();

        let panel = render_runtime_panel(&app, RuntimePanelKind::Lab).await;

        assert!(panel.contains("# Lab Panel"));
        assert!(panel.contains("LabRun:"));
        assert!(panel.contains("Run: status=Active"));
        assert!(panel.contains("Tasks: total=2 open=2 blocked=1"));
        assert!(panel.contains("Validation retries: total=1 escalated=0"));
        assert!(panel.contains("Blockers:"));
        assert!(panel.contains("TUI Lab panel snapshot was missing retry state"));
        assert!(panel.contains("Latest retry:"));
        assert!(panel.contains("recommended: /lab meeting open"));
        assert!(panel.contains("/lab intervene <message>"));
        assert!(panel.contains("/lab closeout auto"));
    }
}
