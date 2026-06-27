//! Provider credential helpers.
//!
//! Phase A.1: read-only status and shell-profile export-line generation.
//! Phase A.2: optional macOS Keychain integration behind a feature flag
//! or explicit `/connect keychain` command.

use std::path::PathBuf;

use super::provider_catalog;

/// Path to the product credential env file.
///
/// On all platforms: `~/.priority-agent/.env`.
/// The directory is created if it does not exist.
pub fn credential_env_path() -> PathBuf {
    if let Some(path) = std::env::var_os("PRIORITY_AGENT_CREDENTIAL_ENV_PATH") {
        return PathBuf::from(path);
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".priority-agent").join(".env")
}

/// Load the product credential env file (`~/.priority-agent/.env`) into the
/// current process environment.  Unknown lines and comments are preserved by
/// `dotenvy` without error.
///
/// Returns `Ok(())` when the file was loaded successfully, or when it does
/// not exist yet (first-run).
pub fn load_product_credential_env() -> Result<(), CredentialLoadError> {
    let path = credential_env_path();
    if !path.exists() {
        return Ok(());
    }
    dotenvy::from_path(&path)
        .map(|_| ())
        .map_err(|e| CredentialLoadError::LoadFailed {
            path: path.display().to_string(),
            reason: e.to_string(),
        })
}

#[derive(Debug)]
pub enum CredentialLoadError {
    LoadFailed { path: String, reason: String },
}

impl std::fmt::Display for CredentialLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadFailed { path, reason } => {
                write!(f, "failed to load credential env {}: {}", path, reason)
            }
        }
    }
}

/// Outcome of saving a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSaveOutcome {
    /// Key was saved, env refreshed, and the registry can construct the provider.
    Verified,
    /// Key was saved, but the provider could not be constructed in this session.
    SavedUnverified,
    /// Key was not saved because the provider is unknown, the key is blank, or writing failed.
    Rejected { reason: String },
}

/// Outcome of removing a stored credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialRemoveOutcome {
    /// A stored credential was removed.
    Removed,
    /// The provider is known, but no stored credential was found.
    NotFound,
    /// The credential was not removed because the provider is unknown or the
    /// store could not be updated.
    Rejected { reason: String },
}

/// Save a provider credential to `~/.priority-agent/.env`.
///
/// Writes the provider-specific key env var from the catalog (e.g.
/// `MINIMAX_API_KEY=<redacted>`) and sets `PRIORITY_AGENT_DEFAULT_PROVIDER`.
/// Preserves existing lines that are not the target variables.
pub fn save_credential(provider_id: &str, key: &str) -> CredentialSaveOutcome {
    let Some(entry) = provider_catalog::find(provider_id) else {
        return CredentialSaveOutcome::Rejected {
            reason: format!("unknown provider '{}'", provider_id),
        };
    };
    let key = key.trim();
    if key.is_empty() {
        return CredentialSaveOutcome::Rejected {
            reason: "key must not be empty".to_string(),
        };
    }
    if key.chars().any(|ch| ch.is_control()) {
        return CredentialSaveOutcome::Rejected {
            reason: "key must be a single-line printable value".to_string(),
        };
    }

    let key_env_var = &entry.key_env_vars[0];
    let store = super::auth_store::AuthStore::from_path(credential_env_path());
    if let Err(err) = store.set(provider_id, key_env_var, key) {
        return CredentialSaveOutcome::Rejected {
            reason: format!("cannot save credential: {}", err),
        };
    }

    // Set in current process immediately
    set_env_for_session(key_env_var, key);
    set_env_for_session("PRIORITY_AGENT_DEFAULT_PROVIDER", provider_id);

    let registry = super::provider::ProviderRegistry::from_env();
    if registry.get(provider_id).is_some() {
        CredentialSaveOutcome::Verified
    } else {
        CredentialSaveOutcome::SavedUnverified
    }
}

