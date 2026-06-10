//! API 路由定义
//!
//! 定义所有 REST API 端点

use axum::{
    body::Body,
    extract::{Json, Path, Query, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

use super::dto;

use super::{ws_handler, ApiError, ApiState, MessageInfo};

/// 创建 API 路由
pub fn create_routes(state: Arc<ApiState>) -> Router {
    let bridge_v1 = Router::new()
        .route(
            "/v1/sessions",
            get(v1_list_sessions_handler).post(v1_create_session_handler),
        )
        .route("/v1/sessions/:id", get(v1_get_session_handler))
        .route(
            "/v1/sessions/:id/status",
            get(v1_get_session_status_handler),
        )
        .route(
            "/v1/sessions/:id/messages",
            get(v1_get_session_messages_handler),
        )
        .route("/v1/triggers/:id/run", post(v1_run_trigger_handler))
        .layer(middleware::from_fn(bridge_auth_middleware));

    // 受保护的 API 路由（需要认证）
    let api_routes = Router::new()
        // Provider chat compatibility API (explicit non-agent lane)
        .route("/api/chat", post(legacy_chat_handler))
        .route("/api/chat/stream", post(chat_stream_handler))
        // WebSocket
        .route("/api/ws", get(ws_handler))
        // Session API
        .route(
            "/api/sessions",
            get(list_sessions_handler).post(create_session_handler),
        )
        .route(
            "/api/sessions/:id",
            get(get_session_handler)
                .delete(delete_session_handler)
                .put(update_session_handler),
        )
        .route(
            "/api/sessions/:id/messages",
            get(get_session_messages_handler),
        )
        // Session prompt (full-agent, requires ApiAgentRuntime)
        .route("/api/sessions/:id/prompt", post(session_prompt_handler))
        // Session run lifecycle
        .route("/api/sessions/:id/wait", post(session_wait_handler))
        .route("/api/sessions/:id/cancel", post(session_cancel_handler))
        .route(
            "/api/sessions/:id/run-status",
            get(session_run_status_handler),
        )
        .route("/api/sessions/:id/compact", post(session_compact_handler))
        // Active context inspection
        .route("/api/sessions/:id/context", get(session_context_handler))
        // Provider chat (explicit non-agent lane)
        .route("/api/provider-chat", post(provider_chat_handler))
        // Tool API
        .route("/api/tools", get(list_tools_handler))
        .route("/api/tools/:name", get(get_tool_handler))
        .route("/api/tools/call", post(call_tool_handler))
        // Config API
        .route(
            "/api/config",
            get(get_config_handler).put(update_config_handler),
        )
        // Stats API
        .route("/api/stats", get(get_stats_handler))
        .route(
            "/api/workflow/metrics/weekly",
            get(get_workflow_weekly_metrics_handler),
        )
        .route(
            "/api/workflow/metrics/calibration/weekly",
            get(get_workflow_weekly_calibration_handler),
        )
        // Audit API
        .route("/api/audit/summary", get(get_audit_summary_handler))
        .route("/api/audit/recent", get(get_audit_recent_handler))
        .route("/api/audit/export", post(export_audit_handler))
        // Session parts & events cursors (Phase 1 productization)
        .route("/api/sessions/:id/parts", get(get_session_parts_handler))
        .route("/api/sessions/:id/events", get(get_session_events_handler))
        .route(
            "/api/sessions/:id/reverts",
            get(get_session_reverts_handler),
        )
        .route(
            "/api/sessions/:id/tool-outputs",
            get(tool_output::get_tool_outputs_handler),
        )
        .route(
            "/api/sessions/:id/tool-outputs/:output_id",
            get(tool_output::get_tool_output_page_handler),
        )
        // Provider status
        .route(
            "/api/provider/status",
            get(tool_output::get_provider_status_handler),
        )
        .route("/api/provider/catalog", get(get_provider_catalog_handler))
        // Session jobs
        .route("/api/sessions/:id/jobs", get(get_session_jobs_handler))
        // Diagnostic export
        .route(
            "/api/diagnostics/latest",
            get(tool_output::get_diagnostics_handler),
        )
        .layer(middleware::from_fn(bridge_auth_middleware));

    // 公开路由（无需认证）
    let public_routes = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/version", get(version_handler));

    Router::new()
        .merge(api_routes)
        .merge(public_routes)
        .merge(bridge_v1)
        .with_state(state)
}

// ── Chat Handlers ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub session_id: Option<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub content: String,
    pub session_id: String,
    pub model: String,
    pub usage: Option<UsageInfo>,
    #[serde(default)]
    pub execution_kind: String,
    #[serde(default)]
    pub full_agent: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_runtime_entrypoint: Option<String>,
    /// Deprecation metadata when called via legacy route.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated_route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_route: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

async fn legacy_chat_handler(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<ChatRequest>,
) -> Result<impl IntoResponse, ApiError> {
    chat_response(state, req, Some("/api/chat"), Some("/api/provider-chat")).await
}

async fn provider_chat_handler(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<ChatRequest>,
) -> Result<impl IntoResponse, ApiError> {
    chat_response(state, req, None, None).await
}

async fn chat_response(
    state: Arc<ApiState>,
    req: ChatRequest,
    deprecated_route: Option<&'static str>,
    replacement_route: Option<&'static str>,
) -> Result<impl IntoResponse, ApiError> {
    let session_id = req
        .session_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let result = state.chat(req).await?;

    Ok((
        StatusCode::OK,
        Json(ChatResponse {
            content: result.content,
            session_id,
            model: result.model,
            usage: result.usage,
            execution_kind: "provider_chat".to_string(),
            full_agent: false,
            agent_runtime_entrypoint: None,
            deprecated_route: deprecated_route.map(str::to_string),
            replacement_route: replacement_route.map(str::to_string),
        }),
    ))
}

async fn chat_stream_handler(
    State(_state): State<Arc<ApiState>>,
    Json(_req): Json<ChatRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // SSE 流式响应
    Err::<(StatusCode, Json<ChatResponse>), ApiError>(ApiError::NotImplemented(
        "Stream endpoint".to_string(),
    ))
}

// ── Session Handlers ───────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSessionRequest {
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionPromptRequest {
    pub message: String,
    pub agent_mode: Option<String>,
    pub stream: Option<bool>,
    pub delivery: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionPromptResponse {
    pub session_id: String,
    pub execution_kind: String,
    pub accepted: bool,
    pub turn_id: Option<String>,
    pub status: String,
    pub events_written: usize,
    pub latest_part_index: Option<i64>,
    pub diagnostic: Option<dto::diagnostic::DiagnosticExportDto>,
    pub agent_runtime_entrypoint: Option<String>,
    pub error: Option<String>,
}

async fn list_sessions_handler(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    let sessions = state.list_sessions(limit).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "sessions": sessions,
            "total": sessions.len(),
        })),
    ))
}

