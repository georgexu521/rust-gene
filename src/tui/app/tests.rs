use super::*;
use crate::engine::human_review::PermissionReviewDecision;
use crate::engine::runtime_facade::{ProviderPhase, ProviderRequestLifecycle};
use crate::services::api::{
    ChatRequest as LlmChatRequest, ChatResponse as LlmChatResponse, LlmProvider, Usage,
};
use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;
use ratatui::{backend::TestBackend, Terminal};

struct MockProvider;

#[async_trait]
impl LlmProvider for MockProvider {
    async fn chat(&self, _request: LlmChatRequest) -> anyhow::Result<LlmChatResponse> {
        Ok(LlmChatResponse {
            content: "ok".to_string(),
            tool_calls: None,
            usage: Some(Usage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
                reasoning_tokens: None,
                cached_tokens: None,
            }),
            tool_call_repair: None,
        })
    }

    async fn chat_stream(
        &self,
        _request: LlmChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        Err(anyhow::anyhow!("not implemented in TUI test"))
    }

    fn base_url(&self) -> &str {
        "https://api.openai.com/v1"
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }
}

fn render_command_palette_text(app: &TuiApp) -> String {
    let backend = TestBackend::new(160, 70);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            crate::tui::screens::main_screen::render_command_palette(frame, app, frame.area());
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
}

#[test]
fn test_cli_app_new() {
    let app = TuiApp::new();
    assert_eq!(app.messages.len(), 1); // 欢迎消息
    assert!(!app.is_querying);
    assert!(!app.paused);
    assert!(!app.focus_mode);
}

#[test]
fn test_memory_snapshot_panel_includes_skip_reasons() {
    let snapshot = crate::memory::MemorySnapshotReport {
        frozen: true,
        snapshot_id: "memsnap-test".to_string(),
        fingerprint: "abc123".to_string(),
        scope: "project".to_string(),
        char_count: 120,
        project_chars: 80,
        user_chars: 40,
        memory_file_count: 2,
        memory_file_chars: 64,
        pinned_sources: vec!["MEMORY.md".to_string(), "memory/design.md".to_string()],
        skipped_record_count: 4,
        skipped_status_count: 1,
        skipped_unsafe_count: 1,
        skipped_stale_count: 1,
        skipped_conflict_count: 1,
    };

    let formatted = format_memory_snapshot_report(&snapshot);

    assert!(formatted.contains("Skipped records: 4"));
    assert!(formatted.contains("Pinned sources: MEMORY.md, memory/design.md"));
    assert!(formatted.contains("status=1"));
    assert!(formatted.contains("unsafe=1"));
    assert!(formatted.contains("stale=1"));
    assert!(formatted.contains("conflicts=1"));
}

#[test]
fn test_tui_reuses_engine_session_binding() {
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session("engine-session", "Engine Session", "mock-model")
        .unwrap();
    let engine = Arc::new(
        crate::engine::streaming::StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(crate::tools::ToolRegistry::new()),
            "mock-model",
        )
        .with_session_store(store, "engine-session".to_string()),
    );

    let app = TuiApp::with_engine(engine, None, None);

    assert_eq!(
        app.session_manager.current_session_id(),
        Some("engine-session")
    );
    assert!(!app.should_persist_messages_from_tui());
}

#[test]
fn test_tui_persists_when_engine_has_no_session_binding() {
    let engine = Arc::new(
        crate::engine::streaming::StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(crate::tools::ToolRegistry::new()),
            "mock-model",
        )
        .with_disable_session_auto_init(),
    );

    let app = TuiApp::with_engine(engine, None, None);

    assert!(app.should_persist_messages_from_tui());
}

#[tokio::test]
async fn test_tui_persists_streaming_assistant_when_engine_has_no_session_binding() {
    let engine = Arc::new(
        crate::engine::streaming::StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(crate::tools::ToolRegistry::new()),
            "mock-model",
        )
        .with_disable_session_auto_init(),
    );
    let mut app = TuiApp::with_engine(engine, None, None);
    let session_id = app
        .session_manager
        .current_session_id()
        .unwrap()
        .to_string();
    app.messages.push(MessageItem {
        id: "assistant-placeholder".to_string(),
        role: MessageRole::Assistant,
        content: String::new(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    });
    {
        let mut response = app.current_response.lock().await;
        *response = "final answer".to_string();
    }
    app.is_querying = true;
    app.stream_done.store(true, Ordering::SeqCst);

    app.on_tick().await;

    let messages = app.session_manager.load_messages(&session_id).unwrap();
    assert!(
        messages
            .iter()
            .any(|message| message.role == MessageRole::Assistant
                && message.content == "final answer")
    );
}

