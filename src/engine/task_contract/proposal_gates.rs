use super::{
    MemoryProposal, MemoryProposalCandidate, MemoryProposalConflictGroup,
    MemoryProposalGateDecision,
};

pub(crate) fn memory_write_target_for_proposal_candidate(
    candidate: &MemoryProposalCandidate,
) -> crate::memory::MemoryWriteTarget {
    let scope = candidate.scope.trim();
    if let Some(topic) = scope.strip_prefix("topic:") {
        if let Some(topic) = normalize_proposal_scope_component(topic) {
            return crate::memory::MemoryWriteTarget::Topic(topic);
        }
    }
    match scope {
        "user" => crate::memory::MemoryWriteTarget::User,
        "topic" => crate::memory::MemoryWriteTarget::Topic(candidate.kind.clone()),
        "project" => crate::memory::MemoryWriteTarget::Index,
        _ => crate::memory::MemoryWriteTarget::Auto,
    }
}

pub(crate) fn proposal_gate_report(
    proposal: &MemoryProposal,
    conflict_groups: &[MemoryProposalConflictGroup],
) -> Vec<MemoryProposalGateDecision> {
    let mut gates = Vec::new();
    gates.push(MemoryProposalGateDecision {
        gate: "write_policy".to_string(),
        candidate_index: None,
        status: if proposal.write_policy == "review_required" {
            "passed".to_string()
        } else {
            "warn".to_string()
        },
        reason: format!("write_policy={}", proposal.write_policy),
    });
    gates.push(MemoryProposalGateDecision {
        gate: "evidence".to_string(),
        candidate_index: None,
        status: if proposal.evidence_items() > 0 {
            "passed".to_string()
        } else {
            "missing".to_string()
        },
        reason: format!("evidence_items={}", proposal.evidence_items()),
    });
    let evidence_findings = proposal_evidence_minimum_findings(proposal);
    let missing_evidence = evidence_findings
        .iter()
        .filter(|finding| finding.status == "missing")
        .count();
    let review_required_evidence = evidence_findings
        .iter()
        .filter(|finding| finding.status == "review_required")
        .count();
    gates.push(MemoryProposalGateDecision {
        gate: "minimum_evidence".to_string(),
        candidate_index: None,
        status: if missing_evidence > 0 {
            "missing".to_string()
        } else if review_required_evidence > 0 {
            "review_required".to_string()
        } else {
            "passed".to_string()
        },
        reason: if evidence_findings.is_empty() {
            "candidate_evidence=0".to_string()
        } else {
            evidence_findings
                .iter()
                .map(|finding| {
                    format!(
                        "{}:{}:{}",
                        finding.kind, finding.status, finding.requirement
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    for (idx, finding) in evidence_findings.iter().enumerate() {
        gates.push(MemoryProposalGateDecision {
            gate: "minimum_evidence".to_string(),
            candidate_index: Some(idx),
            status: finding.status.to_string(),
            reason: format!("{}:{}", finding.kind, finding.requirement),
        });
    }
    let sensitivity_findings = proposal_sensitivity_findings(proposal);
    let blocked_sensitivity = sensitivity_findings
        .iter()
        .filter(|finding| finding.status == "blocked")
        .count();
    let review_required_sensitivity = sensitivity_findings
        .iter()
        .filter(|finding| finding.status == "review_required")
        .count();
    gates.push(MemoryProposalGateDecision {
        gate: "sensitivity".to_string(),
        candidate_index: None,
        status: if blocked_sensitivity > 0 {
            "blocked".to_string()
        } else if review_required_sensitivity > 0 {
            "review_required".to_string()
        } else {
            "passed".to_string()
        },
        reason: if sensitivity_findings.is_empty() {
            "candidate_sensitivity=0".to_string()
        } else {
            sensitivity_findings
                .iter()
                .map(|finding| {
                    format!(
                        "{}:{}:{}:{}",
                        finding.kind, finding.status, finding.sensitivity, finding.reason
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    for (idx, finding) in sensitivity_findings.iter().enumerate() {
        gates.push(MemoryProposalGateDecision {
            gate: "sensitivity".to_string(),
            candidate_index: Some(idx),
            status: finding.status.to_string(),
            reason: format!(
                "{}:{}:{}",
                finding.kind, finding.sensitivity, finding.reason
            ),
        });
    }
    gates.push(MemoryProposalGateDecision {
        gate: "durable_write".to_string(),
        candidate_index: None,
        status: if proposal.write_performed {
            "warn".to_string()
        } else {
            "passed".to_string()
        },
        reason: format!("write_performed={}", proposal.write_performed),
    });
    gates.push(MemoryProposalGateDecision {
        gate: "candidate_count".to_string(),
        candidate_index: None,
        status: if proposal.candidates.is_empty() {
            "missing".to_string()
        } else {
            "passed".to_string()
        },
        reason: format!("candidates={}", proposal.candidates.len()),
    });
    let scope_findings = proposal_scope_identity_findings(proposal);
    let ambiguous_count = scope_findings
        .iter()
        .filter(|finding| finding.status == "review_required")
        .count();
    let invalid_count = scope_findings
        .iter()
        .filter(|finding| finding.status == "missing")
        .count();
    gates.push(MemoryProposalGateDecision {
        gate: "scope_identity".to_string(),
        candidate_index: None,
        status: if invalid_count > 0 {
            "missing".to_string()
        } else if ambiguous_count > 0 {
            "review_required".to_string()
        } else {
            "passed".to_string()
        },
        reason: if scope_findings.is_empty() {
            "candidate_scopes=0".to_string()
        } else {
            scope_findings
                .iter()
                .map(|finding| {
                    format!(
                        "{}:{}:{}",
                        finding.scope, finding.status, finding.identity_label
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        },
    });
    for (idx, finding) in scope_findings.iter().enumerate() {
        gates.push(MemoryProposalGateDecision {
            gate: "scope_identity".to_string(),
            candidate_index: Some(idx),
            status: finding.status.to_string(),
            reason: format!("{}:{}", finding.scope, finding.identity_label),
        });
    }
    let conflict_count = conflict_groups
        .iter()
        .filter(|group| group.group_type == "conflict")
        .count();
    let duplicate_count = conflict_groups
        .iter()
        .filter(|group| group.group_type == "duplicate")
        .count();
    gates.push(MemoryProposalGateDecision {
        gate: "duplicate_conflict".to_string(),
        candidate_index: None,
        status: if conflict_count > 0 {
            "review_required".to_string()
        } else if duplicate_count > 0 {
            "warn".to_string()
        } else {
            "passed".to_string()
        },
        reason: format!("duplicates={duplicate_count} conflicts={conflict_count}"),
    });
    gates
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProposalSensitivityFinding {
    kind: String,
    status: &'static str,
    sensitivity: &'static str,
    reason: String,
}

pub(crate) fn proposal_blocking_sensitivity_reason(proposal: &MemoryProposal) -> Option<String> {
    proposal_sensitivity_findings(proposal)
        .into_iter()
        .find(|finding| finding.status == "blocked")
        .map(|finding| format!("{}:{}", finding.sensitivity, finding.reason))
}

pub(crate) fn proposal_blocking_minimum_evidence_reason(
    proposal: &MemoryProposal,
) -> Option<String> {
    proposal_evidence_minimum_findings(proposal)
        .into_iter()
        .find(|finding| finding.status == "missing")
        .map(|finding| format!("{}:{}", finding.kind, finding.requirement))
}

fn proposal_sensitivity_findings(proposal: &MemoryProposal) -> Vec<ProposalSensitivityFinding> {
    proposal
        .candidates
        .iter()
        .map(proposal_candidate_sensitivity)
        .collect()
}

fn proposal_candidate_sensitivity(
    candidate: &MemoryProposalCandidate,
) -> ProposalSensitivityFinding {
    let kind = normalize_proposal_kind(&candidate.kind);
    match crate::memory::scan_memory_content(&candidate.content) {
        Ok(crate::memory::SensitivityLevel::Public) => ProposalSensitivityFinding {
            kind,
            status: "passed",
            sensitivity: "public_project_fact",
            reason: "public_or_project_fact".to_string(),
        },
        Ok(crate::memory::SensitivityLevel::LocalOnly) => ProposalSensitivityFinding {
            kind,
            status: "review_required",
            sensitivity: "private_user_data",
            reason: "local_only_memory_requires_review_and_minimization".to_string(),
        },
        Ok(crate::memory::SensitivityLevel::SecretLike) => ProposalSensitivityFinding {
            kind,
            status: "blocked",
            sensitivity: "secret_or_credential",
            reason: "secret_like_content".to_string(),
        },
        Ok(crate::memory::SensitivityLevel::Unsafe) => ProposalSensitivityFinding {
            kind,
            status: "blocked",
            sensitivity: "security_sensitive_instruction",
            reason: "unsafe_content".to_string(),
        },
        Err(issue) => ProposalSensitivityFinding {
            kind,
            status: "blocked",
            sensitivity: match issue.sensitivity {
                crate::memory::SensitivityLevel::SecretLike => "secret_or_credential",
                crate::memory::SensitivityLevel::Unsafe => "security_sensitive_instruction",
                crate::memory::SensitivityLevel::LocalOnly => "private_user_data",
                crate::memory::SensitivityLevel::Public => "public_project_fact",
            },
            reason: issue.code,
        },
    }
}

pub(crate) fn memory_proposal_candidate_evidence_refs(
    proposal_id: &str,
    proposal_source: &str,
    candidate: &MemoryProposalCandidate,
) -> Vec<crate::memory::MemoryEvidenceRef> {
    let mut refs = candidate
        .evidence
        .iter()
        .enumerate()
        .map(|(idx, evidence)| {
            crate::memory::MemoryEvidenceRef::new(
                memory_proposal_evidence_kind(evidence),
                format!("memory_proposal:{proposal_id}:evidence:{idx}"),
                evidence.clone(),
                memory_proposal_evidence_confidence(evidence),
            )
        })
        .collect::<Vec<_>>();
    refs.push(crate::memory::MemoryEvidenceRef::new(
        crate::memory::MemoryEvidenceKind::RuntimeObservation,
        format!("memory_proposal:{proposal_id}"),
        format!(
            "accepted proposal source={proposal_source} kind={}",
            candidate.kind
        ),
        0.75,
    ));
    refs
}

fn memory_proposal_evidence_kind(evidence: &str) -> crate::memory::MemoryEvidenceKind {
    let lower = evidence.to_ascii_lowercase();
    if lower.contains("user:") || lower.contains("user_statement") || lower.contains("user message")
    {
        crate::memory::MemoryEvidenceKind::UserStatement
    } else if lower.contains("tool:")
        || lower.contains("tool_output")
        || lower.contains("validation:")
        || lower.contains("cargo ")
        || lower.contains("npm ")
        || lower.contains("pytest")
        || lower.contains("command:")
    {
        crate::memory::MemoryEvidenceKind::ToolOutput
    } else if lower.contains("file:") || lower.contains("changed_files") {
        crate::memory::MemoryEvidenceKind::File
    } else if lower.contains("trace:") {
        crate::memory::MemoryEvidenceKind::Trace
    } else if lower.contains("learning") || lower.contains("experience") {
        crate::memory::MemoryEvidenceKind::LearningEvent
    } else if lower.contains("background:") || lower.contains("inferred") {
        crate::memory::MemoryEvidenceKind::Inference
    } else {
        crate::memory::MemoryEvidenceKind::RuntimeObservation
    }
}

fn memory_proposal_evidence_confidence(evidence: &str) -> f32 {
    match memory_proposal_evidence_kind(evidence) {
        crate::memory::MemoryEvidenceKind::UserStatement => 0.90,
        crate::memory::MemoryEvidenceKind::ToolOutput | crate::memory::MemoryEvidenceKind::File => {
            0.85
        }
        crate::memory::MemoryEvidenceKind::Trace
        | crate::memory::MemoryEvidenceKind::RuntimeObservation => 0.75,
        crate::memory::MemoryEvidenceKind::LearningEvent => 0.70,
        crate::memory::MemoryEvidenceKind::Inference => 0.45,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProposalEvidenceMinimumFinding {
    kind: String,
    status: &'static str,
    requirement: &'static str,
}

fn proposal_evidence_minimum_findings(
    proposal: &MemoryProposal,
) -> Vec<ProposalEvidenceMinimumFinding> {
    proposal
        .candidates
        .iter()
        .map(proposal_candidate_evidence_minimum)
        .collect()
}

fn proposal_candidate_evidence_minimum(
    candidate: &MemoryProposalCandidate,
) -> ProposalEvidenceMinimumFinding {
    let kind = normalize_proposal_kind(&candidate.kind);
    if candidate.evidence.is_empty() {
        return ProposalEvidenceMinimumFinding {
            kind,
            status: "missing",
            requirement: "at_least_one_evidence_item",
        };
    }

    let evidence = candidate
        .evidence
        .iter()
        .map(|item| item.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let has_source_task = evidence_contains_any(&evidence, &["source_task:", "source task:"]);
    let has_closeout = evidence_contains_any(&evidence, &["closeout:", "execution_report:"]);
    let has_user_statement =
        evidence_contains_any(&evidence, &["user:", "user_statement", "user message"]);
    let has_tool_or_file = evidence_contains_any(
        &evidence,
        &[
            "tool:",
            "tool_output",
            "file:",
            "validation:",
            "cargo ",
            "npm ",
            "pytest",
            "command:",
        ],
    );
    let has_risk = evidence_contains_any(&evidence, &["risk:", "residual risk"]);
    let has_next_step = evidence_contains_any(&evidence, &["next_step:", "next step:"]);

    match kind.as_str() {
        "user_preference" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_user_statement {
                "passed"
            } else {
                "review_required"
            },
            requirement: "explicit_user_statement",
        },
        "project_status" | "next_step" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_source_task && has_closeout && (has_next_step || has_tool_or_file) {
                "passed"
            } else {
                "review_required"
            },
            requirement: "source_task_closeout_and_progress_evidence",
        },
        "open_risk" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_source_task && has_closeout && has_risk {
                "passed"
            } else {
                "review_required"
            },
            requirement: "source_task_closeout_and_risk_evidence",
        },
        "validation_baseline" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_source_task && has_closeout && has_tool_or_file {
                "passed"
            } else {
                "review_required"
            },
            requirement: "source_task_closeout_and_validation_evidence",
        },
        "successful_fix" | "failure_pattern" | "tool_quirk" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_tool_or_file || has_closeout {
                "passed"
            } else {
                "review_required"
            },
            requirement: "tool_file_trace_or_closeout_evidence",
        },
        "project_fact" | "workflow_convention" | "decision" => ProposalEvidenceMinimumFinding {
            kind,
            status: if has_source_task || has_tool_or_file || has_closeout || has_user_statement {
                "passed"
            } else {
                "review_required"
            },
            requirement: "source_tool_file_closeout_or_user_evidence",
        },
        _ => ProposalEvidenceMinimumFinding {
            kind,
            status: "passed",
            requirement: "at_least_one_evidence_item",
        },
    }
}

fn normalize_proposal_kind(kind: &str) -> String {
    kind.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn evidence_contains_any(evidence: &[String], needles: &[&str]) -> bool {
    evidence
        .iter()
        .any(|item| needles.iter().any(|needle| item.contains(needle)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProposalScopeIdentityFinding {
    scope: String,
    status: &'static str,
    identity_label: String,
}

fn proposal_scope_identity_findings(
    proposal: &MemoryProposal,
) -> Vec<ProposalScopeIdentityFinding> {
    proposal
        .candidates
        .iter()
        .map(proposal_candidate_scope_identity)
        .collect()
}

fn proposal_candidate_scope_identity(
    candidate: &MemoryProposalCandidate,
) -> ProposalScopeIdentityFinding {
    let scope = candidate.scope.trim().to_ascii_lowercase();
    if scope.is_empty() {
        return ProposalScopeIdentityFinding {
            scope,
            status: "missing",
            identity_label: "missing".to_string(),
        };
    }
    if let Some(topic) = scope.strip_prefix("topic:") {
        return match normalize_proposal_scope_component(topic) {
            Some(topic) => ProposalScopeIdentityFinding {
                scope,
                status: "passed",
                identity_label: format!("topic:{topic}"),
            },
            None => ProposalScopeIdentityFinding {
                scope,
                status: "missing",
                identity_label: "invalid_topic".to_string(),
            },
        };
    }
    match scope.as_str() {
        "user" | "project" | "session" | "agent" => ProposalScopeIdentityFinding {
            identity_label: scope.clone(),
            scope,
            status: "passed",
        },
        "topic" => ProposalScopeIdentityFinding {
            scope,
            status: "review_required",
            identity_label: "ambiguous_topic:missing_topic_id".to_string(),
        },
        _ => ProposalScopeIdentityFinding {
            scope,
            status: "missing",
            identity_label: "unknown_scope".to_string(),
        },
    }
}

fn normalize_proposal_scope_component(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if (ch == '-' || ch == '_' || ch == '.' || ch.is_whitespace())
            && !last_dash
            && !out.is_empty()
        {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