async fn create_session_handler(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = state.create_session(req.title).await?;

    Ok((StatusCode::CREATED, Json(session)))
}

async fn get_session_handler(
    Path(id): Path<String>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let session = state.get_session(&id).await?;

    Ok((StatusCode::OK, Json(session)))
}

async fn update_session_handler(
    Path(id): Path<String>,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<UpdateSessionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = state.update_session(&id, &req.title).await?;

    Ok((StatusCode::OK, Json(session)))
}

async fn delete_session_handler(
    Path(id): Path<String>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    state.delete_session(&id).await?;

    Ok((StatusCode::NO_CONTENT, ()))
}

async fn get_session_messages_handler(
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let messages = state.get_session_messages(&id, limit).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "messages": messages,
            "session_id": id,
        })),
    ))
}

async fn session_prompt_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(req): Json<SessionPromptRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if req.message.trim().is_empty() {
        return Err(ApiError::BadRequest("message is required".to_string()));
    }
    if req.stream.unwrap_or(false) {
        return Err(ApiError::BadRequest(
            "stream=true is not implemented for /api/sessions/:id/prompt yet; use stream=false"
                .to_string(),
        ));
    }
    if let Some(agent_mode) = req.agent_mode.as_deref() {
        if crate::engine::agent_mode::AgentMode::parse(agent_mode).is_none() {
            return Err(ApiError::BadRequest(
                "agent_mode must be one of: auto, normal, build, plan, explore, review".to_string(),
            ));
        }
    }
    if let Some(delivery) = req.delivery.as_deref() {
        match delivery.trim().to_ascii_lowercase().as_str() {
            "" | "run" | "admit_only" | "queue" => {}
            _ => {
                return Err(ApiError::BadRequest(
                    "delivery must be one of: run, admit_only, queue".to_string(),
                ));
            }
        }
    }
    if let Some(idempotency_key) = req.idempotency_key.as_deref() {
        if idempotency_key.trim().starts_with("__") {
            return Err(ApiError::BadRequest(
                "idempotency_key starting with '__' is reserved".to_string(),
            ));
        }
    }

    let Some(ref agent_runtime) = state.agent_runtime else {
        return Ok((
            StatusCode::NOT_IMPLEMENTED,
            Json(SessionPromptResponse {
                session_id: id,
                execution_kind: "full_agent_turn".to_string(),
                accepted: false,
                turn_id: None,
                status: "not_implemented".to_string(),
                events_written: 0,
                latest_part_index: None,
                diagnostic: None,
                agent_runtime_entrypoint: Some("RuntimeController".to_string()),
                error: Some(
                    "full-agent prompt API is not wired to RuntimeController yet".to_string(),
                ),
            }),
        ));
    };

    let input = crate::api::state::ApiSessionPromptInput {
        session_id: id.clone(),
        message: req.message,
        agent_mode: req.agent_mode.clone(),
        stream: req.stream.unwrap_or(false),
        delivery: req.delivery.clone(),
        idempotency_key: req.idempotency_key.clone(),
    };

    let outcome = agent_runtime
        .submit_prompt(input)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let status_code = session_prompt_status_code(&outcome);

    Ok((
        status_code,
        Json(SessionPromptResponse {
            session_id: id,
            execution_kind: "full_agent_turn".to_string(),
            accepted: outcome.accepted,
            turn_id: outcome.turn_id,
            status: outcome.status,
            events_written: outcome.events_written,
            latest_part_index: outcome.latest_part_index,
            diagnostic: outcome.diagnostic,
            agent_runtime_entrypoint: Some("RuntimeController".to_string()),
            error: outcome.error,
        }),
    ))
}

