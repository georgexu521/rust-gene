//! LabRun orchestration primitives.
//!
//! The first slice is intentionally file-backed and command-driven. The
//! runtime loop can build on these typed artifacts without depending on chat
//! history as the source of truth.

pub(crate) mod artifact_semantics;
pub(crate) mod audit_redaction;
pub mod commands;
pub mod context;
pub mod delegation;
pub mod draft;
pub(crate) mod execution_binding;
pub mod model;
pub mod next_action;
pub mod orchestrator;
pub(crate) mod path_scope;
pub(crate) mod policy_overlay;
pub mod provider_certification;
pub mod report;
pub(crate) mod runtime_evidence_redaction;
pub mod scheduler;
pub mod store;
pub(crate) mod validation;
pub(crate) mod workspace_trust;
