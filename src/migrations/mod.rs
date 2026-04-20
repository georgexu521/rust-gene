//! 数据库迁移模块
//!
//! 管理 SQLite 数据库的 schema 演进

pub mod framework;
pub mod v1_initial;
pub mod v2_add_tasks;

pub use framework::{Migration, MigrationRunner};
