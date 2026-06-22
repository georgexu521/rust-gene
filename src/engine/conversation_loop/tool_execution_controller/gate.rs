//! Tool execution controller support.
//!
//! Separates execution gates, runtime context, and batch state from the conversation-loop control flow.

use super::super::tool_context_helpers::{tool_allowed_by_context, tool_not_allowed_result};
use super::super::tool_metadata::{attach_tool_contract_metadata, attach_tool_execution_metadata};
use super::super::tool_result_controller::invalid_tool_params_result;
use super::super::turn_recording::record_goal_drift_if_needed;
use super::super::ConversationLoop;
use super::action_review::{
    attach_action_review_metadata, record_action_decision_if_needed, record_action_review,
    record_tool_observation,
};
use super::runtime_context::{ToolRuntimeContext, ToolRuntimeTiming};
use crate::engine::action_review::{ActionReview, ActionReviewInput};
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::session_goal::SessionGoal;
use crate::engine::task_context::AgentTaskState;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use crate::tools::{ToolContextRetainedContext, ToolRegistry, ToolResult};
use std::collections::HashSet;
use std::path::Path;

pub(super) enum ToolExecutionGateOutcome {
    Allow(Box<ActionReview>),
    Deny(ToolResult),
}

pub(super) struct ReadOnlyJobInput<'a> {
    pub(super) trace: &'a Option<TraceCollector>,
    pub(super) runtime_context: &'a ToolRuntimeContext,
    pub(super) retained_context: &'a ToolContextRetainedContext,
    pub(super) tool_call: &'a ToolCall,
    pub(super) action_review: ActionReview,
    pub(super) parent_tool_calls: Vec<ToolCall>,
    pub(super) parent_assistant_content: String,
}

pub(super) struct ToolExecutionGate<'a> {
    pub(super) tool_registry: &'a ToolRegistry,
    pub(super) active_goal: Option<&'a SessionGoal>,
    pub(super) task_state: Option<&'a AgentTaskState>,
    pub(super) allowed_tools: &'a Option<HashSet<String>>,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) has_changes_before_tools: bool,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) working_dir: &'a Path,
    pub(super) trace: &'a Option<TraceCollector>,
    pub(super) runtime_context: &'a ToolRuntimeContext,
    pub(super) permission_context: &'a crate::permissions::PermissionContext,
}

