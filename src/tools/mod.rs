//! 工具系统模块
//!
//! 模仿 Claude Code 的 Tool 系统架构
//! 每个工具实现 Tool trait，可以被执行、有输入输出、支持权限检查

pub mod agent_tool;
pub mod ask_tool;
pub mod bash_tool;
pub mod browser_tool;
pub mod cache;
pub mod calculate_tool;
pub mod datetime_tool;
pub mod diff_tool;
pub mod encode_tool;
pub mod file_cache;
pub mod file_tool;
pub mod format_tool;
pub mod git_tool;
pub mod github_tool;
pub mod glob_tool;
pub mod grep_tool;
pub mod json_tool;
pub mod lsp_tool;
pub mod mcp_tool;
pub mod memory_tool;
pub mod notebook_tool;
pub mod plan_mode_tool;
pub mod plugin_tool;
pub mod powershell_tool;
pub mod project_tool;
pub mod refactor_tool;
pub mod remote_dev_tool;
pub mod remote_trigger_tool;
pub mod repl_tool;
pub mod send_message_tool;
pub mod share_tool;
pub mod sleep_tool;
pub mod symbol_tool;
pub mod task_tool;
pub mod team_tool;
pub mod telemetry_tool;
pub mod todo_tool;
pub mod voice_tool;
pub mod tool_search_tool;
pub mod web_tools;
pub mod workbench_tool;
pub mod worktree_tool;

#[cfg(test)]
mod examples;

pub use agent_tool::AgentTool;
pub use bash_tool::BashTool;
pub use browser_tool::BrowserTool;
pub use calculate_tool::CalculateTool;
pub use datetime_tool::DatetimeTool;
pub use diff_tool::DiffTool;
pub use encode_tool::EncodeTool;
pub use file_tool::{FileEditTool, FileReadTool, FileWriteTool};
pub use format_tool::FormatTool;
pub use git_tool::GitTool;
pub use github_tool::GitHubTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
pub use json_tool::JsonQueryTool;
pub use lsp_tool::LSPTool;
pub use mcp_tool::{ListMcpResourcesTool, MCPTool, McpAuthTool, ReadMcpResourceTool};
pub use memory_tool::{MemoryClearTool, MemoryLoadTool, MemorySaveTool};
pub use notebook_tool::NotebookTool;
pub use plan_mode_tool::{EnterPlanModeTool, ExitPlanModeTool};
pub use plugin_tool::{PluginListTool, PluginManageTool};
pub use powershell_tool::PowerShellTool;
pub use refactor_tool::RefactorTool;
pub use remote_dev_tool::RemoteDevTool;
pub use remote_trigger_tool::RemoteTriggerTool;
pub use repl_tool::REPLTool;
pub use send_message_tool::SendMessageTool;
pub use share_tool::ShareTool;
pub use sleep_tool::SleepTool;
pub use symbol_tool::SymbolQueryTool;
pub use task_tool::{
    TaskCreateTool, TaskGetTool, TaskListTool, TaskOutputTool, TaskStopTool, TaskUpdateTool,
};
pub use team_tool::TeamTool;
pub use telemetry_tool::TelemetryTool;
pub use todo_tool::TodoWriteTool;
pub use voice_tool::VoiceTool;
pub use tool_search_tool::ToolSearchTool;
pub use web_tools::{WebFetchTool, WebSearchTool};
pub use workbench_tool::WorkbenchTool;
pub use worktree_tool::WorktreeTool;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// 工具 trait - 所有工具必须实现
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述
    fn description(&self) -> &str;

    /// 工具参数 JSON Schema
    fn parameters(&self) -> Value;

    /// 执行工具
    async fn execute(&self, params: Value, context: ToolContext) -> ToolResult;

    /// 是否需要用户确认
    fn requires_confirmation(&self, _params: &Value) -> bool {
        false
    }

    /// 获取确认提示信息
    fn confirmation_prompt(&self, _params: &Value) -> Option<String> {
        None
    }
}

