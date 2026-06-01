use super::tool_context_helpers::tool_call_fingerprint;
use super::tool_execution::is_read_only;
use super::turn_focused_repair_flow_controller::{
    TurnFocusedRepairFlow, TurnFocusedRepairFlowContext, TurnFocusedRepairFlowController,
};
use super::turn_iteration_setup_controller::{
    TurnIterationSetupContext, TurnIterationSetupController, TurnIterationSetupFlow,
};
use super::turn_loop_state_controller::TurnLoopState;
use super::turn_model_step_controller::{
    TurnModelStepContext, TurnModelStepController, TurnModelStepFlow,
};
use super::turn_post_change_closeout_controller::{
    TurnPostChangeCloseoutContext, TurnPostChangeCloseoutController,
};
use super::turn_runtime_context::TurnRuntimeContext;
use super::turn_runtime_state::TurnRuntimeState;
use super::turn_tool_failure_followup_controller::{
    TurnToolFailureFollowupContext, TurnToolFailureFollowupController, TurnToolFailureFollowupFlow,
};
use super::turn_tool_round_step_controller::{
    TurnToolRoundStepContext, TurnToolRoundStepController,
};
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::conversation_loop::main_loop_profile::MainLoopProfile;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::stop_checker::{StopCheckInput, StopChecker};
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::{
    mva_stage_transition_policy, AgentToolRoundObservation, TaskContextBundle,
};
use crate::engine::task_contract::TaskContractBundleExt;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{Message, Tool, ToolCall};
use crate::tools::ToolContextRetainedContext;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub(super) struct TurnIterationContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) iteration: usize,
    pub(super) route: &'a IntentRoute,
    pub(super) profile: MainLoopProfile,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) turn_retrieval_context: Option<&'a RetrievalContext>,
    pub(super) retained_context: &'a ToolContextRetainedContext,
    pub(super) base_tools: &'a [Tool],
    pub(super) available_tools: &'a [Tool],
    pub(super) loop_state: &'a mut TurnLoopState,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) no_diff_audit_closeout_allowed: bool,
    pub(super) code_write_tools_forbidden: bool,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) working_dir: &'a Path,
    pub(super) last_user_preview: &'a str,
    pub(super) required_validation_commands: &'a [String],
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

pub(super) enum TurnIterationFlow {
    Continue,
    Break,
}

pub(super) struct TurnIterationController;

