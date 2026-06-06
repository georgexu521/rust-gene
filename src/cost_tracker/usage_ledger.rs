use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const USAGE_LEDGER_ENV: &str = "PRIORITY_AGENT_USAGE_LEDGER_PATH";
#[cfg(test)]
const USAGE_LEDGER_TEST_ENABLE_ENV: &str = "PRIORITY_AGENT_TEST_ENABLE_USAGE_LEDGER_WRITES";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageLedgerEntry {
    pub ts: u64,
    pub session: String,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cache_hit_tokens: u64,
    pub cache_miss_tokens: u64,
    pub cost_usd: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stable_prefix_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_schema_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_tail_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub miss_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub miss_reason_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_output_cap: Option<u32>,
    #[serde(default)]
    pub tool_schema_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_round_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction_decision: Option<String>,
    // Phase C: provider runtime metadata (opencode alignment)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_count: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageLedgerModelSummary {
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cache_hit_tokens: u64,
    pub cache_miss_tokens: u64,
    pub cost_usd: f64,
    pub tool_schema_tokens: u64,
    pub capped_requests: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageLedgerSummary {
    pub path: PathBuf,
    pub entries: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cache_hit_tokens: u64,
    pub cache_miss_tokens: u64,
    pub cost_usd: f64,
    pub hit_rate: f64,
    pub tool_schema_tokens: u64,
    pub capped_requests: u64,
    pub by_model: HashMap<String, UsageLedgerModelSummary>,
    pub last_miss_reason: Option<String>,
}

impl UsageLedgerSummary {
    pub fn record(&mut self, entry: &UsageLedgerEntry) {
        self.entries += 1;
        self.prompt_tokens += entry.prompt_tokens;
        self.completion_tokens += entry.completion_tokens;
        self.total_tokens += entry.total_tokens;
        self.cache_hit_tokens += entry.cache_hit_tokens;
        self.cache_miss_tokens += entry.cache_miss_tokens;
        self.cost_usd += entry.cost_usd;
        self.tool_schema_tokens += entry.tool_schema_tokens;
        if entry.effective_output_cap.is_some() {
            self.capped_requests += 1;
        }
        self.hit_rate = prompt_cache_hit_rate(self.prompt_tokens, self.cache_hit_tokens);
        if let Some(reason) = &entry.miss_reason {
            self.last_miss_reason = Some(reason.clone());
        }

        let model = self.by_model.entry(entry.model.clone()).or_default();
        model.requests += 1;
        model.prompt_tokens += entry.prompt_tokens;
        model.completion_tokens += entry.completion_tokens;
        model.total_tokens += entry.total_tokens;
        model.cache_hit_tokens += entry.cache_hit_tokens;
        model.cache_miss_tokens += entry.cache_miss_tokens;
        model.cost_usd += entry.cost_usd;
        model.tool_schema_tokens += entry.tool_schema_tokens;
        if entry.effective_output_cap.is_some() {
            model.capped_requests += 1;
        }
    }
}

pub fn default_usage_ledger_path() -> PathBuf {
    if let Ok(path) = std::env::var(USAGE_LEDGER_ENV) {
        return PathBuf::from(path);
    }
    dirs::data_dir()
        .map(|dir| dir.join("priority-agent").join("usage.jsonl"))
        .unwrap_or_else(|| PathBuf::from(".priority-agent/usage.jsonl"))
}

pub fn append_usage_ledger_entry(entry: &UsageLedgerEntry) -> io::Result<()> {
    #[cfg(test)]
    if std::env::var_os(USAGE_LEDGER_TEST_ENABLE_ENV).is_none() {
        return Ok(());
    }
    append_usage_ledger_entry_at(&default_usage_ledger_path(), entry)
}

pub fn append_usage_ledger_entry_at(path: &Path, entry: &UsageLedgerEntry) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, entry).map_err(io::Error::other)?;
    file.write_all(b"\n")
}

pub fn summarize_usage_ledger(session_filter: Option<&str>) -> io::Result<UsageLedgerSummary> {
    summarize_usage_ledger_at(&default_usage_ledger_path(), session_filter)
}

pub fn summarize_usage_ledger_at(
    path: &Path,
    session_filter: Option<&str>,
) -> io::Result<UsageLedgerSummary> {
    let mut summary = UsageLedgerSummary {
        path: path.to_path_buf(),
        ..UsageLedgerSummary::default()
    };
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(summary),
        Err(err) => return Err(err),
    };
    for line in io::BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry = match serde_json::from_str::<UsageLedgerEntry>(&line) {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if session_filter.is_some_and(|session| entry.session != session) {
            continue;
        }
        summary.record(&entry);
    }
    Ok(summary)
}

