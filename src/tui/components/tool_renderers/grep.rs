//! Grep / glob / web_search / json_query tool inline renderer.

use crate::tui::tool_view::ToolRunView;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub fn render_search_tool(
    run: &ToolRunView,
    theme: &crate::tui::theme::Theme,
    width: usize,
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
        let effective_width = width.saturating_sub(6).max(1);
        for raw in output.lines().take(20) {
            let trimmed = raw.trim_end();
            if trimmed.is_empty() {
                lines.push(Line::from(""));
                continue;
            }
            let wrapped = wrap_line(trimmed, effective_width);
            for (idx, piece) in wrapped.iter().enumerate() {
                let prefix = if idx == 0 { "  " } else { "     " };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default()),
                    Span::styled(piece.to_string(), Style::default().fg(theme.tokens.fg.body)),
                ]));
            }
        }
        let total = output.lines().count();
        if total > 20 {
            lines.push(Line::from(vec![Span::styled(
                format!("  ... ({} more lines)", total - 20),
                Style::default().fg(theme.tokens.fg.faint),
            )]));
        }
    }

    lines
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 || line.chars().count() <= width {
        return vec![line.to_string()];
    }
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    for ch in line.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + w > width && !current.is_empty() {
            pieces.push(current.clone());
            current.clear();
            current_width = 0;
        }
        current.push(ch);
        current_width += w;
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
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

        let lines = render_search_tool(&run, &theme, 80);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("TODO"));
        assert!(text.contains("src/lib.rs:10"));
    }
}
