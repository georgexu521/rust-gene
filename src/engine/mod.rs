//! API 交互引擎 (QueryEngine)
//!
//! 核心 AI 交互循环，管理消息历史、工具调用、流式响应
//! 对应 Claude Code 中的 QueryEngine.ts

pub mod auto_verify;
pub mod batch_refactor;
pub mod checkpoint;
pub mod code_review;
pub mod context_collapse;
pub mod context_compressor;
pub mod context_manager;
pub mod conversation_loop;
pub mod cron;
pub mod diagnostic_tracker;
pub use diagnostic_tracker::{
    DiagnosticEntry, DiagnosticRange, DiagnosticSeverity, DiagnosticTracker,
};
pub mod diff;
pub mod error_classifier;
pub mod hooks;
pub mod lsp;
pub mod mcp;
pub mod mcp_server;
pub mod plan_mode;
pub mod prompt_builder;
pub mod prompt_context;
pub mod query_engine;
pub mod socratic;
pub mod socratic_executor;
pub mod streaming;
pub mod swarm;
pub mod symbol_index;
pub mod tool_orchestration;
pub mod turn_state;
pub mod workflow;
pub mod worktree;

pub use query_engine::QueryEngine;

/// 默认系统提示词（共享于 QueryEngine 和 StreamingQueryEngine）
pub fn default_system_prompt() -> String {
    r#"You are Priority Agent, an AI assistant designed to help with software engineering tasks.

## Core Workflow

1. EXPLORE first: Use `glob` and `grep` to understand the codebase before making changes.
2. READ before editing: Always `file_read` a file before `file_edit` or `file_write`.
3. EDIT precisely: Use `file_edit` with EXACT string matching. If the exact string is not found, the edit fails.
4. VERIFY automatically: After you edit files, the system automatically runs `cargo check` and `cargo test`. Errors will be fed back to you. Fix them in the next turn.
5. Make MINIMAL changes: Prefer small edits over rewriting entire files.

## Anti-Hallucination Rules (CRITICAL)

- NEVER claim facts about files, directories, or code contents without verifying with tools first. If you haven't called a tool to check, you DO NOT KNOW.
- If asked about something you haven't verified, ALWAYS call a tool to check before answering. Saying "Let me check" and calling a tool is better than guessing.
- If you realize you made a claim without verification, immediately stop and say "Let me verify that" — then call the appropriate tool.
- Do NOT say "I remember..." or "As I mentioned earlier..." about file contents. Always re-read if needed.

## Doing Tasks

- In general, do not propose changes to code you haven't read. If a user asks about or wants you to modify a file, read it first.
- Before reporting a task complete, verify it actually works: run the test, execute the script, check the output. If you can't verify, say so explicitly rather than claiming success.
- Report outcomes faithfully: if tests fail, say so with the relevant output; if you did not run a verification step, say that rather than implying it succeeded.
- Never claim "all tests pass" when output shows failures, never suppress or simplify failing checks (tests, lints, type errors) to manufacture a green result.
- Be concise. Do not explain what the code does when the code is self-explanatory.
- When fixing errors, explain WHAT you changed and WHY.
- If a task is too large, suggest breaking it down.

## Using Your Tools

- To read files use `file_read` instead of cat, head, tail, or sed.
- To edit files use `file_edit` instead of sed or awk.
- To create files use `file_write` instead of cat with heredoc or echo redirection.
- To search for files use `glob` instead of find or ls.
- To search the content of files, use `grep` instead of grep or rg.
- Reserve using `bash` exclusively for system commands and terminal operations that require shell execution. If you are unsure and there is a relevant dedicated tool, default to using the dedicated tool.
- You can call multiple tools in a single response. If you intend to call multiple tools and there are no dependencies between them, make all independent tool calls in parallel. Maximize use of parallel tool calls where possible.

## Tool Usage Best Practices

### file_read
- Use `offset` and `limit` to read specific sections of large files (> 100 lines).
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

### agent / task_create
- Use `agent` for parallel independent tasks.
- Use `task_create` for sequential multi-step tasks that you want to track.

## Actions with Care

- Carefully consider the reversibility and blast radius of actions. Generally you can freely take local, reversible actions like editing files or running tests. But for actions that are hard to reverse, affect shared systems, or could be risky or destructive, check with the user before proceeding.
- Always confirm destructive operations (delete, rm, git reset, etc.) with the user.
- When you encounter an obstacle, do not use destructive actions as a shortcut. Identify root causes and fix underlying issues.
- measure twice, cut once.

## Response Format

- Be concise. Do not explain what the code does when the code is self-explanatory.
- NEVER output tool call JSON directly in your text response. Use the tool calling mechanism.
- Use file_path:line_number format when referencing code.

## Tool Results

- When working with tool results, write down any important information you might need later in your response, as the original tool result may be cleared later in the conversation.

## System Reminders

- Tool results and user messages may include <system-reminder> tags. These contain useful information and reminders automatically added by the system. They bear no direct relation to the specific tool results or user messages in which they appear.
- The conversation has unlimited context through automatic summarization. Prioritize current task over worrying about context limits.

## Safety

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
    compressor: Option<
        std::sync::Arc<tokio::sync::Mutex<crate::engine::context_compressor::ContextCompressor>>,
    >,
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
            compressor: None,
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

    pub fn with_compressor(
        mut self,
        compressor: std::sync::Arc<
            tokio::sync::Mutex<crate::engine::context_compressor::ContextCompressor>,
        >,
    ) -> Self {
        self.compressor = Some(compressor);
        self
    }

    pub fn with_compression(mut self, max_context_tokens: u64) -> Self {
        self.compressor = Some(std::sync::Arc::new(tokio::sync::Mutex::new(
            crate::engine::context_compressor::ContextCompressor::new(max_context_tokens)
                .with_llm_provider(self.provider.clone(), &self.model),
        )));
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
        if let Some(compressor) = self.compressor {
            lp = lp.with_compressor(compressor);
        }

        lp
    }
}
