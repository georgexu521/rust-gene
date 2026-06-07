//! Provider DTOs — shared types for provider/model status.

use serde::{Deserialize, Serialize};

/// Product-facing provider status DTO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProductStatus {
    pub provider_id: String,
    pub label: String,
    pub model_id: String,
    pub model_display_name: String,
    pub connection_source: String,
    pub configured: bool,
    pub active: bool,
    pub disabled: bool,
    pub base_url_host: String,
    pub protocol_family: String,
    pub supports_streaming_tool_calls: bool,
    pub requires_nonstreaming: bool,
    pub context_limit: Option<u64>,
    pub output_limit: Option<u64>,
    pub configured_max_output: Option<u64>,
    pub cost_input_per_1m: Option<f64>,
    pub cost_output_per_1m: Option<f64>,
    pub cost_cache_read_per_1m: Option<f64>,
    pub latest_health_status: Option<String>,
    pub latest_timeout_category: Option<String>,
    pub last_request_latency_ms: Option<u64>,
    pub last_retry_count: Option<u32>,
    pub request_timeout_secs: u64,
    pub stream_idle_timeout_secs: u64,
    pub capability_summary: String,
}

/// Effective timeout configuration with source tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTimeoutEffectiveDto {
    pub request_secs: u64,
    pub stream_idle_secs: u64,
    pub slow_warning_secs: u64,
    pub max_retry_attempts: u32,
    pub source: String,
}

impl ProviderTimeoutEffectiveDto {
    pub fn from_env() -> Self {
        let source = if std::env::var("PRIORITY_AGENT_REQUEST_TIMEOUT_SECS").is_ok()
            || std::env::var("PRIORITY_AGENT_STREAM_IDLE_TIMEOUT_SECS").is_ok()
        {
            "env"
        } else {
            "default"
        };
        Self {
            request_secs: std::env::var("PRIORITY_AGENT_REQUEST_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(180),
            stream_idle_secs: std::env::var("PRIORITY_AGENT_STREAM_IDLE_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            slow_warning_secs: std::env::var("PRIORITY_AGENT_SLOW_WARNING_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(45),
            max_retry_attempts: std::env::var("PRIORITY_AGENT_MAX_RETRY_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            source: source.to_string(),
        }
    }
}

/// Provider status page response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatusPage {
    pub statuses: Vec<ProviderProductStatus>,
    pub record_count: usize,
}

/// Effective timeout configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTimeoutConfig {
    pub request_secs: u64,
    pub stream_idle_secs: u64,
    pub slow_warning_secs: u64,
    pub max_retry_attempts: u32,
    pub source: String,
}

impl Default for ProviderTimeoutConfig {
    fn default() -> Self {
        Self {
            request_secs: 180,
            stream_idle_secs: 60,
            slow_warning_secs: 45,
            max_retry_attempts: 3,
            source: "default".to_string(),
        }
    }
}
