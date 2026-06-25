use super::*;
use crate::engine::action_decision::{
    ActionDecision, ActionDecisionInput, ActionScoreModifier, ActionScoreModifierSource,
};
use crate::engine::intent_router::{RiskLevel, WorkflowKind};
use crate::engine::task_context::{AgentTaskStage, TaskContextBundle};
use crate::lab::model::LabRole;
use crate::lab::orchestrator::LabOrchestrator;
use crate::tools::{
    BashTool, FormatTool, GitTool, InstallDependenciesTool, ToolContext, ToolResult,
};
use async_trait::async_trait;

struct RequiredPathTool;

#[async_trait]
impl Tool for RequiredPathTool {
    fn name(&self) -> &str {
        "required_path"
    }

    fn description(&self) -> &str {
        "test tool"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
        ToolResult::success("unused")
    }
}

struct ConfirmTool;

#[async_trait]
impl Tool for ConfirmTool {
    fn name(&self) -> &str {
        "confirm_tool"
    }

    fn description(&self) -> &str {
        "test confirmation tool"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({"type": "object", "properties": {}})
    }

    fn requires_confirmation(&self, _params: &Value) -> bool {
        true
    }

    async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
        ToolResult::success("unused")
    }
}

struct FileEditLikeTool;

#[async_trait]
impl Tool for FileEditLikeTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "test file edit tool"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "new_string": { "type": "string" }
            },
            "required": ["path", "new_string"]
        })
    }

    async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
        ToolResult::success("unused")
    }
}

fn call(name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        id: "call_1".to_string(),
        name: name.to_string(),
        arguments,
    }
}

fn decision(tool_call: &ToolCall) -> ActionDecision {
    ActionDecision::for_tool_call(
        tool_call,
        ActionDecisionInput {
            task_stage: AgentTaskStage::Understand,
            route_workflow: None,
            route_risk: None,
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        },
    )
}

fn code_change_decision(tool_call: &ToolCall, stage: AgentTaskStage) -> ActionDecision {
    ActionDecision::for_tool_call(
        tool_call,
        ActionDecisionInput {
            task_stage: stage,
            route_workflow: Some(WorkflowKind::CodeChange),
            route_risk: Some(RiskLevel::Medium),
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        },
    )
}

fn understand_state() -> crate::engine::task_context::AgentTaskState {
    let route = crate::engine::intent_router::IntentRouter::new().route("edit src/lib.rs");
    TaskContextBundle::new("edit src/lib.rs", ".", route, None).agent_state
}

fn review(tool_call: &ToolCall, tool: Option<&dyn Tool>, exposed: HashSet<String>) -> ActionReview {
    let permission_context = PermissionContext::new(".");
    ActionReview::build(ActionReviewInput {
        tool_call,
        tool,
        exposed_tool_names: &exposed,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision: decision(tool_call),
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    })
}

#[test]
fn missing_tool_is_typed_revise() {
    let tool_call = call("missing_tool", serde_json::json!({}));
    let exposed = HashSet::from(["file_read".to_string()]);
    let review = review(&tool_call, None, exposed);

    assert_eq!(review.decision, ActionReviewDecision::Revise);
    assert_eq!(review.primary_reason, ActionReviewReason::ToolNotAvailable);
    assert!(!review.tool_contract.available);
    assert!(review.model_recovery.contains("tool_not_available"));
}

#[test]
fn unexposed_tool_is_typed_revise() {
    let tool = RequiredPathTool;
    let tool_call = call("required_path", serde_json::json!({"path": "src/lib.rs"}));
    let review = review(&tool_call, Some(&tool), HashSet::new());

    assert_eq!(review.decision, ActionReviewDecision::Revise);
    assert_eq!(review.primary_reason, ActionReviewReason::ToolNotExposed);
    assert!(review.tool_contract.available);
    assert!(!review.tool_contract.exposed);
}

#[test]
fn invalid_arguments_are_typed_revise() {
    let tool = RequiredPathTool;
    let tool_call = call("required_path", serde_json::json!({}));
    let exposed = HashSet::from(["required_path".to_string()]);
    let review = review(&tool_call, Some(&tool), exposed);

    assert_eq!(review.decision, ActionReviewDecision::Revise);
    assert_eq!(review.primary_reason, ActionReviewReason::InvalidArguments);
    assert!(review.tool_contract.validation_error.is_some());
}

