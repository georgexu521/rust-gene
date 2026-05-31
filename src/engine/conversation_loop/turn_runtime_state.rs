use super::runtime_diet::RuntimeDietSnapshot;
use super::tool_call_lifecycle::ToolCallLifecycle;
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::repair::storm::StormState;
use crate::engine::route_recovery::RouteRecoveryRuntimeState;
use std::collections::HashMap;

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
    pub(super) successful_read_only_tool_results: HashMap<String, String>,
    pub(super) storm_state: StormState,
}

#[derive(Default)]
pub(super) struct FocusedRepairRuntimeState {
    pub(super) no_code_progress_rounds: usize,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) action_checkpoint_no_change_rounds: usize,
    pub(super) action_checkpoint_requires_patch_before_validation: bool,
    pub(super) patch_synthesis_recovery_used: bool,
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
            successful_read_only_tool_results: HashMap::new(),
            storm_state: StormState::default(),
        }
    }
}
