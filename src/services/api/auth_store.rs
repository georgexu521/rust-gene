//! Structured credential storage.
//!
//! AuthStore provides a typed interface on top of the existing
//! `~/.priority-agent/.env` file used by `credentials::save_credential`.
//! It keeps backward compatibility while giving the rest of the product a
//! provider-centric API (`set`, `get`, `remove`, `list`) instead of raw
//! environment-variable manipulation.

use std::path::{Path, PathBuf};

/// A credential stored for a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCredential {
    pub provider_id: String,
    pub env_var: String,
    pub key: String,
}

/// Structured access to provider API keys.
#[derive(Debug, Clone)]
pub struct AuthStore {
    env_path: PathBuf,
}

impl Default for AuthStore {
    fn default() -> Self {
        Self::from_path(crate::services::api::credentials::credential_env_path())
    }
}

impl AuthStore {
    /// Open the default credential store at `~/.priority-agent/.env`.
    pub fn new_default() -> Self {
        Self::default()
    }

    /// Open a credential store at a specific path.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            env_path: path.into(),
        }
    }

    fn ensure_parent(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.env_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// Read the env file as a vector of preserved lines.
    fn read_lines(&self) -> Vec<String> {
        if !self.env_path.exists() {
            return Vec::new();
        }
        std::fs::read_to_string(&self.env_path)
            .unwrap_or_default()
            .lines()
            .map(|l| l.to_string())
            .collect()
    }

    /// Write lines back to the env file.
    fn write_lines(&self, lines: &[String]) -> anyhow::Result<()> {
        self.ensure_parent()?;
        let content = lines.join("\n") + "\n";
        std::fs::write(&self.env_path, content.as_bytes())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&self.env_path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Save a key for a provider.
    ///
    /// Returns `Ok(true)` if a new provider was added, `Ok(false)` if an
    /// existing credential was updated.
    pub fn set(&self, provider_id: &str, env_var: &str, key: &str) -> anyhow::Result<bool> {
        let key = key.trim();
        if key.is_empty() {
            anyhow::bail!("key must not be empty");
        }
        if key.chars().any(|ch| ch.is_control()) {
            anyhow::bail!("key must be a single-line printable value");
        }

        let mut lines = self.read_lines();
        let existed = lines.iter().any(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && line_assigns_var(trimmed, env_var)
        });

        let target_vars: &[&str] = &[env_var, "PRIORITY_AGENT_DEFAULT_PROVIDER"];
        lines.retain(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return true;
            }
            !target_vars.iter().any(|var| line_assigns_var(trimmed, var))
        });

        lines.push(format!("{}={}", env_var, dotenv_value(key)));
        lines.push(format!("PRIORITY_AGENT_DEFAULT_PROVIDER={}", provider_id));
        self.write_lines(&lines)?;
        Ok(!existed)
    }

    /// Read the stored key for a provider, if any.
    pub fn get(&self, _provider_id: &str, env_var: &str) -> Option<String> {
        // Prefer runtime env so in-process updates are visible immediately.
        if let Ok(value) = std::env::var(env_var) {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
        // Fall back to reading the file directly.
        self.read_lines().iter().find_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                return None;
            }
            let assignment = trimmed
                .strip_prefix("export ")
                .unwrap_or(trimmed)
                .trim_start();
            if !assignment.starts_with(&format!("{}=", env_var)) {
                return None;
            }
            let raw = assignment
                .strip_prefix(&format!("{}=", env_var))
                .unwrap_or("");
            let value = parse_dotenv_value(raw);
            if value.trim().is_empty() {
                None
            } else {
                Some(value)
            }
        })
    }

    /// Remove a provider's credential.
    pub fn remove(&self, provider_id: &str, env_var: &str) -> anyhow::Result<bool> {
        let mut lines = self.read_lines();
        let before = lines.len();
        lines.retain(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return true;
            }
            !line_assigns_var(trimmed, env_var)
        });
        // Also remove the default provider line when this provider is removed.
        let default_line = format!("PRIORITY_AGENT_DEFAULT_PROVIDER={}", provider_id);
        lines.retain(|line| line.trim() != default_line);
        let changed = lines.len() != before;
        if changed {
            self.write_lines(&lines)?;
        }
        Ok(changed)
    }

    /// List provider env vars that appear in the store.
    pub fn list(&self) -> Vec<String> {
        self.read_lines()
            .iter()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    return None;
                }
                let assignment = trimmed
                    .strip_prefix("export ")
                    .unwrap_or(trimmed)
                    .trim_start();
                assignment.split_once('=').map(|(var, _)| var.to_string())
            })
            .filter(|var| var != "PRIORITY_AGENT_DEFAULT_PROVIDER")
            .collect()
    }

    /// Path to the backing env file.
    pub fn path(&self) -> &Path {
        &self.env_path
    }
}