#[test]
fn test_parse_memory_save_args() {
    assert_eq!(
        parse_memory_save_args("remember this"),
        (MemorySaveTarget::Auto, None, "remember this")
    );
    assert_eq!(
        parse_memory_save_args("--user reply in Chinese"),
        (MemorySaveTarget::User, None, "reply in Chinese")
    );
    assert_eq!(
        parse_memory_save_args("--topic tui-design keep bottom anchored"),
        (
            MemorySaveTarget::Topic,
            Some("tui-design"),
            "keep bottom anchored"
        )
    );
    assert_eq!(
        parse_memory_save_args("--topic=context-management track token budget"),
        (
            MemorySaveTarget::Topic,
            Some("context-management"),
            "track token budget"
        )
    );
}

#[test]
fn test_parse_memory_why_args_defaults_to_query() {
    assert_eq!(
        parse_memory_why_args("cache stability", "latest user"),
        Some(("cache stability", None, false))
    );
    assert_eq!(
        parse_memory_why_args("cache stability --item USER.md", "latest user"),
        Some(("cache stability", Some("USER.md"), false))
    );
    assert_eq!(
        parse_memory_why_args("--item USER.md", "latest user"),
        Some(("latest user", Some("USER.md"), false))
    );
    assert_eq!(parse_memory_why_args("--item USER.md", ""), None);
    assert_eq!(
        parse_memory_why_args("--last-turn", "latest user"),
        Some(("latest user", None, true))
    );
    // Combined: --last-turn with --item
    assert_eq!(
        parse_memory_why_args("--last-turn --item USER.md", "latest user"),
        Some(("latest user", Some("USER.md"), true))
    );
    // Combined: query with --last-turn
    assert_eq!(
        parse_memory_why_args("cache stability --last-turn", "latest user"),
        Some(("latest user", None, true))
    );
}

#[test]
fn test_format_memory_write_outcome_reports_safety_block() {
    let outcome = crate::memory::manager::MemoryWriteOutcome {
        status: crate::memory::manager::MemoryWriteOutcomeStatus::Blocked,
        quality_score: None,
        reason: "secret_like_content: memory appears to contain a raw token".to_string(),
        path: None,
        record: None,
        scoring_trace: None,
    };

    let rendered = format_memory_write_outcome("api_key = [redacted]", &outcome);

    assert!(rendered.contains("blocked for safety"));
    assert!(rendered.contains("secret_like_content"));
    assert!(!rendered.contains("Saved memory"));
}

#[test]
fn test_format_memory_records_filters_by_scope() {
    let mut project = crate::memory::MemoryRecord::new(
        "Project convention: run cargo test before commit.",
        crate::memory::MemoryKind::ProjectFact,
        crate::memory::MemoryScope::local("records-test"),
        crate::memory::MemoryProvenance::local("test"),
    );
    project.status = crate::memory::MemoryStatus::Accepted;
    let mut user = crate::memory::MemoryRecord::new(
        "User preference: answer in Chinese.",
        crate::memory::MemoryKind::UserPreference,
        crate::memory::MemoryScope::local("records-test"),
        crate::memory::MemoryProvenance::local("test"),
    );
    user.status = crate::memory::MemoryStatus::Accepted;

    let rendered = format_memory_records(&[project, user], "--scope project");

    assert!(rendered.contains("Memory Records"));
    assert!(rendered.contains("Project convention"));
    assert!(!rendered.contains("User preference"));
}

#[test]
fn test_stream_usage_label_includes_reasoning_and_cached_tokens() {
    let mut app = TuiApp::new();
    app.stream_usage_snapshot = Some(StreamUsageSnapshot {
        prompt_tokens: 100,
        completion_tokens: 25,
        reasoning_tokens: Some(12),
        cached_tokens: Some(80),
    });

    assert_eq!(
        app.stream_usage_label().as_deref(),
        Some("125 tokens / 12 reasoning / 80 cached / 20 miss / 80.0% hit")
    );
}

