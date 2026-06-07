use super::dto;
use super::ApiState;
use crate::diagnostics::provider_health::{ProviderHealthLedgerEntry, ProviderHealthStatus};
use crate::services::api::provider::{ProviderConfig, ProviderEnvSpec, DEFAULT_PROVIDER_ENV_SPECS};
use crate::services::api::provider_protocol::{ProviderCapabilities, ProviderRuntimeProfile};
use std::collections::HashSet;

pub(crate) fn provider_product_statuses(
    state: &ApiState,
    configured_max_output: Option<u64>,
    latest_health: Option<&ProviderHealthLedgerEntry>,
) -> Vec<dto::provider::ProviderProductStatus> {
    let registry = crate::services::api::provider::ProviderRegistry::from_env();
    let selected = registry.selected().map(str::to_string);
    let configs = registry.list_configs();
    let configured_ids: HashSet<&str> = configs.iter().map(|config| config.name.as_str()).collect();
    let mut statuses = Vec::new();

    if configs.is_empty() {
        statuses.push(status_from_current_api_provider(
            state,
            configured_max_output,
            latest_health,
        ));
    } else {
        statuses.extend(configs.iter().map(|config| {
            status_from_config(
                config,
                provider_label(&config.name),
                selected.as_deref() == Some(config.name.as_str()),
                configured_max_output,
                latest_health,
            )
        }));
    }

    for spec in DEFAULT_PROVIDER_ENV_SPECS {
        if configured_ids.contains(spec.id) {
            continue;
        }
        statuses.push(status_from_spec(spec, configured_max_output, latest_health));
    }

    statuses
}

fn status_from_current_api_provider(
    state: &ApiState,
    configured_max_output: Option<u64>,
    latest_health: Option<&ProviderHealthLedgerEntry>,
) -> dto::provider::ProviderProductStatus {
    let base_url = state.provider.base_url().to_string();
    let capabilities = ProviderCapabilities::detect(&base_url, &state.model);
    let profile = ProviderRuntimeProfile::snapshot(&capabilities, &state.model, "api-current");
    let context =
        crate::engine::model_context::ModelContextProfile::detect(&base_url, &state.model);
    let (latest_health_status, latest_timeout_category, last_request_latency_ms) =
        latest_provider_health_fields(latest_health, &state.model, &base_url);
    dto::provider::ProviderProductStatus {
        provider_id: "api-current".to_string(),
        label: "Current API provider".to_string(),
        model_id: state.model.clone(),
        model_display_name: state.model.clone(),
        connection_source: "runtime".to_string(),
        configured: true,
        active: true,
        disabled: false,
        base_url_host: base_url_host(&base_url),
        protocol_family: profile.protocol_family.label().to_string(),
        supports_streaming_tool_calls: profile.supports_streaming_tool_calls,
        requires_nonstreaming: profile.requires_nonstreaming_tool_calls,
        context_limit: Some(context.context_window_tokens),
        output_limit: Some(context.reserved_output_tokens),
        configured_max_output,
        cost_input_per_1m: None,
        cost_output_per_1m: None,
        cost_cache_read_per_1m: None,
        latest_health_status,
        latest_timeout_category,
        last_request_latency_ms,
        last_retry_count: None,
        request_timeout_secs: profile.request_timeout_secs,
        stream_idle_timeout_secs: profile.stream_idle_timeout_secs,
        capability_summary: profile.capability_summary,
    }
}

