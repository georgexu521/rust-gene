//! Pure collapsible block helpers for long TUI output.
//!
//! Renderers stay callback-free: they produce visible `Line`s plus overflow
//! metadata, and input handling toggles expansion state in `TuiApp`.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub const DEFAULT_TOOL_BODY_MAX_LINES: usize = 20;
pub const DEFAULT_TEXT_PART_MAX_LINES: usize = 48;

#[derive(Debug, Clone)]
pub struct CollapsibleBlock {
    pub visible: Vec<Line<'static>>,
    pub is_truncated: bool,
    pub hidden_lines: usize,
    pub hidden_chars: usize,
}

/// Truncate a pre-rendered line list to a line/character budget.
pub fn collapse_lines(
    lines: Vec<Line<'static>>,
    max_lines: usize,
    max_chars: usize,
) -> CollapsibleBlock {
    if max_lines == 0 {
        let hidden_chars = char_count_for_lines(&lines);
        return CollapsibleBlock {
            visible: Vec::new(),
            is_truncated: !lines.is_empty() || hidden_chars > 0,
            hidden_lines: lines.len(),
            hidden_chars,
        };
    }

    let total_chars = char_count_for_lines(&lines);
    let mut visible_count = 0usize;
    let mut visible_chars = 0usize;

    for line in &lines {
        let line_chars = char_count_for_line(line);
        if visible_count < max_lines && visible_chars + line_chars <= max_chars {
            visible_count += 1;
            visible_chars += line_chars;
        } else {
            break;
        }
    }

    let hidden_lines = lines.len().saturating_sub(visible_count);
    let hidden_chars = total_chars.saturating_sub(visible_chars);
    let visible = lines.into_iter().take(visible_count).collect();

    CollapsibleBlock {
        visible,
        is_truncated: hidden_lines > 0 || hidden_chars > 0,
        hidden_lines,
        hidden_chars,
    }
}

/// Build a standard "... N more lines - press Enter to expand" footer.
pub fn collapse_footer(hidden_lines: usize, theme: &crate::tui::theme::Theme) -> Line<'static> {
    let text = if hidden_lines > 0 {
        format!("... {} more lines - press Enter to expand", hidden_lines)
    } else {
        "... more - press Enter to expand".to_string()
    };
    Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            text,
            Style::default()
                .fg(theme.tokens.fg.faint)
                .add_modifier(Modifier::ITALIC),
        ),
    ])
}

/// Wrap a single logical line into physical display lines no wider than
/// `width` cells, respecting Unicode width.
pub fn wrap_line_to_width(line: &str, width: usize) -> Vec<String> {
    if width == 0 || unicode_width::UnicodeWidthStr::width(line) <= width {
        return vec![line.to_string()];
    }
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    for ch in line.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + w > width && !current.is_empty() {
            pieces.push(current.clone());
            current.clear();
            current_width = 0;
        }
        current.push(ch);
        current_width += w;
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
}

fn char_count_for_lines(lines: &[Line<'_>]) -> usize {
    lines.iter().map(char_count_for_line).sum()
}

fn char_count_for_line(line: &Line<'_>) -> usize {
    line.spans.iter().map(|s| s.content.chars().count()).sum()
}

/// Convenience budget for tool bodies: character budget is `width * max_lines`.
pub fn tool_body_budget(width: usize, max_lines: usize) -> usize {
    width.saturating_mul(max_lines)
}

/// Flatten any `\n` characters inside span contents into separate `Line`s.
///
/// Markdown parsers may emit a single `Line` whose spans contain newlines.
/// Collapsing by line count needs those newlines turned into real `Line`s.
pub fn flatten_line_breaks(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();

    for line in lines {
        for span in line.spans {
            let style = span.style;
            let mut pieces = span.content.split('\n').peekable();
            while let Some(piece) = pieces.next() {
                if !piece.is_empty() {
                    current.push(Span::styled(piece.to_string(), style));
                }
                if pieces.peek().is_some() {
                    out.push(Line::from(std::mem::take(&mut current)));
                }
            }
        }
        if !current.is_empty() || out.is_empty() {
            out.push(Line::from(std::mem::take(&mut current)));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::Theme;
    use ratatui::text::Span;

    #[test]
    fn collapse_short_content_is_not_truncated() {
        let lines = vec![Line::from("short")];
        let block = collapse_lines(lines, 10, 100);
        assert!(!block.is_truncated);
        assert_eq!(block.hidden_lines, 0);
    }

    #[test]
    fn collapse_limits_by_lines() {
        let lines: Vec<Line<'_>> = (0..10).map(|i| Line::from(format!("line {}", i))).collect();
        let block = collapse_lines(lines, 3, 1000);
        assert!(block.is_truncated);
        assert_eq!(block.visible.len(), 3);
        assert_eq!(block.hidden_lines, 7);
    }

    #[test]
    fn collapse_limits_by_chars() {
        let lines: Vec<Line<'_>> = (0..3)
            .map(|_| Line::from(Span::raw("aaaaaaaaaa")))
            .collect();
        let block = collapse_lines(lines, 10, 15);
        assert!(block.is_truncated);
        assert_eq!(block.visible.len(), 1);
    }

    #[test]
    fn footer_shows_hidden_line_count() {
        let theme = Theme::default();
        let footer = collapse_footer(5, &theme);
        let text: String = footer.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("5 more lines"));
    }

    #[test]
    fn wrap_line_respects_unicode_width() {
        let pieces = wrap_line_to_width("中文中文", 4);
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0], "中文");
    }
}
