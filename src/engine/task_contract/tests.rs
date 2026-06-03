use super::*;
use crate::engine::intent_router::IntentRouter;
use crate::engine::retrieval_context::{RetrievalContext, RetrievalItem};
use crate::engine::task_context::{
    ActionScoreRecord, AgentTaskStage, StopAction, StopCheckReason, StopCheckRecord,
    StopCheckStatus,
};
use serde_json::json;

#[test]
fn task_contract_materializes_assumptions_scope_and_validation() {
    let prompt = "修改 src/lib.rs";
    let route = IntentRouter::new().route(prompt);
    let mut bundle = TaskContextBundle::new(prompt, ".", route, None);
    bundle.add_file("src/lib.rs");
    bundle.add_constraint("resource_policy=standard");
    bundle.add_acceptance_check("cargo test -q");

    let contract = bundle.task_contract(&["cargo test -q".to_string()]);

    assert_eq!(contract.task_type, TaskContractType::CodeChange);
    assert_eq!(contract.scope.files_allowed, vec!["src/lib.rs"]);
    assert!(contract.validation.proof_required);
    assert!(contract
        .assumptions
        .iter()
        .any(|item| item.source == AssumptionSource::DefaultPolicy));
    assert_eq!(contract.model_profile, ModelProfileMode::Standard);
    assert!(contract.compact_summary().contains("TaskContract id="));
}

#[test]
fn task_contract_uses_review_required_after_failed_validation() {
    let route = IntentRouter::new().route("修复 src/lib.rs 里的测试失败");
    let mut bundle = TaskContextBundle::new("修复 src/lib.rs 里的测试失败", ".", route, None);
    bundle.agent_state.verification_plan.status = VerificationStatus::Failed;

    let contract = bundle.task_contract(&[]);

    assert_eq!(contract.model_profile, ModelProfileMode::ReviewRequired);
}

#[test]
fn task_contract_keeps_standard_profile_for_low_action_score_history() {
    let route = IntentRouter::new().route("修改 src/lib.rs");
    let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
    for idx in 0..2 {
        bundle.agent_state.record_action_score(ActionScoreRecord {
            tool: format!("tool-{idx}"),
            stage: "Edit".to_string(),
            action_score: 2,
            value: 2,
            risk: 2,
            uncertainty_reduction: 1,
            cost: 3,
            reversibility: 8,
            scope_fit: 8,
            formula_stage: None,
            formula_version: None,
            review_decision: None,
            reduced_uncertainty: false,
        });
    }

    let contract = bundle.task_contract(&[]);

    assert_eq!(contract.model_profile, ModelProfileMode::Standard);
}

#[test]
fn context_pack_applies_budgets_and_provenance() {
    let route = IntentRouter::new().route("分析项目");
    let mut retrieval = RetrievalContext::new("分析项目", route.retrieval);
    for idx in 0..12 {
        retrieval.add_item(RetrievalItem::new(
            RetrievalSource::Project,
            format!("project fact {idx}"),
            format!("content {idx}"),
            0.8,
            format!("project.index:{idx}"),
            TrustLevel::High,
        ));
    }
    for idx in 0..7 {
        retrieval.add_item(RetrievalItem::new(
            RetrievalSource::Memory,
            format!("memory fact {idx}"),
            format!("memory {idx}"),
            0.7,
            format!("memory.match:{idx}"),
            TrustLevel::Medium,
        ));
    }
    let mut bundle = TaskContextBundle::new("分析项目", ".", route, None).with_retrieval(retrieval);
    for idx in 0..10 {
        bundle
            .agent_state
            .record_observation("test", format!("observation {idx}"));
    }
    bundle.agent_state.record_stop_check(StopCheckRecord {
        status: StopCheckStatus::Stop,
        terminal_status: None,
        action: StopAction::Closeout,
        reason: StopCheckReason::NoProgress,
        summary: "no progress".to_string(),
        evidence: Vec::new(),
        failure_type: Some("no_progress".to_string()),
        recovery_plan_id: None,
        rollback_candidate: None,
        next_action: Some("ask for missing scope".to_string()),
        no_code_progress_rounds: 2,
        action_checkpoint_active: false,
    });

    let contract = bundle.task_contract(&[]);
    let pack = bundle.context_pack(&contract);

    assert_eq!(pack.project_facts.len(), pack.budget.max_project_facts);
    assert_eq!(pack.memory_records.len(), pack.budget.max_memory_records);
    assert_eq!(
        pack.recent_observations.len(),
        pack.budget.max_recent_observations
    );
    assert!(!pack.failure_summaries.is_empty());
    assert!(pack.overflow_items > 0);
    assert_eq!(pack.fingerprint.len(), 12);
    assert!(pack.compact_summary().contains("ContextPack id="));
}

