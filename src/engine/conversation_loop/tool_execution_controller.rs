use super::permission_controller::PermissionController;
use super::tool_call_lifecycle::{ToolCallLifecycle, ToolCallLifecycleRecord, ToolCallStatus};
use super::tool_context_helpers::{tool_allowed_by_context, tool_not_allowed_result};
use super::tool_execution::{read_only_tool_concurrency, tool_call_is_concurrency_safe};
use super::tool_metadata::{
    attach_tool_contract_metadata, attach_tool_execution_metadata, merge_tool_result_metadata,
    persist_tool_outcome_learning_event, tool_execution_start_progress,
};
use super::tool_result_controller::{invalid_tool_params_result, ToolResultNormalizer};
use super::turn_recording::{
    record_goal_drift_if_needed, record_hook_traces, record_mcp_resource_trace,
    record_web_retrieval_trace,
};
use super::ConversationLoop;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::hooks::HookDecision;
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use crate::tools::{ToolContext, ToolRegistry, ToolResult};
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::debug;

#[derive(Debug, Clone, Default)]
pub(super) struct ToolExecutionBatch {
    results: Vec<(ToolCall, ToolResult)>,
    lifecycle: Vec<(String, ToolCallLifecycleRecord)>,
}

impl ToolExecutionBatch {
    pub(super) fn new(
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
    pub(super) route: Option<&'a IntentRoute>,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) has_changes_before_tools: bool,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) lifecycle: &'a mut ToolCallLifecycle,
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

    fn tool_context(&self, trace: &Option<TraceCollector>) -> ToolContext {
        match trace {
            Some(trace) => self
                .base_tool_context
                .clone()
                .with_trace_collector(trace.clone()),
            None => self.base_tool_context.clone(),
        }
    }
}

enum ToolExecutionGateOutcome {
    Allow,
    Deny(ToolResult),
}

#[derive(Debug, Clone)]
struct ToolRuntimeContext {
    has_route: bool,
    route_intent: String,
    route_workflow: String,
    route_retrieval: String,
    route_reasoning: String,
    route_risk: String,
    policy_latency: String,
    policy_parallelism_limit: usize,
    policy_max_tool_calls: usize,
    policy_context_budget_tokens: usize,
    policy_allow_fallback_model: bool,
    policy_cost_ceiling_usd: String,
    action_checkpoint_active: bool,
    has_changes_before_tools: bool,
    exposed_tools_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct ToolRuntimeTiming {
    started_at_unix_ms: Option<u64>,
    finished_at_unix_ms: Option<u64>,
}

impl ToolRuntimeTiming {
    fn instant() -> Self {
        let now = unix_time_millis();
        Self {
            started_at_unix_ms: now,
            finished_at_unix_ms: now,
        }
    }

    fn finished(started_at_unix_ms: Option<u64>) -> Self {
        Self {
            started_at_unix_ms,
            finished_at_unix_ms: unix_time_millis(),
        }
    }
}

impl ToolRuntimeContext {
    fn new(
        route: Option<&IntentRoute>,
        policy: &ResourcePolicy,
        action_checkpoint_active: bool,
        has_changes_before_tools: bool,
        exposed_tools_count: usize,
    ) -> Self {
        Self {
            has_route: route.is_some(),
            route_intent: route
                .map(|route| serde_label(&route.intent))
                .unwrap_or_default(),
            route_workflow: route
                .map(|route| serde_label(&route.workflow))
                .unwrap_or_default(),
            route_retrieval: route
                .map(|route| serde_label(&route.retrieval))
                .unwrap_or_default(),
            route_reasoning: route
                .map(|route| serde_label(&route.reasoning))
                .unwrap_or_default(),
            route_risk: route
                .map(|route| serde_label(&route.risk))
                .unwrap_or_default(),
            policy_latency: serde_label(&policy.latency),
            policy_parallelism_limit: policy.parallelism_limit,
            policy_max_tool_calls: policy.max_tool_calls,
            policy_context_budget_tokens: policy.context_budget_tokens,
            policy_allow_fallback_model: policy.allow_fallback_model,
            policy_cost_ceiling_usd: format!("{:.4}", policy.cost_ceiling_usd),
            action_checkpoint_active,
            has_changes_before_tools,
            exposed_tools_count,
        }
    }