#[test]
fn confirmation_tool_is_typed_ask_user() {
    let tool = ConfirmTool;
    let tool_call = call("confirm_tool", serde_json::json!({}));
    let exposed = HashSet::from(["confirm_tool".to_string()]);
    let mut permission_context = PermissionContext::new(".");
    permission_context.mode = crate::permissions::PermissionMode::AutoLowRisk;
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision: decision(&tool_call),
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_eq!(review.decision, ActionReviewDecision::AskUser);
    assert!(review.permission.requires_confirmation);
    assert_eq!(
        review.primary_reason,
        ActionReviewReason::PermissionRequired
    );
}

#[test]
fn labrun_policy_blocks_postdoc_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_plan".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();

    let tool = FileEditLikeTool;
    let tool_call = call(
        "file_edit",
        serde_json::json!({"path": "src/lab/model.rs", "new_string": "changed"}),
    );
    let exposed = HashSet::from(["file_edit".to_string()]);
    let mut permission_context = PermissionContext::new(temp.path());
    permission_context.mode = crate::permissions::PermissionMode::AutoAll;

    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision: decision(&tool_call),
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: Some(temp.path()),
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_eq!(review.decision, ActionReviewDecision::Deny);
    assert_eq!(
        review.primary_reason,
        ActionReviewReason::LabRunPolicyViolation
    );
    assert!(review.labrun_policy.applies);
    assert!(!review.labrun_policy.allowed);
    assert_eq!(review.labrun_policy.role.as_deref(), Some("Postdoc"));
}

#[test]
fn explicit_permission_deny_is_typed_deny_not_approval() {
    let tool = BashTool;
    let tool_call = call("bash", serde_json::json!({"command": "cargo test -q"}));
    let exposed = HashSet::from(["bash".to_string()]);
    let mut permission_context = PermissionContext::new(".");
    permission_context.mode = crate::permissions::PermissionMode::Default;
    permission_context.rules = crate::permissions::PermissionRules::new().deny("bash");

    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision: decision(&tool_call),
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_eq!(review.decision, ActionReviewDecision::Deny);
    assert_eq!(review.primary_reason, ActionReviewReason::PermissionDenied);
    assert!(review.permission.denied_by_rule);
    assert!(!review.permission.requires_confirmation);
}

#[test]
fn dependency_install_is_typed_ask_user_in_auto_all() {
    let tool = InstallDependenciesTool;
    let tool_call = call(
        "install_dependencies",
        serde_json::json!({"manager": "pnpm"}),
    );
    let exposed = HashSet::from(["install_dependencies".to_string()]);
    let mut permission_context = PermissionContext::new(".");
    permission_context.mode = crate::permissions::PermissionMode::AutoAll;
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision: decision(&tool_call),
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_eq!(review.decision, ActionReviewDecision::AskUser);
    assert_eq!(
        review.primary_reason,
        ActionReviewReason::NetworkRequiresConfirmation
    );
    assert!(review.permission.requires_confirmation);
    assert_eq!(
        review.side_effects.network.class,
        crate::engine::action_policy::NetworkAccessClass::PackageInstall
    );
}

#[test]
fn exhausted_budget_is_typed_deny() {
    let tool = RequiredPathTool;
    let tool_call = call("required_path", serde_json::json!({"path": "src/lib.rs"}));
    let exposed = HashSet::from(["required_path".to_string()]);
    let permission_context = PermissionContext::new(".");
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
        scheduled_count: 4,
        max_tool_calls: 4,
        action_decision: decision(&tool_call),
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_eq!(review.decision, ActionReviewDecision::Deny);
    assert_eq!(review.primary_reason, ActionReviewReason::BudgetExceeded);
    assert!(!review.budget.allowed);
}

#[test]
fn premature_code_edit_without_read_evidence_is_advisory() {
    let tool = FileEditLikeTool;
    let tool_call = call(
        "file_edit",
        serde_json::json!({"path": "src/lib.rs", "new_string": "updated"}),
    );
    let exposed = HashSet::from(["file_edit".to_string(), "file_read".to_string()]);
    let permission_context = PermissionContext::new(".");
    let task_state = understand_state();
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision: code_change_decision(&tool_call, AgentTaskStage::Understand),
        permission_context: Some(&permission_context),
        task_state: Some(&task_state),
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_eq!(review.decision, ActionReviewDecision::Allow);
    assert_eq!(review.primary_reason, ActionReviewReason::SafeToExecute);
    assert_eq!(review.worth.has_relevant_observation, Some(false));
    assert!(review.worth.premature_mutation);
    assert!(review.reasons.contains(&ActionReviewReason::LowScopeFit));
}

