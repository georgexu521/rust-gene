use super::*;

fn exposure_report(
    model_exposed: bool,
    hidden_reason: Option<&str>,
) -> crate::engine::tool_exposure::ToolExposureReport {
    crate::engine::tool_exposure::ToolExposureReport {
        tool_name: "bash".to_string(),
        registered: true,
        available: true,
        availability_reason: None,
        permission_exposed: model_exposed,
        permission_reason: hidden_reason.map(str::to_string),
        route_scoped_tools: true,
        route_exposed: true,
        route_reason: None,
        provider_schema_compatible: true,
        provider_schema_reason: None,
        model_exposed,
        hidden_reason: hidden_reason.map(str::to_string),
    }
}

#[test]
fn bash_exposure_status_names_exposed_state() {
    let line = format_terminal_bash_exposure(&exposure_report(true, None));

    assert!(line.contains("exposed for terminal requests"));
    assert!(line.contains("route_scoped=on"));
    assert!(line.contains("schema=ok"));
}

#[test]
fn bash_exposure_status_names_hidden_reason() {
    let line = format_terminal_bash_exposure(&exposure_report(
        false,
        Some("permission mode is read_only"),
    ));

    assert!(line.contains("hidden for terminal requests"));
    assert!(line.contains("permission mode is read_only"));
}

#[test]
fn product_readiness_reports_ready_when_diagnostics_are_clean() {
    let report = crate::diagnostics::DiagnosticReport::new(vec![
        crate::diagnostics::CheckResult::ok("git", "available"),
        crate::diagnostics::CheckResult::ok("engine", "model=test"),
    ]);
    let runtime = crate::state::RuntimeStatusSnapshot::default();

    let readiness = evaluate_product_readiness(&report, &runtime);

    assert!(readiness.ready);
    assert_eq!(readiness.label, "READY");
    assert_eq!(
        readiness.to_check_result().status,
        crate::diagnostics::CheckStatus::Ok
    );
    assert!(readiness.format_text().contains("Status: READY"));
}

#[test]
fn product_readiness_surfaces_blockers_and_runtime_warnings() {
    let report = crate::diagnostics::DiagnosticReport::new(vec![
        crate::diagnostics::CheckResult::error("config", "No key", "Set a key"),
        crate::diagnostics::CheckResult::warn("network", "slow", "Check proxy"),
    ]);
    let runtime = crate::state::RuntimeStatusSnapshot {
        failed_tool_count: 2,
        backgrounded_tool_count: 1,
        pending_permission: Some("bash (call_1)".to_string()),
        mcp_repair_hints: vec!["filesystem: approve".to_string()],
        ..crate::state::RuntimeStatusSnapshot::default()
    };

    let readiness = evaluate_product_readiness(&report, &runtime);
    let text = readiness.format_text();

    assert!(!readiness.ready);
    assert_eq!(readiness.label, "BLOCKED");
    assert!(text.contains("config: No key"));
    assert!(text.contains("runtime tools failed=2"));
    assert!(text.contains("approval pending: bash (call_1)"));
    assert_eq!(
        readiness.to_check_result().status,
        crate::diagnostics::CheckStatus::Error
    );
}

#[test]
fn terminal_task_status_counts_empty_tasks() {
    let line = format_terminal_task_status_counts(&[]);

    assert_eq!(line, "Terminal tasks: none");
}

#[test]
fn terminal_task_status_counts_known_task_states() {
    let tasks = serde_json::json!([
        {"status": "running"},
        {"status": "completed"},
        {"status": "failed"},
        {"status": "cancelled"},
        {"status": "timed_out"},
        {"status": "running"}
    ]);
    let line = format_terminal_task_status_counts(tasks.as_array().unwrap());

    assert_eq!(
        line,
        "Terminal tasks: 6 known (2 running, 1 completed, 1 failed, 1 cancelled, 1 timed out)"
    );
}

