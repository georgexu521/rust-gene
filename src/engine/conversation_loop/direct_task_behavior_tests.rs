use super::risk_signal_controller::{RiskSignalController, RiskSignalInput, RiskSignalLevel};
use super::workflow_runtime::turn_entry_workflow_contract_activation;
use super::ConversationLoop;
use crate::engine::intent_router::{
    IntentKind, IntentRouter, ReasoningPolicy, RetrievalPolicy, WorkflowKind,
};
use crate::engine::task_context::{AgentTaskMode, TaskContextBundle, VerificationStatus};
use crate::engine::workflow_contract::WorkflowContractPrompt;
use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, Tool};
use crate::test_utils::env_guard::EnvVarGuard;
use async_openai::types::ChatCompletionResponseStream;

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
        Err(anyhow::anyhow!("chat stream not used in this test"))
    }

    fn base_url(&self) -> &str {
        "https://provider.test/v1"
    }

    fn default_model(&self) -> &str {
        "test-model"
    }
}

fn fake_tools(names: &[&str]) -> Vec<Tool> {
    names
        .iter()
        .map(|name| Tool::new(*name, format!("{} tool", name)))
        .collect()
}

#[test]
fn simple_direct_prompts_keep_direct_route_and_no_tools() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
    env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
    env.remove("PRIORITY_AGENT_TOOL_PROFILE");

    let samples = [
        "简单回答：2+2 等于几？",
        "润色下面一句话：这个功能现在差不多能用了。",
        "解释一下 Rust ownership 是什么，不要看项目代码。",
        "这个报错是什么意思？ Error: failed to connect",
    ];
    let tools = fake_tools(&[
        "agent",
        "bash",
        "diff",
        "file_edit",
        "file_read",
        "git",
        "glob",
        "grep",
        "memory_load",
        "memory_save",
        "web_fetch",
        "web_search",
    ]);

    for prompt in samples {
        let route = IntentRouter::new().route(prompt);

        assert_eq!(route.intent, IntentKind::DirectAnswer, "{prompt}");
        assert_eq!(route.workflow, WorkflowKind::Direct, "{prompt}");
        assert_eq!(route.retrieval, RetrievalPolicy::Light, "{prompt}");
        assert_eq!(
            route.risk,
            crate::engine::intent_router::RiskLevel::Low,
            "{prompt}"
        );
        assert!(
            route.recommended_tools.is_empty(),
            "simple direct prompt should not recommend tools: {prompt}; route reason={}",
            route.reason
        );

        let exposed = ConversationLoop::route_scoped_tools(&tools, &route);
        assert!(
            exposed.is_empty(),
            "simple direct prompt exposed tools: {prompt}; exposed={:?}",
            exposed
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn simple_direct_prompts_do_not_activate_heavy_workflow_contract() {
    let mut env = EnvVarGuard::acquire_blocking();
    env.remove("PRIORITY_AGENT_WORKFLOW_CONTRACT");

    let samples = [
        ("简单回答：2+2 等于几？", ReasoningPolicy::Low),
        (
            "润色下面一句话：这个功能现在差不多能用了。",
            ReasoningPolicy::Low,
        ),
        (
            "解释一下 Rust ownership 是什么，不要看项目代码。",
            ReasoningPolicy::Low,
        ),
        (
            "这个报错是什么意思？ Error: failed to connect",
            ReasoningPolicy::Medium,
        ),
    ];
    let provider = MockProvider;

    for (prompt, expected_reasoning) in samples {
        let route = IntentRouter::new().route(prompt);
        let mut bundle = TaskContextBundle::new(prompt, ".", route.clone(), None);
        let assessment = RiskSignalController::assess_turn_entry(RiskSignalInput {
            route: &route,
            task_bundle: &bundle,
            required_validation_commands: &[],
        });
        let workflow_prompt = WorkflowContractPrompt::new(prompt, route.clone(), ".");
        let activation = turn_entry_workflow_contract_activation(&provider, &assessment);

        assert_eq!(bundle.agent_state.mode, AgentTaskMode::Direct, "{prompt}");
        assert_eq!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::NotRequired,
            "{prompt}"
        );
        assert_eq!(assessment.level, RiskSignalLevel::Ordinary, "{prompt}");
        assert!(
            !assessment.entry_contract,
            "direct prompt should not request workflow contract: {prompt}; reasons={:?}",
            assessment.reasons
        );
        assert!(
            !workflow_prompt.should_ask_model(),
            "direct prompt should not ask the model for workflow judgment: {prompt}"
        );
        assert!(
            !activation.active,
            "auto workflow contract activated for direct prompt: {prompt}; reason={}",
            activation.reason
        );
        assert_eq!(route.reasoning, expected_reasoning, "{prompt}");

        bundle.agent_state.mark_done("answered directly");
        assert!(bundle.agent_state.done_condition.satisfied, "{prompt}");
    }
}
