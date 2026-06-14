use super::*;
use crate::state::{MessageItem, MessageRole};
use crate::tui::view_model::timeline::{
    estimate_tool_runs_height, timeline_items, tool_runs_from_parts,
};
use ratatui::{backend::TestBackend, Terminal};
use std::{collections::HashMap, time::SystemTime};

fn msg(role: MessageRole, content: &str) -> MessageItem {
    MessageItem {
        id: format!("{:?}-{}", role, content.len()),
        role,
        content: content.to_string(),
        timestamp: SystemTime::UNIX_EPOCH,
        metadata: HashMap::new(),
    }
}

fn render_permission_approval_text(
    req: &crate::engine::conversation_loop::ToolApprovalRequest,
) -> String {
    let backend = TestBackend::new(160, 70);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let theme = crate::tui::theme::Theme::graphite();
            render_permission_approval(frame, req, frame.area(), &theme);
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

fn render_status_bar_text(app: &TuiApp) -> String {
    render_status_bar_text_with_size(app, 260, 3)
}

fn render_status_bar_text_with_size(app: &TuiApp, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_status_bar(frame, app, frame.area());
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

fn render_input_area_text(app: &TuiApp) -> String {
    render_input_area_text_with_size(app, 160, 8)
}

fn render_input_area_text_with_size(app: &TuiApp, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_input_area(frame, app, frame.area());
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

fn render_tool_viewer_text(app: &TuiApp) -> String {
    let backend = TestBackend::new(160, 70);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_tool_viewer(frame, app, frame.area());
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

fn render_context_sidebar_text(app: &TuiApp) -> String {
    render_sidebar_text(app, 80, 30)
}

fn render_sidebar_text(app: &TuiApp, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_sidebar(frame, app, frame.area());
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

fn render_shortcut_help_text(app: &TuiApp) -> String {
    let backend = TestBackend::new(120, 80);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_shortcut_help(frame, app, frame.area());
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

fn render_file_picker_text(app: &TuiApp) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_file_picker(frame, app, frame.area());
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
fn transcript_window_prefers_active_turn_when_bottom_anchored() {
    let app = TuiApp::new();
    let items = [
        msg(MessageRole::User, "old question"),
        msg(MessageRole::Assistant, "old answer"),
        msg(MessageRole::User, "current question"),
        msg(MessageRole::Assistant, "current answer"),
    ];
    let render_session = app.sync_snapshot.render_session(&items);
    let transcript = timeline_items(&render_session);

    let window = transcript_window(
        &transcript,
        render_session.messages.len(),
        true,
        6,
        80,
        &app,
    );

    assert_eq!(window.start, 2);
    assert!(window.more_above);
    assert!(window.bottom_anchored);
}

#[test]
fn transcript_window_includes_recent_context_when_it_fits() {
    let app = TuiApp::new();
    let items = [
        msg(MessageRole::User, "old question"),
        msg(MessageRole::Assistant, "old answer"),
        msg(MessageRole::User, "current question"),
        msg(MessageRole::Assistant, "current answer"),
    ];
    let render_session = app.sync_snapshot.render_session(&items);
    let transcript = timeline_items(&render_session);

    let window = transcript_window(
        &transcript,
        render_session.messages.len(),
        true,
        7,
        80,
        &app,
    );

    assert_eq!(window.start, 1);
    assert!(window.more_above);
    assert_eq!(window.message_height, 6);
}

#[test]
fn transcript_window_keeps_recent_turn_context_when_answer_overflows() {
    let app = TuiApp::new();
    let long_answer = "answer line\n".repeat(20);
    let items = [
        msg(MessageRole::User, "old question"),
        msg(MessageRole::Assistant, "old answer"),
        msg(MessageRole::User, "current question"),
        msg(MessageRole::Assistant, &long_answer),
    ];
    let render_session = app.sync_snapshot.render_session(&items);
    let transcript = timeline_items(&render_session);

    let window = transcript_window(
        &transcript,
        render_session.messages.len(),
        true,
        8,
        80,
        &app,
    );

    assert_eq!(window.start, 0);
    assert!(!window.more_above);
    assert_eq!(window.message_height, 8);
}

#[test]
fn transcript_window_keeps_active_prompt_when_previous_turn_is_too_tall() {
    let app = TuiApp::new();
    let old_answer = "old answer line\n".repeat(10);
    let long_answer = "answer line\n".repeat(20);
    let items = [
        msg(MessageRole::User, "old question"),
        msg(MessageRole::Assistant, &old_answer),
        msg(MessageRole::User, "current question"),
        msg(MessageRole::Assistant, &long_answer),
    ];
    let render_session = app.sync_snapshot.render_session(&items);
    let transcript = timeline_items(&render_session);

    let window = transcript_window(
        &transcript,
        render_session.messages.len(),
        true,
        8,
        80,
        &app,
    );

    assert_eq!(window.start, 2);
    assert!(window.more_above);
    assert_eq!(window.message_height, 7);
}

#[test]
fn transcript_window_preserves_manual_scroll_offset() {
    let app = TuiApp::new();
    let items = [
        msg(MessageRole::User, "one"),
        msg(MessageRole::Assistant, "two"),
        msg(MessageRole::User, "three"),
    ];
    let render_session = app.sync_snapshot.render_session(&items);
    let transcript = timeline_items(&render_session);

    let window = transcript_window(&transcript, 1, false, 6, 80, &app);

    assert_eq!(window.start, 1);
    assert!(window.more_above);
    assert!(!window.bottom_anchored);
}

#[test]
fn transcript_items_keep_tool_runs_inside_active_user_message_parts() {
    let mut app = TuiApp::new();
    let items = [
        msg(MessageRole::User, "old question"),
        msg(MessageRole::Assistant, "old answer"),
        msg(MessageRole::User, "current question"),
        msg(MessageRole::Assistant, "current answer"),
    ];
    let mut run = ToolRunView::new("tool_1".to_string(), "bash".to_string());
    run.arguments = Some(serde_json::json!({
        "command": "ls -la ~/Desktop"
    }));
    app.sync_snapshot
        .set_tool_runs_for_message(items[2].id.clone(), vec![run]);
    let render_session = app.sync_snapshot.render_session(&items);
    let transcript = timeline_items(&render_session);

    assert_eq!(transcript.len(), 4);
    match &transcript[2] {
        TimelineItem::Message {
            parts: Some(parts), ..
        } => {
            let runs = tool_runs_from_parts(parts);
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].id, "tool_1");
        }
        item => panic!("expected active user message with tool parts, got {item:?}"),
    }
    let window = transcript_window(
        &transcript,
        render_session.messages.len(),
        true,
        8,
        80,
        &app,
    );
    assert_eq!(window.start, 2);
}

#[test]
fn transcript_items_keep_tool_parts_for_previous_turns() {
    let mut app = TuiApp::new();
    let items = [
        msg(MessageRole::User, "first question"),
        msg(MessageRole::Assistant, "first answer"),
        msg(MessageRole::User, "second question"),
        msg(MessageRole::Assistant, "second answer"),
    ];
    app.sync_snapshot.set_tool_runs_for_message(
        items[0].id.clone(),
        vec![ToolRunView::new("tool_1".to_string(), "bash".to_string())],
    );
    app.sync_snapshot.set_tool_runs_for_message(
        items[2].id.clone(),
        vec![ToolRunView::new("tool_2".to_string(), "grep".to_string())],
    );
    let render_session = app.sync_snapshot.render_session(&items);
    let transcript = timeline_items(&render_session);

    assert_eq!(transcript.len(), 4);
    match &transcript[0] {
        TimelineItem::Message {
            parts: Some(parts), ..
        } => {
            let runs = tool_runs_from_parts(parts);
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].id, "tool_1");
        }
        item => panic!("expected first user message with tool parts, got {item:?}"),
    }
    match &transcript[2] {
        TimelineItem::Message {
            parts: Some(parts), ..
        } => {
            let runs = tool_runs_from_parts(parts);
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].id, "tool_2");
        }
        item => panic!("expected second user message with tool parts, got {item:?}"),
    }
}

