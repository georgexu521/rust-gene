//! LabRun support module.
//!
//! Keeps LabRun scheduling, delegation, reporting, and certification helpers separate from normal agent turns.

use crate::lab::model::{
    GraduateResult, LabArtifactEnvelope, LabBlockerReport, LabCompressionSummary, LabCycleSummary,
    LabMeetingRequest, LabMeetingSummary, LabRevisionTask, PostdocIntegrationSummary, PostdocPlan,
    ProfessorPlan, ProfessorReview, ProfessorSteeringDecision, StageArtifact,
};

pub fn render_stage_artifact_markdown(artifact: &StageArtifact) -> String {
    match artifact {
        StageArtifact::ProfessorPlan(artifact) => render_professor_plan(artifact),
        StageArtifact::PostdocPlan(artifact) => render_postdoc_plan(artifact),
        StageArtifact::GraduateResult(artifact) => render_graduate_result(artifact),
        StageArtifact::PostdocIntegrationSummary(artifact) => {
            render_postdoc_integration_summary(artifact)
        }
        StageArtifact::ProfessorReview(artifact) => render_professor_review(artifact),
        StageArtifact::CycleSummary(artifact) => render_cycle_summary(artifact),
        StageArtifact::CompressionSummary(artifact) => render_compression_summary(artifact),
        StageArtifact::LabMeetingRequest(artifact) => render_lab_meeting_request(artifact),
        StageArtifact::LabMeetingSummary(artifact) => render_lab_meeting_summary(artifact),
        StageArtifact::LabBlockerReport(artifact) => render_lab_blocker_report(artifact),
        StageArtifact::LabRevisionTask(artifact) => render_lab_revision_task(artifact),
        StageArtifact::ProfessorSteeringDecision(artifact) => {
            render_professor_steering_decision(artifact)
        }
    }
}

fn render_header<T>(artifact: &LabArtifactEnvelope<T>) -> String {
    format!(
        "# {}\n\n- lab_run_id: `{}`\n- artifact_id: `{}`\n- artifact_type: `{:?}`\n- stage: `{}`\n- owner: `{:?}`\n- status: `{:?}`\n- validation_status: `{}`\n- created_at: `{}`\n\n",
        artifact.title,
        artifact.lab_run_id,
        artifact.artifact_id,
        artifact.artifact_type,
        artifact.stage,
        artifact.owner,
        artifact.status,
        artifact.validation_status.as_deref().unwrap_or("none"),
        artifact.created_at.to_rfc3339()
    )
}

