use super::runtime_diet::RuntimeDietSnapshot;
use crate::engine::evidence_ledger::EvidenceLedger;

pub(super) struct TurnRuntimeState {
    pub(super) evidence_ledger: EvidenceLedger,
    pub(super) runtime_diet: RuntimeDietSnapshot,
    pub(super) iterations_used: usize,
    pub(super) effective_iterations: usize,
    pub(super) acceptance_repair_attempts: usize,
    pub(super) reserved_repair_rounds: usize,
}

impl TurnRuntimeState {
    pub(super) fn new(route_scoped_tools_enabled: bool) -> Self {
        Self {
            evidence_ledger: EvidenceLedger::new(),
            runtime_diet: RuntimeDietSnapshot::new(route_scoped_tools_enabled),
            iterations_used: 0,
            effective_iterations: 0,
            acceptance_repair_attempts: 0,
            reserved_repair_rounds: 0,
        }
    }
}
