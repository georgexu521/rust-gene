//! Provider catalog DTO — versioned provider/model truth.
//!
//! Slice E of the opencode programming parity plan.

use serde::{Deserialize, Serialize};

/// Versioned provider catalog entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCatalogDto {
    pub schema: String,
    pub providers: Vec<ProviderCatalogEntry>,
}

/// Single provider entry in the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCatalogEntry {
    pub provider_id: String,
    pub label: String,
    pub enabled: bool,
    pub source: String,
    pub base_url_host: String,
    pub default_model: String,
    pub available_model_ids: Vec<String>,
    pub context_limit: Option<u64>,
    pub output_limit: Option<u64>,
    pub auto_compact_threshold: Option<u64>,
    pub token_counter: String,
    pub cache_accounting: String,
    pub protocol_family: String,
    pub supports_streaming: bool,
    pub requires_nonstreaming: bool,
    pub tool_schema_transform: String,
    pub prompt_delta: String,
    pub request_timeout_secs: u64,
    pub stream_idle_timeout_secs: u64,
    pub last_health_status: Option<String>,
    pub last_latency_ms: Option<u64>,
    pub recent_timeout_category: Option<String>,
    pub cost_input_per_1m: Option<f64>,
    pub cost_output_per_1m: Option<f64>,
    pub cost_cache_read_per_1m: Option<f64>,
    pub cost_cache_write_per_1m: Option<f64>,
}
