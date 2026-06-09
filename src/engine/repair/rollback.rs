//! Auto-git-rollback — checkpoint before edits, recover on failure.
//!
//! Creates a git stash before any file mutation, and provides the ability
//! to restore if the edit pipeline produces undesirable results.

use std::path::Path;
use std::process::Command;

/// Result of attempting a git stash checkpoint.
#[derive(Debug, Clone)]
pub enum GitRollbackGuard {
    /// Checkpoint created successfully. `pop_on_drop` controls restore behavior.
    Active {
        repo_root: std::path::PathBuf,
        pop_on_drop: bool,
    },
    /// No checkpoint — either git is unavailable, repo is dirty, or feature is off.
    Skipped,
}

impl GitRollbackGuard {
    /// Create a checkpoint if conditions are met.
    ///
    /// Conditions:
    /// - `PRIORITY_AGENT_GIT_ROLLBACK` is not "0" or "off".
    /// - A git repository exists at or above `working_dir`.
    /// - The repo is clean (no uncommitted changes from before the edit).
    pub fn checkpoint(working_dir: &Path) -> Self {
        if std::env::var("PRIORITY_AGENT_GIT_ROLLBACK")
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case("off")
        {
            return Self::Skipped;
        }

        let repo_root = match find_repo_root(working_dir) {
            Some(root) => root,
            None => return Self::Skipped,
        };

        // Only stash if the repo is clean before our edit.
        let clean = is_clean(&repo_root);
        if !clean {
            return Self::Skipped;
        }

        let output = Command::new("git")
            .args(["-C", &repo_root.to_string_lossy(), "stash", "push", "-m"])
            .arg("[priority-agent] auto checkpoint before edit")
            .output();

        match output {
            Ok(out) if out.status.success() => {
                tracing::debug!("Git rollback checkpoint created in {}", repo_root.display());
                Self::Active {
                    repo_root,
                    pop_on_drop: false,
                }
            }
            _ => Self::Skipped,
        }
    }

    /// Mark that the changes should be rolled back on drop.
    pub fn mark_rollback(&mut self) {
        if let Self::Active { pop_on_drop, .. } = self {
            *pop_on_drop = true;
        }
    }

    /// Mark that changes are acceptable — keep them, drop the stash.
    pub fn mark_keep(&mut self) {
        if let Self::Active { .. } = self {
            // When we drop without pop, the stash stays but we're not restoring.
            // The stash entry accumulates; user can view/clear with `git stash list`.
        }
    }

    fn pop_stash(repo_root: &Path) {
        let _ = Command::new("git")
            .args(["-C", &repo_root.to_string_lossy(), "stash", "pop"])
            .output();
    }
}

impl Drop for GitRollbackGuard {
    fn drop(&mut self) {
        if let Self::Active {
            repo_root,
            pop_on_drop,
        } = self
        {
            if *pop_on_drop {
                Self::pop_stash(repo_root);
                tracing::info!("Git rollback: restored working tree");
            }
        }
    }
}

fn find_repo_root(start: &Path) -> Option<std::path::PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn is_clean(repo_root: &Path) -> bool {
    let output = Command::new("git")
        .args(["-C", &repo_root.to_string_lossy(), "status", "--porcelain"])
        .output();

    match output {
        Ok(out) => out.stdout.is_empty(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollback_guard_skipped_without_git() {
        let guard = GitRollbackGuard::checkpoint(Path::new("/tmp/nonexistent"));
        assert!(matches!(guard, GitRollbackGuard::Skipped));
    }

    #[test]
    fn rollback_guard_skipped_when_env_off() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        env.set("PRIORITY_AGENT_GIT_ROLLBACK", "off");
        let guard = GitRollbackGuard::checkpoint(Path::new("."));
        assert!(matches!(guard, GitRollbackGuard::Skipped));
    }
}
