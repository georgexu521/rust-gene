//! Security utilities
//!
//! Shared security functions used across multiple modules.

pub mod dangerous_command;

pub use dangerous_command::is_dangerous_command;
