use super::*;
use crate::engine::intent_router::{IntentKind, ReasoningPolicy, RetrievalPolicy, WorkflowKind};
use crate::engine::workflow_contract::{
    AcceptanceConfidence, AcceptanceContract, AcceptanceCriterion, AcceptanceNextAction,
    AcceptanceStatus, PriorityLabel, ProgrammingWorkflowJudgment, TaskComplexity, WorkflowPlanStep,
};
use crate::test_utils::env_guard::EnvVarGuard;

fn code_change_route(risk: RiskLevel) -> IntentRoute {
    IntentRoute {
        intent: IntentKind::CodeChange,
        confidence: 0.90,
        workflow: WorkflowKind::CodeChange,
        retrieval: RetrievalPolicy::Project,
        reasoning: ReasoningPolicy::Medium,
        risk,
        recommended_tools: Vec::new(),
        dependency_install_intent: false,
        mcp_auth_intent: false,
        reason: "test route".into(),
    }
}

fn audit_route(risk: RiskLevel) -> IntentRoute {
    IntentRoute {
        reason:
            "live coding audit/regression eval requires project verification; code diff is optional"
                .into(),
        ..code_change_route(risk)
    }
}

#[test]
fn policy_is_strict_for_high_risk_code() {
    let route = IntentRoute {
        intent: IntentKind::CodeChange,
        confidence: 0.95,
        workflow: WorkflowKind::CodeChange,
        retrieval: RetrievalPolicy::Project,
        reasoning: ReasoningPolicy::High,
        risk: RiskLevel::High,
        recommended_tools: Vec::new(),
        dependency_install_intent: false,
        mcp_auth_intent: false,
        reason: "test route".into(),
    };
    let policy = RiskSensitiveWorkflowPolicy::from_route_and_judgment(&route, None);

    assert_eq!(policy.depth, WorkflowDepth::Strict);
    assert!(policy.require_stage_validation);
    assert!(policy.reflection_blocks);
    assert_eq!(policy.max_repair_attempts, 3);
}

#[test]
fn runner_builds_failed_closeout_from_stage_validation() {
    let route = code_change_route(RiskLevel::Medium);
    let bundle = TaskContextBundle::new("修改 CLI 状态栏", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);
    runner.record_stage_validation(
        &bundle,
        &[PathBuf::from("src/main.rs")],
        false,
        &["cargo check failed".to_string()],
    );

    let closeout = runner.build_closeout(&bundle).unwrap();

    assert_eq!(closeout.status, StageValidationStatus::Failed);
    assert!(closeout
        .changed_files
        .iter()
        .any(|path| path == "src/main.rs"));
    assert!(closeout.format_for_final_response().contains("Closeout:"));
    assert!(runner
        .step_states()
        .iter()
        .any(|step| step.status == PlanStepRuntimeStatus::Failed));
}

#[test]
fn concise_closeout_keeps_short_validation_command_evidence() {
    let route = code_change_route(RiskLevel::Medium);
    let bundle = TaskContextBundle::new("创建 Python smoke 脚本", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);
    runner.activate_trigger(AdaptiveWorkflowTrigger::FirstCodeChange);
    runner.record_stage_validation(
        &bundle,
        &[PathBuf::from("hello.py")],
        true,
        &[
            "[Rust verification] cargo check passed with no issues.".to_string(),
            "[python verification] python3 -m py_compile hello.py passed with no issues."
                .to_string(),
        ],
    );

    let closeout = runner.build_closeout(&bundle).unwrap();
    let response = closeout.format_concise_for_final_response();

    assert!(response.contains("Verified:"));
    assert!(response.contains("python3 -m py_compile hello.py passed with no issues"));
}

