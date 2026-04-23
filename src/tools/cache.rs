//! 工具结果缓存
//!
//! 缓存工具执行结果，避免重复执行相同的工具调用

use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// 缓存条目
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub result: Value,
    pub created_at: Instant,
    pub access_count: u64,
    pub last_accessed: Instant,
}

impl CacheEntry {
    pub fn new(result: Value) -> Self {
        let now = Instant::now();
        Self {
            result,
            created_at: now,
            access_count: 1,
            last_accessed: now,
        }
    }

    pub fn touch(&mut self) {
        self.access_count += 1;
        self.last_accessed = Instant::now();
    }
}

/// 缓存键
#[derive(Debug, Clone, Eq)]
pub struct CacheKey {
    pub tool_name: String,
    pub params: Value,
    pub working_dir: String,
}

impl PartialEq for CacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.tool_name == other.tool_name
            && self.working_dir == other.working_dir
            && self.params == other.params
    }
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tool_name.hash(state);
        self.working_dir.hash(state);
        // 使用 serde_json 的字符串表示来哈希
        self.params.to_string().hash(state);
    }
}

/// 工具结果缓存配置
#[derive(Debug, Clone)]
pub struct ToolCacheConfig {
    /// 默认 TTL (秒)
    pub default_ttl_secs: u64,
    /// 最大缓存条目数
    pub max_entries: usize,
    /// 是否启用缓存
    pub enabled: bool,
    /// 特定工具的 TTL 覆盖
    pub tool_ttls: HashMap<String, u64>,
}

impl Default for ToolCacheConfig {
    fn default() -> Self {
        let mut tool_ttls = HashMap::new();
        // file_read 工具缓存较长时间
        tool_ttls.insert("file_read".to_string(), 300); // 5分钟
                                                        // glob 工具缓存中等时间
        tool_ttls.insert("glob".to_string(), 60); // 1分钟
                                                  // project_list 缓存较长时间
        tool_ttls.insert("project_list".to_string(), 30); // 30秒
                                                          // calculate 工具永久缓存（数学结果不变）
        tool_ttls.insert("calculate".to_string(), 3600); // 1小时
                                                         // datetime 不缓存（时间会变）
        tool_ttls.insert("datetime".to_string(), 0); // 不缓存

        Self {
            default_ttl_secs: 60,
            max_entries: 1000,
            enabled: true,
            tool_ttls,
        }
    }
}

/// 工具结果缓存
#[derive(Debug)]
pub struct ToolResultCache {
    cache: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
    config: ToolCacheConfig,
    stats: Arc<RwLock<CacheStats>>,
}

/// 缓存统计
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub total_entries: usize,
}

impl ToolResultCache {
    /// 创建新缓存
    pub fn new() -> Self {
        Self::with_config(ToolCacheConfig::default())
    }

    /// 使用配置创建缓存
    pub fn with_config(config: ToolCacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// 禁用缓存
    pub fn disabled() -> Self {
        Self::with_config(ToolCacheConfig {
            enabled: false,
            ..Default::default()
        })
    }

    /// 获取缓存配置
    pub fn config(&self) -> &ToolCacheConfig {
        &self.config
    }

    /// 更新配置
    pub fn set_config(&mut self, config: ToolCacheConfig) {
        self.config = config;
    }

    /// 获取缓存条目
    pub fn get(&self, tool_name: &str, params: &Value, working_dir: &str) -> Option<Value> {
        if !self.config.enabled {
            return None;
        }

        // 检查此工具是否可缓存
        if let Some(&ttl) = self.config.tool_ttls.get(tool_name) {
            if ttl == 0 {
                return None; // 此工具不缓存
            }
        }

        let key = CacheKey {
            tool_name: tool_name.to_string(),
            params: params.clone(),
            working_dir: working_dir.to_string(),
        };

        let mut cache = self.cache.write().expect("Cache lock poisoned");

        if let Some(entry) = cache.get_mut(&key) {
            let ttl = self.get_ttl(tool_name);
            let age = entry.created_at.elapsed();

            if age < Duration::from_secs(ttl) {
                // 缓存命中且未过期
                entry.touch();
                let result = entry.result.clone();
                drop(cache);
                self.record_hit();
                return Some(result);
            } else {
                // 缓存过期，删除
                cache.remove(&key);
            }
        }

        drop(cache);
        self.record_miss();
        None
    }

    /// 设置缓存条目
    pub fn set(&self, tool_name: &str, params: Value, working_dir: &str, result: Value) {
        if !self.config.enabled {
            return;
        }

        // 检查此工具是否可缓存
        if let Some(&ttl) = self.config.tool_ttls.get(tool_name) {
            if ttl == 0 {
                return; // 此工具不缓存
            }
        }

        let key = CacheKey {
            tool_name: tool_name.to_string(),
            params,
            working_dir: working_dir.to_string(),
        };

        let mut cache = self.cache.write().expect("Cache lock poisoned");

        // 检查是否需要淘汰
        if cache.len() >= self.config.max_entries {
            self.evict_oldest(&mut cache);
        }

        cache.insert(key, CacheEntry::new(result));
    }

    /// 使缓存失效（按工具名）
    pub fn invalidate_tool(&self, tool_name: &str) {
        let mut cache = self.cache.write().expect("Cache lock poisoned");
        cache.retain(|key, _| key.tool_name != tool_name);
    }

    /// 使特定条目的缓存失效
    pub fn invalidate(&self, tool_name: &str, params: &Value, working_dir: &str) {
        let key = CacheKey {
            tool_name: tool_name.to_string(),
            params: params.clone(),
            working_dir: working_dir.to_string(),
        };

        let mut cache = self.cache.write().expect("Cache lock poisoned");
        cache.remove(&key);
    }

    /// 清空所有缓存
    pub fn clear(&self) {
        let mut cache = self.cache.write().expect("Cache lock poisoned");
        cache.clear();
    }

    /// 获取缓存统计
    pub fn stats(&self) -> CacheStats {
        let stats = self.stats.read().expect("Stats lock poisoned").clone();
        let cache = self.cache.read().expect("Cache lock poisoned");
        CacheStats {
            total_entries: cache.len(),
            ..stats
        }
    }

    /// 重置统计
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write().expect("Stats lock poisoned");
        *stats = CacheStats::default();
    }

