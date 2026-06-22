//! API route handlers for persisted agent task state.
//!
//! These handlers expose read-only task lists and task detail snapshots backed
//! by `SessionStore` projection data.

use axum::{extract::Path, extract::State, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::Value;
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
        .map(|r| {
            let artifact = r
                .result_artifact_id
                .and_then(|artifact_id| store.agent_artifact(&id, artifact_id).ok().flatten());
            let payload = artifact
                .as_ref()
                .map(|artifact| &artifact.payload)
                .unwrap_or(&r.payload);
            AgentTaskDto {
                task_id: r.task_id,
                agent_id: r.agent_id,
                profile: r.profile.unwrap_or_default(),
                role: r.role,
                status: r.status,
                description: r.description,
                child_session_id: r
                    .payload
                    .get("child_session_id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                result_artifact_id: r.result_artifact_id,
                artifact_status: artifact.as_ref().map(|artifact| artifact.status.clone()),
                tools_used: string_array_field(payload, "tools_used")
                    .or_else(|| string_array_field(&r.payload, "tools_used"))
                    .unwrap_or_default(),
                proof_kind: r
                    .payload
                    .get("proof_kind")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                completion_sink: payload
                    .get("completion_sink")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .or_else(|| {
                        r.payload
                            .get("completion_sink")
                            .and_then(|value| value.as_str())
                            .map(str::to_string)
                    }),
                recovery_status: r
                    .payload
                    .get("recovery_status")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                created_at: Some(r.created_at),
                updated_at: Some(r.updated_at),
            }
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
        Ok(Some(r)) => {
            let artifact = r.result_artifact_id.and_then(|artifact_id| {
                store
                    .agent_artifact(&session_id, artifact_id)
                    .ok()
                    .flatten()
            });
            let artifact_payload = artifact
                .as_ref()
                .map(|artifact| &artifact.payload)
                .unwrap_or(&r.payload);
            Ok(Json(serde_json::json!({
                "task_id": r.task_id,
                "agent_id": r.agent_id,
                "profile": r.profile,
                "role": r.role,
                "status": r.status,
                "description": r.description,
                "child_session_id": r.payload.get("child_session_id").and_then(|value| value.as_str()),
                "result_artifact_id": r.result_artifact_id,
                "artifact_status": artifact.as_ref().map(|artifact| artifact.status.clone()),
                "tools_used": string_array_field(artifact_payload, "tools_used")
                    .or_else(|| string_array_field(&r.payload, "tools_used"))
                    .unwrap_or_default(),
                "proof_kind": r.payload.get("proof_kind").and_then(|value| value.as_str()),
                "completion_sink": artifact_payload.get("completion_sink").and_then(|value| value.as_str())
                    .or_else(|| r.payload.get("completion_sink").and_then(|value| value.as_str())),
                "recovery_status": r.payload.get("recovery_status").and_then(|value| value.as_str()),
                "recovery_action": r.payload.get("recovery_action").and_then(|value| value.as_str()),
                "created_at": r.created_at,
                "updated_at": r.updated_at,
            })))
        }
        Ok(None) => Err(ApiError::NotFound("agent task not found".to_string())),
        Err(e) => Err(ApiError::Internal(e.to_string())),
    }
}

fn string_array_field(value: &Value, field: &str) -> Option<Vec<String>> {
    Some(
        value
            .get(field)?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    )
}

#[derive(Debug, Serialize)]
struct AgentTaskDto {
    task_id: String,
    agent_id: String,
    profile: String,
    role: String,
    status: String,
    description: String,
    child_session_id: Option<String>,
    result_artifact_id: Option<i64>,
    artifact_status: Option<String>,
    tools_used: Vec<String>,
    proof_kind: Option<String>,
    completion_sink: Option<String>,
    recovery_status: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}
