//! Deterministic behavior evalsets for routing and trace contracts.
//!
//! This runner intentionally starts with non-LLM assertions so it can run in CI
//! and guard agent behavior while deeper replay support is added.

use crate::engine::intent_router::{
    IntentKind, IntentRoute, IntentRouter, ReasoningPolicy, RetrievalPolicy, RiskLevel,
    WorkflowKind,
};
use crate::engine::trace::{TraceEvent, TurnTrace};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSet {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scenarios: Vec<EvalScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalScenario {
    pub id: String,
    pub prompt: String,
    #[serde(default)]
    pub replay: EvalReplay,
    #[serde(default)]
    pub expect: EvalExpect,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalExpect {
    pub intent: Option<IntentKind>,
    pub workflow: Option<WorkflowKind>,
    pub retrieval: Option<RetrievalPolicy>,
    pub reasoning: Option<ReasoningPolicy>,
    pub risk: Option<RiskLevel>,
    pub min_confidence: Option<f32>,
    #[serde(default)]
    pub recommended_tools: Vec<String>,
    #[serde(default)]
    pub forbidden_tools: Vec<String>,
    #[serde(default)]
    pub trace_events: Vec<String>,
    #[serde(default)]
    pub tool_sequence: Vec<String>,
    pub failed_tool: Option<String>,
    pub verification_passed: Option<bool>,
    pub reflection_status: Option<String>,
    pub repair_required: Option<bool>,
    pub permission_approved: Option<bool>,
    pub permission_decision: Option<String>,
    pub permission_persistence_scope: Option<String>,
    pub recovery_category: Option<String>,
    pub recovery_suggested_command: Option<String>,
    pub recovery_safe_retry: Option<bool>,
    pub terminal_task_count: Option<usize>,
    pub terminal_task_id: Option<String>,
    pub terminal_task_status: Option<String>,
    pub terminal_task_read_tool: Option<String>,
    pub terminal_task_cancel_tool: Option<String>,
    pub backgrounded_tool: Option<String>,
    pub file_checkpoint_count: Option<usize>,
    pub file_checkpoint_id: Option<String>,
    pub file_change_id: Option<String>,
    pub file_checkpoint_path: Option<String>,
    pub rewind_target: Option<String>,
    pub rewind_command: Option<String>,
    pub rewind_checkpoint_id: Option<String>,
    pub rewind_restored_files: Option<usize>,
    pub context_compaction_count: Option<usize>,
    pub context_boundary_id: Option<String>,
    pub context_compaction_strategy: Option<String>,
    pub context_before_tokens: Option<usize>,
    pub context_after_tokens: Option<usize>,
    pub context_preserved_tail_count: Option<usize>,
    pub runtime_diet_total_request_tokens: Option<u64>,
    pub runtime_diet_remaining_context_tokens: Option<u64>,
    pub runtime_diet_route_scoped_tools: Option<bool>,
    pub runtime_diet_workflow_context: Option<String>,
    pub subagent_count: Option<usize>,
    pub subagent_agent_id: Option<String>,
    pub subagent_profile: Option<String>,
    pub subagent_role: Option<String>,
    pub subagent_status: Option<String>,
    pub subagent_context_mode: Option<String>,
    pub subagent_allowed_tools: Option<usize>,
    pub isolated_worktree_path: Option<String>,
    pub isolated_worktree_branch: Option<String>,
    pub recursive_fork_guard: Option<bool>,
    pub fork_placeholder_complete: Option<bool>,
    pub fork_message_count: Option<usize>,
    pub agent_worktree_action_count: Option<usize>,
    pub agent_worktree_review_command: Option<String>,
    pub agent_worktree_merge_command: Option<String>,
    pub agent_worktree_cleanup_command: Option<String>,
    pub agent_worktree_review_status: Option<String>,
    pub agent_worktree_merge_status: Option<String>,
    pub agent_worktree_cleanup_status: Option<String>,
    pub agent_worktree_merge_kind: Option<String>,
    pub agent_worktree_cleanup_deleted_branch: Option<bool>,
    pub mcp_resource_count: Option<usize>,
    pub mcp_resource_success_count: Option<usize>,
    pub mcp_resource_failure_count: Option<usize>,
    pub mcp_resource_server: Option<String>,
    pub mcp_resource_uri: Option<String>,
    pub mcp_resource_action: Option<String>,
    pub mcp_resource_success: Option<bool>,
    pub mcp_resource_content_chars: Option<usize>,
    pub mcp_repair_count: Option<usize>,
    pub mcp_repair_server: Option<String>,
    pub mcp_repair_category: Option<String>,
    pub mcp_repair_command: Option<String>,
    pub mcp_repair_status: Option<String>,
    pub mcp_panel_command: Option<String>,
    #[serde(default)]
    pub available_tools: Vec<String>,
    #[serde(default)]
    pub unavailable_tools: Vec<String>,
    #[serde(default)]
    pub available_commands: Vec<String>,
    #[serde(default)]
    pub placeholder_commands: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub agent_profiles: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalReplay {
    #[serde(default)]
    pub tool_calls: Vec<EvalToolCall>,
    #[serde(default)]
    pub workflow_judgment: bool,
    pub acceptance_review_accepted: Option<bool>,
    #[serde(default)]
    pub guided_debugging: bool,
    pub verification_passed: Option<bool>,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub failed_commands: Vec<String>,
    #[serde(default)]
    pub recovery_plans: Vec<EvalRecoveryPlan>,
    #[serde(default)]
    pub terminal_tasks: Vec<EvalTerminalTaskReplay>,
    #[serde(default)]
    pub file_changes: Vec<EvalFileChangeReplay>,
    #[serde(default)]
    pub rewind: Option<EvalRewindReplay>,
    #[serde(default)]
    pub context_compactions: Vec<EvalContextCompactionReplay>,
    #[serde(default)]
    pub runtime_diet: Option<EvalRuntimeDietReplay>,
    #[serde(default)]
    pub subagents: Vec<EvalSubagentReplay>,
    #[serde(default)]
    pub agent_worktree_actions: Vec<EvalAgentWorktreeActionReplay>,
    #[serde(default)]
    pub mcp_resources: Vec<EvalMcpResourceReplay>,
    #[serde(default)]
    pub mcp_repairs: Vec<EvalMcpRepairReplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalToolCall {
    pub tool: String,
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub permission: Option<EvalPermissionReplay>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalPermissionReplay {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub persistence_scope: Option<String>,
    #[serde(default)]
    pub rule_pattern: Option<String>,
    #[serde(default)]
    pub persisted_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRecoveryPlan {
    pub source: String,
    pub category: String,
    pub action: String,
    #[serde(default = "default_true")]
    pub retryable: bool,
    #[serde(default)]
    pub safe_retry: bool,
    #[serde(default)]
    pub suggested_command: Option<String>,
    #[serde(default = "default_recovery_status")]
    pub status: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalTerminalTaskReplay {
    pub id: String,
    #[serde(default)]
    pub source_tool: String,
    #[serde(default = "default_running_status")]
    pub status: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub handle: Option<String>,
    #[serde(default)]
    pub read_tool: Option<String>,
    #[serde(default)]
    pub cancel_tool: Option<String>,
    #[serde(default)]
    pub cancel_handle: Option<String>,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub backgrounded: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalFileChangeReplay {
    pub id: String,
    pub checkpoint_id: String,
    pub path: String,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub existed_before: bool,
    #[serde(default)]
    pub before_hash: Option<String>,
    #[serde(default)]
    pub after_hash: Option<String>,
    #[serde(default)]
    pub diff: Option<String>,
    #[serde(default)]
    pub bytes_written: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalRewindReplay {
    pub target: String,
    pub checkpoint_id: String,
    #[serde(default = "default_rewind_command")]
    pub command: String,
    #[serde(default)]
    pub restored_files: Vec<String>,
    #[serde(default)]
    pub removed_files: Vec<String>,
    #[serde(default)]
    pub failed_files: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvalContextCompactionReplay {
    pub before_tokens: usize,
    pub after_tokens: usize,
    pub strategy: String,
    #[serde(default)]
    pub boundary_id: Option<String>,
    #[serde(default)]
    pub sequence: Option<u32>,
    #[serde(default)]
    pub messages_before: Option<usize>,
    #[serde(default)]
    pub messages_after: Option<usize>,
    #[serde(default)]
    pub preserved_tail_count: Option<usize>,
    #[serde(default)]
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRuntimeDietReplay {
    pub prompt_tokens: u64,
    pub tool_schema_tokens: u64,
    #[serde(default)]
    pub total_request_tokens: u64,
    #[serde(default)]
    pub max_context_tokens: Option<u64>,
    #[serde(default)]
    pub remaining_context_tokens: Option<u64>,
    #[serde(default)]
    pub tool_result_chars: usize,
    #[serde(default)]
    pub tool_result_tokens: u64,
    #[serde(default)]
    pub truncated_tool_results: usize,
    #[serde(default)]
    pub tool_result_artifacts: usize,
    pub exposed_tools: usize,
    #[serde(default)]
    pub memory_snapshot_chars: usize,
    #[serde(default)]
    pub memory_snapshot_tokens: u64,
    #[serde(default)]
    pub retrieval_items: usize,
    #[serde(default)]
    pub retrieval_tokens: u64,
    #[serde(default)]
    pub skill_list_chars: usize,
    #[serde(default)]
    pub skill_list_tokens: u64,
    #[serde(default = "default_true")]
    pub route_scoped_tools: bool,
    #[serde(default = "default_workflow_context")]
    pub workflow_context: String,
    #[serde(default = "default_closeout_visibility")]
    pub closeout_visibility: String,
    #[serde(default = "default_validation_evidence")]
    pub validation_evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSubagentReplay {
    pub agent_id: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default = "default_agent_role")]
    pub role: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_agent_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub allowed_tools: usize,
    #[serde(default = "default_agent_status")]
    pub status: String,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub output_chars: usize,
    #[serde(default)]
    pub tools_used: usize,
    #[serde(default)]
    pub context_mode: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub worktree_branch: Option<String>,
    #[serde(default)]
    pub recursive_fork_guard: bool,
    #[serde(default)]
    pub placeholder_complete: bool,
    #[serde(default)]
    pub fork_message_count: Option<usize>,
    #[serde(default)]
    pub parent_tool_call_ids: Vec<String>,
    #[serde(default)]
    pub cleanup_hooks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalAgentWorktreeActionReplay {
    pub action: String,
    pub agent_id: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default = "default_action_status")]
    pub status: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub commits_ahead: Option<usize>,
    #[serde(default)]
    pub merge_kind: Option<String>,
    #[serde(default)]
    pub cleanup: bool,
    #[serde(default)]
    pub delete_branch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalMcpResourceReplay {
    pub server: String,
    #[serde(default = "default_mcp_uri")]
    pub uri: String,
    #[serde(default = "default_mcp_resource_action")]
    pub action: String,
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub content_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalMcpRepairReplay {
    pub server: String,
    pub category: String,
    pub command: String,
    #[serde(default = "default_mcp_panel_command")]
    pub panel_command: String,
    #[serde(default = "default_recovery_status")]
    pub status: String,
    #[serde(default)]
    pub safe_retry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub set_name: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<EvalFailure>,
}

impl EvalReport {
    pub fn ok(&self) -> bool {
        self.failed == 0
    }

    pub fn summary(&self) -> String {
        let mut out = format!(
            "EvalSet {}: {}/{} passed",
            self.set_name, self.passed, self.total
        );
        if !self.failures.is_empty() {
            out.push_str("\nFailures:");
            for failure in &self.failures {
                out.push_str(&format!("\n- {}: {}", failure.scenario_id, failure.message));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalFailure {
    pub scenario_id: String,
    pub message: String,
}

pub struct EvalRunner {
    router: IntentRouter,
}

impl Default for EvalRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl EvalRunner {
    pub fn new() -> Self {
        Self {
            router: IntentRouter::new(),
        }
    }

    pub fn run_set(&self, set: &EvalSet) -> EvalReport {
        let mut failures = Vec::new();
        for scenario in &set.scenarios {
            let route = self.router.route(&scenario.prompt);
            let trace = trace_from_route("eval", scenario, &route);
            failures.extend(self.check_scenario(scenario, &route, &trace));
        }
        let total = set.scenarios.len();
        let failed = failures.len();
        EvalReport {
            set_name: set.name.clone(),
            total,
            passed: total.saturating_sub(failed),
            failed,
            failures,
        }
    }

    fn check_scenario(
        &self,
        scenario: &EvalScenario,
        route: &IntentRoute,
        trace: &TurnTrace,
    ) -> Vec<EvalFailure> {
        let mut failures = Vec::new();
        let expect = &scenario.expect;

        check_eq(
            &mut failures,
            &scenario.id,
            "intent",
            expect.intent,
            route.intent,
        );
        check_eq(
            &mut failures,
            &scenario.id,
            "workflow",
            expect.workflow,
            route.workflow,
        );
        check_eq(
            &mut failures,
            &scenario.id,
            "retrieval",
            expect.retrieval,
            route.retrieval,
        );
        check_eq(
            &mut failures,
            &scenario.id,
            "reasoning",
            expect.reasoning,
            route.reasoning,
        );
        check_eq(&mut failures, &scenario.id, "risk", expect.risk, route.risk);

        if let Some(min) = expect.min_confidence {
            if route.confidence < min {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!(
                        "confidence expected >= {:.2}, got {:.2}",
                        min, route.confidence
                    ),
                });
            }
        }

        for tool in &expect.recommended_tools {
            if !route.recommended_tools.contains(tool) {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("expected recommended tool '{}'", tool),
                });
            }
        }

        for tool in &expect.forbidden_tools {
            if route.recommended_tools.contains(tool) {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("forbidden tool '{}' was recommended", tool),
                });
            }
        }

        if !expect.trace_events.is_empty() {
            let labels = trace
                .events
                .iter()
                .map(TraceEvent::label)
                .collect::<Vec<_>>();
            for expected in &expect.trace_events {
                if !labels.contains(&expected.as_str()) {
                    failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!("expected trace event '{}'", expected),
                    });
                }
            }
        }

        if !expect.tool_sequence.is_empty() {
            let actual = trace_tool_sequence(trace);
            if actual != expect.tool_sequence {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!(
                        "tool_sequence expected {:?}, got {:?}",
                        expect.tool_sequence, actual
                    ),
                });
            }
        }

        if let Some(expected_tool) = &expect.failed_tool {
            if !trace_has_failed_tool(trace, expected_tool) {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("expected failed tool '{}'", expected_tool),
                });
            }
        }

        if let Some(expected) = expect.verification_passed {
            if trace_verification_status(trace) != Some(expected) {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("verification_passed expected {}", expected),
                });
            }
        }

        if let Some(expected) = &expect.reflection_status {
            if trace_last_reflection_status(trace).as_deref() != Some(expected.as_str()) {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("reflection_status expected {}", expected),
                });
            }
        }

        if let Some(expected) = expect.repair_required {
            if trace_repair_required(trace) != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("repair_required expected {}", expected),
                });
            }
        }

        if let Some(expected) = expect.permission_approved {
            if trace_last_permission_approved(trace) != Some(expected) {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("permission_approved expected {}", expected),
                });
            }
        }

        if let Some(expected) = &expect.permission_decision {
            if trace_last_permission_decision(trace).as_deref() != Some(expected.as_str()) {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("permission_decision expected {}", expected),
                });
            }
        }

        if let Some(expected) = &expect.permission_persistence_scope {
            if trace_last_permission_persistence_scope(trace).as_deref() != Some(expected.as_str())
            {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("permission_persistence_scope expected {}", expected),
                });
            }
        }

        if (expect.recovery_category.is_some()
            || expect.recovery_suggested_command.is_some()
            || expect.recovery_safe_retry.is_some())
            && !trace_has_matching_recovery_plan(
                trace,
                expect.recovery_category.as_deref(),
                expect.recovery_suggested_command.as_deref(),
                expect.recovery_safe_retry,
            )
        {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching recovery plan category={:?} suggested_command={:?} safe_retry={:?}",
                    expect.recovery_category,
                    expect.recovery_suggested_command,
                    expect.recovery_safe_retry
                ),
            });
        }

        self.check_terminal_task_replay(scenario, &mut failures);
        self.check_file_rewind_replay(scenario, &mut failures);
        self.check_context_compaction_replay(trace, scenario, &mut failures);
        self.check_subagent_worktree_replay(scenario, &mut failures);
        self.check_mcp_auth_repair_replay(trace, scenario, &mut failures);
        self.check_feature_reality(scenario, &mut failures);

        failures
    }

    fn check_terminal_task_replay(&self, scenario: &EvalScenario, failures: &mut Vec<EvalFailure>) {
        let expect = &scenario.expect;
        if let Some(expected) = expect.terminal_task_count {
            let actual = scenario.replay.terminal_tasks.len();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("terminal_task_count expected {}, got {}", expected, actual),
                });
            }
        }

        if has_terminal_task_expectation(expect)
            && !replay_has_matching_terminal_task(&scenario.replay, expect)
        {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching terminal task id={:?} status={:?} read_tool={:?} cancel_tool={:?} backgrounded_tool={:?}",
                    expect.terminal_task_id,
                    expect.terminal_task_status,
                    expect.terminal_task_read_tool,
                    expect.terminal_task_cancel_tool,
                    expect.backgrounded_tool
                ),
            });
        }
    }

    fn check_file_rewind_replay(&self, scenario: &EvalScenario, failures: &mut Vec<EvalFailure>) {
        let expect = &scenario.expect;
        if let Some(expected) = expect.file_checkpoint_count {
            let actual = scenario.replay.file_changes.len();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!(
                        "file_checkpoint_count expected {}, got {}",
                        expected, actual
                    ),
                });
            }
        }

        if has_file_checkpoint_expectation(expect)
            && !replay_has_matching_file_change(&scenario.replay, expect)
        {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching file checkpoint change_id={:?} checkpoint={:?} path={:?}",
                    expect.file_change_id, expect.file_checkpoint_id, expect.file_checkpoint_path
                ),
            });
        }

        if has_rewind_expectation(expect) && !replay_has_matching_rewind(&scenario.replay, expect) {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching rewind target={:?} command={:?} checkpoint={:?} restored_files={:?}",
                    expect.rewind_target,
                    expect.rewind_command,
                    expect.rewind_checkpoint_id,
                    expect.rewind_restored_files
                ),
            });
        }
    }

    fn check_context_compaction_replay(
        &self,
        trace: &TurnTrace,
        scenario: &EvalScenario,
        failures: &mut Vec<EvalFailure>,
    ) {
        let expect = &scenario.expect;
        if let Some(expected) = expect.context_compaction_count {
            let actual = trace
                .events
                .iter()
                .filter(|event| matches!(event, TraceEvent::ContextCompacted { .. }))
                .count();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!(
                        "context_compaction_count expected {}, got {}",
                        expected, actual
                    ),
                });
            }
        }

        if has_context_compaction_expectation(expect)
            && !trace_has_matching_context_compaction(trace, expect)
        {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching context compaction boundary={:?} strategy={:?} before={:?} after={:?} preserved_tail={:?}",
                    expect.context_boundary_id,
                    expect.context_compaction_strategy,
                    expect.context_before_tokens,
                    expect.context_after_tokens,
                    expect.context_preserved_tail_count
                ),
            });
        }

        if has_runtime_diet_expectation(expect) && !trace_has_matching_runtime_diet(trace, expect) {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching runtime diet total={:?} remaining={:?} route_scoped={:?} workflow={:?}",
                    expect.runtime_diet_total_request_tokens,
                    expect.runtime_diet_remaining_context_tokens,
                    expect.runtime_diet_route_scoped_tools,
                    expect.runtime_diet_workflow_context
                ),
            });
        }
    }

    fn check_subagent_worktree_replay(
        &self,
        scenario: &EvalScenario,
        failures: &mut Vec<EvalFailure>,
    ) {
        let expect = &scenario.expect;
        if let Some(expected) = expect.subagent_count {
            let actual = scenario.replay.subagents.len();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("subagent_count expected {}, got {}", expected, actual),
                });
            }
        }

        if has_subagent_expectation(expect)
            && !replay_has_matching_subagent(&scenario.replay, expect)
        {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching subagent id={:?} profile={:?} role={:?} status={:?} context={:?} worktree={:?} branch={:?} fork_guard={:?}",
                    expect.subagent_agent_id,
                    expect.subagent_profile,
                    expect.subagent_role,
                    expect.subagent_status,
                    expect.subagent_context_mode,
                    expect.isolated_worktree_path,
                    expect.isolated_worktree_branch,
                    expect.recursive_fork_guard
                ),
            });
        }

        if let Some(expected) = expect.agent_worktree_action_count {
            let actual = scenario.replay.agent_worktree_actions.len();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!(
                        "agent_worktree_action_count expected {}, got {}",
                        expected, actual
                    ),
                });
            }
        }

        self.check_agent_worktree_action(
            scenario,
            failures,
            "agent_review",
            expect.agent_worktree_review_command.as_deref(),
            expect.agent_worktree_review_status.as_deref(),
        );
        self.check_agent_worktree_action(
            scenario,
            failures,
            "agent_merge",
            expect.agent_worktree_merge_command.as_deref(),
            expect.agent_worktree_merge_status.as_deref(),
        );
        self.check_agent_worktree_action(
            scenario,
            failures,
            "agent_cleanup",
            expect.agent_worktree_cleanup_command.as_deref(),
            expect.agent_worktree_cleanup_status.as_deref(),
        );

        if (expect.agent_worktree_merge_kind.is_some()
            || expect.agent_worktree_cleanup_deleted_branch.is_some())
            && !replay_has_matching_agent_worktree_metadata(&scenario.replay, expect)
        {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching agent worktree metadata merge_kind={:?} cleanup_deleted_branch={:?}",
                    expect.agent_worktree_merge_kind,
                    expect.agent_worktree_cleanup_deleted_branch
                ),
            });
        }
    }

    fn check_agent_worktree_action(
        &self,
        scenario: &EvalScenario,
        failures: &mut Vec<EvalFailure>,
        action: &str,
        expected_command: Option<&str>,
        expected_status: Option<&str>,
    ) {
        if expected_command.is_none() && expected_status.is_none() {
            return;
        }
        if !replay_has_agent_worktree_action(
            &scenario.replay,
            action,
            expected_command,
            expected_status,
        ) {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected agent worktree action {} command={:?} status={:?}",
                    action, expected_command, expected_status
                ),
            });
        }
    }

    fn check_mcp_auth_repair_replay(
        &self,
        trace: &TurnTrace,
        scenario: &EvalScenario,
        failures: &mut Vec<EvalFailure>,
    ) {
        let expect = &scenario.expect;
        if let Some(expected) = expect.mcp_resource_count {
            let actual = trace
                .events
                .iter()
                .filter(|event| matches!(event, TraceEvent::McpResourceAccessed { .. }))
                .count();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("mcp_resource_count expected {}, got {}", expected, actual),
                });
            }
        }

        if let Some(expected) = expect.mcp_resource_success_count {
            let actual = trace
                .events
                .iter()
                .filter(|event| {
                    matches!(event, TraceEvent::McpResourceAccessed { success: true, .. })
                })
                .count();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!(
                        "mcp_resource_success_count expected {}, got {}",
                        expected, actual
                    ),
                });
            }
        }

        if let Some(expected) = expect.mcp_resource_failure_count {
            let actual = trace
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        TraceEvent::McpResourceAccessed { success: false, .. }
                    )
                })
                .count();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!(
                        "mcp_resource_failure_count expected {}, got {}",
                        expected, actual
                    ),
                });
            }
        }

        if has_mcp_resource_expectation(expect) && !trace_has_matching_mcp_resource(trace, expect) {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching mcp resource server={:?} uri={:?} action={:?} success={:?} chars={:?}",
                    expect.mcp_resource_server,
                    expect.mcp_resource_uri,
                    expect.mcp_resource_action,
                    expect.mcp_resource_success,
                    expect.mcp_resource_content_chars
                ),
            });
        }

        if let Some(expected) = expect.mcp_repair_count {
            let actual = scenario.replay.mcp_repairs.len();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!("mcp_repair_count expected {}, got {}", expected, actual),
                });
            }
        }

        if has_mcp_repair_expectation(expect)
            && !replay_has_matching_mcp_repair(&scenario.replay, expect)
        {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching mcp repair server={:?} category={:?} command={:?} status={:?} panel={:?}",
                    expect.mcp_repair_server,
                    expect.mcp_repair_category,
                    expect.mcp_repair_command,
                    expect.mcp_repair_status,
                    expect.mcp_panel_command
                ),
            });
        }
    }

    fn check_feature_reality(&self, scenario: &EvalScenario, failures: &mut Vec<EvalFailure>) {
        let expect = &scenario.expect;
        if !expect.available_tools.is_empty() || !expect.unavailable_tools.is_empty() {
            let registry = crate::tools::ToolRegistry::default_registry();
            let context = crate::tools::ToolContext::new(".", "eval");

            for tool_name in &expect.available_tools {
                match registry.get(tool_name) {
                    Some(tool) if tool.is_available(&context) => {}
                    Some(tool) => failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!(
                            "tool '{}' expected available but was unavailable: {}",
                            tool_name,
                            tool.unavailable_reason(&context)
                                .unwrap_or_else(|| "unavailable".to_string())
                        ),
                    }),
                    None => failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!(
                            "tool '{}' expected available but is not registered",
                            tool_name
                        ),
                    }),
                }
            }

            for tool_name in &expect.unavailable_tools {
                match registry.get(tool_name) {
                    Some(tool) if !tool.is_available(&context) => {}
                    Some(_) => failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!(
                            "tool '{}' expected unavailable but was available",
                            tool_name
                        ),
                    }),
                    None => failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!(
                            "tool '{}' expected unavailable but is not registered",
                            tool_name
                        ),
                    }),
                }
            }
        }

        if !expect.available_commands.is_empty() || !expect.placeholder_commands.is_empty() {
            let registry = crate::tui::commands::default_command_registry();

            for command in &expect.available_commands {
                match registry.get(command) {
                    Some(cmd) if !cmd.placeholder => {}
                    Some(_) => failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!(
                            "command '{}' expected production-ready but is placeholder",
                            command
                        ),
                    }),
                    None => failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!("command '{}' expected registered", command),
                    }),
                }
            }

            for command in &expect.placeholder_commands {
                match registry.get(command) {
                    Some(cmd) if cmd.placeholder => {}
                    Some(_) => failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!(
                            "command '{}' expected placeholder but was not marked",
                            command
                        ),
                    }),
                    None => failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!("command '{}' expected registered placeholder", command),
                    }),
                }
            }
        }

        if !expect.skills.is_empty() {
            let runtime = crate::skills::SkillRuntime::load(".");
            for skill in &expect.skills {
                if runtime.get(skill).is_none() {
                    failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!("skill '{}' expected loadable", skill),
                    });
                }
            }
        }

        if !expect.agent_profiles.is_empty() {
            let profiles = crate::agent::profiles::load_profiles(".");
            for profile_name in &expect.agent_profiles {
                if !profiles
                    .iter()
                    .any(|profile| profile.name.eq_ignore_ascii_case(profile_name))
                {
                    failures.push(EvalFailure {
                        scenario_id: scenario.id.clone(),
                        message: format!("agent profile '{}' expected loadable", profile_name),
                    });
                }
            }
        }
    }
}