fn session_prompt_status_code(outcome: &crate::api::state::ApiSessionPromptOutcome) -> StatusCode {
    match outcome.status.as_str() {
        "admitted" | "queued" => StatusCode::ACCEPTED,
        "conflict" => StatusCode::CONFLICT,
        "rejected" => StatusCode::BAD_REQUEST,
        _ if outcome.accepted => StatusCode::OK,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// ── Session Run Lifecycle ─────────────────────────────

async fn session_wait_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    state.runner_registry.wait_idle(&id).await;
    Ok(Json(json!({
        "session_id": id,
        "status": state.runner_registry.status(&id).to_string(),
    })))
}

async fn session_cancel_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let cancelled = match &state.agent_runtime {
        Some(runtime) => runtime
            .cancel(&id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?,
        None => state.runner_registry.request_cancel(&id),
    };
    Ok(Json(json!({
        "session_id": id,
        "cancelled": cancelled,
        "status": state.runner_registry.status(&id).to_string(),
    })))
}

#[derive(Debug, Serialize)]
struct RunStatusResponse {
    session_id: String,
    status: String,
}

async fn session_run_status_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let status = state.runner_registry.status(&id);
    Ok(Json(RunStatusResponse {
        session_id: id,
        status: status.to_string(),
    }))
}

// ── Session Compact ────────────────────────────────────

async fn session_compact_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    match &state.agent_runtime {
        Some(runtime) => match runtime.compact(&id).await {
            Ok(Some(outcome)) => Ok((
                StatusCode::OK,
                Json(json!({
                    "session_id": id,
                    "compacted": true,
                    "boundary_id": outcome.boundary_id,
                    "before_tokens": outcome.before_tokens,
                    "after_tokens": outcome.after_tokens,
                    "messages_before": outcome.messages_before,
                    "messages_after": outcome.messages_after,
                })),
            )),
            Ok(None) => Ok((
                StatusCode::OK,
                Json(json!({
                    "session_id": id,
                    "compacted": false,
                    "reason": "no compaction needed or runtime compaction not available"
                })),
            )),
            Err(e) => Err(ApiError::Internal(e.to_string())),
        },
        None => Ok((
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({
                "session_id": id,
                "compacted": false,
                "reason": "compaction requires RuntimeController (not available in API server mode)"
            })),
        )),
    }
}

// ── Active Context ─────────────────────────────────────

