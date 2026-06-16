use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;
use priority_agent::services::api::{ChatRequest, ChatResponse, LlmProvider};

/// Mock provider that returns scripted responses from a FIFO queue.
pub struct MockProvider {
    base_url: String,
    model: String,
    responses: std::sync::Mutex<Vec<ChatResponse>>,
}

impl MockProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            base_url: "mock://e2e".to_string(),
            model: model.into(),
            responses: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn from_responses(model: impl Into<String>, responses: Vec<ChatResponse>) -> Self {
        Self {
            base_url: "mock://e2e".to_string(),
            model: model.into(),
            responses: std::sync::Mutex::new(responses),
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
        anyhow::bail!("MockProvider chat_stream not implemented — use non-streaming path")
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn default_model(&self) -> &str {
        &self.model
    }
}

/// Build a simple text ChatResponse without tool calls.
pub fn text_response(content: impl Into<String>) -> ChatResponse {
    ChatResponse {
        content: content.into(),
        tool_calls: None,
        usage: None,
        tool_call_repair: None,
        finish_reason: None,
    }
}

/// Build a ChatResponse with a single tool call.
pub fn tool_response(name: impl Into<String>, args: serde_json::Value) -> ChatResponse {
    use priority_agent::services::api::ToolCall;
    ChatResponse {
        content: String::new(),
        tool_calls: Some(vec![ToolCall {
            id: "call_1".to_string(),
            name: name.into(),
            arguments: args,
        }]),
        usage: None,
        tool_call_repair: None,
        finish_reason: None,
    }
}
