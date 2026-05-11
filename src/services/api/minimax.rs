//! MiniMax API 客户端（OpenAI 兼容接入）
//!
//! 参考官方文档：Token Plan 支持 OpenAI 兼容调用。

use crate::services::api::retry::ProviderRetryPolicy;
use crate::services::api::{
    sanitize_assistant_content, ChatRequest, ChatResponse, LlmProvider, ToolCall, Usage,
};
use anyhow::{bail, Context, Result};
use async_openai::{config::OpenAIConfig, types::ChatCompletionResponseStream, Client};
use async_trait::async_trait;
use reqwest::StatusCode;
use tracing::{debug, info};

/// MiniMax 客户端
pub struct MiniMaxClient {
    client: Client<OpenAIConfig>,
    model: String,
    base_url: String,
    api_key: String,
}

impl MiniMaxClient {
    /// 使用 MiniMax Token Plan 的 API Key 初始化
    pub fn new(api_key: &str, base_url: Option<&str>, model: Option<&str>) -> Self {
        let mut config = OpenAIConfig::new().with_api_key(api_key);
        if let Some(url) = base_url {
            config = config.with_api_base(url);
        }
        let client = Client::with_config(config);
        // 官方 quickstart 默认示例模型
        let model = model.unwrap_or("MiniMax-M2.7").to_string();
        let base_url = base_url
            .unwrap_or("https://api.minimaxi.com/v1")
            .to_string();
        info!(
            "MiniMax client initialized with base URL: {}, model: {}",
            base_url, model
        );
        Self {
            client,
            model,
            base_url,
            api_key: api_key.to_string(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("MINIMAX_API_KEY").context("MINIMAX_API_KEY must be set")?;
        if api_key.trim().is_empty() {
            bail!("MINIMAX_API_KEY must be set");
        }
        let base_url = std::env::var("MINIMAX_BASE_URL").ok();
        let model = std::env::var("MINIMAX_MODEL").unwrap_or_else(|_| "MiniMax-M2.7".to_string());
        Ok(Self::new(&api_key, base_url.as_deref(), Some(&model)))
    }

    pub fn default_model(&self) -> &str {
        &self.model
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn fetch_error_body(
        &self,
        req: &async_openai::types::CreateChatCompletionRequest,
    ) -> Option<(StatusCode, String)> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let client = reqwest::Client::new();
        let resp = client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(req)
            .send()
            .await
            .ok()?;
        let status = resp.status();
        let body = resp.text().await.ok()?;
        Some((status, body))
    }

    fn normalize_messages_for_minimax(
        messages: Vec<crate::services::api::Message>,
    ) -> Vec<crate::services::api::Message> {
        let mut system_parts: Vec<String> = Vec::new();
        let mut others: Vec<crate::services::api::Message> = Vec::new();

        for msg in messages {
            match msg {
                crate::services::api::Message::System { content } => system_parts.push(content),
                other => others.push(other),
            }
        }

        if system_parts.is_empty() {
            return others;
        }

        let merged_system = crate::services::api::Message::System {
            content: system_parts.join("\n\n"),
        };

        let mut normalized = Vec::with_capacity(others.len() + 1);
        normalized.push(merged_system);
        normalized.extend(others);
        normalized
    }
}

#[derive(serde::Deserialize)]
struct MiniMaxChatResponseBody {
    choices: Vec<MiniMaxChoice>,
    usage: Option<MiniMaxUsage>,
}

#[derive(serde::Deserialize)]
struct MiniMaxChoice {
    message: MiniMaxMessage,
}

#[derive(serde::Deserialize)]
struct MiniMaxMessage {
    content: Option<String>,
    tool_calls: Option<Vec<MiniMaxToolCall>>,
}

#[derive(serde::Deserialize)]
struct MiniMaxToolCall {
    id: String,
    function: MiniMaxFunctionCall,
}

#[derive(serde::Deserialize)]
struct MiniMaxFunctionCall {
    name: String,
    #[serde(default)]
    arguments: String,
}

#[derive(serde::Deserialize)]
struct MiniMaxUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
    prompt_tokens_details: Option<MiniMaxPromptTokenDetails>,
    completion_tokens_details: Option<MiniMaxCompletionTokenDetails>,
}

#[derive(serde::Deserialize)]
struct MiniMaxPromptTokenDetails {
    cached_tokens: Option<u32>,
}

#[derive(serde::Deserialize)]
struct MiniMaxCompletionTokenDetails {
    reasoning_tokens: Option<u32>,
}

fn parse_minimax_chat_response_body(body: &str) -> Result<ChatResponse> {
    let body: MiniMaxChatResponseBody = serde_json::from_str(body)?;
    let choice = body
        .choices
        .into_iter()
        .next()
        .context("No choices in response")?;
    let message = choice.message;
    let tool_calls = message.tool_calls.map(|calls| {
        calls
            .into_iter()
            .map(|call| ToolCall {
                id: call.id,
                name: call.function.name,
                arguments: serde_json::from_str(&call.function.arguments).unwrap_or_else(|e| {
                    tracing::warn!("Failed to parse MiniMax tool arguments: {}", e);
                    serde_json::Value::Null
                }),
            })
            .collect()
    });
    let usage = body.usage.map(|usage| Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        reasoning_tokens: usage
            .completion_tokens_details
            .and_then(|details| details.reasoning_tokens),
        cached_tokens: usage
            .prompt_tokens_details
            .and_then(|details| details.cached_tokens),
    });

