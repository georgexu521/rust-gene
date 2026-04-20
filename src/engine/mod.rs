//! API 交互引擎 (QueryEngine)
//!
//! 核心 AI 交互循环，管理消息历史、工具调用、流式响应
//! 对应 Claude Code 中的 QueryEngine.ts

pub mod auto_verify;
pub mod code_review;
pub mod context_compressor;
pub mod symbol_index;
pub mod context_manager;
pub mod conversation_loop;
pub mod cron;
pub mod error_classifier;
pub mod hooks;
pub mod lsp;
pub mod mcp;
pub mod plan_mode;
pub mod prompt_builder;
pub mod query_engine;
pub mod socratic;
pub mod socratic_executor;
pub mod streaming;
pub mod swarm;
pub mod turn_state;
pub mod worktree;

pub use query_engine::QueryEngine;

/// 默认系统提示词（共享于 QueryEngine 和 StreamingQueryEngine）
pub fn default_system_prompt() -> String {
    r#"You are Priority Agent, an AI assistant designed to help with software engineering tasks.

You have access to various tools. Follow these rules EXACTLY.

## Core Workflow

1. EXPLORE first: Use `glob` and `grep` to understand the codebase before making changes.
2. READ before editing: Always `file_read` a file before `file_edit` or `file_write`.
3. EDIT precisely: Use `file_edit` with EXACT string matching. If the exact string is not found, the edit fails.
4. VERIFY automatically: After you edit files, the system automatically runs `cargo check` and `cargo test`. Errors will be fed back to you. Fix them in the next turn.
5. Make MINIMAL changes: Prefer small edits over rewriting entire files.

## Tool Usage Best Practices

### file_read
- Use `offset` and `limit` to read specific sections of large files (>
  100 lines).
- Read the relevant section, not the entire file, when you already know where the change goes.

### file_edit (CRITICAL - most common failure source)
The `old_string` parameter must match EXACTLY, including whitespace, indentation, and newlines.

**If exact match fails, immediately switch to `line_start` + `line_end` mode:**
- Set `line_start` and `line_end` (1-indexed) to the range you want to replace.
- Set `new_string` to the replacement content.
- This is MORE RELIABLE than exact string matching for multi-line blocks.

**Example - exact match (preferred for single-line):**
```json
{"path": "src/lib.rs", "old_string": "    let x = 1;", "new_string": "    let x = 2;"}
```

**Example - line range (use when exact match might fail):**
```json
{"path": "src/lib.rs", "line_start": 42, "line_end": 45, "new_string": "    let x = 2;\n    let y = 3;"}
```

**NEVER** guess indentation. Read the file first to see the actual indentation.

### file_write
- Only for creating NEW files or completely rewriting a file.
- Warns before overwriting existing files.

### bash
- Use for running tests, checking compilation, installing dependencies.
- Prefer `cargo check`, `cargo test`, `cargo fmt`, `cargo clippy` for Rust.
- Use `npm test`, `pytest`, `go test` for other languages.
- Do NOT use bash for file operations when `file_edit`/`file_write` tools are available.

### grep
- Use to find where a symbol is defined or used.
- Example: search for "fn my_function" to find its definition.

### glob
- Use to find files by pattern.
- Example: `"src/**/*.rs"` finds all Rust source files.

### symbol_query
- Use to find function/struct/enum definitions by name.
- More precise than grep for finding declarations.
- Actions: `search` (fuzzy), `list_file` (all symbols in file), `list_kind` (filter by type).

### refactor
- Use for semantic refactoring: `rename` (with existence check), `extract_function`, `add_impl_method`.
- Prefer `refactor` over manual `file_edit` for renaming symbols, as it checks the symbol index first.

### agent / task_create
- Use `agent` for parallel independent tasks.
- Use `task_create` for sequential multi-step tasks that you want to track.

## Automated Verification (you do NOT need to manually run these)

After every file edit/write, the system AUTOMATICALLY:
1. Runs `cargo check` (or language-appropriate checker)
2. Runs `cargo test` (if `PRIORITY_AGENT_AUTO_TEST=1` is set)
3. Collects LSP diagnostics
4. Runs code review (unwrap/unsafe/panic scan)

If errors are found, they will appear in your next turn's context. FIX THEM.

## Response Format

- Be concise. Do not explain what the code does when the code is self-explanatory.
- When fixing errors, explain WHAT you changed and WHY.
- If a task is too large, suggest breaking it down with `task_create`.
- NEVER output tool call JSON directly in your text response. Use the tool calling mechanism.

## Safety

- Always confirm destructive operations (delete, rm, git reset, etc.) with the user.
- Do not execute commands that download or run untrusted code.
- Do not expose secrets or credentials in code or output."#
        .to_string()
}

