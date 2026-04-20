//! 流式查询引擎
//!
//! 提供与 Claude Code 类似的流式响应体验

use crate::services::api::{LlmProvider, Message};
use crate::tools::ToolRegistry;
use anyhow::Result;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tracing::warn;
use tokio::sync::mpsc;

/// 流式查询事件
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 开始处理
    Start,
    /// 文本块（增量内容）
    TextChunk(String),
    /// 工具调用开始
    ToolCallStart { id: String, name: String },
    /// 工具调用参数（增量）
    ToolCallArgs { id: String, args_delta: String },
    /// 工具调用完成
    ToolCallComplete { id: String },
    /// 工具执行开始
    ToolExecutionStart { id: String, name: String },
    /// 工具执行进度
    ToolExecutionProgress { id: String, progress: String },
    /// 工具执行完成
    ToolExecutionComplete { id: String, result: String },
    /// 思考内容（如果模型支持）
    Thinking(String),
    /// 使用量统计
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
    },
    /// 完成
    Complete,
    /// 输出被截断（达到 max_tokens 限制）
    OutputTruncated,
    /// 错误
    Error(String),
    /// 工具执行需要用户授权
    PermissionRequest {
        id: String,
        tool_name: String,
        arguments: serde_json::Value,
        prompt: String,
    },
}

/// 流式查询引擎
pub struct StreamingQueryEngine {
    /// LLM 提供商
    provider: Arc<dyn LlmProvider>,
    /// 工具注册表
    tool_registry: Arc<ToolRegistry>,
    /// 模型名称
    model: String,
    /// 系统提示词
    system_prompt: String,
    /// 最大工具调用迭代次数
    max_iterations: usize,
    /// Agent 管理器（可选，用于子 Agent 创建）
    agent_manager: Option<Arc<crate::agent::AgentManager>>,
    /// 任务管理器（可选，用于 task_tool 等）
    task_manager: Option<Arc<crate::task_manager::TaskManager>>,
    /// MCP 管理器（可选，用于调用外部 MCP 工具）
    mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    /// LSP 管理器（可选，用于 lsp_tool 等）
    lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器（可选，用于 worktree_tool 等）
    worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    /// 记忆管理器（可选，用于预取和同步）
    memory_manager: Option<Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>>,
    /// 对话历史（多轮对话支持）
    conversation_history: Arc<tokio::sync::Mutex<Vec<Message>>>,
    /// 上下文压缩器
    compressor: Arc<tokio::sync::Mutex<crate::engine::context_compressor::ContextCompressor>>,
    /// 会话存储（可选）
    session_store: Option<Arc<crate::session_store::SessionStore>>,
    /// 当前会话 ID
    session_id: Option<String>,
    /// 成本追踪器
    cost_tracker: Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>,
    /// 当前权限模式（可在运行时通过 TUI 命令切换）
    permission_mode: Arc<std::sync::RwLock<crate::permissions::PermissionMode>>,
    /// 是否启用 LLM 驱动的记忆提取
    llm_memory_extraction: bool,
    /// 工具授权通道（用于交互式 MCP 授权）
    approval_channel: Option<Arc<crate::engine::conversation_loop::ToolApprovalChannel>>,
    /// Fallback 模型名称（当主模型失败时使用）
    fallback_model: Option<String>,
}

