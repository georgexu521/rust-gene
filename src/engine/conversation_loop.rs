//! 统一对话循环
//!
//! 将 QueryEngine 和 StreamingEngineInner 中重复的工具调用循环合并为一处。
//! 支持流式/非流式两种输出模式，内部逻辑完全一致。
//!
//! 改进（借鉴 hermes-agent）：
//! - 前置压缩（Preflight）：循环前检查总 token，超阈值提前压缩
//! - IterationBudget：迭代预算退还机制（只读工具可退还）

use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, Message, ToolCall};
use crate::tools::{ToolContext, ToolRegistry, ToolResult};
use anyhow::Result;
use futures::StreamExt;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, warn};

use super::context_compressor::{
    estimate_messages_tokens, estimate_tool_schemas_tokens, ContextCompressor,
};
use super::hooks::{HookDecision, ToolHookManager};
use super::streaming::StreamEvent;

/// 只读工具列表（不消耗迭代预算，可并发执行）
const READ_ONLY_TOOLS: &[&str] = &[
    "grep",
    "glob",
    "file_read",
    "project_list",
    "memory_load",
    "skills_list",
    "skill_view",
    "web_search",
];

/// 工具授权请求
#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub tool_call: ToolCall,
    pub prompt: String,
}

/// 待审批的工具请求 + 响应通道
type PendingApproval = Option<(ToolApprovalRequest, tokio::sync::oneshot::Sender<bool>)>;

/// 工具授权通道（类似 PlanApprovalChannel）
pub struct ToolApprovalChannel {
    pending: Arc<Mutex<PendingApproval>>,
}

impl ToolApprovalChannel {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
        }
    }

    /// 提交授权请求并等待响应（60 秒超时）
    pub async fn submit(&self, request: ToolApprovalRequest) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            *pending = Some((request, tx));
        }
        match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
            Ok(result) => result.map_err(|_| anyhow::anyhow!("Approval channel closed")),
            Err(_) => Err(anyhow::anyhow!("Tool approval timed out after 60 seconds")),
        }
    }

    /// TUI 取出待审批的请求
    pub async fn take_pending(
        &self,
    ) -> Option<(ToolApprovalRequest, tokio::sync::oneshot::Sender<bool>)> {
        let mut pending = self.pending.lock().await;
        pending.take()
    }

    /// 是否有待审批的请求
    pub async fn has_pending(&self) -> bool {
        self.pending.lock().await.is_some()
    }
}

/// 统一对话循环
pub struct ConversationLoop {
    provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
    cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
    model: String,
    max_iterations: usize,
    agent_manager: Option<Arc<crate::agent::AgentManager>>,
    mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    hook_manager: Option<Arc<ToolHookManager>>,
    /// 上下文压缩器
    compressor: Option<Mutex<ContextCompressor>>,
    /// 记忆管理器（预取 + 围栏注入 + 同步）
    memory_manager: Option<Arc<Mutex<crate::memory::MemoryManager>>>,
    /// 工具权限模式（由上层引擎注入）
    permission_mode: crate::permissions::PermissionMode,
    /// 是否启用 LLM 驱动的记忆提取
    llm_memory_extraction: bool,
    /// 工具授权通道（用于 MCP 等工具的交互式授权）
    approval_channel: Option<Arc<ToolApprovalChannel>>,
    /// 工具白名单（用于子 Agent 隔离；None 表示不限制）
    allowed_tools: Option<HashSet<String>>,
}

/// 对话循环结果
pub struct LoopResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub iterations: usize,
}

