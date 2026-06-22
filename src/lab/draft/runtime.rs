use super::*;

pub(super) fn lab_cycle_total_tokens(
    store: &LabStore,
    lab_run_id: &str,
    cycle_count: u64,
) -> anyhow::Result<u64> {
    let cycle_id = cycle_count.to_string();
    Ok(store
        .list_cost_usage(lab_run_id)?
        .into_iter()
        .filter(|usage| usage.cycle_id.as_deref() == Some(cycle_id.as_str()))
        .map(|usage| usage.total_tokens)
        .sum())
}

pub(super) fn auto_compress_completed_cycle(
    orchestrator: &LabOrchestrator,
    store: &LabStore,
) -> anyhow::Result<Vec<String>> {
    let run = store
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for completed-cycle compression"))?;
    if !run.cost_policy.auto_compress_after_cycle {
        return Ok(Vec::new());
    }

    let mut artifact_ids = Vec::new();
    for role in [LabRole::Professor, LabRole::Postdoc, LabRole::Runtime] {
        if let Some(created) = orchestrator.create_compression_summary_for_latest(role)? {
            artifact_ids.push(created.artifact.artifact_id().to_string());
        }
    }
    Ok(artifact_ids)
}

pub(super) fn run_deterministic_review_stage(
    orchestrator: &LabOrchestrator,
    stage: &str,
) -> anyhow::Result<LabDeterministicStageStepOutcome> {
    let created = match stage {
        "postdoc_review" => orchestrator.create_postdoc_integration_summary_for_latest(None)?,
        "professor_review" => orchestrator.create_professor_review_for_latest(None)?,
        other => return Err(anyhow!("unsupported deterministic review stage: {other}")),
    };
    let gate_satisfied = created.gate.is_satisfied();
    let mut to_stage = stage.to_string();
    let mut message = if gate_satisfied {
        format!(
            "Created {} artifact and satisfied {} gate.",
            created.artifact.artifact_type().as_str(),
            created.gate.stage
        )
    } else {
        format!(
            "Created {} artifact but {} gate is blocked.",
            created.artifact.artifact_type().as_str(),
            created.gate.stage
        )
    };
    if gate_satisfied {
        let advanced = orchestrator.advance_latest()?;
        to_stage = advanced.current_stage;
        message.push_str(&format!(" Advanced to {}.", to_stage));
    }
    Ok(LabDeterministicStageStepOutcome {
        lab_run_id: created.artifact.lab_run_id().to_string(),
        from_stage: stage.to_string(),
        to_stage,
        artifact_id: created.artifact.artifact_id().to_string(),
        gate_satisfied,
        message,
    })
}

pub(super) fn build_lab_artifact_draft_prompt(
    current_stage: &str,
    required_artifact_type: &str,
    context_layers: &str,
    instructions: &str,
) -> String {
    let instructions = instructions.trim();
    let instructions = if instructions.is_empty() {
        "Draft the current required artifact from the supplied LabRun context."
    } else {
        instructions
    };
    format!(
        "current_stage: {current_stage}\nrequired_artifact_type: {required_artifact_type}\n\nReturn JSON when possible.\nFor ProfessorPlan, use fields: problem_statement, strategic_direction, success_criteria, constraints, risks, handoff_to_postdoc.\nFor PostdocPlan, use fields: implementation_summary, slices, files_expected, validation_plan, graduate_handoff.\nFor GraduateResult, use fields: task_summary, changed_files, validation_attempts, blockers, handoff_to_postdoc.\nFor PostdocIntegrationSummary, use fields: integration_summary, accepted_results, validation_status, remaining_risks, handoff_to_professor.\nFor ProfessorReview, use fields: review_summary, strategic_assessment, accepted, required_revisions, user_report.\n\nUser instructions:\n{instructions}\n\nLabRun context layers:\n{context_layers}\n\nDraft the artifact body now."
    )
}

