use super::*;

#[test]
fn test_memory_path() {
    let path = memory_path();
    assert!(path.to_string_lossy().contains("MEMORY.md"));
}

#[test]
fn test_sanitize_topic() {
    assert_eq!(sanitize_topic("TUI Design").as_deref(), Some("tui-design"));
    assert_eq!(
        sanitize_topic("../Context 管理.md").as_deref(),
        Some("context-管理-md")
    );
    assert_eq!(sanitize_topic("!!!"), None);
}

#[test]
fn test_infer_topic() {
    assert_eq!(
        infer_topic("The TUI should keep Claude-style scroll anchoring.", "note"),
        Some("tui-design")
    );
    assert_eq!(
        infer_topic(
            "Prompt token budget should include memory snapshots.",
            "note"
        ),
        Some("context-management")
    );
    assert_eq!(
        infer_topic("User preference: respond in Chinese", "preference"),
        None
    );
}

#[test]
fn test_memory_document_search_includes_namespaces() {
    let docs = vec![
        MemoryDocument {
            namespace: "user".to_string(),
            path: "USER.md".to_string(),
            content: "language: Chinese".to_string(),
        },
        MemoryDocument {
            namespace: "agent".to_string(),
            path: "memory/agents/reviewer.json".to_string(),
            content: "review_style: strict".to_string(),
        },
    ];

    let results = search_memory_documents(&docs, "strict");
    assert_eq!(results.len(), 1);
    assert!(results[0].starts_with("[agent:memory/agents/reviewer.json]"));
}

#[test]
fn test_memory_conflicts_detect_duplicate_keys() {
    let docs = vec![
        MemoryDocument {
            namespace: "user".to_string(),
            path: "USER.md".to_string(),
            content: "language: Chinese".to_string(),
        },
        MemoryDocument {
            namespace: "topic".to_string(),
            path: "memory/preferences.md".to_string(),
            content: "language: English".to_string(),
        },
    ];

    let conflicts = memory_conflicts(&docs, 8);
    assert_eq!(conflicts.len(), 1);
    assert!(conflicts[0].contains("key 'language'"));
}

#[test]
fn test_memory_decision_counts_from_jsonl() {
    let content = r#"{"status":"accepted"}
{"status":"blocked"}
{"status":"rejected"}
{"status":"accepted"}"#;
    let counts = memory_decision_counts_from_jsonl(content);
    assert_eq!(counts.accepted, 2);
    assert_eq!(counts.blocked, 1);
    assert_eq!(counts.rejected, 1);
}

fn sample_memory_calibration_results() -> Vec<crate::memory::MemoryCalibrationResult> {
    vec![crate::memory::MemoryCalibrationResult {
        id: "sample_project_fact".to_string(),
        expected: crate::memory::MemoryCalibrationExpectation::Accepted,
        actual: crate::memory::MemoryCalibrationActual::Accepted,
        score: Some(0.9),
        passed: true,
        reason: "sample accepted".to_string(),
        rationale: "unit test fixture".to_string(),
    }]
}

fn sample_memory_eval_report() -> crate::memory::MemoryEvalReport {
    crate::memory::MemoryEvalReport {
        total: 1,
        passed: 1,
        failed: 0,
        results: vec![crate::memory::MemoryEvalResult {
            id: "sample_eval".to_string(),
            category: "memory_doctor".to_string(),
            passed: true,
            failure_owner: crate::memory::MemoryEvalFailureOwner::None,
            reason: "unit test fixture".to_string(),
        }],
    }
}

fn sample_memory_snapshot_report() -> crate::memory::MemorySnapshotReport {
    crate::memory::MemorySnapshotReport {
        frozen: false,
        snapshot_id: "memsnap-test".to_string(),
        fingerprint: "test-fingerprint".to_string(),
        scope: "global".to_string(),
        char_count: 16,
        project_chars: 16,
        user_chars: 0,
        memory_file_count: 1,
        memory_file_chars: 16,
        pinned_sources: vec!["MEMORY.md".to_string(), "memory/rust.md".to_string()],
        skipped_record_count: 0,
        skipped_status_count: 0,
        skipped_unsafe_count: 0,
        skipped_stale_count: 0,
        skipped_conflict_count: 0,
    }
}

