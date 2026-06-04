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
mod external_baseline;
pub use external_baseline::*;
mod replay_matchers;
use replay_matchers::*;

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
                    None => {}
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

fn is_evalset_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml" | "yml" | "json")
    )
}

#[cfg(test)]
mod tests;