async fn session_context_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(runtime) = &state.agent_runtime {
        if let Some(snapshot) = runtime
            .context_snapshot(&id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
        {
            let latest_boundary = {
                let store = state.session_store.read().await;
                store.latest_compact_boundary(&id).ok().flatten()
            };
            let latest_compaction = latest_boundary
                .as_ref()
                .map(|b| dto::context::CompactionSummaryDto {
                    boundary_id: b.boundary_id.clone(),
                    strategy: b.strategy.clone(),
                    trigger: b.trigger.clone().unwrap_or_default(),
                    before_tokens: b.before_tokens as u64,
                    after_tokens: b.after_tokens as u64,
                    messages_before: b.messages_before as usize,
                    messages_after: b.messages_after as usize,
                    preserved_tail_count: b.preserved_tail_count.unwrap_or(0) as usize,
                })
                .or_else(|| {
                    snapshot.compact.latest_attempt_tokens_before.map(|before| {
                        dto::context::CompactionSummaryDto {
                            boundary_id: snapshot
                                .compact
                                .latest_boundary_id
                                .clone()
                                .unwrap_or_else(|| "latest_attempt".to_string()),
                            strategy: snapshot.compact.latest_strategy.clone().unwrap_or_default(),
                            trigger: snapshot
                                .compact
                                .latest_attempt_trigger
                                .clone()
                                .unwrap_or_default(),
                            before_tokens: before,
                            after_tokens: snapshot
                                .compact
                                .latest_attempt_tokens_after
                                .unwrap_or(before),
                            messages_before: 0,
                            messages_after: 0,
                            preserved_tail_count: 0,
                        }
                    })
                });
            let compact_boundary_id = snapshot
                .compact
                .latest_boundary_id
                .clone()
                .or_else(|| latest_boundary.as_ref().map(|b| b.boundary_id.clone()));
            return Ok(Json(dto::context::SessionContextDto {
                session_id: id,
                compact_boundary_id,
                estimated_history_tokens: snapshot.history_tokens,
                tool_schema_tokens: snapshot.tool_schema_tokens,
                memory_snapshot_tokens: snapshot.memory_snapshot_tokens,
                stable_prefix_hash: Some(snapshot.stable_prefix_fingerprint),
                dynamic_tail_hash: None,
                latest_compaction,
                message_count_after_compaction: snapshot.history_messages,
            }));
        }
    }

    let store = state.session_store.read().await;
    let parts = store.get_session_parts(&id).unwrap_or_default();
    let latest_boundary = store.latest_compact_boundary(&id).ok().flatten();

    let (
        compact_boundary_id,
        before_tokens,
        after_tokens,
        strategy,
        trigger,
        messages_before,
        messages_after,
        preserved_tail_count,
    ) = if let Some(ref b) = latest_boundary {
        (
            Some(b.boundary_id.clone()),
            b.before_tokens as u64,
            b.after_tokens as u64,
            b.strategy.clone(),
            b.trigger.clone().unwrap_or_default(),
            b.messages_before as usize,
            b.messages_after as usize,
            b.preserved_tail_count.unwrap_or(0),
        )
    } else {
        (None, 0u64, 0u64, String::new(), String::new(), 0, 0, 0)
    };

    let message_count = parts.len();
    let compaction = if latest_boundary.is_some() {
        Some(dto::context::CompactionSummaryDto {
            boundary_id: compact_boundary_id.clone().unwrap_or_default(),
            strategy,
            trigger,
            before_tokens,
            after_tokens,
            messages_before,
            messages_after,
            preserved_tail_count: preserved_tail_count as usize,
        })
    } else {
        None
    };

    Ok(Json(dto::context::SessionContextDto {
        session_id: id,
        compact_boundary_id,
        estimated_history_tokens: before_tokens.max(after_tokens),
        tool_schema_tokens: 0,
        memory_snapshot_tokens: 0,
        stable_prefix_hash: None,
        dynamic_tail_hash: None,
        latest_compaction: compaction,
        message_count_after_compaction: message_count,
    }))
}

// ── Provider Catalog ───────────────────────────────────

fn sanitize_base_url_host(base_url: &str) -> String {
    let without_scheme = base_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(base_url);
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    let host = authority.rsplit('@').next().unwrap_or(authority);
    host.split(':').next().unwrap_or(host).to_string()
}

