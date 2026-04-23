//! 文件操作工具
//!
//! 提供文件读取、写入、编辑功能

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{debug, error, info, warn};

const MAX_EDITABLE_FILE_SIZE_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB

fn is_unc_or_network_path(path: &str) -> bool {
    path.starts_with("\\\\") || path.starts_with("//")
}

fn check_file_size_limit(path: &Path, operation: &str) -> Result<(), String> {
    let metadata = std::fs::metadata(path).map_err(|e| {
        format!(
            "Failed to read file metadata for {} '{}': {}",
            operation,
            path.display(),
            e
        )
    })?;
    if metadata.len() > MAX_EDITABLE_FILE_SIZE_BYTES {
        return Err(format!(
            "Refusing to {} file '{}': {} bytes exceeds limit {} bytes",
            operation,
            path.display(),
            metadata.len(),
            MAX_EDITABLE_FILE_SIZE_BYTES
        ));
    }
    Ok(())
}

/// 智能引号归一化（Claude Code 模式）
/// 处理文件中的智能引号 vs 模型输出的直引号差异
fn normalize_quotes(input: &str) -> String {
    input
        .replace(['\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}'], "'") // single quotes
        .replace(['\u{201C}', '\u{201D}'], "\"") // double quotes
}

/// 反转义处理（Claude Code 使用 &lt;fnr&gt; 等转义）
fn desanitize(input: &str) -> String {
    input
        .replace("<fnr>", "")
        .replace("<n>", "\n")
        .replace("<TAB>", "\t")
        .replace("<NEWLINE>", "\n")
}

/// 文件读取状态跟踪（用于 must-read-before-edit 检查）
/// 全局读取文件状态跟踪（按会话）
static READ_FILES: Lazy<Mutex<HashMap<String, HashSet<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// 标记文件已被读取（用于 must-read-before-edit 检查）
pub fn mark_file_read(session_id: &str, file_path: &str) {
    let mut tracker = READ_FILES.lock().unwrap_or_else(|e| e.into_inner());
    let session_files = tracker
        .entry(session_id.to_string())
        .or_default();
    session_files.insert(file_path.to_string());
}

/// 检查文件是否已被读取
pub fn is_file_read(session_id: &str, file_path: &str) -> bool {
    let tracker = READ_FILES.lock().unwrap_or_else(|e| e.into_inner());
    tracker
        .get(session_id)
        .map(|s: &HashSet<String>| s.contains(file_path))
        .unwrap_or(false)
}

/// 清除会话的读取状态
pub fn clear_read_files(session_id: &str) {
    let mut tracker = READ_FILES.lock().unwrap_or_else(|e| e.into_inner());
    tracker.remove(session_id);
}

/// 文件修改状态跟踪（用于检测外部修改）
#[derive(Clone)]
struct FileState {
    mtime: std::time::SystemTime,
    content_hash: u64,
}

