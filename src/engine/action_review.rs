//! Canonical runtime review for a proposed tool action.
//!
//! This module does not replace the specialized permission, resource, and tool
//! checks. It records their read-only verdicts in one object so traces, tool
//! results, desktop UI, and evals can reason about the same action boundary.
//! Score-derived concerns are advisory unless they map to an explicit safety or
//! evidence gate; the model still owns the semantic choice of the next action.

use crate::engine::action_decision::ActionDecision;
use crate::engine::action_policy::{ActionSideEffectProfile, ExternalSideEffect};
use crate::engine::destructive_scope::DestructiveScopeCheck;
use crate::engine::task_context::{AgentTaskStage, AgentTaskState};
use crate::permissions::{ExplainableDecision, PermissionContext, PermissionDecision};
use crate::services::api::ToolCall;
use crate::tools::{Tool, ToolOperationKind, ToolPermissionLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

const LOW_SCOPE_FIT_THRESHOLD: u8 = 4;
const HIGH_COST_THRESHOLD: u8 = 8;
const LOW_VALUE_THRESHOLD: u8 = 5;
const HIGH_RISK_THRESHOLD: u8 = 8;
const LOW_ACTION_SCORE_THRESHOLD: i16 = 3;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionReviewDecision {
    Allow,
    AskUser,
    Deny,
    Revise,
}

impl ActionReviewDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::AskUser => "ask_user",
            Self::Deny => "deny",
            Self::Revise => "revise",
        }
    }

    pub fn blocks_execution(self) -> bool {
        matches!(self, Self::Deny | Self::Revise)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionReviewReason {
    ToolNotAvailable,
    ToolNotExposed,
    InvalidArguments,
    LowValueAction,
    LowScopeFit,
    LowActionValue,
    HighCostLowValue,
    HighRiskLowValue,
    RepeatedLowScoreAction,
    PermissionRequired,
    PermissionDenied,
    PathOutsideWorkspace,
    DestructiveScopeViolation,
    NetworkRequiresConfirmation,
    ExternalSideEffectRequiresConfirmation,
    BudgetExceeded,
    CheckpointRequired,
    SafeToExecute,
}

impl ActionReviewReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ToolNotAvailable => "tool_not_available",
            Self::ToolNotExposed => "tool_not_exposed",
            Self::InvalidArguments => "invalid_arguments",
            Self::LowValueAction => "low_value_action",
            Self::LowScopeFit => "low_scope_fit",
            Self::LowActionValue => "low_action_value",
            Self::HighCostLowValue => "high_cost_low_value",
            Self::HighRiskLowValue => "high_risk_low_value",
            Self::RepeatedLowScoreAction => "repeated_low_score_action",
            Self::PermissionRequired => "permission_required",
            Self::PermissionDenied => "permission_denied",
            Self::PathOutsideWorkspace => "path_outside_workspace",
            Self::DestructiveScopeViolation => "destructive_scope_violation",
            Self::NetworkRequiresConfirmation => "network_requires_confirmation",
            Self::ExternalSideEffectRequiresConfirmation => {
                "external_side_effect_requires_confirmation"
            }
            Self::BudgetExceeded => "budget_exceeded",
            Self::CheckpointRequired => "checkpoint_required",
            Self::SafeToExecute => "safe_to_execute",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionReview {
    pub schema: String,
    pub tool: String,
    pub call_id: String,
    pub decision: ActionReviewDecision,
    pub primary_reason: ActionReviewReason,
    pub reasons: Vec<ActionReviewReason>,
    pub tool_contract: ToolContractReview,
    pub worth: ActionWorthVerdict,
    pub side_effects: ActionSideEffectProfile,
    pub permission: PermissionReviewVerdict,
    pub scope: ScopeReviewVerdict,
    pub budget: BudgetReviewVerdict,
    pub checkpoint: CheckpointReviewVerdict,
    pub user_reason: String,
    pub model_recovery: String,
    pub debug: Value,
}

impl ActionReview {
    pub fn build(input: ActionReviewInput<'_>) -> Self {
        let tool_contract = ToolContractReview::from_input(&input);
        let worth = ActionWorthVerdict::from_input(&input, &tool_contract);
        let side_effects = ActionSideEffectProfile::from_tool_call(
            input.tool_call,
            input.tool,
            input.working_dir.unwrap_or_else(|| Path::new(".")),
        );
        let permission = PermissionReviewVerdict::from_input(&input);
        let scope = ScopeReviewVerdict::from_check(input.destructive_scope_check.as_ref());
        let budget = BudgetReviewVerdict {
            allowed: input.scheduled_count < input.max_tool_calls,
            scheduled_count: input.scheduled_count,
            max_tool_calls: input.max_tool_calls,
            reason: if input.scheduled_count < input.max_tool_calls {
                "tool-call budget still has room".to_string()
            } else {
                format!(
                    "max tool calls ({}) reached before scheduling this action",
                    input.max_tool_calls
                )
            },
        };
        let checkpoint = CheckpointReviewVerdict::from_action(
            input.tool_call,
            &input.action_decision,
            &tool_contract,
            &side_effects,
        );

        let (decision, primary_reason, mut reasons) = final_decision(
            &tool_contract,
            &worth,
            &permission,
            &scope,
            &budget,
            &checkpoint,
            input.action_checkpoint_rejection.as_deref(),
        );
        if !reasons.contains(&primary_reason) {
            reasons.insert(0, primary_reason);
        }

        let advisory_reasons = reasons
            .iter()
            .copied()
            .filter(|reason| is_score_review_reason(*reason))
            .map(ActionReviewReason::as_str)
            .collect::<Vec<_>>();
        let hard_gate = decision.blocks_execution() || decision == ActionReviewDecision::AskUser;
        let user_reason = user_reason(
            decision,
            primary_reason,
            &reasons,
            &tool_contract,
            &permission,
        );
        let model_recovery = model_recovery(decision, primary_reason, &reasons, &tool_contract);
        let debug = serde_json::json!({
            "schema": "action_review_debug.v1",
            "semantic_authority": "runtime_advisory_scoring_with_hard_safety_gates",
            "hard_gate": hard_gate,
            "hard_gate_reason": hard_gate.then(|| primary_reason.as_str()),
            "advisory_reasons": advisory_reasons,
            "action_checkpoint_rejection": input.action_checkpoint_rejection,
            "exposed_tool_alternatives": tool_contract.available_alternatives,
            "candidate_action_request": candidate_action_request(decision, primary_reason, &reasons, &worth),
        });

        Self {
            schema: "action_review.v1".to_string(),
            tool: input.tool_call.name.clone(),
            call_id: input.tool_call.id.clone(),
            decision,
            primary_reason,
            reasons,
            tool_contract,
            worth,
            side_effects,
            permission,
            scope,
            budget,
            checkpoint,
            user_reason,
            model_recovery,
            debug,
        }
    }

    pub fn metadata(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| {
            serde_json::json!({
                "schema": "action_review.v1",
                "tool": self.tool,
                "call_id": self.call_id,
                "decision": self.decision.as_str(),
                "primary_reason": self.primary_reason.as_str(),
                "serialization_error": true,
            })
        })
    }
}