#[test]
fn mode_switches_product_agent_mode_without_changing_ui_mode() {
    let mut app = TuiApp::new();

    let msg = handle_mode(&mut app, "build");

    assert!(msg.contains("Agent mode switched to build"));
    assert_eq!(app.agent_mode, AgentMode::Build);
    assert!(matches!(app.mode, AppMode::Chat | AppMode::Onboarding));
}

#[test]
fn mode_keeps_legacy_ui_mode_aliases() {
    let mut app = TuiApp::new();

    let msg = handle_mode(&mut app, "vim");

    assert!(msg.contains("UI mode switched to vim_normal"));
    assert_eq!(app.agent_mode, AgentMode::Auto);
    assert_eq!(app.mode, AppMode::VimNormal);
}

#[test]
fn doctor_route_summary_applies_agent_mode_before_exposure_checks() {
    let auto_route = route_for_agent_mode(TERMINAL_EXPOSURE_PROMPT, AgentMode::Auto);
    let plan_route = route_for_agent_mode(TERMINAL_EXPOSURE_PROMPT, AgentMode::Plan);
    let build_route = route_for_agent_mode(TERMINAL_EXPOSURE_PROMPT, AgentMode::Build);

    let auto_allowlist =
        crate::engine::conversation_loop::ConversationLoop::route_tool_allowlist(&auto_route);
    let plan_allowlist =
        crate::engine::conversation_loop::ConversationLoop::route_tool_allowlist(&plan_route);
    let build_allowlist =
        crate::engine::conversation_loop::ConversationLoop::route_tool_allowlist(&build_route);

    assert!(auto_allowlist.contains("bash"));
    assert!(!auto_allowlist.contains("file_edit"));
    assert!(!plan_allowlist.contains("bash"));
    assert!(!plan_allowlist.contains("file_edit"));
    assert!(build_allowlist.contains("bash"));
    assert!(build_allowlist.contains("file_edit"));
}

#[test]
fn doctor_route_summary_keeps_bash_with_learning_feedback() {
    let events = vec![
        crate::session_store::LearningEventRecord {
            id: 1,
            session_id: "s1".to_string(),
            kind: "tool_outcome".to_string(),
            source: "test".to_string(),
            summary: "bash failed".to_string(),
            confidence: 1.0,
            payload: serde_json::json!({"tool": "bash", "success": false}),
            created_at: "now".to_string(),
        },
        crate::session_store::LearningEventRecord {
            id: 2,
            session_id: "s1".to_string(),
            kind: "tool_outcome".to_string(),
            source: "test".to_string(),
            summary: "bash failed".to_string(),
            confidence: 1.0,
            payload: serde_json::json!({"tool": "bash", "success": false}),
            created_at: "now".to_string(),
        },
    ];

    let route =
        route_for_agent_mode_with_learning(TERMINAL_EXPOSURE_PROMPT, AgentMode::Auto, &events);
    let allowlist =
        crate::engine::conversation_loop::ConversationLoop::route_tool_allowlist(&route);

    assert!(route.reason.contains("recent failure"));
    assert!(allowlist.contains("bash"));
}

#[test]
fn doctor_prompt_cache_line_reports_tool_schema_miss_reason() {
    let mut tracker = crate::cost_tracker::CostTracker::new();
    tracker.record_api_call_with_cache_shape(
        "kimi-k2.5",
        1000,
        100,
        Some(0),
        Some(
            crate::engine::cache_stability::request_cache_diagnostic_shape(
                &[
                    crate::services::api::Message::system("stable"),
                    crate::services::api::Message::user("one"),
                ],
                &[crate::services::api::Tool {
                    name: "alpha".to_string(),
                    description: "Alpha tool".to_string(),
                    parameters: serde_json::json!({"type":"object","properties":{}}),
                    strict_schema: false,
                }],
            ),
        ),
    );
    tracker.record_api_call_with_cache_shape(
        "kimi-k2.5",
        1000,
        100,
        Some(500),
        Some(
            crate::engine::cache_stability::request_cache_diagnostic_shape(
                &[
                    crate::services::api::Message::system("stable"),
                    crate::services::api::Message::user("two"),
                ],
                &[
                    crate::services::api::Tool {
                        name: "alpha".to_string(),
                        description: "Alpha tool".to_string(),
                        parameters: serde_json::json!({"type":"object","properties":{}}),
                        strict_schema: false,
                    },
                    crate::services::api::Tool {
                        name: "beta".to_string(),
                        description: "Beta tool".to_string(),
                        parameters: serde_json::json!({"type":"object","properties":{}}),
                        strict_schema: false,
                    },
                ],
            ),
        ),
    );

    let line = format_prompt_cache_doctor_line(&tracker);

    assert!(line.contains("last_reason=tool-list-changed"));
    assert!(line.contains("tool_fp="));
    assert!(line.contains("before_last_user=0"));
}

