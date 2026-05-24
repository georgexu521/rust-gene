use crate::engine::action_decision::{ActionDecision, ActionDecisionInput};
use crate::engine::action_review::{
    ActionReview, ActionReviewDecision, ActionReviewInput, ActionReviewReason,
};
use crate::engine::context_assembly::{ContextAssemblyInput, ContextAssemblyPlan};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::intent_router::{IntentRouter, RiskLevel, WorkflowKind};
use crate::engine::stop_checker::{StopCheckInput, StopChecker};
use crate::engine::task_context::{
    AgentTaskStage, AgentToolRoundObservation, StopCheckReason, StopCheckStatus, TaskContextBundle,
    VerificationStatus,
};
use crate::engine::trace::{format_trace_summary, TraceCollector, TraceEvent, TurnTrace};
use crate::engine::verification_proof::{VerificationProofRequest, VerificationProofStatus};
use crate::services::api::ToolCall;
use crate::tools::{FileEditTool, ToolResult};
use std::collections::HashSet;

#[test]
fn runtime_spine_behavior_contract_covers_context_action_progress_stop_and_proof() {
    let context = ContextAssemblyPlan::new(ContextAssemblyInput {
        stable_prefix: "stable rules".to_string(),
        task_state: "stage=understand".to_string(),
        relevant_material: "src/lib.rs".to_string(),
        recent_observation: "read src/lib.rs".to_string(),
        current_decision_request: "edit src/lib.rs".to_string(),
    });
    let zone_names = context
        .zone_reports()
        .into_iter()
        .map(|zone| zone.name)
        .collect::<Vec<_>>();
    assert_eq!(
        zone_names,
        vec![
            "stable_prefix",
            "task_state",
            "relevant_material",
            "recent_observation",
            "current_decision_request",
        ]
    );

    let route = IntentRouter::new().route("修改 src/lib.rs 并运行 cargo test -q");
    let mut task_bundle = TaskContextBundle::new(
        "修改 src/lib.rs 并运行 cargo test -q",
        ".",
        route.clone(),
        None,
    );
    assert_eq!(task_bundle.agent_state.stage, AgentTaskStage::Understand);

    task_bundle
        .agent_state
        .observe_tool_round(AgentToolRoundObservation {
            any_tool_success: true,
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            has_worktree_changes: false,
            has_successful_validation_commands: false,
            failed_tool_evidence_present: false,
        });
    assert_eq!(task_bundle.agent_state.stage, AgentTaskStage::Edit);

    let edit_call = ToolCall {
        id: "call_edit".to_string(),
        name: "file_edit".to_string(),
        arguments: serde_json::json!({"path": "src/lib.rs"}),
    };
    let decision = ActionDecision::for_tool_call(
        &edit_call,
        ActionDecisionInput {
            task_stage: task_bundle.agent_state.stage,
            route_workflow: Some(route.workflow),
            route_risk: Some(route.risk),
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        },
    );
    assert!(decision.action.phase_aligned);
    assert!(decision.action.mutates_workspace);
    assert!(decision.verification_after.is_some());

    task_bundle
        .agent_state
        .observe_tool_round(AgentToolRoundObservation {
            any_tool_success: true,
            batch_has_unsuccessful_tools: false,
            used_write_tool: true,
            successful_write_tool: true,
            has_worktree_changes: true,
            has_successful_validation_commands: false,
            failed_tool_evidence_present: false,
        });
    assert_eq!(task_bundle.agent_state.stage, AgentTaskStage::Validate);
    assert!(task_bundle
        .agent_state
        .edit_snapshots
        .last()
        .is_some_and(|snapshot| snapshot.label == "tool round applied changes"));

    let stop = StopChecker::evaluate(StopCheckInput {
        any_tool_success: true,
        successful_write_tool: false,
        has_successful_validation_commands: false,
        no_code_progress_rounds: 2,
        action_checkpoint_active: true,
        action_checkpoint_no_change_rounds: 0,
        force_patch_synthesis_after_no_change: false,
        repeated_failed_tools: 0,
        duplicate_read_only_tools: 0,
    });
    assert_eq!(stop.status, StopCheckStatus::Checkpoint);
    assert_eq!(stop.reason, StopCheckReason::NoProgress);
    StopChecker::apply_to_task_state(&mut task_bundle.agent_state, &stop);
    assert_eq!(task_bundle.agent_state.stage, AgentTaskStage::Repair);
    assert!(task_bundle
        .agent_state
        .format_for_context_zone()
        .contains("Stop check: Checkpoint"));

    let required = vec!["cargo test -q".to_string()];
    let mut ledger = EvidenceLedger::new();
    let missing = ledger.verification_proof(VerificationProofRequest {
        required_commands: &required,
        requires_validation: true,
        task_verification_status: VerificationStatus::Pending,
    });
    assert_eq!(missing.status, VerificationProofStatus::NotRun);
    assert_eq!(
        missing.missing_required_commands,
        vec!["cargo test -q".to_string()]
    );

    ledger.record_validation_result("bash", Some("cargo test -q"), true, "tests passed");
    let verified = ledger.verification_proof(VerificationProofRequest {
        required_commands: &required,
        requires_validation: true,
        task_verification_status: VerificationStatus::Verified,
    });
    assert_eq!(verified.status, VerificationProofStatus::Verified);
    assert_eq!(verified.required_passed, 1);
}

