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
        .border_style(Style::default().fg(theme.tokens.tone.brand))
        .style(Style::default().bg(theme.tokens.surface.bg_elev));

    let mut lines = Vec::new();

    for raw_line in diff_text.lines() {
        let line = if raw_line.starts_with("+++") || raw_line.starts_with("---") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.tokens.tone.warn),
            ))
        } else if raw_line.starts_with("@@") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default()
                    .fg(theme.tokens.fg.faint)
                    .add_modifier(Modifier::BOLD),
            ))
        } else if raw_line.starts_with('+') {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.tokens.tone.ok),
            ))
        } else if raw_line.starts_with('-') {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.tokens.tone.err),
            ))
        } else if raw_line.starts_with("diff --git") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default()
                    .fg(theme.tokens.fg.body)
                    .add_modifier(Modifier::BOLD),
            ))
        } else if raw_line.starts_with("index ") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.tokens.fg.faint),
            ))
        } else if raw_line.starts_with("No ") || raw_line.starts_with("Not a git") {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default()
                    .fg(theme.tokens.fg.faint)
                    .add_modifier(Modifier::ITALIC),
            ))
        } else {
            Line::from(Span::styled(
                raw_line.to_string(),
                Style::default().fg(theme.tokens.fg.faint),
            ))
        };
        lines.push(line);
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No differences found.",
            Style::default()
                .fg(theme.tokens.fg.faint)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    // 添加底部提示
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Press ", Style::default().fg(theme.tokens.fg.faint)),
        Span::styled(
            "Esc/q",
            Style::default()
                .fg(theme.tokens.tone.warn)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" close  ", Style::default().fg(theme.tokens.fg.faint)),
        Span::styled(
            "↑/↓",
            Style::default()
                .fg(theme.tokens.tone.info)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" scroll  ", Style::default().fg(theme.tokens.fg.faint)),
        Span::styled(
            "PgUp/PgDn",
            Style::default()
                .fg(theme.tokens.tone.info)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" page", Style::default().fg(theme.tokens.fg.faint)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_diff_text(diff_text: &str, title: &str) -> String {
        let backend = TestBackend::new(160, 70);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = crate::tui::theme::Theme::default();
        terminal
            .draw(|frame| {
                render_diff_viewer(frame, diff_text, title, 0, frame.area(), &theme);
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
    fn render_diff_viewer_shows_unified_diff_and_controls() {
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
index 1111111..2222222 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
+    println!(\"done\");
 }";

        let rendered = render_diff_text(diff, "Working tree");

        assert!(rendered.contains("Diff: Working tree"));
        assert!(rendered.contains("diff --git a/src/main.rs b/src/main.rs"));
        assert!(rendered.contains("--- a/src/main.rs"));
        assert!(rendered.contains("+++ b/src/main.rs"));
        assert!(rendered.contains("@@ -1,3 +1,4 @@"));
        assert!(rendered.contains("-    println!(\"old\");"));
        assert!(rendered.contains("+    println!(\"new\");"));
        assert!(rendered.contains("+    println!(\"done\");"));
        assert!(rendered.contains("Esc/q"));
        assert!(rendered.contains("PgUp/PgDn"));
    }

    #[test]
    fn render_diff_viewer_shows_empty_diff_message() {
        let rendered = render_diff_text("", "No changes");

        assert!(rendered.contains("Diff: No changes"));
        assert!(rendered.contains("No differences found."));
        assert!(rendered.contains("Esc/q"));
    }
}
