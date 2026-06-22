//! Tool execution controller support.
//!
//! Separates execution gates, runtime context, and batch state from the conversation-loop control flow.

use super::super::tool_call_lifecycle::{ToolCallLifecycleRecord, ToolCallStatus};
use super::super::tool_metadata::merge_tool_result_metadata;
use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use std::collections::HashSet;

pub(in crate::engine::conversation_loop) type ToolExecutionResults = Vec<(ToolCall, ToolResult)>;
pub(in crate::engine::conversation_loop) type ToolLifecycleRecords =
    Vec<(String, ToolCallLifecycleRecord)>;

#[derive(Debug, Clone, Default)]
pub(in crate::engine::conversation_loop) struct ToolExecutionBatch {
    results: ToolExecutionResults,
    lifecycle: ToolLifecycleRecords,
}

impl ToolExecutionBatch {
    pub(in crate::engine::conversation_loop) fn new(
        results: ToolExecutionResults,
        lifecycle: ToolLifecycleRecords,
    ) -> Self {
        let (results, lifecycle) = complete_provider_result_pairs(results, lifecycle);
        Self { results, lifecycle }
    }

    #[cfg(test)]
    pub(in crate::engine::conversation_loop) fn results(&self) -> &[(ToolCall, ToolResult)] {
        &self.results
    }

    pub(in crate::engine::conversation_loop) fn results_mut(
        &mut self,
    ) -> &mut [(ToolCall, ToolResult)] {
        &mut self.results
    }

    pub(in crate::engine::conversation_loop) fn any_success(&self) -> bool {
        self.results.iter().any(|(_, result)| result.success)
    }

    pub(in crate::engine::conversation_loop) fn unsuccessful_count(&self) -> usize {
        self.results
            .iter()
            .filter(|(_, result)| !result.success)
            .count()
    }

    pub(in crate::engine::conversation_loop) fn result_successes(
        &self,
    ) -> impl Iterator<Item = (&ToolCall, bool)> {
        self.results
            .iter()
            .map(|(tool_call, result)| (tool_call, result.success))
    }

    pub(in crate::engine::conversation_loop) fn denied_count(&self) -> usize {
        self.lifecycle
            .iter()
            .filter(|(_, record)| record.status == ToolCallStatus::Denied)
            .count()
    }

    pub(in crate::engine::conversation_loop) fn failed_count(&self) -> usize {
        self.lifecycle
            .iter()
            .filter(|(_, record)| record.status == ToolCallStatus::Failed)
            .count()
    }

    pub(in crate::engine::conversation_loop) fn pre_executed_count(&self) -> usize {
        self.lifecycle
            .iter()
            .filter(|(_, record)| record.pre_executed)
            .count()
    }
}

fn complete_provider_result_pairs(
    mut results: ToolExecutionResults,
    mut lifecycle: ToolLifecycleRecords,
) -> (ToolExecutionResults, ToolLifecycleRecords) {
    let result_ids = results
        .iter()
        .map(|(tool_call, _)| tool_call.id.clone())
        .collect::<HashSet<_>>();

    for (call_id, record) in &mut lifecycle {
        if result_ids.contains(call_id) {
            continue;
        }

        let status = record.status;
        let mut result = ToolResult::error(format!(
            "Tool '{}' ended with lifecycle status {:?} but no terminal result was recorded. Treating it as interrupted.",
            record.tool_name, status
        ));
        merge_tool_result_metadata(
            &mut result,
            "tool_lifecycle_recovery",
            serde_json::json!({
                "schema": "tool_lifecycle_recovery.v1",
                "call_id": call_id,
                "tool": record.tool_name.clone(),
                "previous_status": format!("{:?}", status),
                "terminal_result": "interrupted",
                "synthesized": true,
            }),
        );
        results.push((
            ToolCall {
                id: call_id.clone(),
                name: record.tool_name.clone(),
                arguments: serde_json::json!({}),
            },
            result,
        ));
        record.status = ToolCallStatus::Failed;
    }

    (results, lifecycle)
}
