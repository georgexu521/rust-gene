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
    record_permission_denial_recovery_trace, record_remote_bridge_trace,
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
use crate::tools::{ToolContext, ToolContextRetainedContext, ToolRegistry, ToolResult};
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
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
        let (results, lifecycle) = complete_provider_result_pairs(results, lifecycle);
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

fn complete_provider_result_pairs(
    mut results: Vec<(ToolCall, ToolResult)>,
    mut lifecycle: Vec<(String, ToolCallLifecycleRecord)>,
) -> (
    Vec<(ToolCall, ToolResult)>,
    Vec<(String, ToolCallLifecycleRecord)>,
) {
    let result_ids = results
        .iter()
        .map(|(tool_call, _)| tool_call.id.clone())
        .collect::<HashSet<_>>();

    for (call_id, record) in &mut lifecycle {
        if result_ids.contains(call_id) {
            continue;
        }

        let status = record.status;
        let mut result = ToolResult::error(format!(
            "Tool '{}' ended with lifecycle status {:?} but no terminal result was recorded. Treating it as interrupted.",
            record.tool_name, status
        ));
        merge_tool_result_metadata(
            &mut result,
            "tool_lifecycle_recovery",
            serde_json::json!({
                "schema": "tool_lifecycle_recovery.v1",
                "call_id": call_id,
                "tool": record.tool_name.clone(),
                "previous_status": format!("{:?}", status),
                "terminal_result": "interrupted",
                "synthesized": true,
            }),
        );
        results.push((
            ToolCall {
                id: call_id.clone(),
                name: record.tool_name.clone(),
                arguments: serde_json::json!({}),
            },
            result,
        ));
        record.status = ToolCallStatus::Failed;
    }

    (results, lifecycle)
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
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) has_changes_before_tools: bool,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) lifecycle: &'a mut ToolCallLifecycle,
}

