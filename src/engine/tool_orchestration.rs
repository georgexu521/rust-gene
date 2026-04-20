//! 工具并行协调器
//!
//! 对标 Claude Code 的 `toolOrchestration.ts`
//! 读工具并行执行，写工具串行执行

use crate::tools::{ToolContext, ToolResult};
use crate::services::api::ToolCall;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 工具并发安全属性
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolConcurrencySafety {
    /// 读工具，可以并行执行
    ReadOnly,
    /// 写工具，必须串行执行
    Write,
    /// 混合工具（既读又写）
    Mixed,
    /// 未知，需要动态判断
    Unknown,
}

/// 工具执行结果
#[derive(Debug)]
pub struct OrchestratedResult {
    pub results: Vec<ToolCallResult>,
    pub total_duration_ms: u64,
    pub read_parallel_ms: u64,
    pub write_serial_ms: u64,
}

/// 单个工具调用结果
#[derive(Debug)]
pub struct ToolCallResult {
    pub call_id: String,
    pub result: Result<ToolResult, String>,
    pub was_parallel: bool,
    pub duration_ms: u64,
}

/// 工具编排器
pub struct ToolOrchestrator {
    /// 是否启用并行执行
    enabled: bool,
    /// 读工具缓存（避免重复检测）
    read_tool_cache: Arc<RwLock<HashSet<String>>>,
    /// 写工具缓存
    write_tool_cache: Arc<RwLock<HashSet<String>>>,
}

