//! File tool support module.
//!
//! Separates read, write, edit matching, path policy, and mutation history from the file tool entrypoint.

use super::*;

#[derive(Clone, Debug, Default)]
pub(super) struct FileReadRecord {
    full_read: bool,
    ranges: Vec<(usize, usize)>,
}

/// 文件修改状态跟踪（用于检测外部修改）
#[derive(Clone, Debug)]
pub(super) struct FileState {
    pub(super) mtime: std::time::SystemTime,
    pub(super) content_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadBeforeEditStatus {
    Allowed,
    NotRead,
    PartialOnly,
}

#[derive(Debug, Default)]
pub(super) struct FileStateTracker {
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

    pub(super) fn read_before_edit_status(
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

pub(super) fn tracker_key(session_id: &str, file_path: &str) -> String {
    format!("{}:{}", session_id, file_path)
}

pub(super) async fn acquire_file_mutation_lock(
    state_key: &str,
) -> tokio::sync::OwnedMutexGuard<()> {
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
pub(super) fn read_before_edit_enabled() -> bool {
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

pub(super) fn read_before_edit_status(
    session_id: &str,
    file_path: &str,
    line_start: Option<usize>,
    line_end: Option<usize>,
) -> ReadBeforeEditStatus {
    let tracker = FILE_STATE_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    tracker.read_before_edit_status(session_id, file_path, line_start, line_end)
}

pub(super) fn file_state_snapshot(session_id: &str, file_path: &str) -> Option<FileState> {
    let tracker = FILE_STATE_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    tracker
        .file_states
        .get(&tracker_key(session_id, file_path))
        .cloned()
}

pub(super) fn file_read_state_guidance(path: &str, status: ReadBeforeEditStatus) -> String {
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

pub(super) async fn read_directory_result(
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

pub(super) fn compute_content_hash(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn content_hash_hex(content: &str) -> String {
    format!("{:016x}", compute_content_hash(content))
}

pub(super) fn file_read_content_preview<'a, I>(lines: I) -> Option<String>
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
pub(super) fn edit_preview_json(
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

pub(super) fn file_state_key(path: &Path) -> String {
    canonicalize_or_normalize(path)
        .to_string_lossy()
        .to_string()
}

pub(super) fn file_path_identity(
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

pub(super) fn relative_display_path(path: &Path, root: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| path.to_string_lossy().to_string())
}

pub(super) fn path_identity_json(identity: &FilePathIdentity) -> serde_json::Value {
    json!({
        "lexical_path": identity.lexical_path,
        "resolved_path": identity.resolved_path,
        "canonical_path": identity.canonical_path,
        "display_path": identity.display_path,
        "state_key": identity.state_key,
    })
}

pub(super) fn json_line_number(value: usize) -> serde_json::Value {
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

pub(super) fn mark_file_read_with_state_and_coverage(
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
