use super::*;
use crate::engine::code_change_workflow::StageValidationStatus;
use crate::engine::intent_router::{
    IntentKind, IntentRoute, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
};
use crate::engine::task_context::{StopAction, StopCheckReason, StopCheckRecord, StopCheckStatus};
use crate::engine::trace::{TurnStatus, TurnTrace};
use crate::engine::verification_proof::VerificationProofKind;
use crate::test_utils::env_guard::EnvVarGuard;
use std::path::PathBuf;
use std::time::{Duration, Instant};
fn audit_route() -> IntentRoute {
    IntentRoute {
        intent: IntentKind::CodeChange,
        confidence: 0.90,
        workflow: WorkflowKind::CodeChange,
        retrieval: RetrievalPolicy::Project,
        reasoning: ReasoningPolicy::Medium,
        risk: RiskLevel::High,
        recommended_tools: Vec::new(),
        dependency_install_intent: false,
        mcp_auth_intent: false,
        reason: "audit/regression eval requires project verification; code diff is optional"
            .to_string(),
    }
}

fn direct_route() -> IntentRoute {
    IntentRoute {
        intent: IntentKind::DirectAnswer,
        confidence: 0.90,
        workflow: WorkflowKind::Direct,
        retrieval: RetrievalPolicy::Light,
        reasoning: ReasoningPolicy::Low,
        risk: RiskLevel::Low,
        recommended_tools: Vec::new(),
        dependency_install_intent: false,
        mcp_auth_intent: false,
        reason: "direct read-only evidence task".to_string(),
    }
}

fn code_change_route() -> IntentRoute {
    IntentRoute {
        intent: IntentKind::CodeChange,
        confidence: 0.90,
        workflow: WorkflowKind::CodeChange,
        retrieval: RetrievalPolicy::Project,
        reasoning: ReasoningPolicy::Medium,
        risk: RiskLevel::High,
        recommended_tools: Vec::new(),
        dependency_install_intent: false,
        mcp_auth_intent: false,
        reason: "normal code change requires project verification".to_string(),
    }
}

fn isolate_project_memory_stores(env: &mut EnvVarGuard, store_dir: &tempfile::TempDir) {
    env.set(
        "PRIORITY_AGENT_PROJECT_PROGRESS_PATH",
        store_dir
            .path()
            .join("project_progress.jsonl")
            .to_str()
            .unwrap(),
    );
    env.set(
        "PRIORITY_AGENT_MEMORY_PROPOSALS_PATH",
        store_dir
            .path()
            .join("memory_proposals.jsonl")
            .to_str()
            .unwrap(),
    );
}

#[tokio::test]
async fn closeout_background_stage_timeout_does_not_block_closeout() {
    let trace = TraceCollector::new(TurnTrace::new("session", 1, "closeout timeout"));
    let started = Instant::now();
    let result = run_closeout_background_stage(
        trace.clone(),
        "test_timeout",
        Duration::from_millis(20),
        || {
            std::thread::sleep(Duration::from_millis(250));
            Ok(())
        },
    )
    .await;

    assert!(result.is_none());
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "closeout timeout should return before the blocking work finishes"
    );
    assert!(trace.snapshot().events.iter().any(|event| matches!(
        event,
        TraceEvent::CloseoutBackgroundStage { status, .. } if status == "timed_out"
    )));
}

#[test]
fn evaluator_uses_ledger_runtime_validation_for_no_diff_audit_closeout() {
    let mut bundle = TaskContextBundle::new("审查已有实现", ".", audit_route(), None);
    bundle.add_acceptance_check("required regression checks pass");
    let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
    let mut evidence_ledger = EvidenceLedger::new();
    evidence_ledger.record_validation_result(
        "required_validation",
        Some("cargo test -q memory"),
        true,
        "cargo test -q memory passed",
    );

    let required_commands = vec!["cargo test -q memory".to_string()];
    let evaluation = CloseoutEvaluator::evaluate(
        &code_workflow,
        &bundle,
        &evidence_ledger,
        &required_commands,
    );
    let closeout = evaluation.closeout.expect("closeout");

    assert_eq!(
        evaluation.runtime_validation_label.as_deref(),
        Some("passed:1/1")
    );
    assert_eq!(closeout.status, StageValidationStatus::Passed);
    assert!(closeout.changed_files.is_empty());
    assert!(closeout
        .validation
        .iter()
        .any(|item| item == "required validation: passed (passed:1/1)"));
}

