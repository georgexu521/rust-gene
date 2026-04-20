//! API 客户端模块
//!
//! 支持多个 LLM 提供商：Kimi、OpenAI

pub mod kimi;
pub mod minimax;
pub mod openai;
pub mod openai_compat;

use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;

/// LLM Provider trait - 抽象不同的 API 提供商
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 发送聊天请求（非流式）
    async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse>;

    /// 发送聊天请求（流式）
    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream>;

    /// 获取 Base URL
    fn base_url(&self) -> &str;

    /// 获取默认模型
    fn default_model(&self) -> &str;
}

/// 聊天请求
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<Tool>>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: Vec::new(),
            tools: None,
            temperature: Some(0.6),
            max_tokens: None,
        }
    }

    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }
}

/// 消息
#[derive(Debug, Clone)]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Message::System {
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Message::User {
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Message::Assistant {
            content: content.into(),
            tool_calls: None,
        }
    }

    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Message::Assistant {
            content: content.into(),
            tool_calls: Some(tool_calls),
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Message::Tool {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
        }
    }

    /// 获取工具调用列表（如果是 Assistant 消息且有工具调用）
    pub fn tool_calls(&self) -> Option<&Vec<ToolCall>> {
        match self {
            Message::Assistant { tool_calls, .. } => tool_calls.as_ref(),
            _ => None,
        }
    }
}

/// 工具定义
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl Tool {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    pub fn with_parameters(mut self, params: serde_json::Value) -> Self {
        self.parameters = params;
        self
    }
}

/// 工具调用
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// 聊天响应
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub usage: Option<Usage>,
}

/// Token 使用量
#[derive(Debug, Clone)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