impl StreamingQueryEngine {
    /// 创建新的流式查询引擎
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        model: impl Into<String>,
    ) -> Self {
        let provider_clone = provider.clone();
        Self {
            provider,
            tool_registry,
            model: model.into(),
            system_prompt: super::default_system_prompt(),
            max_iterations: 10,
            agent_manager: None,
            task_manager: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            memory_manager: None,
            conversation_history: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            compressor: Arc::new(tokio::sync::Mutex::new(
                crate::engine::context_compressor::ContextCompressor::new(128_000)
                    .with_llm_provider(provider_clone, ""),
            )),
            session_store: None,
            session_id: None,
            cost_tracker: Arc::new(tokio::sync::Mutex::new(
                crate::cost_tracker::CostTracker::new(),
            )),
            permission_mode: Arc::new(std::sync::RwLock::new(
                crate::permissions::PermissionMode::AutoLowRisk,
            )),
            llm_memory_extraction: false,
            approval_channel: None,
            fallback_model: std::env::var("PRIORITY_AGENT_FALLBACK_MODEL").ok(),
        }
    }

    /// 设置任务管理器
    pub fn with_task_manager(mut self, manager: Arc<crate::task_manager::TaskManager>) -> Self {
        self.task_manager = Some(manager);
        self
    }

    /// 获取成本追踪器的引用
    pub fn cost_tracker(&self) -> &Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>> {
        &self.cost_tracker
    }

    /// 设置会话存储
    pub fn with_session_store(
        mut self,
        store: Arc<crate::session_store::SessionStore>,
        session_id: String,
    ) -> Self {
        self.session_store = Some(store);
        self.session_id = Some(session_id);
        self
    }

    /// 设置记忆快照（在 system prompt 中注入冻结的记忆）
    pub fn with_memory_snapshot(mut self, snapshot: String) -> Self {
        if !snapshot.is_empty() {
            self.system_prompt = format!("{}\n{}", snapshot, self.system_prompt);
        }
        self
    }

    /// 设置最大上下文长度
    pub fn with_max_context(mut self, tokens: u64) -> Self {
        self.compressor = Arc::new(tokio::sync::Mutex::new(
            crate::engine::context_compressor::ContextCompressor::new(tokens)
                .with_llm_provider(self.provider.clone(), &self.model),
        ));
        self
    }

    /// 清除对话历史
    pub async fn clear_history(&self) {
        let mut history = self.conversation_history.lock().await;
        history.clear();
    }

    /// 获取对话历史
    pub async fn get_history(&self) -> Vec<Message> {
        self.conversation_history.lock().await.clone()
    }

    /// 设置对话历史
    pub async fn set_history(&self, messages: Vec<Message>) {
        let mut history = self.conversation_history.lock().await;
        *history = messages;
    }

    /// 设置模型
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// 设置系统提示词
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// 设置最大迭代次数
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// 设置 Agent 管理器
    pub fn with_agent_manager(mut self, manager: Arc<crate::agent::AgentManager>) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    /// 设置 MCP 管理器
    pub fn with_mcp_manager(mut self, manager: Arc<crate::engine::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    /// 获取 MCP 管理器
    pub fn mcp_manager(&self) -> Option<Arc<crate::engine::mcp::McpManager>> {
        self.mcp_manager.clone()
    }

    /// 设置 LSP 管理器
    pub fn with_lsp_manager(mut self, manager: Arc<crate::engine::lsp::LspManager>) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    /// 设置 Worktree 管理器
    pub fn with_worktree_manager(
        mut self,
        manager: Arc<crate::engine::worktree::WorktreeManager>,
    ) -> Self {
        self.worktree_manager = Some(manager);
        self
    }

    /// 设置记忆管理器
    pub fn with_memory_manager(
        mut self,
        manager: Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>,
    ) -> Self {
        self.memory_manager = Some(manager);
        self
    }

    /// 设置权限模式
    pub fn with_permission_mode(mut self, mode: crate::permissions::PermissionMode) -> Self {
        self.permission_mode = Arc::new(std::sync::RwLock::new(mode));
        self
    }

    /// 设置是否启用 LLM 驱动的记忆提取
    pub fn with_llm_memory_extraction(mut self, enabled: bool) -> Self {
        self.llm_memory_extraction = enabled;
        self
    }

    /// 设置工具授权通道
    pub fn with_approval_channel(
        mut self,
        channel: Arc<crate::engine::conversation_loop::ToolApprovalChannel>,
    ) -> Self {
        self.approval_channel = Some(channel);
        self
    }

    /// 设置 fallback 模型
    pub fn with_fallback_model(mut self, model: impl Into<String>) -> Self {
        self.fallback_model = Some(model.into());
        self
    }

    /// 获取 fallback 模型名称
    pub fn fallback_model(&self) -> Option<&str> {
        self.fallback_model.as_deref()
    }

    /// 运行时更新权限模式（供 TUI 命令调用）
    pub fn set_permission_mode(&self, mode: crate::permissions::PermissionMode) {
        *self
            .permission_mode
            .write()
            .expect("permission_mode RwLock poisoned while setting mode") = mode;
    }

    /// 获取当前权限模式
    pub fn permission_mode(&self) -> crate::permissions::PermissionMode {
        *self
            .permission_mode
            .read()
            .expect("permission_mode RwLock poisoned while reading mode")
    }

    /// 获取记忆管理器
    pub fn memory_manager(&self) -> Option<Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>> {
        self.memory_manager.clone()
    }

    /// 获取任务管理器
    pub fn task_manager(&self) -> Option<Arc<crate::task_manager::TaskManager>> {
        self.task_manager.clone()
    }

    /// 获取 Agent 管理器
    pub fn agent_manager(&self) -> Option<Arc<crate::agent::AgentManager>> {
        self.agent_manager.clone()
    }

    /// 获取工具授权通道
    pub fn approval_channel(
        &self,
    ) -> Option<Arc<crate::engine::conversation_loop::ToolApprovalChannel>> {
        self.approval_channel.clone()
    }

    /// 获取工具注册表
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    /// 获取当前模型名
    pub fn model_name(&self) -> &str {
        &self.model
    }

    /// 获取当前 Provider 的 base URL（用于状态展示）
    pub fn provider_base_url(&self) -> &str {
        self.provider.base_url()
    }

    /// 执行流式查询（支持多轮对话）
    ///
    /// 返回一个事件流，调用者可以实时接收响应内容
    /// 自动维护对话历史，上下文不够时自动压缩
    pub async fn query_stream(
        &self,
        user_message: impl Into<String>,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        let user_msg = user_message.into();
        let (tx, rx) = mpsc::channel(100);

        // 准备共享资源
        let history = self.conversation_history.clone();
        let compressor = self.compressor.clone();
        let session_store = self.session_store.clone();
        let session_id = self.session_id.clone();

        let mut engine = StreamingEngineInner {
            provider: self.provider.clone(),
            tool_registry: self.tool_registry.clone(),
            model: self.model.clone(),
            system_prompt: self.system_prompt.clone(),
            max_iterations: self.max_iterations,
            agent_manager: self.agent_manager.clone(),
            task_manager: self.task_manager.clone(),
            mcp_manager: self.mcp_manager.clone(),
            lsp_manager: self.lsp_manager.clone(),
            worktree_manager: self.worktree_manager.clone(),
            memory_manager: self.memory_manager.clone(),
            cost_tracker: self.cost_tracker.clone(),
            permission_mode: self.permission_mode(),
            llm_memory_extraction: self.llm_memory_extraction,
            approval_channel: self.approval_channel.clone(),
            fallback_model: self.fallback_model.clone(),
            fallback_state: None,
        };

        tokio::spawn(async move {
            // 1. 添加用户消息到历史
            {
                let mut hist = history.lock().await;
                hist.push(Message::user(&user_msg));

                // 持久化用户消息
                if let (Some(ref store), Some(ref sid)) = (&session_store, &session_id) {
                    let _ = store.add_message(sid, "user", &user_msg, None, None);
                }
            }

            // 2. 检查是否需要压缩
            {
                let mut hist = history.lock().await;
                let mut comp = compressor.lock().await;
                if comp.needs_compression(&hist) {
                    let compressed = comp.compress_async(&hist).await;
                    *hist = compressed;
                }
            }

            // 3. 获取当前历史用于查询
            let messages_for_query = {
                let hist = history.lock().await;
                // 构建完整消息：system + history
                let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let layered =
                    crate::instructions::compose_system_prompt(&engine.system_prompt, &cwd);
                let composed =
                    crate::engine::prompt_builder::compose_task_aware_system_prompt_with_history(
                        &layered,
                        &user_msg,
                        &hist,
                    );
                let mut msgs = vec![Message::system(composed)];
                msgs.extend(hist.clone());
                msgs
            };

            // 4. 执行查询（带 fallback 支持）
            let mut assistant_content = String::new();
            let mut assistant_tool_calls = Vec::new();

            let run_result = engine
                .run_query_with_messages(messages_for_query.clone(), &tx)
                .await;

            match run_result {
                Ok((content, tool_calls)) => {
                    assistant_content = content;
                    assistant_tool_calls = tool_calls;
                }
                Err(e) => {
                    let err_str = e.to_string().to_lowercase();
                    let error_type = ErrorType::from_error_str(&err_str);

                    // 初始化 fallback_state（如果是第一次错误）
                    let fb_state = engine.fallback_state.take().unwrap_or_else(FallbackState::new);
                    let mut fb_state = fb_state;

                    // 记录错误
                    fb_state.record_error(error_type);

                    // 检查是否应触发 fallback（连续 3 次 529 或特定错误类型）
                    let should_try_fallback = if fb_state.fallback_triggered {
                        // 已触发过 fallback，检查是否还有尝试次数
                        !fb_state.max_attempts_reached()
                    } else {
                        // 检查是否应该触发 fallback
                        fb_state.should_trigger_fallback()
                            || error_type == ErrorType::RateLimit
                            || error_type == ErrorType::ContextTooLong
                            || error_type == ErrorType::ServerError
                    };

                    if should_try_fallback && engine.fallback_model.is_some() {
                        // 如果还没触发过 fallback，标记已触发
                        if !fb_state.fallback_triggered {
                            fb_state.fallback_triggered = true;
                            warn!(
                                "Fallback triggered after {} consecutive errors (type: {:?}), trying fallback model",
                                fb_state.consecutive_529_count,
                                error_type
                            );
                        }
                        fb_state.fallback_attempts += 1;

                        // Fallback: 重新执行，stream 事件会继续发送到 tx
                        let fb_model = engine.fallback_model.clone().unwrap();
                        let fb_engine = StreamingEngineInner {
                            provider: engine.provider.clone(),
                            tool_registry: engine.tool_registry.clone(),
                            model: fb_model,
                            system_prompt: engine.system_prompt.clone(),
                            max_iterations: engine.max_iterations,
                            agent_manager: engine.agent_manager.clone(),
                            task_manager: engine.task_manager.clone(),
                            mcp_manager: engine.mcp_manager.clone(),
                            lsp_manager: engine.lsp_manager.clone(),
                            worktree_manager: engine.worktree_manager.clone(),
                            memory_manager: engine.memory_manager.clone(),
                            cost_tracker: engine.cost_tracker.clone(),
                            permission_mode: engine.permission_mode,
                            llm_memory_extraction: engine.llm_memory_extraction,
                            approval_channel: engine.approval_channel.clone(),
                            fallback_model: None, // 防止无限 fallback
                            fallback_state: Some(fb_state),
                        };
                        match fb_engine
                            .run_query_with_messages(messages_for_query.clone(), &tx)
                            .await
                        {
                            Ok((content, tool_calls)) => {
                                assistant_content = content;
                                assistant_tool_calls = tool_calls;
                            }
                            Err(fb_err) => {
                                let _ = tx.send(StreamEvent::Error(fb_err.to_string())).await;
                            }
                        }
                    } else {
                        let _ = tx.send(StreamEvent::Error(e.to_string())).await;
                    }
                }
            }

            // 5. 添加助手回复到历史
            {
                let mut hist = history.lock().await;
                if !assistant_content.is_empty() || !assistant_tool_calls.is_empty() {
                    let assistant_msg = if assistant_tool_calls.is_empty() {
                        Message::assistant(&assistant_content)
                    } else {
                        Message::assistant_with_tools(&assistant_content, assistant_tool_calls)
                    };
                    hist.push(assistant_msg.clone());

                    // 持久化助手消息
                    if let (Some(ref store), Some(ref sid)) = (&session_store, &session_id) {
                        let tool_calls_json = assistant_msg
                            .tool_calls()
                            .map(|tc| serde_json::to_value(tc).unwrap_or(serde_json::Value::Null));
                        let _ = store.add_message(
                            sid,
                            "assistant",
                            &assistant_content,
                            tool_calls_json.as_ref(),
                            None,
                        );
                    }
                }
            }

            // 6. 自动 flush 记忆（每次查询结束后自动写入）
            if let Some(ref mem_mutex) = engine.memory_manager {
                let flush_history = {
                    let hist = history.lock().await;
                    hist.clone()
                };
                let mut mem = mem_mutex.lock().await;
                mem.flush_session_async(&flush_history).await;
            }
        });

        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }

    /// 执行非流式查询（兼容旧接口）
    pub async fn query(&self, user_message: impl Into<String>) -> Result<String> {
        let mut result = String::new();
        let mut stream = self.query_stream(user_message).await;

        use futures::StreamExt;
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextChunk(text) => result.push_str(&text),
                StreamEvent::Complete => break,
                StreamEvent::Error(e) => return Err(anyhow::anyhow!(e)),
                _ => {}
            }
        }

        Ok(result)
    }
}

