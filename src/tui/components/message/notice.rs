use crate::state::MessageItem;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Text},
    widgets::{Paragraph, Wrap},
};

use super::{card_header, text::append_markdown_lines, CardKind};

pub(super) fn render_system_message<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
    kind: Option<CardKind>,
) -> Paragraph<'a> {
    let (glyph, label, color) = match kind {
        Some(CardKind::Error) => (
            theme.tokens.card.error.glyph,
            "Error",
            theme.tokens.card.error.color,
        ),
        Some(CardKind::Warning) => (
            theme.tokens.card.warn.glyph,
            "Warning",
            theme.tokens.card.warn.color,
        ),
        _ => (
            theme.tokens.card.warn.glyph,
            "System",
            theme.tokens.card.warn.color,
        ),
    };
    let mut lines = vec![
        card_header(glyph, label, color, None, theme.tokens.fg.faint),
        Line::from(""),
    ];
    append_markdown_lines(&mut lines, &message.content, theme, "  ");

    Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: true })
        .style(Style::default().add_modifier(Modifier::ITALIC))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MessageRole;
    use ratatui::{backend::TestBackend, Terminal};
    use std::{collections::HashMap, time::SystemTime};

    #[test]
    fn renders_notice_with_error_label() {
        let theme = crate::tui::theme::Theme::graphite();
        let message = MessageItem {
            id: "system".to_string(),
            role: MessageRole::System,
            content: "Error: failed".to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            metadata: HashMap::new(),
        };

        let backend = TestBackend::new(80, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let paragraph = render_system_message(&message, &theme, Some(CardKind::Error));
                frame.render_widget(paragraph, frame.area());
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(text.contains("Error"));
    }
}
