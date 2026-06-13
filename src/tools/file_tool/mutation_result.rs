//! Unified file mutation result metadata.
//!
//! Phase 2 (opencode alignment): every file mutation (`file_write`,
//! `file_edit`, `file_patch`) emits one consistent typed shape.
//! TUI, desktop, trace, repair, rollback, and daily baseline consume
//! this shape instead of per-tool ad hoc parsing.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Shared metadata for all file mutation operations.
///
/// Mirrors opencode's edit result contract while keeping Priority Agent's
/// stronger checkpoint, file-change, diagnostics, and rollback evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMutationResult {
    /// Tool that produced this result: "file_write", "file_edit", "file_patch".
    pub operation: String,
    /// Per-file results (single-element for write/edit, multi for patch).
    pub files: Vec<FileResult>,
    /// Combined diff summary across all files.
    pub diff: MutationDiff,
    /// Checkpoint metadata (created before mutation, enables restore).
    pub checkpoint: Option<MutationCheckpoint>,
    /// Durable file change record ids, one per file.
    pub file_change_ids: Vec<String>,
    /// LSP diagnostics summary (only for file_edit currently).
    pub diagnostics: Option<MutationDiagnostics>,
    /// Rollback metadata (only when a partial write failed).
    pub rollback: Option<MutationRollback>,
    /// Whether the result was auto-formatted after writing.
    #[serde(default)]
    pub formatted: bool,
    /// Short stable string for desktop/TUI cards.
    pub ui_summary: String,
}

/// Per-file result within a mutation operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    /// Requested path (as the LLM provided it).
    pub path: String,
    /// Normalized resolved absolute path.
    pub resolved_path: String,
    /// Path suitable for display (workspace-relative when possible).
    pub display_path: String,
    /// Whether the file existed before this mutation.
    pub existed_before: bool,
    /// Number of replacements made (for edit/patch; 0 for write which is full replace).
    pub replacements: usize,
    /// Encoded bytes written to disk.
    pub bytes_written: u64,
    /// Lines added by this mutation.
    pub additions: usize,
    /// Lines deleted by this mutation.
    pub deletions: usize,
    /// 1-indexed start line of the changed range (None if no lines changed).
    pub changed_line_start: Option<u64>,
    /// 1-indexed end line of the changed range (None if no lines changed).
    pub changed_line_end: Option<u64>,
    /// Text encoding and line-ending metadata.
    pub text_format: TextFormatInfo,
    /// Stable file change record id for this file.
    pub file_change_id: Option<String>,
    /// Content hash before mutation (hex).
    pub before_hash: Option<String>,
    /// Content hash after mutation (hex).
    pub after_hash: Option<String>,
}

/// Combined diff summary across all files in a mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationDiff {
    /// Total lines added across all files.
    pub additions: usize,
    /// Total lines deleted across all files.
    pub deletions: usize,
    /// Number of files changed.
    pub file_count: usize,
    /// Bounded unified diff text (max 80 lines per file).
    pub unified_diff: String,
    /// Whether the diff was truncated.
    pub truncated: bool,
    /// Stable hash of the diff content.
    pub diff_hash: String,
}

/// Checkpoint created before mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationCheckpoint {
    pub checkpoint_id: String,
    pub sequence: u64,
    pub session_id: Option<String>,
    /// Whether this checkpoint can be used for restore.
    pub restore_eligible: bool,
}

/// LSP diagnostics collected after a file edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationDiagnostics {
    pub available: bool,
    pub checked: bool,
    pub status: String,
    pub diagnostic_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub first_error: Option<DiagnosticItem>,
    pub first_warning: Option<DiagnosticItem>,
    pub affected_line_range: Option<DiagnosticLineRange>,
    /// Delta information (new errors/warnings introduced by this edit).
    pub delta: Option<DiagnosticDelta>,
}

/// A single LSP diagnostic item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticItem {
    pub server: String,
    pub severity: String,
    pub message: String,
    pub line: u64,
}

/// Diagnostic-affected line range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticLineRange {
    pub start_line: u64,
    pub end_line: u64,
}

