use crate::engine::intent_router::{
    IntentKind, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSet {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scenarios: Vec<EvalScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalScenario {
    pub id: String,
    pub prompt: String,
    #[serde(default)]
    pub replay: EvalReplay,
    #[serde(default)]
    pub expect: EvalExpect,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalExpect {
    pub intent: Option<IntentKind>,
    pub workflow: Option<WorkflowKind>,
    pub retrieval: Option<RetrievalPolicy>,
    pub reasoning: Option<ReasoningPolicy>,
    pub risk: Option<RiskLevel>,
    pub min_confidence: Option<f32>,
    #[serde(default)]
    pub recommended_tools: Vec<String>,
    #[serde(default)]
    pub forbidden_tools: Vec<String>,
    #[serde(default)]
    pub trace_events: Vec<String>,
    #[serde(default)]
    pub tool_sequence: Vec<String>,
    pub failed_tool: Option<String>,
    pub verification_passed: Option<bool>,
    pub reflection_status: Option<String>,
    pub repair_required: Option<bool>,
    pub permission_approved: Option<bool>,
    pub permission_decision: Option<String>,
    pub permission_persistence_scope: Option<String>,
    pub recovery_category: Option<String>,
    pub recovery_suggested_command: Option<String>,
    pub recovery_safe_retry: Option<bool>,
    pub terminal_task_count: Option<usize>,
    pub terminal_task_id: Option<String>,
    pub terminal_task_status: Option<String>,
    pub terminal_task_read_tool: Option<String>,
    pub terminal_task_cancel_tool: Option<String>,
    pub terminal_task_output_path: Option<String>,
    pub backgrounded_tool: Option<String>,
    pub file_checkpoint_count: Option<usize>,
    pub file_checkpoint_id: Option<String>,
    pub file_change_id: Option<String>,
    pub file_checkpoint_path: Option<String>,
    pub rewind_target: Option<String>,
    pub rewind_command: Option<String>,
    pub rewind_checkpoint_id: Option<String>,
    pub rewind_restored_files: Option<usize>,
    pub context_compaction_count: Option<usize>,
    pub context_boundary_id: Option<String>,
    pub context_compaction_strategy: Option<String>,
    pub context_before_tokens: Option<usize>,
    pub context_after_tokens: Option<usize>,
    pub context_preserved_tail_count: Option<usize>,
    pub runtime_diet_total_request_tokens: Option<u64>,
    pub runtime_diet_remaining_context_tokens: Option<u64>,
    pub runtime_diet_route_scoped_tools: Option<bool>,
    pub runtime_diet_workflow_context: Option<String>,
    pub subagent_count: Option<usize>,
    pub subagent_agent_id: Option<String>,
    pub subagent_profile: Option<String>,
    pub subagent_role: Option<String>,
    pub subagent_status: Option<String>,
    pub subagent_context_mode: Option<String>,
    pub subagent_allowed_tools: Option<usize>,
    pub isolated_worktree_path: Option<String>,
    pub isolated_worktree_branch: Option<String>,
    pub recursive_fork_guard: Option<bool>,
    pub fork_placeholder_complete: Option<bool>,
    pub fork_message_count: Option<usize>,
    pub agent_worktree_action_count: Option<usize>,
    pub agent_worktree_review_command: Option<String>,
    pub agent_worktree_merge_command: Option<String>,
    pub agent_worktree_cleanup_command: Option<String>,
    pub agent_worktree_review_status: Option<String>,
    pub agent_worktree_merge_status: Option<String>,
    pub agent_worktree_cleanup_status: Option<String>,
    pub agent_worktree_merge_kind: Option<String>,
    pub agent_worktree_cleanup_deleted_branch: Option<bool>,
    pub mcp_resource_count: Option<usize>,
    pub mcp_resource_success_count: Option<usize>,
    pub mcp_resource_failure_count: Option<usize>,
    pub mcp_resource_server: Option<String>,
    pub mcp_resource_uri: Option<String>,
    pub mcp_resource_action: Option<String>,
    pub mcp_resource_success: Option<bool>,
    pub mcp_resource_content_chars: Option<usize>,
    pub mcp_repair_count: Option<usize>,
    pub mcp_repair_server: Option<String>,
    pub mcp_repair_category: Option<String>,
    pub mcp_repair_command: Option<String>,
    pub mcp_repair_status: Option<String>,
    pub mcp_panel_command: Option<String>,
    pub context_attachment_count: Option<usize>,
    pub context_attachment_type: Option<String>,
    pub context_attachment_label: Option<String>,
    pub context_attachment_file: Option<String>,
    pub context_attachment_patch_preview_min_chars: Option<usize>,
    #[serde(default)]
    pub available_tools: Vec<String>,
    #[serde(default)]
    pub unavailable_tools: Vec<String>,
    #[serde(default)]
    pub available_commands: Vec<String>,
    #[serde(default)]
    pub placeholder_commands: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub agent_profiles: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalReplay {
    #[serde(default)]
    pub tool_calls: Vec<EvalToolCall>,
    #[serde(default)]
    pub workflow_judgment: bool,
    pub acceptance_review_accepted: Option<bool>,
    #[serde(default)]
    pub guided_debugging: bool,
    pub verification_passed: Option<bool>,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub failed_commands: Vec<String>,
    #[serde(default)]
    pub recovery_plans: Vec<EvalRecoveryPlan>,
    #[serde(default)]
    pub terminal_tasks: Vec<EvalTerminalTaskReplay>,
    #[serde(default)]
    pub file_changes: Vec<EvalFileChangeReplay>,
    #[serde(default)]
    pub rewind: Option<EvalRewindReplay>,
    #[serde(default)]
    pub context_compactions: Vec<EvalContextCompactionReplay>,
    #[serde(default)]
    pub runtime_diet: Option<EvalRuntimeDietReplay>,
    #[serde(default)]
    pub subagents: Vec<EvalSubagentReplay>,
    #[serde(default)]
    pub agent_worktree_actions: Vec<EvalAgentWorktreeActionReplay>,
    #[serde(default)]
    pub mcp_resources: Vec<EvalMcpResourceReplay>,
    #[serde(default)]
    pub mcp_repairs: Vec<EvalMcpRepairReplay>,
    #[serde(default)]
    pub run_contexts: Vec<EvalRunContextReplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalToolCall {
    pub tool: String,
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub permission: Option<EvalPermissionReplay>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalPermissionReplay {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub persistence_scope: Option<String>,
    #[serde(default)]
    pub rule_pattern: Option<String>,
    #[serde(default)]
    pub persisted_path: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalRecoveryPlan {
    pub source: String,
    pub category: String,
    #[serde(default)]
    pub failure_type: String,
    #[serde(default)]
    pub recovery_kind: String,
    pub action: String,
    #[serde(default = "default_true")]
    pub retryable: bool,
    #[serde(default)]
    pub safe_retry: bool,
    #[serde(default)]
    pub allowed_alternatives: Vec<String>,
    #[serde(default)]
    pub retry_budget: Option<usize>,
    #[serde(default)]
    pub side_effect_uncertain: bool,
    #[serde(default)]
    pub requires_user_decision: bool,
    #[serde(default)]
    pub suggested_command: Option<String>,
    #[serde(default = "default_recovery_status")]
    pub status: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalTerminalTaskReplay {
    pub id: String,
    #[serde(default)]
    pub source_tool: String,
    #[serde(default = "default_running_status")]
    pub status: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub handle: Option<String>,
    #[serde(default)]
    pub read_tool: Option<String>,
    #[serde(default)]
    pub cancel_tool: Option<String>,
    #[serde(default)]
    pub cancel_handle: Option<String>,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub backgrounded: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalRunContextReplay {
    #[serde(rename = "type")]
    pub context_type: String,
    pub label: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub patch_preview: String,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalFileChangeReplay {
    pub id: String,
    pub checkpoint_id: String,
    pub path: String,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub existed_before: bool,
    #[serde(default)]
    pub before_hash: Option<String>,
    #[serde(default)]
    pub after_hash: Option<String>,
    #[serde(default)]
    pub diff: Option<String>,
    #[serde(default)]
    pub bytes_written: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalRewindReplay {
    pub target: String,
    pub checkpoint_id: String,
    #[serde(default = "default_rewind_command")]
    pub command: String,
    #[serde(default)]
    pub restored_files: Vec<String>,
    #[serde(default)]
    pub removed_files: Vec<String>,
    #[serde(default)]
    pub failed_files: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalContextCompactionReplay {
    pub before_tokens: usize,
    pub after_tokens: usize,
    pub strategy: String,
    #[serde(default)]
    pub boundary_id: Option<String>,
    #[serde(default)]
    pub sequence: Option<u32>,
    #[serde(default)]
    pub messages_before: Option<usize>,
    #[serde(default)]
    pub messages_after: Option<usize>,
    #[serde(default)]
    pub preserved_tail_count: Option<usize>,
    #[serde(default)]
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRuntimeDietReplay {
    pub prompt_tokens: u64,
    pub tool_schema_tokens: u64,
    #[serde(default)]
    pub total_request_tokens: u64,
    #[serde(default)]
    pub max_context_tokens: Option<u64>,
    #[serde(default)]
    pub remaining_context_tokens: Option<u64>,
    #[serde(default)]
    pub tool_result_chars: usize,
    #[serde(default)]
    pub tool_result_tokens: u64,
    #[serde(default)]
    pub truncated_tool_results: usize,
    #[serde(default)]
    pub tool_result_artifacts: usize,
    pub exposed_tools: usize,
    #[serde(default)]
    pub memory_snapshot_chars: usize,
    #[serde(default)]
    pub memory_snapshot_tokens: u64,
    #[serde(default)]
    pub retrieval_items: usize,
    #[serde(default)]
    pub retrieval_tokens: u64,
    #[serde(default)]
    pub skill_list_chars: usize,
    #[serde(default)]
    pub skill_list_tokens: u64,
    #[serde(default = "default_true")]
    pub route_scoped_tools: bool,
    #[serde(default = "default_workflow_context")]
    pub workflow_context: String,
    #[serde(default = "default_closeout_visibility")]
    pub closeout_visibility: String,
    #[serde(default = "default_validation_evidence")]
    pub validation_evidence: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSubagentReplay {
    pub agent_id: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default = "default_agent_role")]
    pub role: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_agent_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub allowed_tools: usize,
    #[serde(default = "default_agent_status")]
    pub status: String,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub output_chars: usize,
    #[serde(default)]
    pub tools_used: usize,
    #[serde(default)]
    pub context_mode: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub worktree_branch: Option<String>,
    #[serde(default)]
    pub recursive_fork_guard: bool,
    #[serde(default)]
    pub placeholder_complete: bool,
    #[serde(default)]
    pub fork_message_count: Option<usize>,
    #[serde(default)]
    pub parent_tool_call_ids: Vec<String>,
    #[serde(default)]
    pub cleanup_hooks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalAgentWorktreeActionReplay {
    pub action: String,
    pub agent_id: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default = "default_action_status")]
    pub status: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub commits_ahead: Option<usize>,
    #[serde(default)]
    pub merge_kind: Option<String>,
    #[serde(default)]
    pub cleanup: bool,
    #[serde(default)]
    pub delete_branch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalMcpResourceReplay {
    pub server: String,
    #[serde(default = "default_mcp_uri")]
    pub uri: String,
    #[serde(default = "default_mcp_resource_action")]
    pub action: String,
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub content_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalMcpRepairReplay {
    pub server: String,
    pub category: String,
    pub command: String,
    #[serde(default = "default_mcp_panel_command")]
    pub panel_command: String,
    #[serde(default = "default_recovery_status")]
    pub status: String,
    #[serde(default)]
    pub safe_retry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub set_name: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<EvalFailure>,
}

impl EvalReport {
    pub fn ok(&self) -> bool {
        self.failed == 0
    }

    pub fn summary(&self) -> String {
        let mut out = format!(
            "EvalSet {}: {}/{} passed",
            self.set_name, self.passed, self.total
        );
        if !self.failures.is_empty() {
            out.push_str("\nFailures:");
            for failure in &self.failures {
                out.push_str(&format!("\n- {}: {}", failure.scenario_id, failure.message));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalFailure {
    pub scenario_id: String,
    pub message: String,
}

fn default_true() -> bool {
    true
}

fn default_recovery_status() -> String {
    "Planned".to_string()
}

fn default_running_status() -> String {
    "running".to_string()
}

fn default_rewind_command() -> String {
    "/rewind".to_string()
}

fn default_workflow_context() -> String {
    "normal".to_string()
}

fn default_closeout_visibility() -> String {
    "standard".to_string()
}

fn default_validation_evidence() -> String {
    "none".to_string()
}

fn default_agent_role() -> String {
    "specialist".to_string()
}

fn default_agent_timeout_secs() -> u64 {
    120
}

fn default_agent_status() -> String {
    "completed".to_string()
}

fn default_action_status() -> String {
    "success".to_string()
}

fn default_mcp_uri() -> String {
    "*".to_string()
}

fn default_mcp_resource_action() -> String {
    "read".to_string()
}

fn default_mcp_panel_command() -> String {
    "/panel mcp".to_string()
}