static FILE_STATES: Lazy<Mutex<HashMap<String, FileState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn compute_content_hash(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// 标记文件已被读取并记录状态（用于变更检测）
pub fn mark_file_read_with_state(
    session_id: &str,
    file_path: &str,
    content: &str,
    mtime: std::time::SystemTime,
) {
    mark_file_read(session_id, file_path);
    let mut states = FILE_STATES.lock().unwrap_or_else(|e| e.into_inner());
    let key = format!("{}:{}", session_id, file_path);
    states.insert(
        key,
        FileState {
            mtime,
            content_hash: compute_content_hash(content),
        },
    );
}

/// 检查文件是否在读取后被外部修改
pub fn is_file_modified_since_read(
    session_id: &str,
    file_path: &str,
    current_content: &str,
    current_mtime: std::time::SystemTime,
) -> bool {
    let states = FILE_STATES.lock().unwrap_or_else(|e| e.into_inner());
    let key = format!("{}:{}", session_id, file_path);
    if let Some(state) = states.get(&key) {
        // 检查 mtime 是否变化
        if current_mtime != state.mtime {
            return true;
        }
        // 检查内容 hash 是否变化
        if compute_content_hash(current_content) != state.content_hash {
            return true;
        }
    }
    false
}

/// 文件读取工具
pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. \
         Use this to view file contents, source code, configuration files, etc. \
         Returns an error if the file doesn't exist or cannot be read."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read (relative or absolute)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (optional)",
                    "minimum": 1
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-indexed, optional)",
                    "minimum": 1
                }
            },
            "required": ["path"]
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let path = params["path"].as_str().unwrap_or("");
        format!("file_read: {}", path)
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let path_str = params["path"].as_str().unwrap_or("");
        if path_str.is_empty() {
            return ToolResult::error("Path cannot be empty");
        }
        if is_unc_or_network_path(path_str) {
            return ToolResult::error(format!(
                "Refusing to read UNC/network path '{}'. Use a local path instead.",
                path_str
            ));
        }

        let limit = params["limit"].as_u64().map(|u| u as usize);
        let offset = params["offset"].as_u64().and_then(|o| {
            if o == 0 {
                None
            } else {
                Some((o as usize).saturating_sub(1))
            }
        });

        let path = match resolve_path(path_str, &context.working_dir) {
            Ok(path) => path,
            Err(msg) => return ToolResult::error(msg),
        };
        info!("Reading file: {:?}", path);

        // 检查文件是否存在
        if !path.exists() {
            return ToolResult::error(format!("File does not exist: {}", path_str));
        }

        // 检查是否是文件
        if !path.is_file() {
            return ToolResult::error(format!("Path is not a file: {}", path_str));
        }
        if let Err(msg) = check_file_size_limit(&path, "read") {
            return ToolResult::error(msg);
        }

        // 读取文件内容
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to read file: {}", e);
                return ToolResult::error(format!("Failed to read file: {}", e));
            }
        };

        // 文件缓存优化：如果文件在本会话中已读过且未变更，返回短信提示
        if let Some(ref cache) = context.file_cache {
            if cache.is_unchanged_since_last_read(&path) {
                let lines_count = content.lines().count();
                return ToolResult::success_with_data(
                    format!(
                        "[File unchanged since last read: {}] ({} lines)\nIf you need the full content, it was provided in a previous message.",
                        path_str, lines_count
                    ),
                    json!({ "path": path_str, "unchanged": true, "total_lines": lines_count }),
                );
            }
            cache.mark_read(&path);
        }

        // 标记文件已被读取（用于 must-read-before-edit 检查）
        let mtime = std::fs::metadata(&path)
            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        mark_file_read_with_state(&context.session_id, path_str, &content, mtime);

        // 应用 limit 和 offset
        let lines: Vec<&str> = content.lines().collect();
        let start = offset.unwrap_or(0);
        if offset.is_some() && start >= lines.len() {
            return ToolResult::error(format!(
                "Offset {} is beyond end of file ({} lines total)",
                start + 1,
                lines.len()
            ));
        }
        let end = limit
            .map(|l| (start + l).min(lines.len()))
            .unwrap_or(lines.len());

        let selected_lines = if start > 0 || end < lines.len() {
            &lines[start..end]
        } else {
            &lines[..]
        };

        let result = selected_lines.join("\n");

        // 添加行号信息
        let formatted = if lines.len() > 1 {
            selected_lines
                .iter()
                .enumerate()
                .map(|(i, line)| format!("{:4} | {}", start + i + 1, line))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            result.clone()
        };

        let truncated_info = if end < lines.len() || start > 0 {
            format!(
                "\n\n[{} lines total, showing lines {}-{}]",
                lines.len(),
                start + 1,
                end
            )
        } else {
            String::new()
        };

        ToolResult::success_with_data(
            format!("{}{}", formatted, truncated_info),
            json!({
                "path": path_str,
                "total_lines": lines.len(),
                "displayed_lines": selected_lines.len(),
                "size_bytes": content.len()
            }),
        )
    }
}

/// 文件写入工具
pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write content to a file. \
         Creates the file if it doesn't exist, overwrites if it does. \
         Creates parent directories as needed. \
         Use with caution as this will overwrite existing files."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let path = params["path"].as_str().unwrap_or("");
        let content_len = params["content"].as_str().map(|s| s.len()).unwrap_or(0);
        format!("file_write: {} ({} bytes)", path, content_len)
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        // 检查只读模式
        if context.permissions.read_only {
            return ToolResult::error("Cannot write files in read-only mode");
        }

        let path_str = params["path"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");

        if path_str.is_empty() {
            return ToolResult::error("Path cannot be empty");
        }
        if is_unc_or_network_path(path_str) {
            return ToolResult::error(format!(
                "Refusing to write UNC/network path '{}'. Use a local path instead.",
                path_str
            ));
        }
        if content.len() as u64 > MAX_EDITABLE_FILE_SIZE_BYTES {
            return ToolResult::error(format!(
                "Refusing to write content larger than {} bytes",
                MAX_EDITABLE_FILE_SIZE_BYTES
            ));
        }

        let path = match resolve_path(path_str, &context.working_dir) {
            Ok(path) => path,
            Err(msg) => return ToolResult::error(msg),
        };
        info!("Writing file: {:?}", path);

        // 检查父目录是否存在，不存在则创建
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                debug!("Creating parent directories: {:?}", parent);
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    error!("Failed to create parent directories: {}", e);
                    return ToolResult::error(format!("Failed to create directories: {}", e));
                }
            }
        }

        // 创建 checkpoint（文件修改前自动快照）
        let cp_mgr = crate::engine::checkpoint::get_checkpoint_manager(&context.session_id).await;
        {
            let mut cp = cp_mgr.lock().await;
            if let Err(e) = cp.create_checkpoint("file_write", None, None, &[path.clone()]).await {
                warn!("Failed to create checkpoint for file_write: {}", e);
            }
        }

        // 写入文件
        match tokio::fs::write(&path, content).await {
            Ok(_) => {
                // 使文件缓存失效
                if let Some(ref cache) = context.file_cache {
                    cache.invalidate_content(&path);
                    cache.invalidate_metadata(&path);
                }
                info!("Successfully wrote {} bytes to {:?}", content.len(), path);
                ToolResult::success_with_data(
                    format!("File written successfully: {}", path_str),
                    json!({
                        "path": path_str,
                        "bytes_written": content.len()
                    }),
                )
            }
            Err(e) => {
                error!("Failed to write file: {}", e);
                ToolResult::error(format!("Failed to write file: {}", e))
            }
        }
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        let path = params["path"].as_str().unwrap_or("");
        if path.is_empty() {
            return false;
        }
        let input_path = Path::new(path);
        // 对相对路径一律要求确认，避免 cwd 与 tool working_dir 不一致时误判。
        if !input_path.is_absolute() {
            return true;
        }
        // 绝对路径仅在目标已存在时要求确认。
        input_path.exists()
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        params["path"]
            .as_str()
            .map(|p| format!("This will overwrite the existing file: {}\nContinue?", p))
    }
}

