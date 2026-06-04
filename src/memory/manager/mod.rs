//! 记忆管理器
//!
//! 参考 hermes-agent 的 MemoryManager 设计：
//! - 冻结快照：会话开始时冻结记忆，中间写入不 bust prompt cache
//! - 预取：每轮对话前搜索相关记忆注入上下文
//! - 同步：每轮结束后自动提取关键信息保存
//! - 会话结束提取：session 过期时批量提取学习内容

mod helpers;
mod learning;
mod migration;
mod review;
mod snapshot;
mod submit;

use crate::memory::files::{
    format_memory_file_manifest, hash_learning, load_memory_files, safe_memory_content_for_load,
};

#[cfg(test)]
use crate::engine::task_contract::MemoryProposalReviewStore;
#[cfg(test)]
use crate::memory::extraction::parse_llm_memory_candidates;
use crate::memory::provider::{
    LocalMemoryProvider, MemoryOperationJournalEntry, MemoryProviderRegistry,
};
use crate::memory::quality::assess_memory_candidate;
use crate::memory::search_index::MemorySearchDocument;
#[cfg(test)]
use crate::memory::types::{MemoryCandidate, MemoryProvenance};
#[cfg(test)]
use crate::memory::types::{MemoryEvidenceKind, MemoryEvidenceRef, MemoryKind};
use crate::memory::types::{MemoryRecord, MemoryScope, MemoryStatus};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, warn};

pub use crate::memory::reports::{
    MemoryDecisionCounts, MemoryEntry, MemoryFileSnapshot, MemoryFlushReason, MemoryFlushRecord,
    MemoryFlushStatus, MemoryFlushSummary, MemoryMaintenanceReport, MemoryMatch,
    MemoryMigrationFileReport, MemoryMigrationReport, MemoryProductContractReport,
    MemoryRecordSummary, MemoryReviewItem, MemoryReviewReport, MemorySnapshotReport, MemorySummary,
    MemoryTier, MemoryWriteOutcome, MemoryWriteOutcomeStatus, MemoryWriteTarget,
};

pub use self::helpers::{
    kind_label, log_preview, memory_decision_event, memory_llm_timeout, memory_messages_hash,
    memory_scope_label, record_needs_revalidation, MemoryDecisionEvent,
    MAX_LEARNINGS_PER_SESSION_EXTRACT, MAX_LEARNINGS_PER_TURN, MEMORY_DIR_NAME,
    MEMORY_FLUSH_LOG_FILE, MEMORY_FLUSH_MAX_ATTEMPTS, MEMORY_RECORDS_FILE,
};

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
struct BackgroundMemoryWriteDecision {
    source: String,
    status: MemoryWriteOutcomeStatus,
    quality_score: Option<f32>,
    wrote: bool,
    duplicate: bool,
    reason: String,
}

#[cfg(test)]
fn write_background_memory_candidate(
    path: &Path,
    candidate: &str,
    source: &str,
    scope: &MemoryScope,
) -> BackgroundMemoryWriteDecision {
    let base = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let candidate_obj = MemoryCandidate::new(
        candidate,
        "learned",
        scope.clone(),
        MemoryProvenance::local(source),
    );
    let outcome = MemoryManager::with_base_dir(base)
        .submit_candidate(candidate_obj, MemoryWriteTarget::Index);
    BackgroundMemoryWriteDecision {
        source: source.to_string(),
        status: outcome.status,
        quality_score: outcome.quality_score,
        wrote: outcome.status == MemoryWriteOutcomeStatus::Saved,
        duplicate: outcome.status == MemoryWriteOutcomeStatus::Duplicate,
        reason: outcome.reason,
    }
}

fn collect_memory_key_values(
    content: &str,
    out: &mut std::collections::HashMap<String, HashSet<String>>,
) {
    for line in content.lines().map(str::trim) {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key
            .trim()
            .trim_start_matches(['-', '*', '#', ' '])
            .to_lowercase();
        let value = value.trim().trim_matches('`').to_lowercase();
        if key.len() < 2 || key.len() > 48 || value.len() < 2 || value.len() > 180 {
            continue;
        }
        if key.contains("http") || value.contains("http") {
            continue;
        }
        out.entry(key).or_default().insert(value);
    }
}

fn memory_decision_counts_from_jsonl(content: &str) -> MemoryDecisionCounts {
    let mut counts = MemoryDecisionCounts::default();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<MemoryDecisionEvent>(line) {
            match event.decision.as_str() {
                "accepted" => counts.accepted += 1,
                "proposed" => counts.proposed += 1,
                "rejected" => counts.rejected += 1,
                "blocked" => counts.blocked += 1,
                _ => {}
            }
        }
    }
    counts
}

