//! End-to-end deterministic tests for Priority Agent.
//!
//! These tests use a mock LLM provider to verify complete user flows
//! without requiring a real provider API key.

use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;
use priority_agent::services::api::{ChatRequest, ChatResponse, LlmProvider};

/// Mock provider that returns scripted responses.
pub struct MockProvider {
    base_url: String,
    model: String,
    responses: std::sync::Mutex<Vec<ChatResponse>>,
}

impl MockProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            base_url: "http://mock".to_string(),
            model: model.into(),
            responses: std::sync::Mutex::new(Vec::new()),
        }
    }

    #[allow(dead_code)]
    pub fn queue_response(&self, response: ChatResponse) {
        self.responses.lock().unwrap().push(response);
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            anyhow::bail!("MockProvider out of responses");
        }
        Ok(responses.remove(0))
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        anyhow::bail!("MockProvider chat_stream not implemented")
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn default_model(&self) -> &str {
        &self.model
    }
}
