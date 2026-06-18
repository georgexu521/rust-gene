//! 上下文折叠服务
//!
//! 将历史消息持久化到文件，读取时重放，类似 Claude Code 的 `CONTEXT_COLLAPSE`
//! 当会话过长时，将早期消息折叠到磁盘，只保留最近 N 条消息在内存中

use crate::services::api::Message;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// ── Compact Boundary 元数据 ───────────────────────────────

/// 压缩边界元数据（对标 Claude Code 的 compact_boundary）
/// 嵌入在压缩后的摘要消息内容中，用于：
/// 1. 标识压缩发生的位置
/// 2. 记录被保留的尾部消息 UUID（用于恢复）
/// 3. 追踪压缩历史
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompactMetadata {
    /// 压缩序列号（单调递增）
    pub sequence: u32,
    /// 压缩边界唯一 ID
    pub boundary_id: String,
    /// 被保留的尾部消息数量
    pub preserved_tail_count: usize,
    /// 压缩前的消息总数
    pub messages_before: usize,
    /// 压缩后的消息总数
    pub messages_after: usize,
    /// 压缩前的 token 数
    pub tokens_before: u64,
    /// 压缩后的 token 数
    pub tokens_after: u64,
    /// 压缩时间戳
    pub timestamp: String,
}

impl CompactMetadata {
    /// 生成 compact boundary 标记文本（嵌入到消息内容中）
    pub fn to_boundary_marker(&self) -> String {
        format!(
            "\n[COMPACT_BOUNDARY seq={} id={} preserved={} before_msgs={} after_msgs={} before_tokens={} after_tokens={} timestamp={}]",
            self.sequence,
            self.boundary_id,
            self.preserved_tail_count,
            self.messages_before,
            self.messages_after,
            self.tokens_before,
            self.tokens_after,
            self.timestamp
        )
    }

    /// 从消息内容中解析 compact boundary 标记
    pub fn parse_from_text(text: &str) -> Option<(Self, String)> {
        let marker_start = text.find("[COMPACT_BOUNDARY")?;
        let marker_end = text[marker_start..].find(']')? + marker_start + 1;
        let marker = &text[marker_start..marker_end];
        let clean_text = format!("{}{}", &text[..marker_start], &text[marker_end..]);

        let mut seq = 0u32;
        let mut id = String::new();
        let mut preserved = 0usize;
        let mut before_msgs = 0usize;
        let mut after_msgs = 0usize;
        let mut before_tok = 0u64;
        let mut after_tok = 0u64;
        let mut timestamp = String::new();

        for part in marker.split_whitespace() {
            if let Some((k, v)) = part.split_once('=') {
                match k {
                    "seq" => seq = v.parse().unwrap_or(0),
                    "id" => id = v.to_string(),
                    "preserved" => preserved = v.parse().unwrap_or(0),
                    "before_msgs" => before_msgs = v.parse().unwrap_or(0),
                    "after_msgs" => after_msgs = v.parse().unwrap_or(0),
                    "before_tokens" => before_tok = v.parse().unwrap_or(0),
                    "after_tokens" => after_tok = v.parse().unwrap_or(0),
                    "timestamp" => timestamp = v.to_string(),
                    _ => {}
                }
            }
        }

        Some((
            Self {
                sequence: seq,
                boundary_id: id,
                preserved_tail_count: preserved,
                messages_before: before_msgs,
                messages_after: after_msgs,
                tokens_before: before_tok,
                tokens_after: after_tok,
                timestamp,
            },
            clean_text,
        ))
    }
}

/// 从消息列表中提取所有 compact boundary 元数据
pub fn extract_compact_boundaries(messages: &[Message]) -> Vec<CompactMetadata> {
    let mut result = Vec::new();
    for msg in messages {
        let text = message_content(msg);
        if text.contains("[COMPACT_BOUNDARY") {
            if let Some((meta, _)) = CompactMetadata::parse_from_text(text) {
                result.push(meta);
            }
        }
    }
    result
}

fn message_content(message: &Message) -> &str {
    match message {
        Message::System { content }
        | Message::User { content }
        | Message::Assistant { content, .. }
        | Message::Tool { content, .. } => content,
    }
}

/// Runtime-visible compaction strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCompactionStrategy {
    NoOp,
    Snip,
    MicroCompact,
    AutoCompact,
    ReactiveCompact,
    SessionMemoryCompact,
}