#[test]
fn test_status_bar_density_cycle_and_parse() {
    let mut app = TuiApp::new();
    assert_eq!(app.status_bar_density, StatusBarDensity::Normal);
    assert_eq!(app.cycle_status_bar_density(), StatusBarDensity::Debug);
    assert_eq!(app.cycle_status_bar_density(), StatusBarDensity::Compact);
    assert_eq!(
        StatusBarDensity::parse("verbose"),
        Some(StatusBarDensity::Debug)
    );
    assert_eq!(
        StatusBarDensity::parse("minimal"),
        Some(StatusBarDensity::Compact)
    );
}

#[test]
fn test_short_paste_inserts_directly() {
    let mut app = TuiApp::new();
    app.input.insert_str("prefix ");
    app.insert_paste("你好\nworld".to_string());

    assert_eq!(app.input.value(), "prefix 你好\nworld");
    assert_eq!(app.pasted_block_count(), 0);
}

#[test]
fn test_long_paste_uses_placeholder_and_expands() {
    let mut app = TuiApp::new();
    let pasted = (0..20)
        .map(|idx| format!("line {}", idx))
        .collect::<Vec<_>>()
        .join("\n");

    app.input.insert_str("please inspect ");
    app.insert_paste(pasted.clone());

    assert_eq!(app.pasted_block_count(), 1);
    assert!(app.input.value().contains("[[paste:1 20 lines"));
    assert_eq!(
        app.expand_paste_placeholders(app.input.value()),
        format!("please inspect {}", pasted)
    );
}

#[tokio::test]
async fn test_command_palette_accept_inserts_command_that_needs_args() {
    let mut app = TuiApp::new();
    app.open_command_palette();
    app.command_palette_push('s');
    app.command_palette_push('a');
    app.command_palette_push('v');
    app.command_palette_push('e');
    app.accept_command_palette_selection().await;

    assert_eq!(app.mode, AppMode::Chat);
    assert_eq!(app.input.value(), "/save ");
    assert!(app.recent_palette_commands.iter().any(|cmd| cmd == "/save"));
}

#[tokio::test]
async fn test_command_palette_accept_executes_no_arg_command() {
    let mut app = TuiApp::new();
    app.open_command_palette();
    app.command_palette_push('s');
    app.command_palette_push('t');
    app.command_palette_push('a');
    app.command_palette_push('t');
    app.command_palette_push('u');
    app.command_palette_push('s');
    app.accept_command_palette_selection().await;

    assert_eq!(app.mode, AppMode::Chat);
    assert!(app.input.value().is_empty());
    assert!(app
        .recent_palette_commands
        .iter()
        .any(|cmd| cmd == "/status"));
    assert!(app.messages.len() > 1);
}

#[test]
fn test_model_select_filters_choices() {
    let mut app = TuiApp::new();
    app.streaming_engine = Some(Arc::new(
        crate::engine::streaming::StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(crate::tools::ToolRegistry::new()),
            "gpt-4o",
        ),
    ));
    app.model_select_push('m');
    app.model_select_push('i');
    app.model_select_push('n');
    app.model_select_push('i');

    let choices = app.model_choices();
    assert!(choices.iter().all(|choice| choice.model.contains("mini")));
}

#[test]
fn test_model_select_empty_filter_returns_no_choices() {
    let mut app = TuiApp::new();
    app.streaming_engine = Some(Arc::new(
        crate::engine::streaming::StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(crate::tools::ToolRegistry::new()),
            "gpt-4o",
        ),
    ));
    app.model_select_query = "not-a-real-model".to_string();

    assert!(app.model_choices().is_empty());
}

#[test]
fn test_provider_select_filters_missing_providers() {
    let mut app = TuiApp::new();
    app.provider_select_push('k');
    app.provider_select_push('i');
    app.provider_select_push('m');
    app.provider_select_push('i');

    let choices = app.provider_choices();
    assert!(!choices.is_empty());
    assert!(choices
        .iter()
        .all(|choice| choice.name.contains("kimi") || choice.provider_type.contains("Kimi")));
}

#[test]
fn test_provider_select_empty_filter_returns_no_choices() {
    let mut app = TuiApp::new();
    app.provider_select_query = "not-a-real-provider".to_string();

    assert!(app.provider_choices().is_empty());
}

#[test]
fn test_workspace_entries_preview_summarizes_top_level_entries() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").unwrap();

    let preview = workspace_entries_preview(dir.path());

    assert!(preview.contains("1 dirs"));
    assert!(preview.contains("1 files"));
    assert!(preview.contains("src/"));
    assert!(preview.contains("Cargo.toml"));
}

