//! Lightweight key-value preference store for volatile UI state.
//!
//! Backed by a JSON file in the user's config directory. This is intentionally
//! separate from `AppConfig` so that unstable UI-only preferences do not pollute
//! the main runtime/project configuration.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;

pub struct KvStore {
    path: PathBuf,
    data: serde_json::Map<String, Value>,
    persist: bool,
}

impl KvStore {
    /// Load the store from the default config path, creating it if absent.
    pub fn load() -> Result<Self> {
        let path = Self::default_path()?;
        let data = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read KV store at {:?}", path))?;
            serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse KV store at {:?}", path))?
        } else {
            serde_json::Map::new()
        };
        Ok(Self {
            path,
            data,
            persist: true,
        })
    }

    /// Create an in-memory store for tests.
    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::from("memory"),
            data: serde_json::Map::new(),
            persist: false,
        }
    }

    /// Persist the current data to disk. In-memory stores are not saved.
    pub fn save(&self) -> Result<()> {
        if !self.persist {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create KV store directory {:?}", parent))?;
        }
        let content =
            serde_json::to_string_pretty(&self.data).context("Failed to serialize KV store")?;
        std::fs::write(&self.path, content)
            .with_context(|| format!("Failed to write KV store to {:?}", self.path))?;
        Ok(())
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.data
            .get(key)
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    pub fn set_string(&mut self, key: &str, value: &str) -> Result<()> {
        self.data
            .insert(key.to_string(), Value::String(value.to_string()));
        self.save()
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.data.get(key).and_then(|v| v.as_bool())
    }

    pub fn set_bool(&mut self, key: &str, value: bool) -> Result<()> {
        self.data.insert(key.to_string(), Value::Bool(value));
        self.save()
    }

    pub fn remove(&mut self, key: &str) -> Result<()> {
        self.data.remove(key);
        self.save()
    }

    fn default_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().context("Could not determine config directory for KV store")?;
        Ok(config_dir
            .join("priority-agent")
            .join("ui_preferences.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_roundtrip() {
        let mut kv = KvStore::in_memory();
        assert!(kv.get_string("theme").is_none());
        kv.set_string("theme", "nord").unwrap();
        assert_eq!(kv.get_string("theme"), Some("nord".to_string()));
    }

    #[test]
    fn bool_roundtrip() {
        let mut kv = KvStore::in_memory();
        assert!(kv.get_bool("expand_tools").is_none());
        kv.set_bool("expand_tools", true).unwrap();
        assert_eq!(kv.get_bool("expand_tools"), Some(true));
    }

    #[test]
    fn remove_key() {
        let mut kv = KvStore::in_memory();
        kv.set_string("key", "value").unwrap();
        kv.remove("key").unwrap();
        assert!(kv.get_string("key").is_none());
    }
}