/// 工具执行上下文
#[derive(Clone)]
pub struct ToolContext {
    // ── 核心字段 ──
    /// 当前工作目录
    pub working_dir: std::path::PathBuf,
    /// 会话 ID
    pub session_id: String,
    /// 当前模型名称
    pub model: String,

    // ── 权限 ──
    /// 用户设置（如是否总是允许某类操作）
    pub permissions: ToolPermissions,
    /// 权限上下文（细粒度权限控制）
    pub permission_context: crate::permissions::PermissionContext,

    // ── 额外数据 ──
    /// 额外上下文数据
    pub metadata: HashMap<String, String>,

    // ── 子系统管理器（按需注入） ──
    /// LLM Provider（socratic_analyze、swarm 等需要调用 LLM 的工具）
    pub llm_provider: Option<std::sync::Arc<dyn crate::services::api::LlmProvider>>,
    /// Agent 管理器（agent_tool、send_message_tool 创建子 Agent）
    pub agent_manager: Option<std::sync::Arc<crate::agent::AgentManager>>,
    /// MCP 管理器（mcp_tool 调用外部 MCP 工具）
    pub mcp_manager: Option<std::sync::Arc<crate::engine::mcp::McpManager>>,
    /// LSP 管理器（lsp_tool 查询语言服务器）
    pub lsp_manager: Option<std::sync::Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器（worktree_tool 管理 git worktree）
    pub worktree_manager: Option<std::sync::Arc<crate::engine::worktree::WorktreeManager>>,
    /// Task 管理器（task_tool 创建和管理任务）
    pub task_manager: Option<std::sync::Arc<crate::task_manager::TaskManager>>,
    /// 文件状态缓存（file_read/file_edit 优化与变更检测）
    pub file_cache: Option<std::sync::Arc<crate::tools::file_cache::FileStateCache>>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("working_dir", &self.working_dir)
            .field("session_id", &self.session_id)
            .field("model", &self.model)
            .field("permissions", &self.permissions)
            .field("metadata", &self.metadata)
            .field(
                "llm_provider",
                &self.llm_provider.as_ref().map(|_| "<LlmProvider>"),
            )
            .field(
                "agent_manager",
                &self.agent_manager.as_ref().map(|_| "<AgentManager>"),
            )
            .field(
                "mcp_manager",
                &self.mcp_manager.as_ref().map(|_| "<McpManager>"),
            )
            .field(
                "lsp_manager",
                &self.lsp_manager.as_ref().map(|_| "<LspManager>"),
            )
            .field(
                "worktree_manager",
                &self.worktree_manager.as_ref().map(|_| "<WorktreeManager>"),
            )
            .field(
                "task_manager",
                &self.task_manager.as_ref().map(|_| "<TaskManager>"),
            )
            .field(
                "file_cache",
                &self.file_cache.as_ref().map(|_| "<FileStateCache>"),
            )
            .finish()
    }
}

impl ToolContext {
    pub fn new(working_dir: impl Into<std::path::PathBuf>, session_id: impl Into<String>) -> Self {
        let wd = working_dir.into();
        Self {
            permission_context: crate::permissions::PermissionContext::new(&wd),
            working_dir: wd,
            session_id: session_id.into(),
            model: String::new(),
            permissions: ToolPermissions::default(),
            metadata: HashMap::new(),
            llm_provider: None,
            agent_manager: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            task_manager: None,
            file_cache: None,
        }
    }

    /// 设置权限模式
    pub fn with_permission_mode(mut self, mode: crate::permissions::PermissionMode) -> Self {
        self.permission_context.mode = mode;
        self
    }

    /// 设置 Agent 管理器
    pub fn with_agent_manager(
        mut self,
        manager: std::sync::Arc<crate::agent::AgentManager>,
    ) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    /// 设置 LLM Provider
    pub fn with_llm_provider(
        mut self,
        provider: std::sync::Arc<dyn crate::services::api::LlmProvider>,
    ) -> Self {
        self.llm_provider = Some(provider);
        self
    }

