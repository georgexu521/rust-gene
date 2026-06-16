//! Question UI for the CLI.
//!
//! When a tool asks the user a question through `AskChannel`, the CLI shows the
//! question in the footer and reads either a numeric option choice or free-form
//! text from the keyboard.

use crate::shell::footer::{FooterMode, FooterRenderer};
use crate::shell::prompt::PromptEditor;
use crate::shell::theme::{DIM, GREEN, RESET};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

pub struct QuestionState {
    pub question: String,
    pub options: Vec<String>,
    pub selected: Option<usize>,
    pub freeform: String,
    pub freeform_mode: bool,
}

impl QuestionState {
    pub fn new(question: String, options: Vec<String>) -> Self {
        Self {
            question,
            options,
            selected: None,
            freeform: String::new(),
            freeform_mode: false,
        }
    }

    pub fn render_text(&self, width: usize) -> String {
        let mut lines = vec![format!("{GREEN}?{RESET} {}", self.question)];
        if !self.options.is_empty() {
            for (idx, option) in self.options.iter().enumerate() {
                let marker = if Some(idx) == self.selected { ">" } else { " " };
                lines.push(format!("  {DIM}{}.{} {}{RESET}", idx + 1, marker, option));
            }
        }
        if self.freeform_mode || self.options.is_empty() {
            lines.push(format!("{DIM}  Answer: {}{RESET}", self.freeform));
        } else {
            lines.push(format!(
                "{DIM}  [1-{}] select · [0] custom answer · Enter confirm · Esc cancel{RESET}",
                self.options.len()
            ));
        }

        // Soft-wrap long lines to width.
        let mut out = String::new();
        for line in lines {
            if line.len() > width && width > 10 {
                out.push_str(&line[..width]);
                out.push('\n');
                out.push_str(&format!("{DIM}  ...{RESET}"));
            } else {
                out.push_str(&line);
            }
            out.push('\n');
        }
        out.pop(); // remove trailing newline
        out
    }

    pub fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> Option<QuestionResult> {
        if key.kind == KeyEventKind::Release {
            return None;
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => Some(QuestionResult::Cancel),
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if self.freeform_mode {
                    Some(QuestionResult::Answer(self.freeform.clone()))
                } else if let Some(idx) = self.selected {
                    if idx < self.options.len() {
                        Some(QuestionResult::Answer(self.options[idx].clone()))
                    } else {
                        None
                    }
                } else if self.options.is_empty() {
                    Some(QuestionResult::Answer(self.freeform.clone()))
                } else {
                    None
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('0')) if !self.options.is_empty() => {
                self.freeform_mode = true;
                self.selected = None;
                None
            }
            (KeyModifiers::NONE, KeyCode::Char(ch)) if ch.is_ascii_digit() => {
                if self.freeform_mode {
                    self.freeform.push(ch);
                    None
                } else {
                    let digit = ch.to_digit(10).unwrap_or(0) as usize;
                    let idx = digit.saturating_sub(1);
                    if idx < self.options.len() {
                        self.selected = Some(idx);
                    }
                    None
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if self.freeform_mode {
                    self.freeform.pop();
                }
                None
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                if !self.freeform_mode && !self.options.is_empty() {
                    let len = self.options.len();
                    self.selected = Some(
                        self.selected
                            .map_or(0, |s| s.saturating_sub(1).min(len - 1)),
                    );
                }
                None
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                if !self.freeform_mode && !self.options.is_empty() {
                    let len = self.options.len();
                    self.selected = Some(self.selected.map_or(0, |s| (s + 1) % len));
                }
                None
            }
            _ => None,
        }
    }
}

pub enum QuestionResult {
    Answer(String),
    Cancel,
}

/// Poll for a pending question and render a question footer until answered.
pub async fn run_question_ui(
    footer: &mut FooterRenderer,
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
    channel: &std::sync::Arc<crate::tools::ask_tool::AskChannel>,
    width: usize,
) -> anyhow::Result<Option<String>> {
    let Some((question, options, tx)) = channel.take_pending().await else {
        return Ok(None);
    };

    let mut state = QuestionState::new(question, options);
    loop {
        footer.render(
            &FooterMode::Question(state.render_text(width)),
            &PromptEditor::new(),
            width,
        )?;

        let Some(event) = event_rx.recv().await else {
            return Ok(None);
        };
        if let Event::Key(key) = event {
            match state.handle_key(&key) {
                Some(QuestionResult::Answer(answer)) => {
                    let _ = tx.send(answer.clone());
                    return Ok(Some(answer));
                }
                Some(QuestionResult::Cancel) => {
                    let _ = tx.send("User cancelled".to_string());
                    return Ok(None);
                }
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn question_renders_options() {
        let state = QuestionState::new(
            "Choose one".to_string(),
            vec!["A".to_string(), "B".to_string()],
        );
        let text = state.render_text(80);
        assert!(text.contains("Choose one"));
        assert!(text.contains("1.  A"));
        assert!(text.contains("2.  B"));
    }

    #[test]
    fn digit_selects_option() {
        let mut state = QuestionState::new(
            "Choose one".to_string(),
            vec!["A".to_string(), "B".to_string()],
        );
        assert!(state.handle_key(&key(KeyCode::Char('2'))).is_none());
        assert_eq!(state.selected, Some(1));
        assert!(matches!(
            state.handle_key(&key(KeyCode::Enter)),
            Some(QuestionResult::Answer(ans)) if ans == "B"
        ));
    }

    #[test]
    fn zero_switches_to_freeform() {
        let mut state = QuestionState::new("Choose one".to_string(), vec!["A".to_string()]);
        assert!(state.handle_key(&key(KeyCode::Char('0'))).is_none());
        assert!(state.freeform_mode);
    }
}