impl TurnIterationController {
    pub(super) async fn run(
        context: TurnIterationContext<'_>,
    ) -> anyhow::Result<TurnIterationFlow> {
        // Force-summary: inject wrap-up prompt before the last 2 iterations
        // so the model has time to stop calling tools and produce a final answer.
        // Mirrors Reasonix's forceSummaryAfterIterLimit pattern.
        if crate::engine::conversation_loop::force_summary::should_force_summary(
            context.iteration,
            context.conversation.max_iterations,
        ) && context.loop_state.final_content.is_empty()
        {
            let summary_msg =
                crate::engine::conversation_loop::force_summary::force_summary_message();
            context.messages.push(summary_msg);
            context.trace.record(TraceEvent::WorkflowFallback {
                error: format!(
                    "iteration {} of {} — injecting force-summary prompt",
                    context.iteration + 1,
                    context.conversation.max_iterations
                ),
            });
        }

        let model_profile = context
            .task_bundle
            .task_contract(context.required_validation_commands)
            .model_profile;
        let exposure_plan = match TurnIterationSetupController::run(TurnIterationSetupContext {
            iteration: context.iteration,
            max_iterations: context.conversation.max_iterations,
            turn_state: &mut *context.turn_state,
            memory_manager: context.conversation.memory_manager.as_ref(),
            trace: context.trace,
            route_workflow: context.route.workflow,
            task_stage: context.task_bundle.agent_state.stage,
            baseline_git_status_files: context.baseline_git_status_files,
            base_tools: context.base_tools,
            available_tools: context.available_tools,
            required_validation_commands_present: !context.required_validation_commands.is_empty(),
            model_profile,
        })
        .await
        {
            TurnIterationSetupFlow::Continue { exposure_plan } => exposure_plan,
            TurnIterationSetupFlow::Stop => return Ok(TurnIterationFlow::Break),
        };
        let tools = exposure_plan.tools;
        let exposed_tool_names = exposure_plan.exposed_tool_names;

        let (content, mut tool_calls, pre_executed) =
            match TurnModelStepController::run(TurnModelStepContext {
                conversation: context.conversation,
                iteration: context.iteration + 1,
                route: context.route,
                profile: context.profile,
                code_workflow: &*context.code_workflow,
                task_bundle: &*context.task_bundle,
                required_validation_commands: context.required_validation_commands,
                turn_retrieval_context: context.turn_retrieval_context,
                focused_repair_prompt: exposure_plan.focused_repair_prompt,
                tools: &tools,
                exposed_tool_names: &exposed_tool_names,
                resource_policy: context.resource_policy,
                loop_state: &mut *context.loop_state,
                turn_state: &mut *context.turn_state,
                messages: &mut *context.messages,
                trace: context.trace,
                tx: context.tx,
            })
            .await?
            {
                TurnModelStepFlow::Retry => {
                    context.loop_state.consecutive_empty_rounds = 0;
                    return Ok(TurnIterationFlow::Continue);
                }
                TurnModelStepFlow::Finish => {
                    // Double-tap finish: only break after TWO consecutive
                    // responses without tool calls. The first one may be the
                    // model "thinking out loud" before acting (e.g. "next
                    // I'll read X, then Y"). This matches Claude Code
                    // behavior and prevents premature stopping.
                    if context.loop_state.final_content.trim().is_empty() {
                        // Empty content is always bad — prompt the model.
                        if context.iteration < context.conversation.max_iterations {
                            context.messages.push(Message::system(
                                "Your last response was empty. Please summarize what you \
                                 have learned from the tool results above in a few \
                                 sentences, or call tools if you need more information."
                                    .to_string(),
                            ));
                            context.trace.record(TraceEvent::WorkflowFallback {
                                error: "empty assistant response — injecting retry prompt"
                                    .to_string(),
                            });
                            return Ok(TurnIterationFlow::Continue);
                        }
                        return Ok(TurnIterationFlow::Break);
                    }
                    context.loop_state.consecutive_empty_rounds += 1;
                    if context.loop_state.consecutive_empty_rounds < 2 {
                        // First non-tool response: let the model continue.
                        context.trace.record(TraceEvent::WorkflowFallback {
                            error: format!(
                                "non-tool response (round {}/{}) — letting model continue",
                                context.loop_state.consecutive_empty_rounds, 2
                            ),
                        });
                        // But if budget is nearly exhausted, break anyway.
                        if context.iteration + 2 >= context.conversation.max_iterations {
                            return Ok(TurnIterationFlow::Break);
                        }
                        return Ok(TurnIterationFlow::Continue);
                    }
                    return Ok(TurnIterationFlow::Break);
                }
                TurnModelStepFlow::ToolRound {
                    content,
                    tool_calls,
                    pre_executed,
                } => {
                    context.loop_state.consecutive_empty_rounds = 0;
                    (content, tool_calls, pre_executed)
                }
            };

        let redirected_directory_reads =
            redirect_duplicate_directory_file_reads(&mut tool_calls, context.turn_state);
        if redirected_directory_reads > 0 {
            context.trace.record(TraceEvent::WorkflowFallback {
                error: format!(
                    "redirected {redirected_directory_reads} duplicate directory read(s) to the single listed file"
                ),
            });
        }

        if duplicate_read_only_closeout_allowed(context.route, context.required_validation_commands)
        {
            if let Some(message) = duplicate_successful_read_only_pre_execution_closeout(
                &tool_calls,
                context.turn_state,
                context.last_user_preview,
                context.conversation.session_store.as_ref(),
                &context.conversation.session_id,
            ) {
                context.loop_state.final_content.push_str(&message);
                if let Some(tx) = context.tx {
                    let _ = tx.send(StreamEvent::TextChunk(message)).await;
                }
                return Ok(TurnIterationFlow::Break);
            }
        }
        if pre_executed.is_empty() {
            if let Some(filtered) =
                drop_duplicate_successful_read_only_tool_calls(&tool_calls, context.turn_state)
            {
                let dropped = tool_calls.len().saturating_sub(filtered.len());
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: format!(
                        "dropped {dropped} duplicate successful read-only tool call(s) before executing mixed tool batch"
                    ),
                });
                tool_calls = filtered;
            }
        }

        let mut tool_round_state = TurnToolRoundStepController::run(TurnToolRoundStepContext {
            conversation: context.conversation,
            content: &content,
            tool_calls: &tool_calls,
            pre_executed,
            runtime: TurnRuntimeContext {
                tx: context.tx,
                trace: context.trace,
                route: context.route,
                resource_policy: context.resource_policy,
                task_stage: context.task_bundle.agent_state.stage,
                exposed_tool_names: &exposed_tool_names,
                working_dir: context.working_dir,
                last_user_preview: context.last_user_preview,
                required_validation_commands: context.required_validation_commands,
                destructive_scope: context.destructive_scope,
                baseline_git_status_files: context.baseline_git_status_files,
                retained_context: context.retained_context,
            },
            turn_state: &mut *context.turn_state,
            task_bundle: &mut *context.task_bundle,
            messages: &mut *context.messages,
            is_programming_workflow: crate::engine::code_change_workflow::is_programming_workflow(
                context.route.workflow,
            ),
            loop_state: &mut *context.loop_state,
        })
        .await;
        context
            .task_bundle
            .agent_state
            .observe_tool_round(AgentToolRoundObservation {
                any_tool_success: tool_round_state.any_tool_success,
                batch_has_unsuccessful_tools: tool_round_state.batch_has_unsuccessful_tools,
                used_write_tool: tool_round_state.used_write_tool,
                successful_write_tool: tool_round_state.successful_write_tool,
                has_worktree_changes: tool_round_state.has_worktree_changes(),
                has_successful_validation_commands: tool_round_state
                    .has_successful_validation_commands(),
                failed_tool_evidence_present: tool_round_state.failed_tool_evidence_present(),
            });
        record_stop_check(
            context.trace,
            context.task_bundle,
            context.turn_state,
            &tool_round_state,
            exposed_tool_names.len(),
            tool_calls.len(),
            false,
        );

        // NOTE: Unlike previous behavior, we no longer inject a synthesis-only
        // prompt when the model re-reads the same file.  Reasonix lets the model
        // decide when it has enough information; the iteration budget (force
        // summary after max iterations) is the safety net for read-only loops.
        // The storm breaker still guards against mutating-call storms.

        if duplicate_read_only_closeout_allowed(context.route, context.required_validation_commands)
        {
            if let Some(message) = duplicate_successful_read_only_closeout(
                &tool_round_state,
                context.last_user_preview,
            ) {
                context.loop_state.final_content.push_str(&message);
                if let Some(tx) = context.tx {
                    let _ = tx.send(StreamEvent::TextChunk(message)).await;
                }
                return Ok(TurnIterationFlow::Break);
            }
        }

        let focused_repair_flow =
            TurnFocusedRepairFlowController::run(TurnFocusedRepairFlowContext {
                conversation: context.conversation,
                workflow: context.route.workflow,
                no_diff_audit_closeout_allowed: context.no_diff_audit_closeout_allowed,
                code_write_tools_forbidden: context.code_write_tools_forbidden,
                trace: context.trace,
                code_workflow: &mut *context.code_workflow,
                turn_state: &mut *context.turn_state,
                round_state: &mut tool_round_state,
                exposed_tool_names: &exposed_tool_names,
                tx: context.tx,
                resource_policy: context.resource_policy,
                destructive_scope: context.destructive_scope,
                baseline_git_status_files: context.baseline_git_status_files,
                working_dir: context.working_dir,
                last_user_preview: context.last_user_preview,
                messages: &mut *context.messages,
                final_content: &mut context.loop_state.final_content,
                final_tool_calls: &mut context.loop_state.final_tool_calls,
            })
            .await;
        record_stop_check(
            context.trace,
            context.task_bundle,
            context.turn_state,
            &tool_round_state,
            exposed_tool_names.len(),
            tool_calls.len(),
            matches!(
                focused_repair_flow,
                TurnFocusedRepairFlow::Continue | TurnFocusedRepairFlow::Stop
            ),
        );
        // ── Advisory-only post-tool checks ──
        // Reasonix alignment: only 4 hard-stop conditions exist:
        // 1. Budget exhausted → force summary
        // 2. User abort (handled upstream)
        // 3. No tool calls after 2 consecutive Finishes (double-tap)
        // 4. API error
        //
        // All other checks below are advisory — they record traces and
        // update task state but NEVER break the loop. The iteration budget
        // + force summary are the ultimate safety net.
        if matches!(focused_repair_flow, TurnFocusedRepairFlow::Continue) {
            return Ok(TurnIterationFlow::Continue);
        }

        let followup_flow =
            TurnToolFailureFollowupController::run(TurnToolFailureFollowupContext {
                provider: context.conversation.provider.as_ref(),
                model: context.conversation.model.clone(),
                session_store: context.conversation.session_store.as_ref(),
                session_id: &context.conversation.session_id,
                trace: context.trace,
                any_tool_success: tool_round_state.any_tool_success,
                last_user_preview: context.last_user_preview,
                task_bundle: &mut *context.task_bundle,
                round_state: &mut tool_round_state,
                turn_state: context.turn_state,
                failed_tool_names: &context.loop_state.failed_tool_names,
                tx: context.tx,
                final_content: &mut context.loop_state.final_content,
                messages: &mut *context.messages,
            })
            .await;
        if let Some(flow) = flow_after_tool_failure_followup(followup_flow) {
            return Ok(flow);
        }

        let _closeout_flow = TurnPostChangeCloseoutController::run(TurnPostChangeCloseoutContext {
            conversation: context.conversation,
            trace: context.trace,
            route: context.route,
            code_workflow: &mut *context.code_workflow,
            task_bundle: &mut *context.task_bundle,
            round_state: &mut tool_round_state,
            required_validation_commands: context.required_validation_commands,
            successful_required_validation_commands: &mut context
                .loop_state
                .successful_required_validation_commands,
            turn_state: &mut *context.turn_state,
            final_content: &mut context.loop_state.final_content,
            messages: &mut *context.messages,
            last_user_preview: context.last_user_preview,
        })
        .await;

        Ok(TurnIterationFlow::Continue)
    }
}