#[test]
fn test_contextual_palette_prioritizes_pending_permission_actions() {
    let mut app = TuiApp::new();
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({ "command": "ls" }),
        },
        prompt: "Allow?".to_string(),
        review: None,
        audit: None,
    });

    let commands = app.contextual_palette_commands();
    assert_eq!(commands.first().map(String::as_str), Some("/reject"));
    assert!(commands.iter().any(|command| command == "/permissions"));
    assert!(app.is_contextual_palette_command("/reject"));

    let items = app.command_palette_items();
    assert_eq!(items.first().map(|cmd| cmd.name), Some("/reject"));
}

#[test]
fn test_contextual_palette_includes_session_actions_after_chat() {
    let mut app = TuiApp::new();
    app.messages.push(MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "hello".to_string(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    });

    let commands = app.contextual_palette_commands();

    assert!(commands.iter().any(|command| command == "/search"));
    assert!(commands.iter().any(|command| command == "/session"));
    assert!(commands.iter().any(|command| command == "/export"));
}

#[test]
fn test_command_palette_render_marks_placeholder_commands() {
    let mut app = TuiApp::new();
    app.open_command_palette();
    for ch in "desktop".chars() {
        app.command_palette_push(ch);
    }

    let rendered = render_command_palette_text(&app);

    assert!(rendered.contains("Command Palette"));
    assert!(rendered.contains("/desktop"));
    assert!(rendered.contains("[placeholder]"));
    assert!(rendered.contains("Maturity:"));
    assert!(rendered.contains("placeholder"));
}

#[test]
fn test_command_palette_render_marks_usable_commands() {
    let mut app = TuiApp::new();
    app.open_command_palette();
    for ch in "agents".chars() {
        app.command_palette_push(ch);
    }

    let rendered = render_command_palette_text(&app);

    assert!(rendered.contains("/agents"));
    assert!(rendered.contains("[usable]"));
    assert!(rendered.contains("Maturity:"));
    assert!(rendered.contains("usable"));
}

#[tokio::test]
async fn test_help_maturity_slash_reports_buckets() {
    let mut app = TuiApp::new();

    app.handle_slash_command("/help maturity").await;

    let content = app
        .messages
        .last()
        .map(|message| message.content.as_str())
        .unwrap_or("");
    assert!(content.contains("Command maturity:"));
    assert!(content.contains("- usable"));
    assert!(content.contains("/panel"));
    assert!(content.contains("- placeholder"));
}

#[tokio::test]
async fn test_mcp_status_slash_uses_runtime_panel_without_engine() {
    let mut app = TuiApp::new();

    app.handle_slash_command("/mcp status").await;

    let content = app
        .messages
        .last()
        .map(|message| message.content.as_str())
        .unwrap_or("");
    assert!(content.contains("# MCP Panel"));
    assert!(content.contains("Manager: engine unavailable"));
}

#[tokio::test]
async fn test_permissions_slash_includes_pending_approval_panel() {
    let mut app = TuiApp::new();
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({ "command": "cargo check -q" }),
        },
        prompt: "Approve shell command?".to_string(),
        review: None,
        audit: None,
    });

    app.handle_slash_command("/permissions").await;

    let content = app
        .messages
        .last()
        .map(|message| message.content.as_str())
        .unwrap_or("");
    assert!(content.contains("Mode:"));
    assert!(content.contains("# Approval Panel"));
    assert!(content.contains("Name: bash"));
}

#[tokio::test]
async fn test_context_slash_uses_runtime_context_panel() {
    let mut app = TuiApp::new();

    app.handle_slash_command("/context").await;

    let content = app
        .messages
        .last()
        .map(|message| message.content.as_str())
        .unwrap_or("");
    assert!(content.contains("# Context Panel"));
    assert!(content.contains("Context budget: engine unavailable"));
    assert!(!content.contains("# Context Status"));
}

#[test]
fn test_command_palette_render_prioritizes_contextual_permission_actions() {
    let mut app = TuiApp::new();
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({ "command": "ls" }),
        },
        prompt: "Allow?".to_string(),
        review: None,
        audit: None,
    });
    app.open_command_palette();

    let rendered = render_command_palette_text(&app);

    assert!(rendered.contains("Context"));
    assert!(rendered.contains("/reject"));
    assert!(rendered.contains("/permissions"));
    assert!(rendered.contains("Maturity:"));
}

