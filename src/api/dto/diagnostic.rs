//! Diagnostic DTOs — shared types for diagnostic exports and status.

use serde::{Deserialize, Serialize};

/// Diagnostic export summary for API consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticExportDto {
    pub schema: String,
    pub session_id: String,
    pub model: String,
    pub provider: Option<String>,
    pub timestamp_ms: u64,
    pub status: String,
    pub turns: usize,
    pub tool_rounds: usize,
    pub changed_files: Vec<String>,
    pub verification_proof_status: Option<String>,
    pub evidence_category: Option<String>,
    pub evidence_items: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cost_usd: f64,
    pub latency_ms: Option<u64>,
    pub cache_miss_reason: Option<String>,
    pub failure_owner: Option<String>,
    pub failed_tool_names: Vec<String>,
    pub revert_events: usize,
    pub provider_profile: Option<serde_json::Value>,
    pub tool_output_policy: Option<serde_json::Value>,
}
