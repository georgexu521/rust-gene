//! Terminal renderer for scrollback-first CLI output.
//!
//! All rendering is append-only: lines are written to stdout with ANSI escapes
//! and flushed. No alternate screen buffer is used, so native text selection
//! and scrollback continue to work as in a normal terminal session.

use crate::shell::theme::{BOLD, DIM, RESET};
use std::io::{self, Write};

/// Append a user message to the scrollback.
pub fn print_user_message(text: &str) -> io::Result<()> {
    for line in text.lines() {
        println!("{}│{}{} {}", DIM, RESET, DIM, line);
    }
    io::stdout().flush()
}

/// Append a system/notice line to the scrollback.
pub fn print_notice(text: &str) -> io::Result<()> {
    println!("{}{}{}", DIM, text, RESET);
    io::stdout().flush()
}

/// Clear the current transient status line in place.
pub fn clear_status() -> io::Result<()> {
    print!("\r\x1b[2K");
    io::stdout().flush()
}

/// Print a transient status line that will be overwritten by later output.
pub fn show_status(text: &str, color: &str) -> io::Result<()> {
    print!("\r\x1b[2K{}· {}{}", color, text, RESET);
    io::stdout().flush()
}

/// Render one assistant output line with lightweight Markdown softening.
pub fn render_assistant_line(line: &str, in_code_block: &mut bool) -> String {
    let trimmed = line.trim_end();
    if trimmed.trim_start().starts_with("```") {
        let was_in_code_block = *in_code_block;
        *in_code_block = !*in_code_block;
        let label = trimmed.trim_start().trim_start_matches("```").trim();
        if was_in_code_block {
            return format!("{DIM}╰─{RESET}");
        }
        if label.is_empty() {
            return format!("{DIM}╭─ code{RESET}");
        }
        return format!("{DIM}╭─ {label}{RESET}");
    }

    if *in_code_block {
        return format!("{DIM}│{RESET} {trimmed}");
    }

    if let Some(table_line) = render_markdown_table_line(trimmed) {
        return table_line;
    }

    let cleaned = clean_markdown_inline(trimmed);
    let heading = cleaned.trim_start();
    if heading.starts_with('#') {
        let heading_text = heading.trim_start_matches('#').trim_start();
        return format!("{BOLD}{heading_text}{RESET}");
    }
    if let Some(block_quote) = render_block_quote(&cleaned) {
        return block_quote;
    }
    if let Some(list_item) = render_list_item(&cleaned) {
        return list_item;
    }
    if let Some(numbered_item) = render_numbered_item(&cleaned) {
        return numbered_item;
    }
    cleaned
}

fn render_list_item(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len());
    let marker = ["- ", "* ", "• "]
        .iter()
        .find(|marker| trimmed.starts_with(**marker))?;
    let text = trimmed[marker.len()..].trim_start();
    let spaces = " ".repeat(indent.min(6));
    Some(format!("{spaces}{DIM}•{RESET} {text}"))
}

fn render_numbered_item(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len());
    let dot = trimmed.find(". ")?;
    if dot == 0 || dot > 3 {
        return None;
    }
    let number = &trimmed[..dot];
    if !number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let text = trimmed[dot + 2..].trim_start();
    let spaces = " ".repeat(indent.min(6));
    Some(format!("{spaces}{DIM}{number}.{RESET} {text}"))
}

fn render_block_quote(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let text = trimmed.strip_prefix("> ")?;
    let indent = line.len().saturating_sub(trimmed.len()).min(6);
    Some(format!("{}{DIM}│ {text}{RESET}", " ".repeat(indent)))
}

fn render_markdown_table_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }

    let cells: Vec<String> = trimmed
        .trim_matches('|')
        .split('|')
        .map(|cell| clean_markdown_inline(cell.trim()))
        .filter(|cell| !cell.is_empty())
        .collect();

    if cells.is_empty() {
        return Some(String::new());
    }

    let is_separator = cells.iter().all(|cell| {
        cell.chars()
            .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
    });
    if is_separator {
        return Some(String::new());
    }

    Some(format!("  {}", cells.join("  ")))
}

fn clean_markdown_inline(line: &str) -> String {
    let mut out = line.replace("**", "");
    out = out.replace('`', "");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::theme::{DIM, RESET};

    #[test]
    fn markdown_table_separator_is_hidden() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("|---|:---:|", &mut in_code),
            String::new()
        );
    }

    #[test]
    fn markdown_table_is_softened() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("| `gex` | 空文件夹 |", &mut in_code),
            "  gex  空文件夹"
        );
    }

    #[test]
    fn inline_markdown_is_cleaned() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("**文件：** `a.md`", &mut in_code),
            "文件： a.md"
        );
    }

    #[test]
    fn markdown_lists_are_softened() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("- first item", &mut in_code),
            format!("{DIM}•{RESET} first item")
        );
        assert_eq!(
            render_assistant_line("  1. next item", &mut in_code),
            format!("  {DIM}1.{RESET} next item")
        );
    }

    #[test]
    fn markdown_quotes_and_code_blocks_are_softened() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("> note", &mut in_code),
            format!("{DIM}│ note{RESET}")
        );
        assert_eq!(
            render_assistant_line("```rust", &mut in_code),
            format!("{DIM}╭─ rust{RESET}")
        );
        assert_eq!(
            render_assistant_line("let x = 1;", &mut in_code),
            format!("{DIM}│{RESET} let x = 1;")
        );
        assert_eq!(
            render_assistant_line("```", &mut in_code),
            format!("{DIM}╰─{RESET}")
        );
    }
}