pub fn load_evalset(path: impl AsRef<Path>) -> Result<EvalSet> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read evalset {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("failed to parse evalset {}", path.display()))
}

pub fn load_evalsets_from_dir(dir: impl AsRef<Path>) -> Result<Vec<(PathBuf, EvalSet)>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut sets = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !is_evalset_file(&path) {
            continue;
        }
        sets.push((path.clone(), load_evalset(&path)?));
    }
    sets.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sets)
}

pub fn run_evalsets_from_dir(dir: impl AsRef<Path>, name: Option<&str>) -> Result<Vec<EvalReport>> {
    let sets = load_evalsets_from_dir(dir)?;
    let runner = EvalRunner::new();
    let mut reports = Vec::new();
    for (_, set) in sets {
        if name.is_none_or(|target| target == "all" || target == set.name) {
            reports.push(runner.run_set(&set));
        }
    }
    Ok(reports)
}

pub fn format_reports(reports: &[EvalReport]) -> String {
    if reports.is_empty() {
        return "No evalsets matched.".to_string();
    }
    let total = reports.iter().map(|r| r.total).sum::<usize>();
    let passed = reports.iter().map(|r| r.passed).sum::<usize>();
    let failed = reports.iter().map(|r| r.failed).sum::<usize>();
    let mut out = format!(
        "Eval Report\nSets: {}  Scenarios: {}  Passed: {}  Failed: {}",
        reports.len(),
        total,
        passed,
        failed
    );
    for report in reports {
        out.push_str("\n\n");
        out.push_str(&report.summary());
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReportBundle {
    pub generated_at: String,
    pub sets: usize,
    pub scenarios: usize,
    pub passed: usize,
    pub failed: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<EvalBaselineSummary>,
    pub reports: Vec<EvalReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalBaselineSummary {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    pub scenarios: usize,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalExternalBaselineSet {
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub scenarios: Vec<EvalExternalBaselineScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalExternalBaselineScenario {
    pub id: String,
    pub outcome: EvalExternalBaselineOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_turns: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_passed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_evidence_backed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalExternalBaselineOutcome {
    Pass,
    Fail,
    Blocked,
    NotRun,
}

impl EvalExternalBaselineOutcome {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Blocked => "blocked",
            Self::NotRun => "not_run",
        }
    }
}

impl EvalReportBundle {
    pub fn from_reports(reports: &[EvalReport]) -> Self {
        Self {
            generated_at: chrono::Utc::now().to_rfc3339(),
            sets: reports.len(),
            scenarios: reports.iter().map(|r| r.total).sum(),
            passed: reports.iter().map(|r| r.passed).sum(),
            failed: reports.iter().map(|r| r.failed).sum(),
            baseline: None,
            reports: reports.to_vec(),
        }
    }
}

pub fn format_reports_json(reports: &[EvalReport]) -> Result<String> {
    serde_json::to_string_pretty(&EvalReportBundle::from_reports(reports))
        .context("failed to serialize eval report bundle")
}

pub fn safe_eval_report_label(label: &str) -> String {
    let safe_label = label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if safe_label.is_empty() {
        "all".to_string()
    } else {
        safe_label
    }
}

pub fn write_reports_json(
    reports: &[EvalReport],
    dir: impl AsRef<Path>,
    label: &str,
) -> Result<PathBuf> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create eval report dir {}", dir.display()))?;
    let safe_label = safe_eval_report_label(label);
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let path = dir.join(format!("eval-{}-{}.json", timestamp, safe_label));
    let json = format_reports_json(reports)?;
    fs::write(&path, json)
        .with_context(|| format!("failed to write eval report {}", path.display()))?;
    Ok(path)
}

pub fn load_external_baseline(path: impl AsRef<Path>) -> Result<EvalExternalBaselineSet> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read external baseline {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("failed to parse external baseline {}", path.display()))
}

pub fn load_external_baseline_artifact(
    path: impl AsRef<Path>,
    provider: &str,
    model: Option<&str>,
) -> Result<EvalExternalBaselineSet> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read external baseline artifact {}",
            path.display()
        )
    })?;
    if is_evalset_file(path) {
        if let Ok(mut baseline) = serde_yaml::from_str::<EvalExternalBaselineSet>(&content) {
            if baseline.provider.trim().is_empty() {
                baseline.provider = normalized_external_provider(provider);
            }
            if baseline.model.is_none() {
                baseline.model = normalized_external_model(model);
            }
            if baseline.source.is_none() {
                baseline.source = Some(path.display().to_string());
            }
            return Ok(baseline);
        }
    }

    parse_external_baseline_markdown_artifact(&content, path, provider, model)
}

