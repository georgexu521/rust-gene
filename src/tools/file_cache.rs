//! 文件状态缓存
//!
//! 缓存文件元数据（mtime, size），监控文件变更

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};
use tracing::debug;

/// 文件元数据
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    pub created: SystemTime,
    pub cached_at: Instant,
    pub access_count: u64,
}

impl FileMetadata {
    /// 从文件系统读取元数据
    pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        let path = path.as_ref();
        let metadata = std::fs::metadata(path).ok()?;

        Some(Self {
            path: path.to_path_buf(),
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            created: metadata.created().unwrap_or(SystemTime::UNIX_EPOCH),
            cached_at: Instant::now(),
            access_count: 1,
        })
    }

    /// 检查文件是否已变更
    pub fn is_stale(&self) -> bool {
        if let Some(current) = Self::from_path(&self.path) {
            self.modified != current.modified || self.size != current.size
        } else {
            // 文件已不存在
            true
        }
    }

    /// 增加访问计数
    pub fn touch(&mut self) {
        self.access_count += 1;
    }
}

/// 文件缓存配置
#[derive(Debug, Clone)]
pub struct FileCacheConfig {
    /// 最大缓存条目数
    pub max_entries: usize,
    /// 默认 TTL (秒)
    pub default_ttl_secs: u64,
    /// 启用缓存
    pub enabled: bool,
}

impl Default for FileCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            default_ttl_secs: 300, // 5分钟
            enabled: true,
        }
    }
}

/// 文件内容缓存条目
#[derive(Debug, Clone)]
pub struct FileContentEntry {
    pub content: String,
    pub metadata: FileMetadata,
}

/// 文件状态缓存
pub struct FileStateCache {
    /// 文件元数据缓存
    metadata_cache: Arc<RwLock<HashMap<PathBuf, FileMetadata>>>,
    /// 文件内容缓存
    content_cache: Arc<RwLock<HashMap<PathBuf, FileContentEntry>>>,
    /// 每个会话已读取过的文件记录（session id -> path -> 读取时的元数据）
    session_reads: Arc<RwLock<HashMap<String, HashMap<PathBuf, FileMetadata>>>>,
    config: FileCacheConfig,
    stats: Arc<RwLock<FileCacheStats>>,
}

/// 全局文件状态缓存
pub static GLOBAL_FILE_CACHE: once_cell::sync::Lazy<Arc<FileStateCache>> =
    once_cell::sync::Lazy::new(|| Arc::new(FileStateCache::new()));

/// 缓存统计
#[derive(Debug, Default, Clone)]
pub struct FileCacheStats {
    pub metadata_hits: u64,
    pub metadata_misses: u64,
    pub content_hits: u64,
    pub content_misses: u64,
    pub stale_hits: u64,
}

impl FileStateCache {
    /// 创建新缓存
    pub fn new() -> Self {
        Self::with_config(FileCacheConfig::default())
    }

