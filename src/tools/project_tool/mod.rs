//! 项目索引工具 - 高性能文件搜索
//!
//! 特性：
//! - 模糊搜索评分 (fzf/nucleo 风格)
//! - 增量索引缓存 (mtime 检测变更)
//! - .gitignore 支持
//! - 异步友好 (spawn_blocking)

mod fuzzy;
mod gitignore;

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use fuzzy::{fuzzy_search, SearchResult};
use gitignore::GitIgnore;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::{Duration, Instant, SystemTime};

// ── 全局索引缓存 ──────────────────────────────────────────

/// 全局文件索引缓存，按项目根目录 key
static INDEX_CACHE: LazyLock<Arc<RwLock<IndexCache>>> =
    LazyLock::new(|| Arc::new(RwLock::new(IndexCache::new())));

/// 缓存的文件索引
#[derive(Clone)]
struct CachedIndex {
    /// 文件列表（Arc 共享，避免 cache hit 时 clone 整个 Vec）
    files: Arc<Vec<String>>,
    /// 目录树摘要
    tree_summary: Arc<String>,
    /// 索引构建时间
    built_at: Instant,
    /// Git index mtime（用于检测变更）
    git_index_mtime: Option<SystemTime>,
    /// 项目根目录
    root: PathBuf,
}

impl CachedIndex {
    /// 检查缓存是否仍然有效
    fn is_valid(&self) -> bool {
        // TTL: 30 秒内有效
        if self.built_at.elapsed() < Duration::from_secs(30) {
            return true;
        }

        // TTL 过期后，检查 git index mtime 是否变化
        match self.git_index_mtime {
            Some(cached) => self.get_git_index_mtime().ok() == Some(cached),
            None => false,
        }
    }

    /// 获取 .git/index 的 mtime
    fn get_git_index_mtime(&self) -> std::io::Result<SystemTime> {
        std::fs::metadata(self.root.join(".git").join("index"))?.modified()
    }
}

/// 索引缓存管理器
struct IndexCache {
    cache: std::collections::HashMap<PathBuf, CachedIndex>,
}

impl IndexCache {
    fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
        }
    }

    /// 获取缓存索引（如果有效）
    fn get(&self, root: &Path) -> Option<&CachedIndex> {
        self.cache.get(root).filter(|idx| idx.is_valid())
    }

    /// 存入缓存
    fn insert(&mut self, root: PathBuf, index: CachedIndex) {
        self.cache.insert(root, index);
    }

    /// 强制刷新缓存
    fn invalidate(&mut self, root: &Path) {
        self.cache.remove(root);
    }
}

// ── 项目扫描器 ──────────────────────────────────────────

/// 项目扫描器 - 带缓存和 .gitignore 支持
pub struct ProjectScanner {
    /// 文件列表
    files: Arc<Vec<String>>,
    /// 目录树摘要
    tree_summary: Arc<String>,
    /// 项目根目录
    root: PathBuf,
}

