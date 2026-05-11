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
const DEFAULT_MAX_FILE_EDIT_REPLACEMENTS: usize = 20;
const MAX_MATCH_CONTEXT_OCCURRENCES: usize = 12;
const DEFAULT_DIRECTORY_READ_ENTRY_LIMIT: usize = 200;

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

fn max_file_edit_replacements() -> usize {
    std::env::var("PRIORITY_AGENT_MAX_FILE_EDIT_REPLACEMENTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_FILE_EDIT_REPLACEMENTS)
}

fn allow_bulk_code_edit() -> bool {
    std::env::var("PRIORITY_AGENT_ALLOW_BULK_CODE_EDIT").as_deref() == Ok("1")
}

fn is_code_like_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext,
                "rs" | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "py"
                    | "go"
                    | "java"
                    | "kt"
                    | "swift"
                    | "c"
                    | "cc"
                    | "cpp"
                    | "h"
                    | "hpp"
                    | "cs"
                    | "rb"
                    | "php"
                    | "scala"
                    | "sh"
                    | "zsh"
                    | "fish"
            )
        })
        .unwrap_or(false)
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
    let session_files = tracker.entry(session_id.to_string()).or_default();
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
    let mut states = FILE_STATES.lock().unwrap_or_else(|e| e.into_inner());
    let prefix = format!("{}:", session_id);
    states.retain(|key, _| !key.starts_with(&prefix));
}

/// 文件修改状态跟踪（用于检测外部修改）
#[derive(Clone)]
struct FileState {
    mtime: std::time::SystemTime,
    content_hash: u64,
}

static FILE_STATES: Lazy<Mutex<HashMap<String, FileState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

async fn read_directory_result(
    path: &Path,
    requested_path: &str,
    limit: Option<usize>,
    offset: Option<usize>,
) -> ToolResult {
    let mut reader = match tokio::fs::read_dir(path).await {
        Ok(reader) => reader,
        Err(e) => {
            error!("Failed to read directory: {}", e);
            return ToolResult::error(format!("Failed to read directory: {}", e));
        }
    };

    let mut entries = Vec::new();
    loop {
        match reader.next_entry().await {
            Ok(Some(entry)) => {
                let name = entry.file_name().to_string_lossy().to_string();
                let entry_path = entry.path();
                let file_type = entry.file_type().await.ok();
                let is_dir = file_type
                    .as_ref()
                    .map(|kind| kind.is_dir())
                    .unwrap_or_else(|| entry_path.is_dir());
                let is_file = file_type
                    .as_ref()
                    .map(|kind| kind.is_file())
                    .unwrap_or_else(|| entry_path.is_file());
                let is_symlink = file_type
                    .as_ref()
                    .map(|kind| kind.is_symlink())
                    .unwrap_or(false);
                let display_name = if is_dir {
                    format!("{name}/")
                } else {
                    name.clone()
                };
                entries.push((display_name, name, entry_path, is_dir, is_file, is_symlink));
            }
            Ok(None) => break,
            Err(e) => {
                error!("Failed to read directory entry: {}", e);
                return ToolResult::error(format!("Failed to read directory entry: {}", e));
            }
        }
    }

    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let total_entries = entries.len();
    let start = offset.unwrap_or(0);
    if start >= total_entries && total_entries > 0 {
        return ToolResult::error(format!(
            "Offset {} is beyond end of directory ({} entries total)",
            start + 1,
            total_entries
        ));
    }

    let entry_limit = limit.unwrap_or(DEFAULT_DIRECTORY_READ_ENTRY_LIMIT);
    let end = (start + entry_limit).min(total_entries);
    let selected = if total_entries == 0 {
        &entries[..0]
    } else {
        &entries[start..end]
    };
    let truncated = end < total_entries || start > 0;
    let mut lines = selected
        .iter()
        .map(|entry| entry.0.clone())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push("(empty)".to_string());
    }

    let mut content = format!(
        "Directory: {}\nEntries ({}):\n{}",
        path.display(),
        total_entries,
        lines.join("\n")
    );
    if truncated {
        content.push_str(&format!(
            "\n\n[{} entries total, showing entries {}-{}]",
            total_entries,
            start + 1,
            end
        ));
    }

    ToolResult::success_with_data(
        content,
        json!({
            "path": requested_path,
            "resolved_path": path.to_string_lossy().to_string(),
            "kind": "directory",
            "entry_count": total_entries,
            "displayed_entries": selected.len(),
            "truncated": truncated,
            "entries": selected.iter().map(|entry| {
                json!({
                    "name": entry.1,
                    "display_name": entry.0,
                    "path": entry.2.to_string_lossy().to_string(),
                    "is_dir": entry.3,
                    "is_file": entry.4,
                    "is_symlink": entry.5,
                })
            }).collect::<Vec<_>>(),
        }),
    )
}

