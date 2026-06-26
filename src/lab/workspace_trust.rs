//! Project-scoped LabRun workspace trust lookup.
//!
//! This is intentionally separate from process-level environment flags. The
//! environment override remains available for tests and explicit debugging, but
//! normal validation resolves trust from a repository-bound record.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceTrustResolution {
    pub(crate) level: String,
    pub(crate) source: String,
    pub(crate) trust_scope: String,
    pub(crate) canonical_path: String,
    pub(crate) repo_identity: String,
    pub(crate) repo_fingerprint: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TrustedWorkspacesFile {
    #[serde(default)]
    workspaces: Vec<TrustedWorkspaceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustedWorkspaceRecord {
    canonical_path: String,
    #[serde(default)]
    repo_identity: String,
    #[serde(default)]
    repo_fingerprint: String,
    #[serde(default)]
    trusted_at: String,
    #[serde(default)]
    trust_scope: String,
    #[serde(default)]
    trust_scopes: Vec<String>,
    #[serde(default)]
    approved_by: String,
    #[serde(default)]
    source: String,
}

pub(crate) fn resolve_lab_workspace_trust(cwd: &Path) -> WorkspaceTrustResolution {
    let canonical = canonical_path(cwd);
    let repo_identity = repo_identity(cwd).unwrap_or_else(|| "unknown".to_string());
    let repo_fingerprint = repo_fingerprint(&canonical, &repo_identity);
    if let Some(level) = process_override() {
        return WorkspaceTrustResolution {
            level,
            source: "env_override".to_string(),
            trust_scope: "allow_package_scripts".to_string(),
            canonical_path: canonical.display().to_string(),
            repo_identity,
            repo_fingerprint,
        };
    }
    let trusted = read_trusted_workspaces()
        .workspaces
        .into_iter()
        .any(|record| {
            let identity_matches =
                !record.repo_identity.is_empty() && record.repo_identity == repo_identity;
            let fingerprint_matches =
                !record.repo_fingerprint.is_empty() && record.repo_fingerprint == repo_fingerprint;
            record_allows_scope(&record, "allow_package_scripts")
                && record.canonical_path == canonical.display().to_string()
                && (identity_matches || fingerprint_matches)
        });
    WorkspaceTrustResolution {
        level: if trusted { "trusted" } else { "unknown" }.to_string(),
        source: if trusted {
            "trusted_workspaces_file".to_string()
        } else {
            "no_project_trust_record".to_string()
        },
        trust_scope: "allow_package_scripts".to_string(),
        canonical_path: canonical.display().to_string(),
        repo_identity,
        repo_fingerprint,
    }
}

fn record_allows_scope(record: &TrustedWorkspaceRecord, scope: &str) -> bool {
    record.trust_scopes.iter().any(|value| value == scope)
        || (scope == "allow_package_scripts" && record.trust_scope == "package_scripts")
        || record.trust_scope == scope
}

fn process_override() -> Option<String> {
    match std::env::var("PRIORITY_AGENT_LAB_WORKSPACE_TRUST")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "trusted" | "trust" | "true" | "1" => Some("trusted".to_string()),
        "untrusted" | "false" | "0" => Some("untrusted".to_string()),
        _ => None,
    }
}

fn read_trusted_workspaces() -> TrustedWorkspacesFile {
    let Some(path) = trusted_workspaces_path() else {
        return TrustedWorkspacesFile::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn trusted_workspaces_path() -> Option<PathBuf> {
    std::env::var_os("PRIORITY_AGENT_TRUSTED_WORKSPACES_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            dirs::home_dir()
                .map(|home| home.join(".priority-agent").join("trusted-workspaces.json"))
        })
}

fn canonical_path(cwd: &Path) -> PathBuf {
    cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf())
}

fn repo_identity(cwd: &Path) -> Option<String> {
    git_stdout(cwd, &["config", "--get", "remote.origin.url"])
        .or_else(|| git_stdout(cwd, &["rev-parse", "--show-toplevel"]))
}

