//! Artifact construction helpers for LabRun orchestration.

use super::*;

pub(super) fn artifact_type_for_stage(stage: &str) -> anyhow::Result<LabArtifactType> {
    match stage {
        "professor_discussion" => Ok(LabArtifactType::ProfessorPlan),
        "postdoc_plan" => Ok(LabArtifactType::PostdocPlan),
        "graduate_work" => Ok(LabArtifactType::GraduateResult),
        "postdoc_review" => Ok(LabArtifactType::PostdocIntegrationSummary),
        "professor_review" => Ok(LabArtifactType::ProfessorReview),
        _ => Err(anyhow!("unknown LabRun artifact stage: {stage}")),
    }
}

pub(super) fn build_stage_artifact(
    run: &LabRun,
    artifact_type: LabArtifactType,
    note: &str,
) -> StageArtifact {
    let now = Utc::now();
    let note = note.trim();
    let title = if note.is_empty() {
        format!("{} for {}", artifact_type.as_str(), run.lab_run_id)
    } else {
        note.lines().next().unwrap_or(note).trim().to_string()
    };
    let artifact_id = format!(
        "artifact_{}_{}",
        artifact_type.as_str().to_ascii_lowercase(),
        Uuid::new_v4().simple()
    );
    match artifact_type {
        LabArtifactType::ProfessorPlan => StageArtifact::ProfessorPlan(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            ProfessorPlan {
                problem_statement: run.user_goal.clone(),
                strategic_direction: note_or_placeholder(note, "Initial professor direction."),
                success_criteria: vec![
                    "User-visible result is reviewed before closeout.".to_string()
                ],
                constraints: vec![
                    "Do not bypass runtime permission, checkpoint, or validation gates."
                        .to_string(),
                ],
                risks: vec![
                    "Plan content is a runtime draft until reviewed by the professor model."
                        .to_string(),
                ],
                handoff_to_postdoc:
                    "Create an implementation plan with slices, expected files, and validation."
                        .to_string(),
            },
        )),
        LabArtifactType::PostdocPlan => StageArtifact::PostdocPlan(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            PostdocPlan {
                implementation_summary: note_or_placeholder(
                    note,
                    "Postdoc implementation plan draft.",
                ),
                slices: vec!["Implement the smallest verifiable next slice.".to_string()],
                files_expected: Vec::new(),
                validation_plan: vec!["Run the narrowest relevant validation gate.".to_string()],
                graduate_handoff:
                    "Execute the current slice and report changed files, proof, and blockers."
                        .to_string(),
            },
        )),
        LabArtifactType::GraduateResult => StageArtifact::GraduateResult(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            GraduateResult {
                task_summary: note_or_placeholder(note, "Graduate task result draft."),
                changed_files: Vec::new(),
                validation_attempts: Vec::new(),
                blockers: Vec::new(),
                handoff_to_postdoc: "Review implementation quality and integration readiness."
                    .to_string(),
                provenance: LabEvidenceProvenance {
                    lab_run_id: Some(run.lab_run_id.clone()),
                    cycle_id: Some(run.cycle_count.to_string()),
                    ..LabEvidenceProvenance::default()
                },
            },
        )),
        LabArtifactType::PostdocIntegrationSummary => {
            StageArtifact::PostdocIntegrationSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                PostdocIntegrationSummary {
                    integration_summary: note_or_placeholder(
                        note,
                        "Postdoc integration summary draft.",
                    ),
                    accepted_results: Vec::new(),
                    validation_status: "not_verified".to_string(),
                    remaining_risks: Vec::new(),
                    handoff_to_professor:
                        "Review strategic fit, completeness, and user-facing closeout.".to_string(),
                },
            ))
        }
        LabArtifactType::ProfessorReview => {
            StageArtifact::ProfessorReview(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                ProfessorReview {
                    review_summary: note_or_placeholder(note, "Professor review draft."),
                    strategic_assessment: "Strategic assessment requires professor model review."
                        .to_string(),
                    accepted: false,
                    required_revisions: Vec::new(),
                    user_report: "Prepare a concise user-facing report before closeout."
                        .to_string(),
                },
            ))
        }
        LabArtifactType::CycleSummary => StageArtifact::CycleSummary(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            LabCycleSummary {
                cycle_id: run.cycle_count.to_string(),
                current_stage: run.current_stage.clone(),
                owner: run.internal_owner,
                summary: note_or_placeholder(note, "Cycle summary draft."),
                completed_items: Vec::new(),
                evidence_ids: Vec::new(),
                total_tokens: 0,
                cache_hit_rate_percent: 0.0,
                estimated_cost_usd: 0.0,
                next_action: "Continue LabRun orchestration from the current stage.".to_string(),
            },
        )),
        LabArtifactType::CompressionSummary => {
            StageArtifact::CompressionSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                LabCompressionSummary {
                    decision_id: String::new(),
                    role: run.internal_owner,
                    action: LabCompressionAction::None,
                    reason: "Compression summary placeholder.".to_string(),
                    before_tokens: 0,
                    target_budget_tokens: 0,
                    usage_ratio_percent: 0.0,
                    stable_prefix_fingerprint: String::new(),
                    dynamic_tail_fingerprint: String::new(),
                    retained_layers: Vec::new(),
                    evidence_ids: Vec::new(),
                    compressed_summary: note_or_placeholder(note, "Compression summary draft."),
                    next_action: "Continue LabRun orchestration from the current stage."
                        .to_string(),
                },
            ))
        }
        LabArtifactType::LabMeetingRequest => {
            StageArtifact::LabMeetingRequest(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!(
                    "Runtime escalation signal meeting request for {}",
                    run.current_stage
                ),
                now,
                LabMeetingRequest {
                    request_id: "meeting_request_placeholder".to_string(),
                    topic: note_or_placeholder(note, "General LabRun meeting request."),
                    current_stage: run.current_stage.clone(),
                    reason: "runtime_placeholder".to_string(),
                    signals: Vec::new(),
                    requested_by: LabRole::Runtime,
                    next_action: "open_read_only_lab_meeting".to_string(),
                },
            ))
        }
        LabArtifactType::LabMeetingSummary => {
            StageArtifact::LabMeetingSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Lab meeting summary for {}", run.lab_run_id),
                now,
                LabMeetingSummary {
                    meeting_id: "meeting_placeholder".to_string(),
                    topic: note_or_placeholder(note, "General LabRun meeting."),
                    current_stage: run.current_stage.clone(),
                    professor_view: "Runtime placeholder professor view.".to_string(),
                    postdoc_view: "Runtime placeholder postdoc view.".to_string(),
                    decision: "continue_current_plan".to_string(),
                    next_actions: vec!["continue_labrun".to_string()],
                    evidence_ids: Vec::new(),
                    total_tokens: 0,
                    cache_hit_rate_percent: 0.0,
                },
            ))
        }
        LabArtifactType::LabBlockerReport => {
            StageArtifact::LabBlockerReport(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Postdoc blocker report for {}", run.current_stage),
                now,
                LabBlockerReport {
                    blocker_id: "blocker_placeholder".to_string(),
                    current_stage: run.current_stage.clone(),
                    summary: note_or_placeholder(note, "No blocker summary provided."),
                    blocked_tasks: Vec::new(),
                    failed_dispatches: Vec::new(),
                    failure_count: run.failure_count,
                    recommendation: "continue_current_plan".to_string(),
                    handoff_to_professor: "Review blocker state.".to_string(),
                },
            ))
        }
        LabArtifactType::LabRevisionTask => {
            StageArtifact::LabRevisionTask(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Postdoc revision task for {}", run.lab_run_id),
                now,
                LabRevisionTask {
                    revision_id: "revision_placeholder".to_string(),
                    source_review_artifact_id: String::new(),
                    assigned_role: LabRole::Postdoc,
                    summary: note_or_placeholder(note, "Professor requested postdoc revision."),
                    required_revisions: Vec::new(),
                    evidence_ids: Vec::new(),
                    next_action:
                        "Revise postdoc integration before professor review can close out."
                            .to_string(),
                },
            ))
        }
        LabArtifactType::ProfessorSteeringDecision => {
            StageArtifact::ProfessorSteeringDecision(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                format!("Professor steering decision for {}", run.lab_run_id),
                now,
                ProfessorSteeringDecision {
                    decision_id: "professor_steering_placeholder".to_string(),
                    source_message_id: String::new(),
                    decision: "pending_professor_review".to_string(),
                    status: SponsorMessageStatus::Queued,
                    message_type: SponsorMessageType::Concern,
                    urgency: "normal".to_string(),
                    rationale: note_or_placeholder(note, "No steering rationale provided."),
                    next_action: "Review sponsor message before applying any LabRun change."
                        .to_string(),
                    message_summary: String::new(),
                },
            ))
        }
    }
}

pub(super) fn note_or_placeholder(note: &str, placeholder: &str) -> String {
    if note.trim().is_empty() {
        placeholder.to_string()
    } else {
        note.trim().to_string()
    }
}

pub(super) fn postdoc_plan_task_marker(artifact_id: &str) -> String {
    format!("postdoc_plan_artifact_id={}", artifact_id.trim())
}

pub(super) fn compact_task_title(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 72 {
        return compact;
    }
    let mut out = compact.chars().take(69).collect::<String>();
    out.push_str("...");
    out
}

pub(super) fn compact_result_preview(value: &str, limit: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= limit {
        return compact;
    }
    let keep = limit.saturating_sub(3);
    let mut out = compact.chars().take(keep).collect::<String>();
    out.push_str("...");
    out
}

pub(super) fn clean_string_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}
