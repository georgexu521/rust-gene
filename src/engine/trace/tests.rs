use super::*;

#[test]
fn trace_store_retains_latest_entries() {
    let store = TraceStore::new(2);
    store.push(TurnTrace::new("s1", 1, "one"));
    store.push(TurnTrace::new("s1", 2, "two"));
    store.push(TurnTrace::new("s1", 3, "three"));

    assert_eq!(store.len(), 2);
    assert_eq!(store.latest().unwrap().turn_index, 3);
    assert_eq!(store.recent(2)[1].turn_index, 2);
}

#[test]
fn trace_summary_includes_events() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "hello"));
    collector.record(TraceEvent::ToolStarted {
        tool: "bash".to_string(),
        call_id: "abcdef123".to_string(),
        parallel: false,
        pre_executed: false,
    });
    let trace = collector.finish(TurnStatus::Completed);
    let summary = format_trace_summary(&trace, 10);
    assert!(summary.contains("tool.start"));
    assert!(summary.contains("bash"));
}

#[test]
fn trace_summary_handles_route_and_context_diagnostics() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "fix code"));
    collector.record(TraceEvent::RouteCandidateEvaluated {
        intent: "code_change".to_string(),
        confidence: 0.5,
        matched_signals: vec!["code_change".to_string()],
        reason: "heuristic matched".to_string(),
    });
    collector.record(TraceEvent::RouteCompetitionSummary {
        selected_intent: "CodeChange".to_string(),
        selected_confidence: 0.8,
        runner_up_intent: "debug".to_string(),
        runner_up_confidence: 0.5,
        candidate_count: 2,
        delta: 0.3,
    });
    collector.record(TraceEvent::ContextTokenBreakdown {
        total_chars: 100,
        system_chars: 20,
        history_chars: 30,
        tool_result_chars: 10,
        dynamic_zone_chars: 15,
        last_user_chars: 25,
    });

    let trace = collector.finish(TurnStatus::Completed);
    let summary = format_trace_summary(&trace, 10);
    assert!(summary.contains("route candidate intent=code_change"));
    assert!(summary.contains("route competition selected=CodeChange"));
    assert!(summary.contains("context tokens chars total=100"));
}

