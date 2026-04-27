use crate::memory::types::{MemoryRecord, MemoryScope};
use crate::services::api::Message;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct MemoryTurn {
    pub user: String,
    pub assistant: String,
}

#[async_trait]
pub trait MemoryProvider: Send + Sync {
    fn name(&self) -> &str;

    fn is_available(&self) -> bool {
        true
    }

    async fn initialize(&self, _scope: &MemoryScope) -> anyhow::Result<()> {
        Ok(())
    }

    async fn system_prompt_block(&self, _scope: &MemoryScope) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    async fn prefetch(
        &self,
        _query: &str,
        _scope: &MemoryScope,
    ) -> anyhow::Result<Vec<MemoryRecord>> {
        Ok(Vec::new())
    }

    async fn queue_prefetch(&self, _query: &str, _scope: &MemoryScope) -> anyhow::Result<()> {
        Ok(())
    }

    async fn sync_turn(&self, _turn: &MemoryTurn, _scope: &MemoryScope) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_session_end(
        &self,
        _transcript: &[Message],
        _scope: &MemoryScope,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_pre_compress(
        &self,
        _messages: &[Message],
        _scope: &MemoryScope,
    ) -> anyhow::Result<Vec<MemoryRecord>> {
        Ok(Vec::new())
    }

    async fn on_memory_write(
        &self,
        _record: &MemoryRecord,
        _scope: &MemoryScope,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Adapter marker for the current local MEMORY.md / USER.md implementation.
///
/// The existing `MemoryManager` still owns local storage. This provider gives
/// the codebase a stable extension point for future external providers without
/// forcing the current local implementation through a risky rewrite in one
/// patch.
#[derive(Debug, Clone, Default)]
pub struct LocalMemoryProvider;

#[async_trait]
impl MemoryProvider for LocalMemoryProvider {
    fn name(&self) -> &str {
        "local"
    }
}
