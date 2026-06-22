//! Memory provider support module.
//!
//! Defines provider contracts and no-network behavior for controlled memory lifecycle tests.

use serde::{Deserialize, Serialize};

use super::traits::MemoryProvider;

#[derive(Debug, Clone)]
pub struct MemoryTurn {
    pub user: String,
    pub assistant: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryProviderCallStatus {
    Ok,
    SkippedUnavailable,
    SkippedUnsupported,
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
    pub(super) fn ok(provider: &dyn MemoryProvider, hook: &'static str) -> Self {
        Self {
            provider: provider.name().to_string(),
            hook,
            status: MemoryProviderCallStatus::Ok,
            error: None,
        }
    }

    pub(super) fn skipped(provider: &dyn MemoryProvider, hook: &'static str) -> Self {
        Self {
            provider: provider.name().to_string(),
            hook,
            status: MemoryProviderCallStatus::SkippedUnavailable,
            error: None,
        }
    }

    pub(super) fn unsupported(provider: &dyn MemoryProvider, hook: &'static str) -> Self {
        Self {
            provider: provider.name().to_string(),
            hook,
            status: MemoryProviderCallStatus::SkippedUnsupported,
            error: None,
        }
    }

    pub(super) fn failed(
        provider: &dyn MemoryProvider,
        hook: &'static str,
        error: anyhow::Error,
    ) -> Self {
        Self {
            provider: provider.name().to_string(),
            hook,
            status: MemoryProviderCallStatus::Failed,
            error: Some(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProviderLifecycleEntry {
    pub name: String,
    pub kind: String,
    pub available: bool,
    pub hooks: Vec<String>,
    pub capabilities: MemoryProviderCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProviderLifecycleReport {
    pub providers: Vec<MemoryProviderLifecycleEntry>,
    pub external_provider: Option<String>,
    pub lifecycle_hooks: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProviderCapabilities {
    pub prompt_block: bool,
    pub prefetch: bool,
    pub search: bool,
    pub queue_prefetch: bool,
    pub sync_turn: bool,
    pub session_end: bool,
    pub pre_compress: bool,
    pub write_mirror: bool,
    pub tools: bool,
}

impl MemoryProviderCapabilities {
    pub fn local() -> Self {
        Self {
            prompt_block: true,
            prefetch: true,
            search: true,
            queue_prefetch: true,
            sync_turn: true,
            session_end: true,
            pre_compress: true,
            write_mirror: true,
            tools: false,
        }
    }

    pub fn read_only() -> Self {
        Self {
            prompt_block: true,
            prefetch: true,
            search: true,
            queue_prefetch: false,
            sync_turn: false,
            session_end: false,
            pre_compress: false,
            write_mirror: false,
            tools: false,
        }
    }

    pub fn supported_hooks(self) -> Vec<String> {
        let mut hooks = Vec::new();
        hooks.push("initialize".to_string());
        if self.prompt_block {
            hooks.push("system_prompt_block".to_string());
        }
        if self.prefetch {
            hooks.push("prefetch".to_string());
        }
        if self.search {
            hooks.push("search".to_string());
        }
        if self.queue_prefetch {
            hooks.push("queue_prefetch".to_string());
        }
        if self.sync_turn {
            hooks.push("sync_turn".to_string());
        }
        if self.session_end {
            hooks.push("on_session_end".to_string());
        }
        if self.pre_compress {
            hooks.push("on_pre_compress".to_string());
        }
        if self.write_mirror {
            hooks.push("on_memory_write".to_string());
        }
        hooks.push("shutdown".to_string());
        hooks
    }

    pub(super) fn supports_hook(self, hook: &str) -> bool {
        match hook {
            "initialize" | "shutdown" => true,
            "system_prompt_block" => self.prompt_block,
            "prefetch" => self.prefetch,
            "search" => self.search,
            "queue_prefetch" => self.queue_prefetch,
            "sync_turn" => self.sync_turn,
            "on_session_end" => self.session_end,
            "on_pre_compress" => self.pre_compress,
            "on_memory_write" => self.write_mirror,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalMemoryRecordWriteStatus {
    Written,
    Duplicate,
}
