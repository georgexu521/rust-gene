//! Provider-backed and deterministic LabRun drafting boundary.
//!
//! Drafting creates proposal, artifact, meeting, review, and hybrid-cycle
//! outputs. Provider output is parsed and sanitized here, while persistence and
//! stage advancement remain owned by `LabStore` and `LabOrchestrator`.

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

mod runtime;

use runtime::*;

const LAB_ARTIFACT_DRAFT_SYSTEM_PROMPT: &str = r#"You are drafting a LabRun stage artifact.

Write only the artifact body for the current LabRun stage.
Prefer a strict JSON object that matches the required artifact type schema.
Do not claim code was changed or validation passed unless the supplied context says so.
Keep the output concise, concrete, and ready to persist as a project artifact.
For PostdocIntegrationSummary, explicitly consider whether repeated graduate failures, stalled stages, blocker reports, sponsor feedback, or poor progress-to-cost ratio suggest strategic drift that should be escalated to the professor. If escalation is needed, put concrete evidence and the requested professor decision in remaining_risks and handoff_to_professor.
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
Anchor guidance to the current stage, postdoc blocker/integration evidence, changed files, validation results, remaining risks, and the specific tradeoff under review.
Do not give broad, generic, or orthogonal advice. If the blocker is backend architecture, do not answer with unrelated frontend guidance.
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
Professor guidance must be specific to the current blocker, evidence, stage, and tradeoff. Do not give generic advice or switch to an unrelated domain.
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
/// Data model for lab artifact draft outcome in LabRun persistence or orchestration.
pub struct LabArtifactDraftOutcome {
    pub created: CreatedStageArtifact,
    pub draft: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// State set for lab artifact review decision in the LabRun workflow.
pub enum LabArtifactReviewDecision {
    Accept,
    Revise,
}

#[derive(Debug, Clone)]
/// Data model for lab artifact review outcome in LabRun persistence or orchestration.
pub struct LabArtifactReviewOutcome {
    pub artifact_id: String,
    pub decision: LabArtifactReviewDecision,
    pub note: String,
    pub gate: ArtifactGate,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
/// Data model for lab provider stage step outcome in LabRun persistence or orchestration.
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
/// State set for lab provider stage run stop reason in the LabRun workflow.
pub enum LabProviderStageRunStopReason {
    MaxSteps,
    GraduateBoundary,
    NeedsUser,
    NotActive,
    RevisionRequested,
    TerminalStage,
}

#[derive(Debug, Clone)]
/// Data model for lab provider stage run outcome in LabRun persistence or orchestration.
pub struct LabProviderStageRunOutcome {
    pub lab_run_id: String,
    pub steps: Vec<LabProviderStageStepOutcome>,
    pub final_stage: String,
    pub stop_reason: LabProviderStageRunStopReason,
}

#[derive(Debug, Clone)]
/// Data model for lab deterministic stage step outcome in LabRun persistence or orchestration.
pub struct LabDeterministicStageStepOutcome {
    pub lab_run_id: String,
    pub from_stage: String,
    pub to_stage: String,
    pub artifact_id: String,
    pub gate_satisfied: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
/// State set for lab hybrid run step in the LabRun workflow.
pub enum LabHybridRunStep {
    Provider(LabProviderStageStepOutcome),
    Scheduler(LabSchedulerStepResult),
    Deterministic(LabDeterministicStageStepOutcome),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// State set for lab hybrid run stop reason in the LabRun workflow.
pub enum LabHybridRunStopReason {
    MaxSteps,
    NeedsUser,
    NotActive,
    RevisionRequested,
    DeterministicGateBlocked,
    SchedulerStopped(LabSchedulerStepAction),
}

#[derive(Debug, Clone)]
/// Data model for lab hybrid run outcome in LabRun persistence or orchestration.
pub struct LabHybridRunOutcome {
    pub lab_run_id: String,
    pub steps: Vec<LabHybridRunStep>,
    pub final_stage: String,
    pub stop_reason: LabHybridRunStopReason,
}

#[derive(Debug, Clone)]
/// Data model for lab hybrid cycle run in LabRun persistence or orchestration.
pub struct LabHybridCycleRun {
    pub cycle_index: usize,
    pub cycle_count_at_start: u64,
    pub outcome: LabHybridRunOutcome,
    pub continued_to_next_cycle: bool,
    pub compression_artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// State set for lab hybrid cycle run stop reason in the LabRun workflow.
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
/// Data model for lab hybrid cycle run outcome in LabRun persistence or orchestration.
pub struct LabHybridCycleRunOutcome {
    pub lab_run_id: String,
    pub cycles: Vec<LabHybridCycleRun>,
    pub final_stage: String,
    pub final_cycle_count: u64,
    pub stop_reason: LabHybridCycleRunStopReason,
}

#[derive(Debug, Clone)]
/// Data model for lab sponsor message classification outcome in LabRun persistence or orchestration.
pub struct LabSponsorMessageClassificationOutcome {
    pub message: SponsorMessage,
    pub decision: String,
    pub note: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
/// Data model for lab proposal draft outcome in LabRun persistence or orchestration.
pub struct LabProposalDraftOutcome {
    pub proposal: LabProposal,
    pub draft: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
/// Data model for lab meeting draft outcome in LabRun persistence or orchestration.
pub struct LabMeetingDraftOutcome {
    pub created: CreatedStageArtifact,
    pub draft: String,
    pub usage: Option<Usage>,
}

/// Async entry point for draft lab proposal with provider.
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

/// Async entry point for draft current stage artifact.
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

/// Async entry point for classify sponsor message with provider.
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

/// Async entry point for draft lab meeting with provider.
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

/// Async entry point for review stage artifact with provider.
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

/// Async entry point for draft professor review with provider.
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
        .next_back()
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

/// Async entry point for run provider stage step.
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

/// Async entry point for run provider stage steps until boundary.
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

/// Async entry point for run hybrid lab steps until boundary.
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

/// Async entry point for run hybrid lab cycles until boundary.
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

#[cfg(test)]
mod tests;