/// 共享的 ConversationLoop 构建器
/// 消除 QueryEngine 和 StreamingQueryEngine 中的重复逻辑
pub struct ConversationLoopBuilder {
    provider: std::sync::Arc<dyn crate::services::api::LlmProvider>,
    tool_registry: std::sync::Arc<crate::tools::ToolRegistry>,
    cost_tracker: std::sync::Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>,
    model: String,
    max_iterations: usize,
    agent_manager: Option<std::sync::Arc<crate::agent::AgentManager>>,
    mcp_manager: Option<std::sync::Arc<self::mcp::McpManager>>,
    lsp_manager: Option<std::sync::Arc<self::lsp::LspManager>>,
    worktree_manager: Option<std::sync::Arc<self::worktree::WorktreeManager>>,
    memory_manager: Option<std::sync::Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>>,
    hook_manager: Option<std::sync::Arc<self::hooks::ToolHookManager>>,
    permission_mode: crate::permissions::PermissionMode,
    llm_memory_extraction: bool,
    approval_channel: Option<std::sync::Arc<self::conversation_loop::ToolApprovalChannel>>,
    allowed_tools: Option<std::collections::HashSet<String>>,
}

impl ConversationLoopBuilder {
    pub fn new(
        provider: std::sync::Arc<dyn crate::services::api::LlmProvider>,
        tool_registry: std::sync::Arc<crate::tools::ToolRegistry>,
        cost_tracker: std::sync::Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            tool_registry,
            cost_tracker,
            model: model.into(),
            max_iterations: 10,
            agent_manager: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            memory_manager: None,
            hook_manager: None,
            permission_mode: crate::permissions::PermissionMode::AutoLowRisk,
            llm_memory_extraction: false,
            approval_channel: None,
            allowed_tools: None,
        }
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_agent_manager(
        mut self,
        manager: std::sync::Arc<crate::agent::AgentManager>,
    ) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    pub fn with_mcp_manager(mut self, manager: std::sync::Arc<self::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    pub fn with_lsp_manager(mut self, manager: std::sync::Arc<self::lsp::LspManager>) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    pub fn with_worktree_manager(
        mut self,
        manager: std::sync::Arc<self::worktree::WorktreeManager>,
    ) -> Self {
        self.worktree_manager = Some(manager);
        self
    }

    pub fn with_memory_manager(
        mut self,
        manager: std::sync::Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>,
    ) -> Self {
        self.memory_manager = Some(manager);
        self
    }

    pub fn with_hook_manager(
        mut self,
        manager: std::sync::Arc<self::hooks::ToolHookManager>,
    ) -> Self {
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

    pub fn with_approval_channel(
        mut self,
        channel: std::sync::Arc<self::conversation_loop::ToolApprovalChannel>,
    ) -> Self {
        self.approval_channel = Some(channel);
        self
    }

    pub fn with_allowed_tools(mut self, tools: std::collections::HashSet<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    pub fn build(self) -> self::conversation_loop::ConversationLoop {
        let mut lp = self::conversation_loop::ConversationLoop::new(
            self.provider,
            self.tool_registry,
            self.cost_tracker,
            self.model,
        )
        .with_max_iterations(self.max_iterations)
        .with_permission_mode(self.permission_mode)
        .with_llm_memory_extraction(self.llm_memory_extraction);

        if let Some(manager) = self.agent_manager {
            lp = lp.with_agent_manager(manager);
        }
        if let Some(mcp) = self.mcp_manager {
            lp = lp.with_mcp_manager(mcp);
        }
        if let Some(lsp) = self.lsp_manager {
            lp = lp.with_lsp_manager(lsp);
        }
        if let Some(wt) = self.worktree_manager {
            lp = lp.with_worktree_manager(wt);
        }
        if let Some(mem) = self.memory_manager {
            lp = lp.with_memory_manager(mem);
        }
        if let Some(hooks) = self.hook_manager {
            lp = lp.with_hook_manager(hooks);
        }
        if let Some(channel) = self.approval_channel {
            lp = lp.with_approval_channel(channel);
        }
        if let Some(allowed_tools) = self.allowed_tools {
            lp = lp.with_allowed_tools(allowed_tools);
        }

        lp
    }
}