/// 文件编辑工具
pub struct FileEditTool;

/// 查找所有精确匹配位置
fn find_occurrences(content: &str, target: &str) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut start = 0;
    while let Some(pos) = content[start..].find(target) {
        let match_start = start + pos;
        let match_end = match_start + target.len();
        result.push((match_start, match_end));
        start = match_end;
    }
    result
}

/// 查找所有模糊匹配位置（去除首尾空白后匹配）
fn fuzzy_find_occurrences(content: &str, target: &str) -> Vec<(usize, usize)> {
    let trimmed_target = target.trim();
    if trimmed_target.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed_line = line.trim();
        if trimmed_line == trimmed_target {
            // 计算在原始内容中的实际起始位置
            let mut pos = 0;
            for _ in 0..line_idx {
                pos = content[pos..]
                    .find('\n')
                    .map(|p| pos + p + 1)
                    .unwrap_or(pos);
            }
            let line_start = pos;
            let line_end = line_start + line.len();
            result.push((line_start, line_end));
        }
    }
    result
}

/// 归一化空白后查找所有精确匹配位置（限制在同一行内扩展）
fn find_occurrences_normalized(content: &str, target: &str) -> Vec<(usize, usize)> {
    let trimmed_target = target.trim();
    if trimmed_target.is_empty() {
        return find_occurrences(content, target);
    }
    let mut result = Vec::new();
    let mut start = 0;
    while start < content.len() {
        if let Some(pos) = content[start..].find(trimmed_target) {
            let match_start = start + pos;
            let match_end = match_start + trimmed_target.len();

            // 向前扩展：限制在当前行内（不跨越 \n）
            let line_start = content[..match_start]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let actual_start = content[line_start..match_start]
                .find(|c: char| !c.is_whitespace())
                .map(|i| line_start + i)
                .unwrap_or(match_start);

            // 向后扩展：限制在当前行内
            let line_end = content[match_end..]
                .find('\n')
                .map(|i| match_end + i)
                .unwrap_or(content.len());
            let actual_end = content[match_end..line_end]
                .rfind(|c: char| !c.is_whitespace())
                .map(|i| match_end + i + 1)
                .unwrap_or(line_end);

            result.push((actual_start, actual_end));
            start = line_end.max(match_end);
        } else {
            break;
        }
    }
    result
}

/// 构建匹配位置的上下文提示
fn build_match_context(
    content: &str,
    occurrences: &[(usize, usize)],
    context_lines: usize,
) -> String {
    let lines: Vec<&str> = content.lines().collect();

    let mut parts = vec![format!("Found {} occurrence(s):", occurrences.len())];
    for (occ_idx, (start, _end)) in occurrences.iter().enumerate() {
        let start_line = content[..*start].matches('\n').count();
        let ctx_start = start_line.saturating_sub(context_lines);
        let ctx_end = (start_line + 1 + context_lines).min(lines.len());
        parts.push(format!(
            "\n  Match #{} at line {}:",
            occ_idx + 1,
            start_line + 1
        ));
        for (li, line) in lines
            .iter()
            .enumerate()
            .skip(ctx_start)
            .take(ctx_end - ctx_start)
        {
            parts.push(format!("    {:4} | {}", li + 1, line));
        }
    }
    parts.join("\n")
}

