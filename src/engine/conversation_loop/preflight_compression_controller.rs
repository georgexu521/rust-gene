use super::context_budget_controller::ContextBudgetController;
use super::runtime_diet::RuntimeDietSnapshot;
use crate::engine::context_collapse::ContextCompactionStrategy;
use crate::engine::context_compressor::{estimate_messages_tokens, ContextCompressor};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{Message, Tool};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};

pub(super) struct PreflightCompressionContext<'a> {
    pub(super) compressor: Option<&'a Arc<Mutex<ContextCompressor>>>,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) tools: &'a [Tool],
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) trace: &'a TraceCollector,
}

pub(super) struct PreflightCompressionController;

impl PreflightCompressionController {
    pub(super) async fn run(context: PreflightCompressionContext<'_>) {
        let Some(compressor_mutex) = context.compressor else {
            return;
        };

        let mut no_gain_passes = 0u8;
        for pass in 0..3 {
            let compressor = compressor_mutex.lock().await;
            let preflight = ContextBudgetController::observe_preflight(
                &compressor,
                context.messages,
                context.tools,
            );
            ContextBudgetController::record_runtime_diet(
                context.runtime_diet,
                &preflight.observation,
            );
            if !preflight.should_compact {
                break;
            }
            debug!(
                "Preflight compression pass {}/3 ({} msg + {} tool tokens)",
                pass + 1,
                preflight.observation.message_tokens,
                preflight.observation.tool_schema_tokens
            );
            drop(compressor);
            let before_tokens = preflight.observation.message_tokens;
            let mut compressor = compressor_mutex.lock().await;
            let compaction_record_len = compressor.compaction_records().len();
            *context.messages = compressor
                .compress_async_with_strategy(
                    context.messages,
                    ContextCompactionStrategy::AutoCompact,
                )
                .await;
            let compaction_record = compressor
                .compaction_records()
                .get(compaction_record_len)
                .cloned();
            drop(compressor);
            let after_tokens = estimate_messages_tokens(context.messages);
            let mut provenance = compaction_record
                .as_ref()
                .map(|record| record.provenance.clone())
                .unwrap_or_default();
            provenance.push("trigger:preflight".to_string());
            context.trace.record(TraceEvent::ContextCompacted {
                before_tokens: before_tokens as usize,
                after_tokens: after_tokens as usize,
                strategy: compaction_record
                    .as_ref()
                    .map(|record| record.strategy.label().to_string())
                    .unwrap_or_else(|| "auto_compact".to_string()),
                boundary_id: compaction_record
                    .as_ref()
                    .and_then(|record| record.boundary_id.clone()),
                sequence: compaction_record
                    .as_ref()
                    .and_then(|record| record.sequence),
                messages_before: compaction_record
                    .as_ref()
                    .map(|record| record.messages_before),
                messages_after: compaction_record
                    .as_ref()
                    .map(|record| record.messages_after),
                preserved_tail_count: compaction_record
                    .as_ref()
                    .and_then(|record| record.preserved_tail_count),
                provenance,
            });
            if after_tokens >= before_tokens {
                no_gain_passes += 1;
                if no_gain_passes >= 2 {
                    warn!(
                        "Preflight compression made no progress for 2 consecutive passes ({} -> {}). Stop retrying this turn.",
                        before_tokens, after_tokens
                    );
                    break;
                }
            } else {
                no_gain_passes = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::TurnTrace;

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: "tool".to_string(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        }
    }

    #[tokio::test]
    async fn records_preflight_budget_when_compressor_is_available() {
        let compressor = Arc::new(Mutex::new(ContextCompressor::new(1_000)));
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "test"));
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let mut messages = vec![Message::user("hello")];
        let tools = vec![tool("file_read")];

        PreflightCompressionController::run(PreflightCompressionContext {
            compressor: Some(&compressor),
            messages: &mut messages,
            tools: &tools,
            runtime_diet: &mut runtime_diet,
            trace: &trace,
        })
        .await;

        assert_eq!(messages.len(), 1);
        assert!(matches!(
            &messages[0],
            Message::User { content } if content == "hello"
        ));
        assert_eq!(runtime_diet.exposed_tools, 1);
        assert!(runtime_diet.prompt_tokens > 0);
        assert!(runtime_diet.total_request_tokens >= runtime_diet.prompt_tokens);
    }

    #[tokio::test]
    async fn skips_when_compressor_is_absent() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "test"));
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let mut messages = vec![Message::user("hello")];

        PreflightCompressionController::run(PreflightCompressionContext {
            compressor: None,
            messages: &mut messages,
            tools: &[],
            runtime_diet: &mut runtime_diet,
            trace: &trace,
        })
        .await;

        assert_eq!(messages.len(), 1);
        assert!(matches!(
            &messages[0],
            Message::User { content } if content == "hello"
        ));
        assert_eq!(runtime_diet.total_request_tokens, 0);
    }
}