#[test]
fn evaluator_downgrades_verified_status_when_proof_kind_support_is_partial() {
    let mut bundle = TaskContextBundle::new("审查已有 diff", ".", audit_route(), None);
    bundle.add_acceptance_check("diff was reviewed");
    let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
    let mut evidence_ledger = EvidenceLedger::new();
    evidence_ledger.record_validation_result_with_kind(
        "code_review",
        None,
        true,
        "diff reviewed",
        Some(VerificationProofKind::DiffReviewed),
    );

    let evaluation = CloseoutEvaluator::evaluate(&code_workflow, &bundle, &evidence_ledger, &[]);
    let closeout = evaluation.closeout.expect("closeout");

    assert_eq!(
        evaluation.verification_proof.status,
        VerificationProofStatus::Verified
    );
    assert_eq!(
        evaluation.verification_proof.derived_support.status,
        VerificationProofStatus::Partial
    );
    assert_eq!(closeout.status, StageValidationStatus::Partial);
    assert!(closeout
        .validation
        .iter()
        .any(|item| item.contains("verification proof support: partial")));
}

#[tokio::test]
async fn mva_profile_adds_structured_closeout_for_direct_tool_turn() {
    let mut env = EnvVarGuard::acquire().await;
    let store_dir = tempfile::tempdir().unwrap();
    isolate_project_memory_stores(&mut env, &store_dir);
    env.set("PRIORITY_AGENT_RUNTIME_PROFILE", "minimum_viable_agent");
    let bundle = TaskContextBundle::new("inspect one known file", ".", direct_route(), None);
    let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
    let trace = TraceCollector::new(TurnTrace::new("session", 1, "inspect"));
    let mut runtime_diet = RuntimeDietSnapshot::new(true);
    let tool_call = ToolCall {
        id: "call-1".to_string(),
        name: "file_read".to_string(),
        arguments: serde_json::json!({"path": "fixtures/known.txt"}),
    };
    let mut evidence_ledger = EvidenceLedger::new();
    evidence_ledger
        .record_tool_result(&tool_call, &crate::tools::ToolResult::success("known fact"));
    let mut final_content = "Observed known fact.".to_string();

    FinalCloseoutController::apply_final_closeout(FinalCloseoutContext {
        trace: &trace,
        code_workflow: &code_workflow,
        task_bundle: &bundle,
        required_validation_commands: &[],
        runtime_diet: &mut runtime_diet,
        final_content: &mut final_content,
        final_tool_calls: &[],
        iterations_used: 1,
        max_iterations: 10,
        evidence_ledger: &evidence_ledger,
        settlement_gaps: &[],
        memory_generate_enabled: true,
        tx: None,
    })
    .await;

    assert!(final_content.contains("Closeout:"));
    assert!(final_content.contains("- Status: passed"));
    assert!(final_content.contains("- Changed: none"));
    assert!(final_content.contains("target: inspect one known file"));
    assert_eq!(runtime_diet.validation_evidence, "not_applicable");

    let finished = trace.finish(TurnStatus::Completed);
    assert!(finished.events.iter().any(|event| matches!(
        event,
        TraceEvent::FinalCloseoutPrepared {
            status,
            changed_files: 0,
            tool_records: 1,
            ..
        } if status == "passed"
    )));
    assert!(finished.events.iter().any(|event| matches!(
        event,
        TraceEvent::ExecutionReportPrepared {
            status,
            changed_files: 0,
            validation_evidence,
            ..
        } if status == "success" && *validation_evidence > 0
    )));
    assert!(finished.events.iter().any(|event| matches!(
        event,
        TraceEvent::MemoryProposalPrepared {
            status,
            candidates: 0,
            write_performed: false,
            ..
        } if status == "not_applicable"
    )));
}

