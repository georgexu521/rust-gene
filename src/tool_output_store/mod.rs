//! Managed tool output store with URI-based paging.
//!
//! Phase 1 (opencode core alignment): large tool outputs are stored behind
//! `tool-output://<id>` URIs with session-scoped metadata instead of ad-hoc
//! disk files. TUI and desktop can page through full output without re-reading
//! massive inline strings.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// URI scheme prefix for tool output resources.
pub const TOOL_OUTPUT_URI_PREFIX: &str = "tool-output://";

/// Maximum inline content bytes kept in memory before storing to disk.
pub const DEFAULT_STORE_THRESHOLD: usize = 32 * 1024; // 32 KiB

/// Configurable policy for tool output truncation and retention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutputPolicy {
    pub max_bytes: usize,
    pub max_lines: usize,
    pub preview_direction: PreviewDirection,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewDirection {
    Head,
    Tail,
    HeadTail,
}

impl Default for ToolOutputPolicy {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024,
            max_lines: 500,
            preview_direction: PreviewDirection::Tail,
            retention_days: 7,
        }
    }
}

impl ToolOutputPolicy {
    /// Load policy from environment variables.
    pub fn from_env() -> Self {
        let mut policy = Self::default();
        if let Ok(val) = std::env::var("PRIORITY_AGENT_TOOL_OUTPUT_MAX_BYTES") {
            if let Ok(n) = val.parse::<usize>() {
                policy.max_bytes = n;
            }
        }
        if let Ok(val) = std::env::var("PRIORITY_AGENT_TOOL_OUTPUT_MAX_LINES") {
            if let Ok(n) = val.parse::<usize>() {
                policy.max_lines = n;
            }
        }
        if let Ok(val) = std::env::var("PRIORITY_AGENT_TOOL_OUTPUT_PREVIEW") {
            policy.preview_direction = match val.to_lowercase().as_str() {
                "head" => PreviewDirection::Head,
                "head_tail" => PreviewDirection::HeadTail,
                _ => PreviewDirection::Tail,
            };
        }
        if let Ok(val) = std::env::var("PRIORITY_AGENT_TOOL_OUTPUT_RETENTION_DAYS") {
            if let Ok(n) = val.parse::<u32>() {
                policy.retention_days = n;
            }
        }
        policy
    }

    pub fn cleanup_threshold_ms(&self) -> u64 {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        cutoff.saturating_sub((self.retention_days as u64) * 86_400_000)
    }

    pub fn effective_threshold(&self) -> usize {
        self.max_bytes
    }
}

/// Metadata stored alongside each tool output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutputMeta {
    pub id: String,
    pub session_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub mime: String,
    pub original_bytes: u64,
    pub created_at_ms: u64,
}

impl ToolOutputMeta {
    pub fn uri(&self) -> String {
        format!("{TOOL_OUTPUT_URI_PREFIX}{}", self.id)
    }
}

/// The tool output store.
#[derive(Debug, Clone)]
pub struct ToolOutputStore {
    base_dir: PathBuf,
}

impl ToolOutputStore {
    pub fn new() -> Self {
        Self {
            base_dir: Self::default_base_dir(),
        }
    }

    pub fn at(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn default_base_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from(".priority-agent"))
            .join("priority-agent")
            .join("tool-outputs")
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Store or truncate a tool result. If content exceeds the threshold,
    /// the full output is persisted and the result content is replaced with
    /// a preview + URI reference.
    pub async fn truncate_or_store(
        &self,
        session_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        content: &str,
        mime: &str,
    ) -> io::Result<Option<ToolOutputMeta>> {
        if content.len() <= DEFAULT_STORE_THRESHOLD {
            return Ok(None);
        }

        tokio::fs::create_dir_all(&self.base_dir).await?;

        let id = output_id(tool_name, tool_call_id);
        let meta = ToolOutputMeta {
            id: id.clone(),
            session_id: session_id.to_string(),
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            mime: mime.to_string(),
            original_bytes: content.len() as u64,
            created_at_ms: now_ms(),
        };

        let content_path = self.content_path(&id);
        tokio::fs::write(&content_path, content).await?;

        let meta_path = self.meta_path(&id);
        let meta_json = serde_json::to_string_pretty(&meta)?;
        tokio::fs::write(&meta_path, meta_json).await?;

        Ok(Some(meta))
    }

