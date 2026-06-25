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
        .record_evidence_ref(crate::lab::store::LabEvidenceRefInput {
            lab_run_id: &run.lab_run_id,
            kind: crate::lab::model::LabEvidenceKind::File,
            role: LabRole::Postdoc,
            reference: "target/proof.txt",
            summary: "validation proof",
            artifact_id: None,
            cycle_id: Some("0"),
        })
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
                    "files_expected": ["src/lab/draft.rs"],
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
            crate::lab::orchestrator::LabSchedulerStepAction::GraduateDispatched
        )
    );
    assert!(matches!(
        outcome.steps.last(),
        Some(LabHybridRunStep::Scheduler(step))
            if matches!(
                step.action,
                crate::lab::orchestrator::LabSchedulerStepAction::GraduateDispatched
            )
    ));
    assert_eq!(outcome.final_stage, "graduate_work");
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.current_stage, "graduate_work");
}

#[tokio::test]
async fn hybrid_run_stops_at_deterministic_professor_review_gate() {
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
    assert_eq!(
        outcome.stop_reason,
        LabHybridRunStopReason::DeterministicGateBlocked
    );
    assert_eq!(outcome.final_stage, "professor_review");
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
                && step.to_stage == "professor_review"
                && !step.gate_satisfied
    ));
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.current_stage, "professor_review");
    assert!(!saved.needs_user);
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

    assert_eq!(
        outcome.stop_reason,
        LabHybridRunStopReason::DeterministicGateBlocked
    );
    assert_eq!(outcome.final_stage, "professor_review");
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
                && step.to_stage == "professor_review"
                && !step.gate_satisfied
    ));
    let saved_dispatch = store
        .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
        .unwrap();
    assert_eq!(
        saved_dispatch.status,
        crate::lab::model::GraduateDispatchStatus::Succeeded
    );
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.current_stage, "professor_review");
    assert!(!saved.needs_user);
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

    assert_eq!(
        second.stop_reason,
        LabHybridRunStopReason::DeterministicGateBlocked
    );
    assert_eq!(second.final_stage, "professor_review");
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
                && step.to_stage == "professor_review"
                && !step.gate_satisfied
    ));
    let saved = store.latest_run().unwrap().unwrap();
    assert_eq!(saved.current_stage, "professor_review");
    assert!(!saved.needs_user);
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