pub fn external_baseline_template(provider: &str, model: Option<&str>) -> EvalExternalBaselineSet {
    EvalExternalBaselineSet {
        provider: normalized_external_provider(provider),
        generated_at: Some(chrono::Utc::now().to_rfc3339()),
        model: normalized_external_model(model),
        source: Some("TODO: replace with run artifact path or manual baseline notes".to_string()),
        scenarios: crate::engine::scenario_matrix::deterministic_scenarios()
            .iter()
            .map(|scenario| EvalExternalBaselineScenario {
                id: scenario.id.to_string(),
                outcome: EvalExternalBaselineOutcome::NotRun,
                evidence: Some(
                    "TODO: record concrete diff, command, trace, or transcript evidence"
                        .to_string(),
                ),
                notes: Some(scenario.user_task.to_string()),
                tool_calls: None,
                repair_turns: None,
                validation_passed: None,
                final_evidence_backed: None,
            })
            .collect(),
    }
}

pub fn format_external_baseline_template(provider: &str, model: Option<&str>) -> Result<String> {
    serde_yaml::to_string(&external_baseline_template(provider, model))
        .context("failed to serialize external baseline template")
}

pub fn write_external_baseline_template(
    dir: impl AsRef<Path>,
    provider: &str,
    model: Option<&str>,
) -> Result<PathBuf> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create external baseline dir {}", dir.display()))?;
    let provider_for_path = if provider.trim().is_empty() {
        "external-agent"
    } else {
        provider
    };
    let safe_provider = safe_eval_report_label(provider_for_path);
    let path = dir.join(format!("baseline-{}.yaml", safe_provider));
    if path.exists() {
        anyhow::bail!(
            "external baseline template already exists at {}; refusing to overwrite",
            path.display()
        );
    }
    let yaml = format_external_baseline_template(provider, model)?;
    fs::write(&path, yaml).with_context(|| {
        format!(
            "failed to write external baseline template {}",
            path.display()
        )
    })?;
    Ok(path)
}

pub fn write_external_baseline_import(
    artifact: impl AsRef<Path>,
    dir: impl AsRef<Path>,
    provider: &str,
    model: Option<&str>,
) -> Result<PathBuf> {
    let artifact = artifact.as_ref();
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create external baseline dir {}", dir.display()))?;
    let baseline = load_external_baseline_artifact(artifact, provider, model)?;
    let safe_provider = safe_eval_report_label(&baseline.provider);
    let path = dir.join(format!("baseline-{}-import.yaml", safe_provider));
    if path.exists() {
        anyhow::bail!(
            "external baseline import already exists at {}; refusing to overwrite",
            path.display()
        );
    }
    let yaml = serde_yaml::to_string(&baseline).context("failed to serialize baseline import")?;
    fs::write(&path, yaml).with_context(|| {
        format!(
            "failed to write external baseline import {}",
            path.display()
        )
    })?;
    Ok(path)
}

pub fn load_external_baselines_from_dir(
    dir: impl AsRef<Path>,
) -> Result<Vec<(PathBuf, EvalExternalBaselineSet)>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut baselines = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !is_evalset_file(&path) {
            continue;
        }
        baselines.push((path.clone(), load_external_baseline(&path)?));
    }
    baselines.sort_by(|a, b| a.1.provider.cmp(&b.1.provider).then_with(|| a.0.cmp(&b.0)));
    Ok(baselines)
}

pub fn format_external_baseline_comparison(
    baselines: &[(PathBuf, EvalExternalBaselineSet)],
    provider_filter: Option<&str>,
) -> String {
    let expected = crate::engine::scenario_matrix::deterministic_scenarios()
        .iter()
        .map(|scenario| scenario.id)
        .collect::<Vec<_>>();
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    let filter = provider_filter.filter(|value| !value.eq_ignore_ascii_case("all"));
    let filtered = baselines
        .iter()
        .filter(|(_, baseline)| {
            filter.is_none_or(|target| baseline.provider.eq_ignore_ascii_case(target))
        })
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        return match filter {
            Some(provider) => format!(
                "External Baseline Comparison\nNo external baseline found for provider '{}'. Add YAML or JSON files under evalsets/external_baselines/.",
                provider
            ),
            None => "External Baseline Comparison\nNo external baselines found. Add YAML or JSON files under evalsets/external_baselines/.".to_string(),
        };
    }

    let mut lines = vec![
        "External Baseline Comparison".to_string(),
        format!(
            "Expected scenarios: {}  Providers: {}",
            expected.len(),
            filtered.len()
        ),
    ];

    for (path, baseline) in filtered {
        let records = baseline
            .scenarios
            .iter()
            .filter(|record| expected_set.contains(record.id.as_str()))
            .collect::<Vec<_>>();
        let recorded_ids = records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<BTreeSet<_>>();
        let missing = expected
            .iter()
            .copied()
            .filter(|id| !recorded_ids.contains(id))
            .collect::<Vec<_>>();
        let unknown = baseline
            .scenarios
            .iter()
            .filter(|record| !expected_set.contains(record.id.as_str()))
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>();
        let pass = records
            .iter()
            .filter(|record| record.outcome == EvalExternalBaselineOutcome::Pass)
            .count();
        let fail = records
            .iter()
            .filter(|record| record.outcome == EvalExternalBaselineOutcome::Fail)
            .count();
        let blocked = records
            .iter()
            .filter(|record| record.outcome == EvalExternalBaselineOutcome::Blocked)
            .count();
        let not_run = records
            .iter()
            .filter(|record| record.outcome == EvalExternalBaselineOutcome::NotRun)
            .count();
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");
        lines.push(format!(
            "\n{} [{}] file={}",
            baseline.provider,
            baseline.model.as_deref().unwrap_or("model unknown"),
            filename
        ));
        if let Some(generated_at) = &baseline.generated_at {
            lines.push(format!("  generated_at={}", generated_at));
        }
        if let Some(source) = &baseline.source {
            lines.push(format!("  source={}", source));
        }
        lines.push(format!(
            "  coverage={}/{} pass={} fail={} blocked={} not_run={}",
            records.len(),
            expected.len(),
            pass,
            fail,
            blocked,
            not_run
        ));
        if !missing.is_empty() {
            lines.push(format!("  missing: {}", missing.join(", ")));
        }
        if !unknown.is_empty() {
            lines.push(format!("  unknown: {}", unknown.join(", ")));
        }
        for id in &expected {
            if let Some(record) = records.iter().find(|record| record.id == *id) {
                let mut detail = format!("  - {}: {}", id, record.outcome.label());
                if let Some(validation) = record.validation_passed {
                    detail.push_str(&format!(" validation={}", validation));
                }
                if let Some(evidence_backed) = record.final_evidence_backed {
                    detail.push_str(&format!(" evidence_backed={}", evidence_backed));
                }
                if let Some(tool_calls) = record.tool_calls {
                    detail.push_str(&format!(" tool_calls={}", tool_calls));
                }
                if let Some(repair_turns) = record.repair_turns {
                    detail.push_str(&format!(" repair_turns={}", repair_turns));
                }
                if let Some(evidence) = &record.evidence {
                    detail.push_str(&format!(" evidence={}", evidence));
                }
                lines.push(detail);
            }
        }
    }

    lines.join("\n")
}

pub fn format_external_baseline_validation(
    baselines: &[(PathBuf, EvalExternalBaselineSet)],
    provider_filter: Option<&str>,
) -> String {
    let expected = crate::engine::scenario_matrix::deterministic_scenarios()
        .iter()
        .map(|scenario| scenario.id)
        .collect::<Vec<_>>();
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    let filter = provider_filter.filter(|value| !value.eq_ignore_ascii_case("all"));
    let filtered = baselines
        .iter()
        .filter(|(_, baseline)| {
            filter.is_none_or(|target| baseline.provider.eq_ignore_ascii_case(target))
        })
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        return match filter {
            Some(provider) => format!(
                "External Baseline Validation\nNo external baseline found for provider '{}'. Add YAML or JSON files under evalsets/external_baselines/.",
                provider
            ),
            None => "External Baseline Validation\nNo external baselines found. Add YAML or JSON files under evalsets/external_baselines/.".to_string(),
        };
    }

    let mut lines = vec![
        "External Baseline Validation".to_string(),
        format!(
            "Expected scenarios: {}  Providers: {}",
            expected.len(),
            filtered.len()
        ),
    ];

    for (path, baseline) in filtered {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");
        let records = baseline
            .scenarios
            .iter()
            .filter(|record| expected_set.contains(record.id.as_str()))
            .collect::<Vec<_>>();
        let id_counts =
            baseline
                .scenarios
                .iter()
                .fold(BTreeMap::<&str, usize>::new(), |mut counts, record| {
                    *counts.entry(record.id.as_str()).or_default() += 1;
                    counts
                });
        let recorded_ids = records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for id in &expected {
            if !recorded_ids.contains(id) {
                errors.push(format!("missing required scenario {}", id));
            }
        }
        for (id, count) in id_counts {
            if count > 1 {
                errors.push(format!("duplicate scenario {} appears {} times", id, count));
            }
            if !expected_set.contains(id) {
                warnings.push(format!("unknown scenario {} is ignored by comparison", id));
            }
        }
        for record in &records {
            if record.outcome == EvalExternalBaselineOutcome::NotRun {
                warnings.push(format!("{} is not_run", record.id));
            }
            if matches!(
                record.outcome,
                EvalExternalBaselineOutcome::Pass
                    | EvalExternalBaselineOutcome::Fail
                    | EvalExternalBaselineOutcome::Blocked
            ) && !has_meaningful_external_evidence(record.evidence.as_deref())
            {
                warnings.push(format!("{} is missing concrete evidence", record.id));
            }
            if record.outcome == EvalExternalBaselineOutcome::Pass {
                if record.validation_passed != Some(true) {
                    errors.push(format!("{} pass is missing validation=true", record.id));
                }
                if record.final_evidence_backed != Some(true) {
                    errors.push(format!(
                        "{} pass is missing final_evidence_backed=true",
                        record.id
                    ));
                }
            }
            if record.outcome == EvalExternalBaselineOutcome::Fail
                && record.validation_passed.is_none()
            {
                warnings.push(format!(
                    "{} fail should record validation_passed=false when applicable",
                    record.id
                ));
            }
        }

        lines.push(format!(
            "\n{} [{}] file={}",
            baseline.provider,
            baseline.model.as_deref().unwrap_or("model unknown"),
            filename
        ));
        lines.push(format!(
            "  status={} coverage={}/{} errors={} warnings={}",
            if errors.is_empty() {
                "valid"
            } else {
                "invalid"
            },
            records.len(),
            expected.len(),
            errors.len(),
            warnings.len()
        ));
        for error in errors {
            lines.push(format!("  error: {}", error));
        }
        for warning in warnings {
            lines.push(format!("  warn: {}", warning));
        }
    }

    lines.join("\n")
}