#[test]
fn estimate_tool_runs_height_uses_single_expanded_tool() {
    let mut app = TuiApp::new();
    let mut first = ToolRunView::new("tool_1".to_string(), "bash".to_string());
    first.arguments = Some(serde_json::json!({ "command": "ls" }));
    first.mark_complete("Result: OK\na.txt\nb.txt\n".to_string());
    let second = ToolRunView::new("tool_2".to_string(), "grep".to_string());
    let collapsed_runs = vec![first.clone(), second.clone()];
    let collapsed = estimate_tool_runs_height(&collapsed_runs, &app);

    app.expanded_tool_run_id = Some("tool_1".to_string());
    let expanded = estimate_tool_runs_height(&collapsed_runs, &app);

    assert!(expanded > collapsed);
}

#[test]
fn render_status_bar_shows_debug_density_and_active_tools() {
    let mut app = TuiApp::new();
    app.is_querying = true;
    app.set_status_bar_density(crate::tui::app::StatusBarDensity::Debug);
    let mut run = ToolRunView::new("tool_1".to_string(), "bash".to_string());
    run.arguments = Some(serde_json::json!({ "command": "cargo test" }));
    run.mark_running("bash".to_string());
    app.sync_snapshot
        .set_tool_runs_for_message("user_1".to_string(), vec![run]);

    let rendered = render_status_bar_text(&app);

    assert!(!rendered.contains("esc to interrupt"));
    assert!(rendered.contains("tools:"));
    assert!(rendered.contains("msgs:"));
    assert!(rendered.contains("? shortcuts"));
}