fn compute_content_hash(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn file_state_key(path: &Path) -> String {
    canonicalize_or_normalize(path)
        .to_string_lossy()
        .to_string()
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
        "Read the contents of a file or list a directory. \
         Use this to view file contents, source code, configuration files, and directory entries. \
         For directories, returns entry names only, with trailing '/' for subdirectories. \
         Returns an error if the file doesn't exist or cannot be read."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file or directory to read (relative or absolute; supports ~/Desktop for user-approved desktop inspection)"
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

        let path = match resolve_read_path(path_str, &context.working_dir) {
            Ok(path) => path,
            Err(msg) => return ToolResult::error(msg),
        };
        info!("Reading file: {:?}", path);

        // 检查文件是否存在
        if !path.exists() {
            return ToolResult::error(format!("File does not exist: {}", path_str));
        }

        if path.is_dir() {
            return read_directory_result(&path, path_str, limit, offset).await;
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

        let targeted_read = limit.is_some() || offset.is_some();

        // 文件缓存优化：如果文件在本会话中已读过且未变更，返回短信提示。
        // 但 offset/limit 读取是新的局部证据，不能被上一次全文读取短路掉。
        if let Some(ref cache) = context.file_cache {
            if !targeted_read && cache.is_unchanged_since_last_read(&path) {
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
        let state_key = file_state_key(&path);
        mark_file_read_with_state(&context.session_id, &state_key, &content, mtime);

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
         Best for new files or intentional full-file replacement. \
         Use file_edit for targeted changes to existing files. \
         Creates parent directories as needed and replaces the entire file when it already exists."
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

        let existed_before = path.exists();

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
            if let Err(e) = cp
                .create_checkpoint("file_write", None, None, std::slice::from_ref(&path))
                .await
            {
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
                let action = if existed_before {
                    "overwritten"
                } else {
                    "written"
                };
                ToolResult::success_with_data(
                    format!("File {} successfully: {}", action, path_str),
                    json!({
                        "path": path_str,
                        "bytes_written": content.len(),
                        "existed_before": existed_before,
                        "guidance": if existed_before {
                            "file_write replaced the entire file; use file_edit for targeted existing-file changes"
                        } else {
                            "file_write created a new file"
                        }
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
    for (occ_idx, (start, _end)) in occurrences
        .iter()
        .take(MAX_MATCH_CONTEXT_OCCURRENCES)
        .enumerate()
    {
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
    if occurrences.len() > MAX_MATCH_CONTEXT_OCCURRENCES {
        parts.push(format!(
            "\n  ... showing first {} of {} matches. The old_string is too broad; use a unique old_string copied from the target lines or a precise line_start/line_end replacement.",
            MAX_MATCH_CONTEXT_OCCURRENCES,
            occurrences.len()
        ));
    }
    parts.join("\n")
}

fn contains_file_read_line_prefix(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        let Some((digits, rest)) = trimmed.split_once('|') else {
            return false;
        };
        !digits.trim().is_empty()
            && digits.trim().chars().all(|ch| ch.is_ascii_digit())
            && rest.starts_with(' ')
    })
}

fn file_read_line_prefix_guidance(field: &str) -> String {
    format!(
        "{field} appears to include file_read display line prefixes like `12 |`. \
         Those prefixes are not part of the file content. Retry with text copied after the pipe, \
         or use line_start/line_end when the line numbers are the evidence you trust."
    )
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
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
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
         Use after reading the target file. \
         Finds the old_string and replaces it with new_string. \
         Fails if old_string is not found exactly once (unless expected_replacements is set). \
         Supports insert_after and insert_before for adding new lines. \
         \
         CRITICAL: old_string must match EXACTLY, including all whitespace and indentation. \
         Do not include file_read display prefixes such as `12 |`; those are not file content. \
         If you are unsure about exact whitespace, use line_start + line_end instead: \
         set line_start and line_end (1-indexed, inclusive) and provide new_string. \
         This replaces the entire line range and is more reliable for multi-line edits."
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
                    "description": "How many times old_string should appear. Defaults to 1. Use values greater than 1 only for deliberate mass replacements.",
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
                },
                "allow_stale_read": {
                    "type": "boolean",
                    "default": false,
                    "description": "Allow editing even when the file changed since this session last read it. Use only for intentional overwrites."
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
        let allow_stale_read = params["allow_stale_read"].as_bool().unwrap_or(false);

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
        let state_key = file_state_key(&path);

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
            && !is_file_read(&context.session_id, &state_key)
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
        if is_file_modified_since_read(&context.session_id, &state_key, &content, current_mtime) {
            if !allow_stale_read {
                return ToolResult::error(format!(
                    "Refusing file_edit for '{}': file changed since this session last read it. Re-read the file and retry, or set allow_stale_read=true if this overwrite is intentional.",
                    path_str
                ));
            }
            warn!(
                "File '{}' was modified since it was read; continuing because allow_stale_read=true",
                path_str
            );
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
            if let Err(e) = cp
                .create_checkpoint("file_edit", None, None, std::slice::from_ref(&path))
                .await
            {
                warn!("Failed to create checkpoint for file_edit: {}", e);
            }
        }

        // 确定操作模式
        let using_exact_replace = line_start.is_none()
            && line_end.is_none()
            && insert_after.is_none()
            && insert_before.is_none();
        let result = if let (Some(start), Some(end)) = (line_start, line_end) {
            Self::do_replace_lines(content, start, end, &new_string)
        } else if let Some(after) = insert_after {
            Self::do_insert(content, after, &new_string, InsertMode::After)
        } else if let Some(before) = insert_before {
            Self::do_insert(content, before, &new_string, InsertMode::Before)
        } else {
            if old_string.trim().is_empty() {
                return ToolResult::error(
                    "old_string cannot be empty or whitespace-only unless insert_after, insert_before, or line_start/line_end is used. For a known target line, use line_start and line_end instead."
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
                if using_exact_replace
                    && replacements > 1
                    && is_code_like_path(&path)
                    && !allow_bulk_code_edit()
                {
                    return ToolResult::error(format!(
                        "Refusing multi-occurrence file_edit on code file '{}' ({} replacement(s)). Use a unique old_string, line_start/line_end, or set PRIORITY_AGENT_ALLOW_BULK_CODE_EDIT=1 for an intentional bulk code edit.",
                        path_str, replacements
                    ));
                }
                match tokio::fs::write(&path, &new_content).await {
                    Ok(_) => {
                        // 使文件缓存失效
                        if let Some(ref cache) = context.file_cache {
                            cache.invalidate_content(&path);
                            cache.invalidate_metadata(&path);
                        }
                        let new_mtime = std::fs::metadata(&path)
                            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        mark_file_read_with_state(
                            &context.session_id,
                            &state_key,
                            &new_content,
                            new_mtime,
                        );
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
            if old_string.trim().is_empty() {
                return Err(
                    "old_string cannot be empty or whitespace-only unless insert_after, insert_before, or line_start/line_end is used. For a known target line, use line_start and line_end instead."
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

        if old_string.trim().is_empty() {
            return Err(
                "old_string cannot be empty or whitespace-only unless insert_after, insert_before, or line_start/line_end is used. For a known target line, use line_start and line_end instead."
                    .to_string(),
            );
        }

        if occurrences.is_empty() {
            if contains_file_read_line_prefix(old_string) {
                return Err(file_read_line_prefix_guidance("old_string"));
            }
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
        let max_replacements = max_file_edit_replacements();

        if expected_count > max_replacements {
            return Err(format!(
                "Refusing file_edit with {} replacement(s): exceeds safety limit {}. Use narrower anchors or set PRIORITY_AGENT_MAX_FILE_EDIT_REPLACEMENTS explicitly for deliberate bulk edits.",
                expected_count, max_replacements
            ));
        }

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
        if contains_file_read_line_prefix(anchor) {
            let field = match mode {
                InsertMode::After => "insert_after",
                InsertMode::Before => "insert_before",
            };
            return Err(file_read_line_prefix_guidance(field));
        }
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
    resolve_path_with_policy(path, working_dir, false)
}

/// 解析只读路径。相对路径仍限制在工作区内；绝对路径除工作区和临时目录外，
/// 允许读取用户桌面和 `PRIORITY_AGENT_READ_ROOTS` 声明的只读根目录。
pub fn resolve_read_path(
    path: &str,
    working_dir: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    resolve_path_with_policy(path, working_dir, true)
}

fn resolve_path_with_policy(
    path: &str,
    working_dir: &std::path::Path,
    read_only: bool,
) -> Result<std::path::PathBuf, String> {
    let expanded_input = expand_home_path(path);
    let input = expanded_input.as_path();
    let normalized_working_dir = normalize_path(working_dir);

    let candidate = if input.is_absolute() {
        normalize_path(input)
    } else {
        normalize_path(&normalized_working_dir.join(input))
    };

    if input.is_absolute() {
        if !is_allowed_path_for_policy(&candidate, &normalized_working_dir, read_only) {
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
        if !is_allowed_path_for_policy(&real_candidate, &real_working_dir, read_only) {
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

fn is_allowed_path_for_policy(path: &Path, working_dir: &Path, read_only: bool) -> bool {
    if read_only {
        is_allowed_read_absolute_path(path, working_dir)
    } else {
        is_allowed_absolute_path(path, working_dir)
    }
}

fn expand_home_path(path: &str) -> PathBuf {
    if path == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
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

pub fn is_allowed_read_absolute_path(
    path: &std::path::Path,
    working_dir: &std::path::Path,
) -> bool {
    if is_allowed_absolute_path(path, working_dir) {
        return true;
    }

    let normalized_path = normalize_path(path);
    let canonical_path = canonicalize_or_normalize(&normalized_path);
    read_allowed_roots().into_iter().any(|root| {
        let normalized_root = normalize_path(&root);
        let canonical_root = canonicalize_or_normalize(&normalized_root);
        normalized_path.starts_with(&normalized_root) || canonical_path.starts_with(&canonical_root)
    })
}

fn read_allowed_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Desktop"));
    }
    if let Ok(raw) = std::env::var("PRIORITY_AGENT_READ_ROOTS") {
        roots.extend(
            raw.split(':')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(expand_home_path),
        );
    }
    roots
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
    async fn file_read_directory_returns_entries_without_shell_metadata() {
        let tool = FileReadTool;
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join(".DS_Store"), "metadata")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("note.txt"), "hello")
            .await
            .unwrap();
        tokio::fs::create_dir(dir.path().join("nested"))
            .await
            .unwrap();

        let result = tool
            .execute(
                json!({ "path": dir.path().to_string_lossy().to_string() }),
                ToolContext::new(".", "test-session-read-dir"),
            )
            .await;

        assert!(result.success, "read failed: {:?}", result.error);
        assert!(result.content.contains(".DS_Store"));
        assert!(result.content.contains("note.txt"));
        assert!(result.content.contains("nested/"));
        assert!(!result.content.contains("created"));
        assert!(!result.content.contains("size"));
        let data = result.data.expect("directory read should return metadata");
        assert_eq!(data["kind"], "directory");
        assert_eq!(data["entry_count"], 3);
    }

    #[tokio::test]
    async fn file_read_empty_directory_is_explicit() {
        let tool = FileReadTool;
        let dir = tempfile::tempdir().unwrap();

        let result = tool
            .execute(
                json!({ "path": dir.path().to_string_lossy().to_string() }),
                ToolContext::new(".", "test-session-read-empty-dir"),
            )
            .await;

        assert!(result.success, "read failed: {:?}", result.error);
        assert!(result.content.contains("Entries (0):"));
        assert!(result.content.contains("(empty)"));
        let data = result.data.expect("directory read should return metadata");
        assert_eq!(data["kind"], "directory");
        assert_eq!(data["entry_count"], 0);
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

    #[tokio::test]
    async fn test_file_write_existing_file_reports_full_replacement_guidance() {
        let write_tool = FileWriteTool;
        let path = "/tmp/test_priority_agent_file_write_existing.txt";
        tokio::fs::write(path, "old\n").await.unwrap();

        let result = write_tool
            .execute(
                json!({
                    "path": path,
                    "content": "new\n"
                }),
                ToolContext::new(".", "test-session-file-write-existing"),
            )
            .await;

        assert!(result.success, "write failed: {:?}", result.error);
        assert!(result.content.contains("overwritten"));
        let data = result.data.expect("file_write should return metadata");
        assert_eq!(data["existed_before"], true);
        assert!(data["guidance"]
            .as_str()
            .unwrap_or("")
            .contains("file_edit"));

        let _ = tokio::fs::remove_file(path).await;
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
    fn resolve_read_path_allows_home_desktop_without_allowing_writes() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        let home = tempfile::tempdir().unwrap();
        let desktop = home.path().join("Desktop");
        std::fs::create_dir_all(desktop.join("gex")).unwrap();
        env.set("HOME", home.path().to_str().unwrap());

        let working = tempfile::tempdir().unwrap();
        let read_path = resolve_read_path("~/Desktop/gex", working.path()).unwrap();
        assert_eq!(read_path, normalize_path(&desktop.join("gex")));

        let write_path = resolve_path("~/Desktop/gex", working.path());
        assert!(write_path.is_err());
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

    #[tokio::test]
    async fn file_read_targeted_range_is_not_hidden_by_unchanged_cache() {
        let read_tool = FileReadTool;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.rs");
        tokio::fs::write(
            &path,
            "fn first() {}\n\nfn summary_task() {\n    todo!();\n}\n\nfn run_one() {}\n",
        )
        .await
        .unwrap();

        let cache = std::sync::Arc::new(crate::tools::file_cache::FileStateCache::new());
        let context = ToolContext::new(".", "test-session-targeted-cache").with_file_cache(cache);

        let full_read = read_tool
            .execute(
                json!({ "path": path.to_string_lossy().to_string() }),
                context.clone(),
            )
            .await;
        assert!(full_read.success, "full read failed: {:?}", full_read.error);

        let targeted_read = read_tool
            .execute(
                json!({
                    "path": path.to_string_lossy().to_string(),
                    "offset": 3,
                    "limit": 3
                }),
                context.clone(),
            )
            .await;
        assert!(
            targeted_read.success,
            "targeted read failed: {:?}",
            targeted_read.error
        );
        assert!(targeted_read.content.contains("summary_task"));
        assert!(targeted_read.content.contains("todo!();"));
        assert!(!targeted_read
            .content
            .contains("File unchanged since last read"));

        let broad_repeat = read_tool
            .execute(
                json!({ "path": path.to_string_lossy().to_string() }),
                context,
            )
            .await;
        assert!(broad_repeat.success);
        assert!(broad_repeat
            .content
            .contains("File unchanged since last read"));
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
    async fn test_file_edit_rejects_stale_read_by_default() {
        let read_tool = FileReadTool;
        let edit_tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_stale_read.txt";
        let session_id = "test-session-edit-stale-read";
        tokio::fs::write(path, "hello world\n").await.unwrap();

        let read_result = read_tool
            .execute(json!({ "path": path }), ToolContext::new(".", session_id))
            .await;
        assert!(read_result.success, "read failed: {:?}", read_result.error);

        tokio::fs::write(path, "hello changed\n").await.unwrap();
        let edit_result = edit_tool
            .execute(
                json!({
                    "path": path,
                    "old_string": "hello changed",
                    "new_string": "hello edited"
                }),
                ToolContext::new(".", session_id),
            )
            .await;

        assert!(!edit_result.success);
        let err = edit_result.error.unwrap_or_default();
        assert!(err.contains("file changed since this session last read it"));
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert_eq!(content, "hello changed\n");

        let _ = tokio::fs::remove_file(path).await;
        clear_read_files(session_id);
    }

    #[tokio::test]
    async fn test_file_edit_stale_read_uses_resolved_path_key() {
        let read_tool = FileReadTool;
        let edit_tool = FileEditTool;
        let session_id = "test-session-edit-stale-resolved-path";
        let root = std::env::temp_dir().join(format!(
            "test_priority_agent_edit_stale_resolved_path_{}",
            std::process::id()
        ));
        let nested = root.join("nested");
        let path = nested.join("target.txt");
        let _ = tokio::fs::remove_dir_all(&root).await;
        tokio::fs::create_dir_all(&nested).await.unwrap();
        tokio::fs::write(&path, "hello world\n").await.unwrap();

        let read_result = read_tool
            .execute(
                json!({ "path": "nested/target.txt" }),
                ToolContext::new(&root, session_id),
            )
            .await;
        assert!(read_result.success, "read failed: {:?}", read_result.error);

        tokio::fs::write(&path, "hello changed\n").await.unwrap();
        let edit_result = edit_tool
            .execute(
                json!({
                    "path": path.to_string_lossy().to_string(),
                    "old_string": "hello changed",
                    "new_string": "hello edited"
                }),
                ToolContext::new(&root, session_id),
            )
            .await;

        assert!(!edit_result.success);
        let err = edit_result.error.unwrap_or_default();
        assert!(err.contains("file changed since this session last read it"));
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "hello changed\n");

        let _ = tokio::fs::remove_dir_all(&root).await;
        clear_read_files(session_id);
    }

    #[tokio::test]
    async fn test_file_edit_allows_explicit_stale_read_override() {
        let read_tool = FileReadTool;
        let edit_tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_stale_override.txt";
        let session_id = "test-session-edit-stale-override";
        tokio::fs::write(path, "hello world\n").await.unwrap();

        let read_result = read_tool
            .execute(json!({ "path": path }), ToolContext::new(".", session_id))
            .await;
        assert!(read_result.success, "read failed: {:?}", read_result.error);

        tokio::fs::write(path, "hello changed\n").await.unwrap();
        let edit_result = edit_tool
            .execute(
                json!({
                    "path": path,
                    "old_string": "hello changed",
                    "new_string": "hello edited",
                    "allow_stale_read": true
                }),
                ToolContext::new(".", session_id),
            )
            .await;

        assert!(edit_result.success, "edit failed: {:?}", edit_result.error);
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert_eq!(content, "hello edited\n");

        let _ = tokio::fs::remove_file(path).await;
        clear_read_files(session_id);
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
    async fn test_file_edit_rejects_whitespace_only_old_string() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_blank_anchor.txt";
        tokio::fs::write(path, "line1\nline2\nline3\n")
            .await
            .unwrap();

        let result = tool
            .execute(
                json!({
                    "path": path,
                    "old_string": "\n",
                    "new_string": "replacement"
                }),
                ToolContext::new(".", "test-session-edit-blank-anchor"),
            )
            .await;

        assert!(!result.success);
        let err = result.error.unwrap_or_default();
        assert!(err.contains("whitespace-only"));
        assert!(err.contains("line_start"));

        let _ = tokio::fs::remove_file(path).await;
    }

    #[test]
    fn test_match_context_limits_large_occurrence_output() {
        let content = (0..50)
            .map(|i| format!("let value_{i} = true;"))
            .collect::<Vec<_>>()
            .join("\n");
        let occurrences = find_occurrences(&content, "let");
        let context = build_match_context(&content, &occurrences, 0);

        assert!(context.contains("Found 50 occurrence(s)"));
        assert!(context.contains("showing first 12 of 50 matches"));
        assert!(!context.contains("Match #13"));
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
    async fn test_file_edit_rejects_bulk_exact_replace_on_code_file() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_expected.rs";
        tokio::fs::write(path, "let x = 1;\nlet x = 1;\n")
            .await
            .unwrap();

        let params = json!({
            "path": path,
            "old_string": "let x = 1;",
            "new_string": "let x = 2;",
            "expected_replacements": 2
        });
        let context = ToolContext::new(".", "test-session-edit-code-bulk");
        let result = tool.execute(params, context).await;

        assert!(!result.success);
        let err = result.error.unwrap_or_default();
        assert!(err.contains("Refusing multi-occurrence file_edit on code file"));
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert_eq!(content.matches("let x = 1;").count(), 2);

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_rejects_excessive_bulk_replacements() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_bulk_limit.txt";
        tokio::fs::write(path, "aaa\n".repeat(51)).await.unwrap();

        let params = json!({
            "path": path,
            "old_string": "aaa",
            "new_string": "bbb",
            "expected_replacements": 51
        });
        let context = ToolContext::new(".", "test-session-edit-bulk-limit");
        let result = tool.execute(params, context).await;

        assert!(!result.success);
        let err = result.error.unwrap_or_default();
        assert!(err.contains("Refusing file_edit with 51 replacement"));

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
    async fn test_file_edit_rejects_file_read_line_prefix_in_old_string() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_line_prefix.txt";
        tokio::fs::write(path, "hello world\n").await.unwrap();

        let result = tool
            .execute(
                json!({
                    "path": path,
                    "old_string": "   1 | hello world",
                    "new_string": "hi world"
                }),
                ToolContext::new(".", "test-session-edit-line-prefix"),
            )
            .await;

        assert!(!result.success);
        let err = result.error.unwrap_or_default();
        assert!(err.contains("file_read display line prefixes"));
        assert!(err.contains("line_start/line_end"));
        let content = tokio::fs::read_to_string(path).await.unwrap();
        assert_eq!(content, "hello world\n");

        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn test_file_edit_rejects_file_read_line_prefix_in_insert_anchor() {
        let tool = FileEditTool;
        let path = "/tmp/test_priority_agent_edit_insert_line_prefix.txt";
        tokio::fs::write(path, "hello world\n").await.unwrap();

        let result = tool
            .execute(
                json!({
                    "path": path,
                    "insert_after": "   1 | hello world",
                    "new_string": "\nhi world"
                }),
                ToolContext::new(".", "test-session-edit-insert-line-prefix"),
            )
            .await;

        assert!(!result.success);
        let err = result.error.unwrap_or_default();
        assert!(err.contains("insert_after appears to include file_read"));

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
            dirs::home_dir()
                .unwrap()
                .join(".priority-agent")
                .join("checkpoints")
                .join(format!("session-{}", session_id)),
        )
        .await;
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
