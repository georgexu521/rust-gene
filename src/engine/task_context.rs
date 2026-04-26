//! Task context bundle for non-trivial coding and agent workflows.
//!
//! A bundle captures the active goal, route, retrieved evidence, constraints,
//! risks, budgets, and acceptance checks that should travel with a task.

use crate::engine::intent_router::IntentRoute;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::session_goal::SessionGoal;
use crate::engine::workflow_contract::ProgrammingWorkflowJudgment;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContextBundle {
    pub task_id: String,
    pub prompt_preview: String,
    pub working_dir: PathBuf,
    pub goal: Option<SessionGoal>,
    pub route: IntentRoute,
    pub relevant_files: Vec<PathBuf>,
    pub constraints: Vec<String>,
    pub retrieval: Option<RetrievalContext>,
    pub risks: Vec<String>,
    pub tool_budget: TaskToolBudget,
    pub acceptance_checks: Vec<String>,
    pub workflow_judgment: Option<ProgrammingWorkflowJudgment>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TaskToolBudget {
    pub max_tool_calls: usize,
    pub max_seconds: u64,
    pub max_parallel: usize,
}

impl Default for TaskToolBudget {
    fn default() -> Self {
        Self {
            max_tool_calls: 25,
            max_seconds: 600,
            max_parallel: 4,
        }
    }
}

impl TaskContextBundle {
    pub fn new(
        prompt: &str,
        working_dir: impl AsRef<Path>,
        route: IntentRoute,
        goal: Option<SessionGoal>,
    ) -> Self {
        Self {
            task_id: uuid::Uuid::new_v4().to_string(),
            prompt_preview: preview(prompt, 160),
            working_dir: working_dir.as_ref().to_path_buf(),
            goal,
            route,
            relevant_files: Vec::new(),
            constraints: Vec::new(),
            retrieval: None,
            risks: Vec::new(),
            tool_budget: TaskToolBudget::default(),
            acceptance_checks: Vec::new(),
            workflow_judgment: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_retrieval(mut self, retrieval: RetrievalContext) -> Self {
        self.retrieval = Some(retrieval);
        self
    }

    pub fn add_file(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        if !self.relevant_files.contains(&path) {
            self.relevant_files.push(path);
        }
    }

    pub fn add_constraint(&mut self, constraint: impl Into<String>) {
        push_unique(&mut self.constraints, constraint.into());
    }

    pub fn add_risk(&mut self, risk: impl Into<String>) {
        push_unique(&mut self.risks, risk.into());
    }

    pub fn add_acceptance_check(&mut self, check: impl Into<String>) {
        push_unique(&mut self.acceptance_checks, check.into());
    }

    pub fn apply_workflow_judgment(&mut self, judgment: ProgrammingWorkflowJudgment) {
        for assumption in &judgment.assumptions {
            self.add_constraint(format!("assumption: {}", assumption));
        }
        for risk in judgment.risk_notes() {
            self.add_risk(risk);
        }
        for check in judgment.acceptance_checks() {
            self.add_acceptance_check(check);
        }
        self.workflow_judgment = Some(judgment);
    }

    pub fn needs_stronger_acceptance(&self) -> bool {
        matches!(
            self.route.workflow,
            crate::engine::intent_router::WorkflowKind::CodeChange
                | crate::engine::intent_router::WorkflowKind::BugFix
        ) && self.acceptance_checks.is_empty()
    }
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !items.contains(&value) {
        items.push(value);
    }
}

fn preview(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn bundle_flags_missing_acceptance_for_code_change() {
        let route = IntentRouter::new().route("修改 CLI 状态栏");
        let bundle = TaskContextBundle::new("修改 CLI 状态栏", ".", route, None);
        assert!(bundle.needs_stronger_acceptance());
    }

    #[test]
    fn bundle_deduplicates_context_lists() {
        let route = IntentRouter::new().route("你好");
        let mut bundle = TaskContextBundle::new("你好", ".", route, None);
        bundle.add_constraint("keep it short");
        bundle.add_constraint("keep it short");
        bundle.add_file("src/main.rs");
        bundle.add_file("src/main.rs");
        assert_eq!(bundle.constraints.len(), 1);
        assert_eq!(bundle.relevant_files.len(), 1);
    }

    #[test]
    fn bundle_applies_model_workflow_judgment() {
        let route = IntentRouter::new().route("实现一个网站");
        let mut bundle = TaskContextBundle::new("实现一个网站", ".", route, None);
        let judgment = crate::engine::workflow_contract::ProgrammingWorkflowJudgment {
            task_type: "website".into(),
            complexity: crate::engine::workflow_contract::TaskComplexity::Medium,
            risk: crate::engine::intent_router::RiskLevel::Medium,
            requirement_complete_enough: true,
            needs_user_questions: false,
            question_reason: None,
            questions: Vec::new(),
            assumptions: vec!["Use local storage".into()],
            guided_reasoning_required: false,
            guided_reasoning_triggers: Vec::new(),
            plan: Vec::new(),
            acceptance: crate::engine::workflow_contract::AcceptanceContract::pending(
                "实现一个网站",
                vec!["Main page renders".into()],
                Vec::new(),
            ),
        };

        bundle.apply_workflow_judgment(judgment);

        assert!(bundle.workflow_judgment.is_some());
        assert!(bundle
            .constraints
            .iter()
            .any(|item| item.contains("Use local storage")));
        assert!(bundle
            .acceptance_checks
            .iter()
            .any(|item| item == "Main page renders"));
        assert!(!bundle.needs_stronger_acceptance());
    }
}
