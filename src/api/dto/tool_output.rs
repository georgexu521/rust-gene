//! Tool-output DTOs — shared types for stored tool-output paging.

use serde::{Deserialize, Serialize};

/// Index entry for a stored tool output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutputIndexEntry {
    pub id: String,
    pub uri: String,
    pub session_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub mime: String,
    pub original_bytes: u64,
    pub created_at_ms: u64,
}

/// Tool-output index page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutputIndex {
    pub session_id: String,
    pub outputs: Vec<ToolOutputIndexEntry>,
}

/// A single page of stored output content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutputPageDto {
    pub content: String,
    pub offset: u64,
    pub limit: u64,
    pub total_bytes: u64,
    pub has_more: bool,
}

/// Active tool-output policy summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutputPolicyDto {
    pub max_bytes: usize,
    pub max_lines: usize,
    pub preview_direction: String,
    pub retention_days: u32,
}