#[test]
fn runtime_spine_behavior_contract_keeps_high_risk_phase_mismatch_visible() {
    let decision = ActionDecision::for_tool_call(
        &ToolCall {
            id: "call_edit".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({"path": "src/lib.rs"}),
        },
        ActionDecisionInput {
            task_stage: AgentTaskStage::Understand,
            route_workflow: Some(WorkflowKind::CodeChange),
            route_risk: Some(RiskLevel::High),
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        },
    );

    assert!(!decision.action.phase_aligned);
    assert!(decision.requires_confirmation);
    assert!(decision.trace_recommended);
    assert!(decision.scores.risk >= 8);
}

#[test]
fn runtime_spine_behavior_contract_has_typed_action_review_revise() {
    let tool_call = ToolCall {
        id: "call_missing".to_string(),
        name: "missing_tool".to_string(),
        arguments: serde_json::json!({}),
    };
    let action_decision = ActionDecision::for_tool_call(
        &tool_call,
        ActionDecisionInput {
            task_stage: AgentTaskStage::Understand,
            route_workflow: Some(WorkflowKind::CodeChange),
            route_risk: Some(RiskLevel::Medium),
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        },
    );
    let permission_context = crate::permissions::PermissionContext::new(".");
    let exposed_tool_names = HashSet::from(["file_read".to_string(), "grep".to_string()]);
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: None,
        exposed_tool_names: &exposed_tool_names,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision,
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_eq!(review.decision, ActionReviewDecision::Revise);
    assert_eq!(review.primary_reason, ActionReviewReason::ToolNotAvailable);
    assert!(review.model_recovery.contains("tool_not_available"));
}

#[test]
fn runtime_spine_behavior_contract_covers_checkpoint_and_observation_signals() {
    let route = IntentRouter::new().route("修改 src/lib.rs");
    let tool_call = ToolCall {
        id: "call_edit".to_string(),
        name: "file_edit".to_string(),
        arguments: serde_json::json!({
            "path": "src/lib.rs",
            "old_string": "old",
            "new_string": "new"
        }),
    };
    let action_decision = ActionDecision::for_tool_call(
        &tool_call,
        ActionDecisionInput {
            task_stage: AgentTaskStage::Edit,
            route_workflow: Some(WorkflowKind::CodeChange),
            route_risk: Some(route.risk),
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        },
    );
    let permission_context = crate::permissions::PermissionContext::new(".");
    let exposed_tool_names = HashSet::from(["file_edit".to_string()]);
    let tool = FileEditTool;
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed_tool_names,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision,
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_eq!(review.checkpoint.status, "required_and_present");
    assert_eq!(review.checkpoint.rollback_scope, "local_files");

    let mut task_bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
    let result = ToolResult::success_with_data(
        "edited",
        serde_json::json!({
            "path": "src/lib.rs",
            "checkpoint": {"id": "cp_runtime_1"},
            "tool_observation": {
                "schema": "tool_observation.v1",
                "tool": "file_edit",
                "call_id": "call_edit",
                "status": "success",
                "summary": "file_edit succeeded: edited src/lib.rs",
                "files_read": [],
                "files_changed": ["src/lib.rs"],
                "command_run": null,
                "validation_result": null,
                "permission_decision": null,
                "checkpoint_id": "cp_runtime_1",
                "artifact_path": null,
                "state_updates": ["files_changed", "checkpoint"],
                "recommended_next_action": null
            }
        }),
    );

    let observed = task_bundle
        .agent_state
        .observe_tool_context_evidence(&tool_call, &result);
    assert_eq!(observed, 2);
    assert!(task_bundle
        .agent_state
        .format_for_context_zone()
        .contains("tool_observation"));

    let trace = TraceCollector::new(TurnTrace::new("session", 1, "runtime spine"));
    trace.record(TraceEvent::ActionReviewed {
        tool: "file_edit".to_string(),
        call_id: "call_edit".to_string(),
        decision: "allow".to_string(),
        reason: "safe_to_execute".to_string(),
        permission: Some("allow".to_string()),
        scope_allowed: true,
        budget_allowed: true,
        checkpoint: "required_and_present".to_string(),
        network: "none".to_string(),
        external_effect: "local_workspace_mutation".to_string(),
        recovery: "use the observation after execution".to_string(),
    });
    trace.record(TraceEvent::ToolObservationRecorded {
        tool: "file_edit".to_string(),
        call_id: "call_edit".to_string(),
        status: "success".to_string(),
        files_read: 0,
        files_changed: 1,
        checkpoint_id: Some("cp_runtime_1".to_string()),
        summary: "file_edit succeeded: edited src/lib.rs".to_string(),
    });
    let summary = format_trace_summary(&trace.snapshot(), 10);
    assert!(summary.contains("action review"));
    assert!(summary.contains("observation: status=success"));
}