#[tokio::test]
async fn project_partner_profile_adds_direct_execution_report_for_read_only_turn() {
    let mut env = EnvVarGuard::acquire().await;
    let store_dir = tempfile::tempdir().unwrap();
    isolate_project_memory_stores(&mut env, &store_dir);
    env.set(
        "PRIORITY_AGENT_RUNTIME_PROFILE",
        "project_partner_alignment",
    );
    let bundle = TaskContextBundle::new(
        "Resume project from memory and previous execution report",
        ".",
        direct_route(),
        None,
    );
    let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
    let trace = TraceCollector::new(TurnTrace::new("session", 1, "resume"));
    let mut runtime_diet = RuntimeDietSnapshot::new(true);
    let mut evidence_ledger = EvidenceLedger::new();
    evidence_ledger.record_tool_result(
        &ToolCall {
            id: "call-1".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "memory/project.md"}),
        },
        &crate::tools::ToolResult::success("Project Memory: CSV export is next"),
    );
    let mut final_content = "Current state: CSV export is next.".to_string();

    FinalCloseoutController::apply_final_closeout(FinalCloseoutContext {
        trace: &trace,
        code_workflow: &code_workflow,
        task_bundle: &bundle,
        required_validation_commands: &[],
        runtime_diet: &mut runtime_diet,
        final_content: &mut final_content,
        final_tool_calls: &[],
        iterations_used: 1,
        max_iterations: 10,
        evidence_ledger: &evidence_ledger,
        settlement_gaps: &[],
        memory_generate_enabled: true,
        tx: None,
    })
    .await;

    assert!(final_content.contains("Closeout:"));
    assert!(!final_content.contains("ExecutionReport"));

    let finished = trace.finish(TurnStatus::Completed);
    assert!(finished.events.iter().any(|event| matches!(
        event,
        TraceEvent::ExecutionReportPrepared {
            status,
            changed_files: 0,
            validation_evidence,
            ..
        } if status == "success" && *validation_evidence > 0
    )));
}

#[tokio::test]
async fn project_partner_profile_surfaces_review_only_memory_proposal() {
    let mut env = EnvVarGuard::acquire().await;
    let store_dir = tempfile::tempdir().unwrap();
    isolate_project_memory_stores(&mut env, &store_dir);
    env.set(
        "PRIORITY_AGENT_RUNTIME_PROFILE",
        "project_partner_alignment",
    );
    let mut bundle = TaskContextBundle::new(
        "Fix slugify and surface a memory proposal",
        ".",
        audit_route(),
        None,
    );
    bundle.add_acceptance_check("slugify test passes");
    let mut code_workflow = CodeChangeWorkflowRunner::new(&bundle);
    let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix"));
    let mut runtime_diet = RuntimeDietSnapshot::new(true);
    let mut evidence_ledger = EvidenceLedger::new();
    evidence_ledger.record_tool_result(
        &ToolCall {
            id: "call-1".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({"path": "fixtures/project_partner_failure/slugify.py"}),
        },
        &crate::tools::ToolResult::success("File edited successfully"),
    );
    evidence_ledger.record_validation_result(
        "required_validation",
        Some("python3 fixtures/project_partner_failure/test_slugify.py"),
        true,
        "slugify test passed",
    );
    code_workflow.record_stage_validation(
        &bundle,
        &[PathBuf::from("fixtures/project_partner_failure/slugify.py")],
        true,
        &["python3 fixtures/project_partner_failure/test_slugify.py passed".to_string()],
    );
    let required_commands =
        vec!["python3 fixtures/project_partner_failure/test_slugify.py".to_string()];
    let mut final_content = "Fixed slugify.".to_string();

    FinalCloseoutController::apply_final_closeout(FinalCloseoutContext {
        trace: &trace,
        code_workflow: &code_workflow,
        task_bundle: &bundle,
        required_validation_commands: &required_commands,
        runtime_diet: &mut runtime_diet,
        final_content: &mut final_content,
        final_tool_calls: &[],
        iterations_used: 1,
        max_iterations: 10,
        evidence_ledger: &evidence_ledger,
        settlement_gaps: &[],
        memory_generate_enabled: true,
        tx: None,
    })
    .await;

    assert!(final_content.contains("Memory proposal:"));
    assert!(final_content.contains("write_performed=false"));
    assert!(final_content.contains("scope=project"));
    assert!(final_content.contains("evidence="));

    let finished = trace.finish(TurnStatus::Completed);
    assert!(finished.events.iter().any(|event| matches!(
        event,
        TraceEvent::MemoryProposalPrepared {
            status,
            candidates: 1,
            write_performed: false,
            ..
        } if status == "proposed"
    )));
}

