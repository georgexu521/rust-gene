//! Session DTOs — shared vocabulary for TUI, desktop, and API.
//!
//! These are the canonical product-facing types for session state.
//! Frontends consume these DTOs rather than re-interpreting raw
//! `session_events` payloads.

use serde::{Deserialize, Serialize};

/// Lightweight session info for listing and status display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub model: String,
    pub parent_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
}

/// Cursor-based page of projected session parts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPartsPage {
    pub session_id: String,
    pub parts: Vec<SessionPartItem>,
    pub cursor: PartsCursor,
}

/// Single projected part in a cursor page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPartItem {
    pub part_id: String,
    pub part_index: i64,
    pub kind: String,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub status: Option<String>,
    pub payload: serde_json::Value,
    pub projected_to_seq: i64,
    pub updated_at: String,
}

/// Cursor metadata for paginated part reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartsCursor {
    pub after_part_index: Option<i64>,
    pub has_more: bool,
    pub limit: usize,
}

/// Cursor-based page of raw session events (audit/debug).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEventsPage {
    pub session_id: String,
    pub events: Vec<SessionEventItem>,
    pub cursor: EventsCursor,
}

/// Single raw event in a cursor page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEventItem {
    pub id: i64,
    pub seq: i64,
    pub event_type: String,
    pub timestamp_ms: i64,
    pub payload: serde_json::Value,
}

/// Cursor metadata for paginated event reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsCursor {
    pub after_seq: Option<i64>,
    pub has_more: bool,
    pub limit: usize,
}

/// Session revert history item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRevertItem {
    pub id: i64,
    pub operation: String,
    pub status: String,
    pub message_id: Option<String>,
    pub target_part_id: Option<String>,
    pub part_ids: Vec<String>,
    pub checkpoint_ids: Vec<String>,
    pub paths: Vec<String>,
    pub restored_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub errors: Vec<String>,
    pub diff_summary: Option<String>,
    pub snapshot_checkpoint_id: Option<String>,
    pub created_at: String,
    pub unrevert_possible: bool,
    pub unreverted: bool,
    pub payload: serde_json::Value,
}

/// Page of session revert events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRevertsPage {
    pub session_id: String,
    pub reverts: Vec<SessionRevertItem>,
    pub total: usize,
}
