use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySearchDocument {
    pub source: String,
    pub title: String,
    pub content: String,
    pub kind: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemorySearchHit {
    pub source: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct MemorySearchIndex {
    path: PathBuf,
}

impl MemorySearchIndex {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn rebuild(&self, documents: &[MemorySearchDocument]) -> anyhow::Result<usize> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(&self.path)?;
        Self::ensure_schema(&conn)?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM memory_fts", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO memory_fts(source, title, content, kind, scope)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for document in documents {
                if document.content.trim().is_empty() {
                    continue;
                }
                stmt.execute(params![
                    document.source,
                    document.title,
                    document.content,
                    document.kind,
                    document.scope
                ])?;
            }
        }
        tx.commit()?;
        Ok(documents
            .iter()
            .filter(|document| !document.content.trim().is_empty())
            .count())
    }

    pub fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<MemorySearchHit>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let fts_query = fts_query(query);
        if fts_query.is_empty() || !self.path.exists() {
            return Ok(Vec::new());
        }
        let conn = Connection::open(&self.path)?;
        Self::ensure_schema(&conn)?;
        let mut stmt = conn.prepare(
            "SELECT source, title, content, bm25(memory_fts) AS rank
             FROM memory_fts
             WHERE memory_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
            let content: String = row.get(2)?;
            let rank: f64 = row.get(3)?;
            Ok(MemorySearchHit {
                source: row.get(0)?,
                title: row.get(1)?,
                snippet: preview(&content, 800),
                score: (-rank).max(0.01),
            })
        })?;

        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        Ok(hits)
    }

    fn ensure_schema(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
                source UNINDEXED,
                title,
                content,
                kind UNINDEXED,
                scope UNINDEXED,
                tokenize = 'unicode61'
            );",
        )
    }
}

fn fts_query(query: &str) -> String {
    query
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .map(str::trim)
        .filter(|term| term.chars().count() >= 2)
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn preview(content: &str, max_chars: usize) -> String {
    let mut out = content.trim().chars().take(max_chars).collect::<String>();
    if content.trim().chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_and_search_returns_source_and_snippet() {
        let dir = tempfile::tempdir().unwrap();
        let index = MemorySearchIndex::new(dir.path().join("memory-search.sqlite"));
        let documents = vec![MemorySearchDocument {
            source: "memory/build.md".to_string(),
            title: "Build Notes".to_string(),
            content: "Run cargo check after context refactors.".to_string(),
            kind: "topic_file".to_string(),
            scope: "default".to_string(),
        }];

        let indexed = index.rebuild(&documents).unwrap();
        let hits = index.search("cargo check", 5).unwrap();

        assert_eq!(indexed, 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "memory/build.md");
        assert!(hits[0].snippet.contains("cargo check"));
    }

    #[test]
    fn blank_query_returns_no_hits() {
        let dir = tempfile::tempdir().unwrap();
        let index = MemorySearchIndex::new(dir.path().join("memory-search.sqlite"));

        assert!(index.search("\u{0}\n", 5).unwrap().is_empty());
    }
}
