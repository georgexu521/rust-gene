use super::*;
use crate::engine::human_review::PermissionReviewDecision;
use crate::engine::runtime_facade::{
    ProviderPhase, ProviderRequestLifecycle, ToolTurnPhase, ToolTurnSnapshot,
};
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
    assert_eq!(app.messages.len(), 0); // no welcome message
    assert!(!app.is_querying);
    assert!(!app.paused);
    assert!(!app.focus_mode);
}

#[tokio::test]
async fn cancel_active_run_interrupts_query_and_marks_tool_cancelled() {
    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now());
    app.current_tool_anchor_id = Some("msg_0".to_string());
    app.messages.push(MessageItem {
        id: "msg_0".to_string(),
        role: MessageRole::User,
        content: "run pwd".to_string(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    });
    app.messages.push(MessageItem {
        id: "msg_1".to_string(),
        role: MessageRole::Assistant,
        content: String::new(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    });
    {
        let mut sync = app.sync_store.lock().await;
        sync.start_turn("msg_0".to_string(), "msg_1".to_string());
        sync.apply_stream_event(&StreamEvent::ToolCallStart {
            id: "call_1".to_string(),
            name: "bash".to_string(),
        });
    }

    assert!(app.cancel_active_run("Run interrupted").await);

    assert!(!app.is_querying);
    assert!(app.stream_started_at.is_none());
    assert!(app.current_tool_anchor_id.is_none());
    assert!(app.messages[1].content.contains("Cancelled"));
    assert_eq!(
        app.projected_tool_runs()[0].status,
        ToolRunStatus::Cancelled
    );
    let projected_runs = app
        .sync_snapshot
        .tool_runs_for_message("msg_0")
        .expect("projected tool runs");
    assert_eq!(projected_runs[0].status, ToolRunStatus::Cancelled);
}

#[tokio::test]
async fn timeout_active_run_finishes_query_and_marks_tool_failed() {
    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now());
    app.current_tool_anchor_id = Some("msg_0".to_string());
    app.messages.push(MessageItem {
        id: "msg_0".to_string(),
        role: MessageRole::User,
        content: "run pwd".to_string(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    });
    app.messages.push(MessageItem {
        id: "msg_1".to_string(),
        role: MessageRole::Assistant,
        content: String::new(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    });
    {
        let mut sync = app.sync_store.lock().await;
        sync.start_turn("msg_0".to_string(), "msg_1".to_string());
        sync.apply_stream_event(&StreamEvent::ToolCallStart {
            id: "call_1".to_string(),
            name: "bash".to_string(),
        });
    }

    assert!(
        app.timeout_active_run("provider request timed out after 120.0s")
            .await
    );

    assert!(!app.is_querying);
    assert!(app.stream_started_at.is_none());
    assert!(app.current_tool_anchor_id.is_none());
    assert_eq!(
        app.messages[1].content,
        "[Error: provider request timed out after 120.0s]"
    );
    let projected_runs = app.projected_tool_runs();
    assert_eq!(projected_runs[0].status, ToolRunStatus::Failed);
    assert!(projected_runs[0]
        .result_body
        .as_deref()
        .is_some_and(|result| result.contains("provider request timed out")));
}

#[test]
fn timeout_active_run_immediate_writes_visible_error_without_await() {
    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now());
    app.messages.push(MessageItem {
        id: "msg_1".to_string(),
        role: MessageRole::Assistant,
        content: String::new(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    });

    assert!(app.timeout_active_run_immediate("provider request timed out after 30.0s"));

    assert!(!app.is_querying);
    assert!(app.stream_started_at.is_none());
    assert_eq!(
        app.messages[0].content,
        "[Error: provider request timed out after 30.0s]"
    );
}

#[tokio::test]
async fn refresh_response_times_out_stale_provider_wait() {
    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
    app.runtime_facade_state.set_querying(true).await;
    app.runtime_facade_state
        .process_diagnostic(&serde_json::json!({
            "schema": "api_request_stage.v1",
            "stage": "api_request_started",
            "provider_family": "deepseek",
            "model": "deepseek-v4-flash",
            "timeout_ms": 1_000
        }))
        .await;
    app.runtime_facade_state
        .process_diagnostic(&serde_json::json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_timeout",
            "elapsed_ms": 3_000,
            "message": "provider request timed out after 1.0s"
        }))
        .await;
    app.messages.push(MessageItem {
        id: "msg_1".to_string(),
        role: MessageRole::Assistant,
        content: String::new(),
        timestamp: std::time::SystemTime::now(),
        metadata: Default::default(),
    });

    app.refresh_response().await;

    assert!(!app.is_querying);
    assert_eq!(
        app.messages[0].content,
        "[Error: provider request timed out after 1.0s]"
    );
}

