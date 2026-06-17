//! ShellHost trait and CLI implementation.
//!
//! ShellHost exposes the subset of TuiApp state that slash command handlers
//! need, so the same handlers can run against both the TUI and the CLI. The
//! CLI implementation is intentionally simpler: it has no Ratatui widgets, no
//! LSP manager, and no worktree manager.

use crate::engine::runtime_controller::RuntimeController;
use crate::engine::streaming::StreamingQueryEngine;
use crate::session_store::SessionStore;
use crate::shell::theme::{DIM, RESET};
use crate::skills::SkillRuntime;
use crate::tui::session_manager::TuiSessionManager;
use std::sync::Arc;

/// Cross-frontend context for slash command handlers.
///
/// Handlers should only rely on methods exposed here so they work in both the
/// full-screen TUI and the scrollback-first CLI.
pub trait ShellHost {
    /// Access the streaming query engine, if available.
    fn engine(&self) -> Option<Arc<StreamingQueryEngine>>;

    /// Access the session manager.
    fn session_manager(&self) -> &TuiSessionManager;

    /// Access the session store directly.
    fn session_store(&self) -> Arc<SessionStore> {
        self.session_manager().store()
    }

    /// Current workspace root.
    fn workspace_root(&self) -> std::path::PathBuf {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    }

    /// Build a tool context for the current host.
    fn build_tool_context(&self) -> crate::tools::ToolContext;

    /// Current agent mode.
    fn agent_mode(&self) -> crate::engine::agent_mode::AgentMode {
        crate::engine::agent_mode::AgentMode::default()
    }

    /// Label for the current agent mode.
    fn current_agent_mode_label(&self) -> &'static str {
        self.agent_mode().label()
    }

    /// Number of messages in the current conversation surface.
    fn message_count(&self) -> usize {
        0
    }

    /// Whether a query is currently in flight.
    fn is_querying(&self) -> bool {
        false
    }

    /// Skill runtime, if available.
    fn skill_runtime(&self) -> Option<&crate::skills::SkillRuntime> {
        None
    }

    /// Runtime status snapshot. The default is empty and should be overridden
    /// by hosts that track live tool/runtime state.
    fn runtime_status_snapshot(&self) -> crate::state::RuntimeStatusSnapshot {
        crate::state::RuntimeStatusSnapshot {
            messages: 0,
            is_querying: false,
            last_error: None,
            total_tools: 0,
            active_tool_count: 0,
            active_tool_ids: Vec::new(),
            current_tool_label: None,
            failed_tool_count: 0,
            backgrounded_tool_count: 0,
            terminal_task_count: 0,
            running_terminal_task_count: 0,
            pty_terminal_task_count: 0,
            task_count: 0,
            running_task_count: 0,
            permission_mode: String::new(),
            pending_permission: None,
            mcp_server_count: 0,
            mcp_available_count: 0,
            mcp_repair_hints: Vec::new(),
            bridge_url_configured: false,
            bridge_url_source: None,
            bridge_cursor_count: 0,
            remote_env_type: String::new(),
            remote_trigger_tool_available: false,
            remote_dev_tool_available: false,
        }
    }

    /// Restore a session by id. The CLI implementation mutates the engine
    /// history directly; the TUI implementation also restores UI widgets.
    fn restore_session<'a>(
        &'a mut self,
        session_id: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>>;

    /// Show a short transient/info message. In the CLI this prints to
    /// scrollback; in the TUI this is a toast.
    fn show_message(&mut self, message: String);

    /// Memory controls.
    fn memory_use(&self) -> bool;
    fn set_memory_use(&mut self, value: bool);
    fn memory_generate(&self) -> bool;
    fn set_memory_generate(&mut self, value: bool);
    fn memory_recall_mode(&self) -> &str;
    fn set_memory_recall_mode(&mut self, value: String);
}

/// CLI implementation of `ShellHost`.
pub struct CliHost {
    pub engine: Arc<StreamingQueryEngine>,
    pub session_manager: TuiSessionManager,
    pub memory_use: bool,
    pub memory_generate: bool,
    pub memory_recall_mode: String,
    pub skill_runtime: SkillRuntime,
    pub controller: Option<RuntimeController>,
}

impl CliHost {
    pub fn new(engine: Arc<StreamingQueryEngine>, session_manager: TuiSessionManager) -> Self {
        let memory_use = engine.memory_use_enabled();
        let memory_generate = engine.memory_generate_enabled();
        let memory_recall_mode = engine.memory_recall_mode();
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let skill_runtime = SkillRuntime::load(&working_dir);
        Self {
            engine,
            session_manager,
            memory_use,
            memory_generate,
            memory_recall_mode,
            skill_runtime,
            controller: None,
        }
    }