async fn get_provider_catalog_handler(
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let base_url = state.provider.base_url().to_string();
    let capabilities = crate::services::api::provider_protocol::ProviderCapabilities::detect(
        &base_url,
        &state.model,
    );
    let profile = crate::services::api::provider_protocol::ProviderRuntimeProfile::snapshot(
        &capabilities,
        &state.model,
        "api-current",
    );
    let context =
        crate::engine::model_context::ModelContextProfile::detect(&base_url, &state.model);

    Ok(Json(dto::provider_catalog::ProviderCatalogDto {
        schema: "provider_catalog.v1".to_string(),
        providers: vec![dto::provider_catalog::ProviderCatalogEntry {
            provider_id: state.model.clone(),
            label: state.model.clone(),
            enabled: true,
            source: "runtime".to_string(),
            base_url_host: sanitize_base_url_host(&base_url),
            default_model: state.model.clone(),
            available_model_ids: vec![state.model.clone()],
            context_limit: Some(context.context_window_tokens),
            output_limit: Some(context.reserved_output_tokens),
            protocol_family: profile.protocol_family.label().to_string(),
            supports_streaming: profile.supports_streaming_tool_calls,
            requires_nonstreaming: profile.requires_nonstreaming_tool_calls,
            last_health_status: profile.last_health_status.clone(),
            last_latency_ms: None,
            recent_timeout_category: profile.last_timeout_category.clone(),
            cost_input_per_1m: None,
            cost_output_per_1m: None,
        }],
    }))
}

// ── Session Jobs ───────────────────────────────────────

async fn get_session_jobs_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let store = state.session_store.read().await;
    let rows = store.get_session_jobs(&id).unwrap_or_default();
    let jobs: Vec<dto::session_jobs::SessionJobItem> = rows
        .into_iter()
        .map(|r| dto::session_jobs::SessionJobItem {
            job_id: r.job_id,
            session_id: r.session_id,
            command: r.command,
            cwd: r.cwd,
            status: r.status,
            started_at: r.started_at,
            completed_at: r.completed_at,
            exit_code: r.exit_code,
            timed_out: r.timed_out,
            tool_output_uri: r.tool_output_uri,
            cancelled: r.cancelled,
        })
        .collect();
    Ok(Json(dto::session_jobs::SessionJobsPage {
        session_id: id,
        total: jobs.len(),
        jobs,
    }))
}

// ── Tool Handlers ──────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ToolCallRequest {
    pub tool: String,
    pub params: serde_json::Value,
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ToolCallResponse {
    pub success: bool,
    pub content: String,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

async fn list_tools_handler(
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let tools = state.list_tools().await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "tools": tools,
            "count": tools.len(),
        })),
    ))
}

async fn get_tool_handler(
    Path(name): Path<String>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let tool = state.get_tool(&name).await?;

    Ok((StatusCode::OK, Json(tool)))
}

async fn call_tool_handler(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<ToolCallRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if !helpers::api_tool_call_allowed(&req.tool, &req.params) {
        return Err(ApiError::Forbidden(format!(
            "tool '{}' is not allowed via API for security reasons",
            req.tool
        )));
    }

    let session_id = req
        .session_id
        .unwrap_or_else(|| format!("api-{}", uuid::Uuid::new_v4()));

    let result = state.call_tool(&req.tool, req.params, &session_id).await?;

    Ok((StatusCode::OK, Json(result)))
}

pub(crate) fn api_tool_call_allowed(tool: &str, params: &serde_json::Value) -> bool {
    match tool {
        // Local read/search helpers.
        "file_read"
        | "grep"
        | "glob"
        | "project_list"
        | "git_status"
        | "git_diff"
        | "diff"
        | "calculate"
        | "datetime"
        | "json_query"
        | "context"
        | "context_visualization"
        | "tool_search"
        | "symbol_query" => true,
        // Public-network read tools are explicitly allowed; browser automation is not.
        "web_search" | "web_fetch" => true,
        // Task output can append, so only the default/get action is allowed remotely.
        "task_get" | "task_list" => true,
        "task_output" => params["action"]
            .as_str()
            .map(|action| action == "get")
            .unwrap_or(true),
        // Cost reads API-local accounting; ApiState injects the tracker for this call.
        "cost" => true,
        _ => false,
    }
}

// ── Config Handlers ────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub api: ApiConfigInfo,
    pub ui: UiConfigInfo,
    pub features: FeatureFlagsInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiConfigInfo {
    pub model: String,
    pub base_url: String,
    pub temperature: f32,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiConfigInfo {
    pub theme: String,
    pub show_token_usage: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FeatureFlagsInfo {
    pub mcp_enabled: bool,
    pub skills_enabled: bool,
    pub web_search: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    pub api: Option<ApiConfigInfo>,
    pub ui: Option<UiConfigInfo>,
    pub features: Option<FeatureFlagsInfo>,
}

async fn get_config_handler(
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let config = state.get_config().await?;

    Ok((StatusCode::OK, Json(config)))
}

async fn update_config_handler(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<UpdateConfigRequest>,
) -> Result<impl IntoResponse, ApiError> {
    state.update_config(req).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Configuration updated successfully",
        })),
    ))
}