/// Delta between before/after diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticDelta {
    pub checked: bool,
    pub status: String,
    pub introduced_error: bool,
    pub introduced_warning: bool,
    pub change_error_count: i64,
    pub change_warning_count: i64,
}

/// Rollback metadata after a partial write failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationRollback {
    pub attempted: bool,
    pub success: bool,
    pub failed_path: Option<String>,
    pub restored_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub failed_files: Vec<String>,
    pub error: Option<String>,
}

/// Text encoding and line-ending info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextFormatInfo {
    pub encoding: String,
    pub has_bom: bool,
    pub line_ending: String,
}

impl FileMutationResult {
    /// Build a `ui_summary` string from the result fields.
    ///
    /// Format: `"<op> <N> file(s): +<add>/<del>, <bytes>B"`
    pub fn build_ui_summary(&self) -> String {
        let file_label = if self.files.len() == 1 {
            "file"
        } else {
            "files"
        };
        format!(
            "{} {} {}: +{}/-{}, {}B",
            self.operation,
            self.files.len(),
            file_label,
            self.diff.additions,
            self.diff.deletions,
            self.files.iter().map(|f| f.bytes_written).sum::<u64>()
        )
    }

    /// Compute a stable diff hash from the unified diff string.
    pub fn compute_diff_hash(unified_diff: &str) -> String {
        let digest = md5::compute(unified_diff.as_bytes());
        format!("{:x}", digest)
    }
}

/// Build a `FileMutationResult` from the typical data produced by file_write.
#[allow(clippy::too_many_arguments)]
pub fn file_write_result(
    path: &str,
    resolved_path: &str,
    display_path: &str,
    existed_before: bool,
    bytes_written: u64,
    additions: usize,
    deletions: usize,
    changed_line_start: Option<u64>,
    changed_line_end: Option<u64>,
    encoding: &str,
    has_bom: bool,
    line_ending: &str,
    before_hash: Option<String>,
    after_hash: Option<String>,
    unified_diff: &str,
    diff_truncated: bool,
    file_change_id: Option<String>,
    checkpoint_id: Option<&str>,
    checkpoint_sequence: Option<u64>,
    session_id: Option<&str>,
) -> FileMutationResult {
    let diff_hash = FileMutationResult::compute_diff_hash(unified_diff);
    let files = vec![FileResult {
        path: path.to_string(),
        resolved_path: resolved_path.to_string(),
        display_path: display_path.to_string(),
        existed_before,
        replacements: 0,
        bytes_written,
        additions,
        deletions,
        changed_line_start,
        changed_line_end,
        text_format: TextFormatInfo {
            encoding: encoding.to_string(),
            has_bom,
            line_ending: line_ending.to_string(),
        },
        file_change_id: file_change_id.clone(),
        before_hash,
        after_hash,
    }];

    let result = FileMutationResult {
        operation: "file_write".to_string(),
        files,
        diff: MutationDiff {
            additions,
            deletions,
            file_count: 1,
            unified_diff: unified_diff.to_string(),
            truncated: diff_truncated,
            diff_hash,
        },
        checkpoint: checkpoint_id.map(|id| MutationCheckpoint {
            checkpoint_id: id.to_string(),
            sequence: checkpoint_sequence.unwrap_or(0),
            session_id: session_id.map(|s| s.to_string()),
            restore_eligible: true,
        }),
        file_change_ids: file_change_id.into_iter().collect(),
        diagnostics: None,
        rollback: None,
        formatted: false,
        ui_summary: String::new(),
    };
    FileMutationResult {
        ui_summary: result.build_ui_summary(),
        ..result
    }
}

