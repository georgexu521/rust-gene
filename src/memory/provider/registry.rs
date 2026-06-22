//! Memory provider registry and routing.
//!
//! The registry owns local/project/user memory providers and search-index
//! wiring. Runtime callers should use this layer instead of coupling to a
//! concrete provider implementation.

use crate::memory::search_index::{MemorySearchDocument, MemorySearchHit, MemorySearchIndexReport};
use crate::memory::types::{MemoryProjection, MemoryRecord, MemoryScope};
use crate::services::api::Message;
use anyhow::anyhow;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use super::{
    local_provider_record_order, local_provider_record_safe, LocalMemoryMigrationBackupReport,
    LocalMemoryMigrationFileReport, LocalMemoryMigrationRollbackReport, LocalMemoryProvider,
    LocalMemoryRecordWriteStatus, MemoryOperationJournalEntry, MemoryProvider,
    MemoryProviderCallOutcome, MemoryProviderLifecycleEntry, MemoryProviderLifecycleReport,
    MemoryTurn, MEMORY_PROVIDER_LIFECYCLE_HOOKS,
};

#[derive(Clone)]
pub struct MemoryProviderRegistry {
    local: Arc<dyn MemoryProvider>,
    external: Option<Arc<dyn MemoryProvider>>,
}

impl std::fmt::Debug for MemoryProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryProviderRegistry")
            .field("providers", &self.provider_names())
            .finish()
    }
}

impl Default for MemoryProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryProviderRegistry {
    pub fn new() -> Self {
        Self {
            local: Arc::new(LocalMemoryProvider::default()),
            external: None,
        }
    }

    pub fn with_local(local: Arc<dyn MemoryProvider>) -> Self {
        Self {
            local,
            external: None,
        }
    }

    pub fn with_local_for_tests(local: Arc<dyn MemoryProvider>) -> Self {
        Self::with_local(local)
    }

    pub fn provider_names(&self) -> Vec<String> {
        self.providers()
            .into_iter()
            .map(|provider| provider.name().to_string())
            .collect()
    }

    pub fn external_provider_name(&self) -> Option<String> {
        self.external
            .as_ref()
            .map(|provider| provider.name().to_string())
    }

    pub fn lifecycle_report(&self) -> MemoryProviderLifecycleReport {
        let mut providers = vec![MemoryProviderLifecycleEntry {
            name: self.local.name().to_string(),
            kind: "local".to_string(),
            available: self.local.is_available(),
            hooks: self.local.capabilities().supported_hooks(),
            capabilities: self.local.capabilities(),
        }];
        if let Some(external) = self.external.as_ref() {
            providers.push(MemoryProviderLifecycleEntry {
                name: external.name().to_string(),
                kind: "external".to_string(),
                available: external.is_available(),
                hooks: external.capabilities().supported_hooks(),
                capabilities: external.capabilities(),
            });
        }
        MemoryProviderLifecycleReport {
            providers,
            external_provider: self.external_provider_name(),
            lifecycle_hooks: lifecycle_hooks(),
        }
    }

    pub fn local_memory_records(&self) -> anyhow::Result<Vec<MemoryRecord>> {
        self.local_memory_records_raw().map(|records| {
            records
                .into_iter()
                .filter(local_provider_record_safe)
                .collect()
        })
    }

