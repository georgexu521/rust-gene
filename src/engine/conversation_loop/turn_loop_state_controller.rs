use crate::services::api::ToolCall;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct TurnLoopState {
    pub(super) final_content: String,
    pub(super) final_tool_calls: Vec<ToolCall>,
    pub(super) tool_calls_made: bool,
    pub(super) pseudo_tool_retry_used: bool,
    pub(super) filesystem_grounding_retry_used: bool,
    pub(super) companion_context_keys: HashSet<String>,
    pub(super) failed_tool_fingerprints: HashMap<String, usize>,
    pub(super) failed_tool_names: HashMap<String, usize>,
    pub(super) successful_required_validation_commands: HashSet<String>,
    /// Count of consecutive model responses with no tool calls.
    /// The loop only breaks after TWO consecutive empty rounds — the
    /// first one may be "thinking out loud" before acting.
    pub(super) consecutive_empty_rounds: usize,
}

pub(super) struct TurnLoopStateController;

impl TurnLoopStateController {
    pub(super) fn initial_state() -> TurnLoopState {
        TurnLoopState::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_starts_empty() {
        let state = TurnLoopStateController::initial_state();

        assert!(state.final_content.is_empty());
        assert!(state.final_tool_calls.is_empty());
        assert!(!state.tool_calls_made);
        assert!(!state.pseudo_tool_retry_used);
        assert!(!state.filesystem_grounding_retry_used);
        assert!(state.companion_context_keys.is_empty());
        assert!(state.failed_tool_fingerprints.is_empty());
        assert!(state.failed_tool_names.is_empty());
        assert!(state.successful_required_validation_commands.is_empty());
    }
}