#[test]
fn provider_watchdog_honors_explicit_shorter_timeout() {
    let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    guard.set("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "30");

    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
    app.facade_snapshot.provider_request.phase = ProviderPhase::SlowWarning;
    app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
    app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
    app.facade_snapshot.provider_request.timeout_ms = 120_000;
    app.facade_snapshot.provider_request.elapsed_ms = 31_000;

    let reason = app
        .provider_wait_timeout_reason()
        .expect("provider wait should time out");

    assert_eq!(reason, "provider request timed out after 30.0s");
}

#[test]
fn provider_watchdog_uses_current_provider_elapsed_not_whole_turn_elapsed() {
    let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    guard.set("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "30");

    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
    app.facade_snapshot.provider_request.phase = ProviderPhase::Started;
    app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
    app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
    app.facade_snapshot.provider_request.timeout_ms = 120_000;
    app.facade_snapshot.provider_request.elapsed_ms = 1_000;

    assert_eq!(app.provider_wait_timeout_reason(), None);
}

#[test]
fn provider_watchdog_times_out_query_when_provider_phase_is_lost() {
    let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    guard.set("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "30");

    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
    app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
    app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
    app.facade_snapshot.provider_request.timeout_ms = 120_000;

    let reason = app
        .provider_wait_timeout_reason()
        .expect("stale query should still time out");

    assert_eq!(reason, "provider request timed out after 30.0s");
}

#[test]
fn provider_watchdog_ignores_queued_tool_placeholder() {
    let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    guard.set("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "30");

    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
    app.sync_snapshot.set_tool_runs_for_message(
        "user_1".to_string(),
        vec![ToolRunView::new("queued".to_string(), "bash".to_string())],
    );

    let reason = app
        .provider_wait_timeout_reason()
        .expect("queued tool placeholder must not block provider timeout");

    assert_eq!(reason, "provider request timed out after 30.0s");
}

#[test]
fn provider_watchdog_times_out_post_tool_result_stall() {
    let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    guard.set("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "30");

    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
    app.facade_snapshot.provider_request.phase = ProviderPhase::Completed;
    app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
    app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
    app.facade_snapshot.provider_request.timeout_ms = 120_000;
    app.facade_snapshot.provider_request.elapsed_ms = 31_000;
    app.facade_snapshot.tool_turns.push(ToolTurnSnapshot {
        id: "call_1".to_string(),
        name: "bash".to_string(),
        parent_message_id: Some("user_1".to_string()),
        phase: ToolTurnPhase::ResultObserved,
        arguments_preview: None,
        result_preview: Some("Result: OK".to_string()),
        failure: None,
    });

    let reason = app
        .provider_wait_timeout_reason()
        .expect("post-tool result stall should time out");

    assert_eq!(
        reason,
        "tool turn stalled after result observation for 30.0s"
    );
}

#[test]
fn provider_watchdog_does_not_reuse_whole_turn_elapsed_for_post_tool_wait() {
    let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    guard.set("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "30");

    let mut app = TuiApp::new();
    app.is_querying = true;
    app.stream_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
    app.facade_snapshot.provider_request.phase = ProviderPhase::Completed;
    app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
    app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
    app.facade_snapshot.provider_request.timeout_ms = 120_000;
    app.facade_snapshot.provider_request.elapsed_ms = 3_000;
    app.facade_snapshot.tool_turns.push(ToolTurnSnapshot {
        id: "call_1".to_string(),
        name: "file_read".to_string(),
        parent_message_id: Some("user_1".to_string()),
        phase: ToolTurnPhase::SentBackToModel,
        arguments_preview: None,
        result_preview: Some("Result: OK".to_string()),
        failure: None,
    });

    assert_eq!(app.provider_wait_timeout_reason(), None);
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

    let app = TuiApp::with_engine(Some(engine), None, None);

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

    let app = TuiApp::with_engine(Some(engine), None, None);

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
    let mut app = TuiApp::with_engine(Some(engine), None, None);
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
        let mut sync = app.sync_store.lock().await;
        sync.start_turn(
            "user-placeholder".to_string(),
            "assistant-placeholder".to_string(),
        );
        sync.apply_stream_event(&StreamEvent::TextChunk("final answer".to_string()));
        sync.apply_stream_event(&StreamEvent::Complete);
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
    assert_eq!(
        app.pasted_block_summaries(),
        vec!["20 lines / 149 chars".to_string()]
    );
    assert!(app.input.value().contains("[[paste:1 20 lines"));
    assert!(app.open_paste_viewer(None));
    assert_eq!(app.mode, AppMode::ToolViewer);
    assert!(app.tool_viewer_title.contains("Paste 1"));
    assert!(app.tool_viewer_content.contains("line 19"));
    assert_eq!(
        app.expand_paste_placeholders(app.input.value()),
        format!("please inspect {}", pasted)
    );
}

#[test]
fn test_composer_attachments_add_remove_and_clear() {
    let mut app = TuiApp::new();

    let added = app.attach_context_path("Cargo.toml").unwrap();
    assert!(added.contains("Attached context: Cargo.toml"));
    assert_eq!(app.composer_attachment_count(), 1);
    let summaries = app.composer_attachment_summaries();
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].contains("[1] Cargo.toml"));
    assert!(summaries[0].contains("(file,"));

    let duplicate = app.attach_context_path("Cargo.toml").unwrap();
    assert!(duplicate.contains("Already attached"));
    assert_eq!(app.composer_attachment_count(), 1);

    assert_eq!(
        app.remove_composer_attachment(1).as_deref(),
        Some("Cargo.toml")
    );
    assert_eq!(app.composer_attachment_count(), 0);

    app.attach_context_path("Cargo.toml").unwrap();
    assert_eq!(app.clear_composer_attachments(), 1);
    assert_eq!(app.composer_attachment_count(), 0);
}

#[test]
fn test_attachment_preview_opens_tool_viewer_for_file_and_dir() {
    let root = std::env::temp_dir().join(format!(
        "priority-agent-attachment-preview-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("note.txt"), "preview body").unwrap();

    let mut app = TuiApp::new();
    app.attach_context_path(root.join("note.txt").to_str().unwrap())
        .unwrap();
    app.attach_context_path(root.to_str().unwrap()).unwrap();

    assert!(app.open_attachment_viewer(Some(1)));
    assert_eq!(app.mode, AppMode::ToolViewer);
    assert!(app.tool_viewer_title.contains("Attachment 1"));
    assert!(app.tool_viewer_content.contains("preview body"));

    app.mode = AppMode::Chat;
    assert!(app.open_attachment_viewer(Some(2)));
    assert!(app.tool_viewer_title.contains("Attachment 2"));
    assert!(app.tool_viewer_content.contains("Directory:"));
    assert!(app.tool_viewer_content.contains("note.txt"));

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_submit_message_injects_and_clears_composer_attachments() {
    let mut app = TuiApp::new();
    app.attach_context_path("Cargo.toml").unwrap();
    app.input.insert_str("explain this config");

    app.submit_message().await;

    let user_message = app
        .messages
        .iter()
        .find(|msg| msg.role == MessageRole::User)
        .expect("user message should be recorded");
    assert!(user_message.content.contains("Attached context:"));
    assert!(user_message.content.contains("- Cargo.toml"));
    assert!(user_message
        .content
        .contains("User request:\nexplain this config"));
    assert_eq!(app.composer_attachment_count(), 0);
}

#[tokio::test]
async fn test_attach_slash_updates_composer_attachments() {
    let mut app = TuiApp::new();
    app.input.insert_str("/attach Cargo.toml");

    app.submit_message().await;

    assert_eq!(app.composer_attachment_count(), 1);
    assert_eq!(app.composer_attachments[0], "Cargo.toml");
    let system_message = app
        .messages
        .iter()
        .rev()
        .find(|msg| msg.role == MessageRole::System)
        .expect("slash response should be recorded as system message");
    assert!(system_message.content.contains("Attached context"));

    app.input.insert_str("/attach list");
    app.submit_message().await;

    let list_message = app
        .messages
        .iter()
        .rev()
        .find(|msg| msg.role == MessageRole::System)
        .expect("slash list response should be recorded");
    assert!(list_message.content.contains("[1] Cargo.toml"));
    assert!(list_message.content.contains("(file,"));
    assert!(list_message.content.contains("preview:/attach preview <n>"));
    assert!(list_message.content.contains("remove:/attach remove <n>"));
}

#[tokio::test]
async fn test_attach_preview_slash_opens_attachment_viewer() {
    let root = std::env::temp_dir().join(format!(
        "priority-agent-attach-preview-slash-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("note.txt"), "slash preview").unwrap();

    let mut app = TuiApp::new();
    app.attach_context_path(root.join("note.txt").to_str().unwrap())
        .unwrap();
    app.input.insert_str("/attach preview 1");

    app.submit_message().await;

    assert_eq!(app.mode, AppMode::ToolViewer);
    assert!(app.tool_viewer_content.contains("slash preview"));

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_attach_browse_slash_opens_file_picker() {
    let mut app = TuiApp::new();
    app.input.insert_str("/attach browse .");

    app.submit_message().await;

    assert_eq!(app.mode, AppMode::FilePicker);
    assert!(app.file_picker_state.is_some());
}

#[test]
fn test_file_picker_attaches_selected_file() {
    let root =
        std::env::temp_dir().join(format!("priority-agent-file-picker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("note.txt"), "hello").unwrap();

    let mut app = TuiApp::new();
    let opened = app.open_composer_file_picker(Some(root.to_str().unwrap()));
    assert!(opened.contains("File picker opened"));
    assert_eq!(app.mode, AppMode::FilePicker);

    app.file_picker_next();
    let attached = app.accept_file_picker_selection();

    assert!(attached.contains("Attached context"));
    assert_eq!(app.mode, AppMode::Chat);
    assert_eq!(app.composer_attachment_count(), 1);
    assert!(app.composer_attachments[0].ends_with("note.txt"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_file_picker_filter_methods_update_selection() {
    let root = std::env::temp_dir().join(format!(
        "priority-agent-file-picker-filter-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("alpha.rs"), "a").unwrap();
    std::fs::write(root.join("beta.rs"), "b").unwrap();

    let mut app = TuiApp::new();
    app.open_composer_file_picker(Some(root.to_str().unwrap()));
    app.start_file_picker_filter();
    app.push_file_picker_filter_char('a');
    app.push_file_picker_filter_char('l');

    let state = app.file_picker_state.as_ref().unwrap();
    assert_eq!(state.filter_query(), "al");
    assert!(state.selected_path().unwrap().ends_with("alpha.rs"));

    app.pop_file_picker_filter_char();
    assert_eq!(app.file_picker_state.as_ref().unwrap().filter_query(), "a");
    app.clear_file_picker_filter();
    assert_eq!(app.file_picker_state.as_ref().unwrap().filter_query(), "");
    assert!(!app.file_picker_filtering);

    let _ = std::fs::remove_dir_all(&root);
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
    assert!(!app.messages.is_empty());
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
        diff_preview: None,
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
fn test_prompt_history_and_stash_contextual_palette() {
    let mut app = TuiApp::new();
    app.history.push_back("first prompt".to_string());
    app.input.set_value("draft prompt".to_string());

    let commands = app.contextual_palette_commands();

    assert!(commands.contains(&"/prompt-history".to_string()));
    assert!(commands.contains(&"/prompt-stash".to_string()));
}

#[test]
fn test_prompt_stash_save_restore_and_clear() {
    let mut app = TuiApp::new();
    app.input.set_value("draft prompt".to_string());

    assert!(app.save_prompt_stash_from_input());
    assert!(app.input.value().is_empty());
    assert_eq!(app.prompt_stash_summary().as_deref(), Some("draft prompt"));

    assert!(app.restore_prompt_stash_to_input());
    assert_eq!(app.input.value(), "draft prompt");
    assert!(app.prompt_stash.is_none());

    app.input.set_value("second draft".to_string());
    assert!(app.save_prompt_stash_from_input());
    assert!(app.clear_prompt_stash());
    assert!(app.prompt_stash.is_none());
}

#[test]
fn test_jump_to_failed_and_edit_timeline_items() {
    let mut app = TuiApp::new();
    let first_user = MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "first".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    };
    let second_user = MessageItem {
        id: "user_2".to_string(),
        role: MessageRole::User,
        content: "second".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    };
    app.messages.push(first_user.clone());
    app.messages.push(MessageItem {
        id: "assistant_1".to_string(),
        role: MessageRole::Assistant,
        content: "reply".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.messages.push(second_user.clone());

    let mut failed = ToolRunView::new("tool_failed".to_string(), "bash".to_string());
    failed.mark_complete("Result: ERROR\nfailed".to_string());
    app.sync_snapshot
        .set_tool_runs_for_message(first_user.id.clone(), vec![failed]);

    let mut edit = ToolRunView::new("tool_edit".to_string(), "file_edit".to_string());
    edit.mark_complete("Result: OK\nchanged".to_string());
    app.sync_snapshot
        .set_tool_runs_for_message(second_user.id.clone(), vec![edit]);

    let failed_result = app.jump_to_timeline_target("failed");
    assert!(failed_result.contains("failed"));
    assert_eq!(app.scroll_offset, 1);
    assert!(!app.pinned_to_bottom);

    let edit_result = app.jump_to_timeline_target("edit");
    assert!(edit_result.contains("edit"));
    assert_eq!(app.scroll_offset, 4);
}

#[test]
fn test_scroll_down_uses_timeline_item_count_with_tool_groups() {
    let mut app = TuiApp::new();
    let user = MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "run tests".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    };
    app.messages.push(user.clone());
    app.messages.push(MessageItem {
        id: "assistant_1".to_string(),
        role: MessageRole::Assistant,
        content: "done".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.sync_snapshot.set_tool_runs_for_message(
        user.id.clone(),
        vec![ToolRunView::new("tool_1".to_string(), "bash".to_string())],
    );

    assert_eq!(app.timeline_item_count(), 3);

    app.scroll_offset = 2;
    app.pinned_to_bottom = false;
    app.scroll_down();

    assert_eq!(app.scroll_offset, 3);
    assert!(app.pinned_to_bottom);
    assert!(app.scroll_anchor_id.is_none());
}

#[test]
fn test_sync_tool_runs_from_spine_adds_missing_transcript_row() {
    let mut app = TuiApp::new();
    app.facade_snapshot.tool_turns.push(ToolTurnSnapshot {
        id: "tool_1".to_string(),
        name: "bash".to_string(),
        parent_message_id: Some("user_1".to_string()),
        phase: ToolTurnPhase::ResultObserved,
        arguments_preview: Some(r#"{"command":"pwd"}"#.to_string()),
        result_preview: Some("/Users/georgexu/Desktop/rust-agent".to_string()),
        failure: None,
    });

    app.sync_tool_runs_from_spine_snapshot();

    let runs = app
        .sync_snapshot
        .tool_runs_for_message("user_1")
        .expect("spine-backed tool run should be inserted");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].name, "bash");
    assert_eq!(runs[0].status, ToolRunStatus::Completed);
    assert_eq!(
        runs[0]
            .arguments
            .as_ref()
            .and_then(|args| args.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("pwd")
    );
    assert!(runs[0]
        .result_preview
        .as_deref()
        .unwrap_or_default()
        .contains("rust-agent"));
}

#[test]
fn test_tool_runs_for_message_prefers_sync_snapshot_projection() {
    let mut app = TuiApp::new();
    app.sync_snapshot.set_tool_runs_for_message(
        "user_1".to_string(),
        vec![ToolRunView::new(
            "legacy_call".to_string(),
            "bash".to_string(),
        )],
    );
    app.sync_snapshot.set_tool_runs_for_message(
        "user_1".to_string(),
        vec![ToolRunView::new(
            "sync_call".to_string(),
            "file_read".to_string(),
        )],
    );

    let runs = app.tool_runs_for_message("user_1").expect("tool runs");

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, "sync_call");
    assert_eq!(runs[0].name, "file_read");
}

#[tokio::test]
async fn test_visible_timeline_messages_projects_active_assistant_parts() {
    let mut app = TuiApp::new();
    app.messages.push(MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "inspect".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.messages.push(MessageItem {
        id: "assistant_1".to_string(),
        role: MessageRole::Assistant,
        content: String::new(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    {
        let mut sync = app.sync_store.lock().await;
        sync.start_turn("user_1".to_string(), "assistant_1".to_string());
        sync.apply_stream_event(&StreamEvent::ThinkingStart);
        sync.apply_stream_event(&StreamEvent::ThinkingChunk("read README".to_string()));
        sync.apply_stream_event(&StreamEvent::ThinkingComplete);
        sync.apply_stream_event(&StreamEvent::TextChunk(
            "This is a Rust project.".to_string(),
        ));
        app.sync_snapshot = sync.snapshot();
    }

    let messages = app.visible_timeline_messages();

    assert_eq!(messages[1].id, "assistant_1");
    assert_eq!(
        messages[1].content,
        "<think>read README</think>\n\nThis is a Rust project."
    );
}

#[test]
fn test_hydrate_persisted_projection_parts_replays_message_parts() {
    let mut app = TuiApp::new();
    app.messages.push(MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "inspect".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.messages.push(MessageItem {
        id: "assistant_1".to_string(),
        role: MessageRole::Assistant,
        content: String::new(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    let parts = vec![
        PersistedPartFixture::new(1, "reasoning", "reasoning_1")
            .message_id("assistant_1")
            .payload(serde_json::json!({"content": "read README"}))
            .build(),
        PersistedPartFixture::new(2, "assistant_text", "text_1")
            .message_id("assistant_1")
            .payload(serde_json::json!({"content": "This is a Rust project."}))
            .build(),
        PersistedPartFixture::new(3, "tool", "tool_call_1")
            .message_id("assistant_1")
            .tool_call_id("call_1")
            .tool_name("bash")
            .status("completed")
            .payload(serde_json::json!({
                "input_args": "{\"command\":\"pwd\"}",
                "result_preview": "/Users/georgexu/Desktop/rust-agent"
            }))
            .build(),
    ];

    let hydration = app.hydrate_persisted_projection_parts("session_1", &parts);
    let messages = app.visible_timeline_messages();
    let runs = app
        .tool_runs_for_message("user_1")
        .expect("tool runs anchored to prior user");

    assert_eq!(hydration.assistant_text_parts, 1);
    assert_eq!(hydration.reasoning_parts, 1);
    assert_eq!(hydration.tool_runs, 1);
    assert_eq!(
        messages[1].content,
        "<think>read README</think>\n\nThis is a Rust project."
    );
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, "call_1");
    assert_eq!(runs[0].name, "bash");
    assert_eq!(runs[0].status, ToolRunStatus::Completed);

    let hydration = app.hydrate_persisted_projection_parts("session_1", &parts);
    assert_eq!(hydration.tool_runs, 1);
    assert_eq!(
        app.tool_runs_for_message("user_1")
            .expect("tool runs after replay")
            .len(),
        1
    );
}

struct PersistedPartFixture {
    part: crate::session_store::PersistedSessionPart,
}

impl PersistedPartFixture {
    fn new(id: i64, kind: &str, part_id: &str) -> Self {
        Self {
            part: crate::session_store::PersistedSessionPart {
                id,
                session_id: "session_1".to_string(),
                part_index: id,
                part_id: part_id.to_string(),
                kind: kind.to_string(),
                tool_call_id: None,
                tool_name: None,
                status: None,
                payload: serde_json::json!({}),
                projected_to_seq: 42,
                updated_at: "2026-06-13T00:00:00Z".to_string(),
                message_id: None,
            },
        }
    }

    fn message_id(mut self, message_id: &str) -> Self {
        self.part.message_id = Some(message_id.to_string());
        self
    }

    fn tool_call_id(mut self, tool_call_id: &str) -> Self {
        self.part.tool_call_id = Some(tool_call_id.to_string());
        self
    }

    fn tool_name(mut self, tool_name: &str) -> Self {
        self.part.tool_name = Some(tool_name.to_string());
        self
    }

    fn status(mut self, status: &str) -> Self {
        self.part.status = Some(status.to_string());
        self
    }

    fn payload(mut self, payload: serde_json::Value) -> Self {
        self.part.payload = payload;
        self
    }

    fn build(self) -> crate::session_store::PersistedSessionPart {
        self.part
    }
}

#[test]
fn test_persisted_final_answer_matches_current_user_turn() {
    let messages = vec![
        MessageItem {
            id: "user_old".to_string(),
            role: MessageRole::User,
            content: "old".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        },
        MessageItem {
            id: "assistant_old".to_string(),
            role: MessageRole::Assistant,
            content: "old answer".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        },
        MessageItem {
            id: "user_new".to_string(),
            role: MessageRole::User,
            content: "run pwd".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        },
        MessageItem {
            id: "assistant_new".to_string(),
            role: MessageRole::Assistant,
            content: "pwd output".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        },
    ];

    assert_eq!(
        persisted_final_answer_for_user(&messages, "run pwd").as_deref(),
        Some("pwd output")
    );
    assert_eq!(
        persisted_final_answer_for_user(&messages, "missing").as_deref(),
        Some("pwd output")
    );
    let error_messages = vec![
        MessageItem {
            id: "user_1".to_string(),
            role: MessageRole::User,
            content: "run pwd".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        },
        MessageItem {
            id: "assistant_error".to_string(),
            role: MessageRole::Assistant,
            content: "[Error: timed out]".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        },
    ];
    assert_eq!(
        persisted_final_answer_for_user(&error_messages, "run pwd"),
        None
    );
}

#[test]
fn test_toggle_collapse_maps_tool_group_anchor_to_parent_message() {
    let mut app = TuiApp::new();
    let user = MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "inspect".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    };
    app.messages.push(user.clone());
    app.messages.push(MessageItem {
        id: "assistant_1".to_string(),
        role: MessageRole::Assistant,
        content: "reply".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.sync_snapshot.set_tool_runs_for_message(
        user.id.clone(),
        vec![ToolRunView::new(
            "tool_1".to_string(),
            "file_read".to_string(),
        )],
    );
    app.scroll_offset = 1;

    assert!(app.toggle_collapse_at_scroll_anchor());

    assert!(app.collapsed_indices.contains(&0));
    assert!(!app.collapsed_indices.contains(&1));
}

#[test]
fn test_toggle_reasoning_uses_current_assistant_anchor() {
    let mut app = TuiApp::new();
    app.messages.push(MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "question".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.messages.push(MessageItem {
        id: "assistant_1".to_string(),
        role: MessageRole::Assistant,
        content: "<think>hidden reasoning</think>\nanswer".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.scroll_offset = 1;

    assert!(app.toggle_reasoning_at_scroll_anchor());
    assert_eq!(
        app.expanded_reasoning_message_id.as_deref(),
        Some("assistant_1")
    );

    assert!(app.toggle_reasoning_at_scroll_anchor());
    assert!(app.expanded_reasoning_message_id.is_none());
}

#[test]
fn test_manual_scroll_anchor_survives_inserted_timeline_items() {
    let mut app = TuiApp::new();
    app.messages.push(MessageItem {
        id: "old_user_1".to_string(),
        role: MessageRole::User,
        content: "first".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.messages.push(MessageItem {
        id: "assistant_1".to_string(),
        role: MessageRole::Assistant,
        content: "reply".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.messages.push(MessageItem {
        id: "old_user_2".to_string(),
        role: MessageRole::User,
        content: "second".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });

    app.jump_to_timeline_target("user");
    assert_eq!(app.scroll_anchor_id.as_deref(), Some("old_user_2"));
    assert_eq!(app.current_timeline_anchor_index(), 2);

    app.messages.insert(
        0,
        MessageItem {
            id: "new_user".to_string(),
            role: MessageRole::User,
            content: "inserted".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        },
    );

    assert_eq!(app.scroll_anchor_id.as_deref(), Some("old_user_2"));
    assert_eq!(app.current_timeline_anchor_index(), 3);
}

#[test]
fn test_scroll_to_message_index_maps_through_timeline_tool_groups() {
    let mut app = TuiApp::new();
    let first_user = MessageItem {
        id: "user_1".to_string(),
        role: MessageRole::User,
        content: "first".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    };
    app.messages.push(first_user.clone());
    app.messages.push(MessageItem {
        id: "assistant_1".to_string(),
        role: MessageRole::Assistant,
        content: "reply".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.messages.push(MessageItem {
        id: "user_2".to_string(),
        role: MessageRole::User,
        content: "second".to_string(),
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        metadata: Default::default(),
    });
    app.sync_snapshot.set_tool_runs_for_message(
        first_user.id,
        vec![ToolRunView::new("tool_1".to_string(), "bash".to_string())],
    );

    assert!(app.scroll_to_message_index(2));

    assert_eq!(app.scroll_offset, 3);
    assert_eq!(app.scroll_anchor_id.as_deref(), Some("user_2"));
    assert!(!app.pinned_to_bottom);
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
        diff_preview: None,
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
        diff_preview: None,
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
    app.sync_snapshot.set_tool_runs_for_message(
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
fn test_visible_sidebar_sessions_filter_matches_model_and_id() {
    let mut app = TuiApp::new();
    app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
    let deepseek = app
        .session_manager
        .start_session("Work", "deepseek-v4-flash")
        .unwrap();
    let _other = app
        .session_manager
        .start_session("Other", "gpt-4o-mini")
        .unwrap();

    app.sidebar_filter = "v4-flash".to_string();
    let sessions = app.visible_sidebar_sessions(10);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, deepseek);

    app.sidebar_filter = deepseek[..8].to_string();
    let sessions = app.visible_sidebar_sessions(10);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, deepseek);
}

#[test]
fn test_toggle_pinned_session_list_pins_unpins_and_respects_limit() {
    let mut pinned = Vec::new();

    assert!(toggle_pinned_session_list(&mut pinned, "sess_a", 2));
    assert_eq!(pinned, vec!["sess_a"]);
    assert!(!toggle_pinned_session_list(&mut pinned, "sess_a", 2));
    assert!(pinned.is_empty());

    assert!(toggle_pinned_session_list(&mut pinned, "sess_a", 2));
    assert!(toggle_pinned_session_list(&mut pinned, "sess_b", 2));
    assert!(!toggle_pinned_session_list(&mut pinned, "sess_c", 2));
    assert_eq!(pinned, vec!["sess_a", "sess_b"]);
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
    app.sync_snapshot
        .set_tool_runs_for_message("user_1".to_string(), vec![first.clone(), second.clone()]);

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
    app.sync_snapshot
        .set_tool_runs_for_message("user_1".to_string(), vec![first]);

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
    app.sync_snapshot
        .set_tool_runs_for_message("user_1".to_string(), vec![run]);
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
    let mut app = TuiApp::with_engine(Some(engine.clone()), None, None);
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
        diff_preview: None,
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
    let mut app = TuiApp::with_engine(Some(engine.clone()), None, None);
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
        diff_preview: None,
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
        diff_preview: None,
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
        diff_preview: None,
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
        diff_preview: None,
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
        diff_preview: None,
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
        diff_preview: None,
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
        diff_preview: None,
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