fn render_professor_plan(artifact: &LabArtifactEnvelope<ProfessorPlan>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Problem Statement", &body.problem_statement);
    section(&mut out, "Strategic Direction", &body.strategic_direction);
    list_section(&mut out, "Success Criteria", &body.success_criteria);
    list_section(&mut out, "Constraints", &body.constraints);
    list_section(&mut out, "Risks", &body.risks);
    section(&mut out, "Handoff To Postdoc", &body.handoff_to_postdoc);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_postdoc_plan(artifact: &LabArtifactEnvelope<PostdocPlan>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(
        &mut out,
        "Implementation Summary",
        &body.implementation_summary,
    );
    list_section(&mut out, "Slices", &body.slices);
    list_section(&mut out, "Files Expected", &body.files_expected);
    list_section(&mut out, "Validation Plan", &body.validation_plan);
    section(&mut out, "Graduate Handoff", &body.graduate_handoff);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_graduate_result(artifact: &LabArtifactEnvelope<GraduateResult>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Task Summary", &body.task_summary);
    list_section(&mut out, "Changed Files", &body.changed_files);
    list_section(&mut out, "Validation Attempts", &body.validation_attempts);
    list_section(&mut out, "Blockers", &body.blockers);
    section(&mut out, "Handoff To Postdoc", &body.handoff_to_postdoc);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_postdoc_integration_summary(
    artifact: &LabArtifactEnvelope<PostdocIntegrationSummary>,
) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Integration Summary", &body.integration_summary);
    list_section(&mut out, "Accepted Results", &body.accepted_results);
    section(&mut out, "Validation Status", &body.validation_status);
    list_section(&mut out, "Remaining Risks", &body.remaining_risks);
    section(&mut out, "Handoff To Professor", &body.handoff_to_professor);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_professor_review(artifact: &LabArtifactEnvelope<ProfessorReview>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Review Summary", &body.review_summary);
    section(&mut out, "Strategic Assessment", &body.strategic_assessment);
    section(
        &mut out,
        "Accepted",
        if body.accepted { "true" } else { "false" },
    );
    list_section(&mut out, "Required Revisions", &body.required_revisions);
    section(&mut out, "User Report", &body.user_report);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_professor_steering_decision(
    artifact: &LabArtifactEnvelope<ProfessorSteeringDecision>,
) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Decision Id", &body.decision_id);
    section(&mut out, "Source Message", &body.source_message_id);
    section(&mut out, "Decision", &body.decision);
    section(&mut out, "Status", &format!("{:?}", body.status));
    section(
        &mut out,
        "Message Type",
        &format!("{:?}", body.message_type),
    );
    section(&mut out, "Urgency", &body.urgency);
    section(&mut out, "Rationale", &body.rationale);
    section(&mut out, "Next Action", &body.next_action);
    section(&mut out, "Message Summary", &body.message_summary);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_cycle_summary(artifact: &LabArtifactEnvelope<LabCycleSummary>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Cycle Id", &body.cycle_id);
    section(&mut out, "Current Stage", &body.current_stage);
    section(&mut out, "Owner", &format!("{:?}", body.owner));
    section(&mut out, "Summary", &body.summary);
    list_section(&mut out, "Completed Items", &body.completed_items);
    list_section(&mut out, "Evidence Ids", &body.evidence_ids);
    section(&mut out, "Total Tokens", &body.total_tokens.to_string());
    section(
        &mut out,
        "Cache Hit Rate Percent",
        &format!("{:.1}", body.cache_hit_rate_percent),
    );
    section(
        &mut out,
        "Estimated Cost USD",
        &format!("{:.6}", body.estimated_cost_usd),
    );
    section(&mut out, "Next Action", &body.next_action);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_compression_summary(artifact: &LabArtifactEnvelope<LabCompressionSummary>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Decision Id", &body.decision_id);
    section(&mut out, "Role", &format!("{:?}", body.role));
    section(&mut out, "Action", &format!("{:?}", body.action));
    section(&mut out, "Reason", &body.reason);
    section(&mut out, "Before Tokens", &body.before_tokens.to_string());
    section(
        &mut out,
        "Target Budget Tokens",
        &body.target_budget_tokens.to_string(),
    );
    section(
        &mut out,
        "Usage Ratio Percent",
        &format!("{:.1}", body.usage_ratio_percent),
    );
    section(
        &mut out,
        "Stable Prefix Fingerprint",
        &body.stable_prefix_fingerprint,
    );
    section(
        &mut out,
        "Dynamic Tail Fingerprint",
        &body.dynamic_tail_fingerprint,
    );
    list_section(&mut out, "Retained Layers", &body.retained_layers);
    list_section(&mut out, "Evidence Ids", &body.evidence_ids);
    section(&mut out, "Compressed Summary", &body.compressed_summary);
    section(&mut out, "Next Action", &body.next_action);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_lab_meeting_summary(artifact: &LabArtifactEnvelope<LabMeetingSummary>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Meeting Id", &body.meeting_id);
    section(&mut out, "Topic", &body.topic);
    section(&mut out, "Current Stage", &body.current_stage);
    section(&mut out, "Professor View", &body.professor_view);
    section(&mut out, "Postdoc View", &body.postdoc_view);
    section(&mut out, "Decision", &body.decision);
    list_section(&mut out, "Next Actions", &body.next_actions);
    list_section(&mut out, "Evidence Ids", &body.evidence_ids);
    section(&mut out, "Total Tokens", &body.total_tokens.to_string());
    section(
        &mut out,
        "Cache Hit Rate Percent",
        &format!("{:.1}", body.cache_hit_rate_percent),
    );
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_lab_meeting_request(artifact: &LabArtifactEnvelope<LabMeetingRequest>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Request Id", &body.request_id);
    section(&mut out, "Topic", &body.topic);
    section(&mut out, "Current Stage", &body.current_stage);
    section(&mut out, "Reason", &body.reason);
    list_section(&mut out, "Signals", &body.signals);
    section(
        &mut out,
        "Requested By",
        &format!("{:?}", body.requested_by),
    );
    section(&mut out, "Next Action", &body.next_action);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_lab_blocker_report(artifact: &LabArtifactEnvelope<LabBlockerReport>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Blocker Id", &body.blocker_id);
    section(&mut out, "Current Stage", &body.current_stage);
    section(&mut out, "Summary", &body.summary);
    list_section(&mut out, "Blocked Tasks", &body.blocked_tasks);
    list_section(&mut out, "Failed Dispatches", &body.failed_dispatches);
    section(&mut out, "Failure Count", &body.failure_count.to_string());
    section(&mut out, "Recommendation", &body.recommendation);
    section(&mut out, "Handoff To Professor", &body.handoff_to_professor);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn render_lab_revision_task(artifact: &LabArtifactEnvelope<LabRevisionTask>) -> String {
    let body = &artifact.body;
    let mut out = render_header(artifact);
    section(&mut out, "Revision Id", &body.revision_id);
    section(
        &mut out,
        "Source Review Artifact",
        &body.source_review_artifact_id,
    );
    section(
        &mut out,
        "Assigned Role",
        &format!("{:?}", body.assigned_role),
    );
    section(&mut out, "Summary", &body.summary);
    list_section(&mut out, "Required Revisions", &body.required_revisions);
    list_section(&mut out, "Evidence Ids", &body.evidence_ids);
    section(&mut out, "Next Action", &body.next_action);
    evidence_section(&mut out, &artifact.evidence_refs);
    out
}

fn section(out: &mut String, title: &str, value: &str) {
    out.push_str("## ");
    out.push_str(title);
    out.push_str("\n\n");
    if value.trim().is_empty() {
        out.push_str("_None._\n\n");
    } else {
        out.push_str(value.trim());
        out.push_str("\n\n");
    }
}

fn list_section(out: &mut String, title: &str, values: &[String]) {
    out.push_str("## ");
    out.push_str(title);
    out.push_str("\n\n");
    if values.is_empty() {
        out.push_str("- _None._\n\n");
        return;
    }
    for value in values {
        out.push_str("- ");
        out.push_str(value.trim());
        out.push('\n');
    }
    out.push('\n');
}

fn evidence_section(out: &mut String, evidence_refs: &[String]) {
    list_section(out, "Evidence Refs", evidence_refs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::model::{LabArtifactEnvelope, LabArtifactType};
    use chrono::Utc;

    #[test]
    fn renders_lab_metadata_and_professor_plan_body() {
        let artifact = StageArtifact::ProfessorPlan(LabArtifactEnvelope::new(
            "artifact_professor_plan_test".to_string(),
            "labrun_test".to_string(),
            LabArtifactType::ProfessorPlan,
            "Professor Plan".to_string(),
            Utc::now(),
            ProfessorPlan {
                problem_statement: "Build LabRun".to_string(),
                strategic_direction: "Keep runtime in charge of process.".to_string(),
                success_criteria: vec!["Stage gates are enforced.".to_string()],
                constraints: Vec::new(),
                risks: Vec::new(),
                handoff_to_postdoc: "Write implementation plan.".to_string(),
            },
        ));

        let markdown = render_stage_artifact_markdown(&artifact);

        assert!(markdown.contains("lab_run_id: `labrun_test`"));
        assert!(markdown.contains("artifact_id: `artifact_professor_plan_test`"));
        assert!(markdown.contains("Keep runtime in charge of process."));
    }
}