fn line_assigns_var(trimmed_line: &str, var: &str) -> bool {
    let assignment = trimmed_line
        .strip_prefix("export ")
        .unwrap_or(trimmed_line)
        .trim_start();
    assignment.starts_with(&format!("{}=", var))
}

fn dotenv_value(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/'))
    {
        return value.to_string();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn parse_dotenv_value(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        let inner = &raw[1..raw.len() - 1];
        inner.replace("\\\"", "\"").replace("\\\\", "\\")
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::env_guard::EnvVarGuard;

    fn temp_store() -> (AuthStore, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".priority-agent").join(".env");
        let store = AuthStore::from_path(path);
        std::env::remove_var("AUTH_STORE_TEST_API_KEY");
        std::env::remove_var("AUTH_STORE_TEST_OPENAI_KEY");
        (store, tmp)
    }

    #[test]
    fn set_and_get_round_trip() {
        let (store, _tmp) = temp_store();
        assert!(store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "sk-123")
            .unwrap());
        assert_eq!(
            store.get("deepseek", "AUTH_STORE_TEST_API_KEY"),
            Some("sk-123".to_string())
        );
    }

    #[test]
    fn update_existing_key() {
        let (store, _tmp) = temp_store();
        store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "sk-1")
            .unwrap();
        assert!(!store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "sk-2")
            .unwrap());
        assert_eq!(
            store.get("deepseek", "AUTH_STORE_TEST_API_KEY"),
            Some("sk-2".to_string())
        );
    }

    #[test]
    fn get_prefers_runtime_env() {
        let (store, _tmp) = temp_store();
        store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "file-key")
            .unwrap();
        let mut env = EnvVarGuard::acquire_blocking();
        env.set("AUTH_STORE_TEST_API_KEY", "runtime-key");
        assert_eq!(
            store.get("deepseek", "AUTH_STORE_TEST_API_KEY"),
            Some("runtime-key".to_string())
        );
    }

    #[test]
    fn remove_key() {
        let (store, _tmp) = temp_store();
        store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "sk-1")
            .unwrap();
        assert!(store.remove("deepseek", "AUTH_STORE_TEST_API_KEY").unwrap());
        assert!(!store.remove("deepseek", "AUTH_STORE_TEST_API_KEY").unwrap());
    }

    #[test]
    fn list_includes_only_provider_env_vars() {
        let (store, _tmp) = temp_store();
        store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "sk-1")
            .unwrap();
        store
            .set("openai", "AUTH_STORE_TEST_OPENAI_KEY", "sk-2")
            .unwrap();
        let vars = store.list();
        assert!(vars.contains(&"AUTH_STORE_TEST_API_KEY".to_string()));
        assert!(vars.contains(&"AUTH_STORE_TEST_OPENAI_KEY".to_string()));
        assert!(!vars.contains(&"PRIORITY_AGENT_DEFAULT_PROVIDER".to_string()));
    }

    #[test]
    fn rejects_empty_and_multiline_key() {
        let (store, _tmp) = temp_store();
        assert!(store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "")
            .is_err());
        assert!(store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "a\nb")
            .is_err());
    }

    #[test]
    fn preserves_unrelated_lines() {
        let (store, _tmp) = temp_store();
        store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "sk-1")
            .unwrap();
        let path = store.path().to_path_buf();
        let mut content = std::fs::read_to_string(&path).unwrap();
        content.push_str("# keep me\nUNRELATED=value\n");
        std::fs::write(&path, content).unwrap();

        store
            .set("deepseek", "AUTH_STORE_TEST_API_KEY", "sk-2")
            .unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# keep me"));
        assert!(content.contains("UNRELATED=value"));
        assert!(content.contains("AUTH_STORE_TEST_API_KEY=sk-2"));
    }
}
