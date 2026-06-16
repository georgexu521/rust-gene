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
use crate::shell::theme::{CYAN, DIM, RESET};
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
}

/// Scrollback-first surface used by `--no-footer`.
pub struct PlainSurface;

impl PlainSurface {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlainSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl Surface for PlainSurface {
    fn push_line(&mut self, text: &str) -> io::Result<()> {
        for line in text.split('\n') {
            println!("{}", line);
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
                print_plain_prompt(editor, attachments, completion)?;
            }
            FooterMode::Thinking => {
                println!("{DIM}· Thinking…{RESET}");
            }
            FooterMode::Interrupt => {
                println!("{DIM}· Press Ctrl+C again to quit{RESET}");
            }
            _ => {}
        }
        Ok(())
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
}

fn print_plain_prompt(
    editor: &PromptEditor,
    attachments: &AttachmentManager,
    completion: Option<&CompletionState>,
) -> io::Result<()> {
    let prefix = format!("{CYAN}●{RESET} ");
    print!("{}", prefix);
    for (idx, line) in editor.lines().iter().enumerate() {
        if idx > 0 {
            print!("  ");
        }
        print!("{}", line);
    }
    if !attachments.is_empty() {
        print!("  {DIM}[{}]{RESET}", attachments.labels().join(", "));
    }
    if let Some(state) = completion {
        print!("  {DIM}completion:{RESET}");
        for (idx, candidate) in state.candidates.iter().take(6).enumerate() {
            if idx > 0 {
                print!("  ");
            }
            let marker = if idx == state.selected { ">" } else { " " };
            print!("{}{}", marker, candidate.display);
        }
    }
    println!();
    io::stdout().flush()
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