    Ok(ChatResponse {
        content: sanitize_assistant_content(message.content.unwrap_or_default()),
        tool_calls,
        usage,
    })
}

#[async_trait]
impl LlmProvider for MiniMaxClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        use super::openai_compat::{convert_request, convert_response};
        let mut request = request;
        request.messages = Self::normalize_messages_for_minimax(request.messages);
        let req = convert_request(request, &self.model);
        let response = match ProviderRetryPolicy::from_env()
            .retry("MiniMax", "chat.completions", || {
                let req = req.clone();
                async move { self.client.chat().create(req).await }
            })
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                if let Some((status, body)) = self.fetch_error_body(&req).await {
                    if status.is_success() {
                        if let Ok(response) = parse_minimax_chat_response_body(&body) {
                            debug!(
                                "Recovered MiniMax chat response with manual parser after client error: {}",
                                e
                            );
                            return Ok(response);
                        }
                    }
                    anyhow::bail!(
                        "Failed to get response from MiniMax API: {} (status {}) body: {}",
                        e,
                        status,
                        body
                    );
                }
                anyhow::bail!(
                    "Failed to get response from MiniMax API: {} (error body unavailable)",
                    e
                );
            }
        };
        convert_response(response)
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatCompletionResponseStream> {
        use super::openai_compat::convert_request;
        let mut request = request;
        request.messages = Self::normalize_messages_for_minimax(request.messages);
        let mut req = convert_request(request, &self.model);
        req.stream = Some(true);
        // MiniMax's OpenAI-compatible streaming usage chunks can omit
        // prompt_tokens/completion_tokens and include MiniMax-specific fields
        // such as total_characters. async-openai treats that as a hard
        // deserialization error, interrupting otherwise valid tool streams.
        // Do not request stream usage for MiniMax; non-streaming fallback still
        // records usage when needed.
        req.stream_options = None;
        match ProviderRetryPolicy::from_env()
            .retry("MiniMax", "chat.completions.stream", || {
                let req = req.clone();
                async move { self.client.chat().create_stream(req).await }
            })
            .await
        {
            Ok(stream) => Ok(stream),
            Err(e) => {
                if let Some((status, body)) = self.fetch_error_body(&req).await {
                    anyhow::bail!(
                        "Failed to create streaming response from MiniMax API: {} (status {}) body: {}",
                        e,
                        status,
                        body
                    );
                }
                anyhow::bail!(
                    "Failed to create streaming response from MiniMax API: {} (error body unavailable)",
                    e
                );
            }
        }
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn default_model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::retry::is_retryable_provider_error;

    #[test]
    fn test_minimax_client_defaults() {
        let client = MiniMaxClient::new("test-key", None, None);
        assert_eq!(client.default_model(), "MiniMax-M2.7");
        assert_eq!(client.base_url(), "https://api.minimaxi.com/v1");
    }

    #[test]
    fn parses_success_body_after_client_content_error() {
        let body = r#"{
          "choices": [
            {
              "finish_reason": "stop",
              "index": 0,
              "message": {
                "content": "<think>hidden</think>\n\n{\"task_type\":\"feature\"}",
                "role": "assistant"
              }
            }
          ],
          "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15,
            "prompt_tokens_details": {"cached_tokens": 4}
          },
          "base_resp": {"status_code": 0, "status_msg": "success"}
        }"#;

        let response = parse_minimax_chat_response_body(body).unwrap();

        assert_eq!(response.content, "{\"task_type\":\"feature\"}");
        assert!(response.tool_calls.is_none());
        assert_eq!(response.usage.unwrap().cached_tokens, Some(4));
    }

    #[test]
    fn parses_success_body_with_tool_calls_after_client_error() {
        let body = r#"{
          "choices": [
            {
              "finish_reason": "tool_calls",
              "index": 0,
              "message": {
                "content": "<think>hidden</think>\n\n",
                "role": "assistant",
                "tool_calls": [
                  {
                    "id": "call_1",
                    "type": "function",
                    "function": {
                      "name": "file_read",
                      "arguments": "{\"path\":\"scripts/run_live_eval.sh\"}"
                    }
                  }
                ]
              }
            }
          ],
          "usage": {
            "prompt_tokens": 20,
            "completion_tokens": 6,
            "total_tokens": 26,
            "completion_tokens_details": {"reasoning_tokens": 2}
          }
        }"#;

        let response = parse_minimax_chat_response_body(body).unwrap();
        let calls = response.tool_calls.unwrap();

        assert!(response.content.trim().is_empty());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "file_read");
        assert_eq!(
            calls[0].arguments,
            serde_json::json!({"path": "scripts/run_live_eval.sh"})
        );
        assert_eq!(response.usage.unwrap().reasoning_tokens, Some(2));
    }

    #[test]
    fn retries_only_transient_transport_errors() {
        assert!(is_retryable_provider_error("error sending request for url"));
        assert!(is_retryable_provider_error(
            "OpenSSL SSL_read: unexpected eof while reading"
        ));
        assert!(is_retryable_provider_error("operation timed out"));
        assert!(!is_retryable_provider_error(
            "bad_request_error: invalid params"
        ));
        assert!(!is_retryable_provider_error("401 unauthorized"));
    }
}
