//! OpenAI API 客户端

use crate::services::api::retry::ProviderRetryPolicy;
use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
use anyhow::{bail, Context, Result};
use async_openai::{config::OpenAIConfig, types::ChatCompletionResponseStream, Client};
use async_trait::async_trait;
use tracing::info;

/// OpenAI 客户端
pub struct OpenAiClient {
    client: Client<OpenAIConfig>,
    model: String,
    base_url: String,
    provider_label: String,
}

impl OpenAiClient {
    pub fn new(api_key: &str, base_url: Option<&str>, model: Option<&str>) -> Self {
        Self::new_with_label("OpenAI", api_key, base_url, model)
    }

    pub fn new_with_label(
        provider_label: &str,
        api_key: &str,
        base_url: Option<&str>,
        model: Option<&str>,
    ) -> Self {
        let mut config = OpenAIConfig::new().with_api_key(api_key);
        if let Some(url) = base_url {
            config = config.with_api_base(url);
        }
        let client = Client::with_config(config);
        let model = model.unwrap_or("gpt-4o").to_string();
        let base_url = base_url.unwrap_or("https://api.openai.com/v1").to_string();
        info!(
            "{} client initialized with base URL: {}, model: {}",
            provider_label, base_url, model
        );
        Self {
            client,
            model,
            base_url,
            provider_label: provider_label.to_string(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").context("OPENAI_API_KEY must be set")?;
        let api_key = api_key.trim().to_string();
        if api_key.is_empty() {
            bail!("OPENAI_API_KEY must be set");
        }
        let base_url = std::env::var("OPENAI_BASE_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let model = std::env::var("OPENAI_MODEL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "gpt-4o".to_string());
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
        use super::openai_compat::{
            convert_request_for_capabilities, convert_response_for_capabilities,
        };
        use super::provider_protocol::ProviderCapabilities;
        let capabilities = ProviderCapabilities::detect(&self.base_url, &self.model);
        let req = convert_request_for_capabilities(request, &self.model, capabilities);
        let response = ProviderRetryPolicy::from_env()
            .retry(self.provider_label.as_str(), "chat.completions", || {
                let req = req.clone();
                async move { self.client.chat().create(req).await }
            })
            .await
            .with_context(|| format!("Failed to get response from {} API", self.provider_label))?;
        convert_response_for_capabilities(response, capabilities)
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatCompletionResponseStream> {
        use super::openai_compat::convert_request_for_capabilities;
        use super::provider_protocol::ProviderCapabilities;
        use async_openai::types::ChatCompletionStreamOptions;
        let capabilities = ProviderCapabilities::detect(&self.base_url, &self.model);
        let mut req = convert_request_for_capabilities(request, &self.model, capabilities);
        req.stream = Some(true);
        req.stream_options = Some(ChatCompletionStreamOptions {
            include_usage: true,
        });
        ProviderRetryPolicy::from_env()
            .retry(
                self.provider_label.as_str(),
                "chat.completions.stream",
                || {
                    let req = req.clone();
                    async move { self.client.chat().create_stream(req).await }
                },
            )
            .await
            .with_context(|| {
                format!(
                    "Failed to create streaming response from {} API",
                    self.provider_label
                )
            })
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn default_model(&self) -> &str {
        &self.model
    }
}