pub(super) fn build_lab_artifact_review_prompt(
    run: &LabRun,
    artifact: &StageArtifact,
    instructions: &str,
) -> anyhow::Result<String> {
    let instructions = instructions.trim();
    let instructions = if instructions.is_empty() {
        "Review this artifact against the LabRun stage contract."
    } else {
        instructions
    };
    Ok(format!(
        "lab_run_id: {}\ncurrent_stage: {}\nartifact_stage: {}\nartifact_type: {}\nreviewer_role: {:?}\n\nInstructions:\n{}\n\nArtifact JSON:\n{}",
        run.lab_run_id,
        run.current_stage,
        artifact.stage(),
        artifact.artifact_type().as_str(),
        reviewer_role_for_artifact(artifact),
        instructions,
        serde_json::to_string_pretty(artifact)?
    ))
}

pub(super) fn build_provider_professor_review_prompt(
    run: &LabRun,
    integration: &LabArtifactEnvelope<PostdocIntegrationSummary>,
    context_layers: &str,
    instructions: &str,
) -> anyhow::Result<String> {
    let instructions = instructions.trim();
    let instructions = if instructions.is_empty() {
        "Review whether the postdoc evidence is ready for user-facing closeout."
    } else {
        instructions
    };
    Ok(format!(
        "lab_run_id: {}\ncurrent_stage: {}\ncycle: {}\nrequired_artifact_type: ProfessorReview\n\nUser/professor instructions:\n{}\n\nPostdocIntegrationSummary JSON:\n{}\n\nLabRun context layers:\n{}\n\nReturn only the ProfessorReview JSON body.",
        run.lab_run_id,
        run.current_stage,
        run.cycle_count,
        instructions,
        serde_json::to_string_pretty(integration)?,
        context_layers
    ))
}

pub(super) fn build_sponsor_message_classification_prompt(
    run: &LabRun,
    message: &SponsorMessage,
    context_layers: &str,
    instructions: &str,
) -> String {
    let instructions = instructions.trim();
    let instructions = if instructions.is_empty() {
        "Classify the sponsor message into the safest next workflow state."
    } else {
        instructions
    };
    format!(
        "lab_run_id: {}\ncurrent_stage: {}\ncycle: {}\nmessage_id: {}\nmessage_type: {:?}\nurgency: {}\ncurrent_status: {:?}\n\nProfessor instructions:\n{}\n\nSponsor message:\n{}\n\nLabRun context layers:\n{}\n\nReturn only the classification JSON.",
        run.lab_run_id,
        run.current_stage,
        run.cycle_count,
        message.message_id,
        message.message_type,
        message.urgency,
        message.status,
        instructions,
        message.body,
        context_layers
    )
}

pub(super) fn build_lab_meeting_prompt(run: &LabRun, topic: &str, context_layers: &str) -> String {
    format!(
        "lab_run_id: {}\ncurrent_stage: {}\ncycle: {}\nmeeting_topic: {}\n\nLabRun context layers:\n{}\n\nDraft the read-only Lab meeting summary now.",
        run.lab_run_id,
        run.current_stage,
        run.cycle_count,
        topic,
        context_layers
    )
}