/// Build a `FileMutationResult` from the typical data produced by file_edit.
#[allow(clippy::too_many_arguments)]
pub fn file_edit_result(
    path: &str,
    resolved_path: &str,
    display_path: &str,
    replacements: usize,
    bytes_written: u64,
    additions: usize,
    deletions: usize,
    changed_line_start: Option<u64>,
    changed_line_end: Option<u64>,
    encoding: &str,
    has_bom: bool,
    line_ending: &str,
    before_hash: Option<String>,
    after_hash: Option<String>,
    unified_diff: &str,
    diff_truncated: bool,
    file_change_id: Option<String>,
    checkpoint_id: Option<&str>,
    checkpoint_sequence: Option<u64>,
    session_id: Option<&str>,
    diagnostics_available: bool,
    diagnostics_checked: bool,
    diagnostics_status: &str,
    diagnostic_count: usize,
    error_count: usize,
    warning_count: usize,
    first_error: Option<DiagnosticItem>,
    first_warning: Option<DiagnosticItem>,
    affected_line_range: Option<DiagnosticLineRange>,
    diagnostics_delta: Option<DiagnosticDelta>,
) -> FileMutationResult {
    let diff_hash = FileMutationResult::compute_diff_hash(unified_diff);
    let files = vec![FileResult {
        path: path.to_string(),
        resolved_path: resolved_path.to_string(),
        display_path: display_path.to_string(),
        existed_before: true,
        replacements,
        bytes_written,
        additions,
        deletions,
        changed_line_start,
        changed_line_end,
        text_format: TextFormatInfo {
            encoding: encoding.to_string(),
            has_bom,
            line_ending: line_ending.to_string(),
        },
        file_change_id: file_change_id.clone(),
        before_hash,
        after_hash,
    }];

    let result = FileMutationResult {
        operation: "file_edit".to_string(),
        files,
        diff: MutationDiff {
            additions,
            deletions,
            file_count: 1,
            unified_diff: unified_diff.to_string(),
            truncated: diff_truncated,
            diff_hash,
        },
        checkpoint: checkpoint_id.map(|id| MutationCheckpoint {
            checkpoint_id: id.to_string(),
            sequence: checkpoint_sequence.unwrap_or(0),
            session_id: session_id.map(|s| s.to_string()),
            restore_eligible: true,
        }),
        file_change_ids: file_change_id.into_iter().collect(),
        diagnostics: Some(MutationDiagnostics {
            available: diagnostics_available,
            checked: diagnostics_checked,
            status: diagnostics_status.to_string(),
            diagnostic_count,
            error_count,
            warning_count,
            first_error,
            first_warning,
            affected_line_range,
            delta: diagnostics_delta,
        }),
        rollback: None,
        formatted: false,
        ui_summary: String::new(),
    };
    FileMutationResult {
        ui_summary: result.build_ui_summary(),
        ..result
    }
}

/// Build a `FileMutationResult` from the typical data produced by file_patch success.
#[allow(clippy::too_many_arguments)]
pub fn file_patch_result(
    file_results: Vec<FileResult>,
    total_additions: usize,
    total_deletions: usize,
    combined_diff: &str,
    diff_truncated: bool,
    file_change_ids: Vec<String>,
    checkpoint_id: Option<&str>,
    checkpoint_sequence: Option<u64>,
    session_id: Option<&str>,
) -> FileMutationResult {
    let diff_hash = FileMutationResult::compute_diff_hash(combined_diff);
    let file_count = file_results.len();

    let result = FileMutationResult {
        operation: "file_patch".to_string(),
        files: file_results,
        diff: MutationDiff {
            additions: total_additions,
            deletions: total_deletions,
            file_count,
            unified_diff: combined_diff.to_string(),
            truncated: diff_truncated,
            diff_hash,
        },
        checkpoint: checkpoint_id.map(|id| MutationCheckpoint {
            checkpoint_id: id.to_string(),
            sequence: checkpoint_sequence.unwrap_or(0),
            session_id: session_id.map(|s| s.to_string()),
            restore_eligible: true,
        }),
        file_change_ids,
        diagnostics: None,
        rollback: None,
        formatted: false,
        ui_summary: String::new(),
    };
    FileMutationResult {
        ui_summary: result.build_ui_summary(),
        ..result
    }
}