#[tokio::test]
async fn mva_direct_closeout_preserves_low_value_stop_target() {
    let mut env = EnvVarGuard::acquire().await;
    let store_dir = tempfile::tempdir().unwrap();
    isolate_project_memory_stores(&mut env, &store_dir);
    env.set("PRIORITY_AGENT_RUNTIME_PROFILE", "minimum_viable_agent");
    let mut bundle = TaskContextBundle::new(
        "在 fixtures/mva_low_value_replan 里找到 missing-target-token-7391",
        ".",
        direct_route(),
        None,
    );
    bundle.agent_state.record_stop_check(StopCheckRecord {
        status: StopCheckStatus::Stop,
        terminal_status: None,
        action: StopAction::Closeout,
        reason: StopCheckReason::DuplicateReadOnly,
        summary: "duplicate read-only calls would not change task state".to_string(),
        evidence: vec!["checked fixtures/mva_low_value_replan/known.txt".to_string()],
        failure_type: Some("duplicate_read_only".to_string()),
        recovery_plan_id: None,
        rollback_candidate: None,
        next_action: Some("close out with the already-observed evidence".to_string()),
        no_code_progress_rounds: 0,
        action_checkpoint_active: false,
    });
    let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
    let trace = TraceCollector::new(TurnTrace::new("session", 1, "low value"));
    let mut runtime_diet = RuntimeDietSnapshot::new(true);
    let mut evidence_ledger = EvidenceLedger::new();
    evidence_ledger.record_tool_result(
        &ToolCall {
            id: "call-1".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "fixtures/mva_low_value_replan/known.txt"}),
        },
        &crate::tools::ToolResult::success("known fact"),
    );
    let mut final_content = "No matching token was found in the checked file.".to_string();

    FinalCloseoutController::apply_final_closeout(FinalCloseoutContext {
        trace: &trace,
        code_workflow: &code_workflow,
        task_bundle: &bundle,
        required_validation_commands: &[],
        runtime_diet: &mut runtime_diet,
        final_content: &mut final_content,
        final_tool_calls: &[],
        iterations_used: 1,
        max_iterations: 10,
        evidence_ledger: &evidence_ledger,
        settlement_gaps: &[],
        memory_generate_enabled: true,
        tx: None,
    })
    .await;

    assert!(final_content.contains("missing-target-token-7391"));
    assert!(final_content.contains("stop: reason=duplicate_read_only"));
    assert!(final_content.contains("checked evidence:"));
}

#[test]
fn evaluator_prefers_required_command_success_over_exploratory_validation_failure() {
    let mut bundle = TaskContextBundle::new("检查 Python 包安装", ".", audit_route(), None);
    bundle.add_acceptance_check("test -x .venv/bin/python returns success");
    bundle.add_acceptance_check(
        "python -m core_terminal_demo --self-test outputs core-terminal-demo-ok",
    );
    let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
    let mut evidence_ledger = EvidenceLedger::new();
    evidence_ledger.record_validation_result(
        "bash",
        Some("python3 -c \"import core_terminal_demo\""),
        false,
        "ModuleNotFoundError",
    );
    evidence_ledger.record_tool_result(
        &ToolCall {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "test -x .venv/bin/python && echo PASS"
            }),
        },
        &crate::tools::ToolResult::success("PASS"),
    );
    evidence_ledger.record_tool_result(
            &ToolCall {
                id: "call_2".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({
                    "command": ". .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'"
                }),
            },
            &crate::tools::ToolResult::success("core-terminal-demo-ok"),
        );
    let required_commands = vec![
            "test -x .venv/bin/python".to_string(),
            ". .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'".to_string(),
        ];
    let evaluation = CloseoutEvaluator::evaluate(
        &code_workflow,
        &bundle,
        &evidence_ledger,
        &required_commands,
    );
    let closeout = evaluation.closeout.expect("closeout");
    assert_eq!(
        evaluation.runtime_validation_label.as_deref(),
        Some("passed:2/2")
    );
    assert!(evaluation
        .tool_evidence_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("tool evidence: records=2")));
    assert_eq!(closeout.status, StageValidationStatus::Passed);
    assert!(closeout
        .validation
        .iter()
        .any(|item| item.contains("tool evidence: records=2")));
    assert!(closeout.acceptance.iter().any(|item| {
        item.contains("accepted=true") && item.contains("required validation passed")
    }));
    assert_eq!(
        evaluation.verification_proof.status,
        VerificationProofStatus::Verified
    );
    assert!(closeout.validation.iter().any(|item| {
        item.contains("verification proof: verified (required validation passed 2/2 commands)")
    }));
}

