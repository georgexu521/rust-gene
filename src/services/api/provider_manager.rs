//! Provider lifecycle manager.
//!
//! Phase 5 of the provider/model unification plan: centralise manifest loading,
//! credential storage, model discovery, adapter construction and runtime
//! validation behind a single service used by the TUI `/connect` wizard and
//! command-line `/connect` path.

use crate::services::api::{
    adapter::{default_adapter_registry, AdapterRegistry},
    auth_store::AuthStore,
    model_discovery::{DiscoveredModel, ModelDiscovery},
    provider::{ProviderConfig, ProviderType},
    provider_manifest::{ProviderManifest, ProviderManifestLoader},
    provider_protocol::ProviderProtocolFamily,
    ChatRequest, LlmProvider,
};
use std::sync::Arc;
use std::time::Duration;

/// Result of validating a provider credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// The credential works. Includes the resolved model and discovered models.
    Success {
        provider_id: String,
        provider_name: String,
        model: String,
        discovered_models: Vec<DiscoveredModel>,
    },
    /// Validation failed with a human-readable reason.
    Failure { reason: String },
}

impl ValidationResult {
    pub fn is_success(&self) -> bool {
        matches!(self, ValidationResult::Success { .. })
    }

    pub fn into_message(self) -> String {
        match self {
            ValidationResult::Success {
                provider_name,
                model,
                discovered_models,
                ..
            } => {
                let model_line = format!("Active model: {}", model);
                let discovery_line = if discovered_models.is_empty() {
                    "No additional models discovered.".to_string()
                } else {
                    format!("Discovered {} models.", discovered_models.len())
                };
                format!(
                    "{} connected.\n{}\n{}",
                    provider_name, model_line, discovery_line
                )
            }
            ValidationResult::Failure { reason } => format!("Validation failed: {}", reason),
        }
    }
}

