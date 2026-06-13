//! File patch tool inline renderer.

use crate::tui::components::diff_renderer::build_diff_lines;
use crate::tui::tool_view::ToolRunView;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use serde_json::Value;

pub fn render_file_patch_tool(
    run: &ToolRunView,
    theme: &crate::tui::theme::Theme,
    _width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let summary = patch_summary(run);
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            summary,
            Style::default()
                .fg(theme.tokens.fg.body)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let diff = extract_patch_diff(run);
    if diff.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (no diff)".to_string(),
            Style::default().fg(theme.tokens.fg.faint),
        )]));
        return lines;
    }

    let (mut diff_lines, file_count, hunk_count) = build_diff_lines(&diff, theme, false);
    let max_lines = 32usize;
    let truncate = diff_lines.len() > max_lines;
    if truncate {
        diff_lines.truncate(max_lines);
    }
    for dl in diff_lines {
        let mut spans = vec![Span::styled("  ", Style::default())];
        spans.extend(dl.spans);
        lines.push(Line::from(spans));
    }
    if truncate {
        lines.push(Line::from(vec![Span::styled(
            format!(
                "  ... ({} file(s), {} hunk(s) truncated)",
                file_count, hunk_count
            ),
            Style::default().fg(theme.tokens.fg.faint),
        )]));
    }

    lines
}

fn patch_summary(run: &ToolRunView) -> String {
    let file_count = run
        .result_data
        .as_ref()
        .and_then(|d| d.get("files"))
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let (add, del) = run
        .result_data
        .as_ref()
        .and_then(|d| d.get("files"))
        .and_then(|v| v.as_array())
        .map(|files| {
            files.iter().fold((0usize, 0usize), |(a, d), f| {
                let fa = f.get("additions").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let fd = f.get("deletions").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                (a + fa, d + fd)
            })
        })
        .unwrap_or((0, 0));

    if file_count == 0 {
        "Patched files".to_string()
    } else {
        format!("Patched {} file(s): +{}/-{}", file_count, add, del)
    }
}

fn extract_patch_diff(run: &ToolRunView) -> String {
    if let Some(data) = &run.result_data {
        if let Some(diff) = data.get("diff").and_then(|v| v.as_str()) {
            return diff.to_string();
        }
        if let Some(files) = data.get("files").and_then(|v| v.as_array()) {
            return files_to_unified_diff(files);
        }
    }
    if let Some(body) = &run.result_body {
        if body.contains("@@") || body.contains("diff --git") {
            return body.clone();
        }
    }
    String::new()
}

fn files_to_unified_diff(files: &[Value]) -> String {
    let mut out = String::new();
    for file in files {
        let path = file
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        out.push_str(&format!("diff --git a/{path} b/{path}\n"));
        out.push_str(&format!("--- a/{path}\n"));
        out.push_str(&format!("+++ b/{path}\n"));

        if let Some(diff) = file.get("diff").and_then(|v| v.as_str()) {
            out.push_str(diff);
            if !diff.ends_with('\n') {
                out.push('\n');
            }
        } else if let Some(chunks) = file.get("chunks").and_then(|v| v.as_array()) {
            for chunk in chunks {
                let old_start = chunk.get("old_start").and_then(|v| v.as_u64()).unwrap_or(1);
                let old_count = chunk.get("old_count").and_then(|v| v.as_u64()).unwrap_or(0);
                let new_start = chunk.get("new_start").and_then(|v| v.as_u64()).unwrap_or(1);
                let new_count = chunk.get("new_count").and_then(|v| v.as_u64()).unwrap_or(0);
                out.push_str(&format!(
                    "@@ -{},{old_count} +{},{new_count} @@\n",
                    old_start, new_start
                ));
                if let Some(lines) = chunk.get("lines").and_then(|v| v.as_array()) {
                    for line in lines {
                        if let Some(s) = line.as_str() {
                            out.push_str(s);
                            out.push('\n');
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{theme::Theme, tool_view::ToolRunView};

    #[test]
    fn file_patch_renderer_shows_summary_and_diff() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "file_patch".to_string());
        run.result_data = Some(serde_json::json!({
            "files": [
                {
                    "path": "src/main.rs",
                    "additions": 1,
                    "deletions": 1,
                    "diff": "@@ -1,1 +1,1 @@\n-old\n+new\n"
                }
            ]
        }));
        run.mark_complete("ok".to_string());

        let lines = render_file_patch_tool(&run, &theme, 80);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("Patched 1 file(s)"));
        assert!(text.contains("+1/-1"));
        assert!(text.contains("-old"));
        assert!(text.contains("+new"));
    }
}