fn flow_after_tool_failure_followup(
    followup_flow: TurnToolFailureFollowupFlow,
) -> Option<TurnIterationFlow> {
    match followup_flow {
        TurnToolFailureFollowupFlow::Stop => Some(TurnIterationFlow::Break),
        TurnToolFailureFollowupFlow::Continue => None,
    }
}

fn record_stop_check(
    trace: &TraceCollector,
    task_bundle: &mut TaskContextBundle,
    turn_state: &TurnRuntimeState,
    tool_round_state: &super::turn_tool_round_outcome_controller::TurnToolRoundState,
    exposed_tool_count: usize,
    selected_tool_calls: usize,
    force_patch_synthesis_after_no_change: bool,
) {
    let stage_before = task_bundle.agent_state.stage;
    let observations_before = task_bundle.agent_state.observations.len();
    let key_findings_before = task_bundle.agent_state.key_findings.len();
    let duplicate_read_only_tools =
        if crate::engine::code_change_workflow::is_programming_workflow(task_bundle.route.workflow)
            || !task_bundle
                .agent_state
                .verification_plan
                .required_checks
                .is_empty()
        {
            0
        } else {
            tool_round_state.duplicate_successful_read_only_tools.len()
        };
    let decision = StopChecker::evaluate(StopCheckInput {
        any_tool_success: tool_round_state.any_tool_success,
        successful_write_tool: tool_round_state.successful_write_tool,
        has_successful_validation_commands: tool_round_state.has_successful_validation_commands(),
        no_code_progress_rounds: turn_state.focused_repair.no_code_progress_rounds,
        action_checkpoint_active: turn_state.focused_repair.action_checkpoint_active,
        action_checkpoint_no_change_rounds: turn_state
            .focused_repair
            .action_checkpoint_no_change_rounds,
        force_patch_synthesis_after_no_change,
        repeated_failed_tools: tool_round_state.repeated_failed_tools.len(),
        duplicate_read_only_tools,
        max_iterations_reached: false,
        uncertainty_not_reduced_steps: task_bundle.agent_state.uncertainty_not_reduced_steps,
        consecutive_validation_failures: task_bundle.agent_state.consecutive_validation_failures,
        consecutive_edit_failures: task_bundle.agent_state.consecutive_edit_failures,
        consecutive_command_failures: task_bundle.agent_state.consecutive_command_failures,
        consecutive_permission_blocks: task_bundle.agent_state.consecutive_permission_blocks,
        consecutive_low_action_scores: task_bundle.agent_state.consecutive_low_action_scores(),
        consecutive_high_risk_low_value_actions: task_bundle
            .agent_state
            .consecutive_high_risk_low_value_actions(),
        score_without_uncertainty_reduction_rounds: task_bundle
            .agent_state
            .score_without_uncertainty_reduction_rounds(),
        repeated_revised_action_count: task_bundle.agent_state.repeated_revised_action_count(),
        user_interrupted: false,
        model_output_invalid_attempts: 0,
        action_review_decision: None,
        action_review_reason: None,
        rollback_candidate: task_bundle.agent_state.rollback_candidates.last().cloned(),
        failure_type: task_bundle.agent_state.last_failure_family.clone(),
        recovery_plan_id: None,
    });
    StopChecker::apply_to_task_state(&mut task_bundle.agent_state, &decision);
    let stage_after = task_bundle.agent_state.stage;
    let observations_delta = task_bundle
        .agent_state
        .observations
        .len()
        .saturating_sub(observations_before);
    let key_findings_delta = task_bundle
        .agent_state
        .key_findings
        .len()
        .saturating_sub(key_findings_before);
    trace.record(TraceEvent::StopCheckEvaluated {
        status: decision.status.label().to_string(),
        reason: decision.reason.label().to_string(),
        stage: format!("{:?}", task_bundle.agent_state.stage),
        terminal_status: decision
            .terminal_status
            .map(|status| status.label().to_string()),
        action: decision.action.label().to_string(),
        no_code_progress_rounds: decision.no_code_progress_rounds,
        action_checkpoint_active: decision.action_checkpoint_active,
        summary: decision.summary,
        evidence_items: decision.evidence.len(),
        failure_type: decision.failure_type.clone(),
        recovery_plan_id: decision.recovery_plan_id.clone(),
        rollback_recommended: decision.rollback_candidate.is_some(),
        next_action: decision.next_action.clone(),
    });
    let latest_action_score = task_bundle
        .agent_state
        .action_score_history
        .last()
        .map(|record| record.action_score);
    trace.record(TraceEvent::AgentLoopStepEvaluated {
        route_workflow: serde_label(&task_bundle.route.workflow),
        route_risk: serde_label(&task_bundle.route.risk),
        task_mode: serde_label(&task_bundle.agent_state.mode),
        stage_before: format!("{stage_before:?}"),
        stage_after: format!("{stage_after:?}"),
        mva_stage_before: stage_before.mva_stage_label().to_string(),
        mva_stage_after: stage_after.mva_stage_label().to_string(),
        stage_transition_policy: mva_stage_transition_policy(stage_before, stage_after).to_string(),
        exposed_tools: exposed_tool_count,
        selected_tool_calls,
        action_score_records: task_bundle.agent_state.action_score_history.len(),
        latest_action_score,
        observations_delta,
        key_findings_delta,
        stop_status: decision.status.label().to_string(),
        stop_reason: decision.reason.label().to_string(),
        stop_action: decision.action.label().to_string(),
        terminal_status: decision
            .terminal_status
            .map(|status| status.label().to_string()),
        state_delta: format!(
            "stage_changed={} observations_delta={} key_findings_delta={} latest_progress={}",
            stage_before != stage_after,
            observations_delta,
            key_findings_delta,
            task_bundle
                .agent_state
                .last_progress_signal
                .as_deref()
                .unwrap_or("none")
        ),
    });
}