impl<'a> ToolExecutionGate<'a> {
    pub(super) fn evaluate(
        &self,
        tool_call: &ToolCall,
        scheduled_count: usize,
    ) -> ToolExecutionGateOutcome {
        let decision = self.runtime_context.action_decision(tool_call);
        record_action_decision_if_needed(self.trace, tool_call, &decision);
        let tool = self.tool_registry.get(&tool_call.name);
        let context_allows_tool = tool_allowed_by_context(self.allowed_tools, &tool_call.name);
        let destructive_check = self
            .destructive_scope
            .check_tool_call(tool_call, self.working_dir);
        let action_checkpoint_rejection = self.action_checkpoint_rejection(tool_call);
        let review = ActionReview::build(ActionReviewInput {
            tool_call,
            tool,
            exposed_tool_names: self.exposed_tool_names,
            scheduled_count,
            max_tool_calls: self.resource_policy.max_tool_calls,
            action_decision: decision,
            permission_context: Some(self.permission_context),
            task_state: self.task_state,
            working_dir: Some(self.working_dir),
            tool_allowed_by_context: context_allows_tool,
            destructive_scope_check: Some(destructive_check.clone()),
            action_checkpoint_rejection: action_checkpoint_rejection.clone(),
        });
        record_action_review(self.trace, &review);

        if !review.tool_contract.available {
            let result = ToolResult::error(format!("Tool '{}' not found", tool_call.name));
            return self.deny_with_trace(tool_call, result, &review);
        }

        if !review.tool_contract.exposed {
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
            return self.deny_with_trace(tool_call, ToolResult::error(error), &review);
        }

        if let Some(error) = review.tool_contract.validation_error.clone() {
            return self.deny_with_trace(
                tool_call,
                invalid_tool_params_result(tool_call, error),
                &review,
            );
        }

        if !review.budget.allowed {
            let result = ToolResult::error(format!(
                "Resource policy blocked tool '{}': max tool calls ({}) reached.",
                tool_call.name, self.resource_policy.max_tool_calls
            ));
            return self.deny_with_trace(tool_call, result, &review);
        }

        record_goal_drift_if_needed(self.trace, self.active_goal, self.task_state, tool_call);

        if !context_allows_tool {
            return self.deny_with_trace(tool_call, tool_not_allowed_result(tool_call), &review);
        }

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
                return self.deny_with_trace(tool_call, result, &review);
            }
        }

        if let Some(reason) = action_checkpoint_rejection {
            let result = if tool_call.name == "file_edit" {
                ToolResult::error(format!("Action checkpoint file_edit rejected: {reason}"))
            } else {
                ToolResult::error(reason)
            };
            return self.deny_with_trace(tool_call, result, &review);
        }

        if review.decision.blocks_execution() {
            let result = ToolResult::error(format!(
                "{}\nRecovery: {}",
                review.user_reason, review.model_recovery
            ));
            return self.deny_with_trace(tool_call, result, &review);
        }

        ToolExecutionGateOutcome::Allow(Box::new(review))
    }

    fn action_checkpoint_rejection(&self, tool_call: &ToolCall) -> Option<String> {
        if !self.action_checkpoint_active {
            return None;
        }

        if tool_call.name == "bash"
            && !ConversationLoop::bash_allowed_at_action_checkpoint(
                &tool_call.arguments,
                self.has_changes_before_tools,
                self.exposed_tool_names,
            )
        {
            return Some(
                "Bash is restricted during the action checkpoint: use file_edit/file_write/file_patch for patches so permission, stale-read, diff, and rollback checks stay active. Bash is allowed only for validation after files have changed."
                    .to_string(),
            );
        }

        if tool_call.name == "file_edit" {
            return ConversationLoop::action_checkpoint_file_edit_rejection(
                &tool_call.arguments,
                &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            );
        }

        None
    }

    fn deny_with_trace(
        &self,
        tool_call: &ToolCall,
        mut result: ToolResult,
        review: &ActionReview,
    ) -> ToolExecutionGateOutcome {
        attach_tool_execution_metadata(tool_call, &mut result);
        if let Some(tool) = self.tool_registry.get(&tool_call.name) {
            attach_tool_contract_metadata(tool, tool_call, &mut result);
        }
        attach_action_review_metadata(&mut result, review);
        self.runtime_context.attach(
            &mut result,
            false,
            false,
            Some(ToolRuntimeTiming::instant()),
        );
        self.runtime_context
            .attach_action_decision(tool_call, &mut result);
        record_tool_observation(self.trace, tool_call, &result);
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
            let (reason, terminal_status, action) = match review.decision {
                crate::engine::action_review::ActionReviewDecision::Deny => {
                    ("action_denied", "blocked", "stop")
                }
                crate::engine::action_review::ActionReviewDecision::Revise => {
                    ("action_needs_revision", "blocked", "replan")
                }
                crate::engine::action_review::ActionReviewDecision::AskUser => {
                    ("high_risk_needs_user", "needs_user", "ask_user")
                }
                crate::engine::action_review::ActionReviewDecision::Allow => {
                    ("no_issue", "missing", "continue")
                }
            };
            trace.record(TraceEvent::StopCheckEvaluated {
                status: "stop".to_string(),
                reason: reason.to_string(),
                stage: "PreAction".to_string(),
                terminal_status: Some(terminal_status.to_string()),
                action: action.to_string(),
                no_code_progress_rounds: self.runtime_context.no_progress_rounds,
                action_checkpoint_active: self.action_checkpoint_active,
                summary: review.user_reason.clone(),
                evidence_items: review.reasons.len().max(1),
                failure_type: Some(review.primary_reason.as_str().to_string()),
                recovery_plan_id: None,
                rollback_recommended: false,
                next_action: Some(review.model_recovery.clone()),
            });
        }
        ToolExecutionGateOutcome::Deny(result)
    }
}