    pub fn local_memory_records_raw(&self) -> anyhow::Result<Vec<MemoryRecord>> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            return Ok(Vec::new());
        };
        local.memory_records()
    }

    pub fn append_local_memory_record(
        &self,
        record: &MemoryRecord,
        scope: &MemoryScope,
        operation: &str,
        reason: &str,
    ) -> anyhow::Result<LocalMemoryRecordWriteStatus> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            return Ok(LocalMemoryRecordWriteStatus::Duplicate);
        };
        local.append_memory_record(record, scope, operation, reason)
    }

    pub fn replace_local_memory_records(
        &self,
        records: &[MemoryRecord],
        operation: &str,
        reason: &str,
    ) -> anyhow::Result<()> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            return Ok(());
        };
        local.replace_memory_records(records, operation, reason)
    }

    pub fn record_local_memory_operation(
        &self,
        entry: MemoryOperationJournalEntry,
    ) -> anyhow::Result<()> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            return Ok(());
        };
        local.append_operation_journal_entry(&entry)
    }

    pub fn local_memory_operation_journal(
        &self,
    ) -> anyhow::Result<Vec<MemoryOperationJournalEntry>> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            return Ok(Vec::new());
        };
        local.operation_journal_entries()
    }

    pub fn local_search_index_path(&self) -> Option<PathBuf> {
        self.local
            .as_any()
            .downcast_ref::<LocalMemoryProvider>()
            .and_then(LocalMemoryProvider::search_index_path)
    }

    pub fn rebuild_local_search_index(
        &self,
        documents: &[MemorySearchDocument],
    ) -> anyhow::Result<Option<MemorySearchIndexReport>> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            return Ok(None);
        };
        local.rebuild_search_index(documents).map(Some)
    }

    pub fn search_local_index(
        &self,
        query: &str,
        max_results: usize,
    ) -> anyhow::Result<Vec<MemorySearchHit>> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            return Ok(Vec::new());
        };
        local.search_index(query, max_results)
    }

    pub fn local_projection_contains_record(
        &self,
        projection: &MemoryProjection,
        record_id: &str,
    ) -> bool {
        self.local
            .as_any()
            .downcast_ref::<LocalMemoryProvider>()
            .is_some_and(|local| local.projection_contains_record(projection, record_id))
    }

    pub fn append_local_record_to_projection_with_backup(
        &self,
        record: &MemoryRecord,
        projection: &MemoryProjection,
    ) -> anyhow::Result<()> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            return Ok(());
        };
        local.append_record_to_projection_with_backup(record, projection)
    }

    pub fn local_migration_file_reports(
        &self,
    ) -> anyhow::Result<(Vec<LocalMemoryMigrationFileReport>, Vec<String>)> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            return Ok((Vec::new(), Vec::new()));
        };
        Ok(local.migration_file_reports())
    }

    pub fn backup_local_memory_files(
        &self,
        backup_id: &str,
    ) -> anyhow::Result<LocalMemoryMigrationBackupReport> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            anyhow::bail!("local memory provider does not support migration backup");
        };
        local.migration_backup(backup_id)
    }

    pub fn rollback_local_memory_files(
        &self,
        backup_id: &str,
    ) -> anyhow::Result<LocalMemoryMigrationRollbackReport> {
        let Some(local) = self.local.as_any().downcast_ref::<LocalMemoryProvider>() else {
            anyhow::bail!("local memory provider does not support migration rollback");
        };
        local.migration_rollback(backup_id)
    }

    pub fn register_external(&mut self, provider: Arc<dyn MemoryProvider>) -> anyhow::Result<()> {
        if provider.name().trim().is_empty() {
            return Err(anyhow!("external memory provider name cannot be empty"));
        }
        if provider.name() == self.local.name() {
            return Err(anyhow!(
                "external memory provider '{}' conflicts with local provider",
                provider.name()
            ));
        }
        if provider.capabilities().write_mirror {
            return Err(anyhow!(
                "external memory provider '{}' requests write_mirror; external providers are read-only in the current policy",
                provider.name()
            ));
        }
        if provider.capabilities().tools {
            return Err(anyhow!(
                "external memory provider '{}' requests tool schema exposure; external provider tools are not enabled in the current policy",
                provider.name()
            ));
        }
        if let Some(existing) = self.external.as_ref() {
            return Err(anyhow!(
                "external memory provider '{}' already registered",
                existing.name()
            ));
        }
        self.external = Some(provider);
        Ok(())
    }

    pub fn providers(&self) -> Vec<Arc<dyn MemoryProvider>> {
        let mut providers = vec![self.local.clone()];
        if let Some(external) = self.external.as_ref() {
            providers.push(external.clone());
        }
        providers
    }

    pub async fn initialize_all(&self, scope: &MemoryScope) -> Vec<MemoryProviderCallOutcome> {
        self.fanout_unit("initialize", |provider| async move {
            provider.initialize(scope).await
        })
        .await
    }

    pub async fn queue_prefetch_all(
        &self,
        query: &str,
        scope: &MemoryScope,
    ) -> Vec<MemoryProviderCallOutcome> {
        self.fanout_unit("queue_prefetch", |provider| async move {
            provider.queue_prefetch(query, scope).await
        })
        .await
    }

    pub async fn sync_turn_all(
        &self,
        turn: &MemoryTurn,
        scope: &MemoryScope,
    ) -> Vec<MemoryProviderCallOutcome> {
        self.fanout_unit("sync_turn", |provider| async move {
            provider.sync_turn(turn, scope).await
        })
        .await
    }

    pub async fn on_session_end_all(
        &self,
        transcript: &[Message],
        scope: &MemoryScope,
    ) -> Vec<MemoryProviderCallOutcome> {
        self.fanout_unit("on_session_end", |provider| async move {
            provider.on_session_end(transcript, scope).await
        })
        .await
    }

    pub async fn on_memory_write_all(
        &self,
        record: &MemoryRecord,
        scope: &MemoryScope,
    ) -> Vec<MemoryProviderCallOutcome> {
        self.fanout_unit("on_memory_write", |provider| async move {
            provider.on_memory_write(record, scope).await
        })
        .await
    }

    pub async fn shutdown_all(&self) -> Vec<MemoryProviderCallOutcome> {
        self.fanout_unit(
            "shutdown",
            |provider| async move { provider.shutdown().await },
        )
        .await
    }

    pub async fn system_prompt_blocks(
        &self,
        scope: &MemoryScope,
    ) -> (Vec<String>, Vec<MemoryProviderCallOutcome>) {
        let mut blocks = Vec::new();
        let mut outcomes = Vec::new();
        for provider in self.providers() {
            if !provider.is_available() {
                outcomes.push(MemoryProviderCallOutcome::skipped(
                    provider.as_ref(),
                    "system_prompt_block",
                ));
                continue;
            }
            if !provider.capabilities().supports_hook("system_prompt_block") {
                outcomes.push(MemoryProviderCallOutcome::unsupported(
                    provider.as_ref(),
                    "system_prompt_block",
                ));
                continue;
            }
            match provider.system_prompt_block(scope).await {
                Ok(Some(block)) if !block.trim().is_empty() => {
                    blocks.push(block);
                    outcomes.push(MemoryProviderCallOutcome::ok(
                        provider.as_ref(),
                        "system_prompt_block",
                    ));
                }
                Ok(_) => outcomes.push(MemoryProviderCallOutcome::ok(
                    provider.as_ref(),
                    "system_prompt_block",
                )),
                Err(error) => outcomes.push(MemoryProviderCallOutcome::failed(
                    provider.as_ref(),
                    "system_prompt_block",
                    error,
                )),
            }
        }
        (blocks, outcomes)
    }

    pub async fn prefetch_all(
        &self,
        query: &str,
        scope: &MemoryScope,
    ) -> (Vec<MemoryRecord>, Vec<MemoryProviderCallOutcome>) {
        let mut records = Vec::new();
        let mut outcomes = Vec::new();
        for provider in self.providers() {
            if !provider.is_available() {
                outcomes.push(MemoryProviderCallOutcome::skipped(
                    provider.as_ref(),
                    "prefetch",
                ));
                continue;
            }
            if !provider.capabilities().supports_hook("prefetch") {
                outcomes.push(MemoryProviderCallOutcome::unsupported(
                    provider.as_ref(),
                    "prefetch",
                ));
                continue;
            }
            match provider.prefetch(query, scope).await {
                Ok(mut next) => {
                    records.append(&mut next);
                    outcomes.push(MemoryProviderCallOutcome::ok(provider.as_ref(), "prefetch"));
                }
                Err(error) => outcomes.push(MemoryProviderCallOutcome::failed(
                    provider.as_ref(),
                    "prefetch",
                    error,
                )),
            }
        }
        (records, outcomes)
    }

    pub async fn search_all(
        &self,
        query: &str,
        scope: &MemoryScope,
        max_results: usize,
    ) -> (Vec<MemoryRecord>, Vec<MemoryProviderCallOutcome>) {
        let mut records = Vec::new();
        let mut outcomes = Vec::new();
        for provider in self.providers() {
            if !provider.is_available() {
                outcomes.push(MemoryProviderCallOutcome::skipped(
                    provider.as_ref(),
                    "search",
                ));
                continue;
            }
            if !provider.capabilities().supports_hook("search") {
                outcomes.push(MemoryProviderCallOutcome::unsupported(
                    provider.as_ref(),
                    "search",
                ));
                continue;
            }
            match provider.search(query, scope, max_results).await {
                Ok(mut next) => {
                    records.append(&mut next);
                    outcomes.push(MemoryProviderCallOutcome::ok(provider.as_ref(), "search"));
                }
                Err(error) => outcomes.push(MemoryProviderCallOutcome::failed(
                    provider.as_ref(),
                    "search",
                    error,
                )),
            }
        }
        records.sort_by(local_provider_record_order);
        records.truncate(max_results);
        (records, outcomes)
    }

    pub async fn on_pre_compress_all(
        &self,
        messages: &[Message],
        scope: &MemoryScope,
    ) -> (Vec<MemoryRecord>, Vec<MemoryProviderCallOutcome>) {
        let mut records = Vec::new();
        let mut outcomes = Vec::new();
        for provider in self.providers() {
            if !provider.is_available() {
                outcomes.push(MemoryProviderCallOutcome::skipped(
                    provider.as_ref(),
                    "on_pre_compress",
                ));
                continue;
            }
            if !provider.capabilities().supports_hook("on_pre_compress") {
                outcomes.push(MemoryProviderCallOutcome::unsupported(
                    provider.as_ref(),
                    "on_pre_compress",
                ));
                continue;
            }
            match provider.on_pre_compress(messages, scope).await {
                Ok(mut next) => {
                    records.append(&mut next);
                    outcomes.push(MemoryProviderCallOutcome::ok(
                        provider.as_ref(),
                        "on_pre_compress",
                    ));
                }
                Err(error) => outcomes.push(MemoryProviderCallOutcome::failed(
                    provider.as_ref(),
                    "on_pre_compress",
                    error,
                )),
            }
        }
        (records, outcomes)
    }

    async fn fanout_unit<F, Fut>(
        &self,
        hook: &'static str,
        mut call: F,
    ) -> Vec<MemoryProviderCallOutcome>
    where
        F: FnMut(Arc<dyn MemoryProvider>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let mut outcomes = Vec::new();
        for provider in self.providers() {
            if !provider.is_available() {
                outcomes.push(MemoryProviderCallOutcome::skipped(provider.as_ref(), hook));
                continue;
            }
            if !provider.capabilities().supports_hook(hook) {
                outcomes.push(MemoryProviderCallOutcome::unsupported(
                    provider.as_ref(),
                    hook,
                ));
                continue;
            }
            match call(provider.clone()).await {
                Ok(()) => outcomes.push(MemoryProviderCallOutcome::ok(provider.as_ref(), hook)),
                Err(error) => {
                    outcomes.push(MemoryProviderCallOutcome::failed(
                        provider.as_ref(),
                        hook,
                        error,
                    ));
                }
            }
        }
        outcomes
    }
}

fn lifecycle_hooks() -> Vec<String> {
    MEMORY_PROVIDER_LIFECYCLE_HOOKS
        .iter()
        .map(|hook| (*hook).to_string())
        .collect()
}
