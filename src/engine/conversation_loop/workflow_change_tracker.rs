use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) struct WorkflowChangeTracker;

impl WorkflowChangeTracker {
    pub(super) fn git_status_files() -> HashSet<PathBuf> {
        let output = Command::new("git")
            .args(["status", "--short", "--untracked-files=all"])
            .output();
        let Ok(output) = output else {
            return HashSet::new();
        };
        if !output.status.success() {
            return HashSet::new();
        }
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .filter_map(Self::parse_git_status_path)
            .collect()
    }

    pub(super) fn git_status_files_since(baseline: &HashSet<PathBuf>) -> Vec<PathBuf> {
        let mut changed: Vec<_> = Self::git_status_files()
            .into_iter()
            .filter(|path| !baseline.contains(path))
            .filter(|path| Self::is_workflow_relevant_changed_path(path))
            .collect();
        changed.sort();
        changed
    }

    pub(super) fn has_changes_since(baseline: &HashSet<PathBuf>) -> bool {
        !Self::git_status_files_since(baseline).is_empty()
    }

    pub(super) fn append_changed_files_since(
        changed_files: &mut Vec<PathBuf>,
        baseline: &HashSet<PathBuf>,
    ) {
        for path in Self::git_status_files_since(baseline) {
            if !changed_files.iter().any(|existing| existing == &path) {
                changed_files.push(path);
            }
        }
    }

    pub(super) fn is_workflow_relevant_changed_path(path: &Path) -> bool {
        !Self::is_generated_runtime_artifact(path)
    }

    fn is_generated_runtime_artifact(path: &Path) -> bool {
        let mut components = path.components().filter_map(|component| {
            component
                .as_os_str()
                .to_str()
                .map(|part| part.to_ascii_lowercase())
        });

        components.any(|part| {
            matches!(
                part.as_str(),
                ".venv"
                    | "venv"
                    | "env"
                    | "node_modules"
                    | "target"
                    | "__pycache__"
                    | ".pytest_cache"
                    | ".mypy_cache"
                    | ".ruff_cache"
                    | ".ds_store"
            ) || part.ends_with(".egg-info")
                || part.ends_with(".dist-info")
                || part.ends_with(".pyc")
        })
    }

    fn parse_git_status_path(line: &str) -> Option<PathBuf> {
        let path = line.get(3..)?.trim();
        if path.is_empty() {
            return None;
        }
        let path = path.rsplit_once(" -> ").map(|(_, new)| new).unwrap_or(path);
        Some(PathBuf::from(path.trim_matches('"')))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_runtime_artifacts_do_not_count_as_workflow_changes() {
        assert!(!WorkflowChangeTracker::is_workflow_relevant_changed_path(
            Path::new(".venv/lib/python3.12/site-packages/pkg.py")
        ));
        assert!(!WorkflowChangeTracker::is_workflow_relevant_changed_path(
            Path::new("fixtures/demo/core_terminal_demo.egg-info/PKG-INFO")
        ));
        assert!(!WorkflowChangeTracker::is_workflow_relevant_changed_path(
            Path::new("src/__pycache__/main.cpython-312.pyc")
        ));
        assert!(WorkflowChangeTracker::is_workflow_relevant_changed_path(
            Path::new("src/main.rs")
        ));
    }

    #[test]
    fn parse_git_status_path_uses_rename_target_and_unquotes_paths() {
        assert_eq!(
            WorkflowChangeTracker::parse_git_status_path("R  old.rs -> src/new.rs"),
            Some(PathBuf::from("src/new.rs"))
        );
        assert_eq!(
            WorkflowChangeTracker::parse_git_status_path("?? \"src/generated file.rs\""),
            Some(PathBuf::from("src/generated file.rs"))
        );
        assert_eq!(WorkflowChangeTracker::parse_git_status_path("?? "), None);
    }
}