#[test]
fn phase_misaligned_action_records_low_scope_fit_as_advisory() {
    let tool = FileEditLikeTool;
    let tool_call = call(
        "file_edit",
        serde_json::json!({"path": "src/lib.rs", "new_string": "updated"}),
    );
    let exposed = HashSet::from(["file_edit".to_string()]);
    let permission_context = PermissionContext::new(".");
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision: code_change_decision(&tool_call, AgentTaskStage::Understand),
        permission_context: Some(&permission_context),
        task_state: None,
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_eq!(review.decision, ActionReviewDecision::Allow);
    assert_eq!(review.primary_reason, ActionReviewReason::SafeToExecute);
    assert!(review.reasons.contains(&ActionReviewReason::LowScopeFit));
    assert!(review.worth.scope_fit <= LOW_SCOPE_FIT_THRESHOLD);
    assert_eq!(
        review.debug["semantic_authority"],
        "runtime_advisory_scoring_with_hard_safety_gates"
    );
    assert_eq!(
        review.debug["candidate_action_request"]["authority"],
        "advisory_trace"
    );
}

#[test]
fn high_cost_low_value_action_records_advisory_without_blocking() {
    let tool = FileEditLikeTool;
    let tool_call = call(
        "file_edit",
        serde_json::json!({"path": "src/lib.rs", "new_string": "updated"}),
    );
    let exposed = HashSet::from(["file_edit".to_string()]);
    let permission_context = PermissionContext::new(".");
    let mut action_decision = code_change_decision(&tool_call, AgentTaskStage::Edit);
    action_decision.apply_score_modifier(
        ActionScoreModifier::new(
            ActionScoreModifierSource::Review,
            "test_cost_penalty",
            "test high-cost low-value calibration",
        )
        .value(-4)
        .cost(5),
    );
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
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

    assert_eq!(review.decision, ActionReviewDecision::Allow);
    assert_eq!(review.primary_reason, ActionReviewReason::SafeToExecute);
    assert!(review
        .reasons
        .contains(&ActionReviewReason::HighCostLowValue));
    assert!(review.model_recovery.contains("advisory only"));
}

#[test]
fn code_edit_after_relevant_read_evidence_is_not_worth_blocked() {
    let tool = FileEditLikeTool;
    let tool_call = call(
        "file_edit",
        serde_json::json!({"path": "src/lib.rs", "new_string": "updated"}),
    );
    let exposed = HashSet::from(["file_edit".to_string(), "file_read".to_string()]);
    let permission_context = PermissionContext::new(".");
    let mut task_state = understand_state();
    task_state.record_observation("file_read", "read src/lib.rs around target symbol");
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool: Some(&tool),
        exposed_tool_names: &exposed,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision: code_change_decision(&tool_call, AgentTaskStage::Understand),
        permission_context: Some(&permission_context),
        task_state: Some(&task_state),
        working_dir: None,
        tool_allowed_by_context: true,
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });

    assert_ne!(review.primary_reason, ActionReviewReason::LowValueAction);
    assert_eq!(review.worth.has_relevant_observation, Some(true));
    assert!(!review.worth.premature_mutation);
}

#[test]
fn file_edit_checkpoint_is_required_and_tool_managed() {
    let tool = FileEditLikeTool;
    let tool_call = call(
        "file_edit",
        serde_json::json!({"path": "src/lib.rs", "new_string": "updated"}),
    );
    let review = review(
        &tool_call,
        Some(&tool),
        HashSet::from(["file_edit".to_string()]),
    );

    assert!(review.checkpoint.required);
    assert_eq!(review.checkpoint.status, "required_and_present");
    assert_eq!(
        review.checkpoint.enforcement,
        "tool_managed_before_mutation"
    );
    assert_eq!(review.checkpoint.rollback_scope, "local_files");
    assert!(!review.checkpoint.requires_user_approval);
}

