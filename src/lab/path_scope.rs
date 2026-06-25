//! Shared LabRun path-scope normalization.
//!
//! Graduate allowed scopes and changed files can come from different sources:
//! Git, provider JSON, manual binding, or persisted task records. Normalize them
//! once before comparing so scope checks do not depend on string spelling.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LabPathScopeError {
    path: String,
    reason: &'static str,
}

impl LabPathScopeError {
    fn new(path: &str, reason: &'static str) -> Self {
        Self {
            path: path.to_string(),
            reason,
        }
    }
}

impl fmt::Display for LabPathScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid LabRun relative path '{}': {}",
            self.path, self.reason
        )
    }
}

impl std::error::Error for LabPathScopeError {}

pub(crate) fn normalize_lab_relative_path(path: &str) -> Result<String, LabPathScopeError> {
    let original = path;
    let mut path = path.trim().replace('\\', "/");
    while let Some(stripped) = path.strip_prefix("./") {
        path = stripped.to_string();
    }
    while path.ends_with('/') {
        path.pop();
    }
    if path.is_empty() || path == "." {
        return Err(LabPathScopeError::new(original, "path is empty"));
    }
    if path.starts_with('/') || has_windows_drive_prefix(&path) {
        return Err(LabPathScopeError::new(
            original,
            "absolute paths are not allowed",
        ));
    }

    let mut parts = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" => return Err(LabPathScopeError::new(original, "empty path segment")),
            "." => {}
            ".." => {
                return Err(LabPathScopeError::new(
                    original,
                    "parent traversal is not allowed",
                ))
            }
            value => parts.push(value),
        }
    }
    let normalized = parts.join("/");
    if is_internal_lab_runtime_path(&normalized) {
        return Err(LabPathScopeError::new(
            original,
            "internal runtime paths are not allowed",
        ));
    }
    Ok(normalized)
}

pub(crate) fn normalize_lab_relative_paths(
    paths: &[String],
) -> Result<Vec<String>, LabPathScopeError> {
    let mut normalized = Vec::new();
    for path in paths {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        normalized.push(normalize_lab_relative_path(path)?);
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

pub(crate) fn changed_files_within_scope(
    allowed_scope: &[String],
    changed_files: &[String],
) -> Result<(), LabPathScopeError> {
    if changed_files.is_empty() {
        return Ok(());
    }
    let allowed_scope = normalize_lab_relative_paths(allowed_scope)?;
    if allowed_scope.is_empty() {
        return Err(LabPathScopeError::new(
            "",
            "changed files require at least one allowed scope",
        ));
    }
    for file in changed_files {
        let normalized_file = normalize_lab_relative_path(file)?;
        if !path_matches_normalized_scope(&normalized_file, &allowed_scope) {
            return Err(LabPathScopeError::new(
                file,
                "path is outside allowed scope",
            ));
        }
    }
    Ok(())
}

pub(crate) fn is_internal_lab_runtime_path(path: &str) -> bool {
    let path = path.trim().trim_start_matches("./").replace('\\', "/");
    path == ".priority-agent"
        || path.starts_with(".priority-agent/")
        || path == ".git"
        || path.starts_with(".git/")
        || path == ".claude/worktrees"
        || path.starts_with(".claude/worktrees/")
        || path == "target/lab-live-validation"
        || path.starts_with("target/lab-live-validation/")
}

fn path_matches_normalized_scope(file: &str, allowed_scope: &[String]) -> bool {
    allowed_scope.iter().any(|scope| {
        file == scope || file.starts_with(&format!("{}/", scope.trim_end_matches('/')))
    })
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_safe_relative_paths() {
        assert_eq!(
            normalize_lab_relative_path("./src\\lab\\mod.rs").unwrap(),
            "src/lab/mod.rs"
        );
        assert_eq!(
            normalize_lab_relative_paths(&[
                "src/lab".to_string(),
                "./src/lab/".to_string(),
                "src/main.rs".to_string(),
            ])
            .unwrap(),
            vec!["src/lab".to_string(), "src/main.rs".to_string()]
        );
    }

    #[test]
    fn rejects_unsafe_relative_paths() {
        for path in [
            "",
            ".",
            "/tmp/file",
            "C:/tmp/file",
            "../file",
            "src/../secrets.rs",
            ".git/config",
            ".priority-agent/state.json",
            "target/lab-live-validation/run.json",
        ] {
            assert!(
                normalize_lab_relative_path(path).is_err(),
                "{path} should be rejected"
            );
        }
    }

    #[test]
    fn validates_changed_files_against_normalized_scope() {
        assert!(changed_files_within_scope(
            &["src".to_string()],
            &["src\\lab\\mod.rs".to_string()]
        )
        .is_ok());
        assert!(
            changed_files_within_scope(&["src/lab".to_string()], &["src/main.rs".to_string()])
                .is_err()
        );
    }
}
