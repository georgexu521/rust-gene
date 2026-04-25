//! Security utilities
//!
//! Shared security functions used across multiple modules.
//!
//! ## 模块概览
//! - `dangerous_command`: 启发式危险命令检测（Bash）
//! - `denial_tracking`: 权限拒绝追踪与回退机制
//! - `audit_log`: 安全审计日志
//! - `llm_classifier`: LLM 驱动的安全分类器

pub mod audit_log;
pub mod dangerous_command;
pub mod denial_tracking;

pub use audit_log::{SecurityAuditLog, SecurityEvent, SecurityEventType};
pub use dangerous_command::is_dangerous_command;
pub use denial_tracking::{DenialRecord, DenialState, DenialTracker};
