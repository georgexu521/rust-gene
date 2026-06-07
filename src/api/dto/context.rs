//! Active context DTO — exposes what the model will see for a session.
//!
//! Slice B of the opencode programming parity plan.

use serde::{Deserialize, Serialize};

/// Active context view for a session after latest compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContextDto {
    pub session_id: String,
    pub compact_boundary_id: Option<String>,
    pub estimated_history_tokens: u64,
    pub tool_schema_tokens: u64,
    pub memory_snapshot_tokens: u64,
    pub stable_prefix_hash: Option<String>,
    pub dynamic_tail_hash: Option<String>,
    pub latest_compaction: Option<CompactionSummaryDto>,
    pub message_count_after_compaction: usize,
}

/// Summary of the latest compaction attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionSummaryDto {
    pub boundary_id: String,
    pub strategy: String,
    pub trigger: String,
    pub before_tokens: u64,
    pub after_tokens: u64,
    pub messages_before: usize,
    pub messages_after: usize,
    pub preserved_tail_count: usize,
}
