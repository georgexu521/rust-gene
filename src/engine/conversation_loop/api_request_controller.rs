use super::request_timeouts::{profile_driven_slow_warning, profile_driven_timeout};
use super::session_processor::SessionStepResult;
use super::tool_execution::{tool_call_is_concurrency_safe, tool_call_is_read_only};
use super::turn_recording::record_recovery_plan;
use super::ConversationLoop;
use crate::engine::context_collapse::{CompactionDecision, ContextCompactionStrategy};
use crate::engine::context_compressor::{estimate_messages_tokens, CompactionAttemptInput};
use crate::engine::error_classifier::{ClassifiedError, ErrorCategory, ProviderFailureKind};
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::provider_protocol::{
    provider_message_normalization_report, ProviderCapabilities, ProviderLatencyProfile,
    ProviderProtocolFamily,
};
use crate::services::api::{ChatRequest, Message, ProviderRetryObserver, Tool, ToolCall};
use crate::tools::{ToolRegistry, ToolResult};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Instant;
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
    pub(super) model: String,
}

pub(super) struct ApiRequestApplicationContext<'a> {
    pub(super) outcome: ApiRequestOutcome,
    pub(super) final_content: &'a mut String,
    pub(super) final_tool_calls: &'a mut Vec<ToolCall>,
    pub(super) tool_calls_made: &'a mut bool,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
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
            let request_tools = request.tools.as_deref().unwrap_or(context.tools);
            let request_has_tools = !request_tools.is_empty();
            let provider_capabilities = ProviderCapabilities::detect(
                context.conversation.provider.base_url(),
                &request.model,
            );
            context.trace.record(TraceEvent::ApiRequestStarted {
                iteration: context.iteration,
                model: request.model.clone(),
                tools: request_tools.len(),
                provider_family: Some(provider_capabilities.protocol_family.label().to_string()),
                nonstreaming_tools_required: provider_capabilities.requires_nonstreaming_tool_calls,
                tool_result_adjacency_required: provider_capabilities
                    .requires_tool_result_adjacency,
            });
            let normalization_report =
                provider_message_normalization_report(provider_capabilities, &request.messages);
            context
                .trace
                .record(TraceEvent::ProviderMessageSequenceNormalized {
                    provider_family: normalization_report.provider_family.label().to_string(),
                    requires_tool_result_adjacency: normalization_report
                        .requires_tool_result_adjacency,
                    requires_merged_system_messages: normalization_report
                        .requires_merged_system_messages,
                    system_messages_merged: normalization_report.system_messages_merged,
                    input_messages: normalization_report.input_messages,
                    output_messages: normalization_report.output_messages,
                    valid_tool_call_pairs: normalization_report.valid_tool_call_pairs,
                    dropped_assistant_tool_calls: normalization_report.dropped_assistant_tool_calls,
                    dropped_tool_results: normalization_report.dropped_tool_results,
                    valid_tool_call_ids: normalization_report.valid_tool_call_ids,
                    dropped_assistant_tool_call_ids: normalization_report
                        .dropped_assistant_tool_call_ids,
                    dropped_tool_result_ids: normalization_report.dropped_tool_result_ids,
                });
            if let Some(report) = crate::services::api::tool_call_repair::schema_flattening_report(
                request_tools,
                provider_capabilities.protocol_family,
                &request.model,
            ) {
                context
                    .trace
                    .record(TraceEvent::ProviderToolCallRepairApplied {
                        provider_family: report.provider_family,
                        schema_flattened_tools: report.schema_flattened_tools,
                        schema_flattened_fields: report.schema_flattened_fields,
                        scavenged_tool_calls: report.scavenged_tool_calls,
                        argument_repairs: report.argument_repairs,
                        unflattened_arguments: report.unflattened_arguments,
                        dropped_duplicate_calls: report.dropped_duplicate_calls,
                        malformed_tool_calls: report.malformed_tool_calls,
                        warnings: report.warnings,
                    });
            }
            let nonstreaming_tool_request = context.tx.is_some()
                && request_has_tools
                && provider_capabilities.requires_nonstreaming_tool_calls;
            let nonstreaming_provider_request = context.tx.is_some()
                && provider_requires_nonstreaming_frontend_request(provider_capabilities);
            let use_nonstreaming_request =
                nonstreaming_tool_request || nonstreaming_provider_request;
            let latency_profile = ProviderLatencyProfile::for_request(
                &provider_capabilities,
                &request.model,
                request_has_tools,
                context.tx.is_some() && !use_nonstreaming_request,
                fallback_attempted,
                request.messages.len(),
                request_tools.len(),
            );
            let profile_timeout = profile_driven_timeout(&latency_profile);
            let slow_warning_threshold = profile_driven_slow_warning(&latency_profile);
            let actual_timeout_ms = profile_timeout.as_millis() as u64;
            let slow_warning_threshold_ms = slow_warning_threshold.as_millis() as u64;
            let provider_family_label = provider_capabilities.protocol_family.label().to_string();
            let request_shape_label = latency_profile.request_shape.label().to_string();
            context.trace.record(TraceEvent::ProviderRequestStarted {
                provider_family: provider_family_label.clone(),
                model: request.model.clone(),
                request_shape: request_shape_label.clone(),
                timeout_secs: profile_timeout.as_secs(),
                slow_warning_threshold_secs: slow_warning_threshold.as_secs(),
                message_count: request.messages.len(),
                tool_count: request_tools.len(),
                is_known_slow_path: latency_profile.is_known_slow_path(),
            });
            if let Some(tx) = context.tx {
                let _ = tx
                    .send(StreamEvent::RuntimeDiagnostic {
                        diagnostic: serde_json::json!({
                            "schema": "api_request_stage.v1",
                            "stage": "api_request_started",
                            "iteration": context.iteration,
                            "model": request.model.clone(),
                            "tools": request_tools.len(),
                            "streaming": !use_nonstreaming_request,
                            "provider_family": provider_family_label.clone(),
                            "request_shape": request_shape_label.clone(),
                            "timeout_ms": actual_timeout_ms,
                            "slow_warning_threshold_ms": slow_warning_threshold_ms,
                            "is_known_slow_path": latency_profile.is_known_slow_path(),
                            "nonstreaming_tool_request": nonstreaming_tool_request,
                            "nonstreaming_provider_request": nonstreaming_provider_request,
                        }),
                    })
                    .await;
            }
            let request_started_at = Instant::now();
            let provider_retry_count = Arc::new(AtomicU32::new(0));
            let retry_observer: ProviderRetryObserver = {
                let trace = context.trace.clone();
                let tx = context.tx.cloned();
                let provider_family = provider_family_label.clone();
                let model = request.model.clone();
                let request_shape = request_shape_label.clone();
                let retry_count = provider_retry_count.clone();
                std::sync::Arc::new(move |notice| {
                    let elapsed_ms = request_started_at.elapsed().as_millis() as u64;
                    let delay_ms = notice.delay.as_millis() as u64;
                    retry_count.fetch_max(notice.attempt as u32, Ordering::Relaxed);
                    trace.record(TraceEvent::ProviderRequestRetrying {
                        provider_family: provider_family.clone(),
                        model: model.clone(),
                        request_shape: request_shape.clone(),
                        attempt: notice.attempt,
                        max_attempts: notice.max_attempts,
                        delay_ms,
                        elapsed_ms,
                        error_preview: notice.error.clone(),
                    });
                    if let Some(tx) = &tx {
                        let _ = tx.try_send(StreamEvent::RuntimeDiagnostic {
                            diagnostic: serde_json::json!({
                                "schema": "provider_request.v1",
                                "stage": "provider_request_retrying",
                                "provider_family": provider_family.clone(),
                                "model": model.clone(),
                                "request_shape": request_shape.clone(),
                                "attempt": notice.attempt,
                                "max_attempts": notice.max_attempts,
                                "delay_ms": delay_ms,
                                "elapsed_ms": elapsed_ms,
                                "error_preview": notice.error.clone(),
                            }),
                        });
                    }
                })
            };
            request.retry_observer = Some(retry_observer);
            let request_for_provider = request.clone();
            let provider_request = async {
                if let Some(tx) = context.tx {
                    if use_nonstreaming_request {
                        let reason = if nonstreaming_provider_request {
                            "provider stream is incompatible with MiniMax SSE chunks; using non-streaming provider request"
                        } else {
                            "provider stream is incompatible with tool/usage chunks; using non-streaming tool request"
                        };
                        context.trace.record(TraceEvent::WorkflowFallback {
                            error: reason.to_string(),
                        });
                        context
                            .conversation
                            .call_api_with_timeout(request_for_provider.clone(), profile_timeout)
                            .await
                    } else {
                        context
                            .conversation
                            .call_api_streaming(
                                request_for_provider.clone(),
                                tx,
                                context.trace,
                                context.exposed_tool_names,
                            )
                            .await
                    }
                } else {
                    context
                        .conversation
                        .call_api_with_timeout(request_for_provider.clone(), profile_timeout)
                        .await
                }
            };
            tokio::pin!(provider_request);
            let slow_warning_sleep = tokio::time::sleep(slow_warning_threshold);
            tokio::pin!(slow_warning_sleep);
            let mut slow_warning_sent = false;
            api_result = loop {
                tokio::select! {
                    result = &mut provider_request => break result,
                    _ = &mut slow_warning_sleep, if !slow_warning_sent
                        && !slow_warning_threshold.is_zero()
                        && slow_warning_threshold < profile_timeout => {
                        slow_warning_sent = true;
                        let elapsed_ms = request_started_at.elapsed().as_millis() as u64;
                        let message = if latency_profile.is_known_slow_path() {
                            format!(
                                "{} tool-call requests use non-streaming mode for provider protocol compatibility.",
                                provider_capabilities.protocol_family.label()
                            )
                        } else {
                            "provider request is taking longer than expected".to_string()
                        };
                        context.trace.record(TraceEvent::ProviderRequestSlowWarning {
                            provider_family: provider_capabilities.protocol_family.label().to_string(),
                            model: request.model.clone(),
                            request_shape: latency_profile.request_shape.label().to_string(),
                            elapsed_ms,
                            timeout_ms: actual_timeout_ms,
                            message: message.clone(),
                        });
                        if let Some(tx) = context.tx {
                            let _ = tx
                                .send(StreamEvent::RuntimeDiagnostic {
                                    diagnostic: serde_json::json!({
                                        "schema": "provider_request.v1",
                                        "stage": "provider_request_slow_warning",
                                        "provider_family": provider_capabilities.protocol_family.label(),
                                        "model": request.model.clone(),
                                        "request_shape": latency_profile.request_shape.label(),
                                        "elapsed_ms": elapsed_ms,
                                        "timeout_ms": actual_timeout_ms,
                                        "slow_warning_threshold_ms": slow_warning_threshold_ms,
                                        "is_known_slow_path": latency_profile.is_known_slow_path(),
                                        "message": message,
                                    }),
                                })
                                .await;
                        }
                    }
                }
            };

            match &api_result {
                Ok(step) => {
                    let elapsed_ms = request_started_at.elapsed().as_millis() as u64;
                    context.trace.record(TraceEvent::ProviderRequestCompleted {
                        provider_family: provider_capabilities.protocol_family.label().to_string(),
                        model: request.model.clone(),
                        request_shape: latency_profile.request_shape.label().to_string(),
                        elapsed_ms,
                        success: true,
                    });
                    if let Some(tx) = context.tx {
                        let _ = tx
                            .send(StreamEvent::RuntimeDiagnostic {
                                diagnostic: serde_json::json!({
                                    "schema": "provider_request.v1",
                                    "stage": "provider_request_completed",
                                    "provider_family": provider_capabilities.protocol_family.label(),
                                    "model": request.model.clone(),
                                    "request_shape": latency_profile.request_shape.label(),
                                    "elapsed_ms": elapsed_ms,
                                    "timeout_ms": actual_timeout_ms,
                                    "success": true,
                                }),
                            })
                            .await;
                    }
                    let retry_count = provider_retry_count.load(Ordering::Relaxed);
                    context
                        .conversation
                        .record_session_step_usage(
                            step,
                            Some(crate::cost_tracker::ApiUsageMetadata {
                                provider: Some(
                                    provider_capabilities.protocol_family.label().to_string(),
                                ),
                                latency_ms: Some(elapsed_ms),
                                finish_reason: step.finish_reason.clone(),
                                retry_count: (retry_count > 0).then_some(retry_count),
                                ..Default::default()
                            }),
                        )
                        .await;
                    if let Some(tx) = context.tx {
                        if use_nonstreaming_request {
                            context
                                .conversation
                                .emit_non_streaming_tool_events(
                                    tx,
                                    &step.assistant_text,
                                    &step.tool_calls,
                                )
                                .await;
                        }
                    }
                    Self::record_streaming_tool_shadow(
                        context.trace,
                        context.conversation.tool_registry.as_ref(),
                        provider_capabilities,
                        !use_nonstreaming_request && context.tx.is_some(),
                        elapsed_ms,
                        step,
                    );
                    break;
                }
                Err(error) => {
                    let elapsed_ms = request_started_at.elapsed().as_millis() as u64;
                    let error_str = error.to_string();
                    let failure_kind = ProviderFailureKind::from_error(&error_str);
                    let is_timeout = matches!(
                        failure_kind,
                        ProviderFailureKind::Timeout | ProviderFailureKind::StreamIdle
                    ) || elapsed_ms >= actual_timeout_ms;
                    if matches!(failure_kind, ProviderFailureKind::Cancelled) {
                        context.trace.record(TraceEvent::ProviderRequestCancelled {
                            provider_family: provider_capabilities
                                .protocol_family
                                .label()
                                .to_string(),
                            model: request.model.clone(),
                            request_shape: latency_profile.request_shape.label().to_string(),
                            elapsed_ms,
                        });
                        if let Some(tx) = context.tx {
                            let _ = tx
                                .send(StreamEvent::RuntimeDiagnostic {
                                    diagnostic: serde_json::json!({
                                        "schema": "provider_request.v1",
                                        "stage": "provider_request_cancelled",
                                        "provider_family": provider_capabilities.protocol_family.label(),
                                        "model": request.model.clone(),
                                        "request_shape": latency_profile.request_shape.label(),
                                        "elapsed_ms": elapsed_ms,
                                    }),
                                })
                                .await;
                        }
                    } else if is_timeout {
                        context.trace.record(TraceEvent::ProviderRequestTimeout {
                            provider_family: provider_capabilities
                                .protocol_family
                                .label()
                                .to_string(),
                            model: request.model.clone(),
                            request_shape: latency_profile.request_shape.label().to_string(),
                            elapsed_ms,
                            timeout_ms: actual_timeout_ms,
                        });
                        if let Some(tx) = context.tx {
                            let _ = tx
                                .send(StreamEvent::RuntimeDiagnostic {
                                    diagnostic: serde_json::json!({
                                        "schema": "provider_request.v1",
                                        "stage": "provider_request_timeout",
                                        "provider_family": provider_capabilities.protocol_family.label(),
                                        "model": request.model.clone(),
                                        "request_shape": latency_profile.request_shape.label(),
                                        "elapsed_ms": elapsed_ms,
                                        "timeout_ms": actual_timeout_ms,
                                    }),
                                })
                                .await;
                        }
                    } else {
                        context.trace.record(TraceEvent::ProviderRequestCompleted {
                            provider_family: provider_capabilities
                                .protocol_family
                                .label()
                                .to_string(),
                            model: request.model.clone(),
                            request_shape: latency_profile.request_shape.label().to_string(),
                            elapsed_ms,
                            success: false,
                        });
                        if let Some(tx) = context.tx {
                            let _ = tx
                                .send(StreamEvent::RuntimeDiagnostic {
                                    diagnostic: serde_json::json!({
                                        "schema": "provider_request.v1",
                                        "stage": "provider_request_completed",
                                        "provider_family": provider_capabilities.protocol_family.label(),
                                        "model": request.model.clone(),
                                        "request_shape": latency_profile.request_shape.label(),
                                        "elapsed_ms": elapsed_ms,
                                        "timeout_ms": actual_timeout_ms,
                                        "success": false,
                                    }),
                                })
                                .await;
                        }
                    }
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
                        if let Some(ref compressor_mutex) = context.conversation.compressor {
                            let mut compressor = compressor_mutex.lock().await;
                            let before_tokens = estimate_messages_tokens(context.messages);
                            if compressor.compaction_circuit_open() {
                                compressor.record_compaction_decision(CompactionAttemptInput::new(
                                    "api_context_error",
                                    ContextCompactionStrategy::ReactiveCompact,
                                    CompactionDecision::CircuitOpen,
                                    before_tokens,
                                    context.messages.len(),
                                    "reactive compaction circuit open after repeated no-gain/failure attempts",
                                ));
                                drop(compressor);
                                break;
                            }
                            compressor.record_compaction_decision(CompactionAttemptInput::new(
                                "api_context_error",
                                ContextCompactionStrategy::ReactiveCompact,
                                CompactionDecision::Retrying,
                                before_tokens,
                                context.messages.len(),
                                "provider reported context limit; compacting and retrying",
                            ));
                            drop(compressor);
                            let messages_for_compression = if compress_retry == 0 {
                                context.messages.to_vec()
                            } else {
                                let mut compressor = compressor_mutex.lock().await;
                                compressor.micro_compress(context.messages)
                            };
                            let mut compressor = compressor_mutex.lock().await;
                            compressor
                                .set_llm_summary_stable_prefix_from_messages(&request.messages);
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
                            let after_tokens = estimate_messages_tokens(&compressed);
                            let decision = if after_tokens
                                < estimate_messages_tokens(&messages_for_compression)
                            {
                                CompactionDecision::Recovered
                            } else {
                                CompactionDecision::NoGain
                            };
                            compressor.record_compaction_decision(
                                CompactionAttemptInput::new(
                                    "api_context_error",
                                    ContextCompactionStrategy::ReactiveCompact,
                                    decision,
                                    estimate_messages_tokens(&messages_for_compression),
                                    messages_for_compression.len(),
                                    if decision == CompactionDecision::Recovered {
                                        "reactive compaction produced a smaller retry request"
                                    } else {
                                        "reactive compaction did not reduce retry request size"
                                    },
                                )
                                .with_after(Some(after_tokens), Some(compressed.len()))
                                .with_boundary_id(
                                    compaction_record
                                        .as_ref()
                                        .and_then(|record| record.boundary_id.clone()),
                                ),
                            );
                            drop(compressor);
                            if let (Some(store), Some(record)) = (
                                context.conversation.session_store.as_ref(),
                                compaction_record.as_ref(),
                            ) {
                                let _ = store.add_compact_boundary_from_runtime_record(
                                    &context.conversation.session_id,
                                    record,
                                    Some("api_context_error"),
                                    "reactive context compacted after provider limit error",
                                );
                                // Write compaction event to session_events for durable replay.
                                let writer = crate::session_store::SessionEventWriter::new(
                                    store.shared_conn(),
                                    &context.conversation.session_id,
                                );
                                if let Err(err) = writer.compaction(
                                    record.strategy.label(),
                                    "api_context_error",
                                    record.tokens_before,
                                    record.tokens_after,
                                ) {
                                    tracing::warn!(
                                        "Failed to write compaction event for session {}: {}",
                                        context.conversation.session_id,
                                        err
                                    );
                                }
                            }
                            let mut provenance = compaction_record
                                .as_ref()
                                .map(|record| record.provenance.clone())
                                .unwrap_or_default();
                            provenance.push("trigger:api_context_error".to_string());
                            context.trace.record(TraceEvent::ContextCompacted {
                                before_tokens: estimate_messages_tokens(&messages_for_compression)
                                    as usize,
                                after_tokens: after_tokens as usize,
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
                                .with_temperature(0.2)
                                .with_output_cap(Some(8192));
                            crate::tools::file_tool::clear_read_files(
                                &context.conversation.session_id,
                            );
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
            model: request.model,
        })
    }

    pub(super) fn apply_outcome(
        context: ApiRequestApplicationContext<'_>,
    ) -> ApiRequestApplication {
        let session_step = context.outcome.session_step;
        if let Some(report) = &session_step.tool_call_repair {
            emit_tool_call_repair_diagnostic(context.tx, report);
            context
                .trace
                .record(TraceEvent::ProviderToolCallRepairApplied {
                    provider_family: report.provider_family.clone(),
                    schema_flattened_tools: report.schema_flattened_tools,
                    schema_flattened_fields: report.schema_flattened_fields,
                    scavenged_tool_calls: report.scavenged_tool_calls,
                    argument_repairs: report.argument_repairs,
                    unflattened_arguments: report.unflattened_arguments,
                    dropped_duplicate_calls: report.dropped_duplicate_calls,
                    malformed_tool_calls: report.malformed_tool_calls,
                    warnings: report.warnings.clone(),
                });
        }
        if let Some(usage) = &session_step.usage {
            let cache_usage = crate::engine::cache_stability::prompt_cache_usage(
                usage.prompt_tokens as u64,
                usage.cached_tokens.map(u64::from),
            );
            context.trace.record(TraceEvent::PromptCacheUsageRecorded {
                model: context.outcome.model.clone(),
                prompt_tokens: cache_usage.prompt_tokens,
                cached_tokens: cache_usage.cached_tokens,
                cache_miss_tokens: cache_usage.cache_miss_tokens,
                hit_rate: cache_usage.hit_ratio,
            });
        }
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

        *context.final_tool_calls = tool_calls.clone();
        if tool_calls.is_empty() {
            *context.final_content = content.clone();
        } else {
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
        let fallback = crate::services::config::runtime_config().fallback_model()?;
        if fallback.trim().is_empty() || fallback.trim() == current_model {
            return None;
        }
        Some(fallback.trim().to_string())
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

    fn record_streaming_tool_shadow(
        trace: &TraceCollector,
        tool_registry: &ToolRegistry,
        capabilities: ProviderCapabilities,
        streamed_request_path: bool,
        latency_upper_bound_ms: u64,
        step: &SessionStepResult,
    ) {
        let Some(mode) = streaming_tool_execution_shadow_mode() else {
            return;
        };
        let observed_tool_calls = step.tool_calls.len();
        if observed_tool_calls == 0 {
            return;
        }

        let mut read_only_tool_calls = 0usize;
        let mut concurrency_safe_tool_calls = 0usize;
        let mut eligible_tool_calls = 0usize;
        for tool_call in &step.tool_calls {
            let read_only =
                tool_call_is_read_only(tool_registry, &tool_call.name, &tool_call.arguments);
            let concurrency_safe =
                tool_call_is_concurrency_safe(tool_registry, &tool_call.name, &tool_call.arguments);
            if read_only {
                read_only_tool_calls += 1;
            }
            if concurrency_safe {
                concurrency_safe_tool_calls += 1;
            }
            if capabilities.supports_streaming_tool_calls
                && streamed_request_path
                && read_only
                && concurrency_safe
            {
                eligible_tool_calls += 1;
            }
        }

        let reason = if !capabilities.supports_streaming_tool_calls {
            "provider_does_not_support_streaming_tool_calls"
        } else if !streamed_request_path {
            "request_used_nonstreaming_tool_path"
        } else if eligible_tool_calls == 0 {
            "no_read_only_concurrency_safe_tool_calls"
        } else {
            "shadow_only_no_early_execution"
        };

        trace.record(TraceEvent::StreamingToolExecutionShadow {
            mode,
            provider_family: capabilities.protocol_family.label().to_string(),
            provider_supports_streaming_tool_calls: capabilities.supports_streaming_tool_calls,
            streamed_request_path,
            observed_tool_calls,
            read_only_tool_calls,
            concurrency_safe_tool_calls,
            eligible_tool_calls,
            schema_complete_tool_calls: observed_tool_calls,
            latency_upper_bound_ms,
            reason: reason.to_string(),
        });
    }
}

fn streaming_tool_execution_shadow_mode() -> Option<String> {
    crate::services::config::runtime_config().streaming_tool_execution_shadow()
}

fn provider_requires_nonstreaming_frontend_request(capabilities: ProviderCapabilities) -> bool {
    matches!(
        capabilities.protocol_family,
        ProviderProtocolFamily::MiniMax
    )
}

fn emit_tool_call_repair_diagnostic(
    tx: Option<&mpsc::Sender<StreamEvent>>,
    report: &crate::services::api::tool_call_repair::ToolCallRepairReport,
) {
    let Some(tx) = tx else {
        return;
    };
    let _ = tx.try_send(StreamEvent::RuntimeDiagnostic {
        diagnostic: serde_json::json!({
            "schema": "provider_tool_call_repair.v1",
            "provider_family": report.provider_family,
            "schema_flattened_tools": report.schema_flattened_tools,
            "schema_flattened_fields": report.schema_flattened_fields,
            "scavenged_tool_calls": report.scavenged_tool_calls,
            "argument_repairs": report.argument_repairs,
            "unflattened_arguments": report.unflattened_arguments,
            "dropped_duplicate_calls": report.dropped_duplicate_calls,
            "malformed_tool_calls": report.malformed_tool_calls,
            "warnings": report.warnings,
        }),
    });
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
    fn streaming_tool_execution_shadow_mode_is_gated_by_env() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_STREAMING_TOOL_EXECUTION");
        assert_eq!(streaming_tool_execution_shadow_mode(), None);

        env.set("PRIORITY_AGENT_STREAMING_TOOL_EXECUTION", "shadow");
        assert_eq!(
            streaming_tool_execution_shadow_mode().as_deref(),
            Some("shadow")
        );

        env.set("PRIORITY_AGENT_STREAMING_TOOL_EXECUTION", "on");
        assert_eq!(streaming_tool_execution_shadow_mode(), None);
    }

    #[test]
    fn minimax_frontend_requests_use_nonstreaming_provider_path() {
        assert!(provider_requires_nonstreaming_frontend_request(
            ProviderCapabilities::for_family(ProviderProtocolFamily::MiniMax)
        ));
        assert!(!provider_requires_nonstreaming_frontend_request(
            ProviderCapabilities::for_family(ProviderProtocolFamily::OpenAiCompatible)
        ));
    }

    #[test]
    fn deepseek_frontend_tool_requests_use_nonstreaming_tool_path() {
        let capabilities =
            ProviderCapabilities::detect("https://api.deepseek.com", "deepseek-v4-flash");

        assert!(capabilities.requires_nonstreaming_tool_calls);
        assert!(!capabilities.supports_streaming_tool_calls);
        assert!(!provider_requires_nonstreaming_frontend_request(
            capabilities
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
                tool_call_repair: None,
                finish_reason: None,
                source: super::super::session_processor::SessionStepSource::NonStreaming,
                cache_shape: None,
            },
            compressed_this_turn: true,
            model: "mock-model".to_string(),
        };
        let mut final_content = String::new();
        let mut final_tool_calls = Vec::new();
        let mut tool_calls_made = false;

        let application = ApiRequestController::apply_outcome(ApiRequestApplicationContext {
            outcome,
            final_content: &mut final_content,
            final_tool_calls: &mut final_tool_calls,
            tool_calls_made: &mut tool_calls_made,
            tx: None,
            trace: &trace,
            iteration: 2,
        });

        assert_eq!(application.content, "running check");
        assert_eq!(application.tool_calls.len(), 1);
        assert_eq!(application.tool_calls[0].id, tool_call.id);
        assert_eq!(application.tool_calls[0].name, tool_call.name);
        assert!(application.pre_executed.is_empty());
        assert!(
            final_content.is_empty(),
            "tool-call responses must not be treated as final answers"
        );
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

    #[test]
    fn apply_outcome_emits_tool_call_repair_diagnostic() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "api request repair"));
        let (tx, mut rx) = mpsc::channel(4);
        let mut repair = crate::services::api::tool_call_repair::ToolCallRepairReport::new(
            ProviderProtocolFamily::OpenAiCompatible,
        );
        repair.malformed_tool_calls = 1;
        repair.argument_repairs = 1;
        let outcome = ApiRequestOutcome {
            session_step: SessionStepResult {
                assistant_text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "call-repaired".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({ "command": "echo repaired" }),
                }],
                pre_executed_results: HashMap::new(),
                usage: None,
                tool_call_repair: Some(repair),
                finish_reason: None,
                source: super::super::session_processor::SessionStepSource::NonStreaming,
                cache_shape: None,
            },
            compressed_this_turn: false,
            model: "mock-model".to_string(),
        };
        let mut final_content = String::new();
        let mut final_tool_calls = Vec::new();
        let mut tool_calls_made = false;

        let _ = ApiRequestController::apply_outcome(ApiRequestApplicationContext {
            outcome,
            final_content: &mut final_content,
            final_tool_calls: &mut final_tool_calls,
            tool_calls_made: &mut tool_calls_made,
            tx: Some(&tx),
            trace: &trace,
            iteration: 1,
        });

        let event = rx.try_recv().expect("repair diagnostic should be emitted");
        let StreamEvent::RuntimeDiagnostic { diagnostic } = event else {
            panic!("expected runtime diagnostic");
        };
        assert_eq!(diagnostic["schema"], "provider_tool_call_repair.v1");
        assert_eq!(diagnostic["malformed_tool_calls"], 1);
        assert_eq!(diagnostic["argument_repairs"], 1);
    }
}
