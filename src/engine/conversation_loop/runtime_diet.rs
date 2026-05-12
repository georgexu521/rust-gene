use crate::engine::context_compressor::estimate_tokens;
use crate::engine::intent_router::IntentRoute;
use crate::engine::trace::{TraceCollector, TraceEvent};

#[derive(Debug, Clone)]
pub(super) struct RuntimeDietSnapshot {
    pub(super) prompt_tokens: u64,
    pub(super) tool_schema_tokens: u64,
    pub(super) total_request_tokens: u64,
    pub(super) max_context_tokens: Option<u64>,
    pub(super) remaining_context_tokens: Option<u64>,
    pub(super) exposed_tools: usize,
    pub(super) memory_snapshot_chars: usize,
    pub(super) memory_snapshot_tokens: u64,
    pub(super) retrieval_items: usize,
    pub(super) retrieval_tokens: u64,
    pub(super) skill_list_chars: usize,
    pub(super) skill_list_tokens: u64,
    pub(super) route_scoped_tools: bool,
    pub(super) closeout_visibility: String,
    pub(super) validation_evidence: String,
}

impl RuntimeDietSnapshot {
    pub(super) fn new(route_scoped_tools: bool) -> Self {
        Self {
            prompt_tokens: 0,
            tool_schema_tokens: 0,
            total_request_tokens: 0,
            max_context_tokens: None,
            remaining_context_tokens: None,
            exposed_tools: 0,
            memory_snapshot_chars: 0,
            memory_snapshot_tokens: 0,
            retrieval_items: 0,
            retrieval_tokens: 0,
            skill_list_chars: 0,
            skill_list_tokens: 0,
            route_scoped_tools,
            closeout_visibility: "none".to_string(),
            validation_evidence: "none".to_string(),
        }
    }

    pub(super) fn observe_memory_snapshot(&mut self, snapshot: &str) {
        self.memory_snapshot_chars = self.memory_snapshot_chars.max(snapshot.chars().count());
        self.memory_snapshot_tokens = self.memory_snapshot_tokens.max(estimate_tokens(snapshot));
    }

    pub(super) fn observe_retrieval_context(
        &mut self,
        ctx: &crate::engine::retrieval_context::RetrievalContext,
    ) {
        self.retrieval_items = self.retrieval_items.max(ctx.items.len());
        self.retrieval_tokens = self.retrieval_tokens.max(ctx.token_estimate as u64);
    }

    pub(super) fn observe_skill_list_summary(&mut self, summary: &str) {
        self.skill_list_chars = self.skill_list_chars.max(summary.chars().count());
        self.skill_list_tokens = self.skill_list_tokens.max(estimate_tokens(summary));
    }
}

pub(super) fn trace_runtime_diet_report(
    trace: &TraceCollector,
    route: &IntentRoute,
    runner: &crate::engine::code_change_workflow::CodeChangeWorkflowRunner,
    snapshot: &RuntimeDietSnapshot,
) {
    trace.record(TraceEvent::RuntimeDietReport {
        prompt_tokens: snapshot.prompt_tokens,
        tool_schema_tokens: snapshot.tool_schema_tokens,
        total_request_tokens: snapshot.total_request_tokens,
        max_context_tokens: snapshot.max_context_tokens,
        remaining_context_tokens: snapshot.remaining_context_tokens,
        exposed_tools: snapshot.exposed_tools,
        memory_snapshot_chars: snapshot.memory_snapshot_chars,
        memory_snapshot_tokens: snapshot.memory_snapshot_tokens,
        retrieval_items: snapshot.retrieval_items,
        retrieval_tokens: snapshot.retrieval_tokens,
        skill_list_chars: snapshot.skill_list_chars,
        skill_list_tokens: snapshot.skill_list_tokens,
        route_scoped_tools: snapshot.route_scoped_tools,
        workflow_context: runtime_workflow_context_label(route, runner).to_string(),
        closeout_visibility: snapshot.closeout_visibility.clone(),
        validation_evidence: snapshot.validation_evidence.clone(),
    });
}

fn runtime_workflow_context_label(
    route: &IntentRoute,
    runner: &crate::engine::code_change_workflow::CodeChangeWorkflowRunner,
) -> &'static str {
    if !crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
        return "none";
    }
    if matches!(
        runner.policy.depth,
        crate::engine::code_change_workflow::WorkflowDepth::Strict
    ) {
        return "strict";
    }
    if runner.policy.require_workflow_judgment
        || runner.policy.require_stage_validation
        || runner.policy.reflection_blocks
    {
        return "guarded";
    }
    if !runner.adaptive_trigger_labels().is_empty() {
        return "adaptive";
    }
    "minimal"
}
