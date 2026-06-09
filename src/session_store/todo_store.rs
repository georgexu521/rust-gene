//! Session-backed coding todo projection (Phase 5: opencode alignment).
//!
//! Provides persistence and query methods for the `todos` table.
//! The `todo_write` tool calls into these to make todos durable.

use rusqlite::params;
use serde::{Deserialize, Serialize};

/// A single todo item reflecting the session's current coding check-list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoItem {
    pub content: String,
    pub status: String,
    pub priority: String,
}

/// Persisted todo with position and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTodoItem {
    pub content: String,
    pub status: String,
    pub priority: String,
    pub position: usize,
    pub updated_at: String,
}

/// Replace all todos for a session atomically (transactional set semantics).
///
/// Validates that at most one todo is `in_progress`.
pub fn replace_todos(
    conn: &rusqlite::Connection,
    session_id: &str,
    todos: &[TodoItem],
) -> rusqlite::Result<()> {
    let in_progress_count = todos.iter().filter(|t| t.status == "in_progress").count();
    if in_progress_count > 1 {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "at most one todo may be in_progress",
            ),
        )));
    }

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM todos WHERE session_id = ?1",
        params![session_id],
    )?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO todos (session_id, content, status, priority, position)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (i, todo) in todos.iter().enumerate() {
            stmt.execute(params![
                session_id,
                todo.content,
                todo.status,
                todo.priority,
                i as i32
            ])?;
        }
    }
    tx.commit()
}

/// Load the current todo list for a session.
pub fn load_todos(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> rusqlite::Result<Vec<TodoItem>> {
    let mut stmt = conn.prepare(
        "SELECT content, status, priority FROM todos
         WHERE session_id = ?1
         ORDER BY position",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok(TodoItem {
            content: row.get(0)?,
            status: row.get(1)?,
            priority: row.get(2)?,
        })
    })?;
    let mut todos = Vec::new();
    for row in rows {
        todos.push(row?);
    }
    Ok(todos)
}

/// Load persisted todos with position and timestamp for display.
pub fn load_persisted_todos(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> rusqlite::Result<Vec<PersistedTodoItem>> {
    let mut stmt = conn.prepare(
        "SELECT content, status, priority, position, updated_at FROM todos
         WHERE session_id = ?1
         ORDER BY position",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok(PersistedTodoItem {
            content: row.get(0)?,
            status: row.get(1)?,
            priority: row.get(2)?,
            position: row.get::<_, i32>(3)? as usize,
            updated_at: row.get(4)?,
        })
    })?;
    let mut todos = Vec::new();
    for row in rows {
        todos.push(row?);
    }
    Ok(todos)
}