pub(super) fn enforce_professor_review_evidence_boundary(
    integration: &LabArtifactEnvelope<PostdocIntegrationSummary>,
    review: &mut ProfessorReview,
) {
    let mut required_revisions = review.required_revisions.clone();
    if integration.body.accepted_results.is_empty() {
        required_revisions
            .push("Postdoc integration has no accepted graduate results.".to_string());
    }
    if integration.body.validation_status == "needs_revision" {
        required_revisions.push("Postdoc integration is marked needs_revision.".to_string());
    }
    if !review.accepted && required_revisions.is_empty() {
        required_revisions.push(
            "Professor review rejected closeout but did not specify required revisions."
                .to_string(),
        );
    }
    required_revisions.sort();
    required_revisions.dedup();
    if !required_revisions.is_empty() {
        review.accepted = false;
        review.required_revisions = required_revisions;
        if review.user_report.trim().is_empty() {
            review.user_report =
                "LabRun is not ready for closeout; postdoc revision is required.".to_string();
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RawArtifactReviewDecision {
    decision: String,
    note: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawSponsorMessageClassification {
    decision: String,
    note: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawLabProposalIntakeDraft {
    problem_statement: String,
    desired_outcome: String,
    #[serde(default)]
    scope: Vec<String>,
    #[serde(default)]
    non_goals: Vec<String>,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    risks: Vec<String>,
    #[serde(default)]
    success_criteria: Vec<String>,
    recommended_mode: String,
    professor_rationale: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawLabMeetingSummaryDraft {
    pub(super) professor_view: String,
    pub(super) postdoc_view: String,
    pub(super) decision: String,
    #[serde(default)]
    pub(super) next_actions: Vec<String>,
    #[serde(default)]
    pub(super) evidence_ids: Vec<String>,
}

#[derive(Debug)]
pub(super) struct ParsedArtifactReviewDecision {
    pub(super) decision: LabArtifactReviewDecision,
    pub(super) note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SponsorMessageClassificationDecision {
    Review,
    Meeting,
    Task,
    Reject,
}

impl SponsorMessageClassificationDecision {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Meeting => "meeting",
            Self::Task => "task",
            Self::Reject => "reject",
        }
    }

    pub(super) fn status(self) -> SponsorMessageStatus {
        match self {
            Self::Review => SponsorMessageStatus::Reviewed,
            Self::Meeting => SponsorMessageStatus::ConvertedToMeeting,
            Self::Task => SponsorMessageStatus::ConvertedToTask,
            Self::Reject => SponsorMessageStatus::Rejected,
        }
    }
}

#[derive(Debug)]
pub(super) struct ParsedSponsorMessageClassification {
    pub(super) decision: SponsorMessageClassificationDecision,
    pub(super) note: String,
}

pub(super) fn select_sponsor_message_for_classification<'a>(
    messages: &'a [SponsorMessage],
    message_id: &str,
) -> anyhow::Result<&'a SponsorMessage> {
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return Err(anyhow!(
            "sponsor message id cannot be empty; use latest or a message id"
        ));
    }
    if message_id == "latest" {
        return messages
            .iter()
            .rev()
            .find(|message| matches!(message.status, SponsorMessageStatus::Queued))
            .or_else(|| messages.iter().next_back())
            .ok_or_else(|| anyhow!("no sponsor messages found"));
    }
    messages
        .iter()
        .find(|message| message.message_id == message_id)
        .ok_or_else(|| anyhow!("sponsor message not found: {message_id}"))
}

pub(super) fn parse_lab_artifact_review_decision(
    content: &str,
) -> anyhow::Result<ParsedArtifactReviewDecision> {
    let draft = sanitize_lab_artifact_draft(content)?;
    let raw: RawArtifactReviewDecision = serde_json::from_str(&draft)
        .map_err(|err| anyhow!("invalid Lab artifact review JSON: {err}"))?;
    let note = raw.note.trim();
    if note.is_empty() {
        return Err(anyhow!("Lab artifact review note cannot be empty"));
    }
    let decision = match raw.decision.trim().to_ascii_lowercase().as_str() {
        "accept" | "accepted" => LabArtifactReviewDecision::Accept,
        "revise" | "needs_revision" | "revision" => LabArtifactReviewDecision::Revise,
        other => return Err(anyhow!("invalid Lab artifact review decision: {other}")),
    };
    Ok(ParsedArtifactReviewDecision {
        decision,
        note: note.to_string(),
    })
}

pub(super) fn parse_sponsor_message_classification(
    content: &str,
) -> anyhow::Result<ParsedSponsorMessageClassification> {
    let draft = sanitize_lab_artifact_draft(content)?;
    let raw: RawSponsorMessageClassification = serde_json::from_str(&draft)
        .map_err(|err| anyhow!("invalid sponsor message classification JSON: {err}"))?;
    let note = raw.note.trim();
    if note.is_empty() {
        return Err(anyhow!(
            "sponsor message classification note cannot be empty"
        ));
    }
    let decision = match raw.decision.trim().to_ascii_lowercase().as_str() {
        "review" | "reviewed" | "acknowledge" | "acknowledged" => {
            SponsorMessageClassificationDecision::Review
        }
        "meeting" | "lab_meeting" | "convert_to_meeting" => {
            SponsorMessageClassificationDecision::Meeting
        }
        "task" | "graduate_task" | "postdoc_task" | "convert_to_task" => {
            SponsorMessageClassificationDecision::Task
        }
        "reject" | "rejected" | "out_of_scope" => SponsorMessageClassificationDecision::Reject,
        other => {
            return Err(anyhow!(
                "invalid sponsor message classification decision: {other}"
            ))
        }
    };
    Ok(ParsedSponsorMessageClassification {
        decision,
        note: note.to_string(),
    })
}

pub(super) fn parse_lab_proposal_intake_draft(
    content: &str,
) -> anyhow::Result<LabProposalIntakeDraft> {
    let draft = sanitize_lab_artifact_draft(content)?;
    let raw: RawLabProposalIntakeDraft = serde_json::from_str(&draft)
        .map_err(|err| anyhow!("invalid Lab proposal intake JSON: {err}"))?;
    let problem_statement = raw.problem_statement.trim().to_string();
    let desired_outcome = raw.desired_outcome.trim().to_string();
    let professor_rationale = raw.professor_rationale.trim().to_string();
    if problem_statement.is_empty() {
        return Err(anyhow!("Lab proposal problem_statement cannot be empty"));
    }
    if desired_outcome.is_empty() {
        return Err(anyhow!("Lab proposal desired_outcome cannot be empty"));
    }
    if professor_rationale.is_empty() {
        return Err(anyhow!("Lab proposal professor_rationale cannot be empty"));
    }
    let recommended_mode = match raw.recommended_mode.trim().to_ascii_lowercase().as_str() {
        "direct" => RecommendedMode::Direct,
        "goal" => RecommendedMode::Goal,
        "labrun" | "lab" | "lab_run" | "lab-mode" => RecommendedMode::Labrun,
        other => return Err(anyhow!("invalid Lab proposal recommended_mode: {other}")),
    };
    Ok(LabProposalIntakeDraft {
        problem_statement,
        desired_outcome,
        scope: clean_draft_vec(raw.scope),
        non_goals: clean_draft_vec(raw.non_goals),
        constraints: clean_draft_vec(raw.constraints),
        risks: clean_draft_vec(raw.risks),
        success_criteria: clean_draft_vec(raw.success_criteria),
        recommended_mode,
        professor_rationale,
    })
}

pub(super) fn parse_lab_meeting_summary_draft(
    content: &str,
) -> anyhow::Result<RawLabMeetingSummaryDraft> {
    let draft = sanitize_lab_artifact_draft(content)?;
    let raw: RawLabMeetingSummaryDraft =
        serde_json::from_str(&draft).map_err(|err| anyhow!("invalid Lab meeting JSON: {err}"))?;
    let professor_view = raw.professor_view.trim().to_string();
    let postdoc_view = raw.postdoc_view.trim().to_string();
    let decision = raw.decision.trim().to_string();
    if professor_view.is_empty() {
        return Err(anyhow!("Lab meeting professor_view cannot be empty"));
    }
    if postdoc_view.is_empty() {
        return Err(anyhow!("Lab meeting postdoc_view cannot be empty"));
    }
    if decision.is_empty() {
        return Err(anyhow!("Lab meeting decision cannot be empty"));
    }
    let next_actions = clean_draft_vec(raw.next_actions);
    if next_actions.is_empty() {
        return Err(anyhow!("Lab meeting next_actions cannot be empty"));
    }
    Ok(RawLabMeetingSummaryDraft {
        professor_view,
        postdoc_view,
        decision,
        next_actions,
        evidence_ids: clean_draft_vec(raw.evidence_ids),
    })
}

pub(super) fn clean_draft_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

pub(super) fn reviewer_role_for_artifact(artifact: &StageArtifact) -> LabRole {
    match artifact.artifact_type() {
        LabArtifactType::ProfessorPlan => LabRole::Professor,
        LabArtifactType::PostdocPlan => LabRole::Postdoc,
        LabArtifactType::GraduateResult => LabRole::Postdoc,
        LabArtifactType::PostdocIntegrationSummary => LabRole::Professor,
        LabArtifactType::ProfessorReview => LabRole::Professor,
        _ => LabRole::Runtime,
    }
}

pub(super) fn sanitize_lab_artifact_draft(content: &str) -> anyhow::Result<String> {
    let mut draft = sanitize_assistant_content(content).trim().to_string();
    if draft.starts_with("```") && draft.ends_with("```") {
        let without_opening = draft.lines().skip(1).collect::<Vec<_>>().join("\n");
        draft = without_opening.trim_end_matches("```").trim().to_string();
    }
    if draft.trim().is_empty() {
        return Err(anyhow!("provider returned an empty Lab artifact draft"));
    }
    Ok(draft)
}

pub(super) fn create_artifact_from_draft(
    orchestrator: &crate::lab::orchestrator::LabOrchestrator,
    store: &LabStore,
    run: &LabRun,
    draft: &str,
) -> anyhow::Result<CreatedStageArtifact> {
    if let Some(artifact) = parse_structured_stage_artifact(run, draft)? {
        let mut artifact = artifact;
        let consumed_revision_artifact_id =
            orchestrator.apply_pending_revision_task_to_postdoc_plan(run, &mut artifact)?;
        let path = store.write_stage_artifact(&artifact)?;
        let report_path = store.write_stage_artifact_report(&artifact)?;
        if let Some(revision_artifact_id) = consumed_revision_artifact_id.as_deref() {
            orchestrator.mark_revision_task_consumed_by_postdoc_plan(
                run,
                revision_artifact_id,
                artifact.artifact_id(),
            )?;
        }
        let path_ref = path.display().to_string();
        let mut evidence_refs = vec![path_ref.as_str()];
        evidence_refs.extend(artifact.evidence_refs().iter().map(String::as_str));
        let gate = orchestrator.write_satisfied_gate_for_latest_with_evidence_refs(
            artifact.artifact_id(),
            artifact.validation_status(),
            &evidence_refs,
        )?;
        return Ok(CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        });
    }
    orchestrator.create_current_stage_artifact_for_latest(draft)
}

pub(super) fn parse_structured_stage_artifact(
    run: &LabRun,
    draft: &str,
) -> anyhow::Result<Option<StageArtifact>> {
    let trimmed = draft.trim();
    if !trimmed.starts_with('{') {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|err| anyhow!("invalid Lab artifact JSON draft: {err}"))?;
    let artifact_type = artifact_type_for_stage(&run.current_stage)?;
    let body = body_value_for_artifact_type(value, artifact_type)?;
    Ok(Some(build_structured_stage_artifact(
        run,
        artifact_type,
        body,
    )?))
}

pub(super) fn body_value_for_artifact_type(
    value: serde_json::Value,
    artifact_type: LabArtifactType,
) -> anyhow::Result<serde_json::Value> {
    if let Some(value) = value.get("artifact").cloned() {
        return Ok(value);
    }
    let wrapper_key = match artifact_type {
        LabArtifactType::ProfessorPlan => "professor_plan",
        LabArtifactType::PostdocPlan => "postdoc_plan",
        LabArtifactType::GraduateResult => "graduate_result",
        LabArtifactType::PostdocIntegrationSummary => "postdoc_integration_summary",
        LabArtifactType::ProfessorReview => "professor_review",
        _ => return Ok(value),
    };
    Ok(value.get(wrapper_key).cloned().unwrap_or(value))
}

pub(super) fn build_structured_stage_artifact(
    run: &LabRun,
    artifact_type: LabArtifactType,
    body: serde_json::Value,
) -> anyhow::Result<StageArtifact> {
    let now = Utc::now();
    let artifact_id = format!(
        "artifact_{}_{}",
        artifact_type.as_str().to_ascii_lowercase(),
        Uuid::new_v4().simple()
    );
    let title = format!("{} for {}", artifact_type.as_str(), run.lab_run_id);
    let artifact = match artifact_type {
        LabArtifactType::ProfessorPlan => StageArtifact::ProfessorPlan(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            serde_json::from_value::<ProfessorPlan>(normalize_structured_stage_body(
                artifact_type,
                body,
            ))?,
        )),
        LabArtifactType::PostdocPlan => StageArtifact::PostdocPlan(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            serde_json::from_value::<PostdocPlan>(normalize_structured_stage_body(
                artifact_type,
                body,
            ))?,
        )),
        LabArtifactType::GraduateResult => StageArtifact::GraduateResult(LabArtifactEnvelope::new(
            artifact_id,
            run.lab_run_id.clone(),
            artifact_type,
            title,
            now,
            serde_json::from_value::<GraduateResult>(normalize_structured_stage_body(
                artifact_type,
                body,
            ))?,
        )),
        LabArtifactType::PostdocIntegrationSummary => {
            StageArtifact::PostdocIntegrationSummary(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                serde_json::from_value::<PostdocIntegrationSummary>(
                    normalize_structured_stage_body(artifact_type, body),
                )?,
            ))
        }
        LabArtifactType::ProfessorReview => {
            StageArtifact::ProfessorReview(LabArtifactEnvelope::new(
                artifact_id,
                run.lab_run_id.clone(),
                artifact_type,
                title,
                now,
                serde_json::from_value::<ProfessorReview>(normalize_structured_stage_body(
                    artifact_type,
                    body,
                ))?,
            ))
        }
        _ => {
            return Err(anyhow!(
                "unsupported structured Lab artifact type: {artifact_type:?}"
            ))
        }
    };
    Ok(artifact)
}