pub fn format_external_parity_report(
    baselines: &[(PathBuf, EvalExternalBaselineSet)],
    provider_filter: Option<&str>,
) -> String {
    let scenarios = crate::engine::scenario_matrix::deterministic_scenarios();
    let filter = provider_filter.filter(|value| !value.eq_ignore_ascii_case("all"));
    let providers = baselines
        .iter()
        .filter(|(_, baseline)| {
            filter.is_none_or(|target| baseline.provider.eq_ignore_ascii_case(target))
        })
        .collect::<Vec<_>>();

    if providers.is_empty() {
        return match filter {
            Some(provider) => format!(
                "Phase 12 Parity Report\nNo external baseline found for provider '{}'. Add YAML or JSON files under evalsets/external_baselines/.",
                provider
            ),
            None => "Phase 12 Parity Report\nNo external baselines found. Local replay fixtures are ready, but external Claude/Codex rows have not been imported yet.".to_string(),
        };
    }

    let local_ready = scenarios
        .iter()
        .filter(|scenario| {
            scenario.status == crate::engine::scenario_matrix::ReplayStatus::ReplayFixtureReady
        })
        .count();
    let mut provider_pass = BTreeMap::<&str, usize>::new();
    let mut provider_fail = BTreeMap::<&str, usize>::new();
    let mut provider_blocked = BTreeMap::<&str, usize>::new();
    let mut provider_not_run = BTreeMap::<&str, usize>::new();

    for (_, baseline) in &providers {
        for scenario in scenarios {
            let outcome = baseline
                .scenarios
                .iter()
                .find(|record| record.id == scenario.id)
                .map(|record| record.outcome)
                .unwrap_or(EvalExternalBaselineOutcome::NotRun);
            match outcome {
                EvalExternalBaselineOutcome::Pass => {
                    *provider_pass.entry(baseline.provider.as_str()).or_default() += 1;
                }
                EvalExternalBaselineOutcome::Fail => {
                    *provider_fail.entry(baseline.provider.as_str()).or_default() += 1;
                }
                EvalExternalBaselineOutcome::Blocked => {
                    *provider_blocked
                        .entry(baseline.provider.as_str())
                        .or_default() += 1;
                }
                EvalExternalBaselineOutcome::NotRun => {
                    *provider_not_run
                        .entry(baseline.provider.as_str())
                        .or_default() += 1;
                }
            }
        }
    }

    let mut lines = vec![
        "Phase 12 Parity Report".to_string(),
        format!(
            "Local replay-ready: {}/{}  External providers: {}",
            local_ready,
            scenarios.len(),
            providers.len()
        ),
    ];
    for (_, baseline) in &providers {
        lines.push(format!(
            "- {} [{}]: pass={} fail={} blocked={} not_run={}",
            baseline.provider,
            baseline.model.as_deref().unwrap_or("model unknown"),
            provider_pass
                .get(baseline.provider.as_str())
                .copied()
                .unwrap_or(0),
            provider_fail
                .get(baseline.provider.as_str())
                .copied()
                .unwrap_or(0),
            provider_blocked
                .get(baseline.provider.as_str())
                .copied()
                .unwrap_or(0),
            provider_not_run
                .get(baseline.provider.as_str())
                .copied()
                .unwrap_or(0)
        ));
    }

    for scenario in scenarios {
        lines.push(format!("\n{} [{}]", scenario.id, scenario.status.label()));
        lines.push(format!("  task: {}", scenario.user_task));
        for (_, baseline) in &providers {
            let record = baseline
                .scenarios
                .iter()
                .find(|record| record.id == scenario.id);
            let detail = match record {
                Some(record) => format_parity_provider_detail(&baseline.provider, record),
                None => format!("{}=missing gap=external_missing", baseline.provider),
            };
            lines.push(format!("  {}", detail));
        }
    }

    lines.join("\n")
}

pub fn write_external_parity_report(
    baselines: &[(PathBuf, EvalExternalBaselineSet)],
    provider_filter: Option<&str>,
    dir: impl AsRef<Path>,
) -> Result<PathBuf> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create parity report dir {}", dir.display()))?;
    let label = provider_filter
        .filter(|value| !value.eq_ignore_ascii_case("all"))
        .unwrap_or("all");
    let safe_label = safe_eval_report_label(label);
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let path = dir.join(format!("parity-{}-{}.txt", timestamp, safe_label));
    let report = format_external_parity_report(baselines, provider_filter);
    fs::write(&path, report)
        .with_context(|| format!("failed to write parity report {}", path.display()))?;
    Ok(path)
}

fn format_parity_provider_detail(provider: &str, record: &EvalExternalBaselineScenario) -> String {
    let gap = match record.outcome {
        EvalExternalBaselineOutcome::Pass
            if record.validation_passed == Some(true)
                && record.final_evidence_backed == Some(true)
                && has_meaningful_external_evidence(record.evidence.as_deref()) =>
        {
            "none"
        }
        EvalExternalBaselineOutcome::Pass => "evidence_incomplete",
        EvalExternalBaselineOutcome::Fail => "external_failed",
        EvalExternalBaselineOutcome::Blocked => "external_blocked",
        EvalExternalBaselineOutcome::NotRun => "external_not_run",
    };
    let mut detail = format!("{}={} gap={}", provider, record.outcome.label(), gap);
    if let Some(validation) = record.validation_passed {
        detail.push_str(&format!(" validation={}", validation));
    }
    if let Some(evidence_backed) = record.final_evidence_backed {
        detail.push_str(&format!(" evidence_backed={}", evidence_backed));
    }
    if let Some(tool_calls) = record.tool_calls {
        detail.push_str(&format!(" tool_calls={}", tool_calls));
    }
    if let Some(repair_turns) = record.repair_turns {
        detail.push_str(&format!(" repair_turns={}", repair_turns));
    }
    if let Some(evidence) = record
        .evidence
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        detail.push_str(&format!(" evidence={}", evidence));
    }
    detail
}

fn normalized_external_provider(provider: &str) -> String {
    let provider = provider.trim();
    if provider.is_empty() {
        "external-agent".to_string()
    } else {
        provider.to_string()
    }
}

fn normalized_external_model(model: Option<&str>) -> Option<String> {
    model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn parse_external_baseline_markdown_artifact(
    content: &str,
    path: &Path,
    provider: &str,
    model: Option<&str>,
) -> Result<EvalExternalBaselineSet> {
    let expected = crate::engine::scenario_matrix::deterministic_scenarios()
        .iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    let mut scenarios = Vec::new();
    let mut header: Option<Vec<String>> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }
        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect::<Vec<_>>();
        if cells.iter().all(|cell| {
            cell.chars()
                .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
        }) {
            continue;
        }
        if header.is_none() {
            header = Some(
                cells
                    .iter()
                    .map(|cell| normalize_table_header(cell))
                    .collect(),
            );
            continue;
        }
        let Some(headers) = &header else {
            continue;
        };
        let Some(id) = table_cell(headers, &cells, &["id", "scenario", "scenario_id"]) else {
            continue;
        };
        if !expected.contains(id) {
            continue;
        }
        let outcome = table_cell(headers, &cells, &["outcome", "status", "result"])
            .and_then(parse_external_baseline_outcome)
            .unwrap_or(EvalExternalBaselineOutcome::NotRun);
        scenarios.push(EvalExternalBaselineScenario {
            id: id.to_string(),
            outcome,
            evidence: table_cell(headers, &cells, &["evidence", "artifact", "proof"])
                .map(str::to_string),
            notes: table_cell(headers, &cells, &["notes", "note", "summary"]).map(str::to_string),
            tool_calls: table_cell(headers, &cells, &["tool_calls", "tools"])
                .and_then(|value| value.parse::<usize>().ok()),
            repair_turns: table_cell(headers, &cells, &["repair_turns", "repairs"])
                .and_then(|value| value.parse::<usize>().ok()),
            validation_passed: table_cell(headers, &cells, &["validation_passed", "validation"])
                .and_then(parse_bool_cell),
            final_evidence_backed: table_cell(
                headers,
                &cells,
                &["final_evidence_backed", "evidence_backed"],
            )
            .and_then(parse_bool_cell),
        });
    }

    if scenarios.is_empty() {
        anyhow::bail!(
            "no Phase 12 scenario rows found in {}; expected a markdown table with id/scenario and outcome/result columns",
            path.display()
        );
    }

    Ok(EvalExternalBaselineSet {
        provider: normalized_external_provider(provider),
        generated_at: Some(chrono::Utc::now().to_rfc3339()),
        model: normalized_external_model(model),
        source: Some(path.display().to_string()),
        scenarios,
    })
}

fn normalize_table_header(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn table_cell<'a>(headers: &[String], cells: &'a [String], names: &[&str]) -> Option<&'a str> {
    headers
        .iter()
        .position(|header| names.iter().any(|name| header == name))
        .and_then(|index| cells.get(index))
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "-")
}

fn parse_external_baseline_outcome(value: &str) -> Option<EvalExternalBaselineOutcome> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "pass" | "passed" | "ok" | "success" => Some(EvalExternalBaselineOutcome::Pass),
        "fail" | "failed" | "failure" => Some(EvalExternalBaselineOutcome::Fail),
        "blocked" | "block" => Some(EvalExternalBaselineOutcome::Blocked),
        "not_run" | "notrun" | "skip" | "skipped" | "todo" => {
            Some(EvalExternalBaselineOutcome::NotRun)
        }
        _ => None,
    }
}

fn parse_bool_cell(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "y" | "1" | "pass" | "passed" => Some(true),
        "false" | "no" | "n" | "0" | "fail" | "failed" => Some(false),
        _ => None,
    }
}

fn has_meaningful_external_evidence(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let lower = value.to_ascii_lowercase();
    !(lower.starts_with("todo") || lower == "-" || lower == "n/a")
}

pub fn load_eval_report_bundles(
    dir: impl AsRef<Path>,
    limit: usize,
) -> Result<Vec<(PathBuf, EvalReportBundle)>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut paths = fs::read_dir(dir)
        .with_context(|| format!("failed to read eval report dir {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("eval-"))
        })
        .collect::<Vec<_>>();
    paths.sort_by(|a, b| {
        a.file_name()
            .and_then(|name| name.to_str())
            .cmp(&b.file_name().and_then(|name| name.to_str()))
    });
    paths.reverse();

    let mut bundles = Vec::new();
    for path in paths.into_iter().take(limit.max(1)) {
        let json = fs::read_to_string(&path)
            .with_context(|| format!("failed to read eval report {}", path.display()))?;
        let bundle: EvalReportBundle = serde_json::from_str(&json)
            .with_context(|| format!("failed to parse eval report {}", path.display()))?;
        bundles.push((path, bundle));
    }
    Ok(bundles)
}

pub fn format_eval_trend(entries: &[(PathBuf, EvalReportBundle)]) -> String {
    if entries.is_empty() {
        return "No persisted eval reports found. Run /eval record <name|all> first.".to_string();
    }

    let latest = &entries[0];
    let latest_name = latest
        .0
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");
    let mut out = format!(
        "Eval Trend\nReports: {}\nLatest: {}  generated={}  scenarios={}  passed={}  failed={}",
        entries.len(),
        latest_name,
        latest.1.generated_at,
        latest.1.scenarios,
        latest.1.passed,
        latest.1.failed
    );

    if let Some(previous) = entries.get(1) {
        let pass_delta = latest.1.passed as isize - previous.1.passed as isize;
        let fail_delta = latest.1.failed as isize - previous.1.failed as isize;
        let scenario_delta = latest.1.scenarios as isize - previous.1.scenarios as isize;
        out.push_str(&format!(
            "\nDelta vs previous: scenarios={:+}  passed={:+}  failed={:+}",
            scenario_delta, pass_delta, fail_delta
        ));
    }

    if let Some(baseline) = &latest.1.baseline {
        let pass_delta = latest.1.passed as isize - baseline.passed as isize;
        let fail_delta = latest.1.failed as isize - baseline.failed as isize;
        let scenario_delta = latest.1.scenarios as isize - baseline.scenarios as isize;
        let generated = baseline.generated_at.as_deref().unwrap_or("unknown");
        out.push_str(&format!(
            "\nDelta vs baseline '{}': scenarios={:+}  passed={:+}  failed={:+}  baseline_generated={}",
            baseline.name, scenario_delta, pass_delta, fail_delta, generated
        ));
    }

    out.push_str("\n\nRecent reports:");
    for (path, bundle) in entries {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");
        out.push_str(&format!(
            "\n- {}  generated={}  sets={}  scenarios={}  passed={}  failed={}",
            name, bundle.generated_at, bundle.sets, bundle.scenarios, bundle.passed, bundle.failed
        ));
        if let Some(baseline) = &bundle.baseline {
            out.push_str(&format!("  baseline={}", baseline.name));
        }
    }
    out
}

fn check_eq<T>(
    failures: &mut Vec<EvalFailure>,
    scenario_id: &str,
    field: &str,
    expected: Option<T>,
    actual: T,
) where
    T: Copy + PartialEq + std::fmt::Debug,
{
    if let Some(expected) = expected {
        if expected != actual {
            failures.push(EvalFailure {
                scenario_id: scenario_id.to_string(),
                message: format!("{} expected {:?}, got {:?}", field, expected, actual),
            });
        }
    }
}

fn trace_from_route(session_id: &str, scenario: &EvalScenario, route: &IntentRoute) -> TurnTrace {
    let mut trace = TurnTrace::new(session_id, 1, &scenario.prompt);
    trace.events.push(TraceEvent::IntentRouted {
        agent_mode: Some("auto".to_string()),
        intent: format!("{:?}", route.intent),
        workflow: format!("{:?}", route.workflow),
        retrieval: format!("{:?}", route.retrieval),
        confidence: route.confidence,
        risk: format!("{:?}", route.risk),
        reason: route.reason.clone(),
    });
    let policy = crate::engine::resource_policy::ResourcePolicy::from_route(route);
    trace.events.push(TraceEvent::ResourcePolicySelected {
        latency: format!("{:?}", policy.latency),
        target_ms: policy.latency.target_ms(),
        cost_ceiling_usd: policy.cost_ceiling_usd,
        reasoning: format!("{:?}", policy.reasoning),
        parallelism_limit: policy.parallelism_limit,
        max_tool_calls: policy.max_tool_calls,
        context_budget_tokens: policy.context_budget_tokens,
        allow_fallback_model: policy.allow_fallback_model,
        reason: policy.reason,
    });
    if scenario.replay.workflow_judgment {
        trace.events.push(TraceEvent::WorkflowJudgmentCompleted {
            task_type: format!("{:?}", route.workflow),
            complexity: format!("{:?}", route.reasoning),
            risk: format!("{:?}", route.risk),
            plan_steps: 2,
            acceptance_checks: 2,
            questions: 0,
            guided_reasoning: matches!(route.reasoning, ReasoningPolicy::High),
        });
        trace.events.push(TraceEvent::WorkflowPlanProgress {
            total_steps: 2,
            completed_steps: 0,
            active_step: Some("Inspect relevant code and define acceptance checks".to_string()),
            top_priority: Some("P0".to_string()),
            top_importance_score: Some(0.90),
            top_weight_share: Some(0.55),
            weight_source: Some("Factors".to_string()),
            reweighted: false,
        });
    }
    let mut task_bundle = crate::engine::task_context::TaskContextBundle::new(
        &scenario.prompt,
        ".",
        route.clone(),
        None,
    );
    if matches!(
        route.workflow,
        crate::engine::intent_router::WorkflowKind::CodeChange
            | crate::engine::intent_router::WorkflowKind::BugFix
    ) {
        task_bundle.add_risk("code-change tasks require explicit verification");
    }
    if scenario.replay.workflow_judgment {
        task_bundle.add_acceptance_check("Model workflow contract defined acceptance criteria");
    }
    trace.events.push(TraceEvent::TaskContextBuilt {
        task_id: task_bundle.task_id.clone(),
        workflow: format!("{:?}", task_bundle.route.workflow),
        files: task_bundle.relevant_files.len(),
        constraints: task_bundle.constraints.len(),
        risks: task_bundle.risks.len(),
        acceptance_checks: task_bundle.acceptance_checks.len(),
    });
    let reflection = crate::engine::reflection_pass::ReflectionPass::from_task_bundle(&task_bundle);
    trace.events.push(TraceEvent::ReflectionPassCompleted {
        pass_id: reflection.pass_id.clone(),
        task_id: reflection.task_id.clone(),
        status: format!("{:?}", reflection.status),
        findings: reflection.findings.len(),
        unresolved: reflection.unresolved_count(),
    });
    append_replay_trace(&mut trace, scenario, &task_bundle.task_id);
    trace
}

