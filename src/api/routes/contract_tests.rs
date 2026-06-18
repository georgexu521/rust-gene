use super::*;
use crate::services::api::{
    ChatRequest as LlmChatRequest, ChatResponse as LlmChatResponse, LlmProvider, Usage,
};
use crate::test_utils::env_guard::EnvVarGuard;
use async_openai::types::ChatCompletionResponseStream;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::json;
use std::{sync::Arc, time::Instant};
use tower::util::ServiceExt;

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
                cache_write_tokens: None,
            }),
            tool_call_repair: None,
            finish_reason: None,
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
        agent_runtime: None,
        runner_registry: Arc::new(crate::api::session_runner::ApiSessionRunnerRegistry::new()),
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
async fn provider_chat_route_is_explicit_non_agent_lane() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let state = api_test_state();
    let app = create_routes(state);
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/provider-chat",
        Some(json!({
            "message": "explain this term",
            "session_id": "test"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["execution_kind"], "provider_chat");
    assert_eq!(value["full_agent"], false);
    assert!(value["agent_runtime_entrypoint"].is_null());
    assert!(
        value.get("deprecated_route").is_none(),
        "canonical provider-chat route should not mark itself deprecated"
    );
    assert!(
        value.get("replacement_route").is_none(),
        "canonical provider-chat route should not need replacement"
    );
}

#[tokio::test]
async fn legacy_chat_route_points_to_provider_chat_replacement() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let state = api_test_state();
    let app = create_routes(state);
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/chat",
        Some(json!({
            "message": "explain this term",
            "session_id": "test"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["execution_kind"], "provider_chat");
    assert_eq!(value["full_agent"], false);
    assert_eq!(value["deprecated_route"], "/api/chat");
    assert_eq!(value["replacement_route"], "/api/provider-chat");
}

#[tokio::test]
async fn provider_status_has_required_fields() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
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
async fn config_reports_full_agent_prompt_unavailable_without_runtime() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let state = api_test_state();
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/config").await;

    assert_eq!(value["runtime"]["full_agent_prompt_available"], false);
    assert!(value["runtime"]["agent_runtime_entrypoint"].is_null());
    assert_eq!(
        value["runtime"]["session_prompt_endpoint"],
        "/api/sessions/{id}/prompt"
    );
    assert!(value["context"]["token_counter"].is_string());
}

#[tokio::test]
async fn config_reports_full_agent_prompt_available_with_runtime() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let mut state = api_test_state();
    Arc::get_mut(&mut state).unwrap().agent_runtime = Some(Arc::new(successful_fake_runtime(
        Some("run"),
        Some("idem-1"),
    )));
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/config").await;

    assert_eq!(value["runtime"]["full_agent_prompt_available"], true);
    assert_eq!(
        value["runtime"]["agent_runtime_entrypoint"],
        "RuntimeController"
    );
}

#[tokio::test]
async fn diagnostics_has_policy_and_profile_fields() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
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
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let state = api_test_state();
    {
        let store = state.session_store.read().await;
        let _ = store.create_session("test", "Test Session", "mock-model", None);
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
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let state = api_test_state();
    {
        let store = state.session_store.read().await;
        let _ = store.create_session("test", "Test Session", "mock-model", None);
    }
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/sessions/test/reverts?limit=5").await;
    assert_eq!(value["session_id"], "test");
    assert!(value["reverts"].is_array(), "reverts is array");
    assert!(value["total"].is_number(), "total is number");
}

#[tokio::test]
async fn tool_outputs_returns_session_scoped_index() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let state = api_test_state();
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/sessions/test/tool-outputs").await;
    assert_eq!(value["session_id"], "test");
    assert!(value["outputs"].is_array(), "outputs is array");
}

#[tokio::test]
async fn session_events_returns_cursor_shape() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let state = api_test_state();
    {
        let store = state.session_store.read().await;
        let _ = store.create_session("test", "Test Session", "mock-model", None);
    }
    let app = create_routes(state);
    let value = json_get_response(&app, "/api/sessions/test/events?limit=10").await;
    assert_eq!(value["session_id"], "test");
    assert!(value["events"].is_array(), "events is array");
    assert!(value["cursor"]["has_more"].is_boolean(), "cursor.has_more");
}

#[tokio::test]
async fn session_prompt_returns_typed_full_agent_not_implemented_response() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
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

/// Fake runtime that returns a successful outcome for tests.
struct FakeAgentRuntime {
    expected_session_id: String,
    expected_message: String,
    expected_delivery: Option<String>,
    expected_idempotency_key: Option<String>,
    response_accepted: bool,
    response_status: String,
    response_error: Option<String>,
}

#[async_trait::async_trait]
impl crate::api::state::ApiAgentRuntime for FakeAgentRuntime {
    async fn submit_prompt(
        &self,
        input: crate::api::state::ApiSessionPromptInput,
    ) -> anyhow::Result<crate::api::state::ApiSessionPromptOutcome> {
        assert_eq!(input.session_id, self.expected_session_id);
        assert_eq!(input.message, self.expected_message);
        assert_eq!(input.delivery, self.expected_delivery);
        assert_eq!(input.idempotency_key, self.expected_idempotency_key);
        Ok(crate::api::state::ApiSessionPromptOutcome {
            accepted: self.response_accepted,
            turn_id: Some("turn-fake-001".to_string()),
            status: self.response_status.clone(),
            events_written: 3,
            latest_part_index: Some(5),
            diagnostic: None,
            error: self.response_error.clone(),
        })
    }
}