// ── Stats Handlers ─────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_sessions: i64,
    pub total_messages: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub uptime_secs: u64,
    pub version: String,
}

async fn get_stats_handler(
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let stats = state.get_stats().await?;

    Ok((StatusCode::OK, Json(stats)))
}

#[derive(Debug, Serialize)]
pub struct WorkflowWeeklyMetricItem {
    pub week_key: String,
    pub runs: usize,
    pub mainline_hit_rate: f64,
    pub avg_first_plan_coverage: f64,
    pub avg_rework_rate: f64,
    pub avg_objective_score: f64,
}

#[derive(Debug, Serialize)]
pub struct WorkflowWeeklyMetricsResponse {
    pub generated_at: String,
    pub weeks: Vec<WorkflowWeeklyMetricItem>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowWeeklyCalibrationItem {
    pub week_key: String,
    pub samples: usize,
    pub avg_mainline_bias_abs: f64,
    pub avg_coverage_bias_abs: f64,
    pub avg_objective_bias_abs: f64,
}

#[derive(Debug, Serialize)]
pub struct WorkflowWeeklyCalibrationResponse {
    pub generated_at: String,
    pub weeks: Vec<WorkflowWeeklyCalibrationItem>,
}

async fn get_workflow_weekly_metrics_handler(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8);
    let result = state.get_workflow_weekly_metrics(limit).await?;
    Ok((StatusCode::OK, Json(result)))
}

async fn get_workflow_weekly_calibration_handler(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8);
    let result = state.get_workflow_weekly_calibration(limit).await?;
    Ok((StatusCode::OK, Json(result)))
}

// ── Audit Handlers ─────────────────────────────────────

async fn get_audit_summary_handler(
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let summary = state.get_audit_summary().await?;
    Ok((StatusCode::OK, Json(summary)))
}

async fn get_audit_recent_handler(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50);
    let events = state.get_audit_recent(limit).await?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "events": events,
            "count": events.len(),
        })),
    ))
}

#[derive(Debug, Deserialize)]
struct ExportAuditRequest {
    session_id: Option<String>,
    recent_limit: Option<usize>,
    path: Option<String>,
}

async fn export_audit_handler(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<ExportAuditRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // 路径安全校验
    let path_buf = if let Some(path_str) = req.path.as_deref() {
        let path = std::path::PathBuf::from(path_str);
        // 拒绝包含 .. 的路径
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(ApiError::BadRequest(
                "path contains '..' which is not allowed".into(),
            ));
        }
        // 拒绝绝对路径（防止写入系统任意位置）
        if path.is_absolute() {
            return Err(ApiError::BadRequest(
                "absolute paths are not allowed".into(),
            ));
        }
        // 限制路径长度（防止路径截断或缓冲区问题）
        const MAX_PATH_LEN: usize = 4096;
        if path_str.len() > MAX_PATH_LEN {
            return Err(ApiError::BadRequest(format!(
                "path too long (max {} chars)",
                MAX_PATH_LEN
            )));
        }
        Some(path)
    } else {
        None
    };

    let snapshot = state
        .export_audit_snapshot(
            req.session_id.as_deref(),
            req.recent_limit.unwrap_or(200),
            path_buf.as_deref(),
        )
        .await?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "snapshot": snapshot,
            "written_path": path_buf.as_ref().map(|p| p.display().to_string()),
        })),
    ))
}

// ── Health Handlers ────────────────────────────────────

async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

async fn version_handler() -> impl IntoResponse {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
    }))
}

// ── Bridge v1 Handlers ─────────────────────────────────