fn status_from_config(
    config: &ProviderConfig,
    label: String,
    active: bool,
    configured_max_output: Option<u64>,
    latest_health: Option<&ProviderHealthLedgerEntry>,
) -> dto::provider::ProviderProductStatus {
    let base_url = config.base_url.clone().unwrap_or_default();
    let capabilities = config.capabilities();
    let profile =
        ProviderRuntimeProfile::snapshot(&capabilities, &config.default_model, &config.name);
    let context =
        crate::engine::model_context::ModelContextProfile::detect(&base_url, &config.default_model);
    let (latest_health_status, latest_timeout_category, last_request_latency_ms) =
        latest_provider_health_fields(latest_health, &config.default_model, &base_url);
    dto::provider::ProviderProductStatus {
        provider_id: config.name.clone(),
        label,
        model_id: config.default_model.clone(),
        model_display_name: config.default_model.clone(),
        connection_source: "env".to_string(),
        configured: true,
        active,
        disabled: !config.enabled,
        base_url_host: base_url_host(&base_url),
        protocol_family: profile.protocol_family.label().to_string(),
        supports_streaming_tool_calls: profile.supports_streaming_tool_calls,
        requires_nonstreaming: profile.requires_nonstreaming_tool_calls,
        context_limit: Some(context.context_window_tokens),
        output_limit: Some(context.reserved_output_tokens),
        configured_max_output,
        cost_input_per_1m: None,
        cost_output_per_1m: None,
        cost_cache_read_per_1m: None,
        latest_health_status,
        latest_timeout_category,
        last_request_latency_ms,
        last_retry_count: None,
        request_timeout_secs: profile.request_timeout_secs,
        stream_idle_timeout_secs: profile.stream_idle_timeout_secs,
        capability_summary: profile.capability_summary,
    }
}

fn status_from_spec(
    spec: &ProviderEnvSpec,
    configured_max_output: Option<u64>,
    latest_health: Option<&ProviderHealthLedgerEntry>,
) -> dto::provider::ProviderProductStatus {
    let capabilities = spec.provider_type.capabilities();
    let profile = ProviderRuntimeProfile::snapshot(&capabilities, spec.default_model, spec.id);
    let context = crate::engine::model_context::ModelContextProfile::detect(
        spec.default_base_url,
        spec.default_model,
    );
    let (latest_health_status, latest_timeout_category, last_request_latency_ms) =
        latest_provider_health_fields(latest_health, spec.default_model, spec.default_base_url);
    dto::provider::ProviderProductStatus {
        provider_id: spec.id.to_string(),
        label: spec.label.to_string(),
        model_id: spec.default_model.to_string(),
        model_display_name: spec.default_model.to_string(),
        connection_source: spec.primary_key_env().to_string(),
        configured: false,
        active: false,
        disabled: false,
        base_url_host: base_url_host(spec.default_base_url),
        protocol_family: profile.protocol_family.label().to_string(),
        supports_streaming_tool_calls: profile.supports_streaming_tool_calls,
        requires_nonstreaming: profile.requires_nonstreaming_tool_calls,
        context_limit: Some(context.context_window_tokens),
        output_limit: Some(context.reserved_output_tokens),
        configured_max_output,
        cost_input_per_1m: None,
        cost_output_per_1m: None,
        cost_cache_read_per_1m: None,
        latest_health_status,
        latest_timeout_category,
        last_request_latency_ms,
        last_retry_count: None,
        request_timeout_secs: profile.request_timeout_secs,
        stream_idle_timeout_secs: profile.stream_idle_timeout_secs,
        capability_summary: profile.capability_summary,
    }
}

fn provider_label(id: &str) -> String {
    DEFAULT_PROVIDER_ENV_SPECS
        .iter()
        .find(|spec| spec.id == id)
        .map(|spec| spec.label.to_string())
        .unwrap_or_else(|| id.to_string())
}

fn latest_provider_health_fields(
    latest_health: Option<&ProviderHealthLedgerEntry>,
    model: &str,
    base_url: &str,
) -> (Option<String>, Option<String>, Option<u64>) {
    let Some(entry) = latest_health else {
        return (None, None, None);
    };
    if entry.report.model != model || entry.report.base_url != base_url {
        return (None, None, None);
    }
    let status = match entry.report.status {
        ProviderHealthStatus::Ok => "ok",
        ProviderHealthStatus::Failed => "failed",
    }
    .to_string();
    let category = entry
        .report
        .steps
        .iter()
        .find(|step| step.status == ProviderHealthStatus::Failed)
        .and_then(|step| step.error_category.clone());
    let latency = u64::try_from(entry.report.duration_ms).ok();
    (Some(status), category, latency)
}

