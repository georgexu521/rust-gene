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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalToolCall {
    pub tool: String,
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub output: String,
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

        self.check_feature_reality(scenario, &mut failures);

        failures
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
        if name.map_or(true, |target| target == "all" || target == set.name) {
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
            top_priority: Some("P0 1.00".to_string()),
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
    for (idx, call) in scenario.replay.tool_calls.iter().enumerate() {
        let call_id = format!("eval-tool-{}", idx + 1);
        trace.events.push(TraceEvent::ToolStarted {
            tool: call.tool.clone(),
            call_id: call_id.clone(),
            parallel: false,
            pre_executed: false,
        });
        trace.events.push(TraceEvent::ToolCompleted {
            tool: call.tool.clone(),
            call_id,
            success: call.success,
            duration_ms: Some(0),
            output_chars: call.output.chars().count(),
        });
    }

    if let Some(passed) = scenario.replay.verification_passed {
        trace.events.push(TraceEvent::VerificationCompleted {
            changed_files: scenario.replay.changed_files.len(),
            passed,
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

fn default_true() -> bool {
    true
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
                        },
                        EvalToolCall {
                            tool: "bash".to_string(),
                            success: false,
                            output: "cargo test failed".to_string(),
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
}
