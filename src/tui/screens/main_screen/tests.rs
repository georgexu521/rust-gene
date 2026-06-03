use super::*;
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
    let backend = TestBackend::new(260, 3);
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

#[test]
fn transcript_window_prefers_active_turn_when_bottom_anchored() {
    let app = TuiApp::new();
    let items = [
        msg(MessageRole::User, "old question"),
        msg(MessageRole::Assistant, "old answer"),
        msg(MessageRole::User, "current question"),
        msg(MessageRole::Assistant, "current answer"),
    ];
    let refs: Vec<_> = items.iter().collect();
    let transcript = transcript_items(&refs, &app);

    let window = transcript_window(&transcript, refs.len(), true, 6, 80, &app);

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
    let refs: Vec<_> = items.iter().collect();
    let transcript = transcript_items(&refs, &app);

    let window = transcript_window(&transcript, refs.len(), true, 7, 80, &app);

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
    let refs: Vec<_> = items.iter().collect();
    let transcript = transcript_items(&refs, &app);

    let window = transcript_window(&transcript, refs.len(), true, 8, 80, &app);

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
    let refs: Vec<_> = items.iter().collect();
    let transcript = transcript_items(&refs, &app);

    let window = transcript_window(&transcript, refs.len(), true, 8, 80, &app);

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
    let refs: Vec<_> = items.iter().collect();
    let transcript = transcript_items(&refs, &app);

    let window = transcript_window(&transcript, 1, false, 6, 80, &app);

    assert_eq!(window.start, 1);
    assert!(window.more_above);
    assert!(!window.bottom_anchored);
}

#[test]
fn transcript_items_insert_tool_runs_after_active_user() {
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
    app.tool_runs_by_message_id
        .insert(items[2].id.clone(), vec![run]);
    let refs: Vec<_> = items.iter().collect();

    let transcript = transcript_items(&refs, &app);

    assert_eq!(transcript.len(), 5);
    assert!(matches!(transcript[3], TranscriptItem::ToolRuns(_)));
    let window = transcript_window(&transcript, refs.len(), true, 8, 80, &app);
    assert_eq!(window.start, 2);
}

#[test]
fn transcript_items_keep_tool_runs_for_previous_turns() {
    let mut app = TuiApp::new();
    let items = [
        msg(MessageRole::User, "first question"),
        msg(MessageRole::Assistant, "first answer"),
        msg(MessageRole::User, "second question"),
        msg(MessageRole::Assistant, "second answer"),
    ];
    app.tool_runs_by_message_id.insert(
        items[0].id.clone(),
        vec![ToolRunView::new("tool_1".to_string(), "bash".to_string())],
    );
    app.tool_runs_by_message_id.insert(
        items[2].id.clone(),
        vec![ToolRunView::new("tool_2".to_string(), "grep".to_string())],
    );
    let refs: Vec<_> = items.iter().collect();

    let transcript = transcript_items(&refs, &app);

    assert_eq!(transcript.len(), 6);
    assert!(matches!(transcript[1], TranscriptItem::ToolRuns(_)));
    assert!(matches!(transcript[4], TranscriptItem::ToolRuns(_)));
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
    app.tool_runs_snapshot.push(run);

    let rendered = render_status_bar_text(&app);

    assert!(rendered.contains("esc to interrupt"));
    assert!(rendered.contains("tools:"));
    assert!(rendered.contains("msgs:"));
    assert!(rendered.contains("? shortcuts"));
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
    app.tool_runs_snapshot.push(failed);
    app.tool_runs_snapshot.push(pty);
    app.tool_runs_snapshot.push(background);

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