#[test]
fn trace_summary_includes_control_loop_diagnostic() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "fix code"));
    collector.record(TraceEvent::ActionDecisionEvaluated {
        tool: "file_edit".to_string(),
        call_id: "call_edit".to_string(),
        stage: "Edit".to_string(),
        value: 8,
        risk: 4,
        uncertainty_reduction: 2,
        cost: 2,
        reversibility: 7,
        scope_fit: 8,
        action_score: 14,
        formula_stage: "implementation".to_string(),
        formula_version: "action_score.v1".to_string(),
        phase_aligned: true,
        mutates_workspace: true,
        broad_shell: false,
        modifiers: Vec::new(),
        requires_confirmation: false,
        reason: "scoped edit".to_string(),
    });
    collector.record(TraceEvent::PermissionResolved {
        tool: "file_edit".to_string(),
        call_id: "call_edit".to_string(),
        approved: true,
        source: Some("user_once_allow".to_string()),
        decision: Some("allow_once".to_string()),
        persistence_scope: None,
        rule_pattern: None,
        persisted_path: None,
        review: None,
    });
    collector.record(TraceEvent::ToolCompleted {
        tool: "file_edit".to_string(),
        call_id: "call_edit".to_string(),
        success: true,
        duration_ms: Some(12),
        output_chars: 24,
    });
    collector.record(TraceEvent::StopCheckEvaluated {
        status: "continue".to_string(),
        reason: "no_issue".to_string(),
        stage: "Validate".to_string(),
        terminal_status: None,
        action: "continue".to_string(),
        no_code_progress_rounds: 0,
        action_checkpoint_active: false,
        summary: "continue after edit".to_string(),
        evidence_items: 0,
        failure_type: None,
        recovery_plan_id: None,
        rollback_recommended: false,
        next_action: None,
    });
    collector.record(TraceEvent::VerificationCompleted {
        changed_files: 1,
        passed: true,
        check_passed: true,
        tests_passed: true,
        review_passed: true,
        failed_commands: Vec::new(),
    });
    collector.record(TraceEvent::FinalCloseoutPrepared {
        status: "passed".to_string(),
        terminal_status: Some("completed".to_string()),
        stop_reason: None,
        stop_action: None,
        failure_type: None,
        recovery_plan_id: None,
        rollback_status: None,
        changed_files: 1,
        validation_items: 1,
        tool_records: 1,
        tool_evidence: None,
        verification_proof_status: Some("verified".to_string()),
        verification_proof_summary: Some("validation passed".to_string()),
        verification_proof_kind_summary: Some("command_passed".to_string()),
        verification_proof_support_status: Some("verified".to_string()),
        verification_proof_support_summary: Some("verified by command_passed".to_string()),
        verification_proof_supports_verified: Some(true),
        verification_proof_residual_risk: Some(false),
        acceptance_items: 1,
        residual_risks: 0,
    });

    let trace = collector.finish(TurnStatus::Completed);
    let diagnostic = control_loop_diagnostic(&trace);
    let phase = |name: &str| {
        diagnostic
            .phases
            .iter()
            .find(|phase| phase.phase == name)
            .expect("phase exists")
    };

    assert_eq!(phase("context").events, 1);
    assert_eq!(
        phase("decision").latest_label.as_deref(),
        Some("action.decision")
    );
    assert_eq!(
        phase("permission").latest_label.as_deref(),
        Some("permission.resolve")
    );
    assert_eq!(
        phase("tool_execution").latest_label.as_deref(),
        Some("tool.done")
    );
    assert_eq!(
        phase("state_update").latest_label.as_deref(),
        Some("stop.check")
    );
    assert_eq!(
        phase("verification").latest_label.as_deref(),
        Some("verify.done")
    );
    assert_eq!(phase("closeout").latest_label.as_deref(), Some("closeout"));

    let summary = format_trace_summary(&trace, 20);
    assert!(summary.contains("Control Loop:"));
    assert!(summary.contains("context=1 latest=prompt"));
    assert!(summary.contains("decision=1 latest=action.decision"));
    assert!(summary.contains("tool_execution=1 latest=tool.done"));
    assert!(summary.contains("closeout=1 latest=closeout"));
}

#[test]
fn latest_memory_proposal_summary_reports_review_state() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "fix code"));
    collector.record(TraceEvent::MemoryProposalPrepared {
        task_id: "task-123456".to_string(),
        status: "proposed".to_string(),
        candidates: 1,
        candidate_kinds: vec!["successful_fix".to_string()],
        evidence_items: 2,
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: "candidate memory requires review before persistence".to_string(),
    });

    let trace = collector.finish(TurnStatus::Completed);
    let summary = latest_memory_proposal_summary(&trace).expect("memory proposal summary");

    assert!(summary.contains("proposed candidates=1"));
    assert!(summary.contains("kinds=successful_fix"));
    assert!(summary.contains("write_policy=review_required"));
    assert!(summary.contains("wrote=false"));
}

