//! 工具系统模块
//!
//! 模仿 Claude Code 的 Tool 系统架构
//! 每个工具实现 Tool trait，可以被执行、有输入输出、支持权限检查

pub mod agent_tool;
pub mod ask_tool;
pub mod bash_tool;
pub mod brief_tool;
pub mod browser_tool;
pub mod cache;
pub mod calculate_tool;
pub mod clear_tool;
pub mod config_tool;
pub mod context_tool;
pub mod context_vis_tool;
pub mod copy_tool;
pub mod cost_tool;
pub mod datetime_tool;
pub mod desktop_tool;
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
pub mod resume_tool;
pub mod rewind_tool;
pub mod send_message_tool;
pub mod share_tool;
pub mod sleep_tool;
pub mod symbol_tool;
pub mod task_tool;
pub mod team_tool;
pub mod telemetry_tool;
pub mod todo_tool;
pub mod tool_search_tool;
#[cfg(feature = "voice")]
pub mod voice_tool;
pub mod web_tools;
pub mod workbench_tool;
pub mod worktree_tool;

#[cfg(test)]
mod examples;

pub use agent_tool::AgentTool;
pub use bash_tool::{BashCancelTool, BashOutputTool, BashTool};
pub use brief_tool::BriefTool;
pub use browser_tool::BrowserTool;
pub use calculate_tool::CalculateTool;
pub use clear_tool::ClearTool;
pub use config_tool::ConfigTool;
pub use context_tool::ContextTool;
pub use context_vis_tool::ContextVisTool;
pub use copy_tool::CopyTool;
pub use cost_tool::CostTool;
pub use datetime_tool::DatetimeTool;
pub use desktop_tool::DesktopTool;
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
pub use resume_tool::ResumeTool;
pub use rewind_tool::RewindTool;
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
pub use tool_search_tool::ToolSearchTool;
#[cfg(feature = "voice")]
pub use voice_tool::VoiceTool;
pub use web_tools::{WebFetchTool, WebSearchTool};
pub use workbench_tool::WorkbenchTool;
pub use worktree_tool::WorktreeTool;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// 工具错误码
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorCode {
    /// 成功
    Success,
    /// 参数验证失败
    InvalidParams,
    /// 权限被拒绝
    PermissionDenied,
    /// 资源不存在
    NotFound,
    /// 执行超时
    Timeout,
    /// 执行失败
    ExecutionFailed,
    /// 工具不可用
    Unavailable,
    /// 取消执行
    Cancelled,
    /// 危险操作被拦截
    DangerousBlocked,
    /// 未知错误
    #[default]
    Unknown,
}

impl ToolErrorCode {
    /// 从错误信息推断错误码
    pub fn from_error(error: &str) -> Self {
        let e = error.to_ascii_lowercase();
        if e.contains("invalid param")
            || e.contains("missing required")
            || e.contains("must be of type")
        {
            ToolErrorCode::InvalidParams
        } else if e.contains("permission denied") || e.contains("denied") {
            ToolErrorCode::PermissionDenied
        } else if e.contains("not found") || e.contains("does not exist") {
            ToolErrorCode::NotFound
        } else if e.contains("timeout") || e.contains("timed out") {
            ToolErrorCode::Timeout
        } else if e.contains("dangerous") || e.contains("blocked") {
            ToolErrorCode::DangerousBlocked
        } else if e.contains("cancelled") || e.contains("canceled") {
            ToolErrorCode::Cancelled
        } else {
            ToolErrorCode::Unknown
        }
    }

    /// 获取错误码的 HTTP 状态码映射
    pub fn http_status(&self) -> u16 {
        match self {
            ToolErrorCode::Success => 200,
            ToolErrorCode::InvalidParams => 400,
            ToolErrorCode::PermissionDenied => 403,
            ToolErrorCode::NotFound => 404,
            ToolErrorCode::Timeout => 408,
            ToolErrorCode::DangerousBlocked => 451,
            ToolErrorCode::Unavailable
            | ToolErrorCode::ExecutionFailed
            | ToolErrorCode::Cancelled
            | ToolErrorCode::Unknown => 500,
        }
    }
}

