//! Runtime turn tracing.
//!
//! The trace spine records high-level events for a user turn without storing
//! full sensitive tool outputs. It is designed to back `/trace` and future
//! eval assertions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

mod collector;
mod event_summary;
mod formatting;

pub use collector::{TraceCollector, TraceStore};
pub use formatting::{format_trace_recent_line, format_trace_summary};

pub(super) const DEFAULT_MAX_TRACES: usize = 100;
const PREVIEW_CHARS: usize = 120;
pub const RUNTIME_DIET_PROMPT_TOKEN_BUDGET: u64 = 4_000;
pub const RUNTIME_DIET_TOOL_COUNT_BUDGET: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnTrace {
    pub trace_id: String,
    pub session_id: String,
    pub turn_index: u64,
    pub user_message_preview: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: TurnStatus,
    pub events: Vec<TraceEvent>,
}

impl TurnTrace {
    pub fn new(session_id: impl Into<String>, turn_index: u64, user_message: &str) -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            turn_index,
            user_message_preview: preview(user_message),
            started_at: Utc::now(),
            finished_at: None,
            status: TurnStatus::Running,
            events: vec![TraceEvent::UserPromptSubmitted {
                chars: user_message.chars().count(),
            }],
        }
    }

    pub fn finish(&mut self, status: TurnStatus) {
        self.status = status;
        self.finished_at = Some(Utc::now());
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at
            .map(|end| (end - self.started_at).num_milliseconds())
    }
}

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
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlLoopDiagnostic {
    pub phases: Vec<ControlLoopPhaseDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlLoopPhaseDiagnostic {
    pub phase: String,
    pub events: usize,
    pub latest_label: Option<String>,
    pub latest_summary: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionReviewTraceSummary {
    pub total: usize,
    pub allowed: usize,
    pub ask_user: usize,
    pub denied: usize,
    pub revised: usize,
    pub checkpoint_required: usize,
    pub latest_tool: Option<String>,
    pub latest_decision: Option<String>,
    pub latest_reason: Option<String>,
}

impl ControlLoopDiagnostic {
    pub fn compact_summary(&self) -> String {
        self.phases
            .iter()
            .map(|phase| {
                let latest = phase
                    .latest_label
                    .as_deref()
                    .filter(|label| !label.is_empty())
                    .unwrap_or("none");
                format!("{}={} latest={}", phase.phase, phase.events, latest)
            })
            .collect::<Vec<_>>()
            .join(" -> ")
    }
}

impl ActionReviewTraceSummary {
    pub fn compact_summary(&self) -> String {
        let latest = match (
            self.latest_tool.as_deref(),
            self.latest_decision.as_deref(),
            self.latest_reason.as_deref(),
        ) {
            (Some(tool), Some(decision), Some(reason)) => {
                format!("{tool}:{decision}/{reason}")
            }
            _ => "none".to_string(),
        };
        format!(
            "total={} allow={} ask_user={} denied={} revised={} checkpoint_required={} latest={}",
            self.total,
            self.allowed,
            self.ask_user,
            self.denied,
            self.revised,
            self.checkpoint_required,
            latest
        )
    }
}

pub fn control_loop_diagnostic(trace: &TurnTrace) -> ControlLoopDiagnostic {
    let mut phases = CONTROL_LOOP_PHASES
        .iter()
        .map(|phase| ControlLoopPhaseDiagnostic {
            phase: (*phase).to_string(),
            events: 0,
            latest_label: None,
            latest_summary: None,
        })
        .collect::<Vec<_>>();

    for event in &trace.events {
        let Some(phase) = control_loop_phase_for_event(event) else {
            continue;
        };
        let Some(slot) = phases.iter_mut().find(|item| item.phase == phase) else {
            continue;
        };
        slot.events += 1;
        slot.latest_label = Some(event.label().to_string());
        slot.latest_summary = Some(event.summary());
    }

    ControlLoopDiagnostic { phases }
}

pub fn action_review_trace_summary(trace: &TurnTrace) -> Option<ActionReviewTraceSummary> {
    let mut summary = ActionReviewTraceSummary::default();
    for event in &trace.events {
        let TraceEvent::ActionReviewed {
            tool,
            decision,
            reason,
            checkpoint,
            ..
        } = event
        else {
            continue;
        };
        summary.total += 1;
        match decision.as_str() {
            "allow" => summary.allowed += 1,
            "ask_user" => summary.ask_user += 1,
            "deny" => summary.denied += 1,
            "revise" => summary.revised += 1,
            _ => {}
        }
        if action_review_checkpoint_required(checkpoint, reason) {
            summary.checkpoint_required += 1;
        }
        summary.latest_tool = Some(tool.clone());
        summary.latest_decision = Some(decision.clone());
        summary.latest_reason = Some(reason.clone());
    }
    (summary.total > 0).then_some(summary)
}

fn action_review_checkpoint_required(checkpoint: &str, reason: &str) -> bool {
    reason == "checkpoint_required"
        || matches!(
            checkpoint,
            "required_and_present" | "required_but_missing" | "unavailable"
        )
}

const CONTROL_LOOP_PHASES: [&str; 7] = [
    "context",
    "decision",
    "permission",
    "tool_execution",
    "state_update",
    "verification",
    "closeout",
];

fn control_loop_phase_for_event(event: &TraceEvent) -> Option<&'static str> {
    match event {
        TraceEvent::UserPromptSubmitted { .. }
        | TraceEvent::IntentRouted { .. }
        | TraceEvent::ResourcePolicySelected { .. }
        | TraceEvent::TaskContextBuilt { .. }
        | TraceEvent::TaskContractMaterialized { .. }
        | TraceEvent::ContextPackMaterialized { .. }
        | TraceEvent::MemorySnapshotInjected { .. }
        | TraceEvent::MemoryPrefetch { .. }
        | TraceEvent::ActiveMemoryEvaluated { .. }
        | TraceEvent::SelfEvolutionGuidanceInjected { .. }
        | TraceEvent::RetrievalContextBuilt { .. }
        | TraceEvent::ContextZonesMaterialized { .. }
        | TraceEvent::CacheStabilitySnapshot { .. }
        | TraceEvent::PromptCacheUsageRecorded { .. }
        | TraceEvent::MemoryBoundaryEvaluated { .. }
        | TraceEvent::MemorySynced { .. }
        | TraceEvent::ContextCompacted { .. }
        | TraceEvent::RuntimeDietReport { .. }
        | TraceEvent::ApiRequestStarted { .. }
        | TraceEvent::ProviderMessageSequenceNormalized { .. }
        | TraceEvent::ProviderToolCallRepairApplied { .. }
        | TraceEvent::StreamingToolExecutionShadow { .. }
        | TraceEvent::ProviderRequestStarted { .. }
        | TraceEvent::ProviderRequestRetrying { .. }
        | TraceEvent::ProviderRequestSlowWarning { .. }
        | TraceEvent::ProviderRequestCompleted { .. }
        | TraceEvent::ProviderRequestTimeout { .. }
        | TraceEvent::ProviderRequestCancelled { .. } => Some("context"),
        TraceEvent::ImplementationIntentRecorded { .. }
        | TraceEvent::WorkflowJudgmentCompleted { .. }
        | TraceEvent::WorkflowPlanProgress { .. }
        | TraceEvent::WorkflowLearningAdjusted { .. }
        | TraceEvent::WorkflowContractActivation { .. }
        | TraceEvent::RiskSignalAssessed { .. }
        | TraceEvent::AdaptiveWorkflowTriggered { .. }
        | TraceEvent::ActionDecisionEvaluated { .. }
        | TraceEvent::CandidateActionsEvaluated { .. }
        | TraceEvent::ActionReviewed { .. }
        | TraceEvent::WorkflowRouted { .. } => Some("decision"),
        TraceEvent::GoalDriftDetected { .. }
        | TraceEvent::DestructiveScopeChecked { .. }
        | TraceEvent::PermissionRequested { .. }
        | TraceEvent::PermissionResolved { .. } => Some("permission"),
        TraceEvent::ApiRequestCompleted { .. }
        | TraceEvent::ToolStarted { .. }
        | TraceEvent::ToolCompleted { .. }
        | TraceEvent::HookCompleted { .. }
        | TraceEvent::SubagentStarted { .. }
        | TraceEvent::SubagentCompleted { .. }
        | TraceEvent::McpResourceAccessed { .. }
        | TraceEvent::RemoteBridgeAction { .. } => Some("tool_execution"),
        TraceEvent::SessionGoalUpdated { .. }
        | TraceEvent::AgentLoopStepEvaluated { .. }
        | TraceEvent::StopCheckEvaluated { .. }
        | TraceEvent::WorkflowFallback { .. }
        | TraceEvent::ToolObservationRecorded { .. }
        | TraceEvent::RecoveryApplied { .. }
        | TraceEvent::RecoveryPlan { .. } => Some("state_update"),
        TraceEvent::StageValidationCompleted { .. }
        | TraceEvent::ReflectionPassCompleted { .. }
        | TraceEvent::VerificationCompleted { .. }
        | TraceEvent::RequiredValidationHeartbeat { .. }
        | TraceEvent::AcceptanceReviewCompleted { .. }
        | TraceEvent::GuidedDebuggingCompleted { .. } => Some("verification"),
        TraceEvent::WorkflowCompleted { .. }
        | TraceEvent::AssistantResponded { .. }
        | TraceEvent::CompletionContractEvaluated { .. }
        | TraceEvent::FinalCloseoutPrepared { .. }
        | TraceEvent::ExecutionReportPrepared { .. }
        | TraceEvent::MemoryProposalPrepared { .. }
        | TraceEvent::Error { .. } => Some("closeout"),
    }
}

pub fn latest_runtime_diet_summary(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if matches!(event, TraceEvent::RuntimeDietReport { .. }) {
            Some(event.summary())
        } else {
            None
        }
    })
}