fn append_replay_trace(trace: &mut TurnTrace, scenario: &EvalScenario, task_id: &str) {
    for compaction in &scenario.replay.context_compactions {
        trace.events.push(TraceEvent::ContextCompacted {
            before_tokens: compaction.before_tokens,
            after_tokens: compaction.after_tokens,
            strategy: compaction.strategy.clone(),
            boundary_id: compaction.boundary_id.clone(),
            sequence: compaction.sequence,
            messages_before: compaction.messages_before,
            messages_after: compaction.messages_after,
            preserved_tail_count: compaction.preserved_tail_count,
            provenance: compaction.provenance.clone(),
        });
    }

    if let Some(diet) = &scenario.replay.runtime_diet {
        trace.events.push(TraceEvent::RuntimeDietReport {
            prompt_tokens: diet.prompt_tokens,
            tool_schema_tokens: diet.tool_schema_tokens,
            total_request_tokens: diet.total_request_tokens,
            max_context_tokens: diet.max_context_tokens,
            remaining_context_tokens: diet.remaining_context_tokens,
            tool_result_chars: diet.tool_result_chars,
            tool_result_tokens: diet.tool_result_tokens,
            truncated_tool_results: diet.truncated_tool_results,
            tool_result_artifacts: diet.tool_result_artifacts,
            exposed_tools: diet.exposed_tools,
            memory_snapshot_chars: diet.memory_snapshot_chars,
            memory_snapshot_tokens: diet.memory_snapshot_tokens,
            retrieval_items: diet.retrieval_items,
            retrieval_tokens: diet.retrieval_tokens,
            skill_list_chars: diet.skill_list_chars,
            skill_list_tokens: diet.skill_list_tokens,
            route_scoped_tools: diet.route_scoped_tools,
            workflow_context: diet.workflow_context.clone(),
            closeout_visibility: diet.closeout_visibility.clone(),
            validation_evidence: diet.validation_evidence.clone(),
        });
    }

    for subagent in &scenario.replay.subagents {
        trace.events.push(TraceEvent::SubagentStarted {
            agent_id: subagent.agent_id.clone(),
            profile: subagent.profile.clone(),
            role: subagent.role.clone(),
            description: subagent.description.clone(),
            timeout_secs: subagent.timeout_secs,
            allowed_tools: subagent.allowed_tools,
        });
        trace.events.push(TraceEvent::SubagentCompleted {
            agent_id: subagent.agent_id.clone(),
            status: subagent.status.clone(),
            duration_ms: subagent.duration_ms,
            output_chars: subagent.output_chars,
            tools_used: subagent.tools_used,
        });
    }

    for resource in &scenario.replay.mcp_resources {
        trace.events.push(TraceEvent::McpResourceAccessed {
            server: resource.server.clone(),
            uri: resource.uri.clone(),
            action: resource.action.clone(),
            success: resource.success,
            content_chars: resource.content_chars,
        });
    }

    for (idx, repair) in scenario.replay.mcp_repairs.iter().enumerate() {
        trace.events.push(TraceEvent::RecoveryPlan {
            plan_id: format!("eval-mcp-repair-{}", idx + 1),
            source: "mcp".to_string(),
            category: format!("mcp_{}_required", repair.category),
            action: format!(
                "run {} then retry MCP resource access on {}",
                repair.command, repair.server
            ),
            retryable: true,
            safe_retry: repair.safe_retry,
            suggested_command: Some(repair.command.clone()),
            status: repair.status.clone(),
        });
    }

    for (idx, call) in scenario.replay.tool_calls.iter().enumerate() {
        let call_id = format!("eval-tool-{}", idx + 1);
        trace.events.push(TraceEvent::ToolStarted {
            tool: call.tool.clone(),
            call_id: call_id.clone(),
            parallel: false,
            pre_executed: false,
        });
        if let Some(permission) = &call.permission {
            trace.events.push(TraceEvent::PermissionRequested {
                tool: call.tool.clone(),
                call_id: call_id.clone(),
                prompt: if permission.prompt.is_empty() {
                    format!("Allow {}?", call.tool)
                } else {
                    permission.prompt.clone()
                },
            });
            trace.events.push(TraceEvent::PermissionResolved {
                tool: call.tool.clone(),
                call_id: call_id.clone(),
                approved: permission.approved,
                decision: permission.decision.clone(),
                persistence_scope: permission.persistence_scope.clone(),
                rule_pattern: permission.rule_pattern.clone(),
                persisted_path: permission.persisted_path.clone(),
            });
        }
        trace.events.push(TraceEvent::ToolCompleted {
            tool: call.tool.clone(),
            call_id,
            success: call.success,
            duration_ms: Some(0),
            output_chars: call.output.chars().count(),
        });
    }

    for (idx, plan) in scenario.replay.recovery_plans.iter().enumerate() {
        trace.events.push(TraceEvent::RecoveryPlan {
            plan_id: format!("eval-recovery-{}", idx + 1),
            source: plan.source.clone(),
            category: plan.category.clone(),
            action: plan.action.clone(),
            retryable: plan.retryable,
            safe_retry: plan.safe_retry,
            suggested_command: plan.suggested_command.clone(),
            status: plan.status.clone(),
        });
    }

    if let Some(passed) = scenario.replay.verification_passed {
        trace.events.push(TraceEvent::VerificationCompleted {
            changed_files: scenario.replay.changed_files.len(),
            passed,
            check_passed: passed,
            tests_passed: passed,
            review_passed: passed,
            failed_commands: if passed {
                Vec::new()
            } else {
                scenario.replay.failed_commands.clone()
            },
        });
        let changed_files = scenario
            .replay
            .changed_files
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let evidence = scenario
            .replay
            .tool_calls
            .iter()
            .filter(|call| !call.success || !passed)
            .map(|call| {
                if call.output.is_empty() {
                    format!("{} reported failure", call.tool)
                } else {
                    call.output.clone()
                }
            })
            .collect::<Vec<_>>();
        let reflection = crate::engine::reflection_pass::ReflectionPass::from_post_edit(
            task_id.to_string(),
            &changed_files,
            passed,
            &evidence,
        );
        trace.events.push(TraceEvent::ReflectionPassCompleted {
            pass_id: reflection.pass_id.clone(),
            task_id: reflection.task_id.clone(),
            status: format!("{:?}", reflection.status),
            findings: reflection.findings.len(),
            unresolved: reflection.unresolved_count(),
        });
    }

    if let Some(accepted) = scenario.replay.acceptance_review_accepted {
        trace.events.push(TraceEvent::AcceptanceReviewCompleted {
            accepted,
            confidence: if accepted { "High" } else { "Medium" }.to_string(),
            criteria: 2,
            unresolved: if accepted { 0 } else { 1 },
            next_action: if accepted { "Finish" } else { "ContinueRepair" }.to_string(),
        });
        if accepted {
            trace.events.push(TraceEvent::WorkflowPlanProgress {
                total_steps: 2,
                completed_steps: 2,
                active_step: None,
                top_priority: None,
                top_importance_score: None,
                top_weight_share: None,
                weight_source: None,
                reweighted: true,
            });
        }
    }

    if scenario.replay.guided_debugging {
        trace.events.push(TraceEvent::GuidedDebuggingCompleted {
            blocker: true,
            next_action: "Repair".to_string(),
            causes: 1,
            evidence_items: 1,
            ask_user: false,
        });
    }
}

fn trace_tool_sequence(trace: &TurnTrace) -> Vec<String> {
    trace
        .events
        .iter()
        .filter_map(|event| match event {
            TraceEvent::ToolStarted { tool, .. } => Some(tool.clone()),
            _ => None,
        })
        .collect()
}

fn trace_has_failed_tool(trace: &TurnTrace, expected_tool: &str) -> bool {
    trace.events.iter().any(|event| {
        matches!(
            event,
            TraceEvent::ToolCompleted { tool, success, .. }
                if tool == expected_tool && !success
        )
    })
}

fn trace_verification_status(trace: &TurnTrace) -> Option<bool> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::VerificationCompleted { passed, .. } => Some(*passed),
        _ => None,
    })
}

fn trace_last_reflection_status(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::ReflectionPassCompleted { status, .. } => Some(status.clone()),
        _ => None,
    })
}

fn trace_repair_required(trace: &TurnTrace) -> bool {
    trace.events.iter().any(|event| {
        matches!(
            event,
            TraceEvent::ReflectionPassCompleted {
                status,
                unresolved,
                ..
            } if status != "Passed" && *unresolved > 0
        )
    })
}

fn trace_last_permission_approved(trace: &TurnTrace) -> Option<bool> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::PermissionResolved { approved, .. } => Some(*approved),
        _ => None,
    })
}

fn trace_last_permission_decision(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::PermissionResolved { decision, .. } => decision.clone(),
        _ => None,
    })
}

fn trace_last_permission_persistence_scope(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::PermissionResolved {
            persistence_scope, ..
        } => persistence_scope.clone(),
        _ => None,
    })
}

fn trace_has_matching_recovery_plan(
    trace: &TurnTrace,
    expected_category: Option<&str>,
    expected_suggested_command: Option<&str>,
    expected_safe_retry: Option<bool>,
) -> bool {
    trace.events.iter().any(|event| {
        let TraceEvent::RecoveryPlan {
            category,
            safe_retry,
            suggested_command,
            ..
        } = event
        else {
            return false;
        };

        expected_category.is_none_or(|expected| category == expected)
            && expected_suggested_command
                .is_none_or(|expected| suggested_command.as_deref() == Some(expected))
            && expected_safe_retry.is_none_or(|expected| *safe_retry == expected)
    })
}

fn has_terminal_task_expectation(expect: &EvalExpect) -> bool {
    expect.terminal_task_id.is_some()
        || expect.terminal_task_status.is_some()
        || expect.terminal_task_read_tool.is_some()
        || expect.terminal_task_cancel_tool.is_some()
        || expect.backgrounded_tool.is_some()
}

fn replay_has_matching_terminal_task(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.terminal_tasks.iter().any(|task| {
        expect
            .terminal_task_id
            .as_deref()
            .is_none_or(|expected| task.id == expected)
            && expect
                .terminal_task_status
                .as_deref()
                .is_none_or(|expected| task.status == expected)
            && expect
                .terminal_task_read_tool
                .as_deref()
                .is_none_or(|expected| task.read_tool.as_deref() == Some(expected))
            && expect
                .terminal_task_cancel_tool
                .as_deref()
                .is_none_or(|expected| task.cancel_tool.as_deref() == Some(expected))
            && expect
                .backgrounded_tool
                .as_deref()
                .is_none_or(|expected| task.backgrounded && task.source_tool == expected)
    })
}

fn has_file_checkpoint_expectation(expect: &EvalExpect) -> bool {
    expect.file_checkpoint_id.is_some()
        || expect.file_change_id.is_some()
        || expect.file_checkpoint_path.is_some()
}

fn replay_has_matching_file_change(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.file_changes.iter().any(|change| {
        expect
            .file_checkpoint_id
            .as_deref()
            .is_none_or(|expected| change.checkpoint_id == expected)
            && expect
                .file_change_id
                .as_deref()
                .is_none_or(|expected| change.id == expected)
            && expect
                .file_checkpoint_path
                .as_deref()
                .is_none_or(|expected| change.path == expected)
    })
}

fn has_rewind_expectation(expect: &EvalExpect) -> bool {
    expect.rewind_target.is_some()
        || expect.rewind_command.is_some()
        || expect.rewind_checkpoint_id.is_some()
        || expect.rewind_restored_files.is_some()
}

fn replay_has_matching_rewind(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    let Some(rewind) = &replay.rewind else {
        return false;
    };

    expect
        .rewind_target
        .as_deref()
        .is_none_or(|expected| rewind.target == expected)
        && expect
            .rewind_command
            .as_deref()
            .is_none_or(|expected| rewind.command == expected)
        && expect
            .rewind_checkpoint_id
            .as_deref()
            .is_none_or(|expected| rewind.checkpoint_id == expected)
        && expect
            .rewind_restored_files
            .is_none_or(|expected| rewind.restored_files.len() == expected)
        && rewind.failed_files.is_empty()
}

fn has_context_compaction_expectation(expect: &EvalExpect) -> bool {
    expect.context_boundary_id.is_some()
        || expect.context_compaction_strategy.is_some()
        || expect.context_before_tokens.is_some()
        || expect.context_after_tokens.is_some()
        || expect.context_preserved_tail_count.is_some()
}

fn trace_has_matching_context_compaction(trace: &TurnTrace, expect: &EvalExpect) -> bool {
    trace.events.iter().any(|event| {
        let TraceEvent::ContextCompacted {
            before_tokens,
            after_tokens,
            strategy,
            boundary_id,
            preserved_tail_count,
            ..
        } = event
        else {
            return false;
        };

        expect
            .context_boundary_id
            .as_deref()
            .is_none_or(|expected| boundary_id.as_deref() == Some(expected))
            && expect
                .context_compaction_strategy
                .as_deref()
                .is_none_or(|expected| strategy == expected)
            && expect
                .context_before_tokens
                .is_none_or(|expected| *before_tokens == expected)
            && expect
                .context_after_tokens
                .is_none_or(|expected| *after_tokens == expected)
            && expect
                .context_preserved_tail_count
                .is_none_or(|expected| *preserved_tail_count == Some(expected))
    })
}

fn has_runtime_diet_expectation(expect: &EvalExpect) -> bool {
    expect.runtime_diet_total_request_tokens.is_some()
        || expect.runtime_diet_remaining_context_tokens.is_some()
        || expect.runtime_diet_route_scoped_tools.is_some()
        || expect.runtime_diet_workflow_context.is_some()
}