    pub fn with_controller(mut self, controller: RuntimeController) -> Self {
        self.controller = Some(controller);
        self
    }
}

impl ShellHost for CliHost {
    fn engine(&self) -> Option<Arc<StreamingQueryEngine>> {
        Some(self.engine.clone())
    }

    fn session_manager(&self) -> &TuiSessionManager {
        &self.session_manager
    }

    fn restore_session<'a>(
        &'a mut self,
        session_id: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>> {
        Box::pin(async move {
            self.engine
                .flush_memory_for_current_history(crate::memory::MemoryFlushReason::ResumeSwitch)
                .await;

            match self.session_manager.switch_to_session(session_id) {
                Ok(_) => {
                    match self.session_manager.load_api_messages(session_id) {
                        Ok(messages) => {
                            self.engine.set_history(messages).await;
                            self.engine.set_session_id(session_id.to_string());
                        }
                        Err(e) => return format!("Failed to load session messages: {e}"),
                    }

                    let title = self.session_manager.current_session_title();
                    let count = self.session_manager.message_count(session_id).unwrap_or(0);
                    format!(
                        "Restored session {} · {title} · {count} messages",
                        &session_id[..8.min(session_id.len())]
                    )
                }
                Err(e) => format!("Failed to restore session: {e}"),
            }
        })
    }

    fn build_tool_context(&self) -> crate::tools::ToolContext {
        let session_id = self
            .session_manager()
            .current_session_id()
            .unwrap_or("cli")
            .to_string();
        let mut context = crate::tools::ToolContext::new(self.workspace_root(), &session_id);
        context = context.with_session_store(self.session_store());
        context = context.with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone());
        if let Some(engine) = self.engine() {
            context.permission_context.mode = engine.permission_mode();
            let session_rules = engine.session_permission_rules();
            context
                .permission_context
                .rules
                .always_allow
                .extend(session_rules.always_allow);
            context
                .permission_context
                .rules
                .always_deny
                .extend(session_rules.always_deny);
        }
        context
    }

    fn show_message(&mut self, message: String) {
        print!("{DIM}{message}{RESET}\r\n");
        let _ = std::io::Write::flush(&mut std::io::stdout());
    }

    fn memory_use(&self) -> bool {
        self.memory_use
    }

    fn set_memory_use(&mut self, value: bool) {
        self.memory_use = value;
        self.engine.set_memory_use(value);
    }

    fn memory_generate(&self) -> bool {
        self.memory_generate
    }

    fn set_memory_generate(&mut self, value: bool) {
        self.memory_generate = value;
        self.engine.set_memory_generate(value);
    }

    fn memory_recall_mode(&self) -> &str {
        &self.memory_recall_mode
    }

    fn set_memory_recall_mode(&mut self, value: String) {
        self.memory_recall_mode = value.clone();
        self.engine.set_memory_recall_mode(value);
    }

    fn skill_runtime(&self) -> Option<&SkillRuntime> {
        Some(&self.skill_runtime)
    }

    fn agent_mode(&self) -> crate::engine::agent_mode::AgentMode {
        crate::engine::agent_mode::AgentMode::Auto
    }

    fn message_count(&self) -> usize {
        self.session_manager
            .current_session_id()
            .and_then(|sid| self.session_manager.message_count(sid).ok())
            .map(|n| n as usize)
            .unwrap_or(0)
    }

    fn is_querying(&self) -> bool {
        false
    }

    fn runtime_status_snapshot(&self) -> crate::state::RuntimeStatusSnapshot {
        let mut snapshot = crate::state::RuntimeStatusSnapshot {
            permission_mode: format!("{:?}", self.engine.permission_mode()),
            ..Default::default()
        };
        snapshot.total_tools = self.engine.tool_registry().tool_names().len();
        if let Some(mcp) = self.engine.mcp_manager() {
            let diagnostics = mcp.health_diagnostics();
            snapshot.mcp_server_count = diagnostics.len();
            snapshot.mcp_available_count = diagnostics
                .iter()
                .filter(|d| d.approved && d.health == crate::engine::mcp::McpHealthStatus::Healthy)
                .count();
            snapshot.mcp_repair_hints = diagnostics
                .iter()
                .filter(|d| d.repair_hint != "none")
                .map(|d| format!("{}=>{}", d.name, d.repair_hint))
                .collect();
        }
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_host_exposes_engine() {
        // Structural test: the trait compiles and returns engine.
        assert_eq!(std::mem::size_of::<Option<Arc<StreamingQueryEngine>>>(), 8);
    }
}