#[test]
fn test_cycle_expanded_tool_run_moves_through_visible_tools() {
    let mut app = TuiApp::new();
    let user = MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "run tools".to_string(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    };
    app.messages.push(user);
    app.tool_runs_by_message_id.insert(
        "user_1".to_string(),
        vec![
            ToolRunView::new("tool_1".to_string(), "bash".to_string()),
            ToolRunView::new("tool_2".to_string(), "grep".to_string()),
        ],
    );

    app.cycle_expanded_tool_run();
    assert_eq!(app.expanded_tool_run_id.as_deref(), Some("tool_1"));
    app.cycle_expanded_tool_run();
    assert_eq!(app.expanded_tool_run_id.as_deref(), Some("tool_2"));
    app.cycle_expanded_tool_run();
    assert_eq!(app.expanded_tool_run_id, None);
}

#[test]
fn test_open_tool_viewer_uses_expanded_tool_or_latest() {
    let mut app = TuiApp::new();
    let user = MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "run tools".to_string(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    };
    app.messages.push(user);
    let mut first = ToolRunView::new("tool_1".to_string(), "bash".to_string());
    first.mark_complete("Result: OK\nfirst\n".to_string());
    let mut second = ToolRunView::new("tool_2".to_string(), "grep".to_string());
    second.mark_complete("Result: OK\nsecond\n".to_string());
    app.tool_runs_by_message_id
        .insert("user_1".to_string(), vec![first.clone(), second.clone()]);

    assert!(app.open_tool_viewer());
    assert_eq!(app.mode, AppMode::ToolViewer);
    assert!(app.tool_viewer_content.contains("second"));

    app.mode = AppMode::Chat;
    app.expanded_tool_run_id = Some("tool_1".to_string());
    assert!(app.open_tool_viewer());
    assert!(app.tool_viewer_content.contains("first"));
}

#[test]
fn test_tool_output_index_and_open_by_id() {
    let mut app = TuiApp::new();
    let user = MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "run tools".to_string(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    };
    app.messages.push(user);
    let mut first = ToolRunView::new("tool_1".to_string(), "bash".to_string());
    first.mark_complete("Result: OK\nfirst\n".to_string());
    app.tool_runs_by_message_id
        .insert("user_1".to_string(), vec![first]);

    let lines = app.tool_output_index_lines();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("tool_1"));
    assert!(lines[0].contains("[completed]"));
    assert!(app.open_tool_viewer_for("tool_1"));
    assert!(app.tool_viewer_content.contains("first"));
    assert!(!app.open_tool_viewer_for("missing"));
}

#[test]
fn test_runtime_snapshot_keeps_terminal_task_metadata() {
    let mut app = TuiApp::new();
    let mut run = ToolRunView::new("tool_bg".to_string(), "bash".to_string());
    run.mark_complete_with_metadata(
        "Result: OK\nStarted background shell\n".to_string(),
        Some(serde_json::json!({
            "terminal_task": {
                "task_id": "shell_bg_1",
                "status": "running",
                "terminal_kind": "background_shell",
                "command": "npm run dev",
                "handle": "shell_bg_1",
                "read_tool": "bash_output",
                "cancel_handle": "shell_bg_1"
            },
            "operation_kind": "shell",
            "ui_render_kind": "shell",
            "read_only": false,
            "concurrency_safe": false,
            "destructive": false,
            "input_paths": ["package.json"],
            "transcript_summary": "npm run dev"
        })),
    );
    app.tool_runs_snapshot.push(run);
    app.runtime_state_snapshot = app.build_runtime_state_snapshot();

    assert_eq!(
        app.runtime_state_snapshot.tool_uses[0]
            .operation_kind
            .as_deref(),
        Some("shell")
    );
    assert_eq!(
        app.runtime_state_snapshot.tool_uses[0].input_paths,
        vec!["package.json"]
    );
    assert_eq!(app.runtime_state_snapshot.terminal_tasks.len(), 1);
    assert_eq!(
        app.terminal_task_status_label().as_deref(),
        Some("terminal:1 running:1")
    );
}

