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
            failures.extend(self.check_scenario(scenario, &route));
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

    fn check_scenario(&self, scenario: &EvalScenario, route: &IntentRoute) -> Vec<EvalFailure> {
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
            let trace = trace_from_route("eval", scenario, route);
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

        failures
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
    trace
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
    fn bundled_smoke_evalset_passes() {
        let path = std::path::Path::new("evalsets/smoke.yaml");
        if !path.exists() {
            return;
        }
        let set = load_evalset(path).unwrap();
        let report = EvalRunner::new().run_set(&set);
        assert!(report.ok(), "{}", report.summary());
    }
}
