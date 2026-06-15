//! Adapter registry for provider-specific LLM clients.
//!
//! Phase 2 of the provider/model unification plan: replace the hard-coded
//! `create_provider` match with a registry that maps protocol families to
//! adapter factories.

use crate::services::api::{
    provider::ProviderConfig, provider_protocol::ProviderProtocolFamily, LlmProvider,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Alias for a constructed provider client.
pub type ProviderAdapterInstance = Arc<dyn LlmProvider>;

/// Factory that builds a provider adapter from a runtime config.
pub type AdapterFactory =
    Arc<dyn Fn(&ProviderConfig) -> Option<ProviderAdapterInstance> + Send + Sync>;

/// Registry mapping protocol families to adapter factories.
#[derive(Default)]
pub struct AdapterRegistry {
    factories: HashMap<ProviderProtocolFamily, AdapterFactory>,
}

impl AdapterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a factory for a protocol family.
    pub fn register(
        &mut self,
        family: ProviderProtocolFamily,
        factory: impl Fn(&ProviderConfig) -> Option<ProviderAdapterInstance> + Send + Sync + 'static,
    ) {
        self.factories.insert(family, Arc::new(factory));
    }

    /// Build an adapter for the given config, falling back to OpenAI-compatible.
    pub fn build(&self, config: &ProviderConfig) -> Option<ProviderAdapterInstance> {
        let family = config.provider_type.protocol_family();
        if let Some(factory) = self.factories.get(&family) {
            if let Some(adapter) = factory(config) {
                return Some(adapter);
            }
        }
        self.factories
            .get(&ProviderProtocolFamily::OpenAiCompatible)
            .and_then(|factory| factory(config))
    }

    /// Build an adapter for a specific protocol family.
    pub fn build_for_family(
        &self,
        family: ProviderProtocolFamily,
        config: &ProviderConfig,
    ) -> Option<ProviderAdapterInstance> {
        self.factories
            .get(&family)
            .and_then(|factory| factory(config))
    }
}

/// Default registry used by the runtime.
pub fn default_adapter_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();

    registry.register(ProviderProtocolFamily::OpenAiCompatible, |config| {
        Some(Arc::new(
            crate::services::api::openai::OpenAiClient::new_with_label(
                &config.name,
                &config.api_key,
                config.base_url.as_deref(),
                Some(&config.default_model),
            ),
        ))
    });

    registry.register(ProviderProtocolFamily::MiniMax, |config| {
        Some(Arc::new(crate::services::api::minimax::MiniMaxClient::new(
            &config.api_key,
            config.base_url.as_deref(),
            Some(&config.default_model),
        )))
    });

    registry.register(ProviderProtocolFamily::Kimi, |config| {
        let kimi_config = crate::services::api::kimi::KimiConfig {
            api_key: config.api_key.clone(),
            base_url: config.base_url.clone().unwrap_or_else(|| {
                crate::services::api::provider::KIMI_DEFAULT_BASE_URL.to_string()
            }),
            default_model: config.default_model.clone(),
            thinking_enabled: true,
            thinking_budget: None,
        };
        Some(Arc::new(crate::services::api::kimi::KimiClient::new(
            kimi_config,
        )))
    });

    registry
}

/// Thin wrapper so that `ProviderAdapter` can be implemented for existing clients
/// without renaming the trait used elsewhere.
pub use crate::services::api::LlmProvider as ProviderAdapter;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::provider::ProviderType;

    fn fake_config(provider_type: ProviderType) -> ProviderConfig {
        ProviderConfig {
            name: "test".to_string(),
            provider_type,
            api_key: "fake-key".to_string(),
            base_url: Some("https://api.example.com".to_string()),
            default_model: "fake-model".to_string(),
            enabled: true,
        }
    }

    #[test]
    fn default_registry_builds_openai_compatible_adapter() {
        let registry = default_adapter_registry();
        let adapter = registry.build(&fake_config(ProviderType::OpenAI));
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().base_url(), "https://api.example.com");
    }

    #[test]
    fn default_registry_builds_minimax_adapter() {
        let registry = default_adapter_registry();
        let adapter = registry.build(&fake_config(ProviderType::Minimax));
        assert!(adapter.is_some());
    }

    #[test]
    fn default_registry_builds_kimi_adapter() {
        let registry = default_adapter_registry();
        let adapter = registry.build(&fake_config(ProviderType::Kimi));
        assert!(adapter.is_some());
    }

    #[test]
    fn unknown_provider_falls_back_to_openai_compatible() {
        let registry = default_adapter_registry();
        let adapter = registry.build(&fake_config(ProviderType::Custom));
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().base_url(), "https://api.example.com");
    }
}
