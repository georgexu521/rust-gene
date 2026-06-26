//! Runtime stage transition helpers for LabRun orchestration.

use super::*;

pub(super) fn transition_for_stage(stage: &str) -> Option<StageTransition> {
    STAGE_TRANSITIONS
        .iter()
        .copied()
        .find(|transition| transition.from_stage == stage)
}
