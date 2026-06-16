//! @mention file completion for the CLI prompt.
//!
//! Completion is triggered by typing `@` while editing the prompt. The
//! candidates are files and directories under the current working directory,
//! filtered by the prefix after `@`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionCandidate {
    pub display: String,
    pub replacement: String,
    pub is_dir: bool,
}

/// Find @mention candidates for the current prompt text and cursor position.
///
/// Returns `Some((start_index, candidates))` when the cursor is in an active
/// `@` mention; otherwise returns `None`.
pub fn find_candidates(text: &str, cursor_col: usize) -> Option<(usize, Vec<MentionCandidate>)> {
    let before_cursor = text.chars().take(cursor_col).collect::<String>();
    let at_pos = before_cursor.rfind('@')?;
    // Only complete when the @ is at the start of the word.
    if at_pos > 0 {
        let prev = before_cursor.chars().nth(at_pos - 1)?;
        if !prev.is_whitespace() {
            return None;
        }
    }
    let prefix = &before_cursor[at_pos + 1..];
    let candidates = collect_candidates(prefix);
    if candidates.is_empty() {
        return None;
    }
    Some((at_pos + 1, candidates))
}

fn collect_candidates(prefix: &str) -> Vec<MentionCandidate> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let search_dir = if prefix.contains(std::path::MAIN_SEPARATOR) {
        let parent = Path::new(prefix).parent().unwrap_or(Path::new(""));
        cwd.join(parent)
    } else {
        cwd.clone()
    };

    let file_prefix = Path::new(prefix)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(prefix);

    let mut entries = match std::fs::read_dir(&search_dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry_to_candidate(&entry.path(), &cwd, file_prefix))
            .collect::<Vec<_>>(),
        Err(_) => return Vec::new(),
    };

    entries.sort_by(|a, b| a.display.cmp(&b.display));
    entries
}

fn entry_to_candidate(path: &Path, cwd: &Path, prefix: &str) -> Option<MentionCandidate> {
    let name = path.file_name()?.to_string_lossy().to_string();
    if !name.to_lowercase().starts_with(&prefix.to_lowercase()) {
        return None;
    }

    let is_dir = path.is_dir();
    let replacement = if prefix.contains(std::path::MAIN_SEPARATOR) {
        let parent = Path::new(prefix).parent().unwrap_or(Path::new(""));
        let joined = cwd.join(parent).join(&name);
        relativize(&joined, cwd)
    } else {
        name.clone()
    };

    let display = format!("{}{}", name, if is_dir { "/" } else { "" });

    Some(MentionCandidate {
        display,
        replacement,
        is_dir,
    })
}

fn relativize(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_candidates_requires_at_prefix() {
        let text = "hello";
        assert!(find_candidates(text, 5).is_none());
    }

    #[test]
    fn find_candidates_returns_entries() {
        let _cwd = std::env::current_dir().unwrap();
        let candidates = collect_candidates("");
        assert!(!candidates.is_empty());
    }
}