/// Remove a provider credential from the product dotenv mirror.
pub fn remove_credential(provider_id: &str) -> CredentialRemoveOutcome {
    let Some(entry) = provider_catalog::find(provider_id) else {
        return CredentialRemoveOutcome::Rejected {
            reason: format!("unknown provider '{}'", provider_id),
        };
    };
    let key_env_var = &entry.key_env_vars[0];
    let store = super::auth_store::AuthStore::from_path(credential_env_path());
    match store.remove(provider_id, key_env_var) {
        Ok(true) => {
            std::env::remove_var(key_env_var);
            if std::env::var("PRIORITY_AGENT_DEFAULT_PROVIDER")
                .ok()
                .as_deref()
                == Some(provider_id)
            {
                std::env::remove_var("PRIORITY_AGENT_DEFAULT_PROVIDER");
            }
            CredentialRemoveOutcome::Removed
        }
        Ok(false) => CredentialRemoveOutcome::NotFound,
        Err(err) => CredentialRemoveOutcome::Rejected {
            reason: format!("cannot remove credential: {}", err),
        },
    }
}

/// Set an env var in the current process for the duration of this session.
pub fn set_env_for_session(var: &str, value: &str) {
    std::env::set_var(var, value);
}

/// Status of a provider's credentials.
#[derive(Debug, Clone)]
pub struct CredentialStatus {
    pub provider_id: String,
    pub provider_label: String,
    /// Whether at least one key env var is set and non-empty.
    pub configured: bool,
    /// Which env var holds the key (first non-empty from the catalog).
    pub active_env_var: Option<String>,
    /// Shell-profile export line for setup.
    pub export_line: String,
    /// One-line setup hint.
    pub setup_hint: String,
    /// Whether the key is sourced from the macOS Keychain (feature-flagged).
    pub keychain_available: bool,
    /// Whether the current key came from the Keychain.
    pub from_keychain: bool,
}

/// Check credential status for all built-in providers.
pub fn status_all() -> Vec<CredentialStatus> {
    provider_catalog::builtin_catalog()
        .into_iter()
        .map(|entry| {
            let active = entry
                .key_env_vars
                .iter()
                .find(|v| {
                    std::env::var(v)
                        .map(|val| !val.trim().is_empty())
                        .unwrap_or(false)
                })
                .cloned();
            CredentialStatus {
                provider_id: entry.id.clone(),
                provider_label: entry.label.clone(),
                configured: active.is_some(),
                active_env_var: active,
                export_line: provider_catalog::shell_export_line(&entry.id).unwrap_or_default(),
                setup_hint: entry.setup_hint.clone(),
                keychain_available: cfg!(target_os = "macos"),
                from_keychain: false, // Phase A.2
            }
        })
        .collect()
}

/// Check credential status for a single provider.
pub fn status_for(id: &str) -> Option<CredentialStatus> {
    provider_catalog::find(id).map(|entry| {
        let active = entry
            .key_env_vars
            .iter()
            .find(|v| {
                std::env::var(v)
                    .map(|val| !val.trim().is_empty())
                    .unwrap_or(false)
            })
            .cloned();
        CredentialStatus {
            provider_id: entry.id.clone(),
            provider_label: entry.label.clone(),
            configured: active.is_some(),
            active_env_var: active,
            export_line: provider_catalog::shell_export_line(&entry.id).unwrap_or_default(),
            setup_hint: entry.setup_hint.clone(),
            keychain_available: cfg!(target_os = "macos"),
            from_keychain: false,
        }
    })
}

