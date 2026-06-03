//! Runtime facade for unified frontend behavior.
//!
//! This module is the migration point for product runtime state that should be
//! shared across frontends. TUI currently mirrors provider lifecycle and stream
//! usage into this state; desktop/headless callers can move onto the same
//! facade without reimplementing provider slow-tail policy.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Provider request lifecycle state shared across frontends.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderRequestLifecycle {
    pub phase: ProviderPhase,
    pub provider_family: Option<String>,
    pub model: Option<String>,
    pub request_shape: Option<String>,
    pub elapsed_ms: u64,
    pub timeout_ms: u64,
    pub slow_warning_threshold_ms: u64,
    pub is_known_slow_path: bool,
    pub slow_warning_emitted: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPhase {
    #[default]
    Idle,
    Started,
    Retrying,
    SlowWarning,
    Completed,
    TimedOut,
    Cancelled,
}

impl ProviderPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Started => "waiting for provider",
            Self::Retrying => "retrying provider",
            Self::SlowWarning => "slow provider",
            Self::Completed => "provider done",
            Self::TimedOut => "provider timeout",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Started | Self::Retrying | Self::SlowWarning)
    }
}

impl ProviderRequestLifecycle {
    pub fn status_label(&self) -> String {
        match self.phase {
            ProviderPhase::Idle => String::new(),
            ProviderPhase::Started => {
                if self.is_known_slow_path {
                    format!(
                        "non-streaming tool request ({})",
                        self.provider_family.as_deref().unwrap_or("unknown")
                    )
                } else {
                    format!(
                        "waiting on {}",
                        self.provider_family.as_deref().unwrap_or("provider")
                    )
                }
            }
            ProviderPhase::Retrying => {
                format!(
                    "retrying {}",
                    self.provider_family.as_deref().unwrap_or("provider")
                )
            }
            ProviderPhase::SlowWarning => {
                format!(
                    "slow {} ({:.1}s)",
                    self.provider_family.as_deref().unwrap_or("provider"),
                    self.elapsed_ms as f64 / 1000.0
                )
            }
            ProviderPhase::Completed => String::new(),
            ProviderPhase::TimedOut => {
                format!(
                    "{} timed out ({:.1}s)",
                    self.provider_family.as_deref().unwrap_or("provider"),
                    self.elapsed_ms as f64 / 1000.0
                )
            }
            ProviderPhase::Cancelled => "cancelled".to_string(),
        }
    }

    pub fn update_from_diagnostic(&mut self, diagnostic: &serde_json::Value) {
        let schema = diagnostic
            .get("schema")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let stage = diagnostic
            .get("stage")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match (schema, stage) {
            ("api_request_stage.v1", "api_request_started")
            | ("provider_request.v1", "provider_request_started") => {
                self.phase = ProviderPhase::Started;
                self.elapsed_ms = 0;
                self.update_metadata(diagnostic, false);
                self.slow_warning_emitted = false;
                self.message = None;
            }
            ("provider_request.v1", "provider_request_retrying") => {
                self.phase = ProviderPhase::Retrying;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
                self.message = diagnostic
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| self.message.clone());
            }
            ("provider_request.v1", "provider_request_slow_warning") => {
                self.phase = ProviderPhase::SlowWarning;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
                self.slow_warning_emitted = true;
                self.message = diagnostic
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            ("provider_request.v1", "provider_request_completed") => {
                self.phase = ProviderPhase::Completed;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
            }
            ("provider_request.v1", "provider_request_timeout") => {
                self.phase = ProviderPhase::TimedOut;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
                self.message = diagnostic
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| self.message.clone());
            }
            ("provider_request.v1", "provider_request_cancelled") => {
                self.phase = ProviderPhase::Cancelled;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
            }
            _ => {}
        }
    }

    pub fn check_slow_warning(&mut self) -> bool {
        if self.phase != ProviderPhase::Started || self.slow_warning_emitted {
            return false;
        }
        if self.elapsed_ms >= self.slow_warning_threshold_ms && self.slow_warning_threshold_ms > 0 {
            self.phase = ProviderPhase::SlowWarning;
            self.slow_warning_emitted = true;
            return true;
        }
        false
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn update_metadata(&mut self, diagnostic: &serde_json::Value, preserve_existing: bool) {
        self.provider_family = string_field(diagnostic, "provider_family").or_else(|| {
            preserve_existing
                .then(|| self.provider_family.clone())
                .flatten()
        });
        self.model = string_field(diagnostic, "model")
            .or_else(|| preserve_existing.then(|| self.model.clone()).flatten());
        self.request_shape = string_field(diagnostic, "request_shape").or_else(|| {
            preserve_existing
                .then(|| self.request_shape.clone())
                .flatten()
        });
        self.timeout_ms = diagnostic
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(if preserve_existing {
                self.timeout_ms
            } else {
                0
            });
        self.slow_warning_threshold_ms = diagnostic
            .get("slow_warning_threshold_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(if preserve_existing {
                self.slow_warning_threshold_ms
            } else {
                0
            });
        self.is_known_slow_path = diagnostic
            .get("is_known_slow_path")
            .and_then(|v| v.as_bool())
            .or_else(|| {
                diagnostic
                    .get("nonstreaming_tool_request")
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or(if preserve_existing {
                self.is_known_slow_path
            } else {
                false
            });
    }

    fn update_elapsed(&mut self, diagnostic: &serde_json::Value) {
        self.elapsed_ms = diagnostic
            .get("elapsed_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.elapsed_ms);
    }
}

fn string_field(diagnostic: &serde_json::Value, key: &str) -> Option<String> {
    diagnostic
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Runtime facade state snapshot for frontends.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeStateSnapshot {
    pub provider_request: ProviderRequestLifecycle,
    pub is_querying: bool,
    pub current_tool_label: Option<String>,
    pub stream_usage: Option<StreamUsageSnapshot>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct StreamUsageSnapshot {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub reasoning_tokens: Option<u32>,
    pub cached_tokens: Option<u32>,
}

impl StreamUsageSnapshot {
    pub fn total_tokens(self) -> u32 {
        self.prompt_tokens + self.completion_tokens
    }

    pub fn cache_miss_tokens(self) -> Option<u32> {
        self.cached_tokens.map(|cached| {
            self.prompt_tokens
                .saturating_sub(cached.min(self.prompt_tokens))
        })
    }

    pub fn cache_hit_rate_percent(self) -> Option<f64> {
        self.cached_tokens.map(|cached| {
            if self.prompt_tokens == 0 {
                0.0
            } else {
                cached.min(self.prompt_tokens) as f64 / self.prompt_tokens as f64 * 100.0
            }
        })
    }
}

/// Shared runtime facade state.
#[derive(Clone)]
pub struct RuntimeFacadeState {
    inner: Arc<Mutex<RuntimeStateSnapshot>>,
}

impl RuntimeFacadeState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RuntimeStateSnapshot::default())),
        }
    }

    pub async fn snapshot(&self) -> RuntimeStateSnapshot {
        self.inner.lock().await.clone()
    }

    pub async fn update_provider_request<F>(&self, updater: F)
    where
        F: FnOnce(&mut ProviderRequestLifecycle),
    {
        let mut state = self.inner.lock().await;
        updater(&mut state.provider_request);
    }

    pub async fn set_querying(&self, querying: bool) {
        let mut state = self.inner.lock().await;
        state.is_querying = querying;
    }

    pub async fn set_tool_label(&self, label: Option<String>) {
        let mut state = self.inner.lock().await;
        state.current_tool_label = label;
    }

    pub async fn set_stream_usage(&self, usage: Option<StreamUsageSnapshot>) {
        let mut state = self.inner.lock().await;
        state.stream_usage = usage;
    }

    pub async fn reset_provider_request(&self) {
        let mut state = self.inner.lock().await;
        state.provider_request.reset();
    }
}

