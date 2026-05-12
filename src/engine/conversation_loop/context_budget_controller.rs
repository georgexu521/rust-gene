use super::runtime_diet::RuntimeDietSnapshot;
use crate::engine::context_compressor::{
    estimate_messages_tokens, estimate_tool_schemas_tokens, ContextCompressor,
};
use crate::services::api::{Message, Tool};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RequestBudgetObservation {
    pub(super) message_tokens: u64,
    pub(super) tool_schema_tokens: u64,
    pub(super) total_request_tokens: u64,
    pub(super) exposed_tools: usize,
    pub(super) max_context_tokens: Option<u64>,
    pub(super) remaining_context_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreflightBudgetDecision {
    pub(super) observation: RequestBudgetObservation,
    pub(super) should_compact: bool,
}

pub(super) struct ContextBudgetController;

impl ContextBudgetController {
    pub(super) fn observe_request(
        messages: &[Message],
        tools: &[Tool],
    ) -> RequestBudgetObservation {
        Self::observe_request_with_context_limit(messages, tools, None)
    }

    pub(super) fn observe_preflight(
        compressor: &ContextCompressor,
        messages: &[Message],
        tools: &[Tool],
    ) -> PreflightBudgetDecision {
        let stats = compressor.stats();
        let observation = Self::observe_request_with_context_limit(
            messages,
            tools,
            Some(stats.max_context_tokens),
        );
        let should_compact =
            compressor.preflight_check(messages, 0, observation.tool_schema_tokens);
        PreflightBudgetDecision {
            observation,
            should_compact,
        }
    }

    pub(super) fn record_runtime_diet(
        snapshot: &mut RuntimeDietSnapshot,
        observation: &RequestBudgetObservation,
    ) {
        snapshot.prompt_tokens = snapshot.prompt_tokens.max(observation.message_tokens);
        snapshot.tool_schema_tokens = snapshot
            .tool_schema_tokens
            .max(observation.tool_schema_tokens);
        snapshot.exposed_tools = snapshot.exposed_tools.max(observation.exposed_tools);
        snapshot.total_request_tokens = snapshot
            .total_request_tokens
            .max(observation.total_request_tokens);
        if let Some(max_context_tokens) = observation.max_context_tokens {
            snapshot.max_context_tokens = Some(
                snapshot
                    .max_context_tokens
                    .map_or(max_context_tokens, |current| {
                        current.max(max_context_tokens)
                    }),
            );
        }
        if let Some(remaining_context_tokens) = observation.remaining_context_tokens {
            snapshot.remaining_context_tokens = Some(
                snapshot
                    .remaining_context_tokens
                    .map_or(remaining_context_tokens, |current| {
                        current.min(remaining_context_tokens)
                    }),
            );
        }
    }

    fn observe_request_with_context_limit(
        messages: &[Message],
        tools: &[Tool],
        max_context_tokens: Option<u64>,
    ) -> RequestBudgetObservation {
        let message_tokens = estimate_messages_tokens(messages);
        let tool_schema_tokens = estimate_tool_schemas_tokens(tools);
        let total_request_tokens = message_tokens.saturating_add(tool_schema_tokens);
        let remaining_context_tokens =
            max_context_tokens.map(|max| max.saturating_sub(total_request_tokens));
        RequestBudgetObservation {
            message_tokens,
            tool_schema_tokens,
            total_request_tokens,
            exposed_tools: tools.len(),
            max_context_tokens,
            remaining_context_tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tool() -> Tool {
        Tool {
            name: "bash".to_string(),
            description: "Run a shell command".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"}
                },
                "required": ["command"]
            }),
        }
    }

    #[test]
    fn observes_request_budget_from_messages_and_tools() {
        let messages = vec![Message::system("system"), Message::user("hello world")];
        let tools = vec![sample_tool()];

        let observation = ContextBudgetController::observe_request(&messages, &tools);

        assert!(observation.message_tokens > 0);
        assert!(observation.tool_schema_tokens > 0);
        assert_eq!(observation.exposed_tools, 1);
        assert_eq!(
            observation.total_request_tokens,
            observation.message_tokens + observation.tool_schema_tokens
        );
        assert_eq!(observation.max_context_tokens, None);
    }

    #[test]
    fn preflight_observation_reports_remaining_context() {
        let compressor = ContextCompressor::new(1_000);
        let messages = vec![Message::user("hello")];
        let tools = vec![sample_tool()];

        let decision = ContextBudgetController::observe_preflight(&compressor, &messages, &tools);

        assert_eq!(decision.observation.max_context_tokens, Some(1_000));
        assert!(decision.observation.remaining_context_tokens.unwrap() < 1_000);
        assert!(!decision.should_compact);
    }

    #[test]
    fn record_runtime_diet_keeps_peak_usage_and_lowest_remaining_context() {
        let mut snapshot = RuntimeDietSnapshot::new(true);
        ContextBudgetController::record_runtime_diet(
            &mut snapshot,
            &RequestBudgetObservation {
                message_tokens: 100,
                tool_schema_tokens: 20,
                total_request_tokens: 120,
                exposed_tools: 2,
                max_context_tokens: Some(1_000),
                remaining_context_tokens: Some(880),
            },
        );
        ContextBudgetController::record_runtime_diet(
            &mut snapshot,
            &RequestBudgetObservation {
                message_tokens: 90,
                tool_schema_tokens: 40,
                total_request_tokens: 130,
                exposed_tools: 1,
                max_context_tokens: Some(1_000),
                remaining_context_tokens: Some(870),
            },
        );

        assert_eq!(snapshot.prompt_tokens, 100);
        assert_eq!(snapshot.tool_schema_tokens, 40);
        assert_eq!(snapshot.exposed_tools, 2);
        assert_eq!(snapshot.total_request_tokens, 130);
        assert_eq!(snapshot.max_context_tokens, Some(1_000));
        assert_eq!(snapshot.remaining_context_tokens, Some(870));
    }
}
