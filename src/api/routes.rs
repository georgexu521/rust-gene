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
        // Chat API
        .route("/api/chat", post(chat_handler))
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
}

#[derive(Debug, Serialize)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

async fn chat_handler(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<ChatRequest>,
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
    if !api_tool_call_allowed(&req.tool, &req.params) {
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

fn sanitize_tenant_id(raw: &str) -> String {
    let mut s = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            s.push(ch);
        } else {
            s.push('_');
        }
    }
    if s.is_empty() {
        "default".to_string()
    } else {
        s
    }
}

fn tenant_prefix(headers: &HeaderMap) -> String {
    let tenant = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("default");
    format!("tenant_{}_", sanitize_tenant_id(tenant))
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(ch);
    }
    out
}

fn parse_token_list(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c == ';' || c.is_ascii_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn configured_bridge_tokens() -> Vec<String> {
    if let Ok(list) = std::env::var("PRIORITY_AGENT_BRIDGE_TOKENS") {
        let parsed = parse_token_list(&list);
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
    let prefix = tenant_prefix(&headers);

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
    let prefix = tenant_prefix(&headers);
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
    let prefix = tenant_prefix(&headers);
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
    let prefix = tenant_prefix(&headers);
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
    let prefix = tenant_prefix(&headers);
    let session_id = format!("{}{}", prefix, uuid::Uuid::new_v4().simple());
    let title = req
        .title
        .unwrap_or_else(|| format!("Remote: {}", truncate_chars(req.prompt.trim(), 64)));
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
    let prefix = tenant_prefix(&headers);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::{
        ChatRequest as LlmChatRequest, ChatResponse as LlmChatResponse, LlmProvider, Usage,
    };
    use async_openai::types::ChatCompletionResponseStream;
    use axum::body::to_bytes;
    use once_cell::sync::Lazy;
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
    fn test_sanitize_tenant_id() {
        assert_eq!(sanitize_tenant_id("team-a"), "team-a");
        assert_eq!(sanitize_tenant_id("A/B C"), "A_B_C");
        assert_eq!(sanitize_tenant_id(""), "default");
    }

    #[test]
    fn test_truncate_chars() {
        assert_eq!(truncate_chars("hello", 3), "hel");
        assert_eq!(truncate_chars("你好世界", 2), "你好");
    }

    #[test]
    fn test_parse_token_list() {
        let tokens = parse_token_list("a,b; c  d");
        assert_eq!(tokens, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn test_api_tool_allowlist_uses_registered_read_only_names() {
        assert!(api_tool_call_allowed(
            "file_read",
            &json!({"path": "src/main.rs"})
        ));
        assert!(api_tool_call_allowed("git_status", &json!({})));
        assert!(api_tool_call_allowed("git_diff", &json!({})));
        assert!(api_tool_call_allowed(
            "json_query",
            &json!({"input": "{}", "query": "."})
        ));
        assert!(api_tool_call_allowed(
            "task_output",
            &json!({"task_id": "task_1"})
        ));
        assert!(api_tool_call_allowed(
            "task_output",
            &json!({"task_id": "task_1", "action": "get"})
        ));

        assert!(!api_tool_call_allowed("git_read", &json!({})));
        assert!(!api_tool_call_allowed("json_tool", &json!({})));
        assert!(!api_tool_call_allowed(
            "browser",
            &json!({"action": "evaluate_js"})
        ));
        assert!(!api_tool_call_allowed("bash", &json!({"command": "pwd"})));
        assert!(!api_tool_call_allowed(
            "task_output",
            &json!({"task_id": "task_1", "action": "append", "line": "x"})
        ));
    }

    #[tokio::test]
    async fn test_audit_summary_contains_structured_coding_quality() {
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

        {
            let mut tracker = state.audit_tracker.write().await;
            tracker.record_coding_round(false);
            tracker.record_coding_round(true);
        }

        let app = create_routes(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/audit/summary")
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
                    .uri("/api/workflow/metrics/weekly?limit=4")
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
                    .uri("/api/workflow/metrics/calibration/weekly?limit=4")
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
        assert!(
            value["generated_at"].as_str().is_some(),
            "generated_at should be present"
        );
        assert!(value["weeks"].is_array(), "weeks should be array");
    }
}