/// 保存文件快照
#[allow(dead_code)]
async fn save_snapshot(
    path: &Path,
    session_id: &str,
    content: &str,
    tool_name: &str,
) -> Result<PathBuf, String> {
    // 消毒 session_id，防止路径注入
    let safe_session_id: String = session_id
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();

    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S_%3f");
    let snap_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
        .join("snapshots")
        .join(&safe_session_id)
        .join(ts.to_string());

    // 尝试将绝对路径转为相对于 working_dir 的路径，如果失败则使用简化文件名
    let relative = if let Ok(cwd) = std::env::current_dir() {
        path.strip_prefix(&cwd)
            .unwrap_or(path)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "_")
    } else {
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    };

    let snap_path = snap_dir.join(&relative);
    if let Some(parent) = snap_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            return Err(format!("Failed to create snapshot dir: {}", e));
        }
    }
    if let Err(e) = tokio::fs::write(&snap_path, content).await {
        return Err(format!("Failed to write snapshot: {}", e));
    }

    // 保存元数据，记录原文件路径
    let meta_path = snap_dir.join(format!("{}.meta.json", relative));
    let meta = serde_json::json!({
        "original_path": path.to_string_lossy().to_string(),
        "timestamp": ts.to_string(),
    });
    if let Err(e) = tokio::fs::write(&meta_path, meta.to_string()).await {
        return Err(format!("Failed to write snapshot metadata: {}", e));
    }

    // 记录编辑历史到 edits.json
    let edits_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
        .join("snapshots")
        .join(&safe_session_id)
        .join("edits.json");

    let edit_record = serde_json::json!({
        "timestamp": chrono::Local::now().to_rfc3339(),
        "file_path": path.to_string_lossy().to_string(),
        "tool_name": tool_name,
        "snapshot_dir": snap_dir.to_string_lossy().to_string(),
        "snapshot_file": relative,
    });

    let mut edits = if edits_path.exists() {
        tokio::fs::read_to_string(&edits_path)
            .await
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    edits.push(edit_record);

    if let Err(e) = tokio::fs::write(
        &edits_path,
        serde_json::to_string_pretty(&edits).unwrap_or_default(),
    )
    .await
    {
        warn!("Failed to write edits history: {}", e);
    }

    Ok(snap_path)
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing specific text. \
         Finds the old_string and replaces it with new_string. \
         Fails if old_string is not found exactly once (unless expected_replacements is set). \
         Supports insert_after and insert_before for adding new lines. \
         \
         CRITICAL: old_string must match EXACTLY, including all whitespace and indentation. \
         If you are unsure about exact whitespace, use line_start + line_end instead: \
         set line_start and line_end (1-indexed, inclusive) and provide new_string. \
         This replaces the entire line range and is MORE RELIABLE for multi-line edits. \
         \
         Examples:\
         - Exact replace: old_string='    let x = 1;', new_string='    let x = 2;'\
         - Line range: line_start=5, line_end=10, new_string='new content here'\
         - Insert after: insert_after='fn main() {', new_string='    println!(\"hello\");'"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace old_string with"
                },
                "expected_replacements": {
                    "type": "integer",
                    "description": "How many times old_string should appear. Defaults to 1. Set to null or omit to replace all occurrences.",
                    "minimum": 1
                },
                "insert_after": {
                    "type": "string",
                    "description": "If provided, new_string will be inserted after each occurrence of this string (old_string is ignored when this is set)"
                },
                "insert_before": {
                    "type": "string",
                    "description": "If provided, new_string will be inserted before each occurrence of this string (old_string is ignored when this is set)"
                },
                "line_start": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "If provided, replaces lines line_start..=line_end with new_string (old_string is ignored). 1-indexed."
                },
                "line_end": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "End line number for line-range replacement (inclusive). Must be paired with line_start."
                },
                "normalize_whitespace": {
                    "type": "boolean",
                    "default": false,
                    "description": "If true, ignores leading/trailing whitespace differences when matching old_string."
                }
            },
            "required": ["path", "new_string"]
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let path = params["path"].as_str().unwrap_or("");
        let has_old = params["old_string"].as_str().is_some();
        let has_lines = params["line_start"].as_u64().is_some();
        let mode = if has_old {
            "exact"
        } else if has_lines {
            "line_range"
        } else {
            "insert"
        };
        format!("file_edit: {} ({})", path, mode)
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        // 检查只读模式
        if context.permissions.read_only {
            return ToolResult::error("Cannot edit files in read-only mode");
        }

        let path_str = params["path"].as_str().unwrap_or("");
        let old_string = params["old_string"].as_str().unwrap_or("");
        let new_string = params["new_string"].as_str().unwrap_or("");
        let insert_after = params["insert_after"].as_str();
        let insert_before = params["insert_before"].as_str();
        let expected_replacements = params["expected_replacements"].as_u64().map(|n| n as usize);
        let line_start = params["line_start"].as_u64().map(|n| n as usize);
        let line_end = params["line_end"].as_u64().map(|n| n as usize);
        let normalize_ws = params["normalize_whitespace"].as_bool().unwrap_or(false);

        if path_str.is_empty() {
            return ToolResult::error("Path cannot be empty");
        }
        if is_unc_or_network_path(path_str) {
            return ToolResult::error(format!(
                "Refusing to edit UNC/network path '{}'. Use a local path instead.",
                path_str
            ));
        }

        let path = match resolve_path(path_str, &context.working_dir) {
            Ok(path) => path,
            Err(msg) => return ToolResult::error(msg),
        };
        info!("Editing file: {:?}", path);

        // 读取文件内容
        if let Err(msg) = check_file_size_limit(&path, "edit") {
            return ToolResult::error(msg);
        }
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(e) => {
                return ToolResult::error(format!("Failed to read file: {}", e));
            }
        };

        // ── Smart Edit 检查 ───────────────────────────────────────────
        // 1. Must-read-before-edit: 检查文件是否已被读取
        // 仅在 PRIORITY_AGENT_SMART_EDIT=1 时启用此检查
        if std::env::var("PRIORITY_AGENT_SMART_EDIT")
            .as_ref()
            .map(|v| v.as_str())
            == Ok("1")
            && !is_file_read(&context.session_id, path_str)
        {
            return ToolResult::error(
                format!(
                    "File '{}' has not been read yet. You must read a file before editing it. Use file_read tool first.",
                    path_str
                )
            );
        }

        // 2. 文件修改检测：检查文件是否在读取后被外部修改
        let current_mtime = std::fs::metadata(&path)
            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if is_file_modified_since_read(&context.session_id, path_str, &content, current_mtime) {
            warn!("File '{}' was modified since it was read", path_str);
            // 注意：我们仍然允许编辑，只是记录警告
        }

        // 2. 对 old_string 和 new_string 应用 desanitize 和 quote normalization（仅在 PRIORITY_AGENT_SMART_EDIT=1 时）
        let (old_string, new_string) = if std::env::var("PRIORITY_AGENT_SMART_EDIT")
            .as_ref()
            .map(|v| v.as_str())
            == Ok("1")
        {
            (
                desanitize(&normalize_quotes(old_string)),
                desanitize(&normalize_quotes(new_string)),
            )
        } else {
            (old_string.to_string(), new_string.to_string())
        };

        // 创建 checkpoint（文件修改前自动快照）
        let cp_mgr = crate::engine::checkpoint::get_checkpoint_manager(&context.session_id).await;
        {
            let mut cp = cp_mgr.lock().await;
            if let Err(e) = cp.create_checkpoint("file_edit", None, None, &[path.clone()]).await {
                warn!("Failed to create checkpoint for file_edit: {}", e);
            }
        }

        // 确定操作模式
        let result = if let (Some(start), Some(end)) = (line_start, line_end) {
            Self::do_replace_lines(content, start, end, &new_string)
        } else if let Some(after) = insert_after {
            Self::do_insert(content, after, &new_string, InsertMode::After)
        } else if let Some(before) = insert_before {
            Self::do_insert(content, before, &new_string, InsertMode::Before)
        } else {
            if old_string.is_empty() {
                return ToolResult::error(
                    "old_string cannot be empty unless insert_after or insert_before is used"
                        .to_string(),
                );
            }
            Self::do_replace(
                content,
                &old_string,
                &new_string,
                expected_replacements,
                normalize_ws,
            )
        };

        match result {
            Ok((new_content, replacements)) => {
                match tokio::fs::write(&path, &new_content).await {
                    Ok(_) => {
                        // 使文件缓存失效
                        if let Some(ref cache) = context.file_cache {
                            cache.invalidate_content(&path);
                            cache.invalidate_metadata(&path);
                        }
                        info!("Successfully edited file: {:?}", path);
                        let data = json!({
                            "path": path_str,
                            "replacements": replacements,
                        });
                        ToolResult::success_with_data(
                            format!(
                                "File edited successfully: {} ({} replacement(s))",
                                path_str, replacements
                            ),
                            data,
                        )
                    }
                    Err(e) => {
                        error!("Failed to write file: {}", e);
                        ToolResult::error(format!("Failed to write file: {}", e))
                    }
                }
            }
            Err(e) => ToolResult::error(e),
        }
    }

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        true // 编辑文件总是需要确认
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        let path = params["path"].as_str().unwrap_or("unknown file");
        Some(format!("This will edit the file: {}\nContinue?", path))
    }
}

