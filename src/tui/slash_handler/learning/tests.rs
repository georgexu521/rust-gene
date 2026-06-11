use super::*;

#[test]
fn safe_skill_dir_name_rejects_paths() {
    assert!(is_safe_skill_dir_name("rust-debug"));
    assert!(is_safe_skill_dir_name("rust_debug.v1"));
    assert!(!is_safe_skill_dir_name("../rust-debug"));
    assert!(!is_safe_skill_dir_name("rust/debug"));
    assert!(!is_safe_skill_dir_name(".."));
}

#[test]
fn disabled_skill_backups_filters_and_sorts_latest_first() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("lint.disabled-20260101000000")).unwrap();
    std::fs::create_dir_all(dir.path().join("lint.disabled-20260201000000")).unwrap();
    std::fs::create_dir_all(dir.path().join("other.disabled-20260101000000")).unwrap();
    std::fs::create_dir_all(dir.path().join("lint")).unwrap();

    let backups = disabled_skill_backups(dir.path(), Some("lint"));
    assert_eq!(backups.len(), 2);
    assert_eq!(backups[0].backup_name, "lint.disabled-20260201000000");
    assert_eq!(backups[0].skill_name, "lint");

    let latest = resolve_disabled_skill_backup(dir.path(), "lint", None).unwrap();
    assert_eq!(latest.backup_name, "lint.disabled-20260201000000");
}