pub(crate) fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn prompt_cache_hit_rate(prompt_tokens: u64, cached_tokens: u64) -> f64 {
    if prompt_tokens == 0 {
        0.0
    } else {
        cached_tokens.min(prompt_tokens) as f64 / prompt_tokens as f64
    }
}

// ---- SQLite projection (Phase 5: opencode alignment) ----

const USAGE_SQLITE_TABLE: &str = "usage_ledger";

fn usage_sqlite_path() -> PathBuf {
    if let Ok(path) = std::env::var(USAGE_LEDGER_ENV) {
        let mut p = PathBuf::from(path);
        p.set_extension("sqlite");
        return p;
    }
    dirs::data_dir()
        .map(|dir| dir.join("priority-agent").join("usage.sqlite"))
        .unwrap_or_else(|| PathBuf::from(".priority-agent/usage.sqlite"))
}

fn ensure_usage_sqlite(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {table} (
            ts INTEGER NOT NULL,
            session TEXT NOT NULL,
            model TEXT NOT NULL,
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            cache_hit_tokens INTEGER NOT NULL DEFAULT 0,
            cache_miss_tokens INTEGER NOT NULL DEFAULT 0,
            cost_usd REAL NOT NULL DEFAULT 0.0,
            stable_prefix_hash TEXT,
            system_hash TEXT,
            tool_schema_hash TEXT,
            dynamic_tail_hash TEXT,
            miss_reason TEXT,
            miss_reason_detail TEXT,
            request_phase TEXT,
            effective_output_cap INTEGER,
            tool_schema_tokens INTEGER NOT NULL DEFAULT 0,
            tool_round_count INTEGER,
            compaction_decision TEXT,
            request_id TEXT,
            provider TEXT,
            latency_ms INTEGER,
            time_to_first_token_ms INTEGER,
            finish_reason TEXT,
            error_kind TEXT,
            timeout_kind TEXT,
            retry_count INTEGER,
            day TEXT GENERATED ALWAYS AS (date(ts / 1000, 'unixepoch')) STORED
        );
        CREATE INDEX IF NOT EXISTS idx_{table}_session ON {table}(session);
        CREATE INDEX IF NOT EXISTS idx_{table}_model ON {table}(model);
        CREATE INDEX IF NOT EXISTS idx_{table}_day ON {table}(day);
        CREATE INDEX IF NOT EXISTS idx_{table}_ts ON {table}(ts);",
        table = USAGE_SQLITE_TABLE
    ))?;
    for column in [
        "system_hash",
        "tool_schema_hash",
        "dynamic_tail_hash",
        "miss_reason_detail",
        "request_phase",
        "compaction_decision",
    ] {
        ensure_usage_sqlite_text_column(conn, column)?;
    }
    for column in [
        ("effective_output_cap", "INTEGER"),
        ("tool_schema_tokens", "INTEGER NOT NULL DEFAULT 0"),
        ("tool_round_count", "INTEGER"),
        ("request_id", "TEXT"),
        ("provider", "TEXT"),
        ("latency_ms", "INTEGER"),
        ("time_to_first_token_ms", "INTEGER"),
        ("finish_reason", "TEXT"),
        ("error_kind", "TEXT"),
        ("timeout_kind", "TEXT"),
        ("retry_count", "INTEGER"),
    ] {
        ensure_usage_sqlite_column(conn, column.0, column.1)?;
    }
    Ok(())
}

fn ensure_usage_sqlite_text_column(
    conn: &rusqlite::Connection,
    column: &str,
) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({USAGE_SQLITE_TABLE})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(());
        }
    }
    ensure_usage_sqlite_column(conn, column, "TEXT")
}

fn ensure_usage_sqlite_column(
    conn: &rusqlite::Connection,
    column: &str,
    kind: &str,
) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({USAGE_SQLITE_TABLE})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE {USAGE_SQLITE_TABLE} ADD COLUMN {column} {kind}"),
        [],
    )?;
    Ok(())
}

/// Sync one entry into the SQLite projection.
///
/// Called after a successful JSONL append. Insert is idempotent by matching the
/// complete usage payload so rebuilding or retrying the same entry cannot double
/// count costs.
pub fn sync_usage_to_sqlite(entry: &UsageLedgerEntry) {
    let path = usage_sqlite_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = match rusqlite::Connection::open(&path) {
        Ok(conn) => conn,
        Err(_) => return,
    };
    if ensure_usage_sqlite(&conn).is_err() {
        return;
    }
    let _ = insert_usage_sqlite_entry(&conn, entry);
}

