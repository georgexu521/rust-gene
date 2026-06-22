//! 团队协作基础
//!
//! 多 agent 之间的邮箱系统 TeammateMailbox。
//! 支持点对点消息、广播、未读轮询、消息持久化。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// ------------------------------------------------------------------------
// 数据模型
// ------------------------------------------------------------------------

/// 邮箱消息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailboxMessage {
    /// 消息 ID
    pub id: String,
    /// 发件人 agent ID
    pub from: String,
    /// 收件人 agent ID（"*" 表示广播）
    pub to: String,
    /// 消息内容
    pub content: String,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 优先级：high / normal / low
    pub priority: MessagePriority,
    /// 是否已读
    pub read: bool,
    /// 消息类型：request / response / notify / broadcast
    pub kind: MessageKind,
    /// 回复的原消息 ID（可选）
    pub reply_to: Option<String>,
}

/// 消息优先级
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum MessagePriority {
    High,
    #[default]
    Normal,
    Low,
}

/// 消息类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum MessageKind {
    /// 请求对方执行某个任务
    Request,
    /// 响应请求
    Response,
    /// 通知
    #[default]
    Notify,
    /// 广播
    Broadcast,
}

/// 未读消息统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreadSummary {
    pub total: usize,
    pub by_sender: HashMap<String, usize>,
    pub by_priority: HashMap<String, usize>,
}

// ------------------------------------------------------------------------
// TeammateMailbox
// ------------------------------------------------------------------------

/// 多 agent 邮箱系统
pub struct TeammateMailbox {
    inner: Arc<Mutex<MailboxInner>>,
    persistence_path: PathBuf,
    /// 本机 agent ID
    pub self_id: String,
}

#[derive(Debug, Default)]
struct MailboxInner {
    messages: Vec<MailboxMessage>,
    next_id: u64,
}

impl TeammateMailbox {
    /// 创建邮箱，self_id 为本机 agent 标识
    pub fn new(self_id: impl Into<String>) -> Self {
        let self_id = self_id.into();
        let persistence_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("mailbox.jsonl");
        let inner = Self::load(&persistence_path);
        Self {
            inner: Arc::new(Mutex::new(inner)),
            persistence_path,
            self_id,
        }
    }

    /// 指定持久化路径（用于测试）
    pub fn with_path(self_id: impl Into<String>, path: PathBuf) -> Self {
        let self_id = self_id.into();
        let inner = Self::load(&path);
        Self {
            inner: Arc::new(Mutex::new(inner)),
            persistence_path: path,
            self_id,
        }
    }

    /// 发送消息给指定 agent
    pub fn send(
        &self,
        to: impl Into<String>,
        content: impl Into<String>,
        priority: MessagePriority,
        kind: MessageKind,
        reply_to: Option<String>,
    ) -> MailboxMessage {
        let msg = {
            let mut inner = self.inner.lock().expect("team inner lock poisoned");
            inner.next_id += 1;
            let id = format!("msg_{}_{}", self.self_id, inner.next_id);
            let msg = MailboxMessage {
                id,
                from: self.self_id.clone(),
                to: to.into(),
                content: content.into(),
                timestamp: chrono::Utc::now(),
                priority,
                read: false,
                kind,
                reply_to,
            };
            inner.messages.push(msg.clone());
            msg
        };
        let _ = self.append_to_disk(&msg);
        msg
    }

    /// 广播消息给所有 teammates
    pub fn broadcast(
        &self,
        content: impl Into<String>,
        priority: MessagePriority,
    ) -> MailboxMessage {
        self.send("*", content, priority, MessageKind::Broadcast, None)
    }

