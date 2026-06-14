//! Grep / glob / web_search / json_query tool inline renderer.

use crate::tui::components::collapsible::{
    collapse_footer, collapse_lines, tool_body_budget, wrap_line_to_width,
    DEFAULT_TOOL_BODY_MAX_LINES,
};
use crate::tui::tool_view::ToolRunView;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub fn render_search_tool(
    run: &ToolRunView,
    theme: &crate::tui::theme::Theme,
    width: usize,
    inline_expanded: bool,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let (query, target) = match run.name.as_str() {
        "grep" => {
            let q = run
                .arguments
                .as_ref()
                .and_then(|a| a.get("pattern"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let path = run
                .arguments
                .as_ref()
                .and_then(|a| a.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (q.to_string(), path.to_string())
        }
        "glob" => {
            let q = run
                .arguments
                .as_ref()
                .and_then(|a| a.get("pattern"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (q.to_string(), String::new())
        }
        "web_search" => {
            let q = run
                .arguments
                .as_ref()
                .and_then(|a| a.get("query"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (q.to_string(), String::new())
        }
        _ => {
            let q = run
                .arguments
                .as_ref()
                .and_then(|a| a.get("query").or_else(|| a.get("pattern")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (q.to_string(), String::new())
        }
    };

    let title = match run.name.as_str() {
        "grep" => "Grep",
        "glob" => "Glob",
        "web_search" => "Web search",
        _ => "Search",
    };

    let header = if target.is_empty() {
        format!("{} {}", title, query)
    } else {
        format!("{} {} in {}", title, query, target)
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
    if output.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (no results)".to_string(),
            Style::default().fg(theme.tokens.fg.faint),
        )]));
    } else {
        let output_lines = render_search_output(output, width, theme);
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

fn render_search_output(
    output: &str,
    width: usize,
    theme: &crate::tui::theme::Theme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
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
    fn grep_renderer_shows_pattern_and_results() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "grep".to_string());
        run.arguments = Some(serde_json::json!({"pattern": "TODO", "path": "src"}));
        run.mark_complete("src/lib.rs:10: TODO: fix bug".to_string());

        let lines = render_search_tool(&run, &theme, 80, false);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("TODO"));
        assert!(text.contains("src/lib.rs:10"));
    }

    #[test]
    fn grep_renderer_collapses_long_output() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "grep".to_string());
        run.arguments = Some(serde_json::json!({"pattern": "TODO", "path": "src"}));
        run.result_body = Some(
            (1..=100)
                .map(|i| format!("src/{}.rs: TODO {}", i, i))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        run.mark_complete(run.result_body.clone().unwrap());

        let collapsed = render_search_tool(&run, &theme, 80, false);
        let expanded = render_search_tool(&run, &theme, 80, true);

        assert!(collapsed.len() < expanded.len());
        let text: String = collapsed
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("more lines"));
    }
}
