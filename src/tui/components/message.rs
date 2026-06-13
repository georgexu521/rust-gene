//! 消息渲染组件
//!
//! Reasonix 风格：Card header (glyph + role + metadata) + Card body

use crate::{
    state::{MessageItem, MessageRole},
    tui::sync_store::TuiMessagePart,
};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
};

mod assistant;
mod notice;
mod reasoning;
mod system_tool;
mod text;
mod user;

use assistant::render_assistant_message;
use notice::render_system_message;
use system_tool::render_tool_message;
use user::render_user_message;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MessageRenderOptions {
    pub reasoning_expanded: bool,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CardKind {
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
        MessageRole::Assistant => {
            render_assistant_message(message, None, theme, None, MessageRenderOptions::default())
        }
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
        MessageRole::Assistant => render_assistant_message(
            message,
            None,
            theme,
            stream,
            MessageRenderOptions::default(),
        ),
        MessageRole::System => render_system_message(message, theme, kind),
        MessageRole::Tool => render_tool_message(message, theme, kind),
    }
}

pub fn render_message_with_options<'a>(
    message: &'a MessageItem,
    _width: usize,
    theme: &'a crate::tui::theme::Theme,
    stream: Option<&StreamMeta>,
    options: MessageRenderOptions,
    parts: Option<&[TuiMessagePart]>,
) -> Paragraph<'a> {
    let kind = detect_card_kind(message);
    match message.role {
        MessageRole::User => render_user_message(message, theme),
        MessageRole::Assistant => render_assistant_message(message, parts, theme, stream, options),
        MessageRole::System => render_system_message(message, theme, kind),
        MessageRole::Tool => render_tool_message(message, theme, kind),
    }
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
    use ratatui::{backend::TestBackend, Terminal};

    fn make_msg(role: MessageRole, content: &str) -> MessageItem {
        MessageItem {
            id: "test".into(),
            role,
            content: content.into(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        }
    }

    fn render_message_text(message: &MessageItem) -> String {
        render_message_text_with_options(message, MessageRenderOptions::default())
    }

    fn render_message_text_with_options(
        message: &MessageItem,
        options: MessageRenderOptions,
    ) -> String {
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let theme = Theme::graphite();
                let paragraph =
                    render_message_with_options(message, 120, &theme, None, options, None);
                frame.render_widget(paragraph, frame.area());
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
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
    fn assistant_reasoning_body_renders_only_when_expanded() {
        let message = make_msg(
            MessageRole::Assistant,
            "<think>private chain\nsecond line</think>\nFinal answer",
        );

        let collapsed = render_message_text(&message);
        let expanded = render_message_text_with_options(
            &message,
            MessageRenderOptions {
                reasoning_expanded: true,
            },
        );

        assert!(!collapsed.contains("private chain"));
        assert!(expanded.contains("private chain"));
        assert!(expanded.contains("Final answer"));
    }

    #[test]
    fn assistant_completed_metadata_renders_in_header() {
        let mut message = make_msg(MessageRole::Assistant, "Final answer");
        message
            .metadata
            .insert("completion_tokens".to_string(), "63".to_string());
        message
            .metadata
            .insert("reasoning_tokens".to_string(), "12".to_string());
        message
            .metadata
            .insert("elapsed_ms".to_string(), "2730".to_string());
        message
            .metadata
            .insert("validation_status".to_string(), "passed".to_string());
        message
            .metadata
            .insert("tool_count".to_string(), "3".to_string());
        message
            .metadata
            .insert("model_label".to_string(), "deepseek-v4-flash".to_string());

        let rendered = render_message_text(&message);

        assert!(rendered.contains("63 tok"));
        assert!(rendered.contains("2.7s"));
        assert!(rendered.contains("validation passed"));
        assert!(rendered.contains("3 tools"));
        assert!(rendered.contains("12 reasoning"));
        assert!(rendered.contains("deepseek-v4-flash"));
    }

    #[test]
    fn assistant_provider_error_uses_error_header_and_clean_body() {
        let message = make_msg(
            MessageRole::Assistant,
            "[Error: Failed to get response from deepseek API]",
        );

        let rendered = render_message_text(&message);

        assert!(rendered.contains("Error"));
        assert!(rendered.contains("Failed to get response from deepseek API"));
        assert!(!rendered.contains("Reply"));
        assert!(!rendered.contains("[Error:"));
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

    #[test]
    fn assistant_message_collapses_think_blocks() {
        let rendered = render_message_text(&make_msg(
            MessageRole::Assistant,
            "<think>private reasoning</think>\nVisible answer",
        ));

        assert!(rendered.contains("Thinking hidden"));
        assert!(rendered.contains("Visible answer"));
        assert!(!rendered.contains("<think>"));
        assert!(!rendered.contains("private reasoning"));
    }
}
