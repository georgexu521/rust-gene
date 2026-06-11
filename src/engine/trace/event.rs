use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    UserPromptSubmitted {
        chars: usize,
    },
    IntentRouted {
        #[serde(default)]
        agent_mode: Option<String>,
        intent: String,
        workflow: String,
        retrieval: String,
        confidence: f32,
        risk: String,
        reason: String,
    },
    /// Route candidate that matched but was NOT selected (shadow diagnostics).
    RouteCandidateEvaluated {
        intent: String,
        confidence: f32,
        matched_signals: Vec<String>,
        reason: String,
    },
    /// Summary of route competition: selected candidate + runner-up + delta.
    RouteCompetitionSummary {
        selected_intent: String,
        selected_confidence: f32,
        runner_up_intent: String,
        runner_up_confidence: f32,
        candidate_count: usize,
        delta: f32,
    },
    /// Token breakdown per request: distribution of tokens by source type.
    ContextTokenBreakdown {
        total_chars: usize,
        system_chars: usize,
        history_chars: usize,
        tool_result_chars: usize,
        dynamic_zone_chars: usize,
        last_user_chars: usize,
    },
    ResourcePolicySelected {
        latency: String,
        target_ms: u64,
        cost_ceiling_usd: f64,
        reasoning: String,
        parallelism_limit: usize,
        max_tool_calls: usize,
        context_budget_tokens: usize,
        #[serde(default)]
        allow_fallback_model: bool,
        reason: String,
    },
    TaskContextBuilt {
        task_id: String,
        workflow: String,
        files: usize,
        constraints: usize,
        risks: usize,
        acceptance_checks: usize,
    },
    TaskContractMaterialized {
        task_id: String,
        task_type: String,
        model_profile: String,
        assumptions: usize,
        scope_files: usize,
        validation_commands: usize,
        proof_required: bool,
        risk: String,
    },
    ContextPackMaterialized {
        task_id: String,
        project_facts: usize,
        memory_records: usize,
        recent_observations: usize,
        failure_summaries: usize,
        estimated_tokens: usize,
        max_tokens: usize,
        overflow_items: usize,
        fingerprint: String,
    },
    ImplementationIntentRecorded {
        task_id: String,
        workflow: String,
        target_files: usize,
        validation_commands: Vec<String>,
        risks: usize,
        reason: String,
    },
    WorkflowJudgmentCompleted {
        task_type: String,
        complexity: String,
        risk: String,
        plan_steps: usize,
        acceptance_checks: usize,
        questions: usize,
        guided_reasoning: bool,
    },
    WorkflowPlanProgress {
        total_steps: usize,
        completed_steps: usize,
        active_step: Option<String>,
        top_priority: Option<String>,
        #[serde(default)]
        top_importance_score: Option<f32>,
        #[serde(default)]
        top_weight_share: Option<f32>,
        #[serde(default)]
        weight_source: Option<String>,
        reweighted: bool,
    },
    WorkflowLearningAdjusted {
        adjustments: usize,
        before_top_step: Option<String>,
        after_top_step: Option<String>,
        reason: String,
    },
    StageValidationCompleted {
        step: Option<String>,
        status: String,
        changed_files: usize,
        evidence_items: usize,
    },
    ReflectionPassCompleted {
        pass_id: String,
        task_id: String,
        status: String,
        findings: usize,
        unresolved: usize,
    },
    SessionGoalUpdated {
        goal_id: String,
        title: String,
        status: String,
        reason: String,
    },
    GoalDriftDetected {
        goal_id: String,
        tool: String,
        call_id: String,
        level: String,
        reason: String,
        suggested_action: Option<String>,
    },
    DestructiveScopeChecked {
        tool: String,
        call_id: String,
        operation: String,
        target: Option<String>,
        allowed: bool,
        reason: String,
    },
    WorkflowRouted {
        decision: String,
        reason: String,
    },
    WorkflowCompleted {
        steps: usize,
    },
    WorkflowFallback {
        error: String,
    },
    AgentLoopStepEvaluated {
        route_workflow: String,
        route_risk: String,
        task_mode: String,
        stage_before: String,
        stage_after: String,
        #[serde(default)]
        mva_stage_before: String,
        #[serde(default)]
        mva_stage_after: String,
        #[serde(default)]
        stage_transition_policy: String,
        exposed_tools: usize,
        selected_tool_calls: usize,
        action_score_records: usize,
        #[serde(default)]
        latest_action_score: Option<i16>,
        observations_delta: usize,
        key_findings_delta: usize,
        stop_status: String,
        stop_reason: String,
        stop_action: String,
        #[serde(default)]
        terminal_status: Option<String>,
        state_delta: String,
    },
    StopCheckEvaluated {
        status: String,
        reason: String,
        stage: String,
        #[serde(default)]
        terminal_status: Option<String>,
        #[serde(default)]
        action: String,
        no_code_progress_rounds: usize,
        action_checkpoint_active: bool,
        summary: String,
        #[serde(default)]
        evidence_items: usize,
        #[serde(default)]
        failure_type: Option<String>,
        #[serde(default)]
        recovery_plan_id: Option<String>,
        #[serde(default)]
        rollback_recommended: bool,
        #[serde(default)]
        next_action: Option<String>,
    },
    WorkflowContractActivation {
        mode: String,
        phase: String,
        active: bool,
        reason: String,
    },
    RiskSignalAssessed {
        phase: String,
        level: String,
        entry_contract: bool,
        reasons: Vec<String>,
    },
    AdaptiveWorkflowTriggered {
        trigger: String,
        depth: String,
        require_workflow_judgment: bool,
        require_stage_validation: bool,
        max_repair_attempts: usize,
        reason: String,
    },
    MemorySnapshotInjected {
        chars: usize,
    },
    MemoryPrefetch {
        chars: usize,
    },
    ActiveMemoryEvaluated {
        status: String,
        reason: String,
        items: usize,
        timeout_ms: u64,
        elapsed_ms: u128,
    },
    SelfEvolutionGuidanceInjected {
        records: usize,
        chars: usize,
        provenance: Vec<String>,
    },
    RetrievalContextBuilt {
        policy: String,
        sources: Vec<String>,
        items: usize,
        estimated_tokens: usize,
        #[serde(default)]
        provenance: Vec<String>,
        #[serde(default)]
        conflicts: usize,
    },
    ContextZonesMaterialized {
        stable_prefix_tokens: u64,
        task_state_tokens: u64,
        relevant_material_tokens: u64,
        recent_observation_tokens: u64,
        current_decision_request_tokens: u64,
        #[serde(default)]
        stable_prefix_fingerprint: String,
        #[serde(default)]
        task_state_fingerprint: String,
        #[serde(default)]
        relevant_material_fingerprint: String,
        #[serde(default)]
        recent_observation_fingerprint: String,
        #[serde(default)]
        current_decision_request_fingerprint: String,
        #[serde(default)]
        stable_prefix_budget_tokens: u64,
        #[serde(default)]
        task_state_budget_tokens: u64,
        #[serde(default)]
        relevant_material_budget_tokens: u64,
        #[serde(default)]
        recent_observation_budget_tokens: u64,
        #[serde(default)]
        current_decision_request_budget_tokens: u64,
        #[serde(default)]
        stable_prefix_overflow: String,
        #[serde(default)]
        task_state_overflow: String,
        #[serde(default)]
        relevant_material_overflow: String,
        #[serde(default)]
        recent_observation_overflow: String,
        #[serde(default)]
        current_decision_request_overflow: String,
        #[serde(default)]
        task_state_empty: bool,
        #[serde(default)]
        current_decision_request_empty: bool,
        relevant_material_items: usize,
        recent_observation_items: usize,
        #[serde(default)]
        zone_envelope_messages: usize,
        #[serde(default)]
        zone_source_messages: usize,
        #[serde(default)]
        zone_duplicate_blocks_removed: usize,
        #[serde(default)]
        zone_provenance_markers: usize,
    },
    CacheStabilitySnapshot {
        #[serde(default)]
        stable_prefix_fingerprint: String,
        #[serde(default)]
        tool_schema_fingerprint: String,
        #[serde(default)]
        tool_schema_tokens: u64,
        #[serde(default)]
        tool_count: usize,
        #[serde(default)]
        dynamic_zone_messages: usize,
        #[serde(default)]
        dynamic_zones_before_last_user: usize,
        #[serde(default)]
        message_count: usize,
    },
    PromptCacheUsageRecorded {
        model: String,
        prompt_tokens: u64,
        cached_tokens: u64,
        cache_miss_tokens: u64,
        hit_rate: f64,
    },
    MemoryBoundaryEvaluated {
        read_status: String,
        stale_conflict_demotion_status: String,
        closeout_write_candidate_status: String,
        reason: String,
    },
    MemoryProposalPrepared {
        task_id: String,
        status: String,
        candidates: usize,
        candidate_kinds: Vec<String>,
        evidence_items: usize,
        write_policy: String,
        write_performed: bool,
        reason: String,
    },
    CloseoutBackgroundStage {
        stage: String,
        status: String,
        duration_ms: u64,
        timeout_ms: u64,
        detail: String,
    },
    MemorySynced {
        mode: String,
    },
    ContextCompacted {
        before_tokens: usize,
        after_tokens: usize,
        strategy: String,
        #[serde(default)]
        trigger: Option<String>,
        #[serde(default)]
        token_pressure: Option<String>,
        #[serde(default)]
        boundary_id: Option<String>,
        #[serde(default)]
        sequence: Option<u32>,
        #[serde(default)]
        messages_before: Option<usize>,
        #[serde(default)]
        messages_after: Option<usize>,
        #[serde(default)]
        preserved_tail_count: Option<usize>,
        #[serde(default)]
        retained_items: Vec<String>,
        #[serde(default)]
        provenance: Vec<String>,
    },
    RuntimeDietReport {
        prompt_tokens: u64,
        tool_schema_tokens: u64,
        #[serde(default)]
        total_request_tokens: u64,
        #[serde(default)]
        max_context_tokens: Option<u64>,
        #[serde(default)]
        remaining_context_tokens: Option<u64>,
        #[serde(default)]
        tool_result_chars: usize,
        #[serde(default)]
        tool_result_tokens: u64,
        #[serde(default)]
        truncated_tool_results: usize,
        #[serde(default)]
        tool_result_artifacts: usize,
        exposed_tools: usize,
        memory_snapshot_chars: usize,
        memory_snapshot_tokens: u64,
        retrieval_items: usize,
        retrieval_tokens: u64,
        skill_list_chars: usize,
        skill_list_tokens: u64,
        route_scoped_tools: bool,
        workflow_context: String,
        closeout_visibility: String,
        validation_evidence: String,
        #[serde(default)]
        warnings: Vec<String>,
    },
    ApiRequestStarted {
        iteration: usize,
        model: String,
        tools: usize,
        #[serde(default)]
        provider_family: Option<String>,
        #[serde(default)]
        nonstreaming_tools_required: bool,
        #[serde(default)]
        tool_result_adjacency_required: bool,
    },
    ProviderMessageSequenceNormalized {
        provider_family: String,
        requires_tool_result_adjacency: bool,
        requires_merged_system_messages: bool,
        system_messages_merged: usize,
        input_messages: usize,
        output_messages: usize,
        valid_tool_call_pairs: usize,
        dropped_assistant_tool_calls: usize,
        dropped_tool_results: usize,
        #[serde(default)]
        valid_tool_call_ids: Vec<String>,
        #[serde(default)]
        dropped_assistant_tool_call_ids: Vec<String>,
        #[serde(default)]
        dropped_tool_result_ids: Vec<String>,
    },
    ProviderToolCallRepairApplied {
        provider_family: String,
        schema_flattened_tools: usize,
        schema_flattened_fields: usize,
        scavenged_tool_calls: usize,
        argument_repairs: usize,
        unflattened_arguments: usize,
        dropped_duplicate_calls: usize,
        malformed_tool_calls: usize,
        warnings: Vec<String>,
    },
    StreamingToolExecutionShadow {
        mode: String,
        provider_family: String,
        provider_supports_streaming_tool_calls: bool,
        streamed_request_path: bool,
        observed_tool_calls: usize,
        read_only_tool_calls: usize,
        concurrency_safe_tool_calls: usize,
        eligible_tool_calls: usize,
        schema_complete_tool_calls: usize,
        latency_upper_bound_ms: u64,
        reason: String,
    },
    ApiRequestCompleted {
        iteration: usize,
        tool_calls: usize,
        content_chars: usize,
    },
    ProviderRequestStarted {
        provider_family: String,
        model: String,
        request_shape: String,
        timeout_secs: u64,
        slow_warning_threshold_secs: u64,
        message_count: usize,
        tool_count: usize,
        is_known_slow_path: bool,
    },
    ProviderRequestRetrying {
        provider_family: String,
        #[serde(default)]
        model: String,
        request_shape: String,
        attempt: usize,
        max_attempts: usize,
        delay_ms: u64,
        elapsed_ms: u64,
        error_preview: String,
    },
    ProviderRequestSlowWarning {
        provider_family: String,
        #[serde(default)]
        model: String,
        request_shape: String,
        elapsed_ms: u64,
        timeout_ms: u64,
        message: String,
    },
    ProviderRequestCompleted {
        provider_family: String,
        #[serde(default)]
        model: String,
        request_shape: String,
        elapsed_ms: u64,
        success: bool,
    },
    ProviderRequestTimeout {
        provider_family: String,
        #[serde(default)]
        model: String,
        request_shape: String,
        elapsed_ms: u64,
        timeout_ms: u64,
    },
    ProviderRequestCancelled {
        provider_family: String,
        #[serde(default)]
        model: String,
        request_shape: String,
        elapsed_ms: u64,
    },
    ToolStarted {
        tool: String,
        call_id: String,
        parallel: bool,
        pre_executed: bool,
    },
    ActionDecisionEvaluated {
        tool: String,
        call_id: String,
        stage: String,
        value: u8,
        risk: u8,
        uncertainty_reduction: u8,
        cost: u8,
        reversibility: u8,
        #[serde(default)]
        scope_fit: u8,
        #[serde(default)]
        action_score: i16,
        #[serde(default)]
        formula_stage: String,
        #[serde(default)]
        formula_version: String,
        #[serde(default)]
        phase_aligned: bool,
        #[serde(default)]
        mutates_workspace: bool,
        #[serde(default)]
        broad_shell: bool,
        #[serde(default)]
        modifiers: Vec<serde_json::Value>,
        requires_confirmation: bool,
        reason: String,
    },
    CandidateActionsEvaluated {
        mode: String,
        candidate_count: usize,
        selected_id: Option<String>,
        selected_tool: Option<String>,
        selected_score: Option<i16>,
        #[serde(default)]
        selected_runtime_score: Option<i16>,
        #[serde(default)]
        selected_model_score: Option<i16>,
        #[serde(default)]
        runtime_model_score_delta: Option<i16>,
        #[serde(default)]
        runtime_selected_differs_from_model_order: bool,
        #[serde(default)]
        calibration_reason: String,
        #[serde(default)]
        selected_factor_score: Option<i16>,
        #[serde(default)]
        model_factor_coverage: usize,
        #[serde(default)]
        memory_evidence_items: usize,
        #[serde(default)]
        selected_factor_rationale: Option<String>,
        rejected: usize,
        reason: String,
    },
    ActionReviewed {
        tool: String,
        call_id: String,
        decision: String,
        reason: String,
        permission: Option<String>,
        scope_allowed: bool,
        budget_allowed: bool,
        checkpoint: String,
        network: String,
        external_effect: String,
        recovery: String,
    },
    MemoryRecallScored {
        item_count: usize,
        injected: usize,
        available: usize,
        omitted: usize,
        conflict_capped: usize,
        top_score: f32,
        budget_exhausted: bool,
        policy: String,
    },
    MemoryWriteScored {
        candidate_id: String,
        kind: String,
        status: String,
        score: f32,
        threshold: f32,
        explicit: bool,
        duplication: f32,
        reason: String,
    },
    MemoryKeepScored {
        record_id: String,
        kind: String,
        action: String,
        score: f32,
        contradiction_risk: f32,
        redundancy: f32,
        reason: String,
    },
    PermissionRequested {
        tool: String,
        call_id: String,
        prompt: String,
        #[serde(default)]
        review: Option<crate::engine::human_review::HumanReviewAuditRecord>,
    },
    PermissionResolved {
        tool: String,
        call_id: String,
        approved: bool,
        #[serde(default)]
        source: Option<String>,
        #[serde(default)]
        decision: Option<String>,
        #[serde(default)]
        persistence_scope: Option<String>,
        #[serde(default)]
        rule_pattern: Option<String>,
        #[serde(default)]
        persisted_path: Option<String>,
        #[serde(default)]
        review: Option<crate::engine::human_review::HumanReviewAuditRecord>,
    },
    ToolCompleted {
        tool: String,
        call_id: String,
        success: bool,
        duration_ms: Option<u64>,
        output_chars: usize,
    },
    ToolObservationRecorded {
        tool: String,
        call_id: String,
        status: String,
        #[serde(default)]
        result_kind: String,
        #[serde(default)]
        model_visibility: String,
        #[serde(default = "default_true")]
        include_in_next_context: bool,
        #[serde(default = "default_true")]
        store_in_state: bool,
        #[serde(default)]
        key_findings: usize,
        #[serde(default)]
        evidence_items: usize,
        #[serde(default)]
        failure_type: Option<String>,
        #[serde(default)]
        recovery_plan_id: Option<String>,
        #[serde(default)]
        recovery_kind: Option<String>,
        #[serde(default)]
        raw_result_ref: Option<String>,
        #[serde(default)]
        quality_warnings: usize,
        #[serde(default)]
        quality_warning_labels: Vec<String>,
        files_read: usize,
        files_changed: usize,
        checkpoint_id: Option<String>,
        summary: String,
    },
    HookCompleted {
        event: String,
        provider: String,
        hook_name: String,
        call_id: String,
        tool: Option<String>,
        success: bool,
        blocked: bool,
        duration_ms: u64,
        error: Option<String>,
        output_preview: Option<String>,
    },
    SubagentStarted {
        agent_id: String,
        profile: Option<String>,
        role: String,
        description: String,
        timeout_secs: u64,
        allowed_tools: usize,
    },
    SubagentCompleted {
        agent_id: String,
        status: String,
        duration_ms: u64,
        output_chars: usize,
        tools_used: usize,
    },
    VerificationCompleted {
        changed_files: usize,
        passed: bool,
        #[serde(default)]
        check_passed: bool,
        #[serde(default)]
        tests_passed: bool,
        #[serde(default)]
        review_passed: bool,
        #[serde(default)]
        failed_commands: Vec<String>,
    },
    RequiredValidationHeartbeat {
        command_preview: String,
        elapsed_secs: u64,
        timeout_secs: Option<u64>,
    },
    AcceptanceReviewCompleted {
        accepted: bool,
        confidence: String,
        criteria: usize,
        unresolved: usize,
        next_action: String,
    },
    GuidedDebuggingCompleted {
        blocker: bool,
        next_action: String,
        causes: usize,
        evidence_items: usize,
        ask_user: bool,
    },
    RecoveryApplied {
        error: String,
        action: String,
    },
    RecoveryPlan {
        plan_id: String,
        source: String,
        category: String,
        #[serde(default)]
        failure_type: String,
        #[serde(default)]
        recovery_kind: String,
        action: String,
        retryable: bool,
        safe_retry: bool,
        #[serde(default)]
        allowed_alternatives: Vec<String>,
        #[serde(default)]
        retry_budget: Option<usize>,
        #[serde(default)]
        side_effect_uncertain: bool,
        #[serde(default)]
        requires_user_decision: bool,
        suggested_command: Option<String>,
        status: String,
    },
    McpResourceAccessed {
        server: String,
        uri: String,
        action: String,
        success: bool,
        content_chars: usize,
    },
    RemoteBridgeAction {
        tool: String,
        call_id: String,
        action: String,
        target: Option<String>,
        risk: String,
        permission_hint: String,
        success: bool,
        error_code: Option<String>,
    },
    AssistantResponded {
        chars: usize,
        iterations: usize,
    },
    CompletionContractEvaluated {
        mode: String,
        workflow: String,
        status: String,
        terminal_status: String,
        requires_validation: bool,
        verification_status: String,
        verification_proof_status: String,
        changed_files: usize,
        reason: String,
    },
    FinalCloseoutPrepared {
        status: String,
        #[serde(default)]
        terminal_status: Option<String>,
        #[serde(default)]
        stop_reason: Option<String>,
        #[serde(default)]
        stop_action: Option<String>,
        #[serde(default)]
        failure_type: Option<String>,
        #[serde(default)]
        recovery_plan_id: Option<String>,
        #[serde(default)]
        rollback_status: Option<String>,
        changed_files: usize,
        validation_items: usize,
        #[serde(default)]
        tool_records: usize,
        #[serde(default)]
        tool_evidence: Option<String>,
        #[serde(default)]
        verification_proof_status: Option<String>,
        #[serde(default)]
        verification_proof_summary: Option<String>,
        #[serde(default)]
        verification_proof_kind_summary: Option<String>,
        #[serde(default)]
        verification_proof_support_status: Option<String>,
        #[serde(default)]
        verification_proof_support_summary: Option<String>,
        #[serde(default)]
        verification_proof_supports_verified: Option<bool>,
        #[serde(default)]
        verification_proof_residual_risk: Option<bool>,
        acceptance_items: usize,
        residual_risks: usize,
    },
    ExecutionReportPrepared {
        task_id: String,
        status: String,
        changed_files: usize,
        validation_evidence: usize,
        risks: usize,
        next_steps: usize,
    },
    FinalAnswerClaimGate {
        decision: String,
        unsupported_claims: usize,
        repair_attempt: u32,
        changed_files: usize,
        #[serde(default)]
        verification_proof_status: Option<String>,
        summary: String,
    },
    Error {
        message: String,
    },
}

fn default_true() -> bool {
    true
}
