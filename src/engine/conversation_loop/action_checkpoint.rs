use super::workflow_trace::trace_adaptive_workflow_trigger;
use super::ConversationLoop;
use crate::engine::code_change_workflow::{AdaptiveWorkflowTrigger, CodeChangeWorkflowRunner};
use crate::engine::intent_router::WorkflowKind;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::Message;
use std::collections::HashSet;

pub(super) struct ProgressCheckpointRequest {
    pub(super) no_diff_audit_closeout_allowed: bool,
    pub(super) has_worktree_changes: bool,
    pub(super) has_successful_validation_commands: bool,
    pub(super) no_code_progress_rounds: usize,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) action_checkpoint_no_change_rounds: usize,
    pub(super) no_diff_audit_validation_checkpoint_sent: bool,
    pub(super) code_write_tools_forbidden: bool,
    pub(super) code_write_forbidden_checkpoint_sent: bool,
    pub(super) used_action_checkpoint_lookup: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProgressCheckpointAction {
    None,
    AuditNoDiffValidation,
    ExistingDiffNeedsRepair { no_code_progress_rounds: usize },
    ProgressReminder { no_code_progress_rounds: usize },
    EnterActionCheckpoint { no_code_progress_rounds: usize },
    CodeWriteForbidden,
    FocusedLookupNotice { exhausted: bool },
    FocusedRepairStalled,
}

pub(super) struct ProgressCheckpointDecision {
    pub(super) action: ProgressCheckpointAction,
    pub(super) no_code_progress_rounds: usize,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) action_checkpoint_no_change_rounds: usize,
    pub(super) no_diff_audit_validation_checkpoint_sent: bool,
    pub(super) code_write_forbidden_checkpoint_sent: bool,
    pub(super) reset_file_edit_failure_retry: bool,
    pub(super) force_patch_synthesis_after_no_change: bool,
    pub(super) force_patch_synthesis_reason: Option<&'static str>,
}

pub(super) struct FocusedRepairActionRequest<'a> {
    pub(super) action_checkpoint_active: bool,
    pub(super) any_tool_success: bool,
    pub(super) batch_has_unsuccessful_tools: bool,
    pub(super) failed_tool_evidence_present: bool,
    pub(super) force_patch_synthesis_after_no_change: bool,
    pub(super) force_patch_synthesis_reason: Option<&'static str>,
    pub(super) action_checkpoint_no_change_rounds: usize,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) exposed_tool_names: &'a HashSet<String>,
}

pub(super) struct FocusedRepairActionProposal {
    pub(super) reminder: String,
    pub(super) next_no_change_rounds: usize,
    pub(super) enter_patch_synthesis: bool,
    pub(super) trace_error: String,
    pub(super) fallback_owner: &'static str,
    pub(super) fallback_reason: String,
}

pub(super) struct ProgressCheckpointActionContext<'a> {
    pub(super) action: ProgressCheckpointAction,
    pub(super) workflow: WorkflowKind,
    pub(super) trace: &'a TraceCollector,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) tool_results_text: &'a mut String,
}

pub(super) struct ProgressCheckpointController;