pub fn latest_memory_proposal_summary(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::MemoryProposalPrepared {
            status,
            candidates,
            candidate_kinds,
            evidence_items,
            write_policy,
            write_performed,
            reason,
            ..
        } => Some(format!(
            "{} candidates={} kinds={} evidence={} write_policy={} wrote={} reason={}",
            status,
            candidates,
            if candidate_kinds.is_empty() {
                "none".to_string()
            } else {
                candidate_kinds.join("+")
            },
            evidence_items,
            write_policy,
            write_performed,
            preview(reason)
        )),
        _ => None,
    })
}

pub fn latest_tool_record_count(trace: &TurnTrace) -> Option<usize> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::FinalCloseoutPrepared { tool_records, .. } => Some(*tool_records),
        _ => None,
    })
}

pub fn latest_tool_record_evidence_summary(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::FinalCloseoutPrepared {
            status,
            changed_files,
            validation_items,
            tool_records,
            tool_evidence,
            verification_proof_status,
            verification_proof_summary,
            acceptance_items,
            residual_risks,
            ..
        } if *tool_records > 0 || tool_evidence.as_ref().is_some_and(|s| !s.trim().is_empty()) => {
            Some(format!(
                "status={} records={} files={} validation={} acceptance={} risks={} proof={} proof_summary={} evidence={}",
                status,
                tool_records,
                changed_files,
                validation_items,
                acceptance_items,
                residual_risks,
                verification_proof_status.as_deref().unwrap_or("none"),
                verification_proof_summary.as_deref().unwrap_or("none"),
                tool_evidence.as_deref().unwrap_or("none")
            ))
        }
        _ => None,
    })
}