impl ConversationLoop {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
        model: String,
    ) -> Self {
        Self {
            provider,
            tool_registry,
            cost_tracker,
            model,
            max_iterations: 10,
            agent_manager: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            hook_manager: ToolHookManager::from_env().map(Arc::new),
            compressor: None,
            memory_manager: None,
            permission_mode: crate::permissions::PermissionMode::AutoLowRisk,
            llm_memory_extraction: false,
            approval_channel: None,
            allowed_tools: None,
        }
    }

    /// 启用记忆管理器（预取 + 围栏注入 + 同步）
    pub fn with_memory_manager(
        mut self,
        manager: Arc<Mutex<crate::memory::MemoryManager>>,
    ) -> Self {
        self.memory_manager = Some(manager);
        self
    }

    /// 启用上下文压缩（设置最大上下文 token 数）
    pub fn with_compression(mut self, max_context_tokens: u64) -> Self {
        self.compressor = Some(Mutex::new(
            ContextCompressor::new(max_context_tokens)
                .with_llm_provider(self.provider.clone(), &self.model),
        ));
        self
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_agent_manager(mut self, manager: Arc<crate::agent::AgentManager>) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    pub fn with_mcp_manager(mut self, manager: Arc<crate::engine::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    pub fn with_lsp_manager(mut self, manager: Arc<crate::engine::lsp::LspManager>) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    pub fn with_worktree_manager(
        mut self,
        manager: Arc<crate::engine::worktree::WorktreeManager>,
    ) -> Self {
        self.worktree_manager = Some(manager);
        self
    }

    pub fn with_hook_manager(mut self, manager: Arc<ToolHookManager>) -> Self {
        self.hook_manager = Some(manager);
        self
    }

    pub fn with_permission_mode(mut self, mode: crate::permissions::PermissionMode) -> Self {
        self.permission_mode = mode;
        self
    }

    pub fn with_llm_memory_extraction(mut self, enabled: bool) -> Self {
        self.llm_memory_extraction = enabled;
        self
    }

    pub fn with_approval_channel(mut self, channel: Arc<ToolApprovalChannel>) -> Self {
        self.approval_channel = Some(channel);
        self
    }

    pub fn with_allowed_tools(mut self, tools: HashSet<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    /// 创建工具执行上下文
    fn create_tool_context(&self) -> ToolContext {
        let mut ctx = ToolContext::new(".", format!("session-{}", uuid::Uuid::new_v4()));
        if let Some(ref manager) = self.agent_manager {
            ctx = ctx.with_agent_manager(manager.clone());
        }
        if let Some(ref mcp) = self.mcp_manager {
            ctx = ctx.with_mcp_manager(mcp.clone());
        }
        if let Some(ref lsp) = self.lsp_manager {
            ctx = ctx.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            ctx = ctx.with_worktree_manager(wt.clone());
        }
        ctx = ctx.with_llm_provider(self.provider.clone());
        ctx = ctx.with_model(&self.model);
        ctx = ctx.with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone());
        // 权限模式由上层引擎注入（默认 AutoLowRisk）
        ctx.permission_context.mode = self.permission_mode;
        ctx
    }

    /// 运行对话循环（非流式）
    pub async fn run(&self, messages: Vec<Message>) -> Result<LoopResult> {
        self.run_inner(messages, None::<&mpsc::Sender<StreamEvent>>)
            .await
    }

    /// 运行对话循环（流式）
    pub async fn run_streaming(
        &self,
        messages: Vec<Message>,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<LoopResult> {
        self.run_inner(messages, Some(tx)).await
    }

    /// 核心循环实现
    async fn run_inner(
        &self,
        mut messages: Vec<Message>,
        tx: Option<&mpsc::Sender<StreamEvent>>,
    ) -> Result<LoopResult> {
        let tools = self.get_tools();
        let mut final_content = String::new();
        let mut final_tool_calls = Vec::new();
        let mut iterations_used = 0;

        // ── 前置压缩（Preflight）─────────────────────────
        // 进入循环前检查总 token（消息 + 工具 schema），超阈值提前压缩
        // 支持最多 3 轮连续压缩（Hermes 风格）
        if let Some(ref compressor_mutex) = self.compressor {
            for pass in 0..3 {
                let compressor = compressor_mutex.lock().await;
                let tool_tokens = estimate_tool_schemas_tokens(&tools);
                let msg_tokens = estimate_messages_tokens(&messages);
                if !compressor.preflight_check(&messages, msg_tokens, tool_tokens) {
                    break; // 不再需要压缩
                }
                debug!(
                    "Preflight compression pass {}/3 ({} msg + {} tool tokens)",
                    pass + 1,
                    msg_tokens,
                    tool_tokens
                );
                drop(compressor); // 释放锁
                messages = compressor_mutex.lock().await.compress_async(&messages).await;
            }
        }

        if let Some(tx) = tx {
            let _ = tx.send(StreamEvent::Start).await;
        }

        // ── 记忆围栏注入 ───────────────────────────────
        // 将冻结的记忆快照作为 system message 注入（XML 围栏包裹）
        if let Some(ref mem_mutex) = self.memory_manager {
            let mem = mem_mutex.lock().await;
            let snapshot = mem.get_snapshot();
            if !snapshot.is_empty() {
                // 在 system messages 末尾（用户消息之前）注入记忆
                // 找到第一个非 system 消息的位置
                let insert_pos = messages
                    .iter()
                    .position(|m| !matches!(m, Message::System { .. }))
                    .unwrap_or(messages.len());
                messages.insert(insert_pos, Message::system(&snapshot));
                debug!("Injected memory context fence at position {}", insert_pos);
            }
        }

        // ── 迭代预算 ─────────────────────────────────────
        let mut effective_iterations: usize = 0; // 消耗的"有效"迭代（扣除了退还的）

        for iteration in 0..self.max_iterations {
            debug!(
                "Conversation loop iteration {} (effective: {}/{})",
                iteration, effective_iterations, self.max_iterations
            );
            iterations_used = iteration + 1;

            // 每次迭代开始重置预取状态，确保当前轮可再次进行 prefetch
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                mem.reset_turn();
            }

            // 检查有效迭代是否耗尽
            if effective_iterations >= self.max_iterations {
                warn!(
                    "Effective iteration budget exhausted ({}/{})",
                    effective_iterations, self.max_iterations
                );
                break;
            }

            // 构建请求
            // 记忆预取：在每次 API 调用前搜索相关记忆并注入到最后的用户消息
            let mut request_messages = messages.clone();
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                // 找到最后一条用户消息
                if let Some(last_user_idx) = request_messages
                    .iter()
                    .rposition(|m| matches!(m, Message::User { .. }))
                {
                    if let Message::User { content } = &request_messages[last_user_idx] {
                        let prefetch = mem.prefetch(content);
                        if !prefetch.is_empty() {
                            // 将预取的记忆注入到用户消息中（XML 围栏包裹）
                            let enhanced = format!(
                                "{}\n<relevant-memory>\n{}\n</relevant-memory>",
                                content, prefetch
                            );
                            request_messages[last_user_idx] = Message::user(&enhanced);
                            debug!("Prefetched memory context injected into user message");
                        }
                    }
                }
            }

            let request = ChatRequest::new(&self.model)
                .with_messages(request_messages)
                .with_tools(tools.clone());

            // 根据模式选择 API 调用方式
            let (content, tool_calls) = if let Some(tx) = tx {
                self.call_api_streaming(request, tx).await?
            } else {
                self.call_api(request).await?
            };

            final_content = content.clone();
            final_tool_calls = tool_calls.clone();

            // 没有工具调用 → 完成
            if tool_calls.is_empty() {
                break;
            }

            // 有工具调用 → 添加助手消息到历史
            messages.push(Message::assistant_with_tools(&content, tool_calls.clone()));

            // 并行执行工具
            let results = self.execute_tools_parallel(&tool_calls, tx).await;

            // ── 迭代预算退还 ──────────────────────────────
            // 检查本轮工具调用是否全是只读的，如果是则退还迭代
            let all_read_only = tool_calls
                .iter()
                .all(|tc| READ_ONLY_TOOLS.iter().any(|&name| tc.name == name));

            if all_read_only {
                debug!("All tools read-only, refunding iteration budget");
                // 不增加 effective_iterations → 退还
            } else {
                effective_iterations += 1;
            }

            // 将工具结果添加到消息历史
            let mut tool_results_text = String::new();
            let mut changed_files = Vec::new();
            for (tc, result) in &results {
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    result.content
                );
                tool_results_text.push_str(&result_content);
                tool_results_text.push('\n');
                messages.push(Message::tool(tc.id.clone(), result_content));

                // 收集文件修改成功的路径用于自动验证
                if result.success && (tc.name == "file_edit" || tc.name == "file_write") {
                    if let Some(path) = tc.arguments["path"].as_str() {
                        changed_files.push(std::path::PathBuf::from(path));
                    }
                }
            }

            // ── 自动验证闭环 ──────────────────────────────
            if !changed_files.is_empty() {
                let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let verify_results = super::auto_verify::verify_file_changes(
                    &working_dir,
                    &changed_files,
                )
                .await;
                let check_passed = verify_results.iter().all(|r| r.success);
                for result in verify_results {
                    let verify_text = result.to_dialog_text();
                    if !result.success {
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&verify_text);
                        messages.push(Message::system(verify_text));
                    } else {
                        // 验证通过也可作为轻量提示
                        debug!("{}", verify_text);
                    }
                }

                // ── LSP 诊断补充 ───────────────────────────
                // 如果 LSP manager 可用，获取修改文件的缓存诊断
                if let Some(ref lsp_mgr) = self.lsp_manager {
                    let mut lsp_issues = Vec::new();
                    for path in &changed_files {
                        let uri = super::lsp::path_to_uri(path);
                        for name in lsp_mgr.server_names() {
                            if let Some(client) = lsp_mgr.get_client(&name) {
                                let diagnostics = client.get_diagnostics(&uri).await;
                                for d in diagnostics {
                                    let sev = match d.severity {
                                        Some(1) => "error",
                                        Some(2) => "warning",
                                        Some(3) => "info",
                                        Some(4) => "hint",
                                        _ => "diagnostic",
                                    };
                                    lsp_issues.push(format!(
                                        "  [{}] {}:{}: {}",
                                        sev,
                                        path.display(),
                                        d.range.start.line + 1,
                                        d.message.replace('\n', " ")
                                    ));
                                }
                            }
                        }
                    }
                    if !lsp_issues.is_empty() {
                        let lsp_text = format!(
                            "[LSP diagnostics for modified files]:\n{}",
                            lsp_issues.join("\n")
                        );
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&lsp_text);
                        messages.push(Message::system(lsp_text));
                    }
                }

                // ── 自动测试闭环 ──────────────────────────────
                let test_results = super::auto_verify::run_tests(
                    &working_dir,
                    &changed_files,
                    check_passed,
                )
                .await;
                let tests_passed = test_results.iter().all(|r| r.success);
                for result in test_results {
                    let test_text = result.to_dialog_text();
                    if !result.success {
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&test_text);
                        messages.push(Message::system(test_text));
                    } else {
                        debug!("{}", test_text);
                    }
                }

                // ── 代码自审查 ────────────────────────────────
                let review_result = super::code_review::review_changed_files(
                    &working_dir,
                    &changed_files,
                );
                if !review_result.success {
                    let review_text = review_result.to_dialog_text();
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&review_text);
                    messages.push(Message::system(review_text));
                }

                // ── 编程质量可观测性 ───────────────────────
                let verify_passed = check_passed && tests_passed && review_result.success;
                {
                    let mut tracker = self.cost_tracker.lock().await;
                    tracker.record_coding_round(verify_passed);
                }
            }

            // ── 记忆同步 ──────────────────────────────────
            // 从本轮对话中提取学习内容
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                // sync_turn 用最后一条用户消息 + 助手/工具结果来提取学习
                let user_msg = messages
                    .iter()
                    .rposition(|m| matches!(m, Message::User { .. }))
                    .and_then(|i| match &messages[i] {
                        Message::User { content } => Some(content.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");
                if !user_msg.is_empty() {
                    let assistant_text = format!("{} {}", final_content, tool_results_text);
                    if self.llm_memory_extraction {
                        let provider: Option<&dyn LlmProvider> = Some(self.provider.as_ref());
                        mem.sync_turn_llm(user_msg, &assistant_text, provider, &self.model)
                            .await;
                    } else {
                        mem.sync_turn(user_msg, &assistant_text);
                    }
                }
            }
        }

        if let Some(tx) = tx {
            let _ = tx.send(StreamEvent::Complete).await;
        }

        Ok(LoopResult {
            content: final_content,
            tool_calls: final_tool_calls,
            iterations: iterations_used,
        })
    }

    /// 非流式 API 调用
    async fn call_api(&self, request: ChatRequest) -> Result<(String, Vec<ToolCall>)> {
        let response = self.provider.chat(request).await?;
        self.record_cost(&response).await;

        let content = response.content.clone();
        let tool_calls = response.tool_calls.unwrap_or_default();

        Ok((content, tool_calls))
    }

    /// 流式 API 调用
    async fn call_api_streaming(
        &self,
        request: ChatRequest,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<(String, Vec<ToolCall>)> {
        // 保存 fallback 需要的数据
        let fallback_messages = request.messages.clone();
        let fallback_tools = request.tools.clone();

        match self.provider.chat_stream(request).await {
            Ok(mut stream) => {
                let mut full_content = String::new();
                let mut collected_tool_calls: Vec<ToolCall> = Vec::new();
                let mut raw_args_accum: Vec<String> = Vec::new();

                while let Some(result) = stream.next().await {
                    match result {
                        Ok(chunk) => {
                            if let Some(choice) = chunk.choices.first() {
                                // 处理文本内容
                                if let Some(content) = &choice.delta.content {
                                    if !content.is_empty() {
                                        full_content.push_str(content);
                                        let _ =
                                            tx.send(StreamEvent::TextChunk(content.clone())).await;
                                    }
                                }

                                // 处理工具调用增量
                                if let Some(tool_calls) = &choice.delta.tool_calls {
                                    for tc_delta in tool_calls {
                                        let idx = tc_delta.index as usize;
                                        while collected_tool_calls.len() <= idx {
                                            collected_tool_calls.push(ToolCall {
                                                id: String::new(),
                                                name: String::new(),
                                                arguments: serde_json::Value::Null,
                                            });
                                            raw_args_accum.push(String::new());
                                        }
                                        let tc = &mut collected_tool_calls[idx];
                                        if let Some(id) = &tc_delta.id {
                                            tc.id = id.clone();
                                            let _ = tx
                                                .send(StreamEvent::ToolCallStart {
                                                    id: id.clone(),
                                                    name: tc.name.clone(),
                                                })
                                                .await;
                                        }
                                        if let Some(function) = &tc_delta.function {
                                            if let Some(name) = &function.name {
                                                tc.name = name.clone();
                                            }
                                            if let Some(args) = &function.arguments {
                                                raw_args_accum[idx].push_str(args);
                                                let _ = tx
                                                    .send(StreamEvent::ToolCallArgs {
                                                        id: tc.id.clone(),
                                                        args_delta: args.clone(),
                                                    })
                                                    .await;
                                            }
                                        }
                                    }
                                }
                            }

                            if chunk.choices.iter().any(|c| c.finish_reason.is_some()) {
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Stream error: {}", e);
                            let _ = tx
                                .send(StreamEvent::Error(format!("Stream error: {}", e)))
                                .await;
                            break;
                        }
                    }
                }

                // 解析累积的工具调用参数
                for (i, tc) in collected_tool_calls.iter_mut().enumerate() {
                    if i < raw_args_accum.len() && !raw_args_accum[i].is_empty() {
                        tc.arguments =
                            serde_json::from_str(&raw_args_accum[i]).unwrap_or_else(|e| {
                                warn!("Failed to parse tool args: {}", e);
                                serde_json::Value::Null
                            });
                        let _ = tx
                            .send(StreamEvent::ToolCallComplete { id: tc.id.clone() })
                            .await;
                    }
                }

                Ok((full_content, collected_tool_calls))
            }
            Err(e) => {
                // 流式 API 失败，回退到非流式
                warn!("Streaming failed, falling back to non-streaming: {}", e);
                let response = self
                    .provider
                    .chat(
                        ChatRequest::new(&self.model)
                            .with_messages(fallback_messages)
                            .with_tools(fallback_tools.unwrap_or_default()),
                    )
                    .await?;
                self.record_cost(&response).await;

                let content = response.content.clone();
                if !content.is_empty() {
                    let _ = tx.send(StreamEvent::TextChunk(content.clone())).await;
                }
                let tool_calls = response.tool_calls.unwrap_or_default();
                Ok((content, tool_calls))
            }
        }
    }

    /// 记录 API 调用成本
    async fn record_cost(&self, response: &ChatResponse) {
        if let Some(ref usage) = response.usage {
            let mut tracker = self.cost_tracker.lock().await;
            tracker.record_api_call(
                &self.model,
                usage.prompt_tokens as u64,
                usage.completion_tokens as u64,
            );
        }
    }

    /// 获取工具定义列表
    fn get_tools(&self) -> Vec<crate::services::api::Tool> {
        self.tool_registry
            .iter_tools()
            .filter(|t| {
                if let Some(ref allowed) = self.allowed_tools {
                    allowed.contains(t.name())
                } else {
                    true
                }
            })
            .map(|t| crate::services::api::Tool {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters(),
            })
            .collect()
    }

    /// 检查工具是否为只读（可并发执行）
    fn is_read_only(tool_name: &str) -> bool {
        READ_ONLY_TOOLS.contains(&tool_name)
    }

    /// 并行执行工具调用
    async fn execute_tools_parallel(
        &self,
        tool_calls: &[ToolCall],
        tx: Option<&mpsc::Sender<StreamEvent>>,
    ) -> Vec<(ToolCall, ToolResult)> {
        let mut read_only_handles = Vec::new();
        let mut read_write_calls = Vec::new();
        let mut denied_results = Vec::new();

        for tc in tool_calls {
            if tc.name.is_empty() {
                continue;
            }
            if let Some(ref allowed) = self.allowed_tools {
                if !allowed.contains(&tc.name) {
                    denied_results.push((
                        tc.clone(),
                        ToolResult::error(format!(
                            "Tool '{}' is not allowed in this agent context",
                            tc.name
                        )),
                    ));
                    continue;
                }
            }

            if Self::is_read_only(&tc.name) {
                let registry = self.tool_registry.clone();
                let context = self.create_tool_context();
                let tc_clone = tc.clone();
                let tool_name = tc.name.clone();
                let cost_tracker = self.cost_tracker.clone();
                let hook_manager = self.hook_manager.clone();

                let handle = tokio::spawn(async move {
                    let started_at = std::time::Instant::now();
                    let pre_decision = if let Some(ref hooks) = hook_manager {
                        hooks.run_pre_tool(&tc_clone, &context).await
                    } else {
                        HookDecision {
                            allow: true,
                            reason: None,
                        }
                    };

                    let mut result =
                        if !pre_decision.allow {
                            ToolResult::error(pre_decision.reason.unwrap_or_else(|| {
                                format!("blocked by pre-tool hook: {}", tool_name)
                            }))
                        } else if let Some(tool) = registry.get(&tool_name) {
                            tool.execute(tc_clone.arguments.clone(), context.clone())
                                .await
                        } else {
                            ToolResult::error(format!("Tool '{}' not found", tool_name))
                        };
                    let duration_ms = started_at.elapsed().as_millis() as u64;
                    if result.duration_ms.is_none() {
                        result.duration_ms = Some(duration_ms);
                    }

                    if let Some(ref hooks) = hook_manager {
                        hooks.run_post_tool(&tc_clone, &result, &context).await;
                    };
                    {
                        let mut tracker = cost_tracker.lock().await;
                        tracker.record_tool_execution(
                            &tool_name,
                            result.success,
                            duration_ms,
                            result.error.as_deref(),
                        );
                    }
                    (tc_clone, result)
                });
                read_only_handles.push((tc.id.clone(), handle));
            } else {
                read_write_calls.push(tc.clone());
            }
        }

        let mut results = denied_results;

        // 等待所有只读工具完成
        for (tool_id, handle) in read_only_handles {
            if let Some(tx) = tx {
                let _ = tx
                    .send(StreamEvent::ToolExecutionStart {
                        id: tool_id.clone(),
                        name: String::new(),
                    })
                    .await;
            }
            match handle.await {
                Ok((tc, result)) => {
                    if let Some(tx) = tx {
                        let result_content = format!(
                            "Result: {}\n{}",
                            if result.success { "OK" } else { "ERROR" },
                            result.content
                        );
                        let _ = tx
                            .send(StreamEvent::ToolExecutionComplete {
                                id: tool_id.clone(),
                                result: result_content,
                            })
                            .await;
                    }
                    results.push((tc, result));
                }
                Err(e) => {
                    if let Some(tx) = tx {
                        let _ = tx
                            .send(StreamEvent::ToolExecutionComplete {
                                id: tool_id.clone(),
                                result: format!("Error: task panicked: {}", e),
                            })
                            .await;
                    }
                }
            }
        }

        // 串行执行读写工具
        for tc in read_write_calls {
            let tool_id = tc.id.clone();
            let tool_name = tc.name.clone();
            if let Some(ref allowed) = self.allowed_tools {
                if !allowed.contains(&tool_name) {
                    results.push((
                        tc,
                        ToolResult::error(format!(
                            "Tool '{}' is not allowed in this agent context",
                            tool_name
                        )),
                    ));
                    continue;
                }
            }

            if let Some(tx) = tx {
                let _ = tx
                    .send(StreamEvent::ToolExecutionStart {
                        id: tool_id.clone(),
                        name: tool_name.clone(),
                    })
                    .await;
            }

            let (result, hook_context) = if let Some(tool) = self.tool_registry.get(&tool_name) {
                let context = self.create_tool_context();
                let pre_decision = if let Some(ref hooks) = self.hook_manager {
                    hooks.run_pre_tool(&tc, &context).await
                } else {
                    HookDecision {
                        allow: true,
                        reason: None,
                    }
                };

                let started_at = std::time::Instant::now();
                let mut result = if !pre_decision.allow {
                    ToolResult::error(
                        pre_decision
                            .reason
                            .unwrap_or_else(|| format!("blocked by pre-tool hook: {}", tool_name)),
                    )
                } else if context
                    .permission_context
                    .requires_confirmation(&tool_name, &tc.arguments)
                {
                    // 交互式授权（适用于所有需要确认的工具）
                    let mut approved = false;
                    if let (Some(ref channel), Some(tx)) = (&self.approval_channel, tx) {
                        let prompt = if tool_name == "mcp_tool" {
                            let server = tc.arguments["server_name"].as_str().unwrap_or("");
                            let t = tc.arguments["tool_name"].as_str().unwrap_or("");
                            format!(
                                "MCP tool '{}' on server '{}' requires approval. Allow?",
                                t, server
                            )
                        } else {
                            format!("Tool '{}' requires approval. Allow?", tool_name)
                        };
                        let _ = tx
                            .send(StreamEvent::PermissionRequest {
                                id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                arguments: tc.arguments.clone(),
                                prompt: prompt.clone(),
                            })
                            .await;
                        let request = ToolApprovalRequest {
                            tool_call: tc.clone(),
                            prompt,
                        };
                        match channel.submit(request).await {
                            Ok(is_approved) => approved = is_approved,
                            Err(e) => {
                                warn!("Tool approval error: {}", e);
                            }
                        }
                    }
                    if approved {
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamEvent::ToolExecutionProgress {
                                    id: tool_id.clone(),
                                    progress: format!("Executing {}...", tool_name),
                                })
                                .await;
                        }
                        tool.execute(tc.arguments.clone(), context.clone()).await
                    } else {
                        ToolResult::error(format!(
                            "Permission denied: '{}' requires user confirmation.",
                            tool_name
                        ))
                    }
                } else {
                    if let Some(tx) = tx {
                        let _ = tx
                            .send(StreamEvent::ToolExecutionProgress {
                                id: tool_id.clone(),
                                progress: format!("Executing {}...", tool_name),
                            })
                            .await;
                    }
                    tool.execute(tc.arguments.clone(), context.clone()).await
                };
                let duration_ms = started_at.elapsed().as_millis() as u64;
                if result.duration_ms.is_none() {
                    result.duration_ms = Some(duration_ms);
                }
                {
                    let mut tracker = self.cost_tracker.lock().await;
                    tracker.record_tool_execution(
                        &tool_name,
                        result.success,
                        duration_ms,
                        result.error.as_deref(),
                    );
                }

                (result, Some(context))
            } else {
                (
                    ToolResult::error(format!("Tool '{}' not found", tool_name)),
                    None,
                )
            };

            if let (Some(hooks), Some(context)) = (&self.hook_manager, &hook_context) {
                hooks.run_post_tool(&tc, &result, context).await;
            }

            if let Some(tx) = tx {
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    result.content
                );
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tool_id.clone(),
                        result: result_content,
                    })
                    .await;
            }
            results.push((tc, result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::types::ChatCompletionResponseStream;
    use crate::services::api::{ChatResponse, ToolCall, Usage};
    use crate::tools::FileWriteTool;
    use std::collections::VecDeque;
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;

    struct MockLlmProvider {
        responses: StdMutex<VecDeque<ChatResponse>>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockLlmProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let mut guard = self.responses.lock().unwrap();
            guard
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("no mock response left"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    #[tokio::test]
    async fn test_coding_quality_tracks_fail_then_repair_cycle() {
        let tmp = tempdir().expect("create temp dir");
        let target_file = tmp.path().join("sample.rs");
        let target_path = target_file.to_string_lossy().to_string();

        let failing_code = "fn main() { let x = Some(1).unwrap(); let _ = x; }";
        let fixed_code = "fn main() { let x = Some(1); if let Some(v) = x { let _ = v; } }";

        let responses = VecDeque::from(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_path,
                        "content": failing_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                }),
            },
            ChatResponse {
                content: "done".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                }),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_2".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_file.to_string_lossy(),
                        "content": fixed_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                }),
            },
            ChatResponse {
                content: "done".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                }),
            },
        ]);

        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(responses),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileWriteTool);
        let registry = Arc::new(registry);
        let tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

        let loop_engine = ConversationLoop::new(
            provider,
            registry,
            tracker.clone(),
            "mock-model".to_string(),
        )
        .with_permission_mode(crate::permissions::PermissionMode::AutoAll)
        .with_max_iterations(4);

        let old = std::env::var("PRIORITY_AGENT_AUTO_REVIEW").ok();
        std::env::set_var("PRIORITY_AGENT_AUTO_REVIEW", "1");

        let run1 = loop_engine
            .run(vec![Message::system("sys"), Message::user("write failing code")])
            .await;
        assert!(run1.is_ok(), "first run failed: {:?}", run1.err());

        {
            let t = tracker.lock().await;
            assert_eq!(t.coding_quality.file_change_rounds, 1);
            assert_eq!(t.coding_quality.verify_failures, 1);
            assert_eq!(t.coding_quality.repair_cycles, 0);
        }

        let run2 = loop_engine
            .run(vec![Message::system("sys"), Message::user("fix the code")])
            .await;
        assert!(run2.is_ok(), "second run failed: {:?}", run2.err());

        {
            let t = tracker.lock().await;
            assert_eq!(t.coding_quality.file_change_rounds, 2);
            assert_eq!(t.coding_quality.verify_failures, 1);
            assert_eq!(t.coding_quality.repair_cycles, 1);
            assert_eq!(t.coding_quality.first_pass_successes, 0);
        }

        if let Some(v) = old {
            std::env::set_var("PRIORITY_AGENT_AUTO_REVIEW", v);
        } else {
            std::env::remove_var("PRIORITY_AGENT_AUTO_REVIEW");
        }
    }

    #[tokio::test]
    async fn test_coding_quality_tracks_first_pass_success() {
        let tmp = tempdir().expect("create temp dir");
        let target_file = tmp.path().join("sample_ok.rs");
        let target_path = target_file.to_string_lossy().to_string();

        let safe_code = "fn main() { let x = Some(1); if let Some(v) = x { let _ = v; } }";
        let responses = VecDeque::from(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_ok_1".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_path,
                        "content": safe_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                }),
            },
            ChatResponse {
                content: "done".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                }),
            },
        ]);

        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(responses),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileWriteTool);
        let registry = Arc::new(registry);
        let tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

        let loop_engine = ConversationLoop::new(
            provider,
            registry,
            tracker.clone(),
            "mock-model".to_string(),
        )
        .with_permission_mode(crate::permissions::PermissionMode::AutoAll)
        .with_max_iterations(3);

        let old = std::env::var("PRIORITY_AGENT_AUTO_REVIEW").ok();
        std::env::set_var("PRIORITY_AGENT_AUTO_REVIEW", "1");

        let run = loop_engine
            .run(vec![Message::system("sys"), Message::user("write safe code")])
            .await;
        assert!(run.is_ok(), "run failed: {:?}", run.err());

        {
            let t = tracker.lock().await;
            assert_eq!(t.coding_quality.file_change_rounds, 1);
            assert_eq!(t.coding_quality.first_pass_successes, 1);
            assert_eq!(t.coding_quality.verify_failures, 0);
            assert_eq!(t.coding_quality.repair_cycles, 0);
        }

        if let Some(v) = old {
            std::env::set_var("PRIORITY_AGENT_AUTO_REVIEW", v);
        } else {
            std::env::remove_var("PRIORITY_AGENT_AUTO_REVIEW");
        }
    }
}