impl Default for RuntimeFacadeState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn provider_lifecycle_tracks_full_diagnostic_sequence() {
        let mut lifecycle = ProviderRequestLifecycle::default();

        lifecycle.update_from_diagnostic(&json!({
            "schema": "api_request_stage.v1",
            "stage": "api_request_started",
            "provider_family": "openai",
            "model": "gpt-test",
            "request_shape": "streaming_tool_request",
            "timeout_ms": 120_000,
            "slow_warning_threshold_ms": 45_000,
            "is_known_slow_path": true
        }));

        assert_eq!(lifecycle.phase, ProviderPhase::Started);
        assert_eq!(lifecycle.provider_family.as_deref(), Some("openai"));
        assert_eq!(lifecycle.model.as_deref(), Some("gpt-test"));
        assert_eq!(
            lifecycle.request_shape.as_deref(),
            Some("streaming_tool_request")
        );
        assert_eq!(lifecycle.timeout_ms, 120_000);
        assert_eq!(lifecycle.slow_warning_threshold_ms, 45_000);
        assert!(lifecycle.is_known_slow_path);

        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_retrying",
            "elapsed_ms": 7_500
        }));
        assert_eq!(lifecycle.phase, ProviderPhase::Retrying);
        assert_eq!(lifecycle.elapsed_ms, 7_500);
        assert_eq!(lifecycle.provider_family.as_deref(), Some("openai"));

        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_slow_warning",
            "elapsed_ms": 46_000,
            "message": "provider is slow"
        }));
        assert_eq!(lifecycle.phase, ProviderPhase::SlowWarning);
        assert_eq!(lifecycle.elapsed_ms, 46_000);
        assert!(lifecycle.slow_warning_emitted);
        assert_eq!(lifecycle.message.as_deref(), Some("provider is slow"));

        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_completed",
            "elapsed_ms": 51_000
        }));
        assert_eq!(lifecycle.phase, ProviderPhase::Completed);
        assert_eq!(lifecycle.elapsed_ms, 51_000);
        assert!(!lifecycle.phase.is_active());
    }

    #[test]
    fn provider_lifecycle_tracks_timeout_and_cancelled_terminal_states() {
        let mut lifecycle = ProviderRequestLifecycle::default();
        lifecycle.update_from_diagnostic(&json!({
            "schema": "api_request_stage.v1",
            "stage": "api_request_started",
            "provider_family": "minimax",
            "nonstreaming_tool_request": true,
            "timeout_ms": 90_000
        }));
        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_timeout",
            "elapsed_ms": 90_001,
            "message": "timeout"
        }));

        assert_eq!(lifecycle.phase, ProviderPhase::TimedOut);
        assert_eq!(lifecycle.elapsed_ms, 90_001);
        assert_eq!(lifecycle.timeout_ms, 90_000);
        assert!(lifecycle.is_known_slow_path);
        assert_eq!(lifecycle.message.as_deref(), Some("timeout"));
        assert!(!lifecycle.phase.is_active());

        lifecycle.update_from_diagnostic(&json!({
            "schema": "api_request_stage.v1",
            "stage": "api_request_started",
            "provider_family": "openai"
        }));
        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_cancelled",
            "elapsed_ms": 125
        }));

        assert_eq!(lifecycle.phase, ProviderPhase::Cancelled);
        assert_eq!(lifecycle.elapsed_ms, 125);
        assert!(!lifecycle.phase.is_active());
    }
}
