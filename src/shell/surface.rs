//! Render surface abstraction for the CLI.
//!
//! The shell can render in two modes:
//! - `PlainSurface`: writes directly to stdout in scrollback mode (used by
//!   `--no-footer`).
//! - `ScreenSurface`: manages an alternate-screen buffer and redraws the whole
//!   terminal (used by the default CLI mode).

use crate::shell::attachment::AttachmentManager;
use crate::shell::completion_state::CompletionState;
use crate::shell::footer::{AttachmentLine, FooterMode};
use crate::shell::prompt::PromptEditor;
use crate::shell::theme::{CYAN, DIM, RESET, YELLOW};
use std::io::{self, Write};

/// Common interface used by the shell loop and turn renderer.
pub trait Surface {
    /// Append a line of conversation text.
    fn push_line(&mut self, text: &str) -> io::Result<()>;

    /// Render the footer/prompt area for the current mode/editor/attachments.
    fn render_footer(
        &mut self,
        mode: &FooterMode,
        editor: &PromptEditor,
        attachments: &AttachmentManager,
        completion: Option<&CompletionState>,
    ) -> io::Result<()>;

    /// Current usable terminal width.
    fn terminal_width(&self) -> usize;

    /// Finalize and flush any pending output.
    fn flush(&mut self) -> io::Result<()>;

    /// Clear the conversation area.
    fn clear(&mut self) -> io::Result<()>;

    /// Scroll the visible conversation area by the given signed line count.
    /// Positive values scroll down, negative values scroll up.
    fn scroll_by(&mut self, _delta: isize) -> io::Result<()> {
        Ok(())
    }

