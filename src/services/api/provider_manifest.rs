//! External provider manifest loader.
//!
//! This module defines the schema for `providers.toml` and the loader that
//! merges built-in, user, project, and environment-variable-configured
//! provider manifests into a single registry-ready view.

use crate::services::api::provider_protocol::{ProviderCapabilities, ProviderProtocolFamily};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Manifest for a single provider.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProviderManifest {
    /// Machine-readable id (e.g. "deepseek").
    pub id: String,
    /// Human-readable name (e.g. "DeepSeek").
    pub name: String,
    /// Protocol family used to pick an adapter.
    #[serde(with = "provider_family_serde")]
    pub provider_family: ProviderProtocolFamily,
    /// Environment variable names that may hold the API key.
    #[serde(default)]
    pub env: Vec<String>,
    /// Environment variable names that may override the base URL.
    #[serde(default)]
    pub base_url_env: Vec<String>,
    /// Environment variable names that may override the default model.
    #[serde(default)]
    pub model_env: Vec<String>,
    /// Default base URL when none is configured.
    pub base_url: String,
    /// Default model when none is configured.
    pub default_model: String,
    /// How to obtain the list of available models.
    #[serde(default)]
    pub models_source: ModelsSource,
    /// Capability overrides. If omitted, inferred from `provider_family`.
    #[serde(default)]
    pub capabilities: Option<ProviderCapabilitiesSpec>,
    /// Optional link to provider docs or API console.
    #[serde(default)]
    pub docs_url: Option<String>,
    /// One-line setup hint for onboarding.
    #[serde(default)]
    pub setup_hint: String,
}

/// How the list of supported models is obtained for a provider.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelsSource {
    /// Fixed list of model ids.
    Static { models: Vec<String> },
    /// Fetch from an OpenAI-compatible `/models` endpoint.
    #[default]
    OpenAiCompatible,
    /// Fetch from a custom URL.
    Dynamic { list_url: String },
}

impl ModelsSource {
    pub fn static_models(models: Vec<String>) -> Self {
        Self::Static { models }
    }

    /// Return the static models if this source is static, else empty.
    pub fn as_static_models(&self) -> Vec<String> {
        match self {
            Self::Static { models } => models.clone(),
            _ => Vec::new(),
        }
    }
}

/// Capability overrides declared in the manifest.
///
/// Unset fields fall back to the defaults for the provider family.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ProviderCapabilitiesSpec {
    #[serde(default)]
    pub supports_streaming_tool_calls: Option<bool>,
    #[serde(default)]
    pub supports_streaming_usage: Option<bool>,
    #[serde(default)]
    pub supports_reasoning_tokens: Option<bool>,
    #[serde(default)]
    pub requires_nonstreaming_tool_calls: Option<bool>,
    #[serde(default)]
    pub requires_merged_system_messages: Option<bool>,
    #[serde(default)]
    pub requires_tool_result_adjacency: Option<bool>,
}

impl ProviderCapabilitiesSpec {
    /// Merge overrides on top of the family defaults.
    pub fn to_capabilities(&self, family: ProviderProtocolFamily) -> ProviderCapabilities {
        let mut caps = ProviderCapabilities::for_family(family);
        if let Some(v) = self.supports_streaming_tool_calls {
            caps.supports_streaming_tool_calls = v;
        }
        if let Some(v) = self.supports_streaming_usage {
            caps.supports_streaming_usage = v;
        }
        if let Some(v) = self.supports_reasoning_tokens {
            caps.supports_reasoning_tokens = v;
        }
        if let Some(v) = self.requires_nonstreaming_tool_calls {
            caps.requires_nonstreaming_tool_calls = v;
        }
        if let Some(v) = self.requires_merged_system_messages {
            caps.requires_merged_system_messages = v;
        }
        if let Some(v) = self.requires_tool_result_adjacency {
            caps.requires_tool_result_adjacency = v;
        }
        caps
    }
}

/// A complete providers manifest file.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct ProvidersManifest {
    #[serde(default)]
    pub provider: Vec<ProviderManifest>,
}

impl ProvidersManifest {
    /// Load the built-in manifest embedded in the binary.
    pub fn builtin() -> Self {
        static BUILTIN: &str = include_str!("../../../resources/providers.toml");
        Self::from_toml(BUILTIN).expect("built-in providers.toml is valid")
    }

    /// Parse manifest content from TOML.
    pub fn from_toml(content: &str) -> anyhow::Result<Self> {
        let manifest: ProvidersManifest = toml::from_str(content)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate invariants (unique ids, non-empty names, etc.).
    fn validate(&self) -> anyhow::Result<()> {
        let mut seen = HashMap::new();
        for entry in &self.provider {
            if entry.id.is_empty() {
                anyhow::bail!("provider id must not be empty");
            }
            if entry.name.is_empty() {
                anyhow::bail!("provider name must not be empty: {}", entry.id);
            }
            if let Some(dup) = seen.insert(entry.id.clone(), entry.name.clone()) {
                anyhow::bail!(
                    "duplicate provider id '{}' (previous name: {})",
                    entry.id,
                    dup
                );
            }
        }
        Ok(())
    }

    /// Convert to a map keyed by provider id.
    pub fn into_map(self) -> HashMap<String, ProviderManifest> {
        self.provider
            .into_iter()
            .map(|entry| (entry.id.clone(), entry))
            .collect()
    }
}

impl ProviderManifest {
    /// Resolve the first non-empty API key from `env`.
    pub fn resolve_api_key(&self) -> Option<String> {
        self.env.iter().find_map(|key| env_non_empty(key))
    }