fn serde_label<T>(value: &T) -> String
where
    T: serde::Serialize + std::fmt::Debug,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{value:?}"))
}

fn duplicate_successful_read_only_pre_execution_closeout(
    tool_calls: &[ToolCall],
    turn_state: &TurnRuntimeState,
    last_user_preview: &str,
    session_store: Option<&std::sync::Arc<crate::session_store::SessionStore>>,
    session_id: &str,
) -> Option<String> {
    if tool_calls.is_empty() {
        return None;
    }

    let mut duplicate_results = Vec::new();
    for tool_call in tool_calls {
        if !is_read_only(&tool_call.name) {
            return None;
        }
        let fingerprint = tool_call_fingerprint(tool_call);
        if !turn_state
            .successful_read_only_tool_fingerprints
            .contains_key(&fingerprint)
        {
            return None;
        }
        let cached = turn_state
            .successful_read_only_tool_results
            .get(&fingerprint)?
            .trim();
        if cached.is_empty() {
            return None;
        }
        let ledger_summary =
            duplicate_tool_call_ledger_summary(session_store, session_id, tool_call);
        if is_read_cache_notice(cached)
            && normalized_result_lines(cached).is_empty()
            && ledger_summary.is_none()
        {
            return None;
        }
        duplicate_results.push((tool_call.name.as_str(), cached, ledger_summary));
    }

    let parts = duplicate_results
        .iter()
        .map(|(tool_name, result_text, ledger_summary)| {
            (*tool_name, *result_text, ledger_summary.as_deref())
        })
        .collect::<Vec<_>>();
    synthesize_read_only_duplicate_answer_from_parts(&parts, last_user_preview)
}

fn duplicate_read_only_closeout_allowed(
    route: &IntentRoute,
    required_validation_commands: &[String],
) -> bool {
    let _ = (route, required_validation_commands);
    false
}

// Removed: duplicate_successful_read_only_synthesis_prompt + read_only_tool_label
// — Reasonix alignment. Read-only tools always allowed through; iteration
// budget handles loops naturally.

fn drop_duplicate_successful_read_only_tool_calls(
    tool_calls: &[ToolCall],
    turn_state: &TurnRuntimeState,
) -> Option<Vec<ToolCall>> {
    let _ = (tool_calls, turn_state);
    None
}

#[cfg(test)]
#[allow(dead_code)]
fn legacy_drop_duplicate_successful_read_only_tool_calls(
    tool_calls: &[ToolCall],
    turn_state: &TurnRuntimeState,
) -> Option<Vec<ToolCall>> {
    if tool_calls.len() < 2 {
        return None;
    }

    let mut filtered = Vec::with_capacity(tool_calls.len());
    let mut dropped = 0usize;
    for tool_call in tool_calls {
        let duplicate_successful_read = is_read_only(&tool_call.name)
            && turn_state
                .successful_read_only_tool_fingerprints
                .contains_key(&tool_call_fingerprint(tool_call));
        if duplicate_successful_read {
            dropped += 1;
        } else {
            filtered.push(tool_call.clone());
        }
    }

    if dropped > 0 && !filtered.is_empty() {
        Some(filtered)
    } else {
        None
    }
}

fn redirect_duplicate_directory_file_reads(
    tool_calls: &mut [ToolCall],
    turn_state: &TurnRuntimeState,
) -> usize {
    let _ = (tool_calls, turn_state);
    0
}

#[cfg(test)]
#[allow(dead_code)]
fn legacy_redirect_duplicate_directory_file_reads(
    tool_calls: &mut [ToolCall],
    turn_state: &TurnRuntimeState,
) -> usize {
    let mut redirected = 0usize;
    for tool_call in tool_calls {
        if tool_call.name != "file_read" {
            continue;
        }
        let fingerprint = tool_call_fingerprint(tool_call);
        if !turn_state
            .successful_read_only_tool_fingerprints
            .contains_key(&fingerprint)
        {
            continue;
        }
        let Some(result_text) = turn_state
            .successful_read_only_tool_results
            .get(&fingerprint)
        else {
            continue;
        };
        let Some(entry) = single_file_entry_from_directory_listing(result_text) else {
            continue;
        };
        let Some(path) = tool_call
            .arguments
            .get("path")
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        let child_path = format!("{}/{}", path.trim_end_matches('/'), entry);
        let Some(arguments) = tool_call.arguments.as_object_mut() else {
            continue;
        };
        arguments.insert("path".to_string(), serde_json::Value::String(child_path));
        redirected += 1;
    }
    redirected
}

#[cfg(test)]
#[allow(dead_code)]
fn single_file_entry_from_directory_listing(result_text: &str) -> Option<String> {
    let lines = normalized_result_lines(result_text);
    if !lines.iter().any(|line| line.starts_with("Directory:")) {
        return None;
    }
    let mut entries = Vec::new();
    let mut in_entries = false;
    for line in lines {
        if line.starts_with("Entries ") {
            in_entries = true;
            continue;
        }
        if in_entries {
            if line.starts_with("Result:") || line.starts_with("Directory:") {
                continue;
            }
            entries.push(line);
        }
    }
    let files = entries
        .into_iter()
        .filter(|entry| {
            !entry.ends_with('/')
                && !entry.contains('/')
                && !entry.contains('\\')
                && entry != "."
                && entry != ".."
        })
        .collect::<Vec<_>>();
    if files.len() == 1 {
        files.into_iter().next()
    } else {
        None
    }
}

