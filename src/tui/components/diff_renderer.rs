//! Shared diff rendering utilities.
//!
//! Provides a reusable unified-diff line builder that can be used both inside
//! popups (`diff_viewer.rs`) and inline tool cards (`file_edit`, `file_patch`).

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

/// Parsed unified diff metadata.
#[derive(Debug, Clone)]
pub struct ParsedDiff {
    pub hunks: Vec<Hunk>,
    pub all_lines: Vec<DiffLine>,
    pub file_count: usize,
    pub total_hunks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineKind {
    FileHeader,
    HunkHeader,
    Add,
    Remove,
    Context,
    Index,
    Meta,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub header: String,
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<DiffLine>,
}

/// Parse unified diff text into structured lines and hunks.
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
                if let Some(h) = current_hunk.take() {
                    hunks.push(h);
                }
            }
            DiffLineKind::HunkHeader => {
                if let Some(h) = current_hunk.take() {
                    hunks.push(h);
                }
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

/// Build styled `Line`s from a unified diff string.
///
/// Returns the rendered lines plus `(file_count, total_hunks)` metadata.
/// This is the shared renderer used by the diff popup and inline tool cards.
pub fn build_diff_lines(
    diff_text: &str,
    theme: &crate::tui::theme::Theme,
    include_gutter: bool,
) -> (Vec<Line<'static>>, usize, usize) {
    let parsed = parse_diff(diff_text);
    let mut lines = Vec::new();
    let mut old_line: usize = 0;
    let mut new_line: usize = 0;
    let mut current_file_ext = String::new();

    for raw_line in diff_text.lines() {
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
            include_gutter,
            theme,
            &current_file_ext,
        );
        lines.push(line);
    }

    (lines, parsed.file_count, parsed.total_hunks)
}

/// 当 diff_text 为空时，尝试运行 `git diff` 获取工作区变更。
/// 支持 `staged` 参数获取暂存区变更。
pub fn try_git_diff(staged: bool) -> Option<String> {
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

fn build_diff_line(
    raw: &str,
    old_line: Option<usize>,
    new_line: Option<usize>,
    include_gutter: bool,
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

    if include_gutter {
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
    }

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
pub fn highlight_code(code: &str, ext: &str, base_style: Style) -> Vec<Span<'static>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_diff_extracts_hunks_and_files() {
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
    fn build_diff_lines_include_gutter() {
        let theme = crate::tui::theme::Theme::default();
        let diff =
            "@@ -1,3 +1,4 @@\n fn main() {\n-    println!(\"old\");\n+    println!(\"new\");\n }";
        let (lines, _, _) = build_diff_lines(diff, &theme, true);
        assert!(!lines.is_empty());
        let text = lines[1]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect::<String>();
        assert!(text.contains('│'));
    }

    #[test]
    fn build_diff_lines_omit_gutter() {
        let theme = crate::tui::theme::Theme::default();
        let diff =
            "@@ -1,3 +1,4 @@\n fn main() {\n-    println!(\"old\");\n+    println!(\"new\");\n }";
        let (lines, _, _) = build_diff_lines(diff, &theme, false);
        let text = lines[1]
            .spans
            .iter()
            .map(|s| s.content.to_string())
            .collect::<String>();
        assert!(!text.contains('│'));
    }
}
