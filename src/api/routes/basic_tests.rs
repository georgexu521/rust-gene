use super::*;
use crate::services::api::{
    ChatRequest as LlmChatRequest, ChatResponse as LlmChatResponse, LlmProvider, Usage,
};
use async_openai::types::ChatCompletionResponseStream;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use once_cell::sync::Lazy;
use std::{sync::Arc, time::Instant};
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

fn api_test_state() -> Arc<ApiState> {
    Arc::new(crate::api::state::ApiState {
        provider: Arc::new(MockProvider),
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
        agent_runtime: None,
    })
}

async fn get_json(app: axum::Router, uri: &str) -> serde_json::Value {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .method("GET")
                .header(header::AUTHORIZATION, format!("Bearer {TEST_BRIDGE_TOKEN}"))
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&body).expect("valid json response")
}

#[tokio::test]
async fn test_audit_summary_contains_structured_coding_quality() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };
    let state = api_test_state();

    {
        let mut tracker = state.audit_tracker.write().await;
        tracker.record_coding_round(false);
        tracker.record_coding_round(true);
    }

    let value = get_json(create_routes(state), "/api/audit/summary").await;
    let cq = &value["coding_quality"];
    assert!(cq.is_object(), "coding_quality should be an object");
    let rate = cq["first_pass_rate_pct"]
        .as_f64()
        .expect("first_pass_rate_pct should be f64");
    assert!(
        (0.0..=100.0).contains(&rate),
        "first_pass_rate_pct should be in [0,100], got {rate}"
    );
}

#[tokio::test]
async fn test_workflow_weekly_metrics_endpoint() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };

    let value = get_json(
        create_routes(api_test_state()),
        "/api/workflow/metrics/weekly?limit=4",
    )
    .await;
    assert!(
        value["generated_at"].as_str().is_some(),
        "generated_at should be present"
    );
    assert!(value["weeks"].is_array(), "weeks should be array");
}

#[tokio::test]
async fn test_workflow_weekly_calibration_endpoint() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };

    let value = get_json(
        create_routes(api_test_state()),
        "/api/workflow/metrics/calibration/weekly?limit=4",
    )
    .await;
    assert!(
        value["generated_at"].as_str().is_some(),
        "generated_at should be present"
    );
    assert!(value["weeks"].is_array(), "weeks should be array");
}
