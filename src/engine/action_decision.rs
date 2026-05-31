//! Runtime-owned action decision scoring.
//!
//! V1 scores tool calls deterministically from the current route, task stage,
//! and tool semantics. The model can still reason freely; the runtime records a
//! compact decision object at the action boundary.

use crate::engine::intent_router::{RiskLevel, WorkflowKind};
use crate::engine::task_context::AgentTaskStage;
use crate::services::api::ToolCall;
use serde::{Deserialize, Serialize};

pub const ACTION_SCORE_FORMULA_VERSION: &str = "action_score.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionDecision {
    pub reason_summary: String,
    pub action: ProposedAction,
    pub expected_observation: String,
    pub scores: ActionScores,
    #[serde(default)]
    pub score_computation: ActionScoreComputation,
    pub requires_confirmation: bool,
    pub verification_after: Option<String>,
    pub trace_recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposedAction {
    pub tool_name: String,
    pub stage: AgentTaskStage,
    pub mutates_workspace: bool,
    pub broad_shell: bool,
    pub phase_aligned: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionScores {
    pub value: u8,
    pub risk: u8,
    pub uncertainty_reduction: u8,
    pub cost: u8,
    pub reversibility: u8,
    #[serde(default)]
    pub scope_fit: u8,
    #[serde(default)]
    pub action_score: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionScoreComputation {
    #[serde(default)]
    pub formula_stage: ActionScoreStage,
    #[serde(default = "default_formula_version")]
    pub formula_version: String,
    #[serde(default)]
    pub modifiers: Vec<ActionScoreModifier>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionScoreStage {
    #[default]
    Diagnosis,
    Planning,
    Implementation,
    Verification,
    Recovery,
    Closeout,
}

impl Default for ActionScoreComputation {
    fn default() -> Self {
        Self {
            formula_stage: ActionScoreStage::Diagnosis,
            formula_version: ACTION_SCORE_FORMULA_VERSION.to_string(),
            modifiers: Vec::new(),
        }
    }
}

fn default_formula_version() -> String {
    ACTION_SCORE_FORMULA_VERSION.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionScoreModifier {
    pub source: ActionScoreModifierSource,
    pub kind: String,
    pub reason: String,
    pub value_delta: i8,
    pub risk_delta: i8,
    pub uncertainty_reduction_delta: i8,
    pub cost_delta: i8,
    pub reversibility_delta: i8,
    pub scope_fit_delta: i8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionScoreModifierSource {
    Route,
    Checkpoint,
    Progress,
    Phase,
    ToolProfile,
    Memory,
    Observer,
    Review,
}

#[derive(Debug, Clone, Copy)]
pub struct ActionDecisionInput {
    pub task_stage: AgentTaskStage,
    pub route_workflow: Option<WorkflowKind>,
    pub route_risk: Option<RiskLevel>,
    pub action_checkpoint_active: bool,
    pub has_changes_before_tools: bool,
    pub no_progress_rounds: usize,
}

impl ActionDecision {
    pub fn for_tool_call(tool_call: &ToolCall, input: ActionDecisionInput) -> Self {
        let profile = ToolActionProfile::from_tool_call(tool_call);
        let phase_aligned = phase_allows_action(input.task_stage, &profile);
        let mut scores = scores_for_profile(&profile, input.task_stage);
        let mut modifiers = Vec::new();
        let formula_stage = ActionScoreStage::from_task_stage(input.task_stage);

        if input.route_risk == Some(RiskLevel::High) {
            apply_score_modifier_to_scores(
                &mut scores,
                &mut modifiers,
                ActionScoreModifier::new(
                    ActionScoreModifierSource::Route,
                    "high_route_risk",
                    "high-risk route raises action risk",
                )
                .risk(2),
            );
        }
        if matches!(
            input.route_workflow,
            Some(WorkflowKind::CodeChange | WorkflowKind::BugFix)
        ) && matches!(
            profile.kind,
            ToolActionKind::Inspect
                | ToolActionKind::CreateFile
                | ToolActionKind::Edit
                | ToolActionKind::Validate
                | ToolActionKind::Format
        ) {
            apply_score_modifier_to_scores(
                &mut scores,
                &mut modifiers,
                ActionScoreModifier::new(
                    ActionScoreModifierSource::Route,
                    "workflow_scope_fit",
                    "tool kind fits the routed programming workflow",
                )
                .scope_fit(1),
            );
        }
        if input.action_checkpoint_active {
            apply_score_modifier_to_scores(
                &mut scores,
                &mut modifiers,
                ActionScoreModifier::new(
                    ActionScoreModifierSource::Checkpoint,
                    "checkpoint_active",
                    "active checkpoint raises mutation value and risk visibility",
                )
                .value(1)
                .risk(1),
            );
        }
        if input.no_progress_rounds >= 2 {
            apply_score_modifier_to_scores(
                &mut scores,
                &mut modifiers,
                ActionScoreModifier::new(
                    ActionScoreModifierSource::Progress,
                    "no_progress_pressure",
                    "repeated no-progress rounds make useful actions more valuable but costlier",
                )
                .value(1)
                .cost(1),
            );
        }
        if !phase_aligned {
            apply_score_modifier_to_scores(
                &mut scores,
                &mut modifiers,
                ActionScoreModifier::new(
                    ActionScoreModifierSource::Phase,
                    "phase_mismatch",
                    "action does not fit the current task stage",
                )
                .value(-2)
                .risk(2)
                .scope_fit(-4),
            );
        }
        scores.action_score = compute_action_score(scores, formula_stage);

        let requires_confirmation = profile.requires_confirmation
            || (input.route_risk == Some(RiskLevel::High)
                && (profile.mutates_workspace || profile.broad_shell))
            || (!phase_aligned && profile.mutates_workspace);
        let verification_after = if profile.mutates_workspace {
            Some("run the narrowest meaningful validation after the mutation".to_string())
        } else if input.has_changes_before_tools
            && matches!(input.task_stage, AgentTaskStage::Validate)
        {
            Some("use the observation as validation evidence if it succeeds".to_string())
        } else {
            None
        };
        let trace_recommended = requires_confirmation
            || input.route_risk == Some(RiskLevel::High)
            || input.action_checkpoint_active
            || profile.mutates_workspace
            || profile.broad_shell
            || input.no_progress_rounds >= 2
            || !phase_aligned;

        Self {
            reason_summary: reason_summary(&profile, input.task_stage, phase_aligned),
            action: ProposedAction {
                tool_name: tool_call.name.clone(),
                stage: input.task_stage,
                mutates_workspace: profile.mutates_workspace,
                broad_shell: profile.broad_shell,
                phase_aligned,
            },
            expected_observation: profile.expected_observation.to_string(),
            scores,
            score_computation: ActionScoreComputation {
                formula_stage,
                formula_version: ACTION_SCORE_FORMULA_VERSION.to_string(),
                modifiers,
            },
            requires_confirmation,
            verification_after,
            trace_recommended,
        }
    }

    pub fn apply_score_modifier(&mut self, modifier: ActionScoreModifier) {
        apply_score_delta(&mut self.scores.value, modifier.value_delta);
        apply_score_delta(&mut self.scores.risk, modifier.risk_delta);
        apply_score_delta(
            &mut self.scores.uncertainty_reduction,
            modifier.uncertainty_reduction_delta,
        );
        apply_score_delta(&mut self.scores.cost, modifier.cost_delta);
        apply_score_delta(&mut self.scores.reversibility, modifier.reversibility_delta);
        apply_score_delta(&mut self.scores.scope_fit, modifier.scope_fit_delta);
        self.scores.action_score =
            compute_action_score(self.scores, self.score_computation.formula_stage);
        self.score_computation.modifiers.push(modifier);
    }

    pub fn record_score_modifier_evidence(&mut self, modifier: ActionScoreModifier) {
        self.score_computation.modifiers.push(modifier);
    }
}

impl ActionScoreStage {
    pub fn from_task_stage(stage: AgentTaskStage) -> Self {
        match stage {
            AgentTaskStage::Understand => Self::Diagnosis,
            AgentTaskStage::Plan => Self::Planning,
            AgentTaskStage::Edit => Self::Implementation,
            AgentTaskStage::Validate => Self::Verification,
            AgentTaskStage::Repair => Self::Recovery,
            AgentTaskStage::Closeout | AgentTaskStage::Done => Self::Closeout,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Diagnosis => "diagnosis",
            Self::Planning => "planning",
            Self::Implementation => "implementation",
            Self::Verification => "verification",
            Self::Recovery => "recovery",
            Self::Closeout => "closeout",
        }
    }
}

impl ActionScoreModifier {
    pub fn new(
        source: ActionScoreModifierSource,
        kind: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            source,
            kind: kind.into(),
            reason: reason.into(),
            value_delta: 0,
            risk_delta: 0,
            uncertainty_reduction_delta: 0,
            cost_delta: 0,
            reversibility_delta: 0,
            scope_fit_delta: 0,
        }
    }

    pub fn value(mut self, delta: i8) -> Self {
        self.value_delta = delta;
        self
    }

    pub fn risk(mut self, delta: i8) -> Self {
        self.risk_delta = delta;
        self
    }

    pub fn uncertainty_reduction(mut self, delta: i8) -> Self {
        self.uncertainty_reduction_delta = delta;
        self
    }

    pub fn cost(mut self, delta: i8) -> Self {
        self.cost_delta = delta;
        self
    }

    pub fn reversibility(mut self, delta: i8) -> Self {
        self.reversibility_delta = delta;
        self
    }

    pub fn scope_fit(mut self, delta: i8) -> Self {
        self.scope_fit_delta = delta;
        self
    }
}

#[derive(Debug, Clone, Copy)]
struct ToolActionProfile {
    mutates_workspace: bool,
    broad_shell: bool,
    requires_confirmation: bool,
    kind: ToolActionKind,
    expected_observation: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolActionKind {
    Inspect,
    CreateFile,
    Edit,
    Validate,
    Format,
    StartServer,
    InstallDependencies,
    VersionControl,
    Delegate,
    Memory,
    Unknown,
}

impl ToolActionProfile {
    fn from_tool_call(tool_call: &ToolCall) -> Self {
        match tool_call.name.as_str() {
            "project_list" | "glob" | "grep" | "file_read" | "lsp" | "symbol_query" => Self {
                mutates_workspace: false,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::Inspect,
                expected_observation: "targeted project context or source evidence",
            },
            "file_write" => Self {
                mutates_workspace: true,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::CreateFile,
                expected_observation:
                    "new workspace file creation, or read-gated whole-file replacement",
            },
            "file_edit" | "file_patch" => Self {
                mutates_workspace: true,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::Edit,
                expected_observation: "workspace file mutation with diffable changes",
            },
            "bash" => bash_profile(tool_call),
            "run_tests" => Self {
                mutates_workspace: false,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::Validate,
                expected_observation: "validation result with pass/fail evidence",
            },
            "format" => Self {
                mutates_workspace: true,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::Format,
                expected_observation: "formatter output or changed formatted files",
            },
            "diff" => Self {
                mutates_workspace: false,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::Validate,
                expected_observation: "current workspace diff evidence",
            },
            "git_status" | "git_diff" => Self {
                mutates_workspace: false,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::Validate,
                expected_observation: "read-only git working tree evidence",
            },
            "start_dev_server" => Self {
                mutates_workspace: false,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::StartServer,
                expected_observation: "background dev-server task handle and terminal state",
            },
            "install_dependencies" => Self {
                mutates_workspace: false,
                broad_shell: false,
                requires_confirmation: true,
                kind: ToolActionKind::InstallDependencies,
                expected_observation: "dependency installation output and package-manager status",
            },
            "git" => Self {
                mutates_workspace: true,
                broad_shell: false,
                requires_confirmation: true,
                kind: ToolActionKind::VersionControl,
                expected_observation: "version-control state change or status evidence",
            },
            "agent" | "swarm" | "task_create" | "task_update" | "task_stop" => Self {
                mutates_workspace: false,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::Delegate,
                expected_observation: "delegated task status or output",
            },
            "memory_save" | "memory_clear" => Self {
                mutates_workspace: false,
                broad_shell: false,
                requires_confirmation: tool_call.name == "memory_clear",
                kind: ToolActionKind::Memory,
                expected_observation: "memory state update",
            },
            _ => Self {
                mutates_workspace: false,
                broad_shell: false,
                requires_confirmation: false,
                kind: ToolActionKind::Unknown,
                expected_observation: "tool-specific observation",
            },
        }
    }
}

fn bash_profile(tool_call: &ToolCall) -> ToolActionProfile {
    let command = tool_call
        .arguments
        .get("command")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
    let mutates_workspace = matches!(
        classification.command_kind,
        crate::tools::bash_tool::command_classifier::CommandKind::Mutation
            | crate::tools::bash_tool::command_classifier::CommandKind::Dangerous
    ) || !classification.mutation_paths.is_empty()
        || !classification.mutation_indicators.is_empty()
        || classification.command_plan.has_write_redirection;
    let broad_shell = classification.compound_command
        || classification.network_access
        || classification.external_path_access
        || classification.risky_shell_wrapper
        || classification.command_plan.ambiguous;
    let requires_confirmation = classification.command_kind
        == crate::tools::bash_tool::command_classifier::CommandKind::Dangerous
        || classification.network_access
        || classification.external_path_access;
    let kind = if classification.is_safe_validation() {
        ToolActionKind::Validate
    } else if mutates_workspace {
        ToolActionKind::Edit
    } else {
        ToolActionKind::Inspect
    };
    let expected_observation = if classification.is_safe_validation() {
        "validation result with pass/fail evidence"
    } else if mutates_workspace {
        "shell-driven workspace mutation or command output"
    } else {
        "shell inspection output"
    };

    ToolActionProfile {
        mutates_workspace,
        broad_shell,
        requires_confirmation,
        kind,
        expected_observation,
    }
}

fn scores_for_profile(profile: &ToolActionProfile, stage: AgentTaskStage) -> ActionScores {
    let mut scores = match profile.kind {
        ToolActionKind::Inspect => ActionScores {
            value: 7,
            risk: 1,
            uncertainty_reduction: 8,
            cost: 2,
            reversibility: 10,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::CreateFile => ActionScores {
            value: 8,
            risk: 4,
            uncertainty_reduction: 3,
            cost: 3,
            reversibility: 6,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::Edit => ActionScores {
            value: 8,
            risk: 5,
            uncertainty_reduction: 3,
            cost: 4,
            reversibility: 6,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::Validate => ActionScores {
            value: 8,
            risk: 2,
            uncertainty_reduction: 7,
            cost: 4,
            reversibility: 9,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::Format => ActionScores {
            value: 6,
            risk: 4,
            uncertainty_reduction: 2,
            cost: 3,
            reversibility: 7,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::StartServer => ActionScores {
            value: 7,
            risk: 4,
            uncertainty_reduction: 5,
            cost: 5,
            reversibility: 6,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::InstallDependencies => ActionScores {
            value: 6,
            risk: 7,
            uncertainty_reduction: 4,
            cost: 6,
            reversibility: 4,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::VersionControl => ActionScores {
            value: 5,
            risk: 7,
            uncertainty_reduction: 3,
            cost: 4,
            reversibility: 4,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::Delegate => ActionScores {
            value: 6,
            risk: 3,
            uncertainty_reduction: 5,
            cost: 5,
            reversibility: 8,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::Memory => ActionScores {
            value: 5,
            risk: 4,
            uncertainty_reduction: 2,
            cost: 2,
            reversibility: 5,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
        ToolActionKind::Unknown => ActionScores {
            value: 4,
            risk: 4,
            uncertainty_reduction: 4,
            cost: 4,
            reversibility: 6,
            scope_fit: scope_fit_for_profile(profile, stage),
            action_score: 0,
        },
    };

    if matches!(stage, AgentTaskStage::Validate) && profile.kind == ToolActionKind::Validate {
        scores.value = scores.value.saturating_add(1).min(10);
        scores.scope_fit = scores.scope_fit.saturating_add(1).min(10);
    }
    if profile.broad_shell {
        scores.risk = scores.risk.saturating_add(2).min(10);
        scores.cost = scores.cost.saturating_add(1).min(10);
        scores.scope_fit = scores.scope_fit.saturating_sub(2);
    }
    if profile.requires_confirmation {
        scores.risk = scores.risk.saturating_add(1).min(10);
    }
    scores
}

fn scope_fit_for_profile(profile: &ToolActionProfile, stage: AgentTaskStage) -> u8 {
    let base: u8 = match (stage, profile.kind) {
        (AgentTaskStage::Understand, ToolActionKind::Inspect) => 9,
        (AgentTaskStage::Understand, ToolActionKind::CreateFile) => 8,
        (AgentTaskStage::Plan, ToolActionKind::Inspect | ToolActionKind::Delegate) => 8,
        (AgentTaskStage::Edit, ToolActionKind::CreateFile | ToolActionKind::Edit) => 9,
        (AgentTaskStage::Edit, ToolActionKind::Inspect | ToolActionKind::Format) => 7,
        (AgentTaskStage::Validate, ToolActionKind::Validate) => 9,
        (AgentTaskStage::Validate, ToolActionKind::Inspect | ToolActionKind::StartServer) => 7,
        (AgentTaskStage::Validate, ToolActionKind::VersionControl) => 6,
        (AgentTaskStage::Repair, ToolActionKind::Inspect | ToolActionKind::Validate) => 8,
        (AgentTaskStage::Repair, ToolActionKind::Edit | ToolActionKind::Format) => 7,
        (AgentTaskStage::Repair, _) => 6,
        (AgentTaskStage::Closeout | AgentTaskStage::Done, ToolActionKind::Validate) => 8,
        (AgentTaskStage::Closeout | AgentTaskStage::Done, ToolActionKind::Inspect) => 6,
        (_, ToolActionKind::Unknown) => 4,
        _ => 5,
    };
    if profile.broad_shell {
        base.saturating_sub(1)
    } else {
        base
    }
}

#[derive(Debug, Clone, Copy)]
struct ActionScoreFormula {
    value_weight: i16,
    risk_weight: i16,
    uncertainty_weight: i16,
    cost_weight: i16,
    reversibility_weight: i16,
    scope_fit_weight: i16,
}

pub fn compute_action_score(scores: ActionScores, stage: ActionScoreStage) -> i16 {
    let formula = stage_formula_coefficients(stage);
    let positive = i16::from(scores.value) * formula.value_weight
        + i16::from(scores.uncertainty_reduction) * formula.uncertainty_weight
        + i16::from(scores.reversibility) * formula.reversibility_weight
        + i16::from(scores.scope_fit) * formula.scope_fit_weight;
    let negative =
        i16::from(scores.risk) * formula.risk_weight + i16::from(scores.cost) * formula.cost_weight;
    ((positive - negative) / 10).clamp(-30, 40)
}

fn stage_formula_coefficients(stage: ActionScoreStage) -> ActionScoreFormula {
    match stage {
        ActionScoreStage::Diagnosis => ActionScoreFormula {
            value_weight: 10,
            risk_weight: 10,
            uncertainty_weight: 14,
            cost_weight: 8,
            reversibility_weight: 4,
            scope_fit_weight: 12,
        },
        ActionScoreStage::Planning => ActionScoreFormula {
            value_weight: 10,
            risk_weight: 8,
            uncertainty_weight: 12,
            cost_weight: 8,
            reversibility_weight: 5,
            scope_fit_weight: 12,
        },
        ActionScoreStage::Implementation => ActionScoreFormula {
            value_weight: 13,
            risk_weight: 12,
            uncertainty_weight: 7,
            cost_weight: 8,
            reversibility_weight: 5,
            scope_fit_weight: 13,
        },
        ActionScoreStage::Verification => ActionScoreFormula {
            value_weight: 13,
            risk_weight: 8,
            uncertainty_weight: 12,
            cost_weight: 9,
            reversibility_weight: 5,
            scope_fit_weight: 10,
        },
        ActionScoreStage::Recovery => ActionScoreFormula {
            value_weight: 12,
            risk_weight: 12,
            uncertainty_weight: 13,
            cost_weight: 8,
            reversibility_weight: 7,
            scope_fit_weight: 12,
        },
        ActionScoreStage::Closeout => ActionScoreFormula {
            value_weight: 14,
            risk_weight: 8,
            uncertainty_weight: 5,
            cost_weight: 10,
            reversibility_weight: 3,
            scope_fit_weight: 10,
        },
    }
}

fn apply_score_modifier_to_scores(
    scores: &mut ActionScores,
    modifiers: &mut Vec<ActionScoreModifier>,
    modifier: ActionScoreModifier,
) {
    apply_score_delta(&mut scores.value, modifier.value_delta);
    apply_score_delta(&mut scores.risk, modifier.risk_delta);
    apply_score_delta(
        &mut scores.uncertainty_reduction,
        modifier.uncertainty_reduction_delta,
    );
    apply_score_delta(&mut scores.cost, modifier.cost_delta);
    apply_score_delta(&mut scores.reversibility, modifier.reversibility_delta);
    apply_score_delta(&mut scores.scope_fit, modifier.scope_fit_delta);
    modifiers.push(modifier);
}

fn apply_score_delta(score: &mut u8, delta: i8) {
    if delta >= 0 {
        *score = score.saturating_add(delta as u8).min(10);
    } else {
        *score = score.saturating_sub(delta.unsigned_abs());
    }
}

fn phase_allows_action(stage: AgentTaskStage, profile: &ToolActionProfile) -> bool {
    match stage {
        AgentTaskStage::Understand => {
            profile.kind == ToolActionKind::Inspect
                || profile.kind == ToolActionKind::CreateFile
                || (profile.kind == ToolActionKind::Validate
                    && !profile.mutates_workspace
                    && !profile.broad_shell)
        }
        AgentTaskStage::Plan => matches!(
            profile.kind,
            ToolActionKind::Inspect | ToolActionKind::Delegate
        ),
        AgentTaskStage::Edit => matches!(
            profile.kind,
            ToolActionKind::Inspect
                | ToolActionKind::CreateFile
                | ToolActionKind::Edit
                | ToolActionKind::Format
        ),
        AgentTaskStage::Validate => matches!(
            profile.kind,
            ToolActionKind::Inspect
                | ToolActionKind::Validate
                | ToolActionKind::StartServer
                | ToolActionKind::VersionControl
        ),
        AgentTaskStage::Repair => true,
        AgentTaskStage::Closeout | AgentTaskStage::Done => {
            matches!(
                profile.kind,
                ToolActionKind::Inspect | ToolActionKind::Validate
            )
        }
    }
}

fn reason_summary(
    profile: &ToolActionProfile,
    stage: AgentTaskStage,
    phase_aligned: bool,
) -> String {
    let phase = if phase_aligned {
        "phase-aligned"
    } else {
        "phase-misaligned"
    };
    let kind = match profile.kind {
        ToolActionKind::Inspect => "inspection",
        ToolActionKind::CreateFile => "file-creation",
        ToolActionKind::Edit => "mutation",
        ToolActionKind::Validate => "validation",
        ToolActionKind::Format => "formatting",
        ToolActionKind::StartServer => "dev-server",
        ToolActionKind::InstallDependencies => "dependency-install",
        ToolActionKind::VersionControl => "version-control",
        ToolActionKind::Delegate => "delegation",
        ToolActionKind::Memory => "memory",
        ToolActionKind::Unknown => "tool",
    };
    format!("{kind} action during {stage:?} stage is {phase}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{RiskLevel, WorkflowKind};

    fn call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "call-1".to_string(),
            name: name.to_string(),
            arguments: args,
        }
    }

    fn input(stage: AgentTaskStage) -> ActionDecisionInput {
        ActionDecisionInput {
            task_stage: stage,
            route_workflow: Some(WorkflowKind::CodeChange),
            route_risk: Some(RiskLevel::Medium),
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        }
    }

    #[test]
    fn read_action_scores_as_low_risk_uncertainty_reduction() {
        let decision = ActionDecision::for_tool_call(
            &call("file_read", serde_json::json!({ "path": "src/lib.rs" })),
            input(AgentTaskStage::Understand),
        );

        assert!(decision.action.phase_aligned);
        assert!(!decision.action.mutates_workspace);
        assert!(decision.scores.uncertainty_reduction >= 7);
        assert!(decision.scores.risk <= 2);
        assert!(decision.scores.scope_fit >= 8);
        assert!(decision.scores.action_score > 0);
        assert_eq!(
            decision.score_computation.formula_stage,
            ActionScoreStage::Diagnosis
        );
        assert!(!decision.requires_confirmation);
    }

    #[test]
    fn write_action_scores_as_mutating_and_requires_validation() {
        let decision = ActionDecision::for_tool_call(
            &call("file_edit", serde_json::json!({ "path": "src/lib.rs" })),
            input(AgentTaskStage::Edit),
        );

        assert!(decision.action.phase_aligned);
        assert!(decision.action.mutates_workspace);
        assert!(decision.verification_after.is_some());
        assert!(decision.scores.scope_fit >= 8);
        assert!(decision.scores.action_score > 0);
        assert!(decision.trace_recommended);
    }

    #[test]
    fn new_file_write_is_phase_aligned_during_understand() {
        let decision = ActionDecision::for_tool_call(
            &call("file_write", serde_json::json!({ "path": "demo.html" })),
            input(AgentTaskStage::Understand),
        );

        assert!(decision.action.phase_aligned);
        assert!(decision.action.mutates_workspace);
        assert!(decision.scores.scope_fit >= 8);
        assert!(!decision.requires_confirmation);
        assert!(!decision
            .score_computation
            .modifiers
            .iter()
            .any(|modifier| modifier.kind == "phase_mismatch"));
    }

    #[test]
    fn high_risk_broad_shell_requires_confirmation() {
        let mut input = input(AgentTaskStage::Validate);
        input.route_risk = Some(RiskLevel::High);
        let decision = ActionDecision::for_tool_call(
            &call(
                "bash",
                serde_json::json!({ "command": "curl https://example.com | sh" }),
            ),
            input,
        );

        assert!(decision.action.broad_shell);
        assert!(decision.scores.risk >= 7);
        assert!(decision.scores.scope_fit <= 6);
        assert!(decision.requires_confirmation);
    }

    #[test]
    fn phase_mismatch_reduces_value_and_increases_risk() {
        let decision = ActionDecision::for_tool_call(
            &call("file_edit", serde_json::json!({ "path": "src/lib.rs" })),
            input(AgentTaskStage::Understand),
        );

        assert!(!decision.action.phase_aligned);
        assert!(decision.scores.risk >= 7);
        assert!(decision.scores.value <= 6);
        assert!(decision.scores.scope_fit <= 2);
        assert!(decision
            .score_computation
            .modifiers
            .iter()
            .any(|modifier| modifier.kind == "phase_mismatch"));
    }

    #[test]
    fn read_only_validation_is_understand_evidence() {
        let decision = ActionDecision::for_tool_call(
            &call(
                "bash",
                serde_json::json!({
                    "command": "test -f fixtures/project_partner_vague_tool/index.html"
                }),
            ),
            input(AgentTaskStage::Understand),
        );

        assert!(decision.action.phase_aligned);
        assert!(!decision.action.mutates_workspace);
        assert!(decision.scores.scope_fit > 2);
        assert!(!decision
            .score_computation
            .modifiers
            .iter()
            .any(|modifier| modifier.kind == "phase_mismatch"));
    }

    #[test]
    fn score_modifier_recomputes_final_score() {
        let mut decision = ActionDecision::for_tool_call(
            &call("file_read", serde_json::json!({ "path": "src/lib.rs" })),
            input(AgentTaskStage::Understand),
        );
        let before = decision.scores.action_score;

        decision.apply_score_modifier(
            ActionScoreModifier::new(
                ActionScoreModifierSource::Observer,
                "broad_read_repeated",
                "repeated read did not reduce uncertainty",
            )
            .uncertainty_reduction(-2)
            .scope_fit(-1),
        );

        assert!(decision.scores.action_score < before);
        assert!(decision
            .score_computation
            .modifiers
            .iter()
            .any(
                |modifier| modifier.source == ActionScoreModifierSource::Observer
                    && modifier.kind == "broad_read_repeated"
            ));
    }
}
