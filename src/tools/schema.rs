//! 工具元数据类型
//!
//! 从 `tools/mod.rs` 拆分出来的 ToolSchema、ToolOperationKind 等元数据。

use super::result::ToolPermissionLevel;
use super::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool operation semantics used by the runtime scheduler, permission layer,
/// and evidence ledger. This is intentionally coarse: tools can keep their
/// existing prompt/schema surface while the runtime makes Claude-like decisions
/// from stable machine-readable facts instead of name-only heuristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolOperationKind {
    Read,
    Search,
    List,
    Write,
    Edit,
    Patch,
    Shell,
    Task,
    Network,
    #[default]
    Other,
}

/// How to handle user interruption while a tool is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolInterruptBehavior {
    #[default]
    Block,
    Cancel,
}

/// Compact UI/search semantics for read-like tool calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolSearchOrReadSemantics {
    pub is_search: bool,
    pub is_read: bool,
    pub is_list: bool,
}

/// Preferred rendering lane for tool rows and future TUI panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolUiRenderKind {
    #[default]
    Generic,
    File,
    Shell,
    Search,
    Task,
    Network,
    Mcp,
    Diff,
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
    /// Backward-compatible tool names.
    pub aliases: Vec<String>,
    /// Keyword hint for deferred tool search.
    pub search_hint: Option<String>,
    /// Whether the tool should be hidden behind tool_search when supported.
    pub should_defer: bool,
    /// Whether the tool must always be present in the initial schema list.
    pub always_load: bool,
    /// Whether providers that support strict tool schemas should enable it.
    pub strict_schema: bool,
    /// Interrupt behavior for long-running invocations.
    pub interrupt_behavior: ToolInterruptBehavior,
    /// Whether a real user interaction is part of the tool contract.
    pub requires_user_interaction: bool,
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
            output_schema: tool.output_schema(),
            aliases: tool
                .aliases()
                .iter()
                .map(|alias| alias.to_string())
                .collect(),
            search_hint: tool.search_hint().map(str::to_string),
            should_defer: tool.should_defer(),
            always_load: tool.always_load(),
            strict_schema: tool.strict_schema(),
            interrupt_behavior: tool.interrupt_behavior(),
            requires_user_interaction: tool.requires_user_interaction(),
        }
    }
}
