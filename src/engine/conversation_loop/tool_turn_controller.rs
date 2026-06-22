//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::context_budget_controller::ContextBudgetController;
use super::runtime_diet::RuntimeDietSnapshot;
use super::tool_result_controller::{
    append_provider_tool_result, NormalizedToolResult, ProviderToolResultAppendContext,
};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::services::api::{Message, ToolCall};
use crate::tools::ToolResult;
use std::path::Path;

pub(super) struct ToolTurnAppendContext<'a> {
    pub(super) evidence_ledger: &'a mut EvidenceLedger,
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) session_id: Option<&'a str>,
    pub(super) working_dir: &'a Path,
    pub(super) store: Option<&'a crate::session_store::SessionStore>,
}

pub(super) struct ToolTurnController;

impl ToolTurnController {
    pub(super) async fn append_tool_result(
        tool_call: &ToolCall,
        result: &mut ToolResult,
        context: ToolTurnAppendContext<'_>,
    ) -> NormalizedToolResult {
        let normalized = append_provider_tool_result(
            tool_call,
            result,
            ProviderToolResultAppendContext {
                evidence_ledger: context.evidence_ledger,
                tool_results_text: context.tool_results_text,
                messages: context.messages,
                session_id: context.session_id,
                working_dir: context.working_dir,
                store: context.store,
            },
        )
        .await;
        let result_budget = ContextBudgetController::observe_tool_result(&normalized);
        ContextBudgetController::record_tool_result_runtime_diet(
            context.runtime_diet,
            &result_budget,
        );
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bash_tool_call() -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "cargo test -q"}),
        }
    }

    #[tokio::test]
    async fn appends_result_and_records_runtime_budget() {
        let mut evidence_ledger = EvidenceLedger::new();
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let mut result = ToolResult::success("ok");

        let normalized = ToolTurnController::append_tool_result(
            &bash_tool_call(),
            &mut result,
            ToolTurnAppendContext {
                evidence_ledger: &mut evidence_ledger,
                runtime_diet: &mut runtime_diet,
                tool_results_text: &mut tool_results_text,
                messages: &mut messages,
                session_id: Some("session-test"),
                working_dir: std::path::Path::new("."),
                store: None,
            },
        )
        .await;

        assert_eq!(normalized.model_content, "Result: OK\nok");
        assert_eq!(tool_results_text, "Result: OK\nok\n");
        assert_eq!(messages.len(), 1);
        assert_eq!(evidence_ledger.snapshot().command_facts, 1);
        assert_eq!(evidence_ledger.snapshot().validation_facts, 1);
        assert!(runtime_diet.tool_result_chars > 0);
        assert!(runtime_diet.tool_result_tokens > 0);
    }
}