fn runtime_diet_level(prompt_tokens: u64, exposed_tools: usize) -> &'static str {
    if prompt_tokens > RUNTIME_DIET_PROMPT_TOKEN_BUDGET
        || exposed_tools > RUNTIME_DIET_TOOL_COUNT_BUDGET
    {
        "heavy"
    } else {
        "light"
    }
}

fn preview(text: &str) -> String {
    let mut out: String = text.chars().take(PREVIEW_CHARS).collect();
    if text.chars().count() > PREVIEW_CHARS {
        out.push_str("...");
    }
    out.replace('\n', " ")
}

fn default_true() -> bool {
    true
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn compact_id_list(ids: &[String]) -> String {
    if ids.is_empty() {
        return "none".to_string();
    }
    let mut compact = ids
        .iter()
        .take(4)
        .map(|id| short_id(id))
        .collect::<Vec<_>>();
    if ids.len() > compact.len() {
        compact.push(format!("+{}", ids.len() - compact.len()));
    }
    compact.join(",")
}

fn compact_label_list(labels: &[String]) -> String {
    if labels.is_empty() {
        return "none".to_string();
    }
    let mut compact = labels.iter().take(4).cloned().collect::<Vec<_>>();
    if labels.len() > compact.len() {
        compact.push(format!("+{}", labels.len() - compact.len()));
    }
    compact.join(",")
}

#[cfg(test)]
mod tests;