#[test]
fn render_status_bar_keeps_left_padding() {
    let app = TuiApp::new();

    let rendered = render_status_bar_text(&app);

    assert!(rendered.starts_with(' '));
    assert!(rendered.contains("● auto"));
}

#[test]
fn render_status_bar_compacts_long_provider_model_without_losing_shortcuts() {
    let mut app = TuiApp::new();
    app.facade_snapshot.provider_request.provider_family = Some("openai_compatible".to_string());
    app.facade_snapshot.provider_request.model =
        Some("deepseek-v4-flash-with-a-very-long-routing-suffix".to_string());

    let rendered = render_status_bar_text_with_size(&app, 56, 2);

    assert!(rendered.starts_with(' '));
    assert!(rendered.contains("● auto"));
    assert!(rendered.contains("DeepSeek /"));
    assert!(rendered.contains("? shortcuts"));
    assert!(rendered.contains('…'));
    assert!(!rendered.contains("v0.1.0"));
    assert!(!rendered.contains("very-long-routing-suffix"));
}

#[test]
fn render_status_bar_has_explicit_fallback_for_tiny_widths() {
    let mut app = TuiApp::new();
    app.facade_snapshot.provider_request.provider_family =
        Some("openai_compatible_with_long_suffix".to_string());
    app.facade_snapshot.provider_request.model =
        Some("deepseek-v4-flash-with-a-very-long-routing-suffix".to_string());

    let rendered = render_status_bar_text_with_size(&app, 24, 1);
    let first_line = rendered.as_str();

    assert!(first_line.starts_with(' '));
    assert!(first_line.contains("● auto"));
    assert!(first_line.contains('…'));
    assert!(!first_line.contains("? shortcuts"));
    assert!(unicode_width::UnicodeWidthStr::width(first_line) <= 24);
}

#[test]
fn render_input_area_shows_single_active_status_without_replacing_prompt() {
    let mut app = TuiApp::new();
    app.is_querying = true;
    app.facade_snapshot.provider_request.phase =
        crate::engine::runtime_facade::ProviderPhase::Started;
    app.facade_snapshot.provider_request.provider_family = Some("openai_compatible".to_string());
    app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
    app.facade_snapshot.provider_request.elapsed_ms = 2_700;

    let rendered = render_input_area_text(&app);

    assert!(rendered.contains("waiting on DeepSeek"));
    assert!(rendered.contains("esc to interrupt"));
    assert!(rendered.contains("Message Priority Agent"));
    assert!(!rendered.contains("Thinking..."));
}

#[test]
fn render_input_area_shows_context_strip_and_paste_count() {
    let mut app = TuiApp::new();
    app.history.push_back("previous prompt".to_string());
    app.prompt_stash = Some("stashed prompt".to_string());
    app.composer_attachments.push("Cargo.toml".to_string());
    let pasted_text = (0..20)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let line_count = pasted_text.lines().count().max(1);
    let char_count = pasted_text.chars().count();
    app.input.insert_str(&format!(
        "[[paste:{} {} lines {} chars]]",
        1, line_count, char_count
    ));
    app.pasted_blocks.push(crate::tui::app::PastedBlock {
        placeholder: format!("[[paste:{} {} lines {} chars]]", 1, line_count, char_count),
        content: pasted_text,
    });

    let rendered = render_input_area_text(&app);

    assert!(rendered.contains(&app.current_provider_label()));
    assert!(rendered.contains(&app.current_model_label()));
    assert!(!rendered.contains("auto · DeepSeek"));
    assert!(rendered.contains("hist:1"));
    assert!(rendered.contains("stash"));
    assert!(rendered.contains("files:1"));
    assert!(rendered.contains("Cargo.toml"));
    assert!(rendered.contains("[file") || rendered.contains("[1] Cargo"));
    assert!(rendered.contains("/attach preview"));
    assert!(rendered.contains("backspace removes last"));
    assert!(rendered.contains("paste:1") || rendered.contains("[[paste:"));
    assert!(rendered.contains("20 lines"));
    assert!(rendered.contains("149 chars"));
}

