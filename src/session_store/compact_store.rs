use rusqlite::{params, Result as SqlResult, Row};

use super::{CompactBoundaryInsert, CompactBoundaryRecord, SessionStore};

impl SessionStore {
    pub fn add_compact_boundary(&self, boundary: &CompactBoundaryInsert) -> SqlResult<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO compact_boundaries
             (session_id, boundary_id, sequence, strategy, trigger, before_tokens, after_tokens,
              messages_before, messages_after, preserved_tail_count, retained_items, provenance,
              summary, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(session_id, boundary_id) DO UPDATE SET
               sequence = excluded.sequence,
               strategy = excluded.strategy,
               trigger = excluded.trigger,
               before_tokens = excluded.before_tokens,
               after_tokens = excluded.after_tokens,
               messages_before = excluded.messages_before,
               messages_after = excluded.messages_after,
               preserved_tail_count = excluded.preserved_tail_count,
               retained_items = excluded.retained_items,
               provenance = excluded.provenance,
               summary = excluded.summary,
               payload = excluded.payload",
            params![
                &boundary.session_id,
                &boundary.boundary_id,
                boundary.sequence,
                &boundary.strategy,
                &boundary.trigger,
                boundary.before_tokens,
                boundary.after_tokens,
                boundary.messages_before,
                boundary.messages_after,
                boundary.preserved_tail_count,
                boundary.retained_items.to_string(),
                boundary.provenance.to_string(),
                boundary.summary,
                boundary.payload.to_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_compact_boundaries(
        &self,
        session_id: &str,
        limit: i64,
    ) -> SqlResult<Vec<CompactBoundaryRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, boundary_id, sequence, strategy, trigger,
                    before_tokens, after_tokens, messages_before, messages_after,
                    preserved_tail_count, retained_items, provenance, summary, payload, created_at
             FROM compact_boundaries
             WHERE session_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], compact_boundary_from_row)?;
        rows.collect()
    }

    pub fn latest_compact_boundary(
        &self,
        session_id: &str,
    ) -> SqlResult<Option<CompactBoundaryRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, session_id, boundary_id, sequence, strategy, trigger,
                    before_tokens, after_tokens, messages_before, messages_after,
                    preserved_tail_count, retained_items, provenance, summary, payload, created_at
             FROM compact_boundaries
             WHERE session_id = ?1
             ORDER BY id DESC
             LIMIT 1",
            params![session_id],
            compact_boundary_from_row,
        );
        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn add_compact_boundary_from_runtime_record(
        &self,
        session_id: &str,
        record: &crate::engine::context_collapse::CompactionRuntimeRecord,
        trigger: Option<&str>,
        summary: &str,
    ) -> SqlResult<i64> {
        let boundary_id = record
            .boundary_id
            .clone()
            .unwrap_or_else(|| format!("compact-{}", uuid::Uuid::new_v4()));
        self.add_compact_boundary(&CompactBoundaryInsert {
            session_id: session_id.to_string(),
            boundary_id,
            sequence: record.sequence.map(i64::from),
            strategy: record.strategy.label().to_string(),
            trigger: trigger
                .map(str::to_string)
                .or_else(|| record.trigger.clone()),
            before_tokens: i64::try_from(record.tokens_before).unwrap_or(i64::MAX),
            after_tokens: i64::try_from(record.tokens_after).unwrap_or(i64::MAX),
            messages_before: i64::try_from(record.messages_before).unwrap_or(i64::MAX),
            messages_after: i64::try_from(record.messages_after).unwrap_or(i64::MAX),
            preserved_tail_count: record
                .preserved_tail_count
                .and_then(|count| i64::try_from(count).ok()),
            retained_items: serde_json::json!(record.retained_items),
            provenance: serde_json::json!(record.provenance),
            summary: summary.to_string(),
            payload: serde_json::to_value(record).unwrap_or_else(|_| serde_json::json!({})),
        })
    }
}

fn compact_boundary_from_row(row: &Row<'_>) -> SqlResult<CompactBoundaryRecord> {
    let retained_items_text: String = row.get(11)?;
    let provenance_text: String = row.get(12)?;
    let payload_text: String = row.get(14)?;
    Ok(CompactBoundaryRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        boundary_id: row.get(2)?,
        sequence: row.get(3)?,
        strategy: row.get(4)?,
        trigger: row.get(5)?,
        before_tokens: row.get(6)?,
        after_tokens: row.get(7)?,
        messages_before: row.get(8)?,
        messages_after: row.get(9)?,
        preserved_tail_count: row.get(10)?,
        retained_items: serde_json::from_str(&retained_items_text)
            .unwrap_or_else(|_| serde_json::json!([])),
        provenance: serde_json::from_str(&provenance_text)
            .unwrap_or_else(|_| serde_json::json!([])),
        summary: row.get(13)?,
        payload: serde_json::from_str(&payload_text).unwrap_or_else(|_| serde_json::json!({})),
        created_at: row.get(15)?,
    })
}
