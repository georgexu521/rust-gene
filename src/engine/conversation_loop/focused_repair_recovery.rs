//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::{safe_prefix_by_bytes, ConversationLoop};
use crate::engine::streaming::StreamEvent;
use crate::services::api::Message;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::debug;

pub(super) struct DisabledPatchSynthesisRecoveryRequest<'a> {
    pub(super) patch_synthesis_recovery_used: bool,
    pub(super) action_checkpoint_reopen_used: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) exposed_tool_names: &'a HashSet<String>,
}

pub(super) enum DisabledPatchSynthesisRecovery {
    ReturnToModel {
        prompt: String,
    },
    ReopenNormalTools {
        prompt: String,
        trace_error: &'static str,
    },
    Stop {
        message: &'static str,
    },
}

pub(super) enum PatchSynthesisFailureRecovery {
    InsufficientEvidence {
        prompt: String,
    },
    ReopenNormalTools {
        prompt: String,
        trace_error: &'static str,
    },
    Stop {
        message: &'static str,
    },
}

pub(super) struct FocusedRepairRecoveryController;

impl FocusedRepairRecoveryController {
    pub(super) const DISABLED_STOP_MESSAGE: &'static str =
        "[Stopped action checkpoint without patch synthesis; no model-led file change was produced]";
    pub(super) const NO_CHANGE_STOP_MESSAGE: &'static str =
        "[Patch synthesis did not produce a file change; stopped action checkpoint]";
    pub(super) const FAILURE_STOP_MESSAGE: &'static str =
        "[Stopped action checkpoint after repeated invalid tool requests]";

