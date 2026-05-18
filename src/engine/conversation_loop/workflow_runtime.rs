use crate::engine::intent_router::{IntentRoute, RiskLevel, WorkflowKind};
use crate::engine::workflow_contract::ProgrammingWorkflowJudgment;
use crate::services::api::LlmProvider;
use crate::session_store::SessionStore;
use std::sync::Arc;
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorkflowContractMode {
    Off,
    Auto,
    Force,
}

impl WorkflowContractMode {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Auto => "auto",
            Self::Force => "force",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkflowContractActivation {
    pub(super) mode: WorkflowContractMode,
    pub(super) phase: &'static str,
    pub(super) active: bool,
    pub(super) reason: String,
}

pub(super) fn workflow_contract_enabled(provider: &dyn LlmProvider) -> bool {
    !matches!(workflow_contract_mode(provider), WorkflowContractMode::Off)
}

pub(super) fn workflow_contract_mode(provider: &dyn LlmProvider) -> WorkflowContractMode {
    if provider.base_url().starts_with("mock://") {
        return WorkflowContractMode::Off;
    }

    std::env::var("PRIORITY_AGENT_WORKFLOW_CONTRACT")
        .map(|value| workflow_contract_env_mode(&value))
        .unwrap_or(WorkflowContractMode::Auto)
}

fn workflow_contract_env_mode(value: &str) -> WorkflowContractMode {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "0" | "false" | "off" | "no" => WorkflowContractMode::Off,
        "auto" | "" => WorkflowContractMode::Auto,
        "force" | "forced" | "always" | "strict" | "1" | "true" | "on" | "yes" => {
            WorkflowContractMode::Force
        }
        _ => WorkflowContractMode::Force,
    }
}

pub(super) fn turn_entry_workflow_contract_activation(
    provider: &dyn LlmProvider,
    route: &IntentRoute,
    required_validation_commands: &[String],
) -> WorkflowContractActivation {
    let mode = workflow_contract_mode(provider);
    match mode {
        WorkflowContractMode::Off => WorkflowContractActivation {
            mode,
            phase: "turn_entry",
            active: false,
            reason: "workflow contract mode is off".to_string(),
        },
        WorkflowContractMode::Force => WorkflowContractActivation {
            mode,
            phase: "turn_entry",
            active: true,
            reason: "workflow contract mode is force".to_string(),
        },
        WorkflowContractMode::Auto => {
            match auto_turn_entry_reason(route, required_validation_commands) {
                Some(reason) => WorkflowContractActivation {
                    mode,
                    phase: "turn_entry",
                    active: true,
                    reason: reason.to_string(),
                },
                None => WorkflowContractActivation {
                    mode,
                    phase: "turn_entry",
                    active: false,
                    reason: "auto mode skipped entry judgment for ordinary programming turn"
                        .to_string(),
                },
            }
        }
    }
}

fn auto_turn_entry_reason(
    route: &IntentRoute,
    required_validation_commands: &[String],
) -> Option<&'static str> {
    if matches!(route.risk, RiskLevel::High) {
        return Some("route risk is high");
    }
    if matches!(route.workflow, WorkflowKind::BugFix) {
        return Some("bug-fix workflow benefits from explicit acceptance judgment");
    }
    if required_validation_commands.len() >= 4 {
        return Some("complex required-validation surface");
    }
    None
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
            assert_eq!(workflow_contract_env_mode(value), WorkflowContractMode::Off);
        }
        assert_eq!(workflow_contract_env_mode("1"), WorkflowContractMode::Force);
        assert_eq!(
            workflow_contract_env_mode("true"),
            WorkflowContractMode::Force
        );
        assert_eq!(
            workflow_contract_env_mode("auto"),
            WorkflowContractMode::Auto
        );
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

    #[test]
    fn auto_turn_entry_targets_high_risk_bugfix_or_complex_validation() {
        assert_eq!(
            auto_turn_entry_reason(&route(RiskLevel::High), &[]),
            Some("route risk is high")
        );

        let mut bugfix = route(RiskLevel::Medium);
        bugfix.workflow = WorkflowKind::BugFix;
        assert_eq!(
            auto_turn_entry_reason(&bugfix, &[]),
            Some("bug-fix workflow benefits from explicit acceptance judgment")
        );

        let commands = vec![
            "cargo test a".to_string(),
            "cargo test b".to_string(),
            "rg c".to_string(),
            "cargo test".to_string(),
        ];
        assert_eq!(
            auto_turn_entry_reason(&route(RiskLevel::Medium), &commands),
            Some("complex required-validation surface")
        );

        assert_eq!(auto_turn_entry_reason(&route(RiskLevel::Medium), &[]), None);
    }
}