impl ContextCompactionStrategy {
    pub fn label(self) -> &'static str {
        match self {
            ContextCompactionStrategy::NoOp => "no_op",
            ContextCompactionStrategy::Snip => "snip",
            ContextCompactionStrategy::MicroCompact => "microcompact",
            ContextCompactionStrategy::AutoCompact => "auto_compact",
            ContextCompactionStrategy::ReactiveCompact => "reactive_compact",
            ContextCompactionStrategy::SessionMemoryCompact => "session_memory_compact",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionDecision {
    Considered,
    Skipped,
    Compacted,
    NoGain,
    Failed,
    CircuitOpen,
    Retrying,
    Recovered,
}

impl CompactionDecision {
    pub fn label(self) -> &'static str {
        match self {
            Self::Considered => "considered",
            Self::Skipped => "skipped",
            Self::Compacted => "compacted",
            Self::NoGain => "no_gain",
            Self::Failed => "failed",
            Self::CircuitOpen => "circuit_open",
            Self::Retrying => "retrying",
            Self::Recovered => "recovered",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CompactionAttemptRecord {
    pub trigger: String,
    pub strategy: ContextCompactionStrategy,
    pub decision: CompactionDecision,
    pub before_tokens: u64,
    pub after_tokens: Option<u64>,
    pub messages_before: usize,
    pub messages_after: Option<usize>,
    pub reason: String,
    pub attempt_index: u32,
    pub consecutive_no_gain: u32,
    pub consecutive_failures: u32,
    pub circuit_open: bool,
    #[serde(default)]
    pub boundary_id: Option<String>,
}

/// Runtime-visible token pressure at the time compaction was considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextTokenPressure {
    Low,
    Moderate,
    High,
    Critical,
}

impl ContextTokenPressure {
    pub fn from_usage_ratio(ratio: f64) -> Self {
        if ratio >= 0.90 {
            Self::Critical
        } else if ratio >= 0.80 {
            Self::High
        } else if ratio >= 0.60 {
            Self::Moderate
        } else {
            Self::Low
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Moderate => "moderate",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CompactionRuntimeRecord {
    pub strategy: ContextCompactionStrategy,
    pub level: Option<String>,
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub token_pressure: Option<ContextTokenPressure>,
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_before: u64,
    pub tokens_after: u64,
    #[serde(default)]
    pub token_delta: i64,
    #[serde(default)]
    pub stage_order: Vec<String>,
    pub boundary_id: Option<String>,
    pub sequence: Option<u32>,
    pub preserved_tail_count: Option<usize>,
    #[serde(default)]
    pub retained_items: Vec<String>,
    pub provenance: Vec<String>,
}

impl CompactionRuntimeRecord {
    pub fn normalize_provenance(&mut self) {
        let strategy_tag = format!("strategy:{}", self.strategy.label());
        if !self.provenance.iter().any(|tag| tag == &strategy_tag) {
            self.provenance.insert(0, strategy_tag);
        }
        if let Some(boundary_id) = &self.boundary_id {
            let boundary_tag = format!("compact_boundary:{}", boundary_id);
            if !self.provenance.iter().any(|tag| tag == &boundary_tag) {
                self.provenance.push(boundary_tag);
            }
        }
        if let Some(trigger) = &self.trigger {
            let trigger_tag = format!("trigger:{}", trigger);
            if !self.provenance.iter().any(|tag| tag == &trigger_tag) {
                self.provenance.push(trigger_tag);
            }
        }
        if let Some(token_pressure) = self.token_pressure {
            let pressure_tag = format!("token_pressure:{}", token_pressure.label());
            if !self.provenance.iter().any(|tag| tag == &pressure_tag) {
                self.provenance.push(pressure_tag);
            }
        }
        for item in &self.retained_items {
            let retained_tag = format!("retained:{}", item);
            if !self.provenance.iter().any(|tag| tag == &retained_tag) {
                self.provenance.push(retained_tag);
            }
        }
    }
}

/// 折叠条目类型
#[derive(Debug, Clone)]
pub enum ContextCollapseEntry {
    /// 提交条目：将一组消息压缩为一个条目
    Commit {
        id: String,
        timestamp: String,
        message_count: usize,
        content_hash: u64,
    },
    /// 快照条目：完整的消息快照
    Snapshot {
        id: String,
        timestamp: String,
        messages: Vec<Message>,
    },
}

/// 折叠服务配置
#[derive(Debug, Clone)]
pub struct ContextCollapseConfig {
    /// 是否启用上下文折叠
    pub enabled: bool,
    /// 保留的最近消息数
    pub window_size: usize,
    /// 折叠阈值（消息数超过此值时触发折叠）
    pub threshold: usize,
    /// 折叠文件存储目录
    pub storage_dir: PathBuf,
}

impl Default for ContextCollapseConfig {
    fn default() -> Self {
        let storage_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("priority-agent")
            .join("context-collapse");

        let window_size = std::env::var("PRIORITY_AGENT_CONTEXT_COLLAPSE_WINDOW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);

        Self {
            enabled: std::env::var("PRIORITY_AGENT_CONTEXT_COLLAPSE")
                .ok()
                .map(|v| v == "1")
                .unwrap_or(false),
            window_size,
            threshold: window_size + 20, // 超过 window + 20 条时触发折叠
            storage_dir,
        }
    }
}

/// 上下文折叠服务（实验性，未接入主运行时）
///
/// 当前未接入主对话循环。运行时使用 `ContextCompressor`（内存压缩）替代。
/// 保留此服务用于未来可能的磁盘持久化折叠需求。
/// 由 `PRIORITY_AGENT_CONTEXT_COLLAPSE=1` 门控。
#[allow(dead_code)]
pub struct ContextCollapseService {
    config: ContextCollapseConfig,
    /// 当前会话的折叠条目
    entries: Arc<RwLock<Vec<ContextCollapseEntry>>>,
    /// 当前会话 ID
    session_id: Option<String>,
}

/// 实验性：未接入主运行时。见 `ContextCollapseService` 文档。
#[allow(dead_code)]
impl ContextCollapseService {
    /// 创建新的折叠服务
    pub fn new() -> Self {
        let config = ContextCollapseConfig::default();
        if config.enabled {
            let _ = std::fs::create_dir_all(&config.storage_dir);
        }
        Self {
            config,
            entries: Arc::new(RwLock::new(Vec::new())),
            session_id: None,
        }
    }

    /// 创建带有自定义配置的折叠服务
    pub fn with_config(config: ContextCollapseConfig) -> Self {
        if config.enabled {
            let _ = std::fs::create_dir_all(&config.storage_dir);
        }
        Self {
            config,
            entries: Arc::new(RwLock::new(Vec::new())),
            session_id: None,
        }
    }

    /// 检查是否应该进行折叠
    pub fn should_collapse(&self, message_count: usize) -> bool {
        self.config.enabled && message_count > self.config.threshold
    }

    /// 执行折叠：将早期消息写入磁盘，保留最近的消息
    ///
    /// 返回被折叠的消息数量
    pub async fn apply_collapses_if_needed(&self, messages: &mut Vec<Message>) -> Result<usize> {
        if !self.should_collapse(messages.len()) {
            return Ok(0);
        }

        let collapse_count = messages.len() - self.config.window_size;
        if collapse_count == 0 {
            return Ok(0);
        }

        info!(
            "Applying context collapse: {} messages will be collapsed, {} kept",
            collapse_count, self.config.window_size
        );

        // 将要折叠的消息分离出来
        let collapsed_messages = messages[..collapse_count].to_vec();
        let remaining_messages: Vec<Message> = messages[collapse_count..].to_vec();

        // 创建折叠条目
        let entry = self.create_commit_entry(&collapsed_messages).await?;
        let entry_id = match &entry {
            ContextCollapseEntry::Commit { id, .. } => id.clone(),
            ContextCollapseEntry::Snapshot { id, .. } => id.clone(),
        };

        // 写入磁盘
        let file_path = self
            .persist_collapsed_messages(&entry_id, &collapsed_messages)
            .await?;
        debug!("Collapsed messages persisted to: {:?}", file_path);

        self.entries.write().await.push(entry);

        // 更新 messages 列表，只保留窗口内的消息
        *messages = remaining_messages;

        Ok(collapse_count)
    }

    /// 恢复折叠的消息（从磁盘读取）
    pub async fn restore(&self) -> Result<Vec<Message>> {
        let entries = self.entries.read().await;
        let mut restored = Vec::new();

        for entry in entries.iter() {
            match entry {
                ContextCollapseEntry::Commit { id, .. } => {
                    let path = self.get_collapse_file_path(id);
                    if path.exists() {
                        let content = tokio::fs::read_to_string(&path).await?;
                        // 解析并恢复消息
                        // 格式：每条消息用 \n---\n 分隔，JSON 编码
                        for line in content.lines() {
                            if !line.trim().is_empty() {
                                if let Ok(msg) = serde_json::from_str::<Message>(line) {
                                    restored.push(msg);
                                }
                            }
                        }
                    } else {
                        warn!("Collapse file not found: {:?}", path);
                    }
                }
                ContextCollapseEntry::Snapshot { messages, .. } => {
                    restored.extend(messages.clone());
                }
            }
        }

        Ok(restored)
    }

    /// 获取恢复后的完整消息列表（窗口消息 + 恢复的折叠消息）
    pub async fn get_full_messages(&self, window_messages: &[Message]) -> Result<Vec<Message>> {
        let mut restored = self.restore().await?;
        restored.extend_from_slice(window_messages);
        Ok(restored)
    }

    /// 检查是否启用了折叠
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 获取当前折叠条目数量
    pub async fn entry_count(&self) -> usize {
        self.entries.read().await.len()
    }

    /// 设置当前会话 ID
    pub fn set_session_id(&mut self, session_id: &str) {
        self.session_id = Some(session_id.to_string());
    }

    /// 创建提交条目
    async fn create_commit_entry(&self, messages: &[Message]) -> Result<ContextCollapseEntry> {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let message_count = messages.len();
        let content_hash = self.compute_messages_hash(messages);

        Ok(ContextCollapseEntry::Commit {
            id,
            timestamp,
            message_count,
            content_hash,
        })
    }

    /// 将折叠的消息持久化到磁盘
    async fn persist_collapsed_messages(
        &self,
        entry_id: &str,
        messages: &[Message],
    ) -> Result<PathBuf> {
        let session_prefix = self
            .session_id
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let file_name = format!("{}_{}.jsonl", session_prefix, entry_id);
        let file_path = self.config.storage_dir.join(&file_name);

        // JSONL 格式：每行一个 JSON 编码的消息
        let mut content = String::new();
        for msg in messages {
            content.push_str(&serde_json::to_string(msg)?);
            content.push('\n');
        }

        tokio::fs::write(&file_path, content).await?;

        Ok(file_path)
    }

    /// 获取折叠文件的路径
    fn get_collapse_file_path(&self, entry_id: &str) -> PathBuf {
        let session_prefix = self
            .session_id
            .clone()
            .unwrap_or_else(|| "default".to_string());
        self.config
            .storage_dir
            .join(format!("{}_{}.jsonl", session_prefix, entry_id))
    }

    /// 计算消息内容的哈希值
    fn compute_messages_hash(&self, messages: &[Message]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for msg in messages {
            msg.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// 清理旧的折叠文件（保留最近 N 个）
    pub async fn prune_old_entries(&self, keep_recent: usize) -> Result<usize> {
        let mut removed = 0;
        let entries: Vec<_> = self
            .entries
            .read()
            .await
            .iter()
            .rev()
            .skip(keep_recent)
            .cloned()
            .collect();

        for entry in entries {
            match entry {
                ContextCollapseEntry::Commit { id, .. } => {
                    let path = self.get_collapse_file_path(&id);
                    if path.exists() && std::fs::remove_file(&path).is_ok() {
                        removed += 1;
                    }
                }
                ContextCollapseEntry::Snapshot { id, .. } => {
                    let path = self.config.storage_dir.join(format!("{}.snap", id));
                    if path.exists() && std::fs::remove_file(&path).is_ok() {
                        removed += 1;
                    }
                }
            }
        }

        debug!("Pruned {} old collapse entries", removed);
        Ok(removed)
    }

    /// 获取折叠统计信息
    pub async fn stats(&self) -> ContextCollapseStats {
        let entries = self.entries.read().await;
        let mut total_messages = 0usize;
        let mut commits = 0usize;
        let mut snapshots = 0usize;

        for entry in entries.iter() {
            match entry {
                ContextCollapseEntry::Commit { message_count, .. } => {
                    total_messages += message_count;
                    commits += 1;
                }
                ContextCollapseEntry::Snapshot { messages, .. } => {
                    total_messages += messages.len();
                    snapshots += 1;
                }
            }
        }

        ContextCollapseStats {
            enabled: self.config.enabled,
            window_size: self.config.window_size,
            threshold: self.config.threshold,
            total_entries: entries.len(),
            total_collapsed_messages: total_messages,
            commits,
            snapshots,
            storage_dir: self.config.storage_dir.clone(),
        }
    }

    /// 重置折叠服务（开始新会话时调用）
    pub async fn reset(&mut self) {
        self.entries.write().await.clear();
        self.session_id = None;
    }
}

impl Default for ContextCollapseService {
    fn default() -> Self {
        Self::new()
    }
}

/// 折叠统计信息
#[derive(Debug, Clone)]
pub struct ContextCollapseStats {
    pub enabled: bool,
    pub window_size: usize,
    pub threshold: usize,
    pub total_entries: usize,
    pub total_collapsed_messages: usize,
    pub commits: usize,
    pub snapshots: usize,
    pub storage_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_collapse_disabled_by_default() {
        // 默认配置应该禁用折叠
        let service = ContextCollapseService::new();
        assert!(!service.is_enabled());
        assert!(!service.should_collapse(100));
    }

    #[tokio::test]
    async fn test_collapse_threshold() {
        let config = ContextCollapseConfig {
            enabled: true,
            window_size: 10,
            threshold: 15,
            storage_dir: std::env::temp_dir().join("test-collapse"),
        };
        let service = ContextCollapseService::with_config(config);

        assert!(!service.should_collapse(10)); // below threshold
        assert!(!service.should_collapse(15)); // at threshold
        assert!(service.should_collapse(16)); // above threshold
    }

    #[tokio::test]
    async fn test_apply_collapse() {
        let config = ContextCollapseConfig {
            enabled: true,
            window_size: 2,
            threshold: 3,
            storage_dir: std::env::temp_dir().join("test-collapse-apply"),
        };
        let mut service = ContextCollapseService::with_config(config);
        service.set_session_id("test-session");

        let mut messages = vec![
            Message::user("msg1"),
            Message::assistant("reply1"),
            Message::user("msg2"),
            Message::assistant("reply2"),
            Message::user("msg3"),
        ];

        let collapsed = service
            .apply_collapses_if_needed(&mut messages)
            .await
            .unwrap();
        assert_eq!(collapsed, 3); // 5 - 2 = 3 messages collapsed
        assert_eq!(messages.len(), 2); // only window kept
    }

    #[tokio::test]
    async fn test_apply_collapse_restores_persisted_messages() {
        let storage_dir = std::env::temp_dir().join(format!(
            "test-collapse-restore-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let config = ContextCollapseConfig {
            enabled: true,
            window_size: 2,
            threshold: 3,
            storage_dir: storage_dir.clone(),
        };
        let mut service = ContextCollapseService::with_config(config);
        service.set_session_id("test-session");

        let mut messages = vec![
            Message::user("msg1"),
            Message::assistant("reply1"),
            Message::user("msg2"),
            Message::assistant("reply2"),
            Message::user("msg3"),
        ];

        let collapsed = service
            .apply_collapses_if_needed(&mut messages)
            .await
            .unwrap();
        let restored = service.restore().await.unwrap();
        let full_messages = service.get_full_messages(&messages).await.unwrap();

        assert_eq!(collapsed, 3);
        assert_eq!(restored.len(), 3);
        assert!(matches!(
            &restored[0],
            Message::User { content } if content == "msg1"
        ));
        assert_eq!(full_messages.len(), 5);
        assert!(matches!(
            full_messages.last(),
            Some(Message::User { content }) if content == "msg3"
        ));

        let _ = tokio::fs::remove_dir_all(storage_dir).await;
    }

    #[test]
    fn test_compaction_runtime_record_normalizes_provenance() {
        let mut record = CompactionRuntimeRecord {
            strategy: ContextCompactionStrategy::ReactiveCompact,
            level: Some("heavy".to_string()),
            trigger: Some("api_context_error".to_string()),
            token_pressure: Some(ContextTokenPressure::Critical),
            messages_before: 10,
            messages_after: 4,
            tokens_before: 1000,
            tokens_after: 300,
            token_delta: -700,
            stage_order: vec![
                "snip_tool_results".to_string(),
                "sanitize_tool_pairs".to_string(),
            ],
            boundary_id: Some("cb-test".to_string()),
            sequence: Some(2),
            preserved_tail_count: Some(3),
            retained_items: vec!["tail_messages:3".to_string()],
            provenance: vec!["trigger:api_context_error".to_string()],
        };

        record.normalize_provenance();
        record.normalize_provenance();

        assert_eq!(
            record
                .provenance
                .iter()
                .filter(|tag| *tag == "strategy:reactive_compact")
                .count(),
            1
        );
        assert_eq!(
            record
                .provenance
                .iter()
                .filter(|tag| *tag == "compact_boundary:cb-test")
                .count(),
            1
        );
        assert!(record
            .provenance
            .contains(&"token_pressure:critical".to_string()));
        assert!(record
            .provenance
            .contains(&"retained:tail_messages:3".to_string()));
    }
}