impl ProgressCheckpointController {
    pub(super) fn evaluate_read_only_success(
        request: ProgressCheckpointRequest,
    ) -> ProgressCheckpointDecision {
        let mut no_code_progress_rounds = request.no_code_progress_rounds;
        let mut action_checkpoint_active = request.action_checkpoint_active;
        let mut action_checkpoint_lookup_count = request.action_checkpoint_lookup_count;
        let mut action_checkpoint_no_change_rounds = request.action_checkpoint_no_change_rounds;
        let mut no_diff_audit_validation_checkpoint_sent =
            request.no_diff_audit_validation_checkpoint_sent;
        let mut code_write_forbidden_checkpoint_sent = request.code_write_forbidden_checkpoint_sent;
        let mut reset_file_edit_failure_retry = false;
        let mut force_patch_synthesis_after_no_change = false;
        let mut force_patch_synthesis_reason = None;
        let mut activated_checkpoint_this_round = false;
        let mut action = ProgressCheckpointAction::None;

        if (request.no_diff_audit_closeout_allowed || request.has_worktree_changes)
            && request.has_successful_validation_commands
        {
            no_code_progress_rounds = 0;
            action_checkpoint_active = false;
            action_checkpoint_no_change_rounds = 0;
            action_checkpoint_lookup_count = 0;
        } else {
            no_code_progress_rounds += 1;
        }

        if request.no_diff_audit_closeout_allowed && !request.has_worktree_changes {
            if no_code_progress_rounds >= 2
                && !action_checkpoint_active
                && !no_diff_audit_validation_checkpoint_sent
            {
                action = ProgressCheckpointAction::AuditNoDiffValidation;
                no_diff_audit_validation_checkpoint_sent = true;
                no_code_progress_rounds = 0;
            }
        } else if !request.code_write_tools_forbidden
            && request.has_worktree_changes
            && !request.has_successful_validation_commands
            && no_code_progress_rounds >= 2
            && !action_checkpoint_active
        {
            action = ProgressCheckpointAction::ExistingDiffNeedsRepair {
                no_code_progress_rounds,
            };
            action_checkpoint_active = true;
            action_checkpoint_lookup_count =
                ConversationLoop::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET;
            reset_file_edit_failure_retry = true;
            action_checkpoint_no_change_rounds = 2;
            force_patch_synthesis_after_no_change = true;
            force_patch_synthesis_reason =
                Some("existing diff still needs repair after repeated read-only rounds");
            activated_checkpoint_this_round = true;
        } else if !request.code_write_tools_forbidden
            && no_code_progress_rounds == 2
            && !action_checkpoint_active
        {
            action = ProgressCheckpointAction::ProgressReminder {
                no_code_progress_rounds,
            };
        } else if !request.code_write_tools_forbidden
            && no_code_progress_rounds >= 3
            && !action_checkpoint_active
        {
            action = ProgressCheckpointAction::EnterActionCheckpoint {
                no_code_progress_rounds,
            };
            action_checkpoint_active = true;
            action_checkpoint_lookup_count = 0;
            reset_file_edit_failure_retry = true;
            action_checkpoint_no_change_rounds = 2;
            force_patch_synthesis_after_no_change = false;
            force_patch_synthesis_reason = None;
            activated_checkpoint_this_round = true;
        } else if request.code_write_tools_forbidden
            && no_code_progress_rounds >= 2
            && !code_write_forbidden_checkpoint_sent
        {
            action = ProgressCheckpointAction::CodeWriteForbidden;
            code_write_forbidden_checkpoint_sent = true;
            no_code_progress_rounds = 0;
        } else if action_checkpoint_active && request.used_action_checkpoint_lookup {
            action_checkpoint_lookup_count = (action_checkpoint_lookup_count + 1)
                .min(ConversationLoop::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET);
            action_checkpoint_no_change_rounds = 0;
            activated_checkpoint_this_round = true;
            let exhausted = action_checkpoint_lookup_count
                >= ConversationLoop::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET;
            action = ProgressCheckpointAction::FocusedLookupNotice { exhausted };
            if exhausted {
                action_checkpoint_no_change_rounds = 1;
                force_patch_synthesis_after_no_change = true;
                force_patch_synthesis_reason = Some("focused repair lookup budget exhausted");
            }
        }

        if action_checkpoint_active && !activated_checkpoint_this_round {
            action_checkpoint_no_change_rounds += 1;
            if action_checkpoint_no_change_rounds >= 3 {
                action = ProgressCheckpointAction::FocusedRepairStalled;
                force_patch_synthesis_after_no_change = true;
                force_patch_synthesis_reason =
                    Some("focused repair lookup did not produce a patch");
            }
        }

        ProgressCheckpointDecision {
            action,
            no_code_progress_rounds,
            action_checkpoint_active,
            action_checkpoint_lookup_count,
            action_checkpoint_no_change_rounds,
            no_diff_audit_validation_checkpoint_sent,
            code_write_forbidden_checkpoint_sent,
            reset_file_edit_failure_retry,
            force_patch_synthesis_after_no_change,
            force_patch_synthesis_reason,
        }
    }
}

pub(super) struct ProgressCheckpointActionApplier;

