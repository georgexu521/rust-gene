//! Tool execution controller support.
//!
//! Separates execution gates, runtime context, and batch state from the conversation-loop control flow.

use super::permission_controller::{PermissionController, PermissionRequestRuntime};
use super::tool_call_lifecycle::ToolCallLifecycle;
use super::tool_context_helpers::{tool_allowed_by_context, tool_not_allowed_result};
use super::tool_execution::{
    force_serial_tool_dispatch, read_only_tool_concurrency, tool_call_is_concurrency_safe,
    tool_call_is_read_only, tool_call_is_storm_exempt,
};
use super::tool_metadata::{
    attach_tool_contract_metadata, attach_tool_execution_metadata, merge_tool_result_metadata,
    persist_session_job_if_shell, persist_tool_outcome_learning_event,
    tool_execution_start_progress,
};
use super::tool_result_controller::ToolResultNormalizer;
use super::turn_recording::{
    record_hook_traces, record_mcp_resource_trace, record_permission_denial_recovery_trace,
    record_remote_bridge_trace, record_web_retrieval_trace,
};
use super::ConversationLoop;
#[cfg(test)]
use crate::engine::action_decision::{
    ActionDecision, ActionDecisionInput, ActionScoreModifierSource,
};
use crate::engine::action_review::ActionReview;
#[cfg(test)]
use crate::engine::action_review::ActionReviewInput;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::hooks::HookDecision;
use crate::engine::intent_router::IntentRoute;
#[cfg(test)]
use crate::engine::intent_router::{RiskLevel, WorkflowKind};
use crate::engine::repair::storm::{StormDecision, StormState};
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::{AgentTaskStage, AgentTaskState};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
#[cfg(test)]
use crate::tools::ToolContextRetentionItem;
use crate::tools::{ToolContext, ToolContextRetainedContext, ToolRegistry, ToolResult};
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::debug;

mod action_review;
mod batch;
mod gate;
mod runtime_context;

use action_review::{attach_action_review_metadata, record_tool_observation};
pub(super) use batch::ToolExecutionBatch;
use gate::{ReadOnlyJobInput, ToolExecutionGate, ToolExecutionGateOutcome};
#[cfg(test)]
use runtime_context::apply_memory_action_signal;
use runtime_context::{
    unix_time_millis, ToolRuntimeContext, ToolRuntimeContextInput, ToolRuntimeTiming,
};

fn persist_tool_outcome_learning_event_background(
    store: Option<Arc<crate::session_store::SessionStore>>,
    session_id: String,
    tool_call: ToolCall,
    result: ToolResult,
) {
    let Some(store) = store else {
        return;
    };
    tokio::task::spawn_blocking(move || {
        persist_tool_outcome_learning_event(Some(&store), &session_id, &tool_call, &result);
    });
}

pub(super) struct ToolExecutionRequest<'a> {
    pub(super) tool_calls: &'a [ToolCall],
    pub(super) parent_assistant_content: &'a str,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) pre_executed: HashMap<usize, ToolResult>,
    pub(super) trace: Option<TraceCollector>,
    pub(super) route: Option<&'a IntentRoute>,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) retained_context: &'a ToolContextRetainedContext,
    pub(super) task_stage: AgentTaskStage,
    pub(super) task_state: Option<&'a AgentTaskState>,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) no_progress_rounds: usize,
    pub(super) has_changes_before_tools: bool,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) storm_state: &'a mut StormState,
    pub(super) lifecycle: &'a mut ToolCallLifecycle,
}

struct ReadWriteExecutionContext<'a> {
    tx: Option<&'a mpsc::Sender<StreamEvent>>,
    trace: &'a Option<TraceCollector>,
    runtime_context: &'a ToolRuntimeContext,
    retained_context: &'a ToolContextRetainedContext,
    task_state: Option<&'a AgentTaskState>,
    parent_tool_calls: &'a [ToolCall],
    parent_assistant_content: &'a str,
}