fn insert_usage_sqlite_entry(
    conn: &rusqlite::Connection,
    entry: &UsageLedgerEntry,
) -> rusqlite::Result<usize> {
    conn.execute(
        &format!(
            "INSERT INTO {table} (ts, session, model, prompt_tokens, completion_tokens,
             total_tokens, cache_hit_tokens, cache_miss_tokens, cost_usd,
             stable_prefix_hash, system_hash, tool_schema_hash, dynamic_tail_hash,
             miss_reason, miss_reason_detail, request_phase, effective_output_cap,
             tool_schema_tokens, tool_round_count, compaction_decision,
             request_id, provider, latency_ms, time_to_first_token_ms,
             finish_reason, error_kind, timeout_kind, retry_count)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                    ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28
             WHERE NOT EXISTS (
                SELECT 1 FROM {table}
                WHERE ts = ?1
                  AND session = ?2
                  AND model = ?3
                  AND prompt_tokens = ?4
                  AND completion_tokens = ?5
                  AND total_tokens = ?6
                  AND cache_hit_tokens = ?7
                  AND cache_miss_tokens = ?8
                  AND cost_usd = ?9
                  AND COALESCE(stable_prefix_hash, '') = COALESCE(?10, '')
                  AND COALESCE(system_hash, '') = COALESCE(?11, '')
                  AND COALESCE(tool_schema_hash, '') = COALESCE(?12, '')
                  AND COALESCE(dynamic_tail_hash, '') = COALESCE(?13, '')
                  AND COALESCE(miss_reason, '') = COALESCE(?14, '')
                  AND COALESCE(miss_reason_detail, '') = COALESCE(?15, '')
                  AND COALESCE(request_phase, '') = COALESCE(?16, '')
                  AND COALESCE(effective_output_cap, -1) = COALESCE(?17, -1)
                  AND tool_schema_tokens = ?18
                  AND COALESCE(tool_round_count, -1) = COALESCE(?19, -1)
                  AND COALESCE(compaction_decision, '') = COALESCE(?20, '')
                  AND COALESCE(request_id, '') = COALESCE(?21, '')
                  AND COALESCE(provider, '') = COALESCE(?22, '')
                  AND COALESCE(latency_ms, -1) = COALESCE(?23, -1)
                  AND COALESCE(time_to_first_token_ms, -1) = COALESCE(?24, -1)
                  AND COALESCE(finish_reason, '') = COALESCE(?25, '')
                  AND COALESCE(error_kind, '') = COALESCE(?26, '')
                  AND COALESCE(timeout_kind, '') = COALESCE(?27, '')
                  AND COALESCE(retry_count, -1) = COALESCE(?28, -1)
             )",
            table = USAGE_SQLITE_TABLE
        ),
        rusqlite::params![
            entry.ts as i64,
            &entry.session,
            &entry.model,
            entry.prompt_tokens as i64,
            entry.completion_tokens as i64,
            entry.total_tokens as i64,
            entry.cache_hit_tokens as i64,
            entry.cache_miss_tokens as i64,
            entry.cost_usd,
            &entry.stable_prefix_hash,
            &entry.system_hash,
            &entry.tool_schema_hash,
            &entry.dynamic_tail_hash,
            &entry.miss_reason,
            &entry.miss_reason_detail,
            &entry.request_phase,
            entry.effective_output_cap.map(i64::from),
            entry.tool_schema_tokens as i64,
            entry.tool_round_count.map(|v| v as i64),
            &entry.compaction_decision,
            &entry.request_id,
            &entry.provider,
            entry.latency_ms.map(|v| v as i64),
            entry.time_to_first_token_ms.map(|v| v as i64),
            &entry.finish_reason,
            &entry.error_kind,
            &entry.timeout_kind,
            entry.retry_count.map(|v| v as i32),
        ],
    )
}

