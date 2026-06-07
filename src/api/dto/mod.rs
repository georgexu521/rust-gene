//! Shared API DTOs — canonical vocabulary for session, tool-output,
//! provider, permission, and diagnostic state.
//!
//! TUI, desktop, and the experimental API server consume these types
//! without re-interpreting raw trace or event payloads.

pub mod diagnostic;
pub mod permission;
pub mod provider;
pub mod session;
pub mod tool_output;
