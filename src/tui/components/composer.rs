//! Composer state model.
//!
//! The composer owns the plain-text input plus structured prompt parts
//! (file attachments, pasted text, images). It keeps `InputState` free of
//! attachment markers and provides one place to build the submitted prompt
//! payload.

use crate::tui::components::{
    attachment_token::{AttachmentSource, AttachmentToken},
    input::InputState,
};

#[derive(Debug, Clone)]
pub struct ComposerState {
    pub text: InputState,
    pub parts: Vec<ComposerPart>,
}

#[derive(Debug, Clone)]
pub enum ComposerPart {
    File(AttachmentToken),
    PastedText {
        id: String,
        label: String,
        placeholder: String,
        content: String,
    },
    Image {
        id: String,
        label: String,
        content: String,
    },
}

impl ComposerState {
    pub fn new() -> Self {
        Self {
            text: InputState::new(),
            parts: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.parts.is_empty()
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.parts.clear();
    }

    pub fn file_parts(&self) -> impl Iterator<Item = &AttachmentToken> {
        self.parts.iter().filter_map(|part| match part {
            ComposerPart::File(token) => Some(token),
            _ => None,
        })
    }

    pub fn pasted_text_parts(&self) -> impl Iterator<Item = (&str, &str, &str, &str)> {
        self.parts.iter().filter_map(|part| match part {
            ComposerPart::PastedText {
                id,
                label,
                placeholder,
                content,
            } => Some((
                id.as_str(),
                label.as_str(),
                placeholder.as_str(),
                content.as_str(),
            )),
            _ => None,
        })
    }

    pub fn image_parts(&self) -> impl Iterator<Item = (&str, &str, &str)> {
        self.parts.iter().filter_map(|part| match part {
            ComposerPart::Image { id, label, content } => {
                Some((id.as_str(), label.as_str(), content.as_str()))
            }
            _ => None,
        })
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.file_parts().any(|token| token.path == path)
    }

    pub fn add_file(
        &mut self,
        path: impl AsRef<std::path::Path>,
        source: AttachmentSource,
    ) -> Option<AttachmentToken> {
        let token = AttachmentToken::from_path(path, source);
        if self.has_file(&token.path) {
            return None;
        }
        self.parts.push(ComposerPart::File(token.clone()));
        Some(token)
    }

    pub fn add_pasted_text(
        &mut self,
        label: impl Into<String>,
        placeholder: impl Into<String>,
        content: impl Into<String>,
    ) -> String {
        let id = format!("paste_{}", self.parts.len() + 1);
        self.parts.push(ComposerPart::PastedText {
            id: id.clone(),
            label: label.into(),
            placeholder: placeholder.into(),
            content: content.into(),
        });
        id
    }

    pub fn add_image(&mut self, label: impl Into<String>, content: impl Into<String>) -> String {
        let id = format!("image_{}", self.parts.len() + 1);
        self.parts.push(ComposerPart::Image {
            id: id.clone(),
            label: label.into(),
            content: content.into(),
        });
        id
    }

    pub fn remove_part_by_index(&mut self, index: usize) -> Option<ComposerPart> {
        if index < self.parts.len() {
            Some(self.parts.remove(index))
        } else {
            None
        }
    }

    pub fn remove_last_part(&mut self) -> Option<ComposerPart> {
        self.parts.pop()
    }

    pub fn remove_file_by_path(&mut self, path: &str) -> Option<AttachmentToken> {
        let index = self.parts.iter().position(|part| match part {
            ComposerPart::File(token) => token.path == path,
            _ => false,
        })?;
        match self.parts.remove(index) {
            ComposerPart::File(token) => Some(token),
            other => {
                self.parts.insert(index, other);
                None
            }
        }
    }

    pub fn attachment_count(&self) -> usize {
        self.file_parts().count()
    }

    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    pub fn attachment_paths(&self) -> Vec<String> {
        self.file_parts().map(|token| token.label.clone()).collect()
    }

    pub fn all_part_labels(&self) -> Vec<String> {
        self.parts
            .iter()
            .map(|part| match part {
                ComposerPart::File(token) => token.label.clone(),
                ComposerPart::PastedText { label, .. } | ComposerPart::Image { label, .. } => {
                    label.clone()
                }
            })
            .collect()
    }

    pub fn attachment_tokens(&self) -> Vec<AttachmentToken> {
        self.file_parts().cloned().collect()
    }

    /// Build the prompt string submitted to the engine, including all parts
    /// exactly once.
    pub fn build_submission(&self) -> String {
        let text = self.text.value().trim();
        let files: Vec<_> = self.file_parts().collect();
        let pasted: Vec<_> = self.pasted_text_parts().collect();
        let images: Vec<_> = self.image_parts().collect();

        let mut text_without_placeholders = text.to_string();
        for (_id, _label, placeholder, _content) in pasted.clone() {
            text_without_placeholders = text_without_placeholders.replace(placeholder, "");
        }
        let text_without_placeholders = text_without_placeholders.trim();

        let mut sections = Vec::new();

        if !files.is_empty() {
            let mut lines = vec!["Attached context:".to_string()];
            for token in files {
                lines.push(format!("- {}", token.label));
            }
            sections.push(lines.join("\n"));
        }

        if !pasted.is_empty() {
            let mut lines = vec!["Pasted context:".to_string()];
            for (_id, label, _placeholder, content) in pasted {
                lines.push(format!("- {}:\n{}\n", label, content));
            }
            sections.push(lines.join("\n"));
        }

        if !images.is_empty() {
            let mut lines = vec!["Images:".to_string()];
            for (_id, label, _content) in images {
                lines.push(format!("- {}", label));
            }
            sections.push(lines.join("\n"));
        }

        if !text_without_placeholders.is_empty() {
            sections.push(format!("User request:\n{}", text_without_placeholders));
        } else if sections.is_empty() {
            return String::new();
        }

        sections.join("\n\n")
    }
}

impl Default for ComposerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_builds_submission_with_file_and_text() {
        let mut composer = ComposerState::new();
        composer.text.insert_str("summarize this");
        composer.add_file("Cargo.toml", AttachmentSource::File);
        let payload = composer.build_submission();
        assert!(payload.contains("Attached context:"));
        assert!(payload.contains("Cargo.toml"));
        assert!(payload.contains("User request:"));
        assert!(payload.contains("summarize this"));
    }

    #[test]
    fn composer_deduplicates_files_by_path() {
        let mut composer = ComposerState::new();
        assert!(composer
            .add_file("Cargo.toml", AttachmentSource::File)
            .is_some());
        assert!(composer
            .add_file("Cargo.toml", AttachmentSource::File)
            .is_none());
        assert_eq!(composer.attachment_count(), 1);
    }

    #[test]
    fn composer_empty_when_no_text_or_parts() {
        let composer = ComposerState::new();
        assert!(composer.is_empty());
        assert!(composer.build_submission().is_empty());
    }
}