    /// 使用配置创建
    pub fn with_config(config: FileCacheConfig) -> Self {
        Self {
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
            content_cache: Arc::new(RwLock::new(HashMap::new())),
            session_reads: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(FileCacheStats::default())),
        }
    }

    /// 获取文件元数据（带缓存）
    pub fn metadata(&self, path: impl AsRef<Path>) -> Option<FileMetadata> {
        if !self.config.enabled {
            return FileMetadata::from_path(path);
        }

        let path = path.as_ref().to_path_buf();
        let cache_key = path.canonicalize().unwrap_or(path.clone());

        // 检查缓存
        {
            let cache = self.metadata_cache.read().expect("Lock poisoned");
            if let Some(metadata) = cache.get(&cache_key) {
                // 检查是否过期
                let ttl = Duration::from_secs(self.config.default_ttl_secs);
                let is_valid = metadata.cached_at.elapsed() < ttl;
                let result = metadata.clone();
                drop(cache);
                if is_valid {
                    // 缓存有效
                    self.record_metadata_hit();
                    return Some(result);
                }
                // 缓存过期，需要刷新
            }
        }

        // 缓存未命中，从文件系统读取
        self.record_metadata_miss();
        let metadata = FileMetadata::from_path(&path)?;

        // 存入缓存
        {
            let mut cache = self.metadata_cache.write().expect("Lock poisoned");
            self.ensure_capacity(&mut cache);
            cache.insert(cache_key, metadata.clone());
        }

        Some(metadata)
    }

    /// 获取文件内容（带缓存）
    pub fn content(&self, path: impl AsRef<Path>) -> Option<String> {
        if !self.config.enabled {
            return std::fs::read_to_string(path).ok();
        }

        let path = path.as_ref().to_path_buf();
        let cache_key = path.canonicalize().unwrap_or(path.clone());

        // 检查内容缓存
        {
            let cache = self.content_cache.read().expect("Lock poisoned");
            if let Some(entry) = cache.get(&cache_key) {
                let is_stale = entry.metadata.is_stale();
                let content = entry.content.clone();
                drop(cache);

                if is_stale {
                    self.record_stale_hit();
                    // 文件已变更，使缓存失效
                    self.invalidate_content(&cache_key);
                } else {
                    // 缓存有效
                    self.record_content_hit();
                    return Some(content);
                }
            }
        }

        // 缓存未命中，读取文件
        self.record_content_miss();
        let content = std::fs::read_to_string(&path).ok()?;

        // 获取元数据
        let metadata = FileMetadata::from_path(&path)?;

        // 存入缓存
        {
            let mut cache = self.content_cache.write().expect("Lock poisoned");
            self.ensure_content_capacity(&mut cache);
            cache.insert(
                cache_key,
                FileContentEntry {
                    content: content.clone(),
                    metadata,
                },
            );
        }

        Some(content)
    }

    /// 使文件元数据缓存失效
    pub fn invalidate_metadata(&self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        let cache_key = path.canonicalize().unwrap_or(path);

        let mut cache = self.metadata_cache.write().expect("Lock poisoned");
        cache.remove(&cache_key);
    }

    /// 使文件内容缓存失效
    pub fn invalidate_content(&self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        let cache_key = path.canonicalize().unwrap_or(path);

        let mut cache = self.content_cache.write().expect("Lock poisoned");
        cache.remove(&cache_key);
    }

    /// 使目录下所有缓存失效
    pub fn invalidate_directory(&self, dir: impl AsRef<Path>) {
        let dir = dir.as_ref();

        {
            let mut cache = self.metadata_cache.write().expect("Lock poisoned");
            cache.retain(|path, _| !path.starts_with(dir));
        }

        {
            let mut cache = self.content_cache.write().expect("Lock poisoned");
            cache.retain(|path, _| !path.starts_with(dir));
        }
    }

    /// 清空所有缓存
    pub fn clear(&self) {
        {
            let mut cache = self.metadata_cache.write().expect("Lock poisoned");
            cache.clear();
        }
        {
            let mut cache = self.content_cache.write().expect("Lock poisoned");
            cache.clear();
        }
        {
            let mut reads = self.session_reads.write().expect("Lock poisoned");
            reads.clear();
        }
    }

    /// 获取缓存统计
    pub fn stats(&self) -> FileCacheStats {
        self.stats.read().expect("Lock poisoned").clone()
    }

    /// 生成统计报告
    pub fn generate_report(&self) -> String {
        let stats = self.stats();
        let metadata_total = stats.metadata_hits + stats.metadata_misses;
        let content_total = stats.content_hits + stats.content_misses;

        let metadata_hit_rate = if metadata_total > 0 {
            (stats.metadata_hits as f64 / metadata_total as f64) * 100.0
        } else {
            0.0
        };

        let content_hit_rate = if content_total > 0 {
            (stats.content_hits as f64 / content_total as f64) * 100.0
        } else {
            0.0
        };

        let metadata_count = self.metadata_cache.read().expect("Lock poisoned").len();
        let content_count = self.content_cache.read().expect("Lock poisoned").len();

        format!(
            "File Cache Statistics:\n\
             Metadata cache:\n\
               Entries: {}\n\
               Hits: {}\n\
               Misses: {}\n\
               Hit rate: {:.1}%\n\
             Content cache:\n\
               Entries: {}\n\
               Hits: {}\n\
               Misses: {}\n\
               Hit rate: {:.1}%\n\
               Stale hits: {}",
            metadata_count,
            stats.metadata_hits,
            stats.metadata_misses,
            metadata_hit_rate,
            content_count,
            stats.content_hits,
            stats.content_misses,
            content_hit_rate,
            stats.stale_hits
        )
    }

    /// 获取所有已变更的缓存文件
    pub fn get_all_changed_files(&self) -> Vec<PathBuf> {
        let metadata_cache = self.metadata_cache.read().expect("Lock poisoned");
        metadata_cache
            .values()
            .filter(|meta| meta.is_stale())
            .map(|meta| meta.path.clone())
            .collect()
    }

    /// 获取所有被跟踪的文件路径
    pub fn tracked_files(&self) -> Vec<PathBuf> {
        let metadata_cache = self.metadata_cache.read().expect("Lock poisoned");
        metadata_cache.keys().cloned().collect()
    }

    /// 记录文件已在本会话中被读取
    pub fn mark_read(&self, path: impl AsRef<Path>) {
        self.mark_read_for_session("__global__", path);
    }

    /// 记录文件已在指定会话中被读取
    pub fn mark_read_for_session(&self, session_id: &str, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        let cache_key = path.canonicalize().unwrap_or(path);
        if let Some(meta) = self.metadata(&cache_key) {
            let mut reads = self.session_reads.write().expect("Lock poisoned");
            reads
                .entry(session_id.to_string())
                .or_default()
                .insert(cache_key, meta);
        }
    }

    /// 检查文件自上次会话读取以来是否未变更
    pub fn is_unchanged_since_last_read(&self, path: impl AsRef<Path>) -> bool {
        self.is_unchanged_since_last_read_for_session("__global__", path)
    }

    /// 检查文件自指定会话上次读取以来是否未变更
    pub fn is_unchanged_since_last_read_for_session(
        &self,
        session_id: &str,
        path: impl AsRef<Path>,
    ) -> bool {
        let path = path.as_ref().to_path_buf();
        let cache_key = path.canonicalize().unwrap_or(path);
        let reads = self.session_reads.read().expect("Lock poisoned");
        if let Some(meta) = reads
            .get(session_id)
            .and_then(|session_reads| session_reads.get(&cache_key))
        {
            !meta.is_stale()
        } else {
            false
        }
    }

    /// 清空会话读取记录
    pub fn clear_session_reads(&self) {
        let mut reads = self.session_reads.write().expect("Lock poisoned");
        reads.clear();
    }

    fn ensure_capacity(&self, cache: &mut HashMap<PathBuf, FileMetadata>) {
        if cache.len() >= self.config.max_entries {
            // 淘汰最久未访问的条目
            let to_remove: Option<PathBuf> = cache
                .iter()
                .min_by_key(|(_, m)| m.cached_at)
                .map(|(k, _)| k.clone());

            if let Some(key) = to_remove {
                cache.remove(&key);
                debug!("Evicted metadata cache entry: {:?}", key);
            }
        }
    }

    fn ensure_content_capacity(&self, cache: &mut HashMap<PathBuf, FileContentEntry>) {
        if cache.len() >= self.config.max_entries / 4 {
            // 内容缓存使用更严格的限制
            let to_remove: Option<PathBuf> = cache
                .iter()
                .min_by_key(|(_, e)| e.metadata.access_count)
                .map(|(k, _)| k.clone());

            if let Some(key) = to_remove {
                cache.remove(&key);
                debug!("Evicted content cache entry: {:?}", key);
            }
        }
    }

    fn record_metadata_hit(&self) {
        let mut stats = self.stats.write().expect("Lock poisoned");
        stats.metadata_hits += 1;
    }

    fn record_metadata_miss(&self) {
        let mut stats = self.stats.write().expect("Lock poisoned");
        stats.metadata_misses += 1;
    }

    fn record_content_hit(&self) {
        let mut stats = self.stats.write().expect("Lock poisoned");
        stats.content_hits += 1;
    }

    fn record_content_miss(&self) {
        let mut stats = self.stats.write().expect("Lock poisoned");
        stats.content_misses += 1;
    }

    fn record_stale_hit(&self) {
        let mut stats = self.stats.write().expect("Lock poisoned");
        stats.stale_hits += 1;
    }
}