/// 工具权限等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionLevel {
    /// 只读操作，不会修改任何文件或系统状态
    ReadOnly,
    /// 低风险操作，只会读取或创建临时文件
    #[default]
    LowRisk,
    /// 中等风险操作，会修改项目文件但可撤销
    MediumRisk,
    /// 高风险操作，会修改系统配置或不可撤销的操作
    HighRisk,
    /// 最高风险操作，可能影响系统安全或数据
    Critical,
}

impl ToolPermissionLevel {
    /// 从操作名称推断权限等级
    pub fn from_operation(op: &str) -> Self {
        let op_lower = op.to_ascii_lowercase();
        if op_lower.contains("read")
            || op_lower.contains("get")
            || op_lower.contains("list")
            || op_lower.contains("search")
        {
            ToolPermissionLevel::ReadOnly
        } else if op_lower.contains("write")
            || op_lower.contains("edit")
            || op_lower.contains("create")
        {
            ToolPermissionLevel::MediumRisk
        } else if op_lower.contains("delete")
            || op_lower.contains("remove")
            || op_lower.contains("kill")
        {
            ToolPermissionLevel::HighRisk
        } else if op_lower.contains("exec")
            || op_lower.contains("bash")
            || op_lower.contains("shell")
        {
            ToolPermissionLevel::Critical
        } else {
            ToolPermissionLevel::LowRisk
        }
    }

    /// 是否需要确认提示
    pub fn requires_confirmation(&self) -> bool {
        matches!(
            self,
            ToolPermissionLevel::HighRisk | ToolPermissionLevel::Critical
        )
    }
}

/// 工具元数据（schema 标准化）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolSchema {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 参数 JSON Schema
    pub parameters: Value,
    /// 错误码定义
    pub error_codes: Vec<String>,
    /// 权限等级
    pub permission_level: ToolPermissionLevel,
    /// 是否幂等（相同参数重复执行结果相同）
    pub is_idempotent: bool,
    /// 是否可重试
    pub is_retryable: bool,
    /// 预估执行时间（毫秒）
    pub estimated_duration_ms: Option<u64>,
    /// 输入定义
    pub input_schema: Option<Value>,
    /// 输出定义
    pub output_schema: Option<Value>,
}