#[test]
fn closeout_evidence_summary_counts_validation_and_acceptance_states() {
    let closeout = WorkflowCloseout {
        status: StageValidationStatus::Partial,
        risk: RiskLevel::Medium,
        changed_files: vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
        validation: vec![
            "compile: passed".to_string(),
            "unit tests: failed".to_string(),
            "lint: partial".to_string(),
            "docs: not_verified".to_string(),
        ],
        acceptance: vec![
            "accepted=true confidence=High unresolved=0".to_string(),
            "accepted=false confidence=Low unresolved=1".to_string(),
            "pending: user-facing behavior reviewed".to_string(),
        ],
        residual_risks: vec!["test failure unresolved".to_string()],
    };

    let summary = closeout.evidence_summary();

    assert!(summary.contains("changed_files=2"));
    assert!(summary.contains("validation_passed=1"));
    assert!(summary.contains("validation_failed=1"));
    assert!(summary.contains("validation_partial=1"));
    assert!(summary.contains("validation_not_verified=1"));
    assert!(summary.contains("acceptance_passed=1"));
    assert!(summary.contains("acceptance_rejected=1"));
    assert!(summary.contains("acceptance_pending=1"));
    assert!(closeout
        .format_for_final_response()
        .contains("- Evidence: changed_files=2"));
}

#[test]
fn lightweight_closeout_does_not_require_stage_validation_before_trigger() {
    let route = code_change_route(RiskLevel::Medium);
    let mut bundle = TaskContextBundle::new("修复 memory_save 质量门控", ".", route, None);
    bundle.add_acceptance_check("memory_save respects quality gates");
    let runner = CodeChangeWorkflowRunner::new(&bundle);

    let closeout = runner.build_closeout(&bundle).unwrap();

    assert_eq!(closeout.status, StageValidationStatus::NotVerified);
    assert!(closeout
        .validation
        .iter()
        .any(|item| item.contains("No file-change validation was required")));
    assert!(closeout
        .acceptance
        .iter()
        .any(|item| item.contains("pending: memory_save respects quality gates")));
    assert!(closeout
        .residual_risks
        .iter()
        .any(|item| item.contains("No changed files")));
    assert!(closeout
        .format_for_final_response()
        .contains("acceptance_pending=1"));
}

#[test]
fn audit_no_diff_closeout_can_pass_with_runtime_validation() {
    let route = audit_route(RiskLevel::High);
    let mut bundle = TaskContextBundle::new("审查 memory conflict precision", ".", route, None);
    bundle.add_acceptance_check("generic conflict words do not cause false matches");
    bundle.add_acceptance_check("required regression tests pass");
    let runner = CodeChangeWorkflowRunner::new(&bundle);

    let closeout = runner
        .build_closeout_with_runtime_validation(&bundle, Some("passed:3/3"))
        .unwrap();

    assert_eq!(closeout.status, StageValidationStatus::Passed);
    assert!(closeout.changed_files.is_empty());
    assert!(closeout
        .validation
        .iter()
        .any(|item| item == "required validation: passed (passed:3/3)"));
    assert!(closeout
        .acceptance
        .iter()
        .any(|item| item.contains("accepted=true")));
    assert_eq!(closeout.residual_risks, vec!["none recorded".to_string()]);
    let summary = closeout.evidence_summary();
    assert!(summary.contains("validation_passed=1"));
    assert!(summary.contains("acceptance_passed=1"));
    assert!(summary.contains("acceptance_pending=0"));
}

#[test]
fn runtime_validation_does_not_pass_seeded_code_change_without_diff() {
    let route = code_change_route(RiskLevel::High);
    let mut bundle = TaskContextBundle::new("实现缺失功能", ".", route, None);
    bundle.add_acceptance_check("feature is implemented");
    let runner = CodeChangeWorkflowRunner::new(&bundle);

    let closeout = runner
        .build_closeout_with_runtime_validation(&bundle, Some("passed:3/3"))
        .unwrap();

    assert_eq!(closeout.status, StageValidationStatus::NotVerified);
    assert!(closeout
        .residual_risks
        .iter()
        .any(|item| item.contains("No changed files")));
    assert!(closeout
        .format_for_final_response()
        .contains("acceptance_pending=1"));
}

