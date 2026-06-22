//! Memory provider support module.
//!
//! Defines provider contracts and no-network behavior for controlled memory lifecycle tests.

use crate::memory::types::{MemoryRecord, MemoryScope};
use async_trait::async_trait;
use std::path::Path;

use super::{
    filter_provider_records, read_local_memory_records, MemoryProvider, MemoryProviderCapabilities,
};

#[derive(Debug, Clone)]
pub struct NoNetworkMemoryProvider {
    name: String,
    records: Vec<MemoryRecord>,
    available: bool,
    capabilities: MemoryProviderCapabilities,
}

impl NoNetworkMemoryProvider {
    pub fn new(name: impl Into<String>, records: Vec<MemoryRecord>) -> Self {
        Self {
            name: name.into(),
            records,
            available: true,
            capabilities: MemoryProviderCapabilities::read_only(),
        }
    }

    pub fn with_capabilities(
        name: impl Into<String>,
        records: Vec<MemoryRecord>,
        capabilities: MemoryProviderCapabilities,
    ) -> Self {
        Self {
            name: name.into(),
            records,
            available: true,
            capabilities,
        }
    }

    pub fn from_jsonl_file(
        name: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        Ok(Self::new(name, read_local_memory_records(path.as_ref())?))
    }

    pub fn from_jsonl_file_with_capabilities(
        name: impl Into<String>,
        path: impl AsRef<Path>,
        capabilities: MemoryProviderCapabilities,
    ) -> anyhow::Result<Self> {
        Ok(Self::with_capabilities(
            name,
            read_local_memory_records(path.as_ref())?,
            capabilities,
        ))
    }

    pub fn unavailable(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            records: Vec::new(),
            available: false,
            capabilities: MemoryProviderCapabilities::read_only(),
        }
    }
}

#[async_trait]
impl MemoryProvider for NoNetworkMemoryProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn is_available(&self) -> bool {
        self.available
    }

    fn capabilities(&self) -> MemoryProviderCapabilities {
        self.capabilities
    }

    async fn prefetch(
        &self,
        query: &str,
        scope: &MemoryScope,
    ) -> anyhow::Result<Vec<MemoryRecord>> {
        Ok(filter_provider_records(&self.records, query, scope, 8))
    }

    async fn search(
        &self,
        query: &str,
        scope: &MemoryScope,
        max_results: usize,
    ) -> anyhow::Result<Vec<MemoryRecord>> {
        Ok(filter_provider_records(
            &self.records,
            query,
            scope,
            max_results,
        ))
    }
}
