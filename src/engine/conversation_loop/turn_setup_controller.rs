use super::validation_runner::RequiredValidationController;
use super::workflow_prompt_policy::WorkflowPromptPolicy;
use super::ConversationLoop;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::intent_router::{
    IntentKind, IntentRoute, IntentRouter, ReasoningPolicy, RetrievalPolicy, RiskLevel,
    WorkflowKind,
};
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::trace::{TraceCollector, TraceEvent, TurnStatus, TurnTrace};
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
        // Shadow diagnostics: record all matching heuristic candidates (gated by env var)
        IntentRouter::new().record_route_candidates(&last_user_preview, &trace);
        Self::apply_unfinished_route_continuation(
            &mut route,
            &last_user_preview,
            context.conversation,
        );
        context.conversation.agent_mode.apply_to_route(&mut route);
        Self::record_route(&trace, context.conversation, &route);
        let resource_policy = ResourcePolicy::from_route(&route);
        Self::record_resource_policy(&trace, &resource_policy);
        let working_dir = context
            .conversation
            .working_dir_override
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
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

    fn apply_unfinished_route_continuation(
        route: &mut IntentRoute,
        user_message: &str,
        conversation: &ConversationLoop,
    ) {
        if !is_weak_followup_route(route, user_message) || has_topic_switch_signal(user_message) {
            return;
        }
        let Some(previous) = conversation
            .trace_store
            .as_ref()
            .and_then(|store| store.latest())
        else {
            return;
        };
        if previous.status == TurnStatus::Completed {
            return;
        }
        let Some(previous_route) = previous_route_from_trace(&previous) else {
            return;
        };
        match previous_route.workflow {
            WorkflowKind::CodeChange | WorkflowKind::BugFix => {
                route.intent = previous_route.intent;
                route.workflow = previous_route.workflow;
                route.retrieval = RetrievalPolicy::Project;
                route.reasoning = ReasoningPolicy::High;
                route.risk = stronger_risk(previous_route.risk, route.risk);
                route.confidence = route.confidence.max(0.76);
                route.recommended_tools = match previous_route.workflow {
                    WorkflowKind::CodeChange => vec![
                        "project_list".into(),
                        "grep".into(),
                        "file_read".into(),
                        "file_write".into(),
                        "file_edit".into(),
                        "bash".into(),
                    ],
                    WorkflowKind::BugFix => vec!["grep".into(), "file_read".into(), "bash".into()],
                    _ => route.recommended_tools.clone(),
                };
                route.reason.push_str(&format!(
                    "; route continuation: inherited unfinished {:?} turn {}",
                    previous_route.workflow, previous.turn_index
                ));
            }
            _ => {}
        }
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
            allow_fallback_model: resource_policy.allow_fallback_model,
            reason: resource_policy.reason.clone(),
        });
    }
}

#[derive(Debug, Clone, Copy)]
struct PreviousRoute {
    intent: IntentKind,
    workflow: WorkflowKind,
    risk: RiskLevel,
}

fn previous_route_from_trace(trace: &TurnTrace) -> Option<PreviousRoute> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::IntentRouted {
            intent,
            workflow,
            risk,
            ..
        } => Some(PreviousRoute {
            intent: parse_intent_kind(intent)?,
            workflow: parse_workflow_kind(workflow)?,
            risk: parse_risk_level(risk).unwrap_or(RiskLevel::Medium),
        }),
        _ => None,
    })
}

fn is_weak_followup_route(route: &IntentRoute, user_message: &str) -> bool {
    if !matches!(route.intent, IntentKind::DirectAnswer | IntentKind::Unknown) {
        return false;
    }
    if matches!(
        route.retrieval,
        RetrievalPolicy::Project | RetrievalPolicy::Full
    ) {
        return false;
    }
    route.confidence <= 0.72 || has_followup_reference(user_message)
}

fn has_followup_reference(user_message: &str) -> bool {
    let lower = user_message.to_ascii_lowercase();
    let zh = user_message;
    contains_any(
        &lower,
        &[
            "also",
            "same",
            "that file",
            "the other file",
            "continue",
            "next one",
            "that one",
        ],
    ) || contains_any(
        zh,
        &[
            "还有",
            "那个",
            "另一个",
            "同样",
            "继续",
            "也改",
            "也修",
            "接着",
        ],
    )
}

fn has_topic_switch_signal(user_message: &str) -> bool {
    let lower = user_message.to_ascii_lowercase();
    let zh = user_message;
    contains_any(
        &lower,
        &[
            "new topic",
            "different topic",
            "forget that",
            "unrelated",
            "search web",
            "latest",
            "remember this",
            "save memory",
        ],
    ) || contains_any(
        zh,
        &[
            "新话题",
            "换个话题",
            "先别管",
            "无关",
            "搜索",
            "最新",
            "记住",
            "保存记忆",
            "只读",
            "不要修改",
            "不要改",
        ],
    )
}

