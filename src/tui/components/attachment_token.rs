//! Composer attachment token model.
//!
//! Attachment pills live outside the plain-text `InputState`. They are rendered
//! inline around the text input and submitted together with the message.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentToken {
    pub id: String,
    pub path: String,
    pub label: String,
    pub source: AttachmentSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentSource {
    File,
    Pasted,
    Autocomplete,
}

impl AttachmentToken {
    pub fn from_path(path: impl AsRef<Path>, source: AttachmentSource) -> Self {
        let path_ref = path.as_ref();
        let absolute = if path_ref.is_absolute() {
            path_ref.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path_ref)
        };
        let canonical = absolute.canonicalize().unwrap_or(absolute);
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let label = canonical
            .strip_prefix(&cwd)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| canonical.to_string_lossy().to_string());
        let id = format!(
            "att_{}_{}",
            std::process::id(),
            canonical.to_string_lossy().replace(['/', '\\'], "_")
        );
        Self {
            id,
            path: canonical.to_string_lossy().to_string(),
            label,
            source,
        }
    }

    pub fn pill_label(&self) -> String {
        format!("[file {}]", compact_label(&self.label, 36))
    }
}

fn compact_label(label: &str, max: usize) -> String {
    if label.chars().count() <= max {
        return label.to_string();
    }
    let mut out = String::new();
    for ch in label.chars().take(max) {
        out.push(ch);
    }
    out.push('…');
    out
}

/// Merge a path into the token list, avoiding duplicates by absolute path.
pub fn add_attachment_token(
    tokens: &mut Vec<AttachmentToken>,
    path: impl AsRef<Path>,
    source: AttachmentSource,
) -> Option<AttachmentToken> {
    let token = AttachmentToken::from_path(path, source);
    if tokens.iter().any(|t| t.path == token.path) {
        return None;
    }
    tokens.push(token.clone());
    Some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_token_builds_label() {
        let token = AttachmentToken::from_path("Cargo.toml", AttachmentSource::File);
        assert!(token.path.ends_with("Cargo.toml"));
        assert!(!token.id.is_empty());
        assert!(token.pill_label().starts_with("[file"));
    }

    #[test]
    fn duplicate_tokens_are_skipped() {
        let mut tokens = Vec::new();
        assert!(add_attachment_token(&mut tokens, "Cargo.toml", AttachmentSource::File).is_some());
        assert!(add_attachment_token(&mut tokens, "Cargo.toml", AttachmentSource::File).is_none());
        assert_eq!(tokens.len(), 1);
    }
}
