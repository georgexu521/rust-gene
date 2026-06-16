//! Layout and display constants for the CLI shell.

/// Default height of the fixed footer area in lines.
pub const DEFAULT_FOOTER_HEIGHT: usize = 3;

/// Visual width of the prompt prefix (`› `).
pub const PROMPT_PREFIX_WIDTH: usize = 2;

/// Minimum welcome banner width.
pub const WELCOME_WIDTH_MIN: usize = 60;
/// Maximum welcome banner width.
pub const WELCOME_WIDTH_MAX: usize = 110;

/// Width of the context usage bar in `/status`.
pub const STATUS_CONTEXT_BAR_WIDTH: usize = 16;

/// Widths used when rendering the session list.
pub const SESSION_LIST_TITLE_WIDTH: usize = 42;
pub const SESSION_LIST_MODEL_WIDTH: usize = 18;

/// Maximum width for a recent memory snippet in `/status`.
pub const RECENT_MEMORY_SNIPPET_WIDTH: usize = 72;

/// Maximum length for a permission scope summary line.
pub const PERMISSION_SCOPE_MAX_LEN: usize = 80;

/// Maximum widths for tool progress display.
pub const TOOL_PROGRESS_NAME_WIDTH: usize = 24;
pub const TOOL_PROGRESS_LATEST_WIDTH: usize = 96;

/// Maximum widths for compact model/provider strings in the welcome banner.
pub const WELCOME_MODEL_WIDTH: usize = 30;
pub const WELCOME_PROVIDER_WIDTH: usize = 42;

/// Fallback terminal width when `crossterm` cannot detect it.
pub const TERMINAL_WIDTH_FALLBACK: usize = 80;