pub enum InsertMode {
    After,
    Before,
}

impl FileEditTool {
    /// 预览编辑结果（不写入磁盘）
    pub fn preview_edit(params: &serde_json::Value, original: &str) -> Result<String, String> {
        let old_string = params["old_string"].as_str().unwrap_or("");
        let new_string = params["new_string"].as_str().unwrap_or("");
        let insert_after = params["insert_after"].as_str();
        let insert_before = params["insert_before"].as_str();
        let expected_replacements = params["expected_replacements"].as_u64().map(|n| n as usize);
        let line_start = params["line_start"].as_u64().map(|n| n as usize);
        let line_end = params["line_end"].as_u64().map(|n| n as usize);
        let normalize_ws = params["normalize_whitespace"].as_bool().unwrap_or(false);

        if let (Some(start), Some(end)) = (line_start, line_end) {
            Self::do_replace_lines(original.to_string(), start, end, new_string).map(|(s, _)| s)
        } else if let Some(after) = insert_after {
            Self::do_insert(original.to_string(), after, new_string, InsertMode::After)
                .map(|(s, _)| s)
        } else if let Some(before) = insert_before {
            Self::do_insert(original.to_string(), before, new_string, InsertMode::Before)
                .map(|(s, _)| s)
        } else {
            if old_string.is_empty() {
                return Err(
                    "old_string cannot be empty unless insert_after or insert_before is used"
                        .to_string(),
                );
            }
            Self::do_replace(
                original.to_string(),
                old_string,
                new_string,
                expected_replacements,
                normalize_ws,
            )
            .map(|(s, _)| s)
        }
    }

    pub fn do_replace(
        content: String,
        old_string: &str,
        new_string: &str,
        expected: Option<usize>,
        normalize_whitespace: bool,
    ) -> Result<(String, usize), String> {
        let occurrences = if normalize_whitespace {
            find_occurrences_normalized(&content, old_string)
        } else {
            find_occurrences(&content, old_string)
        };

        if occurrences.is_empty() {
            // 尝试模糊匹配
            let fuzzy = fuzzy_find_occurrences(&content, old_string);
            if fuzzy.is_empty() {
                return Err(
                    "Could not find old_string in file. Make sure it matches exactly (including whitespace)."
                        .to_string(),
                );
            }
            // 如果模糊匹配有结果，但不符合预期，也返回详细信息
            let ctx = build_match_context(&content, &fuzzy, 2);
            return Err(format!(
                "old_string not found exactly, but fuzzy matches found:\n{}\n\nPlease adjust old_string to match one of these occurrences precisely.",
                ctx
            ));
        }

        let count = occurrences.len();
        let expected_count = expected.unwrap_or(1);

        if count != expected_count {
            let ctx = build_match_context(&content, &occurrences, 2);
            return Err(format!(
                "Expected {} occurrence(s) of old_string, but found {}.\n{}\n\nPlease provide a more specific old_string or set expected_replacements to {}.",
                expected_count, count, ctx, count
            ));
        }

        // 从后往前替换，避免位置偏移问题
        let mut new_content = content;
        for (start, end) in occurrences.into_iter().rev() {
            new_content.replace_range(start..end, new_string);
        }
        Ok((new_content, count))
    }