/// 内部执行引擎
struct StreamingEngineInner {
    provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
    model: String,
    system_prompt: String,
    max_iterations: usize,
    agent_manager: Option<Arc<crate::agent::AgentManager>>,
    task_manager: Option<Arc<crate::task_manager::TaskManager>>,
    mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    memory_manager: Option<Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>>,
    cost_tracker: Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>,
    permission_mode: crate::permissions::PermissionMode,
    llm_memory_extraction: bool,
    approval_channel: Option<Arc<crate::engine::conversation_loop::ToolApprovalChannel>>,
    fallback_model: Option<String>,
    /// Fallback 状态追踪（连续错误计数）
    fallback_state: Option<FallbackState>,
}

/// Fallback 状态追踪
#[derive(Debug, Clone)]
struct FallbackState {
    /// 连续 529 (Model Overloaded) 错误计数
    pub consecutive_529_count: u32,
    /// 上次错误类型
    pub last_error_type: ErrorType,
    /// 是否已触发 fallback
    pub fallback_triggered: bool,
    /// fallback 尝试次数
    pub fallback_attempts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorType {
    RateLimit,       // 429
    ModelOverloaded, // 529
    ContextTooLong,  // 413
    Timeout,
    AuthError,       // 401/403
    ServerError,     // 500
    Unknown,
}

impl ErrorType {
    fn from_error_str(err_str: &str) -> Self {
        if err_str.contains("rate limit") || err_str.contains("429") {
            ErrorType::RateLimit
        } else if err_str.contains("overloaded") || err_str.contains("529") || err_str.contains("model overloaded") {
            ErrorType::ModelOverloaded
        } else if err_str.contains("context") || err_str.contains("413") || err_str.contains("too long") {
            ErrorType::ContextTooLong
        } else if err_str.contains("timeout") || err_str.contains("timed out") {
            ErrorType::Timeout
        } else if err_str.contains("401") || err_str.contains("403") || err_str.contains("unauthorized") || err_str.contains("forbidden") {
            ErrorType::AuthError
        } else if err_str.contains("500") || err_str.contains("internal server error") {
            ErrorType::ServerError
        } else if err_str.contains("model") {
            ErrorType::ModelOverloaded
        } else {
            ErrorType::Unknown
        }
    }
}

impl FallbackState {
    fn new() -> Self {
        Self {
            consecutive_529_count: 0,
            last_error_type: ErrorType::Unknown,
            fallback_triggered: false,
            fallback_attempts: 0,
        }
    }

