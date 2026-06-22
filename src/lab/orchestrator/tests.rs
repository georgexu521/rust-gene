use super::*;
use crate::lab::model::LAB_SCHEMA_VERSION;
use std::sync::Arc;

fn init_git_dir(path: &Path) {
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "lab@example.test"])
        .current_dir(path)
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "Lab Test"])
        .current_dir(path)
        .output()
        .expect("git config name");
    std::fs::write(path.join("README.md"), "base\n").expect("write base");
    std::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "base"])
        .current_dir(path)
        .output()
        .expect("git commit");
}

fn lab_context_with_agent_worktree(
    project_root: &Path,
    session_id: &str,
    agent_id: &str,
    worktree_path: &Path,
) -> ToolContext {
    lab_context_with_agent_worktree_task_id(
        project_root,
        session_id,
        agent_id,
        agent_id,
        worktree_path,
    )
}

fn lab_context_with_agent_worktree_task_id(
    project_root: &Path,
    session_id: &str,
    task_id: &str,
    agent_id: &str,
    worktree_path: &Path,
) -> ToolContext {
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session(session_id, "lab runtime test", "test-model", None)
        .unwrap();
    store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: session_id.to_string(),
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            profile: Some("lab-graduate".to_string()),
            role: "implementation".to_string(),
            status: "completed".to_string(),
            description: "graduate runtime test".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: None,
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: serde_json::json!({
                "isolated_worktree": {
                    "path": worktree_path.to_string_lossy().to_string(),
                    "branch": "codex/agent-test"
                }
            }),
        })
        .unwrap();
    ToolContext::new(project_root, session_id).with_session_store(store)
}

fn drive_to_user_report_with_explicit_artifacts(orchestrator: &LabOrchestrator) {
    for stage in [
        "professor_discussion",
        "postdoc_plan",
        "graduate_work",
        "postdoc_review",
        "professor_review",
    ] {
        let created = orchestrator
            .create_current_stage_artifact_for_latest(&format!("explicit artifact for {stage}"))
            .unwrap();
        assert!(
            created.gate.is_satisfied(),
            "gate should be satisfied for explicit artifact at {stage}"
        );
        let advanced = orchestrator.advance_latest().unwrap();
        if stage == "professor_review" {
            assert_eq!(advanced.current_stage, "user_report");
            assert!(advanced.needs_user);
        }
    }
}

#[test]
fn approve_creates_initial_professor_plan_gate() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();

    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();

    let gate_path = orchestrator
        .store()
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("artifact_gates")
        .join("professor_discussion.json");
    assert!(gate_path.exists());
    let err = orchestrator.advance_latest().unwrap_err().to_string();
    assert!(err.contains("artifact_id"));
}

#[test]
fn satisfied_gate_allows_stage_advance_and_creates_next_gate() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();

    orchestrator
        .create_current_stage_artifact_for_latest("Professor direction")
        .unwrap();
    let advanced = orchestrator.advance_latest().unwrap();

    assert_eq!(advanced.lab_run_id, run.lab_run_id);
    assert_eq!(advanced.current_stage, "postdoc_plan");
    assert_eq!(advanced.internal_owner, LabRole::Postdoc);
    assert!(orchestrator
        .store()
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("artifact_gates")
        .join("postdoc_plan.json")
        .exists());
}

#[test]
fn current_stage_artifact_is_persisted_and_satisfies_gate() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();

    let created = orchestrator
        .create_current_stage_artifact_for_latest("Professor direction")
        .unwrap();
    assert_eq!(
        created.artifact.artifact_type(),
        LabArtifactType::ProfessorPlan
    );
    assert!(created.path.exists());
    assert_eq!(created.gate.stage, "professor_discussion");
    assert!(created.report_path.exists());

    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert!(saved
        .artifact_ids
        .iter()
        .any(|id| id == created.artifact.artifact_id()));
    assert_eq!(
        saved.resume_cursor.active_artifact_id.as_deref(),
        Some(created.artifact.artifact_id())
    );

    let advanced = orchestrator.advance_latest().unwrap();
    assert_eq!(advanced.current_stage, "postdoc_plan");
}

#[test]
fn accepting_postdoc_plan_queues_graduate_tasks_once() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let professor = orchestrator
        .create_current_stage_artifact_for_latest("Professor direction")
        .unwrap();
    orchestrator
        .accept_artifact_latest(professor.artifact.artifact_id(), "accepted")
        .unwrap();
    let run = orchestrator.advance_latest().unwrap();
    assert_eq!(run.current_stage, "postdoc_plan");

    let postdoc = StageArtifact::PostdocPlan(LabArtifactEnvelope::new(
        "artifact_postdoc_plan_queue_test".to_string(),
        run.lab_run_id.clone(),
        LabArtifactType::PostdocPlan,
        "Postdoc implementation plan".to_string(),
        Utc::now(),
        PostdocPlan {
            implementation_summary: "Implement two concrete slices.".to_string(),
            slices: vec![
                "Add runtime queue bridge".to_string(),
                "Verify scheduler handoff".to_string(),
            ],
            files_expected: vec!["src/lab/orchestrator.rs".to_string()],
            validation_plan: vec!["cargo check -q --tests".to_string()],
            graduate_handoff: "Implement only the scoped files and report proof.".to_string(),
        },
    ));
    orchestrator.store().write_stage_artifact(&postdoc).unwrap();
    orchestrator
        .store()
        .write_stage_artifact_report(&postdoc)
        .unwrap();

    orchestrator
        .accept_artifact_latest(postdoc.artifact_id(), "accepted")
        .unwrap();
    let tasks = orchestrator
        .store()
        .list_graduate_tasks(&run.lab_run_id)
        .unwrap();
    assert_eq!(tasks.len(), 2);
    assert!(tasks
        .iter()
        .all(|task| matches!(task.status, LabTaskStatus::Queued)));
    assert!(tasks.iter().all(|task| task
        .instructions
        .contains("postdoc_plan_artifact_id=artifact_postdoc_plan_queue_test")));
    assert_eq!(
        tasks[0].allowed_scope,
        vec!["src/lab/orchestrator.rs".to_string()]
    );

    orchestrator
        .accept_artifact_latest(postdoc.artifact_id(), "accepted again")
        .unwrap();
    let deduped = orchestrator
        .store()
        .list_graduate_tasks(&run.lab_run_id)
        .unwrap();
    assert_eq!(deduped.len(), 2);
}

#[test]
fn accepting_postdoc_plan_without_scope_blocks_generated_task() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let professor = orchestrator
        .create_current_stage_artifact_for_latest("Professor direction")
        .unwrap();
    orchestrator
        .accept_artifact_latest(professor.artifact.artifact_id(), "accepted")
        .unwrap();
    let run = orchestrator.advance_latest().unwrap();

    let postdoc = orchestrator
        .create_current_stage_artifact_for_latest("Postdoc plan missing scope")
        .unwrap();
    orchestrator
        .accept_artifact_latest(postdoc.artifact.artifact_id(), "accepted")
        .unwrap();

    let tasks = orchestrator
        .store()
        .list_graduate_tasks(&run.lab_run_id)
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, LabTaskStatus::Blocked);
    assert!(tasks[0]
        .blocker
        .as_deref()
        .unwrap_or("")
        .contains("missing files_expected"));
}

#[test]
fn cycle_summary_persists_report_and_advances_cycle_count() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();

    let created = orchestrator
        .create_cycle_summary_for_latest("Finished initial planning slice")
        .unwrap();

    assert_eq!(
        created.artifact.artifact_type(),
        LabArtifactType::CycleSummary
    );
    assert!(created.path.exists());
    assert!(created.report_path.exists());
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.cycle_count, 1);
    assert!(saved
        .artifact_ids
        .iter()
        .any(|id| id == created.artifact.artifact_id()));
}

