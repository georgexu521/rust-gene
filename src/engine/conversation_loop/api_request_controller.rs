use super::session_processor::SessionStepResult;
use super::turn_recording::record_recovery_plan;
use super::{should_use_nonstreaming_tools, ConversationLoop};
use crate::engine::context_compressor::estimate_messages_tokens;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{ChatRequest, Message, Tool, ToolCall};
use crate::tools::ToolResult;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tracing::{debug, warn};

pub(super) struct ApiRequestContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) request: ChatRequest,
    pub(super) messages: &'a [Message],
    pub(super) tools: &'a [Tool],
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) iteration: usize,
}

pub(super) struct ApiRequestOutcome {
    pub(super) session_step: SessionStepResult,
    pub(super) compressed_this_turn: bool,
}

pub(super) struct ApiRequestApplicationContext<'a> {
    pub(super) outcome: ApiRequestOutcome,
    pub(super) final_content: &'a mut String,
    pub(super) final_tool_calls: &'a mut Vec<ToolCall>,
    pub(super) tool_calls_made: &'a mut bool,
    pub(super) trace: &'a TraceCollector,
    pub(super) iteration: usize,
}

pub(super) struct ApiRequestApplication {
    pub(super) content: String,
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) pre_executed: HashMap<usize, ToolResult>,
}

pub(super) struct ApiRequestController;

