//! Footer layout manager for the scrollback-first CLI.
//!
//! The footer is a fixed number of lines at the bottom of the terminal that
//! shows the prompt, transient status, or permission/question overlays. It
//! never enters the alternate screen; instead it uses cursor movement to
//! overwrite the same footer region and leaves scrollback intact.

use crate::shell::prompt::PromptEditor;
use crate::shell::theme::{CYAN, DIM, GREEN, MAGENTA, RESET, YELLOW};
use std::io::{self, Write};

/// Optional attachment pill line shown above the prompt.
#[derive(Debug, Clone, Default)]
pub struct AttachmentLine {
    pub text: String,
}

/// State describing what the footer should currently display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FooterMode {
    /// Editing prompt; show the prompt editor.
    Prompt,
    /// Generation is in progress.
    Thinking,
    /// Tool is running; carries a short description.
    ToolRunning(String),
    /// Permission overlay; carries the prompt text.
    Permission(String),
    /// Question overlay from the model.
    Question(String),
    /// Interrupt confirmation.
    Interrupt,
}

pub struct FooterRenderer {
    /// Number of terminal rows reserved for the footer.
    height: usize,
    /// Last rendered footer height, used to erase stale lines.
    last_rendered_height: usize,
    /// Whether the cursor is currently inside the footer region.
    cursor_in_footer: bool,
}

impl FooterRenderer {
    pub fn new(height: usize) -> Self {
        Self {
            height: height.max(1),
            last_rendered_height: 0,
            cursor_in_footer: false,
        }
    }

    pub fn set_height(&mut self, height: usize) {
        self.height = height.max(1);
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Move cursor from the main scrollback area into the footer start.
    ///
    /// Call this before rendering the footer. It preserves scrollback by only
    /// moving the cursor, not clearing the screen.
    pub fn enter(&mut self) -> io::Result<()> {
        // Move cursor up to the footer start and clear everything below it.
        let move_up = self.last_rendered_height.saturating_sub(1);
        if move_up > 0 {
            print!("\x1b[{move_up}A");
        }
        print!("\x1b[0G\x1b[J");
        self.cursor_in_footer = true;
        io::stdout().flush()
    }

    /// Render the footer for the current mode.
    pub fn render(
        &mut self,
        mode: &FooterMode,
        prompt: &PromptEditor,
        width: usize,
    ) -> io::Result<()> {
        self.render_with_attachments(mode, prompt, width, &AttachmentLine::default())
    }

    /// Render the footer with an optional attachment pill line above the prompt.
    pub fn render_with_attachments(
        &mut self,
        mode: &FooterMode,
        prompt: &PromptEditor,
        width: usize,
        attachments: &AttachmentLine,
    ) -> io::Result<()> {
        self.enter()?;

        let mut lines: Vec<String> = match mode {
            FooterMode::Prompt => render_prompt_footer(prompt, width),
            FooterMode::Thinking => vec![format!("{YELLOW}· Thinking…{RESET}")],
            FooterMode::ToolRunning(desc) => {
                vec![format!("{YELLOW}· running {}{RESET}", desc)]
            }
            FooterMode::Permission(prompt_text) => render_permission_footer(prompt_text, width),
            FooterMode::Question(text) => render_question_footer(text, width),
            FooterMode::Interrupt => vec![format!("{MAGENTA}· Press Ctrl+C again to quit{RESET}")],
        };

        if !attachments.text.is_empty() && matches!(mode, FooterMode::Prompt) {
            lines.insert(0, attachments.text.clone());
        }

        for line in &lines {
            println!("{line}");
        }

        // Pad to fixed footer height so subsequent renders start at same row.
        for _ in lines.len()..self.height {
            println!();
        }

        self.last_rendered_height = self.height;
        self.cursor_in_footer = true;
        io::stdout().flush()
    }

    /// Print content above the footer and move cursor back to footer start.
    pub fn print_above(&mut self, text: &str) -> io::Result<()> {
        self.clear_current()?;
        print!("\x1b[0G{text}");
        if !text.ends_with('\n') {
            println!();
        }
        self.cursor_in_footer = false;
        io::stdout().flush()
    }

    /// Erase any currently rendered footer lines without moving cursor back.
    pub fn clear_current(&mut self) -> io::Result<()> {
        if self.cursor_in_footer && self.last_rendered_height > 0 {
            let move_up = self.last_rendered_height.saturating_sub(1);
            if move_up > 0 {
                print!("\x1b[{move_up}A");
            }
            print!("\x1b[0G\x1b[J");
            io::stdout().flush()?;
        }
        Ok(())
    }

    /// Move cursor to the end of the current prompt line, for editing.
    pub fn position_cursor(
        &mut self,
        prompt: &PromptEditor,
        prompt_prefix_width: usize,
    ) -> io::Result<()> {
        let (row, col) = prompt.cursor();
        let visual_col = prompt_prefix_width + visual_width(&prompt.lines()[row][..col]);
        let up = self.height.saturating_sub(row + 1);
        if up > 0 {
            print!("\x1b[{up}A");
        }
        print!("\x1b[{visual_col}G");
        io::stdout().flush()
    }
}

fn render_prompt_footer(prompt: &PromptEditor, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let prefix = format!("{CYAN}●{RESET} ");
    let prefix_len = 2; // visual width of "● "

    for (idx, line) in prompt.lines().iter().enumerate() {
        let marker = if idx == 0 {
            prefix.clone()
        } else {
            "  ".to_string()
        };
        // Reserve at least 10 columns for usable input on very narrow terminals.
        let usable_width = width.saturating_sub(prefix_len).max(10);
        let wrapped = wrap_line(line, usable_width);
        for (widx, wrapped_line) in wrapped.into_iter().enumerate() {
            let pad = if idx == 0 && widx == 0 { "" } else { "  " };
            lines.push(format!("{pad}{marker}{wrapped_line}"));
        }
    }

    if lines.is_empty() {
        lines.push(prefix);
    }
    lines
}

fn render_permission_footer(prompt_text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![format!("{YELLOW}? Permission required{RESET}")];
    for line in prompt_text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            for wrapped in wrap_line(trimmed, width.saturating_sub(2)) {
                lines.push(format!("  {wrapped}"));
            }
        }
    }
    lines.push(format!(
        "{DIM}  [y] allow once · [n] deny · [a] allow session · [d] deny session{RESET}"
    ));
    lines
}

fn render_question_footer(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![format!(
        "{GREEN}?{RESET} {}",
        text.lines().next().unwrap_or("")
    )];
    for line in text.lines().skip(1) {
        for wrapped in wrap_line(line, width.saturating_sub(2)) {
            lines.push(format!("  {wrapped}"));
        }
    }
    lines.push(format!("{DIM}  Type your answer and press Enter{RESET}"));
    lines
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for ch in line.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > width && !current.is_empty() {
            out.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
    }

    if !current.is_empty() || out.is_empty() {
        out.push(current);
    }
    out
}

fn visual_width(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_footer_renders_lines() {
        let mut editor = PromptEditor::new();
        editor.insert("hello\nworld");
        let lines = render_prompt_footer(&editor, 40);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("hello"));
        assert!(lines[1].contains("world"));
    }

    #[test]
    fn wrap_respects_width() {
        let wrapped = wrap_line("abcdefghij", 4);
        assert_eq!(wrapped.len(), 3);
        assert_eq!(wrapped[0], "abcd");
        assert_eq!(wrapped[1], "efgh");
        assert_eq!(wrapped[2], "ij");
    }
}
