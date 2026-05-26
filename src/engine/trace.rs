//! Runtime turn tracing.
//!
//! The trace spine records high-level events for a user turn without storing
//! full sensitive tool outputs. It is designed to back `/trace` and future
//! eval assertions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

const DEFAULT_MAX_TRACES: usize = 100;
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

impl TraceEvent {
    pub fn label(&self) -> &'static str {
        match self {
            TraceEvent::UserPromptSubmitted { .. } => "prompt",
            TraceEvent::IntentRouted { .. } => "intent",
            TraceEvent::ResourcePolicySelected { .. } => "resource.policy",
            TraceEvent::TaskContextBuilt { .. } => "task.context",
            TraceEvent::TaskContractMaterialized { .. } => "task.contract",
            TraceEvent::ContextPackMaterialized { .. } => "context.pack",
            TraceEvent::ImplementationIntentRecorded { .. } => "implementation.intent",
            TraceEvent::WorkflowJudgmentCompleted { .. } => "workflow.judgment",
            TraceEvent::WorkflowPlanProgress { .. } => "workflow.plan",
            TraceEvent::WorkflowLearningAdjusted { .. } => "workflow.learning",
            TraceEvent::StageValidationCompleted { .. } => "stage.validation",
            TraceEvent::ReflectionPassCompleted { .. } => "reflection.pass",
            TraceEvent::SessionGoalUpdated { .. } => "goal",
            TraceEvent::GoalDriftDetected { .. } => "goal.drift",
            TraceEvent::DestructiveScopeChecked { .. } => "destructive.scope",
            TraceEvent::WorkflowRouted { .. } => "workflow.route",
            TraceEvent::WorkflowCompleted { .. } => "workflow.done",
            TraceEvent::WorkflowFallback { .. } => "workflow.fallback",
            TraceEvent::AgentLoopStepEvaluated { .. } => "agent.loop",
            TraceEvent::StopCheckEvaluated { .. } => "stop.check",
            TraceEvent::WorkflowContractActivation { .. } => "workflow.contract",
            TraceEvent::RiskSignalAssessed { .. } => "risk.signal",
            TraceEvent::AdaptiveWorkflowTriggered { .. } => "workflow.trigger",
            TraceEvent::MemorySnapshotInjected { .. } => "memory.snapshot",
            TraceEvent::MemoryPrefetch { .. } => "memory.prefetch",
            TraceEvent::ActiveMemoryEvaluated { .. } => "memory.active",
            TraceEvent::RetrievalContextBuilt { .. } => "retrieval.context",
            TraceEvent::ContextZonesMaterialized { .. } => "context.zones",
            TraceEvent::MemoryBoundaryEvaluated { .. } => "memory.boundary",
            TraceEvent::MemoryProposalPrepared { .. } => "memory.proposal",
            TraceEvent::MemorySynced { .. } => "memory.sync",
            TraceEvent::ContextCompacted { .. } => "context.compact",
            TraceEvent::RuntimeDietReport { .. } => "runtime.diet",
            TraceEvent::ApiRequestStarted { .. } => "api.start",
            TraceEvent::ProviderMessageSequenceNormalized { .. } => "provider.protocol",
            TraceEvent::StreamingToolExecutionShadow { .. } => "streaming.tool.shadow",
            TraceEvent::ApiRequestCompleted { .. } => "api.done",
            TraceEvent::ToolStarted { .. } => "tool.start",
            TraceEvent::ActionDecisionEvaluated { .. } => "action.decision",
            TraceEvent::CandidateActionsEvaluated { .. } => "action.candidates",
            TraceEvent::ActionReviewed { .. } => "action.review",
            TraceEvent::PermissionRequested { .. } => "permission.request",
            TraceEvent::PermissionResolved { .. } => "permission.resolve",
            TraceEvent::ToolCompleted { .. } => "tool.done",
            TraceEvent::ToolObservationRecorded { .. } => "tool.observation",
            TraceEvent::HookCompleted { .. } => "hook.done",
            TraceEvent::SubagentStarted { .. } => "subagent.start",
            TraceEvent::SubagentCompleted { .. } => "subagent.done",
            TraceEvent::VerificationCompleted { .. } => "verify.done",
            TraceEvent::AcceptanceReviewCompleted { .. } => "acceptance.review",
            TraceEvent::GuidedDebuggingCompleted { .. } => "guided.debug",
            TraceEvent::RecoveryApplied { .. } => "recovery",
            TraceEvent::RecoveryPlan { .. } => "recovery.plan",
            TraceEvent::McpResourceAccessed { .. } => "mcp.resource",
            TraceEvent::RemoteBridgeAction { .. } => "remote.bridge",
            TraceEvent::AssistantResponded { .. } => "assistant",
            TraceEvent::CompletionContractEvaluated { .. } => "completion.contract",
            TraceEvent::FinalCloseoutPrepared { .. } => "closeout",
            TraceEvent::ExecutionReportPrepared { .. } => "execution.report",
            TraceEvent::Error { .. } => "error",
        }
    }

    pub fn summary(&self) -> String {
        match self {
            TraceEvent::UserPromptSubmitted { chars } => format!("user prompt: {} chars", chars),
            TraceEvent::IntentRouted {
                agent_mode,
                intent,
                workflow,
                retrieval,
                confidence,
                risk,
                reason,
            } => {
                let mode = agent_mode
                    .as_deref()
                    .filter(|mode| !mode.is_empty())
                    .unwrap_or("auto");
                format!(
                    "mode={} intent={} workflow={} retrieval={} risk={} confidence={:.2}: {}",
                    mode,
                    intent,
                    workflow,
                    retrieval,
                    risk,
                    confidence,
                    preview(reason)
                )
            }
            TraceEvent::ResourcePolicySelected {
                latency,
                target_ms,
                cost_ceiling_usd,
                reasoning,
                parallelism_limit,
                max_tool_calls,
                context_budget_tokens,
                allow_fallback_model,
                reason,
            } => format!(
                "resource policy: latency={} target={}ms cost<=${:.2} reasoning={} parallel={} tools={} ctx={} fallback={} ({})",
                latency,
                target_ms,
                cost_ceiling_usd,
                reasoning,
                parallelism_limit,
                max_tool_calls,
                context_budget_tokens,
                allow_fallback_model,
                preview(reason)
            ),
            TraceEvent::TaskContextBuilt {
                task_id,
                workflow,
                files,
                constraints,
                risks,
                acceptance_checks,
            } => format!(
                "task context {} workflow={} files={} constraints={} risks={} checks={}",
                short_id(task_id),
                workflow,
                files,
                constraints,
                risks,
                acceptance_checks
            ),
            TraceEvent::TaskContractMaterialized {
                task_id,
                task_type,
                model_profile,
                assumptions,
                scope_files,
                validation_commands,
                proof_required,
                risk,
            } => format!(
                "task contract {} type={} profile={} assumptions={} files={} validations={} proof_required={} risk={}",
                short_id(task_id),
                task_type,
                model_profile,
                assumptions,
                scope_files,
                validation_commands,
                proof_required,
                risk
            ),
            TraceEvent::ContextPackMaterialized {
                task_id,
                project_facts,
                memory_records,
                recent_observations,
                failure_summaries,
                estimated_tokens,
                max_tokens,
                overflow_items,
                fingerprint,
            } => format!(
                "context pack {} project_facts={} memory_records={} observations={} failures={} tokens~{}/{} overflow={} fp={}",
                short_id(task_id),
                project_facts,
                memory_records,
                recent_observations,
                failure_summaries,
                estimated_tokens,
                max_tokens,
                overflow_items,
                preview(fingerprint)
            ),
            TraceEvent::ImplementationIntentRecorded {
                task_id,
                workflow,
                target_files,
                validation_commands,
                risks,
                reason,
            } => format!(
                "implementation intent {} workflow={} targets={} validations={} risks={} reason={}",
                short_id(task_id),
                workflow,
                target_files,
                validation_commands.len(),
                risks,
                preview(reason)
            ),
            TraceEvent::WorkflowJudgmentCompleted {
                task_type,
                complexity,
                risk,
                plan_steps,
                acceptance_checks,
                questions,
                guided_reasoning,
            } => format!(
                "workflow judgment type={} complexity={} risk={} steps={} checks={} questions={} guided={}",
                preview(task_type),
                complexity,
                risk,
                plan_steps,
                acceptance_checks,
                questions,
                guided_reasoning
            ),
            TraceEvent::WorkflowPlanProgress {
                total_steps,
                completed_steps,
                active_step,
                top_priority,
                top_importance_score,
                top_weight_share,
                weight_source,
                reweighted,
            } => format!(
                "workflow plan {}/{} active={} priority={} importance={} share={} source={} reweighted={}",
                completed_steps,
                total_steps,
                active_step
                    .as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                top_priority.as_deref().unwrap_or("none"),
                top_importance_score
                    .map(|score| format!("{:.2}", score))
                    .unwrap_or_else(|| "none".to_string()),
                top_weight_share
                    .map(|share| format!("{:.2}", share))
                    .unwrap_or_else(|| "none".to_string()),
                weight_source.as_deref().unwrap_or("none"),
                reweighted
            ),
            TraceEvent::WorkflowLearningAdjusted {
                adjustments,
                before_top_step,
                after_top_step,
                reason,
            } => format!(
                "workflow learning adjusted count={} before={} after={} reason={}",
                adjustments,
                before_top_step
                    .as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                after_top_step
                    .as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                preview(reason)
            ),
            TraceEvent::StageValidationCompleted {
                step,
                status,
                changed_files,
                evidence_items,
            } => format!(
                "stage validation step={} status={} files={} evidence={}",
                step.as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                status,
                changed_files,
                evidence_items
            ),
            TraceEvent::ReflectionPassCompleted {
                pass_id,
                task_id,
                status,
                findings,
                unresolved,
            } => format!(
                "reflection {} task={} status={} findings={} unresolved={}",
                short_id(pass_id),
                short_id(task_id),
                status,
                findings,
                unresolved
            ),
            TraceEvent::SessionGoalUpdated {
                goal_id,
                title,
                status,
                reason,
            } => format!(
                "goal {} {}: {} ({})",
                short_id(goal_id),
                status,
                preview(title),
                preview(reason)
            ),
            TraceEvent::GoalDriftDetected {
                goal_id,
                tool,
                call_id,
                level,
                reason,
                suggested_action,
            } => format!(
                "{} {} drift={} goal={} reason={} suggested={}",
                tool,
                short_id(call_id),
                level,
                short_id(goal_id),
                preview(reason),
                suggested_action.as_deref().unwrap_or("none")
            ),
            TraceEvent::DestructiveScopeChecked {
                tool,
                call_id,
                operation,
                target,
                allowed,
                reason,
            } => format!(
                "{} {} destructive_scope op={} target={} allowed={} reason={}",
                tool,
                short_id(call_id),
                operation,
                target.as_deref().map(preview).unwrap_or_else(|| "none".to_string()),
                allowed,
                preview(reason)
            ),
            TraceEvent::WorkflowRouted { decision, reason } => {
                format!("workflow decision: {} ({})", decision, preview(reason))
            }
            TraceEvent::WorkflowCompleted { steps } => {
                format!("workflow completed: {} steps", steps)
            }
            TraceEvent::WorkflowFallback { error } => {
                format!("workflow fallback: {}", preview(error))
            }
            TraceEvent::AgentLoopStepEvaluated {
                route_workflow,
                route_risk,
                task_mode,
                stage_before,
                stage_after,
                mva_stage_before,
                mva_stage_after,
                stage_transition_policy,
                exposed_tools,
                selected_tool_calls,
                action_score_records,
                latest_action_score,
                observations_delta,
                key_findings_delta,
                stop_status,
                stop_reason,
                stop_action,
                terminal_status,
                state_delta,
            } => format!(
                "agent loop: mode={} workflow={} risk={} stage={}->{} mva_stage={}->{} transition={} tools_exposed={} calls={} scores={} latest_score={} obs_delta={} findings_delta={} stop={}/{}/{} terminal={} delta={} ",
                task_mode,
                route_workflow,
                route_risk,
                stage_before,
                stage_after,
                mva_stage_before,
                mva_stage_after,
                stage_transition_policy,
                exposed_tools,
                selected_tool_calls,
                action_score_records,
                latest_action_score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                observations_delta,
                key_findings_delta,
                stop_status,
                stop_reason,
                stop_action,
                terminal_status.as_deref().unwrap_or("none"),
                preview(state_delta)
            ),
            TraceEvent::StopCheckEvaluated {
                status,
                reason,
                stage,
                terminal_status,
                action,
                no_code_progress_rounds,
                action_checkpoint_active,
                summary,
                evidence_items,
                failure_type,
                recovery_plan_id,
                rollback_recommended,
                next_action,
            } => format!(
                "stop check status={} reason={} terminal={} action={} stage={} no_progress={} checkpoint={} evidence={} failure={} recovery={} rollback={} next={} ({})",
                status,
                reason,
                terminal_status.as_deref().unwrap_or("none"),
                if action.is_empty() { "none" } else { action },
                stage,
                no_code_progress_rounds,
                action_checkpoint_active,
                evidence_items,
                failure_type.as_deref().unwrap_or("none"),
                recovery_plan_id.as_deref().unwrap_or("none"),
                rollback_recommended,
                next_action.as_deref().unwrap_or("none"),
                preview(summary)
            ),
            TraceEvent::WorkflowContractActivation {
                mode,
                phase,
                active,
                reason,
            } => format!(
                "workflow contract {} phase={} active={} reason={}",
                mode,
                phase,
                active,
                preview(reason)
            ),
            TraceEvent::RiskSignalAssessed {
                phase,
                level,
                entry_contract,
                reasons,
            } => format!(
                "risk signal phase={} level={} entry_contract={} reasons={}",
                phase,
                level,
                entry_contract,
                preview(&reasons.join("; "))
            ),
            TraceEvent::AdaptiveWorkflowTriggered {
                trigger,
                depth,
                require_workflow_judgment,
                require_stage_validation,
                max_repair_attempts,
                reason,
            } => format!(
                "workflow trigger={} depth={} judgment={} stage_validation={} repairs={} reason={}",
                trigger,
                depth,
                require_workflow_judgment,
                require_stage_validation,
                max_repair_attempts,
                preview(reason)
            ),
            TraceEvent::MemorySnapshotInjected { chars } => {
                format!("memory snapshot injected: {} chars", chars)
            }
            TraceEvent::MemoryPrefetch { chars } => format!("memory prefetch: {} chars", chars),
            TraceEvent::ActiveMemoryEvaluated {
                status,
                reason,
                items,
                timeout_ms,
                elapsed_ms,
            } => format!(
                "active memory: status={} items={} elapsed={}ms timeout={}ms reason={}",
                status, items, elapsed_ms, timeout_ms, reason
            ),
            TraceEvent::RetrievalContextBuilt {
                policy,
                sources,
                items,
                estimated_tokens,
                provenance,
                conflicts,
            } => {
                let provenance = if provenance.is_empty() {
                    "none".to_string()
                } else {
                    provenance
                        .iter()
                        .take(3)
                        .map(|item| preview(item))
                        .collect::<Vec<_>>()
                        .join(" | ")
                };
                format!(
                    "retrieval context: policy={} sources={} items={} tokens~{} conflicts={} provenance={}",
                    policy,
                    sources.join(","),
                    items,
                    estimated_tokens,
                    conflicts,
                    provenance
                )
            }
            TraceEvent::ContextZonesMaterialized {
                stable_prefix_tokens,
                task_state_tokens,
                relevant_material_tokens,
                recent_observation_tokens,
                current_decision_request_tokens,
                stable_prefix_fingerprint,
                task_state_fingerprint,
                relevant_material_fingerprint,
                recent_observation_fingerprint,
                current_decision_request_fingerprint,
                stable_prefix_budget_tokens,
                task_state_budget_tokens,
                relevant_material_budget_tokens,
                recent_observation_budget_tokens,
                current_decision_request_budget_tokens,
                stable_prefix_overflow,
                task_state_overflow,
                relevant_material_overflow,
                recent_observation_overflow,
                current_decision_request_overflow,
                task_state_empty,
                current_decision_request_empty,
                relevant_material_items,
                recent_observation_items,
                zone_envelope_messages,
                zone_source_messages,
                zone_duplicate_blocks_removed,
                zone_provenance_markers,
            } => format!(
                "context zones: stable={}t/{} task_state={}t/{} relevant={}t/{}/{} items observation={}t/{}/{} items decision={}t/{} empty_task={} empty_decision={} envelope={} source_msgs={} dedupe_removed={} provenance={} fp={}/{}/{}/{}/{} overflow={}/{}/{}/{}/{}",
                stable_prefix_tokens,
                stable_prefix_budget_tokens,
                task_state_tokens,
                task_state_budget_tokens,
                relevant_material_tokens,
                relevant_material_budget_tokens,
                relevant_material_items,
                recent_observation_tokens,
                recent_observation_budget_tokens,
                recent_observation_items,
                current_decision_request_tokens,
                current_decision_request_budget_tokens,
                task_state_empty,
                current_decision_request_empty,
                zone_envelope_messages,
                zone_source_messages,
                zone_duplicate_blocks_removed,
                zone_provenance_markers,
                preview(stable_prefix_fingerprint),
                preview(task_state_fingerprint),
                preview(relevant_material_fingerprint),
                preview(recent_observation_fingerprint),
                preview(current_decision_request_fingerprint),
                stable_prefix_overflow,
                task_state_overflow,
                relevant_material_overflow,
                recent_observation_overflow,
                current_decision_request_overflow
            ),
            TraceEvent::MemoryBoundaryEvaluated {
                read_status,
                stale_conflict_demotion_status,
                closeout_write_candidate_status,
                reason,
            } => format!(
                "memory boundary: read={} stale_conflict={} closeout_write={} ({})",
                read_status,
                stale_conflict_demotion_status,
                closeout_write_candidate_status,
                preview(reason)
            ),
            TraceEvent::MemoryProposalPrepared {
                task_id,
                status,
                candidates,
                candidate_kinds,
                evidence_items,
                write_policy,
                write_performed,
                reason,
            } => format!(
                "memory proposal {} status={} candidates={} kinds={} evidence={} write_policy={} wrote={} ({})",
                short_id(task_id),
                status,
                candidates,
                if candidate_kinds.is_empty() {
                    "none".to_string()
                } else {
                    candidate_kinds.join(",")
                },
                evidence_items,
                write_policy,
                write_performed,
                preview(reason)
            ),
            TraceEvent::MemorySynced { mode } => format!("memory synced: {}", mode),
            TraceEvent::ContextCompacted {
                before_tokens,
                after_tokens,
                strategy,
                trigger,
                token_pressure,
                boundary_id,
                sequence,
                messages_before,
                messages_after,
                preserved_tail_count,
                retained_items,
                provenance,
            } => format!(
                "context compacted: {} -> {} tokens ({}) trigger={} pressure={} boundary={} seq={} msgs={}->{} preserved={} retained={} provenance={}",
                before_tokens,
                after_tokens,
                strategy,
                trigger.as_deref().unwrap_or("unknown"),
                token_pressure.as_deref().unwrap_or("unknown"),
                boundary_id.as_deref().unwrap_or("none"),
                sequence
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                messages_before
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                messages_after
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                preserved_tail_count
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                if retained_items.is_empty() {
                    "none".to_string()
                } else {
                    retained_items.join(",")
                },
                if provenance.is_empty() {
                    "none".to_string()
                } else {
                    provenance.join(",")
                }
            ),
            TraceEvent::RuntimeDietReport {
                prompt_tokens,
                tool_schema_tokens,
                total_request_tokens,
                max_context_tokens,
                remaining_context_tokens,
                tool_result_chars,
                tool_result_tokens,
                truncated_tool_results,
                tool_result_artifacts,
                exposed_tools,
                memory_snapshot_chars,
                memory_snapshot_tokens,
                retrieval_items,
                retrieval_tokens,
                skill_list_chars,
                skill_list_tokens,
                route_scoped_tools,
                workflow_context,
                closeout_visibility,
                validation_evidence,
                warnings,
            } => {
                let total = if *total_request_tokens > 0 {
                    *total_request_tokens
                } else {
                    prompt_tokens.saturating_add(*tool_schema_tokens)
                };
                let level = runtime_diet_level(*prompt_tokens, *exposed_tools);
                let context_budget = match (remaining_context_tokens, max_context_tokens) {
                    (Some(remaining), Some(max)) => {
                        format!(" context_remaining={}/{}", remaining, max)
                    }
                    _ => String::new(),
                };
                format!(
                    "{} prompt={} tool_schema={} total={} tools={} tool_results={}ch/~{}t truncated={} artifacts={} memory={}ch/~{}t retrieval={}items/~{}t skills={}ch/~{}t route_scoped={} workflow={} closeout={} validation={} warnings={}{}",
                    level,
                    prompt_tokens,
                    tool_schema_tokens,
                    total,
                    exposed_tools,
                    tool_result_chars,
                    tool_result_tokens,
                    truncated_tool_results,
                    tool_result_artifacts,
                    memory_snapshot_chars,
                    memory_snapshot_tokens,
                    retrieval_items,
                    retrieval_tokens,
                    skill_list_chars,
                    skill_list_tokens,
                    route_scoped_tools,
                    workflow_context,
                    closeout_visibility,
                    validation_evidence,
                    compact_label_list(warnings),
                    context_budget
                )
            }
            TraceEvent::ApiRequestStarted {
                iteration,
                model,
                tools,
                provider_family,
                nonstreaming_tools_required,
                tool_result_adjacency_required,
            } => format!(
                "api request #{}: model={}, tools={}, provider={}, nonstreaming_tools={}, tool_adjacency={}",
                iteration,
                model,
                tools,
                provider_family.as_deref().unwrap_or("unknown"),
                nonstreaming_tools_required,
                tool_result_adjacency_required
            ),
            TraceEvent::ProviderMessageSequenceNormalized {
                provider_family,
                requires_tool_result_adjacency,
                requires_merged_system_messages,
                system_messages_merged,
                input_messages,
                output_messages,
                valid_tool_call_pairs,
                dropped_assistant_tool_calls,
                dropped_tool_results,
                valid_tool_call_ids,
                dropped_assistant_tool_call_ids,
                dropped_tool_result_ids,
            } => format!(
                "provider protocol normalized: provider={} messages={}->{} tool_pairs={} dropped_calls={} dropped_results={} merged_system={} adjacency={} merge_system={} valid_ids={} dropped_call_ids={} dropped_result_ids={}",
                provider_family,
                input_messages,
                output_messages,
                valid_tool_call_pairs,
                dropped_assistant_tool_calls,
                dropped_tool_results,
                system_messages_merged,
                requires_tool_result_adjacency,
                requires_merged_system_messages,
                compact_id_list(valid_tool_call_ids),
                compact_id_list(dropped_assistant_tool_call_ids),
                compact_id_list(dropped_tool_result_ids)
            ),
            TraceEvent::StreamingToolExecutionShadow {
                mode,
                provider_family,
                provider_supports_streaming_tool_calls,
                streamed_request_path,
                observed_tool_calls,
                read_only_tool_calls,
                concurrency_safe_tool_calls,
                eligible_tool_calls,
                schema_complete_tool_calls,
                latency_upper_bound_ms,
                reason,
            } => format!(
                "streaming tool shadow: mode={} provider={} supports_streaming_tools={} streamed_path={} calls={} read_only={} concurrency_safe={} schema_complete={} eligible={} latency_upper_bound={}ms reason={}",
                mode,
                provider_family,
                provider_supports_streaming_tool_calls,
                streamed_request_path,
                observed_tool_calls,
                read_only_tool_calls,
                concurrency_safe_tool_calls,
                schema_complete_tool_calls,
                eligible_tool_calls,
                latency_upper_bound_ms,
                preview(reason)
            ),
            TraceEvent::ApiRequestCompleted {
                iteration,
                tool_calls,
                content_chars,
            } => format!(
                "api response #{}: {} tool calls, {} chars",
                iteration, tool_calls, content_chars
            ),
            TraceEvent::ToolStarted {
                tool,
                call_id,
                parallel,
                pre_executed,
            } => format!(
                "{} {} started{}{}",
                tool,
                short_id(call_id),
                if *parallel { " in parallel" } else { "" },
                if *pre_executed { " (pre-executed)" } else { "" }
            ),
            TraceEvent::ActionDecisionEvaluated {
                tool,
                call_id,
                stage,
                value,
                risk,
                uncertainty_reduction,
                cost,
                reversibility,
                scope_fit,
                action_score,
                formula_stage,
                formula_version,
                phase_aligned,
                mutates_workspace,
                broad_shell,
                modifiers,
                requires_confirmation,
                reason,
            } => format!(
                "{} {} action decision: stage={} formula={}/{} score={} value={} risk={} uncertainty={} cost={} reversible={} scope_fit={} aligned={} mutates={} broad_shell={} modifiers={} confirm={} ({})",
                tool,
                short_id(call_id),
                stage,
                formula_stage,
                formula_version,
                action_score,
                value,
                risk,
                uncertainty_reduction,
                cost,
                reversibility,
                scope_fit,
                phase_aligned,
                mutates_workspace,
                broad_shell,
                modifiers.len(),
                requires_confirmation,
                preview(reason)
            ),
            TraceEvent::CandidateActionsEvaluated {
                mode,
                candidate_count,
                selected_id,
                selected_tool,
                selected_score,
                selected_runtime_score,
                selected_model_score,
                runtime_model_score_delta,
                runtime_selected_differs_from_model_order,
                calibration_reason,
                rejected,
                reason,
            } => format!(
                "candidate actions: mode={} count={} selected={} tool={} score={} runtime_score={} model_score={} delta={} differs={} rejected={} calibration={} ({})",
                mode,
                candidate_count,
                selected_id.as_deref().unwrap_or("none"),
                selected_tool.as_deref().unwrap_or("none"),
                selected_score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                selected_runtime_score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                selected_model_score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                runtime_model_score_delta
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                runtime_selected_differs_from_model_order,
                rejected,
                preview(calibration_reason),
                preview(reason)
            ),
            TraceEvent::ActionReviewed {
                tool,
                call_id,
                decision,
                reason,
                permission,
                scope_allowed,
                budget_allowed,
                checkpoint,
                network,
                external_effect,
                recovery,
            } => format!(
                "{} {} action review: decision={} reason={} permission={} scope_allowed={} budget_allowed={} checkpoint={} network={} external_effect={} recovery={}",
                tool,
                short_id(call_id),
                decision,
                reason,
                permission.as_deref().unwrap_or("none"),
                scope_allowed,
                budget_allowed,
                checkpoint,
                network,
                external_effect,
                preview(recovery)
            ),
            TraceEvent::PermissionRequested {
                tool,
                call_id,
                prompt,
                review,
            } => {
                let mut summary = format!(
                    "{} {} requested permission: {}",
                    tool,
                    short_id(call_id),
                    preview(prompt)
                );
                if let Some(review) = review {
                    summary.push_str(&format!(
                        " review={:?}/{}",
                        review.kind,
                        review.risk.as_str()
                    ));
                    if !review.risk_facts.is_empty() {
                        summary.push_str(&format!(" facts={}", review.risk_facts.join(",")));
                    }
                }
                summary
            }
            TraceEvent::PermissionResolved {
                tool,
                call_id,
                approved,
                source,
                decision,
                persistence_scope,
                rule_pattern,
                persisted_path,
                review,
            } => {
                let mut summary = format!(
                    "{} {} permission {}",
                    tool,
                    short_id(call_id),
                    if *approved { "approved" } else { "denied" }
                );
                if let Some(source) = source {
                    summary.push_str(&format!(" source={}", source));
                }
                if let Some(decision) = decision {
                    summary.push_str(&format!(" decision={}", decision));
                }
                if let Some(scope) = persistence_scope {
                    summary.push_str(&format!(" scope={}", scope));
                }
                if let Some(pattern) = rule_pattern {
                    summary.push_str(&format!(" rule={}", pattern));
                }
                if let Some(path) = persisted_path {
                    summary.push_str(&format!(" saved={}", path));
                }
                if let Some(review) = review {
                    if let Some(user_decision) = review.user_decision.as_deref() {
                        summary.push_str(&format!(" user_decision={}", user_decision));
                    }
                    if let Some(hint) = review.recovery_hint.as_deref() {
                        summary.push_str(&format!(" recovery={}", preview(hint)));
                    }
                }
                summary
            }
            TraceEvent::ToolCompleted {
                tool,
                call_id,
                success,
                duration_ms,
                output_chars,
            } => format!(
                "{} {} {} in {}ms ({} chars)",
                tool,
                short_id(call_id),
                if *success { "ok" } else { "failed" },
                duration_ms.unwrap_or_default(),
                output_chars
            ),
            TraceEvent::ToolObservationRecorded {
                tool,
                call_id,
                status,
                result_kind,
                model_visibility,
                include_in_next_context,
                store_in_state,
                key_findings,
                evidence_items,
                failure_type,
                recovery_plan_id,
                recovery_kind,
                raw_result_ref,
                quality_warnings,
                quality_warning_labels,
                files_read,
                files_changed,
                checkpoint_id,
                summary,
            } => format!(
                "{} {} observation: kind={} status={} visibility={} context={} state={} findings={} evidence={} warnings={} warning_labels={} failure={} recovery_plan={} recovery_kind={} files_read={} files_changed={} checkpoint={} raw={} ({})",
                tool,
                short_id(call_id),
                if result_kind.is_empty() {
                    "generic"
                } else {
                    result_kind.as_str()
                },
                status,
                if model_visibility.is_empty() {
                    "unknown"
                } else {
                    model_visibility.as_str()
                },
                include_in_next_context,
                store_in_state,
                key_findings,
                evidence_items,
                quality_warnings,
                compact_label_list(quality_warning_labels),
                failure_type.as_deref().unwrap_or("none"),
                recovery_plan_id.as_deref().unwrap_or("none"),
                recovery_kind.as_deref().unwrap_or("none"),
                files_read,
                files_changed,
                checkpoint_id.as_deref().unwrap_or("none"),
                raw_result_ref.as_deref().unwrap_or("none"),
                preview(summary)
            ),
            TraceEvent::HookCompleted {
                event,
                provider,
                hook_name,
                call_id,
                tool,
                success,
                blocked,
                duration_ms,
                error,
                output_preview,
            } => {
                let detail = error
                    .as_deref()
                    .or(output_preview.as_deref())
                    .map(preview)
                    .unwrap_or_else(|| "no output".to_string());
                format!(
                    "{} hook '{}' provider={} for {} {}{} in {}ms: {}",
                    event,
                    hook_name,
                    provider,
                    tool.as_deref().unwrap_or(call_id),
                    if *success { "ok" } else { "failed" },
                    if *blocked { " blocked" } else { "" },
                    duration_ms,
                    detail
                )
            }
            TraceEvent::SubagentStarted {
                agent_id,
                profile,
                role,
                description,
                timeout_secs,
                allowed_tools,
            } => format!(
                "subagent {} started role={} profile={} timeout={}s tools={} task={}",
                agent_id,
                role,
                profile.as_deref().unwrap_or("none"),
                timeout_secs,
                allowed_tools,
                preview(description)
            ),
            TraceEvent::SubagentCompleted {
                agent_id,
                status,
                duration_ms,
                output_chars,
                tools_used,
            } => format!(
                "subagent {} {} in {}ms ({} chars, {} tools)",
                agent_id, status, duration_ms, output_chars, tools_used
            ),
            TraceEvent::VerificationCompleted {
                changed_files,
                passed,
                check_passed,
                tests_passed,
                review_passed,
                failed_commands,
            } => format!(
                "verification {} for {} changed files (check={} tests={} review={} failed={})",
                if *passed { "passed" } else { "failed" },
                changed_files,
                check_passed,
                tests_passed,
                review_passed,
                failed_commands.len()
            ),
            TraceEvent::AcceptanceReviewCompleted {
                accepted,
                confidence,
                criteria,
                unresolved,
                next_action,
            } => format!(
                "acceptance accepted={} confidence={} criteria={} unresolved={} next={}",
                accepted, confidence, criteria, unresolved, next_action
            ),
            TraceEvent::GuidedDebuggingCompleted {
                blocker,
                next_action,
                causes,
                evidence_items,
                ask_user,
            } => format!(
                "guided debug blocker={} next={} causes={} evidence={} ask_user={}",
                blocker, next_action, causes, evidence_items, ask_user
            ),
            TraceEvent::RecoveryApplied { error, action } => {
                format!("recovery: {} -> {}", preview(error), action)
            }
            TraceEvent::RecoveryPlan {
                plan_id,
                source,
                category,
                failure_type,
                recovery_kind,
                action,
                retryable,
                safe_retry,
                allowed_alternatives,
                retry_budget,
                side_effect_uncertain,
                requires_user_decision,
                suggested_command,
                status,
            } => format!(
                "{} {} {} failure_type={} recovery_kind={} action={} retryable={} safe_retry={} alternatives={} retry_budget={} side_effect_uncertain={} requires_user={} suggested={} status={}",
                source,
                short_id(plan_id),
                category,
                if failure_type.is_empty() { "none" } else { failure_type },
                if recovery_kind.is_empty() { "none" } else { recovery_kind },
                preview(action),
                retryable,
                safe_retry,
                allowed_alternatives.len(),
                retry_budget
                    .map(|budget| budget.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                side_effect_uncertain,
                requires_user_decision,
                suggested_command.as_deref().unwrap_or("none"),
                status
            ),
            TraceEvent::McpResourceAccessed {
                server,
                uri,
                action,
                success,
                content_chars,
            } => format!(
                "{} resource {} on {} success={} ({} chars)",
                action,
                preview(uri),
                server,
                success,
                content_chars
            ),
            TraceEvent::RemoteBridgeAction {
                tool,
                call_id,
                action,
                target,
                risk,
                permission_hint,
                success,
                error_code,
            } => format!(
                "{} {} remote action={} target={} risk={} success={} error={} hint={}",
                tool,
                short_id(call_id),
                action,
                target.as_deref().unwrap_or("none"),
                risk,
                success,
                error_code.as_deref().unwrap_or("none"),
                preview(permission_hint)
            ),
            TraceEvent::AssistantResponded { chars, iterations } => {
                format!(
                    "assistant responded: {} chars, {} iterations",
                    chars, iterations
                )
            }
            TraceEvent::CompletionContractEvaluated {
                mode,
                workflow,
                status,
                terminal_status,
                requires_validation,
                verification_status,
                verification_proof_status,
                changed_files,
                reason,
            } => format!(
                "completion contract: mode={} workflow={} status={} terminal={} validation_required={} verification={} proof={} changed_files={} ({})",
                mode,
                workflow,
                status,
                terminal_status,
                requires_validation,
                verification_status,
                verification_proof_status,
                changed_files,
                preview(reason)
            ),
            TraceEvent::FinalCloseoutPrepared {
                status,
                terminal_status,
                stop_reason,
                stop_action,
                failure_type,
                recovery_plan_id,
                rollback_status,
                changed_files,
                validation_items,
                tool_records,
                tool_evidence,
                verification_proof_status,
                verification_proof_summary,
                verification_proof_kind_summary,
                verification_proof_support_status,
                verification_proof_support_summary,
                verification_proof_supports_verified,
                verification_proof_residual_risk,
                acceptance_items,
                residual_risks,
            } => format!(
                "final closeout status={} terminal={} stop_reason={} stop_action={} failure={} recovery={} rollback={} files={} validation={} tool_records={} tool_evidence={} proof={} proof_summary={} proof_kinds={} proof_support={} proof_supports_verified={} proof_residual_risk={} proof_support_summary={} acceptance={} risks={}",
                status,
                terminal_status.as_deref().unwrap_or("none"),
                stop_reason.as_deref().unwrap_or("none"),
                stop_action.as_deref().unwrap_or("none"),
                failure_type.as_deref().unwrap_or("none"),
                recovery_plan_id.as_deref().unwrap_or("none"),
                rollback_status.as_deref().unwrap_or("none"),
                changed_files,
                validation_items,
                tool_records,
                tool_evidence.as_deref().unwrap_or("none"),
                verification_proof_status.as_deref().unwrap_or("none"),
                verification_proof_summary.as_deref().unwrap_or("none"),
                verification_proof_kind_summary.as_deref().unwrap_or("none"),
                verification_proof_support_status.as_deref().unwrap_or("none"),
                verification_proof_supports_verified
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                verification_proof_residual_risk
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                verification_proof_support_summary.as_deref().unwrap_or("none"),
                acceptance_items,
                residual_risks
            ),
            TraceEvent::ExecutionReportPrepared {
                task_id,
                status,
                changed_files,
                validation_evidence,
                risks,
                next_steps,
            } => format!(
                "execution report {} status={} files={} validation={} risks={} next_steps={}",
                short_id(task_id),
                status,
                changed_files,
                validation_evidence,
                risks,
                next_steps
            ),
            TraceEvent::Error { message } => format!("error: {}", preview(message)),
        }
    }
}