    /// Read a page from a stored output.
    pub fn read_page(
        &self,
        session_id: &str,
        id_or_uri: &str,
        offset: u64,
        limit: u64,
    ) -> io::Result<ToolOutputPage> {
        let id = normalize_id(id_or_uri)?;
        let content_path = self.content_path(id);
        if !content_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("tool output not found: {id}"),
            ));
        }

        let meta = self.read_meta(id)?;
        if meta.session_id != session_id {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("tool output {id} does not belong to session {session_id}"),
            ));
        }
        let total_bytes = meta.original_bytes;

        let mut file = std::fs::File::open(&content_path)?;
        let file_len = file.metadata()?.len();

        if offset >= file_len {
            return Ok(ToolOutputPage {
                content: String::new(),
                offset,
                limit,
                total_bytes,
                has_more: false,
            });
        }

        file.seek(SeekFrom::Start(offset))?;

        let read_limit = limit.min(file_len - offset) as usize;
        let mut buffer = vec![0u8; read_limit];
        let bytes_read = file.read(&mut buffer)?;
        buffer.truncate(bytes_read);

        // UTF-8 safe: truncate incomplete multibyte sequences
        let content = String::from_utf8_lossy(&buffer).to_string();
        let has_more = offset + (bytes_read as u64) < file_len;

        Ok(ToolOutputPage {
            content,
            offset,
            limit,
            total_bytes,
            has_more,
        })
    }

    /// Read metadata for a stored output.
    pub fn read_meta(&self, id: &str) -> io::Result<ToolOutputMeta> {
        let id = normalize_id(id)?;
        let meta_path = self.meta_path(id);
        let text = std::fs::read_to_string(&meta_path)?;
        serde_json::from_str(&text)
            .map_err(|e| io::Error::other(format!("invalid metadata for {id}: {e}")))
    }

    /// List all stored outputs for a session.
    pub fn list_for_session(&self, session_id: &str) -> io::Result<Vec<ToolOutputMeta>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }
        let mut results = Vec::new();
        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && path.file_stem().is_some()
            {
                if let Ok(meta) = serde_json::from_str::<ToolOutputMeta>(
                    &std::fs::read_to_string(&path).unwrap_or_default(),
                ) {
                    if meta.session_id == session_id {
                        results.push(meta);
                    }
                }
            }
        }
        results.sort_by_key(|m| m.created_at_ms);
        Ok(results)
    }

    /// Clean up outputs older than the given epoch timestamp.
    pub fn cleanup_older_than(&self, before_ms: u64) -> io::Result<usize> {
        if !self.base_dir.exists() {
            return Ok(0);
        }
        let mut removed = 0usize;
        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("txt") {
                let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let meta_path = self.meta_path(id);
                if meta_path.exists() {
                    if let Ok(meta) = serde_json::from_str::<ToolOutputMeta>(
                        &std::fs::read_to_string(&meta_path).unwrap_or_default(),
                    ) {
                        if meta.created_at_ms < before_ms {
                            std::fs::remove_file(&path).ok();
                            std::fs::remove_file(&meta_path).ok();
                            removed += 1;
                        }
                    }
                }
            }
        }
        Ok(removed)
    }

    /// Clean up all outputs for a session.
    pub fn cleanup_session(&self, session_id: &str) -> io::Result<usize> {
        let metas = self.list_for_session(session_id)?;
        let mut removed = 0usize;
        for meta in metas {
            std::fs::remove_file(self.content_path(&meta.id)).ok();
            std::fs::remove_file(self.meta_path(&meta.id)).ok();
            removed += 1;
        }
        Ok(removed)
    }

    fn content_path(&self, id: &str) -> PathBuf {
        self.base_dir.join(format!("{id}.txt"))
    }

    fn meta_path(&self, id: &str) -> PathBuf {
        self.base_dir.join(format!("{id}.json"))
    }
}

impl Default for ToolOutputStore {
    fn default() -> Self {
        Self::new()
    }
}

/// A page of tool output for TUI/desktop rendering.
#[derive(Debug, Clone)]
pub struct ToolOutputPage {
    pub content: String,
    pub offset: u64,
    pub limit: u64,
    pub total_bytes: u64,
    pub has_more: bool,
}