#[derive(Debug, Deserialize)]
struct BridgeCreateSessionRequest {
    prompt: String,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BridgeRunTriggerRequest {
    message: Option<String>,
    prompt: Option<String>,
}

fn configured_bridge_tokens() -> Vec<String> {
    if let Ok(list) = std::env::var("PRIORITY_AGENT_BRIDGE_TOKENS") {
        let parsed = helpers::parse_token_list(&list);
        if !parsed.is_empty() {
            return parsed;
        }
    }
    std::env::var("PRIORITY_AGENT_BRIDGE_TOKEN")
        .ok()
        .or_else(|| std::env::var("BRIDGE_TOKEN").ok())
        .map(|t| vec![t])
        .unwrap_or_default()
}

async fn v1_list_sessions_handler(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let prefix = helpers::tenant_prefix(&headers);

    // 先拉更大窗口再过滤，避免多租户混排导致当前租户会话被截断
    let sessions = state.list_sessions((limit.max(20) * 5).min(200)).await?;
    let filtered: Vec<SessionInfo> = sessions
        .into_iter()
        .filter(|s| s.id.starts_with(&prefix))
        .take(limit as usize)
        .collect();

    Ok((
        StatusCode::OK,
        Json(json!({
            "sessions": filtered,
            "total": filtered.len(),
        })),
    ))
}

async fn v1_get_session_handler(
    headers: HeaderMap,
    Path(id): Path<String>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let prefix = helpers::tenant_prefix(&headers);
    if !id.starts_with(&prefix) {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }
    let session = state.get_session(&id).await?;
    Ok((StatusCode::OK, Json(session)))
}

async fn v1_get_session_status_handler(
    headers: HeaderMap,
    Path(id): Path<String>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let prefix = helpers::tenant_prefix(&headers);
    if !id.starts_with(&prefix) {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    let session = state.get_session(&id).await?;
    let store = state.session_store.read().await;
    let count = store.message_count(&id).unwrap_or(0);
    let messages = store.get_messages(&id).unwrap_or_default();
    let last_message_id = messages.last().map(|m| m.id).unwrap_or(0);
    let last_message_at = messages.last().map(|m| m.created_at.clone());

    Ok((
        StatusCode::OK,
        Json(json!({
            "id": id,
            "exists": true,
            "title": session.title,
            "updated_at": session.updated_at,
            "message_count": count,
            "last_message_id": last_message_id,
            "last_message_at": last_message_at,
        })),
    ))
}

async fn v1_get_session_messages_handler(
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let prefix = helpers::tenant_prefix(&headers);
    if !id.starts_with(&prefix) {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    // 先验证会话存在
    let _ = state.get_session(&id).await?;

    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100)
        .clamp(1, 1000);
    let since_id = params
        .get("since_id")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let store = state.session_store.read().await;
    let all = store
        .get_messages(&id)
        .map_err(|e| ApiError::Internal(format!("Failed to load messages: {}", e)))?;
    let filtered: Vec<MessageInfo> = all
        .into_iter()
        .filter(|m| m.id > since_id)
        .take(limit as usize)
        .map(|m| MessageInfo {
            id: m.id,
            role: m.role,
            content: m.content,
            created_at: m.created_at,
        })
        .collect();
    let next_since_id = filtered.last().map(|m| m.id).unwrap_or(since_id);

    Ok((
        StatusCode::OK,
        Json(json!({
            "session_id": id,
            "messages": filtered,
            "count": filtered.len(),
            "since_id": since_id,
            "next_since_id": next_since_id,
        })),
    ))
}

async fn v1_create_session_handler(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<BridgeCreateSessionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if req.prompt.trim().is_empty() {
        return Err(ApiError::BadRequest("prompt is required".to_string()));
    }
    let prefix = helpers::tenant_prefix(&headers);
    let session_id = format!("{}{}", prefix, uuid::Uuid::new_v4().simple());
    let title = req
        .title
        .unwrap_or_else(|| format!("Remote: {}", helpers::truncate_chars(req.prompt.trim(), 64)));
    let session = state
        .create_session_with_id(session_id.clone(), Some(title))
        .await?;
    let response = json!({
        "id": session.id,
        "title": session.title,
        "created_at": session.created_at,
        "updated_at": session.updated_at,
    });
    Ok((StatusCode::CREATED, Json(response)))
}

async fn v1_run_trigger_handler(
    headers: HeaderMap,
    Path(id): Path<String>,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<BridgeRunTriggerRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let prefix = helpers::tenant_prefix(&headers);
    if !id.starts_with(&prefix) {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }

    let message = req
        .message
        .or(req.prompt)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::BadRequest("message or prompt is required".to_string()))?;

    if state.get_session(&id).await.is_err() {
        let _ = state
            .create_session_with_id(id.clone(), Some("Remote Trigger Session".to_string()))
            .await?;
    }

    let chat_resp = state
        .chat(ChatRequest {
            message,
            session_id: Some(id.clone()),
            system_prompt: None,
            model: None,
            temperature: None,
            stream: None,
        })
        .await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "id": id,
            "output": chat_resp.content,
            "usage": chat_resp.usage,
        })),
    ))
}

