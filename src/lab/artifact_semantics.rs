//! Semantic eligibility checks for LabRun stage artifacts.
//!
//! `ArtifactGate` is the structural gate. This module adds artifact-type-specific
//! checks so placeholder-shaped artifacts do not advance the role workflow.

use crate::lab::model::{
    LabArtifactEnvelope, PostdocIntegrationSummary, ProfessorReview, StageArtifact,
};
use crate::lab::validation::{classify_lab_validation_command, LabValidationPolicyDecision};

pub(crate) fn stage_artifact_semantic_blockers(artifact: &StageArtifact) -> Vec<String> {
    let mut blockers = match artifact {
        StageArtifact::ProfessorPlan(artifact) => professor_plan_blockers(artifact),
        StageArtifact::PostdocPlan(artifact) => postdoc_plan_blockers(artifact),
        StageArtifact::GraduateResult(artifact) => graduate_result_blockers(artifact),
        StageArtifact::PostdocIntegrationSummary(artifact) => {
            postdoc_integration_blockers(artifact)
        }
        StageArtifact::ProfessorReview(artifact) => professor_review_blockers(artifact),
        _ => Vec::new(),
    };
    blockers.sort();
    blockers.dedup();
    blockers
}

fn professor_plan_blockers(
    artifact: &LabArtifactEnvelope<crate::lab::model::ProfessorPlan>,
) -> Vec<String> {
    let body = &artifact.body;
    let mut blockers = Vec::new();
    require_text(
        &mut blockers,
        "ProfessorPlan.problem_statement",
        &body.problem_statement,
    );
    require_text(
        &mut blockers,
        "ProfessorPlan.strategic_direction",
        &body.strategic_direction,
    );
    require_non_empty_list(
        &mut blockers,
        "ProfessorPlan.success_criteria",
        &body.success_criteria,
    );
    require_non_empty_list(&mut blockers, "ProfessorPlan.risks", &body.risks);
    require_text(
        &mut blockers,
        "ProfessorPlan.handoff_to_postdoc",
        &body.handoff_to_postdoc,
    );
    blockers
}

fn postdoc_plan_blockers(
    artifact: &LabArtifactEnvelope<crate::lab::model::PostdocPlan>,
) -> Vec<String> {
    let body = &artifact.body;
    let mut blockers = Vec::new();
    require_text(
        &mut blockers,
        "PostdocPlan.implementation_summary",
        &body.implementation_summary,
    );
    require_non_empty_list(&mut blockers, "PostdocPlan.slices", &body.slices);
    require_non_empty_list(
        &mut blockers,
        "PostdocPlan.files_expected",
        &body.files_expected,
    );
    require_non_empty_list(
        &mut blockers,
        "PostdocPlan.validation_plan",
        &body.validation_plan,
    );
    require_text(
        &mut blockers,
        "PostdocPlan.graduate_handoff",
        &body.graduate_handoff,
    );
    for path in &body.files_expected {
        if let Err(err) = crate::lab::path_scope::normalize_lab_relative_path(path) {
            blockers.push(format!("PostdocPlan.files_expected invalid path: {err}"));
        }
    }
    for command in &body.validation_plan {
        if let LabValidationPolicyDecision::Block { reason, .. } =
            classify_lab_validation_command(command)
        {
            blockers.push(format!(
                "PostdocPlan.validation_plan command `{}` is not executable by Lab validation policy: {}",
                command.trim(),
                reason
            ));
        }
    }
    blockers
}

fn graduate_result_blockers(
    artifact: &LabArtifactEnvelope<crate::lab::model::GraduateResult>,
) -> Vec<String> {
    let body = &artifact.body;
    let mut blockers = Vec::new();
    require_text(
        &mut blockers,
        "GraduateResult.task_summary",
        &body.task_summary,
    );
    require_text(
        &mut blockers,
        "GraduateResult.handoff_to_postdoc",
        &body.handoff_to_postdoc,
    );
    if body.blockers.is_empty() {
        require_non_empty_list(
            &mut blockers,
            "GraduateResult.changed_files",
            &body.changed_files,
        );
        require_non_empty_list(
            &mut blockers,
            "GraduateResult.validation_attempts",
            &body.validation_attempts,
        );
    }
    for path in &body.changed_files {
        if let Err(err) = crate::lab::path_scope::normalize_lab_relative_path(path) {
            blockers.push(format!("GraduateResult.changed_files invalid path: {err}"));
        }
    }
    blockers
}

fn postdoc_integration_blockers(
    artifact: &LabArtifactEnvelope<PostdocIntegrationSummary>,
) -> Vec<String> {
    let body = &artifact.body;
    let mut blockers = Vec::new();
    require_text(
        &mut blockers,
        "PostdocIntegrationSummary.integration_summary",
        &body.integration_summary,
    );
    require_text(
        &mut blockers,
        "PostdocIntegrationSummary.handoff_to_professor",
        &body.handoff_to_professor,
    );
    if body.validation_status != "needs_revision" {
        require_non_empty_list(
            &mut blockers,
            "PostdocIntegrationSummary.accepted_results",
            &body.accepted_results,
        );
        require_non_empty_list(
            &mut blockers,
            "PostdocIntegrationSummary.evidence_refs",
            &artifact.evidence_refs,
        );
        if body
            .accepted_results
            .iter()
            .any(|result| result.contains("pending parent verification"))
        {
            blockers.push(
                "PostdocIntegrationSummary accepted results include pending parent verification"
                    .to_string(),
            );
        }
    }
    blockers
}

fn professor_review_blockers(artifact: &LabArtifactEnvelope<ProfessorReview>) -> Vec<String> {
    let body = &artifact.body;
    let mut blockers = Vec::new();
    require_text(
        &mut blockers,
        "ProfessorReview.review_summary",
        &body.review_summary,
    );
    require_text(
        &mut blockers,
        "ProfessorReview.strategic_assessment",
        &body.strategic_assessment,
    );
    require_text(
        &mut blockers,
        "ProfessorReview.user_report",
        &body.user_report,
    );
    if !body.accepted {
        blockers.push("ProfessorReview.accepted must be true for user_report".to_string());
    }
    if body.accepted {
        require_non_empty_list(
            &mut blockers,
            "ProfessorReview.evidence_refs",
            &artifact.evidence_refs,
        );
        if !body.required_revisions.is_empty() {
            blockers.push("ProfessorReview.accepted cannot include required_revisions".to_string());
        }
    }
    blockers
}

fn require_text(blockers: &mut Vec<String>, field: &str, value: &str) {
    if value.trim().is_empty() {
        blockers.push(format!("{field} must be non-empty"));
    }
}

fn require_non_empty_list(blockers: &mut Vec<String>, field: &str, values: &[String]) {
    if values.iter().all(|value| value.trim().is_empty()) {
        blockers.push(format!("{field} must contain at least one item"));
    }
}