impl Default for FileStateCache {
    fn default() -> Self {
        Self::new()
    }
}

/// 批量检查文件变更的工具函数
pub fn find_changed_files(cache: &FileStateCache, paths: &[PathBuf]) -> Vec<(PathBuf, bool)> {
    paths
        .iter()
        .map(|path| {
            if let Some(metadata) = cache.metadata(path) {
                (path.clone(), metadata.is_stale())
            } else {
                (path.clone(), true) // 文件不存在视为已变更
            }
        })
        .collect()
}

/// 扫描项目目录并缓存所有文件的元数据
pub fn scan_project(
    cache: &FileStateCache,
    dir: impl AsRef<Path>,
    respect_gitignore: bool,
) -> usize {
    let dir = dir.as_ref();
    let mut count = 0;

    let walker = {
        let mut builder = ignore::WalkBuilder::new(dir);
        builder.hidden(false);
        if respect_gitignore {
            builder.git_ignore(true);
        } else {
            builder.git_ignore(false);
        }
        builder.build()
    };

    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // 跳过二进制/大文件
        if let Some(size) = entry.metadata().ok().map(|m| m.len()) {
            if size > 5 * 1024 * 1024 {
                // 跳过 > 5MB 文件
                continue;
            }
        }

        let _ = cache.metadata(path);
        count += 1;
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_file_metadata() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file_cache.txt");

        // 创建测试文件
        {
            let mut file = std::fs::File::create(&test_file).unwrap();
            file.write_all(b"Hello, World!").unwrap();
        }

        let cache = FileStateCache::new();

        // 第一次读取（缓存未命中）
        let metadata1 = cache.metadata(&test_file).unwrap();
        assert_eq!(metadata1.size, 13);

        // 第二次读取（缓存命中）
        let metadata2 = cache.metadata(&test_file).unwrap();
        assert_eq!(metadata2.size, metadata1.size);

        let stats = cache.stats();
        assert_eq!(stats.metadata_hits, 1);
        assert_eq!(stats.metadata_misses, 1);

        // 清理
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_file_content_cache() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_content_cache.txt");

        // 创建测试文件
        {
            let mut file = std::fs::File::create(&test_file).unwrap();
            file.write_all(b"Test content").unwrap();
        }

        let cache = FileStateCache::new();

        // 第一次读取（缓存未命中）
        let content1 = cache.content(&test_file).unwrap();
        assert_eq!(content1, "Test content");

        // 第二次读取（缓存命中）
        let content2 = cache.content(&test_file).unwrap();
        assert_eq!(content2, "Test content");

        let stats = cache.stats();
        assert_eq!(stats.content_hits, 1);
        assert_eq!(stats.content_misses, 1);

        // 清理
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_stale_detection() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_stale.txt");

        // 创建测试文件
        {
            let mut file = std::fs::File::create(&test_file).unwrap();
            file.write_all(b"Original").unwrap();
        }

        let cache = FileStateCache::new();

        // 读取内容
        let content1 = cache.content(&test_file).unwrap();
        assert_eq!(content1, "Original");

        // 修改文件
        std::thread::sleep(std::time::Duration::from_millis(10));
        {
            let mut file = std::fs::File::create(&test_file).unwrap();
            file.write_all(b"Modified").unwrap();
        }

        // 再次读取，应该检测到变更
        let content2 = cache.content(&test_file).unwrap();
        assert_eq!(content2, "Modified");

        let stats = cache.stats();
        assert!(stats.stale_hits >= 1);

        // 清理
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn session_read_state_is_isolated_by_session_id() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!(
            "test_session_read_state_{}.txt",
            uuid::Uuid::new_v4()
        ));

        {
            let mut file = std::fs::File::create(&test_file).unwrap();
            file.write_all(b"session scoped").unwrap();
        }

        let cache = FileStateCache::new();
        cache.mark_read_for_session("session-a", &test_file);

        assert!(cache.is_unchanged_since_last_read_for_session("session-a", &test_file));
        assert!(!cache.is_unchanged_since_last_read_for_session("session-b", &test_file));

        let _ = std::fs::remove_file(&test_file);
    }
}
