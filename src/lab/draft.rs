use crate::lab::context::build_lab_context_packet_with_evidence_retries_and_artifact_refs;
use crate::lab::model::{
    ArtifactGate, GraduateResult, LabArtifactEnvelope, LabArtifactType, LabMeetingSummary,
    LabProposal, LabProposalIntakeDraft, LabRole, LabRun, LabRunStatus, PostdocIntegrationSummary,
    PostdocPlan, ProfessorPlan, ProfessorReview, RecommendedMode, SponsorMessage,
    SponsorMessageStatus, StageArtifact,
};
use crate::lab::orchestrator::{
    CreatedStageArtifact, LabOrchestrator, LabSchedulerStepAction, LabSchedulerStepResult,
};
use crate::lab::store::{LabCostTokens, LabStore};
use crate::services::api::{sanitize_assistant_content, ChatRequest, LlmProvider, Message, Usage};
use crate::tools::ToolContext;
use anyhow::anyhow;
use chrono::Utc;
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

const LAB_ARTIFACT_DRAFT_SYSTEM_PROMPT: &str = r#"You are drafting a LabRun stage artifact.

Write only the artifact body for the current LabRun stage.
Prefer a strict JSON object that matches the required artifact type schema.
Do not claim code was changed or validation passed unless the supplied context says so.
Keep the output concise, concrete, and ready to persist as a project artifact.
Do not include hidden reasoning, markdown fences, or surrounding commentary."#;

const LAB_ARTIFACT_REVIEW_SYSTEM_PROMPT: &str = r#"You are reviewing a LabRun artifact.

Return only a strict JSON object with:
{"decision":"accept"|"revise","note":"short concrete reason"}

Accept only when the artifact is coherent for its role and does not claim unproven validation.
Use revise when the artifact is incomplete, vague, unsafe, overclaims evidence, or misses required handoff details.
Accept narrow or minimal artifacts when their required fields are concrete, scoped, and verifiable.
Do not reject solely because a slice is intentionally small, uses an existing file, or has a single validation command.
Do not include hidden reasoning, markdown fences, or surrounding commentary."#;

const LAB_PROFESSOR_STRATEGIC_REVIEW_SYSTEM_PROMPT: &str = r#"You are the LabRun Professor agent.

Review the postdoc integration evidence at a strategic level.
Return only a strict JSON object for ProfessorReview with:
review_summary, strategic_assessment, accepted, required_revisions, user_report.
Do not inspect raw code details beyond the supplied evidence summary.
Do not accept closeout unless validation evidence and postdoc handoff are sufficient.
Do not include hidden reasoning, markdown fences, or surrounding commentary."#;

const LAB_SPONSOR_MESSAGE_CLASSIFICATION_SYSTEM_PROMPT: &str = r#"You are the LabRun Professor agent classifying a sponsor side-channel message.

Return only a strict JSON object with:
{"decision":"review"|"meeting"|"task"|"reject","note":"short concrete reason"}

Use review when the message is acknowledged but does not require workflow changes.
Use meeting when the message should become a read-only lab meeting topic.
Use task when the message should become a scoped postdoc/graduate follow-up task.
Use reject when the message is out of scope, unsafe, contradictory, or already superseded.
Do not execute the decision. Only classify the message for the runtime to persist.
Do not include hidden reasoning, markdown fences, or surrounding commentary."#;

const LAB_PROPOSAL_INTAKE_SYSTEM_PROMPT: &str = r#"You are the LabRun Professor agent during project intake.

Clarify the user's project idea into a proposal draft, but do not approve or start implementation.
Return only a strict JSON object with:
{
  "problem_statement": "...",
  "desired_outcome": "...",
  "scope": ["..."],
  "non_goals": ["..."],
  "constraints": ["..."],
  "risks": ["..."],
  "success_criteria": ["..."],
  "recommended_mode": "direct"|"goal"|"labrun",
  "professor_rationale": "..."
}
Use LabRun only when the work benefits from multi-cycle professor/postdoc/graduate orchestration.
Do not include hidden reasoning, markdown fences, or surrounding commentary."#;

const LAB_MEETING_SYSTEM_PROMPT: &str = r#"You are running a read-only LabRun meeting between the Professor and Postdoc agents.

The Professor owns strategy, scope, risk, and sponsor-facing direction.
The Postdoc owns concrete implementation state, validation evidence, blockers, and next technical steps.
Return only a strict JSON object with:
{
  "professor_view": "...",
  "postdoc_view": "...",
  "decision": "continue_current_plan|revise_plan|open_postdoc_task|ask_user|pause",
  "next_actions": ["..."],
  "evidence_ids": ["..."]
}
Do not claim code was changed. Do not start tasks. Do not mark validation passed unless the supplied context already says so.
Do not include hidden reasoning, markdown fences, or surrounding commentary."#;

