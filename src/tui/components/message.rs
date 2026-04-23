//! 消息渲染组件
//!
//! 渲染不同类型的消息（用户、助手、系统、工具）

use crate::state::{MessageItem, MessageRole};
use crate::tui::components::markdown::parse_markdown;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
};

/// 渲染消息为 Paragraph
pub fn render_message<'a>(
    message: &'a MessageItem,
    _width: usize,
    theme: &'a crate::tui::theme::Theme,
) -> Paragraph<'a> {
    let (prefix, color) = match message.role {
        MessageRole::User => ("You", theme.user_message),
        MessageRole::Assistant => ("Assistant", theme.assistant_message),
        MessageRole::System => ("System", theme.system_message),
        MessageRole::Tool => ("Tool", theme.tool_message),
    };

    let mut lines = Vec::new();

    // 消息头部
    let header = Line::from(vec![
        Span::styled(
            format!("{} ", prefix),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format_time(&message.timestamp),
            Style::default().fg(theme.text_dim),
        ),
    ]);
    lines.push(header);
    lines.push(Line::from(""));

    // 消息内容（所有消息统一使用 Markdown 渲染）
    let markdown_text = parse_markdown(&message.content, theme);
    for line in markdown_text.lines {
        lines.push(line);
    }

    // 工具调用信息（如果有）
    if let Some(tool_call_id) = message.metadata.get("tool_call_id") {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Tool call: ", Style::default().fg(theme.text_dim)),
            Span::styled(tool_call_id, Style::default().fg(theme.tool_message)),
        ]));
    }

    // 底部空行
    lines.push(Line::from(""));

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

/// 格式化时间
fn format_time(time: &std::time::SystemTime) -> String {
    use chrono::{DateTime, Local};
    let datetime: DateTime<Local> = (*time).into();
    datetime.format("%H:%M:%S").to_string()
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