impl ProjectScanner {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Vec::new()),
            tree_summary: Arc::new(String::new()),
            root: PathBuf::new(),
        }
    }

    /// 扫描项目目录（带缓存）
    ///
    /// 优先从全局缓存获取，缓存无效时重新扫描
    pub fn scan(&mut self, root: &Path) {
        self.root = root.to_path_buf();

        // 尝试从缓存获取（Arc clone，O(1)）
        if let Ok(cache) = INDEX_CACHE.read() {
            if let Some(cached) = cache.get(root) {
                self.files = Arc::clone(&cached.files);
                self.tree_summary = Arc::clone(&cached.tree_summary);
                return;
            }
        }

        // 缓存无效，重新扫描
        self.do_scan(root);

        // 写入缓存
        let git_index_mtime = self.get_git_index_mtime(root);
        let index = CachedIndex {
            files: Arc::clone(&self.files),
            tree_summary: Arc::clone(&self.tree_summary),
            built_at: Instant::now(),
            git_index_mtime,
            root: root.to_path_buf(),
        };
        if let Ok(mut cache) = INDEX_CACHE.write() {
            cache.insert(root.to_path_buf(), index);
        }
    }

    /// 实际执行扫描
    fn do_scan(&mut self, root: &Path) {
        let mut files = Vec::new();

        // 优先用 git ls-files（天然尊重 .gitignore）
        if let Ok(output) = std::process::Command::new("git")
            .args(["ls-files", "--cached", "--others", "--exclude-standard"])
            .current_dir(root)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                files = stdout
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(String::from)
                    .collect();
            }
        }

        // 回退：手动遍历（带 .gitignore 支持）
        if files.is_empty() {
            let mut gi = GitIgnore::new();
            gi.load_from_dir(root);
            self.walk_directory(root, root, &gi, &mut files);
        }

        // 去重并排序
        files.sort();
        files.dedup();

        // 构建目录树摘要
        let tree_summary = self.build_tree_summary(&files);

        self.files = Arc::new(files);
        self.tree_summary = Arc::new(tree_summary);
    }

    /// 手动遍目录（带 gitignore 支持）
    fn walk_directory(&self, dir: &Path, root: &Path, gi: &GitIgnore, files: &mut Vec<String>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            let relative = match path.strip_prefix(root) {
                Ok(r) => r.to_string_lossy().to_string(),
                Err(_) => continue,
            };

            if path.is_dir() {
                if !gi.should_skip_dir(&name, &relative) {
                    self.walk_directory(&path, root, gi, files);
                }
            } else if !gi.should_ignore(&relative) {
                files.push(relative);
            }
        }
    }

    /// 获取 .git/index 的 mtime
    fn get_git_index_mtime(&self, root: &Path) -> Option<SystemTime> {
        std::fs::metadata(root.join(".git").join("index"))
            .ok()?
            .modified()
            .ok()
    }

    /// 构建目录树摘要
    fn build_tree_summary(&self, files: &[String]) -> String {
        use std::collections::BTreeMap;

        let mut dirs: BTreeMap<String, usize> = BTreeMap::new();
        let mut exts: BTreeMap<String, usize> = BTreeMap::new();

        for file in files {
            if let Some(parent) = Path::new(file).parent() {
                let dir = parent.to_string_lossy().to_string();
                if !dir.is_empty() && !dir.contains('/') {
                    *dirs.entry(dir).or_insert(0) += 1;
                }
            }

            if let Some(ext) = Path::new(file).extension() {
                let ext = ext.to_string_lossy().to_string();
                *exts.entry(ext).or_insert(0) += 1;
            }
        }

        let mut summary = String::new();
        summary.push_str(&format!("Project: {} files\n\n", files.len()));

        if !dirs.is_empty() {
            summary.push_str("Top-level directories:\n");
            for (dir, count) in dirs.iter().take(15) {
                summary.push_str(&format!("  {}/ ({} files)\n", dir, count));
            }
            summary.push('\n');
        }

        if !exts.is_empty() {
            summary.push_str("File types: ");
            let ext_list: Vec<String> = exts
                .iter()
                .take(10)
                .map(|(ext, count)| format!("*.{} ({})", ext, count))
                .collect();
            summary.push_str(&ext_list.join(", "));
            summary.push('\n');
        }

        summary
    }

    /// 获取文件列表
    pub fn files(&self) -> &[String] {
        &self.files
    }

    /// 获取目录树摘要
    pub fn tree_summary(&self) -> &str {
        &self.tree_summary
    }

    /// 模糊搜索文件（带评分排序）
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        fuzzy_search(query, &self.files, limit)
    }

    /// 获取指定目录下的文件
    pub fn files_in_dir(&self, dir: &str) -> Vec<&String> {
        self.files.iter().filter(|f| f.starts_with(dir)).collect()
    }
}

impl Default for ProjectScanner {
    fn default() -> Self {
        Self::new()
    }
}

// ── 工具接口 ──────────────────────────────────────────

/// Project List 工具 - 让 agent 查看项目结构
pub struct ProjectListTool;

#[async_trait]
impl Tool for ProjectListTool {
    fn name(&self) -> &str {
        "project_list"
    }

