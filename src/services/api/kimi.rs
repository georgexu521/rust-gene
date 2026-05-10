//! Kimi (Moonshot AI) API 客户端
//!
//! 支持 OpenAI 兼容格式的 API 调用，支持 extended thinking

use crate::services::api::{
    normalize_tool_message_sequence, sanitize_assistant_content, ChatRequest, ChatResponse,
    LlmProvider, Message, ToolCall, Usage,
};
use anyhow::{Context, Result};
use async_openai::{
    config::{Config, OpenAIConfig},
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
        ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
        ChatCompletionRequestUserMessage, ChatCompletionStreamOptions, ChatCompletionTool,
        ChatCompletionToolType, CreateChatCompletionRequest, FunctionCall, FunctionObject,
    },
    Client,
};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tracing::{debug, info};

/// Thinking beta header 名称（Anthropic 格式）。
///
/// `HeaderName::from_static` requires a lowercase HTTP header name. Keep this
/// literal lowercase so provider discovery/tests cannot panic when thinking is
/// enabled.
const THINKING_BETA_HEADER: &str = "anthropic-beta";
/// interleaved-thinking beta - 允许在 tool use 期间进行 thinking
const THINKING_BETA_VALUE: &str = "interleaved-thinking=2025-05-14";

/// Kimi API 配置
#[derive(Debug, Clone)]
pub struct KimiConfig {
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
    /// 是否启用 thinking（extended thinking beta）
    pub thinking_enabled: bool,
    /// thinking budget（token 数），如果为 None 则使用 adaptive thinking
    pub thinking_budget: Option<u32>,
}

impl KimiConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("MOONSHOT_API_KEY").context("MOONSHOT_API_KEY must be set")?;

        let base_url = std::env::var("MOONSHOT_BASE_URL")
            .unwrap_or_else(|_| "https://api.moonshot.ai/v1".to_string());

        let default_model =
            std::env::var("MOONSHOT_MODEL").unwrap_or_else(|_| "kimi-k2.5".to_string());

        // PRIORITY_AGENT_THINKING=0 禁用，默认启用
        let thinking_enabled = std::env::var("PRIORITY_AGENT_THINKING")
            .ok()
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true);

        //thinking budget，默认为 adaptive（None）
        let thinking_budget = std::env::var("PRIORITY_AGENT_THINKING_BUDGET")
            .ok()
            .and_then(|v| v.parse::<u32>().ok());

        Ok(Self {
            api_key,
            base_url,
            default_model,
            thinking_enabled,
            thinking_budget,
        })
    }

    /// 加载 .env 文件并创建配置
    pub fn init() -> Result<Self> {
        let _ = dotenvy::dotenv();
        Self::from_env()
    }
}

/// 支持 thinking beta header 的自定义 Config
#[derive(Clone, Debug)]
struct ThinkingConfig {
    inner: OpenAIConfig,
    thinking_header: Option<(HeaderName, HeaderValue)>,
}

impl ThinkingConfig {
    fn new(api_key: &str, base_url: &str, thinking_enabled: bool) -> Self {
        let inner = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(base_url);

        let thinking_header = if thinking_enabled {
            Some((
                HeaderName::from_static(THINKING_BETA_HEADER),
                HeaderValue::from_static(THINKING_BETA_VALUE),
            ))
        } else {
            None
        };

        Self {
            inner,
            thinking_header,
        }
    }
}

impl Config for ThinkingConfig {
    fn headers(&self) -> HeaderMap {
        let mut headers = self.inner.headers();
        if let Some((name, value)) = &self.thinking_header {
            headers.insert(name.clone(), value.clone());
        }
        headers
    }

    fn url(&self, path: &str) -> String {
        self.inner.url(path)
    }

    fn query(&self) -> Vec<(&str, &str)> {
        self.inner.query()
    }

    fn api_base(&self) -> &str {
        self.inner.api_base()
    }

    fn api_key(&self) -> &secrecy::SecretBox<str> {
        self.inner.api_key()
    }
}

/// Kimi API 客户端
pub struct KimiClient {
    client: Client<ThinkingConfig>,
    config: KimiConfig,
}