pub(super) struct ToolExecutionContext {
    tool_registry: Arc<ToolRegistry>,
    cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
    session_id: String,
    session_store: Option<Arc<crate::session_store::SessionStore>>,
    hook_manager: Option<Arc<crate::engine::hooks::ToolHookManager>>,
    approval_channel: Option<Arc<super::ToolApprovalChannel>>,
    allowed_tools: Option<HashSet<String>>,
    denial_tracker: Option<Arc<crate::security::DenialTracker>>,
    audit_log: Option<Arc<crate::security::SecurityAuditLog>>,
    active_goal: Option<crate::engine::session_goal::SessionGoal>,
    base_tool_context: ToolContext,
}

impl ToolExecutionContext {
    pub(super) fn from_conversation(conversation: &ConversationLoop) -> Self {
        Self {
            tool_registry: conversation.tool_registry.clone(),
            cost_tracker: conversation.cost_tracker.clone(),
            session_id: conversation.session_id.clone(),
            session_store: conversation.session_store.clone(),
            hook_manager: conversation.hook_manager.clone(),
            approval_channel: conversation.approval_channel.clone(),
            allowed_tools: conversation.allowed_tools.clone(),
            denial_tracker: conversation.denial_tracker.clone(),
            audit_log: conversation.audit_log.clone(),
            active_goal: conversation
                .goal_manager
                .as_ref()
                .and_then(|manager| manager.current()),
            base_tool_context: conversation.create_tool_context(),
        }
    }

    fn tool_context(
        &self,
        trace: &Option<TraceCollector>,
        retained_context: &ToolContextRetainedContext,
    ) -> ToolContext {
        let context = match trace {
            Some(trace) => self
                .base_tool_context
                .clone()
                .with_trace_collector(trace.clone()),
            None => self.base_tool_context.clone(),
        };
        context.with_retained_context(retained_context.clone())
    }
}

pub(super) struct ToolExecutionController {
    context: ToolExecutionContext,
}

impl ToolExecutionController {
    pub(super) fn new(context: ToolExecutionContext) -> Self {
        Self { context }
    }

