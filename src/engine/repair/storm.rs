//! Storm breaker — detect and suppress repeated tool calls.
//!
//! Tracks a sliding window of recent (tool_name, normalized_args) pairs.
//! If the same call appears ≥3 times in the window, the agent is stuck in
//! a repeat loop and the call is suppressed with an error result.

use serde_json::Value;
use std::collections::VecDeque;

/// Storm breaker state for a single conversation turn.
#[derive(Debug, Clone)]
pub struct StormState {
    /// Sliding window of recent (tool_name, normalized_args_hash).
    window: VecDeque<(String, u64)>,
    /// Maximum window size.
    capacity: usize,
    /// Suppress after this many repeats in the window.
    threshold: usize,
}

impl Default for StormState {
    fn default() -> Self {
        Self {
            window: VecDeque::new(),
            capacity: 6,
            threshold: 3,
        }
    }
}

impl StormState {
    pub fn new(capacity: usize, threshold: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(capacity),
            capacity,
            threshold,
        }
    }

    /// Check a tool call against the storm window.
    ///
    /// Returns `StormDecision::Allow` if the call should proceed,
    /// or `StormDecision::Suppress` with a reason if it should be skipped.
    pub fn check(&mut self, tool_name: &str, args: &Value) -> StormDecision {
        let hash = normalize_and_hash_args(args);
        let key = (tool_name.to_string(), hash);

        // Count occurrences of this exact (name, args_hash) in the window.
        let count = self
            .window
            .iter()
            .filter(|(n, h)| n == tool_name && *h == hash)
            .count();

        // Push the new call into the window.
        self.window.push_back(key);
        if self.window.len() > self.capacity {
            self.window.pop_front();
        }

        if count >= self.threshold {
            StormDecision::Suppress(format!(
                "detected repeated call to `{}` with same arguments ({} times in window of {}); stopping to avoid storm",
                tool_name, count, self.capacity
            ))
        } else {
            StormDecision::Allow
        }
    }

    /// Reset the window (e.g., at the start of a new turn).
    pub fn reset(&mut self) {
        self.window.clear();
    }

    /// Current window size (for diagnostics).
    pub fn len(&self) -> usize {
        self.window.len()
    }

    /// Whether the window is empty.
    pub fn is_empty(&self) -> bool {
        self.window.is_empty()
    }
}

/// Decision from the storm breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StormDecision {
    /// Call should proceed.
    Allow,
    /// Call should be suppressed with this reason.
    Suppress(String),
}

/// Normalize tool arguments for comparison and compute a stable hash.
///
/// - Sorts object keys for deterministic comparison.
/// - Strips whitespace-insignificant differences.
/// - Returns a u64 hash.
fn normalize_and_hash_args(args: &Value) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let normalized = normalize_value(args);
    let json = serde_json::to_string(&normalized).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    json.hash(&mut hasher);
    hasher.finish()
}

/// Recursively normalize a JSON value for comparison.
fn normalize_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut normalized = serde_json::Map::new();
            for key in keys {
                if let Some(val) = map.get(key) {
                    normalized.insert(key.clone(), normalize_value(val));
                }
            }
            Value::Object(normalized)
        }
        Value::String(s) => {
            // Normalize whitespace in strings for comparison purposes.
            let trimmed = s.trim();
            Value::String(trimmed.to_string())
        }
        Value::Array(arr) => Value::Array(arr.iter().map(normalize_value).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn storm_allows_first_occurrences() {
        let mut state = StormState::default();
        assert_eq!(
            state.check("file_read", &json!({"path": "/tmp/a.txt"})),
            StormDecision::Allow
        );
        assert_eq!(
            state.check("file_read", &json!({"path": "/tmp/a.txt"})),
            StormDecision::Allow
        );
    }

    #[test]
    fn storm_suppresses_repeated_calls() {
        let mut state = StormState::default();
        let args = json!({"path": "/tmp/a.txt"});

        // First 3 calls allowed.
        assert_eq!(state.check("file_read", &args), StormDecision::Allow);
        assert_eq!(state.check("file_read", &args), StormDecision::Allow);
        assert_eq!(state.check("file_read", &args), StormDecision::Allow);
        // 4th identical call suppressed (3 already in window).
        assert!(matches!(
            state.check("file_read", &args),
            StormDecision::Suppress(_)
        ));
    }

    #[test]
    fn storm_different_args_not_suppressed() {
        let mut state = StormState::default();
        assert_eq!(
            state.check("file_read", &json!({"path": "/tmp/a.txt"})),
            StormDecision::Allow
        );
        assert_eq!(
            state.check("file_read", &json!({"path": "/tmp/b.txt"})),
            StormDecision::Allow
        );
        assert_eq!(
            state.check("file_read", &json!({"path": "/tmp/c.txt"})),
            StormDecision::Allow
        );
        // Different args don't suppress.
        assert_eq!(
            state.check("file_read", &json!({"path": "/tmp/d.txt"})),
            StormDecision::Allow
        );
    }

    #[test]
    fn storm_different_tools_not_suppressed() {
        let mut state = StormState::default();
        let args = json!({"path": "/tmp/a.txt"});
        assert_eq!(state.check("file_read", &args), StormDecision::Allow);
        assert_eq!(state.check("glob", &args), StormDecision::Allow);
        assert_eq!(state.check("grep", &args), StormDecision::Allow);
        assert_eq!(state.check("file_read", &args), StormDecision::Allow);
        assert_eq!(state.check("glob", &args), StormDecision::Allow);
        assert_eq!(state.check("grep", &args), StormDecision::Allow);
        // Different tools, same args — not suppressed.
        assert_eq!(state.check("file_read", &args), StormDecision::Allow);
    }

    #[test]
    fn storm_window_evicts_old_entries() {
        let mut state = StormState::new(4, 3);

        // Fill with different calls.
        state.check("a", &json!({}));
        state.check("b", &json!({}));
        state.check("c", &json!({}));
        state.check("d", &json!({}));
        // Call a-gain after eviction — should not be suppressed.
        assert_eq!(state.check("a", &json!({})), StormDecision::Allow);
    }

    #[test]
    fn storm_reset_clears_window() {
        let mut state = StormState::default();
        let args = json!({"path": "/tmp/a.txt"});
        state.check("file_read", &args);
        state.check("file_read", &args);
        state.check("file_read", &args);

        state.reset();
        // After reset, same call should be allowed.
        assert_eq!(state.check("file_read", &args), StormDecision::Allow);
    }
}