impl ApiRequestController {
    pub(super) async fn execute(context: ApiRequestContext<'_>) -> Result<ApiRequestOutcome> {
        let mut request = context.request;
        let mut compressed_this_turn = false;
        let mut api_result = Err(anyhow::anyhow!("initial"));

        for compress_retry in 0..3 {
            context.trace.record(TraceEvent::ApiRequestStarted {
                iteration: context.iteration,
                model: context.conversation.model.clone(),
                tools: context.tools.len(),
            });
            let nonstreaming_tool_request = context.tx.is_some()
                && should_use_nonstreaming_tools(
                    context.conversation.provider.as_ref(),
                    context.tools,
                );
            api_result = if let Some(tx) = context.tx {
                if nonstreaming_tool_request {
                    context.trace.record(TraceEvent::WorkflowFallback {
                        error: "provider stream is incompatible with tool/usage chunks; using non-streaming tool request".to_string(),
                    });
                    context.conversation.call_api(request.clone()).await
                } else {
                    context
                        .conversation
                        .call_api_streaming(
                            request.clone(),
                            tx,
                            context.trace,
                            context.exposed_tool_names,
                        )
                        .await
                }
            } else {
                context.conversation.call_api(request.clone()).await
            };

            match &api_result {
                Ok(_) => break,
                Err(error) => {
                    if Self::is_context_size_error(error) && compress_retry < 2 {
                        let classified =
                            crate::engine::error_classifier::ErrorClassifier::from_anyhow(error);
                        let plan = crate::engine::recovery_plan::RecoveryPlan::from_classified(
                            "api_reactive_compress",
                            &classified,
                        )
                        .with_status(crate::engine::recovery_plan::RecoveryStatus::Applied);
                        record_recovery_plan(context.trace, &plan);
                        warn!(
                            "API error (attempt {}/3): {}. Compressing context and retrying...",
                            compress_retry + 1,
                            error
                        );
                        if let Some(ref compressor) = context.conversation.compressor {
                            let messages_for_compression = if compress_retry == 0 {
                                context.messages.to_vec()
                            } else {
                                let mut compressor = compressor.lock().await;
                                compressor.micro_compress(context.messages)
                            };
                            let compressed = compressor
                                .lock()
                                .await
                                .compress_async(&messages_for_compression)
                                .await;
                            context.trace.record(TraceEvent::ContextCompacted {
                                before_tokens: estimate_messages_tokens(&messages_for_compression)
                                    as usize,
                                after_tokens: estimate_messages_tokens(&compressed) as usize,
                                strategy: "reactive".to_string(),
                            });
                            request = ChatRequest::new(&context.conversation.model)
                                .with_messages(compressed)
                                .with_tools(context.tools.to_vec())
                                .with_temperature(0.2);
                            compressed_this_turn = true;
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        let session_step = api_result?;
        debug!(
            "Session step completed: source={:?}, finish_reason={:?}, usage={:?}",
            session_step.source,
            session_step.finish_reason,
            session_step.usage.as_ref().map(|usage| {
                (
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens,
                )
            })
        );
        Ok(ApiRequestOutcome {
            session_step,
            compressed_this_turn,
        })
    }

    pub(super) fn apply_outcome(
        context: ApiRequestApplicationContext<'_>,
    ) -> ApiRequestApplication {
        let session_step = context.outcome.session_step;
        let content = session_step.assistant_text;
        let tool_calls = session_step.tool_calls;
        let pre_executed = session_step.pre_executed_results;
        context.trace.record(TraceEvent::ApiRequestCompleted {
            iteration: context.iteration,
            tool_calls: tool_calls.len(),
            content_chars: content.chars().count(),
        });

        if context.outcome.compressed_this_turn {
            debug!("Context compressed due to size limits");
        }

        *context.final_content = content.clone();
        *context.final_tool_calls = tool_calls.clone();
        if !tool_calls.is_empty() {
            *context.tool_calls_made = true;
        }

        ApiRequestApplication {
            content,
            tool_calls,
            pre_executed,
        }
    }

    fn is_context_size_error(error: &anyhow::Error) -> bool {
        let text = error.to_string().to_lowercase();
        text.contains("payload too large")
            || text.contains("413")
            || text.contains("context")
            || text.contains("too many tokens")
            || text.contains("maximum context length")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    #[test]
    fn context_size_errors_are_detected() {
        assert!(ApiRequestController::is_context_size_error(
            &anyhow::anyhow!("payload too large")
        ));
        assert!(ApiRequestController::is_context_size_error(
            &anyhow::anyhow!("maximum context length exceeded")
        ));
        assert!(ApiRequestController::is_context_size_error(
            &anyhow::anyhow!("HTTP 413")
        ));
        assert!(!ApiRequestController::is_context_size_error(
            &anyhow::anyhow!("permission denied")
        ));
    }

    #[test]
    fn apply_outcome_updates_loop_state_and_records_trace() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "api request"));
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({ "command": "cargo check -q" }),
        };
        let outcome = ApiRequestOutcome {
            session_step: SessionStepResult {
                assistant_text: "running check".to_string(),
                tool_calls: vec![tool_call.clone()],
                pre_executed_results: HashMap::new(),
                usage: None,
                finish_reason: None,
                source: super::super::session_processor::SessionStepSource::NonStreaming,
            },
            compressed_this_turn: true,
        };
        let mut final_content = String::new();
        let mut final_tool_calls = Vec::new();
        let mut tool_calls_made = false;

        let application = ApiRequestController::apply_outcome(ApiRequestApplicationContext {
            outcome,
            final_content: &mut final_content,
            final_tool_calls: &mut final_tool_calls,
            tool_calls_made: &mut tool_calls_made,
            trace: &trace,
            iteration: 2,
        });

        assert_eq!(application.content, "running check");
        assert_eq!(application.tool_calls.len(), 1);
        assert_eq!(application.tool_calls[0].id, tool_call.id);
        assert_eq!(application.tool_calls[0].name, tool_call.name);
        assert!(application.pre_executed.is_empty());
        assert_eq!(final_content, "running check");
        assert_eq!(final_tool_calls.len(), 1);
        assert_eq!(final_tool_calls[0].id, "call-1");
        assert_eq!(final_tool_calls[0].name, "bash");
        assert!(tool_calls_made);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::ApiRequestCompleted {
                iteration: 2,
                tool_calls: 1,
                content_chars: 13,
            }
        )));
    }
}
