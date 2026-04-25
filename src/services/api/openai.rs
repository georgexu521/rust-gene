//! OpenAI API 客户端

use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
use anyhow::{Context, Result};
use async_openai::{config::OpenAIConfig, types::ChatCompletionResponseStream, Client};
use async_trait::async_trait;
use tracing::info;

/// OpenAI 客户端
pub struct OpenAiClient {
    client: Client<OpenAIConfig>,
    model: String,
    base_url: String,
}

impl OpenAiClient {
    pub fn new(api_key: &str, base_url: Option<&str>, model: Option<&str>) -> Self {
        let mut config = OpenAIConfig::new().with_api_key(api_key);
        if let Some(url) = base_url {
            config = config.with_api_base(url);
        }
        let client = Client::with_config(config);
        let model = model.unwrap_or("gpt-4o").to_string();
        let base_url = base_url.unwrap_or("https://api.openai.com/v1").to_string();
        info!(
            "OpenAI client initialized with base URL: {}, model: {}",
            base_url, model
        );
        Self {
            client,
            model,
            base_url,
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").context("OPENAI_API_KEY must be set")?;
        let base_url = std::env::var("OPENAI_BASE_URL").ok();
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
        Ok(Self::new(&api_key, base_url.as_deref(), Some(&model)))
    }

    pub fn default_model(&self) -> &str {
        &self.model
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait]
impl LlmProvider for OpenAiClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        use super::openai_compat::{convert_request, convert_response};
        let req = convert_request(request, &self.model);
        let response = self
            .client
            .chat()
            .create(req)
            .await
            .context("Failed to get response from OpenAI API")?;
        convert_response(response)
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatCompletionResponseStream> {
        use super::openai_compat::convert_request;
        use async_openai::types::ChatCompletionStreamOptions;
        let mut req = convert_request(request, &self.model);
        req.stream = Some(true);
        req.stream_options = Some(ChatCompletionStreamOptions {
            include_usage: true,
        });
        self.client
            .chat()
            .create_stream(req)
            .await
            .context("Failed to create streaming response from OpenAI API")
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn default_model(&self) -> &str {
        &self.model
    }
}