#[test]
fn evaluator_records_not_run_proof_when_required_validation_is_missing() {
    let mut bundle = TaskContextBundle::new("审查已有实现", ".", audit_route(), None);
    bundle.add_acceptance_check("required regression checks pass");
    let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
    let evidence_ledger = EvidenceLedger::new();
    let required_commands = vec!["cargo test -q memory".to_string()];
    let evaluation = CloseoutEvaluator::evaluate(
        &code_workflow,
        &bundle,
        &evidence_ledger,
        &required_commands,
    );
    let closeout = evaluation.closeout.expect("closeout");
    assert_eq!(
        evaluation.verification_proof.status,
        VerificationProofStatus::NotRun
    );
    assert_eq!(closeout.status, StageValidationStatus::NotVerified);
    assert!(closeout
        .validation
        .iter()
        .any(|item| item.contains("verification proof: not_run")));
    assert!(closeout
        .residual_risks
        .iter()
        .any(|item| item.contains("Verification proof is not_run")));
}

#[test]
fn evaluator_promotes_bash_changed_code_with_verified_required_validation() {
    let mut bundle = TaskContextBundle::new("实现一个小功能", ".", code_change_route(), None);
    bundle.add_acceptance_check("required validation command: python3 -m unittest test_app.py");
    let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
    let mut evidence_ledger = EvidenceLedger::new();
    evidence_ledger.record_tool_result(
        &ToolCall {
            id: "write_app".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "cat > src/app.py <<'PY'\nprint('ok')\nPY"
            }),
        },
        &crate::tools::ToolResult::success("wrote app.py"),
    );
    evidence_ledger.record_tool_result(
        &ToolCall {
            id: "run_tests".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "python3 -m unittest test_app.py"
            }),
        },
        &crate::tools::ToolResult::success("Ran 3 tests in 1.0s\n\nOK"),
    );
    let required_commands = vec!["python3 -m unittest test_app.py".to_string()];
    let evaluation = CloseoutEvaluator::evaluate(
        &code_workflow,
        &bundle,
        &evidence_ledger,
        &required_commands,
    );
    let closeout = evaluation.closeout.expect("closeout");
    assert_eq!(closeout.status, StageValidationStatus::Passed);
    assert!(closeout
        .changed_files
        .iter()
        .any(|path| path == "src/app.py"));
    assert!(closeout
        .validation
        .iter()
        .any(|item| item.contains("verification proof: verified")));
    assert!(closeout.acceptance.iter().any(|item| {
        item.contains("accepted=true") && item.contains("required validation passed")
    }));
    assert_eq!(closeout.residual_risks, vec!["none recorded".to_string()]);
}

#[test]
fn verified_change_closeout_records_trace_only_when_ready() {
    let trace = TraceCollector::new(crate::engine::trace::TurnTrace::new("session", 1, "change"));
    assert!(!VerifiedChangeCloseoutController::should_break_for_verified_change(&trace, false,));
    assert!(VerifiedChangeCloseoutController::should_break_for_verified_change(&trace, true,));
    let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
    let matching_events = finished
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                TraceEvent::WorkflowFallback { error }
                    if error == VerifiedChangeCloseoutController::VERIFIED_CHANGE_CLOSEOUT_TRACE
            )
        })
        .count();
    assert_eq!(matching_events, 1);
}