    pub(super) fn code_write_forbidden_prompt() -> &'static str {
        "Patch synthesis skipped because this request forbids code-write tools. Continue with the exposed tools only: run required validation if available, report direct evidence, and close out without arbitrary file edits."
    }

    pub(super) fn disabled_patch_synthesis_recovery(
        request: DisabledPatchSynthesisRecoveryRequest<'_>,
    ) -> DisabledPatchSynthesisRecovery {
        if !request.patch_synthesis_recovery_used {
            let lookup_rule = ConversationLoop::targeted_lookup_budget_rule(
                request.action_checkpoint_lookup_count,
            );
            let recovery = format!(
                "Patch synthesis is disabled by default. Use only the exposed tools ({}) to make the smallest safe patch from the evidence already gathered. Prefer file_edit/file_write/file_patch so permission, stale-read, diff, and rollback checks stay active. If file_read or grep is still exposed, use the remaining focused lookup budget before patching; otherwise patch from the evidence already gathered. {} Do not call tools that are not exposed.",
                request
                    .exposed_tool_names
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                lookup_rule
            );
            return DisabledPatchSynthesisRecovery::ReturnToModel { prompt: recovery };
        }

        if !request.action_checkpoint_reopen_used {
            return DisabledPatchSynthesisRecovery::ReopenNormalTools {
                prompt: "Focused repair did not produce a file change. Return to normal coding tools for one final recovery pass: inspect only the exact function or call site needed, then make a real file_edit/file_write/file_patch change before running validation. Do not close out until a file change succeeds or a concrete blocker is proven."
                    .to_string(),
                trace_error:
                    "focused repair did not produce a patch; reopening normal code-change tools once",
            };
        }

        DisabledPatchSynthesisRecovery::Stop {
            message: Self::DISABLED_STOP_MESSAGE,
        }
    }

    pub(super) fn patch_synthesis_failure_recovery(
        err_text: &str,
        patch_synthesis_recovery_used: bool,
        action_checkpoint_reopen_used: bool,
    ) -> PatchSynthesisFailureRecovery {
        if !patch_synthesis_recovery_used && Self::is_insufficient_evidence_error(err_text) {
            let lookup_rule = ConversationLoop::targeted_lookup_budget_rule(0);
            let recovery = format!(
                "Patch synthesis declined because evidence was insufficient: {}. Use a targeted read/search for the missing symbol, call site, or test, then make the smallest safe edit. {}",
                safe_prefix_by_bytes(err_text, 500),
                lookup_rule
            );
            return PatchSynthesisFailureRecovery::InsufficientEvidence { prompt: recovery };
        }

        if !action_checkpoint_reopen_used {
            return PatchSynthesisFailureRecovery::ReopenNormalTools {
                prompt: format!(
                    "Patch synthesis could not produce an executable edit: {}. Return to normal coding tools for one final recovery pass: inspect only the exact function or call site needed, then make a real file_edit/file_write/file_patch change before validation.",
                    safe_prefix_by_bytes(err_text, 500)
                ),
                trace_error: "patch synthesis failed; reopening normal code-change tools once",
            };
        }

        PatchSynthesisFailureRecovery::Stop {
            message: Self::FAILURE_STOP_MESSAGE,
        }
    }

    pub(super) fn append_system_prompt(
        messages: &mut Vec<Message>,
        tool_results_text: &mut String,
        prompt: impl Into<String>,
    ) {
        let prompt = prompt.into();
        messages.push(super::request_preparation_controller::recent_observation_message(&prompt));
        tool_results_text.push('\n');
        tool_results_text.push_str(&prompt);
    }

    pub(super) async fn stop_with_message(
        tx: Option<&mpsc::Sender<StreamEvent>>,
        final_content: &mut String,
        stop_msg: &str,
    ) {
        debug!("{}", stop_msg);
        if let Some(tx) = tx {
            let _ = tx
                .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                .await;
        }
        if final_content.trim().is_empty() {
            *final_content = stop_msg.to_string();
        } else {
            final_content.push('\n');
            final_content.push_str(stop_msg);
        }
    }

    fn is_insufficient_evidence_error(err_text: &str) -> bool {
        let lower_err = err_text.to_lowercase();
        lower_err.contains("declined")
            || lower_err.contains("inspect more")
            || lower_err.contains("need to inspect")
            || lower_err.contains("not enough evidence")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_patch_synthesis_first_uses_model_recovery_prompt() {
        let exposed = HashSet::from(["file_edit".to_string(), "grep".to_string()]);

        let decision = FocusedRepairRecoveryController::disabled_patch_synthesis_recovery(
            DisabledPatchSynthesisRecoveryRequest {
                patch_synthesis_recovery_used: false,
                action_checkpoint_reopen_used: false,
                action_checkpoint_lookup_count: 1,
                exposed_tool_names: &exposed,
            },
        );

        let DisabledPatchSynthesisRecovery::ReturnToModel { prompt } = decision else {
            panic!("expected model recovery prompt");
        };
        assert!(prompt.contains("Patch synthesis is disabled by default"));
        assert!(prompt.contains("file_edit"));
        assert!(prompt.contains("grep"));
        assert!(prompt.contains("One targeted file_read/grep lookup remains"));
    }

    #[test]
    fn disabled_patch_synthesis_then_reopens_then_stops() {
        let exposed = HashSet::new();

        let reopen = FocusedRepairRecoveryController::disabled_patch_synthesis_recovery(
            DisabledPatchSynthesisRecoveryRequest {
                patch_synthesis_recovery_used: true,
                action_checkpoint_reopen_used: false,
                action_checkpoint_lookup_count: 0,
                exposed_tool_names: &exposed,
            },
        );
        assert!(matches!(
            reopen,
            DisabledPatchSynthesisRecovery::ReopenNormalTools { trace_error, .. }
                if trace_error == "focused repair did not produce a patch; reopening normal code-change tools once"
        ));

        let stop = FocusedRepairRecoveryController::disabled_patch_synthesis_recovery(
            DisabledPatchSynthesisRecoveryRequest {
                patch_synthesis_recovery_used: true,
                action_checkpoint_reopen_used: true,
                action_checkpoint_lookup_count: 0,
                exposed_tool_names: &exposed,
            },
        );
        assert!(matches!(
            stop,
            DisabledPatchSynthesisRecovery::Stop {
                message: FocusedRepairRecoveryController::DISABLED_STOP_MESSAGE
            }
        ));
    }

    #[test]
    fn synthesis_failure_distinguishes_evidence_and_reopen_paths() {
        let insufficient = FocusedRepairRecoveryController::patch_synthesis_failure_recovery(
            "declined: need to inspect more",
            false,
            false,
        );
        assert!(matches!(
            insufficient,
            PatchSynthesisFailureRecovery::InsufficientEvidence { ref prompt }
                if prompt.contains("evidence was insufficient")
        ));

        let reopen = FocusedRepairRecoveryController::patch_synthesis_failure_recovery(
            "schema failed",
            false,
            false,
        );
        assert!(matches!(
            reopen,
            PatchSynthesisFailureRecovery::ReopenNormalTools { trace_error, .. }
                if trace_error == "patch synthesis failed; reopening normal code-change tools once"
        ));
    }
}
