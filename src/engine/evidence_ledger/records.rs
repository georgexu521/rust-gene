//! Evidence ledger support.
//!
//! Stores structured proof records that closeout and diagnostics can inspect later.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionRecord {
    pub call_id: String,
    pub tool: String,
    pub operation_kind: Option<String>,
    pub read_only: Option<bool>,
    pub concurrency_safe: Option<bool>,
    pub destructive: Option<bool>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub search_hint: Option<String>,
    #[serde(default)]
    pub should_defer: Option<bool>,
    #[serde(default)]
    pub always_load: Option<bool>,
    #[serde(default)]
    pub strict_schema: Option<bool>,
    #[serde(default)]
    pub interrupt_behavior: Option<String>,
    #[serde(default)]
    pub requires_user_interaction: Option<bool>,
    #[serde(default)]
    pub open_world: Option<bool>,
    #[serde(default)]
    pub search_or_read: ToolSearchOrReadRecord,
    #[serde(default)]
    pub input_paths: Vec<String>,
    #[serde(default)]
    pub permission_matcher_input: Option<String>,
    #[serde(default)]
    pub transcript_summary: Option<String>,
    #[serde(default)]
    pub ui_render_kind: Option<String>,
    pub status: ToolExecutionStatus,
    pub arguments_hash: String,
    pub duration_ms: Option<u64>,
    pub output_chars: usize,
    pub output: ToolOutputMetadataRecord,
    pub error_code: Option<String>,
    pub error_preview: Option<String>,
    pub permission: Option<ToolPermissionRecord>,
    pub command: Option<String>,
    #[serde(default)]
    pub normalized_command: Option<String>,
    pub command_kind: Option<String>,
    pub command_category: Option<String>,
    pub validation_family: Option<String>,
    #[serde(default)]
    pub path_patterns: Vec<String>,
    #[serde(default)]
    pub absolute_path_patterns: Vec<String>,
    pub safe_for_closeout: Option<bool>,
    #[serde(default)]
    pub network_access: Option<bool>,
    #[serde(default)]
    pub external_path_access: Option<bool>,
    #[serde(default)]
    pub compound_command: Option<bool>,
    #[serde(default)]
    pub shell_control_operators: Vec<String>,
    #[serde(default)]
    pub risky_shell_wrapper: Option<bool>,
    #[serde(default)]
    pub expected_silent_output: Option<bool>,
    #[serde(default)]
    pub permission_rule_suggestions: Vec<serde_json::Value>,
    pub terminal_task: Option<TerminalTaskRecord>,
    pub changed_paths: Vec<String>,
    pub file_evidence: Vec<ToolFileEvidenceLink>,
    pub relevance: ToolExecutionRelevance,
    pub execution: ToolExecutionContextRecord,
    /// How this record entered the evidence ledger.
    ///
    /// `compacted_summary` records come from LLM compaction and must not be
    /// treated as raw verification proof by closeout.
    #[serde(default)]
    pub source_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Completed,
    Failed,
    Denied,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSearchOrReadRecord {
    pub is_search: bool,
    pub is_read: bool,
    pub is_list: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolPermissionRecord {
    pub kind: Option<String>,
    pub approved: bool,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub patterns: Vec<String>,
    pub allowed_always_rules: Vec<String>,
    pub source: ToolPermissionSourceRecord,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolPermissionSourceRecord {
    pub permission_source: Option<String>,
    pub resolved_permission_source: Option<String>,
    pub permission_requires: Option<bool>,
    pub tool_requires: Option<bool>,
    pub raw_tool_requires: Option<bool>,
    pub drift_requires_approval: Option<bool>,
    pub permission_family: Option<String>,
    pub permission_decision: Option<String>,
    pub risk_level: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolOutputMetadataRecord {
    pub data_keys: Vec<String>,
    pub summary_keys: Vec<String>,
    pub display_format: Option<String>,
    pub truncated: Option<bool>,
    pub file_count: Option<usize>,
    pub diagnostics: Option<ToolDiagnosticsMetadataRecord>,
    pub shell_status: Option<String>,
    pub shell_evidence_status: Option<String>,
    pub shell_exit_code: Option<i64>,
    pub shell_background: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolDiagnosticsMetadataRecord {
    pub status: Option<String>,
    pub checked: Option<bool>,
    pub diagnostic_count: Option<u64>,
    pub error_count: Option<u64>,
    pub warning_count: Option<u64>,
    pub first_error_line: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolFileEvidenceLink {
    pub fact_index: usize,
    pub path: Option<String>,
    pub kind: Option<String>,
    pub line_start: Option<u64>,
    pub line_end: Option<u64>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalTaskRecord {
    pub task_id: Option<String>,
    pub status: Option<String>,
    pub terminal_kind: Option<String>,
    pub handle: Option<String>,
    pub output_path: Option<String>,
    pub duration_ms: Option<u64>,
    pub exit_code: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionRelevance {
    pub validation: bool,
    pub closeout: bool,
    pub repair: bool,
    pub policy: ToolExecutionRelevancePolicyRecord,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionRelevancePolicyRecord {
    pub route_workflow: Option<String>,
    pub closeout_reasons: Vec<String>,
    pub repair_reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionContextRecord {
    pub route: Option<ToolRouteRecord>,
    pub policy: Option<ToolExecutionPolicyRecord>,
    pub parallel: bool,
    pub pre_executed: bool,
    pub action_checkpoint_active: bool,
    pub has_changes_before_tools: bool,
    pub exposed_tools_count: Option<usize>,
    pub started_at_unix_ms: Option<u64>,
    pub finished_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolRouteRecord {
    pub intent: Option<String>,
    pub workflow: Option<String>,
    pub retrieval: Option<String>,
    pub reasoning: Option<String>,
    pub risk: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionPolicyRecord {
    pub latency: Option<String>,
    pub parallelism_limit: Option<usize>,
    pub max_tool_calls: Option<usize>,
    pub context_budget_tokens: Option<usize>,
    pub allow_fallback_model: Option<bool>,
    pub cost_ceiling_usd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEvidence {
    pub tool: String,
    pub path: Option<String>,
    pub success: bool,
    pub kind: Option<String>,
    pub line_start: Option<u64>,
    pub line_end: Option<u64>,
    pub total_lines: Option<u64>,
    pub displayed_lines: Option<u64>,
    pub truncated: Option<bool>,
    pub content_hash: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandEvidence {
    pub command: String,
    #[serde(default)]
    pub normalized_command: String,
    pub success: bool,
    pub command_kind: Option<String>,
    pub command_category: Option<String>,
    pub validation_family: Option<String>,
    #[serde(default)]
    pub path_patterns: Vec<String>,
    #[serde(default)]
    pub absolute_path_patterns: Vec<String>,
    pub safe_for_closeout: bool,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub external_path_access: bool,
    #[serde(default)]
    pub compound_command: bool,
    #[serde(default)]
    pub shell_control_operators: Vec<String>,
    #[serde(default)]
    pub risky_shell_wrapper: bool,
    #[serde(default)]
    pub expected_silent_output: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationEvidence {
    pub source: String,
    pub command: Option<String>,
    pub passed: bool,
    pub summary: String,
    #[serde(default)]
    pub proof_kind: Option<VerificationProofKind>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub command_status: Option<String>,
    #[serde(default)]
    pub validation_family: Option<String>,
    #[serde(default)]
    pub source_agent: Option<String>,
    #[serde(default)]
    pub parent_verified: Option<bool>,
    #[serde(default)]
    pub related_to_changed_files: Option<String>,
    #[serde(default)]
    pub residual_risk: Option<String>,
    #[serde(default)]
    pub claim_id: Option<String>,
    #[serde(default)]
    pub claim_type: Option<String>,
    #[serde(default)]
    pub parent_command: Option<String>,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
    #[serde(default)]
    pub verification_verdict: Option<String>,
    #[serde(default)]
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionEvidence {
    pub tool: String,
    pub kind: Option<String>,
    pub approved: bool,
    #[serde(default)]
    pub source: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceSnapshot {
    pub changed_files: Vec<String>,
    pub tool_execution_records: usize,
    pub file_facts: usize,
    pub command_facts: usize,
    pub validation_facts: usize,
    pub passed_validation_facts: usize,
    pub failed_validation_facts: usize,
    pub permission_facts: usize,
    pub denied_permission_facts: usize,
}