#[test]
fn trace_summary_includes_action_review_counts() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "review actions"));
    collector.record(TraceEvent::ActionReviewed {
        tool: "file_read".to_string(),
        call_id: "call_read".to_string(),
        decision: "allow".to_string(),
        reason: "safe_to_execute".to_string(),
        permission: Some("Allow".to_string()),
        scope_allowed: true,
        budget_allowed: true,
        checkpoint: "not_needed".to_string(),
        network: "none".to_string(),
        external_effect: "none".to_string(),
        recovery: "use observation".to_string(),
    });
    collector.record(TraceEvent::ActionReviewed {
        tool: "file_edit".to_string(),
        call_id: "call_edit".to_string(),
        decision: "revise".to_string(),
        reason: "checkpoint_required".to_string(),
        permission: Some("Allow".to_string()),
        scope_allowed: true,
        budget_allowed: true,
        checkpoint: "required_but_missing".to_string(),
        network: "none".to_string(),
        external_effect: "local_workspace_mutation".to_string(),
        recovery: "inspect first".to_string(),
    });
    collector.record(TraceEvent::ActionReviewed {
        tool: "git".to_string(),
        call_id: "call_git".to_string(),
        decision: "deny".to_string(),
        reason: "permission_denied".to_string(),
        permission: Some("Deny".to_string()),
        scope_allowed: true,
        budget_allowed: true,
        checkpoint: "unavailable".to_string(),
        network: "remote_service".to_string(),
        external_effect: "git_remote_publication".to_string(),
        recovery: "choose a safer action".to_string(),
    });
    collector.record(TraceEvent::ActionReviewed {
        tool: "bash".to_string(),
        call_id: "call_bash".to_string(),
        decision: "ask_user".to_string(),
        reason: "permission_required".to_string(),
        permission: Some("Ask".to_string()),
        scope_allowed: true,
        budget_allowed: true,
        checkpoint: "not_needed".to_string(),
        network: "none".to_string(),
        external_effect: "none".to_string(),
        recovery: "wait for approval".to_string(),
    });

    let trace = collector.finish(TurnStatus::Completed);
    let review_summary = action_review_trace_summary(&trace).expect("reviews present");

    assert_eq!(review_summary.total, 4);
    assert_eq!(review_summary.allowed, 1);
    assert_eq!(review_summary.ask_user, 1);
    assert_eq!(review_summary.denied, 1);
    assert_eq!(review_summary.revised, 1);
    assert_eq!(review_summary.checkpoint_required, 2);

    let summary = format_trace_summary(&trace, 20);
    assert!(summary.contains("Action Reviews: total=4"));
    assert!(summary.contains("allow=1 ask_user=1 denied=1 revised=1"));
    assert!(summary.contains("checkpoint_required=2"));
    assert!(summary.contains("latest=bash:ask_user/permission_required"));

    let recent_line = format_trace_recent_line(&trace);
    assert!(recent_line.contains("action_reviews=total=4"));
    assert!(recent_line.contains("latest=bash:ask_user/permission_required"));
}

#[test]
fn trace_summary_includes_scoring_summary() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "score this"));
    collector.record(TraceEvent::WorkflowPlanProgress {
        total_steps: 2,
        completed_steps: 0,
        active_step: Some("inspect target".to_string()),
        top_priority: None,
        top_importance_score: Some(0.82),
        top_weight_share: Some(0.55),
        weight_source: Some("workflow_contract".to_string()),
        reweighted: true,
    });
    collector.record(TraceEvent::ActionDecisionEvaluated {
        tool: "file_read".to_string(),
        call_id: "call_read".to_string(),
        stage: "Inspect".to_string(),
        value: 6,
        risk: 1,
        uncertainty_reduction: 8,
        cost: 1,
        reversibility: 9,
        scope_fit: 7,
        action_score: 18,
        formula_stage: "diagnosis".to_string(),
        formula_version: "action_score.v1".to_string(),
        phase_aligned: true,
        mutates_workspace: false,
        broad_shell: false,
        modifiers: Vec::new(),
        requires_confirmation: false,
        reason: "read reduces uncertainty".to_string(),
    });
    collector.record(TraceEvent::MemoryRecallScored {
        item_count: 3,
        injected: 1,
        available: 1,
        omitted: 1,
        conflict_capped: 0,
        top_score: 0.78,
        budget_exhausted: false,
        policy: "Project".to_string(),
    });
    collector.record(TraceEvent::MemoryWriteScored {
        candidate_id: "mem_1".to_string(),
        kind: "workflow_convention".to_string(),
        status: "accepted".to_string(),
        score: 0.72,
        threshold: 0.65,
        explicit: true,
        duplication: 0.10,
        reason: "stable convention".to_string(),
    });
    collector.record(TraceEvent::MemoryKeepScored {
        record_id: "MEMORY.md".to_string(),
        kind: "project".to_string(),
        action: "KeepActive".to_string(),
        score: 0.81,
        contradiction_risk: 0.0,
        redundancy: 0.1,
        reason: "keep_score=0.81".to_string(),
    });

    let trace = collector.finish(TurnStatus::Completed);
    let summary = format_trace_summary(&trace, 20);

    assert!(summary.contains("Scoring:"));
    assert!(summary.contains("action=file_read score=18"));
    assert!(summary.contains("memory_recall=items=3 injected=1"));
    assert!(summary.contains("memory_write=kind=workflow_convention status=accepted"));
    assert!(summary.contains("memory_keep=MEMORY.md kind=project action=KeepActive"));
    assert!(summary.contains("workflow=step=inspect target importance=0.82"));
}

