use super::{SessionRevertInsert, SessionRevertRecord};
use rusqlite::{Connection, Result as SqlResult};

pub fn insert_revert(
    conn: &Connection,
    insert: &SessionRevertInsert,
) -> SqlResult<SessionRevertRecord> {
    let part_ids = serde_json::to_string(&insert.part_ids).unwrap_or_else(|_| "[]".to_string());
    let checkpoint_ids =
        serde_json::to_string(&insert.checkpoint_ids).unwrap_or_else(|_| "[]".to_string());
    let paths = serde_json::to_string(&insert.paths).unwrap_or_else(|_| "[]".to_string());
    let restored_files =
        serde_json::to_string(&insert.restored_files).unwrap_or_else(|_| "[]".to_string());
    let removed_files =
        serde_json::to_string(&insert.removed_files).unwrap_or_else(|_| "[]".to_string());
    let errors = serde_json::to_string(&insert.errors).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO session_reverts
         (session_id, operation, status, message_id, target_part_id, part_ids_json,
          checkpoint_ids_json, snapshot_checkpoint_id, paths_json, restored_files_json,
          removed_files_json, errors_json, diff_summary, unrevert_possible, unreverted, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        rusqlite::params![
            insert.session_id,
            insert.operation,
            insert.status,
            insert.message_id,
            insert.target_part_id,
            part_ids,
            checkpoint_ids,
            insert.snapshot_checkpoint_id,
            paths,
            restored_files,
            removed_files,
            errors,
            insert.diff_summary,
            insert.unrevert_possible as i64,
            insert.unreverted as i64,
            insert.payload.to_string(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_revert_by_id(conn, id)
}

pub fn mark_latest_revert_unreverted(conn: &Connection, session_id: &str) -> SqlResult<bool> {
    let updated = conn.execute(
        "UPDATE session_reverts
         SET unreverted = 1
         WHERE id = (
             SELECT id FROM session_reverts
             WHERE session_id = ?1
               AND operation = 'revert'
               AND unreverted = 0
             ORDER BY id DESC
             LIMIT 1
         )",
        [session_id],
    )?;
    Ok(updated > 0)
}

pub fn list_reverts(
    conn: &Connection,
    session_id: &str,
    limit: usize,
) -> SqlResult<Vec<SessionRevertRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, operation, status, message_id, target_part_id,
                part_ids_json, checkpoint_ids_json, snapshot_checkpoint_id, paths_json,
                restored_files_json, removed_files_json, errors_json, diff_summary,
                unrevert_possible, unreverted, payload, created_at
         FROM session_reverts
         WHERE session_id = ?1
         ORDER BY id DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![session_id, limit as i64], map_revert)?;
    rows.collect()
}

pub fn latest_revert(
    conn: &Connection,
    session_id: &str,
) -> SqlResult<Option<SessionRevertRecord>> {
    let mut rows = list_reverts(conn, session_id, 1)?;
    Ok(rows.pop())
}

fn get_revert_by_id(conn: &Connection, id: i64) -> SqlResult<SessionRevertRecord> {
    conn.query_row(
        "SELECT id, session_id, operation, status, message_id, target_part_id,
                part_ids_json, checkpoint_ids_json, snapshot_checkpoint_id, paths_json,
                restored_files_json, removed_files_json, errors_json, diff_summary,
                unrevert_possible, unreverted, payload, created_at
         FROM session_reverts
         WHERE id = ?1",
        [id],
        map_revert,
    )
}

fn map_revert(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRevertRecord> {
    let payload: String = row.get(16)?;
    Ok(SessionRevertRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        operation: row.get(2)?,
        status: row.get(3)?,
        message_id: row.get(4)?,
        target_part_id: row.get(5)?,
        part_ids: json_vec(row.get::<_, String>(6)?),
        checkpoint_ids: json_vec(row.get::<_, String>(7)?),
        snapshot_checkpoint_id: row.get(8)?,
        paths: json_vec(row.get::<_, String>(9)?),
        restored_files: json_vec(row.get::<_, String>(10)?),
        removed_files: json_vec(row.get::<_, String>(11)?),
        errors: json_vec(row.get::<_, String>(12)?),
        diff_summary: row.get(13)?,
        unrevert_possible: row.get::<_, i64>(14)? != 0,
        unreverted: row.get::<_, i64>(15)? != 0,
        payload: serde_json::from_str(&payload).unwrap_or_else(|_| serde_json::json!({})),
        created_at: row.get(17)?,
    })
}

fn json_vec(text: String) -> Vec<String> {
    serde_json::from_str(&text).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY);
             INSERT INTO sessions (id) VALUES ('s1');
             CREATE TABLE session_reverts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                operation TEXT NOT NULL DEFAULT 'revert',
                status TEXT NOT NULL,
                message_id TEXT,
                target_part_id TEXT,
                part_ids_json TEXT NOT NULL DEFAULT '[]',
                checkpoint_ids_json TEXT NOT NULL DEFAULT '[]',
                snapshot_checkpoint_id TEXT,
                paths_json TEXT NOT NULL DEFAULT '[]',
                restored_files_json TEXT NOT NULL DEFAULT '[]',
                removed_files_json TEXT NOT NULL DEFAULT '[]',
                errors_json TEXT NOT NULL DEFAULT '[]',
                diff_summary TEXT,
                unrevert_possible INTEGER NOT NULL DEFAULT 0,
                unreverted INTEGER NOT NULL DEFAULT 0,
                payload TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn inserts_and_marks_revert_unreverted() {
        let conn = test_conn();
        let record = insert_revert(
            &conn,
            &SessionRevertInsert {
                session_id: "s1".to_string(),
                operation: "revert".to_string(),
                status: "completed".to_string(),
                message_id: Some("msg-1".to_string()),
                target_part_id: Some("tool-1".to_string()),
                part_ids: vec!["tool-1".to_string()],
                checkpoint_ids: vec!["cp-1".to_string()],
                snapshot_checkpoint_id: Some("cp-1".to_string()),
                paths: vec!["src/lib.rs".to_string()],
                restored_files: vec!["src/lib.rs".to_string()],
                removed_files: Vec::new(),
                errors: Vec::new(),
                diff_summary: Some("diff".to_string()),
                unrevert_possible: true,
                unreverted: false,
                payload: serde_json::json!({"status":"completed"}),
            },
        )
        .unwrap();

        assert_eq!(record.target_part_id.as_deref(), Some("tool-1"));
        assert!(record.unrevert_possible);
        assert!(!record.unreverted);
        assert!(mark_latest_revert_unreverted(&conn, "s1").unwrap());
        let latest = latest_revert(&conn, "s1").unwrap().unwrap();
        assert!(latest.unreverted);
    }
}
