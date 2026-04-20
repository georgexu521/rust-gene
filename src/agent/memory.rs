//! Agent 记忆系统
//!
//! 为子代理提供独立的记忆存储，支持：
//! - 键值对存储
//! - 记忆快照/恢复
//! - 跨子代理共享（可选）

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 最大快照数量
const MAX_SNAPSHOTS: usize = 100;

/// 记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub tags: Vec<String>,
}

/// 记忆快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub agent_id: String,
    pub entries: Vec<MemoryEntry>,
    pub timestamp: u64,
}

/// Agent 记忆
#[derive(Debug, Clone)]
pub struct AgentMemory {
    /// 所属代理 ID
    agent_id: String,
    /// 记忆条目
    entries: Arc<RwLock<HashMap<String, MemoryEntry>>>,
    /// 快照历史
    snapshots: Arc<RwLock<Vec<MemorySnapshot>>>,
}

impl AgentMemory {
    /// 创建新的 Agent 记忆
    pub fn new(agent_id: String) -> Self {
        Self {
            agent_id,
            entries: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 保存记忆
    pub async fn save(&self, key: &str, value: &str) {
        self.save_with_tags(key, value, vec![]).await;
    }

    /// 带标签保存记忆
    pub async fn save_with_tags(&self, key: &str, value: &str, tags: Vec<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut entries = self.entries.write().await;
        let entry = entries.entry(key.to_string()).or_insert_with(|| {
            MemoryEntry {
                key: key.to_string(),
                value: value.to_string(),
                created_at: now,
                updated_at: now,
                tags: tags.clone(),
            }
        });

        // 更新现有条目
        if entry.value != value || entry.tags != tags {
            entry.value = value.to_string();
            entry.updated_at = now;
            entry.tags = tags;
        }
    }

    /// 加载记忆
    pub async fn load(&self, key: &str) -> Option<String> {
        let entries = self.entries.read().await;
        entries.get(key).map(|e| e.value.clone())
    }

    /// 检查记忆是否存在
    pub async fn exists(&self, key: &str) -> bool {
        let entries = self.entries.read().await;
        entries.contains_key(key)
    }

    /// 删除记忆
    pub async fn delete(&self, key: &str) -> bool {
        let mut entries = self.entries.write().await;
        entries.remove(key).is_some()
    }

    /// 获取所有键
    pub async fn keys(&self) -> Vec<String> {
        let entries = self.entries.read().await;
        entries.keys().cloned().collect()
    }

    /// 按标签搜索记忆
    pub async fn search_by_tag(&self, tag: &str) -> Vec<MemoryEntry> {
        let entries = self.entries.read().await;
        entries
            .values()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .cloned()
            .collect()
    }

    /// 模糊搜索记忆
    pub async fn search(&self, query: &str) -> Vec<MemoryEntry> {
        let query_lower = query.to_lowercase();
        let entries = self.entries.read().await;
        entries
            .values()
            .filter(|e| {
                e.key.to_lowercase().contains(&query_lower)
                    || e.value.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    /// 获取记忆数量
    pub async fn len(&self) -> usize {
        let entries = self.entries.read().await;
        entries.len()
    }

    /// 检查记忆是否为空
    pub async fn is_empty(&self) -> bool {
        let entries = self.entries.read().await;
        entries.is_empty()
    }

    /// 清空所有记忆
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }

    /// 创建快照
    pub async fn snapshot(&self) -> MemorySnapshot {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entries = self.entries.read().await;
        let snapshot = MemorySnapshot {
            agent_id: self.agent_id.clone(),
            entries: entries.values().cloned().collect(),
            timestamp: now,
        };

        // 保存到快照历史，限制数量
        let mut snapshots = self.snapshots.write().await;
        if snapshots.len() >= MAX_SNAPSHOTS {
            snapshots.remove(0); // 移除最老的快照
        }
        snapshots.push(snapshot.clone());

        snapshot
    }

    /// 从快照恢复
    pub async fn restore(&self, snapshot: &MemorySnapshot) {
        let mut entries = self.entries.write().await;
        entries.clear();
        for entry in &snapshot.entries {
            entries.insert(entry.key.clone(), entry.clone());
        }
    }

    /// 获取快照历史
    pub async fn get_snapshots(&self) -> Vec<MemorySnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.clone()
    }

    /// 获取指定快照
    pub async fn get_snapshot(&self, timestamp: u64) -> Option<MemorySnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.iter().find(|s| s.timestamp == timestamp).cloned()
    }

    /// 导出记忆为 JSON
    pub async fn to_json(&self) -> String {
        let entries = self.entries.read().await;
        let entries_vec: Vec<&MemoryEntry> = entries.values().collect();
        serde_json::to_string_pretty(&entries_vec).unwrap_or_else(|_| "[]".to_string())
    }

    /// 从 JSON 导入记忆
    pub async fn import_json(&self, json: &str) -> Result<(), String> {
        let entries_vec: Vec<MemoryEntry> = serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let mut entries = self.entries.write().await;
        entries.clear();
        for entry in entries_vec {
            entries.insert(entry.key.clone(), entry);
        }
        Ok(())
    }

    /// 合并另一个 AgentMemory 的记忆
    pub async fn merge(&self, other: &AgentMemory) {
        let other_entries = other.entries.read().await;
        let mut entries = self.entries.write().await;

        for (key, entry) in other_entries.iter() {
            // 总是使用 other 的值覆盖
            entries.insert(key.clone(), entry.clone());
        }
    }

    /// 保存记忆到文件
    pub async fn save_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        let json = self.to_json().await;
        tokio::fs::write(path, json)
            .await
            .map_err(|e| format!("Failed to write memory to {}: {}", path.display(), e))
    }

    /// 从文件加载记忆
    pub async fn load_from_file(&self, path: &std::path::Path) -> Result<(), String> {
        let json = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| format!("Failed to read memory from {}: {}", path.display(), e))?;
        self.import_json(&json).await
    }

