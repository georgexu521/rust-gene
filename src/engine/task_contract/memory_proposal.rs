use super::{
    compact_text, dedupe, format_list_limited, ExecutionReport, ExecutionReportStatus,
    MemoryProposalCandidate, MemoryProposalStatus,
};
use serde::{Deserialize, Serialize};

fn default_memory_proposal_source() -> String {
    "closeout".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProposal {
    pub task_id: String,
    #[serde(default = "default_memory_proposal_source")]
    pub source: String,
    pub status: MemoryProposalStatus,
    pub candidates: Vec<MemoryProposalCandidate>,
    pub write_policy: String,
    pub write_performed: bool,
    pub reason: String,
}

impl MemoryProposal {
    pub fn from_execution_report(report: &ExecutionReport) -> Self {
        let mut candidates = Vec::new();
        match report.status {
            ExecutionReportStatus::Success => {
                if !report.changed_files.is_empty() && !report.validation_evidence.is_empty() {
                    candidates.push(MemoryProposalCandidate {
                        kind: "successful_fix".to_string(),
                        scope: "project".to_string(),
                        content: format!(
                            "Completed `{}` with changed files: {}; validation: {}",
                            compact_text(&report.objective, 180),
                            format_list_limited(&report.changed_files, 5),
                            compact_text(&report.validation_evidence.join("; "), 220)
                        ),
                        evidence: proposal_evidence(report),
                    });
                }
            }
            ExecutionReportStatus::Partial
            | ExecutionReportStatus::Failed
            | ExecutionReportStatus::NotVerified => {
                let has_evidence =
                    !report.validation_evidence.is_empty() || !report.risks.is_empty();
                if has_evidence {
                    candidates.push(MemoryProposalCandidate {
                        kind: "failure_pattern".to_string(),
                        scope: "project".to_string(),
                        content: format!(
                            "Task `{}` ended {}; risks: {}",
                            compact_text(&report.objective, 180),
                            report.status.label(),
                            format_list_limited(&report.risks, 5)
                        ),
                        evidence: proposal_evidence(report),
                    });
                }
            }
        }
        let status = if candidates.is_empty() {
            MemoryProposalStatus::NotApplicable
        } else {
            MemoryProposalStatus::Proposed
        };
        let reason = if candidates.is_empty() {
            "no durable evidence-backed memory candidate was produced".to_string()
        } else {
            "candidate memory requires review before persistence".to_string()
        };
        Self {
            task_id: report.task_id.clone(),
            source: "closeout".to_string(),
            status,
            candidates,
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason,
        }
    }

    pub fn evidence_items(&self) -> usize {
        self.candidates
            .iter()
            .map(|candidate| candidate.evidence.len())
            .sum()
    }

    pub fn candidate_kinds(&self) -> Vec<String> {
        let mut kinds = self
            .candidates
            .iter()
            .map(|candidate| candidate.kind.clone())
            .collect::<Vec<_>>();
        dedupe(&mut kinds);
        kinds
    }

    pub fn compact_summary(&self) -> String {
        format!(
            "MemoryProposal id={} status={} candidates={} write_policy={} write_performed={} evidence={}",
            self.task_id,
            self.status.label(),
            self.candidates.len(),
            self.write_policy,
            self.write_performed,
            self.evidence_items()
        )
    }

    pub fn format_for_final_response(&self) -> String {
        if self.candidates.is_empty() {
            return String::new();
        }
        let mut lines = vec![
            "\nMemory proposal:".to_string(),
            format!(
                "- Status: {} candidates={} evidence={}",
                self.status.label(),
                self.candidates.len(),
                self.evidence_items()
            ),
            format!(
                "- Write policy: {} write_performed={}",
                self.write_policy, self.write_performed
            ),
            format!("- Reason: {}", self.reason),
        ];
        for candidate in self.candidates.iter().take(3) {
            lines.push(format!(
                "- Candidate: kind={} scope={} evidence={} :: {}",
                candidate.kind,
                candidate.scope,
                candidate.evidence.len(),
                compact_text(&candidate.content, 180)
            ));
        }
        lines.join("\n")
    }
}

fn proposal_evidence(report: &ExecutionReport) -> Vec<String> {
    let mut evidence = Vec::new();
    for file in &report.changed_files {
        push_unique(&mut evidence, format!("changed_file: {file}"));
    }
    for validation in &report.validation_evidence {
        push_unique(
            &mut evidence,
            format!("validation: {}", compact_text(validation, 220)),
        );
    }
    for risk in &report.risks {
        if risk != "none recorded" {
            push_unique(&mut evidence, format!("risk: {}", compact_text(risk, 180)));
        }
    }
    evidence
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !items.iter().any(|item| item == &value) {
        items.push(value);
    }
}