    /// 设置模型名称
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// 设置 MCP 管理器
    pub fn with_mcp_manager(
        mut self,
        manager: std::sync::Arc<crate::engine::mcp::McpManager>,
    ) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    /// 设置 LSP 管理器
    pub fn with_lsp_manager(
        mut self,
        manager: std::sync::Arc<crate::engine::lsp::LspManager>,
    ) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    /// 设置 Worktree 管理器
    pub fn with_worktree_manager(
        mut self,
        manager: std::sync::Arc<crate::engine::worktree::WorktreeManager>,
    ) -> Self {
        self.worktree_manager = Some(manager);
        self
    }

    /// 设置 Task 管理器
    pub fn with_task_manager(
        mut self,
        manager: std::sync::Arc<crate::task_manager::TaskManager>,
    ) -> Self {
        self.task_manager = Some(manager);
        self
    }

    /// 设置文件状态缓存
    pub fn with_file_cache(
        mut self,
        cache: std::sync::Arc<crate::tools::file_cache::FileStateCache>,
    ) -> Self {
        self.file_cache = Some(cache);
        self
    }
}

/// 工具权限设置
#[derive(Debug, Clone, Default)]
pub struct ToolPermissions {
    /// 总是允许读文件
    pub allow_all_reads: bool,
    /// 总是允许写文件
    pub allow_all_writes: bool,
    /// 总是允许执行命令
    pub allow_all_bash: bool,
    /// 只读模式（禁止任何写入）
    pub read_only: bool,
}

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 是否成功
    pub success: bool,
    /// 输出内容
    pub content: String,
    /// 错误信息（如果有）
    pub error: Option<String>,
    /// 额外数据（JSON 格式）
    pub data: Option<Value>,
    /// 执行耗时（毫秒）
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

impl ToolResult {
    /// 创建成功结果
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            error: None,
            data: None,
            duration_ms: None,
        }
    }

    /// 创建带数据的成功结果
    pub fn success_with_data(content: impl Into<String>, data: Value) -> Self {
        Self {
            success: true,
            content: content.into(),
            error: None,
            data: Some(data),
            duration_ms: None,
        }
    }

    /// 创建失败结果
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            content: String::new(),
            error: Some(error.into()),
            data: None,
            duration_ms: None,
        }
    }

    /// 创建带内容的失败结果
    pub fn error_with_content(error: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            success: false,
            content: content.into(),
            error: Some(error.into()),
            data: None,
            duration_ms: None,
        }
    }
}
/// 工具注册表
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    ask_channel: Option<Arc<ask_tool::AskChannel>>,
}