    /// 保存快照到文件
    pub async fn save_snapshots_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        let snapshots = self.snapshots.read().await;
        let json = serde_json::to_string_pretty(&*snapshots)
            .map_err(|e| format!("Failed to serialize snapshots: {}", e))?;
        tokio::fs::write(path, json)
            .await
            .map_err(|e| format!("Failed to write snapshots to {}: {}", path.display(), e))
    }

    /// 从文件加载快照
    pub async fn load_snapshots_from_file(&self, path: &std::path::Path) -> Result<(), String> {
        let json = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| format!("Failed to read snapshots from {}: {}", path.display(), e))?;
        let loaded: Vec<MemorySnapshot> = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse snapshots: {}", e))?;
        let mut snapshots = self.snapshots.write().await;
        *snapshots = loaded;
        Ok(())
    }
}

/// 全局记忆管理器
pub struct MemoryManager {
    /// 所有 Agent 的记忆
    memories: Arc<RwLock<HashMap<String, AgentMemory>>>,
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            memories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取或创建 Agent 记忆
    pub async fn get_or_create(&self, agent_id: &str) -> AgentMemory {
        let mut memories = self.memories.write().await;
        memories
            .entry(agent_id.to_string())
            .or_insert_with(|| AgentMemory::new(agent_id.to_string()))
            .clone()
    }

    /// 删除 Agent 记忆
    pub async fn remove(&self, agent_id: &str) -> bool {
        let mut memories = self.memories.write().await;
        memories.remove(agent_id).is_some()
    }