#[test]
fn compression_summary_is_written_when_decision_requires_it() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let mut run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    run.cost_policy.professor_context_budget = 10;
    orchestrator.store().save_run(&run).unwrap();

    let created = orchestrator
        .create_compression_summary_for_latest(LabRole::Professor)
        .unwrap()
        .expect("small budget should require compression");

    assert_eq!(
        created.artifact.artifact_type(),
        LabArtifactType::CompressionSummary
    );
    assert!(created.path.exists());
    assert!(created.report_path.exists());
    let decisions = orchestrator
        .store()
        .list_compression_decisions(&run.lab_run_id)
        .unwrap();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].action, LabCompressionAction::Required);
}

#[test]
fn meeting_summary_writes_read_only_artifact_and_tracks_meeting_id() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let evidence = orchestrator
        .store()
        .record_evidence_ref(crate::lab::store::LabEvidenceRefInput {
            lab_run_id: &run.lab_run_id,
            kind: crate::lab::model::LabEvidenceKind::File,
            role: LabRole::Postdoc,
            reference: "target/meeting-proof.txt",
            summary: "meeting proof",
            artifact_id: None,
            cycle_id: Some("0"),
        })
        .unwrap();

    let created = orchestrator
        .create_meeting_summary_for_latest(Some("review blocked work"))
        .unwrap();

    assert_eq!(
        created.artifact.artifact_type(),
        LabArtifactType::LabMeetingSummary
    );
    assert!(created.path.exists());
    assert!(created.report_path.exists());
    assert!(created
        .gate
        .evidence_refs
        .iter()
        .any(|item| item == &evidence.evidence_id));
    match &created.artifact {
        StageArtifact::LabMeetingSummary(envelope) => {
            assert!(envelope
                .evidence_refs
                .iter()
                .any(|item| item == &evidence.evidence_id));
        }
        other => panic!(
            "expected LabMeetingSummary, got {:?}",
            other.artifact_type()
        ),
    }
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.meeting_ids.len(), 1);
    assert!(saved
        .artifact_ids
        .iter()
        .any(|id| id == created.artifact.artifact_id()));
}

#[test]
fn meeting_recommendation_uses_blocked_task_signal() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .store()
        .block_graduate_task(&run.lab_run_id, &task.task_id, "validation failed twice")
        .unwrap();

    let recommendation = orchestrator.meeting_recommendation_for_latest().unwrap();

    assert!(recommendation.recommended);
    assert!(recommendation.topic.contains("blocked graduate task"));
    assert!(recommendation
        .signals
        .iter()
        .any(|signal| signal.starts_with("blocked_task:")));
}

#[test]
fn meeting_request_persists_runtime_escalation_signal_artifact() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .store()
        .block_graduate_task(&run.lab_run_id, &task.task_id, "validation failed twice")
        .unwrap();

    let recommendation = orchestrator.meeting_recommendation_for_latest().unwrap();
    let created = orchestrator
        .create_meeting_request_for_latest(&recommendation)
        .unwrap();

    assert_eq!(
        created.artifact.artifact_type(),
        LabArtifactType::LabMeetingRequest
    );
    assert_eq!(created.gate.stage, "lab_meeting_request");
    assert_eq!(
        created.gate.next_action.as_deref(),
        Some("open_read_only_lab_meeting")
    );
    assert!(created.path.exists());
    assert!(created.report_path.exists());
    let StageArtifact::LabMeetingRequest(request) = &created.artifact else {
        panic!("expected LabMeetingRequest artifact");
    };
    assert_eq!(request.owner, LabRole::Runtime);
    assert_eq!(
        request.validation_status.as_deref(),
        Some("runtime_escalation_signal")
    );
    assert_eq!(request.body.reason, "runtime_escalation_signals_present");
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert!(saved
        .artifact_ids
        .iter()
        .any(|id| id == created.artifact.artifact_id()));
}

#[test]
fn blocker_report_writes_postdoc_handoff_artifact() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .store()
        .block_graduate_task(&run.lab_run_id, &task.task_id, "scope is unclear")
        .unwrap();

    let created = orchestrator
        .create_blocker_report_for_latest(Some("Need professor decision"))
        .unwrap();

    assert_eq!(
        created.artifact.artifact_type(),
        LabArtifactType::LabBlockerReport
    );
    assert!(created.path.exists());
    assert!(created.report_path.exists());
    let task_ref = format!("task:{}", task.task_id);
    assert!(created
        .gate
        .evidence_refs
        .iter()
        .any(|item| item == &task_ref));
    match &created.artifact {
        StageArtifact::LabBlockerReport(report) => {
            assert!(report.evidence_refs.iter().any(|item| item == &task_ref));
        }
        other => panic!("expected LabBlockerReport, got {:?}", other.artifact_type()),
    }
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert!(saved
        .artifact_ids
        .iter()
        .any(|id| id == created.artifact.artifact_id()));
    assert!(saved
        .blocked_reason
        .as_deref()
        .unwrap_or_default()
        .starts_with("blocker_"));
}

#[test]
fn blocker_escalation_moves_run_to_professor_review_gate() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .store()
        .block_graduate_task(&run.lab_run_id, &task.task_id, "scope is unclear")
        .unwrap();
    orchestrator
        .create_blocker_report_for_latest(Some("Need professor decision"))
        .unwrap();

    let escalated = orchestrator
        .escalate_latest_blocker_to_professor_review()
        .unwrap();

    assert_eq!(escalated.current_stage, "professor_review");
    assert_eq!(escalated.internal_owner, LabRole::Professor);
    let gate = orchestrator.required_gate_for_latest().unwrap();
    assert_eq!(gate.stage, "professor_review");
    assert_eq!(gate.required_artifact_type, "ProfessorReview");
}

#[test]
fn graduate_result_artifact_completes_task_and_preserves_not_verified_status() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab task result binding.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();

    let created = orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Implemented result binding.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            vec!["labevidence_001".to_string()],
        )
        .unwrap();

    assert_eq!(
        created.artifact.artifact_type(),
        LabArtifactType::GraduateResult
    );
    assert_eq!(
        created.artifact.validation_status(),
        Some("subagent_report_not_parent_verified")
    );
    assert!(created.path.exists());
    assert!(created.report_path.exists());
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(
        saved_task.result_artifact_id.as_deref(),
        Some(created.artifact.artifact_id())
    );
    let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert!(saved_run.open_task_ids.is_empty());
}

#[test]
fn postdoc_integration_summary_accepts_unblocked_graduate_results() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab integration bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    let result = orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Implemented integration bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();

    let created = orchestrator
        .create_postdoc_integration_summary_for_latest(Some("Postdoc verified result shape."))
        .unwrap();

    assert_eq!(
        created.artifact.artifact_type(),
        LabArtifactType::PostdocIntegrationSummary
    );
    assert!(created.gate.is_satisfied());
    assert_eq!(
        created.artifact.validation_status(),
        Some("postdoc_integrated_pending_professor_review")
    );
    match created.artifact {
        StageArtifact::PostdocIntegrationSummary(envelope) => {
            assert!(envelope
                .body
                .accepted_results
                .iter()
                .any(|item| item.contains(result.artifact.artifact_id())));
            assert!(envelope
                .body
                .remaining_risks
                .iter()
                .any(|risk| risk.contains("pending parent verification")));
        }
        other => panic!(
            "expected integration summary, got {:?}",
            other.artifact_type()
        ),
    }
}

