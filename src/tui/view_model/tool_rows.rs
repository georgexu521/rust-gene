use crate::{
    engine::runtime_facade::{ToolTurnPhase, ToolTurnSnapshot},
    tui::tool_view::{ToolRunStatus, ToolRunView},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRowSeverity {
    Muted,
    Success,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRow {
    pub id: String,
    pub icon: &'static str,
    pub summary: String,
    pub detail: Option<String>,
    pub preview: Option<String>,
    pub status_label: &'static str,
    pub severity: ToolRowSeverity,
    pub expandable: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRowsView {
    pub rows: Vec<ToolRow>,
    pub hidden_routine_count: usize,
}

pub fn tool_row_lines(row: &ToolRow, expanded: bool, run: &ToolRunView) -> Vec<String> {
    let mut lines = Vec::new();
    let mut title = format!("{} · {}", row.summary, row.status_label);
    if row.expandable && !expanded {
        title.push_str(" · ctrl+o details");
    }
    lines.push(title);
    if let Some(detail) = &row.detail {
        lines.push(format!("  {}", detail));
    }
    if let Some(preview) = &row.preview {
        lines.push(format!("  {}", preview));
    }
    if expanded {
        let expanded_lines = run.render_lines(true);
        lines.extend(expanded_lines.into_iter().skip(1));
    }
    lines
}

pub fn tool_row_height(row: &ToolRow, expanded: bool, run: &ToolRunView) -> usize {
    tool_row_lines(row, expanded, run).len()
}

pub fn tool_rows_for_runs(runs: &[ToolRunView], terminal_width: usize) -> ToolRowsView {
    tool_rows_for_runs_with_spine(runs, &[], terminal_width)
}

pub fn tool_rows_for_runs_with_spine(
    runs: &[ToolRunView],
    tool_turns: &[ToolTurnSnapshot],
    terminal_width: usize,
) -> ToolRowsView {
    let rows = runs
        .iter()
        .map(|run| {
            let turn = tool_turns.iter().rev().find(|turn| turn.id == run.id);
            tool_row_for_run_with_spine(run, turn, terminal_width)
        })
        .collect::<Vec<_>>();
    let hidden_routine_count = rows.iter().filter(|row| !row.visible).count();
    ToolRowsView {
        rows,
        hidden_routine_count,
    }
}

pub fn tool_row_for_run(run: &ToolRunView, terminal_width: usize) -> ToolRow {
    tool_row_for_run_with_spine(run, None, terminal_width)
}

pub fn tool_row_for_run_with_spine(
    run: &ToolRunView,
    turn: Option<&ToolTurnSnapshot>,
    terminal_width: usize,
) -> ToolRow {
    let severity = severity_for_status(run.status);
    let severity = turn
        .map(|turn| severity_for_phase(turn.phase))
        .unwrap_or(severity);
    let visible = turn
        .map(|turn| should_show_turn(run, turn))
        .unwrap_or_else(|| should_show_run(run));
    let expandable =
        run.result_body.is_some() || !run.progress.is_empty() || run.arguments.is_some();
    let preview_limit = terminal_width.saturating_sub(16).clamp(48, 140);
    let preview = turn
        .and_then(|turn| preview_for_turn(turn, preview_limit))
        .or_else(|| preview_for_run(run, preview_limit, severity));

    ToolRow {
        id: run.id.clone(),
        icon: turn
            .map(|turn| icon_for_phase(turn.phase))
            .unwrap_or_else(|| icon_for_status(run.status)),
        summary: trim_status_noise(run.summary()),
        detail: run.detail_line().map(|line| trim_tree_prefix(&line)),
        preview,
        status_label: turn
            .map(|turn| status_label_for_phase(turn.phase))
            .unwrap_or_else(|| status_label(run.status)),
        severity,
        expandable,
        visible,
    }
}

pub fn collapse_output_preview(text: &str, max_lines: usize, max_chars: usize) -> Option<String> {
    let lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return None;
    }

    let shown = lines
        .iter()
        .take(max_lines)
        .map(|line| compact_line(line.trim(), max_chars))
        .collect::<Vec<_>>();
    let remaining = lines.len().saturating_sub(shown.len());
    let mut preview = shown.join(" / ");
    if remaining > 0 {
        if !preview.is_empty() {
            preview.push_str(" / ");
        }
        preview.push_str(&format!("... {} more lines", remaining));
    }
    Some(preview)
}

fn should_show_run(run: &ToolRunView) -> bool {
    if run.is_active() {
        return true;
    }
    if matches!(
        run.status,
        ToolRunStatus::Failed
            | ToolRunStatus::TimedOut
            | ToolRunStatus::Cancelled
            | ToolRunStatus::Backgrounded
    ) {
        return true;
    }
    if is_file_mutation(&run.name) || is_shell(&run.name) || is_validation(&run.name) {
        return true;
    }
    !matches!(run.status, ToolRunStatus::Completed) || !is_routine_read_only(&run.name)
}

fn should_show_turn(run: &ToolRunView, turn: &ToolTurnSnapshot) -> bool {
    if !turn.phase.is_terminal() {
        return true;
    }
    if matches!(
        turn.phase,
        ToolTurnPhase::Failed | ToolTurnPhase::Cancelled | ToolTurnPhase::TimedOut
    ) {
        return true;
    }
    should_show_run(run)
}

fn preview_for_turn(turn: &ToolTurnSnapshot, max_chars: usize) -> Option<String> {
    turn.failure
        .as_ref()
        .map(|text| compact_line(text, max_chars))
        .or_else(|| {
            turn.result_preview
                .as_ref()
                .map(|text| compact_line(text, max_chars))
        })
        .or_else(|| {
            turn.arguments_preview
                .as_ref()
                .filter(|_| {
                    matches!(
                        turn.phase,
                        ToolTurnPhase::Requested
                            | ToolTurnPhase::Accepted
                            | ToolTurnPhase::Executing
                    )
                })
                .map(|text| compact_line(text, max_chars))
        })
}

fn preview_for_run(
    run: &ToolRunView,
    max_chars: usize,
    severity: ToolRowSeverity,
) -> Option<String> {
    if matches!(severity, ToolRowSeverity::Error) {
        return run
            .result_body
            .as_deref()
            .and_then(|body| collapse_output_preview(body, 2, max_chars))
            .or_else(|| {
                run.result_preview
                    .as_ref()
                    .map(|text| compact_line(text, max_chars))
            });
    }
    if run.is_active() {
        return run
            .progress
            .last()
            .map(|line| compact_line(line, max_chars));
    }
    run.result_preview
        .as_ref()
        .filter(|_| is_shell(&run.name) || is_file_mutation(&run.name))
        .map(|text| compact_line(text, max_chars))
}

fn severity_for_status(status: ToolRunStatus) -> ToolRowSeverity {
    match status {
        ToolRunStatus::Queued | ToolRunStatus::Running | ToolRunStatus::WaitingPermission => {
            ToolRowSeverity::Warning
        }
        ToolRunStatus::Failed | ToolRunStatus::TimedOut => ToolRowSeverity::Error,
        ToolRunStatus::Cancelled => ToolRowSeverity::Warning,
        ToolRunStatus::Backgrounded => ToolRowSeverity::Info,
        ToolRunStatus::Completed => ToolRowSeverity::Success,
    }
}

fn severity_for_phase(phase: ToolTurnPhase) -> ToolRowSeverity {
    match phase {
        ToolTurnPhase::Requested | ToolTurnPhase::Accepted | ToolTurnPhase::Executing => {
            ToolRowSeverity::Warning
        }
        ToolTurnPhase::ResultObserved | ToolTurnPhase::SentBackToModel => ToolRowSeverity::Info,
        ToolTurnPhase::FinalAnswer | ToolTurnPhase::Persisted => ToolRowSeverity::Success,
        ToolTurnPhase::Failed | ToolTurnPhase::TimedOut => ToolRowSeverity::Error,
        ToolTurnPhase::Cancelled => ToolRowSeverity::Warning,
    }
}

fn icon_for_status(status: ToolRunStatus) -> &'static str {
    match status {
        ToolRunStatus::Queued | ToolRunStatus::Running => "●",
        ToolRunStatus::WaitingPermission => "?",
        ToolRunStatus::Backgrounded => "↪",
        ToolRunStatus::Completed => "✓",
        ToolRunStatus::Cancelled => "×",
        ToolRunStatus::TimedOut | ToolRunStatus::Failed => "✗",
    }
}

fn icon_for_phase(phase: ToolTurnPhase) -> &'static str {
    match phase {
        ToolTurnPhase::Requested | ToolTurnPhase::Accepted | ToolTurnPhase::Executing => "●",
        ToolTurnPhase::ResultObserved | ToolTurnPhase::SentBackToModel => "→",
        ToolTurnPhase::FinalAnswer | ToolTurnPhase::Persisted => "✓",
        ToolTurnPhase::Cancelled => "×",
        ToolTurnPhase::TimedOut | ToolTurnPhase::Failed => "✗",
    }
}

