use super::*;

#[test]
fn matrix_covers_phase_12_required_scenarios() {
    assert_eq!(
        deterministic_scenarios().len(),
        REQUIRED_PHASE_12_KINDS.len()
    );
    assert!(
        matrix_missing_required_kinds().is_empty(),
        "missing required scenarios: {:?}",
        matrix_missing_required_kinds()
    );

    let ids = deterministic_scenarios()
        .iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for kind in REQUIRED_PHASE_12_KINDS {
        assert!(ids.contains(kind.id()), "missing id {}", kind.id());
    }
}

#[test]
fn each_scenario_requires_local_runtime_evidence() {
    for scenario in deterministic_scenarios() {
        assert!(
            scenario.evidence.iter().any(|evidence| {
                evidence.required
                    && matches!(
                        evidence.surface,
                        EvidenceSurface::TraceEvent
                            | EvidenceSurface::RuntimePanel
                            | EvidenceSurface::RecoveryPlan
                            | EvidenceSurface::ToolMetadata
                            | EvidenceSurface::SessionStore
                    )
            }),
            "{} should require at least one local runtime evidence surface",
            scenario.id
        );
        assert!(
            scenario.evidence.len() >= 3,
            "{} should have enough evidence to avoid a one-bit smoke test",
            scenario.id
        );
    }
}

#[test]
fn external_baseline_is_ready_after_replay_fixtures_land() {
    let summary = scenario_matrix_summary();
    assert_eq!(summary.replay_ready, 6);
    assert!(summary.external_baseline_ready);
    assert!(deterministic_scenarios()
        .iter()
        .all(|scenario| scenario.external_baseline == ExternalBaselineStatus::Ready));
}

#[test]
fn replay_ready_scenarios_are_tracked() {
    let replay_ready = deterministic_scenarios()
        .iter()
        .filter(|scenario| scenario.status == ReplayStatus::ReplayFixtureReady)
        .map(|scenario| scenario.kind)
        .collect::<Vec<_>>();

    assert_eq!(
        replay_ready,
        vec![
            ScenarioKind::FileEditRewind,
            ScenarioKind::BashBackgroundTask,
            ScenarioKind::PermissionDenialRetry,
            ScenarioKind::CompactionBoundary,
            ScenarioKind::SubagentWorktreeWorker,
            ScenarioKind::McpAuthRepair,
        ]
    );
}

#[test]
fn formatted_matrix_lists_all_cases_and_next_gate() {
    let rendered = format_scenario_matrix();
    for scenario in deterministic_scenarios() {
        assert!(rendered.contains(scenario.id), "missing {}", scenario.id);
    }
    assert!(rendered.contains("Next gate"));
    assert!(rendered.contains("External baseline: ready"));
    assert!(rendered.contains("/eval baseline-import"));
    assert!(rendered.contains("/eval parity"));
}

#[test]
fn p0a_runtime_spine_matrix_covers_core_scenarios() {
    assert_eq!(
        runtime_spine_p0a_cases().len(),
        REQUIRED_RUNTIME_SPINE_P0A_KINDS.len()
    );
    assert!(
        runtime_spine_missing_p0a_kinds().is_empty(),
        "missing P0a runtime spine cases: {:?}",
        runtime_spine_missing_p0a_kinds()
    );

    let ids = runtime_spine_p0a_cases()
        .iter()
        .map(|case| case.id)
        .collect::<BTreeSet<_>>();
    for kind in REQUIRED_RUNTIME_SPINE_P0A_KINDS {
        assert!(ids.contains(kind.id()), "missing id {}", kind.id());
    }
}

#[test]
fn p0b_runtime_spine_matrix_covers_extended_scenarios() {
    assert_eq!(
        runtime_spine_p0b_cases().len(),
        REQUIRED_RUNTIME_SPINE_P0B_KINDS.len()
    );
    assert!(
        runtime_spine_missing_p0b_kinds().is_empty(),
        "missing P0b runtime spine cases: {:?}",
        runtime_spine_missing_p0b_kinds()
    );

    let ids = runtime_spine_p0b_cases()
        .iter()
        .map(|case| case.id)
        .collect::<BTreeSet<_>>();
    for kind in REQUIRED_RUNTIME_SPINE_P0B_KINDS {
        assert!(ids.contains(kind.id()), "missing id {}", kind.id());
    }
}

#[test]
fn p0a_cases_have_oracles_for_route_tools_closeout_and_failures() {
    for case in runtime_spine_p0a_cases() {
        assert!(
            !case.expected.route.one_of.is_empty(),
            "{} should define route oracle",
            case.id
        );
        assert!(
            !case.expected.closeout.allowed_status.is_empty(),
            "{} should define closeout oracle",
            case.id
        );
        assert!(
            !case.failure_owner_if_failed.is_empty(),
            "{} should define failure owner candidates",
            case.id
        );
        assert!(
            !case.expected_gate_outcomes.is_empty(),
            "{} should define expected gate outcome classes",
            case.id
        );
        assert_eq!(
            case.golden_trace, CORE_GOLDEN_TRACE,
            "{} should require the standard golden trace surfaces",
            case.id
        );
        assert!(
            case.friction_budget.max_tool_rounds > 0,
            "{} should define a UX friction budget",
            case.id
        );
    }
}

