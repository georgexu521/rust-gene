use crate::engine::intent_router::{IntentRoute, RiskLevel};
use crate::engine::workflow_contract::ProgrammingWorkflowJudgment;
use crate::services::api::LlmProvider;
use crate::session_store::SessionStore;
use std::sync::Arc;
use tracing::warn;

pub(super) fn workflow_contract_enabled(provider: &dyn LlmProvider) -> bool {
    if provider.base_url().starts_with("mock://") {
        return false;
    }

    std::env::var("PRIORITY_AGENT_WORKFLOW_CONTRACT")
        .map(|value| workflow_contract_env_enabled(&value))
        .unwrap_or(true)
}

fn workflow_contract_env_enabled(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    !matches!(value.as_str(), "0" | "false" | "off" | "no")
}

pub(super) fn persist_workflow_learning_event(
    store: Option<&Arc<SessionStore>>,
    session_id: &str,
    kind: &str,
    summary: String,
    confidence: f64,
    payload: serde_json::Value,
) {
    let Some(store) = store else {
        return;
    };
    if let Err(e) = store.add_learning_event(
        session_id,
        kind,
        "conversation_loop",
        &summary,
        confidence,
        &payload,
    ) {
        warn!("Failed to persist workflow learning event: {}", e);
    }
}

pub(super) fn is_high_risk_workflow(
    route: &IntentRoute,
    judgment: Option<&ProgrammingWorkflowJudgment>,
) -> bool {
    matches!(route.risk, RiskLevel::High)
        || judgment
            .map(|judgment| matches!(judgment.risk, RiskLevel::High))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{
        IntentKind, ReasoningPolicy, RetrievalPolicy, WorkflowKind,
    };
    use crate::services::api::{ChatRequest, ChatResponse};
    use async_openai::types::ChatCompletionResponseStream;

    struct MockProvider;

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Err(anyhow::anyhow!("not used"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("not used"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    fn route(risk: RiskLevel) -> IntentRoute {
        IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 1.0,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::Medium,
            risk,
            recommended_tools: Vec::new(),
            reason: "test".to_string(),
        }
    }

    #[test]
    fn workflow_contract_env_false_values_disable_contract() {
        for value in ["0", "false", "off", "no", " FALSE "] {
            assert!(!workflow_contract_env_enabled(value));
        }
        assert!(workflow_contract_env_enabled("1"));
        assert!(workflow_contract_env_enabled("true"));
    }

    #[test]
    fn mock_provider_disables_workflow_contract() {
        assert!(!workflow_contract_enabled(&MockProvider));
    }

    #[test]
    fn route_risk_marks_high_risk_workflow() {
        assert!(is_high_risk_workflow(&route(RiskLevel::High), None));
        assert!(!is_high_risk_workflow(&route(RiskLevel::Medium), None));
    }
}
