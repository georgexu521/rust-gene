//! 工具注册表与缓存执行器
//!
//! 从 `tools/mod.rs` 拆分出来的 ToolRegistry、ToolRegistryProfile、CachedToolExecutor。

use super::tool_trait::Tool;
use super::ToolResult;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// 工具注册表
///
/// 同时提供核心注册（Core profile）和完整注册（Full profile）。
/// 默认注册只暴露 Core 工具，需要完整工具面时设置
/// `PRIORITY_AGENT_TOOL_PROFILE=full`。
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    ask_channel: Option<Arc<super::ask_tool::AskChannel>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRegistryProfile {
    Core,
    Full,
}

impl ToolRegistryProfile {
    pub fn from_env() -> Self {
        match crate::services::config::runtime_config()
            .tool_profile()
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
        if let Some(tool) = self.tools.get(name) {
            return Some(tool.as_ref());
        }
        self.tools
            .values()
            .find(|tool| tool.aliases().iter().any(|alias| alias == &name))
            .map(|tool| tool.as_ref())
    }

    /// 检查工具是否存在
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// 获取所有工具名称
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(|k| k.as_str()).collect()
    }

    /// Build a Claude-like reliability audit for registered tools using
    /// representative inputs. The audit is side-effect free: it only calls
    /// metadata hooks, never `execute`.
    pub fn reliability_audit(&self) -> Vec<super::reliability::ToolReliabilityProfile> {
        super::reliability::audit_registry(self)
    }

    /// 获取用户问答通道
    pub fn ask_channel(&self) -> Option<Arc<super::ask_tool::AskChannel>> {
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
        use super::agent_tool::AgentTool;
        use super::ask_tool::AskUserQuestionTool;
        use super::bash_tool::{BashCancelTool, BashOutputTool, BashTasksTool, BashTool};
        use super::browser_tool::BrowserTool;
        use super::calculate_tool::CalculateTool;
        use super::context_tool::ContextTool;
        use super::context_vis_tool::ContextVisTool;
        use super::copy_tool::CopyTool;
        use super::datetime_tool::DatetimeTool;
        use super::desktop_tool::DesktopTool;
        use super::diff_tool::DiffTool;
        use super::encode_tool::EncodeTool;
        use super::file_tool::{FileEditTool, FilePatchTool, FileReadTool, FileWriteTool};
        use super::format_tool::FormatTool;
        use super::git_read_tool::{GitDiffTool, GitStatusTool};
        use super::git_tool::GitTool;
        use super::github_tool::GitHubTool;
        use super::glob_tool::GlobTool;
        use super::grep_tool::GrepTool;
        use super::install_dependencies_tool::InstallDependenciesTool;
        use super::json_tool::JsonQueryTool;
        use super::lsp_tool::LSPTool;
        use super::mcp_tool::{ListMcpResourcesTool, MCPTool, McpAuthTool, ReadMcpResourceTool};
        use super::memory_tool::{MemoryClearTool, MemoryLoadTool, MemorySaveTool};
        use super::notebook_tool::NotebookTool;
        use super::plan_mode_tool::{EnterPlanModeTool, ExitPlanModeTool};
        use super::plugin_tool::{PluginListTool, PluginManageTool};
        use super::powershell_tool::PowerShellTool;
        use super::project_tool;
        use super::refactor_tool::RefactorTool;
        use super::remote_dev_tool::RemoteDevTool;
        use super::remote_trigger_tool::RemoteTriggerTool;
        use super::repl_tool::REPLTool;
        use super::resume_tool::ResumeTool;
        use super::rewind_tool::RewindTool;
        use super::run_tests_tool::RunTestsTool;
        use super::send_message_tool::SendMessageTool;
        use super::share_tool::ShareTool;
        use super::sleep_tool::SleepTool;
        use super::start_dev_server_tool::StartDevServerTool;
        use super::symbol_tool::SymbolQueryTool;
        use super::team_tool::TeamTool;
        use super::telemetry_tool::TelemetryTool;
        use super::todo_tool::TodoWriteTool;
        use super::tool_search_tool::ToolSearchTool;
        use super::web_tools::{WebFetchTool, WebSearchTool};
        use super::workbench_tool::WorkbenchTool;
        use super::worktree_tool::WorktreeTool;

        let mut registry = Self::new();

        // Core file tools (always available)
        registry.register(FileReadTool);
        registry.register(FileWriteTool);
        registry.register(FileEditTool);
        registry.register(FilePatchTool);

        // Core search tools (always available)
        registry.register(GlobTool);
        registry.register(GrepTool);

        // Core system tools (always available)
        registry.register(BashTool);
        registry.register(BashOutputTool);
        registry.register(BashCancelTool);
        registry.register(BashTasksTool);
        registry.register(RunTestsTool);
        registry.register(StartDevServerTool);

        // Core memory tools (always available when memory is enabled)
        registry.register(MemorySaveTool);
        registry.register(MemoryLoadTool);
        registry.register(MemoryClearTool);

        // Core task management
        registry.register(TodoWriteTool);

        // Core git tools (always available)
        registry.register(GitStatusTool);
        registry.register(GitDiffTool);

        // Core context tools
        registry.register(ContextTool);
        registry.register(ContextVisTool);

        // Core utility tools
        registry.register(CalculateTool);
        registry.register(DatetimeTool);

        // Plan mode tools
        let plan_manager = crate::engine::plan_mode::GLOBAL_PLAN_MANAGER.clone();
        registry.register(EnterPlanModeTool::new(plan_manager.clone()));
        registry.register(ExitPlanModeTool::new(plan_manager.clone()));

        // Agent & communication tools (sub-agent support, always available)
        registry.register(AgentTool::new());
        registry.register(SendMessageTool);

        if matches!(profile, ToolRegistryProfile::Full) {
            // Extended file/system tools
            registry.register(InstallDependenciesTool);

            // Web tools
            registry.register(WebFetchTool);
            registry.register(WebSearchTool);

            // Extended utility tools
            registry.register(CopyTool);
            registry.register(ResumeTool);
            registry.register(RewindTool);
            registry.register(JsonQueryTool);
            registry.register(EncodeTool);
            registry.register(DiffTool);
            registry.register(FormatTool);

            // Git/GitHub tools
            registry.register(GitHubTool);
            registry.register(GitTool);

            // Notebook/REPL tools
            registry.register(NotebookTool);
            registry.register(REPLTool);
            registry.register(PowerShellTool);

            // Communication tools
            registry.register(ShareTool);
            registry.register(SleepTool);

            // Search/management tools
            registry.register(ToolSearchTool);
            registry.register(project_tool::ProjectListTool);

            // MCP tools
            registry.register(crate::engine::mcp::McpManageTool);
            registry.register(MCPTool);
            registry.register(McpAuthTool);
            registry.register(ListMcpResourcesTool);
            registry.register(ReadMcpResourceTool);

            // Engine tools
            registry.register(crate::engine::socratic::SocraticTool);
            registry.register(crate::engine::cron::CronTool);
            registry.register(crate::engine::swarm::SwarmTool);

            // LSP/Symbol tools
            registry.register(LSPTool);
            registry.register(SymbolQueryTool);

            // Worktree/Workbench tools
            registry.register(WorktreeTool);
            registry.register(WorkbenchTool);
            registry.register(RefactorTool);

            // Platform-specific tools
            registry.register(DesktopTool);
            registry.register(RemoteTriggerTool);
            registry.register(RemoteDevTool);
            registry.register(BrowserTool);
            registry.register(TeamTool);
            #[cfg(feature = "voice")]
            registry.register(super::voice_tool::VoiceTool);
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
        let ask_channel = std::sync::Arc::new(super::ask_tool::AskChannel::new());
        registry.ask_channel = Some(ask_channel.clone());
        registry.register(AskUserQuestionTool::new(ask_channel));

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
                    strict: tool.strict_schema().then_some(true),
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
    cache: super::cache::ToolResultCache,
}

impl CachedToolExecutor {
    /// 创建新的带缓存执行器
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry,
            cache: super::cache::ToolResultCache::new(),
        }
    }

    /// 使用指定缓存创建
    pub fn with_cache(registry: ToolRegistry, cache: super::cache::ToolResultCache) -> Self {
        Self { registry, cache }
    }

    /// 执行工具（带缓存）
    pub async fn execute(
        &self,
        tool_name: &str,
        params: Value,
        context: super::tool_trait::ToolContext,
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
    pub fn cache(&self) -> &super::cache::ToolResultCache {
        &self.cache
    }

    /// 获取缓存统计
    pub fn cache_stats(&self) -> super::cache::CacheStats {
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
