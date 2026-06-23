//! Priority Agent runtime library.
//!
//! This crate exposes the reusable agent runtime for CLI, TUI, API, eval, and
//! desktop application entrypoints.

// Public product and integration surfaces. Keep this list deliberate: modules
// that are only implementation detail should stay crate-private so release
// builds do not imply a broad stable library API.
pub mod agent;
#[cfg(feature = "experimental-api-server")]
pub mod api;
pub mod bootstrap;
#[allow(dead_code)]
pub(crate) mod bridge;
#[allow(dead_code)]
pub(crate) mod changelog;
pub(crate) mod components;
pub mod cost_tracker;
pub mod desktop_runtime;
pub mod diagnostics;
pub mod engine;
pub mod entry;
#[allow(dead_code)]
pub(crate) mod errors;
pub(crate) mod github;
#[allow(dead_code)]
pub(crate) mod ide;
pub(crate) mod instructions;
pub mod lab;
pub mod memory;
pub(crate) mod migrations;
pub(crate) mod onboarding;
pub mod permissions;
#[allow(dead_code)]
#[cfg(feature = "experimental-api-server")]
pub(crate) mod platform;
pub(crate) mod plugins;
#[allow(dead_code)]
pub(crate) mod ports;
#[allow(dead_code)]
pub(crate) mod quality_gates;
pub(crate) mod remote;
#[allow(dead_code, unused_imports)]
pub(crate) mod security;
pub mod services;
pub mod session_store;
pub mod shell;
pub mod skills;
#[allow(dead_code)]
pub(crate) mod slo;
#[allow(dead_code)]
pub(crate) mod state;
#[allow(dead_code)]
pub(crate) mod telemetry;
#[cfg(test)]
pub mod test_utils;
pub(crate) mod text_utils;
pub mod tool_output_store;
pub mod tools;
pub mod tui;
#[allow(dead_code)]
pub(crate) mod version;
#[cfg(feature = "voice")]
pub mod voice;
pub(crate) mod workspace;

// Internal and historical support modules used by the runtime implementation.
pub(crate) mod internal;