#[test]
fn format_check_checkpoint_is_not_needed() {
    let tool = FormatTool;
    let tool_call = call(
        "format",
        serde_json::json!({"action": "check", "file_path": "src/lib.rs"}),
    );
    let review = review(
        &tool_call,
        Some(&tool),
        HashSet::from(["format".to_string()]),
    );

    assert!(!review.checkpoint.required);
    assert_eq!(review.checkpoint.status, "not_needed");
    assert_eq!(review.tool_contract.operation_kind.as_deref(), Some("read"));
    assert_eq!(review.tool_contract.requires_confirmation, Some(false));
    assert_eq!(review.tool_contract.destructive, Some(false));
    assert_eq!(
        review.side_effects.external_side_effect,
        ExternalSideEffect::None
    );
}

#[test]
fn format_mutation_checkpoint_is_required_and_tool_managed() {
    let tool = FormatTool;
    let tool_call = call(
        "format",
        serde_json::json!({"action": "format", "file_path": "src/lib.rs"}),
    );
    let review = review(
        &tool_call,
        Some(&tool),
        HashSet::from(["format".to_string()]),
    );

    assert!(review.checkpoint.required);
    assert_eq!(review.checkpoint.status, "required_and_present");
    assert_eq!(
        review.checkpoint.enforcement,
        "tool_managed_before_mutation"
    );
    assert_eq!(review.checkpoint.rollback_scope, "local_files");
    assert!(!review.checkpoint.requires_user_approval);
    assert_eq!(review.tool_contract.operation_kind.as_deref(), Some("edit"));
    assert_eq!(review.tool_contract.requires_confirmation, Some(true));
    assert_eq!(review.tool_contract.destructive, Some(true));
}

#[test]
fn bash_workspace_mutation_checkpoint_is_required_but_missing() {
    let tool = BashTool;
    let tool_call = call(
        "bash",
        serde_json::json!({"command": "printf hi > src/lib.rs"}),
    );
    let review = review(&tool_call, Some(&tool), HashSet::from(["bash".to_string()]));

    assert_eq!(
        review.side_effects.external_side_effect,
        ExternalSideEffect::LocalWorkspaceMutation
    );
    assert!(review.checkpoint.required);
    assert_eq!(review.checkpoint.status, "required_but_missing");
    assert_eq!(review.checkpoint.enforcement, "checkpoint_wrapper_required");
    assert_eq!(review.checkpoint.rollback_scope, "local_files");
    assert!(review.checkpoint.requires_user_approval);
    assert_eq!(review.decision, ActionReviewDecision::Revise);
    assert_eq!(
        review.primary_reason,
        ActionReviewReason::CheckpointRequired
    );
    assert!(review.model_recovery.contains("checkpoint-managed"));
    assert!(review.model_recovery.contains("raw bash"));
}

#[test]
fn bounded_bash_artifact_prep_is_allowed_without_file_edit_tools() {
    let tool = BashTool;
    let tool_call = call(
        "bash",
        serde_json::json!({"command": "python3 fixtures/core_quality/long_output/generate_log.py > fixtures/core_quality/long_output/output.log"}),
    );
    let review = review(
        &tool_call,
        Some(&tool),
        HashSet::from(["bash".to_string(), "file_read".to_string()]),
    );

    assert_eq!(review.checkpoint.status, "required_and_present");
    assert_eq!(
        review.checkpoint.enforcement,
        "tool_managed_before_mutation"
    );
    assert_ne!(
        review.primary_reason,
        ActionReviewReason::CheckpointRequired
    );
}

#[test]
fn git_push_checkpoint_is_unavailable_for_remote_state() {
    let tool = GitTool;
    let tool_call = call(
        "git",
        serde_json::json!({"action": "push", "remote": "origin"}),
    );
    let review = review(&tool_call, Some(&tool), HashSet::from(["git".to_string()]));

    assert_eq!(review.checkpoint.status, "unavailable");
    assert_eq!(review.checkpoint.rollback_scope, "remote");
    assert!(review.checkpoint.requires_user_approval);
}

#[test]
fn git_status_checkpoint_is_not_needed() {
    let tool = GitTool;
    let tool_call = call("git", serde_json::json!({"action": "status"}));
    let review = review(&tool_call, Some(&tool), HashSet::from(["git".to_string()]));

    assert!(!review.checkpoint.required);
    assert_eq!(review.checkpoint.status, "not_needed");
}
