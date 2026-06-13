//! Lightweight workspace detection.
//!
//! A workspace is anchored at a project root (e.g. a directory containing `.git`).
//! The TUI uses this to tag sessions and group them in the session sidebar.

use std::path::{Path, PathBuf};

/// Detect the project root by walking up from `start` looking for `.git`.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join(".git").exists() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

/// Workspace metadata for the current TUI instance.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Absolute project root.
    pub root: PathBuf,
    /// Human-readable display name (last path component).
    pub display_name: String,
}

impl Workspace {
    /// Detect the workspace from a starting directory.
    /// Falls back to the starting directory itself if no project root is found.
    pub fn detect(start: impl AsRef<Path>) -> Self {
        let start = start.as_ref();
        let root = find_project_root(start).unwrap_or_else(|| start.to_path_buf());
        let display_name = root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        Self { root, display_name }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_project_root_finds_git() {
        let tmp = std::env::temp_dir().join(format!("pa-workspace-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join(".git")).unwrap();
        std::fs::create_dir_all(tmp.join("src")).unwrap();

        let found = find_project_root(&tmp.join("src")).unwrap();
        assert_eq!(found, tmp);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn workspace_detect_uses_fallback_when_no_git() {
        let tmp =
            std::env::temp_dir().join(format!("pa-workspace-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let ws = Workspace::detect(&tmp);
        assert_eq!(ws.root, tmp);
        assert!(ws.display_name.starts_with("pa-workspace-fallback"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
