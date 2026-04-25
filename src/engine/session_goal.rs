//! Runtime session goal tracking.
//!
//! This is intentionally lightweight: it records what the current session is
//! trying to accomplish so the CLI can make the active objective visible.

use crate::engine::intent_router::{IntentKind, IntentRoute, WorkflowKind};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Waiting,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionGoal {
    pub id: String,
    pub title: String,
    pub status: GoalStatus,
    pub intent: IntentKind,
    pub workflow: WorkflowKind,
    pub acceptance_criteria: Vec<String>,
    pub last_user_message: String,
    pub updated_at: String,
}

impl SessionGoal {
    fn new(title: String, route: &IntentRoute, user_message: &str) -> Self {
        Self {
            id: new_goal_id(),
            title,
            status: GoalStatus::Active,
            intent: route.intent,
            workflow: route.workflow,
            acceptance_criteria: acceptance_criteria_for(route),
            last_user_message: user_message.trim().to_string(),
            updated_at: now_label(),
        }
    }

    fn update(&mut self, title: String, route: &IntentRoute, user_message: &str) {
        self.title = title;
        self.status = GoalStatus::Active;
        self.intent = route.intent;
        self.workflow = route.workflow;
        self.acceptance_criteria = acceptance_criteria_for(route);
        self.last_user_message = user_message.trim().to_string();
        self.updated_at = now_label();
    }

    pub fn compact_status(&self) -> String {
        format!(
            "{} [{:?}/{:?}, {:?}]",
            self.title, self.intent, self.workflow, self.status
        )
    }

    pub fn format(&self) -> String {
        let criteria = if self.acceptance_criteria.is_empty() {
            "  - none".to_string()
        } else {
            self.acceptance_criteria
                .iter()
                .map(|item| format!("  - {}", item))
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            "Current Goal\n- Id: {}\n- Title: {}\n- Status: {:?}\n- Intent: {:?}\n- Workflow: {:?}\n- Updated: {}\n\nAcceptance criteria:\n{}\n\nLast request:\n{}",
            self.id,
            self.title,
            self.status,
            self.intent,
            self.workflow,
            self.updated_at,
            criteria,
            self.last_user_message
        )
    }
}

#[derive(Debug, Default, Clone)]
pub struct SessionGoalManager {
    current: Arc<RwLock<Option<SessionGoal>>>,
}

impl SessionGoalManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> Option<SessionGoal> {
        self.current
            .read()
            .map(|goal| goal.clone())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut current) = self.current.write() {
            *current = None;
        }
    }

    pub fn set_manual(&self, title: impl Into<String>) -> Option<SessionGoal> {
        let title = normalize_title(&title.into());
        if title.is_empty() {
            return None;
        }
        let route = IntentRoute {
            intent: IntentKind::Planning,
            confidence: 1.0,
            workflow: WorkflowKind::Planning,
            retrieval: crate::engine::intent_router::RetrievalPolicy::Project,
            reasoning: crate::engine::intent_router::ReasoningPolicy::High,
            risk: crate::engine::intent_router::RiskLevel::Medium,
            recommended_tools: Vec::new(),
            reason: "manual goal".to_string(),
        };
        let goal = SessionGoal::new(title.clone(), &route, &title);
        if let Ok(mut current) = self.current.write() {
            *current = Some(goal.clone());
        }
        Some(goal)
    }

    pub fn update_from_user_message(
        &self,
        user_message: &str,
        route: Option<&IntentRoute>,
    ) -> Option<SessionGoal> {
        let text = user_message.trim();
        if text.is_empty() || text.starts_with('/') {
            return None;
        }

        let fallback_route;
        let route = match route {
            Some(route) => route,
            None => {
                fallback_route = crate::engine::intent_router::IntentRouter::new().route(text);
                &fallback_route
            }
        };

        if !should_track(route, text) {
            return None;
        }

        let title = normalize_title(text);
        if title.is_empty() {
            return None;
        }

        let goal = match self.current.write() {
            Ok(mut current) => {
                if let Some(existing) = current.as_mut() {
                    existing.update(title, route, text);
                    existing.clone()
                } else {
                    let goal = SessionGoal::new(title, route, text);
                    *current = Some(goal.clone());
                    goal
                }
            }
            Err(poisoned) => {
                let mut current = poisoned.into_inner();
                let goal = SessionGoal::new(title, route, text);
                *current = Some(goal.clone());
                goal
            }
        };
        Some(goal)
    }

    pub fn format_current(&self) -> String {
        self.current().map(|goal| goal.format()).unwrap_or_else(|| {
            "Current Goal\n- none\n\nUse /goal set <text> to pin a goal.".to_string()
        })
    }
}

fn should_track(route: &IntentRoute, text: &str) -> bool {
    if text.chars().count() < 8 {
        return false;
    }

    matches!(
        route.intent,
        IntentKind::CodeChange
            | IntentKind::Debugging
            | IntentKind::Research
            | IntentKind::Configuration
            | IntentKind::Delegation
            | IntentKind::Planning
    )
}

fn acceptance_criteria_for(route: &IntentRoute) -> Vec<String> {
    match route.intent {
        IntentKind::CodeChange => vec![
            "Relevant code paths inspected before edits".to_string(),
            "Requested behavior implemented with scoped changes".to_string(),
            "Verification run or skipped with an explicit reason".to_string(),
        ],
        IntentKind::Debugging => vec![
            "Failure reproduced or concrete evidence inspected".to_string(),
            "Root cause identified before changing code".to_string(),
            "Fix verified against the failing path".to_string(),
        ],
        IntentKind::Research => vec![
            "Primary or reliable sources inspected".to_string(),
            "Findings connected back to this project".to_string(),
            "Actionable next step captured".to_string(),
        ],
        IntentKind::Delegation => vec![
            "Work split into bounded responsibilities".to_string(),
            "Results integrated into one coherent outcome".to_string(),
            "Conflicts or blockers surfaced clearly".to_string(),
        ],
        IntentKind::Planning | IntentKind::Configuration => vec![
            "Current project state inspected".to_string(),
            "Plan or configuration change is concrete".to_string(),
            "Risks and verification steps are visible".to_string(),
        ],
        _ => vec!["User request addressed".to_string()],
    }
}

fn normalize_title(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let max_chars = 80;
    let mut title = compact.chars().take(max_chars).collect::<String>();
    if compact.chars().count() > max_chars {
        title.push_str("...");
    }
    title
}

fn new_goal_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("goal_{}", millis)
}

fn now_label() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn tracks_code_change_goal() {
        let manager = SessionGoalManager::new();
        let route = IntentRouter::new().route("继续优化 CLI 体验，完善状态栏");
        let goal = manager
            .update_from_user_message("继续优化 CLI 体验，完善状态栏", Some(&route))
            .expect("goal");
        assert_eq!(goal.intent, IntentKind::CodeChange);
        assert!(goal.title.contains("CLI"));
    }

    #[test]
    fn ignores_short_direct_messages() {
        let manager = SessionGoalManager::new();
        let route = IntentRouter::new().route("你好");
        assert!(manager
            .update_from_user_message("你好", Some(&route))
            .is_none());
        assert!(manager.current().is_none());
    }

    #[test]
    fn manual_goal_can_be_set_and_cleared() {
        let manager = SessionGoalManager::new();
        let goal = manager.set_manual("ship trace visibility").expect("goal");
        assert_eq!(goal.title, "ship trace visibility");
        assert!(manager.current().is_some());
        manager.clear();
        assert!(manager.current().is_none());
    }
}