    fn attach(
        &self,
        result: &mut ToolResult,
        parallel: bool,
        pre_executed: bool,
        timing: Option<ToolRuntimeTiming>,
    ) {
        let route = if self.has_route {
            serde_json::json!({
                "intent": self.route_intent,
                "workflow": self.route_workflow,
                "retrieval": self.route_retrieval,
                "reasoning": self.route_reasoning,
                "risk": self.route_risk,
            })
        } else {
            serde_json::Value::Null
        };
        merge_tool_result_metadata(
            result,
            "tool_runtime",
            serde_json::json!({
                "route": route,
                "policy": {
                    "latency": self.policy_latency,
                    "parallelism_limit": self.policy_parallelism_limit,
                    "max_tool_calls": self.policy_max_tool_calls,
                    "context_budget_tokens": self.policy_context_budget_tokens,
                    "allow_fallback_model": self.policy_allow_fallback_model,
                    "cost_ceiling_usd": self.policy_cost_ceiling_usd,
                },
                "execution": {
                    "parallel": parallel,
                    "pre_executed": pre_executed,
                    "action_checkpoint_active": self.action_checkpoint_active,
                    "has_changes_before_tools": self.has_changes_before_tools,
                    "exposed_tools_count": self.exposed_tools_count,
                    "started_at_unix_ms": timing.and_then(|timing| timing.started_at_unix_ms),
                    "finished_at_unix_ms": timing.and_then(|timing| timing.finished_at_unix_ms),
                },
            }),
        );
    }
}

fn unix_time_millis() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
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

struct ToolExecutionGate<'a> {
    tool_registry: &'a ToolRegistry,
    active_goal: Option<&'a crate::engine::session_goal::SessionGoal>,
    allowed_tools: &'a Option<HashSet<String>>,
    resource_policy: &'a ResourcePolicy,
    exposed_tool_names: &'a HashSet<String>,
    action_checkpoint_active: bool,
    action_checkpoint_lookup_count: usize,
    has_changes_before_tools: bool,
    destructive_scope: &'a DestructiveScopeContract,
    trace: &'a Option<TraceCollector>,
    runtime_context: &'a ToolRuntimeContext,
}

impl<'a> ToolExecutionGate<'a> {
    fn evaluate(&self, tool_call: &ToolCall, scheduled_count: usize) -> ToolExecutionGateOutcome {
        if !self.exposed_tool_names.contains(&tool_call.name) {
            let error = if self.action_checkpoint_active {
                ConversationLoop::action_checkpoint_unexposed_tool_message(
                    &tool_call.name,
                    self.exposed_tool_names,
                    self.action_checkpoint_lookup_count,
                )
            } else {
                format!(
                    "Tool '{}' was not exposed in the current request and cannot be executed.",
                    tool_call.name
                )
            };
            return self.deny_with_trace(tool_call, ToolResult::error(error));
        }

        if let Some(tool) = self.tool_registry.get(&tool_call.name) {
            if let Some(error) = tool.validate_params(&tool_call.arguments) {
                return self
                    .deny_with_trace(tool_call, invalid_tool_params_result(tool_call, error));
            }
        }

        if scheduled_count >= self.resource_policy.max_tool_calls {
            let result = ToolResult::error(format!(
                "Resource policy blocked tool '{}': max tool calls ({}) reached.",
                tool_call.name, self.resource_policy.max_tool_calls
            ));
            return self.deny_with_trace(tool_call, result);
        }

        record_goal_drift_if_needed(self.trace, self.active_goal, tool_call);

        if !tool_allowed_by_context(self.allowed_tools, &tool_call.name) {
            return ToolExecutionGateOutcome::Deny(tool_not_allowed_result(tool_call));
        }

        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let destructive_check = self
            .destructive_scope
            .check_tool_call(tool_call, &working_dir);
        if destructive_check.applies {
            if let Some(ref trace) = self.trace {
                trace.record(TraceEvent::DestructiveScopeChecked {
                    tool: tool_call.name.clone(),
                    call_id: tool_call.id.clone(),
                    operation: destructive_check.operation.clone(),
                    target: destructive_check.target.clone(),
                    allowed: destructive_check.allowed,
                    reason: destructive_check.reason.clone(),
                });
            }
            if !destructive_check.allowed {
                let result = ToolResult::error(format!(
                    "Destructive scope blocked: {}",
                    destructive_check.reason
                ));
                return self.deny_with_trace(tool_call, result);
            }
        }

        if self.action_checkpoint_active
            && tool_call.name == "bash"
            && !ConversationLoop::bash_allowed_at_action_checkpoint(
                &tool_call.arguments,
                self.has_changes_before_tools,
            )
        {
            let result = ToolResult::error(
                "Bash is restricted during the action checkpoint: use file_edit/file_write/file_patch for patches so permission, stale-read, diff, and rollback checks stay active. Bash is allowed only for validation after files have changed."
                    .to_string(),
            );
            return self.deny_with_trace(tool_call, result);
        }

        if self.action_checkpoint_active && tool_call.name == "file_edit" {
            if let Some(reason) = ConversationLoop::action_checkpoint_file_edit_rejection(
                &tool_call.arguments,
                &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            ) {
                let result =
                    ToolResult::error(format!("Action checkpoint file_edit rejected: {reason}"));
                return self.deny_with_trace(tool_call, result);
            }
        }

        ToolExecutionGateOutcome::Allow
    }