fn duplicate_successful_read_only_closeout(
    round_state: &super::turn_tool_round_outcome_controller::TurnToolRoundState,
    last_user_preview: &str,
) -> Option<String> {
    if round_state.duplicate_successful_read_only_tools.is_empty() {
        return None;
    }
    let parts = round_state
        .duplicate_successful_read_only_results
        .iter()
        .map(|duplicate| {
            (
                duplicate.tool_name.as_str(),
                duplicate.result_text.as_str(),
                duplicate.ledger_summary.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    synthesize_read_only_duplicate_answer_from_parts(&parts, last_user_preview)
}

fn synthesize_read_only_duplicate_answer_from_parts(
    parts: &[(&str, &str, Option<&str>)],
    last_user_preview: &str,
) -> Option<String> {
    if parts.is_empty() {
        return None;
    }
    let result_text = parts
        .iter()
        .map(|(_, result_text, _)| result_text.trim())
        .filter(|result_text| !result_text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if result_text.trim().is_empty() {
        return None;
    }
    let first_tool = parts[0].0;
    let tool_name = if parts
        .iter()
        .all(|(tool_name, _, _)| *tool_name == first_tool)
    {
        first_tool
    } else {
        "read-only tools"
    };
    let ledger_summary = parts
        .iter()
        .filter_map(|(_, _, ledger_summary)| *ledger_summary)
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    let ledger_summary = (!ledger_summary.is_empty()).then_some(ledger_summary.as_str());
    Some(synthesize_read_only_duplicate_answer(
        tool_name,
        &result_text,
        last_user_preview,
        ledger_summary,
    ))
}

fn synthesize_read_only_duplicate_answer(
    tool_name: &str,
    result_text: &str,
    last_user_preview: &str,
    ledger_summary: Option<&str>,
) -> String {
    let chinese = contains_cjk(last_user_preview);
    let summary = if is_read_cache_notice(result_text) {
        if normalized_result_lines(result_text).is_empty() {
            ledger_summary
                .map(|summary| ledger_reuse_answer(summary, chinese))
                .unwrap_or_else(|| summarize_read_only_result(result_text, chinese))
        } else {
            summarize_read_only_result(result_text, chinese)
        }
    } else {
        summarize_read_only_result(result_text, chinese)
    };
    let missing_note = missing_requested_search_terms(last_user_preview, result_text)
        .map(|terms| {
            if chinese {
                format!("\n\n未在已检查结果中找到：{}。", format_terms(&terms))
            } else {
                format!(
                    "\n\nNot found in the checked result: {}.",
                    format_terms(&terms)
                )
            }
        })
        .unwrap_or_default();
    let provenance = ledger_summary
        .filter(|summary| !summary.trim().is_empty())
        .map(|summary| {
            if chinese {
                format!("\n\n复用依据：{summary}")
            } else {
                format!("\n\nReuse basis: {summary}")
            }
        })
        .unwrap_or_default();
    if chinese {
        format!(
            "我已经读到需要的信息；模型重复请求 `{tool_name}` 时我已停止继续读取，下面直接根据已有结果回答。\n\n{summary}{missing_note}{provenance}"
        )
    } else {
        format!(
            "I already had the needed information, so I stopped the repeated `{tool_name}` read and answered from the existing tool output.\n\n{summary}{missing_note}{provenance}"
        )
    }
}

fn missing_requested_search_terms(
    last_user_preview: &str,
    result_text: &str,
) -> Option<Vec<String>> {
    let result_lower = result_text.to_ascii_lowercase();
    let missing = requested_search_terms(last_user_preview)
        .into_iter()
        .filter(|term| !result_lower.contains(&term.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    (!missing.is_empty()).then_some(missing)
}

fn requested_search_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for (index, part) in text.split('`').enumerate() {
        if index % 2 == 0 {
            continue;
        }
        let term = part.trim();
        if is_search_target_term(term) && !terms.iter().any(|existing| existing == term) {
            terms.push(term.to_string());
        }
    }
    terms
}

fn is_search_target_term(term: &str) -> bool {
    let len = term.chars().count();
    let lower = term.to_ascii_lowercase();
    (3..=120).contains(&len)
        && !term.contains('/')
        && !term.contains('\\')
        && !term.contains('\n')
        && !term.contains('.')
        && !lower.starts_with("minimum-agent-")
        && !lower.starts_with("project-partner-")
        && !matches!(
            lower.as_str(),
            "audit"
                | "bug_fix"
                | "feature"
                | "read_only_audit"
                | "seeded_code_change"
                | "low"
                | "medium"
                | "high"
                | "closeout:"
        )
}

fn format_terms(terms: &[String]) -> String {
    terms
        .iter()
        .map(|term| format!("`{term}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn compact_preview(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let mut text = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        text.push_str("...");
    }
    text
}

fn ledger_reuse_answer(ledger_summary: &str, chinese: bool) -> String {
    if chinese {
        format!(
            "这次重复读取被已有会话上下文接住了。{ledger_summary}。如果需要逐字内容，下一步应读取具体行范围，而不是重复全文读取。"
        )
    } else {
        format!(
            "The repeated read was handled from session context. {ledger_summary}. If exact text is needed, the next step should read a targeted line range instead of rereading the whole file."
        )
    }
}

fn duplicate_tool_call_ledger_summary(
    session_store: Option<&std::sync::Arc<crate::session_store::SessionStore>>,
    session_id: &str,
    tool_call: &ToolCall,
) -> Option<String> {
    let store = session_store?;
    match tool_call.name.as_str() {
        "file_read" => {
            let path = tool_call.arguments.get("path")?.as_str()?.trim();
            let events = store.recent_context_ledger_events(session_id, 20).ok()?;
            events.into_iter().find_map(|event| {
                let entry = crate::engine::context_ledger::file_read_entry_from_event(&event)?;
                let matches_path = entry.path == path
                    || entry.resolved_path == path
                    || entry.resolved_path.ends_with(path.trim_start_matches("~/"));
                matches_path.then(|| {
                    let preview = entry
                        .content_preview
                        .as_deref()
                        .map(|preview| format!(", evidence \"{}\"", compact_preview(preview, 160)))
                        .unwrap_or_default();
                    format!(
                        "ledger: file `{}` was read previously ({} displayed / {} total lines{})",
                        if entry.path.is_empty() {
                            entry.resolved_path
                        } else {
                            entry.path
                        },
                        entry.displayed_lines,
                        entry.total_lines,
                        preview
                    )
                })
            })
        }
        "bash" => {
            let command = tool_call.arguments.get("command")?.as_str()?.trim();
            let events = store.recent_context_ledger_events(session_id, 20).ok()?;
            events.into_iter().find_map(|event| {
                if event.kind != crate::engine::context_ledger::CONTEXT_LEDGER_BASH_READ_KIND {
                    return None;
                }
                let event_command = event.payload.get("command")?.as_str()?.trim();
                if event_command != command {
                    return None;
                }
                let exit_code = event
                    .payload
                    .get("exit_code")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0);
                Some(format!(
                    "ledger: read-only command `{command}` ran previously with exit {exit_code}"
                ))
            })
        }
        _ => None,
    }
}

fn summarize_read_only_result(result_text: &str, chinese: bool) -> String {
    let lines = normalized_result_lines(result_text);
    let title = lines
        .iter()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .or_else(|| {
            lines
                .iter()
                .find(|line| !line.starts_with('#') && !line.starts_with('['))
                .map(|line| line.as_str())
        })
        .unwrap_or("tool result");
    let description = lines
        .iter()
        .filter(|line| !line.starts_with('#'))
        .find(|line| {
            let line = line.trim();
            !line.is_empty()
                && !line.starts_with("- ")
                && !line.starts_with("* ")
                && !line.starts_with('`')
                && !line.starts_with('[')
                && !looks_like_structured_payload_line(line)
        });
    let bullets: Vec<String> = lines
        .iter()
        .filter_map(|line| markdown_bullet_text(line))
        .take(5)
        .collect();

    if chinese {
        let mut answer = match description {
            Some(description) if description != title => {
                format!("根据已读内容，这是 **{title}**：{description}")
            }
            _ => format!("根据已读内容，这是 **{title}**。"),
        };
        if !bullets.is_empty() {
            answer.push_str("\n\n主要信息：");
            for bullet in bullets {
                answer.push_str("\n- ");
                answer.push_str(&bullet);
            }
        }
        return answer;
    }

    let mut answer = match description {
        Some(description) if description != title => {
            format!("Based on the already-read content, this is **{title}**: {description}")
        }
        _ => format!("Based on the already-read content, this is **{title}**."),
    };
    if !bullets.is_empty() {
        answer.push_str("\n\nKey points:");
        for bullet in bullets {
            answer.push_str("\n- ");
            answer.push_str(&bullet);
        }
    }
    answer
}

fn normalized_result_lines(result_text: &str) -> Vec<String> {
    result_text
        .lines()
        .map(strip_file_read_line_prefix)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            !(line.starts_with("[File unchanged since last read:")
                || line.starts_with("[stored read-only result truncated]")
                || line.starts_with('[') && line.contains("lines total"))
        })
        .map(ToString::to_string)
        .collect()
}

fn strip_file_read_line_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    let mut digit_end = 0;
    let mut saw_digit = false;
    for (idx, ch) in trimmed.char_indices() {
        if ch.is_ascii_digit() {
            digit_end = idx + ch.len_utf8();
            saw_digit = true;
            continue;
        }
        break;
    }
    if saw_digit {
        let after_digits = trimmed[digit_end..].trim_start();
        if let Some(rest) = after_digits.strip_prefix('|') {
            return rest.trim_start();
        }
    }
    trimmed
}

fn markdown_bullet_text(line: &str) -> Option<String> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

fn looks_like_structured_payload_line(line: &str) -> bool {
    let trimmed = line.trim();
    if matches!(trimmed, "{" | "}" | "[" | "]" | ",") {
        return true;
    }
    if trimmed.starts_with('{') || trimmed.starts_with('}') {
        return true;
    }
    if trimmed
        .chars()
        .all(|ch| matches!(ch, '{' | '}' | '[' | ']' | ':' | ',' | '"'))
    {
        return true;
    }
    trimmed.starts_with('"') && trimmed.contains(':')
}

fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
}

fn is_read_cache_notice(text: &str) -> bool {
    text.trim_start()
        .starts_with("[File unchanged since last read:")
}

#[cfg(test)]
mod tests {
    use super::super::tool_batch_result_processor::DuplicateSuccessfulReadOnlyToolResult;
    use super::super::turn_loop_state_controller::TurnLoopStateController;
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::{
        IntentKind, IntentRoute, IntentRouter, ReasoningPolicy, RetrievalPolicy, RiskLevel,
        WorkflowKind,
    };
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use tokio::sync::Mutex;

    struct MockProvider {
        responses: StdMutex<VecDeque<anyhow::Result<ChatResponse>>>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("mock response")
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    fn conversation(response: ChatResponse) -> ConversationLoop {
        ConversationLoop::new(
            Arc::new(MockProvider {
                responses: StdMutex::new(VecDeque::from(vec![Ok(response)])),
            }),
            Arc::new(ToolRegistry::new()),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "mock-model".to_string(),
        )
    }

    #[tokio::test]
    async fn plain_model_response_breaks_iteration() {
        let conversation = conversation(ChatResponse {
            content: "done".to_string(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
        });
        let route = IntentRouter::new().route("hello");
        let resource_policy = ResourcePolicy::from_route(&route);
        let working_dir = std::env::current_dir().expect("current dir");
        let destructive_scope = DestructiveScopeContract::from_user_request("hello", &working_dir);
        let mut task_bundle = TaskContextBundle::new("hello", &working_dir, route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let mut turn_state = TurnRuntimeState::new(true);
        let mut loop_state = TurnLoopStateController::initial_state();
        let mut messages = vec![Message::user("hello")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "hello"));
        let base_tools = Vec::new();
        let available_tools = Vec::new();
        let baseline_git_status_files = HashSet::new();
        let retained_context = crate::tools::ToolContextRetainedContext::default();

        let flow = TurnIterationController::run(TurnIterationContext {
            conversation: &conversation,
            iteration: 0,
            route: &route,
            profile: MainLoopProfile::from_turn(&route, &[]),
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            turn_retrieval_context: None,
            retained_context: &retained_context,
            base_tools: &base_tools,
            available_tools: &available_tools,
            loop_state: &mut loop_state,
            turn_state: &mut turn_state,
            no_diff_audit_closeout_allowed: false,
            code_write_tools_forbidden: false,
            resource_policy: &resource_policy,
            working_dir: &working_dir,
            last_user_preview: "hello",
            required_validation_commands: &[],
            destructive_scope: &destructive_scope,
            baseline_git_status_files: &baseline_git_status_files,
            messages: &mut messages,
            trace: &trace,
            tx: None,
        })
        .await
        .expect("iteration");

        // Double-tap finish: first non-tool response returns Continue,
        // letting the model "think out loud" before acting. The second
        // consecutive Finish will break.
        assert!(matches!(flow, TurnIterationFlow::Continue));
        assert_eq!(loop_state.final_content, "done");
        assert_eq!(loop_state.consecutive_empty_rounds, 1);
        assert_eq!(turn_state.iterations_used, 1);
        assert!(!loop_state.tool_calls_made);
    }

    #[test]
    fn repeated_read_only_closeout_summarizes_existing_readme_result() {
        let round_state = super::super::turn_tool_round_outcome_controller::TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: vec!["file_read".to_string()],
            duplicate_successful_read_only_results: vec![
                DuplicateSuccessfulReadOnlyToolResult {
                    tool_name: "file_read".to_string(),
                    result_text: "   1 | # PhageMatch - 噬菌体匹配平台\n   2 | 一个专注的噬菌体-耐药菌匹配平台。\n   3 | - 菌株上传\n   4 | - 噬菌体匹配"
                        .to_string(),
                    ledger_summary: Some(
                        "ledger: file `README.md` was read previously (4 displayed / 4 total lines, hash abc)"
                            .to_string(),
                    ),
                },
            ],
            should_closeout_after_verified_change: false,
        };

        let message = duplicate_successful_read_only_closeout(
            &round_state,
            "再帮我看一下桌面里面的phageGPT的文件夹是什么项目",
        )
        .expect("closeout");

        assert!(message.contains("停止继续读取"));
        assert!(message.contains("PhageMatch - 噬菌体匹配平台"));
        assert!(message.contains("菌株上传"));
        assert!(message.contains("噬菌体匹配"));
    }

    #[test]
    fn repeated_read_only_closeout_reports_missing_requested_search_token() {
        let round_state = super::super::turn_tool_round_outcome_controller::TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: vec!["file_read".to_string()],
            duplicate_successful_read_only_results: vec![DuplicateSuccessfulReadOnlyToolResult {
                tool_name: "file_read".to_string(),
                result_text: "   1 | known fact".to_string(),
                ledger_summary: Some(
                    "ledger: file `fixtures/mva_low_value_replan/known.txt` was read previously"
                        .to_string(),
                ),
            }],
            should_closeout_after_verified_change: false,
        };

        let message = duplicate_successful_read_only_closeout(
            &round_state,
            "在 `fixtures/mva_low_value_replan` 里找到 `missing-target-token-7391`。",
        )
        .expect("closeout");

        assert!(message.contains("missing-target-token-7391"));
        assert!(message.contains("未在已检查结果中找到"));
        assert!(!message.contains("`audit`"));
        assert!(!message.contains("`minimum-agent-low-value-replan`"));
    }

    #[test]
    fn repeated_read_only_pre_execution_closeout_blocks_duplicate_tool_run() {
        let tool_call = ToolCall {
            id: "call_read_again".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "~/Desktop/phageGPT/README.md"}),
        };
        let fingerprint = tool_call_fingerprint(&tool_call);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(fingerprint.clone(), 1);
        turn_state.successful_read_only_tool_results.insert(
            fingerprint,
            "   1 | # PhageMatch - 噬菌体匹配平台\n   2 | 一个专注的噬菌体-耐药菌匹配平台。"
                .to_string(),
        );

        let message = duplicate_successful_read_only_pre_execution_closeout(
            std::slice::from_ref(&tool_call),
            &turn_state,
            "再帮我看一下桌面里面的phageGPT的文件夹是什么项目",
            None,
            "session",
        )
        .expect("duplicate read should close out before execution");

        assert!(message.contains("停止继续读取"));
        assert!(message.contains("PhageMatch - 噬菌体匹配平台"));
    }

    #[test]
    fn repeated_read_only_pre_execution_closeout_combines_duplicate_results() {
        let memory_read = ToolCall {
            id: "call_memory_again".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "fixtures/project_partner_resume/memory/project.md"}),
        };
        let report_read = ToolCall {
            id: "call_report_again".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "fixtures/project_partner_resume/reports/previous_execution_report.json"}),
        };
        let memory_fingerprint = tool_call_fingerprint(&memory_read);
        let report_fingerprint = tool_call_fingerprint(&report_read);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(memory_fingerprint.clone(), 1);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(report_fingerprint.clone(), 1);
        turn_state.successful_read_only_tool_results.insert(
            memory_fingerprint,
            "1 | # Project Memory\n2 |\n3 | - Decision: first version is a local-only lab notebook helper.\n4 | - Next product goal: add CSV export for recorded strain rows."
                .to_string(),
        );
        turn_state.successful_read_only_tool_results.insert(
            report_fingerprint,
            "1 | {\n2 |   \"status\": \"partial\",\n3 |   \"risks\": [\"CSV export is not implemented yet\"],\n4 |   \"next_steps\": [\"Implement CSV export before adding login or cloud sync\"]\n5 | }"
                .to_string(),
        );

        let message = duplicate_successful_read_only_pre_execution_closeout(
            &[memory_read, report_read],
            &turn_state,
            "project-partner-resume-with-memory",
            None,
            "session",
        )
        .expect("duplicate reads should close out from combined evidence");

        assert!(message.contains("local-only"));
        assert!(message.contains("CSV export"));
        assert!(!message.contains("**Project Memory**: {"));
        assert!(!message.contains("**Project Memory**: ]"));
        assert!(!message.contains("Not found"));
    }

    #[test]
    fn mixed_read_only_batch_keeps_duplicate_successful_reads() {
        let duplicate_read = ToolCall {
            id: "call_read_again".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "fixtures/mva_low_value_replan/known.txt"}),
        };
        let search = ToolCall {
            id: "call_search".to_string(),
            name: "grep".to_string(),
            arguments: serde_json::json!({
                "pattern": "missing-target-token-7391",
                "path": "fixtures/mva_low_value_replan"
            }),
        };
        let fingerprint = tool_call_fingerprint(&duplicate_read);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(fingerprint, 1);

        assert!(drop_duplicate_successful_read_only_tool_calls(
            &[duplicate_read, search],
            &turn_state
        )
        .is_none());
    }

    #[test]
    fn repeated_directory_read_stays_under_model_control() {
        let mut tool_call = ToolCall {
            id: "call_read_dir_again".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "fixtures/project_partner_resume/memory"}),
        };
        let fingerprint = tool_call_fingerprint(&tool_call);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(fingerprint.clone(), 1);
        turn_state.successful_read_only_tool_results.insert(
            fingerprint,
            "Directory: /tmp/worktree/fixtures/project_partner_resume/memory\nEntries (1):\nproject.md"
                .to_string(),
        );

        let redirected = redirect_duplicate_directory_file_reads(
            std::slice::from_mut(&mut tool_call),
            &turn_state,
        );

        assert_eq!(redirected, 0);
        assert_eq!(
            tool_call
                .arguments
                .get("path")
                .and_then(|value| value.as_str()),
            Some("fixtures/project_partner_resume/memory")
        );
    }

    #[test]
    fn repeated_directory_read_does_not_redirect_ambiguous_listing() {
        let mut tool_call = ToolCall {
            id: "call_read_dir_again".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "fixtures/project_partner_resume"}),
        };
        let fingerprint = tool_call_fingerprint(&tool_call);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(fingerprint.clone(), 1);
        turn_state.successful_read_only_tool_results.insert(
            fingerprint,
            "Directory: /tmp/worktree/fixtures/project_partner_resume\nEntries (2):\nmemory/\nreports/"
                .to_string(),
        );

        let redirected = redirect_duplicate_directory_file_reads(
            std::slice::from_mut(&mut tool_call),
            &turn_state,
        );

        assert_eq!(redirected, 0);
        assert_eq!(
            tool_call
                .arguments
                .get("path")
                .and_then(|value| value.as_str()),
            Some("fixtures/project_partner_resume")
        );
    }

    #[test]
    fn repeated_read_only_pre_execution_closeout_allows_new_read() {
        let tool_call = ToolCall {
            id: "call_new_read".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "README.md"}),
        };
        let turn_state = TurnRuntimeState::new(true);

        assert!(duplicate_successful_read_only_pre_execution_closeout(
            std::slice::from_ref(&tool_call),
            &turn_state,
            "read README",
            None,
            "session",
        )
        .is_none());
    }

    #[test]
    fn duplicate_read_only_closeout_is_disabled_for_all_workflows() {
        let direct_route = IntentRoute {
            intent: IntentKind::DirectAnswer,
            confidence: 0.95,
            workflow: WorkflowKind::Direct,
            retrieval: RetrievalPolicy::Light,
            reasoning: ReasoningPolicy::Low,
            risk: RiskLevel::Low,
            recommended_tools: Vec::new(),
            dependency_install_intent: false,
            mcp_auth_intent: false,
            reason: "direct read-only question".to_string(),
        };
        let code_route = IntentRoute {
            intent: IntentKind::CodeChange,
            workflow: WorkflowKind::CodeChange,
            reason: "bug fix".to_string(),
            ..direct_route.clone()
        };

        assert!(!duplicate_read_only_closeout_allowed(&direct_route, &[]));
        assert!(!duplicate_read_only_closeout_allowed(
            &direct_route,
            &["cargo test -q".to_string()]
        ));
        assert!(!duplicate_read_only_closeout_allowed(&code_route, &[]));
    }

    #[test]
    fn duplicate_read_only_is_never_synthesis_blocked() {
        // After Reasonix alignment: read-only tools are always allowed through.
        // The iteration budget handles loops naturally. This test verifies that
        // the fingerprints are still tracked (for closeout/caching) but no
        // synthesis prompt is injected.
        let read = ToolCall {
            id: "call_read".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "~/Desktop/phageGPT/README.md"}),
        };
        let fingerprint = tool_call_fingerprint(&read);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(fingerprint.clone(), 2);

        // Fingerprint tracking still works for closeout / caching.
        assert!(turn_state
            .successful_read_only_tool_fingerprints
            .contains_key(&fingerprint));
        assert_eq!(
            *turn_state
                .successful_read_only_tool_fingerprints
                .get(&fingerprint)
                .unwrap(),
            2
        );
    }

    #[test]
    fn tool_failure_followup_stop_breaks_turn_loop() {
        assert!(matches!(
            flow_after_tool_failure_followup(TurnToolFailureFollowupFlow::Stop),
            Some(TurnIterationFlow::Break)
        ));
        assert!(flow_after_tool_failure_followup(TurnToolFailureFollowupFlow::Continue).is_none());
    }

    #[test]
    fn stop_check_ignores_duplicate_read_only_for_code_change() {
        let route = IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 0.95,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::Medium,
            risk: RiskLevel::Medium,
            recommended_tools: Vec::new(),
            dependency_install_intent: false,
            mcp_auth_intent: false,
            reason: "bug fix".to_string(),
        };
        let mut task_bundle = TaskContextBundle::new("fix slugify", ".", route, None);
        let turn_state = TurnRuntimeState::new(true);
        let round_state = super::super::turn_tool_round_outcome_controller::TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: vec!["file_read".to_string()],
            duplicate_successful_read_only_results: Vec::new(),
            should_closeout_after_verified_change: false,
        };
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix slugify"));

        record_stop_check(
            &trace,
            &mut task_bundle,
            &turn_state,
            &round_state,
            4,
            1,
            false,
        );

        let stop_check = task_bundle
            .agent_state
            .stop_checks
            .last()
            .expect("stop check");
        assert_ne!(
            stop_check.reason,
            crate::engine::task_context::StopCheckReason::DuplicateReadOnly
        );
    }

    #[test]
    fn repeated_read_only_pre_execution_closeout_uses_ledger_for_cache_notice() {
        let tool_call = ToolCall {
            id: "call_read_again".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "README.md"}),
        };
        let fingerprint = tool_call_fingerprint(&tool_call);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state
            .successful_read_only_tool_fingerprints
            .insert(fingerprint.clone(), 1);
        turn_state.successful_read_only_tool_results.insert(
            fingerprint,
            "[File unchanged since last read: README.md] (2 lines)".to_string(),
        );
        let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        store.create_session("session", "Test", "model").unwrap();
        store
            .add_learning_event(
                "session",
                crate::engine::context_ledger::CONTEXT_LEDGER_FILE_READ_KIND,
                "file_read",
                "Read README.md",
                1.0,
                &serde_json::json!({
                    "path": "README.md",
                    "resolved_path": "/tmp/project/README.md",
                    "content_hash": "abc123",
                    "content_preview": "# Project memory says local-only first.",
                    "size_bytes": 10,
                    "total_lines": 2,
                    "displayed_lines": 2,
                    "line_start": 1,
                    "line_end": 2,
                    "targeted_read": false,
                    "truncated": false
                }),
            )
            .unwrap();

        let message = duplicate_successful_read_only_pre_execution_closeout(
            std::slice::from_ref(&tool_call),
            &turn_state,
            "read README",
            Some(&store),
            "session",
        )
        .expect("ledger should recover cache notice");

        assert!(message.contains("local-only first"));
        assert!(message.contains("Reuse basis"));
    }

    #[test]
    fn stop_check_records_no_progress_in_task_state_and_trace() {
        let route = IntentRouter::new().route("fix src/main.rs");
        let mut task_bundle = TaskContextBundle::new("fix src/main.rs", ".", route, None);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.focused_repair.no_code_progress_rounds = 2;
        turn_state.focused_repair.action_checkpoint_active = true;
        let round_state = super::super::turn_tool_round_outcome_controller::TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: Vec::new(),
            duplicate_successful_read_only_results: Vec::new(),
            should_closeout_after_verified_change: false,
        };
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix src/main.rs"));

        record_stop_check(
            &trace,
            &mut task_bundle,
            &turn_state,
            &round_state,
            4,
            1,
            false,
        );

        let stop_check = task_bundle
            .agent_state
            .stop_checks
            .last()
            .expect("stop check");
        assert_eq!(
            stop_check.status,
            crate::engine::task_context::StopCheckStatus::Checkpoint
        );
        assert_eq!(
            stop_check.reason,
            crate::engine::task_context::StopCheckReason::NoProgress
        );
        assert_eq!(
            task_bundle.agent_state.stage,
            crate::engine::task_context::AgentTaskStage::Repair
        );

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::StopCheckEvaluated {
                status,
                reason,
                no_code_progress_rounds: 2,
                action_checkpoint_active: true,
                ..
            } if status == "checkpoint" && reason == "no_progress"
        )));
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::AgentLoopStepEvaluated {
                stage_before,
                stage_after,
                selected_tool_calls: 1,
                ..
            } if stage_before == "Understand" && stage_after == "Repair"
        )));
    }
}
