//! 诊断跟踪服务
//!
//! 对标 Claude Code 的 `diagnosticTracking.ts`
//! 编辑前捕获 baseline diagnostics，编辑后对比找出新增错误

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// 诊断条目
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub source: String,
    pub range: DiagnosticRange,
}

#[derive(Debug, Clone, Copy)]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

#[derive(Debug, Clone)]
pub struct DiagnosticRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// 诊断跟踪器
pub struct DiagnosticTracker {
    /// 按文件路径存储的 baseline diagnostics
    baselines: Arc<RwLock<HashMap<String, Vec<DiagnosticEntry>>>>,
    /// 是否已启用（由环境变量控制）
    enabled: bool,
}

impl DiagnosticTracker {
    /// 创建新的诊断跟踪器
    pub fn new() -> Self {
        let enabled = std::env::var("PRIORITY_AGENT_DIAGNOSTIC_TRACKING")
            .ok()
            .map(|v| v == "1")
            .unwrap_or(false);

        Self {
            baselines: Arc::new(RwLock::new(HashMap::new())),
            enabled,
        }
    }

    /// 编辑前捕获 baseline（调用此方法在 file_edit 之前）
    /// 注意：需要外部提供 LSP diagnostics 数据，此处仅记录 baseline
    pub async fn before_edit(&self, file_path: &Path, current_diagnostics: Vec<DiagnosticEntry>) {
        if !self.enabled {
            return;
        }

        let path_str = file_path.to_string_lossy().to_string();
        info!(
            "Capturing diagnostic baseline for: {} ({} diagnostics)",
            path_str,
            current_diagnostics.len()
        );

        let mut baselines = self.baselines.write().await;
        baselines.insert(path_str, current_diagnostics);
    }

    /// 编辑后获取新增的 diagnostics（对比 baseline）
    /// 注意：需要外部提供当前 LSP diagnostics 数据
    pub async fn get_new_diagnostics(
        &self,
        file_path: &Path,
        current_diagnostics: Vec<DiagnosticEntry>,
    ) -> Vec<DiagnosticEntry> {
        if !self.enabled {
            return Vec::new();
        }

        let path_str = file_path.to_string_lossy().to_string();

        // 获取 baseline
        let baselines = self.baselines.read().await;
        let baseline = baselines.get(&path_str).cloned().unwrap_or_default();

        // 对比找出新增的 diagnostics
        let new_diags: Vec<DiagnosticEntry> = current_diagnostics
            .into_iter()
            .filter(|curr| {
                // 如果 baseline 中没有这个 diagnostic，则是新增的
                !baseline.iter().any(|b| self.is_same_diagnostic(b, curr))
            })
            .collect();

        if !new_diags.is_empty() {
            info!("Found {} new diagnostics for {}", new_diags.len(), path_str);
        }

        new_diags
    }

    /// 检查两个 diagnostic 是否是同一个
    fn is_same_diagnostic(&self, a: &DiagnosticEntry, b: &DiagnosticEntry) -> bool {
        a.message == b.message
            && a.range.start_line == b.range.start_line
            && a.range.start_col == b.range.start_col
    }

    /// 清除文件的 baseline
    pub async fn clear_baseline(&self, file_path: &Path) {
        let path_str = file_path.to_string_lossy().to_string();
        let mut baselines = self.baselines.write().await;
        baselines.remove(&path_str);
        debug!("Cleared diagnostic baseline for: {}", path_str);
    }

    /// 清除所有 baselines
    pub async fn clear_all(&mut self) {
        let mut baselines = self.baselines.write().await;
        baselines.clear();
        info!("Cleared all diagnostic baselines");
    }

    /// 获取跟踪的文件数量
    pub async fn tracked_count(&self) -> usize {
        let baselines = self.baselines.read().await;
        baselines.len()
    }
}

impl Default for DiagnosticTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_diagnostic_tracker_no_baseline() {
        let tracker = DiagnosticTracker::new();
        // 没有 baseline 时，应该返回空
        let new_diags = tracker
            .get_new_diagnostics(Path::new("test.rs"), vec![])
            .await;
        assert!(new_diags.is_empty());
    }

    #[tokio::test]
    async fn test_clear_baseline() {
        let tracker = DiagnosticTracker::new();
        tracker.clear_baseline(Path::new("test.rs")).await;
        // 清除后跟踪数为 0
        assert_eq!(tracker.tracked_count().await, 0);
    }
}
