//! HTTP API 服务器
//!
//! REST + WebSocket API，让外部程序调用 Priority Agent。
//! 类似 Claude Code 的 Node SDK 和 Hermes 的 HTTP API。
//!
//! ## API 端点
//!
//! ### Agent Prompt / Provider Chat
//! - `POST /api/sessions/:id/prompt` - 正式 full-agent 用户任务入口
//! - `POST /api/provider-chat` - 显式 provider-chat 非 agent 辅助入口
//! - `POST /api/chat` - legacy provider-chat 兼容入口
//! - `POST /api/chat/stream` - legacy SSE 入口（尚未实现）
//!
//! ### Sessions
//! - `GET /api/sessions` - 列出会话
//! - `POST /api/sessions` - 创建会话
//! - `GET /api/sessions/:id` - 获取会话
//! - `PUT /api/sessions/:id` - 更新会话
//! - `DELETE /api/sessions/:id` - 删除会话
//! - `GET /api/sessions/:id/messages` - 获取会话消息
//!
//! ### Tools
//! - `GET /api/tools` - 列出可用工具
//! - `GET /api/tools/:name` - 获取工具详情
//! - `POST /api/tools/call` - 调用工具
//!
//! ### Config
//! - `GET /api/config` - 获取配置
//! - `PUT /api/config` - 更新配置
//!
//! ### Stats & Health
//! - `GET /api/stats` - 获取统计
//! - `GET /api/workflow/metrics/weekly?limit=8` - 获取 workflow 每周汇总
//! - `GET /api/workflow/metrics/calibration/weekly?limit=8` - 获取 workflow 每周人工校准偏差
//! - `GET /api/audit/summary` - 获取审计概览
//! - `GET /api/audit/recent?limit=50` - 获取最近工具审计事件
//! - `POST /api/audit/export` - 导出审计快照
//! - `GET /api/health` - 健康检查
//! - `GET /api/version` - 版本信息
//!
//! ### WebSocket
//! - `WS /api/ws` - WebSocket 双向通信
//!
//! ### Bridge v1 (Remote Session Control Plane)
//! - `GET /v1/sessions` - 列出租户会话（按 `X-Tenant-Id` 隔离）
//! - `POST /v1/sessions` - 创建远程会话
//! - `GET /v1/sessions/:id` - 获取远程会话
//! - `GET /v1/sessions/:id/status` - 获取会话同步状态
//! - `GET /v1/sessions/:id/messages` - 拉取会话消息（支持 `since_id`）
//! - `POST /v1/triggers/:id/run` - 在远程会话中运行触发
//! - 可选鉴权：
//!   - 单 token: `PRIORITY_AGENT_BRIDGE_TOKEN` / `BRIDGE_TOKEN`
//!   - 多 token 轮换: `PRIORITY_AGENT_BRIDGE_TOKENS`（逗号/分号/空格分隔）

pub mod dto;
pub mod provider_status;
pub mod routes;
pub mod session_runner;
pub mod state;
pub mod websocket;

pub use state::{ApiError, ApiState, MessageInfo};
pub use websocket::ws_handler;

use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

/// 创建 API 路由
pub fn create_router(state: Arc<ApiState>) -> axum::Router {
    routes::create_routes(state).layer(CorsLayer::permissive())
}