/// Rebuild SQLite projection from JSONL. Useful after corruption or schema changes.
pub fn rebuild_usage_sqlite_from_jsonl() -> io::Result<usize> {
    let path = usage_sqlite_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| io::Error::other(format!("sqlite open: {e}")))?;
    ensure_usage_sqlite(&conn).map_err(|e| io::Error::other(format!("sqlite schema: {e}")))?;
    // Truncate before rebuild.
    conn.execute(
        &format!("DELETE FROM {table}", table = USAGE_SQLITE_TABLE),
        [],
    )
    .map_err(|e| io::Error::other(format!("sqlite delete: {e}")))?;

    let jsonl_path = default_usage_ledger_path();
    let mut count = 0;
    let file = match File::open(&jsonl_path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    for line in io::BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<UsageLedgerEntry>(&line) {
            insert_usage_sqlite_entry(&conn, &entry)
                .map_err(|e| io::Error::other(format!("sqlite insert: {e}")))?;
            count += 1;
        }
    }
    Ok(count)
}

/// Query SQLite-projected usage summary by session.
pub fn query_usage_by_session(session: &str) -> io::Result<UsageLedgerSummary> {
    query_usage_sqlite(Some(session), None)
}

/// Query SQLite-projected usage summary overall.
pub fn query_usage_overall() -> io::Result<UsageLedgerSummary> {
    query_usage_sqlite(None, None)
}

fn query_usage_sqlite(
    session_filter: Option<&str>,
    model_filter: Option<&str>,
) -> io::Result<UsageLedgerSummary> {
    let path = usage_sqlite_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = match rusqlite::Connection::open(&path) {
        Ok(conn) => conn,
        Err(_) => return Ok(UsageLedgerSummary::default()),
    };
    ensure_usage_sqlite(&conn).map_err(|e| io::Error::other(format!("sqlite schema: {e}")))?;
    let mut sql = format!(
        "SELECT COUNT(*), COALESCE(SUM(prompt_tokens),0), COALESCE(SUM(completion_tokens),0),
         COALESCE(SUM(total_tokens),0), COALESCE(SUM(cache_hit_tokens),0),
         COALESCE(SUM(cache_miss_tokens),0), COALESCE(SUM(cost_usd),0.0),
         COALESCE(SUM(tool_schema_tokens),0),
         COALESCE(SUM(CASE WHEN effective_output_cap IS NOT NULL THEN 1 ELSE 0 END),0)
         FROM {table}",
        table = USAGE_SQLITE_TABLE
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    let mut conditions = Vec::new();
    if let Some(s) = session_filter {
        conditions.push(format!("session = ?{}", params.len() + 1));
        params.push(Box::new(s.to_string()));
    }
    if let Some(m) = model_filter {
        conditions.push(format!("model = ?{}", params.len() + 1));
        params.push(Box::new(m.to_string()));
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| io::Error::other(format!("sqlite: {e}")))?;
    let mut summary = UsageLedgerSummary {
        path,
        ..UsageLedgerSummary::default()
    };
    if let Ok(row) = stmt.query_row(param_refs.as_slice(), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, f64>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, i64>(8)?,
        ))
    }) {
        summary.entries = row.0 as u64;
        summary.prompt_tokens = row.1 as u64;
        summary.completion_tokens = row.2 as u64;
        summary.total_tokens = row.3 as u64;
        summary.cache_hit_tokens = row.4 as u64;
        summary.cache_miss_tokens = row.5 as u64;
        summary.cost_usd = row.6;
        summary.tool_schema_tokens = row.7 as u64;
        summary.capped_requests = row.8 as u64;
        summary.hit_rate = prompt_cache_hit_rate(summary.prompt_tokens, summary.cache_hit_tokens);
    }
    populate_usage_sqlite_model_summaries(&conn, &mut summary, session_filter, model_filter)?;
    summary.last_miss_reason =
        query_usage_sqlite_last_miss_reason(&conn, session_filter, model_filter)?;
    Ok(summary)
}