#[test]
fn postdoc_integration_summary_includes_graduate_worktree_runtime_proof() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab integration bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Implemented integration bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
    let durable_agent_ref = format!("lab-graduate-{}", task.task_id);
    orchestrator
        .store()
        .record_run_event(
            &run.lab_run_id,
            "lab_graduate_worktree_action",
            serde_json::json!({
                "task_id": task.task_id,
                "agent_ref_kind": "task_id",
                "agent_ref": durable_agent_ref,
                "action": "agent_merge",
                "success": true,
                "result_data": {
                    "merge_kind": "tracked_diff",
                    "dirty": false,
                    "path": temp.path().join(".priority-agent/worktrees/lab-graduate").display().to_string(),
                },
            }),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();

    let created = orchestrator
        .create_postdoc_integration_summary_for_latest(Some(
            "Postdoc verified runtime worktree proof.",
        ))
        .unwrap();

    let report = std::fs::read_to_string(&created.report_path).unwrap();
    match created.artifact {
        StageArtifact::PostdocIntegrationSummary(envelope) => {
            assert!(envelope
                .body
                .accepted_results
                .iter()
                .any(|item| item.contains("runtime worktree proof: agent_merge")
                    && item.contains("merge_kind=tracked_diff")));
            assert!(envelope
                .evidence_refs
                .iter()
                .any(|item| item.starts_with("event:event_")));
        }
        other => panic!(
            "expected integration summary, got {:?}",
            other.artifact_type()
        ),
    }
    assert!(report.contains("runtime worktree proof: agent_merge"));
    assert!(report.contains("merge_kind=tracked_diff"));
    assert!(report.contains("event:event_"));
}

#[test]
fn postdoc_integration_summary_includes_workspace_snapshot_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab integration bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Implemented integration bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
    orchestrator
        .store()
        .record_run_event(
            &run.lab_run_id,
            "lab_graduate_workspace_snapshot",
            serde_json::json!({
                "task_id": task.task_id,
                "dispatch_id": "dispatch_before_snapshot",
                "phase": "before",
                "dirty_path_count": 2,
                "dirty_paths": [
                    "preexisting-user-change.txt",
                    "src/lib.rs"
                ],
                "changed_path_count": 0,
                "changed_paths": [],
            }),
        )
        .unwrap();
    orchestrator
        .store()
        .record_run_event(
            &run.lab_run_id,
            "lab_graduate_workspace_snapshot",
            serde_json::json!({
                "task_id": task.task_id,
                "dispatch_id": "dispatch_after_snapshot",
                "phase": "after",
                "dirty_path_count": 3,
                "dirty_paths": [
                    "preexisting-user-change.txt",
                    "src/lib.rs",
                    "src/lab/model.rs"
                ],
                "changed_path_count": 1,
                "changed_paths": [
                    "src/lab/model.rs"
                ],
            }),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();

    let created = orchestrator
        .create_postdoc_integration_summary_for_latest(Some("Postdoc checked workspace snapshots."))
        .unwrap();

    assert!(created.gate.is_satisfied());
    assert!(created
        .gate
        .evidence_refs
        .iter()
        .any(|item| item.starts_with("event:event_")));
    let report = std::fs::read_to_string(&created.report_path).unwrap();
    match created.artifact {
        StageArtifact::PostdocIntegrationSummary(envelope) => {
            assert!(envelope.body.accepted_results.iter().any(|item| {
                item.contains("runtime workspace delta: after task=")
                    && item.contains("changed=[src/lab/model.rs]")
            }));
            assert!(envelope.body.remaining_risks.iter().any(|risk| {
                risk.contains("pre-existing workspace changes: before task=")
                    && risk.contains("dirty=[preexisting-user-change.txt,src/lib.rs]")
            }));
            assert!(envelope
                .evidence_refs
                .iter()
                .any(|item| item.starts_with("event:event_")));
        }
        other => panic!(
            "expected integration summary, got {:?}",
            other.artifact_type()
        ),
    }
    assert!(report.contains("runtime workspace delta: after task="));
    assert!(report.contains("changed=[src/lab/model.rs]"));
    assert!(report.contains("pre-existing workspace changes: before task="));
    assert!(report.contains("dirty=[preexisting-user-change.txt,src/lib.rs]"));
}

#[test]
fn postdoc_integration_summary_blocks_on_graduate_result_blockers() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab integration bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Could not complete integration.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q failed".to_string()],
            vec!["validation still fails".to_string()],
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();

    let created = orchestrator
        .create_postdoc_integration_summary_for_latest(None)
        .unwrap();

    assert!(!created.gate.is_satisfied());
    assert_eq!(
        created.gate.validation_status.as_deref(),
        Some("needs_revision")
    );
    assert!(created
        .gate
        .blockers
        .iter()
        .any(|blocker| blocker.contains("validation still fails")));
}

#[test]
fn professor_review_blocks_deterministic_closeout() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab professor review bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Implemented professor review bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();
    orchestrator
        .store()
        .record_run_event(
            &run.lab_run_id,
            "lab_graduate_workspace_snapshot",
            serde_json::json!({
                "task_id": task.task_id,
                "dispatch_id": "dispatch_professor_review_snapshot",
                "phase": "after",
                "dirty_path_count": 1,
                "dirty_paths": ["src/lab/orchestrator.rs"],
                "changed_path_count": 1,
                "changed_paths": ["src/lab/orchestrator.rs"],
            }),
        )
        .unwrap();
    orchestrator
        .create_postdoc_integration_summary_for_latest(Some("Postdoc integrated result."))
        .unwrap();
    let advanced = orchestrator.advance_latest().unwrap();
    assert_eq!(advanced.current_stage, "professor_review");

    let review = orchestrator
        .create_professor_review_for_latest(Some("Professor accepts the evidence."))
        .unwrap();

    assert!(!review.gate.is_satisfied());
    assert_eq!(
        review.gate.validation_status.as_deref(),
        Some("needs_revision")
    );
    assert!(review
        .gate
        .evidence_refs
        .iter()
        .any(|item| item.starts_with("event:event_")));
    let report = std::fs::read_to_string(&review.report_path).unwrap();
    assert!(report.contains("event:event_"));
    match review.artifact {
        StageArtifact::ProfessorReview(envelope) => {
            assert!(!envelope.body.accepted);
            assert!(envelope.body.required_revisions.iter().any(|revision| {
                revision.contains("provider or explicit professor review is required")
            }));
            assert!(envelope.body.user_report.contains("not ready for closeout"));
            assert!(envelope
                .evidence_refs
                .iter()
                .any(|item| item.starts_with("event:event_")));
        }
        other => panic!("expected professor review, got {:?}", other.artifact_type()),
    }
    let err = orchestrator.advance_latest().unwrap_err().to_string();
    assert!(err.contains("blocked"));
}

#[test]
fn professor_review_blocks_needs_revision_integration() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab professor review bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Blocked professor review bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q failed".to_string()],
            vec!["validation still fails".to_string()],
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();
    orchestrator
        .create_postdoc_integration_summary_for_latest(None)
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "professor_review".to_string();
    saved.internal_owner = LabRole::Professor;
    orchestrator.store().save_run(&saved).unwrap();

    let review = orchestrator
        .create_professor_review_for_latest(None)
        .unwrap();

    assert!(!review.gate.is_satisfied());
    assert_eq!(
        review.gate.validation_status.as_deref(),
        Some("needs_revision")
    );
    assert!(review
        .gate
        .blockers
        .iter()
        .any(|blocker| blocker.contains("validation still fails")));
}