fn base_url_host(base_url: &str) -> String {
    let without_scheme = base_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(base_url);
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    let host = authority.rsplit('@').next().unwrap_or(authority);
    host.split(':').next().unwrap_or(host).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::routes::create_routes;
    use crate::services::api::{
        ChatRequest as LlmChatRequest, ChatResponse as LlmChatResponse, LlmProvider, Usage,
    };
    use async_openai::types::ChatCompletionResponseStream;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use once_cell::sync::Lazy;
    use std::sync::Arc;
    use std::time::Instant;
    use tower::util::ServiceExt;

    static ENV_LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));
    const TEST_BRIDGE_TOKEN: &str = "test-bridge-token";

    struct MockProvider;

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: LlmChatRequest) -> anyhow::Result<LlmChatResponse> {
            Ok(LlmChatResponse {
                content: "ok".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
                tool_call_repair: None,
            })
        }

        async fn chat_stream(
            &self,
            _request: LlmChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("not implemented in test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    #[test]
    fn test_base_url_host_sanitizes_common_provider_urls() {
        assert_eq!(
            base_url_host("https://api.deepseek.com/v1/chat/completions"),
            "api.deepseek.com"
        );
        assert_eq!(base_url_host("mock://local"), "local");
        assert_eq!(
            base_url_host("https://token@example.com:8443/v1"),
            "example.com"
        );
    }

    #[test]
    fn test_latest_provider_health_fields_match_model_and_base_url() {
        let entry = ProviderHealthLedgerEntry {
            recorded_at_ms: 123,
            report: crate::diagnostics::provider_health::ProviderHealthReport {
                status: ProviderHealthStatus::Failed,
                model: "deepseek-v4-pro".to_string(),
                base_url: "https://api.deepseek.com".to_string(),
                timeout_secs: 30,
                duration_ms: 456,
                steps: vec![crate::diagnostics::provider_health::ProviderHealthStep {
                    name: "plain_chat".to_string(),
                    status: ProviderHealthStatus::Failed,
                    duration_ms: 456,
                    detail: None,
                    error: Some("timed out".to_string()),
                    error_category: Some("timeout".to_string()),
                }],
            },
        };

        let fields = latest_provider_health_fields(
            Some(&entry),
            "deepseek-v4-pro",
            "https://api.deepseek.com",
        );
        assert_eq!(
            fields,
            (
                Some("failed".to_string()),
                Some("timeout".to_string()),
                Some(456)
            )
        );

        let mismatch =
            latest_provider_health_fields(Some(&entry), "other-model", "https://api.deepseek.com");
        assert_eq!(mismatch, (None, None, None));
    }

    #[tokio::test]
    async fn test_provider_status_endpoint_returns_product_dto_page() {
        let _env_guard = ENV_LOCK.lock().await;
        unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };
        let provider = Arc::new(MockProvider);
        let state = Arc::new(crate::api::state::ApiState {
            provider,
            model: "mock-model".to_string(),
            tool_registry: Arc::new(crate::tools::ToolRegistry::new()),
            session_store: Arc::new(tokio::sync::RwLock::new(
                crate::session_store::SessionStore::in_memory().expect("in-memory session store"),
            )),
            config: Arc::new(tokio::sync::RwLock::new(
                crate::services::config::AppConfig::default(),
            )),
            start_time: Instant::now(),
            request_count: Arc::new(tokio::sync::RwLock::new(0)),
            audit_tracker: Arc::new(tokio::sync::RwLock::new(
                crate::cost_tracker::CostTracker::new(),
            )),
            lsp_manager: None,
            worktree_manager: None,
        });

        let app = create_routes(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/provider/status")
                    .method("GET")
                    .header(
                        axum::http::header::AUTHORIZATION,
                        format!("Bearer {TEST_BRIDGE_TOKEN}"),
                    )
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("valid json response");
        assert!(value["statuses"].as_array().is_some());
        assert_eq!(
            value["record_count"].as_u64(),
            Some(value["statuses"].as_array().unwrap().len() as u64)
        );
        assert!(
            value["statuses"].as_array().unwrap().iter().any(|status| {
                status["provider_id"].as_str().is_some()
                    && status["model_id"].as_str().is_some()
                    && status["protocol_family"].as_str().is_some()
                    && status["request_timeout_secs"].as_u64().is_some()
                    && status["stream_idle_timeout_secs"].as_u64().is_some()
            }),
            "provider status page should expose stable ProviderProductStatus fields"
        );
    }
}