impl ToolOrchestrator {
    /// 创建新的编排器
    pub fn new() -> Self {
        let enabled = std::env::var("PRIORITY_AGENT_TOOL_CONCURRENCY")
            .ok()
            .map(|v| v == "1")
            .unwrap_or(true); // 默认启用

        Self {
            enabled,
            read_tool_cache: Arc::new(RwLock::new(HashSet::new())),
            write_tool_cache: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// 检测工具的并发安全属性
    pub fn get_tool_safety(tool_name: &str) -> ToolConcurrencySafety {
        // 读工具列表
        let read_tools: HashSet<&str> = [
            "grep",
            "glob",
            "file_read",
            "lsp",
            "symbol_query",
            "web_fetch",
            "web_search",
            "project_list",
            "memory_load",
            "calculate",
            "json_query",
            "encode",
            "ask_user",
            "worktree_list",
            "session_list",
            "mcp_list_tools",
            "mcp_list_servers",
            "diagnostic_tracker",
        ]
        .into_iter()
        .collect();

        // 写工具列表
        let write_tools: HashSet<&str> = [
            "file_write",
            "file_edit",
            "bash",
            "task_create",
            "task_update",
            "task_done",
            "agent",
            "plan",
            "skill_manage",
            "memory_save",
            "socratic_analyze",
            "mcp",
            "swarm",
            "worktree_create",
            "worktree_remove",
            "remote_trigger",
            "bundle",
            "plugin_manage",
        ]
        .into_iter()
        .collect();

        if read_tools.contains(tool_name) {
            ToolConcurrencySafety::ReadOnly
        } else if write_tools.contains(tool_name) {
            ToolConcurrencySafety::Write
        } else {
            ToolConcurrencySafety::Unknown
        }
    }

    /// 分区工具调用
    pub fn partition_tool_calls(&self, calls: &[ToolCall]) -> (Vec<ToolCall>, Vec<ToolCall>) {
        let mut read_calls = Vec::new();
        let mut write_calls = Vec::new();

        for call in calls {
            let safety = Self::get_tool_safety(&call.name);
            match safety {
                ToolConcurrencySafety::ReadOnly => read_calls.push(call.clone()),
                ToolConcurrencySafety::Write => write_calls.push(call.clone()),
                ToolConcurrencySafety::Mixed => {
                    // 混合工具按写处理（更保守）
                    write_calls.push(call.clone());
                }
                ToolConcurrencySafety::Unknown => {
                    // 未知工具默认按写处理
                    debug!("Unknown tool safety for {}, treating as write", call.name);
                    write_calls.push(call.clone());
                }
            }
        }

        (read_calls, write_calls)
    }

    /// 执行工具调用（编排模式）
    pub async fn execute_orchestrated(
        &self,
        calls: Vec<ToolCall>,
        context: &ToolContext,
        tool_registry: &Arc<crate::tools::ToolRegistry>,
    ) -> OrchestratedResult {
        if !self.enabled || calls.len() <= 1 {
            // 禁用或单工具，直接串行执行
            return self.execute_serial(calls, context, tool_registry).await;
        }

        let start = std::time::Instant::now();
        let (read_calls, write_calls) = self.partition_tool_calls(&calls);

        info!(
            "Tool orchestration: {} read (parallel), {} write (serial)",
            read_calls.len(),
            write_calls.len()
        );

        // 并行执行读工具
        let read_start = std::time::Instant::now();
        let read_results = self.execute_read_parallel(read_calls, context, tool_registry).await;
        let read_duration = read_start.elapsed().as_millis() as u64;

        // 串行执行写工具
        let write_start = std::time::Instant::now();
        let write_results = self.execute_write_serial(write_calls, context, tool_registry).await;
        let write_duration = write_start.elapsed().as_millis() as u64;

        // 合并结果
        let mut all_results = read_results;
        all_results.extend(write_results);

        let total = start.elapsed().as_millis() as u64;

        OrchestratedResult {
            results: all_results,
            total_duration_ms: total,
            read_parallel_ms: read_duration,
            write_serial_ms: write_duration,
        }
    }

    /// 串行执行（禁用并行时的回退）
    async fn execute_serial(
        &self,
        calls: Vec<ToolCall>,
        context: &ToolContext,
        tool_registry: &Arc<crate::tools::ToolRegistry>,
    ) -> OrchestratedResult {
        let start = std::time::Instant::now();
        let mut results = Vec::new();

        for call in calls {
            let call_start = std::time::Instant::now();
            let result = ToolOrchestrator::execute_single(&call, context, tool_registry)
                .await;
            let duration = call_start.elapsed().as_millis() as u64;

            results.push(ToolCallResult {
                call_id: call.id.clone(),
                result: Ok(result),
                was_parallel: false,
                duration_ms: duration,
            });
        }

        let total = start.elapsed().as_millis() as u64;

        OrchestratedResult {
            results,
            total_duration_ms: total,
            read_parallel_ms: 0,
            write_serial_ms: total,
        }
    }

    /// 并行执行读工具
    async fn execute_read_parallel(
        &self,
        calls: Vec<ToolCall>,
        context: &ToolContext,
        tool_registry: &Arc<crate::tools::ToolRegistry>,
    ) -> Vec<ToolCallResult> {
        if calls.is_empty() {
            return Vec::new();
        }

        use futures::StreamExt;
        use futures::stream::FuturesUnordered;

        // 克隆 call IDs 以便后续排序
        let call_ids: Vec<String> = calls.iter().map(|c| c.id.clone()).collect();

        let mut futures = FuturesUnordered::new();

        for call in calls {
            let ctx = context.clone();
            let registry = Arc::clone(tool_registry);

            futures.push(async move {
                let call_start = std::time::Instant::now();
                let result = ToolOrchestrator::execute_single(&call, &ctx, &registry).await;
                let duration = call_start.elapsed().as_millis() as u64;

                ToolCallResult {
                    call_id: call.id.clone(),
                    result: Ok(result),
                    was_parallel: true,
                    duration_ms: duration,
                }
            });
        }

        let mut results = Vec::new();
        while let Some(result) = futures.next().await {
            results.push(result);
        }

        // 按原始顺序排序
        results.sort_by(|a, b| {
            let a_idx = call_ids.iter().position(|id| id == &a.call_id).unwrap_or(0);
            let b_idx = call_ids.iter().position(|id| id == &b.call_id).unwrap_or(0);
            a_idx.cmp(&b_idx)
        });

        results
    }

    /// 串行执行写工具
    async fn execute_write_serial(
        &self,
        calls: Vec<ToolCall>,
        context: &ToolContext,
        tool_registry: &Arc<crate::tools::ToolRegistry>,
    ) -> Vec<ToolCallResult> {
        let mut results = Vec::new();

        for call in calls {
            let call_start = std::time::Instant::now();

            // 写工具执行前检查（如果有诊断跟踪器）
            #[cfg(feature = "experimental-priority")]
            if let Some(diagnostic_tracker) = &context.diagnostic_tracker {
                // 获取工具关联的文件路径（如果有）
                if let Some(path) = self.get_tool_related_path(&call) {
                    let diags = context.lsp_manager.as_ref()
                        .map(|m| m.get_diagnostics(&path).unwrap_or_default())
                        .unwrap_or_default();
                    diagnostic_tracker.before_edit(&path, diags).await;
                }
            }

            let result = ToolOrchestrator::execute_single(&call, context, tool_registry)
                .await;
            let duration = call_start.elapsed().as_millis() as u64;

            results.push(ToolCallResult {
                call_id: call.id.clone(),
                result: Ok(result),
                was_parallel: false,
                duration_ms: duration,
            });
        }

        results
    }

    /// 执行单个工具
    async fn execute_single(
        call: &ToolCall,
        context: &ToolContext,
        tool_registry: &Arc<crate::tools::ToolRegistry>,
    ) -> ToolResult {
        let tool = match tool_registry.get(&call.name) {
            Some(t) => t,
            None => {
                return crate::tools::ToolResult::error(format!("Tool not found: {}", call.name));
            }
        };

        debug!("Executing tool: {} (id: {})", call.name, call.id);

        let result = tool.execute(call.arguments.clone(), context.clone()).await;

        if result.error.is_some() {
            warn!("Tool {} returned error: {}", call.name, result.content);
        }

        result
    }

    /// 获取工具关联的文件路径（用于诊断跟踪）
    #[cfg(feature = "experimental-priority")]
    fn get_tool_related_path(&self, call: &ToolCall) -> Option<std::path::PathBuf> {
        match call.name.as_str() {
            "file_write" | "file_edit" => {
                call.arguments.get("path")
                    .or_else(|| call.arguments.get("file_path"))
                    .and_then(|v| v.as_str())
                    .map(std::path::PathBuf::from)
            }
            _ => None,
        }
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> ToolOrchestratorStats {
        let read_cache = self.read_tool_cache.read().await;
        let write_cache = self.write_tool_cache.read().await;

        ToolOrchestratorStats {
            read_tools_known: read_cache.len(),
            write_tools_known: write_cache.len(),
            enabled: self.enabled,
        }
    }
}

impl Default for ToolOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ToolOrchestratorStats {
    pub read_tools_known: usize,
    pub write_tools_known: usize,
    pub enabled: bool,
}

/// 判断一组工具调用是否可以并行执行
pub fn can_execute_parallel(calls: &[&ToolCall]) -> bool {
    for call in calls {
        let safety = ToolOrchestrator::get_tool_safety(&call.name);
        if safety == ToolConcurrencySafety::Write || safety == ToolConcurrencySafety::Mixed {
            return false;
        }
    }
    true
}

/// 获取工具调度的描述
pub fn describe_partition(read_count: usize, write_count: usize) -> String {
    if read_count == 0 && write_count == 0 {
        "No tools".to_string()
    } else if read_count == 0 {
        format!("{} write (serial)", write_count)
    } else if write_count == 0 {
        format!("{} read (parallel)", read_count)
    } else {
        format!("{} read (parallel) + {} write (serial)", read_count, write_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_safety() {
        assert_eq!(
            ToolOrchestrator::get_tool_safety("grep"),
            ToolConcurrencySafety::ReadOnly
        );
        assert_eq!(
            ToolOrchestrator::get_tool_safety("file_write"),
            ToolConcurrencySafety::Write
        );
        assert_eq!(
            ToolOrchestrator::get_tool_safety("file_edit"),
            ToolConcurrencySafety::Write
        );
        assert_eq!(
            ToolOrchestrator::get_tool_safety("unknown_tool"),
            ToolConcurrencySafety::Unknown
        );
    }

    #[test]
    fn test_partition() {
        let orchestrator = ToolOrchestrator::new();

        let calls = vec![
            ToolCall {
                id: "1".to_string(),
                name: "grep".to_string(),
                arguments: serde_json::json!({}),
            },
            ToolCall {
                id: "2".to_string(),
                name: "file_read".to_string(),
                arguments: serde_json::json!({}),
            },
            ToolCall {
                id: "3".to_string(),
                name: "file_write".to_string(),
                arguments: serde_json::json!({}),
            },
        ];

        let (read, write) = orchestrator.partition_tool_calls(&calls);

        assert_eq!(read.len(), 2);
        assert_eq!(write.len(), 1);
    }

    #[test]
    fn test_can_execute_parallel() {
        let call1 = ToolCall {
            id: "1".to_string(),
            name: "grep".to_string(),
            arguments: serde_json::json!({}),
        };
        let call2 = ToolCall {
            id: "2".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({}),
        };

        let calls = vec![&call1, &call2];

        assert!(can_execute_parallel(&calls));

        let call3 = ToolCall {
            id: "3".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({}),
        };

        let calls_with_write = vec![&call1, &call3];

        assert!(!can_execute_parallel(&calls_with_write));
    }

    #[test]
    fn test_describe_partition() {
        assert_eq!(describe_partition(0, 0), "No tools");
        assert_eq!(describe_partition(0, 3), "3 write (serial)");
        assert_eq!(describe_partition(2, 0), "2 read (parallel)");
        assert_eq!(describe_partition(2, 3), "2 read (parallel) + 3 write (serial)");
    }
}
