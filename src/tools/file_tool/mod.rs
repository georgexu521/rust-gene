//! 文件操作工具
//!
//! 提供文件读取、写入、编辑功能

use crate::engine::context_ledger::{record_file_read, FileReadLedgerInput};
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult, ToolSearchOrReadSemantics};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::json;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};

mod diagnostics;
pub(crate) mod history;
mod patch;
mod path_policy;
mod text_codec;

use diagnostics::{
    collect_file_edit_diagnostics, file_edit_diagnostics_content_line, file_edit_diagnostics_delta,
};
use history::{
    checkpoint_metadata_json, create_file_checkpoint, record_file_change, FileChangeRequest,
};
pub use patch::FilePatchTool;
pub(crate) use path_policy::is_unc_or_network_path;
pub use path_policy::{
    canonicalize_or_normalize, is_allowed_absolute_path, is_allowed_read_absolute_path,
    normalize_path, resolve_path, resolve_read_path,
};
use text_codec::{
    detect_line_ending, normalize_text_line_endings, read_text_file, split_leading_text_bom,
    text_format_json, text_write_format_json, write_text_file, TextFileEncoding,
};

#[cfg(test)]
use text_codec::{decode_text_file, encode_text_content, LineEndingStyle};

const MAX_EDITABLE_FILE_SIZE_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB
const DEFAULT_MAX_FILE_EDIT_REPLACEMENTS: usize = 20;
const MAX_MATCH_CONTEXT_OCCURRENCES: usize = 12;
const DEFAULT_DIRECTORY_READ_ENTRY_LIMIT: usize = 200;
const FILE_READ_PREVIEW_MAX_LINES: usize = 5;
const FILE_READ_PREVIEW_MAX_CHARS: usize = 280;

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

fn allow_high_risk_file_mutation() -> bool {
    std::env::var("PRIORITY_AGENT_ALLOW_HIGH_RISK_FILE_MUTATION").as_deref() == Ok("1")
}