#[derive(Clone)]
pub struct TraceCollector {
    inner: Arc<Mutex<TurnTrace>>,
}

impl TraceCollector {
    pub fn new(trace: TurnTrace) -> Self {
        Self {
            inner: Arc::new(Mutex::new(trace)),
        }
    }

    pub fn record(&self, event: TraceEvent) {
        let mut trace = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        trace.events.push(event);
    }

    pub fn snapshot(&self) -> TurnTrace {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn finish(&self, status: TurnStatus) -> TurnTrace {
        let mut trace = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        trace.finish(status);
        trace.clone()
    }
}

#[derive(Debug)]
pub struct TraceStore {
    max_traces: usize,
    traces: RwLock<VecDeque<TurnTrace>>,
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_TRACES)
    }
}

impl TraceStore {
    pub fn new(max_traces: usize) -> Self {
        Self {
            max_traces: max_traces.max(1),
            traces: RwLock::new(VecDeque::new()),
        }
    }

    pub fn push(&self, trace: TurnTrace) {
        let mut traces = self.traces.write().unwrap_or_else(|e| e.into_inner());
        traces.push_back(trace);
        while traces.len() > self.max_traces {
            traces.pop_front();
        }
    }

    pub fn latest(&self) -> Option<TurnTrace> {
        self.traces
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .back()
            .cloned()
    }

