//! TUI message component renderer.
//!
//! Renders one message kind into timeline rows without owning session or runtime state.

use crate::tui::view_model::reasoning::{expanded_reasoning_lines, AssistantReasoningView};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

/// Render a collapsed reasoning summary line.
#[allow(dead_code)]
pub(super) fn render_reasoning_summary(
    reasoning: &AssistantReasoningView,
    theme: &crate::tui::theme::Theme,
) -> Option<Line<'static>> {
    reasoning.has_hidden_reasoning().then(|| {
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                reasoning.reasoning_label(),
                Style::default()
                    .fg(theme.tokens.fg.faint)
                    .add_modifier(Modifier::ITALIC),
            ),
        ])
    })
}

/// Append the expanded reasoning body to the message lines.
#[allow(dead_code)]
pub(super) fn append_reasoning_body(
    lines: &mut Vec<Line<'static>>,
    reasoning: &AssistantReasoningView,
    theme: &crate::tui::theme::Theme,
) {
    for line in expanded_reasoning_lines(reasoning) {
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(line.to_string(), Style::default().fg(theme.tokens.fg.sub)),
        ]));
    }

    if reasoning.hidden_reasoning_lines > expanded_reasoning_lines(reasoning).len() {
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                format!(
                    "... {} more reasoning lines",
                    reasoning
                        .hidden_reasoning_lines
                        .saturating_sub(expanded_reasoning_lines(reasoning).len())
                ),
                Style::default()
                    .fg(theme.tokens.fg.faint)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{theme::Theme, view_model::reasoning::assistant_reasoning_view};

    #[test]
    fn renders_collapsed_reasoning_summary() {
        let theme = Theme::graphite();
        let reasoning = assistant_reasoning_view("<think>one</think>answer");

        let line = render_reasoning_summary(&reasoning, &theme).unwrap();

        assert!(line
            .spans
            .iter()
            .any(|span| span.content.contains("Thinking hidden")));
    }

    #[test]
    fn appends_expanded_reasoning_body() {
        let theme = Theme::graphite();
        let reasoning = assistant_reasoning_view("<think>one\ntwo</think>answer");
        let mut lines = Vec::new();

        append_reasoning_body(&mut lines, &reasoning, &theme);

        assert_eq!(lines.len(), 2);
        assert!(lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("one")));
    }
}