#[test]
fn execution_report_maps_closeout_statuses() {
    let route = IntentRouter::new().route("修改 src/lib.rs");
    let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
    bundle.add_file("src/lib.rs");
    let contract = bundle.task_contract(&["cargo test -q".to_string()]);
    let closeout = WorkflowCloseout {
        status: StageValidationStatus::NotVerified,
        risk: RiskLevel::Medium,
        changed_files: vec!["src/lib.rs".to_string()],
        validation: vec!["verification proof: not_run".to_string()],
        acceptance: Vec::new(),
        residual_risks: vec!["validation was not run".to_string()],
    };

    let report = ExecutionReport::from_closeout(&contract, &closeout);

    assert_eq!(report.status, ExecutionReportStatus::NotVerified);
    assert_eq!(report.changed_files, vec!["src/lib.rs"]);
    assert_eq!(report.next_steps.len(), 1);
    assert!(report.compact_summary().contains("status=not_verified"));
}

#[test]
fn memory_proposal_is_review_only_and_evidence_backed() {
    let report = ExecutionReport {
        task_id: "task-1".to_string(),
        objective: "修改 src/lib.rs".to_string(),
        status: ExecutionReportStatus::Success,
        changed_files: vec!["src/lib.rs".to_string()],
        validation_evidence: vec!["cargo test -q: passed".to_string()],
        risks: vec!["none recorded".to_string()],
        next_steps: Vec::new(),
        assumptions: Vec::new(),
    };

    let proposal = MemoryProposal::from_execution_report(&report);

    assert_eq!(proposal.status, MemoryProposalStatus::Proposed);
    assert_eq!(proposal.candidates.len(), 1);
    assert_eq!(proposal.candidates[0].kind, "successful_fix");
    assert!(!proposal.write_performed);
    assert_eq!(proposal.write_policy, "review_required");
    assert!(proposal.evidence_items() >= 2);
    assert!(proposal.compact_summary().contains("write_performed=false"));
}

#[test]
fn memory_proposal_skips_unevidenced_direct_success() {
    let report = ExecutionReport {
        task_id: "task-1".to_string(),
        objective: "回答问题".to_string(),
        status: ExecutionReportStatus::Success,
        changed_files: Vec::new(),
        validation_evidence: Vec::new(),
        risks: vec!["none recorded".to_string()],
        next_steps: Vec::new(),
        assumptions: Vec::new(),
    };

    let proposal = MemoryProposal::from_execution_report(&report);

    assert_eq!(proposal.status, MemoryProposalStatus::NotApplicable);
    assert!(proposal.candidates.is_empty());
    assert!(!proposal.write_performed);
}