#[test]
fn trace_summary_includes_mcp_resource_access() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "read mcp resource"));
    collector.record(TraceEvent::McpResourceAccessed {
        server: "filesystem".to_string(),
        uri: "file:///tmp/a.txt".to_string(),
        action: "read".to_string(),
        success: true,
        content_chars: 12,
    });

    let trace = collector.finish(TurnStatus::Completed);
    let summary = format_trace_summary(&trace, 10);
    assert!(summary.contains("mcp.resource"));
    assert!(summary.contains("filesystem"));
    assert!(summary.contains("file:///tmp/a.txt"));
}

#[test]
fn trace_summary_includes_provider_protocol_facts_on_api_start() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "ask provider"));
    collector.record(TraceEvent::ApiRequestStarted {
        iteration: 1,
        model: "MiniMax-M2.7".to_string(),
        tools: 4,
        provider_family: Some("minimax".to_string()),
        nonstreaming_tools_required: true,
        tool_result_adjacency_required: true,
    });

    let trace = collector.finish(TurnStatus::Completed);
    let summary = format_trace_summary(&trace, 10);
    assert!(summary.contains("provider=minimax"));
    assert!(summary.contains("nonstreaming_tools=true"));
    assert!(summary.contains("tool_adjacency=true"));
}

#[test]
fn trace_summary_includes_remote_bridge_action() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "run remote task"));
    collector.record(TraceEvent::RemoteBridgeAction {
        tool: "remote_trigger".to_string(),
        call_id: "remote_call_123".to_string(),
        action: "run".to_string(),
        target: Some("session-1".to_string()),
        risk: "high".to_string(),
        permission_hint: "remote trigger action=run target=session-1 risk=high".to_string(),
        success: false,
        error_code: Some("unavailable".to_string()),
    });

    let trace = collector.finish(TurnStatus::Completed);
    let summary = format_trace_summary(&trace, 10);
    assert!(summary.contains("remote.bridge"));
    assert!(summary.contains("remote_trigger"));
    assert!(summary.contains("risk=high"));
    assert!(summary.contains("error=unavailable"));
}

#[test]
fn trace_summary_includes_closeout_tool_record_count() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "finish task"));
    collector.record(TraceEvent::FinalCloseoutPrepared {
        status: "passed".to_string(),
        terminal_status: Some("completed".to_string()),
        stop_reason: None,
        stop_action: None,
        failure_type: None,
        recovery_plan_id: None,
        rollback_status: None,
        changed_files: 1,
        validation_items: 2,
        tool_records: 3,
        tool_evidence: Some("tool evidence: records=3 completed=3".to_string()),
        verification_proof_status: Some("verified".to_string()),
        verification_proof_summary: Some("validation passed 1/1 current checks".to_string()),
        verification_proof_kind_summary: Some("command_passed".to_string()),
        verification_proof_support_status: Some("verified".to_string()),
        verification_proof_support_summary: Some("verified by command_passed".to_string()),
        verification_proof_supports_verified: Some(true),
        verification_proof_residual_risk: Some(false),
        acceptance_items: 1,
        residual_risks: 0,
    });

    let trace = collector.finish(TurnStatus::Completed);
    let summary = format_trace_summary(&trace, 10);
    assert!(summary.contains("tool_records=3"));
    assert!(summary.contains("tool_evidence=tool evidence: records=3"));
    assert!(summary.contains("proof=verified"));
    assert!(summary.contains("Tool Record Evidence: status=passed records=3"));
    assert!(summary.contains("evidence=tool evidence: records=3"));
    assert_eq!(latest_tool_record_count(&trace), Some(3));
    assert!(format_trace_recent_line(&trace).contains("tool_records=3"));
}