    /// 生成统计报告
    pub fn generate_report(&self) -> String {
        let stats = self.stats();
        let total_requests = stats.hits + stats.misses;
        let hit_rate = if total_requests > 0 {
            (stats.hits as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "Cache Statistics:\n\
             Total entries: {}\n\
             Cache hits: {}\n\
             Cache misses: {}\n\
             Hit rate: {:.1}%\n\
             Evictions: {}",
            stats.total_entries, stats.hits, stats.misses, hit_rate, stats.evictions
        )
    }

    /// 获取工具特定的 TTL
    fn get_ttl(&self, tool_name: &str) -> u64 {
        self.config
            .tool_ttls
            .get(tool_name)
            .copied()
            .unwrap_or(self.config.default_ttl_secs)
    }

    /// 淘汰最旧的条目
    fn evict_oldest(&self, cache: &mut HashMap<CacheKey, CacheEntry>) {
        // 找到最久未访问的条目
        let oldest = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(k, _)| k.clone());

        if let Some(key) = oldest {
            cache.remove(&key);
            let mut stats = self.stats.write().expect("Stats lock poisoned");
            stats.evictions += 1;
        }
    }

    /// 清理过期条目
    pub fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.write().expect("Cache lock poisoned");
        let before_count = cache.len();

        cache.retain(|key, entry| {
            let ttl = self.get_ttl(&key.tool_name);
            entry.created_at.elapsed() < Duration::from_secs(ttl)
        });

        before_count - cache.len()
    }

    fn record_hit(&self) {
        let mut stats = self.stats.write().expect("Stats lock poisoned");
        stats.hits += 1;
    }

    fn record_miss(&self) {
        let mut stats = self.stats.write().expect("Stats lock poisoned");
        stats.misses += 1;
    }
}

impl Default for ToolResultCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = ToolResultCache::new();
        let params = serde_json::json!({"path": "/tmp/test.txt"});
        let result = serde_json::json!({"content": "hello"});

        // 未缓存时应返回 None
        assert!(cache.get("file_read", &params, "/tmp").is_none());

        // 设置缓存
        cache.set("file_read", params.clone(), "/tmp", result.clone());

        // 应返回缓存结果
        let cached = cache.get("file_read", &params, "/tmp");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), result);
    }

    #[test]
    fn test_cache_different_working_dirs() {
        let cache = ToolResultCache::new();
        let params = serde_json::json!({"command": "ls"});
        let result1 = serde_json::json!({"files": ["a.txt"]});
        let result2 = serde_json::json!({"files": ["b.txt"]});

        cache.set("bash", params.clone(), "/dir1", result1.clone());
        cache.set("bash", params.clone(), "/dir2", result2.clone());

        assert_eq!(cache.get("bash", &params, "/dir1"), Some(result1));
        assert_eq!(cache.get("bash", &params, "/dir2"), Some(result2));
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = ToolResultCache::new();
        let params = serde_json::json!({"path": "/tmp/test.txt"});
        let result = serde_json::json!({"content": "hello"});

        cache.set("file_read", params.clone(), "/tmp", result.clone());
        assert!(cache.get("file_read", &params, "/tmp").is_some());

        // 使缓存失效
        cache.invalidate("file_read", &params, "/tmp");
        assert!(cache.get("file_read", &params, "/tmp").is_none());
    }

    #[test]
    fn test_cache_stats() {
        let cache = ToolResultCache::new();
        let params = serde_json::json!({"path": "/tmp/test.txt"});
        let result = serde_json::json!({"content": "hello"});

        cache.set("file_read", params.clone(), "/tmp", result.clone());

        // 两次命中
        let _ = cache.get("file_read", &params, "/tmp");
        let _ = cache.get("file_read", &params, "/tmp");

        // 一次未命中
        let _ = cache.get("file_read", &serde_json::json!({"path": "/other"}), "/tmp");

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.total_entries, 1);
    }

    #[test]
    fn test_uncacheable_tools() {
        let cache = ToolResultCache::new();
        let params = serde_json::json!({"action": "now"});
        let result = serde_json::json!({"time": "12:00"});

        // datetime 工具不应被缓存 (TTL = 0)
        cache.set("datetime", params.clone(), "/tmp", result.clone());
        assert!(cache.get("datetime", &params, "/tmp").is_none());
    }

    #[test]
    fn test_cache_disabled() {
        let cache = ToolCacheConfig {
            enabled: false,
            ..Default::default()
        };
        let cache = ToolResultCache::with_config(cache);
        let params = serde_json::json!({"path": "/tmp/test.txt"});
        let result = serde_json::json!({"content": "hello"});

        cache.set("file_read", params.clone(), "/tmp", result.clone());
        assert!(cache.get("file_read", &params, "/tmp").is_none());
    }
}