/// Build a truncation-aware result preview + URI reference.
pub fn trunched_preview_with_uri(
    original: &str,
    meta: &ToolOutputMeta,
    threshold: usize,
) -> String {
    let original_len = original.len();
    let half = threshold / 2;
    let first = safe_prefix_by_bytes(original, half);
    let last = safe_suffix_by_bytes(original, half);
    format!(
        "[Output truncated: {} bytes → {}]\nFull output: {}\n\n--- First {} bytes ---\n{}\n\n--- Last {} bytes ---\n{}",
        original_len,
        meta.uri(),
        meta.uri(),
        first.len(),
        first,
        last.len(),
        last,
    )
}

/// Build a complete tool result preview string (head + URI + tail).
pub fn build_result_preview(original: &str, meta: &ToolOutputMeta) -> String {
    trunched_preview_with_uri(original, meta, DEFAULT_STORE_THRESHOLD)
}

// ── Helpers ──

fn output_id(tool_name: &str, tool_call_id: &str) -> String {
    let safe_name: String = tool_name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .take(20)
        .collect();
    let safe_id: String = tool_call_id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .take(16)
        .collect();
    format!("{safe_name}_{safe_id}")
}

fn normalize_id(id_or_uri: &str) -> io::Result<&str> {
    let id = id_or_uri
        .strip_prefix(TOOL_OUTPUT_URI_PREFIX)
        .unwrap_or(id_or_uri)
        .trim();
    let valid = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
    if valid {
        Ok(id)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid tool output id: {id_or_uri}"),
        ))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn safe_prefix_by_bytes(text: &str, max_bytes: usize) -> String {
    if text.is_empty() || max_bytes == 0 {
        return String::new();
    }
    text.char_indices()
        .take_while(|(byte_idx, _)| *byte_idx < max_bytes)
        .map(|(_, ch)| ch)
        .collect::<String>()
}

fn safe_suffix_by_bytes(text: &str, max_bytes: usize) -> String {
    if text.is_empty() || max_bytes == 0 {
        return String::new();
    }
    let target_start = text.len().saturating_sub(max_bytes);
    text.char_indices()
        .filter(|(byte_idx, _)| *byte_idx >= target_start)
        .map(|(_, ch)| ch)
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stores_and_reads_page() {
        let dir = std::env::temp_dir().join(format!("tool-store-{}", uuid::Uuid::new_v4()));
        let store = ToolOutputStore::at(dir.clone());

        let content = "0123456789\n".repeat(4096);
        assert!(
            content.len() > DEFAULT_STORE_THRESHOLD,
            "test content must exceed {} bytes",
            DEFAULT_STORE_THRESHOLD
        );
        let meta = store
            .truncate_or_store("sess-1", "call-1", "bash", &content, "text/plain")
            .await
            .unwrap()
            .expect("should store content above threshold");

        assert_eq!(meta.tool_name, "bash");
        assert_eq!(meta.session_id, "sess-1");
        assert!(meta.uri().starts_with("tool-output://"));

        let page = store.read_page("sess-1", &meta.id, 0, 14).unwrap();
        assert!(page.content.starts_with("0123456789"));
        assert!(page.has_more);
        assert_eq!(page.total_bytes, content.len() as u64);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn read_page_rejects_wrong_session() {
        let dir = std::env::temp_dir().join(format!("tool-store-{}", uuid::Uuid::new_v4()));
        let store = ToolOutputStore::at(dir.clone());
        let content = "0123456789\n".repeat(4096);
        let meta = store
            .truncate_or_store("sess-1", "call-1", "bash", &content, "text/plain")
            .await
            .unwrap()
            .unwrap();

        let err = store.read_page("sess-2", &meta.uri(), 0, 14).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn skips_small_output() {
        let dir = std::env::temp_dir().join(format!("tool-store-{}", uuid::Uuid::new_v4()));
        let store = ToolOutputStore::at(dir.clone());

        let short = "short output";
        let meta = store
            .truncate_or_store("sess-1", "call-2", "bash", short, "text/plain")
            .await
            .unwrap();
        assert!(meta.is_none(), "short output should not be stored");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn uri_format_is_stable() {
        assert_eq!(TOOL_OUTPUT_URI_PREFIX, "tool-output://");
    }

    #[test]
    fn safe_prefix_includes_only_complete_chars() {
        assert_eq!(safe_prefix_by_bytes("Hello世界", 5), "Hello");
        assert_eq!(safe_prefix_by_bytes("Hello世界", 8), "Hello世");
    }

    #[test]
    fn safe_suffix_includes_only_complete_chars() {
        assert_eq!(safe_suffix_by_bytes("Hello世界", 3), "界");
    }
}
