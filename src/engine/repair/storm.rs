//! Storm breaker — detect and suppress repeated tool calls.
//!
//! Tracks a sliding window of recent (tool_name, normalized_args) pairs.
//! If the same call appears ≥3 times in the window, the agent is stuck in
//! a repeat loop and the call is suppressed with an error result.
//! Reasonix-style storm exemptions should be declared by the tool registry
//! for cheap polling/inspection calls that are intentionally repeatable.

use serde_json::Value;
use std::collections::VecDeque;

/// Storm breaker state for a single conversation turn.
#[derive(Debug, Clone)]
pub struct StormState {
    /// Sliding window of recent calls.
    window: VecDeque<RecentCall>,
    /// Maximum window size.
    capacity: usize,
    /// Suppress after this many repeats in the window.
    threshold: usize,
}

#[derive(Debug, Clone)]
struct RecentCall {
    tool_name: String,
    args_hash: u64,
    read_only: bool,
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
    pub fn check(
        &mut self,
        tool_name: &str,
        args: &Value,
        read_only: bool,
        storm_exempt: bool,
    ) -> StormDecision {
        if storm_exempt {
            return StormDecision::Allow;
        }

        let hash = normalize_and_hash_args(args);

        if !read_only {
            self.window.retain(|entry| !entry.read_only);
        }

        // Count occurrences of this exact (name, args_hash) in the window.
        let count = self
            .window
            .iter()
            .filter(|entry| entry.tool_name == tool_name && entry.args_hash == hash)
            .count();

        if count >= self.threshold.saturating_sub(1) {
            return StormDecision::Suppress(format!(
                "detected repeated call to `{}` with same arguments ({} times in window of {}); stopping to avoid storm",
                tool_name,
                count + 1,
                self.capacity
            ));
        }

        // Push the new call into the window.
        self.window.push_back(RecentCall {
            tool_name: tool_name.to_string(),
            args_hash: hash,
            read_only,
        });
        if self.window.len() > self.capacity {
            self.window.pop_front();
        }

        StormDecision::Allow
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
        Value::String(s) => Value::String(s.clone()),
        Value::Array(arr) => Value::Array(arr.iter().map(normalize_value).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read(state: &mut StormState, name: &str, args: &Value) -> StormDecision {
        state.check(name, args, true, false)
    }

    fn read_exempt(state: &mut StormState, name: &str, args: &Value) -> StormDecision {
        state.check(name, args, true, true)
    }

    fn write(state: &mut StormState, name: &str, args: &Value) -> StormDecision {
        state.check(name, args, false, false)
    }

    #[test]
    fn storm_allows_first_occurrences() {
        let mut state = StormState::default();
        assert_eq!(
            read(&mut state, "file_read", &json!({"path": "/tmp/a.txt"})),
            StormDecision::Allow
        );
        assert_eq!(
            read(&mut state, "file_read", &json!({"path": "/tmp/a.txt"})),
            StormDecision::Allow
        );
    }

    #[test]
    fn storm_suppresses_repeated_non_exempt_read_only_calls() {
        let mut state = StormState::default();
        let args = json!({"path": "/tmp/a.txt"});

        assert_eq!(read(&mut state, "grep", &args), StormDecision::Allow);
        assert_eq!(read(&mut state, "grep", &args), StormDecision::Allow);
        assert!(matches!(
            read(&mut state, "grep", &args),
            StormDecision::Suppress(_)
        ));
    }

    #[test]
    fn storm_allows_repeated_exempt_read_only_calls() {
        let mut state = StormState::default();
        let args = json!({"path": "/tmp/a.txt"});

        for _ in 0..10 {
            assert_eq!(
                read_exempt(&mut state, "file_read", &args),
                StormDecision::Allow
            );
        }
    }

    #[test]
    fn storm_suppresses_repeated_mutating_calls() {
        let mut state = StormState::default();
        let args = json!({"path": "/tmp/a.txt"});

        assert_eq!(write(&mut state, "file_edit", &args), StormDecision::Allow);
        assert_eq!(write(&mut state, "file_edit", &args), StormDecision::Allow);
        assert!(matches!(
            write(&mut state, "file_edit", &args),
            StormDecision::Suppress(_)
        ));
    }

    #[test]
    fn storm_different_args_not_suppressed() {
        let mut state = StormState::default();
        assert_eq!(
            read(&mut state, "file_read", &json!({"path": "/tmp/a.txt"})),
            StormDecision::Allow
        );
        assert_eq!(
            read(&mut state, "file_read", &json!({"path": "/tmp/b.txt"})),
            StormDecision::Allow
        );
        assert_eq!(
            read(&mut state, "file_read", &json!({"path": "/tmp/c.txt"})),
            StormDecision::Allow
        );
        // Different args don't suppress.
        assert_eq!(
            read(&mut state, "file_read", &json!({"path": "/tmp/d.txt"})),
            StormDecision::Allow
        );
    }

    #[test]
    fn exact_duplicate_read_only_suppresses_without_blocking_new_ranges() {
        let mut state = StormState::default();
        let same = json!({"path": "README.md", "offset": 0, "limit": 80});

        assert_eq!(read(&mut state, "file_read", &same), StormDecision::Allow);
        assert_eq!(read(&mut state, "file_read", &same), StormDecision::Allow);
        assert!(matches!(
            read(&mut state, "file_read", &same),
            StormDecision::Suppress(_)
        ));

        assert_eq!(
            read(
                &mut state,
                "file_read",
                &json!({"path": "README.md", "offset": 80, "limit": 80})
            ),
            StormDecision::Allow
        );
        assert_eq!(
            read(
                &mut state,
                "file_read",
                &json!({"path": "README.md", "offset": 160, "limit": 80})
            ),
            StormDecision::Allow
        );
    }

    #[test]
    fn storm_counts_repeats_per_tool() {
        let mut state = StormState::default();
        let args = json!({"path": "/tmp/a.txt"});
        assert_eq!(read(&mut state, "file_read", &args), StormDecision::Allow);
        assert_eq!(read(&mut state, "glob", &args), StormDecision::Allow);
        assert_eq!(read(&mut state, "grep", &args), StormDecision::Allow);
        assert_eq!(read(&mut state, "file_read", &args), StormDecision::Allow);
        assert_eq!(read(&mut state, "glob", &args), StormDecision::Allow);
        assert_eq!(read(&mut state, "grep", &args), StormDecision::Allow);
        assert!(matches!(
            read(&mut state, "file_read", &args),
            StormDecision::Suppress(_)
        ));
    }

    #[test]
    fn storm_window_evicts_old_entries() {
        let mut state = StormState::new(4, 3);

        // Fill with different calls.
        read(&mut state, "a", &json!({}));
        read(&mut state, "b", &json!({}));
        read(&mut state, "c", &json!({}));
        read(&mut state, "d", &json!({}));
        // Call a-gain after eviction — should not be suppressed.
        assert_eq!(read(&mut state, "a", &json!({})), StormDecision::Allow);
    }

    #[test]
    fn storm_reset_clears_window() {
        let mut state = StormState::default();
        let args = json!({"path": "/tmp/a.txt"});
        read(&mut state, "file_read", &args);
        read(&mut state, "file_read", &args);

        state.reset();
        // After reset, same call should be allowed.
        assert_eq!(read(&mut state, "file_read", &args), StormDecision::Allow);
    }

    #[test]
    fn mutating_call_clears_prior_read_only_entries() {
        let mut state = StormState::default();
        let args = json!({"path": "/tmp/a.txt"});
        assert_eq!(read(&mut state, "file_read", &args), StormDecision::Allow);
        assert_eq!(read(&mut state, "file_read", &args), StormDecision::Allow);
        assert_eq!(write(&mut state, "file_edit", &args), StormDecision::Allow);
        assert_eq!(read(&mut state, "file_read", &args), StormDecision::Allow);
    }

    #[test]
    fn storm_exempt_call_does_not_count() {
        let mut state = StormState::default();
        let args = json!({"question": "continue?"});
        assert_eq!(
            state.check("ask_user", &args, false, true),
            StormDecision::Allow
        );
        assert_eq!(
            state.check("ask_user", &args, false, true),
            StormDecision::Allow
        );
        assert_eq!(
            state.check("ask_user", &args, false, true),
            StormDecision::Allow
        );
    }
}
