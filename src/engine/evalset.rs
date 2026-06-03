//! Deterministic behavior evalsets for routing and trace contracts.
//!
//! This runner intentionally starts with non-LLM assertions so it can run in CI
//! and guard agent behavior while deeper replay support is added.

use crate::engine::intent_router::{IntentRoute, IntentRouter, ReasoningPolicy};
use crate::engine::trace::{TraceEvent, TurnTrace};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

mod model;
pub use model::*;

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
        self.check_run_context_replay(scenario, &mut failures);
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

    fn check_run_context_replay(&self, scenario: &EvalScenario, failures: &mut Vec<EvalFailure>) {
        let expect = &scenario.expect;
        if let Some(expected) = expect.context_attachment_count {
            let actual = scenario.replay.run_contexts.len();
            if actual != expected {
                failures.push(EvalFailure {
                    scenario_id: scenario.id.clone(),
                    message: format!(
                        "context_attachment_count expected {}, got {}",
                        expected, actual
                    ),
                });
            }
        }

        if has_run_context_expectation(expect)
            && !replay_has_matching_run_context(&scenario.replay, expect)
        {
            failures.push(EvalFailure {
                scenario_id: scenario.id.clone(),
                message: format!(
                    "expected matching run context type={:?} label={:?} file={:?} patch_preview_min_chars={:?}",
                    expect.context_attachment_type,
                    expect.context_attachment_label,
                    expect.context_attachment_file,
                    expect.context_attachment_patch_preview_min_chars
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
            trigger: None,
            token_pressure: None,
            boundary_id: compaction.boundary_id.clone(),
            sequence: compaction.sequence,
            messages_before: compaction.messages_before,
            messages_after: compaction.messages_after,
            preserved_tail_count: compaction.preserved_tail_count,
            retained_items: Vec::new(),
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
            warnings: diet.warnings.clone(),
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
            failure_type: format!("mcp_{}_required", repair.category),
            recovery_kind: "run_repair_command".to_string(),
            action: format!(
                "run {} then retry MCP resource access on {}",
                repair.command, repair.server
            ),
            retryable: true,
            safe_retry: repair.safe_retry,
            allowed_alternatives: vec!["inspect MCP server status".to_string()],
            retry_budget: Some(1),
            side_effect_uncertain: !repair.safe_retry,
            requires_user_decision: !repair.safe_retry,
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
                review: None,
            });
            trace.events.push(TraceEvent::PermissionResolved {
                tool: call.tool.clone(),
                call_id: call_id.clone(),
                approved: permission.approved,
                source: permission.source.clone(),
                decision: permission.decision.clone(),
                persistence_scope: permission.persistence_scope.clone(),
                rule_pattern: permission.rule_pattern.clone(),
                persisted_path: permission.persisted_path.clone(),
                review: None,
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
            failure_type: if plan.failure_type.is_empty() {
                plan.category.clone()
            } else {
                plan.failure_type.clone()
            },
            recovery_kind: plan.recovery_kind.clone(),
            action: plan.action.clone(),
            retryable: plan.retryable,
            safe_retry: plan.safe_retry,
            allowed_alternatives: plan.allowed_alternatives.clone(),
            retry_budget: plan.retry_budget,
            side_effect_uncertain: plan.side_effect_uncertain,
            requires_user_decision: plan.requires_user_decision,
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
        || expect.terminal_task_output_path.is_some()
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
                .terminal_task_output_path
                .as_deref()
                .is_none_or(|expected| task.output_path.as_deref() == Some(expected))
            && expect
                .backgrounded_tool
                .as_deref()
                .is_none_or(|expected| task.backgrounded && task.source_tool == expected)
    })
}

fn has_run_context_expectation(expect: &EvalExpect) -> bool {
    expect.context_attachment_type.is_some()
        || expect.context_attachment_label.is_some()
        || expect.context_attachment_file.is_some()
        || expect.context_attachment_patch_preview_min_chars.is_some()
}

fn replay_has_matching_run_context(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.run_contexts.iter().any(|context| {
        expect
            .context_attachment_type
            .as_deref()
            .is_none_or(|expected| context.context_type == expected)
            && expect
                .context_attachment_label
                .as_deref()
                .is_none_or(|expected| context.label == expected)
            && expect
                .context_attachment_file
                .as_deref()
                .is_none_or(|expected| context.files.iter().any(|file| file == expected))
            && expect
                .context_attachment_patch_preview_min_chars
                .is_none_or(|expected| context.patch_preview.chars().count() >= expected)
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

fn is_evalset_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml" | "yml" | "json")
    )
}

#[cfg(test)]
mod tests;