/// 启动 API 服务器
pub async fn start_server(
    provider: Arc<dyn crate::services::api::LlmProvider>,
    model: String,
    tool_registry: Arc<crate::tools::ToolRegistry>,
    port: u16,
    lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    runtime_controller: Option<crate::engine::runtime_controller::RuntimeController>,
) -> anyhow::Result<()> {
    let mut api_state = ApiState::new(
        provider,
        model.clone(),
        tool_registry,
        lsp_manager,
        worktree_manager,
    )?;
    let agent_runtime = runtime_controller.map(|controller| {
        Arc::new(state::RuntimeControllerApiAgentRuntime::new(
            controller,
            model,
            api_state.session_store.clone(),
            api_state.runner_registry.clone(),
        )) as Arc<dyn state::ApiAgentRuntime>
    });
    api_state.agent_runtime = agent_runtime;
    let state = Arc::new(api_state);

    let app = create_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    info!("API server listening on http://{}", addr);
    info!("API endpoints:");
    info!("  POST /api/sessions/:id/prompt  - Full-agent turn (primary)");
    info!("  POST /api/chat                 - Provider chat (legacy)");
    info!("  POST /api/provider-chat        - Provider chat");
    info!("  GET  /api/sessions            - List sessions");
    info!("  GET  /api/sessions/:id/parts  - Session parts cursor");
    info!("  GET  /api/provider/status     - Provider status");
    info!("  GET  /api/health              - Health check");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ── 平台适配器框架 ──────────────────────────────────

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 平台类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Platform {
    Telegram,
    Discord,
    Slack,
    Weixin,
    Cli,
    Api,
    Custom(String),
}

/// 入站消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// 来源平台
    pub platform: Platform,
    /// 会话/频道 ID
    pub chat_id: String,
    /// 用户 ID
    pub user_id: String,
    /// 消息内容
    pub content: String,
    /// 消息类型
    pub message_type: MessageType,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// 消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Text,
    Image,
    File,
    Command(String),
}

/// 出站消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// 目标平台
    pub platform: Platform,
    /// 目标会话/频道
    pub chat_id: String,
    /// 回复内容
    pub content: String,
    /// 是否流式（部分平台支持）
    pub streaming: bool,
}

/// 平台适配器 trait
#[async_trait::async_trait]
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
}

/// 消息处理器 trait — 由核心 agent 实现
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync {
    /// 处理入站消息，返回出站消息
    async fn process(&self, message: &InboundMessage) -> anyhow::Result<OutboundMessage>;
}

/// 平台管理器
pub struct PlatformManager {
    adapters: HashMap<Platform, Box<dyn PlatformAdapter>>,
}

impl PlatformManager {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// 注册平台适配器
    pub fn register(&mut self, adapter: Box<dyn PlatformAdapter>) {
        let platform = adapter.platform();
        info!("Registered platform adapter: {:?}", platform);
        self.adapters.insert(platform, adapter);
    }

    /// 获取适配器
    pub fn get(&self, platform: &Platform) -> Option<&dyn PlatformAdapter> {
        self.adapters.get(platform).map(|a| a.as_ref())
    }

    /// 列出已注册的平台
    pub fn platforms(&self) -> Vec<&Platform> {
        self.adapters.keys().collect()
    }
}

impl Default for PlatformManager {
    fn default() -> Self {
        Self::new()
    }
}

/// CLI 平台适配器
pub struct CliAdapter;

#[async_trait::async_trait]
impl PlatformAdapter for CliAdapter {
    fn platform(&self) -> Platform {
        Platform::Cli
    }

    async fn start_listening(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn send_message(&self, message: &OutboundMessage) -> anyhow::Result<()> {
        println!("{}", message.content);
        Ok(())
    }

    async fn handle_inbound(
        &self,
        _message: &InboundMessage,
        _handler: &dyn MessageHandler,
    ) -> anyhow::Result<()> {
        Ok(())
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

#[async_trait::async_trait]
impl PlatformAdapter for ApiAdapter {
    fn platform(&self) -> Platform {
        Platform::Api
    }

    async fn start_listening(&self) -> anyhow::Result<()> {
        info!("API adapter ready on port {}", self.port);
        Ok(())
    }

    async fn send_message(&self, _message: &OutboundMessage) -> anyhow::Result<()> {
        Ok(())
    }

    async fn handle_inbound(
        &self,
        _message: &InboundMessage,
        _handler: &dyn MessageHandler,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_serialization() {
        let p = Platform::Telegram;
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, "\"Telegram\"");
    }

    #[test]
    fn test_inbound_message() {
        let msg = InboundMessage {
            platform: Platform::Cli,
            chat_id: "test".to_string(),
            user_id: "user1".to_string(),
            content: "Hello".to_string(),
            message_type: MessageType::Text,
            metadata: HashMap::new(),
        };
        assert_eq!(msg.platform, Platform::Cli);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_platform_manager() {
        let mut manager = PlatformManager::new();
        manager.register(Box::new(CliAdapter));
        assert!(manager.get(&Platform::Cli).is_some());
        assert!(manager.get(&Platform::Telegram).is_none());
    }
}
