//! 迁移框架
//!
//! 基于 PRAGMA user_version 的数据库迁移系统

use rusqlite::{Connection, Result as SqlResult};
use std::sync::Arc;
use tracing::{debug, info};

/// 单个迁移
pub trait Migration: Send + Sync {
    /// 迁移版本号
    fn version(&self) -> i32;
    /// 迁移名称
    fn name(&self) -> &str;
    /// 执行迁移
    fn up(&self, conn: &Connection) -> SqlResult<()>;
}

/// 迁移运行器
pub struct MigrationRunner {
    migrations: Vec<Arc<dyn Migration>>,
}

impl MigrationRunner {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    /// 注册迁移
    pub fn register(&mut self, migration: Arc<dyn Migration>) {
        self.migrations.push(migration);
    }

    /// 运行所有待执行的迁移
    pub fn run(&self, conn: &Connection) -> SqlResult<()> {
        let current_version: i32 = conn
            .query_row(
                "SELECT user_version FROM pragma_user_version()",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let mut sorted = self.migrations.clone();
        sorted.sort_by_key(|m| m.version());

        for migration in sorted {
            if migration.version() > current_version {
                info!(
                    "Running migration {}: {}",
                    migration.version(),
                    migration.name()
                );
                migration.up(conn)?;
                conn.pragma_update(None, "user_version", migration.version())?;
                debug!("Set user_version to {}", migration.version());
            }
        }

        Ok(())
    }
}

impl Default for MigrationRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestMigration {
        version_num: i32,
        migration_name: &'static str,
        sql: &'static str,
    }

    impl Migration for TestMigration {
        fn version(&self) -> i32 {
            self.version_num
        }

        fn name(&self) -> &str {
            self.migration_name
        }

        fn up(&self, conn: &Connection) -> SqlResult<()> {
            conn.execute_batch(self.sql)
        }
    }

    #[test]
    fn test_migration_runner_applies_migrations() {
        let conn = Connection::open_in_memory().unwrap();

        let mut runner = MigrationRunner::new();
        runner.register(Arc::new(TestMigration {
            version_num: 1,
            migration_name: "create_users",
            sql: "CREATE TABLE users (id INTEGER PRIMARY KEY);",
        }));
        runner.register(Arc::new(TestMigration {
            version_num: 2,
            migration_name: "create_posts",
            sql: "CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER);",
        }));

        runner.run(&conn).unwrap();

        let version: i32 = conn
            .query_row("SELECT user_version FROM pragma_user_version()", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, 2);

        // Verify tables exist
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='posts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_migration_runner_skips_applied() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE users (id INTEGER PRIMARY KEY); PRAGMA user_version = 1;")
            .unwrap();

        let mut runner = MigrationRunner::new();
        runner.register(Arc::new(TestMigration {
            version_num: 1,
            migration_name: "create_users",
            sql: "CREATE TABLE users2 (id INTEGER PRIMARY KEY);",
        }));
        runner.register(Arc::new(TestMigration {
            version_num: 2,
            migration_name: "create_posts",
            sql: "CREATE TABLE posts (id INTEGER PRIMARY KEY);",
        }));

        runner.run(&conn).unwrap();

        // users2 should NOT exist because v1 was already applied
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='users2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);

        // posts should exist
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='posts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_migration_runner_out_of_order() {
        let conn = Connection::open_in_memory().unwrap();

        let mut runner = MigrationRunner::new();
        // Register out of order
        runner.register(Arc::new(TestMigration {
            version_num: 3,
            migration_name: "v3",
            sql: "CREATE TABLE v3 (id INTEGER PRIMARY KEY);",
        }));
        runner.register(Arc::new(TestMigration {
            version_num: 1,
            migration_name: "v1",
            sql: "CREATE TABLE v1 (id INTEGER PRIMARY KEY);",
        }));
        runner.register(Arc::new(TestMigration {
            version_num: 2,
            migration_name: "v2",
            sql: "CREATE TABLE v2 (id INTEGER PRIMARY KEY);",
        }));

        runner.run(&conn).unwrap();

        let version: i32 = conn
            .query_row("SELECT user_version FROM pragma_user_version()", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, 3);
    }
}
