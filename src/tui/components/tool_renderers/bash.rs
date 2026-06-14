//! Bash / shell tool inline renderer.

use crate::tui::components::collapsible::{
    collapse_footer, collapse_lines, tool_body_budget, wrap_line_to_width,
    DEFAULT_TOOL_BODY_MAX_LINES,
};
use crate::tui::tool_view::ToolRunView;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub fn render_bash_tool(
    run: &ToolRunView,
    theme: &crate::tui::theme::Theme,
    width: usize,
    inline_expanded: bool,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let command = run
        .arguments
        .as_ref()
        .and_then(|args| args.get("command"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            run.arguments
                .as_ref()
                .map(|args| serde_json::to_string(args).unwrap_or_default())
        })
        .unwrap_or_else(|| "bash".to_string());

    let command = crate::tui::view_model::tool_rows::compact_line(
        &command,
        width.saturating_sub(10).clamp(40, 200),
    );
    lines.push(Line::from(vec![
        Span::styled("  $ ", Style::default().fg(theme.tokens.tone.info)),
        Span::styled(
            command,
            Style::default()
                .fg(theme.tokens.fg.body)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let output = run.result_body.as_deref().unwrap_or("");
    let output_lines = render_bash_output(output, width, theme);
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

    if output.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (no output)".to_string(),
            Style::default().fg(theme.tokens.fg.faint),
        )]));
    }

    let status = run.status;
    let elapsed = run.elapsed();
    let (status_text, color) = match status {
        crate::tui::tool_view::ToolRunStatus::Completed => ("ok".to_string(), theme.tokens.tone.ok),
        crate::tui::tool_view::ToolRunStatus::Failed => {
            ("failed".to_string(), theme.tokens.tone.err)
        }
        crate::tui::tool_view::ToolRunStatus::TimedOut => {
            ("timed out".to_string(), theme.tokens.tone.warn)
        }
        crate::tui::tool_view::ToolRunStatus::Cancelled => {
            ("cancelled".to_string(), theme.tokens.tone.warn)
        }
        crate::tui::tool_view::ToolRunStatus::Backgrounded => {
            ("background".to_string(), theme.tokens.tone.info)
        }
        _ => ("running".to_string(), theme.tokens.tone.brand),
    };

    let duration = if elapsed.as_secs() >= 60 {
        format!(
            "{}m {:.1}s",
            elapsed.as_secs() / 60,
            elapsed.as_secs_f64() % 60.0
        )
    } else if elapsed.as_millis() >= 1000 {
        format!("{:.1}s", elapsed.as_secs_f64())
    } else {
        format!("{}ms", elapsed.as_millis())
    };

    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{} · {}", status_text, duration),
            Style::default().fg(color),
        ),
    ]));

    lines
}

fn render_bash_output(
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
    fn bash_renderer_shows_command_and_output() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "bash".to_string());
        run.arguments = Some(serde_json::json!({"command": "pwd"}));
        run.result_body = Some("/tmp/project".to_string());
        run.mark_complete("Result: OK\n/tmp/project".to_string());

        let lines = render_bash_tool(&run, &theme, 80, false);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("$"));
        assert!(text.contains("pwd"));
        assert!(text.contains("/tmp/project"));
    }

    #[test]
    fn bash_renderer_wraps_long_lines() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "bash".to_string());
        run.result_body = Some("a".repeat(200));
        run.mark_complete("ok".to_string());

        let lines = render_bash_tool(&run, &theme, 40, false);
        assert!(lines.len() > 2);
    }

    #[test]
    fn bash_renderer_collapses_long_output() {
        let theme = Theme::default();
        let mut run = ToolRunView::new("call_1".to_string(), "bash".to_string());
        run.arguments = Some(serde_json::json!({"command": "seq 100"}));
        run.result_body = Some(
            (1..=100)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        run.mark_complete(run.result_body.clone().unwrap());

        let collapsed = render_bash_tool(&run, &theme, 80, false);
        let expanded = render_bash_tool(&run, &theme, 80, true);

        assert!(collapsed.len() < expanded.len());
        let text: String = collapsed
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains("more lines"));
    }
}