impl ProgressCheckpointActionApplier {
    pub(super) fn apply(context: ProgressCheckpointActionContext<'_>) {
        match context.action {
            ProgressCheckpointAction::None => {}
            ProgressCheckpointAction::AuditNoDiffValidation => {
                let checkpoint = "Audit/regression checkpoint: this task allows a no-diff closeout when the requested behavior is already present. Do not force an arbitrary edit. Run the required validation commands now; if they pass, provide a Closeout with direct evidence and changed files as none. If a concrete missing behavior is proven, then make the smallest focused edit."
                    .to_string();
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: "audit/regression task should validate before forcing edits".to_string(),
                });
                Self::append_checkpoint(context.messages, context.tool_results_text, checkpoint);
            }
            ProgressCheckpointAction::ExistingDiffNeedsRepair {
                no_code_progress_rounds: rounds,
            } => {
                Self::activate_repeated_no_code_progress(context.trace, context.code_workflow);
                let checkpoint = format!(
                    "Workflow acceptance repair checkpoint: this {:?} task already has code changes, but {} consecutive successful tool rounds made no additional edit. Use the evidence already gathered to synthesize the smallest remaining file_edit/file_write/file_patch change now. If multiple independent acceptance-critical bypasses are visible, fix them together; otherwise stop with a Closeout status of not_verified and name the blocker.",
                    context.workflow, rounds
                );
                context.trace.record(TraceEvent::WorkflowFallback {
                    error:
                        "existing diff still needs repair; entering patch synthesis after repeated read-only rounds"
                            .to_string(),
                });
                Self::append_checkpoint(context.messages, context.tool_results_text, checkpoint);
            }
            ProgressCheckpointAction::ProgressReminder {
                no_code_progress_rounds: rounds,
            } => {
                let lookup_rule = ConversationLoop::targeted_lookup_budget_rule(0);
                let checkpoint = format!(
                    "Workflow progress checkpoint: this is a {:?} task and {} consecutive successful tool rounds produced no code change. Keep investigation focused: on the next response either make the smallest safe file_edit/file_write/file_patch change, or use the focused lookup budget if a required symbol, test, or call site is still missing. {} If a scorer/decision object already returns final status, use that status directly instead of reimplementing acceptance gates.",
                    context.workflow, rounds, lookup_rule
                );
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: "code-change task needs an edit after repeated inspection".to_string(),
                });
                Self::append_checkpoint(context.messages, context.tool_results_text, checkpoint);
            }
            ProgressCheckpointAction::EnterActionCheckpoint {
                no_code_progress_rounds: rounds,
            } => {
                Self::activate_repeated_no_code_progress(context.trace, context.code_workflow);
                let lookup_rule = ConversationLoop::targeted_lookup_budget_rule(0);
                let checkpoint = format!(
                    "Workflow action checkpoint: this is a {:?} task and {} consecutive successful tool rounds produced no code change. On the next response, use file_edit or file_write to apply the smallest safe patch, then run validation after the file changes. If prior grep/file_read results include line numbers, prefer file_edit with line_start/line_end or exact old_string copied from that current source context. Do not call glob/project_list or repeat broad inspection. If a specific symbol, test, or call site is still missing, use the focused lookup budget, then patch. {} If a scorer/decision object already returns final status, use that status directly instead of reimplementing acceptance gates. If you cannot patch safely from the evidence already gathered, stop with a Closeout status of not_verified and a concrete blocker.",
                    context.workflow, rounds, lookup_rule
                );
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: "code-change task made no edit after repeated inspection".to_string(),
                });
                Self::append_checkpoint(context.messages, context.tool_results_text, checkpoint);
            }
            ProgressCheckpointAction::CodeWriteForbidden => {
                let checkpoint = "Tool-scope checkpoint: this request forbids code-write tools. Do not synthesize or call file_edit, file_write, or file_patch. Use the exposed read/terminal tools to gather direct evidence, run required validation when present, then close out with changed files as none unless a concrete blocker prevents validation."
                    .to_string();
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: "code-write tools are forbidden; validation/closeout should replace patch synthesis"
                        .to_string(),
                });
                Self::append_checkpoint(context.messages, context.tool_results_text, checkpoint);
            }
            ProgressCheckpointAction::FocusedLookupNotice { exhausted } => {
                let lookup_notice = if exhausted {
                    "focused repair targeted lookup budget used; next checkpoint request will expose patch tools only"
                } else {
                    "focused repair targeted lookup used; one targeted lookup remains before patch-only mode"
                };
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: lookup_notice.to_string(),
                });
            }
            ProgressCheckpointAction::FocusedRepairStalled => {
                context.trace.record(TraceEvent::WorkflowFallback {
                    error:
                        "action checkpoint entered patch synthesis after repeated focused repair reads"
                            .to_string(),
                });
            }
        }
    }

    fn activate_repeated_no_code_progress(
        trace: &TraceCollector,
        code_workflow: &mut CodeChangeWorkflowRunner,
    ) {
        if code_workflow.activate_trigger(AdaptiveWorkflowTrigger::RepeatedNoCodeProgress) {
            trace_adaptive_workflow_trigger(
                trace,
                AdaptiveWorkflowTrigger::RepeatedNoCodeProgress,
                code_workflow,
            );
            trace.record(TraceEvent::WorkflowFallback {
                error: "adaptive workflow trigger activated: repeated_no_code_progress".to_string(),
            });
        }
    }

    fn append_checkpoint(
        messages: &mut Vec<Message>,
        tool_results_text: &mut String,
        checkpoint: String,
    ) {
        messages.push(Message::system(checkpoint.clone()));
        tool_results_text.push('\n');
        tool_results_text.push_str(&checkpoint);
    }
}

impl ConversationLoop {
    pub(super) const ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET: usize = 2;

    pub(super) fn targeted_lookup_budget_rule(targeted_lookups_used: usize) -> String {
        let remaining =
            Self::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET.saturating_sub(targeted_lookups_used);
        match remaining {
            0 => "The targeted lookup budget has already been used; do not call file_read/grep again. Patch from the evidence already gathered.".to_string(),
            1 => "One targeted file_read/grep lookup remains for a specific missing line range, symbol, test, or call site; after that, patch from the evidence gathered.".to_string(),
            remaining => format!(
                "Up to {remaining} targeted file_read/grep lookups remain for specific missing line ranges, symbols, tests, or call sites; do not repeat broad inspection."
            ),
        }
    }