    fn read_only_job(
        &self,
        input: ReadOnlyJobInput<'_>,
    ) -> impl Future<Output = (ToolCall, ToolResult)> + 'static {
        let execution = &self.context;
        let registry = execution.tool_registry.clone();
        let tc_clone = input.tool_call.clone();
        let tool_name = input.tool_call.name.clone();
        let context = execution
            .tool_context(input.trace, input.retained_context)
            .with_tool_call_metadata(tool_name.clone(), tc_clone.id.clone())
            .with_parent_assistant_tool_calls(
                input.parent_tool_calls,
                input.parent_assistant_content,
            );
        let cost_tracker = execution.cost_tracker.clone();
        let hook_manager = execution.hook_manager.clone();
        let trace = input.trace.clone();
        let runtime_context = input.runtime_context.clone();
        let action_review = input.action_review;
        async move {
            let started_at = std::time::Instant::now();
            let started_at_unix_ms = unix_time_millis();
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::ToolStarted {
                    tool: tool_name.clone(),
                    call_id: tc_clone.id.clone(),
                    parallel: true,
                    pre_executed: false,
                });
            }
            let pre_decision = if let Some(ref hooks) = hook_manager {
                let hook_start = hooks.current_record_sequence();
                let decision = hooks.run_pre_tool(&tc_clone, &context).await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tc_clone.id);
                record_hook_traces(&trace, &hook_records);
                decision
            } else {
                HookDecision {
                    allow: true,
                    reason: None,
                }
            };

            let mut result = if !pre_decision.allow {
                ToolResult::error(
                    pre_decision
                        .reason
                        .unwrap_or_else(|| format!("blocked by pre-tool hook: {}", tool_name)),
                )
            } else if let Some(tool) = registry.get(&tool_name) {
                tool.execute(tc_clone.arguments.clone(), context.clone())
                    .await
            } else {
                ToolResult::error(format!("Tool '{}' not found", tool_name))
            };
            let duration_ms = started_at.elapsed().as_millis() as u64;
            if result.duration_ms.is_none() {
                result.duration_ms = Some(duration_ms);
            }

            if let Some(ref hooks) = hook_manager {
                let hook_start = hooks.current_record_sequence();
                hooks.run_post_tool(&tc_clone, &result, &context).await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tc_clone.id);
                record_hook_traces(&trace, &hook_records);
            };
            attach_tool_execution_metadata(&tc_clone, &mut result);
            if let Some(tool) = registry.get(&tc_clone.name) {
                attach_tool_contract_metadata(tool, &tc_clone, &mut result);
            }
            attach_action_review_metadata(&mut result, &action_review);
            runtime_context.attach(
                &mut result,
                true,
                false,
                Some(ToolRuntimeTiming::finished(started_at_unix_ms)),
            );
            runtime_context.attach_action_decision(&tc_clone, &mut result);
            record_tool_observation(&trace, &tc_clone, &result);
            {
                let mut tracker = cost_tracker.lock().await;
                tracker.record_tool_execution(
                    &tool_name,
                    result.success,
                    duration_ms,
                    result.error.as_deref(),
                );
            }
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::ToolCompleted {
                    tool: tool_name,
                    call_id: tc_clone.id.clone(),
                    success: result.success,
                    duration_ms: result.duration_ms,
                    output_chars: result.content.chars().count(),
                });
                let trace_ref = Some(trace.clone());
                record_mcp_resource_trace(&trace_ref, &tc_clone, &result);
                record_remote_bridge_trace(&trace_ref, &tc_clone, &result);
                record_permission_denial_recovery_trace(&trace_ref, &tc_clone, &result);
                record_web_retrieval_trace(&trace_ref, &tc_clone, &result);
            }
            (tc_clone, result)
        }
    }

    async fn collect_read_only_results<F>(
        &self,
        read_only_jobs: Vec<(usize, F)>,
        concurrency: usize,
        tx: Option<&mpsc::Sender<StreamEvent>>,
        lifecycle: &mut ToolCallLifecycle,
    ) -> Vec<(ToolCall, ToolResult)>
    where
        F: Future<Output = (ToolCall, ToolResult)>,
    {
        let execution = &self.context;
        let mut completed = Vec::new();
        let mut readonly_stream =
            futures::stream::iter(read_only_jobs.into_iter().map(|(order, job)| async move {
                let result = job.await;
                (order, result)
            }))
            .buffer_unordered(concurrency);

        while let Some((order, (tc, result))) = readonly_stream.next().await {
            lifecycle.completed(&tc, &result);
            persist_tool_outcome_learning_event_background(
                execution.session_store.clone(),
                execution.session_id.clone(),
                tc.clone(),
                result.clone(),
            );
            if let Some(tx) = tx {
                let result_content = ToolResultNormalizer::normalize(&tc, &result).ui_content;
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tc.id.clone(),
                        result: result_content,
                        metadata: tool_completion_metadata(&result),
                        result_data: result.data.clone(),
                    })
                    .await;
            }
            completed.push((order, (tc, result)));
        }

        completed.sort_by_key(|(order, _)| *order);
        completed.into_iter().map(|(_, result)| result).collect()
    }

    async fn execute_read_write_calls(
        &self,
        read_write_calls: Vec<(ToolCall, ActionReview)>,
        exec_context: ReadWriteExecutionContext<'_>,
        lifecycle: &mut ToolCallLifecycle,
    ) -> Vec<(ToolCall, ToolResult)> {
        let execution = &self.context;
        let mut results = Vec::new();

        for (tc, action_review) in read_write_calls {
            let tool_id = tc.id.clone();
            let tool_name = tc.name.clone();
            if !tool_allowed_by_context(&execution.allowed_tools, &tool_name) {
                let mut result = tool_not_allowed_result(&tc);
                attach_action_review_metadata(&mut result, &action_review);
                exec_context.runtime_context.attach(
                    &mut result,
                    false,
                    false,
                    Some(ToolRuntimeTiming::instant()),
                );
                exec_context
                    .runtime_context
                    .attach_action_decision(&tc, &mut result);
                record_tool_observation(exec_context.trace, &tc, &result);
                persist_tool_outcome_learning_event_background(
                    execution.session_store.clone(),
                    execution.session_id.clone(),
                    tc.clone(),
                    result.clone(),
                );
                persist_session_job_if_shell(
                    execution.session_store.as_ref(),
                    &execution.session_id,
                    &tc,
                    &result,
                );
                lifecycle.denied(&tc);
                results.push((tc, result));
                continue;
            }
            lifecycle.running(&tc, false, false);

            if let Some(tx) = exec_context.tx {
                let _ = tx
                    .send(StreamEvent::ToolExecutionStart {
                        id: tool_id.clone(),
                        name: tool_name.clone(),
                        metadata: tool_execution_start_metadata(&tool_name, &tc.arguments),
                    })
                    .await;
            }
            if let Some(ref trace) = exec_context.trace {
                trace.record(TraceEvent::ToolStarted {
                    tool: tool_name.clone(),
                    call_id: tool_id.clone(),
                    parallel: false,
                    pre_executed: false,
                });
            }

            let (result, hook_context) = if let Some(tool) = execution.tool_registry.get(&tool_name)
            {
                let mut context = execution
                    .tool_context(exec_context.trace, exec_context.retained_context)
                    .with_tool_call_metadata(tool_name.clone(), tool_id.clone())
                    .with_parent_assistant_tool_calls(
                        exec_context.parent_tool_calls.to_vec(),
                        exec_context.parent_assistant_content.to_string(),
                    );
                let drift_check = crate::engine::goal_drift::GoalDriftDetector::new()
                    .check_with_context(
                        crate::engine::goal_drift::GoalDriftContext {
                            goal: execution.active_goal.as_ref(),
                            task_state: exec_context.task_state,
                        },
                        &tc,
                    );
                let pre_decision = if let Some(ref hooks) = execution.hook_manager {
                    let hook_start = hooks.current_record_sequence();
                    let decision = hooks.run_pre_tool(&tc, &context).await;
                    let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                    record_hook_traces(exec_context.trace, &hook_records);
                    decision
                } else {
                    HookDecision {
                        allow: true,
                        reason: None,
                    }
                };

                let started_at = std::time::Instant::now();
                let started_at_unix_ms = unix_time_millis();
                let permission_evaluation = PermissionController::evaluate_tool_permission(
                    &execution.session_id,
                    &tc,
                    tool,
                    &context,
                    &drift_check,
                );
                let mut result = if !pre_decision.allow {
                    ToolResult::error(
                        pre_decision
                            .reason
                            .unwrap_or_else(|| format!("blocked by pre-tool hook: {}", tool_name)),
                    )
                } else if permission_evaluation.denied {
                    let permission_source =
                        permission_evaluation.record.as_ref().and_then(|record| {
                            record
                                .metadata
                                .get("permission_source")
                                .and_then(serde_json::Value::as_str)
                        });
                    PermissionController::denied_result(
                        &tool_name,
                        permission_evaluation.record.as_ref(),
                        permission_source,
                    )
                } else if permission_evaluation.requires_approval {
                    let permission_outcome = PermissionController::request_user_permission(
                        &tc,
                        &permission_evaluation,
                        PermissionRequestRuntime {
                            approval_channel: execution.approval_channel.as_ref(),
                            tx: exec_context.tx,
                            trace: exec_context.trace,
                            hook_manager: execution.hook_manager.as_ref(),
                            context: &context,
                            action_review: Some(&action_review),
                        },
                    )
                    .await;
                    if permission_outcome.approved {
                        PermissionController::record_approved_session_rule(
                            &mut context,
                            &tool_name,
                        );
                        if let Some(tx) = exec_context.tx {
                            let _ = tx
                                .send(StreamEvent::ToolExecutionProgress {
                                    id: tool_id.clone(),
                                    progress: tool_execution_start_progress(
                                        &tool_name,
                                        &tc.arguments,
                                    ),
                                })
                                .await;
                        }
                        let mut result = tool.execute(tc.arguments.clone(), context.clone()).await;
                        if let Some(record) = permission_evaluation.record.as_ref() {
                            merge_tool_result_metadata(
                                &mut result,
                                "permission_request",
                                record.to_json_with_approval_source(
                                    true,
                                    Some(&permission_outcome.source),
                                ),
                            );
                        }
                        result
                    } else {
                        PermissionController::denied_result(
                            &tool_name,
                            permission_evaluation.record.as_ref(),
                            Some(&permission_outcome.source),
                        )
                    }
                } else {
                    if let Some(tx) = exec_context.tx {
                        let _ = tx
                            .send(StreamEvent::ToolExecutionProgress {
                                id: tool_id.clone(),
                                progress: tool_execution_start_progress(&tool_name, &tc.arguments),
                            })
                            .await;
                    }
                    tool.execute(tc.arguments.clone(), context.clone()).await
                };
                let duration_ms = started_at.elapsed().as_millis() as u64;
                if result.duration_ms.is_none() {
                    result.duration_ms = Some(duration_ms);
                }
                attach_tool_execution_metadata(&tc, &mut result);
                attach_tool_contract_metadata(tool, &tc, &mut result);
                attach_action_review_metadata(&mut result, &action_review);
                exec_context.runtime_context.attach(
                    &mut result,
                    false,
                    false,
                    Some(ToolRuntimeTiming::finished(started_at_unix_ms)),
                );
                exec_context
                    .runtime_context
                    .attach_action_decision(&tc, &mut result);
                record_tool_observation(exec_context.trace, &tc, &result);

                // ── Security Audit & Denial Tracking ──────────────────────
                let params_summary = if let Some(tool) = execution.tool_registry.get(&tool_name) {
                    tool.to_classifier_input(&tc.arguments)
                } else {
                    tool_name.clone()
                };

                if let Some(ref log) = execution.audit_log {
                    let decision = if result.success {
                        "EXECUTED"
                    } else if PermissionController::is_permission_denied(&result) {
                        "DENIED"
                    } else {
                        "FAILED"
                    };
                    log.log_execution(&tool_name, &params_summary, result.success, decision)
                        .await;
                }

                if let Some(ref tracker) = execution.denial_tracker {
                    if result.success {
                        tracker.record_success().await;
                    } else if PermissionController::is_permission_denied(&result)
                        || result
                            .error
                            .as_deref()
                            .unwrap_or("")
                            .contains("Dangerous command")
                    {
                        tracker
                            .record_denial(
                                &tool_name,
                                &params_summary,
                                result.error.as_deref().unwrap_or("security block"),
                            )
                            .await;
                    }
                }
                // ─────────────────────────────────────────────────────────

                {
                    let mut tracker = execution.cost_tracker.lock().await;
                    tracker.record_tool_execution(
                        &tool_name,
                        result.success,
                        duration_ms,
                        result.error.as_deref(),
                    );
                }

                (result, Some(context))
            } else {
                let mut result = ToolResult::error(format!("Tool '{}' not found", tool_name));
                attach_tool_execution_metadata(&tc, &mut result);
                if let Some(tool) = execution.tool_registry.get(&tc.name) {
                    attach_tool_contract_metadata(tool, &tc, &mut result);
                }
                attach_action_review_metadata(&mut result, &action_review);
                exec_context.runtime_context.attach(
                    &mut result,
                    false,
                    false,
                    Some(ToolRuntimeTiming::instant()),
                );
                exec_context
                    .runtime_context
                    .attach_action_decision(&tc, &mut result);
                record_tool_observation(exec_context.trace, &tc, &result);
                (result, None)
            };

            if let (Some(hooks), Some(context)) = (&execution.hook_manager, &hook_context) {
                let hook_start = hooks.current_record_sequence();
                hooks.run_post_tool(&tc, &result, context).await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                record_hook_traces(exec_context.trace, &hook_records);
            }

            if let Some(tx) = exec_context.tx {
                let result_content = ToolResultNormalizer::normalize(&tc, &result).ui_content;
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tool_id.clone(),
                        result: result_content,
                        metadata: tool_completion_metadata(&result),
                        result_data: result.data.clone(),
                    })
                    .await;
            }
            if let Some(ref trace) = exec_context.trace {
                trace.record(TraceEvent::ToolCompleted {
                    tool: tool_name.clone(),
                    call_id: tool_id.clone(),
                    success: result.success,
                    duration_ms: result.duration_ms,
                    output_chars: result.content.chars().count(),
                });
                let trace_ref = Some(trace.clone());
                record_mcp_resource_trace(&trace_ref, &tc, &result);
                record_remote_bridge_trace(&trace_ref, &tc, &result);
                record_permission_denial_recovery_trace(&trace_ref, &tc, &result);
                record_web_retrieval_trace(&trace_ref, &tc, &result);
            }
            if PermissionController::is_permission_denied(&result) {
                lifecycle.denied(&tc);
            } else {
                lifecycle.completed(&tc, &result);
            }
            persist_tool_outcome_learning_event_background(
                execution.session_store.clone(),
                execution.session_id.clone(),
                tc.clone(),
                result.clone(),
            );
            results.push((tc, result));
        }

        results
    }

    pub(super) async fn execute_tools_parallel(
        &self,
        request: ToolExecutionRequest<'_>,
    ) -> ToolExecutionBatch {
        let ToolExecutionRequest {
            tool_calls,
            parent_assistant_content,
            tx,
            pre_executed,
            trace,
            route,
            resource_policy,
            exposed_tool_names,
            retained_context,
            task_stage,
            task_state,
            action_checkpoint_active,
            action_checkpoint_lookup_count,
            no_progress_rounds,
            has_changes_before_tools,
            destructive_scope,
            storm_state,
            lifecycle,
        } = request;
        let execution = &self.context;
        let mut parallel_jobs: Vec<(usize, _)> = Vec::new();
        let mut results: Vec<(ToolCall, ToolResult)> = Vec::new();
        let mut scheduled_count = 0usize;
        lifecycle.pending_batch(tool_calls);
        let runtime_context = ToolRuntimeContext::new(ToolRuntimeContextInput {
            route,
            policy: resource_policy,
            task_stage,
            action_checkpoint_active,
            no_progress_rounds,
            has_changes_before_tools,
            exposed_tools_count: exposed_tool_names.len(),
            retained_context,
            task_state,
        });
        let gate = ToolExecutionGate {
            tool_registry: execution.tool_registry.as_ref(),
            active_goal: execution.active_goal.as_ref(),
            task_state,
            allowed_tools: &execution.allowed_tools,
            resource_policy,
            exposed_tool_names,
            action_checkpoint_active,
            action_checkpoint_lookup_count,
            has_changes_before_tools,
            destructive_scope,
            working_dir: execution.base_tool_context.working_dir.as_path(),
            trace: &trace,
            runtime_context: &runtime_context,
            permission_context: &execution.base_tool_context.permission_context,
        };
        let concurrency = if force_serial_tool_dispatch() {
            1
        } else {
            read_only_tool_concurrency().min(resource_policy.parallelism_limit.max(1))
        };
        let mut serial_boundary_seen = false;

        // ── Phase 1: scan and categorize ──
        for (i, tc) in tool_calls.iter().enumerate() {
            if tc.name.is_empty() {
                continue;
            }

            // Storm breaker: check for repeated calls before dispatching.
            let is_read_only =
                tool_call_is_read_only(execution.tool_registry.as_ref(), &tc.name, &tc.arguments);
            let storm_exempt =
                tool_call_is_storm_exempt(execution.tool_registry.as_ref(), &tc.name);
            match storm_state.check(&tc.name, &tc.arguments, is_read_only, storm_exempt) {
                StormDecision::Suppress(reason) => {
                    if !parallel_jobs.is_empty() {
                        let pending = std::mem::take(&mut parallel_jobs);
                        let parallel_results = self
                            .collect_read_only_results(pending, concurrency, tx, lifecycle)
                            .await;
                        results.extend(parallel_results);
                    }
                    let storm_result = crate::tools::ToolResult::error(reason);
                    emit_denied_tool_events(tx, tc, &storm_result).await;
                    results.push((tc.clone(), storm_result));
                    scheduled_count += 1;
                    continue;
                }
                StormDecision::Allow => {}
            }

            let action_review = match gate.evaluate(tc, scheduled_count) {
                ToolExecutionGateOutcome::Allow(review) => *review,
                ToolExecutionGateOutcome::Deny(result) => {
                    if !parallel_jobs.is_empty() {
                        let pending = std::mem::take(&mut parallel_jobs);
                        let parallel_results = self
                            .collect_read_only_results(pending, concurrency, tx, lifecycle)
                            .await;
                        results.extend(parallel_results);
                    }
                    persist_tool_outcome_learning_event_background(
                        execution.session_store.clone(),
                        execution.session_id.clone(),
                        tc.clone(),
                        result.clone(),
                    );
                    lifecycle.denied(tc);
                    emit_denied_tool_events(tx, tc, &result).await;
                    results.push((tc.clone(), result));
                    scheduled_count += 1;
                    continue;
                }
            };

            // Pre-executed results from streaming phase: use directly.
            if !serial_boundary_seen {
                if let Some(pre_result) = pre_executed.get(&i) {
                    if !parallel_jobs.is_empty() {
                        let pending = std::mem::take(&mut parallel_jobs);
                        let parallel_results = self
                            .collect_read_only_results(pending, concurrency, tx, lifecycle)
                            .await;
                        results.extend(parallel_results);
                    }
                    let mut pre_result = pre_result.clone();
                    attach_tool_execution_metadata(tc, &mut pre_result);
                    if let Some(tool) = execution.tool_registry.get(&tc.name) {
                        attach_tool_contract_metadata(tool, tc, &mut pre_result);
                    }
                    attach_action_review_metadata(&mut pre_result, &action_review);
                    runtime_context.attach(&mut pre_result, true, true, None);
                    runtime_context.attach_action_decision(tc, &mut pre_result);
                    record_tool_observation(&trace, tc, &pre_result);
                    persist_tool_outcome_learning_event_background(
                        execution.session_store.clone(),
                        execution.session_id.clone(),
                        tc.clone(),
                        pre_result.clone(),
                    );
                    if let Some(ref trace) = trace {
                        trace.record(TraceEvent::ToolStarted {
                            tool: tc.name.clone(),
                            call_id: tc.id.clone(),
                            parallel: true,
                            pre_executed: true,
                        });
                        trace.record(TraceEvent::ToolCompleted {
                            tool: tc.name.clone(),
                            call_id: tc.id.clone(),
                            success: pre_result.success,
                            duration_ms: pre_result.duration_ms,
                            output_chars: pre_result.content.chars().count(),
                        });
                        let trace_ref = Some(trace.clone());
                        record_mcp_resource_trace(&trace_ref, tc, &pre_result);
                        record_remote_bridge_trace(&trace_ref, tc, &pre_result);
                        record_permission_denial_recovery_trace(&trace_ref, tc, &pre_result);
                        record_web_retrieval_trace(&trace_ref, tc, &pre_result);
                    }
                    lifecycle.provider_executed(tc, &pre_result);
                    debug!(
                        "Skipping pre-executed read-only tool at index {}: {}",
                        i, tc.name
                    );
                    results.push((tc.clone(), pre_result.clone()));
                    if let Some(tx) = tx {
                        let result_content =
                            ToolResultNormalizer::normalize(tc, &pre_result).ui_content;
                        let _ = tx
                            .send(StreamEvent::ToolExecutionComplete {
                                id: tc.id.clone(),
                                result: result_content,
                                metadata: tool_completion_metadata(&pre_result),
                                result_data: pre_result.data.clone(),
                            })
                            .await;
                    }
                    scheduled_count += 1;
                    continue;
                }
            }

            let concurrency_safe = tool_call_is_concurrency_safe(
                execution.tool_registry.as_ref(),
                &tc.name,
                &tc.arguments,
            );

            if concurrency_safe {
                lifecycle.running(tc, true, false);
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::ToolExecutionStart {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                            metadata: tool_execution_start_metadata(&tc.name, &tc.arguments),
                        })
                        .await;
                }
                parallel_jobs.push((
                    i,
                    self.read_only_job(ReadOnlyJobInput {
                        trace: &trace,
                        runtime_context: &runtime_context,
                        retained_context,
                        tool_call: tc,
                        action_review,
                        parent_tool_calls: tool_calls.to_vec(),
                        parent_assistant_content: parent_assistant_content.to_string(),
                    }),
                ));
                scheduled_count += 1;
            } else {
                if !parallel_jobs.is_empty() {
                    let pending = std::mem::take(&mut parallel_jobs);
                    let parallel_results = self
                        .collect_read_only_results(pending, concurrency, tx, lifecycle)
                        .await;
                    results.extend(parallel_results);
                }
                let read_write_results = self
                    .execute_read_write_calls(
                        vec![(tc.clone(), action_review)],
                        ReadWriteExecutionContext {
                            tx,
                            trace: &trace,
                            runtime_context: &runtime_context,
                            retained_context,
                            task_state,
                            parent_tool_calls: tool_calls,
                            parent_assistant_content,
                        },
                        lifecycle,
                    )
                    .await;
                results.extend(read_write_results);
                serial_boundary_seen = true;
                scheduled_count += 1;
            }
        }

        // ── Phase 2: execute trailing read-only jobs concurrently ──
        if !parallel_jobs.is_empty() {
            let parallel_results = self
                .collect_read_only_results(parallel_jobs, concurrency, tx, lifecycle)
                .await;
            results.extend(parallel_results);
        }

        let lifecycle_snapshot = lifecycle.snapshot_for(tool_calls);
        let lifecycle_summary = lifecycle_snapshot
            .iter()
            .map(|(id, record)| {
                format!(
                    "{}:{}:{:?}:parallel={}:pre_executed={}",
                    id, record.tool_name, record.status, record.parallel, record.pre_executed
                )
            })
            .collect::<Vec<_>>();
        let batch = ToolExecutionBatch::new(results, lifecycle_snapshot);
        if !lifecycle_summary.is_empty() {
            debug!(
                "Tool lifecycle batch: {} (denied={}, failed={}, pre_executed={})",
                lifecycle_summary.join(", "),
                batch.denied_count(),
                batch.failed_count(),
                batch.pre_executed_count()
            );
        }

        batch
    }
}