impl ToolRegistry {
    /// 创建空注册表
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            ask_channel: None,
        }
    }

    /// 注册工具
    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        self.tools.insert(name, Box::new(tool));
    }

    /// 获取工具
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// 检查工具是否存在
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 获取所有工具名称
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(|k| k.as_str()).collect()
    }

    /// 获取用户问答通道
    pub fn ask_channel(&self) -> Option<Arc<ask_tool::AskChannel>> {
        self.ask_channel.clone()
    }

    /// 创建默认注册表（包含所有标准工具）
    pub fn default_registry() -> Self {
        let mut registry = Self::new();

        // 注册文件工具
        registry.register(FileReadTool);
        registry.register(FileWriteTool);
        registry.register(FileEditTool);

        // 注册搜索工具
        registry.register(GlobTool);
        registry.register(GrepTool);

        // 注册系统工具
        registry.register(BashTool);

        // 注册高级工具
        let task_manager = crate::task_manager::GLOBAL_TASK_MANAGER.clone();
        registry.register(TaskCreateTool::new(task_manager.clone()));
        registry.register(TaskGetTool::new(task_manager.clone()));
        registry.register(TaskListTool::new(task_manager.clone()));
        registry.register(TaskUpdateTool::new(task_manager.clone()));
        registry.register(TaskStopTool::new(task_manager.clone()));
        registry.register(TaskOutputTool::new(task_manager.clone()));
        registry.register(AgentTool);

        // 注册新增工具
        registry.register(WebFetchTool);
        registry.register(WebSearchTool);
        registry.register(MemorySaveTool);
        registry.register(MemoryLoadTool);
        registry.register(MemoryClearTool);
        registry.register(TodoWriteTool);

        // 注册新添加的工具
        registry.register(CalculateTool);
        registry.register(DatetimeTool);
        registry.register(JsonQueryTool);
        registry.register(EncodeTool);
        registry.register(DiffTool);
        registry.register(FormatTool);
        registry.register(GitHubTool);
        registry.register(GitTool);
        registry.register(NotebookTool);
        registry.register(REPLTool);
        registry.register(PowerShellTool);
        registry.register(SendMessageTool);
        registry.register(ShareTool);
        registry.register(ToolSearchTool);
        registry.register(SleepTool);
        let plan_manager = crate::engine::plan_mode::GLOBAL_PLAN_MANAGER.clone();
        registry.register(EnterPlanModeTool::new(plan_manager.clone()));
        registry.register(ExitPlanModeTool::new(plan_manager.clone()));

        // 注册核心引擎工具
        registry.register(crate::engine::socratic::SocraticTool);
        registry.register(crate::engine::cron::CronTool);
        registry.register(crate::engine::swarm::SwarmTool);
        registry.register(crate::engine::mcp::McpManageTool);
        registry.register(MCPTool);
        registry.register(McpAuthTool);
        registry.register(ListMcpResourcesTool);
        registry.register(ReadMcpResourceTool);
        registry.register(LSPTool);
        registry.register(SymbolQueryTool);
        registry.register(WorktreeTool);
        registry.register(WorkbenchTool);
        registry.register(RemoteTriggerTool);
        registry.register(RemoteDevTool);
        registry.register(BrowserTool);
        registry.register(TeamTool);
        registry.register(VoiceTool);
        registry.register(TelemetryTool);
        registry.register(PluginListTool);
        registry.register(PluginManageTool);
        registry.register(RefactorTool);
        registry.register(project_tool::ProjectListTool);
        // Skills 工具
        let skills_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".priority-agent")
            .join("skills");
        registry.register(crate::skills::SkillManageTool::new(skills_dir));
        registry.register(crate::skills::SkillListTool);
        registry.register(crate::skills::SkillViewTool);

        // 注册需要通道的工具
        let ask_channel = std::sync::Arc::new(ask_tool::AskChannel::new());
        registry.ask_channel = Some(ask_channel.clone());
        registry.register(ask_tool::AskUserQuestionTool::new(ask_channel));

        registry.register(crate::engine::plan_mode::PlanTool::new(
            crate::engine::plan_mode::GLOBAL_PLAN_MANAGER.clone(),
        ));

        registry
    }

    /// 遍历所有工具
    pub fn iter_tools(&self) -> impl Iterator<Item = &dyn Tool> {
        self.tools.values().map(|t| t.as_ref())
    }

    /// 转换为 OpenAI 工具格式
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn to_openai_tools(&self) -> Vec<async_openai::types::ChatCompletionTool> {
        use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};

        self.iter_tools()
            .map(|tool| ChatCompletionTool {
                r#type: ChatCompletionToolType::Function,
                function: FunctionObject {
                    name: tool.name().to_string(),
                    description: Some(tool.description().to_string()),
                    parameters: Some(tool.parameters()),
                    strict: None,
                },
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 带缓存的工具执行器
pub struct CachedToolExecutor {
    registry: ToolRegistry,
    cache: cache::ToolResultCache,
}

impl CachedToolExecutor {
    /// 创建新的带缓存执行器
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry,
            cache: cache::ToolResultCache::new(),
        }
    }

    /// 使用指定缓存创建
    pub fn with_cache(registry: ToolRegistry, cache: cache::ToolResultCache) -> Self {
        Self { registry, cache }
    }

    /// 执行工具（带缓存）
    pub async fn execute(
        &self,
        tool_name: &str,
        params: Value,
        context: ToolContext,
    ) -> Option<ToolResult> {
        let working_dir = context.working_dir.to_string_lossy().to_string();

        // 尝试从缓存获取
        if let Some(cached_result) = self.cache.get(tool_name, &params, &working_dir) {
            // 将缓存的 Value 转回 ToolResult
            return Some(ToolResult::from_cached_value(cached_result));
        }

        // 执行工具
        let tool = self.registry.get(tool_name)?;
        let result = tool.execute(params.clone(), context).await;

        // 如果成功，缓存结果
        if result.success {
            if let Ok(value) = serde_json::to_value(&result) {
                self.cache.set(tool_name, params, &working_dir, value);
            }
        }

        Some(result)
    }

    /// 获取缓存引用
    pub fn cache(&self) -> &cache::ToolResultCache {
        &self.cache
    }

    /// 获取缓存统计
    pub fn cache_stats(&self) -> cache::CacheStats {
        self.cache.stats()
    }

    /// 清空缓存
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// 使特定工具缓存失效
    pub fn invalidate_tool_cache(&self, tool_name: &str) {
        self.cache.invalidate_tool(tool_name);
    }
}