fn parse_intent_kind(value: &str) -> Option<IntentKind> {
    match value {
        "DirectAnswer" => Some(IntentKind::DirectAnswer),
        "CodeChange" => Some(IntentKind::CodeChange),
        "Debugging" => Some(IntentKind::Debugging),
        "Research" => Some(IntentKind::Research),
        "Memory" => Some(IntentKind::Memory),
        "Configuration" => Some(IntentKind::Configuration),
        "Delegation" => Some(IntentKind::Delegation),
        "Planning" => Some(IntentKind::Planning),
        "Unknown" => Some(IntentKind::Unknown),
        _ => None,
    }
}

fn parse_workflow_kind(value: &str) -> Option<WorkflowKind> {
    match value {
        "Direct" => Some(WorkflowKind::Direct),
        "CodeChange" => Some(WorkflowKind::CodeChange),
        "BugFix" => Some(WorkflowKind::BugFix),
        "Research" => Some(WorkflowKind::Research),
        "Planning" => Some(WorkflowKind::Planning),
        "Delegation" => Some(WorkflowKind::Delegation),
        _ => None,
    }
}

fn parse_risk_level(value: &str) -> Option<RiskLevel> {
    match value {
        "Low" => Some(RiskLevel::Low),
        "Medium" => Some(RiskLevel::Medium),
        "High" => Some(RiskLevel::High),
        _ => None,
    }
}

fn stronger_risk(left: RiskLevel, right: RiskLevel) -> RiskLevel {
    match (risk_rank(left), risk_rank(right)) {
        (l, r) if l >= r => left,
        _ => right,
    }
}

fn risk_rank(value: RiskLevel) -> u8 {
    match value {
        RiskLevel::Low => 0,
        RiskLevel::Medium => 1,
        RiskLevel::High => 2,
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::{TraceStore, TurnStatus};
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
        assert_eq!(setup.required_validation_commands, vec!["cargo test -q"]);
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

    #[test]
    fn prepare_inherits_unfinished_code_change_route_for_weak_followup() {
        let trace_store = Arc::new(TraceStore::default());
        let previous = TraceCollector::new(TurnTrace::new("test-session", 1, "修改 src/lib.rs"));
        previous.record(TraceEvent::IntentRouted {
            agent_mode: Some("auto".to_string()),
            intent: "CodeChange".to_string(),
            workflow: "CodeChange".to_string(),
            retrieval: "Project".to_string(),
            confidence: 0.77,
            risk: "Medium".to_string(),
            reason: "prompt asks for code or product changes".to_string(),
        });
        trace_store.push(previous.finish(TurnStatus::Failed));

        let mut conversation = conversation();
        conversation.session_id = "test-session".to_string();
        conversation = conversation.with_trace_store(trace_store);
        let setup = TurnSetupController::prepare(TurnSetupContext {
            conversation: &conversation,
            messages: &[Message::user("还有那个文件也改一下")],
        });

        assert_eq!(setup.route.intent, IntentKind::CodeChange);
        assert_eq!(setup.route.workflow, WorkflowKind::CodeChange);
        assert_eq!(setup.route.retrieval, RetrievalPolicy::Project);
        assert!(setup
            .route
            .reason
            .contains("route continuation: inherited unfinished CodeChange"));
    }

    #[test]
    fn prepare_does_not_inherit_when_followup_switches_topic() {
        let trace_store = Arc::new(TraceStore::default());
        let previous = TraceCollector::new(TurnTrace::new("test-session", 1, "修复 cargo test"));
        previous.record(TraceEvent::IntentRouted {
            agent_mode: Some("auto".to_string()),
            intent: "Debugging".to_string(),
            workflow: "BugFix".to_string(),
            retrieval: "Project".to_string(),
            confidence: 0.8,
            risk: "Medium".to_string(),
            reason: "prompt describes a failure or debugging task".to_string(),
        });
        trace_store.push(previous.finish(TurnStatus::Failed));

        let mut conversation = conversation();
        conversation.session_id = "test-session".to_string();
        conversation = conversation.with_trace_store(trace_store);
        let setup = TurnSetupController::prepare(TurnSetupContext {
            conversation: &conversation,
            messages: &[Message::user(
                "换个话题，只读看看 docs 目录有什么，不要修改",
            )],
        });

        assert_ne!(setup.route.workflow, WorkflowKind::BugFix);
        assert_eq!(setup.route.intent, IntentKind::DirectAnswer);
        assert!(!setup.route.reason.contains("route continuation"));
    }
}