#[test]
fn trace_recent_line_marks_missing_tool_records_zero() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "inspect task"));
    let trace = collector.finish(TurnStatus::Completed);

    assert_eq!(latest_tool_record_count(&trace), None);
    assert_eq!(latest_tool_record_evidence_summary(&trace), None);
    assert!(format_trace_recent_line(&trace).contains("tool_records=0"));
    assert!(!format_trace_recent_line(&trace).contains("action_reviews="));
}

#[test]
fn trace_summary_includes_runtime_diet_report() {
    let collector = TraceCollector::new(TurnTrace::new("s1", 1, "make a small edit"));
    collector.record(TraceEvent::RuntimeDietReport {
        prompt_tokens: 1_200,
        tool_schema_tokens: 320,
        total_request_tokens: 1_520,
        max_context_tokens: Some(8_000),
        remaining_context_tokens: Some(6_480),
        tool_result_chars: 240,
        tool_result_tokens: 60,
        truncated_tool_results: 1,
        tool_result_artifacts: 1,
        exposed_tools: 6,
        memory_snapshot_chars: 180,
        memory_snapshot_tokens: 45,
        retrieval_items: 2,
        retrieval_tokens: 80,
        skill_list_chars: 120,
        skill_list_tokens: 30,
        route_scoped_tools: true,
        workflow_context: "minimal".to_string(),
        closeout_visibility: "concise".to_string(),
        validation_evidence: "passed".to_string(),
        warnings: vec!["truncated_without_artifact".to_string()],
    });

    let trace = collector.finish(TurnStatus::Completed);
    let summary = format_trace_summary(&trace, 10);
    assert!(summary.contains("Runtime Diet: light"));
    assert!(summary.contains("prompt=1200"));
    assert!(summary.contains("total=1520"));
    assert!(summary.contains("context_remaining=6480/8000"));
    assert!(summary.contains("tool_results=240ch/~60t"));
    assert!(summary.contains("truncated=1"));
    assert!(summary.contains("artifacts=1"));
    assert!(summary.contains("tools=6"));
    assert!(summary.contains("pinned_memory=180ch/~45t"));
    assert!(summary.contains("retrieval=2items/~80t"));
    assert!(summary.contains("skills=120ch/~30t"));
    assert!(summary.contains("workflow=minimal"));
    assert!(summary.contains("warnings=truncated_without_artifact"));
}

#[test]
fn runtime_diet_report_flags_budget_bloat() {
    let event = TraceEvent::RuntimeDietReport {
        prompt_tokens: RUNTIME_DIET_PROMPT_TOKEN_BUDGET + 1,
        tool_schema_tokens: 0,
        total_request_tokens: RUNTIME_DIET_PROMPT_TOKEN_BUDGET + 1,
        max_context_tokens: None,
        remaining_context_tokens: None,
        tool_result_chars: 0,
        tool_result_tokens: 0,
        truncated_tool_results: 0,
        tool_result_artifacts: 0,
        exposed_tools: 1,
        memory_snapshot_chars: 0,
        memory_snapshot_tokens: 0,
        retrieval_items: 0,
        retrieval_tokens: 0,
        skill_list_chars: 0,
        skill_list_tokens: 0,
        route_scoped_tools: true,
        workflow_context: "minimal".to_string(),
        closeout_visibility: "none".to_string(),
        validation_evidence: "none".to_string(),
        warnings: Vec::new(),
    };

    assert!(event.summary().starts_with("heavy "));
}
