//! 平台适配器框架
//!
//! 支持多平台集成：Telegram, Discord, Slack, 微信等

pub mod telegram;

use crate::api::{InboundMessage, OutboundMessage, Platform};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// 平台适配器 trait
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    /// 平台类型
    fn platform(&self) -> Platform;

    /// 启动监听（阻塞式，接收消息）
    async fn start_listening(&self) -> anyhow::Result<()>;

    /// 发送消息
    async fn send_message(&self, message: &OutboundMessage) -> anyhow::Result<()>;

    /// 处理入站消息（由监听器调用）
    async fn handle_inbound(
        &self,
        message: &InboundMessage,
        handler: &dyn MessageHandler,
    ) -> anyhow::Result<()>;

    /// 获取适配器状态
    fn status(&self) -> AdapterStatus {
        AdapterStatus::Unknown
    }
}

/// 消息处理器 trait
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// 处理入站消息，返回出站消息
    async fn process(&self, message: &InboundMessage) -> anyhow::Result<OutboundMessage>;
}

/// 适配器状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdapterStatus {
    Connected,
    Disconnected,
    Connecting,
    Error,
    #[default]
    Unknown,
}

impl std::fmt::Display for AdapterStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterStatus::Connected => write!(f, "connected"),
            AdapterStatus::Disconnected => write!(f, "disconnected"),
            AdapterStatus::Connecting => write!(f, "connecting"),
            AdapterStatus::Error => write!(f, "error"),
            AdapterStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// 平台管理器
pub struct PlatformManager {
    adapters: HashMap<Platform, Arc<dyn PlatformAdapter>>,
    statuses: Arc<RwLock<HashMap<Platform, AdapterStatus>>>,
}

impl PlatformManager {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
            statuses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册平台适配器
    pub fn register(&mut self, adapter: Arc<dyn PlatformAdapter>) {
        let platform = adapter.platform();
        info!("Registering platform adapter: {:?}", platform);
        self.adapters.insert(platform.clone(), adapter);
    }

    /// 获取适配器
    pub fn get(&self, platform: &Platform) -> Option<Arc<dyn PlatformAdapter>> {
        self.adapters.get(platform).cloned()
    }

    /// 列出已注册的平台
    pub fn platforms(&self) -> Vec<&Platform> {
        self.adapters.keys().collect()
    }

    /// 启动所有适配器
    pub async fn start_all(&self, handler: Arc<dyn MessageHandler>) -> anyhow::Result<()> {
        for (platform, adapter) in &self.adapters {
            info!("Starting adapter for {:?}", platform);

            let adapter = adapter.clone();
            let _handler = handler.clone();
            let statuses = self.statuses.clone();
            let platform = platform.clone();

            tokio::spawn(async move {
                // 更新状态为连接中
                {
                    let mut s: tokio::sync::RwLockWriteGuard<'_, HashMap<Platform, AdapterStatus>> =
                        statuses.write().await;
                    s.insert(platform.clone(), AdapterStatus::Connecting);
                }

                // 启动监听循环
                loop {
                    match adapter.start_listening().await {
                        Ok(_) => {
                            info!("Adapter for {:?} stopped gracefully", platform);
                            let mut s: tokio::sync::RwLockWriteGuard<
                                '_,
                                HashMap<Platform, AdapterStatus>,
                            > = statuses.write().await;
                            s.insert(platform.clone(), AdapterStatus::Disconnected);
                            break;
                        }
                        Err(e) => {
                            error!("Adapter for {:?} error: {}. Reconnecting...", platform, e);
                            let mut s: tokio::sync::RwLockWriteGuard<
                                '_,
                                HashMap<Platform, AdapterStatus>,
                            > = statuses.write().await;
                            s.insert(platform.clone(), AdapterStatus::Error);

                            // 等待后重连
                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        }
                    }
                }
            });
        }

        Ok(())
    }

    /// 获取适配器状态
    pub async fn get_status(&self, platform: &Platform) -> AdapterStatus {
        let statuses: tokio::sync::RwLockReadGuard<'_, HashMap<Platform, AdapterStatus>> =
            self.statuses.read().await;
        statuses
            .get(platform)
            .copied()
            .unwrap_or(AdapterStatus::Unknown)
    }

    /// 获取所有状态
    pub async fn get_all_statuses(&self) -> HashMap<Platform, AdapterStatus> {
        let statuses: tokio::sync::RwLockReadGuard<'_, HashMap<Platform, AdapterStatus>> =
            self.statuses.read().await;
        statuses.clone()
    }

    /// 发送消息到指定平台
    pub async fn send(&self, platform: &Platform, message: &OutboundMessage) -> anyhow::Result<()> {
        if let Some(adapter) = self.get(platform) {
            adapter.send_message(message).await
        } else {
            Err(anyhow::anyhow!("Platform {:?} not registered", platform))
        }
    }
}

impl Default for PlatformManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 平台配置
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    /// 平台类型
    pub platform: Platform,
    /// API Token
    pub token: String,
    /// Webhook URL (可选)
    pub webhook_url: Option<String>,
    /// 其他配置参数
    pub extra: HashMap<String, String>,
}

/// CLI 平台适配器
pub struct CliAdapter;

#[async_trait]
impl PlatformAdapter for CliAdapter {
    fn platform(&self) -> Platform {
        Platform::Cli
    }

    async fn start_listening(&self) -> anyhow::Result<()> {
        // CLI 使用现有的 TUI 循环，不需要额外监听
        Ok(())
    }

    async fn send_message(&self, message: &OutboundMessage) -> anyhow::Result<()> {
        println!("[{}] {}", message.chat_id, message.content);
        Ok(())
    }

    async fn handle_inbound(
        &self,
        _message: &InboundMessage,
        _handler: &dyn MessageHandler,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn status(&self) -> AdapterStatus {
        AdapterStatus::Connected
    }
}

/// API 平台适配器
pub struct ApiAdapter {
    port: u16,
}

impl ApiAdapter {
    pub fn new(port: u16) -> Self {
        Self { port }
    }
}

#[async_trait]
impl PlatformAdapter for ApiAdapter {
    fn platform(&self) -> Platform {
        Platform::Api
    }

    async fn start_listening(&self) -> anyhow::Result<()> {
        info!("API adapter ready on port {}", self.port);
        // API 服务器通过 axum 启动，这里只是占位
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }

    async fn send_message(&self, _message: &OutboundMessage) -> anyhow::Result<()> {
        // API 模式下，消息通过 HTTP 响应返回
        Ok(())
    }

    async fn handle_inbound(
        &self,
        _message: &InboundMessage,
        _handler: &dyn MessageHandler,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn status(&self) -> AdapterStatus {
        AdapterStatus::Connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_manager() {
        let mut manager = PlatformManager::new();
        manager.register(Arc::new(CliAdapter));

        assert!(manager.get(&Platform::Cli).is_some());
        assert!(manager.get(&Platform::Telegram).is_none());
    }

    #[test]
    fn test_adapter_status() {
        let status = AdapterStatus::Connected;
        assert_eq!(status.to_string(), "connected");
    }
}