/// Format a human-readable setup message for the `/connect` command.
pub fn connect_message(id: &str) -> Option<String> {
    let status = status_for(id)?;
    if status.configured {
        Some(format!(
            "Provider \"{}\" is already configured (env var: {}).\n\
             To switch to this provider, use /model or select it in the provider picker.",
            status.provider_label,
            status.active_env_var.as_deref().unwrap_or("unknown"),
        ))
    } else {
        let mut msg = format!(
            "To connect to {}:\n\n\
             Add this line to your shell profile (~/.zshrc or ~/.bashrc):\n\
             \n  {}\n\
             \nThen restart your terminal or run: source ~/.zshrc\n\
             \nNext steps:\n\
             - Verify: /provider status\n\
             - Select this provider from the picker or run /model\n",
            status.provider_label, status.export_line,
        );
        if let Some(docs_url) = provider_catalog::find(id).and_then(|e| e.docs_url) {
            msg.push_str(&format!("\nDocs & API console: {}\n", docs_url));
        }
        Some(msg)
    }
}

/// Human-readable credential summary for `/provider status` or diagnostics.
pub fn status_summary() -> String {
    let all = status_all();
    let configured: Vec<_> = all.iter().filter(|s| s.configured).collect();
    let unconfigured: Vec<_> = all.iter().filter(|s| !s.configured).collect();

    let mut out = String::from("Provider Credentials:\n\n");
    if configured.is_empty() {
        out.push_str("No providers are configured.\n\n");
    } else {
        out.push_str("Configured:\n");
        for s in &configured {
            out.push_str(&format!(
                "  {} — using {}\n",
                s.provider_label,
                s.active_env_var.as_deref().unwrap_or("unknown")
            ));
        }
    }
    if !unconfigured.is_empty() {
        out.push_str("\nNot configured:\n");
        for s in &unconfigured {
            out.push_str(&format!("  {} — {}\n", s.provider_label, s.setup_hint));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_all_returns_six_entries() {
        let all = status_all();
        assert_eq!(all.len(), 6);
    }

    #[test]
    fn status_for_valid_id_returns_entry() {
        let s = status_for("deepseek").expect("deepseek should exist");
        assert_eq!(s.provider_label, "DeepSeek");
        assert!(!s.export_line.is_empty());
    }

    #[test]
    fn status_for_unknown_id_returns_none() {
        assert!(status_for("nonexistent").is_none());
    }

    #[test]
    fn connect_message_for_unconfigured_includes_setup() {
        // Without DEEPSEEK_API_KEY set, this provider is unconfigured.
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        env.remove("DEEPSEEK_API_KEY");
        let msg = connect_message("deepseek").expect("should have message");
        assert!(msg.contains("export DEEPSEEK_API_KEY"));
        assert!(msg.contains("source ~/.zshrc"));
    }

    #[test]
    fn status_summary_includes_providers() {
        let summary = status_summary();
        assert!(summary.contains("MiniMax"));
        assert!(summary.contains("DeepSeek"));
    }

    #[test]
    fn credential_env_path_returns_home_dir() {
        let path = credential_env_path();
        assert!(path.ends_with(".priority-agent/.env"));
    }

    #[test]
    fn load_product_credential_env_returns_ok_when_file_missing() {
        // File shouldn't exist under a temp home
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        env.set("HOME", "/tmp/priority-agent-test-nonexistent");
        env.remove("PRIORITY_AGENT_CREDENTIAL_ENV_PATH");
        let result = load_product_credential_env();
        assert!(result.is_ok());
    }

    #[test]
    fn save_credential_writes_to_temp_env_file() {
        let tmp = tempfile::tempdir().unwrap();
        let env_path = tmp.path().join(".priority-agent").join(".env");
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        env.set(
            "PRIORITY_AGENT_CREDENTIAL_ENV_PATH",
            &env_path.display().to_string(),
        );

        let outcome = save_credential("minimax", "sk-test-key-123");
        assert_eq!(outcome, CredentialSaveOutcome::Verified);

        let content = std::fs::read_to_string(&env_path).unwrap();
        assert!(content.contains("MINIMAX_API_KEY=sk-test-key-123"));
        assert!(content.contains("PRIORITY_AGENT_DEFAULT_PROVIDER=minimax"));
        assert_eq!(
            std::env::var("MINIMAX_API_KEY").unwrap_or_default(),
            "sk-test-key-123"
        );
        assert_eq!(
            std::env::var("PRIORITY_AGENT_DEFAULT_PROVIDER").unwrap_or_default(),
            "minimax"
        );
    }

    #[test]
    fn save_credential_trims_key_before_persisting() {
        let tmp = tempfile::tempdir().unwrap();
        let env_path = tmp.path().join(".priority-agent").join(".env");
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        env.set(
            "PRIORITY_AGENT_CREDENTIAL_ENV_PATH",
            &env_path.display().to_string(),
        );

        let outcome = save_credential("minimax", "  trimmed-key  ");
        assert_eq!(outcome, CredentialSaveOutcome::Verified);

        let content = std::fs::read_to_string(&env_path).unwrap();
        assert!(content.contains("MINIMAX_API_KEY=trimmed-key\n"));
        assert_eq!(
            std::env::var("MINIMAX_API_KEY").unwrap_or_default(),
            "trimmed-key"
        );
    }

    #[test]
    fn save_credential_rejects_multiline_key() {
        let result = save_credential("minimax", "first-line\nSECOND_VAR=injected");
        assert!(matches!(
            result,
            CredentialSaveOutcome::Rejected { reason }
                if reason.contains("single-line printable")
        ));
    }

    #[test]
    fn save_credential_quotes_dotenv_special_values() {
        let tmp = tempfile::tempdir().unwrap();
        let env_path = tmp.path().join(".priority-agent").join(".env");
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        env.set(
            "PRIORITY_AGENT_CREDENTIAL_ENV_PATH",
            &env_path.display().to_string(),
        );

        let outcome = save_credential("minimax", "key#with=specials");
        assert_eq!(outcome, CredentialSaveOutcome::Verified);

        let content = std::fs::read_to_string(&env_path).unwrap();
        assert!(content.contains("MINIMAX_API_KEY=\"key#with=specials\"\n"));
        assert_eq!(
            std::env::var("MINIMAX_API_KEY").unwrap_or_default(),
            "key#with=specials"
        );
    }

    #[test]
    fn save_credential_replaces_target_vars_and_preserves_unrelated_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let env_path = tmp.path().join(".priority-agent").join(".env");
        std::fs::create_dir_all(env_path.parent().unwrap()).unwrap();
        std::fs::write(
            &env_path,
            "# keep me\nUNRELATED=value\nexport MINIMAX_API_KEY=old\nPRIORITY_AGENT_DEFAULT_PROVIDER=openai\n",
        )
        .unwrap();
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        env.set(
            "PRIORITY_AGENT_CREDENTIAL_ENV_PATH",
            &env_path.display().to_string(),
        );

        let outcome = save_credential("minimax", "new-key");
        assert_eq!(outcome, CredentialSaveOutcome::Verified);

        let content = std::fs::read_to_string(&env_path).unwrap();
        assert!(content.contains("# keep me\n"));
        assert!(content.contains("UNRELATED=value\n"));
        assert!(content.contains("MINIMAX_API_KEY=new-key\n"));
        assert!(content.contains("PRIORITY_AGENT_DEFAULT_PROVIDER=minimax\n"));
        assert!(!content.contains("old"));
        assert!(!content.contains("PRIORITY_AGENT_DEFAULT_PROVIDER=openai"));
    }

    #[test]
    fn save_credential_rejects_unknown_provider() {
        let result = save_credential("nonexistent", "key");
        assert!(matches!(
            result,
            CredentialSaveOutcome::Rejected { reason } if reason.contains("unknown provider")
        ));
    }

    #[test]
    fn save_credential_rejects_empty_key() {
        let result = save_credential("minimax", "");
        assert!(matches!(
            result,
            CredentialSaveOutcome::Rejected { reason } if reason.contains("must not be empty")
        ));
    }
}
