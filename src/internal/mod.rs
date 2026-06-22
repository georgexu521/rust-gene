//! Internal support modules that are not product-facing entrypoints.
//!
//! These modules are retained for runtime compatibility or historical support,
//! but keeping them under `internal` prevents the crate root from looking like
//! a broad public product surface.

#[allow(dead_code)]
pub(crate) mod ai_analyzer;
#[allow(dead_code)]
pub(crate) mod context_manager;
#[allow(dead_code)]
pub(crate) mod priority;
#[allow(dead_code)]
pub(crate) mod task_analyzer;
#[allow(dead_code)]
pub(crate) mod task_manager;
#[allow(dead_code)]
pub(crate) mod team;