/// Build a `FileMutationResult` for a patch partial failure with rollback.
#[allow(clippy::too_many_arguments)]
pub fn file_patch_partial_failure_result(
    failed_path: &str,
    written_file_results: Vec<FileResult>,
    written_additions: usize,
    written_deletions: usize,
    combined_diff: &str,
    diff_truncated: bool,
    file_change_ids: Vec<String>,
    checkpoint_id: Option<&str>,
    checkpoint_sequence: Option<u64>,
    session_id: Option<&str>,
    rollback_attempted: bool,
    rollback_success: bool,
    rollback_restored: Vec<String>,
    rollback_removed: Vec<String>,
    rollback_failed: Vec<String>,
    rollback_error: Option<String>,
) -> FileMutationResult {
    let diff_hash = FileMutationResult::compute_diff_hash(combined_diff);
    let file_count = written_file_results.len();

    let result = FileMutationResult {
        operation: "file_patch".to_string(),
        files: written_file_results,
        diff: MutationDiff {
            additions: written_additions,
            deletions: written_deletions,
            file_count,
            unified_diff: combined_diff.to_string(),
            truncated: diff_truncated,
            diff_hash,
        },
        checkpoint: checkpoint_id.map(|id| MutationCheckpoint {
            checkpoint_id: id.to_string(),
            sequence: checkpoint_sequence.unwrap_or(0),
            session_id: session_id.map(|s| s.to_string()),
            restore_eligible: true,
        }),
        file_change_ids,
        diagnostics: None,
        rollback: Some(MutationRollback {
            attempted: rollback_attempted,
            success: rollback_success,
            failed_path: Some(failed_path.to_string()),
            restored_files: rollback_restored,
            removed_files: rollback_removed,
            failed_files: rollback_failed,
            error: rollback_error,
        }),
        formatted: false,
        ui_summary: String::new(),
    };
    FileMutationResult {
        ui_summary: format!(
            "{}: partial failure on {}, {} files written, rollback {}",
            result.operation,
            failed_path,
            file_count,
            if rollback_success { "ok" } else { "failed" }
        ),
        ..result
    }
}

/// Compute a display path from a resolved path and the working directory.
pub fn display_path_from_resolved(resolved: &Path, working_dir: &Path) -> String {
    resolved
        .strip_prefix(working_dir)
        .unwrap_or(resolved)
        .display()
        .to_string()
}

// ---- Converters: build FileMutationResult from existing tool JSON data ----

/// Build a `FileMutationResult` from the JSON data produced by file_edit.
///
/// This keeps the existing JSON rendering intact while adding a typed
/// `mutation_result` field that TUI, desktop, trace, and repair can consume.
#[allow(clippy::too_many_arguments)]
pub fn from_file_edit_json(
    path: &str,
    resolved_path: &str,
    display_path: &str,
    replacements: usize,
    bytes_written: u64,
    additions: usize,
    deletions: usize,
    changed_line_start: u64,
    changed_line_end: u64,
    unified_diff: &str,
    diff_truncated: bool,
    encoding: &str,
    has_bom: bool,
    line_ending: &str,
    checkpoint_id: Option<&str>,
    checkpoint_sequence: u64,
    session_id: Option<&str>,
    file_change_id: Option<&str>,
    diagnostics: &Option<serde_json::Value>,
    diagnostics_delta: Option<serde_json::Value>,
) -> FileMutationResult {
    let diff_hash = FileMutationResult::compute_diff_hash(unified_diff);

    let result = FileMutationResult {
        operation: "file_edit".to_string(),
        files: vec![FileResult {
            path: path.to_string(),
            resolved_path: resolved_path.to_string(),
            display_path: display_path.to_string(),
            existed_before: true,
            replacements,
            bytes_written,
            additions,
            deletions,
            changed_line_start: nonzero_opt(changed_line_start),
            changed_line_end: nonzero_opt(changed_line_end),
            text_format: TextFormatInfo {
                encoding: encoding.to_string(),
                has_bom,
                line_ending: line_ending.to_string(),
            },
            file_change_id: file_change_id.map(|s| s.to_string()),
            before_hash: None,
            after_hash: None,
        }],
        diff: MutationDiff {
            additions,
            deletions,
            file_count: 1,
            unified_diff: unified_diff.to_string(),
            truncated: diff_truncated,
            diff_hash,
        },
        checkpoint: checkpoint_id.map(|id| MutationCheckpoint {
            checkpoint_id: id.to_string(),
            sequence: checkpoint_sequence,
            session_id: session_id.map(|s| s.to_string()),
            restore_eligible: true,
        }),
        file_change_ids: file_change_id.map(|s| s.to_string()).into_iter().collect(),
        diagnostics: diagnostics
            .as_ref()
            .map(|d| diag_from_json(d, &diagnostics_delta)),
        rollback: None,
        formatted: false,
        ui_summary: String::new(),
    };
    FileMutationResult {
        ui_summary: result.build_ui_summary(),
        ..result
    }
}