#[test]
fn required_validation_trigger_requests_workflow_judgment() {
    let route = code_change_route(RiskLevel::Medium);
    let bundle = TaskContextBundle::new("修复 memory_save 质量门控", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);

    assert!(!runner.should_request_workflow_judgment());
    assert!(runner.activate_trigger(AdaptiveWorkflowTrigger::RequiredValidation));

    assert!(runner.should_request_workflow_judgment());
    assert!(runner.policy.require_stage_validation);
    assert_eq!(runner.policy.max_repair_attempts, 2);
    assert_eq!(
        runner.adaptive_trigger_labels(),
        vec!["required_validation"]
    );
}

#[test]
fn high_risk_signal_trigger_requests_workflow_judgment() {
    let route = code_change_route(RiskLevel::Medium);
    let bundle = TaskContextBundle::new("修改 provider runtime", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);

    assert!(!runner.should_request_workflow_judgment());
    assert!(runner.activate_trigger(AdaptiveWorkflowTrigger::RiskSignalHigh));

    assert!(runner.should_request_workflow_judgment());
    assert!(runner.policy.require_stage_validation);
    assert!(runner.policy.require_final_closeout);
    assert_eq!(runner.policy.max_repair_attempts, 2);
    assert_eq!(runner.adaptive_trigger_labels(), vec!["risk_signal_high"]);
}

#[test]
fn high_risk_signal_on_direct_task_does_not_require_code_validation() {
    let route = IntentRoute {
        intent: IntentKind::DirectAnswer,
        confidence: 0.90,
        workflow: WorkflowKind::Direct,
        retrieval: RetrievalPolicy::Project,
        reasoning: ReasoningPolicy::Medium,
        risk: RiskLevel::Low,
        recommended_tools: vec!["file_read".to_string(), "grep".to_string()],
        dependency_install_intent: false,
        mcp_auth_intent: false,
        reason: "read-only runtime inspection".into(),
    };
    let bundle = TaskContextBundle::new("检查工具循环实现，不修改文件", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);

    assert!(runner.activate_trigger(AdaptiveWorkflowTrigger::RiskSignalHigh));

    assert!(runner.should_request_workflow_judgment());
    assert!(!runner.policy.require_stage_validation);
    assert!(!runner.policy.require_final_closeout);
    assert_eq!(runner.policy.max_repair_attempts, 0);
    assert!(runner.build_closeout(&bundle).is_none());
    assert_eq!(runner.adaptive_trigger_labels(), vec!["risk_signal_high"]);
}

#[test]
fn verification_failure_trigger_promotes_to_strict_repair() {
    let route = code_change_route(RiskLevel::Medium);
    let bundle = TaskContextBundle::new("修复 memory_save 质量门控", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);

    assert!(runner.activate_trigger(AdaptiveWorkflowTrigger::VerificationFailed));

    assert_eq!(runner.policy.depth, WorkflowDepth::Strict);
    assert!(runner.policy.require_stage_validation);
    assert!(runner.policy.require_final_closeout);
    assert_eq!(runner.policy.max_repair_attempts, 3);
    assert!(runner.should_run_acceptance_review(false, true, false, 1));
}

#[test]
fn closeout_passes_only_with_change_validation_and_clean_acceptance() {
    let route = code_change_route(RiskLevel::Medium);
    let mut bundle = TaskContextBundle::new("修复 memory_save 质量门控", ".", route, None);
    bundle.add_acceptance_check("memory_save respects quality gates");
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);

    runner.record_stage_validation(
        &bundle,
        &[PathBuf::from("src/tools/memory_tool/mod.rs")],
        true,
        &["cargo test -q memory -- --test-threads=1 passed".to_string()],
    );
    runner.record_acceptance_review(AcceptanceReview {
        accepted: true,
        confidence: AcceptanceConfidence::High,
        criteria: Vec::new(),
        unresolved_items: Vec::new(),
        residual_risks: Vec::new(),
        next_action: AcceptanceNextAction::Finish,
    });

    let closeout = runner.build_closeout(&bundle).unwrap();

    assert_eq!(closeout.status, StageValidationStatus::Passed);
    assert_eq!(
        closeout.changed_files,
        vec!["src/tools/memory_tool/mod.rs".to_string()]
    );
    assert!(closeout
        .residual_risks
        .iter()
        .any(|item| item == "none recorded"));
}

