use super::context_budget_controller::ContextBudgetController;
use super::runtime_diet::RuntimeDietSnapshot;
use crate::engine::intent_router::RetrievalPolicy;
use crate::engine::retrieval_context::{RetrievalContext, RetrievalSource};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::memory::MemoryManager;
use crate::services::api::{ChatRequest, LlmProvider, Message, Tool};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

pub(super) struct RequestPreparationContext<'a> {
    pub(super) messages: &'a [Message],
    pub(super) focused_repair_prompt: Option<Message>,
    pub(super) turn_retrieval_context: Option<&'a RetrievalContext>,
    pub(super) retrieval_policy: RetrievalPolicy,
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) provider: Option<&'a dyn LlmProvider>,
    pub(super) model: &'a str,
    pub(super) tools: &'a [Tool],
    pub(super) trace: &'a TraceCollector,
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
}

pub(super) struct PreparedRequest {
    pub(super) request: ChatRequest,
}

struct MemoryPrefetchContext<'a> {
    turn_retrieval_context: Option<&'a RetrievalContext>,
    retrieval_policy: RetrievalPolicy,
    memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    provider: Option<&'a dyn LlmProvider>,
    model: &'a str,
    trace: &'a TraceCollector,
    runtime_diet: &'a mut RuntimeDietSnapshot,
}

pub(super) struct RequestPreparationController;

impl RequestPreparationController {
    pub(super) async fn prepare(context: RequestPreparationContext<'_>) -> PreparedRequest {
        let RequestPreparationContext {
            messages,
            focused_repair_prompt,
            turn_retrieval_context,
            retrieval_policy,
            memory_manager,
            provider,
            model,
            tools,
            trace,
            runtime_diet,
        } = context;

        let mut request_messages = messages.to_vec();
        if let Some(prompt) = focused_repair_prompt {
            request_messages.push(prompt);
        }

        let mut memory_context = MemoryPrefetchContext {
            turn_retrieval_context,
            retrieval_policy,
            memory_manager,
            provider,
            model,
            trace,
            runtime_diet,
        };
        Self::inject_memory_prefetch(&mut request_messages, &mut memory_context).await;

        let request_budget = ContextBudgetController::observe_request(&request_messages, tools);
        ContextBudgetController::record_runtime_diet(memory_context.runtime_diet, &request_budget);

        PreparedRequest {
            request: ChatRequest::new(model)
                .with_messages(request_messages)
                .with_tools(tools.to_vec())
                .with_temperature(0.2),
        }
    }

    async fn inject_memory_prefetch(
        request_messages: &mut [Message],
        context: &mut MemoryPrefetchContext<'_>,
    ) {
        if !context.retrieval_policy.allows_memory_context() {
            return;
        }
        let Some(memory_manager) = context.memory_manager else {
            return;
        };
        let Some(provider) = context.provider else {
            return;
        };
        if context
            .turn_retrieval_context
            .map(|ctx| ctx.item_count_by_source(RetrievalSource::Memory) > 0)
            .unwrap_or(false)
        {
            return;
        }

        let Some(last_user_idx) = request_messages
            .iter()
            .rposition(|message| matches!(message, Message::User { .. }))
        else {
            return;
        };
        let Message::User { content } = &request_messages[last_user_idx] else {
            return;
        };
        let content = content.clone();

        let mut memory = memory_manager.lock().await;
        let retrieval_context = memory
            .prefetch_retrieval_context_with_llm_rerank(
                &content,
                provider,
                context.model,
                context.retrieval_policy,
            )
            .await;
        let Some(ctx) = retrieval_context else {
            return;
        };

        context.runtime_diet.observe_retrieval_context(&ctx);
        context.trace.record(TraceEvent::MemoryPrefetch {
            chars: ctx
                .items
                .iter()
                .map(|item| item.content_preview.chars().count())
                .sum(),
        });
        context.trace.record(TraceEvent::RetrievalContextBuilt {
            policy: format!("{:?}", ctx.policy),
            sources: ctx
                .items
                .iter()
                .map(|item| format!("{:?}", item.source))
                .collect(),
            items: ctx.items.len(),
            estimated_tokens: ctx.token_estimate,
            provenance: ctx.provenance_summaries(),
            conflicts: ctx.conflict_count(),
        });
        let retrieval_block = ctx.format_for_prompt();
        let enhanced = format!("{content}\n{retrieval_block}");
        request_messages[last_user_idx] = Message::user(&enhanced);
        debug!("Prefetched memory context injected into user message");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::TurnTrace;

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        }
    }

    #[tokio::test]
    async fn prepare_appends_focused_prompt_and_records_request_budget() {
        let trace =
            TraceCollector::new(TurnTrace::new("session-test".to_string(), 1, "update code"));
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let focused_prompt = Message::system("focused repair prompt");
        let tools = vec![tool("file_edit"), tool("file_read")];
        let prepared = RequestPreparationController::prepare(RequestPreparationContext {
            messages: &[Message::user("change src/lib.rs")],
            focused_repair_prompt: Some(focused_prompt),
            turn_retrieval_context: None,
            retrieval_policy: RetrievalPolicy::None,
            memory_manager: None,
            provider: None,
            model: "test-model",
            tools: &tools,
            trace: &trace,
            runtime_diet: &mut runtime_diet,
        })
        .await;

        assert_eq!(prepared.request.model, "test-model");
        assert_eq!(prepared.request.messages.len(), 2);
        assert!(matches!(
            prepared.request.messages.last(),
            Some(Message::System { content }) if content == "focused repair prompt"
        ));
        assert_eq!(prepared.request.tools.as_ref().map(Vec::len), Some(2));
        assert_eq!(runtime_diet.exposed_tools, 2);
        assert!(runtime_diet.total_request_tokens > 0);
    }

    #[tokio::test]
    async fn prepare_skips_memory_prefetch_without_memory_manager() {
        let trace = TraceCollector::new(TurnTrace::new(
            "session-test".to_string(),
            1,
            "inspect repo",
        ));
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let tools = vec![tool("file_read")];
        let prepared = RequestPreparationController::prepare(RequestPreparationContext {
            messages: &[Message::user("remembered context should not be injected")],
            focused_repair_prompt: None,
            turn_retrieval_context: None,
            retrieval_policy: RetrievalPolicy::Memory,
            memory_manager: None,
            provider: None,
            model: "test-model",
            tools: &tools,
            trace: &trace,
            runtime_diet: &mut runtime_diet,
        })
        .await;

        assert_eq!(prepared.request.messages.len(), 1);
        assert_eq!(runtime_diet.retrieval_items, 0);
    }
}
