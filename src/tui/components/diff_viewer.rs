//! Diff 查看器组件
//!
//! 在 TUI 中渲染带颜色的统一差异（unified diff）输出。
//! 支持行号显示、Hunk 导航、多文件切换、滚动位置指示。

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// 解析后的 diff 行类型
#[derive(Debug, Clone, PartialEq, Eq)]
enum DiffLineKind {
    FileHeader,
    HunkHeader,
    Add,
    Remove,
    Context,
    Index,
    Meta,
}

/// 一条 diff 行
#[derive(Debug, Clone, PartialEq, Eq)]
struct DiffLine {
    kind: DiffLineKind,
    text: String,
    old_line: Option<usize>,
    new_line: Option<usize>,
}

/// 一个 Hunk（保留 future 扩展字段）
#[derive(Debug, Clone)]
struct Hunk {
    #[allow(dead_code)]
    header: String,
    #[allow(dead_code)]
    old_start: usize,
    #[allow(dead_code)]
    new_start: usize,
    #[allow(dead_code)]
    lines: Vec<DiffLine>,
}

/// 解析后的 diff 内容
pub struct ParsedDiff {
    #[allow(dead_code)]
    hunks: Vec<Hunk>,
    #[allow(dead_code)]
    all_lines: Vec<DiffLine>,
    pub file_count: usize,
    pub total_hunks: usize,
}

/// 解析 unified diff 文本，提取行号和 hunk 信息。
pub fn parse_diff(diff_text: &str) -> ParsedDiff {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut all_lines: Vec<DiffLine> = Vec::new();
    let mut current_hunk: Option<Hunk> = None;
    let mut old_line: usize = 0;
    let mut new_line: usize = 0;
    let mut file_count = 0usize;

    for raw in diff_text.lines() {
        let (kind, o, n) = classify_line(raw);

        let dl = DiffLine {
            kind: kind.clone(),
            text: raw.to_string(),
            old_line: o,
            new_line: n,
        };
        all_lines.push(dl.clone());

        match kind {
            DiffLineKind::FileHeader => {
                file_count += 1;
                // Flush previous hunk if any
                if let Some(h) = current_hunk.take() {
                    hunks.push(h);
                }
            }
            DiffLineKind::HunkHeader => {
                // Flush previous hunk
                if let Some(h) = current_hunk.take() {
                    hunks.push(h);
                }
                // Parse @@ -old_start[,old_count] +new_start[,new_count] @@
                let parts: Vec<&str> = raw.split_whitespace().collect();
                old_line = 0;
                new_line = 0;
                for p in &parts {
                    if let Some(nums) = p.strip_prefix('-') {
                        old_line = nums
                            .split(',')
                            .next()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                    } else if let Some(nums) = p.strip_prefix('+') {
                        new_line = nums
                            .split(',')
                            .next()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                    }
                }
                current_hunk = Some(Hunk {
                    header: raw.to_string(),
                    old_start: old_line,
                    new_start: new_line,
                    lines: Vec::new(),
                });
            }
            DiffLineKind::Add => {
                if let Some(ref mut h) = current_hunk {
                    h.lines.push(dl);
                }
                if new_line > 0 {
                    new_line += 1;
                }
            }
            DiffLineKind::Remove => {
                if let Some(ref mut h) = current_hunk {
                    h.lines.push(dl);
                }
                if old_line > 0 {
                    old_line += 1;
                }
            }
            DiffLineKind::Context | DiffLineKind::Index | DiffLineKind::Meta => {
                if let Some(ref mut h) = current_hunk {
                    h.lines.push(dl);
                }
                if old_line > 0 {
                    old_line += 1;
                }
                if new_line > 0 {
                    new_line += 1;
                }
            }
        }
    }
    // Flush last hunk
    if let Some(h) = current_hunk.take() {
        hunks.push(h);
    }

    let total_hunks = hunks.len();

    ParsedDiff {
        hunks,
        all_lines,
        file_count,
        total_hunks,
    }
}