    pub fn do_insert(
        content: String,
        anchor: &str,
        new_string: &str,
        mode: InsertMode,
    ) -> Result<(String, usize), String> {
        let occurrences = find_occurrences(&content, anchor);
        if occurrences.is_empty() {
            return Err(format!(
                "Could not find anchor '{}' in file for insertion.",
                anchor
            ));
        }
        let count = occurrences.len();
        let mut new_content = content;
        for (start, end) in occurrences.into_iter().rev() {
            match mode {
                InsertMode::After => {
                    new_content.insert_str(end, new_string);
                }
                InsertMode::Before => {
                    new_content.insert_str(start, new_string);
                }
            }
        }
        Ok((new_content, count))
    }

    /// 按行号范围替换内容（1-indexed，包含两端）
    pub fn do_replace_lines(
        content: String,
        line_start: usize,
        line_end: usize,
        new_string: &str,
    ) -> Result<(String, usize), String> {
        if line_start == 0 || line_end == 0 {
            return Err("line_start and line_end must be >= 1".to_string());
        }
        if line_start > line_end {
            return Err(format!(
                "line_start ({}) cannot be greater than line_end ({})",
                line_start, line_end
            ));
        }
        let lines: Vec<&str> = content.lines().collect();
        if line_start > lines.len() {
            return Err(format!(
                "line_start ({}) is beyond end of file ({} lines total)",
                line_start,
                lines.len()
            ));
        }
        let start_idx = line_start - 1;
        let end_idx = (line_end - 1).min(lines.len() - 1);
        let mut new_lines = lines[..start_idx].to_vec();
        new_lines.push(new_string);
        new_lines.extend_from_slice(&lines[end_idx + 1..]);
        let mut new_content = new_lines.join("\n");
        // 如果原始内容以换行符结尾，保留末尾换行符
        if content.ends_with('\n') {
            new_content.push('\n');
        }
        let replaced_count = end_idx - start_idx + 1;
        Ok((new_content, replaced_count))
    }
}

/// 解析路径（支持相对路径和绝对路径），带路径穿越保护
pub fn resolve_path(
    path: &str,
    working_dir: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let input = Path::new(path);
    let normalized_working_dir = normalize_path(working_dir);

    let candidate = if input.is_absolute() {
        normalize_path(input)
    } else {
        normalize_path(&normalized_working_dir.join(input))
    };

    if input.is_absolute() {
        if !is_allowed_absolute_path(&candidate, &normalized_working_dir) {
            return Err(format!(
                "Access denied: absolute path '{}' is outside allowed roots",
                path
            ));
        }
    } else if !candidate.starts_with(&normalized_working_dir) {
        return Err(format!(
            "Access denied: path '{}' escapes working directory",
            path
        ));
    }

    // working_dir 不存在时无法进行可靠的 realpath 比较，保留词法边界检查结果。
    if !normalized_working_dir.exists() {
        return Ok(candidate);
    }

    // 第二层防护：解析已存在祖先的真实路径，阻止通过 symlink 逃逸目录边界。
    let real_candidate = realpath_deepest_existing(&candidate)?;
    let real_working_dir = canonicalize_or_normalize(&normalized_working_dir);

    if input.is_absolute() {
        if !is_allowed_absolute_path(&real_candidate, &real_working_dir) {
            return Err(format!(
                "Access denied: absolute path '{}' resolves outside allowed roots",
                path
            ));
        }
    } else if !real_candidate.starts_with(&real_working_dir) {
        return Err(format!(
            "Access denied: path '{}' escapes working directory via symlink",
            path
        ));
    }

    Ok(candidate)
}

pub fn is_allowed_absolute_path(path: &std::path::Path, working_dir: &std::path::Path) -> bool {
    let normalized_path = normalize_path(path);
    let normalized_working = normalize_path(working_dir);

    if normalized_path.starts_with(&normalized_working) {
        return true;
    }

    // 如果 working_dir 在 /tmp 下，只允许访问 working_dir 内的路径
    // 防止 /tmp/foo 工作目录下访问 /tmp/bar
    let tmp_dir = normalize_path(&std::env::temp_dir());
    let in_tmp = normalized_working.starts_with(&tmp_dir)
        || normalized_working.starts_with(Path::new("/tmp"))
        || normalized_working.starts_with(Path::new("/var/tmp"));
    if in_tmp {
        return false;
    }

    // working_dir 不在 /tmp 下时，允许访问 /tmp 下的项目临时文件
    let allowed_roots = [
        normalize_path(Path::new("/tmp")),
        normalize_path(Path::new("/var/tmp")),
        tmp_dir,
    ];
    let canonical_path = canonicalize_or_normalize(&normalized_path);
    allowed_roots
        .into_iter()
        .any(|root| normalized_path.starts_with(&root) || canonical_path.starts_with(&root))
}

pub fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut normalized = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let _ = normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

pub fn canonicalize_or_normalize(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(p) => normalize_path(&p),
        Err(_) => normalize_path(path),
    }
}