#[test]
fn postdoc_plan_consumes_pending_professor_revision_task() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update lab professor review bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Blocked professor review bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q failed".to_string()],
            vec!["validation still fails".to_string()],
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();
    orchestrator
        .create_postdoc_integration_summary_for_latest(None)
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "professor_review".to_string();
    saved.internal_owner = LabRole::Professor;
    orchestrator.store().save_run(&saved).unwrap();

    let review = orchestrator
        .create_professor_review_for_latest(None)
        .unwrap();
    let inherited_review_ref = match &review.artifact {
        StageArtifact::ProfessorReview(envelope) => envelope
            .evidence_refs
            .iter()
            .find(|item| item.starts_with("artifact:artifact_postdocintegrationsummary_"))
            .cloned()
            .expect("professor review inherited postdoc evidence"),
        other => panic!("expected ProfessorReview, got {:?}", other.artifact_type()),
    };
    let revision_artifact_id = orchestrator
        .store()
        .list_stage_artifacts(&run.lab_run_id)
        .unwrap()
        .into_iter()
        .find_map(|artifact| match artifact {
            StageArtifact::LabRevisionTask(revision) => Some(revision.artifact_id),
            _ => None,
        })
        .expect("revision task artifact");
    let revision_artifact = orchestrator
        .store()
        .load_stage_artifact(&run.lab_run_id, &revision_artifact_id)
        .unwrap();
    match &revision_artifact {
        StageArtifact::LabRevisionTask(revision) => {
            assert!(revision
                .evidence_refs
                .iter()
                .any(|item| item == &format!("artifact:{}", review.artifact.artifact_id())));
            assert!(revision
                .evidence_refs
                .iter()
                .any(|item| item == &inherited_review_ref));
        }
        other => panic!("expected LabRevisionTask, got {:?}", other.artifact_type()),
    }
    let revision_gate = orchestrator
        .store()
        .load_artifact_gate(&run.lab_run_id, "postdoc_revision")
        .unwrap();
    assert!(revision_gate
        .evidence_refs
        .iter()
        .any(|item| item == &inherited_review_ref));

    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_plan".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();
    let postdoc_plan = orchestrator
        .create_current_stage_artifact_for_latest("Revise according to professor feedback.")
        .unwrap();

    match postdoc_plan.artifact {
        StageArtifact::PostdocPlan(plan) => {
            assert!(plan
                .evidence_refs
                .iter()
                .any(|item| item == &format!("artifact:{revision_artifact_id}")));
            assert!(plan
                .body
                .graduate_handoff
                .contains(review.artifact.artifact_id()));
            assert!(plan.body.slices.iter().any(|slice| {
                slice.contains("validation still fails")
                    || slice.contains("Postdoc integration is marked needs_revision")
            }));
        }
        other => panic!("expected PostdocPlan, got {:?}", other.artifact_type()),
    }

    let consumed = orchestrator
        .store()
        .load_stage_artifact(&run.lab_run_id, &revision_artifact_id)
        .unwrap();
    assert_eq!(consumed.validation_status(), Some("consumed"));
}

#[test]
fn graduate_result_rejects_changed_files_outside_allowed_scope() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();

    let err = orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Changed unrelated file.",
            vec!["src/main.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err()
        .to_string();

    assert!(err.contains("outside allowed_scope"));
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, crate::lab::model::LabTaskStatus::Queued);
}

#[test]
fn workspace_change_delta_ignores_preexisting_dirty_files() {
    let before = BTreeMap::from([
        ("src/lib.rs".to_string(), "file:1:aaa".to_string()),
        ("src/main.rs".to_string(), "file:1:bbb".to_string()),
    ]);
    let after = BTreeMap::from([
        (
            ".claude/worktrees/agent-live-proof/".to_string(),
            "non_file:64".to_string(),
        ),
        (
            ".priority-agent/lab/events.jsonl".to_string(),
            "file:1:eee".to_string(),
        ),
        ("src/lib.rs".to_string(), "file:1:aaa".to_string()),
        ("src/lab/model.rs".to_string(), "file:1:ddd".to_string()),
        ("src/main.rs".to_string(), "file:2:ccc".to_string()),
    ]);

    let changed = changed_paths_between(&before, &after);

    assert_eq!(
        changed,
        vec!["src/lab/model.rs".to_string(), "src/main.rs".to_string()]
    );
    assert!(validate_changed_files_within_scope(
        &["src/lab".to_string(), "src/main.rs".to_string()],
        &changed,
    )
    .is_ok());
    assert!(validate_changed_files_within_scope(&["src/lab".to_string()], &changed).is_err());
}

#[test]
fn graduate_runtime_verification_rejects_missing_actual_changes() {
    let project = tempfile::tempdir().unwrap();
    init_git_dir(project.path());
    let context = lab_context_with_agent_worktree(
        project.path(),
        "lab-provider-command",
        "agent_1",
        project.path(),
    );
    let task = GraduateTask {
        schema_version: LAB_SCHEMA_VERSION,
        task_id: "gradtask_test".to_string(),
        lab_run_id: "labrun_test".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: LabRole::Postdoc,
        assigned_role: LabRole::Graduate,
        status: LabTaskStatus::InProgress,
        title: "Write proof".to_string(),
        instructions: "Create proof.txt".to_string(),
        allowed_scope: vec!["proof.txt".to_string()],
        required_validation: vec!["test -f proof.txt".to_string()],
        evidence_ids: Vec::new(),
        result_artifact_id: None,
        blocker: None,
        cycle_id: Some("0".to_string()),
    };

    let err = runtime_verify_graduate_task_result(
        &task,
        &context,
        Some("agent_1"),
        "lab-graduate-gradtask_test",
        &[],
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("no actual file changes"));
}

#[test]
fn graduate_runtime_verification_checks_worktree_scope_and_validation() {
    let project = tempfile::tempdir().unwrap();
    init_git_dir(project.path());
    let worktree = tempfile::tempdir().unwrap();
    init_git_dir(worktree.path());
    std::fs::write(worktree.path().join("proof.txt"), "verified\n").unwrap();
    let context = lab_context_with_agent_worktree(
        project.path(),
        "lab-provider-command",
        "agent_1",
        worktree.path(),
    );
    let task = GraduateTask {
        schema_version: LAB_SCHEMA_VERSION,
        task_id: "gradtask_test".to_string(),
        lab_run_id: "labrun_test".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: LabRole::Postdoc,
        assigned_role: LabRole::Graduate,
        status: LabTaskStatus::InProgress,
        title: "Write proof".to_string(),
        instructions: "Create proof.txt".to_string(),
        allowed_scope: vec!["proof.txt".to_string()],
        required_validation: vec!["test -f proof.txt".to_string()],
        evidence_ids: Vec::new(),
        result_artifact_id: None,
        blocker: None,
        cycle_id: Some("0".to_string()),
    };

    let evidence = runtime_verify_graduate_task_result(
        &task,
        &context,
        Some("agent_1"),
        "lab-graduate-gradtask_test",
        &[],
    )
    .unwrap();

    assert_eq!(evidence.changed_files, vec!["proof.txt".to_string()]);
    assert!(evidence
        .validation_attempts
        .contains(&"runtime validation `test -f proof.txt` passed".to_string()));
}