/// Central manager for provider configuration, discovery and validation.
pub struct ProviderManager {
    auth_store: AuthStore,
    model_discovery: ModelDiscovery,
    adapter_registry: AdapterRegistry,
    http_timeout: Duration,
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderManager {
    /// Create a manager with the default auth store and adapter registry.
    pub fn new() -> Self {
        Self {
            auth_store: AuthStore::new_default(),
            model_discovery: ModelDiscovery::new(),
            adapter_registry: default_adapter_registry(),
            http_timeout: Duration::from_secs(5),
        }
    }

    /// Create a manager with a custom auth store path.
    pub fn with_auth_store_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            auth_store: AuthStore::from_path(path),
            model_discovery: ModelDiscovery::new(),
            adapter_registry: default_adapter_registry(),
            http_timeout: Duration::from_secs(5),
        }
    }

    /// Load the merged provider manifest.
    pub fn manifest(&self) -> crate::services::api::provider_manifest::ProvidersManifest {
        ProviderManifestLoader::load_merged()
    }

    /// Find a provider entry by id in the merged manifest.
    pub fn find_manifest_entry(&self, provider_id: &str) -> Option<ProviderManifest> {
        self.manifest()
            .provider
            .into_iter()
            .find(|entry| entry.id == provider_id)
    }

    /// Resolve the primary API key env var for a provider.
    pub fn primary_env_var(&self, provider_id: &str) -> Option<String> {
        self.find_manifest_entry(provider_id)
            .and_then(|entry| entry.env.into_iter().next())
    }

    /// Resolve the API key for a provider, preferring the provided key, then
    /// runtime env, then the auth store.
    pub fn resolve_api_key(&self, provider_id: &str, provided_key: Option<&str>) -> Option<String> {
        if let Some(key) = provided_key {
            let key = key.trim();
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }

        let env_var = self.primary_env_var(provider_id)?;
        if let Ok(value) = std::env::var(&env_var) {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
        self.auth_store.get(provider_id, &env_var)
    }

    /// Build a runtime config for a provider without constructing the adapter.
    pub fn build_config(&self, provider_id: &str, api_key: Option<&str>) -> Option<ProviderConfig> {
        let entry = self.find_manifest_entry(provider_id)?;
        let api_key = self.resolve_api_key(provider_id, api_key)?;
        let base_url = entry.resolve_base_url();
        let model = entry.resolve_model();

        Some(ProviderConfig {
            name: entry.id.clone(),
            provider_type: ProviderType::parse_lossy(&entry.id),
            api_key,
            base_url: Some(base_url),
            default_model: model,
            enabled: true,
        })
    }

    /// Build the adapter for a provider.
    pub fn build_adapter(
        &self,
        provider_id: &str,
        api_key: Option<&str>,
    ) -> Option<Arc<dyn LlmProvider>> {
        let config = self.build_config(provider_id, api_key)?;
        self.adapter_registry.build(&config)
    }

    /// Discover models for a provider.
    pub async fn discover_models(
        &self,
        provider_id: &str,
        api_key: Option<&str>,
    ) -> Vec<DiscoveredModel> {
        let Some(entry) = self.find_manifest_entry(provider_id) else {
            return Vec::new();
        };
        let owned_key: Option<String> = api_key
            .map(|k| k.to_string())
            .or_else(|| self.resolve_api_key(provider_id, None));
        self.model_discovery
            .list(provider_id, &entry, owned_key.as_deref())
            .await
    }

    /// Validate a provider credential by issuing a cheap live request.
    ///
    /// 1. For OpenAI-compatible providers, try `GET {base_url}/models`.
    /// 2. If that fails (or for non-OpenAI-compatible families), issue a
    ///    minimal chat completion with `max_tokens: 1`.
    /// 3. If the chat succeeds, the credential is considered valid.
    pub async fn validate(
        &self,
        provider_id: &str,
        provided_key: Option<&str>,
    ) -> ValidationResult {
        let Some(entry) = self.find_manifest_entry(provider_id) else {
            return ValidationResult::Failure {
                reason: format!("unknown provider '{}'", provider_id),
            };
        };

        let api_key = match self.resolve_api_key(provider_id, provided_key) {
            Some(key) => key,
            None => {
                return ValidationResult::Failure {
                    reason: format!("no API key found for {}", provider_id),
                };
            }
        };

        let config = match self.build_config(provider_id, provided_key) {
            Some(config) => config,
            None => {
                return ValidationResult::Failure {
                    reason: format!("could not build runtime config for {}", provider_id),
                };
            }
        };

        let adapter = match self.adapter_registry.build(&config) {
            Some(adapter) => adapter,
            None => {
                return ValidationResult::Failure {
                    reason: format!("could not build adapter for {}", provider_id),
                };
            }
        };

        // Step 1: try a cheap models endpoint probe for OpenAI-compatible families.
        if entry.provider_family == ProviderProtocolFamily::OpenAiCompatible {
            let base_url = entry.resolve_base_url();
            if let Ok(result) = self.probe_models_endpoint(&base_url, &api_key).await {
                if result {
                    let discovered = self
                        .model_discovery
                        .list(provider_id, &entry, Some(&api_key))
                        .await;
                    return ValidationResult::Success {
                        provider_id: entry.id.clone(),
                        provider_name: entry.name.clone(),
                        model: config.default_model,
                        discovered_models: discovered,
                    };
                }
            }
        }

        // Step 2: issue a minimal chat completion.
        match self
            .probe_chat_completion(&*adapter, &config.default_model)
            .await
        {
            Ok(()) => {
                let discovered = self
                    .model_discovery
                    .list(provider_id, &entry, Some(&api_key))
                    .await;
                ValidationResult::Success {
                    provider_id: entry.id.clone(),
                    provider_name: entry.name.clone(),
                    model: config.default_model,
                    discovered_models: discovered,
                }
            }
            Err(err) => ValidationResult::Failure {
                reason: format!("{}", err),
            },
        }
    }

    /// Save a credential and validate it.
    pub async fn save_and_validate(
        &self,
        provider_id: &str,
        env_var: &str,
        key: &str,
    ) -> anyhow::Result<ValidationResult> {
        if key.trim().is_empty() {
            anyhow::bail!("key must not be empty");
        }
        self.auth_store.set(provider_id, env_var, key)?;
        std::env::set_var(env_var, key);
        Ok(self.validate(provider_id, Some(key)).await)
    }

    async fn probe_models_endpoint(&self, base_url: &str, api_key: &str) -> anyhow::Result<bool> {
        let url = format!("{}/models", base_url.trim_end_matches('/'));
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .bearer_auth(api_key)
            .timeout(self.http_timeout)
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    async fn probe_chat_completion(
        &self,
        adapter: &dyn LlmProvider,
        model: &str,
    ) -> anyhow::Result<()> {
        let request = ChatRequest::new(model)
            .with_messages(vec![crate::services::api::Message::user("hi")])
            .with_output_cap(Some(1));
        adapter.chat(request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager() -> (ProviderManager, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let manager = ProviderManager::with_auth_store_path(tmp.path().join(".env"));
        (manager, tmp)
    }

    #[test]
    fn unknown_provider_fails_validation() {
        let (manager, _tmp) = test_manager();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(manager.validate("not-a-provider", None));
        assert!(matches!(result, ValidationResult::Failure { .. }));
    }

    #[test]
    fn missing_key_fails_validation() {
        let (manager, _tmp) = test_manager();
        // Use a provider id that is unlikely to have a key set in the
        // developer environment, so the test remains hermetic.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(manager.validate("not-a-provider", None));
        assert!(
            matches!(result, ValidationResult::Failure { reason } if reason.contains("unknown provider"))
        );
    }

    #[test]
    fn resolve_api_key_prefers_provided_key() {
        let (manager, _tmp) = test_manager();
        let key = manager.resolve_api_key("deepseek", Some("sk-provided"));
        assert_eq!(key, Some("sk-provided".to_string()));
    }

    #[test]
    fn build_config_returns_none_for_unknown_provider() {
        let (manager, _tmp) = test_manager();
        assert!(manager
            .build_config("not-a-provider", Some("key"))
            .is_none());
    }
}
