use super::session_processor::SessionStepResult;
use super::turn_recording::record_recovery_plan;
use super::ConversationLoop;
use crate::engine::context_collapse::ContextCompactionStrategy;
use crate::engine::context_compressor::estimate_messages_tokens;
use crate::engine::error_classifier::{ClassifiedError, ErrorCategory};
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::provider_protocol::ProviderCapabilities;
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
    pub(super) resource_policy: &'a ResourcePolicy,
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
        let mut fallback_attempted = false;
        let mut api_result = Err(anyhow::anyhow!("initial"));

        for compress_retry in 0..3 {
            let provider_capabilities = ProviderCapabilities::detect(
                context.conversation.provider.base_url(),
                &request.model,
            );
            context.trace.record(TraceEvent::ApiRequestStarted {
                iteration: context.iteration,
                model: request.model.clone(),
                tools: context.tools.len(),
                provider_family: Some(provider_capabilities.protocol_family.label().to_string()),
                nonstreaming_tools_required: provider_capabilities.requires_nonstreaming_tool_calls,
                tool_result_adjacency_required: provider_capabilities
                    .requires_tool_result_adjacency,
            });
            let nonstreaming_tool_request = context.tx.is_some()
                && !context.tools.is_empty()
                && provider_capabilities.requires_nonstreaming_tool_calls;
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
                    let mut recovered = false;
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
                            let mut compressor = compressor.lock().await;
                            let compaction_record_len = compressor.compaction_records().len();
                            let compressed = compressor
                                .compress_async_with_strategy(
                                    &messages_for_compression,
                                    ContextCompactionStrategy::ReactiveCompact,
                                )
                                .await;
                            compressor.annotate_compaction_record_trigger(
                                compaction_record_len,
                                "api_context_error",
                            );
                            let compaction_record = compressor
                                .compaction_records()
                                .get(compaction_record_len)
                                .cloned();
                            drop(compressor);
                            let mut provenance = compaction_record
                                .as_ref()
                                .map(|record| record.provenance.clone())
                                .unwrap_or_default();
                            provenance.push("trigger:api_context_error".to_string());
                            context.trace.record(TraceEvent::ContextCompacted {
                                before_tokens: estimate_messages_tokens(&messages_for_compression)
                                    as usize,
                                after_tokens: estimate_messages_tokens(&compressed) as usize,
                                strategy: compaction_record
                                    .as_ref()
                                    .map(|record| record.strategy.label().to_string())
                                    .unwrap_or_else(|| "reactive_compact".to_string()),
                                trigger: Some("api_context_error".to_string()),
                                token_pressure: compaction_record.as_ref().and_then(|record| {
                                    record
                                        .token_pressure
                                        .map(|pressure| pressure.label().to_string())
                                }),
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
                                retained_items: compaction_record
                                    .as_ref()
                                    .map(|record| record.retained_items.clone())
                                    .unwrap_or_default(),
                                provenance,
                            });
                            request = ChatRequest::new(&context.conversation.model)
                                .with_messages(compressed)
                                .with_tools(context.tools.to_vec())
                                .with_temperature(0.2);
                            compressed_this_turn = true;
                            recovered = true;
                        }
                    }

                    if recovered {
                        continue;
                    }

                    if let Some((fallback_model, classified)) = Self::fallback_model_for_error(
                        error,
                        context.resource_policy,
                        &request.model,
                        fallback_attempted,
                    ) {
                        let plan = crate::engine::recovery_plan::RecoveryPlan::fallback_model(
                            "api_request",
                            &classified.message,
                            &fallback_model,
                        )
                        .with_status(crate::engine::recovery_plan::RecoveryStatus::Applied);
                        record_recovery_plan(context.trace, &plan);
                        context.trace.record(TraceEvent::WorkflowFallback {
                            error: format!(
                                "provider error category={} triggered fallback model {}",
                                classified.category, fallback_model
                            ),
                        });
                        request.model = fallback_model;
                        fallback_attempted = true;
                        continue;
                    }

                    break;
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

    fn fallback_model_for_error(
        error: &anyhow::Error,
        resource_policy: &ResourcePolicy,
        current_model: &str,
        fallback_attempted: bool,
    ) -> Option<(String, ClassifiedError)> {
        if fallback_attempted || !resource_policy.allow_fallback_model {
            return None;
        }
        let fallback_model = Self::configured_fallback_model(current_model)?;
        let classified = crate::engine::error_classifier::ErrorClassifier::from_anyhow(error);
        if Self::category_allows_fallback_model(&classified.category) {
            Some((fallback_model, classified))
        } else {
            None
        }
    }

    fn configured_fallback_model(current_model: &str) -> Option<String> {
        let fallback = std::env::var("PRIORITY_AGENT_FALLBACK_MODEL").ok()?;
        let fallback = fallback.trim();
        if fallback.is_empty() || fallback == current_model {
            return None;
        }
        Some(fallback.to_string())
    }

    fn category_allows_fallback_model(category: &ErrorCategory) -> bool {
        matches!(
            category,
            ErrorCategory::RateLimited
                | ErrorCategory::Overloaded
                | ErrorCategory::ContextOverflow
                | ErrorCategory::PayloadTooLarge
                | ErrorCategory::Timeout
                | ErrorCategory::ConnectionError
                | ErrorCategory::MalformedResponse
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TurnStatus, TurnTrace};
    use crate::test_utils::env_guard::EnvVarGuard;

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
    fn fallback_model_policy_allows_transient_provider_errors() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.set("PRIORITY_AGENT_FALLBACK_MODEL", "fallback-model");
        let route = IntentRouter::new().route("fix the bug");
        let policy = ResourcePolicy::from_route(&route);

        let decision = ApiRequestController::fallback_model_for_error(
            &anyhow::anyhow!("server overloaded"),
            &policy,
            "primary-model",
            false,
        );

        let (model, classified) = decision.expect("fallback should be selected");
        assert_eq!(model, "fallback-model");
        assert_eq!(classified.category, ErrorCategory::Overloaded);
    }

    #[test]
    fn fallback_model_policy_blocks_provider_protocol_errors() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.set("PRIORITY_AGENT_FALLBACK_MODEL", "fallback-model");
        let route = IntentRouter::new().route("fix the bug");
        let policy = ResourcePolicy::from_route(&route);

        let decision = ApiRequestController::fallback_model_for_error(
            &anyhow::anyhow!("tool call result does not follow tool call"),
            &policy,
            "primary-model",
            false,
        );

        assert!(decision.is_none());
    }

    #[test]
    fn fallback_model_policy_skips_same_model_and_repeated_attempt() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.set("PRIORITY_AGENT_FALLBACK_MODEL", "primary-model");
        let route = IntentRouter::new().route("fix the bug");
        let policy = ResourcePolicy::from_route(&route);

        assert!(ApiRequestController::fallback_model_for_error(
            &anyhow::anyhow!("server overloaded"),
            &policy,
            "primary-model",
            false,
        )
        .is_none());

        env.set("PRIORITY_AGENT_FALLBACK_MODEL", "fallback-model");
        assert!(ApiRequestController::fallback_model_for_error(
            &anyhow::anyhow!("server overloaded"),
            &policy,
            "primary-model",
            true,
        )
        .is_none());
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
