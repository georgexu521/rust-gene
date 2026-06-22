//! Scenario matrix reporting support.
//!
//! Builds diagnostic summaries for scenario coverage without changing runtime policy.

use super::*;

pub fn format_runtime_spine_p0a_matrix() -> String {
    let summary = runtime_spine_p0a_summary();
    let mut lines = vec![
        "P0a Runtime Spine Deterministic Matrix".to_string(),
        format!(
            "Scenarios: {}  Validation-required: {}  No-mutation: {}  Golden trace surfaces: {}",
            summary.scenarios,
            summary.cases_with_validation,
            summary.cases_requiring_no_mutation,
            summary.golden_trace_surfaces,
        ),
    ];
    for case in runtime_spine_p0a_cases() {
        lines.push(format!(
            "- {} [{}]: {}",
            case.id,
            case.phase.label(),
            case.title
        ));
        lines.push(format!(
            "  route: {:?} min_confidence={} closeout={:?}",
            case.expected.route.one_of,
            case.expected.route.min_confidence,
            case.expected.closeout.allowed_status
        ));
        lines.push(format!(
            "  tools: must={:?} may={:?} must_not={:?}",
            case.expected.tools.must_expose,
            case.expected.tools.may_expose,
            case.expected.tools.must_not_expose
        ));
    }
    lines.join("\n")
}

pub fn format_runtime_spine_p0b_matrix() -> String {
    let summary = runtime_spine_p0b_summary();
    let mut lines = vec![
        "P0b Runtime Spine Extended Matrix".to_string(),
        format!(
            "Scenarios: {}  Validation-required: {}  No-mutation: {}  Golden trace surfaces: {}",
            summary.scenarios,
            summary.cases_with_validation,
            summary.cases_requiring_no_mutation,
            summary.golden_trace_surfaces,
        ),
    ];
    for case in runtime_spine_p0b_cases() {
        lines.push(format!(
            "- {} [{}]: {}",
            case.id,
            case.phase.label(),
            case.title
        ));
        lines.push(format!(
            "  route: {:?} min_confidence={} closeout={:?}",
            case.expected.route.one_of,
            case.expected.route.min_confidence,
            case.expected.closeout.allowed_status
        ));
        lines.push(format!(
            "  failure_owners={:?} gates={:?}",
            case.failure_owner_if_failed, case.expected_gate_outcomes
        ));
    }
    lines.join("\n")
}

pub fn format_scenario_matrix() -> String {
    let summary = scenario_matrix_summary();
    let mut lines = vec![
        "Phase 12 Deterministic Scenario Matrix".to_string(),
        format!(
            "Scenarios: {}  Replay-ready: {}  Required evidence: {}  External baseline: {}",
            summary.scenarios,
            summary.replay_ready,
            summary.required_evidence,
            if summary.external_baseline_ready {
                "ready"
            } else {
                "deferred"
            }
        ),
    ];

    for scenario in deterministic_scenarios() {
        lines.push(format!(
            "- {} [{}]: {}",
            scenario.id,
            scenario.status.label(),
            scenario.title
        ));
        let evidence = scenario
            .evidence
            .iter()
            .map(|item| format!("{}={}", item.surface.label(), item.target))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("  evidence: {}", evidence));
    }

    lines.push("Next gate: generate real external Claude/Codex run artifacts, import them with /eval baseline-import, validate with /eval baseline-validate, then compare with /eval parity.".to_string());
    lines.join("\n")
}

pub fn matrix_missing_required_kinds() -> Vec<ScenarioKind> {
    let covered = deterministic_scenarios()
        .iter()
        .map(|scenario| scenario.kind)
        .collect::<BTreeSet<_>>();
    REQUIRED_PHASE_12_KINDS
        .iter()
        .copied()
        .filter(|kind| !covered.contains(kind))
        .collect()
}
