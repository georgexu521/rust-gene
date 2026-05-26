use crate::memory::types::{MemoryRecord, MemoryScope};
use crate::services::api::Message;
use anyhow::anyhow;
use async_trait::async_trait;
use std::future::Future;
use std::sync::Arc;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryProviderCallStatus {
    Ok,
    SkippedUnavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProviderCallOutcome {
    pub provider: String,
    pub hook: &'static str,
    pub status: MemoryProviderCallStatus,
    pub error: Option<String>,
}

impl MemoryProviderCallOutcome {
    fn ok(provider: &dyn MemoryProvider, hook: &'static str) -> Self {
        Self {
            provider: provider.name().to_string(),
            hook,
            status: MemoryProviderCallStatus::Ok,
            error: None,
        }
    }

    fn skipped(provider: &dyn MemoryProvider, hook: &'static str) -> Self {
        Self {
            provider: provider.name().to_string(),
            hook,
            status: MemoryProviderCallStatus::SkippedUnavailable,
            error: None,
        }
    }

    fn failed(provider: &dyn MemoryProvider, hook: &'static str, error: anyhow::Error) -> Self {
        Self {
            provider: provider.name().to_string(),
            hook,
            status: MemoryProviderCallStatus::Failed,
            error: Some(error.to_string()),
        }
    }
}

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
            local: Arc::new(LocalMemoryProvider),
            external: None,
        }
    }

    pub fn with_local_for_tests(local: Arc<dyn MemoryProvider>) -> Self {
        Self {
            local,
            external: None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;

    #[derive(Debug)]
    struct TestProvider {
        name: &'static str,
        available: AtomicBool,
        initialize_calls: AtomicUsize,
        queue_calls: AtomicUsize,
        fail_initialize: bool,
        fail_prefetch: bool,
        prefetch_records: Mutex<Vec<MemoryRecord>>,
        observed_scopes: Mutex<Vec<MemoryScope>>,
    }

    impl TestProvider {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                available: AtomicBool::new(true),
                initialize_calls: AtomicUsize::new(0),
                queue_calls: AtomicUsize::new(0),
                fail_initialize: false,
                fail_prefetch: false,
                prefetch_records: Mutex::new(Vec::new()),
                observed_scopes: Mutex::new(Vec::new()),
            }
        }

        fn unavailable(name: &'static str) -> Self {
            let provider = Self::new(name);
            provider.available.store(false, Ordering::SeqCst);
            provider
        }

        fn failing(name: &'static str) -> Self {
            Self {
                fail_initialize: true,
                ..Self::new(name)
            }
        }

        fn failing_prefetch(name: &'static str) -> Self {
            Self {
                fail_prefetch: true,
                ..Self::new(name)
            }
        }

        fn with_prefetch_record(name: &'static str, record: MemoryRecord) -> Self {
            let provider = Self::new(name);
            provider.prefetch_records.lock().unwrap().push(record);
            provider
        }
    }

    #[async_trait]
    impl MemoryProvider for TestProvider {
        fn name(&self) -> &str {
            self.name
        }

        fn is_available(&self) -> bool {
            self.available.load(Ordering::SeqCst)
        }

        async fn initialize(&self, _scope: &MemoryScope) -> anyhow::Result<()> {
            self.initialize_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_initialize {
                Err(anyhow!("init failed"))
            } else {
                Ok(())
            }
        }

        async fn queue_prefetch(&self, _query: &str, _scope: &MemoryScope) -> anyhow::Result<()> {
            self.queue_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn prefetch(
            &self,
            _query: &str,
            scope: &MemoryScope,
        ) -> anyhow::Result<Vec<MemoryRecord>> {
            self.observed_scopes.lock().unwrap().push(scope.clone());
            if self.fail_prefetch {
                Err(anyhow!("prefetch failed"))
            } else {
                Ok(self.prefetch_records.lock().unwrap().clone())
            }
        }
    }

    #[tokio::test]
    async fn registry_fans_out_unit_hooks_to_local_and_external_provider() {
        let local = Arc::new(TestProvider::new("local-test"));
        let external = Arc::new(TestProvider::new("external-test"));
        let mut registry = MemoryProviderRegistry::with_local_for_tests(local.clone());
        registry.register_external(external.clone()).unwrap();

        let outcomes = registry
            .initialize_all(&MemoryScope::local("session"))
            .await;

        assert_eq!(outcomes.len(), 2);
        assert!(outcomes
            .iter()
            .all(|outcome| outcome.status == MemoryProviderCallStatus::Ok));
        assert_eq!(local.initialize_calls.load(Ordering::SeqCst), 1);
        assert_eq!(external.initialize_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn registry_rejects_second_external_provider() {
        let mut registry = MemoryProviderRegistry::new();

        registry
            .register_external(Arc::new(TestProvider::new("external-one")))
            .unwrap();
        let error = registry
            .register_external(Arc::new(TestProvider::new("external-two")))
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("external memory provider 'external-one' already registered"));
    }

    #[tokio::test]
    async fn registry_skips_unavailable_and_reports_failures_without_stopping_fanout() {
        let local = Arc::new(TestProvider::unavailable("local-test"));
        let external = Arc::new(TestProvider::failing("external-test"));
        let mut registry = MemoryProviderRegistry::with_local_for_tests(local.clone());
        registry.register_external(external.clone()).unwrap();

        let outcomes = registry
            .initialize_all(&MemoryScope::local("session"))
            .await;

        assert_eq!(outcomes.len(), 2);
        assert_eq!(
            outcomes[0].status,
            MemoryProviderCallStatus::SkippedUnavailable
        );
        assert_eq!(outcomes[1].status, MemoryProviderCallStatus::Failed);
        assert_eq!(local.initialize_calls.load(Ordering::SeqCst), 0);
        assert_eq!(external.initialize_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn registry_prefetch_collects_records_and_isolates_provider_failure() {
        let mut scope = MemoryScope::local("session-provider-prefetch");
        scope.project_root = Some(std::path::PathBuf::from("/tmp/provider-project"));
        let record = MemoryRecord::new(
            "Use cargo check before closeout",
            crate::memory::types::MemoryKind::WorkflowConvention,
            scope.clone(),
            crate::memory::types::MemoryProvenance::local("test"),
        );
        let local = Arc::new(TestProvider::with_prefetch_record("local-test", record));
        let external = Arc::new(TestProvider::failing_prefetch("external-test"));
        let mut registry = MemoryProviderRegistry::with_local_for_tests(local.clone());
        registry.register_external(external.clone()).unwrap();

        let (records, outcomes) = registry.prefetch_all("cargo check", &scope).await;

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].scope, scope);
        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].status, MemoryProviderCallStatus::Ok);
        assert_eq!(outcomes[1].status, MemoryProviderCallStatus::Failed);
        assert!(outcomes[1]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("prefetch failed"));
        assert_eq!(local.observed_scopes.lock().unwrap().as_slice(), &[scope]);
    }
}