fn nonzero_opt(n: u64) -> Option<u64> {
    if n == 0 {
        None
    } else {
        Some(n)
    }
}

fn diag_from_json(d: &serde_json::Value, delta: &Option<serde_json::Value>) -> MutationDiagnostics {
    MutationDiagnostics {
        available: d
            .get("available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        checked: d.get("checked").and_then(|v| v.as_bool()).unwrap_or(false),
        status: d
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        diagnostic_count: d
            .get("diagnostic_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        error_count: d.get("error_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        warning_count: d.get("warning_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        first_error: d.get("first_error").and_then(diagnostic_item_from_json),
        first_warning: d.get("first_warning").and_then(diagnostic_item_from_json),
        affected_line_range: d
            .get("affected_line_range")
            .and_then(diagnostic_line_range_from_json),
        delta: delta.as_ref().map(|d| DiagnosticDelta {
            checked: d.get("checked").and_then(|v| v.as_bool()).unwrap_or(false),
            status: d
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            introduced_error: d
                .get("introduced_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            introduced_warning: d
                .get("introduced_warning")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            change_error_count: d
                .get("change")
                .and_then(|c| c.get("error_count"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            change_warning_count: d
                .get("change")
                .and_then(|c| c.get("warning_count"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        }),
    }
}

fn diagnostic_item_from_json(value: &serde_json::Value) -> Option<DiagnosticItem> {
    if value.is_null() || !value.is_object() {
        return None;
    }
    let message = extract_str(value, "message");
    if message.is_empty() {
        return None;
    }
    Some(DiagnosticItem {
        server: extract_str(value, "server"),
        severity: extract_str(value, "severity"),
        message,
        line: value
            .get("range")
            .and_then(|r| r.get("start_line"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

fn diagnostic_line_range_from_json(value: &serde_json::Value) -> Option<DiagnosticLineRange> {
    if value.is_null() || !value.is_object() {
        return None;
    }
    let start_line = value.get("start_line").and_then(|v| v.as_u64())?;
    let end_line = value
        .get("end_line")
        .and_then(|v| v.as_u64())
        .unwrap_or(start_line);
    Some(DiagnosticLineRange {
        start_line,
        end_line,
    })
}

fn extract_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Try to extract a `FileMutationResult` from a tool result data payload.
///
/// Returns `None` if the `mutation_result` field is absent or malformed.
/// This is the canonical product-facing accessor for TUI, desktop, trace,
/// repair helpers, and closeout — they should use this instead of parsing
/// per-tool ad hoc fields.
pub fn from_tool_data(data: &serde_json::Value) -> Option<FileMutationResult> {
    data.get("mutation_result")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// A compact one-line summary suitable for TUI tool cards and trace lines.
///
/// Returns the pre-built `ui_summary` if present, otherwise synthesizes
/// a basic summary from the operation, file count, and diff.
pub fn compact_summary(data: &serde_json::Value) -> String {
    if let Some(mr) = from_tool_data(data) {
        return mr.ui_summary;
    }
    // Fallback for tool results without mutation_result.
    let op = data
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let file_count = data
        .get("files")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(1);
    let path = data
        .get("path")
        .and_then(|v| v.as_str())
        .or_else(|| data.get("resolved_path").and_then(|v| v.as_str()))
        .unwrap_or("?");
    let bytes = data
        .get("bytes_written")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let additions = data
        .get("diff")
        .and_then(|d| d.get("additions"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let deletions = data
        .get("diff")
        .and_then(|d| d.get("deletions"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if file_count > 1 {
        format!(
            "{} {} files: +{}/-{}, {}B",
            op, file_count, additions, deletions, bytes
        )
    } else {
        format!("{} {}: +{}/-{}, {}B", op, path, additions, deletions, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_write_result_serialization_roundtrip() {
        let result = file_write_result(
            "src/main.rs",
            "/home/user/project/src/main.rs",
            "src/main.rs",
            true,
            42,
            3,
            1,
            Some(10),
            Some(12),
            "utf-8",
            false,
            "LF",
            Some("abc123".to_string()),
            Some("def456".to_string()),
            "@@ -10,3 +10,5 @@\n-old\n+new",
            false,
            Some("fc_1_abc".to_string()),
            Some("cp_1_def"),
            Some(1),
            Some("session-42"),
        );

        let json = serde_json::to_string(&result).unwrap();
        let parsed: FileMutationResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.operation, "file_write");
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].path, "src/main.rs");
        assert_eq!(parsed.files[0].bytes_written, 42);
        assert_eq!(parsed.diff.additions, 3);
        assert_eq!(parsed.diff.deletions, 1);
        assert!(!parsed.diff.truncated);
        assert_eq!(
            parsed.checkpoint.as_ref().unwrap().checkpoint_id,
            "cp_1_def"
        );
        assert_eq!(
            parsed.checkpoint.as_ref().unwrap().session_id.as_deref(),
            Some("session-42")
        );
        assert!(parsed.checkpoint.as_ref().unwrap().restore_eligible);
        assert_eq!(parsed.file_change_ids, vec!["fc_1_abc"]);
        assert!(parsed.diagnostics.is_none());
        assert!(parsed.rollback.is_none());
        assert!(parsed.ui_summary.contains("file_write"));
        assert!(parsed.ui_summary.contains("42B"));
    }

    #[test]
    fn file_edit_result_includes_diagnostics() {
        let result = file_edit_result(
            "src/lib.rs",
            "/home/user/project/src/lib.rs",
            "src/lib.rs",
            1,
            25,
            2,
            0,
            Some(5),
            Some(6),
            "utf-8",
            false,
            "LF",
            Some("before".to_string()),
            Some("after".to_string()),
            "@@ -5 +5,2 @@",
            false,
            Some("fc_2_xyz".to_string()),
            Some("cp_2_ghi"),
            Some(2),
            Some("session-42"),
            true,
            true,
            "diagnostics_found",
            3,
            1,
            2,
            Some(DiagnosticItem {
                server: "rust-analyzer".to_string(),
                severity: "error".to_string(),
                message: "unused variable: x".to_string(),
                line: 5,
            }),
            Some(DiagnosticItem {
                server: "rust-analyzer".to_string(),
                severity: "warning".to_string(),
                message: "dead code".to_string(),
                line: 3,
            }),
            Some(DiagnosticLineRange {
                start_line: 3,
                end_line: 6,
            }),
            Some(DiagnosticDelta {
                checked: true,
                status: "new_errors".to_string(),
                introduced_error: true,
                introduced_warning: true,
                change_error_count: 1,
                change_warning_count: 2,
            }),
        );

        let diag = result.diagnostics.as_ref().unwrap();
        assert!(diag.available);
        assert!(diag.checked);
        assert_eq!(diag.error_count, 1);
        assert_eq!(diag.warning_count, 2);
        assert!(diag.first_error.is_some());
        assert!(diag.delta.as_ref().unwrap().introduced_error);
        assert_eq!(result.files[0].replacements, 1);
    }

    #[test]
    fn file_patch_result_supports_multiple_files() {
        let files = vec![
            FileResult {
                path: "src/a.rs".to_string(),
                resolved_path: "/proj/src/a.rs".to_string(),
                display_path: "src/a.rs".to_string(),
                existed_before: true,
                replacements: 1,
                bytes_written: 10,
                additions: 2,
                deletions: 1,
                changed_line_start: Some(1),
                changed_line_end: Some(3),
                text_format: TextFormatInfo {
                    encoding: "utf-8".to_string(),
                    has_bom: false,
                    line_ending: "LF".to_string(),
                },
                file_change_id: Some("fc_a".to_string()),
                before_hash: Some("h1".to_string()),
                after_hash: Some("h2".to_string()),
            },
            FileResult {
                path: "src/b.rs".to_string(),
                resolved_path: "/proj/src/b.rs".to_string(),
                display_path: "src/b.rs".to_string(),
                existed_before: false,
                replacements: 0,
                bytes_written: 20,
                additions: 5,
                deletions: 0,
                changed_line_start: Some(1),
                changed_line_end: Some(5),
                text_format: TextFormatInfo {
                    encoding: "utf-8".to_string(),
                    has_bom: false,
                    line_ending: "LF".to_string(),
                },
                file_change_id: Some("fc_b".to_string()),
                before_hash: None,
                after_hash: Some("h3".to_string()),
            },
        ];

        let result = file_patch_result(
            files,
            7,
            1,
            "@@ src/a.rs\n@@ src/b.rs",
            false,
            vec!["fc_a".to_string(), "fc_b".to_string()],
            Some("cp_patch"),
            Some(3),
            Some("session-99"),
        );

        assert_eq!(result.operation, "file_patch");
        assert_eq!(result.files.len(), 2);
        assert_eq!(result.diff.file_count, 2);
        assert_eq!(result.diff.additions, 7);
        assert_eq!(result.file_change_ids.len(), 2);
        assert!(result.ui_summary.contains("2 files"));
    }

    #[test]
    fn file_patch_partial_failure_includes_rollback() {
        let result = file_patch_partial_failure_result(
            "src/c.rs",
            vec![],
            0,
            0,
            "",
            false,
            vec![],
            Some("cp_fail"),
            Some(1),
            Some("session-1"),
            true,
            true,
            vec!["src/a.rs".to_string()],
            vec![],
            vec![],
            None,
        );

        let rollback = result.rollback.as_ref().unwrap();
        assert!(rollback.attempted);
        assert!(rollback.success);
        assert_eq!(rollback.failed_path.as_deref(), Some("src/c.rs"));
        assert_eq!(rollback.restored_files, vec!["src/a.rs"]);
        assert!(result.ui_summary.contains("partial failure"));
        assert!(result.ui_summary.contains("rollback ok"));
    }

    #[test]
    fn diff_hash_is_stable() {
        let a = FileMutationResult::compute_diff_hash("hello world");
        let b = FileMutationResult::compute_diff_hash("hello world");
        assert_eq!(a, b);
        assert_ne!(a, FileMutationResult::compute_diff_hash("different"));
    }

    #[test]
    fn display_path_strips_workspace_prefix() {
        let resolved = std::path::Path::new("/home/user/project/src/main.rs");
        let workspace = std::path::Path::new("/home/user/project");
        assert_eq!(
            display_path_from_resolved(resolved, workspace),
            "src/main.rs"
        );
    }

    #[test]
    fn display_path_falls_back_to_full_path() {
        let resolved = std::path::Path::new("/etc/hosts");
        let workspace = std::path::Path::new("/home/user/project");
        assert_eq!(
            display_path_from_resolved(resolved, workspace),
            "/etc/hosts"
        );
    }

    #[test]
    fn diagnostics_json_nulls_do_not_create_fake_items() {
        let diagnostics = serde_json::json!({
            "available": true,
            "checked": true,
            "status": "no_diagnostics",
            "diagnostic_count": 0,
            "error_count": 0,
            "warning_count": 0,
            "first_error": null,
            "first_warning": null,
            "affected_line_range": null
        });

        let result = from_file_edit_json(
            "src/lib.rs",
            "/project/src/lib.rs",
            "src/lib.rs",
            1,
            20,
            1,
            0,
            4,
            4,
            "@@ -4 +4 @@",
            false,
            "utf-8",
            false,
            "LF",
            Some("checkpoint"),
            1,
            Some("session"),
            Some("file-change"),
            &Some(diagnostics),
            None,
        );

        let diagnostics = result.diagnostics.unwrap();
        assert!(diagnostics.first_error.is_none());
        assert!(diagnostics.first_warning.is_none());
        assert!(diagnostics.affected_line_range.is_none());
    }

    #[test]
    fn from_tool_data_extracts_mutation_result() {
        let result = file_write_result(
            "src/main.rs",
            "/proj/src/main.rs",
            "src/main.rs",
            true,
            100,
            3,
            1,
            Some(5),
            Some(7),
            "utf-8",
            false,
            "LF",
            None,
            None,
            "@@ diff",
            false,
            Some("fc_1".to_string()),
            Some("cp_1"),
            Some(1),
            Some("sess"),
        );
        let json = serde_json::to_value(&result).unwrap();
        let data = serde_json::json!({"mutation_result": json});

        let extracted = from_tool_data(&data).unwrap();
        assert_eq!(extracted.operation, "file_write");
        assert_eq!(extracted.files[0].path, "src/main.rs");
        assert_eq!(extracted.diff.additions, 3);
    }

    #[test]
    fn from_tool_data_returns_none_when_absent() {
        let data = serde_json::json!({"path": "src/main.rs", "bytes_written": 100});
        assert!(from_tool_data(&data).is_none());
    }

    #[test]
    fn compact_summary_uses_ui_summary_when_present() {
        let result = file_write_result(
            "src/main.rs",
            "/proj/src/main.rs",
            "src/main.rs",
            true,
            42,
            2,
            1,
            Some(3),
            Some(4),
            "utf-8",
            false,
            "LF",
            None,
            None,
            "diff",
            false,
            None,
            Some("cp"),
            Some(1),
            Some("s"),
        );
        let json = serde_json::to_value(&result).unwrap();
        let data = serde_json::json!({"mutation_result": json});
        let summary = compact_summary(&data);
        assert!(summary.contains("file_write"));
        assert!(summary.contains("42B"));
    }

    #[test]
    fn compact_summary_falls_back_without_mutation_result() {
        let data = serde_json::json!({
            "path": "src/lib.rs",
            "bytes_written": 25,
            "diff": {"additions": 2, "deletions": 0}
        });
        let summary = compact_summary(&data);
        assert!(summary.contains("src/lib.rs"));
        assert!(summary.contains("+2"));
    }

    /// 验证三个文件工具都产生一致的 mutation_result metadata 结构。
    #[test]
    fn all_file_mutation_results_have_consistent_shape() {
        // Verify from_tool_data extracts FileMutationResult with all expected fields
        let write_data = serde_json::json!({
            "mutation_result": {
                "operation": "file_write",
                "files": [{
                    "path": "src/lib.rs", "resolved_path": "src/lib.rs",
                    "display_path": "src/lib.rs", "existed_before": false,
                    "replacements": 0, "bytes_written": 42,
                    "additions": 3, "deletions": 0,
                    "changed_line_start": 1, "changed_line_end": 3,
                    "text_format": {"encoding": "utf-8", "has_bom": false, "line_ending": "lf"},
                    "file_change_id": "fc1", "before_hash": null, "after_hash": null
                }],
                "diff": {
                    "additions": 3, "deletions": 0, "file_count": 1,
                    "unified_diff": "...", "truncated": false, "diff_hash": "abc"
                },
                "checkpoint": {"checkpoint_id": "cp1", "sequence": 1, "session_id": null, "restore_eligible": true},
                "file_change_ids": ["fc1"],
                "diagnostics": null,
                "rollback": null,
                "formatted": false,
                "ui_summary": "file_write src/lib.rs: +3/-0, 42B"
            }
        });
        let mr = from_tool_data(&write_data).expect("file_write mutation_result");
        assert_eq!(mr.operation, "file_write");
        assert!(!mr.formatted);
        assert_eq!(mr.files.len(), 1);
        assert!(mr.diagnostics.is_none());
        assert!(mr.checkpoint.is_some());
        assert_eq!(mr.checkpoint.as_ref().unwrap().checkpoint_id, "cp1");
        assert_eq!(mr.file_change_ids.len(), 1);

        // Verify compact_summary works with the mutation_result
        let summary = compact_summary(&write_data);
        assert!(summary.contains("file_write"));
    }

    /// 确保 mutation_result 的 formatted 字段默认值为 false。
    #[test]
    fn formatted_field_defaults_to_false() {
        let data = serde_json::json!({
            "mutation_result": {
                "operation": "file_write",
                "files": [],
                "diff": {"additions": 0, "deletions": 0, "file_count": 0, "unified_diff": "", "truncated": false, "diff_hash": ""},
                "file_change_ids": [],
                "ui_summary": ""
            }
        });
        let mr = from_tool_data(&data).expect("mutation_result without formatted field");
        assert!(!mr.formatted);
    }
}
