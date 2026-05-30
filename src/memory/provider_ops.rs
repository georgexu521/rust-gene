//! Provider 编排层
//!
//! 薄 orchestration：`MemoryManager` 上对 `provider_registry` 的委托方法。
//! 每个方法都是对 `self.provider_registry.xxx()` 的单行包装，负责将
//! MemoryManager 上下文转发到 provider 基础设施层。

use super::manager::MemoryManager;
use super::provider::{
    MemoryProvider, MemoryProviderCallOutcome, MemoryProviderCapabilities,
    MemoryProviderLifecycleReport, MemoryTurn, NoNetworkMemoryProvider,
};
use super::types::{MemoryRecord, MemoryScope};
use crate::services::api::Message;
use std::sync::Arc;

impl MemoryManager {
    pub fn memory_provider_names(&self) -> Vec<String> {
        self.provider_registry.provider_names()
    }

    pub fn memory_provider_lifecycle_report(&self) -> MemoryProviderLifecycleReport {
        self.provider_registry.lifecycle_report()
    }

    pub fn register_external_memory_provider(
        &mut self,
        provider: Arc<dyn MemoryProvider>,
    ) -> anyhow::Result<()> {
        self.provider_registry.register_external(provider)
    }

    pub fn configure_external_memory_provider_from_config(
        &mut self,
        config: &crate::services::config::ExternalMemoryProviderConfig,
    ) -> anyhow::Result<bool> {
        if !config.enabled {
            return Ok(false);
        }
        let capabilities = MemoryProviderCapabilities {
            prompt_block: config.prompt_block,
            prefetch: config.prefetch,
            search: config.search,
            queue_prefetch: config.queue_prefetch,
            sync_turn: config.sync_turn,
            session_end: config.session_end,
            pre_compress: config.pre_compress,
            write_mirror: config.write_mirror,
            tools: config.tools,
        };
        match config.provider_type.as_str() {
            "no_network_jsonl" => {
                let records_path = config.records_path.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "memory.external_provider.records_path is required for no_network_jsonl"
                    )
                })?;
                let provider = NoNetworkMemoryProvider::from_jsonl_file_with_capabilities(
                    config.name.clone(),
                    records_path,
                    capabilities,
                )?;
                self.register_external_memory_provider(Arc::new(provider))?;
                Ok(true)
            }
            "none" => Ok(false),
            other => Err(anyhow::anyhow!(
                "unsupported external memory provider type '{}'",
                other
            )),
        }
    }

    pub async fn initialize_memory_providers(
        &self,
        scope: &MemoryScope,
    ) -> Vec<MemoryProviderCallOutcome> {
        self.provider_registry.initialize_all(scope).await
    }

    pub async fn provider_system_prompt_blocks(
        &self,
        scope: &MemoryScope,
    ) -> (Vec<String>, Vec<MemoryProviderCallOutcome>) {
        self.provider_registry.system_prompt_blocks(scope).await
    }

    pub async fn provider_prefetch(
        &self,
        query: &str,
        scope: &MemoryScope,
    ) -> (Vec<MemoryRecord>, Vec<MemoryProviderCallOutcome>) {
        self.provider_registry.prefetch_all(query, scope).await
    }

    pub async fn provider_search(
        &self,
        query: &str,
        scope: &MemoryScope,
        max_results: usize,
    ) -> (Vec<MemoryRecord>, Vec<MemoryProviderCallOutcome>) {
        self.provider_registry
            .search_all(query, scope, max_results)
            .await
    }

    pub async fn queue_memory_provider_prefetch(
        &self,
        query: &str,
        scope: &MemoryScope,
    ) -> Vec<MemoryProviderCallOutcome> {
        self.provider_registry
            .queue_prefetch_all(query, scope)
            .await
    }

    pub async fn sync_memory_providers_turn(
        &self,
        user: &str,
        assistant: &str,
        scope: &MemoryScope,
    ) -> Vec<MemoryProviderCallOutcome> {
        let turn = MemoryTurn {
            user: user.to_string(),
            assistant: assistant.to_string(),
        };
        self.provider_registry.sync_turn_all(&turn, scope).await
    }

    pub async fn notify_memory_providers_session_end(
        &self,
        transcript: &[Message],
        scope: &MemoryScope,
    ) -> Vec<MemoryProviderCallOutcome> {
        self.provider_registry
            .on_session_end_all(transcript, scope)
            .await
    }

    pub async fn notify_memory_providers_pre_compress(
        &self,
        messages: &[Message],
        scope: &MemoryScope,
    ) -> (Vec<MemoryRecord>, Vec<MemoryProviderCallOutcome>) {
        self.provider_registry
            .on_pre_compress_all(messages, scope)
            .await
    }

    pub async fn notify_memory_providers_write(
        &self,
        record: &MemoryRecord,
        scope: &MemoryScope,
    ) -> Vec<MemoryProviderCallOutcome> {
        self.provider_registry
            .on_memory_write_all(record, scope)
            .await
    }

    pub async fn shutdown_memory_providers(&self) -> Vec<MemoryProviderCallOutcome> {
        self.provider_registry.shutdown_all().await
    }
}