#[derive(Debug, Clone)]
pub struct LabArtifactDraftOutcome {
    pub created: CreatedStageArtifact,
    pub draft: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabArtifactReviewDecision {
    Accept,
    Revise,
}

#[derive(Debug, Clone)]
pub struct LabArtifactReviewOutcome {
    pub artifact_id: String,
    pub decision: LabArtifactReviewDecision,
    pub note: String,
    pub gate: ArtifactGate,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
pub struct LabProviderStageStepOutcome {
    pub lab_run_id: String,
    pub from_stage: String,
    pub to_stage: String,
    pub artifact_id: String,
    pub review_decision: LabArtifactReviewDecision,
    pub review_note: String,
    pub advanced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabProviderStageRunStopReason {
    MaxSteps,
    GraduateBoundary,
    NeedsUser,
    NotActive,
    RevisionRequested,
    TerminalStage,
}

#[derive(Debug, Clone)]
pub struct LabProviderStageRunOutcome {
    pub lab_run_id: String,
    pub steps: Vec<LabProviderStageStepOutcome>,
    pub final_stage: String,
    pub stop_reason: LabProviderStageRunStopReason,
}

#[derive(Debug, Clone)]
pub struct LabDeterministicStageStepOutcome {
    pub lab_run_id: String,
    pub from_stage: String,
    pub to_stage: String,
    pub artifact_id: String,
    pub gate_satisfied: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum LabHybridRunStep {
    Provider(LabProviderStageStepOutcome),
    Scheduler(LabSchedulerStepResult),
    Deterministic(LabDeterministicStageStepOutcome),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabHybridRunStopReason {
    MaxSteps,
    NeedsUser,
    NotActive,
    RevisionRequested,
    DeterministicGateBlocked,
    SchedulerStopped(LabSchedulerStepAction),
}

#[derive(Debug, Clone)]
pub struct LabHybridRunOutcome {
    pub lab_run_id: String,
    pub steps: Vec<LabHybridRunStep>,
    pub final_stage: String,
    pub stop_reason: LabHybridRunStopReason,
}

#[derive(Debug, Clone)]
pub struct LabHybridCycleRun {
    pub cycle_index: usize,
    pub cycle_count_at_start: u64,
    pub outcome: LabHybridRunOutcome,
    pub continued_to_next_cycle: bool,
    pub compression_artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabHybridCycleRunStopReason {
    MaxCycles,
    CostBudgetExceeded {
        cycle_id: String,
        total_tokens: u64,
        max_cycle_tokens: u64,
    },
    Stopped(LabHybridRunStopReason),
}

#[derive(Debug, Clone)]
pub struct LabHybridCycleRunOutcome {
    pub lab_run_id: String,
    pub cycles: Vec<LabHybridCycleRun>,
    pub final_stage: String,
    pub final_cycle_count: u64,
    pub stop_reason: LabHybridCycleRunStopReason,
}

#[derive(Debug, Clone)]
pub struct LabSponsorMessageClassificationOutcome {
    pub message: SponsorMessage,
    pub decision: String,
    pub note: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
pub struct LabProposalDraftOutcome {
    pub proposal: LabProposal,
    pub draft: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
pub struct LabMeetingDraftOutcome {
    pub created: CreatedStageArtifact,
    pub draft: String,
    pub usage: Option<Usage>,
}

pub async fn draft_lab_proposal_with_provider(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    user_goal: &str,
    user_session_id: Option<String>,
) -> anyhow::Result<LabProposalDraftOutcome> {
    let user_goal = user_goal.trim();
    if user_goal.is_empty() {
        return Err(anyhow!("proposal goal cannot be empty"));
    }
    let prompt = format!(
        "Project root: {}\n\nUser project idea:\n{}\n\nDraft a proposal for formal user approval. Do not start a LabRun.",
        project_root.as_ref().display(),
        user_goal
    );
    let request = ChatRequest {
        max_tokens: Some(1_000),
        ..ChatRequest::new(model).with_messages(vec![
            Message::system(LAB_PROPOSAL_INTAKE_SYSTEM_PROMPT),
            Message::user(prompt),
        ])
    };
    let response = provider.chat(request).await?;
    let draft = sanitize_lab_artifact_draft(&response.content)?;
    let intake = parse_lab_proposal_intake_draft(&draft)?;
    let store = LabStore::for_project(project_root.as_ref());
    let proposal = store.create_proposal_with_intake(
        user_goal,
        user_session_id,
        intake,
        "proposal_professor_drafted",
    )?;
    Ok(LabProposalDraftOutcome {
        proposal,
        draft,
        usage: response.usage,
    })
}

pub async fn draft_current_stage_artifact(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    instructions: &str,
) -> anyhow::Result<LabArtifactDraftOutcome> {
    let project_root = project_root.as_ref();
    let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(project_root);
    let store = LabStore::for_project(project_root);
    let run = store
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for artifact draft"))?;
    let gate = orchestrator.required_gate_for_latest()?;
    let cost = store.cost_summary(&run.lab_run_id)?;
    let evidence = store.list_evidence_refs(&run.lab_run_id)?;
    let retries = store.list_validation_retries(&run.lab_run_id)?;
    let artifact_gate_refs =
        orchestrator.artifact_gate_evidence_context_for_run(&run.lab_run_id, 20)?;
    let packet = build_lab_context_packet_with_evidence_retries_and_artifact_refs(
        &run,
        run.internal_owner,
        &cost,
        &evidence,
        &retries,
        &artifact_gate_refs,
    );
    let mut context_layers = packet
        .layers
        .iter()
        .map(|layer| {
            format!(
                "[{} {} {:?} estimated_tokens={}]\n{}",
                layer.layer, layer.label, layer.stability, layer.estimated_tokens, layer.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    if run.current_stage == "postdoc_plan" {
        if let Some(revision_context) = orchestrator.pending_revision_context_for_run(&run)? {
            context_layers.push_str("\n\n[L6 pending-professor-revision DynamicTail]\n");
            context_layers.push_str(&revision_context);
        }
    }
    let prompt = build_lab_artifact_draft_prompt(
        &run.current_stage,
        &gate.required_artifact_type,
        &context_layers,
        instructions,
    );
    let request = ChatRequest {
        max_tokens: Some(1_200),
        ..ChatRequest::new(model.clone()).with_messages(vec![
            Message::system(LAB_ARTIFACT_DRAFT_SYSTEM_PROMPT),
            Message::user(prompt),
        ])
    };
    let response = provider.chat(request).await?;
    let draft = sanitize_lab_artifact_draft(&response.content)?;
    let created = create_artifact_from_draft(&orchestrator, &store, &run, &draft)?;
    if let Some(usage) = response.usage.as_ref() {
        store.record_cost_usage(
            &run.lab_run_id,
            run.internal_owner,
            &model,
            LabCostTokens {
                prompt_tokens: usage.prompt_tokens as u64,
                completion_tokens: usage.completion_tokens as u64,
                reasoning_tokens: usage.reasoning_tokens.unwrap_or(0) as u64,
                cached_tokens: usage.cached_tokens.unwrap_or(0) as u64,
                cache_write_tokens: usage.cache_write_tokens.unwrap_or(0) as u64,
                cycle_id: Some(run.cycle_count.to_string()),
                meeting_id: None,
            },
            0.0,
            Some("llm_lab_artifact_draft"),
        )?;
    }

    Ok(LabArtifactDraftOutcome {
        created,
        draft,
        usage: response.usage,
    })
}

pub async fn classify_sponsor_message_with_provider(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    message_id: &str,
    instructions: &str,
) -> anyhow::Result<LabSponsorMessageClassificationOutcome> {
    let project_root = project_root.as_ref();
    let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(project_root);
    let store = LabStore::for_project(project_root);
    let run = store
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for sponsor message classification"))?;
    let messages = store.list_sponsor_messages(&run.lab_run_id)?;
    let selected = select_sponsor_message_for_classification(&messages, message_id)?;
    if matches!(
        selected.status,
        SponsorMessageStatus::Applied | SponsorMessageStatus::Superseded
    ) {
        return Err(anyhow!(
            "sponsor message {} is {:?} and cannot be reclassified",
            selected.message_id,
            selected.status
        ));
    }

    let cost = store.cost_summary(&run.lab_run_id)?;
    let evidence = store.list_evidence_refs(&run.lab_run_id)?;
    let retries = store.list_validation_retries(&run.lab_run_id)?;
    let artifact_gate_refs =
        orchestrator.artifact_gate_evidence_context_for_run(&run.lab_run_id, 20)?;
    let packet = build_lab_context_packet_with_evidence_retries_and_artifact_refs(
        &run,
        LabRole::Professor,
        &cost,
        &evidence,
        &retries,
        &artifact_gate_refs,
    );
    let context_layers = packet
        .layers
        .iter()
        .map(|layer| {
            format!(
                "[{} {} {:?} estimated_tokens={}]\n{}",
                layer.layer, layer.label, layer.stability, layer.estimated_tokens, layer.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let prompt =
        build_sponsor_message_classification_prompt(&run, selected, &context_layers, instructions);
    let request = ChatRequest {
        max_tokens: Some(400),
        ..ChatRequest::new(model.clone()).with_messages(vec![
            Message::system(LAB_SPONSOR_MESSAGE_CLASSIFICATION_SYSTEM_PROMPT),
            Message::user(prompt),
        ])
    };
    let response = provider.chat(request).await?;
    let parsed = parse_sponsor_message_classification(&response.content)?;
    let updated = store.update_latest_sponsor_message_status(
        &selected.message_id,
        parsed.decision.status(),
        &format!(
            "provider_classification decision={} note={}",
            parsed.decision.as_str(),
            parsed.note
        ),
    )?;
    if let Some(usage) = response.usage.as_ref() {
        store.record_cost_usage(
            &run.lab_run_id,
            LabRole::Professor,
            &model,
            LabCostTokens {
                prompt_tokens: usage.prompt_tokens as u64,
                completion_tokens: usage.completion_tokens as u64,
                reasoning_tokens: usage.reasoning_tokens.unwrap_or(0) as u64,
                cached_tokens: usage.cached_tokens.unwrap_or(0) as u64,
                cache_write_tokens: usage.cache_write_tokens.unwrap_or(0) as u64,
                cycle_id: Some(run.cycle_count.to_string()),
                meeting_id: None,
            },
            0.0,
            Some("llm_sponsor_message_classification"),
        )?;
    }

    Ok(LabSponsorMessageClassificationOutcome {
        message: updated,
        decision: parsed.decision.as_str().to_string(),
        note: parsed.note,
        usage: response.usage,
    })
}

pub async fn draft_lab_meeting_with_provider(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    topic: Option<&str>,
) -> anyhow::Result<LabMeetingDraftOutcome> {
    let project_root = project_root.as_ref();
    let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(project_root);
    let store = LabStore::for_project(project_root);
    let run = store
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for provider meeting"))?;
    if !matches!(run.status, LabRunStatus::Active | LabRunStatus::NeedsUser) {
        return Err(anyhow!(
            "LabRun {} is not ready for provider meeting: status={:?}",
            run.lab_run_id,
            run.status
        ));
    }
    let topic = topic
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("general LabRun progress review")
        .to_string();
    let cost = store.cost_summary(&run.lab_run_id)?;
    let evidence = store.list_evidence_refs(&run.lab_run_id)?;
    let retries = store.list_validation_retries(&run.lab_run_id)?;
    let artifact_gate_refs =
        orchestrator.artifact_gate_evidence_context_for_run(&run.lab_run_id, 20)?;
    let packet = build_lab_context_packet_with_evidence_retries_and_artifact_refs(
        &run,
        LabRole::Professor,
        &cost,
        &evidence,
        &retries,
        &artifact_gate_refs,
    );
    let context_layers = packet
        .layers
        .iter()
        .map(|layer| {
            format!(
                "[{} {} {:?} estimated_tokens={}]\n{}",
                layer.layer, layer.label, layer.stability, layer.estimated_tokens, layer.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let prompt = build_lab_meeting_prompt(&run, &topic, &context_layers);
    let request = ChatRequest {
        max_tokens: Some(1_000),
        ..ChatRequest::new(model.clone()).with_messages(vec![
            Message::system(LAB_MEETING_SYSTEM_PROMPT),
            Message::user(prompt),
        ])
    };
    let response = provider.chat(request).await?;
    let draft = sanitize_lab_artifact_draft(&response.content)?;
    let parsed = parse_lab_meeting_summary_draft(&draft)?;
    let meeting_id = format!("meeting_{}", Uuid::new_v4().simple());
    let known_evidence_ids = evidence
        .iter()
        .map(|item| item.evidence_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut evidence_ids = parsed
        .evidence_ids
        .into_iter()
        .filter(|id| known_evidence_ids.contains(id.as_str()))
        .collect::<Vec<_>>();
    if evidence_ids.is_empty() {
        evidence_ids = evidence
            .iter()
            .rev()
            .take(20)
            .map(|item| item.evidence_id.clone())
            .collect();
    }
    let artifact_evidence_refs = evidence_ids.clone();
    let usage_total = response
        .usage
        .as_ref()
        .map(|usage| {
            usage.prompt_tokens as u64
                + usage.completion_tokens as u64
                + usage.reasoning_tokens.unwrap_or(0) as u64
        })
        .unwrap_or(0);
    let artifact_id = format!("artifact_labmeeting_{}", Uuid::new_v4().simple());
    let mut artifact = StageArtifact::LabMeetingSummary(LabArtifactEnvelope::new(
        artifact_id,
        run.lab_run_id.clone(),
        LabArtifactType::LabMeetingSummary,
        format!("Provider Lab meeting summary for {}", topic),
        Utc::now(),
        LabMeetingSummary {
            meeting_id: meeting_id.clone(),
            topic: topic.clone(),
            current_stage: run.current_stage.clone(),
            professor_view: parsed.professor_view,
            postdoc_view: parsed.postdoc_view,
            decision: parsed.decision,
            next_actions: parsed.next_actions,
            evidence_ids: evidence_ids.clone(),
            total_tokens: cost.total_tokens.saturating_add(usage_total),
            cache_hit_rate_percent: cost.cache_hit_rate_percent(),
        },
    ));
    if let StageArtifact::LabMeetingSummary(envelope) = &mut artifact {
        envelope.validation_status = Some("read_only_provider_summary".to_string());
        envelope.evidence_refs = artifact_evidence_refs.clone();
    }
    let path = store.write_stage_artifact(&artifact)?;
    let report_path = store.write_stage_artifact_report(&artifact)?;
    let mut saved = store.load_run(&run.lab_run_id)?;
    if !saved.meeting_ids.iter().any(|id| id == &meeting_id) {
        saved.meeting_ids.push(meeting_id.clone());
    }
    saved.updated_at = Utc::now();
    store.save_run(&saved)?;
    if let Some(usage) = response.usage.as_ref() {
        store.record_cost_usage(
            &run.lab_run_id,
            LabRole::Professor,
            &model,
            LabCostTokens {
                prompt_tokens: usage.prompt_tokens as u64,
                completion_tokens: usage.completion_tokens as u64,
                reasoning_tokens: usage.reasoning_tokens.unwrap_or(0) as u64,
                cached_tokens: usage.cached_tokens.unwrap_or(0) as u64,
                cache_write_tokens: usage.cache_write_tokens.unwrap_or(0) as u64,
                cycle_id: Some(run.cycle_count.to_string()),
                meeting_id: Some(meeting_id.clone()),
            },
            0.0,
            Some("llm_lab_meeting"),
        )?;
    }
    store.record_run_event(
        &run.lab_run_id,
        "lab_provider_meeting_summary_written",
        serde_json::json!({
            "meeting_id": meeting_id,
            "topic": topic,
            "artifact_id": artifact.artifact_id(),
            "report_path": report_path.display().to_string(),
            "evidence_refs": artifact_evidence_refs,
        }),
    )?;
    let mut gate = ArtifactGate::new("lab_meeting", "LabMeetingSummary", LabRole::Runtime);
    gate.artifact_id = Some(artifact.artifact_id().to_string());
    gate.validation_status = artifact.validation_status().map(str::to_string);
    gate.evidence_refs.push(path.display().to_string());
    gate.evidence_refs.extend(artifact_evidence_refs);
    gate.evidence_refs.sort();
    gate.evidence_refs.dedup();
    gate.next_action = Some("continue_labrun".to_string());
    Ok(LabMeetingDraftOutcome {
        created: CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        },
        draft,
        usage: response.usage,
    })
}

pub async fn review_stage_artifact_with_provider(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    artifact_id: &str,
    instructions: &str,
) -> anyhow::Result<LabArtifactReviewOutcome> {
    let project_root = project_root.as_ref();
    let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(project_root);
    let store = LabStore::for_project(project_root);
    let run = store
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for artifact review"))?;
    let artifact = store.load_stage_artifact(&run.lab_run_id, artifact_id)?;
    let prompt = build_lab_artifact_review_prompt(&run, &artifact, instructions)?;
    let request = ChatRequest {
        max_tokens: Some(512),
        ..ChatRequest::new(model.clone()).with_messages(vec![
            Message::system(LAB_ARTIFACT_REVIEW_SYSTEM_PROMPT),
            Message::user(prompt),
        ])
    };
    let response = provider.chat(request).await?;
    let review = parse_lab_artifact_review_decision(&response.content)?;
    let gate = match review.decision {
        LabArtifactReviewDecision::Accept => {
            orchestrator.accept_artifact_latest(artifact.artifact_id(), &review.note)?
        }
        LabArtifactReviewDecision::Revise => {
            orchestrator.revise_artifact_latest(artifact.artifact_id(), &review.note)?
        }
    };
    if let Some(usage) = response.usage.as_ref() {
        store.record_cost_usage(
            &run.lab_run_id,
            reviewer_role_for_artifact(&artifact),
            &model,
            LabCostTokens {
                prompt_tokens: usage.prompt_tokens as u64,
                completion_tokens: usage.completion_tokens as u64,
                reasoning_tokens: usage.reasoning_tokens.unwrap_or(0) as u64,
                cached_tokens: usage.cached_tokens.unwrap_or(0) as u64,
                cache_write_tokens: usage.cache_write_tokens.unwrap_or(0) as u64,
                cycle_id: Some(run.cycle_count.to_string()),
                meeting_id: None,
            },
            0.0,
            Some("llm_lab_artifact_review"),
        )?;
    }
    Ok(LabArtifactReviewOutcome {
        artifact_id: artifact.artifact_id().to_string(),
        decision: review.decision,
        note: review.note,
        gate,
        usage: response.usage,
    })
}

pub async fn draft_professor_review_with_provider(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    instructions: &str,
) -> anyhow::Result<LabArtifactDraftOutcome> {
    let project_root = project_root.as_ref();
    let store = LabStore::for_project(project_root);
    let run = store
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for provider professor review"))?;
    if !matches!(run.status, LabRunStatus::Active) {
        return Err(anyhow!(
            "LabRun {} is not active: {:?}",
            run.lab_run_id,
            run.status
        ));
    }
    if run.current_stage != "professor_review" {
        return Err(anyhow!(
            "LabRun {} is at stage '{}', not professor_review",
            run.lab_run_id,
            run.current_stage
        ));
    }

    let integration = store
        .list_stage_artifacts(&run.lab_run_id)?
        .into_iter()
        .filter_map(|artifact| match artifact {
            StageArtifact::PostdocIntegrationSummary(summary) => Some(summary),
            _ => None,
        })
        .last()
        .ok_or_else(|| {
            anyhow!(
                "LabRun {} has no PostdocIntegrationSummary artifact for provider professor review",
                run.lab_run_id
            )
        })?;

    let cost = store.cost_summary(&run.lab_run_id)?;
    let evidence = store.list_evidence_refs(&run.lab_run_id)?;
    let retries = store.list_validation_retries(&run.lab_run_id)?;
    let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(project_root);
    let artifact_gate_refs =
        orchestrator.artifact_gate_evidence_context_for_run(&run.lab_run_id, 20)?;
    let packet = build_lab_context_packet_with_evidence_retries_and_artifact_refs(
        &run,
        LabRole::Professor,
        &cost,
        &evidence,
        &retries,
        &artifact_gate_refs,
    );
    let prompt = build_provider_professor_review_prompt(
        &run,
        &integration,
        &packet
            .layers
            .iter()
            .map(|layer| {
                format!(
                    "[{} {} {:?} estimated_tokens={}]\n{}",
                    layer.layer,
                    layer.label,
                    layer.stability,
                    layer.estimated_tokens,
                    layer.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        instructions,
    )?;
    let request = ChatRequest {
        max_tokens: Some(1_200),
        ..ChatRequest::new(model.clone()).with_messages(vec![
            Message::system(LAB_PROFESSOR_STRATEGIC_REVIEW_SYSTEM_PROMPT),
            Message::user(prompt),
        ])
    };
    let response = provider.chat(request).await?;
    let draft = sanitize_lab_artifact_draft(&response.content)?;
    let value: serde_json::Value = serde_json::from_str(&draft)
        .map_err(|err| anyhow!("invalid provider ProfessorReview JSON: {err}"))?;
    let body = body_value_for_artifact_type(value, LabArtifactType::ProfessorReview)?;
    let mut review: ProfessorReview = serde_json::from_value(body)?;
    enforce_professor_review_evidence_boundary(&integration, &mut review);

    let mut professor_evidence_refs = vec![
        format!("artifact:{}", integration.artifact_id),
        format!("stage:{}", integration.stage),
    ];
    professor_evidence_refs.extend(integration.evidence_refs.iter().cloned());
    professor_evidence_refs.sort();
    professor_evidence_refs.dedup();
    let artifact_id = format!("artifact_professorreview_{}", Uuid::new_v4().simple());
    let mut artifact = StageArtifact::ProfessorReview(LabArtifactEnvelope::new(
        artifact_id,
        run.lab_run_id.clone(),
        LabArtifactType::ProfessorReview,
        "Provider professor review".to_string(),
        Utc::now(),
        review.clone(),
    ));
    if let StageArtifact::ProfessorReview(envelope) = &mut artifact {
        envelope.evidence_refs = professor_evidence_refs.clone();
        envelope.validation_status = Some(if review.accepted {
            "validated".to_string()
        } else {
            "needs_revision".to_string()
        });
    }
    let path = store.write_stage_artifact(&artifact)?;
    let report_path = store.write_stage_artifact_report(&artifact)?;

    let mut gate = ArtifactGate::new(
        "professor_review",
        LabArtifactType::ProfessorReview.as_str(),
        LabRole::Professor,
    );
    gate.artifact_id = Some(artifact.artifact_id().to_string());
    gate.validation_status = artifact.validation_status().map(str::to_string);
    gate.evidence_refs.push(path.display().to_string());
    gate.evidence_refs.extend(professor_evidence_refs.clone());
    gate.evidence_refs.sort();
    gate.evidence_refs.dedup();
    if review.accepted {
        gate.next_action = Some("user_report".to_string());
    } else {
        gate.blockers = review.required_revisions.clone();
        gate.next_action = Some("postdoc_revision".to_string());
    }
    store.write_artifact_gate(&run.lab_run_id, &gate)?;
    if gate.is_satisfied() {
        store.validate_artifact_gate(&run.lab_run_id, "professor_review")?;
    }
    store.record_run_event(
        &run.lab_run_id,
        "provider_professor_review_written",
        serde_json::json!({
            "artifact_id": artifact.artifact_id(),
            "postdoc_integration_artifact_id": integration.artifact_id,
            "accepted": review.accepted,
            "report_path": report_path.display().to_string(),
            "validation_status": artifact.validation_status(),
            "evidence_refs": professor_evidence_refs,
        }),
    )?;
    if !review.accepted {
        crate::lab::orchestrator::LabOrchestrator::for_project(project_root)
            .create_revision_task_from_professor_review_artifact(&run, &artifact)?;
    }

    if let Some(usage) = response.usage.as_ref() {
        store.record_cost_usage(
            &run.lab_run_id,
            LabRole::Professor,
            &model,
            LabCostTokens {
                prompt_tokens: usage.prompt_tokens as u64,
                completion_tokens: usage.completion_tokens as u64,
                reasoning_tokens: usage.reasoning_tokens.unwrap_or(0) as u64,
                cached_tokens: usage.cached_tokens.unwrap_or(0) as u64,
                cache_write_tokens: usage.cache_write_tokens.unwrap_or(0) as u64,
                cycle_id: Some(run.cycle_count.to_string()),
                meeting_id: None,
            },
            0.0,
            Some("llm_lab_professor_review"),
        )?;
    }

    Ok(LabArtifactDraftOutcome {
        created: CreatedStageArtifact {
            artifact,
            path,
            report_path,
            gate,
        },
        draft,
        usage: response.usage,
    })
}

pub async fn run_provider_stage_step(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    instructions: &str,
) -> anyhow::Result<LabProviderStageStepOutcome> {
    let project_root = project_root.as_ref();
    let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(project_root);
    let store = LabStore::for_project(project_root);
    let run = store
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for provider stage step"))?;
    if run.needs_user || !matches!(run.status, crate::lab::model::LabRunStatus::Active) {
        return Err(anyhow!(
            "LabRun {} is not ready for provider stage step: status={:?} needs_user={}",
            run.lab_run_id,
            run.status,
            run.needs_user
        ));
    }
    if run.current_stage == "graduate_work" {
        return Err(anyhow!(
            "provider stage step does not execute graduate_work; use strict graduate task scheduler"
        ));
    }
    let draft =
        draft_current_stage_artifact(project_root, provider.clone(), model.clone(), instructions)
            .await?;
    let review = review_stage_artifact_with_provider(
        project_root,
        provider,
        model,
        draft.created.artifact.artifact_id(),
        "Review the just-drafted artifact for this LabRun stage. Accept only if it is specific enough for the next role handoff and does not overclaim evidence.",
    )
    .await?;
    let mut to_stage = run.current_stage.clone();
    let mut advanced = false;
    if matches!(review.decision, LabArtifactReviewDecision::Accept) {
        let advanced_run = orchestrator.advance_latest()?;
        to_stage = advanced_run.current_stage;
        advanced = true;
    }
    Ok(LabProviderStageStepOutcome {
        lab_run_id: run.lab_run_id,
        from_stage: run.current_stage,
        to_stage,
        artifact_id: draft.created.artifact.artifact_id().to_string(),
        review_decision: review.decision,
        review_note: review.note,
        advanced,
    })
}

pub async fn run_provider_stage_steps_until_boundary(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    max_steps: usize,
    instructions: &str,
) -> anyhow::Result<LabProviderStageRunOutcome> {
    if max_steps == 0 {
        return Err(anyhow!("max_steps must be greater than zero"));
    }

    let project_root = project_root.as_ref();
    let store = LabStore::for_project(project_root);
    let mut steps = Vec::new();

    loop {
        let run = store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for provider stage run"))?;
        if run.needs_user || matches!(run.status, LabRunStatus::NeedsUser) {
            return Ok(LabProviderStageRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabProviderStageRunStopReason::NeedsUser,
            });
        }
        if !matches!(run.status, LabRunStatus::Active) {
            return Ok(LabProviderStageRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabProviderStageRunStopReason::NotActive,
            });
        }
        if run.current_stage == "user_report" {
            return Ok(LabProviderStageRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabProviderStageRunStopReason::TerminalStage,
            });
        }
        if run.current_stage == "graduate_work" {
            return Ok(LabProviderStageRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabProviderStageRunStopReason::GraduateBoundary,
            });
        }
        if steps.len() >= max_steps {
            return Ok(LabProviderStageRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabProviderStageRunStopReason::MaxSteps,
            });
        }

        let step =
            run_provider_stage_step(project_root, provider.clone(), model.clone(), instructions)
                .await?;
        let advanced = step.advanced;
        steps.push(step);
        if !advanced {
            let run = store
                .latest_run()?
                .ok_or_else(|| anyhow!("no LabRun found after provider stage step"))?;
            return Ok(LabProviderStageRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabProviderStageRunStopReason::RevisionRequested,
            });
        }
    }
}

pub async fn run_hybrid_lab_steps_until_boundary(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    max_steps: usize,
    instructions: &str,
    tool_context: ToolContext,
) -> anyhow::Result<LabHybridRunOutcome> {
    if max_steps == 0 {
        return Err(anyhow!("max_steps must be greater than zero"));
    }

    let project_root = project_root.as_ref();
    let orchestrator = LabOrchestrator::for_project(project_root);
    let store = LabStore::for_project(project_root);
    let mut steps = Vec::new();

    loop {
        let run = store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for hybrid LabRun step"))?;
        if run.needs_user || matches!(run.status, LabRunStatus::NeedsUser) {
            return Ok(LabHybridRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabHybridRunStopReason::NeedsUser,
            });
        }
        if !matches!(run.status, LabRunStatus::Active) {
            return Ok(LabHybridRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabHybridRunStopReason::NotActive,
            });
        }
        if steps.len() >= max_steps {
            return Ok(LabHybridRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabHybridRunStopReason::MaxSteps,
            });
        }

        if run.current_stage == "graduate_work" {
            let step = orchestrator
                .run_scheduler_step_latest_with_context(tool_context.clone())
                .await?;
            let should_continue = matches!(step.action, LabSchedulerStepAction::TickAdvanced);
            let stop_action = step.action.clone();
            steps.push(LabHybridRunStep::Scheduler(step));
            if should_continue {
                continue;
            }
            let run = store
                .latest_run()?
                .ok_or_else(|| anyhow!("no LabRun found after scheduler step"))?;
            return Ok(LabHybridRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabHybridRunStopReason::SchedulerStopped(stop_action),
            });
        }

        if run.current_stage == "postdoc_review" || run.current_stage == "professor_review" {
            let step = run_deterministic_review_stage(&orchestrator, &run.current_stage)?;
            let gate_satisfied = step.gate_satisfied;
            steps.push(LabHybridRunStep::Deterministic(step));
            if gate_satisfied {
                continue;
            }
            let run = store
                .latest_run()?
                .ok_or_else(|| anyhow!("no LabRun found after deterministic review step"))?;
            return Ok(LabHybridRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabHybridRunStopReason::DeterministicGateBlocked,
            });
        }

        let step =
            run_provider_stage_step(project_root, provider.clone(), model.clone(), instructions)
                .await?;
        let advanced = step.advanced;
        steps.push(LabHybridRunStep::Provider(step));
        if !advanced {
            let run = store
                .latest_run()?
                .ok_or_else(|| anyhow!("no LabRun found after provider stage step"))?;
            return Ok(LabHybridRunOutcome {
                lab_run_id: run.lab_run_id,
                steps,
                final_stage: run.current_stage,
                stop_reason: LabHybridRunStopReason::RevisionRequested,
            });
        }
    }
}

pub async fn run_hybrid_lab_cycles_until_boundary(
    project_root: impl AsRef<Path>,
    provider: Arc<dyn LlmProvider>,
    model: String,
    max_cycles: usize,
    max_steps_per_cycle: usize,
    instructions: &str,
    tool_context: ToolContext,
) -> anyhow::Result<LabHybridCycleRunOutcome> {
    if max_cycles == 0 {
        return Err(anyhow!("max_cycles must be greater than zero"));
    }
    if max_steps_per_cycle == 0 {
        return Err(anyhow!("max_steps_per_cycle must be greater than zero"));
    }

    let project_root = project_root.as_ref();
    let orchestrator = LabOrchestrator::for_project(project_root);
    let store = LabStore::for_project(project_root);
    let mut cycles = Vec::new();
    let mut completed_cycles = 0usize;

    loop {
        let run = store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found for hybrid cycle run"))?;
        let cycle_tokens = lab_cycle_total_tokens(&store, &run.lab_run_id, run.cycle_count)?;
        if cycle_tokens >= run.cost_policy.max_cycle_tokens {
            return Ok(LabHybridCycleRunOutcome {
                lab_run_id: run.lab_run_id,
                cycles,
                final_stage: run.current_stage,
                final_cycle_count: run.cycle_count,
                stop_reason: LabHybridCycleRunStopReason::CostBudgetExceeded {
                    cycle_id: run.cycle_count.to_string(),
                    total_tokens: cycle_tokens,
                    max_cycle_tokens: run.cost_policy.max_cycle_tokens,
                },
            });
        }
        if cycles.is_empty()
            && run.current_stage == "user_report"
            && (run.needs_user || matches!(run.status, LabRunStatus::NeedsUser))
        {
            orchestrator.continue_latest_from_user_report(
                "Bounded hybrid cycle run explicitly continued from user_report.",
            )?;
            continue;
        }

        let cycle_index = cycles.len() + 1;
        let cycle_count_at_start = run.cycle_count;
        let outcome = run_hybrid_lab_steps_until_boundary(
            project_root,
            provider.clone(),
            model.clone(),
            max_steps_per_cycle,
            instructions,
            tool_context.clone(),
        )
        .await?;
        let reached_user_report = outcome.final_stage == "user_report"
            && matches!(outcome.stop_reason, LabHybridRunStopReason::NeedsUser);
        cycles.push(LabHybridCycleRun {
            cycle_index,
            cycle_count_at_start,
            outcome,
            continued_to_next_cycle: false,
            compression_artifact_ids: Vec::new(),
        });

        if reached_user_report {
            completed_cycles = completed_cycles.saturating_add(1);
            let compression_artifact_ids = auto_compress_completed_cycle(&orchestrator, &store)?;
            if let Some(last) = cycles.last_mut() {
                last.compression_artifact_ids = compression_artifact_ids;
            }
            let run = store
                .latest_run()?
                .ok_or_else(|| anyhow!("no LabRun found after hybrid cycle"))?;
            if completed_cycles >= max_cycles {
                return Ok(LabHybridCycleRunOutcome {
                    lab_run_id: run.lab_run_id,
                    cycles,
                    final_stage: run.current_stage,
                    final_cycle_count: run.cycle_count,
                    stop_reason: LabHybridCycleRunStopReason::MaxCycles,
                });
            }
            orchestrator.continue_latest_from_user_report(&format!(
                "Bounded hybrid cycle run continuing after cycle {}.",
                run.cycle_count
            ))?;
            if let Some(last) = cycles.last_mut() {
                last.continued_to_next_cycle = true;
            }
            continue;
        }

        let run = store
            .latest_run()?
            .ok_or_else(|| anyhow!("no LabRun found after hybrid cycle stop"))?;
        let stop_reason = cycles
            .last()
            .map(|cycle| LabHybridCycleRunStopReason::Stopped(cycle.outcome.stop_reason.clone()))
            .unwrap_or(LabHybridCycleRunStopReason::Stopped(
                LabHybridRunStopReason::NotActive,
            ));
        return Ok(LabHybridCycleRunOutcome {
            lab_run_id: run.lab_run_id,
            cycles,
            final_stage: run.current_stage,
            final_cycle_count: run.cycle_count,
            stop_reason,
        });
    }
}

fn lab_cycle_total_tokens(
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

fn auto_compress_completed_cycle(
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

fn run_deterministic_review_stage(
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

fn build_lab_artifact_draft_prompt(
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

fn build_lab_artifact_review_prompt(
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

fn build_provider_professor_review_prompt(
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

fn build_sponsor_message_classification_prompt(
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

fn build_lab_meeting_prompt(run: &LabRun, topic: &str, context_layers: &str) -> String {
    format!(
        "lab_run_id: {}\ncurrent_stage: {}\ncycle: {}\nmeeting_topic: {}\n\nLabRun context layers:\n{}\n\nDraft the read-only Lab meeting summary now.",
        run.lab_run_id,
        run.current_stage,
        run.cycle_count,
        topic,
        context_layers
    )
}

fn enforce_professor_review_evidence_boundary(
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
struct RawArtifactReviewDecision {
    decision: String,
    note: String,
}

#[derive(Debug, Deserialize)]
struct RawSponsorMessageClassification {
    decision: String,
    note: String,
}

#[derive(Debug, Deserialize)]
struct RawLabProposalIntakeDraft {
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
struct RawLabMeetingSummaryDraft {
    professor_view: String,
    postdoc_view: String,
    decision: String,
    #[serde(default)]
    next_actions: Vec<String>,
    #[serde(default)]
    evidence_ids: Vec<String>,
}

#[derive(Debug)]
struct ParsedArtifactReviewDecision {
    decision: LabArtifactReviewDecision,
    note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SponsorMessageClassificationDecision {
    Review,
    Meeting,
    Task,
    Reject,
}

impl SponsorMessageClassificationDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Meeting => "meeting",
            Self::Task => "task",
            Self::Reject => "reject",
        }
    }

    fn status(self) -> SponsorMessageStatus {
        match self {
            Self::Review => SponsorMessageStatus::Reviewed,
            Self::Meeting => SponsorMessageStatus::ConvertedToMeeting,
            Self::Task => SponsorMessageStatus::ConvertedToTask,
            Self::Reject => SponsorMessageStatus::Rejected,
        }
    }
}

#[derive(Debug)]
struct ParsedSponsorMessageClassification {
    decision: SponsorMessageClassificationDecision,
    note: String,
}

fn select_sponsor_message_for_classification<'a>(
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
            .or_else(|| messages.iter().rev().next())
            .ok_or_else(|| anyhow!("no sponsor messages found"));
    }
    messages
        .iter()
        .find(|message| message.message_id == message_id)
        .ok_or_else(|| anyhow!("sponsor message not found: {message_id}"))
}

fn parse_lab_artifact_review_decision(
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

fn parse_sponsor_message_classification(
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

fn parse_lab_proposal_intake_draft(content: &str) -> anyhow::Result<LabProposalIntakeDraft> {
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

fn parse_lab_meeting_summary_draft(content: &str) -> anyhow::Result<RawLabMeetingSummaryDraft> {
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

fn clean_draft_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn reviewer_role_for_artifact(artifact: &StageArtifact) -> LabRole {
    match artifact.artifact_type() {
        LabArtifactType::ProfessorPlan => LabRole::Professor,
        LabArtifactType::PostdocPlan => LabRole::Postdoc,
        LabArtifactType::GraduateResult => LabRole::Postdoc,
        LabArtifactType::PostdocIntegrationSummary => LabRole::Professor,
        LabArtifactType::ProfessorReview => LabRole::Professor,
        _ => LabRole::Runtime,
    }
}

fn sanitize_lab_artifact_draft(content: &str) -> anyhow::Result<String> {
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

fn create_artifact_from_draft(
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

fn parse_structured_stage_artifact(
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

fn body_value_for_artifact_type(
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

fn build_structured_stage_artifact(
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

fn normalize_structured_stage_body(
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

fn structured_string_fields(artifact_type: LabArtifactType) -> &'static [&'static str] {
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

fn normalize_string_value(value: &mut serde_json::Value) {
    if let Some(item) = structured_value_to_string(value) {
        *value = serde_json::Value::String(item);
    }
}

fn normalize_string_list_value(value: &mut serde_json::Value) {
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

fn structured_value_to_string(value: &serde_json::Value) -> Option<String> {
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

fn artifact_type_for_stage(stage: &str) -> anyhow::Result<LabArtifactType> {
    match stage {
        "professor_discussion" => Ok(LabArtifactType::ProfessorPlan),
        "postdoc_plan" => Ok(LabArtifactType::PostdocPlan),
        "graduate_work" => Ok(LabArtifactType::GraduateResult),
        "postdoc_review" => Ok(LabArtifactType::PostdocIntegrationSummary),
        "professor_review" => Ok(LabArtifactType::ProfessorReview),
        _ => Err(anyhow!("unknown LabRun artifact stage: {stage}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::model::{LabArtifactType, StageArtifact};
    use crate::services::api::{ChatResponse, ToolCall};
    use async_openai::types::ChatCompletionResponseStream;
    use async_trait::async_trait;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[test]
    fn sponsor_message_classification_parses_task_decision() {
        let parsed = parse_sponsor_message_classification(
            r#"{"decision":"convert_to_task","note":"Needs a scoped follow-up."}"#,
        )
        .unwrap();

        assert_eq!(parsed.decision, SponsorMessageClassificationDecision::Task);
        assert_eq!(
            parsed.decision.status(),
            SponsorMessageStatus::ConvertedToTask
        );
        assert_eq!(parsed.note, "Needs a scoped follow-up.");
    }

    #[test]
    fn proposal_intake_draft_parses_mode_and_cleans_lists() {
        let parsed = parse_lab_proposal_intake_draft(
            r#"{
                "problem_statement":"Build a lab workflow",
                "desired_outcome":"A resumable LabRun project loop",
                "scope":[" professor intake ","","postdoc planning"],
                "non_goals":["ship without approval"],
                "constraints":["preserve runtime gates"],
                "risks":["graduate drift"],
                "success_criteria":["approval required before mutation"],
                "recommended_mode":"lab_run",
                "professor_rationale":"This needs multi-cycle coordination."
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.recommended_mode, RecommendedMode::Labrun);
        assert_eq!(
            parsed.scope,
            vec![
                "professor intake".to_string(),
                "postdoc planning".to_string()
            ]
        );
        assert_eq!(
            parsed.success_criteria,
            vec!["approval required before mutation".to_string()]
        );
    }

    #[test]
    fn lab_meeting_draft_requires_views_decision_and_actions() {
        let parsed = parse_lab_meeting_summary_draft(
            r#"{
                "professor_view":"Strategy should stay focused.",
                "postdoc_view":"Implementation is blocked on validation.",
                "decision":"revise_plan",
                "next_actions":["revise postdoc plan"],
                "evidence_ids":[" labevidence_1 ",""]
            }"#,
        )
        .unwrap();

        assert_eq!(parsed.decision, "revise_plan");
        assert_eq!(parsed.next_actions, vec!["revise postdoc plan".to_string()]);
        assert_eq!(parsed.evidence_ids, vec!["labevidence_1".to_string()]);
        assert!(parse_lab_meeting_summary_draft(
            r#"{"professor_view":"ok","postdoc_view":"ok","decision":"continue","next_actions":[]}"#,
        )
        .unwrap_err()
        .to_string()
        .contains("next_actions"));
    }

    #[test]
    fn structured_postdoc_plan_accepts_object_list_items_from_provider() {
        let now = Utc::now();
        let proposal = LabProposal::new(
            "proposal_test".to_string(),
            "/tmp/lab".to_string(),
            None,
            "Validate hybrid cycle parser".to_string(),
            now,
        );
        let mut run = LabRun::from_proposal("labrun_test".to_string(), &proposal, now);
        run.current_stage = "postdoc_plan".to_string();

        let artifact = parse_structured_stage_artifact(
            &run,
            r#"{
                "postdoc_plan": {
                    "implementation_summary": "Use a minimal scoped proof.",
                    "slices": [
                        {"title": "Create proof file", "description": "Write one file"},
                        "Verify proof file"
                    ],
                    "files_expected": [
                        {"path": "lab-proof.md"},
                        "README.md"
                    ],
                    "validation_plan": "test -f lab-proof.md",
                    "graduate_handoff": {"summary": "Create the scoped proof file only."}
                }
            }"#,
        )
        .unwrap()
        .expect("structured artifact");

        let StageArtifact::PostdocPlan(plan) = artifact else {
            panic!("expected PostdocPlan");
        };
        assert_eq!(
            plan.body.slices,
            vec![
                "Create proof file".to_string(),
                "Verify proof file".to_string()
            ]
        );
        assert_eq!(
            plan.body.files_expected,
            vec!["lab-proof.md".to_string(), "README.md".to_string()]
        );
        assert_eq!(
            plan.body.validation_plan,
            vec!["test -f lab-proof.md".to_string()]
        );
    }

    struct DraftProvider {
        response: String,
        seen_prompt: Mutex<Option<String>>,
    }

    #[async_trait]
    impl LlmProvider for DraftProvider {
        async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let prompt = match request.messages.last() {
                Some(Message::User { content }) => content.clone(),
                _ => String::new(),
            };
            *self.seen_prompt.lock().unwrap() = Some(prompt);
            Ok(ChatResponse {
                content: self.response.clone(),
                tool_calls: None::<Vec<ToolCall>>,
                usage: Some(Usage {
                    prompt_tokens: 100,
                    completion_tokens: 25,
                    total_tokens: 125,
                    reasoning_tokens: Some(5),
                    cached_tokens: Some(20),
                    cache_write_tokens: Some(10),
                }),
                tool_call_repair: None,
                finish_reason: Some("stop".to_string()),
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            anyhow::bail!("streaming is not used by Lab artifact draft tests")
        }

        fn base_url(&self) -> &str {
            "mock://lab-draft"
        }

        fn default_model(&self) -> &str {
            "mock-lab-draft"
        }
    }

    #[tokio::test]
    async fn llm_proposal_draft_creates_structured_proposal_without_labrun() {
        let temp = tempfile::tempdir().unwrap();
        let provider = Arc::new(DraftProvider {
            response: serde_json::json!({
                "problem_statement": "The project needs a formal lab workflow.",
                "desired_outcome": "A persisted professor/postdoc/graduate loop.",
                "scope": ["professor intake", "approval boundary", "runtime persistence"],
                "non_goals": ["mutate code before approval"],
                "constraints": ["preserve existing direct mode"],
                "risks": ["too much ceremony"],
                "success_criteria": ["proposal exists before LabRun", "approval remains explicit"],
                "recommended_mode": "labrun",
                "professor_rationale": "The work spans design, implementation, and review."
            })
            .to_string(),
            seen_prompt: Mutex::new(None),
        });

        let outcome = draft_lab_proposal_with_provider(
            temp.path(),
            provider.clone(),
            "mock-lab-proposal".to_string(),
            "Build Lab Mode",
            Some("session_1".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(outcome.proposal.user_goal, "Build Lab Mode");
        assert_eq!(
            outcome.proposal.problem_statement,
            "The project needs a formal lab workflow."
        );
        assert_eq!(outcome.proposal.recommended_mode, RecommendedMode::Labrun);
        assert_eq!(
            outcome.proposal.success_criteria,
            vec![
                "proposal exists before LabRun".to_string(),
                "approval remains explicit".to_string()
            ]
        );
        let store = LabStore::for_project(temp.path());
        assert!(store.latest_run().unwrap().is_none());
        let saved = store.latest_proposal().unwrap().unwrap();
        assert_eq!(saved.proposal_id, outcome.proposal.proposal_id);
        let prompt = provider.seen_prompt.lock().unwrap().clone().unwrap();
        assert!(prompt.contains("Build Lab Mode"));
        assert!(prompt.contains("Do not start a LabRun"));
    }

    #[tokio::test]
    async fn provider_meeting_writes_read_only_summary_and_usage() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let evidence = store
            .record_evidence_ref(
                &run.lab_run_id,
                crate::lab::model::LabEvidenceKind::File,
                LabRole::Postdoc,
                "target/proof.txt",
                "validation proof",
                None,
                Some("0"),
            )
            .unwrap();
        let provider = Arc::new(DraftProvider {
            response: serde_json::json!({
                "professor_view": "Strategy should stay narrow.",
                "postdoc_view": "Implementation needs one validation repair.",
                "decision": "revise_plan",
                "next_actions": ["revise the postdoc plan"],
                "evidence_ids": [evidence.evidence_id, "made_up_evidence"]
            })
            .to_string(),
            seen_prompt: Mutex::new(None),
        });

        let outcome = draft_lab_meeting_with_provider(
            temp.path(),
            provider.clone(),
            "mock-lab-meeting".to_string(),
            Some("validation repair meeting"),
        )
        .await
        .unwrap();

        assert_eq!(
            outcome.created.artifact.artifact_type(),
            LabArtifactType::LabMeetingSummary
        );
        let StageArtifact::LabMeetingSummary(envelope) = &outcome.created.artifact else {
            panic!("expected LabMeetingSummary");
        };
        assert_eq!(envelope.body.topic, "validation repair meeting");
        assert_eq!(envelope.body.decision, "revise_plan");
        assert_eq!(
            envelope.validation_status.as_deref(),
            Some("read_only_provider_summary")
        );
        assert_eq!(envelope.body.evidence_ids.len(), 1);
        assert!(envelope
            .evidence_refs
            .iter()
            .any(|item| item == &evidence.evidence_id));
        assert!(outcome
            .created
            .gate
            .evidence_refs
            .iter()
            .any(|item| item == &evidence.evidence_id));
        let saved = store.load_run(&run.lab_run_id).unwrap();
        assert_eq!(saved.meeting_ids.len(), 1);
        assert!(outcome.created.report_path.exists());
        let usage = store.list_cost_usage(&run.lab_run_id).unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(
            usage[0].meeting_id.as_deref(),
            Some(envelope.body.meeting_id.as_str())
        );
        assert_eq!(usage[0].note.as_deref(), Some("llm_lab_meeting"));
        let prompt = provider.seen_prompt.lock().unwrap().clone().unwrap();
        assert!(prompt.contains("validation repair meeting"));
        assert!(prompt.contains("LabRun context layers"));
    }

    struct SequenceProvider {
        responses: Mutex<VecDeque<String>>,
    }

    #[async_trait]
    impl LlmProvider for SequenceProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let content = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("missing mock response");
            Ok(ChatResponse {
                content,
                tool_calls: None::<Vec<ToolCall>>,
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                    cached_tokens: None,
                    cache_write_tokens: None,
                }),
                tool_call_repair: None,
                finish_reason: Some("stop".to_string()),
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            anyhow::bail!("streaming is not used by Lab provider step tests")
        }

        fn base_url(&self) -> &str {
            "mock://lab-provider-step"
        }

        fn default_model(&self) -> &str {
            "mock-lab-provider-step"
        }
    }

    #[tokio::test]
    async fn llm_draft_writes_current_stage_artifact_and_usage() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let provider = Arc::new(DraftProvider {
            response: "Professor plan\n\nKeep runtime gates strict.".to_string(),
            seen_prompt: Mutex::new(None),
        });

        let outcome = draft_current_stage_artifact(
            temp.path(),
            provider.clone(),
            "mock-lab-draft".to_string(),
            "focus on architecture",
        )
        .await
        .unwrap();

        assert_eq!(
            outcome.created.artifact.artifact_type(),
            LabArtifactType::ProfessorPlan
        );
        assert!(outcome.created.path.exists());
        assert!(outcome.created.report_path.exists());
        assert!(matches!(
            outcome.created.artifact,
            StageArtifact::ProfessorPlan(ref plan)
                if plan.body.strategic_direction.contains("Keep runtime gates strict")
        ));
        let gate = store
            .load_artifact_gate(&run.lab_run_id, "professor_discussion")
            .unwrap();
        assert_eq!(
            gate.artifact_id.as_deref(),
            Some(outcome.created.artifact.artifact_id())
        );
        let usage = store.list_cost_usage(&run.lab_run_id).unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].prompt_tokens, 100);
        assert_eq!(usage[0].cached_tokens, 20);
        let prompt = provider.seen_prompt.lock().unwrap().clone().unwrap();
        assert!(prompt.contains("required_artifact_type: ProfessorPlan"));
        assert!(prompt.contains("focus on architecture"));
    }

    #[tokio::test]
    async fn llm_draft_parses_structured_professor_plan_json() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let provider = Arc::new(DraftProvider {
            response: serde_json::json!({
                "professor_plan": {
                    "problem_statement": "Build the LabRun workflow.",
                    "strategic_direction": "Preserve runtime gates while adding role loops.",
                    "success_criteria": ["Explicit professor gate", "Postdoc owns validation"],
                    "constraints": ["Do not bypass permissions"],
                    "risks": ["Over-automation without evidence"],
                    "handoff_to_postdoc": "Create scoped implementation slices."
                }
            })
            .to_string(),
            seen_prompt: Mutex::new(None),
        });

        let outcome = draft_current_stage_artifact(
            temp.path(),
            provider,
            "mock-lab-draft".to_string(),
            "write strict JSON",
        )
        .await
        .unwrap();

        match outcome.created.artifact {
            StageArtifact::ProfessorPlan(plan) => {
                assert_eq!(
                    plan.body.strategic_direction,
                    "Preserve runtime gates while adding role loops."
                );
                assert_eq!(
                    plan.body.success_criteria,
                    vec![
                        "Explicit professor gate".to_string(),
                        "Postdoc owns validation".to_string()
                    ]
                );
                assert_eq!(
                    plan.body.handoff_to_postdoc,
                    "Create scoped implementation slices."
                );
            }
            other => panic!("expected ProfessorPlan, got {:?}", other.artifact_type()),
        }
    }

    #[tokio::test]
    async fn llm_draft_structured_postdoc_plan_gate_inherits_revision_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path());
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.current_stage = "postdoc_plan".to_string();
        run.internal_owner = LabRole::Postdoc;
        store.save_run(&run).unwrap();

        let review_artifact = StageArtifact::ProfessorReview(LabArtifactEnvelope::new(
            "artifact_professorreview_requires_revision".to_string(),
            run.lab_run_id.clone(),
            LabArtifactType::ProfessorReview,
            "Professor review requiring revision".to_string(),
            Utc::now(),
            ProfessorReview {
                review_summary: "Professor requires a narrower implementation plan.".to_string(),
                strategic_assessment: "Current plan lacks validation evidence.".to_string(),
                accepted: false,
                required_revisions: vec!["Add scoped validation evidence.".to_string()],
                user_report: "Not ready for user review.".to_string(),
            },
        ));
        let revision = orchestrator
            .create_revision_task_from_professor_review_artifact(&run, &review_artifact)
            .unwrap()
            .expect("revision task");
        let revision_ref = format!("artifact:{}", revision.artifact.artifact_id());

        let provider = Arc::new(DraftProvider {
            response: serde_json::json!({
                "postdoc_plan": {
                    "implementation_summary": "Revise LabRun with scoped validation evidence.",
                    "slices": ["Add scoped validation evidence."],
                    "files_expected": ["src/lab/draft.rs"],
                    "validation_plan": ["cargo test -q draft_current_stage_artifact"],
                    "graduate_handoff": "Implement only the scoped validation evidence change."
                }
            })
            .to_string(),
            seen_prompt: Mutex::new(None),
        });

        let outcome = draft_current_stage_artifact(
            temp.path(),
            provider,
            "mock-lab-draft".to_string(),
            "write strict postdoc JSON",
        )
        .await
        .unwrap();

        match &outcome.created.artifact {
            StageArtifact::PostdocPlan(plan) => {
                assert!(plan.evidence_refs.iter().any(|item| item == &revision_ref));
                assert!(plan
                    .body
                    .graduate_handoff
                    .contains(review_artifact.artifact_id()));
            }
            other => panic!("expected PostdocPlan, got {:?}", other.artifact_type()),
        }
        assert!(outcome
            .created
            .gate
            .evidence_refs
            .iter()
            .any(|item| item == &revision_ref));
    }

    #[tokio::test]
    async fn provider_professor_review_enforces_postdoc_evidence_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let mut run = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.current_stage = "professor_review".to_string();
        run.internal_owner = LabRole::Professor;
        store.save_run(&run).unwrap();

        let mut integration = StageArtifact::PostdocIntegrationSummary(LabArtifactEnvelope::new(
            "artifact_postdocintegration_empty".to_string(),
            run.lab_run_id.clone(),
            LabArtifactType::PostdocIntegrationSummary,
            "Incomplete postdoc integration".to_string(),
            Utc::now(),
            PostdocIntegrationSummary {
                integration_summary: "No accepted graduate results yet.".to_string(),
                accepted_results: Vec::new(),
                validation_status: "needs_revision".to_string(),
                remaining_risks: vec!["validation missing".to_string()],
                handoff_to_professor: "Do not close out yet.".to_string(),
            },
        ));
        if let StageArtifact::PostdocIntegrationSummary(envelope) = &mut integration {
            envelope.evidence_refs = vec![
                "artifact:artifact_graduateresult_test".to_string(),
                "event:event_provider_professor_review_test".to_string(),
            ];
        }
        store.write_stage_artifact(&integration).unwrap();

        let provider = Arc::new(DraftProvider {
            response: serde_json::json!({
                "accepted": true,
                "review_summary": "Looks ready.",
                "strategic_assessment": "Ship it.",
                "required_revisions": [],
                "user_report": "Ready for user review."
            })
            .to_string(),
            seen_prompt: Mutex::new(None),
        });

        let outcome = draft_professor_review_with_provider(
            temp.path(),
            provider.clone(),
            "mock-lab-draft".to_string(),
            "make a strategic call",
        )
        .await
        .unwrap();

        assert!(matches!(
            outcome.created.artifact,
            StageArtifact::ProfessorReview(ref review)
                if !review.body.accepted
                    && review
                        .body
                        .required_revisions
                        .iter()
                        .any(|item| item.contains("no accepted graduate results"))
        ));
        match &outcome.created.artifact {
            StageArtifact::ProfessorReview(review) => {
                assert!(review
                    .evidence_refs
                    .iter()
                    .any(|item| item == "event:event_provider_professor_review_test"));
                assert!(review
                    .evidence_refs
                    .iter()
                    .any(|item| item == "artifact:artifact_graduateresult_test"));
            }
            other => panic!("expected ProfessorReview, got {:?}", other.artifact_type()),
        }
        assert_eq!(
            outcome.created.gate.validation_status.as_deref(),
            Some("needs_revision")
        );
        assert!(outcome
            .created
            .gate
            .evidence_refs
            .iter()
            .any(|item| item == "event:event_provider_professor_review_test"));
        assert!(outcome
            .created
            .gate
            .blockers
            .iter()
            .any(|item| item.contains("Postdoc integration is marked needs_revision")));
        let artifacts = store.list_stage_artifacts(&run.lab_run_id).unwrap();
        assert!(artifacts.iter().any(|artifact| matches!(
            artifact,
            StageArtifact::LabRevisionTask(revision)
                if revision.body.source_review_artifact_id == outcome.created.artifact.artifact_id()
        )));
        let prompt = provider.seen_prompt.lock().unwrap().clone().unwrap();
        assert!(prompt.contains("PostdocIntegrationSummary JSON"));
        assert!(prompt.contains("make a strategic call"));
    }

    #[tokio::test]
    async fn llm_draft_rejects_incomplete_structured_json() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let provider = Arc::new(DraftProvider {
            response: r#"{"professor_plan":{"strategic_direction":"too partial"}}"#.to_string(),
            seen_prompt: Mutex::new(None),
        });

        let err = draft_current_stage_artifact(
            temp.path(),
            provider,
            "mock-lab-draft".to_string(),
            "write strict JSON",
        )
        .await
        .unwrap_err()
        .to_string();

        assert!(err.contains("missing field"));
        let run = store.latest_run().unwrap().unwrap();
        assert!(store
            .list_stage_artifacts(&run.lab_run_id)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn llm_review_accepts_artifact_and_updates_gate() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path());
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let created = orchestrator
            .create_current_stage_artifact_for_latest("Professor direction")
            .unwrap();
        let provider = Arc::new(DraftProvider {
            response: r#"{"decision":"accept","note":"coherent enough for postdoc handoff"}"#
                .to_string(),
            seen_prompt: Mutex::new(None),
        });

        let outcome = review_stage_artifact_with_provider(
            temp.path(),
            provider,
            "mock-lab-review".to_string(),
            created.artifact.artifact_id(),
            "review strictly",
        )
        .await
        .unwrap();

        assert_eq!(outcome.decision, LabArtifactReviewDecision::Accept);
        assert_eq!(outcome.gate.validation_status.as_deref(), Some("accepted"));
        let reviewed = store
            .load_stage_artifact(&run.lab_run_id, created.artifact.artifact_id())
            .unwrap();
        assert_eq!(
            reviewed.status(),
            crate::lab::model::LabArtifactStatus::Accepted
        );
        let usage = store.list_cost_usage(&run.lab_run_id).unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].model, "mock-lab-review");
    }

    #[tokio::test]
    async fn llm_review_revise_blocks_gate() {
        let temp = tempfile::tempdir().unwrap();
        let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path());
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let created = orchestrator
            .create_current_stage_artifact_for_latest("Professor direction")
            .unwrap();
        let provider = Arc::new(DraftProvider {
            response: r#"{"decision":"revise","note":"missing concrete constraints"}"#.to_string(),
            seen_prompt: Mutex::new(None),
        });

        let outcome = review_stage_artifact_with_provider(
            temp.path(),
            provider,
            "mock-lab-review".to_string(),
            created.artifact.artifact_id(),
            "review strictly",
        )
        .await
        .unwrap();

        assert_eq!(outcome.decision, LabArtifactReviewDecision::Revise);
        assert_eq!(
            outcome.gate.validation_status.as_deref(),
            Some("needs_revision")
        );
        assert_eq!(
            outcome.gate.blockers,
            vec!["missing concrete constraints".to_string()]
        );
        let reviewed = store
            .load_stage_artifact(&run.lab_run_id, created.artifact.artifact_id())
            .unwrap();
        assert_eq!(
            reviewed.status(),
            crate::lab::model::LabArtifactStatus::NeedsRevision
        );
        let err = store
            .validate_artifact_gate(&run.lab_run_id, "professor_discussion")
            .unwrap_err()
            .to_string();
        assert!(err.contains("blocked") || err.contains("needs revision"));
    }

    #[tokio::test]
    async fn provider_stage_step_accepts_and_advances_non_graduate_stage() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let provider = Arc::new(SequenceProvider {
            responses: Mutex::new(VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Build LabRun",
                        "strategic_direction": "Keep runtime gates strict.",
                        "success_criteria": ["Advance after accepted plan"],
                        "constraints": ["No gate bypass"],
                        "risks": ["Overclaiming proof"],
                        "handoff_to_postdoc": "Create scoped implementation plan."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready for postdoc"}"#.to_string(),
            ])),
        });

        let outcome = run_provider_stage_step(
            temp.path(),
            provider,
            "mock-lab-provider-step".to_string(),
            "draft and review",
        )
        .await
        .unwrap();

        assert!(outcome.advanced);
        assert_eq!(outcome.from_stage, "professor_discussion");
        assert_eq!(outcome.to_stage, "postdoc_plan");
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.current_stage, "postdoc_plan");
        let usage = store.list_cost_usage(&saved.lab_run_id).unwrap();
        assert_eq!(usage.len(), 2);
    }

    #[tokio::test]
    async fn provider_stage_step_revision_keeps_stage_blocked() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let run = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let provider = Arc::new(SequenceProvider {
            responses: Mutex::new(VecDeque::from([
                "Professor direction without enough detail".to_string(),
                r#"{"decision":"revise","note":"needs concrete handoff"}"#.to_string(),
            ])),
        });

        let outcome = run_provider_stage_step(
            temp.path(),
            provider,
            "mock-lab-provider-step".to_string(),
            "draft and review",
        )
        .await
        .unwrap();

        assert!(!outcome.advanced);
        assert_eq!(outcome.from_stage, "professor_discussion");
        assert_eq!(outcome.to_stage, "professor_discussion");
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.current_stage, "professor_discussion");
        let err = store
            .validate_artifact_gate(&run.lab_run_id, "professor_discussion")
            .unwrap_err()
            .to_string();
        assert!(err.contains("blocked") || err.contains("needs revision"));
    }

    #[tokio::test]
    async fn provider_stage_run_stops_at_graduate_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let provider = Arc::new(SequenceProvider {
            responses: Mutex::new(VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Build LabRun",
                        "strategic_direction": "Keep runtime gates strict.",
                        "success_criteria": ["Reach postdoc planning"],
                        "constraints": ["No gate bypass"],
                        "risks": ["Overclaiming proof"],
                        "handoff_to_postdoc": "Create scoped implementation plan."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready for postdoc"}"#.to_string(),
                serde_json::json!({
                    "postdoc_plan": {
                        "implementation_summary": "Implement the bounded provider run slice.",
                        "slices": ["Add run helper", "Add shell command", "Add tests"],
                        "files_expected": ["src/lab/draft.rs", "src/shell/mod.rs"],
                        "validation_plan": ["cargo check -q --tests"],
                        "graduate_handoff": "Stop at graduate_work so strict task dispatch owns code execution."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready for graduate boundary"}"#.to_string(),
            ])),
        });

        let outcome = run_provider_stage_steps_until_boundary(
            temp.path(),
            provider,
            "mock-lab-provider-run".to_string(),
            5,
            "advance non-graduate stages",
        )
        .await
        .unwrap();

        assert_eq!(outcome.steps.len(), 2);
        assert_eq!(
            outcome.stop_reason,
            LabProviderStageRunStopReason::GraduateBoundary
        );
        assert_eq!(outcome.final_stage, "graduate_work");
        assert_eq!(outcome.steps[0].from_stage, "professor_discussion");
        assert_eq!(outcome.steps[1].from_stage, "postdoc_plan");
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.current_stage, "graduate_work");
        let usage = store.list_cost_usage(&saved.lab_run_id).unwrap();
        assert_eq!(usage.len(), 4);
    }

    #[tokio::test]
    async fn provider_stage_run_stops_on_revision_request() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let provider = Arc::new(SequenceProvider {
            responses: Mutex::new(VecDeque::from([
                "Professor direction without enough detail".to_string(),
                r#"{"decision":"revise","note":"needs clearer postdoc handoff"}"#.to_string(),
            ])),
        });

        let outcome = run_provider_stage_steps_until_boundary(
            temp.path(),
            provider,
            "mock-lab-provider-run".to_string(),
            5,
            "advance non-graduate stages",
        )
        .await
        .unwrap();

        assert_eq!(outcome.steps.len(), 1);
        assert_eq!(
            outcome.stop_reason,
            LabProviderStageRunStopReason::RevisionRequested
        );
        assert_eq!(outcome.final_stage, "professor_discussion");
        assert!(!outcome.steps[0].advanced);
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.current_stage, "professor_discussion");
    }

    #[tokio::test]
    async fn hybrid_run_hands_graduate_stage_to_strict_scheduler() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let provider = Arc::new(SequenceProvider {
            responses: Mutex::new(VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Build LabRun",
                        "strategic_direction": "Keep runtime gates strict.",
                        "success_criteria": ["Reach postdoc planning"],
                        "constraints": ["No gate bypass"],
                        "risks": ["Overclaiming proof"],
                        "handoff_to_postdoc": "Create scoped implementation plan."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready for postdoc"}"#.to_string(),
                serde_json::json!({
                    "postdoc_plan": {
                        "implementation_summary": "Implement the hybrid run slice.",
                        "slices": ["Provider planning", "Strict graduate boundary"],
                        "files_expected": [],
                        "validation_plan": ["cargo check -q --tests"],
                        "graduate_handoff": "Scheduler must not execute without scoped graduate work."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready for graduate scheduler"}"#.to_string(),
            ])),
        });

        let outcome = run_hybrid_lab_steps_until_boundary(
            temp.path(),
            provider,
            "mock-lab-hybrid-run".to_string(),
            5,
            "advance until graduate scheduler boundary",
            crate::tools::ToolContext::new(temp.path(), "lab-hybrid-test"),
        )
        .await
        .unwrap();

        assert_eq!(outcome.steps.len(), 3);
        assert_eq!(
            outcome.stop_reason,
            LabHybridRunStopReason::SchedulerStopped(
                crate::lab::orchestrator::LabSchedulerStepAction::Blocked
            )
        );
        assert!(matches!(
            outcome.steps.last(),
            Some(LabHybridRunStep::Scheduler(step))
                if matches!(step.action, crate::lab::orchestrator::LabSchedulerStepAction::Blocked)
        ));
        assert_eq!(outcome.final_stage, "graduate_work");
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.current_stage, "graduate_work");
    }

    #[tokio::test]
    async fn hybrid_run_uses_deterministic_review_bridges_to_user_report() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path());
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update deterministic review bridge.",
                vec!["src/lab/draft.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Implemented deterministic review bridge.",
                vec!["src/lab/draft.rs".to_string()],
                vec!["cargo check -q passed".to_string()],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let mut saved = store.load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = crate::lab::model::LabRole::Postdoc;
        store.save_run(&saved).unwrap();
        let provider = Arc::new(SequenceProvider {
            responses: Mutex::new(VecDeque::new()),
        });

        let outcome = run_hybrid_lab_steps_until_boundary(
            temp.path(),
            provider,
            "mock-lab-hybrid-run".to_string(),
            5,
            "",
            crate::tools::ToolContext::new(temp.path(), "lab-hybrid-test"),
        )
        .await
        .unwrap();

        assert_eq!(outcome.steps.len(), 2);
        assert_eq!(outcome.stop_reason, LabHybridRunStopReason::NeedsUser);
        assert_eq!(outcome.final_stage, "user_report");
        assert!(matches!(
            &outcome.steps[0],
            LabHybridRunStep::Deterministic(step)
                if step.from_stage == "postdoc_review"
                    && step.to_stage == "professor_review"
                    && step.gate_satisfied
        ));
        assert!(matches!(
            &outcome.steps[1],
            LabHybridRunStep::Deterministic(step)
                if step.from_stage == "professor_review"
                    && step.to_stage == "user_report"
                    && step.gate_satisfied
        ));
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.current_stage, "user_report");
        assert!(saved.needs_user);
    }

    #[tokio::test]
    async fn hybrid_run_syncs_completed_durable_graduate_and_reaches_user_report() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path());
        let mut run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        run.current_stage = "graduate_work".to_string();
        run.internal_owner = LabRole::Graduate;
        store.save_run(&run).unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Implement durable hybrid slice",
                "Update hybrid graduate proof.",
                vec!["src/lab/draft.rs".to_string()],
                vec!["test -f src/lab/draft.rs".to_string()],
            )
            .unwrap();
        let dispatch = crate::lab::delegation::build_graduate_task_dispatch(&task).unwrap();
        let record = store
            .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
            .unwrap();
        store
            .start_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();

