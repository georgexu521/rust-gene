//! Alternate-screen renderer for the CLI.
//!
//! Maintains an in-memory conversation buffer and redraws the whole terminal on
//! every change. This avoids the cursor-dance artifacts that occur when raw-mode
//! line editing, streaming output, and CJK characters mix in the main
//! scrollback.

use crate::shell::attachment::AttachmentManager;
use crate::shell::completion_state::CompletionState;
use crate::shell::footer::{AttachmentLine, FooterMode, FooterRenderer};
use crate::shell::prompt::PromptEditor;
use crate::shell::surface::{build_attachment_line, build_completion_line, Surface};
use crate::shell::theme::RESET;
use std::io::{self, Write};

const FOOTER_ROWS: usize = 6;

pub struct ScreenSurface {
    width: usize,
    height: usize,
    /// Logical conversation lines (with ANSI markup).
    lines: Vec<String>,
    /// First visible conversation line index.
    scroll_offset: usize,
    footer: FooterRenderer,
    pending_footer_mode: FooterMode,
    pending_editor: PromptEditor,
    pending_attachment_line: AttachmentLine,
    pending_completion_line: String,
    in_alternate_screen: bool,
}

impl ScreenSurface {
    pub fn new(width: usize, height: usize) -> io::Result<Self> {
        let mut surface = Self {
            width,
            height,
            lines: Vec::new(),
            scroll_offset: 0,
            footer: FooterRenderer::new(FOOTER_ROWS),
            pending_footer_mode: FooterMode::Prompt,
            pending_editor: PromptEditor::new(),
            pending_attachment_line: AttachmentLine::default(),
            pending_completion_line: String::new(),
            in_alternate_screen: false,
        };
        surface.enter_alternate_screen()?;
        Ok(surface)
    }

    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        if !self.in_alternate_screen {
            print!("\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H");
            io::stdout().flush()?;
            self.in_alternate_screen = true;
        }
        Ok(())
    }

    fn leave_alternate_screen(&mut self) -> io::Result<()> {
        if self.in_alternate_screen {
            print!("\x1b[?25h\x1b[?1049l");
            io::stdout().flush()?;
            self.in_alternate_screen = false;
        }
        Ok(())
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.clamp_scroll();
    }

    fn visible_rows(&self) -> usize {
        self.height.saturating_sub(FOOTER_ROWS).max(1)
    }

    fn clamp_scroll(&mut self) {
        let visible = self.visible_rows();
        let max_offset = self.lines.len().saturating_sub(visible);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    fn redraw(&mut self) -> io::Result<()> {
        if !self.in_alternate_screen {
            return Ok(());
        }

        let visible = self.visible_rows();
        let rows: Vec<String> = self
            .lines
            .iter()
            .flat_map(|line| wrap_visual(line, self.width))
            .collect();
        let max_offset = rows.len().saturating_sub(visible);
        self.scroll_offset = self.scroll_offset.min(max_offset);

        let mut out = String::with_capacity(self.width * self.height * 2);
        out.push_str("\x1b[H");

        for row in 0..visible {
            let idx = self.scroll_offset + row;
            if let Some(line) = rows.get(idx) {
                out.push_str(&truncate_visual(line, self.width));
            }
            out.push('\n');
        }

        let mut attachment_line = self.pending_attachment_line.clone();
        if !self.pending_completion_line.is_empty() {
            if attachment_line.text.is_empty() {
                attachment_line.text = self.pending_completion_line.clone();
            } else {
                attachment_line.text.push('\n');
                attachment_line.text.push_str(&self.pending_completion_line);
            }
        }

        let footer_lines = self.footer.render_lines(
            &self.pending_footer_mode,
            &self.pending_editor,
            self.width,
            &attachment_line,
        );

        for (i, line) in footer_lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&truncate_visual(line, self.width));
        }

        // Pad footer to fixed height so the bottom of the screen stays stable.
        for _ in footer_lines.len()..FOOTER_ROWS {
            out.push('\n');
        }

        print!("{}", out);
        io::stdout().flush()
    }

    /// Print the conversation to the main scrollback after leaving alternate
    /// screen. This preserves history for the user.
    pub fn dump_to_scrollback(&mut self) {
        let _ = self.leave_alternate_screen();
        if self.lines.is_empty() {
            return;
        }
        for line in &self.lines {
            println!("{}", strip_ansi(line));
        }
        let _ = io::stdout().flush();
    }
}

impl Surface for ScreenSurface {
    fn push_line(&mut self, text: &str) -> io::Result<()> {
        for line in text.split('\n') {
            self.lines.push(line.to_string());
        }
        self.redraw()
    }

    fn render_footer(
        &mut self,
        mode: &FooterMode,
        editor: &PromptEditor,
        attachments: &AttachmentManager,
        completion: Option<&CompletionState>,
    ) -> io::Result<()> {
        self.pending_footer_mode = mode.clone();
        self.pending_editor = editor.clone();
        self.pending_attachment_line = build_attachment_line(attachments);
        self.pending_completion_line = build_completion_line(completion);
        self.redraw()
    }

    fn terminal_width(&self) -> usize {
        self.width
    }

    fn flush(&mut self) -> io::Result<()> {
        self.redraw()
    }

    fn clear(&mut self) -> io::Result<()> {
        self.lines.clear();
        self.scroll_offset = 0;
        self.redraw()
    }
}

impl Drop for ScreenSurface {
    fn drop(&mut self) {
        let _ = self.leave_alternate_screen();
    }
}

/// Wrap a line into one or more visual rows, preserving ANSI sequences and
/// respecting CJK character widths. Empty input produces one empty row so the
/// caller can always render a line.
fn wrap_visual(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }
    let mut rows: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    let mut in_ansi = false;

    for ch in line.chars() {
        if ch == '\x1b' {
            in_ansi = true;
            current.push(ch);
            continue;
        }
        if in_ansi {
            current.push(ch);
            if ch.is_ascii_alphabetic() {
                in_ansi = false;
            }
            continue;
        }
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + w > width && !current.is_empty() {
            rows.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += w;
    }

    if !current.is_empty() || rows.is_empty() {
        rows.push(current);
    }
    rows
}

/// Truncate a line to a target visual width, preserving ANSI sequences and
/// padding with spaces so stale characters are overwritten.
fn truncate_visual(line: &str, width: usize) -> String {
    let mut out = String::with_capacity(width * 2);
    let mut current_width = 0usize;
    let mut in_ansi = false;

    for ch in line.chars() {
        if ch == '\x1b' {
            in_ansi = true;
            out.push(ch);
            continue;
        }
        if in_ansi {
            out.push(ch);
            if ch.is_ascii_alphabetic() {
                in_ansi = false;
            }
            continue;
        }
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + w > width {
            break;
        }
        out.push(ch);
        current_width += w;
    }

    // Reset any active ANSI styles and pad to width so stale characters are
    // cleared when a line becomes shorter.
    out.push_str(RESET);
    while current_width < width {
        out.push(' ');
        current_width += 1;
    }
    out
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_ansi = false;
    for ch in line.chars() {
        if ch == '\x1b' {
            in_ansi = true;
            continue;
        }
        if in_ansi {
            if ch.is_ascii_alphabetic() {
                in_ansi = false;
            }
            continue;
        }
        out.push(ch);
    }
    out
}