#[test]
fn p0b_cases_have_oracles_for_complex_controls() {
    for case in runtime_spine_p0b_cases() {
        assert_eq!(
            case.phase,
            RuntimeSpinePhase::P0bExtended,
            "{} should be in the P0b phase",
            case.id
        );
        assert!(
            !case.expected.route.one_of.is_empty(),
            "{} should define route oracle",
            case.id
        );
        assert!(
            !case.expected.closeout.allowed_status.is_empty(),
            "{} should define closeout oracle",
            case.id
        );
        assert!(
            !case.failure_owner_if_failed.is_empty(),
            "{} should define failure owner candidates",
            case.id
        );
        assert!(
            !case.expected_gate_outcomes.is_empty(),
            "{} should define expected gate outcome classes",
            case.id
        );
        assert_eq!(
            case.golden_trace, CORE_GOLDEN_TRACE,
            "{} should require the standard golden trace surfaces",
            case.id
        );
        assert!(
            case.friction_budget.max_tool_rounds >= 3,
            "{} should define a realistic extended friction budget",
            case.id
        );
    }
}

#[test]
fn p0b_subagent_memory_and_skill_cases_do_not_silently_broaden_mutation() {
    for case in runtime_spine_p0b_cases() {
        match case.kind {
            RuntimeSpineCaseKind::SubagentVerifier
            | RuntimeSpineCaseKind::MemoryRetrievalConflict => {
                assert_eq!(
                    case.expected.mutation.expected_changed_files,
                    ExpectedChangedFiles::None,
                    "{} should remain read-only unless parent runtime records mutation",
                    case.id
                );
                assert!(
                    case.expected.tools.must_not_expose.contains(&"file_edit")
                        || case.expected.tools.must_not_expose.contains(&"file_patch"),
                    "{} should explicitly protect mutation surface",
                    case.id
                );
            }
            RuntimeSpineCaseKind::IsolatedWorktreeImplementer => {
                assert!(case.expected.tools.must_expose.contains(&"agent"));
                assert!(case
                    .expected
                    .validation
                    .accepted_families
                    .contains(&"parent_reviewed_subagent_patch"));
            }
            RuntimeSpineCaseKind::SkillGuidance => {
                assert!(case
                    .expected
                    .validation
                    .accepted_families
                    .contains(&"skill_guided_validation"));
                assert!(case.expected.validation.required);
            }
            _ => {}
        }
    }
}

#[test]
fn p0a_mutation_expectations_match_task_type() {
    for case in runtime_spine_p0a_cases() {
        match case.task_type {
            RuntimeSpineTaskType::DirectAnswer | RuntimeSpineTaskType::ReadOnlyAudit => {
                assert_eq!(
                    case.expected.mutation.expected_changed_files,
                    ExpectedChangedFiles::None,
                    "{} should not expect changed files",
                    case.id
                );
                assert!(
                    case.expected.tools.must_not_expose.contains(&"file_edit")
                        || case.expected.tools.must_not_expose.contains(&"file_patch"),
                    "{} should explicitly prevent edit surface",
                    case.id
                );
            }
            RuntimeSpineTaskType::CodeChange | RuntimeSpineTaskType::BugFix => {
                assert!(
                    case.expected.validation.required,
                    "{} should require validation",
                    case.id
                );
                assert!(
                    !case.expected.validation.accepted_families.is_empty(),
                    "{} should define accepted validation families",
                    case.id
                );
            }
        }
    }
}

#[test]
fn failure_owner_labels_are_normalized_for_existing_reports() {
    assert_eq!(FailureOwner::from_label("none"), Some(FailureOwner::None));
    assert_eq!(
        FailureOwner::from_label("llm_reasoning"),
        Some(FailureOwner::ModelPlanning)
    );
    assert_eq!(
        FailureOwner::from_label("eval_harness"),
        Some(FailureOwner::Harness)
    );
    assert_eq!(FailureOwner::ModelPlanning.label(), "model_planning");
}

#[test]
fn formatted_runtime_spine_matrix_lists_core_cases() {
    let rendered = format_runtime_spine_p0a_matrix();
    assert!(rendered.contains("P0a Runtime Spine"));
    for case in runtime_spine_p0a_cases() {
        assert!(rendered.contains(case.id), "missing {}", case.id);
    }
}

#[test]
fn formatted_runtime_spine_matrix_lists_extended_cases() {
    let rendered = format_runtime_spine_p0b_matrix();
    assert!(rendered.contains("P0b Runtime Spine"));
    for case in runtime_spine_p0b_cases() {
        assert!(rendered.contains(case.id), "missing {}", case.id);
    }
}
