use super::runtime_timeouts::{llm_request_timeout, stream_chunk_idle_timeout};
use super::text_sanitizer::{strip_hidden_blocks, VisibleTextSanitizer};
use super::tool_execution::read_only_tool_concurrency;
use super::turn_recording::{
    persist_turn_learning_event, promote_trace_candidate_memories, record_hook_traces,
    record_recovery_plan,
};
use super::ConversationLoop;
use crate::engine::hooks::HookDecision;
use crate::engine::streaming::{emit_text_progressively, StreamEvent};
use crate::engine::trace::{TraceCollector, TurnStatus};
use crate::services::api::provider_protocol::ProviderCapabilities;
use crate::services::api::tool_call_repair::ToolCallRepairReport;
use crate::services::api::{ChatRequest, ChatResponse, ToolCall, Usage};
use crate::tools::ToolResult;
use anyhow::Result;
use futures::StreamExt;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

pub(super) struct SessionStepResult {
    pub(super) assistant_text: String,
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) pre_executed_results: std::collections::HashMap<usize, ToolResult>,
    pub(super) usage: Option<Usage>,
    pub(super) tool_call_repair: Option<ToolCallRepairReport>,
    pub(super) finish_reason: Option<String>,
    pub(super) source: SessionStepSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SessionStepSource {
    NonStreaming,
    Streaming,
    StreamingFallback { reason: String },
}

impl SessionStepResult {
    fn new(
        assistant_text: String,
        tool_calls: Vec<ToolCall>,
        pre_executed_results: std::collections::HashMap<usize, ToolResult>,
        usage: Option<Usage>,
        tool_call_repair: Option<ToolCallRepairReport>,
        finish_reason: Option<String>,
        source: SessionStepSource,
    ) -> Self {
        Self {
            assistant_text,
            tool_calls,
            pre_executed_results,
            usage,
            tool_call_repair,
            finish_reason,
            source,
        }
    }
}

async fn emit_usage_event(response: &ChatResponse, tx: &mpsc::Sender<StreamEvent>) {
    if let Some(usage) = &response.usage {
        let _ = tx
            .send(StreamEvent::Usage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                reasoning_tokens: usage.reasoning_tokens,
                cached_tokens: usage.cached_tokens,
            })
            .await;
    }
}

fn cache_shape_for_request(
    request: &ChatRequest,
) -> crate::engine::cache_stability::CacheDiagnosticShape {
    let mut shape = cache_shape_for_messages_and_tools(
        &request.messages,
        request.tools.as_deref().unwrap_or(&[]),
    );
    shape.effective_output_cap = request.max_tokens;
    shape.request_phase = Some(request_phase_for_output_cap(request.max_tokens).to_string());
    shape.tool_round_count = Some(
        request
            .messages
            .iter()
            .filter(|message| matches!(message, crate::services::api::Message::Tool { .. }))
            .count() as u64,
    );
    shape
}

fn cache_shape_for_messages_and_tools(
    messages: &[crate::services::api::Message],
    tools: &[crate::services::api::Tool],
) -> crate::engine::cache_stability::CacheDiagnosticShape {
    crate::engine::cache_stability::request_cache_diagnostic_shape(messages, tools)
}

fn request_phase_for_output_cap(cap: Option<u32>) -> &'static str {
    match cap {
        Some(1024) => "repair",
        Some(2048) => "compact",
        Some(8192) => "coding",
        Some(_) => "capped",
        None => "uncapped",
    }
}

impl ConversationLoop {
    #[cfg(test)]
    pub(super) async fn call_api(&self, request: ChatRequest) -> Result<SessionStepResult> {
        self.call_api_with_timeout(request, llm_request_timeout())
            .await
    }

