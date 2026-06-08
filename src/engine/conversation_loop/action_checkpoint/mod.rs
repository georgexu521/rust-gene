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
    #[allow(dead_code)]
    pub(super) force_patch_synthesis_after_no_change: bool,
    #[allow(dead_code)]
    pub(super) force_patch_synthesis_reason: Option<&'static str>,
}

#[cfg(test)]
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

#[cfg(test)]
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
                Self::record_no_diff_route_recovery(
                    context.trace,
                    context.workflow,
                    rounds,
                    "existing diff still needs repair after repeated read-only rounds",
                );
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
                Self::record_no_diff_route_recovery(
                    context.trace,
                    context.workflow,
                    rounds,
                    "code-change task made no edit after repeated inspection",
                );
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
                Self::record_no_diff_route_recovery(
                    context.trace,
                    context.workflow,
                    3,
                    "focused repair lookup did not produce a patch",
                );
                context.trace.record(TraceEvent::WorkflowFallback {
                    error:
                        "action checkpoint entered patch synthesis after repeated focused repair reads"
                            .to_string(),
                });
            }
        }
    }

    fn record_no_diff_route_recovery(
        trace: &TraceCollector,
        workflow: WorkflowKind,
        no_code_progress_rounds: usize,
        reason: &str,
    ) {
        let decision = crate::engine::route_recovery::no_diff_code_change_decision(
            workflow,
            no_code_progress_rounds,
            reason,
        );
        super::turn_recording::record_recovery_plan(trace, &decision.plan);
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
        messages
            .push(super::request_preparation_controller::recent_observation_message(&checkpoint));
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

    #[cfg(test)]
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
                    || evidence.contains("Could not find old_string")
                    || evidence.contains("old_string not found")
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
            "File edit repair correction:\n{}\nNext action is still a patch, not closeout. The previous file_edit did not modify a file because its anchor was missing, stale, empty, whitespace-only, or non-unique. Use one of these safer forms:\n- If prior file_read/grep output shows the target line number, call file_edit with path, line_start, line_end, and new_string for that exact line.\n- If the old_string was not found, re-read the target at most once, then patch using the latest line numbers or an exact old_string copied from that current content.\n- Otherwise copy a multi-line old_string that includes the surrounding function call and is unique exactly once.\nDo not retry the same broad or stale old_string. Do not close out until a file_edit/file_write succeeds and validation runs.",
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

    #[cfg(test)]
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
        _exposed_tool_names: &std::collections::HashSet<String>,
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
        if has_changes_before_tools
            && validation_markers
                .iter()
                .any(|marker| command.contains(marker))
        {
            return true;
        }

        command.starts_with("python -m venv .venv")
            || command.starts_with("python3 -m venv .venv")
            || command.contains("pip install")
                && command.contains("fixtures/core_quality/terminal_app")
            || command.contains(
                "fixtures/core_quality/long_output/generate_log.py > fixtures/core_quality/long_output/output.log",
            )
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
#[cfg(test)]
mod tests;
