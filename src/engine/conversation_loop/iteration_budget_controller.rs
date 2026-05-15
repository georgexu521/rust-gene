use super::tool_execution::is_read_only;
use super::turn_runtime_state::TurnRuntimeState;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum IterationBudgetCheck {
    Continue,
    Stop {
        effective_iterations: usize,
        max_iterations: usize,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ToolRoundBudgetOutcome {
    pub(super) refunded: bool,
}

pub(super) struct IterationBudgetController;

impl IterationBudgetController {
    pub(super) fn check_before_request(
        turn_state: &mut TurnRuntimeState,
        max_iterations: usize,
        trace: &TraceCollector,
    ) -> IterationBudgetCheck {
        if turn_state.effective_iterations < max_iterations {
            return IterationBudgetCheck::Continue;
        }

        if turn_state.reserved_repair_rounds > 0 {
            turn_state.reserved_repair_rounds -= 1;
            trace.record(TraceEvent::WorkflowFallback {
                error: format!(
                    "using reserved repair round after validation failure (remaining={})",
                    turn_state.reserved_repair_rounds
                ),
            });
            return IterationBudgetCheck::Continue;
        }

        IterationBudgetCheck::Stop {
            effective_iterations: turn_state.effective_iterations,
            max_iterations,
        }
    }

    pub(super) fn record_tool_round(
        turn_state: &mut TurnRuntimeState,
        tool_calls: &[ToolCall],
    ) -> ToolRoundBudgetOutcome {
        let refunded = tool_calls
            .iter()
            .all(|tool_call| is_read_only(&tool_call.name));
        if !refunded {
            turn_state.effective_iterations += 1;
        }
        ToolRoundBudgetOutcome { refunded }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    fn tool_call(name: &str) -> ToolCall {
        ToolCall {
            id: format!("call_{}", name),
            name: name.to_string(),
            arguments: serde_json::json!({}),
        }
    }

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session", 1, "test"))
    }

    #[test]
    fn read_only_tool_round_refunds_iteration_budget() {
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;

        let outcome = IterationBudgetController::record_tool_round(
            &mut turn_state,
            &[tool_call("grep"), tool_call("file_read")],
        );

        assert_eq!(outcome, ToolRoundBudgetOutcome { refunded: true });
        assert_eq!(turn_state.effective_iterations, 2);
    }

    #[test]
    fn write_tool_round_charges_iteration_budget() {
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;

        let outcome = IterationBudgetController::record_tool_round(
            &mut turn_state,
            &[tool_call("file_write")],
        );

        assert_eq!(outcome, ToolRoundBudgetOutcome { refunded: false });
        assert_eq!(turn_state.effective_iterations, 3);
    }

    #[test]
    fn reserved_repair_round_allows_one_extra_request_and_records_trace() {
        let trace = trace();
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 3;
        turn_state.reserved_repair_rounds = 1;

        assert_eq!(
            IterationBudgetController::check_before_request(&mut turn_state, 3, &trace),
            IterationBudgetCheck::Continue
        );
        assert_eq!(turn_state.reserved_repair_rounds, 0);
        assert_eq!(
            IterationBudgetController::check_before_request(&mut turn_state, 3, &trace),
            IterationBudgetCheck::Stop {
                effective_iterations: 3,
                max_iterations: 3
            }
        );

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error.contains("using reserved repair round after validation failure (remaining=0)")
        )));
    }
}