#[test]
fn empty_validation_evidence_does_not_pass_code_change_closeout() {
    let route = code_change_route(RiskLevel::Medium);
    let bundle = TaskContextBundle::new("写一个 Python 脚本", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);

    let record = runner.record_stage_validation(&bundle, &[PathBuf::from("snake.py")], true, &[]);
    let closeout = runner.build_closeout(&bundle).unwrap();

    assert_eq!(record.status, StageValidationStatus::NotVerified);
    assert_eq!(closeout.status, StageValidationStatus::NotVerified);
    assert!(closeout
        .residual_risks
        .iter()
        .any(|item| item.contains("unresolved validation")));
}

#[test]
fn simple_passed_closeout_formats_concise_user_response_by_default() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_CLOSEOUT_VISIBILITY");
    let route = code_change_route(RiskLevel::Medium);
    let bundle = TaskContextBundle::new("写一个 Python 脚本", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);
    runner.record_stage_validation(
        &bundle,
        &[PathBuf::from("snake.py")],
        true,
        &["python3 -m py_compile snake.py passed".to_string()],
    );

    let closeout = runner.build_closeout(&bundle).unwrap();
    let response = closeout.format_for_user_response();

    assert_eq!(closeout.status, StageValidationStatus::Passed);
    assert_eq!(closeout.default_visibility(), CloseoutVisibility::Concise);
    assert!(response.contains("Done."));
    assert!(response.contains("Verified:"));
    assert!(!response.contains("Closeout:"));
}

#[test]
fn low_risk_not_verified_closeout_formats_concise_caveat_by_default() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_CLOSEOUT_VISIBILITY");
    let route = code_change_route(RiskLevel::Medium);
    let bundle = TaskContextBundle::new("写一个 Python 脚本", ".", route, None);
    let runner = CodeChangeWorkflowRunner::new(&bundle);

    let closeout = runner.build_closeout(&bundle).unwrap();
    let response = closeout.format_for_user_response();

    assert_eq!(closeout.status, StageValidationStatus::NotVerified);
    assert_eq!(closeout.default_visibility(), CloseoutVisibility::Concise);
    assert!(response.contains("Done with caveats."));
    assert!(response.contains("Not verified:"));
    assert!(!response.contains("Closeout:"));
}

#[test]
fn high_risk_passed_closeout_stays_full_by_default() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_CLOSEOUT_VISIBILITY");
    let route = code_change_route(RiskLevel::High);
    let bundle = TaskContextBundle::new("修改权限系统", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);
    runner.record_stage_validation(
        &bundle,
        &[PathBuf::from("src/permissions/mod.rs")],
        true,
        &["cargo test -q permissions passed".to_string()],
    );

    let closeout = runner.build_closeout(&bundle).unwrap();
    let response = closeout.format_for_user_response();

    assert_eq!(closeout.status, StageValidationStatus::Passed);
    assert_eq!(closeout.default_visibility(), CloseoutVisibility::Full);
    assert!(response.contains("Closeout:"));
    assert!(response.contains("Status: passed"));
}

#[test]
fn full_closeout_visibility_env_preserves_structured_output() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.set("PRIORITY_AGENT_CLOSEOUT_VISIBILITY", "full");
    let route = code_change_route(RiskLevel::Medium);
    let bundle = TaskContextBundle::new("写一个 Python 脚本", ".", route, None);
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);
    runner.record_stage_validation(
        &bundle,
        &[PathBuf::from("snake.py")],
        true,
        &["python3 -m py_compile snake.py passed".to_string()],
    );

    let closeout = runner.build_closeout(&bundle).unwrap();
    let response = closeout.format_for_user_response();

    assert!(response.contains("Closeout:"));
    assert!(response.contains("Status: passed"));
}