#[test]
fn graduate_runtime_verification_falls_back_to_durable_task_id() {
    let project = tempfile::tempdir().unwrap();
    init_git_dir(project.path());
    let worktree = tempfile::tempdir().unwrap();
    init_git_dir(worktree.path());
    std::fs::write(worktree.path().join("proof.txt"), "verified\n").unwrap();
    let context = lab_context_with_agent_worktree_task_id(
        project.path(),
        "lab-provider-command",
        "lab-graduate-gradtask_test",
        "agent_1",
        worktree.path(),
    );
    let task = GraduateTask {
        schema_version: LAB_SCHEMA_VERSION,
        task_id: "gradtask_test".to_string(),
        lab_run_id: "labrun_test".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: LabRole::Postdoc,
        assigned_role: LabRole::Graduate,
        status: LabTaskStatus::InProgress,
        title: "Write proof".to_string(),
        instructions: "Create proof.txt".to_string(),
        allowed_scope: vec!["proof.txt".to_string()],
        required_validation: vec!["test -f proof.txt".to_string()],
        evidence_ids: Vec::new(),
        result_artifact_id: None,
        blocker: None,
        cycle_id: Some("0".to_string()),
    };

    let evidence = runtime_verify_graduate_task_result(
        &task,
        &context,
        Some("unknown_agent"),
        "lab-graduate-gradtask_test",
        &[],
    )
    .unwrap();

    assert_eq!(evidence.changed_files, vec!["proof.txt".to_string()]);
    assert!(evidence
        .validation_attempts
        .contains(&"runtime validation `test -f proof.txt` passed".to_string()));
}

#[test]
fn git_status_path_parser_handles_renames_and_quotes() {
    assert_eq!(
        parse_git_status_path(" M src/lib.rs").as_deref(),
        Some("src/lib.rs")
    );
    assert_eq!(
        parse_git_status_path("R  old.rs -> src/new.rs").as_deref(),
        Some("src/new.rs")
    );
    assert_eq!(
        parse_git_status_path("?? \"docs/lab plan.md\"").as_deref(),
        Some("docs/lab plan.md")
    );
}

