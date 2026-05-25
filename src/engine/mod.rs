//! API 交互引擎 (QueryEngine)
//!
//! 核心 AI 交互循环，管理消息历史、工具调用、流式响应
//! 对应 Claude Code 中的 QueryEngine.ts

pub mod action_decision;
pub mod action_policy;
pub mod action_review;
pub mod agent_mode;
pub mod auto_verify;
pub mod batch_refactor;
pub mod candidate_action;
pub mod checkpoint;
pub mod code_change_workflow;
pub mod code_review;
pub mod context_assembly;
pub mod context_collapse;
pub mod context_compressor;
pub mod context_ledger;
pub mod context_manager;
pub mod context_usage;
pub mod conversation_loop;
pub mod cron;
pub mod destructive_scope;
pub mod diagnostic_tracker;
pub use diagnostic_tracker::{
    DiagnosticEntry, DiagnosticRange, DiagnosticSeverity, DiagnosticTracker,
};
pub mod diff;
pub mod error_classifier;
pub mod evalset;
pub mod evidence_ledger;
pub mod evolution_controller;
pub mod experience_ledger;
pub mod goal_drift;
pub mod hooks;
pub mod human_review;
pub mod improvement;
pub mod intent_router;
pub mod learning_planning;
pub mod lightweight_planner;
pub mod lsp;
pub mod mcp;
pub mod mcp_server;
pub mod model_context;
pub mod plan_mode;
pub mod prompt_builder;
pub mod prompt_context;
pub mod prompt_templates;
pub mod query_engine;
pub mod recovery_plan;
pub mod reflection_pass;
pub mod repair_spec;
pub mod resource_policy;
pub mod retrieval_context;
#[cfg(test)]
mod runtime_spine_behavior_tests;
pub mod scenario_matrix;
pub mod session_goal;
pub mod skill_evolution;
pub mod socratic;
pub mod socratic_executor;
pub mod stop_checker;
pub mod streaming;
pub mod swarm;
pub mod symbol_index;
pub mod task_context;
pub mod task_contract;
pub mod task_mode_score;
pub mod tool_exposure;
pub mod tool_orchestration;
pub mod trace;
pub mod turn_state;
pub mod verification_proof;
pub mod workflow;
pub mod workflow_contract;
pub mod worktree;

pub use query_engine::QueryEngine;

