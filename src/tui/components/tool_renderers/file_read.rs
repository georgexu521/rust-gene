//! File read / git_status / diff / lsp / context tool inline renderer.

use crate::tui::tool_view::ToolRunView;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub fn render_file_read_tool(
    run: &ToolRunView,
    theme: &crate::tui::theme::Theme,
    width: usize,
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
    if output.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (no output)".to_string(),
            Style::default().fg(theme.tokens.fg.faint),
        )]));
    } else {
        let effective_width = width.saturating_sub(6).max(1);
        for raw in output.lines().take(24) {
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
        if total > 24 {
            lines.push(Line::from(vec![Span::styled(
                format!("  ... ({} more lines)", total - 24),
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
    fn file_read_renderer_shows_path_and_content() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "file_read".to_string());
        run.arguments = Some(serde_json::json!({"path": "src/lib.rs"}));
        run.mark_complete("pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}".to_string());

        let lines = render_file_read_tool(&run, &theme, 80);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("src/lib.rs"));
        assert!(text.contains("pub fn add"));
    }
}