    pub(super) fn focused_repair_mode_prompt(
        exposed_names: &[String],
        targeted_lookups_used: usize,
    ) -> String {
        let lookup_rule = Self::targeted_lookup_budget_rule(targeted_lookups_used);
        format!(
            "Current tool mode: FOCUSED REPAIR. The exposed tools for this request are: {}. Patch files as soon as the target line is known, using file_edit/file_write/file_patch so permission, stale-read, diff, and rollback checks stay active. {} Do not call glob/project_list or any tool that is not in the exposed list. Do not use bash for patching or read-only inspection; after a file changes, bash may run validation. If previous validation reported compile/type errors, fix those exact errors first using the latest verification source context. If you have line numbers from earlier grep/file_read/verification output, prefer file_edit with line_start/line_end or exact old_string copied from that current source context; use file_patch for coordinated multi-file changes. Do not invent enum variants, struct fields, functions, or APIs not visible in prior tool output; reuse existing names exactly. If a scorer/decision object already returns a final status, use that status directly; do not wrap it with explicit/score checks that can bypass safety, volatility, or duplication hard stops.",
            exposed_names.join(", "),
            lookup_rule
        )
    }

    pub(super) fn file_edit_failure_repair_correction(
        failed_tool_evidence: &[String],
    ) -> Option<String> {
        let relevant = failed_tool_evidence
            .iter()
            .filter(|evidence| evidence.contains("file_edit"))
            .filter(|evidence| {
                evidence.contains("Expected 1 occurrence")
                    || evidence.contains("old_string cannot be empty")
                    || evidence.contains("old_string cannot be empty or whitespace-only")
                    || evidence.contains("Action checkpoint file_edit rejected")
                    || evidence.contains("unique edit anchor")
            })
            .take(2)
            .cloned()
            .collect::<Vec<_>>();

        if relevant.is_empty() {
            return None;
        }

        Some(format!(
            "File edit repair correction:\n{}\nNext action is still a patch, not closeout. The previous file_edit did not modify a file because its anchor was empty, whitespace-only, or non-unique. Use one of these safer forms:\n- If prior file_read/grep output shows the target line number, call file_edit with path, line_start, line_end, and new_string for that exact line.\n- Otherwise copy a multi-line old_string that includes the surrounding function call and is unique exactly once.\nDo not retry the same broad old_string. Do not close out until a file_edit/file_write succeeds and validation runs.",
            relevant.join("\n\n")
        ))
    }

    pub(super) fn should_retry_after_file_edit_failure_correction(
        action_checkpoint_active: bool,
        file_edit_failure_correction_added: bool,
        file_edit_failure_retry_used: bool,
        successful_write_tool: bool,
    ) -> bool {
        action_checkpoint_active
            && file_edit_failure_correction_added
            && !file_edit_failure_retry_used
            && !successful_write_tool
    }

    pub(super) fn action_checkpoint_unexposed_tool_message(
        tool_name: &str,
        exposed_tool_names: &HashSet<String>,
        targeted_lookups_used: usize,
    ) -> String {
        let mut exposed = exposed_tool_names.iter().cloned().collect::<Vec<_>>();
        exposed.sort();
        format!(
            "Tool '{tool_name}' was not exposed in the current focused repair request and cannot be executed. Exposed tools: {}. Use file_edit/file_write/file_patch for patches so permission, stale-read, diff, and rollback checks stay active. Use file_read or grep only when it is exposed and the focused repair lookup budget still has room. Use bash only for validation after a file change. {} Do not call glob/project_list or repeat broad inspection.",
            exposed.join(", "),
            Self::targeted_lookup_budget_rule(targeted_lookups_used)
        )
    }

    pub(super) fn focused_repair_action_proposal(
        request: FocusedRepairActionRequest<'_>,
    ) -> Option<FocusedRepairActionProposal> {
        let failed_tool_boundary = !request.any_tool_success
            && request.batch_has_unsuccessful_tools
            && request.failed_tool_evidence_present;
        let should_intervene = request.action_checkpoint_active
            && (failed_tool_boundary || request.force_patch_synthesis_after_no_change);
        if !should_intervene {
            return None;
        }

        let next_no_change_rounds = request.action_checkpoint_no_change_rounds + 1;
        let lookup_rule = Self::targeted_lookup_budget_rule(request.action_checkpoint_lookup_count);
        let mut exposed = request
            .exposed_tool_names
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        exposed.sort();
        let reminder = format!(
            "Focused repair correction: the last tool call did not execute. The current request only permits these tools: {}. Use file_edit/file_write for exact replacements or line_start/line_end replacements from earlier line-numbered output. If a specific symbol or call site is missing, use the focused lookup budget, then patch. {}",
            exposed.join(", "),
            lookup_rule
        );
        let fallback_reason = if request.force_patch_synthesis_after_no_change {
            request
                .force_patch_synthesis_reason
                .unwrap_or("repeated no-change checkpoint")
                .to_string()
        } else {
            "repeated invalid tools in focused repair".to_string()
        };
        let trace_error = format!(
            "action checkpoint entered patch synthesis: {}",
            fallback_reason
        );

        Some(FocusedRepairActionProposal {
            reminder,
            next_no_change_rounds,
            enter_patch_synthesis: next_no_change_rounds >= 2,
            trace_error,
            fallback_owner: "action_checkpoint",
            fallback_reason,
        })
    }

