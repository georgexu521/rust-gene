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
use serde_json::json;
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
    let session_store =
        crate::session_store::SessionStore::in_memory().expect("in-memory session store");
    Arc::new(crate::api::state::ApiState {
        provider: Arc::new(MockProvider),
        model: "mock-model".to_string(),
        tool_registry: Arc::new(crate::tools::ToolRegistry::new()),
        session_store: Arc::new(tokio::sync::RwLock::new(session_store)),
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
    })
}

async fn json_get_response(app: &axum::Router, uri: &str) -> serde_json::Value {
    let (status, value) = json_request_response(app, "GET", uri, None).await;
    assert_eq!(status, StatusCode::OK, "expected 200 for {uri}");
    value
}

async fn json_request_response(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder()
        .uri(uri)
        .method(method)
        .header(header::AUTHORIZATION, format!("Bearer {TEST_BRIDGE_TOKEN}"));
    let body = if let Some(body) = body {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_vec(&body).expect("serialize request body"))
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).expect("build request"))
        .await
        .expect("request should succeed");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    (
        status,
        serde_json::from_slice(&body).expect("valid json response"),
    )
}

#[tokio::test]
async fn provider_status_has_required_fields() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };
    let state = api_test_state();
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/provider/status").await;
    assert!(value["statuses"].is_array(), "statuses is array");
    assert!(value["record_count"].is_number(), "record_count");
    assert!(value["timeout_effective"].is_object(), "timeout_effective");
    assert!(
        value["timeout_effective"]["request_secs"].is_number(),
        "timeout_effective.request_secs"
    );
    assert!(
        value["timeout_effective"]["source"].is_string(),
        "timeout_effective.source"
    );
    let first = &value["statuses"][0];
    assert!(first["provider_id"].is_string(), "provider_id");
    assert!(first["model_id"].is_string(), "model_id");
    assert!(first["protocol_family"].is_string(), "protocol_family");
}

#[tokio::test]
async fn diagnostics_has_policy_and_profile_fields() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };
    let state = api_test_state();
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/diagnostics/latest?session_id=test").await;
    assert!(value["schema"].is_string(), "schema");
    assert!(value["provider_profile"].is_object(), "provider_profile");
    assert!(
        value["tool_output_policy"].is_object(),
        "tool_output_policy"
    );
    assert!(value["revert_events"].is_number(), "revert_events");
}

#[tokio::test]
async fn session_parts_returns_cursor_shape() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };
    let state = api_test_state();
    {
        let store = state.session_store.read().await;
        let _ = store.create_session("test", "Test Session", "mock-model");
    }
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/sessions/test/parts?limit=10").await;
    assert_eq!(value["session_id"], "test");
    assert!(value["parts"].is_array(), "parts is array");
    assert!(value["cursor"].is_object(), "cursor is object");
    assert!(value["cursor"]["has_more"].is_boolean(), "cursor.has_more");
    assert!(value["cursor"]["limit"].is_number(), "cursor.limit");
}

#[tokio::test]
async fn session_reverts_reads_durable_parts() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };
    let state = api_test_state();
    {
        let store = state.session_store.read().await;
        let _ = store.create_session("test", "Test Session", "mock-model");
    }
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/sessions/test/reverts?limit=5").await;
    assert_eq!(value["session_id"], "test");
    assert!(value["reverts"].is_array(), "reverts is array");
    assert!(value["total"].is_number(), "total is number");
}

#[tokio::test]
async fn tool_outputs_returns_session_scoped_index() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };
    let state = api_test_state();
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/sessions/test/tool-outputs").await;
    assert_eq!(value["session_id"], "test");
    assert!(value["outputs"].is_array(), "outputs is array");
}

#[tokio::test]
async fn session_events_returns_cursor_shape() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };
    let state = api_test_state();
    {
        let store = state.session_store.read().await;
        let _ = store.create_session("test", "Test Session", "mock-model");
    }
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/sessions/test/events?limit=10").await;
    assert_eq!(value["session_id"], "test");
    assert!(value["events"].is_array(), "events is array");
    assert!(value["cursor"]["has_more"].is_boolean(), "cursor.has_more");
}

#[tokio::test]
async fn session_prompt_returns_typed_full_agent_not_implemented_response() {
    let _env_guard = ENV_LOCK.lock().await;
    unsafe { std::env::set_var("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN) };
    let state = api_test_state();
    let app = create_routes(state);
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/sessions/test/prompt",
        Some(json!({
            "message": "implement the requested change",
            "agent_mode": "normal",
            "stream": false
        })),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(value["session_id"], "test");
    assert_eq!(value["execution_kind"], "full_agent_turn");
    assert_eq!(value["accepted"], false);
    assert_eq!(value["status"], "not_implemented");
    assert_eq!(value["events_written"], 0);
    assert_eq!(value["agent_runtime_entrypoint"], "RuntimeController");
    assert!(value["turn_id"].is_null(), "turn_id must be empty");
    assert!(value["diagnostic"].is_null(), "diagnostic must be empty");
}
