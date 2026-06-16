//! Shared text and terminal formatting utilities for the shell.

use std::path::Path;

use crate::shell::constants::TERMINAL_WIDTH_FALLBACK;

/// Compact a line of text by collapsing whitespace and truncating with an
/// ellipsis if it exceeds `max_chars`.
pub fn compact_line(text: &str, max_chars: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max_chars {
        return text;
    }

    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Compact an absolute path by replacing the home directory with `~`.
pub fn compact_home_path(path: &Path) -> String {
    let home = dirs::home_dir();
    if let Some(home) = home.as_ref() {
        if let Ok(stripped) = path.strip_prefix(home) {
            let suffix = stripped.to_string_lossy();
            if suffix.is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", suffix);
        }
    }
    path.display().to_string()
}

/// Render a percentage as an ASCII progress bar of the given width.
pub fn percent_bar(percent: u64, width: usize) -> String {
    let filled = ((percent as usize) * width).div_ceil(100).min(width);
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// Return a horizontal rule of `len` characters in the given ANSI color.
pub fn colored_rule(len: usize, color: &str) -> String {
    if len == 0 {
        String::new()
    } else {
        format!("{color}{}{}", "─".repeat(len), crate::shell::theme::RESET)
    }
}

/// Current terminal width, falling back to 80 columns.
pub fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(width, _)| width as usize)
        .unwrap_or(TERMINAL_WIDTH_FALLBACK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_bar_renders_fixed_width() {
        assert_eq!(percent_bar(0, 4), "[░░░░]");
        assert_eq!(percent_bar(50, 4), "[██░░]");
        assert_eq!(percent_bar(100, 4), "[████]");
    }
}