    /// Cancel any user scroll offset and jump to the newest content.
    fn scroll_to_bottom(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Scrollback-first surface used by `--no-footer` and by the default normal-screen
/// CLI mode. It prints directly to stdout and redraws the active prompt line in
/// place using ANSI erase sequences.
pub struct PlainSurface {
    /// Whether the cursor is currently on a prompt line that should be redrawn.
    in_prompt: bool,
    /// How many screen lines the last prompt occupied.
    last_prompt_lines: usize,
}

impl PlainSurface {
    pub fn new() -> Self {
        Self {
            in_prompt: false,
            last_prompt_lines: 0,
        }
    }

    fn end_prompt_if_needed(&mut self) {
        if self.in_prompt {
            print!("\r\n");
            self.in_prompt = false;
            self.last_prompt_lines = 0;
        }
    }

    fn clear_prompt_lines(&self) -> io::Result<()> {
        use crossterm::cursor::{MoveToColumn, MoveToPreviousLine};
        use crossterm::terminal::{Clear, ClearType};
        for _ in 0..self.last_prompt_lines.saturating_sub(1) {
            crossterm::execute!(io::stdout(), MoveToPreviousLine(1))?;
        }
        crossterm::execute!(
            io::stdout(),
            MoveToColumn(0),
            Clear(ClearType::FromCursorDown)
        )?;
        Ok(())
    }
}

impl Default for PlainSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl Surface for PlainSurface {
    fn push_line(&mut self, text: &str) -> io::Result<()> {
        if self.in_prompt {
            // Replace the active prompt line(s) with the new line instead of
            // leaving the prompt visible and appending a duplicate below it.
            self.clear_prompt_lines()?;
            self.in_prompt = false;
            self.last_prompt_lines = 0;
            for (idx, line) in text.split('\n').enumerate() {
                if idx > 0 {
                    print!("\r\n");
                }
                print!("{}", line);
            }
            print!("\r\n");
        } else {
            for line in text.split('\n') {
                print!("{}\r\n", line);
            }
        }
        io::stdout().flush()
    }

    fn render_footer(
        &mut self,
        mode: &FooterMode,
        editor: &PromptEditor,
        attachments: &AttachmentManager,
        completion: Option<&CompletionState>,
    ) -> io::Result<()> {
        match mode {
            FooterMode::Prompt => {
                let prompt = build_prompt_line(editor, attachments, completion);
                if self.in_prompt {
                    self.clear_prompt_lines()?;
                }
                print!("{}", prompt);
                self.in_prompt = true;
                self.last_prompt_lines = prompt.lines().count().max(1);
            }
            FooterMode::Thinking => {
                self.end_prompt_if_needed();
                print!("{DIM}· Thinking…{RESET}\r\n");
            }
            FooterMode::Interrupt => {
                self.end_prompt_if_needed();
                print!("{DIM}· Press Ctrl+C again to quit{RESET}\r\n");
            }
            FooterMode::ToolRunning(desc) => {
                self.end_prompt_if_needed();
                print!("{DIM}· {}{RESET}\r\n", desc);
            }
            FooterMode::Permission(text) => {
                self.end_prompt_if_needed();
                print!("{YELLOW}?{RESET} Permission required\r\n");
                for line in text.lines() {
                    print!("{}\r\n", line);
                }
            }
            FooterMode::Question(text) => {
                self.end_prompt_if_needed();
                for line in text.lines() {
                    print!("{}\r\n", line);
                }
            }
        }
        io::stdout().flush()
    }

    fn terminal_width(&self) -> usize {
        crate::shell::text::terminal_width()
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }

    fn clear(&mut self) -> io::Result<()> {
        print!("\x1b[2J\x1b[H");
        io::stdout().flush()
    }

    fn scroll_by(&mut self, _delta: isize) -> io::Result<()> {
        // In normal-screen mode the terminal handles scrolling.
        Ok(())
    }

    fn scroll_to_bottom(&mut self) -> io::Result<()> {
        // No-op: the terminal viewport follows the newest output naturally.
        Ok(())
    }
}

fn build_prompt_line(
    editor: &PromptEditor,
    attachments: &AttachmentManager,
    completion: Option<&CompletionState>,
) -> String {
    let mut out = String::new();
    let lines = editor.lines();
    if lines.is_empty() {
        out.push_str(&format!("{CYAN}●{RESET} "));
    } else {
        for (idx, line) in lines.iter().enumerate() {
            if idx == 0 {
                out.push_str(&format!("{CYAN}●{RESET} {}", line));
            } else {
                out.push_str(&format!("\n  {}", line));
            }
        }
    }
    if !attachments.is_empty() {
        out.push_str(&format!(
            "  {DIM}[{}]{RESET}",
            attachments.labels().join(", ")
        ));
    }
    if let Some(state) = completion {
        out.push_str("  {DIM}completion:{RESET}");
        for (idx, candidate) in state.candidates.iter().take(6).enumerate() {
            if idx > 0 {
                out.push_str("  ");
            }
            let marker = if idx == state.selected { ">" } else { " " };
            out.push_str(&format!("{}{}", marker, candidate.display));
        }
    }
    out
}

/// Helpers for building footer attachment lines.
pub(crate) fn build_attachment_line(attachments: &AttachmentManager) -> AttachmentLine {
    if attachments.is_empty() {
        AttachmentLine::default()
    } else {
        AttachmentLine {
            text: format!("{}[{}]{}", DIM, attachments.labels().join(", "), RESET),
        }
    }
}

pub(crate) fn build_completion_line(completion: Option<&CompletionState>) -> String {
    let Some(state) = completion else {
        return String::new();
    };
    let mut line = String::from("Completion: ");
    for (idx, candidate) in state.candidates.iter().take(6).enumerate() {
        if idx > 0 {
            line.push_str("  ");
        }
        let marker = if idx == state.selected { ">" } else { " " };
        line.push_str(&format!("{}{}", marker, candidate.display));
    }
    line
}

/// Test surface that collects output in memory.
#[cfg(test)]
pub struct TestSurface {
    pub lines: Vec<String>,
    pub footer_modes: Vec<FooterMode>,
}

#[cfg(test)]
impl TestSurface {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            footer_modes: Vec::new(),
        }
    }
}

#[cfg(test)]
impl Default for TestSurface {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl Surface for TestSurface {
    fn push_line(&mut self, text: &str) -> io::Result<()> {
        for line in text.split('\n') {
            self.lines.push(line.to_string());
        }
        Ok(())
    }

    fn render_footer(
        &mut self,
        mode: &FooterMode,
        _editor: &PromptEditor,
        _attachments: &AttachmentManager,
        _completion: Option<&CompletionState>,
    ) -> io::Result<()> {
        self.footer_modes.push(mode.clone());
        Ok(())
    }

    fn terminal_width(&self) -> usize {
        80
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.lines.clear();
        Ok(())
    }
}