impl KimiClient {
    /// 创建新的 Kimi 客户端
    pub fn new(config: KimiConfig) -> Self {
        let thinking_config =
            ThinkingConfig::new(&config.api_key, &config.base_url, config.thinking_enabled);

        let client = Client::with_config(thinking_config);

        info!(
            "Kimi client initialized with base URL: {}, thinking: {}",
            config.base_url, config.thinking_enabled
        );

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

    /// 是否启用了 thinking
    pub fn is_thinking_enabled(&self) -> bool {
        self.config.thinking_enabled
    }

    /// 获取 thinking budget
    pub fn thinking_budget(&self) -> Option<u32> {
        self.config.thinking_budget
    }
}

#[async_trait]
impl LlmProvider for KimiClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        debug!(
            "Sending chat request to Kimi API (thinking: {})",
            self.config.thinking_enabled
        );

        let messages: Vec<ChatCompletionRequestMessage> =
            normalize_tool_message_sequence(request.messages)
                .into_iter()
                .map(convert_message)
                .collect();

        let mut req = CreateChatCompletionRequest {
            model: request.model.clone(),
            messages,
            temperature: request.temperature,
            max_completion_tokens: request.max_tokens,
            tools: None,
            ..Default::default()
        };

        // 如果有 thinking budget，添加到请求中
        if let Some(budget) = self.config.thinking_budget {
            req.max_completion_tokens = Some(budget);
        }

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
            reasoning_tokens: u
                .completion_tokens_details
                .as_ref()
                .and_then(|d| d.reasoning_tokens),
            cached_tokens: u
                .prompt_tokens_details
                .as_ref()
                .and_then(|d| d.cached_tokens),
        });

        Ok(ChatResponse {
            content: sanitize_assistant_content(message.content.unwrap_or_default()),
            tool_calls,
            usage,
        })
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<async_openai::types::ChatCompletionResponseStream> {
        debug!(
            "Sending streaming chat request to Kimi API (thinking: {})",
            self.config.thinking_enabled
        );

        let messages: Vec<ChatCompletionRequestMessage> =
            normalize_tool_message_sequence(request.messages)
                .into_iter()
                .map(convert_message)
                .collect();

        let mut req = CreateChatCompletionRequest {
            model: request.model.clone(),
            messages,
            temperature: request.temperature,
            max_completion_tokens: request.max_tokens,
            tools: None,
            ..Default::default()
        };

        // 如果有 thinking budget，添加到请求中
        if let Some(budget) = self.config.thinking_budget {
            req.max_completion_tokens = Some(budget);
        }

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
        req.stream_options = Some(ChatCompletionStreamOptions {
            include_usage: true,
        });

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
            let has_tool_calls = tool_calls.as_ref().is_some_and(|calls| !calls.is_empty());
            let content = if has_tool_calls && content.trim().is_empty() {
                None
            } else {
                Some(ChatCompletionRequestAssistantMessageContent::Text(content))
            };
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
    use crate::test_utils::env_guard::EnvVarGuard;

    #[test]
    fn test_kimi_config_from_env() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.set("MOONSHOT_API_KEY", "test-key");
        env.set("MOONSHOT_BASE_URL", "https://test.api/v1");
        env.set("MOONSHOT_MODEL", "kimi-k2.5");

        let config = KimiConfig::from_env().unwrap();
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.base_url, "https://test.api/v1");
        assert_eq!(config.default_model, "kimi-k2.5");
        assert!(config.thinking_enabled); // 默认启用
        assert!(config.thinking_budget.is_none()); // 默认 adaptive
    }

    #[test]
    fn assistant_tool_call_omits_empty_content_for_strict_providers() {
        let message = convert_message(Message::assistant_with_tools(
            "",
            vec![ToolCall {
                id: "call_1".to_string(),
                name: "file_read".to_string(),
                arguments: serde_json::json!({"path": "Cargo.toml"}),
            }],
        ));
        let ChatCompletionRequestMessage::Assistant(assistant) = message else {
            panic!("expected assistant message");
        };

        assert!(assistant.content.is_none());
        assert_eq!(assistant.tool_calls.unwrap().len(), 1);
    }
}