fn tool_completion_metadata(result: &ToolResult) -> Option<serde_json::Value> {
    let data = result.data.as_ref()?;
    let mut metadata = data
        .get("tool_summary")
        .cloned()
        .or_else(|| data.get("tool_observation").cloned())?;
    if let (Some(object), Some(observation)) =
        (metadata.as_object_mut(), data.get("tool_observation"))
    {
        object.insert("tool_observation".to_string(), observation.clone());
    }
    Some(metadata)
}

async fn emit_denied_tool_events(
    tx: Option<&mpsc::Sender<StreamEvent>>,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(tx) = tx else {
        return;
    };
    let _ = tx
        .send(StreamEvent::ToolExecutionStart {
            id: tool_call.id.clone(),
            name: tool_call.name.clone(),
            metadata: tool_execution_start_metadata(&tool_call.name, &tool_call.arguments),
        })
        .await;
    let result_content = ToolResultNormalizer::normalize(tool_call, result).ui_content;
    let _ = tx
        .send(StreamEvent::ToolExecutionComplete {
            id: tool_call.id.clone(),
            result: result_content,
            metadata: tool_completion_metadata(result),
            result_data: result.data.clone(),
        })
        .await;
}

fn tool_execution_start_metadata(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Option<serde_json::Value> {
    let mut object = serde_json::Map::new();
    object.insert(
        "tool".to_string(),
        serde_json::Value::String(tool_name.to_string()),
    );
    for key in ["path", "command", "pattern", "query"] {
        if let Some(value) = arguments.get(key).and_then(serde_json::Value::as_str) {
            object.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
    (object.len() > 1).then_some(serde_json::Value::Object(object))
}

#[cfg(test)]
mod tests;