#[test]
fn test_session_permission_rule_is_added_when_approving_for_session() {
    let engine = Arc::new(crate::engine::streaming::StreamingQueryEngine::new(
        Arc::new(MockProvider),
        Arc::new(crate::tools::ToolRegistry::new()),
        "gpt-4o",
    ));
    let mut app = TuiApp::with_engine(engine.clone(), None, None);
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tc_1".to_string(),
            name: "mcp_tool".to_string(),
            arguments: serde_json::json!({
                "server_name": "filesystem",
                "tool_name": "write_file"
            }),
        },
        prompt: "Approve MCP?".to_string(),
        review: None,
        audit: None,
    });
    app.permission_response_tx = Some(tx);
    app.mode = AppMode::PermissionApproval;

    app.respond_to_permission_with_rule(true, Some("allow"), Some(RuleSource::User));

    let response = rx.try_recv().unwrap();
    assert!(response.approved);
    assert_eq!(
        response.decision,
        Some(PermissionReviewDecision::ApproveSession)
    );
    assert_eq!(response.persistence_scope.as_deref(), Some("session"));
    assert_eq!(
        response.rule_pattern.as_deref(),
        Some("mcp/filesystem/write_file")
    );
    let rules = engine.session_permission_rules();
    assert!(rules
        .always_allow
        .iter()
        .any(|rule| rule.pattern == "mcp/filesystem/write_file"));
    assert_eq!(app.mode, AppMode::Chat);
}

#[test]
fn test_bash_session_permission_rule_uses_command_scope() {
    let engine = Arc::new(crate::engine::streaming::StreamingQueryEngine::new(
        Arc::new(MockProvider),
        Arc::new(crate::tools::ToolRegistry::new()),
        "gpt-4o",
    ));
    let mut app = TuiApp::with_engine(engine.clone(), None, None);
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tc_bash".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "cargo test -q"}),
        },
        prompt: "Approve bash?".to_string(),
        review: None,
        audit: None,
    });
    app.permission_response_tx = Some(tx);
    app.mode = AppMode::PermissionApproval;

    app.respond_to_permission_with_rule(true, Some("allow"), Some(RuleSource::User));

    let response = rx.try_recv().unwrap();
    assert!(response.approved);
    assert_eq!(response.rule_pattern.as_deref(), Some("bash:cargo test*"));
    let rules = engine.session_permission_rules();
    assert!(rules
        .always_allow
        .iter()
        .any(|rule| rule.pattern == "bash:cargo test*"));
    assert_eq!(app.mode, AppMode::Chat);
}

#[test]
fn test_model_selection_updates_engine_model() {
    let mut app = TuiApp::new();
    app.streaming_engine = Some(Arc::new(
        crate::engine::streaming::StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(crate::tools::ToolRegistry::new()),
            "gpt-4o",
        ),
    ));
    app.open_model_select();
    let choices = app.model_choices();
    let target = choices
        .iter()
        .position(|choice| choice.model == "gpt-4o-mini")
        .expect("openai preset expected");
    app.model_select_selected = target;
    app.accept_model_selection();

    assert_eq!(app.current_model_label(), "gpt-4o-mini");
    assert_eq!(app.mode, AppMode::Chat);
}

#[tokio::test]
async fn test_send_message_blocked_when_paused() {
    let mut app = TuiApp::new();
    app.paused = true;
    let before = app.messages.len();
    app.send_message("hello".to_string()).await;
    assert_eq!(app.messages.len(), before + 1);
    let last = app.messages.last().expect("system message expected");
    assert_eq!(last.role, MessageRole::System);
    assert!(last.content.contains("Agent is paused"));
}

#[tokio::test]
async fn test_send_message_keeps_bottom_anchor_after_assistant_placeholder() {
    let mut app = TuiApp::new();
    app.streaming_engine = Some(Arc::new(
        crate::engine::streaming::StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(crate::tools::ToolRegistry::new()),
            "mock-model",
        ),
    ));

    app.send_message("hello".to_string()).await;

    assert_eq!(app.messages.last().unwrap().role, MessageRole::Assistant);
    assert_eq!(app.scroll_offset, app.messages.len());
}

