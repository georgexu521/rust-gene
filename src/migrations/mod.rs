//! 数据库迁移模块
//!
//! 管理 SQLite 数据库的 schema 演进

pub mod framework;
pub mod v10_add_session_inputs;
pub mod v11_add_session_parts;
pub mod v12_add_session_input_idempotency;
pub mod v13_add_session_reverts;
pub mod v14_add_provider_health_runs;
pub mod v1_initial;
pub mod v2_add_tasks;
pub mod v3_add_traces;
pub mod v4_add_learning_events;
pub mod v5_add_agent_artifacts;
pub mod v6_add_agent_task_states;
pub mod v7_add_compact_boundaries;
pub mod v8_add_todos;
pub mod v9_add_session_events;

pub use framework::{Migration, MigrationRunner};