pub(super) fn normalize_structured_stage_body(
    artifact_type: LabArtifactType,
    mut body: serde_json::Value,
) -> serde_json::Value {
    let fields: &[&str] = match artifact_type {
        LabArtifactType::ProfessorPlan => &["success_criteria", "constraints", "risks"],
        LabArtifactType::PostdocPlan => &["slices", "files_expected", "validation_plan"],
        LabArtifactType::GraduateResult => &["changed_files", "validation_attempts", "blockers"],
        LabArtifactType::PostdocIntegrationSummary => &["accepted_results", "remaining_risks"],
        LabArtifactType::ProfessorReview => &["required_revisions"],
        _ => &[],
    };
    if let Some(object) = body.as_object_mut() {
        for field in fields {
            if let Some(value) = object.get_mut(*field) {
                normalize_string_list_value(value);
            }
        }
        for field in structured_string_fields(artifact_type) {
            if let Some(value) = object.get_mut(*field) {
                normalize_string_value(value);
            }
        }
    }
    body
}

pub(super) fn structured_string_fields(artifact_type: LabArtifactType) -> &'static [&'static str] {
    match artifact_type {
        LabArtifactType::ProfessorPlan => &[
            "problem_statement",
            "strategic_direction",
            "handoff_to_postdoc",
        ],
        LabArtifactType::PostdocPlan => &["implementation_summary", "graduate_handoff"],
        LabArtifactType::GraduateResult => &["task_summary", "handoff_to_postdoc"],
        LabArtifactType::PostdocIntegrationSummary => &[
            "integration_summary",
            "validation_status",
            "handoff_to_professor",
        ],
        LabArtifactType::ProfessorReview => {
            &["review_summary", "strategic_assessment", "user_report"]
        }
        _ => &[],
    }
}