fn allow_edit_without_read() -> bool {
    std::env::var("PRIORITY_AGENT_ALLOW_EDIT_WITHOUT_READ").as_deref() == Ok("1")
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

fn path_component_names(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

fn high_risk_file_target_diagnostic(
    path: &Path,
    identity: &FilePathIdentity,
    working_dir: &Path,
    operation: &str,
) -> Option<(String, serde_json::Value)> {
    if allow_high_risk_file_mutation() {
        return None;
    }

    let components = path_component_names(path);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let classification = if extension == "ipynb" {
        Some((
            "wrong_tool_notebook",
            "notebook files should be changed through notebook-aware tooling, not raw file mutation",
            "use_notebook_tool",
        ))
    } else if file_name == ".env"
        || file_name.starts_with(".env.")
        || file_name.ends_with(".env")
        || matches!(
            file_name.as_str(),
            "id_rsa" | "id_dsa" | "id_ecdsa" | "id_ed25519" | "authorized_keys" | "known_hosts"
        )
        || matches!(
            extension.as_str(),
            "pem" | "key" | "p12" | "pfx" | "crt" | "cer"
        )
    {
        Some((
            "secret_or_credential_target",
            "target looks like an environment, credential, certificate, or SSH key file",
            "ask_user_for_explicit_secret_file_plan",
        ))
    } else if components.iter().any(|component| component == ".git") {
        Some((
            "vcs_metadata_target",
            "target is inside .git metadata",
            "use_git_tool_or_choose_project_file",
        ))
    } else if is_live_eval_worktree_path(path, working_dir) {
        None
    } else if components.iter().any(|component| {
        matches!(
            component.as_str(),
            "target"
                | "node_modules"
                | "dist"
                | "build"
                | ".next"
                | ".nuxt"
                | ".cache"
                | "coverage"
        )
    }) {
        Some((
            "generated_or_dependency_target",
            "target is inside a generated, build, cache, coverage, or dependency directory",
            "edit_source_file_instead",
        ))
    } else {
        let canonical_path = canonicalize_or_normalize(path);
        let canonical_working_dir = canonicalize_or_normalize(working_dir);
        let home_config = std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| {
                canonical_path.starts_with(canonicalize_or_normalize(&home.join(".config")))
            })
            .unwrap_or(false);
        if home_config && !canonical_path.starts_with(&canonical_working_dir) {
            Some((
                "home_config_outside_project",
                "target is a home configuration file outside the selected project",
                "open_config_setup_or_switch_project",
            ))
        } else {
            None
        }
    }?;

    let (failure, reason, recommended_action) = classification;
    let message = format!(
        "Refusing {operation} for '{}': {reason}. Set PRIORITY_AGENT_ALLOW_HIGH_RISK_FILE_MUTATION=1 only after an explicit user-approved plan.",
        identity.lexical_path
    );
    Some((
        message,
        json!({
            "failure": failure,
            "operation": operation,
            "path_identity": path_identity_json(identity),
            "guardrail": {
                "reason": reason,
                "override_env": "PRIORITY_AGENT_ALLOW_HIGH_RISK_FILE_MUTATION",
                "override_required_value": "1",
            },
            "recovery": {
                "recommended_action": recommended_action,
                "next_actions": ["choose_safer_target", "ask_user_for_explicit_approval", "retry_with_source_file"],
            }
        }),
    ))
}

fn is_live_eval_worktree_path(path: &Path, working_dir: &Path) -> bool {
    let normalized_path = normalize_path(path);
    let normalized_working_dir = normalize_path(working_dir);
    let canonical_path = canonicalize_or_normalize(path);
    let canonical_working_dir = canonicalize_or_normalize(working_dir);

    let inside_working_dir = normalized_path.starts_with(&normalized_working_dir)
        || normalized_path.starts_with(&canonical_working_dir)
        || canonical_path.starts_with(&normalized_working_dir)
        || canonical_path.starts_with(&canonical_working_dir);
    if !inside_working_dir {
        return false;
    }

    let is_live_eval_worktree = [&normalized_working_dir, &canonical_working_dir]
        .into_iter()
        .map(|path| path.to_string_lossy().to_ascii_lowercase())
        .any(|lower| lower.contains("/target/live-evals/") && lower.ends_with("/worktree"));
    is_live_eval_worktree
}

fn high_risk_file_target_result(
    path: &Path,
    identity: &FilePathIdentity,
    working_dir: &Path,
    operation: &str,
) -> Option<ToolResult> {
    high_risk_file_target_diagnostic(path, identity, working_dir, operation)
        .map(|(message, data)| file_edit_error_with_data(message, data))
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

#[derive(Clone, Debug, Default)]
struct FileReadRecord {
    full_read: bool,
    ranges: Vec<(usize, usize)>,
}

/// 文件修改状态跟踪（用于检测外部修改）
#[derive(Clone, Debug)]
struct FileState {
    mtime: std::time::SystemTime,
    content_hash: u64,
}

#[derive(Clone, Debug)]
struct FilePathIdentity {
    lexical_path: String,
    resolved_path: String,
    canonical_path: String,
    display_path: String,
    state_key: String,
}

#[derive(Clone, Debug)]
pub(crate) struct EditDiffSummary {
    pub(crate) additions: usize,
    pub(crate) deletions: usize,
    pub(crate) changed_line_start: usize,
    pub(crate) changed_line_end: usize,
    pub(crate) unified_diff: String,
    pub(crate) preview_truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadBeforeEditStatus {
    Allowed,
    NotRead,
    PartialOnly,
}

#[derive(Debug, Default)]
struct FileStateTracker {
    read_files: HashMap<String, HashMap<String, FileReadRecord>>,
    file_states: HashMap<String, FileState>,
}

impl FileStateTracker {
    fn mark_read_coverage(
        &mut self,
        session_id: &str,
        file_path: &str,
        line_range: Option<(usize, usize)>,
    ) {
        let session_files = self.read_files.entry(session_id.to_string()).or_default();
        let record = session_files.entry(file_path.to_string()).or_default();
        if let Some((start, end)) = line_range {
            if start > 0 && end >= start {
                record.ranges.push((start, end));
            }
        } else {
            record.full_read = true;
        }
    }

    fn mark_read_with_state(
        &mut self,
        session_id: &str,
        file_path: &str,
        content: &str,
        mtime: std::time::SystemTime,
        line_range: Option<(usize, usize)>,
    ) {
        self.mark_read_coverage(session_id, file_path, line_range);
        let key = tracker_key(session_id, file_path);
        self.file_states.insert(
            key,
            FileState {
                mtime,
                content_hash: compute_content_hash(content),
            },
        );
    }

    fn is_file_read(&self, session_id: &str, file_path: &str) -> bool {
        self.read_files
            .get(session_id)
            .and_then(|files| files.get(file_path))
            .is_some()
    }

    fn read_before_edit_status(
        &self,
        session_id: &str,
        file_path: &str,
        line_start: Option<usize>,
        line_end: Option<usize>,
    ) -> ReadBeforeEditStatus {
        let Some(record) = self
            .read_files
            .get(session_id)
            .and_then(|files| files.get(file_path))
        else {
            return ReadBeforeEditStatus::NotRead;
        };
        if record.full_read {
            return ReadBeforeEditStatus::Allowed;
        }
        let Some((start, end)) = line_start.zip(line_end) else {
            return ReadBeforeEditStatus::PartialOnly;
        };
        if record
            .ranges
            .iter()
            .any(|(range_start, range_end)| *range_start <= start && *range_end >= end)
        {
            ReadBeforeEditStatus::Allowed
        } else {
            ReadBeforeEditStatus::PartialOnly
        }
    }

    fn is_modified_since_read(
        &self,
        session_id: &str,
        file_path: &str,
        current_content: &str,
        current_mtime: std::time::SystemTime,
    ) -> bool {
        let key = tracker_key(session_id, file_path);
        if let Some(state) = self.file_states.get(&key) {
            if current_mtime != state.mtime {
                return true;
            }
            if compute_content_hash(current_content) != state.content_hash {
                return true;
            }
        }
        false
    }

    fn clear_session(&mut self, session_id: &str) {
        self.read_files.remove(session_id);
        let prefix = format!("{}:", session_id);
        self.file_states.retain(|key, _| !key.starts_with(&prefix));
    }
}

static FILE_STATE_TRACKER: Lazy<Mutex<FileStateTracker>> =
    Lazy::new(|| Mutex::new(FileStateTracker::default()));

static FILE_MUTATION_LOCKS: Lazy<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn tracker_key(session_id: &str, file_path: &str) -> String {
    format!("{}:{}", session_id, file_path)
}

async fn acquire_file_mutation_lock(state_key: &str) -> tokio::sync::OwnedMutexGuard<()> {
    let lock = {
        let mut locks = FILE_MUTATION_LOCKS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        locks
            .entry(state_key.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    lock.lock_owned().await
}

/// 标记文件已被读取（用于 must-read-before-edit 检查）
pub fn mark_file_read(session_id: &str, file_path: &str) {
    let mut tracker = FILE_STATE_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    tracker.mark_read_coverage(session_id, file_path, None);
}

/// 检查文件是否已被读取
pub fn is_file_read(session_id: &str, file_path: &str) -> bool {
    let tracker = FILE_STATE_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    tracker.is_file_read(session_id, file_path)
}

/// Read-before-edit enforcement gate (env: PRIORITY_AGENT_READ_BEFORE_EDIT, default on).
fn read_before_edit_enabled() -> bool {
    std::env::var("PRIORITY_AGENT_READ_BEFORE_EDIT")
        .unwrap_or_else(|_| "1".to_string())
        .trim()
        != "0"
}

/// Check that a file was read before writing/editing. Returns Some(error) if blocked.
pub fn check_read_before_write(session_id: &str, file_path: &str) -> Option<ToolResult> {
    if !read_before_edit_enabled() {
        return None;
    }
    // Use canonical path as the lookup key to match what FileReadTool records
    // (canonicalize resolves symlinks like /var → /private/var on macOS).
    let canonical_key = canonicalize_or_normalize(std::path::Path::new(file_path))
        .to_string_lossy()
        .to_string();
    if !is_file_read(session_id, &canonical_key) && !is_file_read(session_id, file_path) {
        Some(ToolResult::error(format!(
            "File '{}' has not been read yet in this session. \
             Read the file first with file_read before editing or writing to it. \
             (Set PRIORITY_AGENT_READ_BEFORE_EDIT=0 to disable this check.)",
            file_path
        )))
    } else {
        None
    }
}

fn read_before_edit_status(
    session_id: &str,
    file_path: &str,
    line_start: Option<usize>,
    line_end: Option<usize>,
) -> ReadBeforeEditStatus {
    let tracker = FILE_STATE_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    tracker.read_before_edit_status(session_id, file_path, line_start, line_end)
}

fn file_state_snapshot(session_id: &str, file_path: &str) -> Option<FileState> {
    let tracker = FILE_STATE_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    tracker
        .file_states
        .get(&tracker_key(session_id, file_path))
        .cloned()
}

fn file_read_state_guidance(path: &str, status: ReadBeforeEditStatus) -> String {
    match status {
        ReadBeforeEditStatus::Allowed => String::new(),
        ReadBeforeEditStatus::NotRead => format!(
            "File '{}' has not been read yet. You must read a file before editing it. Use file_read tool first.",
            path
        ),
        ReadBeforeEditStatus::PartialOnly => format!(
            "File '{}' has only been partially read in this session. Re-read the full file before exact/insert edits, or use line_start/line_end within a previously read range.",
            path
        ),
    }
}

/// 清除会话的读取状态
pub fn clear_read_files(session_id: &str) {
    let mut tracker = FILE_STATE_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    tracker.clear_session(session_id);
}

async fn read_directory_result(
    path: &Path,
    requested_path: &str,
    identity: &FilePathIdentity,
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
            "resolved_path": identity.resolved_path,
            "path_identity": path_identity_json(identity),
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

fn content_hash_hex(content: &str) -> String {
    format!("{:016x}", compute_content_hash(content))
}

fn file_read_content_preview<'a, I>(lines: I) -> Option<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut parts = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        parts.push(trimmed.to_string());
        if parts.len() >= FILE_READ_PREVIEW_MAX_LINES {
            break;
        }
    }
    if parts.is_empty() {
        return None;
    }
    let mut preview = parts.join(" | ");
    if preview.chars().count() > FILE_READ_PREVIEW_MAX_CHARS {
        preview = preview.chars().take(FILE_READ_PREVIEW_MAX_CHARS).collect();
        preview.push_str("...");
    }
    Some(preview)
}

pub(crate) fn edit_diff_summary(
    path: &str,
    old_content: &str,
    new_content: &str,
) -> EditDiffSummary {
    const CONTEXT_LINES: usize = 3;
    const MAX_DIFF_LINES: usize = 80;

    let old_lines = old_content.lines().collect::<Vec<_>>();
    let new_lines = new_content.lines().collect::<Vec<_>>();

    let mut prefix_len = 0usize;
    while prefix_len < old_lines.len()
        && prefix_len < new_lines.len()
        && old_lines[prefix_len] == new_lines[prefix_len]
    {
        prefix_len += 1;
    }

    let mut suffix_len = 0usize;
    while suffix_len + prefix_len < old_lines.len()
        && suffix_len + prefix_len < new_lines.len()
        && old_lines[old_lines.len() - 1 - suffix_len]
            == new_lines[new_lines.len() - 1 - suffix_len]
    {
        suffix_len += 1;
    }

    let old_changed_end = old_lines.len().saturating_sub(suffix_len);
    let new_changed_end = new_lines.len().saturating_sub(suffix_len);
    let old_changed = old_changed_end.saturating_sub(prefix_len);
    let new_changed = new_changed_end.saturating_sub(prefix_len);

    let changed_line_start = if new_lines.is_empty() {
        0
    } else {
        (prefix_len + 1).min(new_lines.len())
    };
    let changed_line_end = if new_changed == 0 {
        changed_line_start
    } else {
        new_changed_end.min(new_lines.len())
    };

    let context_start = prefix_len.saturating_sub(CONTEXT_LINES);
    let old_context_end = (old_changed_end + CONTEXT_LINES).min(old_lines.len());
    let new_context_end = (new_changed_end + CONTEXT_LINES).min(new_lines.len());
    let old_hunk_count = old_context_end.saturating_sub(context_start);
    let new_hunk_count = new_context_end.saturating_sub(context_start);

    let mut diff_lines = Vec::new();
    diff_lines.push(format!("--- a/{path}"));
    diff_lines.push(format!("+++ b/{path}"));
    diff_lines.push(format!(
        "@@ -{},{} +{},{} @@",
        context_start + 1,
        old_hunk_count,
        context_start + 1,
        new_hunk_count
    ));

    for line in &old_lines[context_start..prefix_len] {
        diff_lines.push(format!(" {line}"));
    }
    for line in &old_lines[prefix_len..old_changed_end] {
        diff_lines.push(format!("-{line}"));
    }
    for line in &new_lines[prefix_len..new_changed_end] {
        diff_lines.push(format!("+{line}"));
    }
    for line in &new_lines[new_changed_end..new_context_end] {
        diff_lines.push(format!(" {line}"));
    }

    let preview_truncated = diff_lines.len() > MAX_DIFF_LINES;
    if preview_truncated {
        diff_lines.truncate(MAX_DIFF_LINES);
        diff_lines.push(format!(
            "[diff preview truncated: showing first {MAX_DIFF_LINES} lines]"
        ));
    }

    EditDiffSummary {
        additions: new_changed,
        deletions: old_changed,
        changed_line_start,
        changed_line_end,
        unified_diff: diff_lines.join("\n"),
        preview_truncated,
    }
}

pub(crate) fn edit_diff_summary_json(diff: &EditDiffSummary) -> serde_json::Value {
    json!({
        "additions": diff.additions,
        "deletions": diff.deletions,
        "changed_line_start": json_line_number(diff.changed_line_start),
        "changed_line_end": json_line_number(diff.changed_line_end),
        "unified_diff": diff.unified_diff,
        "preview_truncated": diff.preview_truncated,
    })
}

#[allow(clippy::too_many_arguments)]
fn edit_preview_json(
    identity: &FilePathIdentity,
    existed_before: bool,
    before_content: Option<&str>,
    after_content: &str,
    diff: &EditDiffSummary,
    text_format: serde_json::Value,
    checkpoint: serde_json::Value,
    file_change: serde_json::Value,
    replacements: Option<usize>,
    bytes_written: u64,
    validation_stage: &str,
) -> serde_json::Value {
    let checkpoint_id = checkpoint
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let file_change_id = file_change
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    json!({
        "path": identity.lexical_path.clone(),
        "resolved_path": identity.resolved_path.clone(),
        "canonical_path": identity.canonical_path.clone(),
        "display_path": identity.display_path.clone(),
        "state_key": identity.state_key.clone(),
        "existed_before": existed_before,
        "before_hash": before_content.map(content_hash_hex),
        "after_hash": content_hash_hex(after_content),
        "replacements": replacements,
        "bytes_written": bytes_written,
        "changed_range": {
            "start": json_line_number(diff.changed_line_start),
            "end": json_line_number(diff.changed_line_end),
        },
        "additions": diff.additions,
        "deletions": diff.deletions,
        "diff_preview": diff.unified_diff.clone(),
        "diff_preview_truncated": diff.preview_truncated,
        "text_format": text_format,
        "validation_stage": validation_stage,
        "external_modified": false,
        "checkpoint_id": checkpoint_id,
        "file_change_id": file_change_id,
        "rollback": {
            "kind": "checkpoint",
            "checkpoint_id": checkpoint.get("id").cloned().unwrap_or(serde_json::Value::Null),
            "file_change_id": file_change.get("id").cloned().unwrap_or(serde_json::Value::Null),
        }
    })
}

fn file_state_key(path: &Path) -> String {
    canonicalize_or_normalize(path)
        .to_string_lossy()
        .to_string()
}

fn file_path_identity(
    requested_path: &str,
    resolved_path: &Path,
    working_dir: &Path,
) -> FilePathIdentity {
    let normalized_resolved_path = normalize_path(resolved_path);
    let canonical_path = canonicalize_or_normalize(&normalized_resolved_path);
    let normalized_working_dir = normalize_path(working_dir);
    let canonical_working_dir = canonicalize_or_normalize(working_dir);
    let display_path = relative_display_path(&normalized_resolved_path, &normalized_working_dir)
        .or_else(|| relative_display_path(&canonical_path, &canonical_working_dir))
        .unwrap_or_else(|| normalized_resolved_path.to_string_lossy().to_string());

    FilePathIdentity {
        lexical_path: requested_path.to_string(),
        resolved_path: normalized_resolved_path.to_string_lossy().to_string(),
        canonical_path: canonical_path.to_string_lossy().to_string(),
        display_path,
        state_key: file_state_key(&canonical_path),
    }
}

fn relative_display_path(path: &Path, root: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| path.to_string_lossy().to_string())
}

fn path_identity_json(identity: &FilePathIdentity) -> serde_json::Value {
    json!({
        "lexical_path": identity.lexical_path,
        "resolved_path": identity.resolved_path,
        "canonical_path": identity.canonical_path,
        "display_path": identity.display_path,
        "state_key": identity.state_key,
    })
}

fn json_line_number(value: usize) -> serde_json::Value {
    if value == 0 {
        serde_json::Value::Null
    } else {
        json!(value)
    }
}

/// 标记文件已被读取并记录状态（用于变更检测）
pub fn mark_file_read_with_state(
    session_id: &str,
    file_path: &str,
    content: &str,
    mtime: std::time::SystemTime,
) {
    mark_file_read_with_state_and_coverage(session_id, file_path, content, mtime, None);
}

fn mark_file_read_with_state_and_coverage(
    session_id: &str,
    file_path: &str,
    content: &str,
    mtime: std::time::SystemTime,
    line_range: Option<(usize, usize)>,
) {
    let mut tracker = FILE_STATE_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    tracker.mark_read_with_state(session_id, file_path, content, mtime, line_range);
}

/// 检查文件是否在读取后被外部修改
pub fn is_file_modified_since_read(
    session_id: &str,
    file_path: &str,
    current_content: &str,
    current_mtime: std::time::SystemTime,
) -> bool {
    let tracker = FILE_STATE_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    tracker.is_modified_since_read(session_id, file_path, current_content, current_mtime)
}

/// 文件读取工具
pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read a workspace file or directory. Files return full content by default; \
         use limit and offset for line ranges. Directories list entries with '/' on \
         subdirs. If unchanged since last read, reuse the previous content. Use glob \
         for filenames and grep for content search."
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

    fn aliases(&self) -> &'static [&'static str] {
        &["read"]
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("view file contents directory entries")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_search_or_read_command(&self, _params: &serde_json::Value) -> ToolSearchOrReadSemantics {
        ToolSearchOrReadSemantics {
            is_read: true,
            ..Default::default()
        }
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let path = params["path"].as_str()?.trim();
        if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        }
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
        let identity = file_path_identity(path_str, &path, &context.working_dir);

        // 检查文件是否存在
        if !path.exists() {
            return ToolResult::error(format!("File does not exist: {}", path_str));
        }

        if path.is_dir() {
            if let Some(ref tracker) = context.read_tracker {
                tracker.mark_read(&path);
            }
            return read_directory_result(&path, path_str, &identity, limit, offset).await;
        }

        // 检查是否是文件
        if !path.is_file() {
            return ToolResult::error(format!("Path is not a file: {}", path_str));
        }
        if let Err(msg) = check_file_size_limit(&path, "read") {
            return ToolResult::error(msg);
        }

        // 读取文件内容
        let snapshot = match read_text_file(&path, "read").await {
            Ok(snapshot) => snapshot,
            Err(e) => {
                error!("{}", e);
                return ToolResult::error(e);
            }
        };
        // Read-before-edit guard: record this successful read.
        if let Some(ref tracker) = context.read_tracker {
            tracker.mark_read(&path);
        }
        let content = snapshot.content.as_str();

        let targeted_read = limit.is_some() || offset.is_some();

        // 文件缓存优化：如果文件在本会话中已读过且未变更，返回短信提示。
        // 但 offset/limit 读取是新的局部证据，不能被上一次全文读取短路掉。
        // Skip cache short-circuit in eval/non-interactive mode so the model
        // always sees full file content when reading the same file multiple times.
        let eval_no_cache = std::env::var("PRIORITY_AGENT_EVAL_NO_FILE_CACHE")
            .unwrap_or_default()
            .trim()
            == "1";

        if let Some(ref cache) = context.file_cache {
            if !eval_no_cache
                && !targeted_read
                && cache.is_unchanged_since_last_read_for_session(&context.session_id, &path)
            {
                let lines_count = content.lines().count();
                let content_preview = file_read_content_preview(content.lines());
                let preview_note = content_preview
                    .as_deref()
                    .map(|preview| format!("\nContext preview: {preview}"))
                    .unwrap_or_default();
                return ToolResult::success_with_data(
                    format!(
                        "[File unchanged since last read: {}] ({} lines)\nIf you need the full content, it was provided in a previous message.{}",
                        path_str, lines_count, preview_note
                    ),
                    json!({
                        "path": path_str,
                        "resolved_path": identity.resolved_path,
                        "path_identity": path_identity_json(&identity),
                        "kind": "file",
                        "unchanged": true,
                        "total_lines": lines_count,
                        "displayed_lines": 0,
                        "line_start": serde_json::Value::Null,
                        "line_end": serde_json::Value::Null,
                        "truncated": false,
                        "targeted_read": false,
                        "read_coverage": "full",
                        "size_bytes": snapshot.byte_len,
                        "content_hash": content_hash_hex(content),
                        "content_preview": content_preview,
                        "text_format": text_format_json(&snapshot),
                        "display_format": "unchanged_notice",
                        "content_format": {
                            "visible_content": "unchanged_notice",
                            "raw_content_in_tool_result": false,
                            "display_prefix": serde_json::Value::Null,
                            "truncation_hint_in_content": false
                        }
                    }),
                );
            }
            cache.mark_read_for_session(&context.session_id, &path);
        }

        let mtime = std::fs::metadata(&path)
            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let state_key = identity.state_key.clone();

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
        let truncated = end < lines.len() || start > 0;
        let line_start_display = if selected_lines.is_empty() {
            0
        } else {
            start + 1
        };
        let line_end_display = if selected_lines.is_empty() { 0 } else { end };
        let read_coverage = if targeted_read {
            (line_start_display > 0 && line_end_display >= line_start_display)
                .then_some((line_start_display, line_end_display))
        } else {
            None
        };
        mark_file_read_with_state_and_coverage(
            &context.session_id,
            &state_key,
            content,
            mtime,
            read_coverage,
        );

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

        let truncated_info = if truncated {
            format!(
                "\n\n[{} lines total, showing lines {}-{}]",
                lines.len(),
                start + 1,
                end
            )
        } else {
            String::new()
        };
        let content_hash = content_hash_hex(content);
        let selected_content_hash = content_hash_hex(&result);
        let content_preview = file_read_content_preview(selected_lines.iter().copied());
        if let Some(store) = context.session_store.as_ref() {
            record_file_read(
                store,
                &FileReadLedgerInput {
                    session_id: &context.session_id,
                    path: path_str,
                    resolved_path: &identity.resolved_path,
                    content_hash: &content_hash,
                    content_preview: content_preview.as_deref(),
                    size_bytes: snapshot.byte_len as u64,
                    total_lines: lines.len(),
                    displayed_lines: selected_lines.len(),
                    line_start: (line_start_display > 0).then_some(line_start_display),
                    line_end: (line_end_display > 0).then_some(line_end_display),
                    targeted_read,
                    truncated,
                    mtime: Some(mtime),
                },
            );
        }

        ToolResult::success_with_data(
            format!("{}{}", formatted, truncated_info),
            json!({
                "path": path_str,
                "resolved_path": identity.resolved_path,
                "path_identity": path_identity_json(&identity),
                "kind": "file",
                "total_lines": lines.len(),
                "displayed_lines": selected_lines.len(),
                "line_start": json_line_number(line_start_display),
                "line_end": json_line_number(line_end_display),
                "truncated": truncated,
                "targeted_read": targeted_read,
                "read_coverage": if targeted_read { "partial" } else { "full" },
                "size_bytes": snapshot.byte_len,
                "content_hash": content_hash,
                "selected_content_hash": selected_content_hash,
                "content_preview": content_preview,
                "text_format": text_format_json(&snapshot),
                "display_format": if lines.len() > 1 { "line_numbered_content" } else { "raw_content" },
                "content_format": {
                    "visible_content": if lines.len() > 1 { "line_numbered_display" } else { "raw_content" },
                    "raw_content_in_tool_result": lines.len() <= 1,
                    "display_prefix": if lines.len() > 1 { serde_json::Value::String("{line} | ".to_string()) } else { serde_json::Value::Null },
                    "truncation_hint_in_content": truncated
                }
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
        "Create a new file, or overwrite an existing file only after reading it \
         in this session. Parent directories are created as needed. For targeted \
         changes to existing files, use file_edit instead — this tool replaces \
         the entire file. Existing-file writes are rejected until file_read has \
         provided current file content."
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

    fn aliases(&self) -> &'static [&'static str] {
        &["write"]
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("create overwrite files")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Write
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_destructive(&self, params: &serde_json::Value) -> bool {
        let path = params["path"].as_str().unwrap_or("");
        let input_path = Path::new(path);
        input_path.is_absolute() && input_path.exists()
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let path = params["path"].as_str()?.trim();
        if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        }
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
        let identity = file_path_identity(path_str, &path, &context.working_dir);
        if let Some(result) =
            high_risk_file_target_result(&path, &identity, &context.working_dir, "file_write")
        {
            return result;
        }
        let file_guard = acquire_file_mutation_lock(&identity.state_key).await;

        // ReadTracker: block overwriting unreferenced files.
        if let Some(ref tracker) = context.read_tracker {
            if !allow_edit_without_read() {
                if let Err(msg) = tracker.check_edit(&path, content) {
                    return ToolResult::error(msg);
                }
            }
        }

        let existed_before = path.exists();
        let existing_snapshot = if existed_before {
            if let Err(msg) = check_file_size_limit(&path, "write") {
                return ToolResult::error(msg);
            }
            match read_text_file(&path, "write").await {
                Ok(snapshot) => Some(snapshot),
                Err(e) => return ToolResult::error(e),
            }
        } else {
            None
        };
        if existed_before && !allow_edit_without_read() {
            if let Some(error) = check_read_before_write(&context.session_id, &identity.state_key) {
                return error;
            }
        }
        let (content_has_bom, content_body) = split_leading_text_bom(content);
        let encoding = existing_snapshot
            .as_ref()
            .map(|snapshot| snapshot.encoding)
            .unwrap_or(TextFileEncoding::Utf8);
        let has_bom = existing_snapshot
            .as_ref()
            .map(|snapshot| snapshot.has_bom)
            .unwrap_or(false)
            || content_has_bom;
        let line_ending = existing_snapshot
            .as_ref()
            .map(|snapshot| snapshot.line_ending)
            .unwrap_or_else(|| detect_line_ending(content_body));

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

        let before_content = existing_snapshot
            .as_ref()
            .map(|snapshot| snapshot.content.as_str());
        let diff_summary = edit_diff_summary(
            &identity.display_path,
            before_content.unwrap_or(""),
            content_body,
        );
        if let Some(result) =
            priority_agent_settings_validation_error(&identity, content_body, "schema_guard")
        {
            return result;
        }
        let checkpoint = match create_file_checkpoint(&context, "file_write", &path).await {
            Some(checkpoint) => checkpoint,
            None => {
                return checkpoint_creation_failed_result("file_write", path_str, &identity);
            }
        };

        // 写入文件
        match write_text_file(
            &path,
            content_body,
            encoding,
            has_bom,
            line_ending,
            MAX_EDITABLE_FILE_SIZE_BYTES,
        )
        .await
        {
            Ok(bytes_written) => {
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
                drop(file_guard);
                let file_change = record_file_change(
                    &context,
                    FileChangeRequest {
                        checkpoint: Some(&checkpoint),
                        tool_name: "file_write",
                        path: &path,
                        existed_before,
                        before_content,
                        after_content: content_body,
                        diff: &diff_summary,
                        bytes_written: bytes_written as u64,
                    },
                )
                .await;
                let checkpoint_json = checkpoint_metadata_json(Some(&checkpoint));
                let file_change_json = file_change.unwrap_or(serde_json::Value::Null);
                let text_format = text_write_format_json(encoding, has_bom, line_ending);
                let edit_preview = edit_preview_json(
                    &identity,
                    existed_before,
                    before_content,
                    content_body,
                    &diff_summary,
                    text_format.clone(),
                    checkpoint_json.clone(),
                    file_change_json.clone(),
                    None,
                    bytes_written as u64,
                    "write_complete",
                );
                ToolResult::success_with_data(
                    format!("File {} successfully: {}", action, path_str),
                    json!({
                        "path": path_str,
                        "resolved_path": identity.resolved_path,
                        "path_identity": path_identity_json(&identity),
                        "bytes_written": bytes_written,
                        "existed_before": existed_before,
                        "checkpoint": checkpoint_json,
                        "file_change": file_change_json,
                        "diff": edit_diff_summary_json(&diff_summary),
                        "text_format": text_format,
                        "edit_preview": edit_preview,
                        "guidance": if existed_before {
                            "file_write replaced the entire file; use file_edit for targeted existing-file changes"
                        } else {
                            "file_write created a new file"
                        }
                    }),
                )
            }
            Err(e) => ToolResult::error(e),
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

fn file_edit_error_with_data(error: impl Into<String>, data: serde_json::Value) -> ToolResult {
    let error = error.into();
    let mut result = ToolResult::error_with_content(error, data.to_string());
    result.data = Some(data);
    result
}

fn priority_agent_settings_validation_error(
    identity: &FilePathIdentity,
    content: &str,
    stage: &str,
) -> Option<ToolResult> {
    let path = std::path::Path::new(&identity.resolved_path);
    if !is_priority_agent_settings_path(path) {
        return None;
    }

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let validation = if filename == "permissions.toml" {
        validate_permissions_toml(content)
    } else if filename == "config.toml"
        || path.extension().and_then(|ext| ext.to_str()) == Some("toml")
    {
        toml::from_str::<toml::Value>(content)
            .map(|_| ())
            .map_err(|err| format!("invalid TOML: {err}"))
    } else if filename.ends_with(".json") {
        serde_json::from_str::<serde_json::Value>(content)
            .map(|_| ())
            .map_err(|err| format!("invalid JSON: {err}"))
    } else {
        Ok(())
    };

    validation.err().map(|error| {
        file_edit_error_with_data(
            format!(
                "Refusing to write Priority Agent settings file '{}': {}",
                identity.display_path, error
            ),
            json!({
                "failure": "settings_schema_validation",
                "stage": stage,
                "path_identity": path_identity_json(identity),
                "schema_error": error,
            }),
        )
    })
}

fn checkpoint_creation_failed_result(
    tool_name: &str,
    path_str: &str,
    identity: &FilePathIdentity,
) -> ToolResult {
    file_edit_error_with_data(
        format!(
            "Refusing {tool_name} for '{}': checkpoint creation failed before write, so rollback would be unavailable.",
            path_str
        ),
        json!({
            "failure": "checkpoint_creation_failed",
            "stage": "checkpoint_guard",
            "tool": tool_name,
            "path_identity": path_identity_json(identity),
        }),
    )
}

fn is_priority_agent_settings_path(path: &std::path::Path) -> bool {
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    components.windows(2).any(|window| {
        matches!(
            window,
            [".priority-agent", _] | ["priority-agent", "config.toml"]
        )
    })
}

fn validate_permissions_toml(content: &str) -> Result<(), String> {
    let value =
        toml::from_str::<toml::Value>(content).map_err(|err| format!("invalid TOML: {err}"))?;
    let table = value
        .as_table()
        .ok_or_else(|| "permissions.toml must be a TOML table".to_string())?;

    for key in table.keys() {
        if !matches!(key.as_str(), "always_allow" | "always_deny" | "always_ask") {
            return Err(format!(
                "unsupported permissions key '{}'; expected always_allow, always_deny, or always_ask",
                key
            ));
        }
    }

    for key in ["always_allow", "always_deny", "always_ask"] {
        let Some(value) = table.get(key) else {
            continue;
        };
        let Some(rules) = value.as_array() else {
            return Err(format!("{key} must be an array of rule tables"));
        };
        for (index, rule) in rules.iter().enumerate() {
            let Some(rule_table) = rule.as_table() else {
                return Err(format!("{key}[{index}] must be a table with a pattern"));
            };
            let pattern = rule_table
                .get("pattern")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .trim();
            if pattern.is_empty() {
                return Err(format!("{key}[{index}].pattern must be a non-empty string"));
            }
            if let Some(source) = rule_table.get("source") {
                let source = source
                    .as_str()
                    .ok_or_else(|| format!("{key}[{index}].source must be a string"))?;
                if !matches!(
                    source,
                    "global"
                        | "project"
                        | "user"
                        | "system"
                        | "Global"
                        | "Project"
                        | "User"
                        | "System"
                ) {
                    return Err(format!(
                        "{key}[{index}].source has unsupported value '{source}'"
                    ));
                }
            }
        }
    }

    Ok(())
}

fn occurrence_line_numbers(content: &str, occurrences: &[(usize, usize)]) -> Vec<usize> {
    occurrences
        .iter()
        .take(MAX_MATCH_CONTEXT_OCCURRENCES)
        .map(|(start, _)| content[..*start].matches('\n').count() + 1)
        .collect()
}

fn stale_conflict_json(
    identity: &FilePathIdentity,
    session_id: &str,
    current_content: &str,
    current_mtime: std::time::SystemTime,
    stage: &str,
) -> serde_json::Value {
    let read_state = file_state_snapshot(session_id, &identity.state_key);
    let read_hash = read_state
        .as_ref()
        .map(|state| format!("{:016x}", state.content_hash));
    let mtime_changed = read_state
        .as_ref()
        .map(|state| state.mtime != current_mtime)
        .unwrap_or(false);
    let content_changed = read_state
        .as_ref()
        .map(|state| compute_content_hash(current_content) != state.content_hash)
        .unwrap_or(false);
    json!({
        "failure": "stale_read_conflict",
        "stage": stage,
        "path_identity": path_identity_json(identity),
        "conflict": {
            "read_hash": read_hash,
            "current_hash": content_hash_hex(current_content),
            "mtime_changed": mtime_changed,
            "content_changed": content_changed,
        },
        "recovery": {
            "recommended_action": "re_read_file",
            "next_actions": ["file_read", "regenerate_patch", "retry_file_edit"],
            "allow_stale_read_available": true,
            "allow_stale_read_warning": "Use allow_stale_read=true only for intentional overwrites after reviewing the current file content."
        }
    })
}

fn exact_replace_preflight_error(
    identity: &FilePathIdentity,
    content: &str,
    old_string: &str,
    new_string: &str,
    expected: Option<usize>,
    normalize_whitespace: bool,
) -> Option<ToolResult> {
    let base_data = |failure: &str| {
        json!({
            "failure": failure,
            "path_identity": path_identity_json(identity),
            "operation": "exact_replace",
            "recovery": {
                "recommended_action": "adjust_anchor",
                "next_actions": ["file_read", "use_line_start_line_end", "retry_file_edit"],
            }
        })
    };

    if old_string.trim().is_empty() {
        let message = "old_string cannot be empty or whitespace-only unless insert_after, insert_before, or line_start/line_end is used. For a known target line, use line_start and line_end instead.";
        return Some(file_edit_error_with_data(
            message,
            json!({
                "failure": "empty_old_string",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "recovery": {
                    "recommended_action": "use_line_range",
                    "next_actions": ["file_read", "use_line_start_line_end", "retry_file_edit"],
                }
            }),
        ));
    }

    if old_string == new_string {
        return Some(file_edit_error_with_data(
            "Refusing file_edit no-op: old_string and new_string are identical.",
            json!({
                "failure": "no_op_edit",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "old_hash": content_hash_hex(old_string),
                "new_hash": content_hash_hex(new_string),
                "recovery": {
                    "recommended_action": "change_replacement_or_skip",
                    "next_actions": ["skip_edit", "provide_different_new_string"],
                }
            }),
        ));
    }

    if contains_file_read_line_prefix(old_string) {
        return Some(file_edit_error_with_data(
            file_read_line_prefix_guidance("old_string"),
            json!({
                "failure": "file_read_line_prefix_in_old_string",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "recovery": {
                    "recommended_action": "remove_display_line_prefix",
                    "next_actions": ["copy_text_after_pipe", "use_line_start_line_end", "retry_file_edit"],
                }
            }),
        ));
    }

    let occurrences = if normalize_whitespace {
        find_occurrences_normalized(content, old_string)
    } else {
        find_occurrences(content, old_string)
    };
    let expected_count = expected.unwrap_or(1);
    let max_replacements = max_file_edit_replacements();

    if expected_count > max_replacements {
        return Some(file_edit_error_with_data(
            format!(
                "Refusing file_edit with {} replacement(s): exceeds safety limit {}. Use narrower anchors or set PRIORITY_AGENT_MAX_FILE_EDIT_REPLACEMENTS explicitly for deliberate bulk edits.",
                expected_count, max_replacements
            ),
            json!({
                "failure": "replacement_limit_exceeded",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "expected_replacements": expected_count,
                "max_replacements": max_replacements,
                "recovery": {
                    "recommended_action": "narrow_anchor",
                    "next_actions": ["use_more_specific_old_string", "use_line_start_line_end"],
                }
            }),
        ));
    }

    if occurrences.is_empty() {
        let fuzzy = fuzzy_find_occurrences(content, old_string);
        let mut data = base_data("old_string_not_found");
        data["match_diagnostics"] = json!({
            "expected_occurrences": expected_count,
            "exact_occurrences": 0,
            "fuzzy_occurrences": fuzzy.len(),
            "fuzzy_lines": occurrence_line_numbers(content, &fuzzy),
            "context": if fuzzy.is_empty() {
                serde_json::Value::Null
            } else {
                json!(build_match_context(content, &fuzzy, 2))
            },
        });
        data["recovery"]["recommended_action"] = if fuzzy.is_empty() {
            json!("re_read_once_then_line_range_edit")
        } else {
            json!("copy_exact_fuzzy_match")
        };
        let message = if fuzzy.is_empty() {
            "Could not find old_string in file. Make sure it matches exactly (including whitespace).".to_string()
        } else {
            format!(
                "old_string not found exactly, but fuzzy matches found:\n{}\n\nPlease adjust old_string to match one of these occurrences precisely.",
                build_match_context(content, &fuzzy, 2)
            )
        };
        return Some(file_edit_error_with_data(message, data));
    }

    if occurrences.len() != expected_count {
        let ctx = build_match_context(content, &occurrences, 2);
        return Some(file_edit_error_with_data(
            format!(
                "Expected {} occurrence(s) of old_string, but found {}.\n{}\n\nPlease provide a more specific old_string or set expected_replacements to {}.",
                expected_count,
                occurrences.len(),
                ctx,
                occurrences.len()
            ),
            json!({
                "failure": "old_string_occurrence_mismatch",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "match_diagnostics": {
                    "expected_occurrences": expected_count,
                    "actual_occurrences": occurrences.len(),
                    "lines": occurrence_line_numbers(content, &occurrences),
                    "context": ctx,
                },
                "recovery": {
                    "recommended_action": "narrow_anchor",
                    "next_actions": ["use_more_specific_old_string", "use_line_start_line_end", "set_expected_replacements_if_intentional"],
                    "safe_expected_replacements": occurrences.len(),
                }
            }),
        ));
    }

    None
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
        "Apply a SEARCH/REPLACE edit to an existing file. \
         You MUST call file_read on this path first — the tool refuses otherwise, \
         since SEARCH must match on-disk bytes exactly. \
         \
         `old_string` is whitespace-sensitive plain text (no regex) and must be \
         UNIQUE in the file; otherwise the edit is refused to avoid surprise rewrites. \
         Do NOT include file_read display prefixes like `12 |`; those are not file content. \
         \
         If you're unsure about exact whitespace, use line_start + line_end instead: \
         set both (1-indexed, inclusive) and provide new_string for a reliable range replace. \
         For coordinated changes across multiple files, use file_patch instead."
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
                    "description": "How many times old_string or an insert anchor should appear. Defaults to 1. Use values greater than 1 only for deliberate mass edits.",
                    "minimum": 1
                },
                "insert_after": {
                    "type": "string",
                    "description": "If provided, new_string will be inserted after this anchor (old_string is ignored when this is set). The anchor must appear expected_replacements times, default 1."
                },
                "insert_before": {
                    "type": "string",
                    "description": "If provided, new_string will be inserted before this anchor (old_string is ignored when this is set). The anchor must appear expected_replacements times, default 1."
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

    fn aliases(&self) -> &'static [&'static str] {
        &["edit"]
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("replace insert file text")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Edit
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let path = params["path"].as_str()?.trim();
        if path.is_empty() {
            return None;
        }
        let mode = if params["old_string"].as_str().is_some() {
            "replace"
        } else if params["line_start"].as_u64().is_some() {
            "line_range"
        } else if params["insert_after"].as_str().is_some() {
            "insert_after"
        } else if params["insert_before"].as_str().is_some() {
            "insert_before"
        } else {
            "edit"
        };
        Some(format!("{path} ({mode})"))
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
        let identity = file_path_identity(path_str, &path, &context.working_dir);
        if let Some(result) =
            high_risk_file_target_result(&path, &identity, &context.working_dir, "file_edit")
        {
            return result;
        }
        let state_key = identity.state_key.clone();
        let file_guard = acquire_file_mutation_lock(&state_key).await;

        // 读取文件内容
        if let Err(msg) = check_file_size_limit(&path, "edit") {
            return ToolResult::error(msg);
        }
        let snapshot = match read_text_file(&path, "edit").await {
            Ok(snapshot) => snapshot,
            Err(e) => {
                return ToolResult::error(e);
            }
        };
        let content = snapshot.content.clone();

        // ── Edit safety checks ────────────────────────────────────────
        // Claude-like write discipline: existing files must be read in this
        // session before mutation so stale/partial context cannot silently win.
        if !allow_edit_without_read() {
            let status =
                read_before_edit_status(&context.session_id, &state_key, line_start, line_end);
            if status != ReadBeforeEditStatus::Allowed {
                return ToolResult::error(file_read_state_guidance(path_str, status));
            }
            // ReadTracker — simpler path-level guard, cleared on context fold.
            if let Some(ref tracker) = context.read_tracker {
                if let Err(msg) = tracker.check_edit(&path, old_string) {
                    return ToolResult::error(msg);
                }
            }
        }

        // 2. 文件修改检测：检查文件是否在读取后被外部修改
        let current_mtime = std::fs::metadata(&path)
            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if is_file_modified_since_read(&context.session_id, &state_key, &content, current_mtime) {
            if !allow_stale_read {
                let message = format!(
                    "Refusing file_edit for '{}': file changed since this session last read it. Re-read the file and retry, or set allow_stale_read=true if this overwrite is intentional.",
                    path_str
                );
                return file_edit_error_with_data(
                    message,
                    stale_conflict_json(
                        &identity,
                        &context.session_id,
                        &content,
                        current_mtime,
                        "pre_write_stale_check",
                    ),
                );
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
                normalize_text_line_endings(&desanitize(&normalize_quotes(old_string))),
                normalize_text_line_endings(&desanitize(&normalize_quotes(new_string))),
            )
        } else {
            (
                normalize_text_line_endings(old_string),
                normalize_text_line_endings(new_string),
            )
        };

        // 确定操作模式
        let using_exact_replace = line_start.is_none()
            && line_end.is_none()
            && insert_after.is_none()
            && insert_before.is_none();
        if using_exact_replace {
            if let Some(result) = exact_replace_preflight_error(
                &identity,
                &content,
                &old_string,
                &new_string,
                expected_replacements,
                normalize_ws,
            ) {
                return result;
            }
        }

        let result = if let (Some(start), Some(end)) = (line_start, line_end) {
            Self::do_replace_lines(content, start, end, &new_string)
        } else if let Some(after) = insert_after {
            Self::do_insert(
                content,
                after,
                &new_string,
                InsertMode::After,
                expected_replacements,
            )
        } else if let Some(before) = insert_before {
            Self::do_insert(
                content,
                before,
                &new_string,
                InsertMode::Before,
                expected_replacements,
            )
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
                if let Some(result) = priority_agent_settings_validation_error(
                    &identity,
                    &new_content,
                    "schema_guard",
                ) {
                    return result;
                }
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
                let checkpoint = match create_file_checkpoint(&context, "file_edit", &path).await {
                    Some(checkpoint) => checkpoint,
                    None => {
                        return checkpoint_creation_failed_result("file_edit", path_str, &identity)
                    }
                };
                let diagnostics_before =
                    collect_file_edit_diagnostics(&context, &path, &snapshot.content).await;
                let before_write_snapshot = match read_text_file(&path, "verify before edit").await
                {
                    Ok(snapshot) => snapshot,
                    Err(e) => return ToolResult::error(e),
                };
                let before_write_mtime = std::fs::metadata(&path)
                    .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                if before_write_mtime != current_mtime
                    || before_write_snapshot.content.as_str() != snapshot.content.as_str()
                {
                    let message = format!(
                        "Refusing file_edit for '{}': file changed while this edit was being prepared. Re-read the file and retry.",
                        path_str
                    );
                    return file_edit_error_with_data(
                        message,
                        stale_conflict_json(
                            &identity,
                            &context.session_id,
                            &before_write_snapshot.content,
                            before_write_mtime,
                            "pre_write_race_check",
                        ),
                    );
                }
                let diff_summary =
                    edit_diff_summary(&identity.display_path, &snapshot.content, &new_content);
                match write_text_file(
                    &path,
                    &new_content,
                    snapshot.encoding,
                    snapshot.has_bom,
                    snapshot.line_ending,
                    MAX_EDITABLE_FILE_SIZE_BYTES,
                )
                .await
                {
                    Ok(bytes_written) => {
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
                        drop(file_guard);
                        let file_change = record_file_change(
                            &context,
                            FileChangeRequest {
                                checkpoint: Some(&checkpoint),
                                tool_name: "file_edit",
                                path: &path,
                                existed_before: true,
                                before_content: Some(snapshot.content.as_str()),
                                after_content: &new_content,
                                diff: &diff_summary,
                                bytes_written: bytes_written as u64,
                            },
                        )
                        .await;
                        let diagnostics =
                            collect_file_edit_diagnostics(&context, &path, &new_content).await;
                        let diagnostics_delta =
                            file_edit_diagnostics_delta(&diagnostics_before, &diagnostics);
                        let diagnostics_line = file_edit_diagnostics_content_line(&diagnostics);
                        let checkpoint_json = checkpoint_metadata_json(Some(&checkpoint));
                        let file_change_json = file_change.unwrap_or(serde_json::Value::Null);
                        let text_format = text_write_format_json(
                            snapshot.encoding,
                            snapshot.has_bom,
                            snapshot.line_ending,
                        );
                        let edit_preview = edit_preview_json(
                            &identity,
                            true,
                            Some(snapshot.content.as_str()),
                            &new_content,
                            &diff_summary,
                            text_format.clone(),
                            checkpoint_json.clone(),
                            file_change_json.clone(),
                            Some(replacements),
                            bytes_written as u64,
                            "edit_complete",
                        );
                        let data = json!({
                            "path": path_str,
                            "resolved_path": identity.resolved_path,
                            "path_identity": path_identity_json(&identity),
                            "replacements": replacements,
                            "bytes_written": bytes_written,
                            "text_format": text_format,
                            "checkpoint": checkpoint_json,
                            "file_change": file_change_json,
                            "diff": edit_diff_summary_json(&diff_summary),
                            "edit_preview": edit_preview,
                            "diagnostics_before": diagnostics_before,
                            "diagnostics": diagnostics.clone(),
                            "diagnostics_after": diagnostics,
                            "diagnostics_delta": diagnostics_delta,
                        });
                        let mut content = format!(
                            "File edited successfully: {} ({} replacement(s))",
                            path_str, replacements
                        );
                        if let Some(line) = diagnostics_line {
                            content.push('\n');
                            content.push_str(&line);
                        }
                        ToolResult::success_with_data(content, data)
                    }
                    Err(e) => ToolResult::error(e),
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
            Self::do_insert(
                original.to_string(),
                after,
                new_string,
                InsertMode::After,
                expected_replacements,
            )
            .map(|(s, _)| s)
        } else if let Some(before) = insert_before {
            Self::do_insert(
                original.to_string(),
                before,
                new_string,
                InsertMode::Before,
                expected_replacements,
            )
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
        if old_string == new_string {
            return Err(
                "Refusing file_edit no-op: old_string and new_string are identical.".to_string(),
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
        expected: Option<usize>,
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
        let expected_count = expected.unwrap_or(1);
        if count != expected_count {
            let field = match mode {
                InsertMode::After => "insert_after",
                InsertMode::Before => "insert_before",
            };
            let ctx = build_match_context(&content, &occurrences, 2);
            return Err(format!(
                "Expected {} occurrence(s) of {} anchor, but found {}.\n{}\n\nPlease provide a more specific anchor or set expected_replacements to {} if this bulk insert is intentional.",
                expected_count, field, count, ctx, count
            ));
        }
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

#[cfg(test)]
mod tests;