#[test]
fn agent_task_state_lines_include_runtime_details() {
    let states = vec![crate::session_store::AgentTaskStateRecord {
        id: 1,
        session_id: "s1".to_string(),
        task_id: "task_1".to_string(),
        agent_id: "agent_1".to_string(),
        profile: Some("implementer".to_string()),
        role: "specialist".to_string(),
        status: "completed".to_string(),
        description: "edit code".to_string(),
        transcript_path: None,
        tool_ids_in_progress: vec!["tool_1".to_string()],
        permission_requests: vec!["file_write".to_string()],
        result_artifact_id: Some(9),
        cleanup_hooks: vec!["worktree_cleanup".to_string()],
        payload: serde_json::json!({
            "isolated_worktree": {
                "path": "/tmp/agent-worktree",
                "branch": "codex/agent-1234"
            },
            "fork_context": {
                "message_count": 3,
                "placeholder_complete": true
            }
        }),
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
    }];

    let rendered = format_agent_task_state_lines(&states).join("\n");

    assert!(rendered.contains("tools=1"));
    assert!(rendered.contains("permissions=1"));
    assert!(rendered.contains("cleanup=worktree_cleanup"));
    assert!(rendered.contains("worktree: /tmp/agent-worktree (codex/agent-1234)"));
    assert!(rendered.contains("fork_context: messages=3 placeholder_complete=true"));
}

#[test]
fn mcp_repair_plan_separates_explicit_and_auto_safe_repairs() {
    let diagnostics = vec![
        crate::engine::mcp::McpServerHealth {
            name: "filesystem".to_string(),
            transport: "stdio".to_string(),
            health: crate::engine::mcp::McpHealthStatus::Pending,
            circuit_breaker: "CLOSED".to_string(),
            approved: false,
            oauth_configured: false,
            oauth_token_present: false,
            repair_hint: "/mcp approve filesystem".to_string(),
        },
        crate::engine::mcp::McpServerHealth {
            name: "github".to_string(),
            transport: "http".to_string(),
            health: crate::engine::mcp::McpHealthStatus::Healthy,
            circuit_breaker: "CLOSED".to_string(),
            approved: true,
            oauth_configured: true,
            oauth_token_present: false,
            repair_hint: "/mcp auth github".to_string(),
        },
        crate::engine::mcp::McpServerHealth {
            name: "jira".to_string(),
            transport: "websocket".to_string(),
            health: crate::engine::mcp::McpHealthStatus::Degraded,
            circuit_breaker: "HALF-OPEN".to_string(),
            approved: true,
            oauth_configured: false,
            oauth_token_present: false,
            repair_hint: "/mcp repair jira".to_string(),
        },
    ];

    let plan = format_mcp_repair_plan(&diagnostics);

    assert!(plan.contains("approval repair: /mcp approve filesystem"));
    assert!(plan.contains("auth repair: /mcp auth github"));
    assert!(plan.contains("circuit repair: /mcp repair jira"));
    assert_eq!(mcp_circuit_repair_targets(&diagnostics), vec!["jira"]);
}

#[test]
fn handle_telemetry_includes_status_fields() {
    let output = super::handle_telemetry();
    assert!(output.contains("Telemetry Status"));
    assert!(output.contains("Consent:"));
    assert!(output.contains("Enabled:"));
    assert!(output.contains("Recorded sessions:"));
}
