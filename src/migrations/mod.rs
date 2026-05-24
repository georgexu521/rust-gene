//! 数据库迁移模块
//!
//! 管理 SQLite 数据库的 schema 演进

pub mod framework;
pub mod v1_initial;
pub mod v2_add_tasks;
pub mod v3_add_traces;
pub mod v4_add_learning_events;
pub mod v5_add_agent_artifacts;
pub mod v6_add_agent_task_states;
pub mod v7_add_compact_boundaries;

pub use framework::{Migration, MigrationRunner};