impl ToolSchema {
    /// 从 Tool trait 获取 schema
    pub fn from_tool(tool: &dyn Tool) -> Self {
        Self {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            parameters: tool.parameters(),
            error_codes: tool.error_codes(),
            permission_level: tool.permission_level(),
            is_idempotent: tool.is_idempotent(),
            is_retryable: tool.is_retryable(),
            estimated_duration_ms: tool.estimated_duration_ms(),
            input_schema: None,
            output_schema: None,
        }
    }
}

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

    /// Whether this tool is currently usable with the provided runtime context.
    ///
    /// Tools that depend on optional managers should return false here so they
    /// are not advertised to the model or command UI when the backing subsystem
    /// is not wired for the current session.
    fn is_available(&self, _context: &ToolContext) -> bool {
        true
    }

    /// Human-readable reason for an unavailable tool.
    fn unavailable_reason(&self, _context: &ToolContext) -> Option<String> {
        None
    }

    /// 是否需要用户确认
    fn requires_confirmation(&self, _params: &Value) -> bool {
        false
    }

    /// 获取确认提示信息
    fn confirmation_prompt(&self, _params: &Value) -> Option<String> {
        None
    }

    /// 为安全分类器提供精简输入
    ///
    /// 返回工具的精简参数摘要，用于 LLM 分类器 transcript。
    /// 返回空字符串表示该工具不需要分类器审查。
    fn to_classifier_input(&self, params: &Value) -> String {
        // 默认实现：返回工具名 + 参数键列表
        let keys: Vec<String> = params
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        format!("{}({})", self.name(), keys.join(", "))
    }

    /// 验证参数是否符合 schema（返回 None 表示验证通过，Some(msg) 表示错误）
    fn validate_params(&self, params: &Value) -> Option<String> {
        let schema = self.parameters();
        if let Some(obj) = schema.get("properties")?.as_object() {
            for (key, prop) in obj {
                // 检查必需字段
                if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                    if required.iter().any(|r| r.as_str() == Some(key)) && params.get(key).is_none()
                    {
                        return Some(format!("Missing required parameter: {}", key));
                    }
                }
                // 类型检查
                if let Some(value) = params.get(key) {
                    if let Some(type_str) = prop.get("type").and_then(|t| t.as_str()) {
                        let type_matches = match (type_str, value) {
                            ("any", _) => true,
                            ("null", Value::Null) => true,
                            ("boolean", Value::Bool(_)) => true,
                            ("string", Value::String(_)) => true,
                            ("array", Value::Array(_)) => true,
                            ("object", Value::Object(_)) => true,
                            ("number", Value::Number(_)) => true,
                            ("integer", Value::Number(n)) => n.is_i64() || n.is_u64(),
                            _ => false,
                        };
                        if !type_matches {
                            let actual_type = match value {
                                Value::Null => "null",
                                Value::Bool(_) => "boolean",
                                Value::Number(n) => {
                                    if n.is_i64() || n.is_u64() {
                                        "integer"
                                    } else {
                                        "number"
                                    }
                                }
                                Value::String(_) => "string",
                                Value::Array(_) => "array",
                                Value::Object(_) => "object",
                            };
                            return Some(format!(
                                "Parameter '{}' must be of type {}, got {}",
                                key, type_str, actual_type
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    /// 渲染工具结果（用于 TUI 展示）
    fn render_result(&self, result: &ToolResult) -> String {
        if result.success {
            // 成功结果：截断长输出，保留关键部分
            let content = &result.content;
            if content.len() > 2000 {
                let safe: String = content.chars().take(2000).collect();
                format!(
                    "{}\n\n[Output truncated - {} bytes total]",
                    safe,
                    content.len()
                )
            } else {
                content.clone()
            }
        } else {
            // 错误结果：显示完整错误
            result.content.clone()
        }
    }

    /// 获取支持的错误码列表
    fn error_codes(&self) -> Vec<String> {
        vec![
            "unknown".to_string(),
            "invalid_params".to_string(),
            "permission_denied".to_string(),
            "timeout".to_string(),
            "execution_failed".to_string(),
        ]
    }

    /// 获取权限等级（默认 LowRisk）
    fn permission_level(&self) -> ToolPermissionLevel {
        ToolPermissionLevel::LowRisk
    }

    /// 是否幂等（相同参数重复执行结果相同）
    fn is_idempotent(&self) -> bool {
        false
    }

    /// 是否可重试（默认可重试）
    fn is_retryable(&self) -> bool {
        true
    }

    /// 预估执行时间（毫秒）
    fn estimated_duration_ms(&self) -> Option<u64> {
        None
    }

    /// 获取工具 schema
    fn schema(&self) -> ToolSchema
    where
        Self: Sized,
    {
        ToolSchema::from_tool(self)
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
    /// 当前 turn trace（用于工具记录内部生命周期事件）
    pub trace_collector: Option<crate::engine::trace::TraceCollector>,
    /// 会话存储（用于工具持久化运行时 artifact）
    pub session_store: Option<std::sync::Arc<crate::session_store::SessionStore>>,
    /// MCP 管理器（mcp_tool 调用外部 MCP 工具）
    pub mcp_manager: Option<std::sync::Arc<crate::engine::mcp::McpManager>>,
    /// LSP 管理器（lsp_tool 查询语言服务器）
    pub lsp_manager: Option<std::sync::Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器（worktree_tool 管理 git worktree）
    pub worktree_manager: Option<std::sync::Arc<crate::engine::worktree::WorktreeManager>>,
    /// Task 管理器（task_tool 创建和管理任务）
    pub task_manager: Option<std::sync::Arc<crate::task_manager::TaskManager>>,
    /// 成本追踪器（cost_tool 查询 token 和费用统计）
    pub cost_tracker: Option<std::sync::Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>>,
    /// 文件状态缓存（file_read/file_edit 优化与变更检测）
    pub file_cache: Option<std::sync::Arc<crate::tools::file_cache::FileStateCache>>,
    /// 诊断跟踪器（用于 diagnostic tracking 功能）
    pub diagnostic_tracker: Option<std::sync::Arc<crate::engine::DiagnosticTracker>>,
    /// Checkpoint 管理器（文件修改快照）
    pub checkpoint_manager:
        Option<std::sync::Arc<tokio::sync::Mutex<crate::engine::checkpoint::CheckpointManager>>>,
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
                "trace_collector",
                &self.trace_collector.as_ref().map(|_| "<TraceCollector>"),
            )
            .field(
                "session_store",
                &self.session_store.as_ref().map(|_| "<SessionStore>"),
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
            trace_collector: None,
            session_store: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            task_manager: None,
            cost_tracker: None,
            file_cache: None,
            diagnostic_tracker: None,
            checkpoint_manager: None,
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

    /// 设置当前 turn trace collector
    pub fn with_trace_collector(mut self, trace: crate::engine::trace::TraceCollector) -> Self {
        self.trace_collector = Some(trace);
        self
    }

    /// 设置会话存储
    pub fn with_session_store(
        mut self,
        store: std::sync::Arc<crate::session_store::SessionStore>,
    ) -> Self {
        self.session_store = Some(store);
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

    /// 设置成本追踪器
    pub fn with_cost_tracker(
        mut self,
        tracker: std::sync::Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>,
    ) -> Self {
        self.cost_tracker = Some(tracker);
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

    /// 设置诊断跟踪器
    pub fn with_diagnostic_tracker(
        mut self,
        tracker: std::sync::Arc<crate::engine::DiagnosticTracker>,
    ) -> Self {
        self.diagnostic_tracker = Some(tracker);
        self
    }

    /// 设置 Checkpoint 管理器
    pub fn with_checkpoint_manager(
        mut self,
        manager: std::sync::Arc<tokio::sync::Mutex<crate::engine::checkpoint::CheckpointManager>>,
    ) -> Self {
        self.checkpoint_manager = Some(manager);
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolResult {
    /// 是否成功
    pub success: bool,
    /// 输出内容
    pub content: String,
    /// 错误信息（如果有）
    pub error: Option<String>,
    /// 错误码
    #[serde(default)]
    pub error_code: Option<ToolErrorCode>,
    /// 额外数据（JSON 格式）
    pub data: Option<Value>,
    /// 执行耗时（毫秒）
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// 工具名称（用于审计）
    #[serde(default)]
    pub tool_name: Option<String>,
}

impl ToolResult {
    /// 创建成功结果
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            error_code: Some(ToolErrorCode::Success),
            ..Default::default()
        }
    }

    /// 创建带数据的成功结果
    pub fn success_with_data(content: impl Into<String>, data: Value) -> Self {
        Self {
            success: true,
            content: content.into(),
            error_code: Some(ToolErrorCode::Success),
            data: Some(data),
            ..Default::default()
        }
    }

    /// 创建失败结果
    pub fn error(error: impl Into<String>) -> Self {
        let err_str = error.into();
        Self {
            success: false,
            error: Some(err_str.clone()),
            error_code: Some(ToolErrorCode::from_error(&err_str)),
            ..Default::default()
        }
    }

    /// 创建带内容的失败结果
    pub fn error_with_content(error: impl Into<String>, content: impl Into<String>) -> Self {
        let err_str = error.into();
        Self {
            success: false,
            content: content.into(),
            error: Some(err_str.clone()),
            error_code: Some(ToolErrorCode::from_error(&err_str)),
            ..Default::default()
        }
    }

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
            error: error.clone(),
            error_code: error.as_ref().map(|e| ToolErrorCode::from_error(e)),
            data,
            ..Default::default()
        }
    }
}
/// 工具注册表
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    ask_channel: Option<Arc<ask_tool::AskChannel>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRegistryProfile {
    Core,
    Full,
}

impl ToolRegistryProfile {
    fn from_env() -> Self {
        match std::env::var("PRIORITY_AGENT_TOOL_PROFILE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "full" | "all" | "experimental" => Self::Full,
            _ => Self::Core,
        }
    }
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

    /// 创建默认注册表。默认只暴露核心工具；需要完整实验工具面时设置
    /// `PRIORITY_AGENT_TOOL_PROFILE=full`。
    pub fn default_registry() -> Self {
        Self::with_profile(ToolRegistryProfile::from_env())
    }

    /// 创建完整注册表（包含实验性、平台相关和低频工具）。
    pub fn full_registry() -> Self {
        Self::with_profile(ToolRegistryProfile::Full)
    }

    pub fn with_profile(profile: ToolRegistryProfile) -> Self {
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
        registry.register(BashOutputTool);
        registry.register(BashCancelTool);

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

        // 注册核心辅助工具
        registry.register(CostTool);
        registry.register(BriefTool);
        registry.register(ClearTool);
        registry.register(ConfigTool);
        registry.register(ContextTool);
        registry.register(ContextVisTool);
        registry.register(CopyTool);
        registry.register(ResumeTool);
        registry.register(RewindTool);
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
        registry.register(RefactorTool);
        registry.register(project_tool::ProjectListTool);

        if matches!(profile, ToolRegistryProfile::Full) {
            registry.register(DesktopTool);
            registry.register(RemoteTriggerTool);
            registry.register(RemoteDevTool);
            registry.register(BrowserTool);
            registry.register(TeamTool);
            #[cfg(feature = "voice")]
            registry.register(VoiceTool);
            registry.register(TelemetryTool);
            registry.register(PluginListTool);
            registry.register(PluginManageTool);
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct IntegerParamTool;

    #[async_trait]
    impl Tool for IntegerParamTool {
        fn name(&self) -> &str {
            "integer_param_tool"
        }

        fn description(&self) -> &str {
            "test integer validation"
        }

        fn parameters(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "timeout": { "type": "integer" }
                },
                "required": ["timeout"]
            })
        }

        async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
            ToolResult::success("ok")
        }
    }

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
    fn test_validate_params_accepts_integer_type_for_json_number() {
        let tool = IntegerParamTool;
        let err = tool.validate_params(&json!({ "timeout": 60 }));
        assert!(
            err.is_none(),
            "integer JSON number should pass schema validation"
        );
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
            "bash_output",
            "bash_cancel",
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

    #[test]
    fn test_full_registry_includes_low_frequency_tools() {
        let registry = ToolRegistry::full_registry();
        let registered = registry.tool_names();

        for &name in &[
            "desktop",
            "remote_trigger",
            "remote_dev",
            "browser",
            "team",
            #[cfg(feature = "voice")]
            "voice",
            "telemetry",
            "plugin_list",
            "plugin_manage",
        ] {
            assert!(
                registered.contains(&name),
                "Full registry should include gated tool '{}'.",
                name
            );
        }
    }

    #[test]
    fn core_tool_contract_descriptions_stay_compact() {
        let registry = ToolRegistry::default_registry();
        let budgets = [
            ("file_read", 320usize),
            ("file_write", 360usize),
            ("file_edit", 900usize),
            ("bash", 420usize),
            ("agent", 650usize),
            ("skill_view", 260usize),
        ];

        for (name, max_chars) in budgets {
            let tool = registry.get(name).expect("core tool should be registered");
            let chars = tool.description().chars().count();
            assert!(
                chars <= max_chars,
                "tool contract for '{}' is too large: {} chars > {}. Move rare guidance into failure-specific messages.",
                name,
                chars,
                max_chars
            );
        }
    }

    /// 工具数量不能回退
    #[test]
    fn test_tool_count_not_regressed() {
        let registry = ToolRegistry::full_registry();
        let count = registry.tool_names().len();
        assert!(
            count >= 50,
            "Tool count regressed! Expected >= 50, got {}",
            count
        );
    }
}
