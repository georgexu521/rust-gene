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
    pub(super) allow_validation_without_changes: bool,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) working_dir: &'a Path,
    pub(super) labrun_context: Option<crate::lab::policy_overlay::LabRunExecutionContext>,
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
            labrun_context: self.labrun_context.clone(),
            tool_allowed_by_context: context_allows_tool,
            destructive_scope_check: Some(destructive_check.clone()),
            action_checkpoint_rejection: action_checkpoint_rejection.clone(),
        });
        let _ = crate::lab::policy_overlay::record_labrun_policy_event(
            self.working_dir,
            &review.labrun_policy,
        );
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

        if let Err(reason) = crate::lab::policy_overlay::revalidate_labrun_policy_review(
            self.working_dir,
            &review.labrun_policy,
        ) {
            let result = ToolResult::error(format!(
                "LabRun policy state changed before execution: {reason}"
            ));
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
                self.allow_validation_without_changes,
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
                self.working_dir,
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

#[cfg(test)]
mod tests {
    use super::super::runtime_context::ToolRuntimeContextInput;
    use super::*;
    use crate::engine::intent_router::ReasoningPolicy;
    use crate::engine::resource_policy::{LatencyTarget, ResourcePolicy};
    use crate::engine::task_context::AgentTaskStage;
    use crate::lab::execution_binding::LabExecutionBinding;
    use crate::lab::orchestrator::LabOrchestrator;
    use crate::permissions::{PermissionContext, PermissionMode};
    use crate::services::api::ToolCall;
    use crate::tools::{BashTool, FileWriteTool, ToolContextRetainedContext, ToolRegistry};
    use std::collections::{HashMap, HashSet};

    fn test_resource_policy() -> ResourcePolicy {
        ResourcePolicy {
            latency: LatencyTarget::Fast,
            cost_ceiling_usd: 0.0,
            reasoning: ReasoningPolicy::Low,
            parallelism_limit: 1,
            max_tool_calls: 8,
            context_budget_tokens: 1024,
            allow_fallback_model: false,
            reason: "test policy".to_string(),
        }
    }

    fn test_runtime_context<'a>(
        policy: &'a ResourcePolicy,
        retained_context: &'a ToolContextRetainedContext,
    ) -> ToolRuntimeContext {
        ToolRuntimeContext::new(ToolRuntimeContextInput {
            route: None,
            policy,
            task_stage: AgentTaskStage::Edit,
            action_checkpoint_active: false,
            no_progress_rounds: 0,
            has_changes_before_tools: false,
            exposed_tools_count: 2,
            retained_context,
            task_state: None,
        })
    }

    #[test]
    fn labrun_child_file_write_scope_violation_is_denied_before_execution() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        std::fs::create_dir_all(project_root.join("src/lab")).unwrap();
        let isolated = project_root.join("isolated-worktree");
        std::fs::create_dir_all(&isolated).unwrap();
        std::fs::write(isolated.join("README.md"), "original\n").unwrap();

        let orchestrator = LabOrchestrator::for_project(project_root);
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped LabRun slice",
                "Only edit src/lab.",
                vec!["src/lab".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let binding = LabExecutionBinding::for_graduate_task(
            &task,
            "dispatch_scope_test",
            "agent_task_scope_test",
            project_root,
            &isolated,
            Some("state_v1".to_string()),
        )
        .unwrap();
        let mut metadata = HashMap::new();
        binding.insert_into_metadata(&mut metadata).unwrap();
        let labrun_context =
            crate::lab::policy_overlay::LabRunExecutionContext::from_metadata(&metadata);

        let mut registry = ToolRegistry::new();
        registry.register(FileWriteTool);
        let resource_policy = test_resource_policy();
        let retained_context = ToolContextRetainedContext::default();
        let runtime_context = test_runtime_context(&resource_policy, &retained_context);
        let exposed_tool_names = HashSet::from(["file_write".to_string()]);
        let allowed_tools = None;
        let destructive_scope =
            crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
                "make a scoped LabRun edit",
                &isolated,
            );
        let mut permission_context = PermissionContext::new(&isolated);
        permission_context.mode = PermissionMode::AutoAll;
        let trace = None;
        let gate = ToolExecutionGate {
            tool_registry: &registry,
            active_goal: None,
            task_state: None,
            allowed_tools: &allowed_tools,
            resource_policy: &resource_policy,
            exposed_tool_names: &exposed_tool_names,
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            has_changes_before_tools: false,
            allow_validation_without_changes: false,
            destructive_scope: &destructive_scope,
            working_dir: &isolated,
            labrun_context,
            trace: &trace,
            runtime_context: &runtime_context,
            permission_context: &permission_context,
        };
        let tool_call = ToolCall {
            id: "call_file_write".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({
                "path": "README.md",
                "content": "out of scope\n",
            }),
        };

        let result = match gate.evaluate(&tool_call, 0) {
            ToolExecutionGateOutcome::Deny(result) => result,
            ToolExecutionGateOutcome::Allow(_) => {
                panic!("out-of-scope LabRun child file_write should be denied")
            }
        };

        assert!(!result.success);
        assert!(result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("labrun_policy_violation"));
        assert_eq!(
            std::fs::read_to_string(isolated.join("README.md")).unwrap(),
            "original\n"
        );
        let events = orchestrator
            .store()
            .list_run_events(&run.lab_run_id)
            .unwrap();
        assert!(events.iter().any(|event| {
            event.event_type == "labrun_policy_blocked"
                && event.payload["active_graduate_task_id"] == task.task_id
                && event.payload["active_dispatch_id"] == "dispatch_scope_test"
                && event.payload["action_family"] == "file_mutation"
                && event.payload["allowed_scope"]
                    .as_array()
                    .is_some_and(|paths| paths.iter().any(|path| path == "src/lab"))
        }));
    }

    #[test]
    fn labrun_child_bash_scope_violation_is_denied_before_execution() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        std::fs::create_dir_all(project_root.join("src/lab")).unwrap();
        let isolated = project_root.join("isolated-worktree");
        std::fs::create_dir_all(&isolated).unwrap();
        std::fs::write(isolated.join("README.md"), "original\n").unwrap();

        let orchestrator = LabOrchestrator::for_project(project_root);
        let proposal = orchestrator
            .store()
            .create_proposal("Build LabRun", None)
            .unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = orchestrator
            .store()
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped LabRun slice",
                "Only edit src/lab.",
                vec!["src/lab".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        let binding = LabExecutionBinding::for_graduate_task(
            &task,
            "dispatch_bash_scope_test",
            "agent_task_bash_scope_test",
            project_root,
            &isolated,
            Some("state_v1".to_string()),
        )
        .unwrap();
        let mut metadata = HashMap::new();
        binding.insert_into_metadata(&mut metadata).unwrap();
        let labrun_context =
            crate::lab::policy_overlay::LabRunExecutionContext::from_metadata(&metadata);

        let mut registry = ToolRegistry::new();
        registry.register(BashTool);
        let resource_policy = test_resource_policy();
        let retained_context = ToolContextRetainedContext::default();
        let runtime_context = test_runtime_context(&resource_policy, &retained_context);
        let exposed_tool_names = HashSet::from(["bash".to_string()]);
        let allowed_tools = None;
        let destructive_scope =
            crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
                "make a scoped LabRun edit",
                &isolated,
            );
        let mut permission_context = PermissionContext::new(&isolated);
        permission_context.mode = PermissionMode::AutoAll;
        let trace = None;
        let gate = ToolExecutionGate {
            tool_registry: &registry,
            active_goal: None,
            task_state: None,
            allowed_tools: &allowed_tools,
            resource_policy: &resource_policy,
            exposed_tool_names: &exposed_tool_names,
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            has_changes_before_tools: false,
            allow_validation_without_changes: false,
            destructive_scope: &destructive_scope,
            working_dir: &isolated,
            labrun_context,
            trace: &trace,
            runtime_context: &runtime_context,
            permission_context: &permission_context,
        };
        let tool_call = ToolCall {
            id: "call_bash".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "printf hacked > README.md",
            }),
        };

        let result = match gate.evaluate(&tool_call, 0) {
            ToolExecutionGateOutcome::Deny(result) => result,
            ToolExecutionGateOutcome::Allow(_) => {
                panic!("out-of-scope LabRun child bash mutation should be denied")
            }
        };

        assert!(!result.success);
        assert!(result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("labrun_policy_violation"));
        assert_eq!(
            std::fs::read_to_string(isolated.join("README.md")).unwrap(),
            "original\n"
        );
        let events = orchestrator
            .store()
            .list_run_events(&run.lab_run_id)
            .unwrap();
        assert!(events.iter().any(|event| {
            event.event_type == "labrun_policy_blocked"
                && event.payload["active_graduate_task_id"] == task.task_id
                && event.payload["active_dispatch_id"] == "dispatch_bash_scope_test"
                && event.payload["action_family"] == "shell_mutation"
        }));
    }
}