    fn description(&self) -> &str {
        "List all files in the project or search for files with fuzzy matching. \
         Use this to understand the project structure and find relevant files. \
         Cached for 30s to avoid repeated scans."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["summary", "list", "search", "dir", "refresh"],
                    "description": "summary: show project overview. list: all files. \
                                   search: fuzzy find files matching query. \
                                   dir: list files in directory. \
                                   refresh: force rebuild index cache."
                },
                "query": {
                    "type": "string",
                    "description": "Search query (for 'search') or directory path (for 'dir')"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return (default 30, max 100)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("summary");
        let query = params["query"].as_str().unwrap_or("");
        let limit = params["limit"].as_u64().unwrap_or(30).clamp(1, 100) as usize;

        // refresh: 强制刷新缓存
        if action == "refresh" {
            if let Ok(mut cache) = INDEX_CACHE.write() {
                cache.invalidate(&context.working_dir);
            }
        }

        // 异步扫描（大项目不会阻塞）
        let working_dir = context.working_dir.clone();
        let scanner = tokio::task::spawn_blocking(move || {
            let mut s = ProjectScanner::new();
            s.scan(&working_dir);
            s
        })
        .await
        .unwrap_or_else(|_| ProjectScanner::new());

        match action {
            "summary" => ToolResult::success(scanner.tree_summary().to_string()),

            "list" => {
                let files = scanner.files();
                if files.len() > limit {
                    let preview: Vec<String> = files.iter().take(limit).cloned().collect();
                    ToolResult::success(format!(
                        "{} files total. First {}:\n{}\n\n... use 'search' to find specific files",
                        files.len(),
                        limit,
                        preview.join("\n")
                    ))
                } else {
                    ToolResult::success(files.join("\n"))
                }
            }

            "search" => {
                if query.is_empty() {
                    return ToolResult::error("Query required for search action");
                }
                let results = scanner.search(query, limit);
                if results.is_empty() {
                    ToolResult::success(format!("No files matching '{}'", query))
                } else {
                    let output: Vec<String> = results
                        .iter()
                        .map(|r| format!("  {} (score: {})", r.path, r.score))
                        .collect();
                    ToolResult::success(format!(
                        "{} fuzzy matches:\n{}",
                        results.len(),
                        output.join("\n")
                    ))
                }
            }

            "dir" => {
                if query.is_empty() {
                    return ToolResult::error("Directory path required for dir action");
                }
                let results = scanner.files_in_dir(query);
                if results.is_empty() {
                    ToolResult::success(format!("No files in directory '{}'", query))
                } else {
                    let output: Vec<String> =
                        results.iter().take(50).map(|s| s.to_string()).collect();
                    ToolResult::success(format!(
                        "{} files in {}:\n{}",
                        results.len(),
                        query,
                        output.join("\n")
                    ))
                }
            }

            "refresh" => ToolResult::success(format!(
                "Index refreshed. {} files found.",
                scanner.files().len()
            )),

            _ => ToolResult::error(format!(
                "Unknown action: {}. Use summary, list, search, dir, or refresh",
                action
            )),
        }
    }
}

// ── 测试 ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_scan() {
        let mut scanner = ProjectScanner::new();
        scanner.scan(Path::new("."));
        assert!(!scanner.files().is_empty());
    }

    #[test]
    fn test_scanner_fuzzy_search() {
        let mut scanner = ProjectScanner::new();
        scanner.scan(Path::new("."));

        let results = scanner.search("main", 10);
        assert!(!results.is_empty());
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }
    }

    #[test]
    fn test_scanner_tree_summary() {
        let mut scanner = ProjectScanner::new();
        scanner.scan(Path::new("."));

        let summary = scanner.tree_summary();
        assert!(summary.contains("files"));
    }

    #[test]
    fn test_scanner_files_in_dir() {
        let mut scanner = ProjectScanner::new();
        scanner.scan(Path::new("."));

        let src_files = scanner.files_in_dir("src");
        assert!(!src_files.is_empty());
    }

    #[test]
    fn test_cache_hit() {
        let mut scanner1 = ProjectScanner::new();
        scanner1.scan(Path::new("."));
        let count1 = scanner1.files().len();

        let mut scanner2 = ProjectScanner::new();
        scanner2.scan(Path::new("."));
        let count2 = scanner2.files().len();

        assert_eq!(count1, count2);
    }

    #[test]
    fn test_gitignore_respected() {
        let mut scanner = ProjectScanner::new();
        scanner.scan(Path::new("."));

        for file in scanner.files() {
            assert!(
                !file.starts_with("target/"),
                "target/ should be ignored but found: {}",
                file
            );
        }
    }

    #[test]
    fn test_fuzzy_search_no_match() {
        let mut scanner = ProjectScanner::new();
        scanner.scan(Path::new("."));

        let results = scanner.search("xyznonexistent", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_files_sorted() {
        let mut scanner = ProjectScanner::new();
        scanner.scan(Path::new("."));

        let files = scanner.files();
        for i in 1..files.len() {
            assert!(files[i - 1] <= files[i], "Files should be sorted");
        }
    }
}
