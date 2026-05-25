use crate::engine::context_compressor::estimate_tokens;
use crate::engine::intent_router::IntentRoute;
use crate::engine::trace::{
    TraceCollector, TraceEvent, RUNTIME_DIET_PROMPT_TOKEN_BUDGET, RUNTIME_DIET_TOOL_COUNT_BUDGET,
};

#[derive(Debug, Clone)]
pub(super) struct RuntimeDietSnapshot {
    pub(super) prompt_tokens: u64,
    pub(super) tool_schema_tokens: u64,
    pub(super) total_request_tokens: u64,
    pub(super) max_context_tokens: Option<u64>,
    pub(super) remaining_context_tokens: Option<u64>,
    pub(super) tool_result_chars: usize,
    pub(super) tool_result_tokens: u64,
    pub(super) truncated_tool_results: usize,
    pub(super) tool_result_artifacts: usize,
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
    pub(super) warnings: Vec<String>,
}

impl RuntimeDietSnapshot {
    pub(super) fn new(route_scoped_tools: bool) -> Self {
        Self {
            prompt_tokens: 0,
            tool_schema_tokens: 0,
            total_request_tokens: 0,
            max_context_tokens: None,
            remaining_context_tokens: None,
            tool_result_chars: 0,
            tool_result_tokens: 0,
            truncated_tool_results: 0,
            tool_result_artifacts: 0,
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
            warnings: Vec::new(),
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
    let warnings = runtime_diet_warnings(snapshot);
    trace.record(TraceEvent::RuntimeDietReport {
        prompt_tokens: snapshot.prompt_tokens,
        tool_schema_tokens: snapshot.tool_schema_tokens,
        total_request_tokens: snapshot.total_request_tokens,
        max_context_tokens: snapshot.max_context_tokens,
        remaining_context_tokens: snapshot.remaining_context_tokens,
        tool_result_chars: snapshot.tool_result_chars,
        tool_result_tokens: snapshot.tool_result_tokens,
        truncated_tool_results: snapshot.truncated_tool_results,
        tool_result_artifacts: snapshot.tool_result_artifacts,
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
        warnings,
    });
}

pub(super) fn runtime_diet_warnings(snapshot: &RuntimeDietSnapshot) -> Vec<String> {
    let mut warnings = snapshot.warnings.clone();
    if snapshot.prompt_tokens > RUNTIME_DIET_PROMPT_TOKEN_BUDGET {
        warnings.push("prompt_budget_heavy".to_string());
    }
    if snapshot.exposed_tools > RUNTIME_DIET_TOOL_COUNT_BUDGET {
        warnings.push("exposed_tool_schema_heavy".to_string());
    }
    if snapshot.truncated_tool_results > snapshot.tool_result_artifacts {
        warnings.push("truncated_without_artifact".to_string());
    }
    if snapshot.tool_result_tokens > snapshot.prompt_tokens.max(1) {
        warnings.push("tool_result_tokens_exceed_prompt".to_string());
    }
    warnings.sort();
    warnings.dedup();
    warnings
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_diet_warnings_name_context_waste_signals() {
        let mut snapshot = RuntimeDietSnapshot::new(true);
        snapshot.prompt_tokens = RUNTIME_DIET_PROMPT_TOKEN_BUDGET + 1;
        snapshot.exposed_tools = RUNTIME_DIET_TOOL_COUNT_BUDGET + 1;
        snapshot.truncated_tool_results = 2;
        snapshot.tool_result_artifacts = 1;
        snapshot.tool_result_tokens = snapshot.prompt_tokens + 1;

        let warnings = runtime_diet_warnings(&snapshot);

        assert!(warnings.contains(&"prompt_budget_heavy".to_string()));
        assert!(warnings.contains(&"exposed_tool_schema_heavy".to_string()));
        assert!(warnings.contains(&"truncated_without_artifact".to_string()));
        assert!(warnings.contains(&"tool_result_tokens_exceed_prompt".to_string()));
    }
}