fn sample_memory_doctor_diagnostics() -> MemoryDoctorDiagnostics {
    MemoryDoctorDiagnostics {
        counts: MemoryDecisionCounts {
            accepted: 1,
            proposed: 0,
            rejected: 0,
            blocked: 0,
        },
        flushes: crate::memory::MemoryFlushSummary {
            completed: 1,
            total: 1,
            ..Default::default()
        },
        operation_journal: Vec::new(),
        proposal_queue: MemoryProposalQueueJson {
            total: 0,
            proposed: 0,
            accepted: 0,
            rejected: 0,
            applied: 0,
            background: 0,
            closeout: 0,
            conflict_groups: 0,
            recent: Vec::new(),
        },
        last_background_review: None,
        last_retrieval_trace: None,
        record_summary: crate::memory::MemoryRecordSummary {
            total: 1,
            accepted: 1,
            ..Default::default()
        },
        store_paths: MemoryStorePathsJson {
            memory_md: "MEMORY.md".to_string(),
            user_md: "USER.md".to_string(),
            memory_dir: "memory".to_string(),
            records_jsonl: "memory/records.jsonl".to_string(),
            operations_jsonl: "memory/operations.jsonl".to_string(),
            proposals_jsonl: "memory/proposals.jsonl".to_string(),
            retrieval_trace_json: "memory/retrieval_trace.json".to_string(),
            decisions_jsonl: "memory/decisions.jsonl".to_string(),
            flush_queue_jsonl: "memory/flush_queue.jsonl".to_string(),
        },
    }
}

#[test]
fn test_format_memory_doctor_includes_conflicts_and_counts() {
    let docs = vec![MemoryDocument {
        namespace: "project".to_string(),
        path: "MEMORY.md".to_string(),
        content: "language: Chinese".to_string(),
    }];
    let lifecycle = default_memory_provider_lifecycle_panel();
    let snapshot = sample_memory_snapshot_report();
    let doctor = format_memory_doctor_with_reports(
        &docs,
        &["- key 'language' conflicts".to_string()],
        &lifecycle,
        &snapshot,
        sample_memory_calibration_results(),
        sample_memory_eval_report(),
        sample_memory_doctor_diagnostics(),
    );
    assert!(doctor.contains("Memory Doctor"));
    assert!(doctor.contains("Documents: 1 total"));
    assert!(doctor.contains("Store paths:"));
    assert!(doctor.contains("records:"));
    assert!(doctor.contains("proposals:"));
    assert!(doctor.contains("Surfaces:"));
    assert!(doctor.contains("Pinned snapshot:"));
    assert!(doctor.contains("Pending memory candidates:"));
    assert!(doctor.contains("Providers:"));
    assert!(doctor.contains("mode="));
    assert!(doctor.contains("Lifecycle:"));
    assert!(doctor.contains("Operation journal:"));
    assert!(doctor.contains("Last background review:"));
    assert!(doctor.contains("Last retrieval trace:"));
    assert!(doctor.contains("Conflicts: 1"));
    assert!(doctor.contains("Quality gates:"));
    assert!(doctor.contains("Calibration:"));
}

#[test]
fn test_memory_doctor_json_includes_calibration_and_gates() {
    let docs = vec![MemoryDocument {
        namespace: "project".to_string(),
        path: "MEMORY.md".to_string(),
        content: "language: Chinese".to_string(),
    }];
    let lifecycle = default_memory_provider_lifecycle_panel();
    let snapshot = sample_memory_snapshot_report();
    let report = memory_doctor_json_with_reports(
        &docs,
        &[],
        &lifecycle,
        &snapshot,
        sample_memory_calibration_results(),
        sample_memory_eval_report(),
        sample_memory_doctor_diagnostics(),
    );
    assert_eq!(report["documents"]["total"].as_u64(), Some(1));
    assert!(report["store_paths"]["records_jsonl"].is_string());
    assert!(report["store_paths"]["proposals_jsonl"].is_string());
    assert!(report["snapshot"]["fingerprint"].is_string());
    assert!(report["proposal_queue"]["recent"].is_array());
    assert!(report.get("last_background_review").is_some());
    assert!(report.get("last_retrieval_trace").is_some());
    assert_eq!(
        report["provider_lifecycle"]["providers"][0]["name"].as_str(),
        Some("local")
    );
    assert!(report["calibration"]["total"].as_u64().unwrap_or(0) >= 1);
    assert!(report["operation_journal"].is_array());
    let accept_threshold = report["quality_gates"]["accept_threshold"]
        .as_f64()
        .unwrap_or_default();
    assert!((accept_threshold - 0.65).abs() < 0.001);
}

