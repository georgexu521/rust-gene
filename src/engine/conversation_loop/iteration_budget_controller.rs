use super::turn_runtime_state::TurnRuntimeState;
use crate::services::api::ToolCall;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ToolRoundBudgetOutcome {
    pub(super) counted: bool,
}

pub(super) struct IterationBudgetController;

impl IterationBudgetController {
    pub(super) fn record_tool_round(
        turn_state: &mut TurnRuntimeState,
        tool_calls: &[ToolCall],
    ) -> ToolRoundBudgetOutcome {
        let counted = !tool_calls.is_empty();
        if counted {
            turn_state.effective_iterations += 1;
        }
        ToolRoundBudgetOutcome { counted }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(name: &str) -> ToolCall {
        ToolCall {
            id: format!("call_{}", name),
            name: name.to_string(),
            arguments: serde_json::json!({}),
        }
    }

    #[test]
    fn read_only_tool_round_counts_against_simple_iteration_budget() {
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;

        let outcome = IterationBudgetController::record_tool_round(
            &mut turn_state,
            &[tool_call("grep"), tool_call("file_read")],
        );

        assert_eq!(outcome, ToolRoundBudgetOutcome { counted: true });
        assert_eq!(turn_state.effective_iterations, 3);
    }

    #[test]
    fn write_tool_round_charges_iteration_budget() {
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;

        let outcome = IterationBudgetController::record_tool_round(
            &mut turn_state,
            &[tool_call("file_write")],
        );

        assert_eq!(outcome, ToolRoundBudgetOutcome { counted: true });
        assert_eq!(turn_state.effective_iterations, 3);
    }
}
