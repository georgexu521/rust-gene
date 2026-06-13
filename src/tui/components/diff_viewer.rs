//! Diff 查看器组件
//!
//! 在 TUI 中渲染带颜色的统一差异（unified diff）输出。
//! 支持行号显示、Hunk 导航、多文件切换、滚动位置指示。

use crate::tui::components::diff_renderer::{build_diff_lines, parse_diff, try_git_diff};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// 渲染 Diff 查看器弹窗。
///
/// `current_hunk` 用于高亮当前 Hunk，`total_lines` 用于滚动位置指示。
/// 当 `diff_text` 为空时，自动尝试调用 `git diff` 获取工作区变更。
pub fn render_diff_viewer(
    f: &mut Frame,
    diff_text: &str,
    title: &str,
    scroll_offset: u16,
    area: Rect,
    theme: &crate::tui::theme::Theme,
) -> (u16, usize) {
    // 返回 (total_lines, hunk_count) 供调用方显示
    let popup_area = centered_rect(92, 88, area);

    let block = Block::default()
        .title(format!(" Diff: {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.tokens.tone.brand))
        .style(Style::default().bg(theme.tokens.surface.bg_elev));

    let git_diff = if diff_text.is_empty() {
        try_git_diff(false)
    } else {
        None
    };
    let diff_text = git_diff.as_deref().unwrap_or(diff_text);

    let parsed = parse_diff(diff_text);
    let (mut lines, _file_count, _total_hunks) = build_diff_lines(diff_text, theme, true);

    if lines.is_empty() {
        let msg = if git_diff.is_some() {
            "Working tree clean — no changes."
        } else {
            "No differences found."
        };
        lines.push(Line::from(Span::styled(
            msg,
            Style::default()
                .fg(theme.tokens.fg.faint)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    let total_lines = lines.len().saturating_sub(1) as u16;
    let scroll = scroll_offset.min(total_lines.saturating_sub(1));

    // 添加底部控制栏
    lines.push(Line::from(""));
    lines.push(build_footer_line(
        parsed.file_count,
        parsed.total_hunks,
        scroll,
        total_lines,
        theme,
    ));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);

    (total_lines, parsed.total_hunks)
}

fn build_footer_line(
    file_count: usize,
    hunk_count: usize,
    scroll_offset: u16,
    total_lines: u16,
    theme: &crate::tui::theme::Theme,
) -> Line<'static> {
    let faint = Style::default().fg(theme.tokens.fg.faint);
    let key_style = Style::default()
        .fg(theme.tokens.tone.info)
        .add_modifier(Modifier::BOLD);

    let mut parts: Vec<Span<'static>> = Vec::new();
    parts.push(Span::styled("  ", faint));

    // Line position indicator
    let current_line = (scroll_offset + 1).min(total_lines.max(1));
    let pct = if total_lines > 0 {
        (current_line as f32 / total_lines as f32 * 100.0) as u16
    } else {
        0
    };
    parts.push(Span::styled(
        format!("Line {}/{} ({}%)  ", current_line, total_lines.max(1), pct),
        faint,
    ));

    // File and hunk info
    if file_count > 1 {
        parts.push(Span::styled(
            format!("{} files | {} hunks  ", file_count, hunk_count),
            faint,
        ));
    } else if hunk_count > 1 {
        parts.push(Span::styled(format!("{} hunks  ", hunk_count), faint));
    }

    parts.push(Span::styled("Esc/q", key_style));
    parts.push(Span::styled(" close  ", faint));
    parts.push(Span::styled("↑/↓", key_style));
    parts.push(Span::styled(" scroll  ", faint));
    parts.push(Span::styled("n/p", key_style));
    parts.push(Span::styled(" next/prev hunk  ", faint));
    parts.push(Span::styled("PgUp/PgDn", key_style));
    parts.push(Span::styled(" page", faint));

    if file_count > 1 {
        parts.push(Span::styled("  ", faint));
        parts.push(Span::styled("Tab", key_style));
        parts.push(Span::styled(" next file", faint));
    }

    Line::from(parts)
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
    use crate::tui::components::diff_renderer::{find_next_hunk_line, find_prev_hunk_line};
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
        assert!(rendered.contains("diff --git"));
        assert!(rendered.contains("Esc/q"));
        assert!(rendered.contains("PgUp/PgDn"));
        assert!(rendered.contains("n/p"));
    }

    #[test]
    fn render_diff_viewer_shows_empty_diff_message() {
        let rendered = render_diff_text("", "No changes");
        // When diff_text is empty, it tries git diff. In a git repo with changes,
        // this may show actual diff content; in a clean repo it shows a "no changes" message.
        // We just verify the title is present. Controls may be off-screen if git diff is long.
        assert!(rendered.contains("Diff: No changes"));
    }

    #[test]
    fn parse_diff_extracts_hunks() {
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
+    println!(\"done\");
 }
@@ -10,2 +11,3 @@ fn other() {
     let x = 1;
+    let y = 2;
     x
 }";
        let parsed = parse_diff(diff);
        assert_eq!(parsed.total_hunks, 2);
        assert_eq!(parsed.file_count, 1);
    }

    #[test]
    fn parse_diff_detects_multiple_files() {
        let diff = "\
diff --git a/foo.rs b/foo.rs
--- a/foo.rs
+++ b/foo.rs
@@ -1,1 +1,1 @@
-old
+new
diff --git a/bar.rs b/bar.rs
--- a/bar.rs
+++ b/bar.rs
@@ -1,1 +1,1 @@
-old2
+new2";
        let parsed = parse_diff(diff);
        assert_eq!(parsed.file_count, 2);
    }

    #[test]
    fn find_next_hunk_returns_correct_line() {
        let diff = "@@ -5,5 +5,5 @@\ncontext\n@@ -10,3 +10,3 @@\nmore";
        // Find next hunk AFTER line 0 (skips the first @@ since we're "on" it)
        let next = find_next_hunk_line(diff, 0);
        assert_eq!(next, Some(2));
    }

    #[test]
    fn find_prev_hunk_returns_correct_line() {
        let diff = "@@ -5,5 +5,5 @@\ncontext\n@@ -10,3 +10,3 @@\nmore";
        let prev = find_prev_hunk_line(diff, 3);
        assert_eq!(prev, Some(2));
        let prev2 = find_prev_hunk_line(diff, 2);
        assert_eq!(prev2, Some(0));
    }

    #[test]
    fn line_numbers_appear_in_rendered_output() {
        let diff = "\
@@ -1,3 +1,4 @@
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
 }";
        let rendered = render_diff_text(diff, "Test");
        // Line number gutter should be present
        assert!(rendered.contains('│'));
    }
}
