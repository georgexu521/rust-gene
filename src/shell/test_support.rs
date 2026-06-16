//! Test support for the CLI shell module.
//!
//! Exposes helpers to construct a minimal `StreamingQueryEngine` and `CliHost`
//! so integration tests can exercise local command dispatch without an
//! interactive terminal.

use crate::engine::streaming::StreamingQueryEngine;
use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
use crate::shell::host::CliHost;
use crate::tui::session_manager::TuiSessionManager;
use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;
use std::sync::Arc;

/// A deterministic provider that always returns empty text. It is sufficient
/// for tests that only need a valid engine/session wiring.
pub struct NoopProvider;

#[async_trait]
impl LlmProvider for NoopProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        Ok(ChatResponse {
            content: String::new(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
            finish_reason: None,
        })
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        Ok(Box::pin(
            tokio_stream::wrappers::UnboundedReceiverStream::new(rx),
        ))
    }

    fn base_url(&self) -> &str {
        "test://noop"
    }

    fn default_model(&self) -> &str {
        "noop-model"
    }
}

/// Build an in-memory engine wired to a `NoopProvider`.
pub fn test_engine() -> Arc<StreamingQueryEngine> {
    let registry = Arc::new(crate::tools::ToolRegistry::default_registry());
    let engine = StreamingQueryEngine::new(Arc::new(NoopProvider), registry, "noop-model")
        .with_disable_session_auto_init();
    Arc::new(engine)
}

/// Build a `CliHost` backed by an in-memory session manager.
pub fn test_cli_host(engine: Arc<StreamingQueryEngine>) -> CliHost {
    let session_manager = TuiSessionManager::in_memory().expect("in-memory session manager");
    CliHost::new(engine, session_manager)
}
