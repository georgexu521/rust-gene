//! Shared API DTOs — canonical vocabulary for session, tool-output,
//! provider, permission, diagnostic, context, file-mutation, and job state.
//!
//! TUI, desktop, and the experimental API server consume these types
//! without re-interpreting raw trace or event payloads.

pub mod context;
pub mod diagnostic;
pub mod file_mutation;
pub mod permission;
pub mod provider;
pub mod provider_catalog;
pub mod session;
pub mod session_jobs;
pub mod tool_output;