    pub(super) fn bash_allowed_at_action_checkpoint(
        arguments: &serde_json::Value,
        has_changes_before_tools: bool,
    ) -> bool {
        let command = arguments["command"]
            .as_str()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if command.trim().is_empty() {
            return false;
        }
        let validation_markers = [
            "bash -n",
            "cargo test",
            "cargo check",
            "cargo fmt",
            "cargo clippy",
            "npm test",
            "npm run test",
            "pnpm test",
            "pytest",
            "make test",
            "scripts/run_live_eval.sh",
        ];
        has_changes_before_tools
            && validation_markers
                .iter()
                .any(|marker| command.contains(marker))
    }

    pub(super) fn action_checkpoint_file_edit_rejection(
        arguments: &serde_json::Value,
        cwd: &std::path::Path,
    ) -> Option<String> {
        let path = arguments["path"].as_str().unwrap_or_default().trim();
        if path.is_empty() {
            return Some("file_edit path is empty".to_string());
        }
        let raw_path = std::path::Path::new(path);
        for component in raw_path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Some(format!(
                        "file_edit path contains parent traversal: {}",
                        path
                    ));
                }
                std::path::Component::Normal(part)
                    if part == ".git" || part == "target" || part == "node_modules" =>
                {
                    return Some(format!(
                        "file_edit path targets ignored/generated directory: {}",
                        path
                    ));
                }
                _ => {}
            }
        }

        let expected_replacements = arguments["expected_replacements"]
            .as_u64()
            .map(|value| value as usize)
            .unwrap_or(1);
        if expected_replacements != 1 {
            return Some(format!(
                "action checkpoint only permits one replacement per file_edit call; got expected_replacements={}. Split the patch into single, reviewable edits.",
                expected_replacements
            ));
        }

        let new_string = arguments["new_string"].as_str().unwrap_or_default();
        if new_string.len() > 20_000 {
            return Some("file_edit new_string is too large for action checkpoint".to_string());
        }

        let old_string = arguments["old_string"].as_str();
        let insert_after = arguments["insert_after"].as_str();
        let insert_before = arguments["insert_before"].as_str();
        let line_start = arguments["line_start"].as_u64().map(|value| value as usize);
        let line_end = arguments["line_end"].as_u64().map(|value| value as usize);

        if let (Some(start), Some(end)) = (line_start, line_end) {
            if start == 0 || end == 0 || start > end {
                return Some(format!(
                    "file_edit line range is invalid: {}..={}",
                    start, end
                ));
            }
            if start != end {
                return Some(format!(
                    "action checkpoint line-range edits must touch exactly one line; got {}..={}. Use exact old_string for larger changes or split into single-line edits.",
                    start, end
                ));
            }
            if end.saturating_sub(start) + 1 > 40 {
                return Some(format!(
                    "action checkpoint line range is too large: {} lines. Use a smaller edit.",
                    end.saturating_sub(start) + 1
                ));
            }
            return None;
        }

        let has_edit_anchor =
            old_string.is_some() || insert_after.is_some() || insert_before.is_some();
        if !has_edit_anchor {
            return Some(
                "file_edit must use old_string, insert_after, insert_before, or line_start/line_end"
                    .to_string(),
            );
        }

        let candidate = if raw_path.is_absolute() {
            raw_path.to_path_buf()
        } else {
            cwd.join(raw_path)
        };
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let Ok(canonical_file) = candidate.canonicalize() else {
            return Some(format!("file_edit target does not exist: {}", path));
        };
        if !canonical_file.starts_with(&canonical_cwd) || !canonical_file.is_file() {
            return Some(format!(
                "file_edit target is outside the working tree: {}",
                path
            ));
        }
        let Ok(content) = std::fs::read_to_string(&canonical_file) else {
            return Some(format!("file_edit target is not readable: {}", path));
        };

        let anchor = old_string
            .or(insert_after)
            .or(insert_before)
            .unwrap_or_default();
        if anchor.trim().is_empty() {
            return Some("file_edit anchor is empty".to_string());
        }
        let count = content.matches(anchor).count();
        if count != 1 {
            return Some(format!(
                "action checkpoint requires a unique edit anchor; found {} occurrence(s). Use a more specific old_string or a small line_start/line_end range.",
                count
            ));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::tempdir;

    fn progress_request() -> ProgressCheckpointRequest {
        ProgressCheckpointRequest {
            no_diff_audit_closeout_allowed: false,
            has_worktree_changes: false,
            has_successful_validation_commands: false,
            no_code_progress_rounds: 0,
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            action_checkpoint_no_change_rounds: 0,
            no_diff_audit_validation_checkpoint_sent: false,
            code_write_tools_forbidden: false,
            code_write_forbidden_checkpoint_sent: false,
            used_action_checkpoint_lookup: false,
        }
    }

    #[test]
    fn audit_no_diff_validation_resets_rounds_and_marks_sent() {
        let decision =
            ProgressCheckpointController::evaluate_read_only_success(ProgressCheckpointRequest {
                no_diff_audit_closeout_allowed: true,
                no_code_progress_rounds: 1,
                ..progress_request()
            });

        assert_eq!(
            decision.action,
            ProgressCheckpointAction::AuditNoDiffValidation
        );
        assert_eq!(decision.no_code_progress_rounds, 0);
        assert!(decision.no_diff_audit_validation_checkpoint_sent);
        assert!(!decision.force_patch_synthesis_after_no_change);
    }

    #[test]
    fn existing_diff_repair_enters_patch_synthesis() {
        let decision =
            ProgressCheckpointController::evaluate_read_only_success(ProgressCheckpointRequest {
                has_worktree_changes: true,
                no_code_progress_rounds: 1,
                ..progress_request()
            });

        assert_eq!(
            decision.action,
            ProgressCheckpointAction::ExistingDiffNeedsRepair {
                no_code_progress_rounds: 2
            }
        );
        assert!(decision.action_checkpoint_active);
        assert_eq!(
            decision.action_checkpoint_lookup_count,
            ConversationLoop::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET
        );
        assert_eq!(decision.action_checkpoint_no_change_rounds, 2);
        assert!(decision.reset_file_edit_failure_retry);
        assert!(decision.force_patch_synthesis_after_no_change);
        assert_eq!(
            decision.force_patch_synthesis_reason,
            Some("existing diff still needs repair after repeated read-only rounds")
        );
    }

    #[test]
    fn focused_lookup_exhaustion_forces_patch_synthesis() {
        let decision =
            ProgressCheckpointController::evaluate_read_only_success(ProgressCheckpointRequest {
                action_checkpoint_active: true,
                action_checkpoint_lookup_count: 1,
                used_action_checkpoint_lookup: true,
                ..progress_request()
            });

        assert_eq!(
            decision.action,
            ProgressCheckpointAction::FocusedLookupNotice { exhausted: true }
        );
        assert_eq!(
            decision.action_checkpoint_lookup_count,
            ConversationLoop::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET
        );
        assert_eq!(decision.action_checkpoint_no_change_rounds, 1);
        assert!(decision.force_patch_synthesis_after_no_change);
        assert_eq!(
            decision.force_patch_synthesis_reason,
            Some("focused repair lookup budget exhausted")
        );
    }

    #[test]
    fn focused_repair_stalled_forces_patch_synthesis() {
        let decision =
            ProgressCheckpointController::evaluate_read_only_success(ProgressCheckpointRequest {
                action_checkpoint_active: true,
                action_checkpoint_no_change_rounds: 2,
                ..progress_request()
            });

        assert_eq!(
            decision.action,
            ProgressCheckpointAction::FocusedRepairStalled
        );
        assert_eq!(decision.action_checkpoint_no_change_rounds, 3);
        assert!(decision.force_patch_synthesis_after_no_change);
        assert_eq!(
            decision.force_patch_synthesis_reason,
            Some("focused repair lookup did not produce a patch")
        );
    }

    fn code_workflow(
        prompt: &str,
    ) -> crate::engine::code_change_workflow::CodeChangeWorkflowRunner {
        let route = crate::engine::intent_router::IntentRouter::new().route(prompt);
        let bundle = crate::engine::task_context::TaskContextBundle::new(prompt, ".", route, None);
        crate::engine::code_change_workflow::CodeChangeWorkflowRunner::new(&bundle)
    }

    fn trace() -> TraceCollector {
        TraceCollector::new(crate::engine::trace::TurnTrace::new("session", 1, "test"))
    }

    #[test]
    fn progress_action_appender_adds_system_checkpoint() {
        let trace = trace();
        let mut code_workflow = code_workflow("modify CLI status");
        let mut messages = Vec::new();
        let mut tool_results_text = String::new();

        ProgressCheckpointActionApplier::apply(ProgressCheckpointActionContext {
            action: ProgressCheckpointAction::ProgressReminder {
                no_code_progress_rounds: 2,
            },
            workflow: WorkflowKind::CodeChange,
            trace: &trace,
            code_workflow: &mut code_workflow,
            messages: &mut messages,
            tool_results_text: &mut tool_results_text,
        });

        assert_eq!(messages.len(), 1);
        assert!(matches!(
            &messages[0],
            Message::System { content } if content.contains("Workflow progress checkpoint")
        ));
        assert!(tool_results_text.contains("Workflow progress checkpoint"));
        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "code-change task needs an edit after repeated inspection"
        )));
    }

    #[test]
    fn existing_diff_action_activates_repeated_no_code_progress_trigger() {
        let trace = trace();
        let mut code_workflow = code_workflow("fix bug in parser");
        let mut messages = Vec::new();
        let mut tool_results_text = String::new();

        ProgressCheckpointActionApplier::apply(ProgressCheckpointActionContext {
            action: ProgressCheckpointAction::ExistingDiffNeedsRepair {
                no_code_progress_rounds: 2,
            },
            workflow: WorkflowKind::BugFix,
            trace: &trace,
            code_workflow: &mut code_workflow,
            messages: &mut messages,
            tool_results_text: &mut tool_results_text,
        });

        assert!(code_workflow
            .adaptive_trigger_labels()
            .contains(&"repeated_no_code_progress"));
        assert!(tool_results_text.contains("Workflow acceptance repair checkpoint"));
        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::AdaptiveWorkflowTriggered { trigger, .. }
                if trigger == "repeated_no_code_progress"
        )));
    }

    #[test]
    fn action_checkpoint_blocks_patch_bash_and_allows_validation_after_changes() {
        assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "python3 - <<'PY'\nfrom pathlib import Path\nPath('x').write_text('y')\nPY"}),
            false,
        ));
        assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "apply_patch <<'PATCH'\n*** Begin Patch\n*** End Patch\nPATCH"}),
            false,
        ));
        assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "cat > src/main.rs <<'EOF'\nfn main() {}\nEOF"}),
            false,
        ));
        assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "sed -n '1,20p' src/main.rs"}),
            false,
        ));
        assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "cargo test -q"}),
            false,
        ));
        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "cargo test -q"}),
            true,
        ));
        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke"}),
            true,
        ));
        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "bash -n scripts/run_live_eval.sh"}),
            true,
        ));
    }

    #[test]
    fn focused_repair_blocks_bash_patch_bypass() {
        for command in [
            "apply_patch <<'PATCH'\n*** Begin Patch\n*** End Patch\nPATCH",
            "python3 - <<'PY'\nopen('x', 'w').write('y')\nPY",
            "sed -i '' 's/a/b/' src/main.rs",
            "cat > src/main.rs <<'EOF'\nfn main() {}\nEOF",
            "tee src/main.rs <<'EOF'\nfn main() {}\nEOF",
        ] {
            assert!(
                !ConversationLoop::bash_allowed_at_action_checkpoint(
                    &serde_json::json!({ "command": command }),
                    true,
                ),
                "mutating bash command should not bypass file tools: {command}"
            );
        }

        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "cargo test -q"}),
            true,
        ));
    }

    #[test]
    fn focused_repair_prompt_allows_one_targeted_read_without_broad_tools() {
        let exposed = vec![
            "file_edit".to_string(),
            "file_read".to_string(),
            "grep".to_string(),
        ];

        let prompt = ConversationLoop::focused_repair_mode_prompt(&exposed, 0);

        assert!(prompt.contains("Up to 2 targeted file_read/grep lookups remain"));
        assert!(prompt.contains("Do not call glob/project_list"));
        assert!(prompt.contains("using file_edit/file_write/file_patch so permission"));
        assert!(prompt.contains("Do not use bash for patching"));
        assert!(!prompt.contains("Do not call grep/glob/file_read/project_list"));

        let prompt_after_one_lookup = ConversationLoop::focused_repair_mode_prompt(&exposed, 1);
        assert!(prompt_after_one_lookup.contains("One targeted file_read/grep lookup remains"));

        let prompt_after_budget = ConversationLoop::focused_repair_mode_prompt(&exposed, 2);
        assert!(prompt_after_budget.contains("targeted lookup budget has already been used"));
        assert!(prompt_after_budget.contains("do not call file_read/grep again"));
    }

    #[test]
    fn file_edit_failure_correction_prefers_line_range_retry() {
        let correction = ConversationLoop::file_edit_failure_repair_correction(&[r#"
file_edit call_1 failed:
Expected 1 occurrence(s) of old_string, but found 1487.
  ... showing first 12 of 1487 matches. The old_string is too broad.
"#
        .to_string()])
        .expect("ambiguous file_edit should produce a correction");

        assert!(correction.contains("line_start, line_end"));
        assert!(correction.contains("Do not retry the same broad old_string"));
        assert!(correction.contains("not close out"));
    }

    #[test]
    fn file_edit_failure_correction_gets_one_model_retry_before_synthesis() {
        assert!(
            ConversationLoop::should_retry_after_file_edit_failure_correction(
                true, true, false, false,
            )
        );
        assert!(
            !ConversationLoop::should_retry_after_file_edit_failure_correction(
                true, true, true, false,
            )
        );
        assert!(
            !ConversationLoop::should_retry_after_file_edit_failure_correction(
                true, true, false, true,
            )
        );
        assert!(
            !ConversationLoop::should_retry_after_file_edit_failure_correction(
                false, true, false, false,
            )
        );
    }

    #[test]
    fn action_checkpoint_unexposed_tool_message_lists_allowed_tools() {
        let exposed = HashSet::from([
            "file_edit".to_string(),
            "file_read".to_string(),
            "grep".to_string(),
        ]);

        let message =
            ConversationLoop::action_checkpoint_unexposed_tool_message("project_list", &exposed, 0);

        assert!(message.contains("project_list"));
        assert!(message.contains("Exposed tools: file_edit, file_read, grep"));
        assert!(message.contains("Use file_edit/file_write/file_patch for patches"));
        assert!(message.contains("lookup budget still has room"));
        assert!(message.contains("Up to 2 targeted file_read/grep lookups remain"));

        let exhausted =
            ConversationLoop::action_checkpoint_unexposed_tool_message("file_read", &exposed, 2);
        assert!(exhausted.contains("targeted lookup budget has already been used"));
    }

    #[test]
    fn focused_repair_action_proposal_records_budget_and_fallback_reason() {
        let exposed = HashSet::from([
            "grep".to_string(),
            "file_edit".to_string(),
            "file_read".to_string(),
        ]);

        let proposal =
            ConversationLoop::focused_repair_action_proposal(FocusedRepairActionRequest {
                action_checkpoint_active: true,
                any_tool_success: false,
                batch_has_unsuccessful_tools: true,
                failed_tool_evidence_present: true,
                force_patch_synthesis_after_no_change: false,
                force_patch_synthesis_reason: None,
                action_checkpoint_no_change_rounds: 0,
                action_checkpoint_lookup_count: 1,
                exposed_tool_names: &exposed,
            })
            .expect("focused repair failure should propose a recovery action");

        assert!(!proposal.enter_patch_synthesis);
        assert_eq!(proposal.next_no_change_rounds, 1);
        assert_eq!(proposal.fallback_owner, "action_checkpoint");
        assert_eq!(
            proposal.fallback_reason,
            "repeated invalid tools in focused repair"
        );
        assert!(proposal.reminder.contains("file_edit, file_read, grep"));
        assert!(proposal
            .reminder
            .contains("One targeted file_read/grep lookup remains"));
    }

    #[test]
    fn focused_repair_action_proposal_enters_patch_synthesis_after_budget() {
        let exposed = HashSet::from(["file_edit".to_string()]);

        let proposal =
            ConversationLoop::focused_repair_action_proposal(FocusedRepairActionRequest {
                action_checkpoint_active: true,
                any_tool_success: true,
                batch_has_unsuccessful_tools: false,
                failed_tool_evidence_present: false,
                force_patch_synthesis_after_no_change: true,
                force_patch_synthesis_reason: Some("focused repair lookup budget exhausted"),
                action_checkpoint_no_change_rounds: 1,
                action_checkpoint_lookup_count: 2,
                exposed_tool_names: &exposed,
            })
            .expect("forced no-change repair should propose patch synthesis");

        assert!(proposal.enter_patch_synthesis);
        assert_eq!(proposal.next_no_change_rounds, 2);
        assert_eq!(
            proposal.fallback_reason,
            "focused repair lookup budget exhausted"
        );
        assert!(proposal
            .trace_error
            .contains("focused repair lookup budget exhausted"));
        assert!(proposal
            .reminder
            .contains("targeted lookup budget has already been used"));
    }

    #[test]
    fn action_checkpoint_rejects_multi_replacement_file_edit() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let status = true;\nlet status = false;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "let status",
                "new_string": "let checked_status",
                "expected_replacements": 2
            }),
            tmp.path(),
        )
        .expect("multi replacement edit should be rejected");

        assert!(rejection.contains("only permits one replacement"));
    }

    #[test]
    fn action_checkpoint_rejects_non_unique_anchor() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let status = true;\nlet status = false;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "let status",
                "new_string": "let checked_status"
            }),
            tmp.path(),
        )
        .expect("non-unique anchor should be rejected");

        assert!(rejection.contains("unique edit anchor"));
    }

    #[test]
    fn action_checkpoint_rejects_multi_line_range_edit() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let write_decision = score();\nlet score = write_decision.score;\nlet status = write_decision.status;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "line_start": 1,
                "line_end": 3,
                "new_string": "let status = write_decision.status;"
            }),
            tmp.path(),
        )
        .expect("multi-line action checkpoint edit should be rejected");

        assert!(rejection.contains("exactly one line"));
    }

    #[test]
    fn action_checkpoint_accepts_unique_anchor() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let status = true;\nlet other = false;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "let status = true;",
                "new_string": "let status = false;"
            }),
            tmp.path(),
        );

        assert!(rejection.is_none(), "{rejection:?}");
    }
}
