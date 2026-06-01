//! Port interfaces — abstract traits for external dependencies.
//!
//! Mirrors Reasonix's `src/ports/` pattern: each port defines a trait that
//! can be swapped for testing. Current production implementations are in the
//! corresponding engine/service modules.
//!
//! Ports:
//! - `ModelClient` — LLM API abstraction.
//! - `ToolHost` — tool registration and dispatch.
//! - `EventSink` — streaming event output.
//! - `MemoryStore` — persistent memory CRUD.
//! - `HookRunner` — Pre/Post tool hook execution.
//! - `CheckpointStore` — file snapshot persistence.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;

// ── ModelClient ──────────────────────────────────────────────

/// Abstract LLM model client. Existing implementation:
/// `crate::services::api::LlmProvider`.
#[async_trait]
pub trait ModelClient: Send + Sync {
    /// Send a chat completion request and receive a text response.
    async fn chat(&self, model: &str, messages: &[Value]) -> Result<String>;

    /// Whether this model supports streaming responses.
    fn supports_streaming(&self) -> bool;
}

// ── ToolHost ────────────────────────────────────────────────

/// Result of a single tool execution.
#[derive(Debug, Clone)]
pub struct PortToolResult {
    pub success: bool,
    pub content: String,
    pub metadata: Value,
}

/// Abstract tool registry and dispatch. Existing implementation:
/// `crate::tools::ToolRegistry`.
#[async_trait]
pub trait ToolHost: Send + Sync {
    /// List names of all registered tools.
    fn tool_names(&self) -> Vec<String>;

    /// Execute a tool by name with the given arguments.
    async fn dispatch(
        &self,
        name: &str,
        args: &Value,
        working_dir: &Path,
    ) -> Result<PortToolResult>;

    /// Whether a tool is read-only (safe for concurrent execution).
    fn is_read_only(&self, name: &str, args: &Value) -> bool;

    /// Whether a tool is safe to run in parallel with other tools.
    fn is_concurrency_safe(&self, name: &str, args: &Value) -> bool;
}

// ── EventSink ────────────────────────────────────────────────

/// Streaming event output. Existing implementation:
/// `crate::engine::streaming::StreamEvent` via `tokio::sync::mpsc::Sender`.
#[async_trait]
pub trait EventSink: Send + Sync {
    /// Emit a streaming event to the consumer (TUI, API, desktop).
    async fn emit(&self, event_type: &str, payload: &Value) -> Result<()>;

    /// Check whether the consumer has disconnected (abort signal).
    fn is_closed(&self) -> bool;
}

// ── MemoryStore ──────────────────────────────────────────────

/// Persistent memory CRUD. Existing implementation:
/// `crate::memory::MemoryManager` + `crate::session_store::SessionStore`.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Save a memory entry.
    async fn save(&self, key: &str, content: &str, scope: &str) -> Result<()>;

    /// Load memory entries by scope.
    async fn load(&self, scope: &str) -> Result<Vec<(String, String)>>;

    /// Delete a memory entry.
    async fn delete(&self, key: &str, scope: &str) -> Result<()>;

    /// List all memory keys for a scope.
    async fn list_keys(&self, scope: &str) -> Result<Vec<String>>;
}

// ── HookRunner ───────────────────────────────────────────────

/// Hook execution outcome.
#[derive(Debug, Clone)]
pub enum HookDecision {
    Allow,
    Block { reason: String },
}

/// Pre/Post tool hook execution. Existing implementation:
/// `crate::engine::hooks`.
#[async_trait]
pub trait HookRunner: Send + Sync {
    /// Run pre-tool hooks. Returns `Block` if any hook rejects.
    async fn run_pre_tool(&self, tool_name: &str, tool_args: &Value) -> Result<HookDecision>;

    /// Run post-tool hooks (observational only — cannot block).
    async fn run_post_tool(
        &self,
        tool_name: &str,
        tool_args: &Value,
        tool_result: &str,
    ) -> Result<()>;
}

// ── CheckpointStore ──────────────────────────────────────────

/// File snapshot persistence for rollback. Existing implementation:
/// `crate::engine::checkpoint::CheckpointManager`.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Create a checkpoint (snapshot) of a file.
    async fn create(&self, path: &Path) -> Result<String>;

    /// Restore a file from a checkpoint id.
    async fn restore(&self, checkpoint_id: &str) -> Result<()>;

    /// Delete a checkpoint.
    async fn delete(&self, checkpoint_id: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Verify traits are object-safe (can be used as `dyn Trait`).
    #[test]
    fn model_client_is_object_safe() {
        let _: Option<Arc<dyn ModelClient>> = None;
    }

    #[test]
    fn tool_host_is_object_safe() {
        let _: Option<Arc<dyn ToolHost>> = None;
    }

    #[test]
    fn event_sink_is_object_safe() {
        let _: Option<Arc<dyn EventSink>> = None;
    }

    #[test]
    fn memory_store_is_object_safe() {
        let _: Option<Arc<dyn MemoryStore>> = None;
    }

    #[test]
    fn hook_runner_is_object_safe() {
        let _: Option<Arc<dyn HookRunner>> = None;
    }

    #[test]
    fn checkpoint_store_is_object_safe() {
        let _: Option<Arc<dyn CheckpointStore>> = None;
    }
}
