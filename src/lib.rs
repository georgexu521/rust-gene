//! Priority Agent runtime library.
//!
//! This crate exposes the reusable agent runtime for CLI, TUI, API, eval, and
//! desktop application entrypoints.

pub mod agent;
pub mod ai_analyzer;
#[cfg(feature = "experimental-api-server")]
pub mod api;
pub mod bootstrap;
pub mod bridge;
pub mod changelog;
pub mod context_manager;
pub mod cost_tracker;
pub mod desktop_runtime;
pub mod diagnostics;
pub mod engine;
pub mod errors;
pub mod github;
pub mod ide;
pub mod instructions;
pub mod memory;
pub mod migrations;
pub mod onboarding;
pub mod permissions;
#[cfg(feature = "experimental-api-server")]
pub mod platform;
pub mod plugins;
pub mod ports;
pub mod priority;
pub mod quality_gates;
pub mod remote;
pub mod security;
pub mod services;
pub mod session_store;
pub mod shell;
pub mod skills;
pub mod slo;
pub mod state;
pub mod task_analyzer;
pub mod task_manager;
pub mod team;
pub mod telemetry;
#[cfg(test)]
pub mod test_utils;
pub mod tool_output_store;
pub mod tools;
pub mod tui;
pub mod version;
#[cfg(feature = "voice")]
pub mod voice;
