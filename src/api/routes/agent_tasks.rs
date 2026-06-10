use axum::{extract::Path, extract::State, response::IntoResponse, Json};
use serde::Serialize;
use std::sync::Arc;

use super::ApiError;
use crate::api::state::ApiState;

// ── Agent Tasks (Subagent Projection) ────────────────────

pub(super) async fn get_agent_tasks_handler(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let store = state.session_store.read().await;
    let rows = store.recent_agent_task_states(&id, 50).unwrap_or_default();
    let tasks: Vec<AgentTaskDto> = rows
        .into_iter()
        .map(|r| AgentTaskDto {
            task_id: r.task_id,
            agent_id: r.agent_id,
            profile: r.profile.unwrap_or_default(),
            status: r.status,
            description: r.description,
            created_at: Some(r.created_at),
            updated_at: Some(r.updated_at),
        })
        .collect();
    Ok(Json(serde_json::json!({
        "session_id": id,
        "total": tasks.len(),
        "tasks": tasks,
    })))
}

pub(super) async fn get_agent_task_handler(
    State(state): State<Arc<ApiState>>,
    Path((session_id, task_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let store = state.session_store.read().await;
    match store.agent_task_state(&session_id, &task_id) {
        Ok(Some(r)) => Ok(Json(serde_json::json!({
            "task_id": r.task_id,
            "agent_id": r.agent_id,
            "profile": r.profile,
            "role": r.role,
            "status": r.status,
            "description": r.description,
            "created_at": r.created_at,
            "updated_at": r.updated_at,
        }))),
        Ok(None) => Err(ApiError::NotFound("agent task not found".to_string())),
        Err(e) => Err(ApiError::Internal(e.to_string())),
    }
}

#[derive(Debug, Serialize)]
struct AgentTaskDto {
    task_id: String,
    agent_id: String,
    profile: String,
    status: String,
    description: String,
    created_at: Option<String>,
    updated_at: Option<String>,
}