#[tokio::test]
async fn test_restore_session() {
    let mut app = TuiApp::new();

    // 创建一个测试会话并添加消息
    let session_id = app
        .session_manager
        .start_session("Test Session", "kimi-k2.5")
        .unwrap();
    app.session_manager
        .add_message(MessageRole::User, "Hello")
        .unwrap();
    app.session_manager
        .add_message(MessageRole::Assistant, "Hi there!")
        .unwrap();

    // 验证消息已保存
    let count = app.session_manager.message_count(&session_id).unwrap();
    assert_eq!(count, 2);

    // 清空当前消息（模拟切换到新会话后的状态）
    app.messages.clear();
    app.messages.push(MessageItem {
        id: "temp".to_string(),
        role: MessageRole::System,
        content: "Temp".to_string(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    });

    // 恢复会话
    let result = app.restore_session(&session_id).await;
    assert!(result.contains("Restored session"));
    assert!(result.contains("2 messages"));
    assert!(result.contains("Recent context:"));
    assert!(result.contains("Hello"));
    assert!(result.contains("Hi there!"));

    // 验证 UI 消息已恢复
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].role, MessageRole::User);
    assert_eq!(app.messages[0].content, "Hello");
    assert_eq!(app.messages[1].role, MessageRole::Assistant);
    assert_eq!(app.messages[1].content, "Hi there!");

    // 验证当前会话 ID 已更新
    assert_eq!(
        app.session_manager.current_session_id(),
        Some(session_id.as_str())
    );
}

#[tokio::test]
async fn test_restore_session_not_found() {
    let mut app = TuiApp::new();
    let result = app.restore_session("nonexistent_session").await;
    assert!(result.contains("Failed to restore session"));
}

#[test]
fn test_respond_to_permission() {
    let mut app = TuiApp::new();
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tc_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "echo hello"}),
        },
        prompt: "Approve bash?".to_string(),
        review: None,
        audit: None,
    });
    app.permission_response_tx = Some(tx);
    app.mode = AppMode::PermissionApproval;

    app.respond_to_permission(true);

    assert_eq!(app.mode, AppMode::Chat);
    assert!(app.pending_permission_request.is_none());
    assert!(app.permission_response_tx.is_none());
    let response = rx.try_recv().unwrap();
    assert!(response.approved);
    assert_eq!(
        response.decision,
        Some(PermissionReviewDecision::ApproveOnce)
    );
    assert!(response.persistence_scope.is_none());
}

#[test]
fn test_compute_permission_diff_file_write() {
    let mut app = TuiApp::new();
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tc_1".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({
                "path": "src/main.rs",
                "content": "fn main() {\n    println!(\"hello\");\n}"
            }),
        },
        prompt: "Approve file write?".to_string(),
        review: None,
        audit: None,
    });

    let (title, diff) = app.compute_permission_diff().unwrap();
    assert_eq!(title, "Preview: src/main.rs");
    assert!(diff.contains("+++ b/src/main.rs"));
    assert!(diff.contains("+fn main() {"));
    assert!(diff.contains("+    println!(\"hello\");"));
}

#[test]
fn test_compute_permission_diff_file_edit_replace() {
    let mut app = TuiApp::new();
    // 使用不存在的文件路径确保回退到旧行为
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tc_1".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({
                "path": "nonexistent_file.rs",
                "old_string": "println!(\"hello\");",
                "new_string": "println!(\"world\");"
            }),
        },
        prompt: "Approve file edit?".to_string(),
        review: None,
        audit: None,
    });

    let (title, diff) = app.compute_permission_diff().unwrap();
    assert_eq!(title, "Preview: nonexistent_file.rs");
    assert!(diff.contains("--- old_string ---"));
    assert!(diff.contains("-println!(\"hello\");"));
    assert!(diff.contains("+++ new_string +++"));
    assert!(diff.contains("+println!(\"world\");"));
}

#[test]
fn test_compute_permission_diff_file_edit_insert() {
    let mut app = TuiApp::new();
    // 使用不存在的文件路径确保回退到旧行为
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tc_1".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({
                "path": "nonexistent_file.rs",
                "insert_after": "fn main() {",
                "new_string": "    // new line"
            }),
        },
        prompt: "Approve file edit?".to_string(),
        review: None,
        audit: None,
    });

    let (title, diff) = app.compute_permission_diff().unwrap();
    assert_eq!(title, "Preview: nonexistent_file.rs");
    assert!(diff.contains("Insert after:"));
    assert!(diff.contains("fn main() {"));
    assert!(diff.contains("// new line"));
}

#[test]
fn test_compute_permission_diff_bash() {
    let mut app = TuiApp::new();
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tc_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "cargo test",
                "working_dir": "/tmp",
                "timeout": 60
            }),
        },
        prompt: "Approve bash?".to_string(),
        review: None,
        audit: None,
    });

    let (title, diff) = app.compute_permission_diff().unwrap();
    assert_eq!(title, "Preview: bash command");
    assert!(diff.contains("cargo test"));
    assert!(diff.contains("/tmp"));
    assert!(diff.contains("60s"));
}