#[test]
fn graduate_agent_result_parser_requires_structured_validation() {
    let data = serde_json::json!({
        "graduate_result": {
            "summary": "Implemented the scoped slice.",
            "changed_files": ["src/lab/orchestrator.rs"],
            "validation_results": ["cargo check -q passed"],
            "blockers": [],
            "evidence_ids": ["labevidence_1"]
        }
    });

    let parsed = parse_graduate_agent_result(Some(&data), "").unwrap();

    assert_eq!(parsed.task_summary, "Implemented the scoped slice.");
    assert_eq!(parsed.changed_files, vec!["src/lab/orchestrator.rs"]);
    assert_eq!(parsed.validation_attempts, vec!["cargo check -q passed"]);
    assert_eq!(parsed.evidence_ids, vec!["labevidence_1"]);
    let fenced = parse_graduate_agent_result(
        None,
        r#"```json
{"graduate_result":{"summary":"Implemented fenced JSON.","changed_files":["src/lab/model.rs"],"validation_results":["cargo check -q passed"],"blockers":[],"evidence_ids":[]}}
```"#,
    )
    .unwrap();
    assert_eq!(fenced.task_summary, "Implemented fenced JSON.");
    let prose = parse_graduate_agent_result(
        None,
        r#"Done:
{"graduate_result":{"summary":"Implemented prose JSON.","changed_files":["src/lab/model.rs"],"validation_results":["cargo check -q passed"],"blockers":[],"evidence_ids":[]}}
Thanks."#,
    )
    .unwrap();
    assert_eq!(prose.task_summary, "Implemented prose JSON.");
    assert!(parse_graduate_agent_result(None, "plain text result").is_none());
    assert!(parse_graduate_agent_result(
        Some(&serde_json::json!({"summary": "missing validation"})),
        ""
    )
    .is_none());
}

#[test]
fn unbound_graduate_success_is_failed_and_blocks_task() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    let dispatch = build_graduate_task_dispatch(&task).unwrap();
    let record = orchestrator
        .store()
        .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
        .unwrap();
    orchestrator
        .store()
        .start_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();

    let failed = orchestrator
        .mark_unbound_graduate_success_failed(
            &run,
            &task,
            &record.dispatch_id,
            Some("agent_test".to_string()),
            "I finished it, but I did not return JSON.",
        )
        .unwrap();

    assert_eq!(failed.status, GraduateDispatchStatus::Failed);
    assert_eq!(failed.agent_id.as_deref(), Some("agent_test"));
    assert!(failed
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("without bindable GraduateResult JSON"));
    assert!(failed
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("result_preview=I finished it"));
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, LabTaskStatus::Blocked);
    assert!(saved_task
        .blocker
        .as_deref()
        .unwrap_or_default()
        .contains("without bindable GraduateResult JSON"));
    let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved_run.failure_count, 1);
}

#[test]
fn unbound_graduate_success_can_bind_runtime_verified_result() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Create proof file",
            "Create only the proof file.",
            vec!["lab-live-graduate-proof.md".to_string()],
            vec!["test -f lab-live-graduate-proof.md".to_string()],
        )
        .unwrap();
    orchestrator
        .store()
        .start_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();

    let worktree = temp.path().join("graduate-unbound-runtime-worktree");
    std::fs::create_dir_all(&worktree).unwrap();
    init_git_dir(&worktree);
    std::fs::write(
        worktree.join("lab-live-graduate-proof.md"),
        "runtime verified\n",
    )
    .unwrap();
    let agent_task_id = graduate_agent_task_id(&task);
    let context = lab_context_with_agent_worktree_task_id(
        temp.path(),
        "lab-test",
        &agent_task_id,
        "agent_runtime_verified",
        &worktree,
    );

    let created = orchestrator
        .create_runtime_verified_graduate_result_for_unbound_success(
            &task,
            &context,
            Some("agent_runtime_verified"),
            &[],
            "The iteration limit was reached before final JSON.",
        )
        .unwrap();

    match created.artifact {
        StageArtifact::GraduateResult(envelope) => {
            assert_eq!(
                envelope.body.changed_files,
                vec!["lab-live-graduate-proof.md".to_string()]
            );
            assert!(envelope.body.validation_attempts.contains(
                &"runtime validation `test -f lab-live-graduate-proof.md` passed".to_string()
            ));
            assert!(envelope
                .body
                .task_summary
                .contains("without bindable GraduateResult JSON"));
            assert!(envelope
                .evidence_refs
                .contains(&format!("agent_task:{agent_task_id}")));
            assert!(envelope
                .evidence_refs
                .contains(&"agent:agent_runtime_verified".to_string()));
        }
        other => panic!("expected GraduateResult, got {:?}", other.artifact_type()),
    }
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, LabTaskStatus::Completed);
}

#[tokio::test]
async fn graduate_dispatch_execution_records_failure_without_agent_manager() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();

    let dispatch = orchestrator
        .execute_graduate_task_latest_with_context(
            &task.task_id,
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await
        .unwrap();

    assert_eq!(dispatch.status, GraduateDispatchStatus::Failed);
    assert!(dispatch
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("AgentManager not available"));
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, LabTaskStatus::Blocked);
    let dispatches = orchestrator
        .store()
        .list_graduate_dispatches(&run.lab_run_id)
        .unwrap();
    assert_eq!(dispatches.len(), 1);
    assert_eq!(dispatches[0].status, GraduateDispatchStatus::Failed);
    let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved_run.failure_count, 1);
    assert_eq!(saved_run.status, LabRunStatus::Active);
}

#[tokio::test]
async fn graduate_dispatch_records_workspace_snapshots_around_execution() {
    let temp = tempfile::tempdir().unwrap();
    init_git_dir(temp.path());
    std::fs::write(
        temp.path().join("preexisting-user-change.txt"),
        "user edit\n",
    )
    .unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();

    let dispatch = orchestrator
        .execute_graduate_task_latest_with_context(
            &task.task_id,
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await
        .unwrap();

    assert_eq!(dispatch.status, GraduateDispatchStatus::Failed);
    let events = orchestrator
        .store()
        .list_run_events(&run.lab_run_id)
        .unwrap()
        .into_iter()
        .filter(|event| event.event_type == "lab_graduate_workspace_snapshot")
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].payload["phase"], "before");
    assert_eq!(events[0].payload["task_id"], task.task_id);
    assert_eq!(events[0].payload["dispatch_id"], dispatch.dispatch_id);
    assert_eq!(events[0].payload["dirty_path_count"], 1);
    assert_eq!(
        events[0].payload["dirty_paths"][0],
        "preexisting-user-change.txt"
    );
    assert_eq!(events[1].payload["phase"], "after");
    assert_eq!(events[1].payload["changed_path_count"], 0);
}

#[tokio::test]
async fn graduate_dispatch_binds_completed_durable_agent_state_without_agent_manager() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Bind durable graduate result",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["test -f src/lab/model.rs".to_string()],
        )
        .unwrap();

    let worktree = temp.path().join("graduate-durable-run-worktree");
    std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
    init_git_dir(&worktree);
    std::fs::write(worktree.join("src/lab/model.rs"), "durable graduate edit\n").unwrap();
    let agent_task_id = graduate_agent_task_id(&task);
    let context = lab_context_with_agent_worktree_task_id(
        temp.path(),
        "lab-test",
        &agent_task_id,
        "agent_durable_run",
        &worktree,
    );
    let session_store = context.session_store.as_ref().unwrap();
    let agent_artifact_id = session_store
        .add_agent_artifact(
            "lab-test",
            "agent_durable_run",
            Some("lab-graduate"),
            "implementation",
            "completed",
            "durable graduate result",
            r#"{"graduate_result":{"summary":"Durable graduate result was bound.","changed_files":["src/lab/model.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
            &serde_json::json!({"completion_sink": "agent_manager", "tools_used": ["file_write", "bash"]}),
        )
        .unwrap();
    session_store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "lab-test".to_string(),
            task_id: agent_task_id.clone(),
            agent_id: "agent_durable_run".to_string(),
            profile: Some("lab-graduate".to_string()),
            role: "implementation".to_string(),
            status: "completed".to_string(),
            description: "durable graduate result".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: Some(agent_artifact_id),
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: serde_json::json!({
                "completion_sink": "agent_manager",
                "tools_used": ["file_write", "bash"],
                "isolated_worktree": {
                    "path": worktree.to_string_lossy().to_string(),
                    "branch": "codex/graduate-durable-run"
                }
            }),
        })
        .unwrap();

    let dispatch = orchestrator
        .execute_graduate_task_latest_with_context(&task.task_id, context)
        .await
        .unwrap();

    assert_eq!(dispatch.status, GraduateDispatchStatus::Succeeded);
    assert_eq!(dispatch.agent_id.as_deref(), Some("agent_durable_run"));
    assert!(dispatch.result_artifact_id.is_some());
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, LabTaskStatus::Completed);
    let artifact = orchestrator
        .store()
        .load_stage_artifact(
            &run.lab_run_id,
            dispatch.result_artifact_id.as_deref().unwrap(),
        )
        .unwrap();
    match artifact {
        StageArtifact::GraduateResult(envelope) => {
            assert_eq!(
                envelope.body.changed_files,
                vec!["src/lab/model.rs".to_string()]
            );
            assert!(envelope
                .body
                .validation_attempts
                .iter()
                .any(|item| item.contains("runtime validation")));
        }
        other => panic!("expected GraduateResult, got {:?}", other.artifact_type()),
    }
}

#[tokio::test]
async fn graduate_dispatch_is_not_blocked_by_provider_name_before_agent_run() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    let mut context = ToolContext::new(temp.path(), "lab-test").with_model("deepseek-v4-flash");
    context
        .metadata
        .insert("provider_id".to_string(), "deepseek".to_string());

    let dispatch = orchestrator
        .execute_graduate_task_latest_with_context(&task.task_id, context)
        .await
        .unwrap();

    assert_eq!(dispatch.status, GraduateDispatchStatus::Failed);
    let error = dispatch.error.as_deref().unwrap_or_default().to_string();
    assert!(!error.contains("not certified"));
    assert!(!error.contains("formal Lab graduate certification"));
    assert!(!error.contains("graduate provider"));
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, LabTaskStatus::Blocked);
    let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved_run.failure_count, 1);
}

#[test]
fn graduate_agent_task_sync_binds_completed_durable_state_after_runtime_verification() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let mut run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    run.current_stage = "graduate_work".to_string();
    run.internal_owner = LabRole::Graduate;
    orchestrator.store().save_run(&run).unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement durable sync",
            "Update only the lab orchestrator.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["test -f src/lab/orchestrator.rs".to_string()],
        )
        .unwrap();
    let dispatch = build_graduate_task_dispatch(&task).unwrap();
    let record = orchestrator
        .store()
        .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
        .unwrap();
    orchestrator
        .store()
        .start_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();

    let worktree = temp.path().join("graduate-worktree");
    std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
    init_git_dir(&worktree);
    std::fs::write(
        worktree.join("src/lab/orchestrator.rs"),
        "verified graduate edit\n",
    )
    .unwrap();
    let agent_task_id = graduate_agent_task_id(&task);
    let context = lab_context_with_agent_worktree_task_id(
        temp.path(),
        "lab-test",
        &agent_task_id,
        "agent_sync",
        &worktree,
    );
    let session_store = context.session_store.as_ref().unwrap();
    let agent_artifact_id = session_store
        .add_agent_artifact(
            "lab-test",
            "agent_sync",
            Some("lab-graduate"),
            "implementation",
            "completed",
            "graduate durable sync result",
            r#"{"graduate_result":{"summary":"Synced durable graduate result.","changed_files":["src/lab/orchestrator.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
            &serde_json::json!({"completion_sink": "agent_manager"}),
        )
        .unwrap();
    session_store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "lab-test".to_string(),
            task_id: agent_task_id.clone(),
            agent_id: "agent_sync".to_string(),
            profile: Some("lab-graduate".to_string()),
            role: "implementation".to_string(),
            status: "completed".to_string(),
            description: "graduate durable sync result".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: Some(agent_artifact_id),
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: serde_json::json!({
                "completion_sink": "agent_manager",
                "tools_used": ["file_write", "bash"],
                "isolated_worktree": {
                    "path": worktree.to_string_lossy().to_string(),
                    "branch": "codex/graduate-sync"
                }
            }),
        })
        .unwrap();

    let created = orchestrator
        .sync_graduate_agent_task_latest_with_context(&task.task_id, context)
        .unwrap();
    let graduate_result_artifact_id = created.artifact.artifact_id().to_string();

    match &created.artifact {
        StageArtifact::GraduateResult(envelope) => {
            assert_eq!(
                envelope.body.changed_files,
                vec!["src/lab/orchestrator.rs".to_string()]
            );
            assert!(envelope
                .body
                .validation_attempts
                .iter()
                .any(|attempt| attempt
                    == "runtime validation `test -f src/lab/orchestrator.rs` passed"));
            assert!(envelope
                .evidence_refs
                .contains(&format!("agent_task:{agent_task_id}")));
            assert!(envelope
                .evidence_refs
                .contains(&format!("agent_artifact:{agent_artifact_id}")));
        }
        other => panic!("expected GraduateResult, got {:?}", other.artifact_type()),
    }
    assert!(created.gate.is_satisfied());
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, LabTaskStatus::Completed);
    let saved_dispatch = orchestrator
        .store()
        .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
        .unwrap();
    assert_eq!(saved_dispatch.status, GraduateDispatchStatus::Succeeded);
    assert_eq!(saved_dispatch.agent_id.as_deref(), Some("agent_sync"));
    assert_eq!(
        saved_dispatch.result_artifact_id.as_deref(),
        Some(graduate_result_artifact_id.as_str())
    );
}

#[tokio::test]
async fn repeated_graduate_dispatch_failures_escalate_to_needs_user() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();

    let _ = orchestrator
        .execute_graduate_task_latest_with_context(
            &task.task_id,
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await
        .unwrap();
    let second = orchestrator
        .execute_graduate_task_latest_with_context(
            &task.task_id,
            ToolContext::new(temp.path(), "lab-test"),
        )
        .await
        .unwrap();

    assert_eq!(second.status, GraduateDispatchStatus::Failed);
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.failure_count, 2);
    assert_eq!(saved.status, LabRunStatus::NeedsUser);
    assert!(saved.needs_user);
}

#[tokio::test]
async fn scheduler_blocks_graduate_work_without_queued_task() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let mut run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    run.current_stage = "graduate_work".to_string();
    run.internal_owner = LabRole::Graduate;
    orchestrator.store().save_run(&run).unwrap();

    let step = orchestrator
        .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
        .await
        .unwrap();

    assert_eq!(step.action, LabSchedulerStepAction::Blocked);
    assert!(step.message.contains("requires a queued GraduateTask"));
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert!(saved.artifact_ids.is_empty());
}

#[tokio::test]
async fn scheduler_refuses_to_run_without_active_lease() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    std::fs::remove_file(orchestrator.store().root().join("active_lease.json")).unwrap();

    let err = orchestrator
        .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("active lease is missing"));
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert!(saved.artifact_ids.is_empty());
}

#[tokio::test]
async fn scheduler_dispatches_queued_graduate_task() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let mut run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    run.current_stage = "graduate_work".to_string();
    run.internal_owner = LabRole::Graduate;
    orchestrator.store().save_run(&run).unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();

    let step = orchestrator
        .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
        .await
        .unwrap();

    assert_eq!(step.action, LabSchedulerStepAction::GraduateDispatched);
    assert_eq!(step.task_id.as_deref(), Some(task.task_id.as_str()));
    assert!(step.dispatch_id.is_some());
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, LabTaskStatus::Blocked);
}

#[tokio::test]
async fn scheduler_advances_after_verified_graduate_result() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let mut run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    run.current_stage = "graduate_work".to_string();
    run.internal_owner = LabRole::Graduate;
    orchestrator.store().save_run(&run).unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab orchestrator.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Implemented scoped slice.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["runtime validation `cargo check -q` passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

    let step = orchestrator
        .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
        .await
        .unwrap();

    assert_eq!(step.action, LabSchedulerStepAction::TickAdvanced);
    assert_eq!(step.stage, "postdoc_review");
    assert!(step.message.contains("verified GraduateResult"));
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.current_stage, "postdoc_review");
    assert_eq!(saved.internal_owner, LabRole::Postdoc);
}

#[tokio::test]
async fn scheduler_syncs_completed_durable_graduate_task_before_blocking_in_progress() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let mut run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    run.current_stage = "graduate_work".to_string();
    run.internal_owner = LabRole::Graduate;
    orchestrator.store().save_run(&run).unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Sync completed durable task",
            "Update only the lab orchestrator.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["test -f src/lab/orchestrator.rs".to_string()],
        )
        .unwrap();
    let dispatch = build_graduate_task_dispatch(&task).unwrap();
    let record = orchestrator
        .store()
        .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
        .unwrap();
    orchestrator
        .store()
        .start_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();

    let worktree = temp.path().join("graduate-scheduler-sync-worktree");
    std::fs::create_dir_all(worktree.join("src/lab")).unwrap();
    init_git_dir(&worktree);
    std::fs::write(
        worktree.join("src/lab/orchestrator.rs"),
        "scheduler durable graduate edit\n",
    )
    .unwrap();
    let agent_task_id = graduate_agent_task_id(&task);
    let context = lab_context_with_agent_worktree_task_id(
        temp.path(),
        "lab-test",
        &agent_task_id,
        "agent_scheduler_sync",
        &worktree,
    );
    let session_store = context.session_store.as_ref().unwrap();
    let agent_artifact_id = session_store
        .add_agent_artifact(
            "lab-test",
            "agent_scheduler_sync",
            Some("lab-graduate"),
            "implementation",
            "completed",
            "graduate scheduler durable sync result",
            r#"{"graduate_result":{"summary":"Scheduler synced durable graduate result.","changed_files":["src/lab/orchestrator.rs"],"validation_results":["claimed validation"],"blockers":[],"evidence_ids":[]}}"#,
            &serde_json::json!({"completion_sink": "agent_manager"}),
        )
        .unwrap();
    session_store
        .upsert_agent_task_state(&crate::session_store::AgentTaskStateUpsert {
            session_id: "lab-test".to_string(),
            task_id: agent_task_id.clone(),
            agent_id: "agent_scheduler_sync".to_string(),
            profile: Some("lab-graduate".to_string()),
            role: "implementation".to_string(),
            status: "completed".to_string(),
            description: "graduate scheduler durable sync result".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: Some(agent_artifact_id),
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: serde_json::json!({
                "completion_sink": "agent_manager",
                "tools_used": ["file_write", "bash"],
                "isolated_worktree": {
                    "path": worktree.to_string_lossy().to_string(),
                    "branch": "codex/graduate-scheduler-sync"
                }
            }),
        })
        .unwrap();

    let step = orchestrator
        .run_scheduler_step_latest_with_context(context)
        .await
        .unwrap();

    assert_eq!(step.action, LabSchedulerStepAction::TickAdvanced);
    assert_eq!(step.stage, "postdoc_review");
    assert_eq!(step.task_id.as_deref(), Some(task.task_id.as_str()));
    assert!(step.message.contains("synced durable graduate result"));
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, LabTaskStatus::Completed);
    let saved_dispatch = orchestrator
        .store()
        .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
        .unwrap();
    assert_eq!(saved_dispatch.status, GraduateDispatchStatus::Succeeded);
    assert_eq!(
        saved_dispatch.agent_id.as_deref(),
        Some("agent_scheduler_sync")
    );
    let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved_run.current_stage, "postdoc_review");
    assert_eq!(saved_run.internal_owner, LabRole::Postdoc);
}

#[tokio::test]
async fn scheduler_blocks_completed_durable_graduate_task_without_artifact() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let mut run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    run.current_stage = "graduate_work".to_string();
    run.internal_owner = LabRole::Graduate;
    orchestrator.store().save_run(&run).unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Sync incomplete durable task",
            "Update only the lab orchestrator.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["test -f src/lab/orchestrator.rs".to_string()],
        )
        .unwrap();
    let dispatch = build_graduate_task_dispatch(&task).unwrap();
    let record = orchestrator
        .store()
        .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
        .unwrap();
    orchestrator
        .store()
        .start_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    let agent_task_id = graduate_agent_task_id(&task);
    let context = lab_context_with_agent_worktree_task_id(
        temp.path(),
        "lab-test",
        &agent_task_id,
        "agent_missing_artifact",
        temp.path(),
    );

    let step = orchestrator
        .run_scheduler_step_latest_with_context(context)
        .await
        .unwrap();

    assert_eq!(step.action, LabSchedulerStepAction::Blocked);
    assert_eq!(step.stage, "graduate_work");
    assert_eq!(step.task_id.as_deref(), Some(task.task_id.as_str()));
    assert!(step.message.contains("has no result artifact"));
    let saved_task = orchestrator
        .store()
        .load_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(saved_task.status, LabTaskStatus::Blocked);
    assert!(saved_task
        .blocker
        .as_deref()
        .unwrap_or_default()
        .contains("has no result artifact"));
    let saved_dispatch = orchestrator
        .store()
        .load_graduate_dispatch(&run.lab_run_id, &record.dispatch_id)
        .unwrap();
    assert_eq!(saved_dispatch.status, GraduateDispatchStatus::Failed);
    assert_eq!(
        saved_dispatch.agent_id.as_deref(),
        Some("agent_missing_artifact")
    );
    assert!(saved_dispatch
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("has no result artifact"));
    let saved_run = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved_run.failure_count, 1);
}

#[tokio::test]
async fn scheduler_stops_at_role_review_boundaries() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update scheduler review bridges.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Implemented scheduler review bridges.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q passed".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();

    let postdoc = orchestrator
        .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
        .await
        .unwrap();
    assert_eq!(postdoc.action, LabSchedulerStepAction::Blocked);
    assert_eq!(postdoc.stage, "postdoc_review");
    assert!(postdoc
        .message
        .contains("PostdocIntegrationSummary artifact is required"));
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.current_stage, "postdoc_review");
    assert!(!saved.needs_user);
}

#[tokio::test]
async fn explicit_professor_review_writes_revision_task_without_scheduler_auto_repair() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Repair validation failure.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Could not complete validation.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q failed".to_string()],
            vec!["validation still fails".to_string()],
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();
    orchestrator
        .create_postdoc_integration_summary_for_latest(None)
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "professor_review".to_string();
    saved.internal_owner = LabRole::Professor;
    orchestrator.store().save_run(&saved).unwrap();
    let professor_review = orchestrator
        .create_professor_review_for_latest(Some("Explicit professor revision request."))
        .unwrap();

    let step = orchestrator
        .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
        .await
        .unwrap();

    assert_eq!(step.action, LabSchedulerStepAction::Blocked);
    assert_eq!(step.stage, "professor_review");
    assert!(step
        .message
        .contains("ProfessorReview artifact is required"));
    let resumed = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(resumed.current_stage, "professor_review");
    assert_eq!(resumed.internal_owner, LabRole::Professor);
    assert!(!resumed.needs_user);
    let revision_artifact_id = orchestrator
        .store()
        .list_stage_artifacts(&run.lab_run_id)
        .unwrap()
        .into_iter()
        .find_map(|artifact| match artifact {
            StageArtifact::LabRevisionTask(revision) => Some(revision.artifact_id),
            _ => None,
        })
        .expect("revision task artifact");
    assert!(professor_review
        .gate
        .blockers
        .iter()
        .any(|blocker| blocker.contains("runtime placeholder")));
    let gate = orchestrator
        .store()
        .load_artifact_gate(&run.lab_run_id, "postdoc_revision")
        .unwrap();
    assert_eq!(gate.required_artifact_type, "LabRevisionTask");
    assert_eq!(
        gate.artifact_id.as_deref(),
        Some(revision_artifact_id.as_str())
    );
    let revision = orchestrator
        .store()
        .load_stage_artifact(&run.lab_run_id, &revision_artifact_id)
        .unwrap();
    assert_eq!(revision.validation_status(), Some("not_started"));
}

#[tokio::test]
async fn scheduler_blocks_when_postdoc_review_bridge_is_blocked() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    let task = orchestrator
        .store()
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update scheduler review bridges.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    orchestrator
        .create_graduate_result_for_task_latest(
            &task.task_id,
            "Could not finish scheduler review bridge.",
            vec!["src/lab/orchestrator.rs".to_string()],
            vec!["cargo check -q failed".to_string()],
            vec!["validation still fails".to_string()],
            Vec::new(),
        )
        .unwrap();
    let mut saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    saved.current_stage = "postdoc_review".to_string();
    saved.internal_owner = LabRole::Postdoc;
    orchestrator.store().save_run(&saved).unwrap();

    let step = orchestrator
        .run_scheduler_step_latest_with_context(ToolContext::new(temp.path(), "lab-test"))
        .await
        .unwrap();

    assert_eq!(step.action, LabSchedulerStepAction::Blocked);
    assert_eq!(step.stage, "postdoc_review");
    assert!(step.message.contains("PostdocIntegrationSummary"));
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.current_stage, "postdoc_review");
}

#[test]
fn tick_blocks_without_current_stage_artifact_gate() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();

    let tick = orchestrator.tick_latest().unwrap();

    assert_eq!(tick.status, LabTickStatus::Blocked);
    assert_eq!(tick.from_stage, "professor_discussion");
    assert_eq!(tick.to_stage, "professor_discussion");
    assert!(tick.artifact_id.is_none());
    assert!(tick.report_path.is_none());
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.current_stage, "professor_discussion");
    assert_eq!(saved.internal_owner, LabRole::Professor);
}

#[test]
fn tick_remains_blocked_until_role_artifact_exists() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();

    let first = orchestrator.tick_latest().unwrap();
    assert_eq!(first.status, LabTickStatus::Blocked);
    assert_eq!(first.to_stage, "professor_discussion");

    let second = orchestrator.tick_latest().unwrap();
    assert_eq!(second.status, LabTickStatus::Blocked);
    assert_eq!(second.to_stage, "professor_discussion");
    let saved = orchestrator.store().load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.status, LabRunStatus::Active);
    assert!(!saved.needs_user);
    assert_eq!(saved.current_stage, "professor_discussion");
}

#[test]
fn continue_from_user_report_starts_next_cycle_with_fresh_professor_gate() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();

    drive_to_user_report_with_explicit_artifacts(&orchestrator);

    let continued = orchestrator
        .continue_latest_from_user_report("first cycle reviewed; continue")
        .unwrap();

    assert_eq!(continued.lab_run_id, run.lab_run_id);
    assert_eq!(continued.status, LabRunStatus::Active);
    assert_eq!(continued.current_stage, "professor_discussion");
    assert_eq!(continued.internal_owner, LabRole::Professor);
    assert!(!continued.needs_user);
    assert_eq!(continued.cycle_count, 1);
    let gate = orchestrator
        .store()
        .load_artifact_gate(&run.lab_run_id, "professor_discussion")
        .unwrap();
    assert_eq!(gate.required_artifact_type, "ProfessorPlan");
    assert!(gate.artifact_id.is_none());
    let artifacts = orchestrator
        .store()
        .list_stage_artifacts(&run.lab_run_id)
        .unwrap();
    let cycle_summary = artifacts
        .iter()
        .find_map(|artifact| match artifact {
            StageArtifact::CycleSummary(summary) => Some(summary),
            _ => None,
        })
        .expect("cycle summary");
    assert_eq!(
        cycle_summary.validation_status.as_deref(),
        Some("read_only_runtime_summary")
    );
    assert!(cycle_summary
        .evidence_refs
        .iter()
        .any(|item| item.starts_with("artifact:artifact_professorreview_")));
    assert!(cycle_summary
        .evidence_refs
        .iter()
        .any(|item| item.starts_with("artifact:artifact_postdocintegrationsummary_")));
}

#[test]
fn final_user_report_closeout_derives_status_from_professor_gate() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let proposal = orchestrator
        .store()
        .create_proposal("Build LabRun", None)
        .unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();

    drive_to_user_report_with_explicit_artifacts(&orchestrator);

    let closed = orchestrator
        .closeout_latest_from_user_report("final report shown to user")
        .unwrap();

    assert_eq!(closed.lab_run_id, run.lab_run_id);
    assert_eq!(closed.status, LabRunStatus::Completed);
    assert_eq!(
        closed.closeout_status,
        Some(LabCloseoutStatus::CompletedNotVerified)
    );
    assert!(!closed.needs_user);
    assert!(!orchestrator
        .store()
        .root()
        .join("active_lease.json")
        .exists());
}
