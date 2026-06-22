//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::runtime_diet::RuntimeDietSnapshot;
use super::tool_call_lifecycle::ToolCallLifecycle;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::intent_router::IntentRoute;
use crate::engine::repair::storm::StormState;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::route_recovery::RouteRecoveryRuntimeState;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::AgentTaskStage;
use crate::engine::trace::TraceCollector;
use crate::services::api::ToolCall;
use crate::tools::ToolContextRetainedContext;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// TurnLoopState / TurnLoopStateController
// ---------------------------------------------------------------------------

#[derive(Default)]
pub(super) struct TurnLoopState {
    pub(super) final_content: String,
    pub(super) final_tool_calls: Vec<ToolCall>,
    pub(super) tool_calls_made: bool,
    pub(super) tools_used: Vec<String>,
    pub(super) pseudo_tool_retry_used: bool,
    pub(super) filesystem_grounding_retry_used: bool,
    pub(super) continuation_retry_used: bool,
    /// Post-tool empty nudge: when the model returns empty content after
    /// having already executed tool calls, inject one "keep going" nudge.
    pub(super) post_tool_empty_retry_used: bool,
    /// Claim gate repair: tracks whether a final-answer claim-gate repair
    /// has already been used for this turn to prevent infinite loops.
    pub(super) claim_gate_repair_used: bool,
    pub(super) companion_context_keys: HashSet<String>,
    pub(super) failed_tool_fingerprints: HashMap<String, usize>,
    pub(super) failed_tool_names: HashMap<String, usize>,
    pub(super) successful_required_validation_commands: HashSet<String>,
    /// Session-scoped consecutive repair count for progressive output cap.
    pub(super) consecutive_repairs: u32,
}

pub(super) struct TurnLoopStateController;

impl TurnLoopStateController {
    pub(super) fn initial_state() -> TurnLoopState {
        TurnLoopState::default()
    }
}