fn trace_has_matching_runtime_diet(trace: &TurnTrace, expect: &EvalExpect) -> bool {
    trace.events.iter().any(|event| {
        let TraceEvent::RuntimeDietReport {
            total_request_tokens,
            remaining_context_tokens,
            route_scoped_tools,
            workflow_context,
            ..
        } = event
        else {
            return false;
        };

        expect
            .runtime_diet_total_request_tokens
            .is_none_or(|expected| *total_request_tokens == expected)
            && expect
                .runtime_diet_remaining_context_tokens
                .is_none_or(|expected| *remaining_context_tokens == Some(expected))
            && expect
                .runtime_diet_route_scoped_tools
                .is_none_or(|expected| *route_scoped_tools == expected)
            && expect
                .runtime_diet_workflow_context
                .as_deref()
                .is_none_or(|expected| workflow_context == expected)
    })
}

fn has_subagent_expectation(expect: &EvalExpect) -> bool {
    expect.subagent_agent_id.is_some()
        || expect.subagent_profile.is_some()
        || expect.subagent_role.is_some()
        || expect.subagent_status.is_some()
        || expect.subagent_context_mode.is_some()
        || expect.subagent_allowed_tools.is_some()
        || expect.isolated_worktree_path.is_some()
        || expect.isolated_worktree_branch.is_some()
        || expect.recursive_fork_guard.is_some()
        || expect.fork_placeholder_complete.is_some()
        || expect.fork_message_count.is_some()
}

fn replay_has_matching_subagent(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.subagents.iter().any(|subagent| {
        expect
            .subagent_agent_id
            .as_deref()
            .is_none_or(|expected| subagent.agent_id == expected)
            && expect
                .subagent_profile
                .as_deref()
                .is_none_or(|expected| subagent.profile.as_deref() == Some(expected))
            && expect
                .subagent_role
                .as_deref()
                .is_none_or(|expected| subagent.role == expected)
            && expect
                .subagent_status
                .as_deref()
                .is_none_or(|expected| subagent.status == expected)
            && expect
                .subagent_context_mode
                .as_deref()
                .is_none_or(|expected| subagent.context_mode.as_deref() == Some(expected))
            && expect
                .subagent_allowed_tools
                .is_none_or(|expected| subagent.allowed_tools == expected)
            && expect
                .isolated_worktree_path
                .as_deref()
                .is_none_or(|expected| subagent.worktree_path.as_deref() == Some(expected))
            && expect
                .isolated_worktree_branch
                .as_deref()
                .is_none_or(|expected| subagent.worktree_branch.as_deref() == Some(expected))
            && expect
                .recursive_fork_guard
                .is_none_or(|expected| subagent.recursive_fork_guard == expected)
            && expect
                .fork_placeholder_complete
                .is_none_or(|expected| subagent.placeholder_complete == expected)
            && expect
                .fork_message_count
                .is_none_or(|expected| subagent.fork_message_count == Some(expected))
    })
}

fn replay_has_agent_worktree_action(
    replay: &EvalReplay,
    action: &str,
    expected_command: Option<&str>,
    expected_status: Option<&str>,
) -> bool {
    replay.agent_worktree_actions.iter().any(|record| {
        record.action == action
            && expected_command.is_none_or(|expected| record.command.as_deref() == Some(expected))
            && expected_status.is_none_or(|expected| record.status == expected)
    })
}

fn replay_has_matching_agent_worktree_metadata(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    let merge_matches = expect
        .agent_worktree_merge_kind
        .as_deref()
        .is_none_or(|expected| {
            replay.agent_worktree_actions.iter().any(|record| {
                record.action == "agent_merge" && record.merge_kind.as_deref() == Some(expected)
            })
        });
    let cleanup_matches = expect
        .agent_worktree_cleanup_deleted_branch
        .is_none_or(|expected| {
            replay
                .agent_worktree_actions
                .iter()
                .any(|record| record.action == "agent_cleanup" && record.delete_branch == expected)
        });

    merge_matches && cleanup_matches
}

fn has_mcp_resource_expectation(expect: &EvalExpect) -> bool {
    expect.mcp_resource_server.is_some()
        || expect.mcp_resource_uri.is_some()
        || expect.mcp_resource_action.is_some()
        || expect.mcp_resource_success.is_some()
        || expect.mcp_resource_content_chars.is_some()
}

fn trace_has_matching_mcp_resource(trace: &TurnTrace, expect: &EvalExpect) -> bool {
    trace.events.iter().any(|event| {
        let TraceEvent::McpResourceAccessed {
            server,
            uri,
            action,
            success,
            content_chars,
        } = event
        else {
            return false;
        };

        expect
            .mcp_resource_server
            .as_deref()
            .is_none_or(|expected| server == expected)
            && expect
                .mcp_resource_uri
                .as_deref()
                .is_none_or(|expected| uri == expected)
            && expect
                .mcp_resource_action
                .as_deref()
                .is_none_or(|expected| action == expected)
            && expect
                .mcp_resource_success
                .is_none_or(|expected| *success == expected)
            && expect
                .mcp_resource_content_chars
                .is_none_or(|expected| *content_chars == expected)
    })
}

fn has_mcp_repair_expectation(expect: &EvalExpect) -> bool {
    expect.mcp_repair_server.is_some()
        || expect.mcp_repair_category.is_some()
        || expect.mcp_repair_command.is_some()
        || expect.mcp_repair_status.is_some()
        || expect.mcp_panel_command.is_some()
}

fn replay_has_matching_mcp_repair(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.mcp_repairs.iter().any(|repair| {
        expect
            .mcp_repair_server
            .as_deref()
            .is_none_or(|expected| repair.server == expected)
            && expect
                .mcp_repair_category
                .as_deref()
                .is_none_or(|expected| repair.category == expected)
            && expect
                .mcp_repair_command
                .as_deref()
                .is_none_or(|expected| repair.command == expected)
            && expect
                .mcp_repair_status
                .as_deref()
                .is_none_or(|expected| repair.status == expected)
            && expect
                .mcp_panel_command
                .as_deref()
                .is_none_or(|expected| repair.panel_command == expected)
    })
}

fn default_true() -> bool {
    true
}

fn default_recovery_status() -> String {
    "Planned".to_string()
}

fn default_running_status() -> String {
    "running".to_string()
}

fn default_rewind_command() -> String {
    "/rewind".to_string()
}

fn default_workflow_context() -> String {
    "normal".to_string()
}

fn default_closeout_visibility() -> String {
    "standard".to_string()
}

fn default_validation_evidence() -> String {
    "none".to_string()
}

fn default_agent_role() -> String {
    "specialist".to_string()
}

fn default_agent_timeout_secs() -> u64 {
    120
}

fn default_agent_status() -> String {
    "completed".to_string()
}

fn default_action_status() -> String {
    "success".to_string()
}

fn default_mcp_uri() -> String {
    "*".to_string()
}

fn default_mcp_resource_action() -> String {
    "read".to_string()
}

fn default_mcp_panel_command() -> String {
    "/panel mcp".to_string()
}

