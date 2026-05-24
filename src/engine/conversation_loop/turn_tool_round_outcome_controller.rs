use super::tool_batch_result_processor::{
    DuplicateSuccessfulReadOnlyToolResult, ToolBatchProcessingOutcome,
};
use std::path::PathBuf;

pub(super) struct TurnToolRoundState {
    pub(super) tool_results_text: String,
    pub(super) changed_files: Vec<PathBuf>,
    pub(super) batch_has_unsuccessful_tools: bool,
    pub(super) used_write_tool: bool,
    pub(super) successful_write_tool: bool,
    pub(super) used_action_checkpoint_lookup: bool,
    pub(super) any_tool_success: bool,
    pub(super) repeated_failed_tools: Vec<String>,
    pub(super) failed_tool_names_this_round: Vec<String>,
    pub(super) failed_tool_evidence: Vec<String>,
    pub(super) file_edit_failure_correction_added: bool,
    pub(super) successful_validation_commands: Vec<String>,
    pub(super) duplicate_successful_read_only_tools: Vec<String>,
    pub(super) duplicate_successful_read_only_results: Vec<DuplicateSuccessfulReadOnlyToolResult>,
    pub(super) should_closeout_after_verified_change: bool,
}

impl TurnToolRoundState {
    pub(super) fn has_worktree_changes(&self) -> bool {
        !self.changed_files.is_empty()
    }

    pub(super) fn has_successful_validation_commands(&self) -> bool {
        !self.successful_validation_commands.is_empty()
    }

    pub(super) fn failed_tool_evidence_present(&self) -> bool {
        !self.failed_tool_evidence.is_empty()
    }
}

pub(super) struct TurnToolRoundOutcomeController;

impl TurnToolRoundOutcomeController {
    pub(super) fn from_batch(outcome: ToolBatchProcessingOutcome) -> TurnToolRoundState {
        TurnToolRoundState {
            tool_results_text: outcome.tool_results_text,
            changed_files: outcome.changed_files,
            batch_has_unsuccessful_tools: outcome.batch_has_unsuccessful_tools,
            used_write_tool: outcome.used_write_tool,
            successful_write_tool: outcome.successful_write_tool,
            used_action_checkpoint_lookup: outcome.used_action_checkpoint_lookup,
            any_tool_success: outcome.any_tool_success,
            repeated_failed_tools: outcome.repeated_failed_tools,
            failed_tool_names_this_round: outcome.failed_tool_names_this_round,
            failed_tool_evidence: outcome.failed_tool_evidence,
            file_edit_failure_correction_added: outcome.file_edit_failure_correction_added,
            successful_validation_commands: outcome.successful_validation_commands,
            duplicate_successful_read_only_tools: outcome.duplicate_successful_read_only_tools,
            duplicate_successful_read_only_results: outcome.duplicate_successful_read_only_results,
            should_closeout_after_verified_change: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_batch_preserves_round_outcome_and_starts_closeout_false() {
        let state = TurnToolRoundOutcomeController::from_batch(ToolBatchProcessingOutcome {
            tool_results_text: "result text".to_string(),
            changed_files: vec![PathBuf::from("src/lib.rs")],
            batch_has_unsuccessful_tools: true,
            used_write_tool: true,
            successful_write_tool: true,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: vec!["bash".to_string()],
            failed_tool_names_this_round: vec!["bash".to_string()],
            failed_tool_evidence: vec!["bash failed".to_string()],
            file_edit_failure_correction_added: false,
            successful_validation_commands: vec!["cargo test -q".to_string()],
            duplicate_successful_read_only_tools: vec!["file_read".to_string()],
            duplicate_successful_read_only_results: vec![DuplicateSuccessfulReadOnlyToolResult {
                tool_name: "file_read".to_string(),
                result_text: "# Readme".to_string(),
                ledger_summary: None,
            }],
        });

        assert_eq!(state.tool_results_text, "result text");
        assert!(state.has_worktree_changes());
        assert!(state.has_successful_validation_commands());
        assert!(state.failed_tool_evidence_present());
        assert!(state.batch_has_unsuccessful_tools);
        assert!(state.used_write_tool);
        assert!(state.successful_write_tool);
        assert!(state.any_tool_success);
        assert!(!state.used_action_checkpoint_lookup);
        assert_eq!(state.repeated_failed_tools, vec!["bash".to_string()]);
        assert_eq!(state.failed_tool_names_this_round, vec!["bash".to_string()]);
        assert_eq!(
            state.successful_validation_commands,
            vec!["cargo test -q".to_string()]
        );
        assert_eq!(
            state.duplicate_successful_read_only_tools,
            vec!["file_read".to_string()]
        );
        assert_eq!(
            state.duplicate_successful_read_only_results[0].result_text,
            "# Readme"
        );
        assert!(!state.should_closeout_after_verified_change);
    }
}
