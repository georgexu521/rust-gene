//! Telegram 平台适配器
//!
//! 集成 Telegram Bot API，支持接收和发送消息

use super::{AdapterStatus, MessageHandler, PlatformAdapter, PlatformConfig};
use crate::api::{InboundMessage, MessageType, OutboundMessage, Platform};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Telegram 适配器
pub struct TelegramAdapter {
    config: PlatformConfig,
    client: reqwest::Client,
    offset: Arc<RwLock<i64>>,
    status: Arc<RwLock<AdapterStatus>>,
}

impl TelegramAdapter {
    /// 创建新的 Telegram 适配器
    pub fn new(token: impl Into<String>) -> Self {
        let mut extra = HashMap::new();
        extra.insert(
            "api_url".to_string(),
            "https://api.telegram.org".to_string(),
        );

        let config = PlatformConfig {
            platform: Platform::Telegram,
            token: token.into(),
            webhook_url: None,
            extra,
        };

        Self {
            config,
            client: reqwest::Client::new(),
            offset: Arc::new(RwLock::new(0)),
            status: Arc::new(RwLock::new(AdapterStatus::Disconnected)),
        }
    }

    /// 使用自定义配置创建
    pub fn with_config(config: PlatformConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            offset: Arc::new(RwLock::new(0)),
            status: Arc::new(RwLock::new(AdapterStatus::Disconnected)),
        }
    }

    /// 获取 Bot API URL
    fn api_url(&self, method: &str) -> String {
        let base = self
            .config
            .extra
            .get("api_url")
            .cloned()
            .unwrap_or_else(|| "https://api.telegram.org".to_string());
        format!("{}/bot{}/{}", base, self.config.token, method)
    }

    /// 获取 Bot 信息
    pub async fn get_me(&self) -> anyhow::Result<TelegramUser> {
        let url = self.api_url("getMe");
        let response: TelegramResponse<TelegramUser> =
            self.client.get(&url).send().await?.json().await?;

        if response.ok {
            response
                .result
                .ok_or_else(|| anyhow::anyhow!("Empty result"))
        } else {
            Err(anyhow::anyhow!(
                "Telegram API error: {:?}",
                response.description
            ))
        }
    }

    /// 发送文本消息
    async fn send_text_message(
        &self,
        chat_id: &str,
        text: &str,
    ) -> anyhow::Result<TelegramMessage> {
        let url = self.api_url("sendMessage");

        let params = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown",
        });

        let response: TelegramResponse<TelegramMessage> = self
            .client
            .post(&url)
            .json(&params)
            .send()
            .await?
            .json()
            .await?;

        if response.ok {
            response
                .result
                .ok_or_else(|| anyhow::anyhow!("Empty result"))
        } else {
            Err(anyhow::anyhow!(
                "Failed to send message: {:?}",
                response.description
            ))
        }
    }

    /// 获取更新（轮询模式）
    async fn get_updates(&self) -> anyhow::Result<Vec<TelegramUpdate>> {
        let url = self.api_url("getUpdates");
        let offset = *self.offset.read().await;

        let params = serde_json::json!({
            "offset": offset,
            "limit": 100,
            "timeout": 30,
        });

        let response: TelegramResponse<Vec<TelegramUpdate>> = self
            .client
            .post(&url)
            .json(&params)
            .send()
            .await?
            .json()
            .await?;

        if response.ok {
            Ok(response.result.unwrap_or_default())
        } else {
            Err(anyhow::anyhow!(
                "Failed to get updates: {:?}",
                response.description
            ))
        }
    }

    /// 处理更新
    async fn process_updates(&self, updates: Vec<TelegramUpdate>, handler: &dyn MessageHandler) {
        for update in updates {
            // 更新 offset
            if let Some(update_id) = update.update_id {
                let mut offset = self.offset.write().await;
                *offset = update_id + 1;
            }

            // 处理消息
            if let Some(message) = update.message {
                if let Some(text) = message.text {
                    let chat_id = message.chat.id.to_string();
                    let user_id = message
                        .from
                        .as_ref()
                        .map(|u| u.id.to_string())
                        .unwrap_or_default();

                    let inbound = InboundMessage {
                        platform: Platform::Telegram,
                        chat_id: chat_id.clone(),
                        user_id,
                        content: text,
                        message_type: MessageType::Text,
                        metadata: {
                            let mut map = HashMap::new();
                            map.insert("message_id".to_string(), message.message_id.to_string());
                            map
                        },
                    };

                    // 调用处理器
                    match handler.process(&inbound).await {
                        Ok(outbound) => {
                            if let Err(e) = self.send_message(&outbound).await {
                                error!("Failed to send response: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Message handler error: {}", e);
                            // 发送错误提示
                            let error_msg = OutboundMessage {
                                platform: Platform::Telegram,
                                chat_id,
                                content: "Sorry, I encountered an error processing your message."
                                    .to_string(),
                                streaming: false,
                            };
                            let _ = self.send_message(&error_msg).await;
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl PlatformAdapter for TelegramAdapter {
    fn platform(&self) -> Platform {
        Platform::Telegram
    }

    async fn start_listening(&self) -> anyhow::Result<()> {
        // 验证 Bot Token
        match self.get_me().await {
            Ok(bot_info) => {
                info!(
                    "Telegram Bot connected: @{} ({}",
                    bot_info.username.as_deref().unwrap_or("unknown"),
                    bot_info.first_name
                );
                *self.status.write().await = AdapterStatus::Connected;
            }
            Err(e) => {
                error!("Failed to connect to Telegram: {}", e);
                *self.status.write().await = AdapterStatus::Error;
                return Err(e);
            }
        }

        // 轮询循环
        info!("Starting Telegram polling loop...");
        loop {
            match self.get_updates().await {
                Ok(updates) => {
                    if !updates.is_empty() {
                        debug!("Received {} updates from Telegram", updates.len());
                    }
                }
                Err(e) => {
                    error!("Telegram polling error: {}", e);
                    *self.status.write().await = AdapterStatus::Error;
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    async fn send_message(&self, message: &OutboundMessage) -> anyhow::Result<()> {
        self.send_text_message(&message.chat_id, &message.content)
            .await?;
        Ok(())
    }

    async fn handle_inbound(
        &self,
        message: &InboundMessage,
        handler: &dyn MessageHandler,
    ) -> anyhow::Result<()> {
        let outbound = handler.process(message).await?;
        self.send_message(&outbound).await
    }

    fn status(&self) -> AdapterStatus {
        self.status
            .try_read()
            .map(|s| *s)
            .unwrap_or(AdapterStatus::Unknown)
    }
}

// ── Telegram API Types ─────────────────────────────────

/// Telegram API 响应
#[derive(Debug, Deserialize)]
struct TelegramResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

/// Telegram 用户
#[derive(Debug, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

/// Telegram 聊天
#[derive(Debug, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    pub type_: String,
    #[serde(rename = "type")]
    pub chat_type: String,
}

/// Telegram 消息
#[derive(Debug, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub from: Option<TelegramUser>,
    pub chat: TelegramChat,
    pub date: i64,
    pub text: Option<String>,
}

/// Telegram 更新
#[derive(Debug, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: Option<i64>,
    pub message: Option<TelegramMessage>,
    pub edited_message: Option<TelegramMessage>,
}

/// Telegram 发送消息请求
#[derive(Debug, Serialize)]
pub struct SendMessageRequest {
    pub chat_id: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_adapter_creation() {
        let adapter = TelegramAdapter::new("test_token");
        assert!(matches!(adapter.platform(), Platform::Telegram));
    }

    #[test]
    fn test_api_url_generation() {
        let adapter = TelegramAdapter::new("123456:abc");
        let url = adapter.api_url("getMe");
        assert_eq!(url, "https://api.telegram.org/bot123456:abc/getMe");
    }

    #[test]
    fn test_telegram_response_parsing() {
        let json = r#"{
            "ok": true,
            "result": {
                "id": 123456,
                "is_bot": true,
                "first_name": "Test Bot",
                "username": "test_bot"
            }
        }"#;

        let response: TelegramResponse<TelegramUser> = serde_json::from_str(json).unwrap();
        assert!(response.ok);
        assert!(response.result.is_some());
        assert_eq!(response.result.unwrap().first_name, "Test Bot");
    }
}
