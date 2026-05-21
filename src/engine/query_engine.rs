//! QueryEngine - 核心查询引擎
//!
//! 管理与 LLM 的交互，处理消息循环、工具调用、流式响应

use crate::services::api::{ChatRequest, LlmProvider, Message};
use crate::tools::ToolRegistry;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

/// 查询引擎
pub struct QueryEngine {
    /// LLM 提供商
    provider: Arc<dyn LlmProvider>,
    /// 工具注册表
    tool_registry: Arc<ToolRegistry>,
    /// 模型名称
    model: String,
    /// 系统提示词
    system_prompt: String,
    /// Agent 管理器（可选，用于子 Agent 创建）
    agent_manager: Option<Arc<crate::agent::AgentManager>>,
    /// 任务管理器（可选，用于 task_tool 等）
    task_manager: Option<Arc<crate::task_manager::TaskManager>>,
    /// LSP 管理器（可选，用于 lsp_tool 等）
    lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器（可选，用于 worktree_tool 等）
    worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    /// MCP 管理器（可选，用于 mcp_tool 等）
    mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    /// 成本追踪器
    cost_tracker: Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>,
    /// 工具调用循环最大迭代次数
    max_iterations: usize,
}

impl std::fmt::Debug for QueryEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryEngine")
            .field("model", &self.model)
            .finish()
    }
}

impl QueryEngine {
    fn composed_system_prompt_for_user(
        &self,
        override_prompt: Option<&str>,
        user_message: &str,
        history: Option<&[Message]>,
    ) -> String {
        let base = override_prompt.unwrap_or(&self.system_prompt);
        let assembler =
            crate::engine::prompt_context::PromptContextAssembler::from_current_dir(base);
        if let Some(hist) = history {
            assembler.build_for_turn(user_message, hist).system_prompt
        } else {
            assembler
                .build_for_single_user_message(user_message)
                .system_prompt
        }
    }

    /// 创建新的查询引擎
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            tool_registry,
            model: model.into(),
            system_prompt: super::default_system_prompt(),
            agent_manager: None,
            task_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            mcp_manager: None,
            cost_tracker: Arc::new(tokio::sync::Mutex::new(
                crate::cost_tracker::CostTracker::new(),
            )),
            max_iterations: 10,
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