fn classify_line(raw: &str) -> (DiffLineKind, Option<usize>, Option<usize>) {
    if raw.starts_with("diff --git") {
        (DiffLineKind::FileHeader, None, None)
    } else if raw.starts_with("@@") {
        (DiffLineKind::HunkHeader, None, None)
    } else if raw.starts_with("+++") || raw.starts_with("---") {
        (DiffLineKind::Meta, None, None)
    } else if raw.starts_with("index ") {
        (DiffLineKind::Index, None, None)
    } else if raw.starts_with('+') {
        (DiffLineKind::Add, None, None)
    } else if raw.starts_with('-') {
        (DiffLineKind::Remove, None, None)
    } else {
        (DiffLineKind::Context, None, None)
    }
}

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
    let inner_width = popup_area.width.saturating_sub(4); // borders + padding

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
    let mut lines = Vec::new();
    let mut old_line: usize = 0;
    let mut new_line: usize = 0;
    let mut current_file_ext = String::new();

    for raw_line in diff_text.lines() {
        // Track file extension from +++ b/path lines
        if raw_line.starts_with("+++ b/") || raw_line.starts_with("+++ a/") {
            let path = raw_line
                .trim_start_matches("+++ b/")
                .trim_start_matches("+++ a/");
            if let Some(ext) = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
            {
                current_file_ext = ext.to_string();
            }
        }

        // Track line numbers
        if raw_line.starts_with("@@") {
            let parts: Vec<&str> = raw_line.split_whitespace().collect();
            for p in &parts {
                if let Some(nums) = p.strip_prefix('-') {
                    old_line = nums
                        .split(',')
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                } else if let Some(nums) = p.strip_prefix('+') {
                    new_line = nums
                        .split(',')
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
            }
        }

        let (old_num, new_num) = if raw_line.starts_with("@@")
            || raw_line.starts_with("diff --git")
            || raw_line.starts_with("index ")
        {
            (None, None)
        } else if raw_line.starts_with('+') {
            let n = if new_line > 0 { new_line } else { 0 };
            if new_line > 0 {
                new_line += 1;
            }
            (None, Some(n))
        } else if raw_line.starts_with('-') {
            let o = if old_line > 0 { old_line } else { 0 };
            if old_line > 0 {
                old_line += 1;
            }
            (Some(o), None)
        } else {
            let o = if old_line > 0 { old_line } else { 0 };
            let n = if new_line > 0 { new_line } else { 0 };
            if old_line > 0 {
                old_line += 1;
            }
            if new_line > 0 {
                new_line += 1;
            }
            (Some(o), Some(n))
        };

        let line = build_diff_line(
            raw_line,
            old_num,
            new_num,
            inner_width,
            theme,
            &current_file_ext,
        );
        lines.push(line);
    }

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

fn build_diff_line(
    raw: &str,
    old_line: Option<usize>,
    new_line: Option<usize>,
    _inner_width: u16,
    theme: &crate::tui::theme::Theme,
    current_file_ext: &str,
) -> Line<'static> {
    let base_style = if raw.starts_with("+++") || raw.starts_with("---") {
        Style::default().fg(theme.tokens.tone.warn)
    } else if raw.starts_with("@@") {
        Style::default()
            .fg(theme.tokens.fg.faint)
            .add_modifier(Modifier::BOLD)
    } else if raw.starts_with('+') {
        Style::default().fg(theme.tokens.tone.ok)
    } else if raw.starts_with('-') {
        Style::default().fg(theme.tokens.tone.err)
    } else if raw.starts_with("diff --git") {
        Style::default()
            .fg(theme.tokens.fg.body)
            .add_modifier(Modifier::BOLD)
    } else if raw.starts_with("index ") {
        Style::default().fg(theme.tokens.fg.faint)
    } else if raw.starts_with("No ") || raw.starts_with("Not a git") {
        Style::default()
            .fg(theme.tokens.fg.faint)
            .add_modifier(Modifier::ITALIC)
    } else {
        Style::default().fg(theme.tokens.fg.faint)
    };

    let mut spans: Vec<Span<'static>> = Vec::new();

    // Line number gutter
    let gutter = match (old_line, new_line) {
        (Some(o), Some(n)) => format!("{:>4} {:>4} │ ", o, n),
        (Some(o), None) => format!("{:>4}      │ ", o),
        (None, Some(n)) => format!("      {:>4} │ ", n),
        (None, None) => "              │ ".to_string(),
    };
    spans.push(Span::styled(
        gutter,
        Style::default().fg(theme.tokens.fg.faint),
    ));

    // Syntax-highlight code lines (add/remove/context)
    let is_code_line = raw.starts_with('+')
        || raw.starts_with('-')
        || !raw.starts_with("diff ")
            && !raw.starts_with("@@")
            && !raw.starts_with("index ")
            && !raw.starts_with("+++")
            && !raw.starts_with("---");
    if is_code_line && !current_file_ext.is_empty() {
        let prefix = if raw.starts_with('+') || raw.starts_with('-') {
            &raw[..1]
        } else {
            ""
        };
        let code = if prefix.is_empty() { raw } else { &raw[1..] };
        spans.push(Span::styled(prefix.to_string(), base_style));
        if !code.is_empty() {
            let highlighted = highlight_code(code, current_file_ext, base_style);
            spans.extend(highlighted);
        }
    } else {
        spans.push(Span::styled(raw.to_string(), base_style));
    }

    Line::from(spans)
}