fn successful_fake_runtime(
    expected_delivery: Option<&str>,
    expected_idempotency_key: Option<&str>,
) -> FakeAgentRuntime {
    FakeAgentRuntime {
        expected_session_id: "test".to_string(),
        expected_message: "fix the bug".to_string(),
        expected_delivery: expected_delivery.map(str::to_string),
        expected_idempotency_key: expected_idempotency_key.map(str::to_string),
        response_accepted: true,
        response_status: "completed".to_string(),
        response_error: None,
    }
}

#[tokio::test]
async fn session_prompt_with_runtime_returns_accepted_true() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let mut state = api_test_state();
    Arc::get_mut(&mut state).unwrap().agent_runtime = Some(Arc::new(successful_fake_runtime(
        Some("run"),
        Some("idem-1"),
    )));
    let app = create_routes(state);
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/sessions/test/prompt",
        Some(json!({
            "message": "fix the bug",
            "agent_mode": "normal",
            "stream": false,
            "delivery": "run",
            "idempotency_key": "idem-1"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["session_id"], "test");
    assert_eq!(value["execution_kind"], "full_agent_turn");
    assert_eq!(value["accepted"], true);
    assert_eq!(value["status"], "completed");
    assert_eq!(value["events_written"], 3);
    assert_eq!(value["latest_part_index"], 5);
    assert_eq!(value["turn_id"], "turn-fake-001");
    assert_eq!(value["agent_runtime_entrypoint"], "RuntimeController");
    assert!(value["error"].is_null(), "error must be null on success");
}

#[tokio::test]
async fn session_prompt_without_runtime_still_returns_501() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let state = api_test_state();
    assert!(state.agent_runtime.is_none());
    let app = create_routes(state);
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/sessions/test/prompt",
        Some(json!({
            "message": "fix the bug",
        })),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(value["accepted"], false);
    assert_eq!(
        value["error"],
        "full-agent prompt runtime is unavailable in this API state"
    );
}

#[tokio::test]
async fn session_prompt_accepts_queue_delivery_as_admission_contract() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let mut state = api_test_state();
    let mut fake = successful_fake_runtime(Some("queue"), Some("queue-1"));
    fake.response_status = "queued".to_string();
    Arc::get_mut(&mut state).unwrap().agent_runtime = Some(Arc::new(fake));
    let app = create_routes(state);
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/sessions/test/prompt",
        Some(json!({
            "message": "fix the bug",
            "delivery": "queue",
            "idempotency_key": "queue-1"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(value["accepted"], true);
    assert_eq!(value["status"], "queued");
}

#[tokio::test]
async fn session_prompt_maps_idempotency_conflict_to_409() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let mut state = api_test_state();
    let mut fake = successful_fake_runtime(Some("run"), Some("idem-1"));
    fake.response_accepted = false;
    fake.response_status = "conflict".to_string();
    fake.response_error =
        Some("idempotency_key was reused with different message content".to_string());
    Arc::get_mut(&mut state).unwrap().agent_runtime = Some(Arc::new(fake));
    let app = create_routes(state);
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/sessions/test/prompt",
        Some(json!({
            "message": "fix the bug",
            "delivery": "run",
            "idempotency_key": "idem-1"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(value["accepted"], false);
    assert_eq!(value["status"], "conflict");
}

#[tokio::test]
async fn session_prompt_rejects_unknown_delivery_modes() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let app = create_routes(api_test_state());
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/sessions/test/prompt",
        Some(json!({
            "message": "fix the bug",
            "delivery": "later"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value["error"],
        "delivery must be one of: run, admit_only, queue"
    );
}

#[tokio::test]
async fn session_prompt_rejects_reserved_idempotency_keys() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let app = create_routes(api_test_state());
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/sessions/test/prompt",
        Some(json!({
            "message": "fix the bug",
            "idempotency_key": "__internal"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value["error"],
        "idempotency_key starting with '__' is reserved"
    );
}

#[tokio::test]
async fn session_prompt_rejects_unimplemented_streaming_mode() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let app = create_routes(api_test_state());
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/sessions/test/prompt",
        Some(json!({
            "message": "fix the bug",
            "stream": true
        })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value["error"],
        "stream=true is not implemented for /api/sessions/:id/prompt yet; use stream=false"
    );
}

#[tokio::test]
async fn session_prompt_rejects_unknown_agent_mode() {
    let mut env = EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_BRIDGE_TOKEN", TEST_BRIDGE_TOKEN);
    let app = create_routes(api_test_state());
    let (status, value) = json_request_response(
        &app,
        "POST",
        "/api/sessions/test/prompt",
        Some(json!({
            "message": "fix the bug",
            "agent_mode": "fast"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value["error"],
        "agent_mode must be one of: auto, normal, build, plan, explore, review"
    );
}