fn status_label(status: ToolRunStatus) -> &'static str {
    match status {
        ToolRunStatus::Queued => "waiting",
        ToolRunStatus::Running => "running",
        ToolRunStatus::Backgrounded => "backgrounded",
        ToolRunStatus::WaitingPermission => "permission",
        ToolRunStatus::TimedOut => "timed out",
        ToolRunStatus::Cancelled => "cancelled",
        ToolRunStatus::Completed => "done",
        ToolRunStatus::Failed => "failed",
    }
}

fn status_label_for_phase(phase: ToolTurnPhase) -> &'static str {
    phase.label()
}

fn trim_status_noise(summary: String) -> String {
    summary
        .replace(" · waiting", "")
        .replace(" · running", "")
        .replace(" · done", "")
}

fn trim_tree_prefix(line: &str) -> String {
    line.trim()
        .trim_start_matches('└')
        .trim_start_matches('├')
        .trim()
        .to_string()
}

pub fn compact_line(line: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in line.chars().take(max_chars) {
        out.push(ch);
    }
    if line.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn is_file_mutation(name: &str) -> bool {
    matches!(
        name,
        "file_write" | "file_edit" | "file_patch" | "format" | "rewind" | "memory_save"
    )
}

fn is_shell(name: &str) -> bool {
    matches!(
        name,
        "bash" | "powershell" | "repl" | "bash_output" | "bash_cancel"
    )
}

fn is_validation(name: &str) -> bool {
    matches!(name, "run_tests" | "git_status" | "git_diff")
}

fn is_routine_read_only(name: &str) -> bool {
    matches!(
        name,
        "file_read"
            | "grep"
            | "glob"
            | "web_search"
            | "json_query"
            | "memory_load"
            | "context"
            | "context_visualization"
            | "datetime"
            | "cost"
            | "telemetry"
            | "project_list"
            | "skill_view"
            | "lsp"
            | "symbol_query"
            | "brief"
            | "notebook"
            | "bash_tasks"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_successful_routine_reads_but_keeps_count() {
        let mut read = ToolRunView::new("tool_1".to_string(), "file_read".to_string());
        read.mark_complete("Result: OK\nhello".to_string());

        let view = tool_rows_for_runs(&[read], 100);

        assert_eq!(view.hidden_routine_count, 1);
        assert!(!view.rows[0].visible);
    }

    #[test]
    fn keeps_shell_and_failures_visible() {
        let mut shell = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        shell.mark_complete("Result: OK\nfinished".to_string());
        let mut failed_read = ToolRunView::new("tool_2".to_string(), "file_read".to_string());
        failed_read.mark_complete("Result: ERROR\nmissing".to_string());

        let view = tool_rows_for_runs(&[shell, failed_read], 100);

        assert_eq!(view.hidden_routine_count, 0);
        assert!(view.rows.iter().all(|row| row.visible));
        assert_eq!(view.rows[0].severity, ToolRowSeverity::Success);
        assert_eq!(view.rows[1].severity, ToolRowSeverity::Error);
    }

    #[test]
    fn collapse_preview_bounds_lines_and_characters() {
        let preview =
            collapse_output_preview("first line is long\nsecond line\nthird line", 2, 10).unwrap();

        assert_eq!(preview, "first line… / second lin… / ... 1 more lines");
    }

    #[test]
    fn row_lines_add_expanded_tool_details_only_when_requested() {
        let mut run = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        run.arguments = Some(serde_json::json!({ "command": "cargo test" }));
        run.mark_complete("Result: OK\nfinished".to_string());
        let row = tool_row_for_run(&run, 100);

        let collapsed = tool_row_lines(&row, false, &run);
        let expanded = tool_row_lines(&row, true, &run);

        assert!(collapsed.iter().any(|line| line.contains("ctrl+o details")));
        assert!(expanded.len() > collapsed.len());
        assert_eq!(tool_row_height(&row, true, &run), expanded.len());
    }

    #[test]
    fn spine_phase_overrides_stale_legacy_run_status() {
        let run = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        let turn = ToolTurnSnapshot {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            parent_message_id: None,
            phase: ToolTurnPhase::SentBackToModel,
            arguments_preview: Some("{\"command\":\"pwd\"}".to_string()),
            result_preview: Some("/tmp/project".to_string()),
            failure: None,
        };

        let view = tool_rows_for_runs_with_spine(&[run], &[turn], 100);

        assert_eq!(view.rows[0].status_label, "sent back to model");
        assert_eq!(view.rows[0].severity, ToolRowSeverity::Info);
        assert_eq!(view.rows[0].icon, "→");
        assert_eq!(view.rows[0].preview.as_deref(), Some("/tmp/project"));
        assert!(view.rows[0].visible);
    }

    #[test]
    fn spine_failure_stays_visible_for_routine_read() {
        let mut run = ToolRunView::new("tool_1".to_string(), "file_read".to_string());
        run.mark_complete("Result: OK\nignored".to_string());
        let turn = ToolTurnSnapshot {
            id: "tool_1".to_string(),
            name: "file_read".to_string(),
            parent_message_id: None,
            phase: ToolTurnPhase::Failed,
            arguments_preview: None,
            result_preview: None,
            failure: Some("permission denied".to_string()),
        };

        let view = tool_rows_for_runs_with_spine(&[run], &[turn], 100);

        assert_eq!(view.hidden_routine_count, 0);
        assert_eq!(view.rows[0].status_label, "failed");
        assert_eq!(view.rows[0].severity, ToolRowSeverity::Error);
        assert_eq!(view.rows[0].preview.as_deref(), Some("permission denied"));
        assert!(view.rows[0].visible);
    }
}