    fn deny_with_trace(
        &self,
        tool_call: &ToolCall,
        mut result: ToolResult,
    ) -> ToolExecutionGateOutcome {
        attach_tool_execution_metadata(tool_call, &mut result);
        if let Some(tool) = self.tool_registry.get(&tool_call.name) {
            attach_tool_contract_metadata(tool, tool_call, &mut result);
        }
        self.runtime_context.attach(
            &mut result,
            false,
            false,
            Some(ToolRuntimeTiming::instant()),
        );
        if let Some(ref trace) = self.trace {
            trace.record(TraceEvent::ToolStarted {
                tool: tool_call.name.clone(),
                call_id: tool_call.id.clone(),
                parallel: false,
                pre_executed: false,
            });
            trace.record(TraceEvent::ToolCompleted {
                tool: tool_call.name.clone(),
                call_id: tool_call.id.clone(),
                success: false,
                duration_ms: Some(0),
                output_chars: result.content.chars().count(),
            });
        }
        ToolExecutionGateOutcome::Deny(result)
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
        trace: &Option<TraceCollector>,
        runtime_context: &ToolRuntimeContext,
        tool_call: &ToolCall,
    ) -> impl Future<Output = (ToolCall, ToolResult)> + 'static {
        let execution = &self.context;
        let registry = execution.tool_registry.clone();
        let tc_clone = tool_call.clone();
        let tool_name = tool_call.name.clone();
        let context = execution
            .tool_context(trace)
            .with_tool_call_metadata(tool_name.clone(), tc_clone.id.clone());
        let cost_tracker = execution.cost_tracker.clone();
        let hook_manager = execution.hook_manager.clone();
        let trace = trace.clone();
        let runtime_context = runtime_context.clone();
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
            runtime_context.attach(
                &mut result,
                true,
                false,
                Some(ToolRuntimeTiming::finished(started_at_unix_ms)),
            );
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
        }
    }

    async fn collect_read_only_results<F>(
        &self,
        read_only_jobs: Vec<F>,
        concurrency: usize,
        tx: Option<&mpsc::Sender<StreamEvent>>,
        lifecycle: &mut ToolCallLifecycle,
    ) -> Vec<(ToolCall, ToolResult)>
    where
        F: Future<Output = (ToolCall, ToolResult)>,
    {
        let execution = &self.context;
        let mut results = Vec::new();
        let mut readonly_stream =
            futures::stream::iter(read_only_jobs).buffer_unordered(concurrency);

        while let Some((tc, result)) = readonly_stream.next().await {
            lifecycle.completed(&tc, &result);
            persist_tool_outcome_learning_event(
                execution.session_store.as_ref(),
                &execution.session_id,
                &tc,
                &result,
            );
            if let Some(tx) = tx {
                let result_content = ToolResultNormalizer::normalize(&tc, &result).ui_content;
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tc.id.clone(),
                        result: result_content,
                        metadata: tool_completion_metadata(&result),
                    })
                    .await;
            }
            results.push((tc, result));
        }

        results
    }

    async fn execute_read_write_calls(
        &self,
        read_write_calls: Vec<ToolCall>,
        tx: Option<&mpsc::Sender<StreamEvent>>,
        trace: &Option<TraceCollector>,
        runtime_context: &ToolRuntimeContext,
        lifecycle: &mut ToolCallLifecycle,
    ) -> Vec<(ToolCall, ToolResult)> {
        let execution = &self.context;
        let mut results = Vec::new();

        for tc in read_write_calls {
            let tool_id = tc.id.clone();
            let tool_name = tc.name.clone();
            if !tool_allowed_by_context(&execution.allowed_tools, &tool_name) {
                let mut result = tool_not_allowed_result(&tc);
                runtime_context.attach(
                    &mut result,
                    false,
                    false,
                    Some(ToolRuntimeTiming::instant()),
                );
                persist_tool_outcome_learning_event(
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

            let (result, hook_context) = if let Some(tool) = execution.tool_registry.get(&tool_name)
            {
                let mut context = execution
                    .tool_context(trace)
                    .with_tool_call_metadata(tool_name.clone(), tool_id.clone());
                let drift_check = execution
                    .active_goal
                    .as_ref()
                    .map(|goal| {
                        crate::engine::goal_drift::GoalDriftDetector::new().check(goal, &tc)
                    })
                    .unwrap_or_else(crate::engine::goal_drift::DriftCheck::ok);
                let pre_decision = if let Some(ref hooks) = execution.hook_manager {
                    let hook_start = hooks.current_record_sequence();
                    let decision = hooks.run_pre_tool(&tc, &context).await;
                    let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                    record_hook_traces(trace, &hook_records);
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
                } else if permission_evaluation.requires_approval {
                    let approved = PermissionController::request_user_permission(
                        &tc,
                        &permission_evaluation,
                        execution.approval_channel.as_ref(),
                        tx,
                        trace,
                    )
                    .await;
                    if approved {
                        PermissionController::record_approved_session_rule(
                            &mut context,
                            &tool_name,
                        );
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
                        let mut result = tool.execute(tc.arguments.clone(), context.clone()).await;
                        if let Some(record) = permission_evaluation.record.as_ref() {
                            merge_tool_result_metadata(
                                &mut result,
                                "permission_request",
                                record.to_json_with_approval(true),
                            );
                        }
                        result
                    } else {
                        PermissionController::denied_result(
                            &tool_name,
                            permission_evaluation.record.as_ref(),
                        )
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
                attach_tool_contract_metadata(tool, &tc, &mut result);
                runtime_context.attach(
                    &mut result,
                    false,
                    false,
                    Some(ToolRuntimeTiming::finished(started_at_unix_ms)),
                );

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
                runtime_context.attach(
                    &mut result,
                    false,
                    false,
                    Some(ToolRuntimeTiming::instant()),
                );
                (result, None)
            };

            if let (Some(hooks), Some(context)) = (&execution.hook_manager, &hook_context) {
                let hook_start = hooks.current_record_sequence();
                hooks.run_post_tool(&tc, &result, context).await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                record_hook_traces(trace, &hook_records);
            }

            if let Some(tx) = tx {
                let result_content = ToolResultNormalizer::normalize(&tc, &result).ui_content;
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tool_id.clone(),
                        result: result_content,
                        metadata: tool_completion_metadata(&result),
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
            if PermissionController::is_permission_denied(&result) {
                lifecycle.denied(&tc);
            } else {
                lifecycle.completed(&tc, &result);
            }
            persist_tool_outcome_learning_event(
                execution.session_store.as_ref(),
                &execution.session_id,
                &tc,
                &result,
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
            tx,
            pre_executed,
            trace,
            route,
            resource_policy,
            exposed_tool_names,
            action_checkpoint_active,
            action_checkpoint_lookup_count,
            has_changes_before_tools,
            destructive_scope,
            lifecycle,
        } = request;
        let execution = &self.context;
        let mut read_only_jobs = Vec::new();
        let mut read_write_calls = Vec::new();
        let mut denied_results = Vec::new();
        let mut results: Vec<(ToolCall, ToolResult)> = Vec::new();
        lifecycle.pending_batch(tool_calls);
        let runtime_context = ToolRuntimeContext::new(
            route,
            resource_policy,
            action_checkpoint_active,
            has_changes_before_tools,
            exposed_tool_names.len(),
        );
        let gate = ToolExecutionGate {
            tool_registry: execution.tool_registry.as_ref(),
            active_goal: execution.active_goal.as_ref(),
            allowed_tools: &execution.allowed_tools,
            resource_policy,
            exposed_tool_names,
            action_checkpoint_active,
            action_checkpoint_lookup_count,
            has_changes_before_tools,
            destructive_scope,
            trace: &trace,
            runtime_context: &runtime_context,
        };

        for (i, tc) in tool_calls.iter().enumerate() {
            if tc.name.is_empty() {
                continue;
            }
            let scheduled_count = results.len()
                + denied_results.len()
                + read_only_jobs.len()
                + read_write_calls.len();
            if let ToolExecutionGateOutcome::Deny(result) = gate.evaluate(tc, scheduled_count) {
                persist_tool_outcome_learning_event(
                    execution.session_store.as_ref(),
                    &execution.session_id,
                    tc,
                    &result,
                );
                lifecycle.denied(tc);
                denied_results.push((tc.clone(), result));
                continue;
            }

            if let Some(pre_result) = pre_executed.get(&i) {
                let mut pre_result = pre_result.clone();
                attach_tool_execution_metadata(tc, &mut pre_result);
                if let Some(tool) = execution.tool_registry.get(&tc.name) {
                    attach_tool_contract_metadata(tool, tc, &mut pre_result);
                }
                runtime_context.attach(&mut pre_result, true, true, None);
                persist_tool_outcome_learning_event(
                    execution.session_store.as_ref(),
                    &execution.session_id,
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
                    let result_content =
                        ToolResultNormalizer::normalize(tc, &pre_result).ui_content;
                    let _ = tx
                        .send(StreamEvent::ToolExecutionComplete {
                            id: tc.id.clone(),
                            result: result_content,
                            metadata: tool_completion_metadata(&pre_result),
                        })
                        .await;
                }
                continue;
            }

            if tool_call_is_concurrency_safe(
                execution.tool_registry.as_ref(),
                &tc.name,
                &tc.arguments,
            ) {
                lifecycle.running(tc, true, false);
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::ToolExecutionStart {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                        })
                        .await;
                }
                read_only_jobs.push(self.read_only_job(&trace, &runtime_context, tc));
            } else {
                read_write_calls.push(tc.clone());
            }
        }

        results.append(&mut denied_results);

        let concurrency =
            read_only_tool_concurrency().min(resource_policy.parallelism_limit.max(1));
        let read_only_results = self
            .collect_read_only_results(read_only_jobs, concurrency, tx, lifecycle)
            .await;
        results.extend(read_only_results);

        let read_write_results = self
            .execute_read_write_calls(read_write_calls, tx, &trace, &runtime_context, lifecycle)
            .await;
        results.extend(read_write_results);

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

fn tool_completion_metadata(result: &ToolResult) -> Option<serde_json::Value> {
    result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_summary"))
        .cloned()
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