/// 默认系统提示词（共享于 QueryEngine 和 StreamingQueryEngine）
pub fn default_system_prompt() -> String {
    r#"You are Priority Agent, an AI assistant designed to help with software engineering tasks.

## Core Conduct

- Inspect before asserting file, code, command, or project specifics; answer from evidence.
- For local file or workspace checks, call tools. Do not paste commands as answers.
- Do not infer size, item count, or creation time from `ls -la`; use explicit tools when asked.
- Make the smallest coherent change that satisfies the user's request. Leave unrelated code and user edits alone.
- Ask only when a real human decision, missing requirement, or ambiguous destructive scope blocks safe progress. Otherwise make conservative assumptions and continue.
- Be direct and concise. Explain what changed, why it matters, and what was or was not verified.

## Model-Led Programming Workflow

You provide the engineering judgment. Runtime checks may route tools, validate,
or request repair, but they do not replace your reasoning.

For code tasks:

- Understand the request and acceptance criteria before changing files.
- Use `file_read`, `grep`, and `glob` for targeted context.
- Use `file_edit` for existing files and `file_write` for new files; use `bash` for validation and shell-only work.
- Prefer focused patches over broad rewrites.
- Before finishing, connect the result back to the original request and name any remaining risk or assumption.

## Verification And Reporting

- Verify changed behavior with the narrowest meaningful command, then broader checks when risk warrants it.
- Report failures honestly with the important output. Do not claim success, passing tests, or completed acceptance criteria unless the evidence supports it.
- If a check was skipped or impossible, say that clearly.
- Never suppress, reinterpret, or simplify failing diagnostics to make a result look green.

## Actions with Care

- Local, reversible actions such as reading, editing, and running tests are usually okay. For hard-to-reverse, shared-system, credential, purchase, publish, or destructive actions, get explicit approval first.
- For destructive requests, keep the scope exact: act only on the file/path/ref the user named or clearly approved. After completing a delete/remove/reset, do not suggest deleting parent directories, sibling files, unrelated folders, or broader cleanup unless the user explicitly asked for that scope.
- Do not use destructive actions as a shortcut around obstacles. Find the root cause or report the blocker.
- Do not expose secrets or credentials in code or output.

## Response Format

- NEVER output tool call JSON directly in your text response. Use the tool calling mechanism.
- Use file_path:line_number format when referencing code.
- When tool output contains details needed for the final answer, carry forward only the relevant facts."#
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
    session_permission_rules: crate::permissions::PermissionRules,
    llm_memory_extraction: bool,
    approval_channel: Option<std::sync::Arc<self::conversation_loop::ToolApprovalChannel>>,
    allowed_tools: Option<std::collections::HashSet<String>>,
    working_dir_override: Option<std::path::PathBuf>,
    allowed_mcp_servers: Option<Vec<String>>,
    trace_store: Option<std::sync::Arc<self::trace::TraceStore>>,
    goal_manager: Option<std::sync::Arc<self::session_goal::SessionGoalManager>>,
    session_store: Option<std::sync::Arc<crate::session_store::SessionStore>>,
    session_id: Option<String>,
    compressor: Option<
        std::sync::Arc<tokio::sync::Mutex<crate::engine::context_compressor::ContextCompressor>>,
    >,
    agent_mode: self::agent_mode::AgentMode,
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
            permission_mode: crate::permissions::PermissionMode::AutoAll,
            session_permission_rules: crate::permissions::PermissionRules::new(),
            llm_memory_extraction: false,
            approval_channel: None,
            allowed_tools: None,
            working_dir_override: None,
            allowed_mcp_servers: None,
            trace_store: None,
            goal_manager: None,
            session_store: None,
            session_id: None,
            compressor: None,
            agent_mode: self::agent_mode::AgentMode::Auto,
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

    pub fn with_session_permission_rules(
        mut self,
        rules: crate::permissions::PermissionRules,
    ) -> Self {
        self.session_permission_rules = rules;
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

    pub fn with_working_dir(mut self, working_dir: impl Into<std::path::PathBuf>) -> Self {
        self.working_dir_override = Some(working_dir.into());
        self
    }

    pub fn with_allowed_mcp_servers(mut self, servers: Vec<String>) -> Self {
        let servers = servers
            .into_iter()
            .map(|server| server.trim().to_string())
            .filter(|server| !server.is_empty())
            .collect::<Vec<_>>();
        if !servers.is_empty() {
            self.allowed_mcp_servers = Some(servers);
        }
        self
    }

    pub fn with_trace_store(mut self, store: std::sync::Arc<self::trace::TraceStore>) -> Self {
        self.trace_store = Some(store);
        self
    }

    pub fn with_session_goal_manager(
        mut self,
        manager: std::sync::Arc<self::session_goal::SessionGoalManager>,
    ) -> Self {
        self.goal_manager = Some(manager);
        self
    }

    pub fn with_session_store(
        mut self,
        store: std::sync::Arc<crate::session_store::SessionStore>,
        session_id: impl Into<String>,
    ) -> Self {
        self.session_store = Some(store);
        self.session_id = Some(session_id.into());
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

    pub fn with_model_context_profile(mut self) -> Self {
        let profile =
            self::model_context::ModelContextProfile::detect(self.provider.base_url(), &self.model);
        self.compressor = Some(std::sync::Arc::new(tokio::sync::Mutex::new(
            crate::engine::context_compressor::ContextCompressor::from_model_context_profile(
                &profile,
            )
            .with_llm_provider(self.provider.clone(), &self.model),
        )));
        self
    }

    pub fn with_agent_mode(mut self, mode: self::agent_mode::AgentMode) -> Self {
        self.agent_mode = mode;
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
        .with_session_permission_rules(self.session_permission_rules)
        .with_llm_memory_extraction(self.llm_memory_extraction)
        .with_agent_mode(self.agent_mode);

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
        if let Some(working_dir) = self.working_dir_override {
            lp = lp.with_working_dir(working_dir);
        }
        if let Some(servers) = self.allowed_mcp_servers {
            lp = lp.with_allowed_mcp_servers(servers);
        }
        if let Some(trace_store) = self.trace_store {
            lp = lp.with_trace_store(trace_store);
        }
        if let Some(goal_manager) = self.goal_manager {
            lp = lp.with_session_goal_manager(goal_manager);
        }
        if let (Some(session_store), Some(session_id)) = (self.session_store, self.session_id) {
            lp = lp.with_session_store(session_store, session_id);
        }
        if let Some(compressor) = self.compressor {
            lp = lp.with_compressor(compressor);
        }

        lp
    }
}