    /// 记录错误并更新状态
    fn record_error(&mut self, error_type: ErrorType) {
        self.last_error_type = error_type;
        if error_type == ErrorType::ModelOverloaded {
            self.consecutive_529_count += 1;
        } else {
            self.consecutive_529_count = 0;
        }
    }

    /// 检查是否应该触发 fallback（连续 3 次 529 后触发）
    fn should_trigger_fallback(&self) -> bool {
        self.consecutive_529_count >= 3
    }

    /// 获取最大 fallback 尝试次数
    fn max_fallback_attempts() -> u32 {
        std::env::var("PRIORITY_AGENT_FALLBACK_MAX_ATTEMPTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3)
    }

    /// 检查是否达到最大尝试次数
    fn max_attempts_reached(&self) -> bool {
        self.fallback_attempts >= Self::max_fallback_attempts()
    }
}

impl StreamingEngineInner {
    /// 使用预构建的消息列表执行查询，委托给统一对话循环
    async fn run_query_with_messages(
        &self,
        messages: Vec<Message>,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<(String, Vec<crate::services::api::ToolCall>)> {
        let mut builder = super::ConversationLoopBuilder::new(
            self.provider.clone(),
            self.tool_registry.clone(),
            self.cost_tracker.clone(),
            &self.model,
        )
        .with_max_iterations(self.max_iterations)
        .with_permission_mode(self.permission_mode)
        .with_llm_memory_extraction(self.llm_memory_extraction);

        if let Some(ref manager) = self.agent_manager {
            builder = builder.with_agent_manager(manager.clone());
        }
        if let Some(ref mcp) = self.mcp_manager {
            builder = builder.with_mcp_manager(mcp.clone());
        }
        if let Some(ref lsp) = self.lsp_manager {
            builder = builder.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            builder = builder.with_worktree_manager(wt.clone());
        }
        if let Some(ref mem) = self.memory_manager {
            builder = builder.with_memory_manager(mem.clone());
        }
        if let Some(ref channel) = self.approval_channel {
            builder = builder.with_approval_channel(channel.clone());
        }

        let result = builder.build().run_streaming(messages, tx).await?;
        Ok((result.content, result.tool_calls))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_creation() {
        let event = StreamEvent::TextChunk("Hello".to_string());
        assert!(matches!(event, StreamEvent::TextChunk(_)));
    }
}
