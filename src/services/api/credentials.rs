//! Provider credential helpers.
//!
//! Phase A.1: read-only status and shell-profile export-line generation.
//! Phase A.2: optional macOS Keychain integration behind a feature flag
//! or explicit `/connect keychain` command.

use super::provider_catalog;

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
}