#[test]
fn memory_search_output_shows_retrieval_score_explanation() {
    let matches = vec![crate::memory::manager::MemoryMatch {
        source: "USER.md".to_string(),
        score: 36,
        rerank_score: Some(0.92),
        snippet: "User preference: answer concise Chinese status updates.".to_string(),
    }];
    let ctx = crate::engine::retrieval_context::RetrievalContext::from_memory_matches(
        "Chinese status",
        matches,
        &[],
        crate::engine::intent_router::RetrievalPolicy::Memory,
    )
    .expect("retrieval context");

    let output = format_memory_retrieval_context(&ctx);

    assert!(output.contains("decision selected USER.md"));
    assert!(output.contains("lexical="));
    assert!(output.contains("scope_match="));
    assert!(output.contains("confidence="));
    assert!(output.contains("conflict_penalty="));
    assert!(output.contains("pinned_bonus="));
}

#[test]
fn test_compute_permission_diff_unsupported_tool() {
    let mut app = TuiApp::new();
    app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tc_1".to_string(),
            name: "grep".to_string(),
            arguments: serde_json::json!({"pattern": "foo"}),
        },
        prompt: "Approve grep?".to_string(),
        review: None,
        audit: None,
    });

    assert!(app.compute_permission_diff().is_none());
}

#[test]
fn skill_outcome_prefers_acceptance_review_signal() {
    let mut trace = crate::engine::trace::TurnTrace::new("s1", 1, "use skill");
    trace
        .events
        .push(crate::engine::trace::TraceEvent::VerificationCompleted {
            changed_files: 2,
            passed: true,
            check_passed: true,
            tests_passed: true,
            review_passed: true,
            failed_commands: Vec::new(),
        });
    trace.events.push(
        crate::engine::trace::TraceEvent::AcceptanceReviewCompleted {
            accepted: true,
            confidence: "high".to_string(),
            criteria: 3,
            unresolved: 0,
            next_action: "close".to_string(),
        },
    );

    let outcome = skill_outcome_attribution(Some(&trace), true, false, false);

    assert!(outcome.success);
    assert_eq!(outcome.acceptance_passed, Some(true));
    assert_eq!(outcome.tests_passed, Some(true));
    assert_eq!(outcome.source, "acceptance_review");
    assert!(outcome.confidence > 0.8);
}

#[test]
fn skill_outcome_blocks_on_unresolved_acceptance() {
    let mut trace = crate::engine::trace::TurnTrace::new("s1", 1, "use skill");
    trace
        .events
        .push(crate::engine::trace::TraceEvent::VerificationCompleted {
            changed_files: 2,
            passed: true,
            check_passed: true,
            tests_passed: true,
            review_passed: true,
            failed_commands: Vec::new(),
        });
    trace.events.push(
        crate::engine::trace::TraceEvent::AcceptanceReviewCompleted {
            accepted: false,
            confidence: "medium".to_string(),
            criteria: 3,
            unresolved: 1,
            next_action: "repair".to_string(),
        },
    );

    let outcome = skill_outcome_attribution(Some(&trace), true, false, false);

    assert!(!outcome.success);
    assert_eq!(outcome.acceptance_passed, Some(false));
    assert_eq!(outcome.tests_passed, Some(true));
    assert!(outcome.risk_penalty >= 0.45);
}

#[test]
fn provider_lifecycle_clears_timer_on_terminal_diagnostic() {
    let mut lifecycle = ProviderRequestLifecycle::default();

    lifecycle.update_from_diagnostic(&serde_json::json!({
        "schema": "api_request_stage.v1",
        "stage": "api_request_started",
        "provider_family": "openai",
        "slow_warning_threshold_ms": 10
    }));
    assert!(lifecycle.phase.is_active());

    lifecycle.update_from_diagnostic(&serde_json::json!({
        "schema": "provider_request.v1",
        "stage": "provider_request_completed",
        "elapsed_ms": 12
    }));

    assert!(!lifecycle.phase.is_active());
    assert_eq!(lifecycle.phase, ProviderPhase::Completed);
    assert_eq!(lifecycle.elapsed_ms, 12);
}
