//! Shell attachment manager.
//!
//! Wraps the shared `ComposerState` so the CLI can add, remove, and list file
//! attachments while keeping the prompt editor free of attachment markers.

use crate::components::attachment_token::{
    add_attachment_token, AttachmentSource, AttachmentToken,
};
use crate::components::composer::{ComposerPart, ComposerState};
use crate::shell::theme::{BRIGHT_BLUE, DIM, RESET};
use std::path::Path;

pub struct AttachmentManager {
    composer: ComposerState,
}

impl AttachmentManager {
    pub fn new() -> Self {
        Self {
            composer: ComposerState::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.composer.is_empty()
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.composer.has_file(path)
    }

    pub fn add_file(
        &mut self,
        path: impl AsRef<Path>,
        source: AttachmentSource,
    ) -> Option<AttachmentToken> {
        // `ComposerState::add_file` returns `None` if already attached.
        self.composer.add_file(path, source)
    }

    pub fn add_pasted_text(
        &mut self,
        label: impl Into<String>,
        placeholder: impl Into<String>,
        content: impl Into<String>,
    ) -> String {
        self.composer.add_pasted_text(label, placeholder, content)
    }

    pub fn remove_file_by_path(&mut self, path: &str) -> Option<AttachmentToken> {
        self.composer.remove_file_by_path(path)
    }

    pub fn remove_last(&mut self) -> Option<ComposerPart> {
        self.composer.remove_last_part()
    }

    pub fn remove_by_index(&mut self, index: usize) -> Option<ComposerPart> {
        self.composer.remove_part_by_index(index)
    }

    pub fn clear(&mut self) {
        self.composer.clear();
    }

    pub fn tokens(&self) -> Vec<AttachmentToken> {
        self.composer.attachment_tokens()
    }

    pub fn labels(&self) -> Vec<String> {
        self.composer.all_part_labels()
    }

    pub fn count(&self) -> usize {
        self.composer.part_count()
    }

    pub fn file_count(&self) -> usize {
        self.composer.attachment_count()
    }

    /// Build the prompt string that should be submitted to the engine.
    pub fn build_submission(&self, text: &str) -> String {
        let mut composer = self.composer.clone();
        composer.text.insert_str(text);
        composer.build_submission()
    }

    /// Render a compact single-line summary of attachments for the footer.
    pub fn render_pills(&self, max_width: usize) -> String {
        let labels: Vec<String> = self
            .composer
            .all_part_labels()
            .into_iter()
            .map(|label| format!("{BRIGHT_BLUE}[file {label}]{RESET}"))
            .collect();
        if labels.is_empty() {
            return String::new();
        }

        let mut line = String::new();
        let mut width = 0usize;
        for (idx, label) in labels.iter().enumerate() {
            let sep = if idx == 0 { "" } else { " " };
            let added_width = sep.width() + strip_ansi(label).chars().count();
            if !line.is_empty() && width + added_width > max_width {
                break;
            }
            line.push_str(sep);
            line.push_str(label);
            width += added_width;
        }
        let remaining = self.count().saturating_sub(labels.len());
        if remaining > 0 {
            line.push_str(&format!(" {DIM}+{}{RESET}", remaining));
        }
        line
    }
}

impl Default for AttachmentManager {
    fn default() -> Self {
        Self::new()
    }
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_escape = false;
    for ch in text.chars() {
        if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else if ch == '\x1b' {
            in_escape = true;
        } else {
            out.push(ch);
        }
    }
    out
}

trait Width {
    fn width(&self) -> usize;
}

impl Width for &str {
    fn width(&self) -> usize {
        self.len()
    }
}

/// Add a file path to a token list if it is not already present.
pub fn add_attachment_if_missing(
    tokens: &mut Vec<AttachmentToken>,
    path: impl AsRef<Path>,
    source: AttachmentSource,
) -> Option<AttachmentToken> {
    add_attachment_token(tokens, path, source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_builds_submission_with_text_and_files() {
        let mut mgr = AttachmentManager::new();
        mgr.add_file("Cargo.toml", AttachmentSource::File);
        let submission = mgr.build_submission("summarize");
        assert!(submission.contains("Attached context:"));
        assert!(submission.contains("Cargo.toml"));
        assert!(submission.contains("User request:"));
        assert!(submission.contains("summarize"));
    }

    #[test]
    fn manager_deduplicates_files() {
        let mut mgr = AttachmentManager::new();
        assert!(mgr.add_file("Cargo.toml", AttachmentSource::File).is_some());
        assert!(mgr.add_file("Cargo.toml", AttachmentSource::File).is_none());
        assert_eq!(mgr.file_count(), 1);
    }

    #[test]
    fn render_pills_is_empty_when_no_attachments() {
        let mgr = AttachmentManager::new();
        assert!(mgr.render_pills(80).is_empty());
    }

    #[test]
    fn render_pills_shows_file_label() {
        let mut mgr = AttachmentManager::new();
        mgr.add_file("Cargo.toml", AttachmentSource::File);
        let pills = mgr.render_pills(80);
        assert!(pills.contains("[file"));
        assert!(pills.contains("Cargo.toml"));
    }
}
