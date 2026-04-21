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

/// 上下文折叠服务
pub struct ContextCollapseService {
    config: ContextCollapseConfig,
    /// 当前会话的折叠条目
    entries: Arc<RwLock<Vec<ContextCollapseEntry>>>,
    /// 当前会话 ID
    session_id: Option<String>,
}

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
        self.entries.write().await.push(entry);

        // 写入磁盘
        let file_path = self.persist_collapsed_messages(&collapsed_messages).await?;
        debug!("Collapsed messages persisted to: {:?}", file_path);

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
    async fn persist_collapsed_messages(&self, messages: &[Message]) -> Result<PathBuf> {
        let session_prefix = self
            .session_id
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let entry_id = uuid::Uuid::new_v4().to_string();
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
}
