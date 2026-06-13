//! TUI view models.
//!
//! These selectors keep product state choices out of rendering code so multiple
//! widgets do not invent competing labels for the same runtime event.

pub mod activity;
pub mod footer;
pub mod reasoning;
pub mod timeline;
pub mod tool_rows;
