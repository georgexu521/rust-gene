//! Dynamic model discovery for configured providers.
//!
//! Fetches the list of available models from a provider's `/models` endpoint
//! when the manifest declares `models_source = { type = "openai_compatible" }`,
//! and caches results on disk for offline fallback.

use crate::services::api::provider_manifest::{ModelsSource, ProviderManifest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Discovered model metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredModel {
    pub id: String,
    pub name: Option<String>,
}

impl DiscoveredModel {
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.id.clone())
    }
}

/// Cached discovery result for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCacheEntry {
    pub provider_id: String,
    pub fetched_at: u64,
    pub models: Vec<DiscoveredModel>,
}

/// Dynamic model discovery service.
#[derive(Debug, Clone)]
pub struct ModelDiscovery {
    cache: Arc<Mutex<HashMap<String, ModelCacheEntry>>>,
    cache_dir: PathBuf,
    http_timeout: Duration,
}

impl Default for ModelDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelDiscovery {
    /// Create a discovery service using the default cache directory.
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .map(|d| d.join("priority-agent").join("models"))
            .unwrap_or_else(|| PathBuf::from(".priority-agent").join("models"));
        Self::with_cache_dir(cache_dir)
    }

    /// Create a discovery service with a specific cache directory.
    pub fn with_cache_dir(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_dir: cache_dir.into(),
            http_timeout: Duration::from_secs(5),
        }
    }

    /// Return cached models for a provider, if any.
    pub fn cached(&self, provider_id: &str) -> Option<Vec<DiscoveredModel>> {
        self.cache
            .lock()
            .ok()
            .and_then(|guard| guard.get(provider_id).cloned().map(|entry| entry.models))
    }

    /// List models for a provider, using cache if fresh.
    ///
    /// - `Static` sources return the manifest list immediately.
    /// - `OpenAiCompatible` sources first check the cache, then attempt a live
    ///   fetch. Live failures fall back to the cache/static list.
    pub async fn list(
        &self,
        provider_id: &str,
        manifest: &ProviderManifest,
        api_key: Option<&str>,
    ) -> Vec<DiscoveredModel> {
        match &manifest.models_source {
            ModelsSource::Static { models } => models
                .iter()
                .map(|id| DiscoveredModel {
                    id: id.clone(),
                    name: None,
                })
                .collect(),
            ModelsSource::OpenAiCompatible => {
                let base_url = manifest.resolve_base_url();
                let cache_path = self.cache_file_path(provider_id);
                if let Some(cached) = self.load_cache_file(&cache_path) {
                    if self.is_fresh(cached.fetched_at, 300) {
                        cached.models
                    } else {
                        // Stale cache: try live fetch, but keep stale fallback.
                        let live = self
                            .fetch_openai_compatible_models(&base_url, api_key, &cache_path)
                            .await;
                        if !live.is_empty() {
                            live
                        } else {
                            cached.models
                        }
                    }
                } else {
                    self.fetch_openai_compatible_models(&base_url, api_key, &cache_path)
                        .await
                }
            }
            ModelsSource::Dynamic { list_url } => {
                let cache_path = self.cache_file_path(provider_id);
                let live = self
                    .fetch_dynamic_models(list_url, api_key, &cache_path)
                    .await;
                if !live.is_empty() {
                    live
                } else {
                    self.load_cache_file(&cache_path)
                        .map(|entry| entry.models)
                        .unwrap_or_default()
                }
            }
        }
    }

    /// Force a live refresh of the model list.
    pub async fn refresh(
        &self,
        provider_id: &str,
        manifest: &ProviderManifest,
        api_key: Option<&str>,
    ) -> Vec<DiscoveredModel> {
        match &manifest.models_source {
            ModelsSource::Static { models } => models
                .iter()
                .map(|id| DiscoveredModel {
                    id: id.clone(),
                    name: None,
                })
                .collect(),
            ModelsSource::OpenAiCompatible => {
                let cache_path = self.cache_file_path(provider_id);
                self.fetch_openai_compatible_models(
                    &manifest.resolve_base_url(),
                    api_key,
                    &cache_path,
                )
                .await
            }
            ModelsSource::Dynamic { list_url } => {
                let cache_path = self.cache_file_path(provider_id);
                self.fetch_dynamic_models(list_url, api_key, &cache_path)
                    .await
            }
        }
    }

    fn cache_file_path(&self, provider_id: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.json", provider_id))
    }

    fn is_fresh(&self, fetched_at: u64, max_age_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(fetched_at) <= max_age_secs
    }

    fn load_cache_file(&self, path: &Path) -> Option<ModelCacheEntry> {
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save_cache_file(&self, path: &Path, entry: &ModelCacheEntry) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(entry) {
            let _ = std::fs::write(path, content);
        }
    }

    async fn fetch_openai_compatible_models(
        &self,
        base_url: &str,
        api_key: Option<&str>,
        cache_path: &Path,
    ) -> Vec<DiscoveredModel> {
        let url = format!("{}/models", base_url.trim_end_matches('/'));
        let client = reqwest::Client::new();
        let mut req = client.get(&url).timeout(self.http_timeout);
        if let Some(key) = api_key {
            req = req.bearer_auth(key);
        }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(err) => {
                tracing::debug!("Failed to fetch models from {}: {}", url, err);
                return Vec::new();
            }
        };
        if !resp.status().is_success() {
            tracing::debug!("Models endpoint returned status {}", resp.status());
            return Vec::new();
        }
        let body = match resp.text().await {
            Ok(b) => b,
            Err(err) => {
                tracing::debug!("Failed to read models response: {}", err);
                return Vec::new();
            }
        };
        self.parse_openai_compatible_models(&body, cache_path)
    }

    fn parse_openai_compatible_models(
        &self,
        body: &str,
        cache_path: &Path,
    ) -> Vec<DiscoveredModel> {
        let provider_id = cache_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(err) => {
                tracing::debug!("Failed to parse models response: {}", err);
                return Vec::new();
            }
        };
        let models: Vec<DiscoveredModel> = parsed
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let id = item.get("id")?.as_str()?;
                        Some(DiscoveredModel {
                            id: id.to_string(),
                            name: item.get("name").and_then(|v| v.as_str()).map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        if !models.is_empty() {
            let fetched_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let entry = ModelCacheEntry {
                provider_id,
                fetched_at,
                models: models.clone(),
            };
            self.save_cache_file(cache_path, &entry);
            if let Ok(mut guard) = self.cache.lock() {
                guard.insert(entry.provider_id.clone(), entry);
            }
        }

        models
    }

    async fn fetch_dynamic_models(
        &self,
        list_url: &str,
        api_key: Option<&str>,
        cache_path: &Path,
    ) -> Vec<DiscoveredModel> {
        let client = reqwest::Client::new();
        let mut req = client.get(list_url).timeout(self.http_timeout);
        if let Some(key) = api_key {
            req = req.bearer_auth(key);
        }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(err) => {
                tracing::debug!("Failed to fetch models from {}: {}", list_url, err);
                return Vec::new();
            }
        };
        if !resp.status().is_success() {
            tracing::debug!("Dynamic models endpoint returned status {}", resp.status());
            return Vec::new();
        }
        let body = match resp.text().await {
            Ok(b) => b,
            Err(err) => {
                tracing::debug!("Failed to read dynamic models response: {}", err);
                return Vec::new();
            }
        };
        self.parse_dynamic_models(&body, cache_path)
    }

    fn parse_dynamic_models(&self, body: &str, cache_path: &Path) -> Vec<DiscoveredModel> {
        let provider_id = cache_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(err) => {
                tracing::debug!("Failed to parse dynamic models response: {}", err);
                return Vec::new();
            }
        };
        // Best-effort: accept an array of strings or objects with `id`.
        let models: Vec<DiscoveredModel> = if let Some(arr) = parsed.as_array() {
            arr.iter()
                .filter_map(|item| {
                    if let Some(id) = item.as_str() {
                        Some(DiscoveredModel {
                            id: id.to_string(),
                            name: None,
                        })
                    } else {
                        let id = item.get("id")?.as_str()?;
                        Some(DiscoveredModel {
                            id: id.to_string(),
                            name: item.get("name").and_then(|v| v.as_str()).map(String::from),
                        })
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        if !models.is_empty() {
            let fetched_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let entry = ModelCacheEntry {
                provider_id,
                fetched_at,
                models: models.clone(),
            };
            self.save_cache_file(cache_path, &entry);
            if let Ok(mut guard) = self.cache.lock() {
                guard.insert(entry.provider_id.clone(), entry);
            }
        }

        models
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::provider_manifest::ModelsSource;

    fn sample_manifest(models_source: ModelsSource) -> ProviderManifest {
        ProviderManifest {
            id: "test".to_string(),
            name: "Test".to_string(),
            provider_family:
                crate::services::api::provider_protocol::ProviderProtocolFamily::OpenAiCompatible,
            env: vec!["TEST_API_KEY".to_string()],
            base_url_env: Vec::new(),
            model_env: Vec::new(),
            base_url: "https://api.test.com".to_string(),
            default_model: "test-model".to_string(),
            models_source,
            capabilities: None,
            docs_url: None,
            setup_hint: String::new(),
        }
    }

    #[test]
    fn static_source_returns_manifest_models() {
        let discovery = ModelDiscovery::new();
        let manifest = sample_manifest(ModelsSource::static_models(vec![
            "m1".to_string(),
            "m2".to_string(),
        ]));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let models = rt.block_on(discovery.list("test", &manifest, None));
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "m1");
    }

    #[test]
    fn parses_openai_compatible_response() {
        let discovery = ModelDiscovery::with_cache_dir(tempfile::tempdir().unwrap().path());
        let body = r#"{"data":[{"id":"gpt-4o","name":"GPT-4o"},{"id":"gpt-4o-mini"}]}"#;
        let cache_path = discovery.cache_file_path("openai");
        let models = discovery.parse_openai_compatible_models(body, &cache_path);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-4o");
        assert_eq!(models[0].name.as_deref(), Some("GPT-4o"));
        // Cache file should be written.
        assert!(cache_path.exists());
    }

    #[test]
    fn parses_dynamic_array_response() {
        let discovery = ModelDiscovery::with_cache_dir(tempfile::tempdir().unwrap().path());
        let body = r#"["model-a","model-b"]"#;
        let cache_path = discovery.cache_file_path("custom");
        let models = discovery.parse_dynamic_models(body, &cache_path);
        assert_eq!(models.len(), 2);
        assert_eq!(models[1].id, "model-b");
    }

    #[test]
    fn empty_response_returns_empty_without_crashing() {
        let discovery = ModelDiscovery::with_cache_dir(tempfile::tempdir().unwrap().path());
        let body = r#"{}"#;
        let cache_path = discovery.cache_file_path("empty");
        let models = discovery.parse_openai_compatible_models(body, &cache_path);
        assert!(models.is_empty());
        assert!(!cache_path.exists());
    }
}
