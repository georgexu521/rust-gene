//! Per-tool inline renderers for the TUI transcript.
//!
//! Each renderer turns a `ToolRunView` into styled `Line`s that are displayed
//! inline when the tool row is expanded. The collapsed row view (summary line)
//! is still produced by `tool_rows.rs`; renderers only own the expanded body.

use crate::tui::tool_view::ToolRunView;
use ratatui::text::Line;

pub mod bash;
pub mod file_edit;
pub mod file_patch;
pub mod file_read;
pub mod grep;

/// Render the expanded body of a tool run.
///
/// `width` is the available terminal width in cells. Renderers should wrap or
/// truncate content to fit.
pub fn render_tool_lines(
    run: &ToolRunView,
    theme: &crate::tui::theme::Theme,
    width: usize,
    inline_expanded: bool,
) -> Vec<Line<'static>> {
    match run.name.as_str() {
        "bash" | "powershell" | "repl" => {
            bash::render_bash_tool(run, theme, width, inline_expanded)
        }
        "file_edit" | "format" | "rewind" => file_edit::render_file_edit_tool(run, theme, width),
        "file_patch" => file_patch::render_file_patch_tool(run, theme, width),
        "file_read" | "git_status" | "git_diff" | "diff" | "lsp" | "context" => {
            file_read::render_file_read_tool(run, theme, width, inline_expanded)
        }
        "grep" | "glob" | "web_search" | "json_query" => {
            grep::render_search_tool(run, theme, width, inline_expanded)
        }
        _ => render_fallback(run, theme, width),
    }
}

/// Estimate the visible height of a tool body in lines.
pub fn estimate_tool_body_height(run: &ToolRunView, width: usize, inline_expanded: bool) -> usize {
    render_tool_lines(
        run,
        &crate::tui::theme::Theme::default(),
        width,
        inline_expanded,
    )
    .len()
}

fn render_fallback(
    run: &ToolRunView,
    theme: &crate::tui::theme::Theme,
    _width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(args) = &run.arguments {
        let text = serde_json::to_string_pretty(args).unwrap_or_default();
        for raw in text.lines() {
            lines.push(Line::from(vec![
                ratatui::text::Span::styled("  ", ratatui::style::Style::default()),
                ratatui::text::Span::styled(
                    raw.to_string(),
                    ratatui::style::Style::default().fg(theme.tokens.fg.body),
                ),
            ]));
        }
    }
    if let Some(body) = &run.result_body {
        for raw in body.lines() {
            lines.push(Line::from(vec![
                ratatui::text::Span::styled("  ", ratatui::style::Style::default()),
                ratatui::text::Span::styled(
                    raw.to_string(),
                    ratatui::style::Style::default().fg(theme.tokens.fg.faint),
                ),
            ]));
        }
    }
    lines
}

pub fn tool_status_color(
    status: crate::tui::tool_view::ToolRunStatus,
    theme: &crate::tui::theme::Theme,
) -> ratatui::style::Color {
    use crate::tui::tool_view::ToolRunStatus;
    match status {
        ToolRunStatus::Completed | ToolRunStatus::Backgrounded => theme.tokens.tone.ok,
        ToolRunStatus::Running | ToolRunStatus::Queued | ToolRunStatus::WaitingPermission => {
            theme.tokens.tone.brand
        }
        ToolRunStatus::Failed => theme.tokens.tone.err,
        ToolRunStatus::TimedOut | ToolRunStatus::Cancelled => theme.tokens.tone.warn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{theme::Theme, tool_view::ToolRunView};

    #[test]
    fn fallback_renderer_shows_args_and_body() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "unknown_tool".to_string());
        run.arguments = Some(serde_json::json!({"key": "value"}));
        run.result_body = Some("result line".to_string());

        let lines = render_tool_lines(&run, &theme, 80, false);

        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("key"));
        assert!(text.contains("result line"));
    }
}
