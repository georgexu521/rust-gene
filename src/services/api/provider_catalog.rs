//! Centralized provider catalog.
//!
//! Single source of truth for built-in provider metadata: id, label,
//! provider type, required env vars, base URL, default model, supported
//! model list, docs URL, and setup guidance.
//!
//! This replaces the duplicated model lists in `palette.rs` and
//! `desktop_state.rs`, and extends the existing `DEFAULT_PROVIDER_ENV_SPECS`
//! from `provider.rs` with additional product metadata.

use serde::{Deserialize, Serialize};

/// Static metadata for a built-in provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCatalogEntry {
    /// Machine-readable id (e.g. "minimax", "deepseek").
    pub id: String,
    /// Human-readable label (e.g. "MiniMax", "DeepSeek").
    pub label: String,
    /// Provider type for client construction.
    pub provider_type: ProviderCatalogType,
    /// Environment variable names for the API key (first non-empty wins).
    pub key_env_vars: Vec<String>,
    /// Environment variable names for base URL override.
    pub base_url_env_vars: Vec<String>,
    /// Environment variable names for model override.
    pub model_env_vars: Vec<String>,
    /// Default base URL when none is configured.
    pub default_base_url: String,
    /// Default model when none is configured.
    pub default_model: String,
    /// Supported models (visible in model picker).
    pub supported_models: Vec<String>,
    /// Link to provider docs or API console.
    pub docs_url: Option<String>,
    /// One-line setup hint for onboarding.
    pub setup_hint: String,
}

/// Provider type tag for the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderCatalogType {
    Minimax,
    Kimi,
    KimiCode,
    DeepSeek,
    Glm,
    OpenAI,
    OpenAICompat,
    Anthropic,
    Google,
    Azure,
    Custom,
}

/// The canonical built-in provider catalog.
///
/// Order is deterministic and advisory.  Providers without a configured
/// API key are shown as "unconfigured" in UI pickers.
pub fn builtin_catalog() -> Vec<ProviderCatalogEntry> {
    vec![
        ProviderCatalogEntry {
            id: "minimax".into(),
            label: "MiniMax".into(),
            provider_type: ProviderCatalogType::Minimax,
            key_env_vars: vec!["MINIMAX_API_KEY".into()],
            base_url_env_vars: vec!["MINIMAX_BASE_URL".into()],
            model_env_vars: vec!["MINIMAX_MODEL".into()],
            default_base_url: "https://api.minimax.io/v1".into(),
            default_model: "MiniMax-M3".into(),
            supported_models: vec![
                "MiniMax-M3".into(),
                "MiniMax-M2.7".into(),
                "MiniMax-M2.7-highspeed".into(),
                "MiniMax-M2.5".into(),
                "MiniMax-M2".into(),
            ],
            docs_url: Some("https://platform.minimax.io".into()),
            setup_hint: "Set MINIMAX_API_KEY to your API key from platform.minimax.io".into(),
        },
        ProviderCatalogEntry {
            id: "kimi-code".into(),
            label: "Kimi Code".into(),
            provider_type: ProviderCatalogType::KimiCode,
            key_env_vars: vec!["KIMI_CODE_API_KEY".into()],
            base_url_env_vars: vec!["KIMI_CODE_BASE_URL".into()],
            model_env_vars: vec!["KIMI_CODE_MODEL".into()],
            default_base_url: "https://api.kimi.com/coding/v1".into(),
            default_model: "kimi-for-coding".into(),
            supported_models: vec!["kimi-for-coding".into()],
            docs_url: Some("https://platform.moonshot.cn".into()),
            setup_hint: "Set KIMI_CODE_API_KEY to your Kimi Code API key".into(),
        },
        ProviderCatalogEntry {
            id: "deepseek".into(),
            label: "DeepSeek".into(),
            provider_type: ProviderCatalogType::DeepSeek,
            key_env_vars: vec!["DEEPSEEK_API_KEY".into()],
            base_url_env_vars: vec!["DEEPSEEK_BASE_URL".into()],
            model_env_vars: vec!["DEEPSEEK_MODEL".into()],
            default_base_url: "https://api.deepseek.com".into(),
            default_model: "deepseek-v4-pro".into(),
            supported_models: vec![
                "deepseek-v4-pro".into(),
                "deepseek-v4-flash".into(),
                "deepseek-chat".into(),
            ],
            docs_url: Some("https://platform.deepseek.com".into()),
            setup_hint: "Set DEEPSEEK_API_KEY to your API key from platform.deepseek.com".into(),
        },
        ProviderCatalogEntry {
            id: "glm".into(),
            label: "GLM".into(),
            provider_type: ProviderCatalogType::Glm,
            key_env_vars: vec![
                "GLM_API_KEY".into(),
                "ZAI_API_KEY".into(),
                "ZHIPUAI_API_KEY".into(),
                "BIGMODEL_API_KEY".into(),
            ],
            base_url_env_vars: vec![
                "GLM_BASE_URL".into(),
                "ZAI_BASE_URL".into(),
                "ZHIPUAI_BASE_URL".into(),
                "BIGMODEL_BASE_URL".into(),
            ],
            model_env_vars: vec![
                "GLM_MODEL".into(),
                "ZAI_MODEL".into(),
                "ZHIPUAI_MODEL".into(),
                "BIGMODEL_MODEL".into(),
            ],
            default_base_url: "https://open.bigmodel.cn/api/paas/v4".into(),
            default_model: "glm-5.1".into(),
            supported_models: vec!["glm-5.1".into(), "glm-4.7".into(), "glm-4.6".into()],
            docs_url: Some("https://open.bigmodel.cn".into()),
            setup_hint:
                "Set GLM_API_KEY (or ZHIPUAI_API_KEY) to your API key from open.bigmodel.cn".into(),
        },
        ProviderCatalogEntry {
            id: "kimi".into(),
            label: "Kimi".into(),
            provider_type: ProviderCatalogType::Kimi,
            key_env_vars: vec!["MOONSHOT_API_KEY".into()],
            base_url_env_vars: vec!["MOONSHOT_BASE_URL".into()],
            model_env_vars: vec!["MOONSHOT_MODEL".into()],
            default_base_url: "https://api.moonshot.cn/v1".into(),
            default_model: "kimi-k2.5".into(),
            supported_models: vec!["kimi-k2.5".into(), "kimi-k2.5-thinking".into()],
            docs_url: Some("https://platform.moonshot.cn".into()),
            setup_hint: "Set MOONSHOT_API_KEY to your Kimi API key from platform.moonshot.cn"
                .into(),
        },
        ProviderCatalogEntry {
            id: "openai".into(),
            label: "OpenAI".into(),
            provider_type: ProviderCatalogType::OpenAI,
            key_env_vars: vec!["OPENAI_API_KEY".into()],
            base_url_env_vars: vec!["OPENAI_BASE_URL".into()],
            model_env_vars: vec!["OPENAI_MODEL".into()],
            default_base_url: "https://api.openai.com/v1".into(),
            default_model: "gpt-4o".into(),
            supported_models: vec!["gpt-4o".into(), "gpt-4o-mini".into()],
            docs_url: Some("https://platform.openai.com".into()),
            setup_hint: "Set OPENAI_API_KEY to your API key from platform.openai.com".into(),
        },
    ]
}