    /// 获取当前累计成本（美元）
    pub async fn estimated_cost_usd(&self) -> f64 {
        self.cost_tracker.lock().await.estimated_cost_usd
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

    /// 设置 Agent 管理器
    pub fn with_agent_manager(mut self, manager: Arc<crate::agent::AgentManager>) -> Self {
        self.agent_manager = Some(manager);
        self
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

    /// 设置 MCP 管理器
    pub fn with_mcp_manager(mut self, manager: Arc<crate::engine::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    /// 设置工具调用循环最大迭代次数
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// 创建统一对话循环
    #[allow(dead_code)]
    fn create_loop(&self) -> super::conversation_loop::ConversationLoop {
        self.create_loop_with_allowed_tools(None)
    }

    /// 执行单次查询（无工具）
    pub async fn query_simple(&self, user_message: impl Into<String>) -> Result<String> {
        let user_message = user_message.into();
        let system_prompt = self.composed_system_prompt_for_user(None, &user_message, None);
        let messages = vec![Message::system(system_prompt), Message::user(user_message)];

        let request = ChatRequest::new(&self.model).with_messages(messages);

        let response = self
            .provider
            .chat(request)
            .await
            .context("Failed to get response from LLM")?;

        // 记录成本
        if let Some(ref usage) = response.usage {
            let mut tracker = self.cost_tracker.lock().await;
            tracker.record_api_call(
                &self.model,
                usage.prompt_tokens as u64,
                usage.completion_tokens as u64,
                usage.cached_tokens.map(|t| t as u64),
            );
        }

        Ok(response.content)
    }

    /// 执行带工具调用的查询
    pub async fn query_with_tools(
        &self,
        user_message: impl Into<String>,
        options: QueryOptions,
    ) -> Result<QueryResult> {
        self.query_with_tools_with_system_prompt(user_message, options, None)
            .await
    }

    /// 执行带工具调用的查询（可覆盖 system prompt，用于子 Agent 等场景）
    pub async fn query_with_tools_with_system_prompt(
        &self,
        user_message: impl Into<String>,
        options: QueryOptions,
        system_prompt_override: Option<&str>,
    ) -> Result<QueryResult> {
        let user_msg = user_message.into();
        let preview: String = user_msg.chars().take(50).collect();
        info!("Starting query with tools: {}", preview);
        let context_ref = options.context_messages.as_deref();
        let working_dir = options.working_dir.clone();

        // 构建消息
        let system_prompt =
            self.composed_system_prompt_for_user(system_prompt_override, &user_msg, context_ref);
        let mut messages = vec![Message::system(system_prompt)];
        if let Some(context_messages) = options.context_messages {
            messages.extend(context_messages);
        }
        messages.push(Message::user(user_msg));

        // 委托给统一对话循环（非流式）
        let mut lp = self
            .create_loop_with_allowed_tools(options.allowed_tools.clone())
            .with_max_iterations(options.max_tool_iterations.unwrap_or(self.max_iterations));
        if let Some(working_dir) = working_dir {
            lp = lp.with_working_dir(working_dir);
        }
        let result = lp.run(messages).await?;

        Ok(QueryResult {
            content: result.content,
            iterations: result.iterations,
            tool_calls_made: result.tool_calls_made,
        })
    }
}

impl QueryEngine {
    fn create_loop_with_allowed_tools(
        &self,
        allowed_tools: Option<Vec<String>>,
    ) -> super::conversation_loop::ConversationLoop {
        let mut builder = super::ConversationLoopBuilder::new(
            self.provider.clone(),
            self.tool_registry.clone(),
            self.cost_tracker.clone(),
            &self.model,
        )
        .with_max_iterations(self.max_iterations)
        .with_compression(128_000);

        if let Some(ref manager) = self.agent_manager {
            builder = builder.with_agent_manager(manager.clone());
        }
        if let Some(ref lsp_manager) = self.lsp_manager {
            builder = builder.with_lsp_manager(lsp_manager.clone());
        }
        if let Some(ref worktree_manager) = self.worktree_manager {
            builder = builder.with_worktree_manager(worktree_manager.clone());
        }
        if let Some(ref mcp_manager) = self.mcp_manager {
            builder = builder.with_mcp_manager(mcp_manager.clone());
        }
        if let Some(allowed) = allowed_tools {
            let set: HashSet<String> = allowed.into_iter().collect();
            builder = builder.with_allowed_tools(set);
        }

        builder.build()
    }
}

/// 查询选项
pub struct QueryOptions {
    /// 最大工具调用迭代次数
    pub max_tool_iterations: Option<usize>,
    /// 上下文消息（用于多轮对话）
    pub context_messages: Option<Vec<Message>>,
    /// 温度参数
    pub temperature: Option<f32>,
    /// 允许的工具白名单（None 表示不限制）
    pub allowed_tools: Option<Vec<String>>,
    /// Optional working directory override for isolated agent/worktree runs.
    pub working_dir: Option<PathBuf>,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            max_tool_iterations: Some(10),
            context_messages: None,
            temperature: Some(0.2),
            allowed_tools: None,
            working_dir: None,
        }
    }
}

impl QueryOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_tool_iterations = Some(max);
        self
    }

    pub fn with_context(mut self, messages: Vec<Message>) -> Self {
        self.context_messages = Some(messages);
        self
    }

    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    pub fn with_working_dir(mut self, working_dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(working_dir.into());
        self
    }
}

/// 查询结果
pub struct QueryResult {
    /// 响应内容
    pub content: String,
    /// 迭代次数
    pub iterations: usize,
    /// 是否执行了工具调用
    pub tool_calls_made: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_options() {
        let opts = QueryOptions::new()
            .with_max_iterations(5)
            .with_working_dir("/tmp/isolated-agent");

        assert_eq!(opts.max_tool_iterations, Some(5));
        assert_eq!(opts.working_dir, Some(PathBuf::from("/tmp/isolated-agent")));
    }

    #[test]
    fn test_default_system_prompt() {
        let prompt = crate::engine::default_system_prompt();
        assert!(prompt.contains("Priority Agent"));
        assert!(prompt.contains("file_read"));
        assert!(prompt.contains("Model-Led Programming Workflow"));
        assert!(prompt.contains("acceptance criteria"));
        assert!(prompt.contains("Verify changed behavior"));
    }

    #[test]
    fn default_system_prompt_stays_under_runtime_diet_budget() {
        let prompt = crate::engine::default_system_prompt();
        let tokens = crate::engine::context_compressor::estimate_tokens(&prompt);
        assert!(
            tokens <= 700,
            "default system prompt grew past runtime diet budget: {tokens} tokens"
        );
    }

    #[test]
    fn default_system_prompt_keeps_repair_details_out_of_always_on_context() {
        let prompt = crate::engine::default_system_prompt();
        assert!(!prompt.contains("\"old_string\""));
        assert!(!prompt.contains("\"line_start\""));
        assert!(!prompt.contains("EXACT string matching"));
        assert!(!prompt.contains("Tool Usage Best Practices"));
    }
}