async fn bridge_auth_middleware(req: Request<Body>, next: Next) -> Response {
    let configured = configured_bridge_tokens();
    if configured.is_empty() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "bridge authentication not configured",
                "status": 403
            })),
        )
            .into_response();
    }
    let bearer = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim);
    let header_token = req
        .headers()
        .get("x-bridge-token")
        .and_then(|v| v.to_str().ok())
        .map(str::trim);
    let authed = bearer
        .map(|t| configured.iter().any(|x| x == t))
        .unwrap_or(false)
        || header_token
            .map(|t| configured.iter().any(|x| x == t))
            .unwrap_or(false);
    if !authed {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "unauthorized",
                "status": 401
            })),
        )
            .into_response();
    }
    next.run(req).await
}

// ── Phase 1: Session Parts / Events / Reverts / Tool-Outputs / Provider / Diagnostics ──

#[derive(Debug, Deserialize)]
struct PartsQuery {
    after: Option<i64>,
    limit: Option<usize>,
}

async fn get_session_parts_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Query(query): Query<PartsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let store = state.session_store.read().await;
    let after = query.after.unwrap_or(-1);
    let limit = query.limit.unwrap_or(50).min(200);
    let parts = store
        .get_session_parts_after(&id, after, limit)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let has_more = parts.len() >= limit;
    let items: Vec<dto::session::SessionPartItem> = parts
        .into_iter()
        .map(|p| dto::session::SessionPartItem {
            part_id: p.part_id,
            part_index: p.part_index,
            kind: p.kind,
            tool_call_id: p.tool_call_id,
            tool_name: p.tool_name,
            status: p.status,
            payload: p.payload,
            projected_to_seq: p.projected_to_seq,
            updated_at: p.updated_at,
        })
        .collect();
    let cursor = dto::session::PartsCursor {
        after_part_index: items.last().map(|p| p.part_index),
        has_more,
        limit,
    };
    Ok(Json(dto::session::SessionPartsPage {
        session_id: id,
        parts: items,
        cursor,
    }))
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    after: Option<i64>,
    limit: Option<usize>,
}

async fn get_session_events_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let store = state.session_store.read().await;
    let after = query.after.unwrap_or(0);
    let limit = query.limit.unwrap_or(50).min(200);
    let events = store
        .get_session_events_after(&id, after)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let has_more = events.len() > limit;
    let page_events: Vec<dto::session::SessionEventItem> = events
        .into_iter()
        .take(limit)
        .map(|e| dto::session::SessionEventItem {
            id: e.id,
            seq: e.seq,
            event_type: e.event_type,
            timestamp_ms: e.timestamp_ms,
            payload: serde_json::from_str(&e.payload).unwrap_or_default(),
        })
        .collect();
    let cursor = dto::session::EventsCursor {
        after_seq: page_events.last().map(|e| e.seq),
        has_more,
        limit,
    };
    Ok(Json(dto::session::SessionEventsPage {
        session_id: id,
        events: page_events,
        cursor,
    }))
}

#[derive(Debug, Deserialize)]
struct RevertsQuery {
    limit: Option<usize>,
}

async fn get_session_reverts_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Query(query): Query<RevertsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let store = state.session_store.read().await;
    let limit = query.limit.unwrap_or(20).min(100);
    let records = store
        .list_session_reverts(&id, limit)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let reverts: Vec<dto::session::SessionRevertItem> = records
        .into_iter()
        .map(|record| dto::session::SessionRevertItem {
            id: record.id,
            operation: record.operation,
            status: record.status,
            message_id: record.message_id,
            target_part_id: record.target_part_id,
            part_ids: record.part_ids,
            checkpoint_ids: record.checkpoint_ids,
            paths: record.paths,
            restored_files: record.restored_files,
            removed_files: record.removed_files,
            errors: record.errors,
            diff_summary: record.diff_summary,
            snapshot_checkpoint_id: record.snapshot_checkpoint_id,
            created_at: record.created_at,
            unrevert_possible: record.unrevert_possible,
            unreverted: record.unreverted,
            payload: record.payload,
        })
        .collect();
    let total = reverts.len();
    Ok(Json(dto::session::SessionRevertsPage {
        session_id: id,
        reverts,
        total,
    }))
}

pub mod helpers;
pub mod tool_output;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod basic_tests;

#[cfg(test)]
mod contract_tests;
