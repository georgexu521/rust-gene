use super::validation_runner::RequiredValidationController;
use super::workflow_prompt_policy::WorkflowPromptPolicy;
use super::ConversationLoop;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::intent_router::{IntentRoute, IntentRouter};
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::trace::{TraceCollector, TraceEvent, TurnTrace};
use crate::services::api::Message;
use crate::session_store::LearningEventRecord;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

pub(super) struct TurnSetupContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) messages: &'a [Message],
}

pub(super) struct TurnSetup {
    pub(super) last_user_preview: String,
    pub(super) required_validation_commands: Vec<String>,
    pub(super) no_diff_audit_closeout_allowed: bool,
    pub(super) code_write_tools_forbidden: bool,
    pub(super) trace: TraceCollector,
    pub(super) learning_events: Vec<LearningEventRecord>,
    pub(super) route: IntentRoute,
    pub(super) resource_policy: ResourcePolicy,
    pub(super) working_dir: PathBuf,
    pub(super) destructive_scope: DestructiveScopeContract,
}

pub(super) struct TurnSetupController;

impl TurnSetupController {
    pub(super) fn prepare(context: TurnSetupContext<'_>) -> TurnSetup {
        let last_user_preview = Self::last_user_preview(context.messages).to_string();
        let required_validation_commands =
            RequiredValidationController::extract_commands(&last_user_preview);
        let no_diff_audit_closeout_allowed =
            WorkflowPromptPolicy::allows_no_diff_audit_closeout(&last_user_preview);
        let code_write_tools_forbidden =
            WorkflowPromptPolicy::forbids_code_write_tools(&last_user_preview);
        let turn_index = Self::next_turn_index(context.conversation);
        let trace = TraceCollector::new(TurnTrace::new(
            context.conversation.session_id.clone(),
            turn_index,
            &last_user_preview,
        ));
        let learning_events = context
            .conversation
            .session_store
            .as_ref()
            .and_then(|store| {
                store
                    .recent_learning_events(&context.conversation.session_id, 20)
                    .ok()
            })
            .unwrap_or_default();
        let mut route =
            IntentRouter::new().route_with_learning(&last_user_preview, &learning_events);
        context.conversation.agent_mode.apply_to_route(&mut route);
        Self::record_route(&trace, context.conversation, &route);
        let resource_policy = ResourcePolicy::from_route(&route);
        Self::record_resource_policy(&trace, &resource_policy);
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let destructive_scope =
            DestructiveScopeContract::from_user_request(&last_user_preview, &working_dir);

        TurnSetup {
            last_user_preview,
            required_validation_commands,
            no_diff_audit_closeout_allowed,
            code_write_tools_forbidden,
            trace,
            learning_events,
            route,
            resource_policy,
            working_dir,
            destructive_scope,
        }
    }

    fn last_user_preview(messages: &[Message]) -> &str {
        messages
            .iter()
            .rposition(|message| matches!(message, Message::User { .. }))
            .and_then(|index| match &messages[index] {
                Message::User { content } => Some(content.as_str()),
                _ => None,
            })
            .unwrap_or("")
    }

    fn next_turn_index(conversation: &ConversationLoop) -> u64 {
        conversation
            .trace_store
            .as_ref()
            .and_then(|store| store.latest().map(|trace| trace.turn_index + 1))
            .unwrap_or_else(|| conversation.turn_counter.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn record_route(trace: &TraceCollector, conversation: &ConversationLoop, route: &IntentRoute) {
        trace.record(TraceEvent::IntentRouted {
            agent_mode: Some(conversation.agent_mode.label().to_string()),
            intent: format!("{:?}", route.intent),
            workflow: format!("{:?}", route.workflow),
            retrieval: format!("{:?}", route.retrieval),
            confidence: route.confidence,
            risk: format!("{:?}", route.risk),
            reason: route.reason.clone(),
        });
    }

    fn record_resource_policy(trace: &TraceCollector, resource_policy: &ResourcePolicy) {
        trace.record(TraceEvent::ResourcePolicySelected {
            latency: format!("{:?}", resource_policy.latency),
            target_ms: resource_policy.latency.target_ms(),
            cost_ceiling_usd: resource_policy.cost_ceiling_usd,
            reasoning: format!("{:?}", resource_policy.reasoning),
            parallelism_limit: resource_policy.parallelism_limit,
            max_tool_calls: resource_policy.max_tool_calls,
            context_budget_tokens: resource_policy.context_budget_tokens,
            reason: resource_policy.reason.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::TurnStatus;
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockProvider;

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Err(anyhow::anyhow!("chat not used in this test"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    fn conversation() -> ConversationLoop {
        ConversationLoop::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::new()),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "mock-model".to_string(),
        )
    }

    #[test]
    fn last_user_preview_uses_last_user_message() {
        let messages = vec![
            Message::user("first"),
            Message::assistant("assistant"),
            Message::user("second"),
        ];

        assert_eq!(TurnSetupController::last_user_preview(&messages), "second");
    }

    #[test]
    fn prepare_records_route_and_resource_policy() {
        let conversation = conversation();
        let messages = vec![Message::user("运行 cargo test -q")];

        let setup = TurnSetupController::prepare(TurnSetupContext {
            conversation: &conversation,
            messages: &messages,
        });

        assert_eq!(setup.last_user_preview, "运行 cargo test -q");
        assert!(setup.required_validation_commands.is_empty());
        assert!(setup.working_dir.is_absolute() || setup.working_dir == PathBuf::from("."));
        let finished = setup.trace.finish(TurnStatus::Completed);
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::IntentRouted { .. })));
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::ResourcePolicySelected { .. })));
    }
}