struct ReadWriteExecutionContext<'a> {
    tx: Option<&'a mpsc::Sender<StreamEvent>>,
    trace: &'a Option<TraceCollector>,
    runtime_context: &'a ToolRuntimeContext,
    retained_context: &'a ToolContextRetainedContext,
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
    retained_retrieval_items: usize,
    retained_skill_triggers: usize,
    retained_context_tokens: usize,
    retained_context_provenance: Vec<String>,
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
        retained_context: &ToolContextRetainedContext,
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
            retained_retrieval_items: retained_context.retrieval_items.len(),
            retained_skill_triggers: retained_context.skill_triggers.len(),
            retained_context_tokens: retained_context.token_estimate,
            retained_context_provenance: retained_context.provenance.clone(),
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
                "retained_context": {
                    "retrieval_items": self.retained_retrieval_items,
                    "skill_triggers": self.retained_skill_triggers,
                    "token_estimate": self.retained_context_tokens,
                    "provenance": self.retained_context_provenance,
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
    working_dir: &'a Path,
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

        let destructive_check = self
            .destructive_scope
            .check_tool_call(tool_call, self.working_dir);
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
        retained_context: &ToolContextRetainedContext,
        tool_call: &ToolCall,
        parent_tool_calls: Vec<ToolCall>,
        parent_assistant_content: String,
    ) -> impl Future<Output = (ToolCall, ToolResult)> + 'static {
        let execution = &self.context;
        let registry = execution.tool_registry.clone();
        let tc_clone = tool_call.clone();
        let tool_name = tool_call.name.clone();
        let context = execution
            .tool_context(trace, retained_context)
            .with_tool_call_metadata(tool_name.clone(), tc_clone.id.clone())
            .with_parent_assistant_tool_calls(parent_tool_calls, parent_assistant_content);
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
            completed.push((order, (tc, result)));
        }

        completed.sort_by_key(|(order, _)| *order);
        completed.into_iter().map(|(_, result)| result).collect()
    }

    async fn execute_read_write_calls(
        &self,
        read_write_calls: Vec<ToolCall>,
        exec_context: ReadWriteExecutionContext<'_>,
        lifecycle: &mut ToolCallLifecycle,
    ) -> Vec<(ToolCall, ToolResult)> {
        let execution = &self.context;
        let mut results = Vec::new();

        for tc in read_write_calls {
            let tool_id = tc.id.clone();
            let tool_name = tc.name.clone();
            if !tool_allowed_by_context(&execution.allowed_tools, &tool_name) {
                let mut result = tool_not_allowed_result(&tc);
                exec_context.runtime_context.attach(
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

            if let Some(tx) = exec_context.tx {
                let _ = tx
                    .send(StreamEvent::ToolExecutionStart {
                        id: tool_id.clone(),
                        name: tool_name.clone(),
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
                } else if permission_evaluation.requires_approval {
                    let approved = PermissionController::request_user_permission(
                        &tc,
                        &permission_evaluation,
                        execution.approval_channel.as_ref(),
                        exec_context.tx,
                        exec_context.trace,
                        execution.hook_manager.as_ref(),
                        &context,
                    )
                    .await;
                    if approved {
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
                exec_context.runtime_context.attach(
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
                exec_context.runtime_context.attach(
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
                record_hook_traces(exec_context.trace, &hook_records);
            }

            if let Some(tx) = exec_context.tx {
                let result_content = ToolResultNormalizer::normalize(&tc, &result).ui_content;
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tool_id.clone(),
                        result: result_content,
                        metadata: tool_completion_metadata(&result),
                    })
                    .await;
            }
            if let Some(ref trace) = exec_context.trace {
                trace.record(TraceEvent::ToolCompleted {
                    tool: tool_name,
                    call_id: tool_id,
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
            parent_assistant_content,
            tx,
            pre_executed,
            trace,
            route,
            resource_policy,
            exposed_tool_names,
            retained_context,
            action_checkpoint_active,
            action_checkpoint_lookup_count,
            has_changes_before_tools,
            destructive_scope,
            lifecycle,
        } = request;
        let execution = &self.context;
        let mut read_only_jobs = Vec::new();
        let mut results: Vec<(ToolCall, ToolResult)> = Vec::new();
        let mut scheduled_count = 0usize;
        let mut serial_boundary_seen = false;
        lifecycle.pending_batch(tool_calls);
        let runtime_context = ToolRuntimeContext::new(
            route,
            resource_policy,
            action_checkpoint_active,
            has_changes_before_tools,
            exposed_tool_names.len(),
            retained_context,
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
            working_dir: execution.base_tool_context.working_dir.as_path(),
            trace: &trace,
            runtime_context: &runtime_context,
        };
        let concurrency =
            read_only_tool_concurrency().min(resource_policy.parallelism_limit.max(1));

        for (i, tc) in tool_calls.iter().enumerate() {
            if tc.name.is_empty() {
                continue;
            }

            let concurrency_safe = tool_call_is_concurrency_safe(
                execution.tool_registry.as_ref(),
                &tc.name,
                &tc.arguments,
            );
            if !concurrency_safe && !read_only_jobs.is_empty() {
                let read_only_results = self
                    .collect_read_only_results(
                        std::mem::take(&mut read_only_jobs),
                        concurrency,
                        tx,
                        lifecycle,
                    )
                    .await;
                results.extend(read_only_results);
            }

            if let ToolExecutionGateOutcome::Deny(result) = gate.evaluate(tc, scheduled_count) {
                if !read_only_jobs.is_empty() {
                    let read_only_results = self
                        .collect_read_only_results(
                            std::mem::take(&mut read_only_jobs),
                            concurrency,
                            tx,
                            lifecycle,
                        )
                        .await;
                    results.extend(read_only_results);
                }
                persist_tool_outcome_learning_event(
                    execution.session_store.as_ref(),
                    &execution.session_id,
                    tc,
                    &result,
                );
                lifecycle.denied(tc);
                results.push((tc.clone(), result));
                scheduled_count += 1;
                serial_boundary_seen = true;
                continue;
            }

            if let Some(pre_result) = pre_executed.get(&i).filter(|_| !serial_boundary_seen) {
                if !read_only_jobs.is_empty() {
                    let read_only_results = self
                        .collect_read_only_results(
                            std::mem::take(&mut read_only_jobs),
                            concurrency,
                            tx,
                            lifecycle,
                        )
                        .await;
                    results.extend(read_only_results);
                }
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
                        })
                        .await;
                }
                scheduled_count += 1;
                continue;
            }

            if concurrency_safe {
                lifecycle.running(tc, true, false);
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::ToolExecutionStart {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                        })
                        .await;
                }
                read_only_jobs.push((
                    i,
                    self.read_only_job(
                        &trace,
                        &runtime_context,
                        retained_context,
                        tc,
                        tool_calls.to_vec(),
                        parent_assistant_content.to_string(),
                    ),
                ));
                scheduled_count += 1;
            } else {
                let read_write_results = self
                    .execute_read_write_calls(
                        vec![tc.clone()],
                        ReadWriteExecutionContext {
                            tx,
                            trace: &trace,
                            runtime_context: &runtime_context,
                            retained_context,
                            parent_tool_calls: tool_calls,
                            parent_assistant_content,
                        },
                        lifecycle,
                    )
                    .await;
                results.extend(read_write_results);
                scheduled_count += 1;
                serial_boundary_seen = true;
            }
        }

        if !read_only_jobs.is_empty() {
            let read_only_results = self
                .collect_read_only_results(read_only_jobs, concurrency, tx, lifecycle)
                .await;
            results.extend(read_only_results);
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
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::resource_policy::ResourcePolicy;
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::{Tool, ToolContext};
    use async_openai::types::ChatCompletionResponseStream;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn tool_call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: serde_json::json!({}),
        }
    }

    struct NoopProvider;

    #[async_trait]
    impl LlmProvider for NoopProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Ok(ChatResponse {
                content: String::new(),
                tool_calls: None,
                usage: None,
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("unused test provider stream"))
        }

        fn base_url(&self) -> &str {
            "test://noop"
        }

        fn default_model(&self) -> &str {
            "test"
        }
    }

    struct ProbeReadTool {
        writes: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for ProbeReadTool {
        fn name(&self) -> &str {
            "probe_read"
        }

        fn description(&self) -> &str {
            "Read the probe write counter"
        }

        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }

        async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
            ToolResult::success(format!(
                "writes_seen={}",
                self.writes.load(Ordering::SeqCst)
            ))
        }

        fn is_read_only(&self, _params: &Value) -> bool {
            true
        }

        fn is_concurrency_safe(&self, _params: &Value) -> bool {
            true
        }
    }

    struct ProbeWriteTool {
        writes: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for ProbeWriteTool {
        fn name(&self) -> &str {
            "probe_write"
        }

        fn description(&self) -> &str {
            "Increment the probe write counter"
        }

        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }

        async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
            let previous = self.writes.fetch_add(1, Ordering::SeqCst);
            ToolResult::success(format!("writes_before={previous}"))
        }
    }

    fn probe_loop(writes: Arc<AtomicUsize>) -> ConversationLoop {
        let mut registry = ToolRegistry::new();
        registry.register(ProbeReadTool {
            writes: writes.clone(),
        });
        registry.register(ProbeWriteTool { writes });
        ConversationLoop::new(
            Arc::new(NoopProvider),
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        )
    }

    async fn execute_probe_tools(
        loop_instance: &ConversationLoop,
        tool_calls: &[ToolCall],
        pre_executed: HashMap<usize, ToolResult>,
    ) -> ToolExecutionBatch {
        let route = IntentRouter::new().route("probe ordered tools");
        let mut policy = ResourcePolicy::from_route(&route);
        policy.max_tool_calls = 20;
        policy.parallelism_limit = 4;
        let destructive_scope = DestructiveScopeContract::from_user_request(
            "probe ordered tools",
            &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
        let exposed_tool_names =
            HashSet::from(["probe_read".to_string(), "probe_write".to_string()]);
        let mut lifecycle = ToolCallLifecycle::default();

        ToolExecutionController::new(ToolExecutionContext::from_conversation(loop_instance))
            .execute_tools_parallel(ToolExecutionRequest {
                tool_calls,
                parent_assistant_content: "",
                tx: None,
                pre_executed,
                trace: None,
                route: Some(&route),
                resource_policy: &policy,
                exposed_tool_names: &exposed_tool_names,
                retained_context: &crate::tools::ToolContextRetainedContext::default(),
                action_checkpoint_active: false,
                action_checkpoint_lookup_count: 0,
                has_changes_before_tools: false,
                destructive_scope: &destructive_scope,
                lifecycle: &mut lifecycle,
            })
            .await
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

    #[test]
    fn batch_synthesizes_terminal_result_for_missing_lifecycle_result() {
        let pending = tool_call("call_missing", "bash");
        let mut lifecycle = ToolCallLifecycle::default();
        lifecycle.pending_batch(std::slice::from_ref(&pending));

        let batch = ToolExecutionBatch::new(Vec::new(), lifecycle.snapshot());

        assert_eq!(batch.results().len(), 1);
        assert_eq!(batch.results()[0].0.id, "call_missing");
        assert!(!batch.results()[0].1.success);
        assert!(batch.results()[0]
            .1
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("no terminal result was recorded"));
        assert_eq!(batch.failed_count(), 1);
        assert_eq!(
            batch.results()[0].1.data.as_ref().unwrap()["tool_lifecycle_recovery"]
                ["terminal_result"],
            "interrupted"
        );
    }

    #[tokio::test]
    async fn mixed_read_write_round_preserves_tool_call_order() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("call_read_before", "probe_read"),
            tool_call("call_write", "probe_write"),
            tool_call("call_read_after", "probe_read"),
        ];

        let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, _)| call.id.as_str())
                .collect::<Vec<_>>(),
            vec!["call_read_before", "call_write", "call_read_after"]
        );
        assert_eq!(results[0].1.content, "writes_seen=0");
        assert_eq!(results[1].1.content, "writes_before=0");
        assert_eq!(results[2].1.content, "writes_seen=1");
    }

    #[tokio::test]
    async fn consecutive_read_batches_stay_ordered_across_writes() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("read_1", "probe_read"),
            tool_call("read_2", "probe_read"),
            tool_call("write_1", "probe_write"),
            tool_call("read_3", "probe_read"),
            tool_call("read_4", "probe_read"),
        ];

        let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, result)| (call.id.as_str(), result.content.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("read_1", "writes_seen=0"),
                ("read_2", "writes_seen=0"),
                ("write_1", "writes_before=0"),
                ("read_3", "writes_seen=1"),
                ("read_4", "writes_seen=1"),
            ]
        );
    }

    #[tokio::test]
    async fn denied_tool_between_read_batches_preserves_result_order() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("read_before", "probe_read"),
            tool_call("denied", "probe_denied"),
            tool_call("read_after", "probe_read"),
        ];

        let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, _)| call.id.as_str())
                .collect::<Vec<_>>(),
            vec!["read_before", "denied", "read_after"]
        );
        assert_eq!(results[0].1.content, "writes_seen=0");
        assert!(!results[1].1.success);
        assert!(results[1].1.content.contains("was not exposed"));
        assert_eq!(results[2].1.content, "writes_seen=0");
        assert_eq!(batch.denied_count(), 1);
    }

    #[tokio::test]
    async fn pre_executed_read_only_result_before_serial_boundary_keeps_original_position() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("read_pre_executed", "probe_read"),
            tool_call("write", "probe_write"),
            tool_call("read_after", "probe_read"),
        ];
        let pre_executed = HashMap::from([(0usize, ToolResult::success("pre_executed_read"))]);

        let batch = execute_probe_tools(&loop_instance, &tool_calls, pre_executed).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, result)| (call.id.as_str(), result.content.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("read_pre_executed", "pre_executed_read"),
                ("write", "writes_before=0"),
                ("read_after", "writes_seen=1"),
            ]
        );
        assert_eq!(batch.pre_executed_count(), 1);
    }

    #[tokio::test]
    async fn pre_executed_read_only_result_after_serial_boundary_is_rerun() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("read_before", "probe_read"),
            tool_call("write", "probe_write"),
            tool_call("read_pre_executed", "probe_read"),
        ];
        let pre_executed = HashMap::from([(2usize, ToolResult::success("pre_executed_read"))]);

        let batch = execute_probe_tools(&loop_instance, &tool_calls, pre_executed).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, result)| (call.id.as_str(), result.content.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("read_before", "writes_seen=0"),
                ("write", "writes_before=0"),
                ("read_pre_executed", "writes_seen=1"),
            ]
        );
        assert_eq!(batch.pre_executed_count(), 0);
    }
}