pub(super) fn memory_flush_records_from_jsonl(content: &str) -> HashMap<String, MemoryFlushRecord> {
    let mut records = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(record) = serde_json::from_str::<MemoryFlushRecord>(line) {
            records.insert(record.id.clone(), record);
        }
    }
    records
}

#[cfg(test)]
fn write_memory_records(path: &Path, records: &[MemoryRecord]) -> std::io::Result<()> {
    let content = records
        .iter()
        .map(|record| serde_json::to_string(record).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, content)
}

/// 记忆管理器
pub struct MemoryManager {
    /// MEMORY.md 路径
    pub(super) memory_path: PathBuf,
    /// USER.md 路径（用户偏好）
    pub(super) user_path: PathBuf,
    /// 分主题长期记忆目录（~/.priority-agent/memory/*.md）
    pub(super) memory_dir: PathBuf,
    /// 记忆决策日志（accepted/proposed/rejected/blocked）
    pub(super) decision_log_path: PathBuf,
    /// typed memory record sidecar (`memory/records.jsonl`)
    records_path: PathBuf,
    /// durable memory flush lifecycle log
    pub(super) flush_log_path: PathBuf,
    /// 冻结快照（会话开始时捕获，整个会话不变）
    pub(super) frozen_memory: Option<String>,
    frozen_user: Option<String>,
    pub(super) frozen_memory_files: Vec<MemoryFileSnapshot>,
    /// 字符限制
    memory_char_limit: usize,
    user_char_limit: usize,
    /// 本轮是否已预取
    pub(super) prefetched_this_turn: bool,
    /// 累积的学习内容（会话结束时批量保存）
    pub(super) pending_learnings: Vec<String>,
    /// 已记录的学习内容哈希（去重）
    pub(super) seen_hashes: HashSet<u64>,
    /// 本会话轮数（用于 throttle LLM 提取）
    turn_count: usize,
    /// 上次 LLM 提取的轮数
    last_llm_extraction_turn: usize,
    /// LLM 提取次数（用于 telemetry）
    llm_extraction_count: usize,
    /// 主 agent 已写入标记（mutual exclusion）
    main_agent_wrote_this_turn: bool,
    /// Forked agent 模式（环境变量 PRIORITY_AGENT_LLM_MEMORY_FORKED=1）
    pub(super) forked_mode: bool,
    /// Trailing run 模式（环境变量 PRIORITY_AGENT_LLM_MEMORY_TRAILING=1）
    pub(super) trailing_mode: bool,
    /// Trailing run 是否已执行
    pub(super) trailing_completed: bool,
    /// 缓存命中率统计
    cache_hits: usize,
    cache_misses: usize,
    /// Provider lifecycle registry. Local storage still lives in this manager
    /// during the first provider-boundary phase.
    pub(super) provider_registry: MemoryProviderRegistry,
    /// Active scope for memory candidates created by manager-owned write paths.
    pub(super) active_scope: MemoryScope,
}

impl MemoryManager {
    pub fn new() -> Self {
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent");

        let mut manager = Self::with_base_dir(base);
        match crate::services::config::AppConfig::load() {
            Ok(config) => {
                if let Err(error) = manager.configure_external_memory_provider_from_config(
                    &config.memory.external_provider,
                ) {
                    warn!("External memory provider config ignored: {}", error);
                }
            }
            Err(error) => {
                debug!("Memory manager using default provider config: {}", error);
            }
        }
        manager
    }

    /// 使用指定 base dir 创建记忆管理器。主要用于测试，也让上层可注入项目级存储位置。
    pub fn with_base_dir(base: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&base);
        let memory_dir = base.join(MEMORY_DIR_NAME);
        let _ = std::fs::create_dir_all(&memory_dir);

        // 从环境变量读取配置
        let forked_mode = std::env::var("PRIORITY_AGENT_LLM_MEMORY_FORKED")
            .ok()
            .map(|v| v == "1")
            .unwrap_or(false);

        let trailing_mode = std::env::var("PRIORITY_AGENT_LLM_MEMORY_TRAILING")
            .ok()
            .map(|v| v == "1")
            .unwrap_or(false);

