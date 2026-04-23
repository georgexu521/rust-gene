//! 消息渲染组件
//!
//! Claude Code 风格：简洁、无边框、用留白和颜色区分角色

use crate::state::{MessageItem, MessageRole};
use crate::tui::components::markdown::parse_markdown;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
};

/// 渲染消息为 Paragraph（Claude Code 风格）
pub fn render_message<'a>(
    message: &'a MessageItem,
    _width: usize,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    match message.role {
        MessageRole::User => render_user_message(message, theme),
        MessageRole::Assistant => render_assistant_message(message, theme),
        MessageRole::System => render_system_message(message, theme),
        MessageRole::Tool => render_tool_message(message, theme),
    }
}

fn render_user_message<'a>(message: &'a MessageItem, theme: &'a crate::tui::theme::Theme) -> Paragraph<'a> {
    let markdown_text = parse_markdown(&message.content, theme);
    let mut lines = Vec::new();
    // 用户消息：无额外前缀，内容直接展示
    for line in markdown_text.lines {
        lines.push(line);
    }
    Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(theme.user_message_bg))
}

fn render_assistant_message<'a>(message: &'a MessageItem, theme: &'a crate::tui::theme::Theme) -> Paragraph<'a> {
    let markdown_text = parse_markdown(&message.content, theme);
    let mut lines = Vec::new();
    for line in markdown_text.lines {
        // 助手消息第一行加 ● 前缀，其余行缩进对齐
        let is_first = lines.is_empty();
        let mut spans = Vec::new();
        if is_first {
            spans.push(Span::styled("● ", Style::default().fg(theme.assistant_message)));
        } else {
            spans.push(Span::styled("  ", Style::default()));
        }
        for span in line.spans {
            spans.push(span);
        }
        lines.push(Line::from(spans));
    }
    Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true })
}

fn render_system_message<'a>(message: &'a MessageItem, theme: &'a crate::tui::theme::Theme) -> Paragraph<'a> {
    let markdown_text = parse_markdown(&message.content, theme);
    let mut lines = Vec::new();
    for line in markdown_text.lines {
        let mut spans = Vec::new();
        spans.push(Span::styled("  ", Style::default()));
        for span in line.spans {
            spans.push(span);
        }
        lines.push(Line::from(spans));
    }
    Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(theme.text_dim).add_modifier(Modifier::ITALIC))
}

fn render_tool_message<'a>(message: &'a MessageItem, theme: &'a crate::tui::theme::Theme) -> Paragraph<'a> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("⎿ ", Style::default().fg(theme.text_dim)),
        Span::styled(&message.content, Style::default().fg(theme.text_dim)),
    ]));
    Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true })
}

/// 简化版本的消息渲染（用于紧凑显示）
pub fn render_message_compact<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    let (prefix, color) = match message.role {
        MessageRole::User => ("▸", theme.user_message),
        MessageRole::Assistant => ("◆", theme.assistant_message),
        MessageRole::System => ("●", theme.system_message),
        MessageRole::Tool => ("▪", theme.tool_message),
    };

    let content = if message.content.len() > 100 {
        let truncated: String = message.content.chars().take(100).collect();
        format!("{}...", truncated)
    } else {
        message.content.clone()
    };

    let text = Text::from(vec![Line::from(vec![
        Span::styled(format!("{} ", prefix), Style::default().fg(color)),
        Span::styled(content, Style::default().fg(theme.text)),
    ])]);

    Paragraph::new(text)
}

/// 获取消息角色的颜色
pub fn role_color(role: MessageRole, theme: &crate::tui::theme::Theme) -> Color {
    match role {
        MessageRole::User => theme.user_message,
        MessageRole::Assistant => theme.assistant_message,
        MessageRole::System => theme.system_message,
        MessageRole::Tool => theme.tool_message,
    }
}

/// 获取消息角色的图标
pub fn role_icon(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "👤",
        MessageRole::Assistant => "🤖",
        MessageRole::System => "⚙️",
        MessageRole::Tool => "🔧",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::Theme;

    #[test]
    fn test_role_colors() {
        let theme = Theme::dark();
        assert_eq!(role_color(MessageRole::User, &theme), theme.user_message);
        assert_eq!(
            role_color(MessageRole::Assistant, &theme),
            theme.assistant_message
        );
    }
}
