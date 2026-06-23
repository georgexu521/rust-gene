//! Shared dependency injection for `ConversationLoopBuilder`.
//!
//! Query and streaming engines own different request flows, but optional loop
//! dependencies should be wired in one place to avoid manager/session drift.

use crate::engine::ConversationLoopBuilder;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct LoopBuilderDeps {
    pub agent_manager: Option<Arc<crate::agent::AgentManager>>,
    pub mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    pub lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    pub worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    pub working_dir_override: Option<PathBuf>,
    pub memory_manager: Option<Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>>,
    pub approval_channel: Option<Arc<crate::engine::conversation_loop::ToolApprovalChannel>>,
    pub allowed_tools: Option<HashSet<String>>,
    pub session_binding: Option<(Arc<crate::session_store::SessionStore>, String)>,
}

impl LoopBuilderDeps {
    pub(crate) fn apply_to(self, mut builder: ConversationLoopBuilder) -> ConversationLoopBuilder {
        if let Some(manager) = self.agent_manager {
            builder = builder.with_agent_manager(manager);
        }
        if let Some(mcp) = self.mcp_manager {
            builder = builder.with_mcp_manager(mcp);
        }
        if let Some(lsp) = self.lsp_manager {
            builder = builder.with_lsp_manager(lsp);
        }
        if let Some(worktree) = self.worktree_manager {
            builder = builder.with_worktree_manager(worktree);
        }
        if let Some(working_dir) = self.working_dir_override {
            builder = builder.with_working_dir(working_dir);
        }
        if let Some(memory) = self.memory_manager {
            builder = builder.with_memory_manager(memory);
        }
        if let Some(channel) = self.approval_channel {
            builder = builder.with_approval_channel(channel);
        }
        if let Some(allowed) = self.allowed_tools {
            builder = builder.with_allowed_tools(allowed);
        }
        if let Some((store, session_id)) = self.session_binding {
            builder = builder.with_session_store(store, session_id);
        }
        builder
    }
}
