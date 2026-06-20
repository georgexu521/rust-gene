//! LabRun orchestration primitives.
//!
//! The first slice is intentionally file-backed and command-driven. The
//! runtime loop can build on these typed artifacts without depending on chat
//! history as the source of truth.

pub mod commands;
pub mod context;
pub mod delegation;
pub mod draft;
pub mod model;
pub mod orchestrator;
pub mod provider_certification;
pub mod report;
pub mod scheduler;
pub mod store;