impl ToolResult {
    /// 从缓存值重建 ToolResult
    fn from_cached_value(value: Value) -> Self {
        // 尝试从缓存的 JSON 重建
        if let Ok(result) = serde_json::from_value::<ToolResult>(value.clone()) {
            return result;
        }

        // 如果反序列化失败，创建一个通用的成功结果
        let content = value
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("Cached result")
            .to_string();

        let success = value
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let error = value
            .get("error")
            .and_then(|v| v.as_str())
            .map(String::from);

        let data = value.get("data").cloned();

        Self {
            success,
            content,
            error,
            data,
            duration_ms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result() {
        let success = ToolResult::success("Done");
        assert!(success.success);
        assert_eq!(success.content, "Done");

        let error = ToolResult::error("Failed");
        assert!(!error.success);
        assert_eq!(error.error, Some("Failed".to_string()));
    }

    #[test]
    fn test_tool_registry() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool);

        assert!(registry.has("bash"));
        assert!(!registry.has("nonexistent"));
    }

    /// 一致性测试：确保所有核心工具在默认注册表中可用
    /// 防止"文档写了有，模型调不到"的问题
    #[test]
    fn test_all_core_tools_registered() {
        let registry = ToolRegistry::default_registry();
        let registered = registry.tool_names();

        let expected_core = [
            "file_read",
            "file_write",
            "file_edit",
            "glob",
            "grep",
            "bash",
            "task_create",
            "task_get",
            "task_list",
            "task_update",
            "task_stop",
            "task_output",
            "agent",
            "web_fetch",
            "web_search",
            "memory_save",
            "memory_load",
            "memory_clear",
            "todo_write",
            "calculate",
            "datetime",
            "json_query",
            "encode",
            "diff",
            "format",
            "git",
            "notebook",
            "repl",
            "powershell",
            "enter_plan_mode",
            "exit_plan_mode",
            "send_message",
            "tool_search",
            "sleep",
            "socratic_analyze",
            "cron",
            "swarm",
            "mcp",
            "mcp_tool",
            "mcp_auth",
            "list_mcp_resources",
            "read_mcp_resource",
            "lsp",
            "symbol_query",
            "worktree",
            "workbench",
            "remote_trigger",
            "project_list",
            "refactor",
            "skill_manage",
            "skills_list",
            "skill_view",
            "ask_user",
            "plan",
        ];

        for &name in &expected_core {
            assert!(
                registered.contains(&name),
                "Core tool '{}' NOT in default_registry! Models can't call it.",
                name
            );
        }
    }

    /// 工具数量不能回退
    #[test]
    fn test_tool_count_not_regressed() {
        let registry = ToolRegistry::default_registry();
        let count = registry.tool_names().len();
        assert!(
            count >= 50,
            "Tool count regressed! Expected >= 50, got {}",
            count
        );
    }
}
