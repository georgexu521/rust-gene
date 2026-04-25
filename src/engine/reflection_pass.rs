//! Structured reflection artifacts.
//!
//! Reflection turns informal self-review into a typed artifact that evalsets,
//! traces, and CLI dashboards can inspect.

use crate::engine::task_context::TaskContextBundle;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReflectionStatus {
    Passed,
    NeedsWork,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReflectionSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionFinding {
    pub severity: ReflectionSeverity,
    pub issue: String,
    pub evidence: String,
    pub proposed_fix: Option<String>,
    pub fixed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionPass {
    pub pass_id: String,
    pub task_id: String,
    pub status: ReflectionStatus,
    pub findings: Vec<ReflectionFinding>,
    pub checks: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl ReflectionPass {
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            pass_id: uuid::Uuid::new_v4().to_string(),
            task_id: task_id.into(),
            status: ReflectionStatus::Passed,
            findings: Vec::new(),
            checks: Vec::new(),
            created_at: Utc::now(),
        }
    }

    pub fn from_task_bundle(bundle: &TaskContextBundle) -> Self {
        let mut pass = Self::new(bundle.task_id.clone());
        pass.checks.push("task context bundle reviewed".to_string());

        if bundle.needs_stronger_acceptance() {
            pass.add_finding(ReflectionFinding {
                severity: ReflectionSeverity::Warning,
                issue: "missing acceptance checks".to_string(),
                evidence: "code-change or bug-fix task has no acceptance criteria".to_string(),
                proposed_fix: Some(
                    "add at least one concrete verification command or check".into(),
                ),
                fixed: false,
            });
        }

        if bundle.retrieval.is_none()
            && matches!(
                bundle.route.retrieval,
                crate::engine::intent_router::RetrievalPolicy::Project
                    | crate::engine::intent_router::RetrievalPolicy::Full
            )
        {
            pass.add_finding(ReflectionFinding {
                severity: ReflectionSeverity::Info,
                issue: "retrieval context not attached".to_string(),
                evidence:
                    "router requested project/full retrieval but bundle has no retrieval context"
                        .to_string(),
                proposed_fix: Some(
                    "attach project or memory retrieval evidence before execution".into(),
                ),
                fixed: false,
            });
        }

        pass
    }

    pub fn add_finding(&mut self, finding: ReflectionFinding) {
        if matches!(finding.severity, ReflectionSeverity::Error) {
            self.status = ReflectionStatus::Blocked;
        } else if matches!(finding.severity, ReflectionSeverity::Warning)
            && self.status == ReflectionStatus::Passed
        {
            self.status = ReflectionStatus::NeedsWork;
        }
        self.findings.push(finding);
    }

    pub fn unresolved_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| !finding.fixed)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::task_context::TaskContextBundle;

    #[test]
    fn reflection_warns_when_acceptance_is_missing() {
        let route = IntentRouter::new().route("修复 cargo test 报错");
        let bundle = TaskContextBundle::new("修复 cargo test 报错", ".", route, None);
        let pass = ReflectionPass::from_task_bundle(&bundle);
        assert_eq!(pass.status, ReflectionStatus::NeedsWork);
        assert!(pass
            .findings
            .iter()
            .any(|finding| finding.issue == "missing acceptance checks"));
    }

    #[test]
    fn reflection_passes_when_context_is_sufficient() {
        let route = IntentRouter::new().route("你好");
        let mut bundle = TaskContextBundle::new("你好", ".", route, None);
        bundle.add_acceptance_check("answer directly");
        let pass = ReflectionPass::from_task_bundle(&bundle);
        assert_eq!(pass.status, ReflectionStatus::Passed);
        assert_eq!(pass.unresolved_count(), 0);
    }
}
