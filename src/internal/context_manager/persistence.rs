//! 持久化管理

use crate::internal::context_manager::state::{ContextSnapshot, SessionState};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 存储后端 trait
pub trait StorageBackend {
    /// 保存数据
    fn save(&self, key: &str, data: &str) -> Result<(), PersistenceError>;
    /// 加载数据
    fn load(&self, key: &str) -> Result<Option<String>, PersistenceError>;
    /// 删除数据
    fn delete(&self, key: &str) -> Result<(), PersistenceError>;
    /// 列出所有键
    fn list_keys(&self) -> Result<Vec<String>, PersistenceError>;
}

/// 持久化错误
#[derive(Debug, Clone, PartialEq)]
pub enum PersistenceError {
    IoError(String),
    SerializationError(String),
    NotFound(String),
    InvalidData(String),
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistenceError::IoError(msg) => write!(f, "IO错误: {}", msg),
            PersistenceError::SerializationError(msg) => write!(f, "序列化错误: {}", msg),
            PersistenceError::NotFound(key) => write!(f, "未找到: {}", key),
            PersistenceError::InvalidData(msg) => write!(f, "无效数据: {}", msg),
        }
    }
}

impl std::error::Error for PersistenceError {}

/// 文件存储后端
pub struct FileStorage {
    base_path: PathBuf,
}

impl FileStorage {
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        let base_path = base_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_path).ok();
        Self { base_path }
    }

    fn key_to_path(&self, key: &str) -> PathBuf {
        // 简单的键到文件路径映射
        let safe_key = key.replace(['/', '\\'], "_");
        self.base_path.join(format!("{}.json", safe_key))
    }
}

impl StorageBackend for FileStorage {
    fn save(&self, key: &str, data: &str) -> Result<(), PersistenceError> {
        let path = self.key_to_path(key);
        std::fs::write(&path, data).map_err(|e| PersistenceError::IoError(e.to_string()))
    }

    fn load(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        let path = self.key_to_path(key);
        match std::fs::read_to_string(&path) {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(PersistenceError::IoError(e.to_string())),
        }
    }

    fn delete(&self, key: &str) -> Result<(), PersistenceError> {
        let path = self.key_to_path(key);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(PersistenceError::IoError(e.to_string())),
        }
    }

    fn list_keys(&self) -> Result<Vec<String>, PersistenceError> {
        let mut keys = Vec::new();

        for entry in std::fs::read_dir(&self.base_path)
            .map_err(|e| PersistenceError::IoError(e.to_string()))?
        {
            let entry = entry.map_err(|e| PersistenceError::IoError(e.to_string()))?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "json") {
                if let Some(stem) = path.file_stem() {
                    keys.push(stem.to_string_lossy().to_string());
                }
            }
        }

        Ok(keys)
    }
}

/// 持久化管理器
pub struct PersistenceManager {
    storage: Box<dyn StorageBackend>,
    cache: HashMap<String, String>,
}

impl PersistenceManager {
    pub fn new(storage: Box<dyn StorageBackend>) -> Self {
        Self {
            storage,
            cache: HashMap::new(),
        }
    }

    /// 使用文件存储创建
    pub fn with_file_storage(base_path: impl AsRef<Path>) -> Self {
        Self::new(Box::new(FileStorage::new(base_path)))
    }

    /// 保存会话状态
    pub fn save_state(&mut self, state: &SessionState) -> Result<(), PersistenceError> {
        let data = serde_json::to_string_pretty(state)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        self.storage.save("session_state", &data)?;
        self.cache.insert("session_state".to_string(), data);

        Ok(())
    }

    /// 加载会话状态
    pub fn load_state(&self) -> Result<Option<SessionState>, PersistenceError> {
        if let Some(data) = self.storage.load("session_state")? {
            let state = serde_json::from_str(&data)
                .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    /// 保存快照
    pub fn save_snapshot(&mut self, snapshot: &ContextSnapshot) -> Result<(), PersistenceError> {
        let key = format!("snapshot/{}", snapshot.id);
        let data = serde_json::to_string_pretty(snapshot)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        self.storage.save(&key, &data)?;
        Ok(())
    }

    /// 加载快照
    pub fn load_snapshot(&self, id: &str) -> Result<Option<ContextSnapshot>, PersistenceError> {
        let key = format!("snapshot/{}", id);

        if let Some(data) = self.storage.load(&key)? {
            let snapshot = serde_json::from_str(&data)
                .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }

    /// 列出所有快照
    pub fn list_snapshots(&self) -> Result<Vec<String>, PersistenceError> {
        let keys = self.storage.list_keys()?;
        Ok(keys
            .into_iter()
            .filter(|k| k.starts_with("snapshot/"))
            .map(|k| k.trim_start_matches("snapshot/").to_string())
            .collect())
    }

    /// 删除快照
    pub fn delete_snapshot(&mut self, id: &str) -> Result<(), PersistenceError> {
        let key = format!("snapshot/{}", id);
        self.storage.delete(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 内存存储后端（用于测试）
    struct MemoryStorage {
        data: std::cell::RefCell<HashMap<String, String>>,
    }

    impl MemoryStorage {
        fn new() -> Self {
            Self {
                data: std::cell::RefCell::new(HashMap::new()),
            }
        }
    }

    impl StorageBackend for MemoryStorage {
        fn save(&self, key: &str, data: &str) -> Result<(), PersistenceError> {
            self.data
                .borrow_mut()
                .insert(key.to_string(), data.to_string());
            Ok(())
        }

        fn load(&self, key: &str) -> Result<Option<String>, PersistenceError> {
            Ok(self.data.borrow().get(key).cloned())
        }

        fn delete(&self, key: &str) -> Result<(), PersistenceError> {
            self.data.borrow_mut().remove(key);
            Ok(())
        }

        fn list_keys(&self) -> Result<Vec<String>, PersistenceError> {
            Ok(self.data.borrow().keys().cloned().collect())
        }
    }

    #[test]
    fn test_persistence_manager() {
        let storage = Box::new(MemoryStorage::new());
        let mut manager = PersistenceManager::new(storage);

        let state = SessionState::new();
        manager.save_state(&state).unwrap();

        let loaded = manager.load_state().unwrap();
        assert!(loaded.is_some());
    }
}
