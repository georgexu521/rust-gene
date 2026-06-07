//! File mutation result DTO — unified schema for write/edit/patch.
//!
//! Slice C of the opencode programming parity plan.

use serde::{Deserialize, Serialize};

/// Compact, model-friendly mutation result shared across file_write, file_edit,
/// and file_patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMutationResultV2 {
    pub operation: String,
    pub changed_paths: Vec<String>,
    pub checkpoint_id: Option<String>,
    pub diff_preview: Option<String>,
    pub additions: usize,
    pub deletions: usize,
    pub stale_state: Option<String>,
    pub diagnostics_delta: Option<serde_json::Value>,
    pub rollback_status: Option<String>,
    pub error_hint: Option<String>,
}

impl FileMutationResultV2 {
    /// Standard stale-content recovery hint.
    pub fn stale_content_hint() -> &'static str {
        "file changed since read; re-run file_read and retry the edit"
    }

    /// Standard ambiguous match recovery hint.
    pub fn ambiguous_match_hint(count: usize) -> String {
        format!("old_string matched {count} times; add surrounding context lines to make it unique")
    }
}

/// Normalized error message for common file mutation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMutationErrorKind {
    StaleContent,
    AmbiguousMatch,
    AnchorNotFound,
    ReadLinePrefix,
    NoOp,
    ReplacementLimit,
}

impl FileMutationErrorKind {
    pub fn model_hint(self) -> &'static str {
        match self {
            Self::StaleContent => FileMutationResultV2::stale_content_hint(),
            Self::AnchorNotFound => {
                "old_string not found in file; read the file again and copy the exact text to replace"
            }
            Self::ReadLinePrefix => {
                "old_string contains display line prefixes (e.g. '12 |'); copy text after the pipe"
            }
            Self::NoOp => "old_string and new_string are identical; provide a different replacement",
            Self::ReplacementLimit => "too many matches; use a more specific old_string or line_start/line_end",
            Self::AmbiguousMatch => "old_string matched multiple times; add surrounding context lines",
        }
    }
}
