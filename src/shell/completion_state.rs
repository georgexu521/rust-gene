//! Mention-completion state for the CLI prompt.
//!
//! Keeps track of the candidate list, the column where the `@` mention starts,
//! and the currently selected index while the user types or navigates with
//! arrow keys.

use crate::shell::completion::{find_candidates, MentionCandidate};
use crate::shell::prompt::PromptEditor;

/// State for an active `@` mention completion popup.
#[derive(Debug, Clone)]
pub struct CompletionState {
    /// Column in the editor text where the mention prefix starts.
    pub start_col: usize,
    /// Candidate entries matching the current prefix.
    pub candidates: Vec<MentionCandidate>,
    /// Index into `candidates` of the highlighted entry.
    pub selected: usize,
}

impl CompletionState {
    /// Create a new completion state highlighting the first candidate.
    pub fn new(start_col: usize, candidates: Vec<MentionCandidate>) -> Self {
        Self {
            start_col,
            candidates,
            selected: 0,
        }
    }

    /// Move the selection up, clamping at zero.
    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Move the selection down, clamping at the last candidate.
    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1).min(self.candidates.len().saturating_sub(1));
    }

    /// Return the currently highlighted candidate, if any.
    pub fn selected_candidate(&self) -> Option<&MentionCandidate> {
        self.candidates.get(self.selected)
    }

    /// Recompute candidates after the user edits the prompt while a completion
    /// popup is active. Returns `None` if the cursor moved before the mention
    /// start or no candidates remain.
    pub fn update_after_edit(
        editor: &PromptEditor,
        state: Option<Self>,
        cursor_col: usize,
    ) -> Option<Self> {
        let state = state?;
        if cursor_col < state.start_col {
            return None;
        }
        let (new_start, candidates) = find_candidates(&editor.text(), cursor_col)?;
        if candidates.is_empty() {
            return None;
        }
        Some(Self::new(new_start, candidates))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_state_selects_first_by_default() {
        let state = CompletionState::new(
            0,
            vec![
                MentionCandidate {
                    display: "a".to_string(),
                    replacement: "a".to_string(),
                    is_dir: false,
                },
                MentionCandidate {
                    display: "b".to_string(),
                    replacement: "b".to_string(),
                    is_dir: false,
                },
            ],
        );
        assert_eq!(state.selected, 0);
        assert_eq!(state.selected_candidate().unwrap().display, "a");
    }

    #[test]
    fn completion_state_navigation_clamps() {
        let mut state = CompletionState::new(
            0,
            vec![
                MentionCandidate {
                    display: "a".to_string(),
                    replacement: "a".to_string(),
                    is_dir: false,
                },
                MentionCandidate {
                    display: "b".to_string(),
                    replacement: "b".to_string(),
                    is_dir: false,
                },
            ],
        );
        state.select_previous();
        assert_eq!(state.selected, 0);
        state.select_next();
        assert_eq!(state.selected, 1);
        state.select_next();
        assert_eq!(state.selected, 1);
    }
}
