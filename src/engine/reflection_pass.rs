//! Structured reflection artifacts.
//!
//! Reflection turns informal self-review into a typed artifact that evalsets,
//! traces, and CLI dashboards can inspect.

use crate::engine::task_context::TaskContextBundle;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

        if matches!(
            bundle.route.workflow,
            crate::engine::intent_router::WorkflowKind::CodeChange
                | crate::engine::intent_router::WorkflowKind::BugFix
        ) {
            pass.checks.push(
                "karpathy-guidelines: assumptions, simplicity, surgical diff, verification"
                    .to_string(),
            );
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

    pub fn from_post_edit(
        task_id: impl Into<String>,
        changed_files: &[PathBuf],
        verification_passed: bool,
        evidence: &[String],
    ) -> Self {
        let mut pass = Self::new(task_id);
        pass.checks.push("post-edit changes reviewed".to_string());
        pass.checks
            .push(format!("changed files: {}", changed_files.len()));

        if changed_files.is_empty() {
            return pass;
        }

        if verification_passed {
            pass.checks
                .push("verification, tests, diagnostics, and code review passed".to_string());
            return pass;
        }

        pass.add_finding(ReflectionFinding {
            severity: ReflectionSeverity::Error,
            issue: "post-edit verification failed".to_string(),
            evidence: summarize_evidence(evidence),
            proposed_fix: Some(
                "inspect the failed verification output, update the changed files, and rerun the relevant checks"
                    .to_string(),
            ),
            fixed: false,
        });
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

    pub fn format_for_prompt(&self) -> String {
        let findings = if self.findings.is_empty() {
            "none".to_string()
        } else {
            self.findings
                .iter()
                .map(|finding| {
                    format!(
                        "- {:?}: {} | evidence: {}{}",
                        finding.severity,
                        finding.issue,
                        finding.evidence,
                        finding
                            .proposed_fix
                            .as_ref()
                            .map(|fix| format!(" | proposed fix: {}", fix))
                            .unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            "<reflection-pass id=\"{}\" status=\"{:?}\" unresolved=\"{}\">\nchecks:\n{}\nfindings:\n{}\n</reflection-pass>",
            self.pass_id,
            self.status,
            self.unresolved_count(),
            self.checks
                .iter()
                .map(|check| format!("- {}", check))
                .collect::<Vec<_>>()
                .join("\n"),
            findings
        )
    }
}

fn summarize_evidence(evidence: &[String]) -> String {
    let joined = evidence
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .take(4)
        .collect::<Vec<_>>()
        .join("\n---\n");
    if joined.is_empty() {
        "verification reported failure without detailed output".to_string()
    } else {
        let mut out = joined.chars().take(1200).collect::<String>();
        if joined.chars().count() > 1200 {
            out.push_str("...");
        }
        out
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
        assert!(pass
            .checks
            .iter()
            .any(|check| check.contains("karpathy-guidelines")));
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

    #[test]
    fn post_edit_reflection_blocks_failed_verification() {
        let pass = ReflectionPass::from_post_edit(
            "task-1",
            &[PathBuf::from("src/main.rs")],
            false,
            &["cargo test failed".to_string()],
        );
        assert_eq!(pass.status, ReflectionStatus::Blocked);
        assert_eq!(pass.unresolved_count(), 1);
        assert!(pass
            .format_for_prompt()
            .contains("post-edit verification failed"));
    }

    #[test]
    fn post_edit_reflection_passes_successful_verification() {
        let pass =
            ReflectionPass::from_post_edit("task-1", &[PathBuf::from("src/main.rs")], true, &[]);
        assert_eq!(pass.status, ReflectionStatus::Passed);
        assert_eq!(pass.unresolved_count(), 0);
    }
}
