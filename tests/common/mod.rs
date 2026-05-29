//! Shared test fixtures for integration tests.

use priority_agent::services::api::{ChatRequest, ChatResponse, LlmProvider, Message, Usage};
use priority_agent::tools::ToolRegistry;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A mock LLM provider that returns pre-configured responses in sequence.
pub struct MockProvider {
    responses: Mutex<VecDeque<ChatResponse>>,
    pub call_count: std::sync::atomic::AtomicU32,
}

impl MockProvider {
    pub fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            call_count: std::sync::atomic::AtomicU32::new(0),
        }
    }

    pub fn single(response: ChatResponse) -> Self {
        Self::new(vec![response])
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        Self::single(ChatResponse {
            content: text.into(),
            tool_calls: None,
            usage: None,
        })
    }

    pub fn with_tool_call(name: &str, arguments: &str) -> Self {
        Self::single(ChatResponse {
            content: "".to_string(),
            tool_calls: Some(vec![priority_agent::services::api::ToolCall {
                id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                name: name.to_string(),
                arguments: serde_json::from_str(arguments)
                    .unwrap_or_else(|_| serde_json::Value::String(arguments.to_string())),
            }]),
            usage: None,
        })
    }

    pub fn call_count(&self) -> u32 {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut responses = self.responses.lock().unwrap();
        responses
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("no more mock responses"))
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<async_openai::types::ChatCompletionResponseStream> {
        anyhow::bail!("streaming not implemented in mock provider")
    }

    fn base_url(&self) -> &str {
        "https://mock.local"
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }
}

/// Create a temporary workspace directory for tests.
pub fn temp_workspace() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "priority-agent-test-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Create a tool registry with only built-in tools (no plugin injection).
pub fn tool_registry() -> Arc<ToolRegistry> {
    Arc::new(ToolRegistry::default_registry())
}