#[test]
fn memory_proposal_detail_shows_review_fields() {
    use crate::engine::task_contract::{
        MemoryProposal, MemoryProposalCandidate, MemoryProposalConflictGroup,
        MemoryProposalConflictMatch, MemoryProposalGateDecision, MemoryProposalReviewRecord,
        MemoryProposalStatus, MemoryProposalStatusHistoryEntry,
    };

    let proposal = MemoryProposal {
        task_id: "task-memory-review".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "successful_fix".to_string(),
            scope: "project".to_string(),
            content: "Completed parser fix with cargo test parser passing".to_string(),
            evidence: vec!["validation: cargo test parser passed".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    let record = MemoryProposalReviewRecord {
        id: proposal.task_id.clone(),
        proposal,
        created_at: "2026-05-27T00:00:00Z".to_string(),
        updated_at: "2026-05-27T00:01:00Z".to_string(),
        source_session: Some("session-1".to_string()),
        source_task: "task-memory-review".to_string(),
        source: "closeout".to_string(),
        active_scope: "project".to_string(),
        project_id: Some("project:rust-agent".to_string()),
        project_labels: vec!["project_root:/tmp/rust-agent".to_string()],
        gate_report: vec![MemoryProposalGateDecision {
            gate: "write_policy".to_string(),
            candidate_index: None,
            status: "passed".to_string(),
            reason: "write_policy=review_required".to_string(),
        }],
        duplicate_conflict_summary: "not_checked".to_string(),
        conflict_groups: vec![MemoryProposalConflictGroup {
            group_type: "conflict".to_string(),
            key: "language".to_string(),
            scope: "user".to_string(),
            kind: "user_preference".to_string(),
            matches: vec![MemoryProposalConflictMatch {
                proposal_id: "task-memory-review".to_string(),
                candidate_index: 0,
                status: MemoryProposalStatus::Proposed,
                source: "closeout".to_string(),
                value: "Chinese".to_string(),
                content: "language: Chinese".to_string(),
            }],
            resolution_hint: "prefer newer explicit user correction".to_string(),
        }],
        status_history: vec![MemoryProposalStatusHistoryEntry {
            at: "2026-05-27T00:00:00Z".to_string(),
            status: MemoryProposalStatus::Proposed,
            reason: "created".to_string(),
        }],
    };

    let detail = format_memory_proposal_detail(&record);

    assert!(detail.contains("Review state: pending user review; accept before apply"));
    assert!(detail.contains("ID: task-memory-review"));
    assert!(detail.contains("Affects future sessions: after accept/apply only"));
    assert!(detail.contains("Why this was suggested: candidate memory requires review"));
    assert!(detail.contains("Source session: session-1"));
    assert!(detail.contains("Active scope: project"));
    assert!(detail.contains("Project: project:rust-agent"));
    assert!(detail.contains("evidence 1: validation: cargo test parser passed"));
    assert!(detail.contains("Gate report:"));
    assert!(detail.contains("write_policy [proposal]: passed"));
    assert!(detail.contains("Duplicate/conflict: not_checked"));
    assert!(detail.contains("Conflict groups:"));
    assert!(detail.contains("key=language"));
    assert!(detail.contains("Status history:"));
}

#[test]
fn memory_proposal_filter_parses_blocked_flag() {
    let filter = parse_memory_proposal_batch_filter(&[
        "--blocked",
        "--scope",
        "project",
        "--project",
        "rust-agent",
    ]);

    assert!(filter.blocked_only);
    assert_eq!(filter.scope.as_deref(), Some("project"));
    assert_eq!(filter.project.as_deref(), Some("rust-agent"));
}

#[test]
fn memory_proposal_conflict_panel_shows_resolution_command() {
    use crate::engine::task_contract::{
        MemoryProposal, MemoryProposalCandidate, MemoryProposalConflictGroup,
        MemoryProposalConflictMatch, MemoryProposalReviewRecord, MemoryProposalStatus,
    };

    let proposal = MemoryProposal {
        task_id: "pref-keep".to_string(),
        source: "closeout".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates: vec![MemoryProposalCandidate {
            kind: "user_preference".to_string(),
            scope: "user".to_string(),
            content: "language: Chinese".to_string(),
            evidence: vec!["user: Chinese".to_string()],
        }],
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    };
    let record = MemoryProposalReviewRecord {
        id: proposal.task_id.clone(),
        proposal,
        created_at: "2026-05-27T00:00:00Z".to_string(),
        updated_at: "2026-05-27T00:01:00Z".to_string(),
        source_session: None,
        source_task: "pref-keep".to_string(),
        source: "closeout".to_string(),
        active_scope: "user".to_string(),
        project_id: Some("project:rust-agent".to_string()),
        project_labels: vec!["project_root:/tmp/rust-agent".to_string()],
        gate_report: Vec::new(),
        duplicate_conflict_summary: "conflicts=1".to_string(),
        conflict_groups: vec![MemoryProposalConflictGroup {
            group_type: "conflict".to_string(),
            key: "language".to_string(),
            scope: "user".to_string(),
            kind: "user_preference".to_string(),
            matches: vec![MemoryProposalConflictMatch {
                proposal_id: "pref-keep".to_string(),
                candidate_index: 0,
                status: MemoryProposalStatus::Proposed,
                source: "closeout".to_string(),
                value: "Chinese".to_string(),
                content: "language: Chinese".to_string(),
            }],
            resolution_hint: "prefer newer explicit user correction".to_string(),
        }],
        status_history: Vec::new(),
    };

    let panel = format_memory_proposal_conflict_panel(&[record]);

    assert!(panel.contains("Memory Proposal Conflicts"));
    assert!(panel.contains("key=language"));
    assert!(panel.contains("status=proposed source=closeout evidence=1"));
    assert!(panel.contains("content=language: Chinese"));
    assert!(panel.contains("/memory-proposals show pref-keep"));
    assert!(panel.contains("/memory-proposals resolve-conflict <keep-task-id>"));
}

#[test]
fn memory_proposal_batch_apply_result_shows_applied_candidates_and_failures() {
    let result = crate::engine::task_contract::MemoryProposalBatchApply {
        matched: 3,
        applied: 2,
        applied_candidates: 4,
        failed: 1,
        proposal_ids: vec!["proposal-a".to_string(), "proposal-b".to_string()],
        failures: vec!["proposal-c: missing evidence".to_string()],
    };

    let output = format_memory_proposal_batch_apply_result(&result);

    assert!(output.contains("Batch applied memory proposals"));
    assert!(output.contains("- matched: 3"));
    assert!(output.contains("- applied: 2"));
    assert!(output.contains("- candidates applied: 4"));
    assert!(output.contains("- failed: 1"));
    assert!(output.contains("proposal-a, proposal-b"));
    assert!(output.contains("proposal-c: missing evidence"));
}

fn test_improvement_proposal() -> crate::engine::improvement::ImprovementProposal {
    crate::engine::improvement::ImprovementProposal {
        id: "imp_learning_test".to_string(),
        trigger_event_ids: vec![1, 2],
        target: crate::engine::improvement::ImprovementTarget::ToolGuidance,
        proposed_change:
            "Add guidance for repeated bash failures: inspect arguments before retrying."
                .to_string(),
        expected_benefit: "Reduce repeated tool failures.".to_string(),
        risk: crate::engine::intent_router::RiskLevel::Medium,
        validation: vec!["Run tool guidance evalset.".to_string()],
        eval_status: crate::engine::improvement::ProposalEvalStatus::Pending,
        eval_summary: None,
        evalset_bindings: Vec::new(),
        status: crate::engine::improvement::ProposalStatus::Accepted,
        evidence: vec!["learning event showed repeated bash failures".to_string()],
        rollback_plan: "Deactivate applied guidance.".to_string(),
        applied_ref: None,
        rollback_ref: None,
        created_at: "2026-05-28T00:00:00Z".to_string(),
        updated_at: "2026-05-28T00:00:00Z".to_string(),
    }
}

#[test]
fn improvement_eval_blocks_apply_without_bound_evalset() {
    let proposal = test_improvement_proposal();

    let eval = evaluate_improvement_proposal_for_apply(&proposal);

    assert!(!eval.passed);
    assert!(eval.summary.contains("missing bound evalset"));
    assert!(eval.summary.contains("failure_owner=framework"));
}

#[test]
fn improvement_detail_shows_applied_guidance_and_effect_summary() {
    let dir = tempfile::tempdir().unwrap();
    let store =
        crate::engine::improvement::ImprovementStore::new(dir.path().join("improvements.jsonl"));
    let mut proposal = test_improvement_proposal();
    proposal.evalset_bindings = vec!["tool-guidance-smoke".to_string()];
    proposal.eval_status = crate::engine::improvement::ProposalEvalStatus::Passed;
    proposal.eval_summary = Some("eval passed".to_string());
    store.upsert(&proposal).unwrap();
    store
        .update_status(
            &proposal.id,
            crate::engine::improvement::ProposalStatus::Applied,
        )
        .unwrap();
    store
        .effect_store()
        .record(
            &proposal.id,
            "tool-guidance-smoke",
            "run-1",
            crate::engine::improvement::ImprovementEffectOutcome::Positive,
            "none",
            "reduced repeated tool failures",
        )
        .unwrap();
    let applied = store.get(&proposal.id).unwrap();

    let detail = format_improvement_detail_with_state(&applied, &store);

    assert!(detail.contains("Applied guidance:"));
    assert!(detail.contains("status=Active"));
    assert!(detail.contains("Effect summary:"));
    assert!(detail.contains("positive=1"));
}

#[test]
fn applied_guidance_panel_and_effect_panel_show_operational_state() {
    let mut proposal = test_improvement_proposal();
    proposal.evalset_bindings = vec!["tool-guidance-smoke".to_string()];
    proposal.eval_status = crate::engine::improvement::ProposalEvalStatus::Passed;
    let guidance = crate::engine::improvement::AppliedGuidanceRecord::from_proposal(
        &proposal,
        "2026-05-28T00:00:00Z".to_string(),
    );
    let list = format_applied_guidance_list(&[guidance]);

    assert!(list.contains("Active Applied Guidance (1 total)"));
    assert!(list.contains("activation=ToolContractHint"));
    assert!(list.contains("evalsets=tool-guidance-smoke"));

    let summary = crate::engine::improvement::ImprovementEffectSummary {
        proposal_id: proposal.id.clone(),
        total: 1,
        positive: 0,
        neutral: 0,
        negative: 1,
        rollback_recommended: false,
        recent: vec![crate::engine::improvement::ImprovementEffectRecord {
            id: "effect-1".to_string(),
            proposal_id: proposal.id,
            evalset: "tool-guidance-smoke".to_string(),
            run_id: "run-1".to_string(),
            outcome: crate::engine::improvement::ImprovementEffectOutcome::Negative,
            failure_owner: "framework".to_string(),
            reason: "regressed validation".to_string(),
            created_at: "2026-05-28T00:01:00Z".to_string(),
        }],
    };
    let effect = format_improvement_effect_summary(&summary);

    assert!(effect.contains("negative=1"));
    assert!(effect.contains("owner=framework"));
    assert!(effect.contains("regressed validation"));
}

#[test]
fn compact_inline_truncates_and_joins() {
    assert_eq!(compact_inline("hello world", 100), "hello world");
    assert_eq!(
        compact_inline("line1\nline2\nline3", 100),
        "line1 line2 line3"
    );
    let long = "a".repeat(200);
    let result = compact_inline(&long, 10);
    assert!(result.ends_with("..."));
    assert!(result.len() <= 13); // 10 chars + "..."
}

#[test]
fn count_debug_values_counts_distinct() {
    let items = ["a", "b", "a", "c", "b", "a"];
    let counts: Vec<(String, usize)> = count_debug_values::<&&str>(items.iter());
    assert_eq!(counts.len(), 3);
    let a = counts.iter().find(|c| c.0 == "\"a\"").unwrap();
    assert_eq!(a.1, 3);
}

#[test]
fn format_counts_empty_returns_none() {
    assert_eq!(format_counts(&[]), "none");
}

#[test]
fn format_counts_formats_entries() {
    let counts = vec![("foo".to_string(), 5), ("bar".to_string(), 2)];
    let result = format_counts(&counts);
    assert!(result.contains("foo=5"));
    assert!(result.contains("bar=2"));
}

#[test]
fn is_evolution_learning_event_matches_keywords() {
    use crate::session_store::LearningEventRecord;
    let base = LearningEventRecord {
        id: 1,
        session_id: "s1".into(),
        kind: "improvement_proposal".into(),
        source: "evolution".into(),
        summary: "test".into(),
        confidence: 0.9,
        payload: serde_json::json!({}),
        created_at: "2026-01-01".into(),
    };
    assert!(is_evolution_learning_event(&base));

    let skill = LearningEventRecord {
        kind: "skill_promotion".into(),
        ..base.clone()
    };
    assert!(is_evolution_learning_event(&skill));

    let evo = LearningEventRecord {
        kind: "evolution_status".into(),
        ..base.clone()
    };
    assert!(is_evolution_learning_event(&evo));

    let other = LearningEventRecord {
        kind: "tool_execution".into(),
        ..base
    };
    assert!(!is_evolution_learning_event(&other));
}

#[test]
fn format_learning_event_detail_includes_fields() {
    use crate::session_store::LearningEventRecord;
    let event = LearningEventRecord {
        id: 42,
        session_id: "s1".into(),
        kind: "improvement".into(),
        source: "closeout".into(),
        summary: "test summary".into(),
        confidence: 0.8,
        payload: serde_json::json!({"key": "value"}),
        created_at: "2026-01-01T00:00:00Z".into(),
    };
    let detail = format_learning_event_detail(&event);
    assert!(detail.contains("Learning Event #42"));
    assert!(detail.contains("Kind: improvement"));
    assert!(detail.contains("Source: closeout"));
}

#[test]
fn format_experience_event_includes_fields() {
    use crate::session_store::LearningEventRecord;
    let event = LearningEventRecord {
        id: 7,
        session_id: "s1".into(),
        kind: "experience".into(),
        source: "evaluation".into(),
        summary: "test".into(),
        confidence: 0.5,
        payload: serde_json::json!({"experience": {"workflow": "code-change", "outcome": "success", "tool_calls": 3}}),
        created_at: "2026-01-01T00:00:00Z".into(),
    };
    let detail = format_experience_event(&event);
    assert!(detail.contains("Experience #7"));
    assert!(detail.contains("Kind: experience"));
}

// ── latest_*_label tests ─────────────────────────────────

use crate::engine::trace::TurnTrace;

fn empty_trace() -> TurnTrace {
    TurnTrace::new("session-test".to_string(), 1, "test")
}

#[test]
fn latest_resource_policy_label_returns_none_for_empty_trace() {
    assert_eq!(latest_resource_policy_label(&empty_trace()), None);
}

#[test]
fn latest_contract_state_label_returns_default_for_empty() {
    let label = latest_contract_state_label(&empty_trace());
    assert!(label.contains("standalone") || label.contains("none"));
}

#[test]
fn latest_closeout_label_returns_none_for_empty_trace() {
    assert_eq!(latest_closeout_label(&empty_trace()), None);
}

#[test]
fn latest_acceptance_label_returns_none_for_empty_trace() {
    assert_eq!(latest_acceptance_label(&empty_trace()), None);
}

#[test]
fn latest_retrieval_context_label_returns_none_for_empty() {
    assert_eq!(latest_retrieval_context_label(&empty_trace()), None);
}

#[test]
fn latest_reflection_label_returns_none_for_empty() {
    assert_eq!(latest_reflection_label(&empty_trace()), None);
}

#[test]
fn latest_stage_validation_label_returns_none_for_empty() {
    assert_eq!(latest_stage_validation_label(&empty_trace()), None);
}

#[test]
fn latest_workflow_plan_label_returns_none_for_empty() {
    assert_eq!(latest_workflow_plan_label(&empty_trace()), None);
}

#[test]
fn latest_guided_debugging_label_returns_none_for_empty() {
    assert_eq!(latest_guided_debugging_label(&empty_trace()), None);
}

#[test]
fn goal_drift_count_label_returns_none_for_empty() {
    let label = goal_drift_count_label(&empty_trace());
    assert_eq!(label, "none");
}

#[test]
fn format_goal_drift_report_has_header() {
    let report = format_goal_drift_report(&empty_trace(), 10);
    assert!(report.contains("Goal Drift"));
}
