//! Bounded route/task-mode recovery policy.
//!
//! Initial routing is advisory, not a single point of failure. This module
//! records drift signals and permits narrow recovery that can expand
//! understanding tools without silently expanding mutation authority.

use crate::engine::intent_router::WorkflowKind;
use crate::engine::recovery_plan::{RecoveryPlan, RecoveryStatus};
use serde::{Deserialize, Serialize};

const SAFE_READ_SEARCH_TOOLS: &[&str] = &[
    "project_list",
    "glob",
    "grep",
    "file_read",
    "lsp",
    "symbol_query",
    "ask_user",
];

const MUTATION_TOOLS: &[&str] = &[
    "file_edit",
    "file_write",
    "file_patch",
    "format",
    "install_dependencies",
    "git",
    "git_push",
    "worktree",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteRecoveryDriftSignal {
    HiddenReadSearchToolRequested,
    HiddenMutationToolRequested,
    CodeChangeNoDiffAfterRepeatedProgress,
}

impl RouteRecoveryDriftSignal {
    pub fn label(self) -> &'static str {
        match self {
            Self::HiddenReadSearchToolRequested => "hidden_read_search_tool_requested",
            Self::HiddenMutationToolRequested => "hidden_mutation_tool_requested",
            Self::CodeChangeNoDiffAfterRepeatedProgress => {
                "code_change_no_diff_after_repeated_progress"
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouteRecoveryDecision {
    pub signal: RouteRecoveryDriftSignal,
    pub expanded_read_search: bool,
    pub mode_escalates_to_light: bool,
    pub plan: RecoveryPlan,
    pub correction: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RouteRecoveryRuntimeState {
    pub read_search_expanded: bool,
    pub read_search_expansions: usize,
    pub blocked_mutation_requests: usize,
    pub last_signal: Option<RouteRecoveryDriftSignal>,
}

impl RouteRecoveryRuntimeState {
    pub fn observe_unexposed_tool_request(
        &mut self,
        route_workflow: WorkflowKind,
        current_mode: impl Into<String>,
        tool_name: &str,
        error: &str,
    ) -> Option<RouteRecoveryDecision> {
        if is_safe_read_search_tool(tool_name) {
            return self.expand_read_search_once(route_workflow, current_mode, tool_name, error);
        }

        if is_mutation_tool(tool_name) {
            self.blocked_mutation_requests = self.blocked_mutation_requests.saturating_add(1);
            self.last_signal = Some(RouteRecoveryDriftSignal::HiddenMutationToolRequested);
            return Some(blocked_mutation_decision(route_workflow, tool_name, error));
        }

        None
    }

    fn expand_read_search_once(
        &mut self,
        route_workflow: WorkflowKind,
        current_mode: impl Into<String>,
        tool_name: &str,
        error: &str,
    ) -> Option<RouteRecoveryDecision> {
        self.last_signal = Some(RouteRecoveryDriftSignal::HiddenReadSearchToolRequested);
        if self.read_search_expanded {
            return None;
        }

        self.read_search_expanded = true;
        self.read_search_expansions = self.read_search_expansions.saturating_add(1);
        Some(expand_read_search_decision(
            route_workflow,
            current_mode.into(),
            tool_name,
            error,
        ))
    }
}

pub fn is_unexposed_tool_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("was not exposed")
        || lower.contains("not exposed in the current")
        || lower.contains("tool is not exposed")
}

pub fn is_safe_read_search_tool(tool_name: &str) -> bool {
    SAFE_READ_SEARCH_TOOLS.contains(&tool_name)
}

pub fn is_mutation_tool(tool_name: &str) -> bool {
    MUTATION_TOOLS.contains(&tool_name)
}

pub fn safe_read_search_tools() -> &'static [&'static str] {
    SAFE_READ_SEARCH_TOOLS
}

fn expand_read_search_decision(
    route_workflow: WorkflowKind,
    current_mode: String,
    tool_name: &str,
    error: &str,
) -> RouteRecoveryDecision {
    let signal = RouteRecoveryDriftSignal::HiddenReadSearchToolRequested;
    let action = format!(
        "expand read/search tools for corrected route understanding; requested_tool={} route_workflow={:?} current_mode={}",
        tool_name, route_workflow, current_mode
    );
    RouteRecoveryDecision {
        signal,
        expanded_read_search: true,
        mode_escalates_to_light: current_mode == "direct",
        plan: RecoveryPlan {
            id: format!("route_recovery_{}", uuid::Uuid::new_v4().simple()),
            source: "route_recovery".to_string(),
            category: "route_drift".to_string(),
            failure_type: signal.label().to_string(),
            recovery_kind: "expand_read_search_only".to_string(),
            primary_error: truncate(error, 240),
            action: action.clone(),
            retryable: true,
            safe_retry: true,
            allowed_alternatives: SAFE_READ_SEARCH_TOOLS
                .iter()
                .map(|tool| (*tool).to_string())
                .collect(),
            retry_budget: Some(1),
            side_effect_uncertain: false,
            requires_user_decision: false,
            suggested_command: None,
            user_note: "Route recovery expanded only read/search tools; mutation tools remain gated."
                .to_string(),
            status: RecoveryStatus::Applied,
        },
        correction: format!(
            "Route recovery: `{}` was hidden by the initial route, so the next request may use read/search tools only ({}). Do not treat this as permission to edit or run destructive actions.",
            tool_name,
            SAFE_READ_SEARCH_TOOLS.join(", ")
        ),
    }
}

fn blocked_mutation_decision(
    route_workflow: WorkflowKind,
    tool_name: &str,
    error: &str,
) -> RouteRecoveryDecision {
    let signal = RouteRecoveryDriftSignal::HiddenMutationToolRequested;
    RouteRecoveryDecision {
        signal,
        expanded_read_search: false,
        mode_escalates_to_light: false,
        plan: RecoveryPlan {
            id: format!("route_recovery_{}", uuid::Uuid::new_v4().simple()),
            source: "route_recovery".to_string(),
            category: "route_drift".to_string(),
            failure_type: signal.label().to_string(),
            recovery_kind: "no_silent_mutation_expansion".to_string(),
            primary_error: truncate(error, 240),
            action: format!(
                "keep mutation tool hidden; requested_tool={} route_workflow={:?}; require task contract or user intent before mutation",
                tool_name, route_workflow
            ),
            retryable: false,
            safe_retry: false,
            allowed_alternatives: SAFE_READ_SEARCH_TOOLS
                .iter()
                .map(|tool| (*tool).to_string())
                .collect(),
            retry_budget: None,
            side_effect_uncertain: true,
            requires_user_decision: true,
            suggested_command: None,
            user_note:
                "Route recovery did not expand mutation authority; re-plan or ask for explicit mutation scope."
                    .to_string(),
            status: RecoveryStatus::Planned,
        },
        correction: format!(
            "Route recovery: `{}` remains hidden because route drift cannot silently expand mutation authority. Use read/search evidence or ask for explicit mutation scope.",
            tool_name
        ),
    }
}

pub fn no_diff_code_change_decision(
    route_workflow: WorkflowKind,
    no_code_progress_rounds: usize,
    reason: &str,
) -> RouteRecoveryDecision {
    let signal = RouteRecoveryDriftSignal::CodeChangeNoDiffAfterRepeatedProgress;
    let action = format!(
        "recover code-change no-diff drift without expanding mutation authority; route_workflow={:?} no_code_progress_rounds={}",
        route_workflow, no_code_progress_rounds
    );
    RouteRecoveryDecision {
        signal,
        expanded_read_search: false,
        mode_escalates_to_light: false,
        plan: RecoveryPlan {
            id: format!("route_recovery_{}", uuid::Uuid::new_v4().simple()),
            source: "route_recovery".to_string(),
            category: "route_drift".to_string(),
            failure_type: signal.label().to_string(),
            recovery_kind: "code_change_no_diff_replan".to_string(),
            primary_error: truncate(reason, 240),
            action: action.clone(),
            retryable: true,
            safe_retry: true,
            allowed_alternatives: vec![
                "replan_under_code_change_contract".to_string(),
                "targeted_lookup_if_missing_anchor".to_string(),
                "honest_not_verified_closeout".to_string(),
            ],
            retry_budget: Some(1),
            side_effect_uncertain: false,
            requires_user_decision: false,
            suggested_command: None,
            user_note:
                "Route recovery recorded code-change no-diff drift; it does not expand mutation tools."
                    .to_string(),
            status: RecoveryStatus::Applied,
        },
        correction: format!(
            "Route recovery: this {:?} task has made {} successful no-diff progress round(s). Re-plan under the existing task contract, make the smallest scoped patch if safe, or close out as not_verified with a concrete blocker. This does not grant any new mutation authority.",
            route_workflow, no_code_progress_rounds
        ),
    }
}

fn truncate(text: &str, max: usize) -> String {
    let mut out = text.chars().take(max).collect::<String>();
    if text.chars().count() > max {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_read_search_request_expands_once() {
        let mut state = RouteRecoveryRuntimeState::default();
        let decision = state
            .observe_unexposed_tool_request(
                WorkflowKind::Direct,
                "direct",
                "file_read",
                "Tool 'file_read' was not exposed in the current request.",
            )
            .expect("read request should recover");

        assert!(state.read_search_expanded);
        assert_eq!(state.read_search_expansions, 1);
        assert!(decision.expanded_read_search);
        assert!(decision.mode_escalates_to_light);
        assert_eq!(
            decision.plan.recovery_kind,
            "expand_read_search_only".to_string()
        );
        assert!(decision
            .plan
            .allowed_alternatives
            .contains(&"file_read".to_string()));

        let second = state.observe_unexposed_tool_request(
            WorkflowKind::Direct,
            "light",
            "grep",
            "Tool 'grep' was not exposed in the current request.",
        );
        assert!(second.is_none());
        assert_eq!(state.read_search_expansions, 1);
    }

    #[test]
    fn hidden_mutation_request_does_not_expand_tools() {
        let mut state = RouteRecoveryRuntimeState::default();
        let decision = state
            .observe_unexposed_tool_request(
                WorkflowKind::Research,
                "light",
                "file_edit",
                "Tool 'file_edit' was not exposed in the current request.",
            )
            .expect("mutation request should record a blocked decision");

        assert!(!state.read_search_expanded);
        assert_eq!(state.blocked_mutation_requests, 1);
        assert!(!decision.expanded_read_search);
        assert!(!decision.plan.safe_retry);
        assert!(decision.plan.requires_user_decision);
        assert_eq!(
            decision.plan.recovery_kind,
            "no_silent_mutation_expansion".to_string()
        );
    }

    #[test]
    fn unexposed_error_detection_is_specific() {
        assert!(is_unexposed_tool_error(
            "Tool 'bash' was not exposed in the current request and cannot be executed."
        ));
        assert!(!is_unexposed_tool_error("command exited with status 101"));
    }

    #[test]
    fn no_diff_code_change_recovery_does_not_add_mutation_alternatives() {
        let decision = no_diff_code_change_decision(
            WorkflowKind::CodeChange,
            3,
            "code-change task made no edit after repeated inspection",
        );

        assert_eq!(
            decision.signal,
            RouteRecoveryDriftSignal::CodeChangeNoDiffAfterRepeatedProgress
        );
        assert!(!decision.expanded_read_search);
        assert_eq!(decision.plan.recovery_kind, "code_change_no_diff_replan");
        assert!(decision.plan.safe_retry);
        assert!(!decision.plan.requires_user_decision);
        assert!(decision
            .plan
            .allowed_alternatives
            .iter()
            .all(|tool| !is_mutation_tool(tool)));
    }
}
