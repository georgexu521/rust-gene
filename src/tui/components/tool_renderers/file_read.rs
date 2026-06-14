//! File read / git_status / diff / lsp / context tool inline renderer.

use crate::tui::components::collapsible::{
    collapse_footer, collapse_lines, tool_body_budget, wrap_line_to_width,
    DEFAULT_TOOL_BODY_MAX_LINES,
};
use crate::tui::tool_view::ToolRunView;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub fn render_file_read_tool(
    run: &ToolRunView,
    theme: &crate::tui::theme::Theme,
    width: usize,
    inline_expanded: bool,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let path = run
        .arguments
        .as_ref()
        .and_then(|args| args.get("path").or_else(|| args.get("file_path")))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let title = match run.name.as_str() {
        "git_status" => "Git status",
        "git_diff" => "Git diff",
        "diff" => "Diff",
        "lsp" => "LSP diagnostics",
        "context" => "Context",
        _ => {
            if path.is_empty() {
                "Read"
            } else {
                "Read file"
            }
        }
    };

    let header = if path.is_empty() {
        title.to_string()
    } else {
        format!("{} {}", title, path)
    };

    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            header,
            Style::default()
                .fg(theme.tokens.fg.body)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let output = run.result_body.as_deref().unwrap_or("");
    let output_lines = render_file_read_output(output, width, theme);
    if output.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (no output)".to_string(),
            Style::default().fg(theme.tokens.fg.faint),
        )]));
    } else {
        let collapsed = if inline_expanded {
            collapse_lines(output_lines, usize::MAX, usize::MAX)
        } else {
            collapse_lines(
                output_lines,
                DEFAULT_TOOL_BODY_MAX_LINES,
                tool_body_budget(width.saturating_sub(6).max(1), DEFAULT_TOOL_BODY_MAX_LINES),
            )
        };
        lines.extend(collapsed.visible);
        if collapsed.is_truncated {
            lines.push(collapse_footer(collapsed.hidden_lines, theme));
        }
    }

    lines
}

fn render_file_read_output(
    output: &str,
    width: usize,
    theme: &crate::tui::theme::Theme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if output.is_empty() {
        return lines;
    }
    let effective_width = width.saturating_sub(6).max(1);
    for raw in output.lines() {
        let trimmed = raw.trim_end();
        if trimmed.is_empty() {
            lines.push(Line::from(""));
            continue;
        }
        let wrapped = wrap_line_to_width(trimmed, effective_width);
        for (idx, piece) in wrapped.iter().enumerate() {
            let prefix = if idx == 0 { "  " } else { "     " };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default()),
                Span::styled(piece.to_string(), Style::default().fg(theme.tokens.fg.body)),
            ]));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{theme::Theme, tool_view::ToolRunView};

    #[test]
    fn file_read_renderer_shows_path_and_content() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "file_read".to_string());
        run.arguments = Some(serde_json::json!({"path": "src/lib.rs"}));
        run.mark_complete("pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}".to_string());

        let lines = render_file_read_tool(&run, &theme, 80, false);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("src/lib.rs"));
        assert!(text.contains("pub fn add"));
    }

    #[test]
    fn file_read_renderer_collapses_long_output() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "file_read".to_string());
        run.arguments = Some(serde_json::json!({"path": "big.txt"}));
        run.result_body = Some(
            (1..=100)
                .map(|i| format!("line {}", i))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        run.mark_complete(run.result_body.clone().unwrap());

        let collapsed = render_file_read_tool(&run, &theme, 80, false);
        let expanded = render_file_read_tool(&run, &theme, 80, true);

        assert!(collapsed.len() < expanded.len());
        let text: String = collapsed
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("more lines"));
    }
}