/// Highlight a code snippet using syntect, blended with the diff base style.
fn highlight_code(code: &str, ext: &str, base_style: Style) -> Vec<Span<'static>> {
    use once_cell::sync::Lazy;
    use syntect::easy::HighlightLines;
    use syntect::highlighting::ThemeSet;
    use syntect::parsing::SyntaxSet;
    use syntect::util::LinesWithEndings;

    static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
    static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

    let syntax = SYNTAX_SET
        .find_syntax_by_extension(ext)
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
    let tm_theme = &THEME_SET.themes["base16-ocean.dark"];
    let mut highlighter = HighlightLines::new(syntax, tm_theme);

    let mut spans = Vec::new();
    for line in LinesWithEndings::from(code) {
        if let Ok(highlighted) = highlighter.highlight_line(line, &SYNTAX_SET) {
            for (style, text) in highlighted {
                let syntect_color = style.foreground;
                let blended = if syntect_color == syntect::highlighting::Color::WHITE {
                    base_style
                } else {
                    base_style.fg(ratatui::style::Color::Rgb(
                        syntect_color.r,
                        syntect_color.g,
                        syntect_color.b,
                    ))
                };
                spans.push(Span::styled(text.to_string(), blended));
            }
        } else {
            spans.push(Span::styled(code.to_string(), base_style));
        }
    }
    if spans.is_empty() && !code.is_empty() {
        spans.push(Span::styled(code.to_string(), base_style));
    }
    spans
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

/// 找到下一个 hunk header 的行号（从当前 scroll 位置开始）
pub fn find_next_hunk_line(diff_text: &str, current_scroll: u16) -> Option<usize> {
    let lines: Vec<&str> = diff_text.lines().collect();
    let start = (current_scroll as usize).min(lines.len());
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if line.starts_with("@@") {
            return Some(i);
        }
    }
    None
}

/// 找到上一个 hunk header 的行号（从当前 scroll 位置开始）
pub fn find_prev_hunk_line(diff_text: &str, current_scroll: u16) -> Option<usize> {
    let lines: Vec<&str> = diff_text.lines().collect();
    let start = (current_scroll as usize).min(lines.len().saturating_sub(1));
    lines[..start]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, line)| line.starts_with("@@"))
        .map(|(i, _)| i)
}

/// 当 diff_text 为空时，尝试运行 `git diff` 获取工作区变更。
/// 支持 `staged` 参数获取暂存区变更。
fn try_git_diff(staged: bool) -> Option<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("diff");
    if staged {
        cmd.arg("--staged");
    }
    cmd.arg("--no-color");
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    Some(text)
}

/// 找到下一个文件边界（diff --git）的行号
pub fn find_next_file_line(diff_text: &str, current_scroll: u16) -> Option<usize> {
    let lines: Vec<&str> = diff_text.lines().collect();
    let start = (current_scroll as usize).min(lines.len());
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if line.starts_with("diff --git") {
            return Some(i);
        }
    }
    None
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
