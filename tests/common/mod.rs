//! Shared test fixtures for integration tests.
#![allow(dead_code)]

use async_openai::types::{
    ChatChoiceStream, ChatCompletionMessageToolCallChunk, ChatCompletionResponseStream,
    ChatCompletionStreamResponseDelta, ChatCompletionToolType, CreateChatCompletionStreamResponse,
    FinishReason, FunctionCallStream,
};
use once_cell::sync::Lazy;
use priority_agent::services::api::{ChatRequest, ChatResponse, LlmProvider};
use priority_agent::tools::ToolRegistry;
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

static ENV_LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));

pub struct EnvGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    saved: HashMap<String, Option<String>>,
}

impl EnvGuard {
    pub async fn acquire() -> Self {
        Self {
            _lock: ENV_LOCK.lock().await,
            saved: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.capture_if_needed(key);
        unsafe { std::env::set_var(key, value) };
    }

    fn capture_if_needed(&mut self, key: &str) {
        if !self.saved.contains_key(key) {
            self.saved.insert(key.to_string(), std::env::var(key).ok());
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, old_value) in self.saved.drain() {
            match old_value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

/// Synchronous environment variable guard for non-async tests.
pub struct EnvGuardSync {
    saved: HashMap<String, Option<String>>,
}

impl EnvGuardSync {
    pub fn new() -> Self {
        Self {
            saved: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        if !self.saved.contains_key(key) {
            self.saved.insert(key.to_string(), std::env::var(key).ok());
        }
        unsafe { std::env::set_var(key, value) };
    }
}

impl Default for EnvGuardSync {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for EnvGuardSync {
    fn drop(&mut self) {
        for (key, old_value) in self.saved.drain() {
            match old_value {
                Some(value) => unsafe { std::env::set_var(&key, &value) },
                None => unsafe { std::env::remove_var(&key) },
            }
        }
    }
}

/// A mock LLM provider that returns pre-configured responses in sequence.
pub struct MockProvider {
    responses: Mutex<VecDeque<ChatResponse>>,
    stream_responses: Mutex<VecDeque<Vec<CreateChatCompletionStreamResponse>>>,
    pub call_count: std::sync::atomic::AtomicU32,
}

impl MockProvider {
    pub fn with_streams(stream_responses: Vec<Vec<CreateChatCompletionStreamResponse>>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
            stream_responses: Mutex::new(stream_responses.into()),
            call_count: std::sync::atomic::AtomicU32::new(0),
        }
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
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let chunks = self
            .stream_responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("no more mock stream responses"))?;
        Ok(Box::pin(futures::stream::iter(chunks.into_iter().map(Ok))))
    }

    fn base_url(&self) -> &str {
        "https://mock.local"
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }
}

/// Create a tool registry with only built-in tools (no plugin injection).
pub fn tool_registry() -> Arc<ToolRegistry> {
    Arc::new(ToolRegistry::default_registry())
}

pub fn stream_text_response(text: &str) -> Vec<CreateChatCompletionStreamResponse> {
    vec![stream_chunk(
        Some(text.to_string()),
        None,
        None,
        Some(FinishReason::Stop),
    )]
}

pub fn stream_tool_call_response(
    id: &str,
    name: &str,
    arguments: serde_json::Value,
) -> Vec<CreateChatCompletionStreamResponse> {
    vec![stream_chunk(
        None,
        Some(id.to_string()),
        Some((name.to_string(), arguments.to_string())),
        Some(FinishReason::ToolCalls),
    )]
}

#[allow(deprecated)]
fn stream_chunk(
    content: Option<String>,
    tool_call_id: Option<String>,
    function: Option<(String, String)>,
    finish_reason: Option<FinishReason>,
) -> CreateChatCompletionStreamResponse {
    let tool_calls = tool_call_id.map(|id| {
        vec![ChatCompletionMessageToolCallChunk {
            index: 0,
            id: Some(id),
            r#type: Some(ChatCompletionToolType::Function),
            function: function.map(|(name, arguments)| FunctionCallStream {
                name: Some(name),
                arguments: Some(arguments),
            }),
        }]
    });

    CreateChatCompletionStreamResponse {
        id: format!("chatcmpl_{}", uuid::Uuid::new_v4().simple()),
        choices: vec![ChatChoiceStream {
            index: 0,
            delta: ChatCompletionStreamResponseDelta {
                content,
                function_call: None,
                tool_calls,
                role: Some(async_openai::types::Role::Assistant),
                refusal: None,
            },
            finish_reason,
            logprobs: None,
        }],
        created: 0,
        model: "mock-model".to_string(),
        service_tier: None,
        system_fingerprint: None,
        object: "chat.completion.chunk".to_string(),
        usage: None,
    }
}

pub fn calculate_tool_call_stream() -> Vec<CreateChatCompletionStreamResponse> {
    stream_tool_call_response("call_1", "calculate", json!({"expression": "2 + 3"}))
}
