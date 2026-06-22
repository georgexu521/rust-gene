//! Bridge v1 compatibility routes.
//!
//! These handlers provide token-protected remote session and trigger endpoints.
//! They are compatibility shims over the normal API/session state and must not
//! bypass tenant prefix checks or bridge authentication.

use super::{helpers, ApiError, ApiState, ChatRequest, MessageInfo, SessionInfo};
use axum::{
    body::Body,
    extract::{Json, Path, Query, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub(super) struct BridgeCreateSessionRequest {
    prompt: String,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct BridgeRunTriggerRequest {
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

pub(super) async fn v1_list_sessions_handler(
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

pub(super) async fn v1_get_session_handler(
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

pub(super) async fn v1_get_session_status_handler(
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

pub(super) async fn v1_get_session_messages_handler(
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

pub(super) async fn v1_create_session_handler(
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

pub(super) async fn v1_run_trigger_handler(
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

pub(super) async fn bridge_auth_middleware(req: Request<Body>, next: Next) -> Response {
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
