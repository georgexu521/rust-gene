//! 消息渲染组件
//!
//! Reasonix 风格：Card header (glyph + role + metadata) + Card body

use crate::state::{MessageItem, MessageRole};
use crate::tui::components::markdown::parse_markdown;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
};

/// Render a card header line: `glyph  ROLE  · meta`
fn card_header(
    glyph: &str,
    role_label: &str,
    color: Color,
    meta: Option<String>,
    faint: Color,
) -> Line<'static> {
    let mut spans = vec![
        Span::styled(glyph.to_string(), Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled("  ".to_string(), Style::default()),
        Span::styled(role_label.to_string(), Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ];
    if let Some(m) = meta {
        spans.push(Span::styled(" · ".to_string(), Style::default().fg(faint)));
        spans.push(Span::styled(m, Style::default().fg(faint)));
    }
    Line::from(spans)
}

/// Streaming state for assistant card rendering
pub struct StreamMeta {
    pub is_streaming: bool,
    pub tick: usize,
    pub token_count: Option<u32>,
}

/// 渲染消息为 Paragraph（Reasonix card 风格）
pub fn render_message<'a>(
    message: &'a MessageItem,
    _width: usize,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    match message.role {
        MessageRole::User => render_user_message(message, theme),
        MessageRole::Assistant => render_assistant_message(message, theme, None),
        MessageRole::System => render_system_message(message, theme),
        MessageRole::Tool => render_tool_message(message, theme),
    }
}

/// 渲染消息为 Paragraph（带 streaming 状态）
pub fn render_message_with_stream<'a>(
    message: &'a MessageItem,
    _width: usize,
    theme: &'a crate::tui::theme::Theme,
    stream: Option<&StreamMeta>,
) -> Paragraph<'a> {
    match message.role {
        MessageRole::User => render_user_message(message, theme),
        MessageRole::Assistant => render_assistant_message(message, theme, stream),
        MessageRole::System => render_system_message(message, theme),
        MessageRole::Tool => render_tool_message(message, theme),
    }
}

fn render_user_message<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    let card = &theme.tokens.card.user;
    let mut lines = vec![card_header(
        card.glyph,
        "You",
        card.color,
        None,
        theme.tokens.fg.faint,
    )];
    lines.push(Line::from(""));

    let markdown_text = parse_markdown(&message.content, theme);
    for line in markdown_text.lines {
        let mut spans = vec![Span::styled("  ", Style::default())];
        spans.extend(line.spans);
        lines.push(Line::from(spans));
    }
    Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(theme.tokens.message_bg.user))
}

fn render_assistant_message<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
    stream: Option<&StreamMeta>,
) -> Paragraph<'a> {
    let is_streaming = stream.map(|s| s.is_streaming).unwrap_or(false);
    let tick = stream.map(|s| s.tick).unwrap_or(0);

    // Header: pulse animation during streaming, static glyph when done
    let (glyph, label, header_color) = if is_streaming {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        (frames[tick % frames.len()], "Writing", theme.tokens.tone.brand)
    } else {
        ("‹", "Reply", theme.tokens.tone.ok)
    };

    // Meta: token count if available
    let meta = stream.and_then(|s| s.token_count.map(|n| format!("{} tok", n)));

    let mut lines = vec![card_header(
        glyph,
        label,
        header_color,
        meta,
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

fn render_system_message<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    let card = &theme.tokens.card.warn;
    let mut lines = vec![
        card_header(
            card.glyph,
            "System",
            card.color,
            None,
            theme.tokens.fg.faint,
        ),
        Line::from(""),
    ];

    let markdown_text = parse_markdown(&message.content, theme);
    for line in markdown_text.lines {
        let mut spans = vec![Span::styled("  ", Style::default())];
        spans.extend(line.spans);
        lines.push(Line::from(spans));
    }
    Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: true })
        .style(Style::default().add_modifier(Modifier::ITALIC))
}

fn render_tool_message<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    let card = &theme.tokens.card.tool;
    let lines = vec![
        card_header(
            card.glyph,
            "Tool",
            card.color,
            None,
            theme.tokens.fg.faint,
        ),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(&message.content, Style::default().fg(theme.tokens.fg.faint)),
        ]),
    ];
    Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true })
}

/// 简化版本的消息渲染（用于紧凑显示）
pub fn render_message_compact<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    let (prefix, color) = match message.role {
        MessageRole::User => (theme.tokens.card.user.glyph, theme.tokens.card.user.color),
        MessageRole::Assistant => (theme.tokens.card.streaming.glyph, theme.tokens.tone.ok),
        MessageRole::System => (theme.tokens.card.warn.glyph, theme.tokens.tone.warn),
        MessageRole::Tool => (theme.tokens.card.tool.glyph, theme.tokens.tone.info),
    };

    let content = if message.content.len() > 100 {
        let truncated: String = message.content.chars().take(100).collect();
        format!("{}...", truncated)
    } else {
        message.content.clone()
    };

    let text = Text::from(vec![Line::from(vec![
        Span::styled(format!("{} ", prefix), Style::default().fg(color)),
        Span::styled(content, Style::default().fg(theme.tokens.fg.body)),
    ])]);

    Paragraph::new(text)
}

/// 获取消息角色的颜色
pub fn role_color(role: MessageRole, theme: &crate::tui::theme::Theme) -> Color {
    match role {
        MessageRole::User => theme.tokens.card.user.color,
        MessageRole::Assistant => theme.tokens.tone.ok,
        MessageRole::System => theme.tokens.tone.warn,
        MessageRole::Tool => theme.tokens.tone.info,
    }
}

/// 获取消息角色的图标（使用 card glyph）
pub fn role_icon(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "◇",
        MessageRole::Assistant => "◈",
        MessageRole::System => "⚠",
        MessageRole::Tool => "▣",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::Theme;

    #[test]
    fn test_role_colors() {
        let theme = Theme::dark();
        assert_eq!(role_color(MessageRole::User, &theme), theme.tokens.card.user.color);
        assert_eq!(
            role_color(MessageRole::Assistant, &theme),
            theme.tokens.tone.ok
        );
    }
}