        Self {
            memory_path: base.join("MEMORY.md"),
            user_path: base.join("USER.md"),
            decision_log_path: base.join(MEMORY_DIR_NAME).join("decisions.jsonl"),
            records_path: base.join(MEMORY_DIR_NAME).join(MEMORY_RECORDS_FILE),
            flush_log_path: base.join(MEMORY_DIR_NAME).join(MEMORY_FLUSH_LOG_FILE),
            memory_dir,
            frozen_memory: None,
            frozen_user: None,
            frozen_memory_files: Vec::new(),
            memory_char_limit: 3000,
            user_char_limit: 1500,
            prefetched_this_turn: false,
            pending_learnings: Vec::new(),
            seen_hashes: HashSet::new(),
            turn_count: 0,
            last_llm_extraction_turn: 0,
            llm_extraction_count: 0,
            main_agent_wrote_this_turn: false,
            forked_mode,
            trailing_mode,
            trailing_completed: false,
            cache_hits: 0,
            cache_misses: 0,
            provider_registry: MemoryProviderRegistry::with_local(Arc::new(
                LocalMemoryProvider::with_base_dir(base.clone()),
            )),
            active_scope: MemoryScope::local("unknown"),
        }
    }

    pub fn records_path(&self) -> &Path {
        &self.records_path
    }

    pub fn search_index_path(&self) -> PathBuf {
        self.provider_registry
            .local_search_index_path()
            .unwrap_or_else(|| self.memory_dir.join("search.sqlite"))
    }

    pub fn active_scope(&self) -> MemoryScope {
        self.active_scope.clone()
    }

    pub fn set_active_scope(&mut self, scope: MemoryScope) {
        self.active_scope = scope;
    }

    pub fn memory_records(&self) -> Vec<MemoryRecord> {
        self.provider_registry
            .local_memory_records()
            .unwrap_or_else(|error| {
                warn!("failed to read local memory provider records: {error}");
                Vec::new()
            })
    }

    pub fn memory_operation_journal(&self) -> Vec<MemoryOperationJournalEntry> {
        self.provider_registry
            .local_memory_operation_journal()
            .unwrap_or_else(|error| {
                warn!("failed to read local memory operation journal: {error}");
                Vec::new()
            })
    }

    pub(super) fn search_index_documents(&self) -> Vec<MemorySearchDocument> {
        let mut documents = Vec::new();

        // MEMORY.md content
        let memory_content = std::fs::read_to_string(&self.memory_path).unwrap_or_default();
        if !memory_content.trim().is_empty() {
            documents.push(MemorySearchDocument {
                source: "MEMORY.md".to_string(),
                title: "Project Memory".to_string(),
                content: memory_content,
                kind: "project".to_string(),
                scope: memory_scope_label(&self.active_scope),
            });
        }

        // USER.md content
        let user_content = std::fs::read_to_string(&self.user_path).unwrap_or_default();
        if !user_content.trim().is_empty() {
            documents.push(MemorySearchDocument {
                source: "USER.md".to_string(),
                title: "User Memory".to_string(),
                content: user_content,
                kind: "user".to_string(),
                scope: "user".to_string(),
            });
        }

        // Topic memory files
        for file in crate::memory::files::load_memory_files(&self.memory_dir) {
            if !file.content.trim().is_empty() {
                documents.push(MemorySearchDocument {
                    source: format!("memory/{}", file.relative_path),
                    title: file.relative_path.clone(),
                    content: file.content,
                    kind: "topic".to_string(),
                    scope: memory_scope_label(&self.active_scope),
                });
            }
        }

        // Typed records
        for record in self.memory_records() {
            if matches!(
                record.status,
                MemoryStatus::Accepted | MemoryStatus::Proposed
            ) {
                documents.push(MemorySearchDocument {
                    source: format!("memory_record/{}", record.id),
                    title: kind_label(record.kind).to_string(),
                    content: format!("{} {}", record.summary, record.content),
                    kind: kind_label(record.kind).to_string(),
                    scope: memory_scope_label(&record.scope),
                });
            }
        }

        documents
    }

    pub fn memory_conflicts(&self, max_conflicts: usize) -> Vec<String> {
        let mut by_key: std::collections::HashMap<String, HashSet<String>> =
            std::collections::HashMap::new();
        collect_memory_key_values(&self.load_tier(MemoryTier::Project), &mut by_key);
        collect_memory_key_values(&self.load_tier(MemoryTier::User), &mut by_key);
        for file in load_memory_files(&self.memory_dir) {
            collect_memory_key_values(&file.content, &mut by_key);
        }

        let mut conflicts: Vec<String> = by_key
            .into_iter()
            .filter_map(|(key, values)| {
                if values.len() > 1 {
                    let mut values = values.into_iter().collect::<Vec<_>>();
                    values.sort();
                    Some(format!(
                        "- key '{}' has conflicting values: {}",
                        key,
                        values.join(" | ")
                    ))
                } else {
                    None
                }
            })
            .collect();
        conflicts.sort();
        conflicts.truncate(max_conflicts);
        conflicts
    }

    /// 保存 Workflow 决策到记忆
    ///
    /// 将 Gate 决策、计划审批结果、执行结果等工作流关键决策
    /// 写入 Project Memory，供未来会话参考。
    pub fn save_workflow_decision(
        &mut self,
        decision_type: &str,
        task: &str,
        outcome: &str,
        reasoning: &str,
    ) {
        let content = format!(
            "[{}] Task: {} | Outcome: {} | Reason: {}",
            decision_type, task, outcome, reasoning
        );
        self.add_learning(&content, "workflow");
    }

    /// 异步保存 Workflow 决策
    pub async fn save_workflow_decision_async(
        &self,
        decision_type: &str,
        task: &str,
        outcome: &str,
        reasoning: &str,
    ) {
        let content = format!(
            "[{}] Task: {} | Outcome: {} | Reason: {}",
            decision_type, task, outcome, reasoning
        );
        self.add_learning_async(&content, "workflow").await;
    }

    /// 重置预取状态（每轮开始时调用）
    pub fn reset_turn(&mut self) {
        self.prefetched_this_turn = false;
        self.main_agent_wrote_this_turn = false;
    }

    /// 本轮结束，增加轮数计数
    pub fn increment_turn(&mut self) {
        self.turn_count += 1;
    }

    /// 获取 LLM 提取间隔（环境变量可配置）
    pub fn llm_extraction_interval() -> usize {
        std::env::var("PRIORITY_AGENT_LLM_MEMORY_INTERVAL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5)
    }

    /// 获取 telemetry 统计
    pub fn extraction_stats(&self) -> (usize, usize, usize) {
        (
            self.llm_extraction_count,
            self.turn_count,
            self.last_llm_extraction_turn,
        )
    }

    /// 获取缓存命中率统计
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.cache_hits, self.cache_misses)
    }

    pub fn memory_decision_counts(&self) -> MemoryDecisionCounts {
        let content = std::fs::read_to_string(&self.decision_log_path).unwrap_or_default();
        memory_decision_counts_from_jsonl(&content)
    }

    pub fn memory_flush_summary(&self) -> MemoryFlushSummary {
        let content = std::fs::read_to_string(&self.flush_log_path).unwrap_or_default();
        let records = memory_flush_records_from_jsonl(&content);
        let mut summary = MemoryFlushSummary {
            total: records.len(),
            ..Default::default()
        };
        for record in records.values() {
            match record.status {
                MemoryFlushStatus::Pending => summary.pending += 1,
                MemoryFlushStatus::Running => summary.running += 1,
                MemoryFlushStatus::Completed => summary.completed += 1,
                MemoryFlushStatus::Failed => summary.failed += 1,
                MemoryFlushStatus::SkippedDuplicate => summary.skipped_duplicate += 1,
                MemoryFlushStatus::SkippedReviewOnly => summary.skipped_review_only += 1,
            }
        }
        summary
    }

    /// 检查是否有自某时间点以来的记忆写入（用于 forked agent 互斥）
    pub fn has_memory_writes_since(&self, turn: usize) -> bool {
        // 如果主 agent 在指定 turn 之后写过，返回 true
        // 这会阻止 forked agent 在主 agent 已写入后进行提取
        // 当前实现基于 main_agent_wrote_this_turn，它每轮重置
        // 对于精确的 turn 检查，我们依赖 throttle 机制
        self.main_agent_wrote_this_turn && self.turn_count >= turn
    }

    /// 主 agent 已写入，阻止后台 LLM 提取
    pub fn mark_main_agent_wrote(&mut self) {
        self.main_agent_wrote_this_turn = true;
    }

    /// 检查是否应进行 LLM 提取（throttle + mutual exclusion）
    pub fn should_extract_with_llm(&self) -> bool {
        // mutual exclusion：主 agent 已写则跳过
        if self.main_agent_wrote_this_turn {
            return false;
        }
        // throttle：每 N 轮提取一次
        let interval = Self::llm_extraction_interval();
        self.turn_count - self.last_llm_extraction_turn >= interval
    }

    /// 记录一次 LLM/forked 记忆提取已启动，用于 throttle 和 telemetry。
    pub fn mark_llm_extraction_started(&mut self) {
        self.last_llm_extraction_turn = self.turn_count;
        self.llm_extraction_count += 1;
    }

    /// 是否启用了 forked 模式
    pub fn is_forked_mode(&self) -> bool {
        self.forked_mode
    }

    /// 是否启用了 trailing 模式
    pub fn is_trailing_mode(&self) -> bool {
        self.trailing_mode
    }

    /// Trailing run 是否已完成
    pub fn is_trailing_completed(&self) -> bool {
        self.trailing_completed
    }

    /// 标记 trailing run 已完成
    pub fn mark_trailing_completed(&mut self) {
        self.trailing_completed = true;
    }

    /// 检查内容是否已重复
    pub fn is_duplicate(&self, content: &str) -> bool {
        let hash = hash_learning(content);
        self.seen_hashes.contains(&hash)
    }

    /// 加载指定层级的记忆内容
    pub fn load_tier(&self, tier: MemoryTier) -> String {
        match tier {
            MemoryTier::Session => {
                // Session memory is transient - return empty for injection
                String::new()
            }
            MemoryTier::Project => {
                let content = std::fs::read_to_string(&self.memory_path).unwrap_or_default();
                let content =
                    safe_memory_content_for_load("MEMORY.md", &content).unwrap_or_default();
                let trimmed = content.trim();
                let manifest =
                    format_memory_file_manifest(&load_memory_files(&self.memory_dir), 2000);
                if trimmed.is_empty() && manifest.trim().is_empty() {
                    String::new()
                } else {
                    let mut parts = Vec::new();
                    if !trimmed.is_empty() {
                        parts.push(format!(
                            "[Project Memory]\n{}",
                            trimmed
                                .chars()
                                .take(self.memory_char_limit)
                                .collect::<String>()
                        ));
                    }
                    if !manifest.trim().is_empty() {
                        parts.push(format!("[Memory File Index]\n{}", manifest));
                    }
                    parts.join("\n\n")
                }
            }
            MemoryTier::User => {
                let content = std::fs::read_to_string(&self.user_path).unwrap_or_default();
                let content = safe_memory_content_for_load("USER.md", &content).unwrap_or_default();
                let trimmed = content.trim();
                if trimmed.is_empty() {
                    String::new()
                } else {
                    format!(
                        "[User Preferences]\n{}",
                        trimmed
                            .chars()
                            .take(self.user_char_limit)
                            .collect::<String>()
                    )
                }
            }
        }
    }

    /// 获取所有层级记忆的摘要（用于上下文可视化）
    pub fn memory_summary(&self) -> MemorySummary {
        let project_size = std::fs::read_to_string(&self.memory_path)
            .map(|s| s.len())
            .unwrap_or(0);
        let memory_files = load_memory_files(&self.memory_dir);
        let memory_file_chars = memory_files.iter().map(|file| file.chars).sum();
        let user_size = std::fs::read_to_string(&self.user_path)
            .map(|s| s.len())
            .unwrap_or(0);
        let session_count = self.pending_learnings.len();

        MemorySummary {
            project_memory_chars: project_size,
            project_memory_files: memory_files.len(),
            project_memory_file_chars: memory_file_chars,
            user_memory_chars: user_size,
            session_memory_items: session_count,
            has_frozen_snapshot: self.frozen_memory.is_some()
                || self.frozen_user.is_some()
                || !self.frozen_memory_files.is_empty(),
        }
    }

    /// 尝试添加学习内容到 pending，去重
    fn push_learning(&mut self, content: String) {
        let content = content.trim();
        if !Self::passes_quality_gate(content) {
            debug!(
                "Skip low-signal memory candidate: {}",
                log_preview(content, 60)
            );
            return;
        }
        let hash = hash_learning(content);
        if self.seen_hashes.insert(hash) {
            self.pending_learnings.push(content.to_string());
        }
    }

    pub(super) fn ingest_learnings(&mut self, learnings: Vec<String>, max_items: usize) {
        let mut accepted = 0usize;
        for learning in learnings {
            let before = self.pending_learnings.len();
            self.push_learning(learning);
            if self.pending_learnings.len() > before {
                accepted += 1;
            }
            if accepted >= max_items {
                break;
            }
        }
    }

    pub(super) fn passes_quality_gate(content: &str) -> bool {
        assess_memory_candidate(content, "learned", "", false)
            .map(|assessment| assessment.status == MemoryStatus::Accepted)
            .unwrap_or(false)
    }

    /// 获取待保存的学习内容数量
    pub fn pending_count(&self) -> usize {
        self.pending_learnings.len()
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
fn parse_llm_memory_candidate_contents(content: &str) -> Vec<String> {
    parse_llm_memory_candidates(
        content,
        MemoryScope::local("parse-preview"),
        MemoryProvenance::local("parse_preview"),
    )
    .into_iter()
    .map(|candidate| candidate.content)
    .collect()
}

#[cfg(test)]
mod tests;
