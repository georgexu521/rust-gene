use super::*;

fn tool_run(
    name: &str,
    status: crate::tui::tool_view::ToolRunStatus,
    command: &str,
) -> crate::tui::tool_view::ToolRunView {
    crate::tui::tool_view::ToolRunView {
        id: "id-1".to_string(),
        name: name.to_string(),
        args_buffer: String::new(),
        arguments: Some(serde_json::json!({"command": command})),
        status,
        progress: Vec::new(),
        result_body: None,
        result_preview: None,
        result_data: None,
        metadata: None,
        started_at: std::time::Instant::now(),
        completed_at: None,
    }
}

#[test]
fn parse_export_format_accepts_aliases() {
    assert!(parse_export_format("json").is_some());
    assert!(parse_export_format("md").is_some());
    assert!(parse_export_format("markdown").is_some());
    assert!(parse_export_format("").is_some()); // default
    assert!(parse_export_format("xml").is_none());
    assert!(parse_export_format("PDF").is_none());
}

#[test]
fn parse_export_privacy_accepts_modes() {
    assert!(parse_export_privacy("full").is_some());
    assert!(parse_export_privacy("redacted").is_some());
    assert!(parse_export_privacy("summary").is_some());
    assert!(parse_export_privacy("").is_some()); // default
    assert!(parse_export_privacy("secret").is_none());
}

#[test]
fn diagnostic_failed_tool_names_collects_failures() {
    let runs = vec![
        tool_run(
            "file_read",
            crate::tui::tool_view::ToolRunStatus::Completed,
            "",
        ),
        tool_run(
            "bash",
            crate::tui::tool_view::ToolRunStatus::Failed,
            "cargo test",
        ),
        tool_run(
            "file_write",
            crate::tui::tool_view::ToolRunStatus::TimedOut,
            "",
        ),
        tool_run(
            "bash",
            crate::tui::tool_view::ToolRunStatus::Cancelled,
            "npm test",
        ),
    ];
    let names = diagnostic_failed_tool_names(&runs);
    assert_eq!(names.len(), 2); // deduped
    assert!(names.contains(&"bash".to_string()));
    assert!(names.contains(&"file_write".to_string()));
}

#[test]
fn diagnostic_validation_status_with_changes_and_passing() {
    let runs = vec![tool_run(
        "bash",
        crate::tui::tool_view::ToolRunStatus::Completed,
        "cargo test -q",
    )];
    let status = diagnostic_validation_status(&runs, true);
    assert_eq!(status.as_deref(), Some("verified"));
}

#[test]
fn diagnostic_validation_status_without_changes() {
    let runs = vec![tool_run(
        "bash",
        crate::tui::tool_view::ToolRunStatus::Completed,
        "cargo test -q",
    )];
    let status = diagnostic_validation_status(&runs, false);
    // No file changes: should report no-diff pass
    assert!(status.is_some());
}

#[test]
fn tool_run_looks_like_validation_detects_cargo_test() {
    let run = tool_run(
        "bash",
        crate::tui::tool_view::ToolRunStatus::Completed,
        "cargo test -q",
    );
    assert!(tool_run_looks_like_validation(&run));
}

#[test]
fn tool_run_looks_like_validation_detects_npm_test() {
    let run = tool_run(
        "bash",
        crate::tui::tool_view::ToolRunStatus::Completed,
        "npm test",
    );
    assert!(tool_run_looks_like_validation(&run));
}

#[test]
fn tool_run_looks_like_validation_rejects_echo() {
    let run = tool_run(
        "bash",
        crate::tui::tool_view::ToolRunStatus::Completed,
        "echo hello",
    );
    assert!(!tool_run_looks_like_validation(&run));
}