fn realpath_deepest_existing(path: &Path) -> Result<PathBuf, String> {
    let mut current = path.to_path_buf();
    let mut deepest_existing: Option<PathBuf> = None;

    loop {
        if std::fs::symlink_metadata(&current).is_ok() {
            deepest_existing = Some(current.clone());
            break;
        }
        if !current.pop() {
            break;
        }
    }

    let deepest_existing = deepest_existing
        .ok_or_else(|| format!("Access denied: cannot resolve path '{}'", path.display()))?;

    let real_base = std::fs::canonicalize(&deepest_existing).map_err(|e| {
        format!(
            "Access denied: failed to resolve symlink for '{}': {}",
            path.display(),
            e
        )
    })?;

    let suffix = path
        .strip_prefix(&deepest_existing)
        .map_err(|_| format!("Access denied: invalid path '{}'", path.display()))?;

    Ok(normalize_path(&real_base.join(suffix)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_read() {
        let tool = FileReadTool;
        // 使用 Cargo.toml 作为测试文件
        let params = json!({
            "path": "Cargo.toml"
        });
        let context = ToolContext::new(".", "test-session");

        let result = tool.execute(params, context).await;

        assert!(result.success);
        assert!(result.content.contains("[package]"));
    }

    #[tokio::test]
    async fn test_file_write_and_read() {
        let write_tool = FileWriteTool;
        let read_tool = FileReadTool;

        let test_content = "Hello, World!";
        let params = json!({
            "path": "/tmp/test_priority_agent_file.txt",
            "content": test_content
        });
        let context = ToolContext::new(".", "test-session");

        // 写入
        let write_result = write_tool.execute(params, context.clone()).await;
        assert!(write_result.success);

        // 读取
        let read_params = json!({
            "path": "/tmp/test_priority_agent_file.txt"
        });
        let read_result = read_tool.execute(read_params, context).await;
        assert!(read_result.success);
        assert!(read_result.content.contains("Hello, World!"));

        // 清理
        let _ = tokio::fs::remove_file("/tmp/test_priority_agent_file.txt").await;
    }

    #[test]
    fn test_resolve_path() {
        let working_dir = std::path::Path::new("/home/user/project");

        let denied = resolve_path("/etc/config", working_dir);
        assert!(denied.is_err());

        let relative = resolve_path("src/main.rs", working_dir).unwrap();
        assert_eq!(
            relative,
            std::path::Path::new("/home/user/project/src/main.rs")
        );

        let escaped = resolve_path("../secret.txt", working_dir);
        assert!(escaped.is_err());

        let allowed_tmp = resolve_path("/tmp/test_priority_agent_file.txt", working_dir).unwrap();
        assert_eq!(
            allowed_tmp,
            std::path::Path::new("/tmp/test_priority_agent_file.txt")
        );
    }

    #[test]
    fn test_file_write_requires_confirmation_for_relative_path() {
        let tool = FileWriteTool;
        let params = json!({
            "path": "relative.txt",
            "content": "hello"
        });
        assert!(tool.requires_confirmation(&params));
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_path_blocks_symlink_escape() {
        use std::os::unix::fs::symlink;

        let base = tempfile::tempdir().unwrap();
        let working = base.path().join("workspace");
        let outside = base.path().join("outside");
        std::fs::create_dir_all(&working).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        symlink(&outside, working.join("link")).unwrap();

        let escaped = resolve_path("link/secret.txt", &working);
        assert!(escaped.is_err());
    }

    #[tokio::test]
    async fn test_file_read_offset_out_of_bounds() {
        let read_tool = FileReadTool;
        let path = "/tmp/test_priority_agent_offset.txt";
        tokio::fs::write(path, "line1\nline2\n").await.unwrap();

        let params = json!({
            "path": path,
            "offset": 100
        });
        let context = ToolContext::new(".", "test-session");
        let result = read_tool.execute(params, context).await;
        assert!(!result.success);
        assert!(result.error.unwrap_or_default().contains("Offset"));

        let _ = tokio::fs::remove_file(path).await;
    }

    // ===== FileEditTool 增强测试 =====

    #[tokio::test]
    async fn test_file_edit_success() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_success.txt";
        tokio::fs::write(path, "hello world\nfoo bar\n")
            .await
            .unwrap();

        let params = json!({
            "path": path,
            "old_string": "foo bar",
            "new_string": "baz qux"
        });
        let context = ToolContext::new(".", "test-session-edit-success");
        let result = tool.execute(params, context).await;

        assert!(result.success, "edit failed: {:?}", result.error);
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert!(content.contains("baz qux"));
        assert!(!content.contains("foo bar"));

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_multiple_occurrences_error() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_multi.txt";
        tokio::fs::write(path, "aaa\naaa\naaa\n").await.unwrap();

        let params = json!({
            "path": path,
            "old_string": "aaa",
            "new_string": "bbb"
        });
        let context = ToolContext::new(".", "test-session-edit-multi");
        let result = tool.execute(params, context).await;

        assert!(!result.success);
        let err = result.error.unwrap_or_default();
        assert!(err.contains("Expected 1 occurrence"));
        assert!(err.contains("but found 3"));

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_expected_replacements() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_expected.txt";
        tokio::fs::write(path, "aaa\naaa\n").await.unwrap();

        let params = json!({
            "path": path,
            "old_string": "aaa",
            "new_string": "bbb",
            "expected_replacements": 2
        });
        let context = ToolContext::new(".", "test-session-edit-expected");
        let result = tool.execute(params, context).await;

        assert!(result.success, "edit failed: {:?}", result.error);
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert_eq!(content.matches("bbb").count(), 2);

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_fuzzy_match_hint() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_fuzzy.txt";
        tokio::fs::write(path, "    hello world\n").await.unwrap();

        // 提交带有额外空格的 old_string，精确匹配失败但模糊匹配成功
        let params = json!({
            "path": path,
            "old_string": "  hello world  ",
            "new_string": "hi world"
        });
        let context = ToolContext::new(".", "test-session-edit-fuzzy");
        let result = tool.execute(params, context).await;

        assert!(!result.success);
        let err = result.error.unwrap_or_default();
        assert!(err.contains("fuzzy matches found"));

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_insert_after() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_insert_after.txt";
        tokio::fs::write(path, "line1\nline2\n").await.unwrap();

        let params = json!({
            "path": path,
            "insert_after": "line1",
            "new_string": "\nline1.5"
        });
        let context = ToolContext::new(".", "test-session-edit-insert");
        let result = tool.execute(params, context).await;

        assert!(result.success, "insert failed: {:?}", result.error);
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert!(content.contains("line1\nline1.5\nline2"));

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_insert_before() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_insert_before.txt";
        tokio::fs::write(path, "line1\nline2\n").await.unwrap();

        let params = json!({
            "path": path,
            "insert_before": "line2",
            "new_string": "line1.5\n"
        });
        let context = ToolContext::new(".", "test-session-edit-insert-before");
        let result = tool.execute(params, context).await;

        assert!(result.success, "insert failed: {:?}", result.error);
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert!(content.contains("line1\nline1.5\nline2"));

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_checkpoint_created() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_checkpoint.txt";
        let original = "original content\n";
        tokio::fs::write(path, original).await.unwrap();

        let params = json!({
            "path": path,
            "old_string": "original",
            "new_string": "modified"
        });
        let session_id = "test-session-checkpoint";
        let context = ToolContext::new(".", session_id);
        let result = tool.execute(params, context).await;

        assert!(result.success, "edit failed: {:?}", result.error);

        // 验证 checkpoint 被创建
        let mgr = crate::engine::checkpoint::get_checkpoint_manager(session_id).await;
        let cp = mgr.lock().await;
        let checkpoints = cp.list_checkpoints();
        assert!(!checkpoints.is_empty(), "checkpoint should be created");

        let latest = checkpoints.last().unwrap();
        assert_eq!(latest.tool_name, "file_edit");
        assert_eq!(latest.file_backups.len(), 1);
        assert_eq!(latest.file_backups[0].original_path, path);
        assert!(latest.file_backups[0].existed_before);

        // 验证可以恢复
        let restore_result = cp.restore_checkpoint(&latest.id).await.unwrap();
        assert_eq!(restore_result.restored_files.len(), 1);
        let restored_content = tokio::fs::read_to_string(path).await.unwrap();
        assert_eq!(restored_content, original);

        let _ = tokio::fs::remove_file(path).await;
        let _ = tokio::fs::remove_dir_all(
            dirs::home_dir().unwrap().join(".priority-agent").join("checkpoints").join(format!("session-{}", session_id))
        ).await;
    }

    #[tokio::test]
    async fn test_file_edit_line_range() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_lines.txt";
        tokio::fs::write(path, "line1\nline2\nline3\nline4\n")
            .await
            .unwrap();

        let params = json!({
            "path": path,
            "line_start": 2,
            "line_end": 3,
            "new_string": "REPLACED"
        });
        let context = ToolContext::new(".", "test-session-edit-lines");
        let result = tool.execute(params, context).await;

        assert!(result.success, "line edit failed: {:?}", result.error);
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert_eq!(content, "line1\nREPLACED\nline4\n");

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_normalize_whitespace() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_normws.txt";
        tokio::fs::write(path, "    hello world    \n")
            .await
            .unwrap();

        // old_string 有额外空白，但 normalize_whitespace=true 应能匹配
        let params = json!({
            "path": path,
            "old_string": "hello world",
            "new_string": "hi world",
            "normalize_whitespace": true
        });
        let context = ToolContext::new(".", "test-session-edit-normws");
        let result = tool.execute(params, context).await;

        assert!(result.success, "normalize edit failed: {:?}", result.error);
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert!(content.contains("hi world"));
        assert!(!content.contains("hello world"));

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_line_range_out_of_bounds() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_lines_oob.txt";
        tokio::fs::write(path, "line1\n").await.unwrap();

        let params = json!({
            "path": path,
            "line_start": 5,
            "line_end": 6,
            "new_string": "REPLACED"
        });
        let context = ToolContext::new(".", "test-session-edit-lines-oob");
        let result = tool.execute(params, context).await;

        assert!(!result.success);
        let err = result.error.unwrap_or_default();
        assert!(err.contains("beyond end of file"));

        let _ = tokio::fs::remove_file(path).await;
    }

    #[test]
    fn test_find_occurrences_normalized() {
        let content = "  hello world  \n    hello world    \n";
        let target = "hello world";
        let occ = find_occurrences_normalized(content, target);
        assert_eq!(occ.len(), 2);
        // 第一个匹配应包含前导空格
        assert_eq!(occ[0], (2, 15)); // "  hello world  "
        assert_eq!(occ[1], (20, 35)); // "    hello world    "
    }
}
