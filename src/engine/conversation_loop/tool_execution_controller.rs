use super::tool_call_lifecycle::{ToolCallLifecycle, ToolCallLifecycleRecord, ToolCallStatus};
use super::tool_execution::{is_read_only, read_only_tool_concurrency};
use super::tool_metadata::{
    attach_tool_execution_metadata, persist_tool_outcome_learning_event,
    provider_tool_result_content, tool_execution_start_progress,
};
use super::turn_recording::{
    record_goal_drift_if_needed, record_hook_traces, record_mcp_resource_trace,
    record_web_retrieval_trace,
};
use super::{tool_allowed_by_context, tool_not_allowed_result, ConversationLoop};
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::hooks::HookDecision;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tracing::{debug, warn};

#[derive(Debug, Clone, Default)]
pub(super) struct ToolExecutionBatch {
    results: Vec<(ToolCall, ToolResult)>,
    lifecycle: Vec<(String, ToolCallLifecycleRecord)>,
}

impl ToolExecutionBatch {
    fn new(
        results: Vec<(ToolCall, ToolResult)>,
        lifecycle: Vec<(String, ToolCallLifecycleRecord)>,
    ) -> Self {
        Self { results, lifecycle }
    }

    #[cfg(test)]
    pub(super) fn results(&self) -> &[(ToolCall, ToolResult)] {
        &self.results
    }

    pub(super) fn results_mut(&mut self) -> &mut [(ToolCall, ToolResult)] {
        &mut self.results
    }

    pub(super) fn any_success(&self) -> bool {
        self.results.iter().any(|(_, result)| result.success)
    }

    pub(super) fn unsuccessful_count(&self) -> usize {
        self.results
            .iter()
            .filter(|(_, result)| !result.success)
            .count()
    }

    pub(super) fn result_successes(&self) -> impl Iterator<Item = (&ToolCall, bool)> {
        self.results
            .iter()
            .map(|(tool_call, result)| (tool_call, result.success))
    }

    pub(super) fn denied_count(&self) -> usize {
        self.lifecycle
            .iter()
            .filter(|(_, record)| record.status == ToolCallStatus::Denied)
            .count()
    }

    pub(super) fn failed_count(&self) -> usize {
        self.lifecycle
            .iter()
            .filter(|(_, record)| record.status == ToolCallStatus::Failed)
            .count()
    }

    pub(super) fn pre_executed_count(&self) -> usize {
        self.lifecycle
            .iter()
            .filter(|(_, record)| record.pre_executed)
            .count()
    }
}

pub(super) struct ToolExecutionRequest<'a> {
    pub(super) tool_calls: &'a [ToolCall],
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) pre_executed: HashMap<usize, ToolResult>,
    pub(super) trace: Option<TraceCollector>,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) has_changes_before_tools: bool,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) lifecycle: &'a mut ToolCallLifecycle,
}

pub(super) struct ToolExecutionController<'a> {
    conversation: &'a ConversationLoop,
}

impl<'a> ToolExecutionController<'a> {
    pub(super) fn new(conversation: &'a ConversationLoop) -> Self {
        Self { conversation }
    }

