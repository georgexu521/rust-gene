//! MiniMax API 客户端（OpenAI 兼容接入）
//!
//! 参考官方文档：Token Plan 支持 OpenAI 兼容调用。

use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
use anyhow::{Context, Result};
use async_openai::{config::OpenAIConfig, types::ChatCompletionResponseStream, Client};
use async_trait::async_trait;
use reqwest::StatusCode;
use tracing::info;

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
        let url = format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/')
        );
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

#[async_trait]
impl LlmProvider for MiniMaxClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        use super::openai_compat::{convert_request, convert_response};
        let mut request = request;
        request.messages = Self::normalize_messages_for_minimax(request.messages);
        let req = convert_request(request, &self.model);
        let response = match self.client.chat().create(req.clone()).await {
            Ok(resp) => resp,
            Err(e) => {
                if let Some((status, body)) = self.fetch_error_body(&req).await {
                    anyhow::bail!(
                        "Failed to get response from MiniMax API: {} (status {}) body: {}",
                        e,
                        status,
                        body
                    );
                }
                return Err(e).context("Failed to get response from MiniMax API");
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
        match self.client.chat().create_stream(req.clone()).await {
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
                Err(e).context("Failed to create streaming response from MiniMax API")
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

    #[test]
    fn test_minimax_client_defaults() {
        let client = MiniMaxClient::new("test-key", None, None);
        assert_eq!(client.default_model(), "MiniMax-M2.7");
        assert_eq!(client.base_url(), "https://api.minimaxi.com/v1");
    }
}