#[test]
fn closeout_uses_latest_acceptance_review_for_current_status() {
    let route = code_change_route(RiskLevel::Medium);
    let mut bundle = TaskContextBundle::new("修复 memory_save 质量门控", ".", route, None);
    bundle.add_acceptance_check("memory_save respects quality gates");
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);

    runner.record_stage_validation(
        &bundle,
        &[PathBuf::from("src/tools/memory_tool/mod.rs")],
        true,
        &["cargo test -q memory -- --test-threads=1 passed".to_string()],
    );
    runner.record_acceptance_review(AcceptanceReview {
        accepted: false,
        confidence: AcceptanceConfidence::Medium,
        criteria: Vec::new(),
        unresolved_items: vec!["initial review missed runtime save outcome".to_string()],
        residual_risks: vec!["format_memory_write_outcome not verified".to_string()],
        next_action: AcceptanceNextAction::ContinueRepair,
    });
    runner.record_acceptance_review(AcceptanceReview {
        accepted: true,
        confidence: AcceptanceConfidence::High,
        criteria: Vec::new(),
        unresolved_items: Vec::new(),
        residual_risks: Vec::new(),
        next_action: AcceptanceNextAction::Finish,
    });

    let closeout = runner.build_closeout(&bundle).unwrap();

    assert_eq!(closeout.status, StageValidationStatus::Passed);
    assert!(closeout
        .acceptance
        .iter()
        .any(|item| item.contains("accepted=false")));
    assert!(closeout
        .acceptance
        .iter()
        .any(|item| item.contains("accepted=true")));
    assert_eq!(closeout.residual_risks, vec!["none recorded".to_string()]);
}

#[test]
fn clean_acceptance_completes_remaining_plan_steps() {
    let route = code_change_route(RiskLevel::Medium);
    let mut bundle = TaskContextBundle::new("接入持久记忆到规划", ".", route, None);
    bundle.workflow_judgment = Some(ProgrammingWorkflowJudgment {
        task_type: "bug_fix".to_string(),
        complexity: TaskComplexity::Medium,
        risk: RiskLevel::Medium,
        requirement_complete_enough: true,
        needs_user_questions: false,
        question_reason: None,
        questions: Vec::new(),
        assumptions: Vec::new(),
        guided_reasoning_required: false,
        guided_reasoning_triggers: Vec::new(),
        plan: vec![
            WorkflowPlanStep {
                id: Some("inspect".to_string()),
                description: "Inspect memory retrieval integration".to_string(),
                priority: PriorityLabel::P0,
                weight: None,
                importance_score: None,
                weight_share: None,
                factors: None,
                override_adjustment: None,
                computation: None,
                reason: "Find current call order".to_string(),
                acceptance_criteria: Vec::new(),
            },
            WorkflowPlanStep {
                id: Some("wire".to_string()),
                description: "Add persistent memory prefetch before workflow judgment".to_string(),
                priority: PriorityLabel::P1,
                weight: None,
                importance_score: None,
                weight_share: None,
                factors: None,
                override_adjustment: None,
                computation: None,
                reason: "Planning must see memory signals".to_string(),
                acceptance_criteria: Vec::new(),
            },
        ],
        acceptance: AcceptanceContract {
            original_user_goal: "接入持久记忆到规划".to_string(),
            assumptions: Vec::new(),
            criteria: vec![AcceptanceCriterion {
                criterion: "required validation passes".to_string(),
                status: AcceptanceStatus::Passed,
                evidence: Some("cargo test passed".to_string()),
            }],
            unresolved_items: Vec::new(),
            residual_risks: Vec::new(),
        },
    });
    let mut runner = CodeChangeWorkflowRunner::new(&bundle);

    runner.record_stage_validation(
        &bundle,
        &[PathBuf::from("src/engine/conversation_loop/mod.rs")],
        true,
        &["required validation passed".to_string()],
    );
    runner.record_acceptance_review(AcceptanceReview {
        accepted: true,
        confidence: AcceptanceConfidence::High,
        criteria: Vec::new(),
        unresolved_items: Vec::new(),
        residual_risks: Vec::new(),
        next_action: AcceptanceNextAction::Finish,
    });

    let closeout = runner.build_closeout(&bundle).unwrap();

    assert_eq!(closeout.status, StageValidationStatus::Passed);
    assert!(runner
        .step_states()
        .iter()
        .all(|step| step.status == PlanStepRuntimeStatus::Passed));
}
