//! TUI message component renderer.
//!
//! Renders one message kind into timeline rows without owning session or runtime state.

use crate::{state::MessageItem, tui::components::markdown::parse_markdown};
use ratatui::{
    style::Style,
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
};

pub(super) fn render_user_message<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    let card = &theme.tokens.card.user;
    let mut lines = vec![super::card_header(
        card.glyph,
        "You",
        card.color,
        None,
        theme.tokens.fg.faint,
    )];

    let markdown_text = parse_markdown(&message.content, theme);
    for line in markdown_text.lines {
        let mut spans = vec![Span::styled("  ", Style::default())];
        spans.extend(line.spans);
        lines.push(Line::from(spans));
    }
    Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true })
}
