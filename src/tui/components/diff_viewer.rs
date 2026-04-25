//! Diff 查看器组件
//!
//! 在 TUI 中渲染带颜色的统一差异（unified diff）输出

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// 渲染 Diff 查看器弹窗
pub fn render_diff_viewer(
    f: &mut Frame,
    diff_text: &str,
    title: &str,
    scroll_offset: u16,
    area: Rect,
    theme: &crate::tui::theme::Theme,
) {
    let popup_area = centered_rect(90, 85, area);

    let block = Block::default()
        .title(format!(" Diff: {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_active))
        .style(Style::default().bg(theme.bg_popup));

    let mut lines = Vec::new();

    for raw_line in diff_text.lines() {
        let line = if raw_line.starts_with("+++") || raw_line.starts_with("---") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.diff_header),
            ))
        } else if raw_line.starts_with("@@") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default()
                    .fg(theme.diff_line_number)
                    .add_modifier(Modifier::BOLD),
            ))
        } else if raw_line.starts_with('+') {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.diff_add),
            ))
        } else if raw_line.starts_with('-') {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.diff_remove),
            ))
        } else if raw_line.starts_with("diff --git") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ))
        } else if raw_line.starts_with("index ") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.border),
            ))
        } else if raw_line.starts_with("No ") || raw_line.starts_with("Not a git") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default()
                    .fg(theme.text_dim)
                    .add_modifier(Modifier::ITALIC),
            ))
        } else {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.text_dim),
            ))
        };
        lines.push(line);
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No differences found.",
            Style::default()
                .fg(theme.text_dim)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    // 添加底部提示
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Press ", Style::default().fg(theme.border)),
        Span::styled(
            "Esc/q",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" close  ", Style::default().fg(theme.border)),
        Span::styled(
            "↑/↓",
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" scroll  ", Style::default().fg(theme.border)),
        Span::styled(
            "PgUp/PgDn",
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" page", Style::default().fg(theme.border)),
    ]));

    let total_lines = lines.len().saturating_sub(1) as u16;
    let scroll = scroll_offset.min(total_lines);

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

/// 计算居中矩形
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