#[test]
fn test_last_background_review_uses_latest_background_record() {
    let closeout = crate::engine::task_contract::MemoryProposalReviewRecord {
        id: "closeout-1".to_string(),
        proposal: crate::engine::task_contract::MemoryProposal {
            task_id: "closeout-task".to_string(),
            source: "closeout".to_string(),
            status: crate::engine::task_contract::MemoryProposalStatus::Proposed,
            candidates: Vec::new(),
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "closeout proposal".to_string(),
        },
        created_at: "2026-05-27T00:00:00Z".to_string(),
        updated_at: "2026-05-27T00:00:00Z".to_string(),
        source_session: None,
        source_task: "closeout-task".to_string(),
        source: "closeout".to_string(),
        active_scope: "project".to_string(),
        project_id: Some("project:rust-agent".to_string()),
        project_labels: vec!["project_root:/tmp/rust-agent".to_string()],
        gate_report: Vec::new(),
        duplicate_conflict_summary: String::new(),
        conflict_groups: Vec::new(),
        status_history: Vec::new(),
    };
    let background = crate::engine::task_contract::MemoryProposalReviewRecord {
        id: "background-1".to_string(),
        proposal: crate::engine::task_contract::MemoryProposal {
            task_id: "background-task".to_string(),
            source: "background".to_string(),
            status: crate::engine::task_contract::MemoryProposalStatus::Proposed,
            candidates: vec![crate::engine::task_contract::MemoryProposalCandidate {
                kind: "next_step".to_string(),
                scope: "project".to_string(),
                content: "Continue Phase 7 doctor UX.".to_string(),
                evidence: vec!["closeout: next step".to_string()],
            }],
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason: "background review produced review-required candidates".to_string(),
        },
        created_at: "2026-05-27T00:01:00Z".to_string(),
        updated_at: "2026-05-27T00:01:00Z".to_string(),
        source_session: None,
        source_task: "background-task".to_string(),
        source: "background".to_string(),
        active_scope: "project".to_string(),
        project_id: Some("project:rust-agent".to_string()),
        project_labels: vec!["project_root:/tmp/rust-agent".to_string()],
        gate_report: Vec::new(),
        duplicate_conflict_summary: String::new(),
        conflict_groups: Vec::new(),
        status_history: Vec::new(),
    };

    let review =
        last_background_review_from_records(vec![closeout, background]).expect("background review");

    assert_eq!(review.task_id, "background-task");
    assert_eq!(review.candidates, 1);
    assert_eq!(review.candidate_kinds, vec!["next_step".to_string()]);
    let formatted = format_last_background_review(Some(&review));
    assert!(formatted.contains("Last background review: background-task"));
    assert!(formatted.contains("write_performed=false"));
}

#[test]
fn test_last_memory_retrieval_trace_round_trips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("retrieval_trace.json");
    let mut item = crate::engine::retrieval_context::RetrievalItem::new(
        crate::engine::retrieval_context::RetrievalSource::Memory,
        "Project convention",
        "Use cargo test -q before verified closeout.",
        0.88,
        "memory:records.jsonl#project",
        crate::engine::retrieval_context::TrustLevel::High,
    )
    .with_reason("scope and lexical match");
    item.id = "mem_project_test_gate".to_string();
    let ctx = crate::engine::retrieval_context::RetrievalContext {
        query: "test validation".to_string(),
        policy: crate::engine::intent_router::RetrievalPolicy::Memory,
        created_at: chrono::Utc::now(),
        token_estimate: item.token_estimate,
        items: vec![item],
        memory_trace: Some(crate::engine::retrieval_context::MemoryRetrievalTrace {
            query: "test validation".to_string(),
            selected_records: 1,
            selected_chars: 42,
            max_records: 8,
            max_chars: 4800,
            skipped_unrelated: 2,
            skipped_unsafe: 1,
            skipped_stale_conflict: 1,
            skipped_budget: 0,
            skipped_duplicate: 0,
            per_scope: vec![
                crate::engine::retrieval_context::MemoryRetrievalScopeTrace {
                    scope: "project".to_string(),
                    selected: 1,
                    skipped: 0,
                    cap: 4,
                },
            ],
            decisions: vec![crate::engine::retrieval_context::MemoryRetrievalDecision {
                source: "memory:records.jsonl#project".to_string(),
                scope: "project".to_string(),
                action: "selected".to_string(),
                reason: "scope and lexical match".to_string(),
                score: 88,
                chars: 42,
                score_explanation: Some(
                    crate::engine::retrieval_context::MemoryRetrievalScoreExplanation {
                        lexical_match: 0.9,
                        recency: 0.7,
                        scope_match: 1.0,
                        confidence: 0.8,
                        status: "accepted".to_string(),
                        conflict_penalty: 0.0,
                        user_pinned_bonus: 0.12,
                        final_score: 0.88,
                    },
                ),
            }],
        }),
    };

    write_last_memory_retrieval_trace_to_path(&path, &ctx).expect("write trace");
    let loaded = load_last_memory_retrieval_trace_from_path(&path).expect("load trace");

    assert_eq!(loaded.query, "test validation");
    assert_eq!(loaded.selected_records, 1);
    assert_eq!(loaded.skipped_unsafe, 1);
    assert_eq!(loaded.decisions[0].action, "selected");
    assert_eq!(loaded.selected_items[0].id, "mem_project_test_gate");
    let formatted = format_last_memory_retrieval_trace(Some(&loaded));
    assert!(formatted.contains("Last retrieval trace: query=test validation"));
    assert!(formatted.contains("pinned_bonus=0.12"));
}

#[test]
fn test_agent_memory_json_formats_as_key_values() {
    let content = r#"[{"key":"review_style","value":"strict","created_at":1,"updated_at":1,"tags":["review"]}]"#;
    let formatted = format_agent_memory_content(content);
    assert!(formatted.contains("review_style: strict [review]"));
}
