//! Memory provider support module.
//!
//! Defines provider contracts and no-network behavior for controlled memory lifecycle tests.

use crate::memory::types::{MemoryRecord, MemoryScope};
use crate::services::api::Message;
use async_trait::async_trait;

use super::types::{MemoryProviderCapabilities, MemoryTurn};

#[async_trait]
pub trait MemoryProvider: Send + Sync {
    fn name(&self) -> &str;

    fn as_any(&self) -> &dyn std::any::Any;

    fn is_available(&self) -> bool {
        true
    }

    fn capabilities(&self) -> MemoryProviderCapabilities {
        MemoryProviderCapabilities::read_only()
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

    async fn search(
        &self,
        query: &str,
        scope: &MemoryScope,
        max_results: usize,
    ) -> anyhow::Result<Vec<MemoryRecord>> {
        let mut records = self.prefetch(query, scope).await?;
        records.truncate(max_results);
        Ok(records)
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