    /// Resolve base URL, preferring environment overrides.
    pub fn resolve_base_url(&self) -> String {
        env_first_non_empty(&self.base_url_env).unwrap_or_else(|| self.base_url.clone())
    }

    /// Resolve default model, preferring environment overrides.
    pub fn resolve_model(&self) -> String {
        env_first_non_empty(&self.model_env).unwrap_or_else(|| self.default_model.clone())
    }

    /// Return static supported models, if any.
    pub fn supported_models(&self) -> Vec<String> {
        self.models_source.as_static_models()
    }

    /// Final capabilities, merging manifest overrides with family defaults.
    pub fn resolved_capabilities(&self) -> ProviderCapabilities {
        self.capabilities
            .map(|spec| spec.to_capabilities(self.provider_family))
            .unwrap_or_else(|| ProviderCapabilities::for_family(self.provider_family))
    }
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_first_non_empty(keys: &[String]) -> Option<String> {
    keys.iter().find_map(|key| env_non_empty(key))
}

/// Loader that discovers and merges provider manifests from multiple sources.
pub struct ProviderManifestLoader;

impl ProviderManifestLoader {
    /// Resolve the providers config file path.
    ///
    /// Priority:
    /// 1. `PRIORITY_AGENT_PROVIDERS_CONFIG`
    /// 2. `./.priority-agent/providers.toml`
    /// 3. `~/.config/priority-agent/providers.toml`
    pub fn resolve_path() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("PRIORITY_AGENT_PROVIDERS_CONFIG") {
            let path = PathBuf::from(path);
            if path.is_file() {
                return Some(path);
            }
        }

        let project = PathBuf::from(".priority-agent").join("providers.toml");
        if project.is_file() {
            return Some(project);
        }

        dirs::config_dir()
            .map(|d| d.join("priority-agent").join("providers.toml"))
            .filter(|p| p.is_file())
    }

    /// Load the merged manifest: built-in providers first, then user/project overrides.
    pub fn load_merged() -> ProvidersManifest {
        let mut manifest = ProvidersManifest::builtin();

        if let Some(path) = Self::resolve_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                match ProvidersManifest::from_toml(&content) {
                    Ok(user) => {
                        let mut by_id = manifest.into_map();
                        for entry in user.provider {
                            by_id.insert(entry.id.clone(), entry);
                        }
                        manifest = ProvidersManifest {
                            provider: by_id.into_values().collect(),
                        };
                        manifest.provider.sort_by(|a, b| a.id.cmp(&b.id));
                    }
                    Err(err) => {
                        tracing::warn!(
                            "Failed to parse providers config {}: {}",
                            path.display(),
                            err
                        );
                    }
                }
            }
        }

        manifest
    }

    /// Load a manifest from a specific path (for tests).
    pub fn load_from(path: &Path) -> anyhow::Result<ProvidersManifest> {
        let content = std::fs::read_to_string(path)?;
        ProvidersManifest::from_toml(&content)
    }
}

mod provider_family_serde {
    use crate::services::api::provider_protocol::ProviderProtocolFamily;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ProviderProtocolFamily, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "openai_compatible" => Ok(ProviderProtocolFamily::OpenAiCompatible),
            "minimax" => Ok(ProviderProtocolFamily::MiniMax),
            "kimi" => Ok(ProviderProtocolFamily::Kimi),
            "anthropic_like" => Ok(ProviderProtocolFamily::AnthropicLike),
            "reasoning_capable" => Ok(ProviderProtocolFamily::ReasoningCapable),
            _ => Err(serde::de::Error::custom(format!(
                "unknown provider family: {}",
                s
            ))),
        }
    }

    pub fn serialize<S>(family: &ProviderProtocolFamily, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str((*family).label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_manifest_loads() {
        let manifest = ProvidersManifest::builtin();
        assert!(!manifest.provider.is_empty());
        let ids: Vec<_> = manifest.provider.iter().map(|p| p.id.clone()).collect();
        assert!(ids.contains(&"deepseek".to_string()));
        assert!(ids.contains(&"openai".to_string()));
    }

    #[test]
    fn static_models_source_round_trips() {
        let src = ModelsSource::static_models(vec!["a".into(), "b".into()]);
        let toml = toml::to_string(&src).unwrap();
        let parsed: ModelsSource = toml::from_str(&toml).unwrap();
        assert_eq!(src, parsed);
    }

    #[test]
    fn capabilities_override_applies() {
        let spec = ProviderCapabilitiesSpec {
            supports_streaming_tool_calls: Some(false),
            requires_nonstreaming_tool_calls: Some(true),
            ..Default::default()
        };
        let caps = spec.to_capabilities(ProviderProtocolFamily::OpenAiCompatible);
        assert!(!caps.supports_streaming_tool_calls);
        assert!(caps.requires_nonstreaming_tool_calls);
        // Unset fields keep family defaults.
        assert!(caps.supports_streaming_usage);
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let content = r#"
[[provider]]
id = "dup"
name = "Dup"
provider_family = "openai_compatible"
base_url = "https://example.com"
default_model = "m"

[[provider]]
id = "dup"
name = "Dup2"
provider_family = "openai_compatible"
base_url = "https://example.com"
default_model = "m"
"#;
        assert!(ProvidersManifest::from_toml(content).is_err());
    }
}
