//! Kimi (Moonshot AI) API 客户端
//!
//! 支持 OpenAI 兼容格式的 API 调用

use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, Message, ToolCall, Usage};
use anyhow::{Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
        ChatCompletionRequestUserMessage, ChatCompletionTool, ChatCompletionToolType,
        CreateChatCompletionRequest, FunctionCall, FunctionObject,
    },
    Client,
};
use async_trait::async_trait;
use tracing::{debug, info};

/// Kimi API 配置
#[derive(Debug, Clone)]
pub struct KimiConfig {
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
}

impl KimiConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("MOONSHOT_API_KEY").context("MOONSHOT_API_KEY must be set")?;

        let base_url = std::env::var("MOONSHOT_BASE_URL")
            .unwrap_or_else(|_| "https://api.moonshot.ai/v1".to_string());

        let default_model =
            std::env::var("MOONSHOT_MODEL").unwrap_or_else(|_| "kimi-k2.5".to_string());

        Ok(Self {
            api_key,
            base_url,
            default_model,
        })
    }

    /// 加载 .env 文件并创建配置
    pub fn init() -> Result<Self> {
        // 尝试加载 .env 文件（如果不存在则忽略）
        let _ = dotenvy::dotenv();
        Self::from_env()
    }
}

/// Kimi API 客户端
pub struct KimiClient {
    client: Client<OpenAIConfig>,
    config: KimiConfig,
}

impl KimiClient {
    /// 创建新的 Kimi 客户端
    pub fn new(config: KimiConfig) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_key(&config.api_key)
            .with_api_base(&config.base_url);

        let client = Client::with_config(openai_config);

        info!("Kimi client initialized with base URL: {}", config.base_url);

        Self { client, config }
    }

    /// 从环境变量创建客户端
    pub fn from_env() -> Result<Self> {
        let config = KimiConfig::init()?;
        Ok(Self::new(config))
    }

    /// 获取默认模型名称
    pub fn default_model(&self) -> &str {
        &self.config.default_model
    }

    /// 获取 Base URL
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }
}

#[async_trait]
impl LlmProvider for KimiClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        debug!("Sending chat request to Kimi API");

        let messages: Vec<ChatCompletionRequestMessage> =
            request.messages.into_iter().map(convert_message).collect();

        let mut req = CreateChatCompletionRequest {
            model: request.model,
            messages,
            temperature: request.temperature,
            max_completion_tokens: request.max_tokens,
            tools: None,
            ..Default::default()
        };

        // 添加工具（如果提供）
        if let Some(tools) = request.tools {
            req.tools = Some(
                tools
                    .into_iter()
                    .map(|t| ChatCompletionTool {
                        r#type: ChatCompletionToolType::Function,
                        function: FunctionObject {
                            name: t.name,
                            description: Some(t.description),
                            parameters: Some(t.parameters),
                            strict: None,
                        },
                    })
                    .collect(),
            );
        }

        debug!("Request: {:?}", req);

        let response = self
            .client
            .chat()
            .create(req)
            .await
            .context("Failed to get response from Kimi API")?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .context("No choices in response")?;

        let message = choice.message;

        // 提取工具调用
        let tool_calls: Option<Vec<ToolCall>> = message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|call| ToolCall {
                    id: call.id,
                    name: call.function.name,
                    arguments: serde_json::from_str(&call.function.arguments).unwrap_or_else(|e| {
                        tracing::warn!(
                            "Failed to parse tool arguments '{}': {}",
                            &call.function.arguments,
                            e
                        );
                        serde_json::Value::Null
                    }),
                })
                .collect()
        });

        // 提取使用量
        let usage = response.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ChatResponse {
            content: message.content.unwrap_or_default(),
            tool_calls,
            usage,
        })
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<async_openai::types::ChatCompletionResponseStream> {
        debug!("Sending streaming chat request to Kimi API");

        let messages: Vec<ChatCompletionRequestMessage> =
            request.messages.into_iter().map(convert_message).collect();

        let mut req = CreateChatCompletionRequest {
            model: request.model,
            messages,
            temperature: request.temperature,
            max_completion_tokens: request.max_tokens,
            tools: None,
            ..Default::default()
        };

        // 添加工具（如果提供）
        if let Some(tools) = request.tools {
            req.tools = Some(
                tools
                    .into_iter()
                    .map(|t| ChatCompletionTool {
                        r#type: ChatCompletionToolType::Function,
                        function: FunctionObject {
                            name: t.name,
                            description: Some(t.description),
                            parameters: Some(t.parameters),
                            strict: None,
                        },
                    })
                    .collect(),
            );
        }

        // 启用流式响应
        req.stream = Some(true);

        let stream = self
            .client
            .chat()
            .create_stream(req)
            .await
            .context("Failed to create streaming response from Kimi API")?;

        Ok(stream)
    }

    fn base_url(&self) -> &str {
        &self.config.base_url
    }

    fn default_model(&self) -> &str {
        &self.config.default_model
    }
}

/// 将内部 Message 转换为 OpenAI 格式
fn convert_message(msg: Message) -> ChatCompletionRequestMessage {
    match msg {
        Message::System { content } => ChatCompletionRequestSystemMessage::from(content).into(),
        Message::User { content } => ChatCompletionRequestUserMessage::from(content).into(),
        Message::Assistant {
            content,
            tool_calls,
        } => {
            let content = Some(ChatCompletionRequestAssistantMessageContent::Text(content));
            let tool_calls = tool_calls.map(|calls| {
                calls
                    .into_iter()
                    .map(|call| ChatCompletionMessageToolCall {
                        id: call.id,
                        r#type: ChatCompletionToolType::Function,
                        function: FunctionCall {
                            name: call.name,
                            arguments: call.arguments.to_string(),
                        },
                    })
                    .collect()
            });

            ChatCompletionRequestAssistantMessage {
                content,
                tool_calls,
                ..Default::default()
            }
            .into()
        }
        Message::Tool {
            tool_call_id,
            content,
        } => ChatCompletionRequestToolMessage {
            content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(content),
            tool_call_id,
        }
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kimi_config_from_env() {
        // 设置测试环境变量
        std::env::set_var("MOONSHOT_API_KEY", "test-key");
        std::env::set_var("MOONSHOT_BASE_URL", "https://test.api/v1");
        std::env::set_var("MOONSHOT_MODEL", "kimi-k2.5");

        let config = KimiConfig::from_env().unwrap();
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.base_url, "https://test.api/v1");
        assert_eq!(config.default_model, "kimi-k2.5");
    }
}
