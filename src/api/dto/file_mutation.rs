//! API aliases for canonical file mutation metadata.
//!
//! The source of truth lives with the file tools so API, TUI, desktop, trace,
//! repair, and rollback do not drift into separate schemas.

pub type FileMutationResultV2 = crate::tools::file_tool::mutation_result::FileMutationResult;
pub type FileMutationFileResultV2 = crate::tools::file_tool::mutation_result::FileResult;
pub type FileMutationDiffV2 = crate::tools::file_tool::mutation_result::MutationDiff;
pub type FileMutationCheckpointV2 = crate::tools::file_tool::mutation_result::MutationCheckpoint;

/// Standard stale-content recovery hint.
pub fn stale_content_hint() -> &'static str {
    "file changed since read; re-run file_read and retry the edit"
}

/// Standard ambiguous match recovery hint.
pub fn ambiguous_match_hint(count: usize) -> String {
    format!("old_string matched {count} times; add surrounding context lines to make it unique")
}

pub fn from_tool_data(data: &serde_json::Value) -> Option<FileMutationResultV2> {
    crate::tools::file_tool::mutation_result::from_tool_data(data)
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
            Self::StaleContent => stale_content_hint(),
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
