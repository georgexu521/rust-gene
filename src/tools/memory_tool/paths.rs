use std::path::PathBuf;

pub(super) fn memory_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
}

/// MEMORY.md 文件路径
pub(super) fn memory_path() -> PathBuf {
    memory_root().join("MEMORY.md")
}

pub(super) fn user_path() -> PathBuf {
    memory_root().join("USER.md")
}

pub(super) fn memory_dir() -> PathBuf {
    memory_root().join("memory")
}

pub(super) fn legacy_agent_memory_dir() -> PathBuf {
    memory_root().join("agent_memories")
}

pub(super) fn memory_decision_log_path() -> PathBuf {
    memory_dir().join("decisions.jsonl")
}

pub(super) fn memory_flush_log_path() -> PathBuf {
    memory_dir().join("flush_queue.jsonl")
}

pub(super) fn memory_retrieval_trace_path() -> PathBuf {
    memory_dir().join("retrieval_trace.json")
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct MemoryStorePathsJson {
    pub(super) memory_md: String,
    pub(super) user_md: String,
    pub(super) memory_dir: String,
    pub(super) records_jsonl: String,
    pub(super) operations_jsonl: String,
    pub(super) proposals_jsonl: String,
    pub(super) retrieval_trace_json: String,
    pub(super) decisions_jsonl: String,
    pub(super) flush_queue_jsonl: String,
}

pub(super) fn memory_store_paths() -> MemoryStorePathsJson {
    MemoryStorePathsJson {
        memory_md: memory_path().display().to_string(),
        user_md: user_path().display().to_string(),
        memory_dir: memory_dir().display().to_string(),
        records_jsonl: memory_dir().join("records.jsonl").display().to_string(),
        operations_jsonl: memory_dir().join("operations.jsonl").display().to_string(),
        proposals_jsonl: crate::engine::task_contract::MemoryProposalReviewStore::default_path()
            .display()
            .to_string(),
        retrieval_trace_json: memory_retrieval_trace_path().display().to_string(),
        decisions_jsonl: memory_decision_log_path().display().to_string(),
        flush_queue_jsonl: memory_flush_log_path().display().to_string(),
    }
}

pub(super) fn format_memory_store_paths(paths: &MemoryStorePathsJson) -> String {
    format!(
        "  Store paths:\n    MEMORY.md: {}\n    USER.md: {}\n    records: {}\n    operations: {}\n    proposals: {}\n    retrieval_trace: {}\n    decisions: {}\n    flush_queue: {}\n",
        paths.memory_md,
        paths.user_md,
        paths.records_jsonl,
        paths.operations_jsonl,
        paths.proposals_jsonl,
        paths.retrieval_trace_json,
        paths.decisions_jsonl,
        paths.flush_queue_jsonl
    )
}
