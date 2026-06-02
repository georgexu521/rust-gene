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
    let glyph_owned = glyph.to_string();
    let label_owned = role_label.to_string();
    let mut spans = vec![
        Span::styled(
            glyph_owned,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            label_owned,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(m) = meta {
        spans.push(Span::styled(" · ", Style::default().fg(faint)));
        spans.push(Span::styled(m, Style::default().fg(faint)));
    }
    Line::from(spans)
}

/// Streaming state for assistant card rendering
pub struct StreamMeta {
    pub is_streaming: bool,
    pub tick: usize,
    pub token_count: Option<u32>,
    pub model_label: Option<String>,
    pub started_at: Option<std::time::Instant>,
}

/// Format relative time like Reasonix: "just now", "5s ago", "3m ago"
fn format_relative_time(ts: std::time::SystemTime) -> String {
    let elapsed = ts.elapsed().unwrap_or_default();
    let secs = elapsed.as_secs();
    if secs < 5 {
        "just now".into()
    } else if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

/// Detects card kind from message content for rich rendering.
/// Uses conservative heuristics — prefers false-negative over false-positive.
fn detect_card_kind(msg: &MessageItem) -> Option<CardKind> {
    match msg.role {
        MessageRole::System => {
            let c = &msg.content;
            // Only flag as error if the message clearly starts with an error indicator
            if c.starts_with("Error:") || c.starts_with("✗") || c.starts_with("[Error") {
                Some(CardKind::Error)
            } else if c.starts_with("⚠") || c.starts_with("Warning:") {
                Some(CardKind::Warning)
            } else {
                None
            }
        }
        MessageRole::Tool => {
            let c = &msg.content;
            // Search: typical grep/glob result header patterns
            if c.contains("matches found") || c.starts_with("Found ") && c.contains("matches") {
                Some(CardKind::Search)
            } else if c.starts_with("Agent ") || c.contains(" subagent ") {
                Some(CardKind::SubAgent)
            } else {
                None
            }
        }
        _ => None,
    }
}

enum CardKind {
    Error,
    Warning,
    Search,
    SubAgent,
}

/// 渲染消息为 Paragraph（Reasonix card 风格）
pub fn render_message<'a>(
    message: &'a MessageItem,
    _width: usize,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    let kind = detect_card_kind(message);
    match message.role {
        MessageRole::User => render_user_message(message, theme),
        MessageRole::Assistant => render_assistant_message(message, theme, None),
        MessageRole::System => render_system_message(message, theme, kind),
        MessageRole::Tool => render_tool_message(message, theme, kind),
    }
}

/// 渲染消息为 Paragraph（带 streaming 状态）
pub fn render_message_with_stream<'a>(
    message: &'a MessageItem,
    _width: usize,
    theme: &'a crate::tui::theme::Theme,
    stream: Option<&StreamMeta>,
) -> Paragraph<'a> {
    let kind = detect_card_kind(message);
    match message.role {
        MessageRole::User => render_user_message(message, theme),
        MessageRole::Assistant => render_assistant_message(message, theme, stream),
        MessageRole::System => render_system_message(message, theme, kind),
        MessageRole::Tool => render_tool_message(message, theme, kind),
    }
}

fn render_user_message<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    let card = &theme.tokens.card.user;
    let time_str = format_relative_time(message.timestamp);
    let mut lines = vec![card_header(
        card.glyph,
        "You",
        card.color,
        Some(time_str),
        theme.tokens.fg.faint,
    )];
    lines.push(Line::from(""));

    let markdown_text = parse_markdown(&message.content, theme);
    for (i, line) in markdown_text.lines.into_iter().enumerate() {
        let mut spans = if i == 0 {
            vec![Span::styled("↳ ", Style::default().fg(theme.tokens.fg.sub))]
        } else {
            vec![Span::styled("  ", Style::default())]
        };
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
        (
            frames[tick % frames.len()],
            "Writing",
            theme.tokens.tone.brand,
        )
    } else {
        ("‹", "Reply", theme.tokens.tone.ok)
    };

    // Meta: token count + t/s rate if streaming
    let meta = if is_streaming {
        let tok = stream.and_then(|s| s.token_count).unwrap_or(0);
        let tps = stream
            .and_then(|s| s.started_at)
            .map(|start| {
                let elapsed = start.elapsed().as_secs_f64().max(0.5);
                let rate = tok as f64 / elapsed;
                format!("{} tok · {:.0} t/s", tok, rate)
            })
            .unwrap_or_else(|| format!("{} tok", tok));
        Some(tps)
    } else {
        stream.and_then(|s| s.token_count.map(|n| format!("{} tok", n)))
    };

    // Model badge appended to meta
    let meta = if let Some(model) = stream.and_then(|s| s.model_label.as_ref().map(|m| m.as_str()))
    {
        match meta {
            Some(m) => Some(format!("{} · {}", m, model)),
            None => Some(model.to_string()),
        }
    } else {
        meta
    };

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
        card_header(glyph, label, color, None, theme.tokens.fg.faint),
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

    fn make_msg(role: MessageRole, content: &str) -> MessageItem {
        MessageItem {
            id: "test".into(),
            role,
            content: content.into(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        }
    }

    #[test]
    fn test_role_colors() {
        let theme = Theme::dark();
        assert_eq!(
            role_color(MessageRole::User, &theme),
            theme.tokens.card.user.color
        );
        assert_eq!(
            role_color(MessageRole::Assistant, &theme),
            theme.tokens.tone.ok
        );
    }

    #[test]
    fn test_detect_card_kind_error() {
        assert!(matches!(
            detect_card_kind(&make_msg(
                MessageRole::System,
                "Error: something went wrong"
            )),
            Some(CardKind::Error)
        ));
        assert!(matches!(
            detect_card_kind(&make_msg(MessageRole::System, "✗ build failed")),
            Some(CardKind::Error)
        ));
        // Should NOT flag "failed" mid-sentence
        assert!(detect_card_kind(&make_msg(MessageRole::System, "The build failed")).is_none());
    }

    #[test]
    fn test_detect_card_kind_search() {
        assert!(matches!(
            detect_card_kind(&make_msg(MessageRole::Tool, "Found 5 matches in 3 files")),
            Some(CardKind::Search)
        ));
        assert!(matches!(
            detect_card_kind(&make_msg(
                MessageRole::Tool,
                "12 matches found across 4 files"
            )),
            Some(CardKind::Search)
        ));
    }

    #[test]
    fn test_detect_card_kind_default() {
        // Regular tool message without search/agent keywords
        assert!(detect_card_kind(&make_msg(
            MessageRole::Tool,
            "Command executed successfully."
        ))
        .is_none());
        // Regular system message
        assert!(detect_card_kind(&make_msg(MessageRole::System, "Session started.")).is_none());
    }
}