    pub fn recent(&self, limit: usize) -> Vec<TurnTrace> {
        self.traces
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.traces.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub fn format_trace_summary(trace: &TurnTrace, max_events: usize) -> String {
    let duration = trace
        .duration_ms()
        .map(|ms| format!("{}ms", ms))
        .unwrap_or_else(|| "running".to_string());
    let mut lines = vec![format!(
        "Trace {}\nSession: {}\nTurn: {}\nStatus: {:?}\nDuration: {}\nPrompt: {}",
        short_id(&trace.trace_id),
        trace.session_id,
        trace.turn_index,
        trace.status,
        duration,
        trace.user_message_preview
    )];
    if let Some(diet) = latest_runtime_diet_summary(trace) {
        lines.push(format!("\nRuntime Diet: {}", diet));
    }
    if let Some(tool_record_evidence) = latest_tool_record_evidence_summary(trace) {
        lines.push(format!("\nTool Record Evidence: {}", tool_record_evidence));
    }
    lines.push(format!(
        "\nControl Loop: {}",
        control_loop_diagnostic(trace).compact_summary()
    ));
    if let Some(action_reviews) = action_review_trace_summary(trace) {
        lines.push(format!(
            "\nAction Reviews: {}",
            action_reviews.compact_summary()
        ));
    }

    lines.push("\nEvents:".to_string());
    for (idx, event) in trace.events.iter().take(max_events).enumerate() {
        lines.push(format!(
            "{:>2}. {:<20} {}",
            idx + 1,
            event.label(),
            event.summary()
        ));
    }
    if trace.events.len() > max_events {
        lines.push(format!(
            "... {} more events",
            trace.events.len().saturating_sub(max_events)
        ));
    }

    lines.join("\n")
}

pub fn format_trace_recent_line(trace: &TurnTrace) -> String {
    let action_review_summary = action_review_trace_summary(trace)
        .map(|summary| format!(" action_reviews={}", summary.compact_summary()))
        .unwrap_or_default();
    format!(
        "- {} turn {} {:?} events={} tool_records={}{} prompt={}",
        short_id(&trace.trace_id),
        trace.turn_index,
        trace.status,
        trace.events.len(),
        latest_tool_record_count(trace).unwrap_or(0),
        action_review_summary,
        trace.user_message_preview
    )
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
        | TraceEvent::RetrievalContextBuilt { .. }
        | TraceEvent::ContextZonesMaterialized { .. }
        | TraceEvent::MemoryBoundaryEvaluated { .. }
        | TraceEvent::MemorySynced { .. }
        | TraceEvent::ContextCompacted { .. }
        | TraceEvent::RuntimeDietReport { .. }
        | TraceEvent::ApiRequestStarted { .. }
        | TraceEvent::ProviderMessageSequenceNormalized { .. }
        | TraceEvent::StreamingToolExecutionShadow { .. } => Some("context"),
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
mod tests {
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
        assert!(summary.contains("memory=180ch/~45t"));
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
}
