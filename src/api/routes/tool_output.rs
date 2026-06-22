//! API route handlers for captured tool output.
//!
//! These endpoints expose bounded tool-output bodies and metadata without
//! requiring clients to inspect the on-disk output store directly.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::{dto, ApiError};

pub(crate) async fn get_tool_outputs_handler(
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let store = crate::tool_output_store::ToolOutputStore::new();
    let metas = store
        .list_for_session(&id)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let outputs: Vec<dto::tool_output::ToolOutputIndexEntry> = metas
        .into_iter()
        .map(|m| {
            let uri = m.uri();
            dto::tool_output::ToolOutputIndexEntry {
                id: m.id,
                uri,
                session_id: m.session_id,
                tool_call_id: m.tool_call_id,
                tool_name: m.tool_name,
                mime: m.mime,
                original_bytes: m.original_bytes,
                created_at_ms: m.created_at_ms,
            }
        })
        .collect();
    Ok(Json(dto::tool_output::ToolOutputIndex {
        session_id: id,
        outputs,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ToolOutputPageQuery {
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

pub(crate) async fn get_tool_output_page_handler(
    Path((session_id, output_id)): Path<(String, String)>,
    Query(query): Query<ToolOutputPageQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let store = crate::tool_output_store::ToolOutputStore::new();
    let page = store
        .read_page(
            &session_id,
            &format!("tool-output://{output_id}"),
            query.offset.unwrap_or(0),
            query.limit.unwrap_or(65536),
        )
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(dto::tool_output::ToolOutputPageDto {
        content: page.content,
        offset: page.offset,
        limit: page.limit,
        total_bytes: page.total_bytes,
        has_more: page.has_more,
    }))
}

pub(crate) async fn get_provider_status_handler(
    State(state): State<Arc<super::ApiState>>,
) -> Result<impl IntoResponse, ApiError> {
    let configured_max_output = state.config.read().await.api.max_tokens.map(u64::from);
    let latest_health = crate::diagnostics::provider_health::latest_provider_health_entry()
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let statuses = crate::api::provider_status::provider_product_statuses(
        &state,
        configured_max_output,
        latest_health.as_ref(),
    );
    let record_count = statuses.len();
    Ok(Json(dto::provider::ProviderStatusPage {
        statuses,
        record_count,
        timeout_effective: dto::provider::ProviderTimeoutEffectiveDto::from_env(),
    }))
}

pub(crate) async fn get_diagnostics_handler(
    State(state): State<Arc<super::ApiState>>,
    Query(query): Query<DiagnosticQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let session_id = query.session_id.unwrap_or_else(|| "unknown".to_string());
    let store = state.session_store.read().await;

    // Build a lightweight diagnostic snapshot from what's available
    let parts = store.get_session_parts(&session_id).unwrap_or_default();
    let revert_events = parts.iter().filter(|p| p.kind == "revert").count();
    let policy = crate::tool_output_store::ToolOutputPolicy::from_env();
    let capabilities =
        crate::services::api::provider_protocol::ProviderCapabilities::detect("auto", &state.model);
    let profile = crate::services::api::provider_protocol::ProviderRuntimeProfile::snapshot(
        &capabilities,
        &state.model,
        &state.model,
    );

    Ok(Json(dto::diagnostic::DiagnosticExportDto {
        schema: "diagnostic.v1".to_string(),
        session_id,
        model: state.model.clone(),
        provider: Some(state.model.clone()),
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        status: "snapshot".to_string(),
        turns: 0,
        tool_rounds: 0,
        changed_files: Vec::new(),
        verification_proof_status: None,
        evidence_category: None,
        evidence_items: 0,
        prompt_tokens: 0,
        completion_tokens: 0,
        cost_usd: 0.0,
        latency_ms: None,
        cache_miss_reason: None,
        failure_owner: None,
        failed_tool_names: Vec::new(),
        revert_events,
        provider_profile: Some(serde_json::json!({
            "provider_id": profile.provider_id,
            "model_id": profile.model_id,
            "protocol_family": profile.protocol_family.label(),
            "request_timeout_secs": profile.request_timeout_secs,
            "stream_idle_timeout_secs": profile.stream_idle_timeout_secs,
        })),
        tool_output_policy: Some(serde_json::json!({
            "max_bytes": policy.max_bytes,
            "max_lines": policy.max_lines,
            "preview_direction": format!("{:?}", policy.preview_direction),
            "retention_days": policy.retention_days,
        })),
    }))
}

#[derive(Debug, Deserialize)]
pub struct DiagnosticQuery {
    pub session_id: Option<String>,
}