pub struct ActionReviewInput<'a> {
    pub tool_call: &'a ToolCall,
    pub tool: Option<&'a dyn Tool>,
    pub exposed_tool_names: &'a HashSet<String>,
    pub scheduled_count: usize,
    pub max_tool_calls: usize,
    pub action_decision: ActionDecision,
    pub permission_context: Option<&'a PermissionContext>,
    pub task_state: Option<&'a AgentTaskState>,
    pub working_dir: Option<&'a Path>,
    pub tool_allowed_by_context: bool,
    pub destructive_scope_check: Option<DestructiveScopeCheck>,
    pub action_checkpoint_rejection: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolContractReview {
    pub available: bool,
    pub exposed: bool,
    pub validation_error: Option<String>,
    pub operation_kind: Option<String>,
    pub permission_level: Option<String>,
    pub read_only: Option<bool>,
    pub destructive: Option<bool>,
    pub open_world: Option<bool>,
    pub requires_confirmation: Option<bool>,
    pub input_paths: Vec<String>,
    pub available_alternatives: Vec<String>,
}

impl ToolContractReview {
    fn from_input(input: &ActionReviewInput<'_>) -> Self {
        let params = &input.tool_call.arguments;
        let mut available_alternatives =
            input.exposed_tool_names.iter().cloned().collect::<Vec<_>>();
        available_alternatives.sort();
        available_alternatives.truncate(20);

        match input.tool {
            Some(tool) => Self {
                available: true,
                exposed: input.exposed_tool_names.contains(&input.tool_call.name),
                validation_error: tool.validate_params(params),
                operation_kind: Some(operation_kind_label(tool.operation_kind(params))),
                permission_level: Some(permission_level_label(tool.permission_level())),
                read_only: Some(tool.is_read_only(params)),
                destructive: Some(tool.is_destructive(params)),
                open_world: Some(tool.is_open_world(params)),
                requires_confirmation: Some(tool.requires_confirmation(params)),
                input_paths: tool.input_paths(params),
                available_alternatives,
            },
            None => Self {
                available: false,
                exposed: input.exposed_tool_names.contains(&input.tool_call.name),
                validation_error: None,
                operation_kind: None,
                permission_level: None,
                read_only: None,
                destructive: None,
                open_world: None,
                requires_confirmation: None,
                input_paths: Vec::new(),
                available_alternatives,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionWorthVerdict {
    pub stage: String,
    pub phase_aligned: bool,
    pub mutates_workspace: bool,
    pub broad_shell: bool,
    pub value: u8,
    pub risk: u8,
    pub uncertainty_reduction: u8,
    pub cost: u8,
    pub reversibility: u8,
    pub scope_fit: u8,
    pub action_score: i16,
    pub formula_stage: String,
    pub formula_version: String,
    pub modifier_count: usize,
    pub requires_confirmation: bool,
    pub low_value: bool,
    pub low_scope_fit: bool,
    pub high_cost_low_value: bool,
    pub high_risk_low_value: bool,
    pub repeated_low_score: bool,
    pub has_relevant_observation: Option<bool>,
    pub premature_mutation: bool,
    pub reason: String,
    pub suggested_verification: Option<String>,
    pub suggested_next_action: Option<String>,
}

impl ActionWorthVerdict {
    fn from_input(input: &ActionReviewInput<'_>, contract: &ToolContractReview) -> Self {
        let decision = &input.action_decision;
        let low_value = decision.scores.value <= 3
            && decision.scores.uncertainty_reduction <= 3
            && decision.scores.risk >= 7;
        let low_scope_fit = decision.scores.scope_fit <= LOW_SCOPE_FIT_THRESHOLD;
        let high_cost_low_value = decision.scores.cost >= HIGH_COST_THRESHOLD
            && decision.scores.value <= LOW_VALUE_THRESHOLD;
        let high_risk_low_value = decision.scores.risk >= HIGH_RISK_THRESHOLD
            && decision.scores.value <= LOW_VALUE_THRESHOLD;
        let repeated_low_score = input
            .task_state
            .map(|state| state.consecutive_low_action_scores() >= 2)
            .unwrap_or(false)
            && decision.scores.action_score <= LOW_ACTION_SCORE_THRESHOLD;
        let has_relevant_observation = relevant_observation(input.task_state, contract);
        let code_like_mutation = decision.action.mutates_workspace
            && contract.input_paths.iter().any(|path| code_like_path(path));
        let premature_mutation = matches!(decision.action.stage, AgentTaskStage::Understand)
            && code_like_mutation
            && has_relevant_observation == Some(false)
            && decision.scores.risk >= 7
            && decision.scores.uncertainty_reduction <= 3;
        let suggested_next_action = premature_mutation.then(|| {
            let target = contract
                .input_paths
                .first()
                .cloned()
                .unwrap_or_else(|| "the target file".to_string());
            format!("Inspect {target} with file_read or grep before mutating it")
        });
        Self {
            stage: format!("{:?}", decision.action.stage),
            phase_aligned: decision.action.phase_aligned,
            mutates_workspace: decision.action.mutates_workspace,
            broad_shell: decision.action.broad_shell,
            value: decision.scores.value,
            risk: decision.scores.risk,
            uncertainty_reduction: decision.scores.uncertainty_reduction,
            cost: decision.scores.cost,
            reversibility: decision.scores.reversibility,
            scope_fit: decision.scores.scope_fit,
            action_score: decision.scores.action_score,
            formula_stage: decision
                .score_computation
                .formula_stage
                .as_str()
                .to_string(),
            formula_version: decision.score_computation.formula_version.clone(),
            modifier_count: decision.score_computation.modifiers.len(),
            requires_confirmation: decision.requires_confirmation,
            low_value,
            low_scope_fit,
            high_cost_low_value,
            high_risk_low_value,
            repeated_low_score,
            has_relevant_observation,
            premature_mutation,
            reason: decision.reason_summary.clone(),
            suggested_verification: decision.verification_after.clone(),
            suggested_next_action,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionReviewVerdict {
    pub allowed_by_context: bool,
    pub requires_confirmation: bool,
    pub permission_requires_confirmation: bool,
    pub raw_tool_requires_confirmation: bool,
    pub tool_requires_confirmation: bool,
    pub decision: Option<String>,
    pub risk_level: Option<String>,
    pub confidence: Option<f32>,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
    pub matched_rules: Vec<MatchedPermissionRule>,
}

impl PermissionReviewVerdict {
    fn from_input(input: &ActionReviewInput<'_>) -> Self {
        let raw_tool_requires_confirmation = input
            .tool
            .map(|tool| tool.requires_confirmation(&input.tool_call.arguments))
            .unwrap_or(false);
        let permission_explanation = input.permission_context.map(|context| {
            context.explain_decision(&input.tool_call.name, &input.tool_call.arguments)
        });
        let permission_requires_confirmation = input
            .permission_context
            .map(|context| {
                context.requires_confirmation(&input.tool_call.name, &input.tool_call.arguments)
            })
            .unwrap_or(false);
        let tool_requires_confirmation = match input.permission_context {
            Some(context) => {
                raw_tool_requires_confirmation
                    && !context.auto_approves_tool_confirmation(
                        &input.tool_call.name,
                        &input.tool_call.arguments,
                    )
            }
            None => raw_tool_requires_confirmation,
        };
        let requires_confirmation = permission_requires_confirmation || tool_requires_confirmation;

        let permission_fields = permission_fields(permission_explanation.as_ref());

        Self {
            allowed_by_context: input.tool_allowed_by_context,
            requires_confirmation,
            permission_requires_confirmation,
            raw_tool_requires_confirmation,
            tool_requires_confirmation,
            decision: permission_fields.decision,
            risk_level: permission_fields.risk_level,
            confidence: permission_fields.confidence,
            reasons: permission_fields.reasons,
            warnings: permission_fields.warnings,
            matched_rules: permission_fields.matched_rules,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatchedPermissionRule {
    pub decision: String,
    pub source: String,
    pub pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopeReviewVerdict {
    pub applies: bool,
    pub allowed: bool,
    pub operation: Option<String>,
    pub target: Option<String>,
    pub reason: String,
}

impl ScopeReviewVerdict {
    fn from_check(check: Option<&DestructiveScopeCheck>) -> Self {
        match check {
            Some(check) => Self {
                applies: check.applies,
                allowed: check.allowed,
                operation: check.applies.then(|| check.operation.clone()),
                target: check.target.clone(),
                reason: check.reason.clone(),
            },
            None => Self {
                applies: false,
                allowed: true,
                operation: None,
                target: None,
                reason: "destructive scope check did not apply".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetReviewVerdict {
    pub allowed: bool,
    pub scheduled_count: usize,
    pub max_tool_calls: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckpointReviewVerdict {
    pub required: bool,
    pub status: String,
    pub enforcement: String,
    pub rollback_scope: String,
    pub checkpoint_id: Option<String>,
    pub requires_user_approval: bool,
    pub reason: String,
}

impl CheckpointReviewVerdict {
    fn from_action(
        tool_call: &ToolCall,
        action_decision: &ActionDecision,
        contract: &ToolContractReview,
        side_effects: &ActionSideEffectProfile,
    ) -> Self {
        match tool_call.name.as_str() {
            "file_write" | "file_edit" | "file_patch" => {
                return Self::required_and_present(
                    "local_files",
                    "file mutation tools create rollback checkpoints before writing",
                );
            }
            "format" => {
                return match tool_call.arguments["action"].as_str() {
                    Some("check") => Self::not_needed("format check is observational"),
                    _ => Self::required_and_present(
                        "local_files",
                        "format creates rollback checkpoints before rewriting files",
                    ),
                };
            }
            "bash"
                if side_effects.external_side_effect
                    == ExternalSideEffect::LocalWorkspaceMutation =>
            {
                if allow_bash_artifact_prep_without_checkpoint(tool_call, contract, side_effects) {
                    return Self::required_and_present(
                        "local_files",
                        "bounded bash artifact preparation is allowed when checkpoint-managed file tools are not exposed",
                    );
                }
                return Self::checkpoint_wrapper_required(
                    "local_files",
                    "raw bash workspace mutation must use a checkpoint-managed wrapper",
                );
            }
            "git" => {
                return match tool_call.arguments["action"].as_str() {
                    Some("status" | "diff" | "log" | "show") => {
                        Self::not_needed("git read action is observational")
                    }
                    Some("push") => Self::unavailable(
                        "remote",
                        true,
                        "git push mutates remote state and cannot be rolled back by a local checkpoint",
                    ),
                    Some("add" | "commit" | "checkout" | "branch" | "reset" | "clean") => {
                        Self::unavailable(
                            "repository_metadata",
                            true,
                            "git mutation changes repository metadata or worktree state outside the file checkpoint contract",
                        )
                    }
                    _ => Self::unavailable(
                        "repository_metadata",
                        true,
                        "unknown git mutation has no local checkpoint rollback contract",
                    ),
                };
            }
            _ => {}
        }

        match side_effects.external_side_effect {
            ExternalSideEffect::None | ExternalSideEffect::NetworkRead
                if !mutation_requires_checkpoint(action_decision, contract) =>
            {
                Self::not_needed("reviewed action is observational")
            }
            ExternalSideEffect::LocalWorkspaceMutation => Self::required_but_missing(
                "local_files",
                true,
                "workspace mutation has no explicit checkpoint contract",
            ),
            ExternalSideEffect::LocalMachineMutation => Self::unavailable(
                "local_machine",
                true,
                "local machine mutation cannot be rolled back by a workspace file checkpoint",
            ),
            ExternalSideEffect::NetworkWrite => Self::unavailable(
                "remote",
                true,
                "network write cannot be rolled back by a local checkpoint",
            ),
            ExternalSideEffect::GitRemotePublication => Self::unavailable(
                "remote",
                true,
                "git remote publication cannot be rolled back by a local checkpoint",
            ),
            ExternalSideEffect::DatabaseOrDeploy => Self::unavailable(
                "external_service",
                true,
                "database or deploy action has external effects outside local checkpoint rollback",
            ),
            ExternalSideEffect::CredentialOrAuth => Self::unavailable(
                "credential_or_auth",
                true,
                "credential or auth mutation is outside local checkpoint rollback",
            ),
            ExternalSideEffect::PluginOrMcpUnknown => Self::unavailable(
                "external_plugin_or_mcp",
                true,
                "plugin or MCP side effects are not locally checkpointable by default",
            ),
            ExternalSideEffect::None | ExternalSideEffect::NetworkRead => {
                Self::required_but_missing(
                    "unknown",
                    true,
                    "action is classified as mutating but no checkpoint contract is available",
                )
            }
        }
    }

    fn not_needed(reason: &str) -> Self {
        Self {
            required: false,
            status: "not_needed".to_string(),
            enforcement: "not_required".to_string(),
            rollback_scope: "none".to_string(),
            checkpoint_id: None,
            requires_user_approval: false,
            reason: reason.to_string(),
        }
    }

    fn required_and_present(rollback_scope: &str, reason: &str) -> Self {
        Self {
            required: true,
            status: "required_and_present".to_string(),
            enforcement: "tool_managed_before_mutation".to_string(),
            rollback_scope: rollback_scope.to_string(),
            checkpoint_id: None,
            requires_user_approval: false,
            reason: reason.to_string(),
        }
    }

    fn required_but_missing(
        rollback_scope: &str,
        requires_user_approval: bool,
        reason: &str,
    ) -> Self {
        Self {
            required: true,
            status: "required_but_missing".to_string(),
            enforcement: "not_enforced_yet".to_string(),
            rollback_scope: rollback_scope.to_string(),
            checkpoint_id: None,
            requires_user_approval,
            reason: reason.to_string(),
        }
    }

    fn checkpoint_wrapper_required(rollback_scope: &str, reason: &str) -> Self {
        Self {
            required: true,
            status: "required_but_missing".to_string(),
            enforcement: "checkpoint_wrapper_required".to_string(),
            rollback_scope: rollback_scope.to_string(),
            checkpoint_id: None,
            requires_user_approval: true,
            reason: reason.to_string(),
        }
    }

    fn unavailable(rollback_scope: &str, requires_user_approval: bool, reason: &str) -> Self {
        Self {
            required: true,
            status: "unavailable".to_string(),
            enforcement: "not_available_for_side_effect".to_string(),
            rollback_scope: rollback_scope.to_string(),
            checkpoint_id: None,
            requires_user_approval,
            reason: reason.to_string(),
        }
    }
}

fn mutation_requires_checkpoint(
    action_decision: &ActionDecision,
    contract: &ToolContractReview,
) -> bool {
    action_decision.action.mutates_workspace
        || matches!(
            contract.operation_kind.as_deref(),
            Some("write" | "edit" | "patch" | "shell")
        ) && contract.destructive.unwrap_or(false)
}

fn allow_bash_artifact_prep_without_checkpoint(
    tool_call: &ToolCall,
    _contract: &ToolContractReview,
    side_effects: &ActionSideEffectProfile,
) -> bool {
    if tool_call.name != "bash"
        || side_effects.external_side_effect != ExternalSideEffect::LocalWorkspaceMutation
    {
        return false;
    }
    if side_effects.paths.iter().any(|path| {
        !path.inside_workspace
            || matches!(
                path.class,
                crate::engine::action_policy::WorkspacePathClass::External
                    | crate::engine::action_policy::WorkspacePathClass::System
                    | crate::engine::action_policy::WorkspacePathClass::HomePrivate
                    | crate::engine::action_policy::WorkspacePathClass::RepoMetadata
                    | crate::engine::action_policy::WorkspacePathClass::Credential
            )
    }) {
        return false;
    }
    let Some(command) = tool_call.arguments["command"].as_str() else {
        return false;
    };
    let normalized =
        crate::tools::bash_tool::command_classifier::normalize_command_for_match(command)
            .to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    normalized.starts_with("python -m venv .venv")
        || normalized.starts_with("python3 -m venv .venv")
        || normalized.contains(
            "fixtures/core_quality/long_output/generate_log.py > fixtures/core_quality/long_output/output.log",
        )
}

fn final_decision(
    contract: &ToolContractReview,
    worth: &ActionWorthVerdict,
    permission: &PermissionReviewVerdict,
    scope: &ScopeReviewVerdict,
    budget: &BudgetReviewVerdict,
    checkpoint: &CheckpointReviewVerdict,
    action_checkpoint_rejection: Option<&str>,
) -> (
    ActionReviewDecision,
    ActionReviewReason,
    Vec<ActionReviewReason>,
) {
    if !contract.available {
        return (
            ActionReviewDecision::Revise,
            ActionReviewReason::ToolNotAvailable,
            vec![ActionReviewReason::ToolNotAvailable],
        );
    }
    if !contract.exposed {
        return (
            ActionReviewDecision::Revise,
            ActionReviewReason::ToolNotExposed,
            vec![ActionReviewReason::ToolNotExposed],
        );
    }
    if contract.validation_error.is_some() {
        return (
            ActionReviewDecision::Revise,
            ActionReviewReason::InvalidArguments,
            vec![ActionReviewReason::InvalidArguments],
        );
    }
    if !budget.allowed {
        return (
            ActionReviewDecision::Deny,
            ActionReviewReason::BudgetExceeded,
            vec![ActionReviewReason::BudgetExceeded],
        );
    }
    if !permission.allowed_by_context {
        return (
            ActionReviewDecision::Deny,
            ActionReviewReason::PermissionDenied,
            vec![ActionReviewReason::PermissionDenied],
        );
    }
    if scope.applies && !scope.allowed {
        return (
            ActionReviewDecision::Deny,
            ActionReviewReason::DestructiveScopeViolation,
            vec![ActionReviewReason::DestructiveScopeViolation],
        );
    }
    if action_checkpoint_rejection.is_some() {
        return (
            ActionReviewDecision::Revise,
            ActionReviewReason::CheckpointRequired,
            vec![ActionReviewReason::CheckpointRequired],
        );
    }
    if checkpoint.enforcement == "checkpoint_wrapper_required" {
        return (
            ActionReviewDecision::Revise,
            ActionReviewReason::CheckpointRequired,
            vec![ActionReviewReason::CheckpointRequired],
        );
    }
    if permission.requires_confirmation {
        let reason = permission_confirmation_reason(permission, contract);
        return (ActionReviewDecision::AskUser, reason, vec![reason]);
    }
    let mut reasons = vec![ActionReviewReason::SafeToExecute];
    reasons.extend(advisory_score_reasons(worth));

    (
        ActionReviewDecision::Allow,
        ActionReviewReason::SafeToExecute,
        reasons,
    )
}

fn permission_confirmation_reason(
    permission: &PermissionReviewVerdict,
    contract: &ToolContractReview,
) -> ActionReviewReason {
    if contract.open_world == Some(true)
        || permission
            .warnings
            .iter()
            .any(|warning| warning.contains("NETWORK") || warning.contains("REMOTE"))
    {
        return ActionReviewReason::NetworkRequiresConfirmation;
    }
    if permission
        .warnings
        .iter()
        .any(|warning| warning.contains("OUTSIDE_WORKSPACE"))
    {
        return ActionReviewReason::PathOutsideWorkspace;
    }
    if permission
        .warnings
        .iter()
        .any(|warning| warning.contains("REMOTE") || warning.contains("AUTH"))
    {
        return ActionReviewReason::ExternalSideEffectRequiresConfirmation;
    }
    ActionReviewReason::PermissionRequired
}

fn user_reason(
    decision: ActionReviewDecision,
    reason: ActionReviewReason,
    reasons: &[ActionReviewReason],
    contract: &ToolContractReview,
    permission: &PermissionReviewVerdict,
) -> String {
    match decision {
        ActionReviewDecision::Allow if has_score_review_reason(reasons) => {
            "Action passed runtime review; score-based concerns were recorded as advisory evidence."
                .to_string()
        }
        ActionReviewDecision::Allow => "Action passed runtime review.".to_string(),
        ActionReviewDecision::AskUser => format!(
            "Action requires user confirmation before execution: {}.",
            reason.as_str()
        ),
        ActionReviewDecision::Deny => {
            format!("Action denied before execution: {}.", reason.as_str())
        }
        ActionReviewDecision::Revise => {
            if reason == ActionReviewReason::LowValueAction {
                return "Action rejected before execution: mutation is premature for the current understanding stage.".to_string();
            }
            if is_score_review_reason(reason) {
                return format!(
                    "Action rejected before execution: {}. Choose a narrower, lower-risk action that fits the current task stage.",
                    reason.as_str()
                );
            }
            if reason == ActionReviewReason::CheckpointRequired {
                return "Action rejected before execution: workspace mutation requires a checkpoint-managed tool.".to_string();
            }
            if let Some(error) = contract.validation_error.as_deref() {
                format!(
                    "Action rejected before execution: {} ({error}).",
                    reason.as_str()
                )
            } else if !contract.available {
                "Action rejected before execution: tool is not registered.".to_string()
            } else if !contract.exposed {
                "Action rejected before execution: tool is not exposed for this request."
                    .to_string()
            } else if !permission.allowed_by_context {
                "Action rejected before execution: tool is outside the allowed runtime context."
                    .to_string()
            } else {
                format!("Action rejected before execution: {}.", reason.as_str())
            }
        }
    }
}

fn model_recovery(
    decision: ActionReviewDecision,
    reason: ActionReviewReason,
    reasons: &[ActionReviewReason],
    contract: &ToolContractReview,
) -> String {
    match decision {
        ActionReviewDecision::Allow if has_score_review_reason(reasons) => {
            "Action passed runtime review; runtime score concerns are advisory only, so use the tool observation and keep the next-step judgment model-led.".to_string()
        }
        ActionReviewDecision::Allow => {
            "Action passed runtime review; use the observation after execution.".to_string()
        }
        ActionReviewDecision::AskUser => format!(
            "Action needs user approval before execution: {}. Wait for the permission result and do not claim the tool ran until it succeeds.",
            reason.as_str()
        ),
        ActionReviewDecision::Deny => format!(
            "Action denied before execution: {}. Choose a lower-risk action inside the current task scope.",
            reason.as_str()
        ),
        ActionReviewDecision::Revise => {
            if reason == ActionReviewReason::LowValueAction {
                return "Action rejected before execution: low_value_action. Inspect the target with file_read or grep first, then retry the smallest safe mutation if the evidence supports it.".to_string();
            }
            if is_score_review_reason(reason) {
                return format!(
                    "Action rejected before execution: {}. Propose a safer candidate action with scope_fit above {}, value above {}, and risk/cost justified by current evidence.",
                    reason.as_str(),
                    LOW_SCOPE_FIT_THRESHOLD,
                    LOW_VALUE_THRESHOLD
                );
            }
            if reason == ActionReviewReason::CheckpointRequired {
                return "Action rejected before execution: checkpoint_required. Use file_write/file_edit/file_patch, format, or another checkpoint-managed wrapper; do not mutate workspace files through raw bash.".to_string();
            }
            let alternatives = if contract.available_alternatives.is_empty() {
                "the exposed tools".to_string()
            } else {
                format!(
                    "one of the exposed tools ({})",
                    contract.available_alternatives.join(", ")
                )
            };
            format!(
                "Action rejected before execution: {}. Choose {alternatives} and provide valid arguments.",
                reason.as_str()
            )
        }
    }
}

fn is_score_review_reason(reason: ActionReviewReason) -> bool {
    matches!(
        reason,
        ActionReviewReason::LowScopeFit
            | ActionReviewReason::LowActionValue
            | ActionReviewReason::HighCostLowValue
            | ActionReviewReason::HighRiskLowValue
            | ActionReviewReason::RepeatedLowScoreAction
    )
}

fn has_score_review_reason(reasons: &[ActionReviewReason]) -> bool {
    reasons.iter().copied().any(is_score_review_reason)
}

fn advisory_score_reasons(worth: &ActionWorthVerdict) -> Vec<ActionReviewReason> {
    let mut reasons = Vec::new();
    if worth.repeated_low_score {
        reasons.push(ActionReviewReason::RepeatedLowScoreAction);
    }
    if worth.low_scope_fit
        && (worth.mutates_workspace || worth.broad_shell || worth.risk >= HIGH_RISK_THRESHOLD)
    {
        reasons.push(ActionReviewReason::LowScopeFit);
    }
    if worth.high_cost_low_value {
        reasons.push(ActionReviewReason::HighCostLowValue);
    }
    if worth.high_risk_low_value {
        reasons.push(ActionReviewReason::HighRiskLowValue);
    }
    if (worth.low_value || worth.action_score <= LOW_ACTION_SCORE_THRESHOLD)
        && (worth.mutates_workspace || worth.broad_shell || worth.risk >= HIGH_RISK_THRESHOLD)
    {
        reasons.push(ActionReviewReason::LowActionValue);
    }
    reasons
}

fn candidate_action_request(
    decision: ActionReviewDecision,
    reason: ActionReviewReason,
    reasons: &[ActionReviewReason],
    worth: &ActionWorthVerdict,
) -> Value {
    let mode = crate::engine::candidate_action::CandidateActionMode::from_env();
    let advisory_reasons = reasons
        .iter()
        .copied()
        .filter(|reason| is_score_review_reason(*reason))
        .map(ActionReviewReason::as_str)
        .collect::<Vec<_>>();
    let hard_score_repair = decision != ActionReviewDecision::Allow
        && (is_score_review_reason(reason) || reason == ActionReviewReason::LowValueAction);
    let triggered = hard_score_repair || !advisory_reasons.is_empty();
    serde_json::json!({
        "schema": "candidate_action_request.v1",
        "mode": mode.as_str(),
        "triggered": triggered,
        "authority": if hard_score_repair { "hard_gate_repair" } else if !advisory_reasons.is_empty() { "advisory_trace" } else { "none" },
        "reason": reason.as_str(),
        "advisory_reasons": advisory_reasons,
        "selected_action_score": worth.action_score,
        "selected_scope_fit": worth.scope_fit,
        "min_scope_fit": LOW_SCOPE_FIT_THRESHOLD + 1,
        "min_value": LOW_VALUE_THRESHOLD + 1,
        "instructions": if triggered {
            "Score-based runtime concerns are advisory unless a hard gate blocked execution. Keep the next-step choice model-led, using exposed tools and current evidence."
        } else {
            "none"
        },
    })
}

struct PermissionFields {
    decision: Option<String>,
    risk_level: Option<String>,
    confidence: Option<f32>,
    reasons: Vec<String>,
    warnings: Vec<String>,
    matched_rules: Vec<MatchedPermissionRule>,
}

fn permission_fields(explanation: Option<&ExplainableDecision>) -> PermissionFields {
    match explanation {
        Some(explanation) => PermissionFields {
            decision: Some(permission_decision_label(explanation.decision)),
            risk_level: Some(format!("{:?}", explanation.risk_level)),
            confidence: Some(explanation.confidence),
            reasons: explanation.reasons.clone(),
            warnings: explanation.warnings.clone(),
            matched_rules: explanation
                .matched_rules
                .iter()
                .map(|(decision, rule)| MatchedPermissionRule {
                    decision: permission_decision_label(*decision),
                    source: format!("{:?}", rule.source),
                    pattern: rule.pattern.clone(),
                })
                .collect(),
        },
        None => PermissionFields {
            decision: None,
            risk_level: None,
            confidence: None,
            reasons: Vec::new(),
            warnings: Vec::new(),
            matched_rules: Vec::new(),
        },
    }
}

fn permission_decision_label(decision: PermissionDecision) -> String {
    match decision {
        PermissionDecision::Allow => "allow",
        PermissionDecision::Deny => "deny",
        PermissionDecision::Ask => "ask",
    }
    .to_string()
}

fn operation_kind_label(kind: ToolOperationKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn permission_level_label(level: ToolPermissionLevel) -> String {
    serde_json::to_value(level)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{level:?}"))
}

fn relevant_observation(
    task_state: Option<&AgentTaskState>,
    contract: &ToolContractReview,
) -> Option<bool> {
    let task_state = task_state?;
    if contract.input_paths.is_empty() {
        return Some(!task_state.observations.is_empty() || !task_state.completed_steps.is_empty());
    }

    Some(contract.input_paths.iter().any(|path| {
        let path = path.to_ascii_lowercase();
        let file_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path.as_str())
            .to_ascii_lowercase();
        task_state
            .active_files
            .iter()
            .any(|active| path_matches(active.to_string_lossy().as_ref(), &path, &file_name))
            || task_state.observations.iter().any(|observation| {
                path_matches(&observation.source, &path, &file_name)
                    || path_matches(&observation.summary, &path, &file_name)
            })
            || task_state
                .completed_steps
                .iter()
                .any(|step| path_matches(&step.summary, &path, &file_name))
    }))
}

fn path_matches(text: &str, path: &str, file_name: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.contains(path) || (!file_name.is_empty() && text.contains(file_name))
}

fn code_like_path(path: &str) -> bool {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "go"
            | "java"
            | "kt"
            | "swift"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "scala"
            | "sh"
            | "zsh"
            | "fish"
            | "toml"
            | "yaml"
            | "yml"
            | "json"
            | "md"
    )
}

#[cfg(test)]
mod tests;