    pub(super) async fn call_api_with_timeout(
        &self,
        request: ChatRequest,
        timeout: std::time::Duration,
    ) -> Result<SessionStepResult> {
        let response = self
            .provider_chat_with_tracking(request, "non-streaming chat", timeout)
            .await?;

        let content = strip_hidden_blocks(&response.content);
        let tool_calls = response.tool_calls.unwrap_or_default();
        let usage = response.usage.clone();
        let tool_call_repair = response.tool_call_repair.clone();

        Ok(SessionStepResult::new(
            content,
            tool_calls,
            std::collections::HashMap::new(),
            usage,
            tool_call_repair,
            None,
            SessionStepSource::NonStreaming,
        ))
    }

    async fn provider_chat_with_tracking(
        &self,
        request: ChatRequest,
        purpose: &str,
        timeout: std::time::Duration,
    ) -> Result<ChatResponse> {
        let cache_shape = cache_shape_for_request(&request);
        let response = self
            .provider_chat_with_timeout(request, purpose, timeout)
            .await?;
        self.record_cost(&response, Some(cache_shape)).await;
        Ok(response)
    }

    async fn provider_chat_with_timeout(
        &self,
        request: ChatRequest,
        purpose: &str,
        timeout: std::time::Duration,
    ) -> Result<ChatResponse> {
        match tokio::time::timeout(timeout, self.provider.chat(request)).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!(
                "{} timed out after {}s",
                purpose,
                timeout.as_secs()
            )),
        }
    }

    /// Fallback to non-streaming request when streaming fails.
    /// Tries with tools first, then without tools if that fails.
    async fn fallback_to_nonstreaming(
        &self,
        fallback_messages: &[crate::services::api::Message],
        fallback_tools: &Option<Vec<crate::services::api::Tool>>,
        reason: &str,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<SessionStepResult> {
        let base_request = ChatRequest::new(&self.model)
            .with_messages(fallback_messages.to_vec())
            .with_temperature(0.2)
            .with_output_cap(Some(8192));
        let default_timeout = llm_request_timeout();
        let response = if let Some(tools) = fallback_tools.clone() {
            match self
                .provider_chat_with_tracking(
                    base_request.clone().with_tools(tools),
                    "non-streaming fallback with tools",
                    default_timeout,
                )
                .await
            {
                Ok(r) => r,
                Err(with_tools_err) => {
                    warn!(
                        "Non-streaming fallback with tools failed: {}. Retrying without tools.",
                        with_tools_err
                    );
                    self.provider_chat_with_tracking(
                        base_request,
                        "non-streaming fallback without tools",
                        default_timeout,
                    )
                    .await?
                }
            }
        } else {
            self.provider_chat_with_tracking(
                base_request,
                "non-streaming fallback",
                default_timeout,
            )
            .await?
        };
        emit_usage_event(&response, tx).await;

        let content = strip_hidden_blocks(&response.content);
        if !content.is_empty() {
            emit_text_progressively(tx, content.clone()).await;
        }
        let tool_calls = response.tool_calls.unwrap_or_default();
        Ok(SessionStepResult::new(
            content,
            tool_calls,
            std::collections::HashMap::new(),
            response.usage.clone(),
            response.tool_call_repair.clone(),
            None,
            SessionStepSource::StreamingFallback {
                reason: reason.to_string(),
            },
        ))
    }

    /// 流式 API 调用
    pub(super) async fn call_api_streaming(
        &self,
        request: ChatRequest,
        tx: &mpsc::Sender<StreamEvent>,
        trace: &TraceCollector,
        exposed_tool_names: &HashSet<String>,
    ) -> Result<SessionStepResult> {
        let fallback_messages = request.messages.clone();
        let fallback_tools = request.tools.clone();
        let provider_family =
            ProviderCapabilities::detect(self.provider.base_url(), &request.model).protocol_family;
        let streaming_cache_shape = cache_shape_for_request(&request);

        let stream_open =
            tokio::time::timeout(llm_request_timeout(), self.provider.chat_stream(request)).await;
        match stream_open {
            Ok(Ok(mut stream)) => {
                let mut raw_content = String::new();
                let mut collected_tool_calls: Vec<ToolCall> = Vec::new();
                let mut raw_args_accum: Vec<String> = Vec::new();
                let mut stream_failed: Option<String> = None;
                let mut stream_usage: Option<Usage> = None;
                let mut finish_reason: Option<String> = None;
                let mut visible_sanitizer = VisibleTextSanitizer::default();

                let _ = tx.send(StreamEvent::ThinkingStart).await;

                let mut read_only_tasks: std::collections::HashMap<
                    usize,
                    tokio::task::JoinHandle<ToolResult>,
                > = std::collections::HashMap::new();
                let read_only_concurrency = read_only_tool_concurrency();
                let tool_registry = self.tool_registry.clone();
                let tool_context = self.create_tool_context_with_trace(trace);
                let cost_tracker = self.cost_tracker.clone();
                let hook_manager = self.hook_manager.clone();

                let stream_idle_timeout = stream_chunk_idle_timeout();
                loop {
                    let Some(result) =
                        (match tokio::time::timeout(stream_idle_timeout, stream.next()).await {
                            Ok(next) => next,
                            Err(_) => {
                                stream_failed = Some(format!(
                                    "stream idle timeout after {}s",
                                    stream_idle_timeout.as_secs()
                                ));
                                break;
                            }
                        })
                    else {
                        break;
                    };
                    match result {
                        Ok(chunk) => {
                            if let Some(usage) = &chunk.usage {
                                stream_usage = Some(Usage {
                                    prompt_tokens: usage.prompt_tokens,
                                    completion_tokens: usage.completion_tokens,
                                    total_tokens: usage.total_tokens,
                                    reasoning_tokens: usage
                                        .completion_tokens_details
                                        .as_ref()
                                        .and_then(|d| d.reasoning_tokens),
                                    cached_tokens: usage
                                        .prompt_tokens_details
                                        .as_ref()
                                        .and_then(|d| d.cached_tokens),
                                });
                                let _ = tx
                                    .send(StreamEvent::Usage {
                                        prompt_tokens: usage.prompt_tokens,
                                        completion_tokens: usage.completion_tokens,
                                        reasoning_tokens: usage
                                            .completion_tokens_details
                                            .as_ref()
                                            .and_then(|d| d.reasoning_tokens),
                                        cached_tokens: usage
                                            .prompt_tokens_details
                                            .as_ref()
                                            .and_then(|d| d.cached_tokens),
                                    })
                                    .await;
                            }
                            if let Some(choice) = chunk.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    if !content.is_empty() {
                                        raw_content.push_str(content);
                                        let visible_chunk = visible_sanitizer.push_chunk(content);
                                        if !visible_chunk.is_empty() {
                                            let _ = tx
                                                .send(StreamEvent::TextChunk(visible_chunk))
                                                .await;
                                        }
                                    }
                                }

                                if let Some(tool_calls) = &choice.delta.tool_calls {
                                    for tc_delta in tool_calls {
                                        let idx = tc_delta.index as usize;
                                        while collected_tool_calls.len() <= idx {
                                            collected_tool_calls.push(ToolCall {
                                                id: String::new(),
                                                name: String::new(),
                                                arguments: serde_json::Value::Null,
                                            });
                                            raw_args_accum.push(String::new());
                                        }

                                        let mut tool_name_for_spawn: Option<String> = None;
                                        let mut tool_id_for_spawn: Option<String> = None;
                                        let mut args_for_spawn: Option<String> = None;

                                        let tc = &mut collected_tool_calls[idx];
                                        if let Some(id) = &tc_delta.id {
                                            tc.id = id.clone();
                                            let _ = tx
                                                .send(StreamEvent::ToolCallStart {
                                                    id: id.clone(),
                                                    name: tc.name.clone(),
                                                })
                                                .await;
                                        }
                                        if let Some(function) = &tc_delta.function {
                                            if let Some(name) = &function.name {
                                                tc.name = name.clone();
                                            }
                                            if let Some(args) = &function.arguments {
                                                raw_args_accum[idx].push_str(args);

                                                tool_name_for_spawn = Some(tc.name.clone());
                                                tool_id_for_spawn = Some(tc.id.clone());
                                                args_for_spawn = Some(raw_args_accum[idx].clone());

                                                let _ = tx
                                                    .send(StreamEvent::ToolCallArgs {
                                                        id: tc.id.clone(),
                                                        args_delta: args.clone(),
                                                    })
                                                    .await;
                                            }
                                        }

                                        if let (Some(tool_name), Some(tid), Some(current_args)) =
                                            (tool_name_for_spawn, tool_id_for_spawn, args_for_spawn)
                                        {
                                            if !tool_name.is_empty()
                                                && exposed_tool_names.contains(&tool_name)
                                                && !read_only_tasks.contains_key(&idx)
                                                && read_only_tasks.len() < read_only_concurrency
                                            {
                                                let Some(tool) = tool_registry.get(&tool_name)
                                                else {
                                                    continue;
                                                };
                                                let Ok(parsed_args) =
                                                    serde_json::from_str::<serde_json::Value>(
                                                        &current_args,
                                                    )
                                                else {
                                                    continue;
                                                };
                                                if tool.validate_params(&parsed_args).is_some() {
                                                    continue;
                                                }
                                                if !tool.is_concurrency_safe(&parsed_args) {
                                                    continue;
                                                }

                                                let registry = tool_registry.clone();
                                                let context = tool_context.clone();
                                                let ct = cost_tracker.clone();
                                                let hooks = hook_manager.clone();
                                                let tid2 = tid.clone();
                                                let tool_n = tool_name.clone();
                                                let tool_n2 = tool_name.clone();
                                                let trace_for_task = Some(trace.clone());

                                                read_only_tasks.insert(
                                                    idx,
                                                    tokio::spawn(async move {
                                                        let started_at =
                                                            std::time::Instant::now();
                                                        let pre_decision = if let Some(ref h)
                                                            = hooks
                                                        {
                                                            let t = ToolCall {
                                                                id: tid.clone(),
                                                                name: tool_n.clone(),
                                                                arguments: parsed_args.clone(),
                                                            };
                                                            let hook_start =
                                                                h.current_record_sequence();
                                                            let decision =
                                                                h.run_pre_tool(&t, &context).await;
                                                            let hook_records = h
                                                                .recent_records_after_for(
                                                                    hook_start,
                                                                    &t.id,
                                                                );
                                                            record_hook_traces(
                                                                &trace_for_task,
                                                                &hook_records,
                                                            );
                                                            decision
                                                        } else {
                                                            HookDecision {
                                                                allow: true,
                                                                reason: None,
                                                            }
                                                        };

                                                        let ctx_clone = context.clone();
                                                        let mut result = if !pre_decision.allow {
                                                            ToolResult::error(
                                                                pre_decision.reason.unwrap_or_else(
                                                                    || format!(
                                                                        "blocked by pre-tool hook: {}",
                                                                        tool_n
                                                                    ),
                                                                ),
                                                            )
                                                        } else if let Some(tool) =
                                                            registry.get(&tool_n)
                                                        {
                                                            tool.execute(parsed_args.clone(), context)
                                                                .await
                                                        } else {
                                                            ToolResult::error(format!(
                                                                "Tool '{}' not found",
                                                                tool_n
                                                            ))
                                                        };

                                                        let duration_ms =
                                                            started_at.elapsed().as_millis()
                                                                as u64;
                                                        if result.duration_ms.is_none() {
                                                            result.duration_ms =
                                                                Some(duration_ms);
                                                        }
                                                        if let Some(ref h) = hooks {
                                                            let tc_for_hook = ToolCall {
                                                                id: tid2.clone(),
                                                                name: tool_n2.clone(),
                                                                arguments: parsed_args.clone(),
                                                            };
                                                            let hook_start =
                                                                h.current_record_sequence();
                                                            h.run_post_tool(
                                                                &tc_for_hook,
                                                                &result,
                                                                &ctx_clone,
                                                            )
                                                                .await;
                                                            let hook_records = h
                                                                .recent_records_after_for(
                                                                    hook_start,
                                                                    &tc_for_hook.id,
                                                                );
                                                            record_hook_traces(
                                                                &trace_for_task,
                                                                &hook_records,
                                                            );
                                                        }
                                                        {
                                                            let mut tracker = ct.lock().await;
                                                            tracker.record_tool_execution(
                                                                &tool_n,
                                                                result.success,
                                                                duration_ms,
                                                                result.error.as_deref(),
                                                            );
                                                        }
                                                        result
                                                    }),
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            let truncated = chunk.choices.iter().any(|c| {
                                c.finish_reason
                                    .as_ref()
                                    .is_some_and(|fr| format!("{:?}", fr).contains("Length"))
                            });
                            if truncated {
                                let _ = tx.send(StreamEvent::OutputTruncated).await;
                            }
                            if chunk.choices.iter().any(|c| c.finish_reason.is_some()) {
                                finish_reason = chunk
                                    .choices
                                    .iter()
                                    .find_map(|choice| {
                                        choice
                                            .finish_reason
                                            .as_ref()
                                            .map(|reason| format!("{:?}", reason))
                                    })
                                    .or(finish_reason);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Stream error: {}", e);
                            stream_failed = Some(e.to_string());
                            break;
                        }
                    }
                }

                let _ = tx.send(StreamEvent::ThinkingComplete).await;
                let visible_tail = visible_sanitizer.finish();
                if !visible_tail.is_empty() {
                    let _ = tx.send(StreamEvent::TextChunk(visible_tail)).await;
                }

                let mut repair_report = ToolCallRepairReport::new(provider_family);
                for (i, tc) in collected_tool_calls.iter_mut().enumerate() {
                    if i < raw_args_accum.len() && !raw_args_accum[i].is_empty() {
                        tc.arguments = crate::services::api::tool_call_repair::parse_tool_arguments(
                            &raw_args_accum[i],
                            &mut repair_report,
                        );
                        let _ = tx
                            .send(StreamEvent::ToolCallComplete { id: tc.id.clone() })
                            .await;
                    }
                }
                let collected_tool_call_count = collected_tool_calls.len();
                let repaired = crate::services::api::tool_call_repair::repair_response(
                    &raw_content,
                    Some(collected_tool_calls),
                    provider_family,
                    repair_report,
                );
                if let Some(report) = &repaired.report {
                    warn!("Streaming tool-call repair applied: {:?}", report);
                }
                let repaired_tool_calls = repaired.tool_calls.unwrap_or_default();
                let repaired_content = repaired.content;
                let repaired_report = repaired.report;

                let mut pre_executed: std::collections::HashMap<usize, ToolResult> =
                    std::collections::HashMap::new();
                for (idx, handle) in read_only_tasks {
                    if let Ok(result) = handle.await {
                        debug!(
                            "Read-only tool at index {} pre-executed with result: {}",
                            idx,
                            if result.success { "OK" } else { "ERROR" }
                        );
                        pre_executed.insert(idx, result);
                    }
                }

                // If streaming fails mid-response, fall back to a non-streaming request for the
                // same turn. Some OpenAI-compatible providers emit non-standard streaming usage
                // payloads after partial tool-call deltas; treating that as terminal would stop a
                // valid coding task before any final tool execution happens.
                if let Some(stream_err) = stream_failed {
                    let plan = crate::engine::recovery_plan::RecoveryPlan::streaming_fallback(
                        "stream_interrupted",
                        &stream_err,
                    );
                    record_recovery_plan(trace, &plan);
                    warn!("{}", plan.user_note);
                    warn!(
                        "Streaming interrupted after {} visible chars and {} partial tool call(s) (error: {}). Falling back to non-streaming",
                        raw_content.chars().count(),
                        collected_tool_call_count,
                        stream_err
                    );
                    return self
                        .fallback_to_nonstreaming(
                            &fallback_messages,
                            &fallback_tools,
                            "stream_interrupted",
                            tx,
                        )
                        .await;
                }

                if let Some(usage) = &stream_usage {
                    self.record_usage(usage, Some(streaming_cache_shape)).await;
                }

                Ok(SessionStepResult::new(
                    repaired_content,
                    repaired_tool_calls,
                    pre_executed,
                    stream_usage,
                    repaired_report,
                    finish_reason,
                    SessionStepSource::Streaming,
                ))
            }
            Ok(Err(e)) => {
                let plan = crate::engine::recovery_plan::RecoveryPlan::streaming_fallback(
                    "stream_open",
                    &e.to_string(),
                );
                record_recovery_plan(trace, &plan);
                warn!("{}", plan.user_note);
                warn!("Streaming failed, falling back to non-streaming: {}", e);
                self.fallback_to_nonstreaming(
                    &fallback_messages,
                    &fallback_tools,
                    "stream_open",
                    tx,
                )
                .await
            }
            Err(_) => {
                let timeout_msg = format!(
                    "stream open timed out after {}s",
                    llm_request_timeout().as_secs()
                );
                let plan = crate::engine::recovery_plan::RecoveryPlan::streaming_fallback(
                    "stream_open_timeout",
                    &timeout_msg,
                );
                record_recovery_plan(trace, &plan);
                warn!("{}", plan.user_note);
                warn!("Streaming open timed out, falling back to non-streaming");
                self.fallback_to_nonstreaming(
                    &fallback_messages,
                    &fallback_tools,
                    "stream_open_timeout",
                    tx,
                )
                .await
            }
        }
    }

    /// 记录 API 调用成本
    async fn record_cost(
        &self,
        response: &ChatResponse,
        cache_shape: Option<crate::engine::cache_stability::CacheDiagnosticShape>,
    ) {
        if let Some(ref usage) = response.usage {
            self.record_usage(usage, cache_shape).await;
        }
    }

    async fn record_usage(
        &self,
        usage: &Usage,
        cache_shape: Option<crate::engine::cache_stability::CacheDiagnosticShape>,
    ) {
        let mut tracker = self.cost_tracker.lock().await;
        tracker.record_api_call_with_session_and_cache_shape(
            Some(&self.session_id),
            &self.model,
            usage.prompt_tokens as u64,
            usage.completion_tokens as u64,
            usage.cached_tokens.map(|t| t as u64),
            cache_shape,
        );
    }

    pub(super) async fn finish_trace(&self, trace: TraceCollector, status: TurnStatus) {
        let trace = trace.finish(status);
        if let Some(store) = &self.trace_store {
            store.push(trace.clone());
        }
        if let Some(store) = &self.session_store {
            if let Err(e) = store.add_turn_trace(&trace) {
                warn!("Failed to persist turn trace: {}", e);
            }
            if let Err(e) = persist_turn_learning_event(store, &trace) {
                warn!("Failed to persist learning event: {}", e);
            }
        }
        if let Some(memory_manager) = &self.memory_manager {
            let memory = memory_manager.lock().await;
            let promoted = promote_trace_candidate_memories(&memory, &trace).await;
            if promoted > 0 {
                debug!(
                    "Promoted {} trace candidate memories from turn outcome",
                    promoted
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::Message;

    #[test]
    fn cache_shape_for_request_captures_output_cap_and_tool_rounds() {
        let request = ChatRequest::new("test-model")
            .with_messages(vec![
                Message::system("system"),
                Message::user("run test"),
                Message::tool("tool-1", "ok"),
            ])
            .with_output_cap(Some(8192));

        let shape = cache_shape_for_request(&request);

        assert_eq!(shape.effective_output_cap, Some(8192));
        assert_eq!(shape.request_phase.as_deref(), Some("coding"));
        assert_eq!(shape.tool_round_count, Some(1));
    }
}