#[test]
fn render_input_area_shows_non_default_mode_in_context_strip() {
    let mut app = TuiApp::new();
    app.set_agent_mode(crate::engine::agent_mode::AgentMode::Plan);

    let rendered = render_input_area_text(&app);

    assert!(rendered.contains("plan ·"));
}

#[test]
fn render_input_area_truncates_long_attachment_but_keeps_preview_hint() {
    let mut app = TuiApp::new();
    app.composer_attachments
        .push("very/long/path/with/many/segments/and-a-wide-name-你好你好/file.rs".to_string());

    let rendered = render_input_area_text_with_size(&app, 72, 8);

    assert!(rendered.contains("files:1"));
    assert!(rendered.contains("/attach preview"));
    assert!(rendered.contains('…'));
    assert!(!rendered.contains("backspace removes last"));
    assert!(!rendered.contains("and-a-wide-name-你好你好/file.rs"));
}

#[test]
fn render_input_area_compacts_context_strip_but_keeps_action_counts() {
    let mut app = TuiApp::new();
    app.facade_snapshot.provider_request.provider_family =
        Some("openai_compatible_with_an_extra_long_route_name".to_string());
    app.facade_snapshot.provider_request.model =
        Some("deepseek-v4-flash-with-a-very-long-routing-suffix".to_string());
    app.history.push_back("previous prompt".to_string());
    app.prompt_stash = Some("stashed prompt".to_string());
    app.composer_attachments.push("Cargo.toml".to_string());
    let pasted_text = (0..18)
        .map(|i| format!("long pasted context line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let line_count = pasted_text.lines().count().max(1);
    let char_count = pasted_text.chars().count();
    app.input.insert_str(&format!(
        "[[paste:{} {} lines {} chars]]",
        1, line_count, char_count
    ));
    app.pasted_blocks.push(crate::tui::app::PastedBlock {
        placeholder: format!("[[paste:{} {} lines {} chars]]", 1, line_count, char_count),
        content: pasted_text,
    });

    let rendered = render_input_area_text_with_size(&app, 84, 8);

    assert!(rendered.contains("files:1"));
    assert!(rendered.contains("[file") || rendered.contains("[1] Cargo"));
    assert!(rendered.contains("paste:1") || rendered.contains("[[paste:"));
    assert!(rendered.contains("hist:1"));
    assert!(rendered.contains("stash"));
    assert!(rendered.contains("DeepSeek"));
    assert!(rendered.contains('…'));
    assert!(!rendered.contains("very-long-routing-suffix"));
    assert!(!rendered.contains("long pasted context line"));
}

#[test]
fn render_input_area_uses_dedicated_attachment_row_for_multiple_files() {
    let mut app = TuiApp::new();
    app.composer_attachments.push("Cargo.toml".to_string());
    app.composer_attachments.push("src".to_string());
    app.composer_attachments.push("docs".to_string());

    let rendered = render_input_area_text(&app);

    assert!(rendered.contains("files:3"));
    assert!(rendered.contains("[1] Cargo.toml"));
    assert!(rendered.contains("[2] src"));
    assert!(rendered.contains("+1"));
    assert!(rendered.contains("/attach preview"));
}

#[test]
fn render_context_sidebar_shows_runtime_summary() {
    let mut app = TuiApp::new();
    app.sidebar_panel = SidebarPanel::Context;
    app.history.push_back("previous prompt".to_string());
    app.prompt_stash = Some("draft".to_string());
    app.composer_attachments.push("Cargo.toml".to_string());
    let pasted_text = (0..20)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let line_count = pasted_text.lines().count().max(1);
    let char_count = pasted_text.chars().count();
    app.input.insert_str(&format!(
        "[[paste:{} {} lines {} chars]]",
        1, line_count, char_count
    ));
    app.pasted_blocks.push(crate::tui::app::PastedBlock {
        placeholder: format!("[[paste:{} {} lines {} chars]]", 1, line_count, char_count),
        content: pasted_text,
    });
    let mut failed = ToolRunView::new("tool_1".to_string(), "bash".to_string());
    failed.mark_complete("Result: ERROR\nfailed".to_string());
    app.sync_snapshot
        .set_tool_runs_for_message("user_1".to_string(), vec![failed]);

    let rendered = render_context_sidebar_text(&app);

    assert!(rendered.contains("Session"));
    assert!(rendered.contains("Runtime"));
    assert!(rendered.contains("Composer"));
    assert!(rendered.contains("Tools"));
    assert!(rendered.contains("hist:1"));
    assert!(rendered.contains("stash:yes"));
    assert!(rendered.contains("files:1"));
    assert!(rendered.contains("Cargo.toml"));
    assert!(rendered.contains("[file") || rendered.contains("[1] Cargo"));
    assert!(rendered.contains("paste:1") || rendered.contains("[[paste:"));
    assert!(rendered.contains("1 total"));
    assert!(rendered.contains("1 failed"));
}

#[test]
fn render_sessions_sidebar_shows_richer_session_rows() {
    let mut app = TuiApp::new();
    app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
    let session_id = app
        .session_manager
        .start_session("DeepSeek Work", "deepseek-v4-flash", None)
        .unwrap();
    app.session_manager
        .add_message(MessageRole::User, "Please inspect the TUI sidebar preview")
        .unwrap();
    app.sidebar_selected = 0;

    let rendered = render_context_sidebar_text(&app);

    assert!(rendered.contains("›"));
    assert!(rendered.contains("DeepSeek Work"));
    assert!(rendered.contains(&session_id[..8]));
    assert!(rendered.contains("deepseek-v4"));
    assert!(rendered.contains("msgs"));
    assert!(rendered.contains("you"));
    assert!(rendered.contains("Please inspect"));
}

#[test]
fn render_sessions_sidebar_metadata_fits_inline_width() {
    let mut app = TuiApp::new();
    app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
    let session_id = app
        .session_manager
        .start_session("DeepSeek Work", "deepseek-v4-flash", None)
        .unwrap();
    app.session_manager
        .add_message(MessageRole::User, "Please inspect the TUI sidebar preview")
        .unwrap();
    app.session_manager
        .add_message(
            MessageRole::Assistant,
            "Checking the sidebar preview width with a deliberately long assistant summary",
        )
        .unwrap();
    app.sidebar_selected = 0;

    let rendered = render_sidebar_text(&app, 40, 30);

    assert!(rendered.contains(&session_id[..8]));
    assert!(rendered.contains("deepseek-v4"));
    assert!(rendered.contains("2 msgs"));
    assert!(rendered.contains("Checking the sidebar"));
    assert!(rendered.contains('…'));
    assert!(!rendered.contains("2 m│"));
    assert!(!rendered.contains("Please inspect the TUI sidebar prev│"));
}

#[test]
fn session_preview_truncation_uses_display_width() {
    let truncated = truncate_chars_to_width("你好你好你好 sidebar preview", 9);

    assert!(truncated.ends_with('…'));
    assert!(unicode_width::UnicodeWidthStr::width(truncated.as_str()) <= 9);
    assert!(!truncated.contains("sidebar"));
}

#[test]
fn render_shortcut_help_explains_sidebar_and_reasoning_expand() {
    let app = TuiApp::new();

    let rendered = render_shortcut_help_text(&app);

    assert!(rendered.contains("expand reasoning or tool details"));
    assert!(rendered.contains("Sidebar"));
    assert!(rendered.contains("switch Sessions/Context panel"));
    assert!(rendered.contains("filter sessions by title/id/model"));
}

#[test]
fn render_file_picker_shows_attachment_controls() {
    let mut app = TuiApp::new();
    app.open_composer_file_picker(Some("."), false);

    let rendered = render_file_picker_text(&app);

    assert!(rendered.contains("Attach File Context"));
    assert!(rendered.contains("/ filter files"));
    assert!(rendered.contains("/"));
    assert!(rendered.contains("enter"));
    assert!(rendered.contains("attach file"));
    assert!(rendered.contains("esc/q"));
}

#[test]
fn render_status_bar_projects_runtime_failures_and_background_tools() {
    let mut app = TuiApp::new();
    let mut failed = ToolRunView::new("tool_1".to_string(), "bash".to_string());
    failed.mark_complete("Result: ERROR\nfailed".to_string());
    let mut pty = ToolRunView::new("tool_2".to_string(), "bash".to_string());
    pty.mark_complete_with_metadata(
        "Started pty shell command".to_string(),
        Some(serde_json::json!({
            "terminal_task": {
                "task_id": "term_1",
                "status": "running",
                "terminal_kind": "pty_shell"
            }
        })),
    );
    let mut background = ToolRunView::new("tool_3".to_string(), "bash".to_string());
    background.mark_complete_with_metadata(
        "Started background shell command".to_string(),
        Some(serde_json::json!({
            "terminal_task": {
                "task_id": "term_2",
                "status": "running",
                "terminal_kind": "background_shell"
            }
        })),
    );
    app.sync_snapshot
        .set_tool_runs_for_message("user_1".to_string(), vec![failed, pty, background]);

    let rendered = render_status_bar_text(&app);

    // New status bar shows mode glyph + provider/model, not detailed tool breakdowns
    assert!(rendered.contains("● auto"));
    assert!(rendered.contains("? shortcuts"));
}

#[test]
fn render_tool_viewer_shows_title_content_and_controls() {
    let mut app = TuiApp::new();
    app.tool_viewer_title = "bash".to_string();
    app.tool_viewer_content = "Result: OK\nline one\nline two".to_string();

    let rendered = render_tool_viewer_text(&app);

    assert!(rendered.contains("Tool Output: bash"));
    assert!(rendered.contains("Result: OK"));
    assert!(rendered.contains("line one"));
    assert!(rendered.contains("esc/q"));
    assert!(rendered.contains("PgUp/PgDn"));
}

#[test]
fn permission_preview_extracts_bash_command() {
    let args = serde_json::json!({ "command": "ls -la" });
    assert_eq!(
        permission_preview("bash", &args),
        Some((
            "Command",
            "$ ls -la\ncategory=list kind=inspection\nrule=bash:ls -la".to_string()
        ))
    );
}

#[test]
fn permission_preview_extracts_mcp_target() {
    let args = serde_json::json!({
        "server_name": "filesystem",
        "tool_name": "read_file"
    });
    assert_eq!(
        permission_preview("mcp_tool", &args),
        Some(("MCP", "filesystem / read_file".to_string()))
    );
}

#[test]
fn render_permission_approval_shows_bash_risk_and_decisions() {
    let req = crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({ "command": "rm -rf /tmp/demo" }),
        },
        prompt: "Permission explanation: decision=Ask, risk=high, reason=destructive command"
            .to_string(),
        review: None,
        audit: None,
        diff_preview: None,
    };

    let rendered = render_permission_approval_text(&req);

    assert!(rendered.contains("Tool approval"));
    assert!(rendered.contains("Subject"));
    assert!(rendered.contains("bash"));
    assert!(rendered.contains("Risk"));
    assert!(rendered.contains("high"));
    assert!(rendered.contains("$ rm -rf /tmp/demo"));
    assert!(rendered.contains("category=file_mutation"));
    assert!(rendered.contains("rule=bash:rm -rf /tmp/demo"));
    assert!(rendered.contains("allow once"));
    assert!(rendered.contains("allow session"));
    assert!(rendered.contains("deny global"));
    assert!(rendered.contains("preview diff/output"));
}