    pub(super) async fn execute_tools_parallel(
        &self,
        request: ToolExecutionRequest<'_>,
    ) -> ToolExecutionBatch {
        let ToolExecutionRequest {
            tool_calls,
            tx,
            pre_executed,
            trace,
            resource_policy,
            exposed_tool_names,
            action_checkpoint_active,
            action_checkpoint_lookup_count,
            has_changes_before_tools,
            destructive_scope,
            lifecycle,
        } = request;
        let conversation = self.conversation;
        let mut read_only_jobs = Vec::new();
        let mut read_write_calls = Vec::new();
        let mut denied_results = Vec::new();
        let mut results: Vec<(ToolCall, ToolResult)> = Vec::new();
        let active_goal = conversation
            .goal_manager
            .as_ref()
            .and_then(|manager| manager.current());
        lifecycle.pending_batch(tool_calls);

        for (i, tc) in tool_calls.iter().enumerate() {
            if tc.name.is_empty() {
                continue;
            }
            if !exposed_tool_names.contains(&tc.name) {
                let error = if action_checkpoint_active {
                    ConversationLoop::action_checkpoint_unexposed_tool_message(
                        &tc.name,
                        exposed_tool_names,
                        action_checkpoint_lookup_count,
                    )
                } else {
                    format!(
                        "Tool '{}' was not exposed in the current request and cannot be executed.",
                        tc.name
                    )
                };
                let mut result = ToolResult::error(error);
                attach_tool_execution_metadata(tc, &mut result);
                if let Some(ref trace) = trace {
                    trace.record(TraceEvent::ToolStarted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        parallel: false,
                        pre_executed: false,
                    });
                    trace.record(TraceEvent::ToolCompleted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        success: false,
                        duration_ms: Some(0),
                        output_chars: result.content.chars().count(),
                    });
                }
                persist_tool_outcome_learning_event(
                    conversation.session_store.as_ref(),
                    &conversation.session_id,
                    tc,
                    &result,
                );
                lifecycle.denied(tc);
                denied_results.push((tc.clone(), result));
                continue;
            }
            if results.len() + denied_results.len() + read_only_jobs.len() + read_write_calls.len()
                >= resource_policy.max_tool_calls
            {
                let mut result = ToolResult::error(format!(
                    "Resource policy blocked tool '{}': max tool calls ({}) reached.",
                    tc.name, resource_policy.max_tool_calls
                ));
                attach_tool_execution_metadata(tc, &mut result);
                if let Some(ref trace) = trace {
                    trace.record(TraceEvent::ToolStarted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        parallel: false,
                        pre_executed: false,
                    });
                    trace.record(TraceEvent::ToolCompleted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        success: false,
                        duration_ms: Some(0),
                        output_chars: result.content.chars().count(),
                    });
                }
                persist_tool_outcome_learning_event(
                    conversation.session_store.as_ref(),
                    &conversation.session_id,
                    tc,
                    &result,
                );
                lifecycle.denied(tc);
                denied_results.push((tc.clone(), result));
                continue;
            }
            record_goal_drift_if_needed(&trace, active_goal.as_ref(), tc);
            if !tool_allowed_by_context(&conversation.allowed_tools, &tc.name) {
                let result = tool_not_allowed_result(tc);
                persist_tool_outcome_learning_event(
                    conversation.session_store.as_ref(),
                    &conversation.session_id,
                    tc,
                    &result,
                );
                lifecycle.denied(tc);
                denied_results.push((tc.clone(), result));
                continue;
            }

            let working_dir =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let destructive_check = destructive_scope.check_tool_call(tc, &working_dir);
            if destructive_check.applies {
                if let Some(ref trace) = trace {
                    trace.record(TraceEvent::DestructiveScopeChecked {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        operation: destructive_check.operation.clone(),
                        target: destructive_check.target.clone(),
                        allowed: destructive_check.allowed,
                        reason: destructive_check.reason.clone(),
                    });
                }
                if !destructive_check.allowed {
                    let mut result = ToolResult::error(format!(
                        "Destructive scope blocked: {}",
                        destructive_check.reason
                    ));
                    attach_tool_execution_metadata(tc, &mut result);
                    if let Some(ref trace) = trace {
                        trace.record(TraceEvent::ToolStarted {
                            tool: tc.name.clone(),
                            call_id: tc.id.clone(),
                            parallel: false,
                            pre_executed: false,
                        });
                        trace.record(TraceEvent::ToolCompleted {
                            tool: tc.name.clone(),
                            call_id: tc.id.clone(),
                            success: false,
                            duration_ms: Some(0),
                            output_chars: result.content.chars().count(),
                        });
                    }
                    persist_tool_outcome_learning_event(
                        conversation.session_store.as_ref(),
                        &conversation.session_id,
                        tc,
                        &result,
                    );
                    lifecycle.denied(tc);
                    denied_results.push((tc.clone(), result));
                    continue;
                }
            }

            if action_checkpoint_active
                && tc.name == "bash"
                && !ConversationLoop::bash_allowed_at_action_checkpoint(
                    &tc.arguments,
                    has_changes_before_tools,
                )
            {
                let mut result = ToolResult::error(
                    "Bash is restricted during the action checkpoint: use it only to apply a patch (for example python/perl/sed -i/apply_patch/redirect/tee) or, after files have changed, to run validation. Do not use bash for read-only inspection at this checkpoint."
                        .to_string(),
                );
                attach_tool_execution_metadata(tc, &mut result);
                if let Some(ref trace) = trace {
                    trace.record(TraceEvent::ToolStarted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        parallel: false,
                        pre_executed: false,
                    });
                    trace.record(TraceEvent::ToolCompleted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        success: false,
                        duration_ms: Some(0),
                        output_chars: result.content.chars().count(),
                    });
                }
                persist_tool_outcome_learning_event(
                    conversation.session_store.as_ref(),
                    &conversation.session_id,
                    tc,
                    &result,
                );
                lifecycle.denied(tc);
                denied_results.push((tc.clone(), result));
                continue;
            }
            if action_checkpoint_active && tc.name == "file_edit" {
                if let Some(reason) = ConversationLoop::action_checkpoint_file_edit_rejection(
                    &tc.arguments,
                    &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
                ) {
                    let mut result = ToolResult::error(format!(
                        "Action checkpoint file_edit rejected: {reason}"
                    ));
                    attach_tool_execution_metadata(tc, &mut result);
                    if let Some(ref trace) = trace {
                        trace.record(TraceEvent::ToolStarted {
                            tool: tc.name.clone(),
                            call_id: tc.id.clone(),
                            parallel: false,
                            pre_executed: false,
                        });
                        trace.record(TraceEvent::ToolCompleted {
                            tool: tc.name.clone(),
                            call_id: tc.id.clone(),
                            success: false,
                            duration_ms: Some(0),
                            output_chars: result.content.chars().count(),
                        });
                    }
                    persist_tool_outcome_learning_event(
                        conversation.session_store.as_ref(),
                        &conversation.session_id,
                        tc,
                        &result,
                    );
                    lifecycle.denied(tc);
                    denied_results.push((tc.clone(), result));
                    continue;
                }
            }

            if let Some(pre_result) = pre_executed.get(&i) {
                let mut pre_result = pre_result.clone();
                attach_tool_execution_metadata(tc, &mut pre_result);
                persist_tool_outcome_learning_event(
                    conversation.session_store.as_ref(),
                    &conversation.session_id,
                    tc,
                    &pre_result,
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
                    record_web_retrieval_trace(&trace_ref, tc, &pre_result);
                }
                lifecycle.provider_executed(tc, &pre_result);
                debug!(
                    "Skipping pre-executed read-only tool at index {}: {}",
                    i, tc.name
                );
                results.push((tc.clone(), pre_result.clone()));
                if let Some(tx) = tx {
                    let result_content = provider_tool_result_content(tc, &pre_result);
                    let _ = tx
                        .send(StreamEvent::ToolExecutionComplete {
                            id: tc.id.clone(),
                            result: result_content,
                        })
                        .await;
                }
                continue;
            }

            if is_read_only(&tc.name) {
                lifecycle.running(tc, true, false);
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::ToolExecutionStart {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                        })
                        .await;
                }
                let registry = conversation.tool_registry.clone();
                let context = conversation.create_tool_context_with_optional_trace(&trace);
                let tc_clone = tc.clone();
                let tool_name = tc.name.clone();
                let cost_tracker = conversation.cost_tracker.clone();
                let hook_manager = conversation.hook_manager.clone();
                let trace = trace.clone();
                read_only_jobs.push(async move {
                    let started_at = std::time::Instant::now();
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

                    let mut result =
                        if !pre_decision.allow {
                            ToolResult::error(pre_decision.reason.unwrap_or_else(|| {
                                format!("blocked by pre-tool hook: {}", tool_name)
                            }))
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
                        record_web_retrieval_trace(&trace_ref, &tc_clone, &result);
                    }
                    (tc_clone, result)
                });
            } else {
                read_write_calls.push(tc.clone());
            }
        }

        results.append(&mut denied_results);

        let concurrency =
            read_only_tool_concurrency().min(resource_policy.parallelism_limit.max(1));
        let mut readonly_stream =
            futures::stream::iter(read_only_jobs).buffer_unordered(concurrency);

        while let Some((tc, result)) = readonly_stream.next().await {
            lifecycle.completed(&tc, &result);
            persist_tool_outcome_learning_event(
                conversation.session_store.as_ref(),
                &conversation.session_id,
                &tc,
                &result,
            );
            if let Some(tx) = tx {
                let result_content = provider_tool_result_content(&tc, &result);
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tc.id.clone(),
                        result: result_content,
                    })
                    .await;
            }
            results.push((tc, result));
        }

        for tc in read_write_calls {
            let tool_id = tc.id.clone();
            let tool_name = tc.name.clone();
            if !tool_allowed_by_context(&conversation.allowed_tools, &tool_name) {
                let result = tool_not_allowed_result(&tc);
                persist_tool_outcome_learning_event(
                    conversation.session_store.as_ref(),
                    &conversation.session_id,
                    &tc,
                    &result,
                );
                lifecycle.denied(&tc);
                results.push((tc, result));
                continue;
            }
            lifecycle.running(&tc, false, false);

            if let Some(tx) = tx {
                let _ = tx
                    .send(StreamEvent::ToolExecutionStart {
                        id: tool_id.clone(),
                        name: tool_name.clone(),
                    })
                    .await;
            }
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::ToolStarted {
                    tool: tool_name.clone(),
                    call_id: tool_id.clone(),
                    parallel: false,
                    pre_executed: false,
                });
            }

            let (result, hook_context) = if let Some(tool) =
                conversation.tool_registry.get(&tool_name)
            {
                let mut context = conversation.create_tool_context_with_optional_trace(&trace);
                let drift_check = active_goal
                    .as_ref()
                    .map(|goal| {
                        crate::engine::goal_drift::GoalDriftDetector::new().check(goal, &tc)
                    })
                    .unwrap_or_else(crate::engine::goal_drift::DriftCheck::ok);
                let drift_requires_approval = drift_check.requires_approval();
                let pre_decision = if let Some(ref hooks) = conversation.hook_manager {
                    let hook_start = hooks.current_record_sequence();
                    let decision = hooks.run_pre_tool(&tc, &context).await;
                    let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                    record_hook_traces(&trace, &hook_records);
                    decision
                } else {
                    HookDecision {
                        allow: true,
                        reason: None,
                    }
                };

                let started_at = std::time::Instant::now();
                let requires_approval = {
                    let permission_requires = context
                        .permission_context
                        .requires_confirmation(&tool_name, &tc.arguments);
                    let tool_requires = tool.requires_confirmation(&tc.arguments)
                        && !context
                            .permission_context
                            .auto_approves_tool_confirmation(&tool_name, &tc.arguments);
                    permission_requires || tool_requires || drift_requires_approval
                };
                let mut result = if !pre_decision.allow {
                    ToolResult::error(
                        pre_decision
                            .reason
                            .unwrap_or_else(|| format!("blocked by pre-tool hook: {}", tool_name)),
                    )
                } else if requires_approval {
                    let mut approved = false;
                    if let (Some(ref channel), Some(tx)) = (&conversation.approval_channel, tx) {
                        let base_prompt = if drift_requires_approval {
                            format!(
                                "Tool '{}' may drift from the current goal. Reason: {} Suggested action: {} Allow?",
                                tool_name,
                                drift_check.reason,
                                drift_check
                                    .suggested_action
                                    .as_deref()
                                    .unwrap_or("review before executing")
                            )
                        } else if tool_name == "mcp_tool" {
                            let server = tc.arguments["server_name"].as_str().unwrap_or("");
                            let t = tc.arguments["tool_name"].as_str().unwrap_or("");
                            format!(
                                "MCP tool '{}' on server '{}' requires approval. Allow?",
                                t, server
                            )
                        } else if let Some(prompt) = tool.confirmation_prompt(&tc.arguments) {
                            prompt
                        } else {
                            format!("Tool '{}' requires approval. Allow?", tool_name)
                        };
                        let prompt = if drift_requires_approval {
                            base_prompt
                        } else {
                            let explanation = context
                                .permission_context
                                .explain_decision(&tool_name, &tc.arguments)
                                .concise_summary();
                            format!("{}\nPermission explanation: {}", base_prompt, explanation)
                        };
                        let _ = tx
                            .send(StreamEvent::PermissionRequest {
                                id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                arguments: tc.arguments.clone(),
                                prompt: prompt.clone(),
                            })
                            .await;
                        if let Some(ref trace) = trace {
                            trace.record(TraceEvent::PermissionRequested {
                                tool: tool_name.clone(),
                                call_id: tool_id.clone(),
                                prompt: prompt.clone(),
                            });
                        }
                        let request = super::ToolApprovalRequest {
                            tool_call: tc.clone(),
                            prompt,
                            review: None,
                        };
                        match channel.submit(request).await {
                            Ok(is_approved) => approved = is_approved,
                            Err(e) => {
                                warn!("Tool approval error: {}", e);
                            }
                        }
                        if let Some(ref trace) = trace {
                            trace.record(TraceEvent::PermissionResolved {
                                tool: tool_name.clone(),
                                call_id: tool_id.clone(),
                                approved,
                            });
                        }
                    }
                    if approved {
                        if context.permission_context.mode
                            == crate::permissions::PermissionMode::Once
                        {
                            context.permission_context.grant_once(&tool_name);
                        }
                        if let Some(tx) = tx {
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
                        tool.execute(tc.arguments.clone(), context.clone()).await
                    } else {
                        ToolResult::error(format!(
                            "Permission denied: '{}' requires user confirmation.",
                            tool_name
                        ))
                    }
                } else {
                    if let Some(tx) = tx {
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

                // ── Security Audit & Denial Tracking ──────────────────────
                let params_summary = if let Some(tool) = conversation.tool_registry.get(&tool_name)
                {
                    tool.to_classifier_input(&tc.arguments)
                } else {
                    tool_name.clone()
                };

                if let Some(ref log) = conversation.audit_log {
                    let decision = if result.success {
                        "EXECUTED"
                    } else if result
                        .error
                        .as_deref()
                        .unwrap_or("")
                        .contains("Permission denied")
                    {
                        "DENIED"
                    } else {
                        "FAILED"
                    };
                    log.log_execution(&tool_name, &params_summary, result.success, decision)
                        .await;
                }

                if let Some(ref tracker) = conversation.denial_tracker {
                    if result.success {
                        tracker.record_success().await;
                    } else if result
                        .error
                        .as_deref()
                        .unwrap_or("")
                        .contains("Permission denied")
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
                    let mut tracker = conversation.cost_tracker.lock().await;
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
                (result, None)
            };

            if let (Some(hooks), Some(context)) = (&conversation.hook_manager, &hook_context) {
                let hook_start = hooks.current_record_sequence();
                hooks.run_post_tool(&tc, &result, context).await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                record_hook_traces(&trace, &hook_records);
            }

            if let Some(tx) = tx {
                let result_content = provider_tool_result_content(&tc, &result);
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tool_id.clone(),
                        result: result_content,
                    })
                    .await;
            }
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::ToolCompleted {
                    tool: tool_name,
                    call_id: tool_id,
                    success: result.success,
                    duration_ms: result.duration_ms,
                    output_chars: result.content.chars().count(),
                });
                let trace_ref = Some(trace.clone());
                record_mcp_resource_trace(&trace_ref, &tc, &result);
                record_web_retrieval_trace(&trace_ref, &tc, &result);
            }
            if result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("Permission denied")
            {
                lifecycle.denied(&tc);
            } else {
                lifecycle.completed(&tc, &result);
            }
            persist_tool_outcome_learning_event(
                conversation.session_store.as_ref(),
                &conversation.session_id,
                &tc,
                &result,
            );
            results.push((tc, result));
        }

        let lifecycle_snapshot = lifecycle.snapshot();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: serde_json::json!({}),
        }
    }

    #[test]
    fn batch_summarizes_results_and_lifecycle_statuses() {
        let mut lifecycle = ToolCallLifecycle::default();
        let denied = tool_call("call_1", "file_write");
        let failed = tool_call("call_2", "bash");
        let pre_executed = tool_call("call_3", "file_read");

        lifecycle.denied(&denied);
        lifecycle.completed(&failed, &ToolResult::error("nope"));
        lifecycle.provider_executed(&pre_executed, &ToolResult::success("ok"));

        let batch = ToolExecutionBatch::new(
            vec![
                (denied.clone(), ToolResult::error("denied")),
                (failed.clone(), ToolResult::error("nope")),
                (pre_executed.clone(), ToolResult::success("ok")),
            ],
            lifecycle.snapshot(),
        );

        assert!(batch.any_success());
        assert_eq!(batch.unsuccessful_count(), 2);
        let result_successes = batch
            .result_successes()
            .map(|(tool_call, success)| (tool_call.id.as_str(), success))
            .collect::<Vec<_>>();
        assert_eq!(
            result_successes,
            vec![("call_1", false), ("call_2", false), ("call_3", true)]
        );
        assert_eq!(batch.denied_count(), 1);
        assert_eq!(batch.failed_count(), 1);
        assert_eq!(batch.pre_executed_count(), 1);
    }
}
