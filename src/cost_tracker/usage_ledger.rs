use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const USAGE_LEDGER_ENV: &str = "PRIORITY_AGENT_USAGE_LEDGER_PATH";

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
    if cfg!(test) && std::env::var_os(USAGE_LEDGER_ENV).is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!((summary.hit_rate - 0.8).abs() < f64::EPSILON);

        let _ = std::fs::remove_dir_all(dir);
    }
}