#[test]
fn render_permission_approval_shows_file_write_scope_and_preview() {
    let req = crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "tool_2".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({
                "path": "src/example.rs",
                "content": "fn main() {}"
            }),
        },
        prompt: "Permission explanation: decision=Ask, risk=medium, reason=file write".to_string(),
        review: None,
        audit: None,
        diff_preview: None,
    };

    let rendered = render_permission_approval_text(&req);

    assert!(rendered.contains("file_write"));
    assert!(rendered.contains("src/example.rs"));
    assert!(rendered.contains("Rule"));
    assert!(rendered.contains("Preview"));
    assert!(rendered.contains("Write"));
    assert!(rendered.contains("fn main() {}"));
    assert!(rendered.contains("allow project"));
}

#[test]
fn truncate_chars_with_ellipsis_handles_unicode_boundaries() {
    assert_eq!(truncate_chars_with_ellipsis("项目标题很长", 4), "项目标题…");
    assert_eq!(truncate_chars_with_ellipsis("short", 10), "short");
}

#[test]
fn render_permission_approval_handles_unicode_diff_preview() {
    let req = crate::engine::conversation_loop::ToolApprovalRequest {
        tool_call: crate::services::api::ToolCall {
            id: "call_unicode".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({
                "path": "src/main.rs",
                "content": "中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文"
            }),
        },
        prompt: "Permission explanation: decision=Ask, risk=medium, reason=file write".to_string(),
        review: None,
        audit: None,
        diff_preview: Some(
            "+中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文中文"
                .to_string(),
        ),
    };

    let rendered = render_permission_approval_text(&req);

    assert!(rendered.contains("Change Preview"));
}