    /// 列出所有 Agent ID
    pub async fn list_agents(&self) -> Vec<String> {
        let memories = self.memories.read().await;
        memories.keys().cloned().collect()
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局记忆管理器实例
static MEMORY_MANAGER: once_cell::sync::Lazy<MemoryManager> =
    once_cell::sync::Lazy::new(MemoryManager::new);

/// 获取全局记忆管理器
pub fn global_memory_manager() -> &'static MemoryManager {
    &MEMORY_MANAGER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_memory_basic() {
        let memory = AgentMemory::new("test-agent".to_string());

        // 保存和加载
        memory.save("key1", "value1").await;
        assert_eq!(memory.load("key1").await, Some("value1".to_string()));

        // 更新
        memory.save("key1", "value2").await;
        assert_eq!(memory.load("key1").await, Some("value2".to_string()));

        // 检查存在
        assert!(memory.exists("key1").await);
        assert!(!memory.exists("key2").await);

        // 删除
        assert!(memory.delete("key1").await);
        assert!(!memory.exists("key1").await);
    }

    #[tokio::test]
    async fn test_agent_memory_snapshot() {
        let memory = AgentMemory::new("test-agent".to_string());

        memory.save("key1", "value1").await;
        memory.save("key2", "value2").await;

        let snapshot = memory.snapshot().await;
        assert_eq!(snapshot.entries.len(), 2);

        // 修改记忆
        memory.save("key1", "modified").await;
        memory.delete("key2").await;

        // 从快照恢复
        memory.restore(&snapshot).await;
        assert_eq!(memory.load("key1").await, Some("value1".to_string()));
        assert_eq!(memory.load("key2").await, Some("value2".to_string()));
    }

    #[tokio::test]
    async fn test_agent_memory_search() {
        let memory = AgentMemory::new("test-agent".to_string());

        memory
            .save_with_tags("config", "debug=true", vec!["settings".to_string()])
            .await;
        memory
            .save_with_tags("state", "running", vec!["status".to_string()])
            .await;
        memory
            .save_with_tags("debug_info", "no errors", vec!["status".to_string()])
            .await;

        // 按标签搜索
        let status_entries = memory.search_by_tag("status").await;
        assert_eq!(status_entries.len(), 2);

        // 模糊搜索
        let debug_entries = memory.search("debug").await;
        assert_eq!(debug_entries.len(), 2);
    }

    #[tokio::test]
    async fn test_agent_memory_merge() {
        let memory1 = AgentMemory::new("agent1".to_string());
        let memory2 = AgentMemory::new("agent2".to_string());

        memory1.save("key1", "value1").await;
        memory1.save("key2", "value2").await;

        // 等待一小段时间确保时间戳不同
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        memory2.save("key2", "updated").await;
        memory2.save("key3", "value3").await;

        memory1.merge(&memory2).await;

        assert_eq!(memory1.load("key1").await, Some("value1".to_string()));
        assert_eq!(memory1.load("key2").await, Some("updated".to_string()));
        assert_eq!(memory1.load("key3").await, Some("value3".to_string()));
    }

    #[tokio::test]
    async fn test_memory_manager() {
        let manager = MemoryManager::new();

        let memory1 = manager.get_or_create("agent1").await;
        let memory2 = manager.get_or_create("agent2").await;

        memory1.save("key", "value1").await;
        memory2.save("key", "value2").await;

        // 不同 Agent 的记忆是独立的
        assert_eq!(memory1.load("key").await, Some("value1".to_string()));
        assert_eq!(memory2.load("key").await, Some("value2".to_string()));

        // 列出 Agent
        let agents = manager.list_agents().await;
        assert_eq!(agents.len(), 2);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let memory = AgentMemory::new("test-agent".to_string());
        let memory_clone = memory.clone();

        // 并发写入
        let handle1 = tokio::spawn(async move {
            for i in 0..100 {
                memory_clone.save(&format!("key{}", i), &format!("value{}", i)).await;
            }
        });

        let memory_clone2 = memory.clone();
        let handle2 = tokio::spawn(async move {
            for i in 100..200 {
                memory_clone2.save(&format!("key{}", i), &format!("value{}", i)).await;
            }
        });

        // 等待两个任务完成
        let _ = tokio::join!(handle1, handle2);

        // 验证所有键都存在
        assert_eq!(memory.len().await, 200);
    }

    #[tokio::test]
    async fn test_snapshot_limit() {
        let memory = AgentMemory::new("test-agent".to_string());

        // 创建超过限制的快照
        for i in 0..150 {
            memory.save(&format!("key{}", i), &format!("value{}", i)).await;
            memory.snapshot().await;
        }

        // 验证快照数量被限制
        let snapshots = memory.get_snapshots().await;
        assert_eq!(snapshots.len(), 100); // MAX_SNAPSHOTS
    }

    #[tokio::test]
    async fn test_persistence_roundtrip() {
        let tmp = std::env::temp_dir().join("memory-persistence-test.json");
        let memory = AgentMemory::new("test-agent".to_string());

        // 保存一些数据
        memory.save("key1", "value1").await;
        memory.save("key2", "value2").await;
        memory.save_with_tags("key3", "value3", vec!["tag1".to_string()]).await;

        // 保存到文件
        memory.save_to_file(&tmp).await.unwrap();

        // 创建新的 memory 实例并加载
        let memory2 = AgentMemory::new("test-agent-2".to_string());
        memory2.load_from_file(&tmp).await.unwrap();

        // 验证数据被正确加载
        assert_eq!(memory2.load("key1").await, Some("value1".to_string()));
        assert_eq!(memory2.load("key2").await, Some("value2".to_string()));
        assert_eq!(memory2.load("key3").await, Some("value3".to_string()));

        // 清理
        let _ = tokio::fs::remove_file(&tmp).await;
    }
}