pub(super) fn normalize_string_value(value: &mut serde_json::Value) {
    if let Some(item) = structured_value_to_string(value) {
        *value = serde_json::Value::String(item);
    }
}

pub(super) fn normalize_string_list_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            let normalized = items
                .iter()
                .filter_map(structured_value_to_string)
                .filter(|item| !item.trim().is_empty())
                .map(serde_json::Value::String)
                .collect::<Vec<_>>();
            *items = normalized;
        }
        serde_json::Value::String(item) => {
            let item = item.trim().to_string();
            if item.is_empty() {
                *value = serde_json::Value::Array(Vec::new());
            } else {
                *value = serde_json::Value::Array(vec![serde_json::Value::String(item)]);
            }
        }
        other => {
            if let Some(item) = structured_value_to_string(other) {
                *other = serde_json::Value::Array(vec![serde_json::Value::String(item)]);
            }
        }
    }
}

pub(super) fn structured_value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) => Some(value.trim().to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Array(items) => {
            let joined = items
                .iter()
                .filter_map(structured_value_to_string)
                .filter(|item| !item.trim().is_empty())
                .collect::<Vec<_>>()
                .join("; ");
            (!joined.is_empty()).then_some(joined)
        }
        serde_json::Value::Object(object) => {
            for key in [
                "path",
                "file",
                "command",
                "title",
                "name",
                "summary",
                "description",
                "task",
                "goal",
            ] {
                if let Some(value) = object.get(key).and_then(serde_json::Value::as_str) {
                    let value = value.trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
            serde_json::to_string(value).ok()
        }
    }
}

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
