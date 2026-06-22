//! TUI message component renderer.
//!
//! Renders one message kind into timeline rows without owning session or runtime state.

use crate::state::MessageItem;
use ratatui::{
    style::Style,
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
};

use super::CardKind;

pub(super) fn render_tool_message<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
    kind: Option<CardKind>,
) -> Paragraph<'a> {
    let (glyph, label, color) = match kind {
        Some(CardKind::Search) => (
            theme.tokens.card.search.glyph,
            "Search",
            theme.tokens.card.search.color,
        ),
        Some(CardKind::SubAgent) => (
            theme.tokens.card.subagent.glyph,
            "Agent",
            theme.tokens.card.subagent.color,
        ),
        _ => (
            theme.tokens.card.tool.glyph,
            "Tool",
            theme.tokens.card.tool.color,
        ),
    };
    let lines = vec![
        super::card_header(glyph, label, color, None, theme.tokens.fg.faint),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(&message.content, Style::default().fg(theme.tokens.fg.faint)),
        ]),
    ];
    Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true })
}