#[test]
fn memory_proposal_review_store_tracks_status_by_prefix() {
    let path = std::env::temp_dir().join(format!(
        "priority-agent-memory-proposals-{}.jsonl",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(path.clone());
    let report = ExecutionReport {
        task_id: "task-review-123".to_string(),
        objective: "fix parser".to_string(),
        status: ExecutionReportStatus::Success,
        changed_files: vec!["src/parser.rs".to_string()],
        validation_evidence: vec!["cargo test parser: passed".to_string()],
        risks: Vec::new(),
        next_steps: Vec::new(),
        assumptions: Vec::new(),
    };
    let proposal = MemoryProposal::from_execution_report(&report);

    store.upsert(&proposal).unwrap();
    let updated = store
        .update_status("task-review", MemoryProposalStatus::Accepted)
        .unwrap()
        .unwrap();

    assert_eq!(updated.status, MemoryProposalStatus::Accepted);
    assert_eq!(store.list().len(), 1);
    assert_eq!(
        store.get("task-review").unwrap().status,
        MemoryProposalStatus::Accepted
    );
    let record = store.get_record("task-review").unwrap();
    assert!(record.id.starts_with("mp-"));
    assert_ne!(record.id, "task-review-123");
    assert_eq!(
        store.get_record(&record.id).unwrap().proposal.task_id,
        "task-review-123"
    );
    assert_eq!(record.source_task, "task-review-123");
    assert_eq!(record.active_scope, "project");
    assert!(record.project_id.is_some());
    assert!(memory_proposal_record_matches_filter(
        &record,
        &MemoryProposalBatchFilter {
            project: record.project_id.clone(),
            ..Default::default()
        }
    ));
    assert!(!memory_proposal_record_matches_filter(
        &record,
        &MemoryProposalBatchFilter {
            project: Some("definitely-not-this-project".to_string()),
            ..Default::default()
        }
    ));
    assert!(record
        .gate_report
        .iter()
        .any(|gate| gate.gate == "write_policy" && gate.status == "passed"));
    assert!(record.status_history.iter().any(|entry| {
        entry.status == MemoryProposalStatus::Proposed
            || entry.status == MemoryProposalStatus::Accepted
    }));
    let edited = store
        .edit_first_candidate("task-review", "Completed parser fix with cargo test parser")
        .unwrap()
        .unwrap();
    assert_eq!(edited.status, MemoryProposalStatus::Proposed);
    assert_eq!(
        edited.candidates[0].content,
        "Completed parser fix with cargo test parser"
    );
    let edited_record = store.get_record("task-review").unwrap();
    assert_eq!(edited_record.id, record.id);
    assert!(edited_record
        .status_history
        .iter()
        .any(|entry| entry.reason.contains("edited candidate content")));
    let _ = std::fs::remove_file(path);
}

#[test]
fn memory_proposal_review_actions_are_operation_journaled() {
    let base = std::env::temp_dir().join(format!(
        "priority-agent-memory-proposal-review-journal-{}",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
    let proposal = MemoryProposal {
        task_id: "review-journal-proposal".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "User preference: gex explicitly prefers concise Chinese updates.".to_string(),
            evidence: vec!["user_statement: gex prefers concise Chinese updates".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    store.upsert(&proposal).unwrap();
    let record_id = store.get_record("review-journal-proposal").unwrap().id;
    store
        .update_status("review-journal-proposal", MemoryProposalStatus::Accepted)
        .unwrap()
        .unwrap();
    let mut memory = crate::memory::MemoryManager::with_base_dir(base.clone());
    store
        .apply("review-journal-proposal", &mut memory)
        .unwrap()
        .unwrap();

    let journal = crate::memory::LocalMemoryProvider::with_base_dir(&base)
        .operation_journal_entries()
        .unwrap();

    assert!(journal
        .iter()
        .any(|entry| entry.operation == "memory_proposal_create"
            && entry.record_id.as_deref() == Some(record_id.as_str())
            && entry.candidate_id.as_deref() == Some("review-journal-proposal")));
    assert!(journal
        .iter()
        .any(|entry| entry.operation == "memory_proposal_accept"
            && entry.record_id.as_deref() == Some(record_id.as_str())
            && entry.status == "accepted"));
    assert!(journal
        .iter()
        .any(|entry| entry.operation == "memory_proposal_apply"
            && entry.record_id.as_deref() == Some(record_id.as_str())
            && entry.status == "applied"));

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn memory_proposal_gate_reports_topic_scope_identity() {
    let explicit_topic = MemoryProposal {
        task_id: "topic-explicit".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "workflow_convention".to_string(),
            scope: "topic:Rust Workflow".to_string(),
            content: "Rust workflow convention: run cargo test before closeout.".to_string(),
            evidence: vec!["source_task: topic-explicit".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "test".to_string(),
    };
    let bare_topic = MemoryProposal {
        task_id: "topic-ambiguous".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "workflow_convention".to_string(),
            scope: "topic".to_string(),
            content: "Workflow convention: run cargo test before closeout.".to_string(),
            evidence: vec!["source_task: topic-ambiguous".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "test".to_string(),
    };

    let explicit_gate = proposal_gate_report(&explicit_topic, &[])
        .into_iter()
        .find(|gate| gate.gate == "scope_identity")
        .expect("scope identity gate");
    let ambiguous_gate = proposal_gate_report(&bare_topic, &[])
        .into_iter()
        .find(|gate| gate.gate == "scope_identity")
        .expect("scope identity gate");
    let ambiguous_candidate_gate = proposal_gate_report(&bare_topic, &[])
        .into_iter()
        .find(|gate| gate.gate == "scope_identity" && gate.candidate_index == Some(0))
        .expect("candidate scope identity gate");

    assert_eq!(explicit_gate.status, "passed");
    assert!(explicit_gate.reason.contains("topic:rust-workflow"));
    assert_eq!(ambiguous_gate.status, "review_required");
    assert!(ambiguous_gate.reason.contains("ambiguous_topic"));
    assert_eq!(ambiguous_candidate_gate.status, "review_required");
    assert!(ambiguous_candidate_gate
        .reason
        .contains("ambiguous_topic:missing_topic_id"));
}

#[test]
fn memory_proposal_gate_reports_kind_specific_evidence_minimums() {
    let explicit_preference = MemoryProposal {
        task_id: "pref-explicit".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "User preference: answer in Chinese.".to_string(),
            evidence: vec!["user: answer in Chinese".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "test".to_string(),
    };
    let inferred_preference = MemoryProposal {
        task_id: "pref-inferred".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "User preference: answer in Chinese.".to_string(),
            evidence: vec!["background: inferred language preference".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "test".to_string(),
    };
    let validation_baseline = MemoryProposal {
        task_id: "validation-baseline".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "validation_baseline".to_string(),
            scope: "project".to_string(),
            content: "Validation baseline: cargo test -q".to_string(),
            evidence: vec![
                "source_task: validation-baseline".to_string(),
                "closeout: status=success validation=1".to_string(),
                "cargo test -q passed".to_string(),
            ],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "test".to_string(),
    };

    let explicit_gate = proposal_gate_report(&explicit_preference, &[])
        .into_iter()
        .find(|gate| gate.gate == "minimum_evidence")
        .expect("minimum evidence gate");
    let inferred_gate = proposal_gate_report(&inferred_preference, &[])
        .into_iter()
        .find(|gate| gate.gate == "minimum_evidence")
        .expect("minimum evidence gate");
    let validation_gate = proposal_gate_report(&validation_baseline, &[])
        .into_iter()
        .find(|gate| gate.gate == "minimum_evidence")
        .expect("minimum evidence gate");
    let inferred_candidate_gate = proposal_gate_report(&inferred_preference, &[])
        .into_iter()
        .find(|gate| gate.gate == "minimum_evidence" && gate.candidate_index == Some(0))
        .expect("candidate minimum evidence gate");

    assert_eq!(explicit_gate.status, "passed");
    assert!(explicit_gate.reason.contains("explicit_user_statement"));
    assert_eq!(inferred_gate.status, "review_required");
    assert!(inferred_gate.reason.contains("explicit_user_statement"));
    assert_eq!(validation_gate.status, "passed");
    assert!(validation_gate.reason.contains("validation_evidence"));
    assert_eq!(inferred_candidate_gate.status, "review_required");
    assert!(inferred_candidate_gate
        .reason
        .contains("explicit_user_statement"));
}

#[test]
fn memory_proposal_gate_reports_sensitivity_and_apply_blocks_secret() {
    let base = std::env::temp_dir().join(format!(
        "priority-agent-sensitive-proposal-{}",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
    let local_path = MemoryProposal {
        task_id: "local-path-proposal".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "project_fact".to_string(),
            scope: "project".to_string(),
            content: "Local path for this project: /Users/gex/src/rust-agent".to_string(),
            evidence: vec!["source_task: local-path-proposal".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "test".to_string(),
    };
    let secret = MemoryProposal {
        task_id: "secret-proposal".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Accepted,
        candidates: vec![MemoryProposalCandidate {
            kind: "project_fact".to_string(),
            scope: "project".to_string(),
            content: "OPENAI_API_KEY=sk-123456789012345678901234".to_string(),
            evidence: vec!["source_task: secret-proposal".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "accepted for test".to_string(),
    };

    let local_gate = proposal_gate_report(&local_path, &[])
        .into_iter()
        .find(|gate| gate.gate == "sensitivity")
        .expect("sensitivity gate");
    let secret_gate = proposal_gate_report(&secret, &[])
        .into_iter()
        .find(|gate| gate.gate == "sensitivity")
        .expect("sensitivity gate");
    store.upsert(&secret).unwrap();
    let secret_candidate_gate = store
        .get_record("secret-proposal")
        .unwrap()
        .gate_report
        .into_iter()
        .find(|gate| gate.gate == "sensitivity" && gate.candidate_index == Some(0))
        .expect("candidate sensitivity gate");
    let secret_record = store.get_record("secret-proposal").unwrap();
    let mut manager = crate::memory::MemoryManager::with_base_dir(base.clone());
    let apply_error = store
        .apply("secret-proposal", &mut manager)
        .expect_err("secret proposal apply should be blocked");

    assert_eq!(local_gate.status, "review_required");
    assert!(local_gate.reason.contains("private_user_data"));
    assert_eq!(secret_gate.status, "blocked");
    assert!(secret_gate.reason.contains("secret_or_credential"));
    assert_eq!(secret_candidate_gate.status, "blocked");
    assert!(secret_candidate_gate
        .reason
        .contains("secret_or_credential"));
    assert!(memory_proposal_record_matches_filter(
        &secret_record,
        &MemoryProposalBatchFilter {
            blocked_only: true,
            ..Default::default()
        }
    ));
    assert!(apply_error.to_string().contains("sensitivity gate blocked"));
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn accepted_proposal_apply_preserves_evidence_refs_on_memory_record() {
    let base = std::env::temp_dir().join(format!(
        "priority-agent-proposal-evidence-{}",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
    let proposal = MemoryProposal {
        task_id: "evidence-apply".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Accepted,
        candidates: vec![MemoryProposalCandidate {
            kind: "project_fact".to_string(),
            scope: "project".to_string(),
            content: "Project fact: memory proposal apply preserves evidence refs.".to_string(),
            evidence: vec![
                "source_task: evidence-apply".to_string(),
                "closeout: status=success validation=1".to_string(),
                "cargo test -q passed".to_string(),
            ],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "accepted for evidence apply test".to_string(),
    };
    let mut manager = crate::memory::MemoryManager::with_base_dir(base.clone());

    store.upsert(&proposal).unwrap();
    let applied = store
        .apply("evidence-apply", &mut manager)
        .unwrap()
        .unwrap();
    let records = manager.memory_records();

    assert_eq!(applied.1, 1);
    let record = records
        .iter()
        .find(|record| record.content.contains("preserves evidence refs"))
        .expect("applied memory record");
    assert!(record
        .evidence
        .iter()
        .any(|evidence| evidence.source == "memory_proposal:evidence-apply"));
    assert!(record.evidence.iter().any(|evidence| {
        evidence.summary.contains("cargo test -q passed")
            && matches!(evidence.kind, crate::memory::MemoryEvidenceKind::ToolOutput)
    }));
    assert!(record
        .evidence
        .iter()
        .any(|evidence| evidence.summary.contains("closeout:")));
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn accepted_proposal_apply_blocks_missing_candidate_evidence() {
    let base = std::env::temp_dir().join(format!(
        "priority-agent-proposal-missing-evidence-{}",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
    let proposal = MemoryProposal {
        task_id: "missing-evidence-apply".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Accepted,
        candidates: vec![MemoryProposalCandidate {
            kind: "project_fact".to_string(),
            scope: "project".to_string(),
            content: "Project fact: this should not apply without evidence.".to_string(),
            evidence: Vec::new(),
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "accepted for missing evidence test".to_string(),
    };
    let mut manager = crate::memory::MemoryManager::with_base_dir(base.clone());

    store.upsert(&proposal).unwrap();
    let error = store
        .apply("missing-evidence-apply", &mut manager)
        .expect_err("missing evidence should block apply");

    assert!(error.to_string().contains("minimum evidence gate blocked"));
    assert!(manager.memory_records().is_empty());
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn accepted_topic_scope_proposal_applies_to_named_topic_file() {
    let base = std::env::temp_dir().join(format!(
        "priority-agent-topic-scope-{}",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
    let proposal = MemoryProposal {
        task_id: "topic-apply".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Accepted,
        candidates: vec![MemoryProposalCandidate {
            kind: "workflow_convention".to_string(),
            scope: "topic:Rust Workflow".to_string(),
            content: "Rust workflow convention: run cargo test before closeout.".to_string(),
            evidence: vec!["source_task: topic-apply".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "accepted for apply".to_string(),
    };
    let mut manager = crate::memory::MemoryManager::with_base_dir(base.clone());

    store.upsert(&proposal).unwrap();
    let applied = store.apply("topic-apply", &mut manager).unwrap().unwrap();

    assert_eq!(applied.1, 1);
    let topic_content =
        std::fs::read_to_string(base.join("memory").join("rust-workflow.md")).unwrap_or_default();
    assert!(topic_content.contains("Rust workflow convention"));
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn memory_proposal_review_store_batch_updates_by_source_scope_and_status() {
    let path = std::env::temp_dir().join(format!(
        "priority-agent-memory-proposals-batch-{}.jsonl",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(path.clone());
    let mut closeout = MemoryProposal::from_execution_report(&ExecutionReport {
        task_id: "task-batch-closeout".to_string(),
        objective: "fix parser".to_string(),
        status: ExecutionReportStatus::Success,
        changed_files: vec!["src/parser.rs".to_string()],
        validation_evidence: vec!["cargo test parser: passed".to_string()],
        risks: Vec::new(),
        next_steps: Vec::new(),
        assumptions: Vec::new(),
    });
    closeout.source = "closeout".to_string();
    let background = MemoryProposal {
        task_id: "task-batch-background".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "next_step".to_string(),
            scope: "project".to_string(),
            content: "Next step: rerun parser eval".to_string(),
            evidence: vec!["closeout: parser eval remains".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    store.upsert(&closeout).unwrap();
    store.upsert(&background).unwrap();

    let result = store
        .batch_update_status(
            MemoryProposalBatchFilter {
                source: Some("background".to_string()),
                scope: Some("project".to_string()),
                status: Some(MemoryProposalStatus::Proposed),
                ..Default::default()
            },
            MemoryProposalStatus::Accepted,
            "batch accepted for memory apply",
        )
        .unwrap();

    assert_eq!(result.matched, 1);
    assert_eq!(result.updated, 1);
    assert_eq!(
        store.get("task-batch-background").unwrap().status,
        MemoryProposalStatus::Accepted
    );
    assert_eq!(
        store.get("task-batch-closeout").unwrap().status,
        MemoryProposalStatus::Proposed
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn memory_proposal_review_store_batch_applies_accepted_filtered_proposals() {
    let base = std::env::temp_dir().join(format!(
        "priority-agent-memory-proposals-batch-apply-{}",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
    let mut memory = crate::memory::MemoryManager::with_base_dir(base.clone());
    let accepted_user = MemoryProposal {
        task_id: "batch-apply-user".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Accepted,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "User prefers concise Chinese status updates.".to_string(),
            evidence: vec!["user_statement: prefer concise Chinese updates".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "accepted for batch apply".to_string(),
    };
    let accepted_project = MemoryProposal {
        task_id: "batch-apply-project".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Accepted,
        candidates: vec![MemoryProposalCandidate {
            kind: "workflow_convention".to_string(),
            scope: "project".to_string(),
            content: "Project convention: run cargo test before memory closeout.".to_string(),
            evidence: vec![
                "source_task: batch-apply-project".to_string(),
                "closeout: validation baseline".to_string(),
            ],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "accepted for batch apply".to_string(),
    };
    let proposed = MemoryProposal {
        task_id: "batch-apply-proposed".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "Pending user preference should not apply yet.".to_string(),
            evidence: vec!["user_statement: pending".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "pending review".to_string(),
    };
    store.upsert(&accepted_user).unwrap();
    store.upsert(&accepted_project).unwrap();
    store.upsert(&proposed).unwrap();

    let result = store
        .batch_apply(
            MemoryProposalBatchFilter {
                scope: Some("user".to_string()),
                ..Default::default()
            },
            &mut memory,
        )
        .unwrap();

    assert_eq!(result.matched, 1);
    assert_eq!(result.applied, 1);
    assert_eq!(result.applied_candidates, 1);
    assert_eq!(result.failed, 0);
    assert_eq!(result.proposal_ids, vec!["batch-apply-user".to_string()]);
    assert_eq!(
        store.get("batch-apply-user").unwrap().status,
        MemoryProposalStatus::Applied
    );
    assert_eq!(
        store.get("batch-apply-project").unwrap().status,
        MemoryProposalStatus::Accepted
    );
    assert_eq!(
        store.get("batch-apply-proposed").unwrap().status,
        MemoryProposalStatus::Proposed
    );
    assert!(std::fs::read_to_string(base.join("USER.md"))
        .unwrap_or_default()
        .contains("concise Chinese"));

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn memory_proposal_review_store_rejects_stale_duplicate_and_superseded() {
    let path = std::env::temp_dir().join(format!(
        "priority-agent-memory-proposals-stale-{}.jsonl",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(path.clone());
    let stale = MemoryProposal {
        task_id: "task-stale".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "note".to_string(),
            scope: "project".to_string(),
            content: "Old stale memory candidate".to_string(),
            evidence: vec!["background: old".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    let duplicate = MemoryProposal {
        task_id: "task-duplicate".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "note".to_string(),
            scope: "project".to_string(),
            content: "Duplicate memory candidate".to_string(),
            evidence: vec!["duplicate: existing record".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    let replacement = MemoryProposal {
        task_id: "task-replacement".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "note".to_string(),
            scope: "project".to_string(),
            content: "Replacement memory candidate".to_string(),
            evidence: vec!["closeout: newer".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    store.upsert(&stale).unwrap();
    store.upsert(&duplicate).unwrap();
    store.upsert(&replacement).unwrap();
    let mut stale_record = store.get_record("task-stale").unwrap();
    stale_record.created_at = (chrono::Utc::now() - chrono::Duration::days(45)).to_rfc3339();
    let mut duplicate_record = store.get_record("task-duplicate").unwrap();
    duplicate_record.duplicate_conflict_summary = "duplicate existing memory".to_string();
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(file, "{}", serde_json::to_string(&stale_record).unwrap()).unwrap();
    writeln!(
        file,
        "{}",
        serde_json::to_string(&duplicate_record).unwrap()
    )
    .unwrap();

    let stale_result = store
        .batch_update_status(
            MemoryProposalBatchFilter {
                status: Some(MemoryProposalStatus::Proposed),
                stale_days: Some(30),
                ..Default::default()
            },
            MemoryProposalStatus::Rejected,
            "batch rejected as stale proposal",
        )
        .unwrap();
    let duplicate_result = store
        .batch_update_status(
            MemoryProposalBatchFilter {
                status: Some(MemoryProposalStatus::Proposed),
                duplicate_only: true,
                ..Default::default()
            },
            MemoryProposalStatus::Rejected,
            "batch rejected as duplicate/conflicting",
        )
        .unwrap();
    let superseded = store
        .supersede("task-replacement", "task-duplicate")
        .unwrap()
        .unwrap();

    assert_eq!(stale_result.updated, 1);
    assert_eq!(duplicate_result.updated, 1);
    assert_eq!(
        store.get("task-stale").unwrap().status,
        MemoryProposalStatus::Rejected
    );
    assert_eq!(
        store.get("task-duplicate").unwrap().status,
        MemoryProposalStatus::Rejected
    );
    assert_eq!(superseded.status, MemoryProposalStatus::Rejected);
    assert!(superseded.reason.contains("superseded by memory proposal"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn memory_proposal_review_store_groups_duplicate_and_preference_conflicts() {
    let path = std::env::temp_dir().join(format!(
        "priority-agent-memory-proposals-conflicts-{}.jsonl",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(path.clone());
    let chinese = MemoryProposal {
        task_id: "pref-chinese".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "language: Chinese".to_string(),
            evidence: vec!["user: answer in Chinese".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    let english = MemoryProposal {
        task_id: "pref-english".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "language: English".to_string(),
            evidence: vec!["background: inferred language preference".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    let duplicate = MemoryProposal {
        task_id: "pref-chinese-duplicate".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "language: Chinese".to_string(),
            evidence: vec!["background: same preference".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };

    store.upsert(&chinese).unwrap();
    store.upsert(&english).unwrap();
    store.upsert(&duplicate).unwrap();

    let english_record = store.get_record("pref-english").unwrap();
    assert!(english_record
        .conflict_groups
        .iter()
        .any(|group| group.group_type == "conflict"
            && group.key == "language"
            && group.matches.len() == 2));
    assert!(english_record
        .gate_report
        .iter()
        .any(|gate| gate.gate == "duplicate_conflict" && gate.status == "review_required"));

    let duplicate_record = store.get_record("pref-chinese-duplicate").unwrap();
    assert!(duplicate_record
        .conflict_groups
        .iter()
        .any(|group| group.group_type == "duplicate"
            && group.key == "language"
            && group.matches.len() == 2));
    assert!(duplicate_record
        .duplicate_conflict_summary
        .contains("duplicates=1"));
    let duplicate_only = store
        .batch_update_status(
            MemoryProposalBatchFilter {
                status: Some(MemoryProposalStatus::Proposed),
                duplicate_only: true,
                ..Default::default()
            },
            MemoryProposalStatus::Rejected,
            "batch rejected as duplicate/conflicting",
        )
        .unwrap();
    assert_eq!(duplicate_only.updated, 3);

    let _ = std::fs::remove_file(path);
}

#[test]
fn memory_proposal_review_store_resolves_conflict_by_accepting_keep_and_rejecting_peers() {
    let path = std::env::temp_dir().join(format!(
        "priority-agent-memory-proposals-resolve-conflict-{}.jsonl",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(path.clone());
    let keep = MemoryProposal {
        task_id: "pref-keep".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "language: Chinese".to_string(),
            evidence: vec!["user: answer in Chinese".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    let conflict = MemoryProposal {
        task_id: "pref-conflict".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "language: English".to_string(),
            evidence: vec!["background: inferred language preference".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    let duplicate = MemoryProposal {
        task_id: "pref-duplicate".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "language: Chinese".to_string(),
            evidence: vec!["background: same preference".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    store.upsert(&keep).unwrap();
    store.upsert(&conflict).unwrap();
    store.upsert(&duplicate).unwrap();
    let keep_id = store.get_record("pref-keep").unwrap().id;

    let result = store.resolve_conflict_keep(&keep_id).unwrap().unwrap();

    assert_eq!(result.kept_id, keep_id);
    assert!(result.accepted_keep);
    assert_eq!(result.conflict_groups, 2);
    assert_eq!(result.rejected_ids.len(), 2);
    assert_eq!(
        store.get("pref-keep").unwrap().status,
        MemoryProposalStatus::Accepted
    );
    assert_eq!(
        store.get("pref-conflict").unwrap().status,
        MemoryProposalStatus::Rejected
    );
    assert_eq!(
        store.get("pref-duplicate").unwrap().status,
        MemoryProposalStatus::Rejected
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn memory_proposal_apply_blocks_unresolved_active_conflicts() {
    let base = std::env::temp_dir().join(format!(
        "priority-agent-memory-proposals-apply-conflict-{}",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
    let keep = MemoryProposal {
        task_id: "pref-apply-keep".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Accepted,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "User preference: gex explicitly prefers concise Chinese status updates."
                .to_string(),
            evidence: vec!["user_statement: gex prefers concise Chinese updates".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "accepted for apply".to_string(),
    };
    let conflict = MemoryProposal {
        task_id: "pref-apply-conflict".to_string(),
        source: "background".to_string(),
        status: MemoryProposalStatus::Accepted,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "User preference: gex explicitly prefers concise English status updates."
                .to_string(),
            evidence: vec!["user_statement: gex prefers concise English updates".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "accepted for apply".to_string(),
    };
    store.upsert(&keep).unwrap();
    store.upsert(&conflict).unwrap();
    let mut memory = crate::memory::MemoryManager::with_base_dir(base.clone());

    let error = store
        .apply("pref-apply-keep", &mut memory)
        .expect_err("unresolved active conflict should block durable apply");

    assert!(error.to_string().contains("conflict review is unresolved"));
    assert_eq!(
        store.get("pref-apply-keep").unwrap().status,
        MemoryProposalStatus::Accepted
    );
    assert!(std::fs::read_to_string(base.join("USER.md"))
        .unwrap_or_default()
        .is_empty());

    store
        .resolve_conflict_keep("pref-apply-keep")
        .unwrap()
        .unwrap();
    let (applied, candidate_count) = store
        .apply("pref-apply-keep", &mut memory)
        .unwrap()
        .unwrap();

    assert_eq!(candidate_count, 1);
    assert_eq!(applied.status, MemoryProposalStatus::Applied);
    assert_eq!(
        store.get("pref-apply-conflict").unwrap().status,
        MemoryProposalStatus::Rejected
    );
    assert!(std::fs::read_to_string(base.join("USER.md"))
        .unwrap_or_default()
        .contains("concise Chinese status updates"));

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn memory_proposal_review_store_edit_and_apply_records_review_history() {
    let path = std::env::temp_dir().join(format!(
        "priority-agent-memory-proposals-edit-apply-{}.jsonl",
        uuid::Uuid::new_v4()
    ));
    let base = std::env::temp_dir().join(format!(
        "priority-agent-memory-edit-apply-{}",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryProposalReviewStore::new(path.clone());
    let proposal = MemoryProposal {
        task_id: "edit-apply-proposal".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "decision".to_string(),
            scope: "project".to_string(),
            content: "project_decision: old wording".to_string(),
            evidence: vec!["review: user requested edit".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    store.upsert(&proposal).unwrap();
    let mut memory = crate::memory::MemoryManager::with_base_dir(base.clone());

    let (applied_proposal, applied) = store
        .edit_and_apply(
            "edit-apply-proposal",
            "project_decision: edited wording",
            &mut memory,
        )
        .unwrap()
        .unwrap();

    assert_eq!(applied, 1);
    assert_eq!(applied_proposal.status, MemoryProposalStatus::Applied);
    assert!(applied_proposal.write_performed);
    let record = store.get_record("edit-apply-proposal").unwrap();
    assert_eq!(record.proposal.status, MemoryProposalStatus::Applied);
    assert!(record
        .status_history
        .iter()
        .any(|entry| entry.status == MemoryProposalStatus::Accepted
            && entry.reason.contains("edited candidate content")));
    assert!(memory
        .memory_records()
        .iter()
        .any(|record| record.content.contains("edited wording")));

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn background_memory_review_output_requires_strict_schema() {
    let valid = r#"{
            "candidates": [{
                "kind": "next_step",
                "scope": "project",
                "content": "Next step: run focused parser tests",
                "evidence": ["closeout: parser work remains"]
            }],
            "no_op_reason": null,
            "rejected_observations": []
        }"#;

    let output = BackgroundMemoryReviewOutput::strict_from_json(valid).unwrap();

    assert_eq!(output.candidates.len(), 1);
    assert!(BackgroundMemoryReviewOutput::strict_from_json("not json").is_err());
    assert!(BackgroundMemoryReviewOutput::strict_from_json(
        r#"{"candidates":[],"no_op_reason":null,"rejected_observations":[]}"#
    )
    .is_err());
}

#[test]
fn background_memory_review_worker_creates_proposal_only_candidates() {
    let report = ExecutionReport {
        task_id: "task-background-review".to_string(),
        objective: "finish parser repair".to_string(),
        status: ExecutionReportStatus::Partial,
        changed_files: vec!["src/parser.rs".to_string()],
        validation_evidence: vec!["cargo test parser: failed on edge case".to_string()],
        risks: vec!["edge case remains unresolved".to_string()],
        next_steps: vec!["repair parser edge case".to_string()],
        assumptions: Vec::new(),
    };
    let packet = BackgroundReviewPacket::from_execution_report(&report, &[]);

    let output = BackgroundMemoryReviewWorker::review_execution_report(&packet, &report);
    let proposal = BackgroundMemoryReviewWorker::proposal_from_output(&packet, output);

    assert_eq!(proposal.source, "background");
    assert_eq!(proposal.status, MemoryProposalStatus::Proposed);
    assert_eq!(proposal.write_policy, "review_required");
    assert!(!proposal.write_performed);
    assert!(proposal
        .candidates
        .iter()
        .any(|candidate| candidate.kind == "next_step"));
    assert!(proposal
        .candidates
        .iter()
        .all(|candidate| !candidate.evidence.is_empty()));
}

#[test]
fn serialized_contract_uses_documented_field_names() {
    let route = IntentRouter::new().route("分析项目");
    let bundle = TaskContextBundle::new("分析项目", ".", route, None);
    let contract = bundle.task_contract(&[]);

    let value = serde_json::to_value(&contract).expect("json");

    assert!(value.get("task_type").is_some());
    assert_eq!(value["model_profile"], json!("standard"));
    assert!(value["assumptions"][0].get("source").is_some());
}

#[test]
fn context_pack_stage_tracks_agent_state() {
    let route = IntentRouter::new().route("修改 src/lib.rs");
    let mut bundle = TaskContextBundle::new("修改 src/lib.rs", ".", route, None);
    bundle.agent_state.set_stage(AgentTaskStage::Validate);
    let contract = bundle.task_contract(&[]);

    let pack = bundle.context_pack(&contract);

    assert_eq!(pack.current_stage, "Validate");
}