/// Clear all todos for a session.
pub fn clear_todos(conn: &rusqlite::Connection, session_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM todos WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

/// Format a compact status-line summary of the current todos.
pub fn format_todo_status(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return String::new();
    }
    let pending = todos.iter().filter(|t| t.status == "pending").count();
    let in_progress = todos.iter().filter(|t| t.status == "in_progress").count();
    let completed = todos.iter().filter(|t| t.status == "completed").count();
    let active = todos.iter().find(|t| t.status == "in_progress");
    if let Some(current) = active {
        let content = truncate_chars(&current.content, 40);
        format!(
            "📋 {} [{}/{}]",
            content,
            completed,
            pending + in_progress + completed
        )
    } else if todos.is_empty() || completed == todos.len() {
        String::new()
    } else {
        let first = &todos[0].content;
        let preview = truncate_chars(first, 30);
        format!("📋 {} [{}/{}]", preview, completed, todos.len())
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let mut out: String = value.chars().take(keep).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup() -> (Connection, String) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE todos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending',
                priority TEXT NOT NULL DEFAULT '',
                position INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .unwrap();
        (conn, "test-session".to_string())
    }

    #[test]
    fn replace_and_load_todos() {
        let (conn, sid) = setup();
        let items = vec![
            TodoItem {
                content: "add login endpoint".to_string(),
                status: "in_progress".to_string(),
                priority: "high".to_string(),
            },
            TodoItem {
                content: "add logout".to_string(),
                status: "pending".to_string(),
                priority: "".to_string(),
            },
        ];
        replace_todos(&conn, &sid, &items).unwrap();

        let loaded = load_todos(&conn, &sid).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].content, "add login endpoint");
        assert_eq!(loaded[0].status, "in_progress");
        assert_eq!(loaded[1].content, "add logout");
        assert_eq!(loaded[1].status, "pending");
    }

    #[test]
    fn replace_clears_existing_and_replaces() {
        let (conn, sid) = setup();
        replace_todos(
            &conn,
            &sid,
            &[TodoItem {
                content: "old".to_string(),
                status: "pending".to_string(),
                priority: "".to_string(),
            }],
        )
        .unwrap();

        replace_todos(
            &conn,
            &sid,
            &[TodoItem {
                content: "new".to_string(),
                status: "in_progress".to_string(),
                priority: "high".to_string(),
            }],
        )
        .unwrap();

        let loaded = load_todos(&conn, &sid).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].content, "new");
    }

    #[test]
    fn empty_list_clears_all() {
        let (conn, sid) = setup();
        replace_todos(
            &conn,
            &sid,
            &[TodoItem {
                content: "temp".to_string(),
                status: "pending".to_string(),
                priority: "".to_string(),
            }],
        )
        .unwrap();

        replace_todos(&conn, &sid, &[]).unwrap();
        assert!(load_todos(&conn, &sid).unwrap().is_empty());
    }

    #[test]
    fn rejects_multiple_in_progress() {
        let (conn, sid) = setup();
        let items = vec![
            TodoItem {
                content: "a".to_string(),
                status: "in_progress".to_string(),
                priority: "".to_string(),
            },
            TodoItem {
                content: "b".to_string(),
                status: "in_progress".to_string(),
                priority: "".to_string(),
            },
        ];
        assert!(replace_todos(&conn, &sid, &items).is_err());
    }

    #[test]
    fn clear_todos_removes_all() {
        let (conn, sid) = setup();
        replace_todos(
            &conn,
            &sid,
            &[TodoItem {
                content: "x".to_string(),
                status: "pending".to_string(),
                priority: "".to_string(),
            }],
        )
        .unwrap();

        clear_todos(&conn, &sid).unwrap();
        assert!(load_todos(&conn, &sid).unwrap().is_empty());
    }

    #[test]
    fn format_status_shows_in_progress() {
        let todos = vec![
            TodoItem {
                content: "fix login bug".to_string(),
                status: "in_progress".to_string(),
                priority: "high".to_string(),
            },
            TodoItem {
                content: "write tests".to_string(),
                status: "pending".to_string(),
                priority: "".to_string(),
            },
        ];
        let status = format_todo_status(&todos);
        assert!(status.contains("📋"));
        assert!(status.contains("fix login bug"));
        assert!(status.contains("[0/2]"));
    }

    #[test]
    fn format_status_truncates_unicode_without_panic() {
        let todos = vec![TodoItem {
            content: "修复中文待办摘要不要按字节截断导致崩溃并保留可读预览".to_string(),
            status: "in_progress".to_string(),
            priority: "high".to_string(),
        }];
        let status = format_todo_status(&todos);
        assert!(status.contains("📋"));
        assert!(status.contains("[0/1]"));
    }

    #[test]
    fn format_status_empty_for_no_todos() {
        assert_eq!(format_todo_status(&[]), "");
    }

    #[test]
    fn format_status_empty_when_all_completed() {
        let todos = vec![TodoItem {
            content: "done".to_string(),
            status: "completed".to_string(),
            priority: "".to_string(),
        }];
        assert_eq!(format_todo_status(&todos), "");
    }
}
