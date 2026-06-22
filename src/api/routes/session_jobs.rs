//! API route handlers for session job records.
//!
//! Session jobs are read-only API projections of background or long-running
//! work associated with a session.

use axum::{extract::Path, extract::State, response::IntoResponse, Json};
use std::sync::Arc;

use crate::api::{dto, routes::ApiError, state::ApiState};

pub(super) async fn get_session_jobs_handler(
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