    /// 接收发给本机的消息（未读）
    pub fn receive(&self, limit: usize) -> Vec<MailboxMessage> {
        let inner = self.inner.lock().expect("team inner lock poisoned");
        inner
            .messages
            .iter()
            .filter(|m| (m.to == self.self_id || m.to == "*") && !m.read)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 接收指定发件人的消息
    pub fn receive_from(&self, from: &str, limit: usize) -> Vec<MailboxMessage> {
        let inner = self.inner.lock().expect("team inner lock poisoned");
        inner
            .messages
            .iter()
            .filter(|m| (m.to == self.self_id || m.to == "*") && m.from == from && !m.read)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 获取单条消息
    pub fn get_message(&self, id: &str) -> Option<MailboxMessage> {
        let inner = self.inner.lock().expect("team inner lock poisoned");
        inner.messages.iter().find(|m| m.id == id).cloned()
    }

    /// 标记消息已读
    pub fn mark_read(&self, id: &str) -> bool {
        let found = {
            let mut inner = self.inner.lock().expect("team inner lock poisoned");
            if let Some(m) = inner.messages.iter_mut().find(|m| m.id == id) {
                m.read = true;
                true
            } else {
                false
            }
        };
        if found {
            let _ = self.flush_to_disk();
        }
        found
    }

    /// 标记所有消息已读
    pub fn mark_all_read(&self) -> usize {
        let count = {
            let mut inner = self.inner.lock().expect("team inner lock poisoned");
            let mut count = 0;
            for m in &mut inner.messages {
                if (m.to == self.self_id || m.to == "*") && !m.read {
                    m.read = true;
                    count += 1;
                }
            }
            count
        };
        if count > 0 {
            let _ = self.flush_to_disk();
        }
        count
    }

    /// 未读消息统计
    pub fn unread_summary(&self) -> UnreadSummary {
        let inner = self.inner.lock().expect("team inner lock poisoned");
        let unread: Vec<_> = inner
            .messages
            .iter()
            .filter(|m| (m.to == self.self_id || m.to == "*") && !m.read)
            .cloned()
            .collect();

        let mut by_sender = HashMap::new();
        let mut by_priority = HashMap::new();
        for m in &unread {
            *by_sender.entry(m.from.clone()).or_insert(0) += 1;
            let p = format!("{:?}", m.priority).to_lowercase();
            *by_priority.entry(p).or_insert(0) += 1;
        }

        UnreadSummary {
            total: unread.len(),
            by_sender,
            by_priority,
        }
    }

    /// 列出最近消息（包含已读）
    pub fn list_messages(&self, limit: usize) -> Vec<MailboxMessage> {
        let inner = self.inner.lock().expect("team inner lock poisoned");
        inner
            .messages
            .iter()
            .filter(|m| m.to == self.self_id || m.to == "*" || m.from == self.self_id)
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// 删除消息
    pub fn delete_message(&self, id: &str) -> bool {
        let removed = {
            let mut inner = self.inner.lock().expect("team inner lock poisoned");
            let len_before = inner.messages.len();
            inner.messages.retain(|m| m.id != id);
            inner.messages.len() < len_before
        };
        if removed {
            let _ = self.flush_to_disk();
        }
        removed
    }

    // --- Persistence ---

    fn load(path: &PathBuf) -> MailboxInner {
        if !path.exists() {
            return MailboxInner::default();
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let mut messages = Vec::new();
        let mut next_id = 0u64;
        for line in content.lines() {
            if let Ok(msg) = serde_json::from_str::<MailboxMessage>(line) {
                // 尝试从 id 推断最大 id 号
                if let Some(num) = msg.id.rsplit('_').next() {
                    if let Ok(n) = num.parse::<u64>() {
                        next_id = next_id.max(n);
                    }
                }
                messages.push(msg);
            }
        }
        MailboxInner { messages, next_id }
    }

    fn append_to_disk(&self, msg: &MailboxMessage) -> anyhow::Result<()> {
        if let Some(parent) = self.persistence_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(msg)?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.persistence_path)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    fn flush_to_disk(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.persistence_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let inner = self.inner.lock().expect("team inner lock poisoned");
        let mut file = std::fs::File::create(&self.persistence_path)?;
        for msg in &inner.messages {
            let line = serde_json::to_string(msg)?;
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }
}

impl Default for TeammateMailbox {
    fn default() -> Self {
        Self::new("default_agent")
    }
}

// ------------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mailbox_send_and_receive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mailbox.jsonl");
        let mb = TeammateMailbox::with_path("alice", path);

        let msg = mb.send(
            "alice",
            "Hello self!",
            MessagePriority::Normal,
            MessageKind::Notify,
            None,
        );
        assert_eq!(msg.from, "alice");
        assert_eq!(msg.to, "alice");

        let msgs = mb.receive(10);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "Hello self!");
    }

    #[test]
    fn test_mailbox_broadcast() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mailbox.jsonl");
        let mb = TeammateMailbox::with_path("alice", path);

        let _ = mb.broadcast("All hands meeting", MessagePriority::High);
        assert_eq!(mb.list_messages(10)[0].kind, MessageKind::Broadcast);
    }

    #[test]
    fn test_mailbox_mark_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mailbox.jsonl");
        let mb = TeammateMailbox::with_path("me", path);

        let msg = mb.send(
            "me",
            "todo 1",
            MessagePriority::Normal,
            MessageKind::Request,
            None,
        );
        assert_eq!(mb.receive(10).len(), 1);

        assert!(mb.mark_read(&msg.id));
        assert!(mb.receive(10).is_empty());
    }

    #[test]
    fn test_mailbox_unread_summary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mailbox.jsonl");
        let mb = TeammateMailbox::with_path("me", path);

        mb.send(
            "me",
            "msg1",
            MessagePriority::High,
            MessageKind::Notify,
            None,
        );
        mb.send(
            "me",
            "msg2",
            MessagePriority::Normal,
            MessageKind::Notify,
            None,
        );
        mb.send(
            "me",
            "msg3",
            MessagePriority::High,
            MessageKind::Notify,
            None,
        );

        let summary = mb.unread_summary();
        assert_eq!(summary.total, 3);
    }

    #[test]
    fn test_mailbox_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mailbox.jsonl");
        let mb = TeammateMailbox::with_path("me", path);

        let msg = mb.send(
            "me",
            "delete me",
            MessagePriority::Normal,
            MessageKind::Notify,
            None,
        );
        assert!(mb.delete_message(&msg.id));
        assert!(mb.get_message(&msg.id).is_none());
    }

    #[test]
    fn test_mailbox_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mailbox.jsonl");
        {
            let mb = TeammateMailbox::with_path("agent", path.clone());
            mb.send(
                "other",
                "persistent",
                MessagePriority::Normal,
                MessageKind::Notify,
                None,
            );
        }
        {
            let mb = TeammateMailbox::with_path("other", path);
            let msgs = mb.receive(10);
            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].content, "persistent");
        }
    }
}
