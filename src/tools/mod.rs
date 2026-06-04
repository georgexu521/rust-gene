//! 工具系统模块
//!
//! 模仿 Claude Code 的 Tool 系统架构
//! 每个工具实现 Tool trait，可以被执行、有输入输出、支持权限检查
//!
//! 核心类型已拆分为独立子模块：
//! - `result` — ToolErrorCode, ToolPermissionLevel, ToolResult
//! - `schema` — ToolOperationKind, ToolInterruptBehavior, ToolSchema 等
//! - `tool_trait` — Tool trait, ToolContext, JSON 参数校验
//! - `registry` — ToolRegistry, ToolRegistryProfile, CachedToolExecutor

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
pub mod git_read_tool;
pub mod git_tool;
pub mod github_tool;
pub mod glob_tool;
pub mod grep_tool;
pub mod install_dependencies_tool;
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
pub mod registry;
pub mod reliability;
pub mod remote_dev_tool;
pub mod remote_trigger_tool;
pub mod repl_tool;
pub mod result;
pub mod resume_tool;
pub mod rewind_tool;
pub mod run_tests_tool;
pub mod schema;
pub mod send_message_tool;
pub mod share_tool;
pub mod sleep_tool;
pub mod start_dev_server_tool;
pub mod symbol_tool;
pub mod task_tool;
pub mod team_tool;
pub mod telemetry_tool;
pub mod todo_tool;
pub mod tool_search_tool;
pub mod tool_trait;
#[cfg(feature = "voice")]
pub mod voice_tool;
pub mod web_tools;
pub mod workbench_tool;
pub mod worktree_tool;

#[cfg(test)]
mod examples;

// ── 核心类型重导出 ──
pub use registry::{CachedToolExecutor, ToolRegistry, ToolRegistryProfile};
pub use result::{ToolErrorCode, ToolPermissionLevel, ToolResult};
pub use schema::{
    ToolInterruptBehavior, ToolOperationKind, ToolSchema, ToolSearchOrReadSemantics,
    ToolUiRenderKind,
};
pub use tool_trait::{
    Tool, ToolContext, ToolContextRetainedContext, ToolContextRetentionItem,
    ToolContextSkillTrigger, ToolPermissions,
};

// ── 工具重导出 ──
pub use agent_tool::AgentTool;
pub use bash_tool::{BashCancelTool, BashOutputTool, BashTasksTool, BashTool};
pub use browser_tool::BrowserTool;
pub use calculate_tool::CalculateTool;
pub use context_tool::ContextTool;
pub use context_vis_tool::ContextVisTool;
pub use copy_tool::CopyTool;
pub use datetime_tool::DatetimeTool;
pub use desktop_tool::DesktopTool;
pub use diff_tool::DiffTool;
pub use encode_tool::EncodeTool;
pub use file_tool::{FileEditTool, FilePatchTool, FileReadTool, FileWriteTool};
pub use format_tool::FormatTool;
pub use git_read_tool::{GitDiffTool, GitStatusTool};
pub use git_tool::GitTool;
pub use github_tool::GitHubTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
pub use install_dependencies_tool::InstallDependenciesTool;
pub use json_tool::JsonQueryTool;
pub use lsp_tool::LSPTool;
pub use mcp_tool::{ListMcpResourcesTool, MCPTool, McpAuthTool, ReadMcpResourceTool};
pub use memory_tool::{MemoryClearTool, MemoryLoadTool, MemorySaveTool};
pub use notebook_tool::NotebookTool;
pub use plan_mode_tool::{EnterPlanModeTool, ExitPlanModeTool};
pub use plugin_tool::{PluginListTool, PluginManageTool};
pub use powershell_tool::PowerShellTool;
pub use refactor_tool::RefactorTool;
pub use reliability::{
    audit_release_tool_contracts, representative_tool_samples, ToolReliabilityIssue,
    ToolReliabilityProfile, ToolReliabilitySample,
};
pub use remote_dev_tool::RemoteDevTool;
pub use remote_trigger_tool::RemoteTriggerTool;
pub use repl_tool::REPLTool;
pub use resume_tool::ResumeTool;
pub use rewind_tool::RewindTool;
pub use run_tests_tool::RunTestsTool;
pub use send_message_tool::SendMessageTool;
pub use share_tool::ShareTool;
pub use sleep_tool::SleepTool;
pub use start_dev_server_tool::StartDevServerTool;
pub use symbol_tool::SymbolQueryTool;
pub use team_tool::TeamTool;
pub use telemetry_tool::TelemetryTool;
pub use todo_tool::TodoWriteTool;
pub use tool_search_tool::ToolSearchTool;
#[cfg(feature = "voice")]
pub use voice_tool::VoiceTool;
pub use web_tools::{WebFetchTool, WebSearchTool};
pub use workbench_tool::WorkbenchTool;
pub use worktree_tool::WorktreeTool;

#[cfg(test)]
mod tests;
