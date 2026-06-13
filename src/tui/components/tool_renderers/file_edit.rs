//! File edit / format / rewind tool inline renderer.

use crate::tui::components::diff_renderer::build_diff_lines;
use crate::tui::tool_view::ToolRunView;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub fn render_file_edit_tool(
    run: &ToolRunView,
    theme: &crate::tui::theme::Theme,
    _width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let path = run
        .arguments
        .as_ref()
        .and_then(|args| args.get("path").or_else(|| args.get("file_path")))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown file");

    let title = match run.name.as_str() {
        "format" => "Formatted",
        "rewind" => "Rewound",
        _ => "Edited",
    };

    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{} {}", title, path),
            Style::default()
                .fg(theme.tokens.fg.body)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let diff = extract_diff(run);
    if diff.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (no diff)".to_string(),
            Style::default().fg(theme.tokens.fg.faint),
        )]));
        return lines;
    }

    // Render inline diff without line-number gutter to save horizontal space.
    let (mut diff_lines, _files, _hunks) = build_diff_lines(&diff, theme, false);
    let max_lines = 24usize;
    let truncate = diff_lines.len() > max_lines;
    if truncate {
        diff_lines.truncate(max_lines);
    }
    for dl in diff_lines {
        let mut spans = vec![Span::styled("  ", Style::default())];
        spans.extend(dl.spans.into_iter().map(|mut s| {
            s.style = s.style.patch(Style::default());
            s
        }));
        lines.push(Line::from(spans));
    }
    if truncate {
        lines.push(Line::from(vec![Span::styled(
            "  ... (diff truncated)".to_string(),
            Style::default().fg(theme.tokens.fg.faint),
        )]));
    }

    lines
}

fn extract_diff(run: &ToolRunView) -> String {
    // Prefer structured diff from result_data.
    if let Some(data) = &run.result_data {
        if let Some(diff) = data.get("diff").and_then(|v| v.as_str()) {
            return diff.to_string();
        }
        if let Some(changes) = data.get("changes").and_then(|v| v.as_array()) {
            let mut out = String::new();
            for change in changes {
                if let Some(old) = change.get("old_string").and_then(|v| v.as_str()) {
                    for line in old.lines() {
                        out.push('-');
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                if let Some(new) = change.get("new_string").and_then(|v| v.as_str()) {
                    for line in new.lines() {
                        out.push('+');
                        out.push_str(line);
                        out.push('\n');
                    }
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
    }

    // Fallback: try to parse a diff from the result body.
    if let Some(body) = &run.result_body {
        if body.contains("@@") || body.contains("diff --git") {
            return body.clone();
        }
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{theme::Theme, tool_view::ToolRunView};

    #[test]
    fn file_edit_renderer_shows_path_and_diff() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "file_edit".to_string());
        run.arguments = Some(serde_json::json!({"path": "src/main.rs"}));
        run.result_data = Some(serde_json::json!({
            "diff": "@@ -1,1 +1,1 @@\n-old\n+new\n"
        }));
        run.mark_complete("ok".to_string());

        let lines = render_file_edit_tool(&run, &theme, 80);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("src/main.rs"));
        assert!(text.contains("-old"));
        assert!(text.contains("+new"));
    }
}