        let worktree = temp.path().join("hybrid-graduate-worktree");
        std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&worktree)
            .output()
            .expect("git init worktree");
        std::fs::write(worktree.join("src/lab/draft.rs"), "hybrid graduate edit\n").unwrap();

        let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        session_store
            .create_session("lab-hybrid-test", "hybrid durable graduate", "model", None)
            .unwrap();
        let agent_task_id = crate::lab::delegation::graduate_agent_task_id(&task);
        let artifact_id = session_store
            .add_agent_artifact(
                "lab-hybrid-test",
                "agent_hybrid_sync",
                Some("lab-graduate"),
                "implementation",
                "completed",
                "hybrid durable graduate result",
                r#"{"graduate_result":{"summary":"Hybrid synced durable graduate result.","changed_files":["src/lab/draft.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
                &serde_json::json!({"completion_sink": "agent_manager"}),
            )
            .unwrap();
        session_store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: "lab-hybrid-test".to_string(),
                task_id: agent_task_id.clone(),
                agent_id: "agent_hybrid_sync".to_string(),
                profile: Some("lab-graduate".to_string()),
                role: "implementation".to_string(),
                status: "completed".to_string(),
                description: "hybrid durable graduate result".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: Some(artifact_id),
                cleanup_hooks: vec!["worktree_cleanup".to_string()],
                payload: serde_json::json!({
                    "completion_sink": "agent_manager",
                    "tools_used": ["file_write", "bash"],
                    "isolated_worktree": {
                        "path": worktree.to_string_lossy().to_string(),
                        "branch": "codex/hybrid-graduate-sync"
                    }
                }),
            })
            .unwrap();
        let provider = Arc::new(SequenceProvider {
            responses: Mutex::new(VecDeque::new()),
        });

        let outcome = run_hybrid_lab_steps_until_boundary(
            temp.path(),
            provider,
            "mock-lab-hybrid-run".to_string(),
            5,
            "",
            crate::tools::ToolContext::new(temp.path(), "lab-hybrid-test")
                .with_session_store(session_store),
        )
        .await
        .unwrap();

        assert_eq!(outcome.stop_reason, LabHybridRunStopReason::NeedsUser);
        assert_eq!(outcome.final_stage, "user_report");
        assert_eq!(outcome.steps.len(), 3);
        assert!(matches!(
            &outcome.steps[0],
            LabHybridRunStep::Scheduler(step)
                if step.action == LabSchedulerStepAction::TickAdvanced
                    && step.stage == "postdoc_review"
                    && step.message.contains("synced durable graduate result")
        ));
        assert!(matches!(
            &outcome.steps[1],
            LabHybridRunStep::Deterministic(step)
                if step.from_stage == "postdoc_review"
                    && step.to_stage == "professor_review"
                    && step.gate_satisfied
        ));
        assert!(matches!(
            &outcome.steps[2],
            LabHybridRunStep::Deterministic(step)
                if step.from_stage == "professor_review"
                    && step.to_stage == "user_report"
                    && step.gate_satisfied
        ));
        let saved_dispatch = store
            .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
            .unwrap();
        assert_eq!(
            saved_dispatch.status,
            crate::lab::model::GraduateDispatchStatus::Succeeded
        );
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.current_stage, "user_report");
        assert!(saved.needs_user);
    }

    #[tokio::test]
    async fn hybrid_run_plans_queues_graduate_syncs_and_reaches_user_report() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        crate::lab::orchestrator::LabOrchestrator::for_project(temp.path())
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let provider = Arc::new(SequenceProvider {
            responses: Mutex::new(VecDeque::from([
                serde_json::json!({
                    "professor_plan": {
                        "problem_statement": "Build LabRun",
                        "strategic_direction": "Keep runtime gates strict while delegating implementation.",
                        "success_criteria": ["Queue a scoped graduate task", "Reach user report"],
                        "constraints": ["No proof without runtime evidence"],
                        "risks": ["Graduate completion claims without file proof"],
                        "handoff_to_postdoc": "Create one scoped implementation slice."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready for postdoc"}"#.to_string(),
                serde_json::json!({
                    "postdoc_plan": {
                        "implementation_summary": "Implement the durable graduate sync slice.",
                        "slices": ["Durable graduate sync bridge"],
                        "files_expected": ["src/lab/draft.rs"],
                        "validation_plan": ["test -f src/lab/draft.rs"],
                        "graduate_handoff": "Use a durable lab-graduate task and provide runtime-verifiable file proof."
                    }
                })
                .to_string(),
                r#"{"decision":"accept","note":"ready for graduate"}"#.to_string(),
            ])),
        });

        let first = run_provider_stage_steps_until_boundary(
            temp.path(),
            provider,
            "mock-lab-hybrid-run".to_string(),
            5,
            "plan and queue graduate work",
        )
        .await
        .unwrap();

        assert_eq!(
            first.stop_reason,
            LabProviderStageRunStopReason::GraduateBoundary
        );
        assert_eq!(first.final_stage, "graduate_work");
        assert_eq!(first.steps.len(), 2);
        let run = store.latest_run().unwrap().unwrap();
        let tasks = store.list_graduate_tasks(&run.lab_run_id).unwrap();
        assert_eq!(tasks.len(), 1);
        let task = tasks[0].clone();
        assert_eq!(task.status, crate::lab::model::LabTaskStatus::Queued);
        assert_eq!(task.allowed_scope, vec!["src/lab/draft.rs".to_string()]);
        assert_eq!(
            task.required_validation,
            vec!["test -f src/lab/draft.rs".to_string()]
        );
        let dispatch = crate::lab::delegation::build_graduate_task_dispatch(&task).unwrap();
        store
            .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
            .unwrap();
        store
            .start_graduate_task(&run.lab_run_id, &task.task_id)
            .unwrap();

        let worktree = temp.path().join("hybrid-full-graduate-worktree");
        std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&worktree)
            .output()
            .expect("git init worktree");
        std::fs::write(
            worktree.join("src/lab/draft.rs"),
            "hybrid full graduate edit\n",
        )
        .unwrap();

        let session_store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        session_store
            .create_session(
                "lab-hybrid-test",
                "hybrid full durable graduate",
                "model",
                None,
            )
            .unwrap();
        let agent_task_id = crate::lab::delegation::graduate_agent_task_id(&task);
        let artifact_id = session_store
            .add_agent_artifact(
                "lab-hybrid-test",
                "agent_hybrid_full_sync",
                Some("lab-graduate"),
                "implementation",
                "completed",
                "hybrid full durable graduate result",
                r#"{"graduate_result":{"summary":"Hybrid full run synced durable graduate result.","changed_files":["src/lab/draft.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
                &serde_json::json!({"completion_sink": "agent_manager"}),
            )
            .unwrap();
        session_store
            .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
                session_id: "lab-hybrid-test".to_string(),
                task_id: agent_task_id.clone(),
                agent_id: "agent_hybrid_full_sync".to_string(),
                profile: Some("lab-graduate".to_string()),
                role: "implementation".to_string(),
                status: "completed".to_string(),
                description: "hybrid full durable graduate result".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: Some(artifact_id),
                cleanup_hooks: vec!["worktree_cleanup".to_string()],
                payload: serde_json::json!({
                    "completion_sink": "agent_manager",
                    "tools_used": ["file_write", "bash"],
                    "isolated_worktree": {
                        "path": worktree.to_string_lossy().to_string(),
                        "branch": "codex/hybrid-full-graduate-sync"
                    }
                }),
            })
            .unwrap();

        let second = run_hybrid_lab_steps_until_boundary(
            temp.path(),
            Arc::new(SequenceProvider {
                responses: Mutex::new(VecDeque::new()),
            }),
            "mock-lab-hybrid-run".to_string(),
            5,
            "continue from durable graduate completion",
            crate::tools::ToolContext::new(temp.path(), "lab-hybrid-test")
                .with_session_store(session_store),
        )
        .await
        .unwrap();

        assert_eq!(second.stop_reason, LabHybridRunStopReason::NeedsUser);
        assert_eq!(second.final_stage, "user_report");
        assert_eq!(second.steps.len(), 3);
        assert!(matches!(
            &second.steps[0],
            LabHybridRunStep::Scheduler(step)
                if step.action == LabSchedulerStepAction::TickAdvanced
                    && step.stage == "postdoc_review"
                    && step.message.contains("synced durable graduate result")
        ));
        assert!(matches!(
            &second.steps[1],
            LabHybridRunStep::Deterministic(step)
                if step.from_stage == "postdoc_review"
                    && step.to_stage == "professor_review"
                    && step.gate_satisfied
        ));
        assert!(matches!(
            &second.steps[2],
            LabHybridRunStep::Deterministic(step)
                if step.from_stage == "professor_review"
                    && step.to_stage == "user_report"
                    && step.gate_satisfied
        ));
        let saved = store.latest_run().unwrap().unwrap();
        assert_eq!(saved.current_stage, "user_report");
        assert!(saved.needs_user);
    }

    #[tokio::test]
    async fn hybrid_run_stops_when_deterministic_review_gate_blocks() {
        let temp = tempfile::tempdir().unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.create_proposal("Build LabRun", None).unwrap();
        let orchestrator = crate::lab::orchestrator::LabOrchestrator::for_project(temp.path());
        let run = orchestrator
            .approve_proposal(&proposal.proposal_id)
            .unwrap();
        let task = store
            .create_graduate_task(
                &run.lab_run_id,
                "Implement scoped slice",
                "Update deterministic review bridge.",
                vec!["src/lab/draft.rs".to_string()],
                vec!["cargo check -q".to_string()],
            )
            .unwrap();
        orchestrator
            .create_graduate_result_for_task_latest(
                &task.task_id,
                "Could not complete deterministic bridge.",
                vec!["src/lab/draft.rs".to_string()],
                vec!["cargo check -q failed".to_string()],
                vec!["validation still fails".to_string()],
                Vec::new(),
            )
            .unwrap();
        let mut saved = store.load_run(&run.lab_run_id).unwrap();
        saved.current_stage = "postdoc_review".to_string();
        saved.internal_owner = crate::lab::model::LabRole::Postdoc;
        store.save_run(&saved).unwrap();
        let provider = Arc::new(SequenceProvider {
            responses: Mutex::new(VecDeque::new()),
        });

        let outcome = run_hybrid_lab_steps_until_boundary(
            temp.path(),
            provider,
            "mock-lab-hybrid-run".to_string(),
            5,
            "",
            crate::tools::ToolContext::new(temp.path(), "lab-hybrid-test"),
        )
        .await
        .unwrap();

        assert_eq!(outcome.steps.len(), 1);
        assert_eq!(
            outcome.stop_reason,
            LabHybridRunStopReason::DeterministicGateBlocked
        );
        assert_eq!(outcome.final_stage, "postdoc_review");
        assert!(matches!(
            &outcome.steps[0],
            LabHybridRunStep::Deterministic(step)
                if step.from_stage == "postdoc_review" && !step.gate_satisfied
        ));
    }

    #[test]
    fn draft_sanitizer_rejects_empty_content() {
        assert!(sanitize_lab_artifact_draft("<think>hidden</think>").is_err());
    }

    #[test]
    fn review_parser_accepts_json_fence() {
        let parsed = parse_lab_artifact_review_decision(
            "```json\n{\"decision\":\"accept\",\"note\":\"ready\"}\n```",
        )
        .unwrap();

        assert_eq!(parsed.decision, LabArtifactReviewDecision::Accept);
        assert_eq!(parsed.note, "ready");
    }
}