impl TurnLoopState {
    pub(super) fn record_executed_tool_calls(&mut self, tool_calls: &[ToolCall]) {
        for tool_call in tool_calls {
            if tool_call.name.trim().is_empty() {
                continue;
            }
            if !self.tools_used.iter().any(|name| name == &tool_call.name) {
                self.tools_used.push(tool_call.name.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TurnRuntimeState / FocusedRepairRuntimeState
// ---------------------------------------------------------------------------

pub(super) struct TurnRuntimeState {
    pub(super) evidence_ledger: EvidenceLedger,
    pub(super) runtime_diet: RuntimeDietSnapshot,
    pub(super) tool_lifecycle: ToolCallLifecycle,
    pub(super) focused_repair: FocusedRepairRuntimeState,
    pub(super) route_recovery: RouteRecoveryRuntimeState,
    pub(super) iterations_used: usize,
    pub(super) effective_iterations: usize,
    pub(super) acceptance_repair_attempts: usize,
    pub(super) reserved_repair_rounds: usize,
    pub(super) successful_read_only_tool_fingerprints: HashMap<String, usize>,
    pub(super) storm_state: StormState,
}

#[derive(Default)]
pub(super) struct FocusedRepairRuntimeState {
    pub(super) no_code_progress_rounds: usize,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) action_checkpoint_no_change_rounds: usize,
    pub(super) action_checkpoint_requires_patch_before_validation: bool,
    #[cfg(test)]
    pub(super) patch_synthesis_recovery_used: bool,
    #[cfg(test)]
    pub(super) action_checkpoint_reopen_used: bool,
    pub(super) no_diff_audit_validation_checkpoint_sent: bool,
    pub(super) code_write_forbidden_checkpoint_sent: bool,
    pub(super) file_edit_failure_retry_used: bool,
    pub(super) no_effective_diff_repair_rounds: usize,
}

impl TurnRuntimeState {
    pub(super) fn new(route_scoped_tools_enabled: bool) -> Self {
        Self {
            evidence_ledger: EvidenceLedger::new(),
            runtime_diet: RuntimeDietSnapshot::new(route_scoped_tools_enabled),
            tool_lifecycle: ToolCallLifecycle::default(),
            focused_repair: FocusedRepairRuntimeState::default(),
            route_recovery: RouteRecoveryRuntimeState::default(),
            iterations_used: 0,
            effective_iterations: 0,
            acceptance_repair_attempts: 0,
            reserved_repair_rounds: 0,
            successful_read_only_tool_fingerprints: HashMap::new(),
            storm_state: StormState::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// TurnRuntimeContext / SessionRuntimeState
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(super) struct TurnRuntimeContext<'a> {
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) task_stage: AgentTaskStage,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) working_dir: &'a Path,
    pub(super) last_user_preview: &'a str,
    pub(super) required_validation_commands: &'a [String],
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) retained_context: &'a ToolContextRetainedContext,
}

#[derive(Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct SessionRuntimeState {
    pub(super) companion_context_keys: HashSet<String>,
    pub(super) failed_tool_fingerprints: HashMap<String, usize>,
    pub(super) failed_tool_names: HashMap<String, usize>,
    pub(super) successful_required_validation_commands: HashSet<String>,
}

impl SessionRuntimeState {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::resource_policy::ResourcePolicy;
    use crate::engine::trace::{TraceCollector, TurnTrace};

    #[test]
    fn initial_state_starts_empty() {
        let state = TurnLoopStateController::initial_state();

        assert!(state.final_content.is_empty());
        assert!(state.final_tool_calls.is_empty());
        assert!(!state.tool_calls_made);
        assert!(state.tools_used.is_empty());
        assert!(!state.pseudo_tool_retry_used);
        assert!(!state.filesystem_grounding_retry_used);
        assert!(!state.continuation_retry_used);
        assert!(!state.claim_gate_repair_used);
        assert!(state.companion_context_keys.is_empty());
        assert!(state.failed_tool_fingerprints.is_empty());
        assert!(state.failed_tool_names.is_empty());
        assert!(state.successful_required_validation_commands.is_empty());
    }

    #[test]
    fn turn_loop_state_records_executed_tool_names_once_in_order() {
        let mut state = TurnLoopStateController::initial_state();
        state.record_executed_tool_calls(&[
            ToolCall {
                id: "call_1".to_string(),
                name: "file_write".to_string(),
                arguments: serde_json::json!({}),
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({}),
            },
            ToolCall {
                id: "call_3".to_string(),
                name: "file_write".to_string(),
                arguments: serde_json::json!({}),
            },
        ]);

        assert_eq!(state.tools_used, vec!["file_write", "bash"]);
    }

    #[test]
    fn session_runtime_state_starts_empty() {
        let state = SessionRuntimeState::new();

        assert!(state.companion_context_keys.is_empty());
        assert!(state.failed_tool_fingerprints.is_empty());
        assert!(state.failed_tool_names.is_empty());
        assert!(state.successful_required_validation_commands.is_empty());
    }

    #[test]
    fn turn_runtime_context_groups_per_turn_refs() {
        let route = IntentRouter::new().route("inspect files");
        let policy = ResourcePolicy::from_route(&route);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "inspect files"));
        let destructive_scope =
            DestructiveScopeContract::from_user_request("inspect files", Path::new("."));
        let exposed = HashSet::from(["grep".to_string()]);
        let baseline = HashSet::new();
        let required = vec!["cargo check -q".to_string()];
        let retained_context = ToolContextRetainedContext::default();
        let context = TurnRuntimeContext {
            tx: None,
            trace: &trace,
            route: &route,
            resource_policy: &policy,
            task_stage: AgentTaskStage::Understand,
            exposed_tool_names: &exposed,
            working_dir: Path::new("."),
            last_user_preview: "inspect files",
            required_validation_commands: &required,
            destructive_scope: &destructive_scope,
            baseline_git_status_files: &baseline,
            retained_context: &retained_context,
        };

        assert_eq!(context.exposed_tool_names.len(), 1);
        assert_eq!(context.required_validation_commands[0], "cargo check -q");
        assert_eq!(context.last_user_preview, "inspect files");
        assert!(context.retained_context.is_empty());
    }
}
