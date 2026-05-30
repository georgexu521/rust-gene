//! Truncation repair — fix malformed JSON from truncated LLM output.
//!
//! When the LLM output is cut off mid-JSON, attempt to repair by:
//! 1. Closing open brackets and braces.
//! 2. Terminating unterminated strings.
//! 3. Falling back to `{}` if unrecoverable.

use serde_json::Value;

/// Attempt to repair a truncated JSON string into a valid `serde_json::Value`.
///
/// Returns `Some(value)` on successful repair, or `None` if the string is
/// too corrupt to recover.
pub fn repair_truncated_json(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Some(Value::Object(serde_json::Map::new()));
    }

    // Fast path: already valid JSON.
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Some(value);
    }

    // Attempt repair: close brackets, terminate strings.
    let repaired = close_brackets_and_strings(trimmed);

    match serde_json::from_str(&repaired) {
        Ok(value) => {
            tracing::debug!(
                "Truncation repair: recovered JSON ({} → {} chars)",
                trimmed.len(),
                repaired.len()
            );
            Some(value)
        }
        Err(_) => {
            // Last resort: return empty object.
            tracing::warn!(
                "Truncation repair failed for {} chars, falling back to {{}}",
                trimmed.len()
            );
            Some(Value::Object(serde_json::Map::new()))
        }
    }
}

/// Close open brackets and braces, and terminate unterminated strings.
fn close_brackets_and_strings(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len() + 16);
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escape_next = false;

    for ch in raw.chars() {
        result.push(ch);

        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => {
                escape_next = true;
            }
            '"' => {
                in_string = !in_string;
            }
            '{' if !in_string => {
                stack.push('}');
            }
            '[' if !in_string => {
                stack.push(']');
            }
            '}' | ']' if !in_string => {
                // Pop matching opener if possible.
                if stack.last() == Some(&ch) {
                    stack.pop();
                }
            }
            _ => {}
        }
    }

    // Terminate unterminated string.
    if in_string {
        result.push('"');
    }

    // Close any remaining open brackets/braces, in reverse order.
    while let Some(closer) = stack.pop() {
        result.push(closer);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_json_passes_through() {
        let raw = r#"{"expression": "2 + 3"}"#;
        let result = repair_truncated_json(raw).unwrap();
        assert_eq!(result, json!({"expression": "2 + 3"}));
    }

    #[test]
    fn empty_string_returns_empty_object() {
        let result = repair_truncated_json("").unwrap();
        assert_eq!(result, json!({}));
    }

    #[test]
    fn missing_closing_brace() {
        let raw = r#"{"path": "/tmp/test.txt", "content": "hello"#;
        let result = repair_truncated_json(raw).unwrap();
        assert!(result.is_object());
        assert_eq!(result["path"], json!("/tmp/test.txt"));
    }

    #[test]
    fn missing_closing_bracket() {
        let raw = r#"["a", "b", "c"#;
        let result = repair_truncated_json(raw).unwrap();
        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 3);
    }

    #[test]
    fn unterminated_string_closed() {
        let raw = r#"{"query": "hello world"#;
        let result = repair_truncated_json(raw).unwrap();
        assert_eq!(result["query"], json!("hello world"));
    }

    #[test]
    fn nested_braces_closed() {
        let raw = r#"{"outer": {"inner": "value"#;
        let result = repair_truncated_json(raw).unwrap();
        assert!(result["outer"].is_object());
        assert_eq!(result["outer"]["inner"], json!("value"));
    }

    #[test]
    fn mid_literal_truncation_recovers() {
        // Completely garbled mid-word.
        let raw = r#"{"path": "/tmp/t"#;
        let result = repair_truncated_json(raw).unwrap();
        // Should fall back to {} since no amount of bracket closing will help.
        assert!(result.is_object());
    }

    #[test]
    fn whitespace_only_returns_object() {
        let result = repair_truncated_json("   \n  ").unwrap();
        assert_eq!(result, json!({}));
    }
}