fn is_evalset_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml" | "yml" | "json")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_runner_passes_matching_route() {
        let set = EvalSet {
            name: "smoke".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "debug-route".to_string(),
                prompt: "cargo test 报错了，帮我修复".to_string(),
                replay: EvalReplay::default(),
                expect: EvalExpect {
                    intent: Some(IntentKind::Debugging),
                    workflow: Some(WorkflowKind::BugFix),
                    retrieval: Some(RetrievalPolicy::Project),
                    recommended_tools: vec!["bash".to_string()],
                    trace_events: vec!["prompt".to_string(), "intent".to_string()],
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_reports_mismatch() {
        let set = EvalSet {
            name: "bad".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "bad-route".to_string(),
                prompt: "你好".to_string(),
                replay: EvalReplay::default(),
                expect: EvalExpect {
                    intent: Some(IntentKind::Debugging),
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(!report.ok());
        assert_eq!(report.failed, 1);
        assert!(report.summary().contains("bad-route"));
    }

    #[test]
    fn loads_yaml_evalset() {
        let yaml = r#"
name: route_smoke
scenarios:
  - id: memory
    prompt: "记住我喜欢 compact 状态栏"
    expect:
      intent: memory
      retrieval: memory
      recommended_tools: ["memory_save"]
"#;
        let set: EvalSet = serde_yaml::from_str(yaml).unwrap();
        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_replays_tool_trajectory_and_reflection_gate() {
        let set = EvalSet {
            name: "trajectory".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "failed-edit".to_string(),
                prompt: "修改代码并修复测试".to_string(),
                replay: EvalReplay {
                    tool_calls: vec![
                        EvalToolCall {
                            tool: "file_edit".to_string(),
                            success: true,
                            output: "edited src/main.rs".to_string(),
                            permission: None,
                        },
                        EvalToolCall {
                            tool: "bash".to_string(),
                            success: false,
                            output: "cargo test failed".to_string(),
                            permission: None,
                        },
                    ],
                    verification_passed: Some(false),
                    changed_files: vec!["src/main.rs".to_string()],
                    ..Default::default()
                },
                expect: EvalExpect {
                    tool_sequence: vec!["file_edit".to_string(), "bash".to_string()],
                    failed_tool: Some("bash".to_string()),
                    verification_passed: Some(false),
                    reflection_status: Some("Blocked".to_string()),
                    repair_required: Some(true),
                    trace_events: vec![
                        "tool.start".to_string(),
                        "tool.done".to_string(),
                        "verify.done".to_string(),
                        "reflection.pass".to_string(),
                    ],
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_replays_workflow_contract_events() {
        let set = EvalSet {
            name: "workflow_contract".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "contract-visible".to_string(),
                prompt: "帮我修改代码，新增标签过滤页面".to_string(),
                replay: EvalReplay {
                    workflow_judgment: true,
                    acceptance_review_accepted: Some(true),
                    verification_passed: Some(true),
                    changed_files: vec!["src/app.rs".to_string()],
                    ..Default::default()
                },
                expect: EvalExpect {
                    workflow: Some(WorkflowKind::CodeChange),
                    trace_events: vec![
                        "workflow.judgment".to_string(),
                        "workflow.plan".to_string(),
                        "acceptance.review".to_string(),
                    ],
                    verification_passed: Some(true),
                    repair_required: Some(false),
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_replays_guided_debugging_event() {
        let set = EvalSet {
            name: "guided_debugging".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "tool-failure-debugging".to_string(),
                prompt: "cargo test 报错了，帮我修复".to_string(),
                replay: EvalReplay {
                    tool_calls: vec![EvalToolCall {
                        tool: "bash".to_string(),
                        success: false,
                        output: "cargo test failed".to_string(),
                        permission: None,
                    }],
                    guided_debugging: true,
                    ..Default::default()
                },
                expect: EvalExpect {
                    workflow: Some(WorkflowKind::BugFix),
                    failed_tool: Some("bash".to_string()),
                    trace_events: vec!["tool.done".to_string(), "guided.debug".to_string()],
                    repair_required: Some(true),
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_replays_permission_denial_and_recovery_plan() {
        let set = EvalSet {
            name: "permission_recovery".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "permission-denial-retry".to_string(),
                prompt: "危险命令被拒绝后改用安全路径继续".to_string(),
                replay: EvalReplay {
                    tool_calls: vec![
                        EvalToolCall {
                            tool: "bash".to_string(),
                            success: false,
                            output: "Permission denied: 'bash' requires user confirmation."
                                .to_string(),
                            permission: Some(EvalPermissionReplay {
                                prompt: "Allow bash rm -rf fixtures/tmp?".to_string(),
                                approved: false,
                                decision: Some("reject_once".to_string()),
                                ..Default::default()
                            }),
                        },
                        EvalToolCall {
                            tool: "file_read".to_string(),
                            success: true,
                            output: "safe readonly fallback".to_string(),
                            permission: None,
                        },
                    ],
                    recovery_plans: vec![EvalRecoveryPlan {
                        source: "tool_execution".to_string(),
                        category: "permission_denied".to_string(),
                        action: "explain denial and retry with safe readonly path".to_string(),
                        retryable: false,
                        safe_retry: false,
                        suggested_command: Some("/permissions explain".to_string()),
                        status: "Planned".to_string(),
                    }],
                    ..Default::default()
                },
                expect: EvalExpect {
                    failed_tool: Some("bash".to_string()),
                    tool_sequence: vec!["bash".to_string(), "file_read".to_string()],
                    permission_approved: Some(false),
                    permission_decision: Some("reject_once".to_string()),
                    recovery_category: Some("permission_denied".to_string()),
                    recovery_suggested_command: Some("/permissions explain".to_string()),
                    recovery_safe_retry: Some(false),
                    repair_required: Some(false),
                    trace_events: vec![
                        "permission.request".to_string(),
                        "permission.resolve".to_string(),
                        "recovery.plan".to_string(),
                    ],
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_replays_background_terminal_task() {
        let set = EvalSet {
            name: "background_terminal".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "bash-background-task".to_string(),
                prompt: "启动后台服务并读取一段输出".to_string(),
                replay: EvalReplay {
                    tool_calls: vec![
                        EvalToolCall {
                            tool: "bash".to_string(),
                            success: true,
                            output: "Started background shell command. Handle: shell-bg-eval-1"
                                .to_string(),
                            permission: None,
                        },
                        EvalToolCall {
                            tool: "bash_output".to_string(),
                            success: true,
                            output: "server ready".to_string(),
                            permission: None,
                        },
                    ],
                    terminal_tasks: vec![EvalTerminalTaskReplay {
                        id: "shell-bg-eval-1".to_string(),
                        source_tool: "bash".to_string(),
                        status: "running".to_string(),
                        command: Some("npm run dev".to_string()),
                        handle: Some("shell-bg-eval-1".to_string()),
                        read_tool: Some("bash_output".to_string()),
                        cancel_tool: Some("bash_cancel".to_string()),
                        cancel_handle: Some("shell-bg-eval-1".to_string()),
                        output_path: None,
                        backgrounded: true,
                    }],
                    ..Default::default()
                },
                expect: EvalExpect {
                    tool_sequence: vec!["bash".to_string(), "bash_output".to_string()],
                    terminal_task_count: Some(1),
                    terminal_task_id: Some("shell-bg-eval-1".to_string()),
                    terminal_task_status: Some("running".to_string()),
                    terminal_task_read_tool: Some("bash_output".to_string()),
                    terminal_task_cancel_tool: Some("bash_cancel".to_string()),
                    backgrounded_tool: Some("bash".to_string()),
                    trace_events: vec!["tool.start".to_string(), "tool.done".to_string()],
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_replays_file_checkpoint_and_rewind() {
        let set = EvalSet {
            name: "file_rewind".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "file-edit-rewind".to_string(),
                prompt: "改一个文件，然后回滚这次修改".to_string(),
                replay: EvalReplay {
                    tool_calls: vec![EvalToolCall {
                        tool: "file_edit".to_string(),
                        success: true,
                        output: "edited src/lib.rs with checkpoint cp_eval_1".to_string(),
                        permission: None,
                    }],
                    file_changes: vec![EvalFileChangeReplay {
                        id: "fc_eval_1".to_string(),
                        checkpoint_id: "cp_eval_1".to_string(),
                        path: "src/lib.rs".to_string(),
                        tool_name: "file_edit".to_string(),
                        existed_before: true,
                        before_hash: Some("before123".to_string()),
                        after_hash: Some("after456".to_string()),
                        diff: Some("-old\n+new".to_string()),
                        bytes_written: 42,
                    }],
                    rewind: Some(EvalRewindReplay {
                        target: "fc_eval_1".to_string(),
                        checkpoint_id: "cp_eval_1".to_string(),
                        command: "/rewind".to_string(),
                        restored_files: vec!["src/lib.rs".to_string()],
                        removed_files: Vec::new(),
                        failed_files: Vec::new(),
                    }),
                    ..Default::default()
                },
                expect: EvalExpect {
                    tool_sequence: vec!["file_edit".to_string()],
                    file_checkpoint_count: Some(1),
                    file_change_id: Some("fc_eval_1".to_string()),
                    file_checkpoint_id: Some("cp_eval_1".to_string()),
                    file_checkpoint_path: Some("src/lib.rs".to_string()),
                    rewind_target: Some("fc_eval_1".to_string()),
                    rewind_command: Some("/rewind".to_string()),
                    rewind_checkpoint_id: Some("cp_eval_1".to_string()),
                    rewind_restored_files: Some(1),
                    available_commands: vec!["/rewind".to_string(), "/checkpoints".to_string()],
                    trace_events: vec!["tool.start".to_string(), "tool.done".to_string()],
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_replays_compaction_boundary_and_runtime_diet() {
        let set = EvalSet {
            name: "compaction_boundary".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "compaction-boundary".to_string(),
                prompt: "长会话压缩后继续执行当前修复".to_string(),
                replay: EvalReplay {
                    context_compactions: vec![EvalContextCompactionReplay {
                        before_tokens: 122_000,
                        after_tokens: 64_000,
                        strategy: "semantic_boundary".to_string(),
                        boundary_id: Some("cb-eval-1".to_string()),
                        sequence: Some(7),
                        messages_before: Some(48),
                        messages_after: Some(19),
                        preserved_tail_count: Some(6),
                        provenance: vec![
                            "summary:project_state".to_string(),
                            "tail:latest_tool_results".to_string(),
                        ],
                    }],
                    runtime_diet: Some(EvalRuntimeDietReplay {
                        prompt_tokens: 64_000,
                        tool_schema_tokens: 3_200,
                        total_request_tokens: 67_200,
                        max_context_tokens: Some(128_000),
                        remaining_context_tokens: Some(60_800),
                        tool_result_chars: 4_096,
                        tool_result_tokens: 1_024,
                        truncated_tool_results: 1,
                        tool_result_artifacts: 1,
                        exposed_tools: 12,
                        memory_snapshot_chars: 0,
                        memory_snapshot_tokens: 0,
                        retrieval_items: 2,
                        retrieval_tokens: 720,
                        skill_list_chars: 0,
                        skill_list_tokens: 0,
                        route_scoped_tools: true,
                        workflow_context: "strict".to_string(),
                        closeout_visibility: "full".to_string(),
                        validation_evidence: "pending".to_string(),
                    }),
                    tool_calls: vec![EvalToolCall {
                        tool: "file_read".to_string(),
                        success: true,
                        output: "compacted context retained target".to_string(),
                        permission: None,
                    }],
                    ..Default::default()
                },
                expect: EvalExpect {
                    context_compaction_count: Some(1),
                    context_boundary_id: Some("cb-eval-1".to_string()),
                    context_compaction_strategy: Some("semantic_boundary".to_string()),
                    context_before_tokens: Some(122_000),
                    context_after_tokens: Some(64_000),
                    context_preserved_tail_count: Some(6),
                    runtime_diet_total_request_tokens: Some(67_200),
                    runtime_diet_remaining_context_tokens: Some(60_800),
                    runtime_diet_route_scoped_tools: Some(true),
                    runtime_diet_workflow_context: Some("strict".to_string()),
                    tool_sequence: vec!["file_read".to_string()],
                    trace_events: vec![
                        "context.compact".to_string(),
                        "runtime.diet".to_string(),
                        "tool.done".to_string(),
                    ],
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_replays_subagent_isolated_worktree_review_merge_cleanup() {
        let set = EvalSet {
            name: "subagent_worktree".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "subagent-worktree-worker".to_string(),
                prompt: "派子 agent 在隔离 worktree 里修改路由，然后 review/merge/cleanup"
                    .to_string(),
                replay: EvalReplay {
                    subagents: vec![EvalSubagentReplay {
                        agent_id: "agent_eval_1".to_string(),
                        profile: Some("implementer".to_string()),
                        role: "specialist".to_string(),
                        description: "Implement scoped route repair".to_string(),
                        timeout_secs: 300,
                        allowed_tools: 4,
                        status: "completed".to_string(),
                        duration_ms: 1_200,
                        output_chars: 512,
                        tools_used: 3,
                        context_mode: Some("isolated_worktree_fork".to_string()),
                        worktree_path: Some(
                            "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval"
                                .to_string(),
                        ),
                        worktree_branch: Some("codex/agent-eval1".to_string()),
                        recursive_fork_guard: true,
                        placeholder_complete: true,
                        fork_message_count: Some(4),
                        parent_tool_call_ids: vec!["parent_call_1".to_string()],
                        cleanup_hooks: vec!["worktree_cleanup".to_string()],
                    }],
                    agent_worktree_actions: vec![
                        EvalAgentWorktreeActionReplay {
                            action: "agent_review".to_string(),
                            agent_id: "agent_eval_1".to_string(),
                            command: Some("/agents worktree review agent_eval_1".to_string()),
                            status: "success".to_string(),
                            path: Some(
                                "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval"
                                    .to_string(),
                            ),
                            branch: Some("codex/agent-eval1".to_string()),
                            commits_ahead: Some(1),
                            merge_kind: None,
                            cleanup: false,
                            delete_branch: false,
                        },
                        EvalAgentWorktreeActionReplay {
                            action: "agent_merge".to_string(),
                            agent_id: "agent_eval_1".to_string(),
                            command: Some("/agents worktree merge agent_eval_1 --yes".to_string()),
                            status: "success".to_string(),
                            path: Some(
                                "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval"
                                    .to_string(),
                            ),
                            branch: Some("codex/agent-eval1".to_string()),
                            commits_ahead: Some(1),
                            merge_kind: Some("branch".to_string()),
                            cleanup: false,
                            delete_branch: false,
                        },
                        EvalAgentWorktreeActionReplay {
                            action: "agent_cleanup".to_string(),
                            agent_id: "agent_eval_1".to_string(),
                            command: Some(
                                "/agents worktree cleanup agent_eval_1 --yes --delete-branch"
                                    .to_string(),
                            ),
                            status: "success".to_string(),
                            path: Some(
                                "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval"
                                    .to_string(),
                            ),
                            branch: Some("codex/agent-eval1".to_string()),
                            commits_ahead: None,
                            merge_kind: None,
                            cleanup: true,
                            delete_branch: true,
                        },
                    ],
                    tool_calls: vec![
                        EvalToolCall {
                            tool: "agent".to_string(),
                            success: true,
                            output: "agent_eval_1 completed in isolated worktree".to_string(),
                            permission: None,
                        },
                        EvalToolCall {
                            tool: "worktree".to_string(),
                            success: true,
                            output: "Agent worktree review: agent_eval_1".to_string(),
                            permission: None,
                        },
                        EvalToolCall {
                            tool: "worktree".to_string(),
                            success: true,
                            output: "Merged branch: codex/agent-eval1".to_string(),
                            permission: None,
                        },
                        EvalToolCall {
                            tool: "worktree".to_string(),
                            success: true,
                            output: "Removed agent worktree".to_string(),
                            permission: None,
                        },
                    ],
                    ..Default::default()
                },
                expect: EvalExpect {
                    subagent_count: Some(1),
                    subagent_agent_id: Some("agent_eval_1".to_string()),
                    subagent_profile: Some("implementer".to_string()),
                    subagent_role: Some("specialist".to_string()),
                    subagent_status: Some("completed".to_string()),
                    subagent_context_mode: Some("isolated_worktree_fork".to_string()),
                    subagent_allowed_tools: Some(4),
                    isolated_worktree_path: Some(
                        "/tmp/priority-agent/.claude/worktrees/agent-route-fix-eval".to_string(),
                    ),
                    isolated_worktree_branch: Some("codex/agent-eval1".to_string()),
                    recursive_fork_guard: Some(true),
                    fork_placeholder_complete: Some(true),
                    fork_message_count: Some(4),
                    agent_worktree_action_count: Some(3),
                    agent_worktree_review_command: Some(
                        "/agents worktree review agent_eval_1".to_string(),
                    ),
                    agent_worktree_merge_command: Some(
                        "/agents worktree merge agent_eval_1 --yes".to_string(),
                    ),
                    agent_worktree_cleanup_command: Some(
                        "/agents worktree cleanup agent_eval_1 --yes --delete-branch".to_string(),
                    ),
                    agent_worktree_review_status: Some("success".to_string()),
                    agent_worktree_merge_status: Some("success".to_string()),
                    agent_worktree_cleanup_status: Some("success".to_string()),
                    agent_worktree_merge_kind: Some("branch".to_string()),
                    agent_worktree_cleanup_deleted_branch: Some(true),
                    tool_sequence: vec![
                        "agent".to_string(),
                        "worktree".to_string(),
                        "worktree".to_string(),
                        "worktree".to_string(),
                    ],
                    trace_events: vec![
                        "subagent.start".to_string(),
                        "subagent.done".to_string(),
                        "tool.done".to_string(),
                    ],
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_runner_replays_mcp_auth_repair_and_retry() {
        let set = EvalSet {
            name: "mcp_auth_repair".to_string(),
            description: String::new(),
            scenarios: vec![EvalScenario {
                id: "mcp-auth-repair".to_string(),
                prompt: "MCP server 未批准时提示修复，然后批准并重试 resource read".to_string(),
                replay: EvalReplay {
                    mcp_resources: vec![
                        EvalMcpResourceReplay {
                            server: "filesystem".to_string(),
                            uri: "file:///repo/README.md".to_string(),
                            action: "read".to_string(),
                            success: false,
                            content_chars: 0,
                        },
                        EvalMcpResourceReplay {
                            server: "filesystem".to_string(),
                            uri: "file:///repo/README.md".to_string(),
                            action: "read".to_string(),
                            success: true,
                            content_chars: 128,
                        },
                    ],
                    mcp_repairs: vec![EvalMcpRepairReplay {
                        server: "filesystem".to_string(),
                        category: "approval".to_string(),
                        command: "/mcp approve filesystem".to_string(),
                        panel_command: "/panel mcp".to_string(),
                        status: "Planned".to_string(),
                        safe_retry: false,
                    }],
                    tool_calls: vec![
                        EvalToolCall {
                            tool: "read_mcp_resource".to_string(),
                            success: false,
                            output: "MCP server 'filesystem' is pending approval".to_string(),
                            permission: None,
                        },
                        EvalToolCall {
                            tool: "mcp".to_string(),
                            success: true,
                            output: "MCP server 'filesystem' approved.".to_string(),
                            permission: None,
                        },
                        EvalToolCall {
                            tool: "read_mcp_resource".to_string(),
                            success: true,
                            output: "resource content after approval".to_string(),
                            permission: None,
                        },
                    ],
                    ..Default::default()
                },
                expect: EvalExpect {
                    failed_tool: Some("read_mcp_resource".to_string()),
                    tool_sequence: vec![
                        "read_mcp_resource".to_string(),
                        "mcp".to_string(),
                        "read_mcp_resource".to_string(),
                    ],
                    mcp_resource_count: Some(2),
                    mcp_resource_failure_count: Some(1),
                    mcp_resource_success_count: Some(1),
                    mcp_resource_server: Some("filesystem".to_string()),
                    mcp_resource_uri: Some("file:///repo/README.md".to_string()),
                    mcp_resource_action: Some("read".to_string()),
                    mcp_resource_success: Some(true),
                    mcp_resource_content_chars: Some(128),
                    mcp_repair_count: Some(1),
                    mcp_repair_server: Some("filesystem".to_string()),
                    mcp_repair_category: Some("approval".to_string()),
                    mcp_repair_command: Some("/mcp approve filesystem".to_string()),
                    mcp_repair_status: Some("Planned".to_string()),
                    mcp_panel_command: Some("/panel mcp".to_string()),
                    recovery_category: Some("mcp_approval_required".to_string()),
                    recovery_suggested_command: Some("/mcp approve filesystem".to_string()),
                    recovery_safe_retry: Some(false),
                    available_commands: vec!["/mcp".to_string(), "/panel".to_string()],
                    trace_events: vec![
                        "mcp.resource".to_string(),
                        "recovery.plan".to_string(),
                        "tool.done".to_string(),
                    ],
                    ..Default::default()
                },
            }],
        };

        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn bundled_smoke_evalset_passes() {
        let path = std::path::Path::new("evalsets/smoke.yaml");
        if !path.exists() {
            return;
        }
        let set = load_evalset(path).unwrap();
        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn bundled_feature_reality_evalset_passes() {
        let path = std::path::Path::new("evalsets/feature_reality.yaml");
        if !path.exists() {
            return;
        }
        let set = load_evalset(path).unwrap();
        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn bundled_coding_replay_matrix_passes() {
        let path = std::path::Path::new("evalsets/coding_replay_matrix.yaml");
        if !path.exists() {
            return;
        }
        let set = load_evalset(path).unwrap();
        assert!(
            set.scenarios.len() >= 25,
            "coding replay matrix should cover at least 25 scenarios"
        );
        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }

    #[test]
    fn eval_reports_json_contains_trend_fields() {
        let reports = vec![EvalReport {
            set_name: "sample".to_string(),
            total: 2,
            passed: 1,
            failed: 1,
            failures: vec![EvalFailure {
                scenario_id: "case-1".to_string(),
                message: "expected trace event".to_string(),
            }],
        }];
        let json = format_reports_json(&reports).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["sets"], 1);
        assert_eq!(value["scenarios"], 2);
        assert_eq!(value["passed"], 1);
        assert_eq!(value["failed"], 1);
        assert!(value["generated_at"].as_str().unwrap_or("").contains('T'));
        assert_eq!(value["reports"][0]["failures"][0]["scenario_id"], "case-1");
    }

    #[test]
    fn safe_eval_report_label_removes_path_separators() {
        assert_eq!(
            safe_eval_report_label("../coding replay/matrix.yaml"),
            "coding-replay-matrix-yaml"
        );
        assert_eq!(safe_eval_report_label("../../"), "all");
        assert_eq!(safe_eval_report_label("smoke_1"), "smoke_1");
    }

    #[test]
    fn write_reports_json_creates_trend_file() {
        let dir = tempfile::tempdir().unwrap();
        let reports = vec![EvalReport {
            set_name: "sample".to_string(),
            total: 1,
            passed: 1,
            failed: 0,
            failures: Vec::new(),
        }];

        let path = write_reports_json(&reports, dir.path(), "../sample").unwrap();

        assert_eq!(path.parent(), Some(dir.path()));
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("-sample.json"));
        let json = fs::read_to_string(path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["sets"], 1);
        assert_eq!(value["scenarios"], 1);
        assert_eq!(value["failed"], 0);
    }

    #[test]
    fn load_eval_report_bundles_returns_latest_first() {
        let dir = tempfile::tempdir().unwrap();
        let old = EvalReportBundle {
            generated_at: "2026-05-03T01:00:00Z".to_string(),
            sets: 1,
            scenarios: 2,
            passed: 1,
            failed: 1,
            baseline: None,
            reports: Vec::new(),
        };
        let new = EvalReportBundle {
            generated_at: "2026-05-03T02:00:00Z".to_string(),
            sets: 1,
            scenarios: 2,
            passed: 2,
            failed: 0,
            baseline: None,
            reports: Vec::new(),
        };
        fs::write(
            dir.path().join("eval-20260503T010000Z-all.json"),
            serde_json::to_string_pretty(&old).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.path().join("eval-20260503T020000Z-all.json"),
            serde_json::to_string_pretty(&new).unwrap(),
        )
        .unwrap();
        fs::write(dir.path().join("notes.json"), "{}").unwrap();

        let entries = load_eval_report_bundles(dir.path(), 10).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].1.generated_at, "2026-05-03T02:00:00Z");
        assert_eq!(entries[1].1.generated_at, "2026-05-03T01:00:00Z");
    }

    #[test]
    fn format_eval_trend_shows_latest_and_delta() {
        let entries = vec![
            (
                PathBuf::from("eval-20260503T020000Z-all.json"),
                EvalReportBundle {
                    generated_at: "2026-05-03T02:00:00Z".to_string(),
                    sets: 1,
                    scenarios: 3,
                    passed: 3,
                    failed: 0,
                    baseline: None,
                    reports: Vec::new(),
                },
            ),
            (
                PathBuf::from("eval-20260503T010000Z-all.json"),
                EvalReportBundle {
                    generated_at: "2026-05-03T01:00:00Z".to_string(),
                    sets: 1,
                    scenarios: 2,
                    passed: 1,
                    failed: 1,
                    baseline: None,
                    reports: Vec::new(),
                },
            ),
        ];

        let trend = format_eval_trend(&entries);

        assert!(trend.contains("Eval Trend"));
        assert!(trend.contains("Latest: eval-20260503T020000Z-all.json"));
        assert!(trend.contains("scenarios=+1"));
        assert!(trend.contains("passed=+2"));
        assert!(trend.contains("failed=-1"));
    }

    #[test]
    fn eval_report_bundle_parses_legacy_json_without_baseline() {
        let json = r#"{
            "generated_at": "2026-05-03T01:00:00Z",
            "sets": 1,
            "scenarios": 2,
            "passed": 2,
            "failed": 0,
            "reports": []
        }"#;

        let bundle: EvalReportBundle = serde_json::from_str(json).unwrap();

        assert_eq!(bundle.generated_at, "2026-05-03T01:00:00Z");
        assert!(bundle.baseline.is_none());
    }

    #[test]
    fn format_eval_trend_shows_external_baseline_delta() {
        let entries = vec![(
            PathBuf::from("eval-20260503T020000Z-all.json"),
            EvalReportBundle {
                generated_at: "2026-05-03T02:00:00Z".to_string(),
                sets: 1,
                scenarios: 20,
                passed: 18,
                failed: 2,
                baseline: Some(EvalBaselineSummary {
                    name: "claude-code-local".to_string(),
                    generated_at: Some("2026-05-03T01:30:00Z".to_string()),
                    scenarios: 20,
                    passed: 19,
                    failed: 1,
                }),
                reports: Vec::new(),
            },
        )];

        let trend = format_eval_trend(&entries);

        assert!(trend.contains("Delta vs baseline 'claude-code-local'"));
        assert!(trend.contains("scenarios=+0"));
        assert!(trend.contains("passed=-1"));
        assert!(trend.contains("failed=+1"));
        assert!(trend.contains("baseline_generated=2026-05-03T01:30:00Z"));
        assert!(trend.contains("baseline=claude-code-local"));
    }

    #[test]
    fn load_external_baselines_and_format_matrix_comparison() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("claude-code.yaml"),
            r#"
provider: claude-code
generated_at: "2026-05-21T12:00:00Z"
model: claude-opus
source: manual smoke run
scenarios:
  - id: file_edit_rewind
    outcome: pass
    validation_passed: true
    final_evidence_backed: true
    tool_calls: 4
    repair_turns: 0
    evidence: "edited, tested, rewound"
  - id: bash_background_task
    outcome: fail
    validation_passed: false
    final_evidence_backed: true
    tool_calls: 3
    repair_turns: 1
  - id: extra_untracked_case
    outcome: pass
"#,
        )
        .unwrap();

        let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
        let rendered = format_external_baseline_comparison(&baselines, Some("all"));

        assert_eq!(baselines.len(), 1);
        assert!(rendered.contains("External Baseline Comparison"));
        assert!(rendered.contains("claude-code [claude-opus]"));
        assert!(rendered.contains("coverage=2/6 pass=1 fail=1 blocked=0 not_run=0"));
        assert!(rendered.contains("missing: permission_denial_retry"));
        assert!(rendered.contains("unknown: extra_untracked_case"));
        assert!(rendered.contains("- file_edit_rewind: pass validation=true"));
        assert!(rendered.contains("evidence=edited, tested, rewound"));
    }

    #[test]
    fn external_baseline_provider_filter_reports_missing_provider() {
        let rendered = format_external_baseline_comparison(&[], Some("codex"));

        assert!(rendered.contains("No external baseline found for provider 'codex'"));
        assert!(rendered.contains("evalsets/external_baselines"));
    }

    #[test]
    fn external_baseline_template_covers_required_phase_12_ids() {
        let yaml = format_external_baseline_template("codex", Some("gpt-5.2")).unwrap();
        let parsed: EvalExternalBaselineSet = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.provider, "codex");
        assert_eq!(parsed.model.as_deref(), Some("gpt-5.2"));
        assert_eq!(
            parsed.scenarios.len(),
            crate::engine::scenario_matrix::deterministic_scenarios().len()
        );
        assert!(parsed
            .scenarios
            .iter()
            .all(|scenario| scenario.outcome == EvalExternalBaselineOutcome::NotRun));
        assert!(parsed
            .scenarios
            .iter()
            .any(|scenario| scenario.id == "mcp_auth_repair"));
    }

    #[test]
    fn write_external_baseline_template_refuses_overwrite() {
        let dir = tempfile::tempdir().unwrap();

        let path = write_external_baseline_template(dir.path(), "claude-code", None).unwrap();
        let err = write_external_baseline_template(dir.path(), "claude-code", None).unwrap_err();

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("baseline-claude-code.yaml")
        );
        assert!(err.to_string().contains("refusing to overwrite"));
    }

    #[test]
    fn imports_external_baseline_from_markdown_table() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("claude-run.md");
        fs::write(
            &artifact,
            r#"
| scenario | result | validation | evidence backed | tools | repairs | evidence |
| --- | --- | --- | --- | --- | --- | --- |
| file_edit_rewind | pass | yes | yes | 4 | 0 | checkpoint restored |
| bash_background_task | fail | no | yes | 3 | 1 | task timed out |
| unknown_case | pass | yes | yes | 1 | 0 | ignored |
"#,
        )
        .unwrap();

        let baseline =
            load_external_baseline_artifact(&artifact, "claude-code", Some("claude-opus")).unwrap();

        assert_eq!(baseline.provider, "claude-code");
        assert_eq!(baseline.model.as_deref(), Some("claude-opus"));
        assert_eq!(baseline.scenarios.len(), 2);
        let file_case = baseline
            .scenarios
            .iter()
            .find(|scenario| scenario.id == "file_edit_rewind")
            .unwrap();
        assert_eq!(file_case.outcome, EvalExternalBaselineOutcome::Pass);
        assert_eq!(file_case.validation_passed, Some(true));
        assert_eq!(file_case.final_evidence_backed, Some(true));
        assert_eq!(file_case.tool_calls, Some(4));
        assert_eq!(file_case.repair_turns, Some(0));
        assert_eq!(file_case.evidence.as_deref(), Some("checkpoint restored"));
    }

    #[test]
    fn write_external_baseline_import_refuses_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("codex-run.md");
        fs::write(
            &artifact,
            r#"
| id | outcome | evidence |
| --- | --- | --- |
| mcp_auth_repair | blocked | auth unavailable |
"#,
        )
        .unwrap();

        let path = write_external_baseline_import(&artifact, dir.path(), "codex", Some("gpt-5.2"))
            .unwrap();
        let err = write_external_baseline_import(&artifact, dir.path(), "codex", Some("gpt-5.2"))
            .unwrap_err();

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("baseline-codex-import.yaml")
        );
        assert!(err.to_string().contains("refusing to overwrite"));
    }

    #[test]
    fn validates_external_baseline_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("baseline-claude-code.yaml");
        fs::write(
            &path,
            r#"
provider: claude-code
model: claude-opus
scenarios:
  - id: file_edit_rewind
    outcome: pass
    validation_passed: true
    final_evidence_backed: false
    evidence: "TODO: fill later"
  - id: file_edit_rewind
    outcome: pass
    validation_passed: true
    final_evidence_backed: true
    evidence: "checkpoint restored"
  - id: bash_background_task
    outcome: fail
    evidence: "task timed out"
  - id: extra_case
    outcome: pass
"#,
        )
        .unwrap();

        let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
        let rendered = format_external_baseline_validation(&baselines, Some("all"));

        assert!(rendered.contains("External Baseline Validation"));
        assert!(rendered.contains("status=invalid"));
        assert!(rendered.contains("duplicate scenario file_edit_rewind appears 2 times"));
        assert!(rendered.contains("missing required scenario permission_denial_retry"));
        assert!(rendered.contains("unknown scenario extra_case"));
        assert!(rendered.contains("file_edit_rewind pass is missing final_evidence_backed=true"));
        assert!(rendered.contains("file_edit_rewind is missing concrete evidence"));
        assert!(
            rendered.contains("bash_background_task fail should record validation_passed=false")
        );
    }

    #[test]
    fn validates_complete_external_baseline_as_valid() {
        let dir = tempfile::tempdir().unwrap();
        let mut baseline = external_baseline_template("codex", Some("gpt-5.2"));
        for scenario in &mut baseline.scenarios {
            scenario.outcome = EvalExternalBaselineOutcome::Pass;
            scenario.validation_passed = Some(true);
            scenario.final_evidence_backed = Some(true);
            scenario.evidence = Some(format!("artifact for {}", scenario.id));
        }
        fs::write(
            dir.path().join("baseline-codex.yaml"),
            serde_yaml::to_string(&baseline).unwrap(),
        )
        .unwrap();

        let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
        let rendered = format_external_baseline_validation(&baselines, Some("codex"));

        assert!(rendered.contains("status=valid coverage=6/6 errors=0 warnings=0"));
    }

    #[test]
    fn formats_external_parity_report_with_provider_gaps() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("baseline-claude-code.yaml"),
            r#"
provider: claude-code
model: claude-opus
scenarios:
  - id: file_edit_rewind
    outcome: pass
    validation_passed: true
    final_evidence_backed: true
    evidence: "checkpoint restored"
  - id: bash_background_task
    outcome: fail
    validation_passed: false
    final_evidence_backed: true
    evidence: "background handle lost"
  - id: permission_denial_retry
    outcome: pass
    validation_passed: true
    final_evidence_backed: false
    evidence: "manual transcript"
"#,
        )
        .unwrap();

        let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
        let rendered = format_external_parity_report(&baselines, Some("all"));

        assert!(rendered.contains("Phase 12 Parity Report"));
        assert!(rendered.contains("Local replay-ready: 6/6  External providers: 1"));
        assert!(rendered.contains("- claude-code [claude-opus]: pass=2 fail=1 blocked=0 not_run=3"));
        assert!(rendered.contains("file_edit_rewind [replay_fixture_ready]"));
        assert!(rendered.contains("claude-code=pass gap=none validation=true evidence_backed=true"));
        assert!(rendered.contains("claude-code=fail gap=external_failed validation=false"));
        assert!(rendered.contains("claude-code=pass gap=evidence_incomplete"));
        assert!(rendered.contains("claude-code=missing gap=external_missing"));
    }

    #[test]
    fn parity_report_provider_filter_reports_missing_provider() {
        let rendered = format_external_parity_report(&[], Some("claude-code"));

        assert!(rendered.contains("Phase 12 Parity Report"));
        assert!(rendered.contains("No external baseline found for provider 'claude-code'"));
    }

    #[test]
    fn writes_external_parity_report_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let mut baseline = external_baseline_template("codex", Some("gpt-5.2"));
        for scenario in &mut baseline.scenarios {
            scenario.outcome = EvalExternalBaselineOutcome::Pass;
            scenario.validation_passed = Some(true);
            scenario.final_evidence_backed = Some(true);
            scenario.evidence = Some(format!("artifact for {}", scenario.id));
        }
        let baseline_path = dir.path().join("baseline-codex.yaml");
        fs::write(&baseline_path, serde_yaml::to_string(&baseline).unwrap()).unwrap();

        let baselines = load_external_baselines_from_dir(dir.path()).unwrap();
        let report_dir = dir.path().join("reports");
        let path = write_external_parity_report(&baselines, Some("codex"), &report_dir).unwrap();
        let content = fs::read_to_string(&path).unwrap();

        assert_eq!(path.parent(), Some(report_dir.as_path()));
        assert!(path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("parity-") && name.ends_with("-codex.txt")));
        assert!(content.contains("Phase 12 Parity Report"));
        assert!(content.contains("codex [gpt-5.2]: pass=6 fail=0 blocked=0 not_run=0"));
        assert!(content.contains("gap=none"));
    }
}