/// Find a catalog entry by id.
pub fn find(id: &str) -> Option<ProviderCatalogEntry> {
    builtin_catalog().into_iter().find(|e| e.id == id)
}

/// Check whether a provider is configured (has a non-empty API key).
pub fn is_configured(id: &str) -> bool {
    let entry = match find(id) {
        Some(e) => e,
        None => return false,
    };
    entry.key_env_vars.iter().any(|v| {
        std::env::var(v)
            .map(|val| !val.trim().is_empty())
            .unwrap_or(false)
    })
}

/// List configured provider ids.
pub fn configured_providers() -> Vec<String> {
    builtin_catalog()
        .into_iter()
        .filter(|e| is_configured(&e.id))
        .map(|e| e.id)
        .collect()
}

/// Build a shell-profile export line for a provider's API key env var.
///
/// Returns the line to add to `~/.zshrc` or `~/.bashrc`, e.g.:
/// `export MINIMAX_API_KEY="<your-key>"`
pub fn shell_export_line(id: &str) -> Option<String> {
    let entry = find(id)?;
    let var = entry.key_env_vars.first()?;
    Some(format!("export {}=\"<your-key>\"", var))
}

/// Status DTO for a single provider suitable for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub id: String,
    pub label: String,
    pub configured: bool,
    pub default_model: String,
    pub supported_models: Vec<String>,
    pub setup_hint: String,
    pub export_line: Option<String>,
    pub docs_url: Option<String>,
}

/// Build status DTOs for all built-in providers.
pub fn provider_status_list() -> Vec<ProviderStatus> {
    builtin_catalog()
        .into_iter()
        .map(|e| ProviderStatus {
            id: e.id.clone(),
            label: e.label.clone(),
            configured: is_configured(&e.id),
            default_model: e.default_model.clone(),
            supported_models: e.supported_models.clone(),
            setup_hint: e.setup_hint.clone(),
            export_line: shell_export_line(&e.id),
            docs_url: e.docs_url.clone(),
        })
        .collect()
}

/// Build a status DTO for a single provider.
pub fn provider_status(id: &str) -> Option<ProviderStatus> {
    let entry = find(id)?;
    Some(ProviderStatus {
        id: entry.id.clone(),
        label: entry.label.clone(),
        configured: is_configured(&entry.id),
        default_model: entry.default_model.clone(),
        supported_models: entry.supported_models.clone(),
        setup_hint: entry.setup_hint.clone(),
        export_line: shell_export_line(&entry.id),
        docs_url: entry.docs_url.clone(),
    })
}

/// Get the supported models for a provider id.
pub fn supported_models(id: &str) -> Vec<String> {
    find(id).map(|e| e.supported_models).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_six_entries() {
        assert_eq!(builtin_catalog().len(), 6);
    }

    #[test]
    fn find_returns_entry_for_valid_id() {
        let entry = find("deepseek").expect("deepseek should exist");
        assert_eq!(entry.label, "DeepSeek");
        assert_eq!(entry.default_model, "deepseek-v4-pro");
    }

    #[test]
    fn find_returns_none_for_unknown_id() {
        assert!(find("nonexistent").is_none());
    }

    #[test]
    fn shell_export_line_contains_export_keyword() {
        let line = shell_export_line("deepseek").expect("should have export line");
        assert!(line.starts_with("export "));
        assert!(line.contains("DEEPSEEK_API_KEY"));
    }

    #[test]
    fn provider_status_list_has_all_entries() {
        let list = provider_status_list();
        assert_eq!(list.len(), 6);
        // All entries should have an id and label.
        for s in &list {
            assert!(!s.id.is_empty());
            assert!(!s.label.is_empty());
        }
    }

    #[test]
    fn supported_models_are_non_empty() {
        for entry in builtin_catalog() {
            assert!(
                !entry.supported_models.is_empty(),
                "{} should have models",
                entry.id
            );
        }
    }

    #[test]
    fn each_entry_has_setup_hint() {
        for entry in builtin_catalog() {
            assert!(
                !entry.setup_hint.is_empty(),
                "{} should have a setup hint",
                entry.id
            );
        }
    }
}