fn append_usage_sqlite_conditions(
    sql: &mut String,
    session_filter: Option<&str>,
    model_filter: Option<&str>,
    params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
) {
    let mut conditions = Vec::new();
    if let Some(s) = session_filter {
        conditions.push(format!("session = ?{}", params.len() + 1));
        params.push(Box::new(s.to_string()));
    }
    if let Some(m) = model_filter {
        conditions.push(format!("model = ?{}", params.len() + 1));
        params.push(Box::new(m.to_string()));
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
}

fn populate_usage_sqlite_model_summaries(
    conn: &rusqlite::Connection,
    summary: &mut UsageLedgerSummary,
    session_filter: Option<&str>,
    model_filter: Option<&str>,
) -> io::Result<()> {
    let mut sql = format!(
        "SELECT model, COUNT(*), COALESCE(SUM(prompt_tokens),0), COALESCE(SUM(completion_tokens),0),
         COALESCE(SUM(total_tokens),0), COALESCE(SUM(cache_hit_tokens),0),
         COALESCE(SUM(cache_miss_tokens),0), COALESCE(SUM(cost_usd),0.0),
         COALESCE(SUM(tool_schema_tokens),0),
         COALESCE(SUM(CASE WHEN effective_output_cap IS NOT NULL THEN 1 ELSE 0 END),0)
         FROM {table}",
        table = USAGE_SQLITE_TABLE
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    append_usage_sqlite_conditions(&mut sql, session_filter, model_filter, &mut params);
    sql.push_str(" GROUP BY model ORDER BY model");
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| io::Error::other(format!("sqlite: {e}")))?;
    let mut rows = stmt
        .query(param_refs.as_slice())
        .map_err(|e| io::Error::other(format!("sqlite query: {e}")))?;
    while let Some(row) = rows
        .next()
        .map_err(|e| io::Error::other(format!("sqlite row: {e}")))?
    {
        let model: String = row
            .get(0)
            .map_err(|e| io::Error::other(format!("sqlite model: {e}")))?;
        summary.by_model.insert(
            model,
            UsageLedgerModelSummary {
                requests: row.get::<_, i64>(1).unwrap_or_default() as u64,
                prompt_tokens: row.get::<_, i64>(2).unwrap_or_default() as u64,
                completion_tokens: row.get::<_, i64>(3).unwrap_or_default() as u64,
                total_tokens: row.get::<_, i64>(4).unwrap_or_default() as u64,
                cache_hit_tokens: row.get::<_, i64>(5).unwrap_or_default() as u64,
                cache_miss_tokens: row.get::<_, i64>(6).unwrap_or_default() as u64,
                cost_usd: row.get::<_, f64>(7).unwrap_or_default(),
                tool_schema_tokens: row.get::<_, i64>(8).unwrap_or_default() as u64,
                capped_requests: row.get::<_, i64>(9).unwrap_or_default() as u64,
            },
        );
    }
    Ok(())
}

fn query_usage_sqlite_last_miss_reason(
    conn: &rusqlite::Connection,
    session_filter: Option<&str>,
    model_filter: Option<&str>,
) -> io::Result<Option<String>> {
    let mut sql = format!(
        "SELECT miss_reason FROM {table}",
        table = USAGE_SQLITE_TABLE
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut conditions = vec![
        "miss_reason IS NOT NULL".to_string(),
        "miss_reason != ''".to_string(),
    ];
    if let Some(s) = session_filter {
        conditions.push(format!("session = ?{}", params.len() + 1));
        params.push(Box::new(s.to_string()));
    }
    if let Some(m) = model_filter {
        conditions.push(format!("model = ?{}", params.len() + 1));
        params.push(Box::new(m.to_string()));
    }
    sql.push_str(" WHERE ");
    sql.push_str(&conditions.join(" AND "));
    sql.push_str(" ORDER BY ts DESC LIMIT 1");
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| io::Error::other(format!("sqlite: {e}")))?;
    match stmt.query_row(param_refs.as_slice(), |row| row.get::<_, String>(0)) {
        Ok(reason) => Ok(Some(reason)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(io::Error::other(format!("sqlite last miss reason: {err}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn appends_and_summarizes_jsonl_usage() {
        let dir = std::env::temp_dir().join(format!(
            "priority-agent-usage-ledger-{}",
            uuid::Uuid::new_v4()
        ));
        let path = dir.join("usage.jsonl");
        let entry = UsageLedgerEntry {
            ts: 1,
            session: "session-a".to_string(),
            model: "kimi-k2.5".to_string(),
            prompt_tokens: 1000,
            completion_tokens: 50,
            total_tokens: 1050,
            cache_hit_tokens: 800,
            cache_miss_tokens: 200,
            cost_usd: 0.001,
            stable_prefix_hash: Some("prefix".to_string()),
            system_hash: Some("system".to_string()),
            tool_schema_hash: Some("tools".to_string()),
            dynamic_tail_hash: Some("tail".to_string()),
            miss_reason: Some("dynamic-tail-changed".to_string()),
            miss_reason_detail: Some("tail changed".to_string()),
            request_phase: Some("coding".to_string()),
            effective_output_cap: Some(8192),
            tool_schema_tokens: 320,
            tool_round_count: Some(2),
            compaction_decision: Some("skipped".to_string()),
            request_id: None,
            provider: None,
            latency_ms: None,
            time_to_first_token_ms: None,
            finish_reason: None,
            error_kind: None,
            timeout_kind: None,
            retry_count: None,
        };
        append_usage_ledger_entry_at(&path, &entry).unwrap();
        append_usage_ledger_entry_at(
            &path,
            &UsageLedgerEntry {
                session: "session-b".to_string(),
                ..entry.clone()
            },
        )
        .unwrap();

        let summary = summarize_usage_ledger_at(&path, Some("session-a")).unwrap();
        assert_eq!(summary.entries, 1);
        assert_eq!(summary.prompt_tokens, 1000);
        assert_eq!(summary.cache_hit_tokens, 800);
        assert_eq!(summary.cache_miss_tokens, 200);
        assert_eq!(summary.tool_schema_tokens, 320);
        assert_eq!(summary.capped_requests, 1);
        assert!((summary.hit_rate - 0.8).abs() < f64::EPSILON);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sqlite_projection_syncs_and_queries() {
        let _guard = env_test_lock();
        let dir = std::env::temp_dir().join(format!(
            "priority-agent-usage-sqlite-{}",
            uuid::Uuid::new_v4()
        ));
        let jsonl_path = dir.join("usage.jsonl");
        let entry = UsageLedgerEntry {
            ts: 1000,
            session: "session-sql".to_string(),
            model: "test-model".to_string(),
            prompt_tokens: 500,
            completion_tokens: 100,
            total_tokens: 600,
            cache_hit_tokens: 400,
            cache_miss_tokens: 100,
            cost_usd: 0.002,
            stable_prefix_hash: Some("abc".to_string()),
            system_hash: None,
            tool_schema_hash: None,
            dynamic_tail_hash: None,
            miss_reason: Some("cold-start".to_string()),
            miss_reason_detail: None,
            request_phase: Some("coding".to_string()),
            effective_output_cap: Some(8192),
            tool_schema_tokens: 123,
            tool_round_count: Some(1),
            compaction_decision: Some("skipped".to_string()),
            request_id: None,
            provider: None,
            latency_ms: None,
            time_to_first_token_ms: None,
            finish_reason: None,
            error_kind: None,
            timeout_kind: None,
            retry_count: None,
        };
        append_usage_ledger_entry_at(&jsonl_path, &entry).unwrap();

        // Sync to SQLite via JSONL path env override.
        std::env::set_var(USAGE_LEDGER_ENV, jsonl_path.to_str().unwrap());
        sync_usage_to_sqlite(&entry);
        sync_usage_to_sqlite(&entry);

        let summary = query_usage_by_session("session-sql").unwrap();
        assert_eq!(summary.entries, 1);
        assert_eq!(summary.prompt_tokens, 500);
        assert_eq!(summary.cache_hit_tokens, 400);
        assert_eq!(summary.tool_schema_tokens, 123);
        assert_eq!(summary.capped_requests, 1);
        assert!((summary.hit_rate - 0.8).abs() < f64::EPSILON);
        assert_eq!(summary.cost_usd, 0.002);
        assert_eq!(summary.last_miss_reason.as_deref(), Some("cold-start"));
        assert_eq!(
            summary
                .by_model
                .get("test-model")
                .map(|model| model.requests),
            Some(1)
        );

        // Rebuild from JSONL should not duplicate.
        let count = rebuild_usage_sqlite_from_jsonl().unwrap();
        assert_eq!(count, 1);
        let rebuilt = query_usage_by_session("session-sql").unwrap();
        assert_eq!(rebuilt.entries, 1);
        assert_eq!(rebuilt.prompt_tokens, 500);

        // Non-existent session returns empty.
        let empty = query_usage_by_session("nonexistent").unwrap();
        assert_eq!(empty.entries, 0);

        std::env::remove_var(USAGE_LEDGER_ENV);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sqlite_projection_query_on_empty_db_returns_empty_summary() {
        let _guard = env_test_lock();
        let dir = std::env::temp_dir().join(format!(
            "priority-agent-usage-empty-sqlite-{}",
            uuid::Uuid::new_v4()
        ));
        let jsonl_path = dir.join("usage.jsonl");
        std::env::set_var(USAGE_LEDGER_ENV, jsonl_path.to_str().unwrap());

        let summary = query_usage_by_session("missing-session").unwrap();
        assert_eq!(summary.entries, 0);
        assert_eq!(summary.hit_rate, 0.0);
        assert!(summary.by_model.is_empty());
        assert!(summary.last_miss_reason.is_none());

        std::env::remove_var(USAGE_LEDGER_ENV);
        let _ = std::fs::remove_dir_all(dir);
    }
}