fn repo_fingerprint(canonical: &Path, repo_identity: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical.display().to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(repo_identity.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn git_stdout(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EnvRestore {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os(key);
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            unsafe { std::env::remove_var(key) };
            Self { key, previous }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe { std::env::set_var(self.key, value) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    #[test]
    fn workspace_trust_is_project_scoped() {
        let project_a = tempfile::tempdir().unwrap();
        let project_b = tempfile::tempdir().unwrap();
        let trust_file = tempfile::NamedTempFile::new().unwrap();
        let _trust_path =
            EnvRestore::set("PRIORITY_AGENT_TRUSTED_WORKSPACES_PATH", trust_file.path());
        let _process_override = EnvRestore::remove("PRIORITY_AGENT_LAB_WORKSPACE_TRUST");

        let before = resolve_lab_workspace_trust(project_a.path());
        assert_eq!(before.level, "unknown");
        let record = TrustedWorkspacesFile {
            workspaces: vec![TrustedWorkspaceRecord {
                canonical_path: before.canonical_path.clone(),
                repo_identity: String::new(),
                repo_fingerprint: before.repo_fingerprint.clone(),
                trusted_at: "2026-06-26T00:00:00Z".to_string(),
                trust_scope: String::new(),
                trust_scopes: vec!["allow_package_scripts".to_string()],
                approved_by: "test".to_string(),
                source: "test".to_string(),
            }],
        };
        std::fs::write(
            trust_file.path(),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();

        let trusted_a = resolve_lab_workspace_trust(project_a.path());
        assert_eq!(trusted_a.level, "trusted");
        assert_eq!(trusted_a.source, "trusted_workspaces_file");
        assert_eq!(trusted_a.trust_scope, "allow_package_scripts");

        let unknown_b = resolve_lab_workspace_trust(project_b.path());
        assert_eq!(unknown_b.level, "unknown");
        assert_eq!(unknown_b.source, "no_project_trust_record");
    }

    #[test]
    fn workspace_trust_env_override_is_explicit_and_global() {
        let project = tempfile::tempdir().unwrap();
        let _trust_path = EnvRestore::set(
            "PRIORITY_AGENT_TRUSTED_WORKSPACES_PATH",
            project.path().join("missing-trust.json"),
        );
        let _process_override = EnvRestore::set("PRIORITY_AGENT_LAB_WORKSPACE_TRUST", "trusted");

        let resolution = resolve_lab_workspace_trust(project.path());

        assert_eq!(resolution.level, "trusted");
        assert_eq!(resolution.source, "env_override");
        assert_eq!(resolution.trust_scope, "allow_package_scripts");
    }

    #[test]
    fn workspace_trust_supports_legacy_package_script_scope() {
        let project = tempfile::tempdir().unwrap();
        let trust_file = tempfile::NamedTempFile::new().unwrap();
        let _trust_path =
            EnvRestore::set("PRIORITY_AGENT_TRUSTED_WORKSPACES_PATH", trust_file.path());
        let _process_override = EnvRestore::remove("PRIORITY_AGENT_LAB_WORKSPACE_TRUST");

        let before = resolve_lab_workspace_trust(project.path());
        let record = TrustedWorkspacesFile {
            workspaces: vec![TrustedWorkspaceRecord {
                canonical_path: before.canonical_path.clone(),
                repo_identity: String::new(),
                repo_fingerprint: before.repo_fingerprint.clone(),
                trusted_at: "2026-06-26T00:00:00Z".to_string(),
                trust_scope: "package_scripts".to_string(),
                trust_scopes: Vec::new(),
                approved_by: "test".to_string(),
                source: "legacy_test".to_string(),
            }],
        };
        std::fs::write(
            trust_file.path(),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();

        let trusted = resolve_lab_workspace_trust(project.path());
        assert_eq!(trusted.level, "trusted");
        assert_eq!(trusted.trust_scope, "allow_package_scripts");
    }
}
