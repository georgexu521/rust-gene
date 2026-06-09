//! JSON 工具
//!
//! JSON 查询、格式化、验证

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

/// JSON 查询工具
pub struct JsonQueryTool;

#[async_trait]
impl Tool for JsonQueryTool {
    fn name(&self) -> &str {
        "json_query"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Search
    }

    fn description(&self) -> &str {
        "Query and manipulate JSON data using dot-notation paths. \
         Supports extracting values, filtering arrays, and formatting."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get", "set", "format", "validate"],
                    "description": "Action: 'get' (extract value), 'set' (modify value), \
                                   'format' (pretty print), 'validate' (check syntax)"
                },
                "json": {
                    "type": "string",
                    "description": "JSON string to operate on"
                },
                "path": {
                    "type": "string",
                    "description": "Dot-notation path (e.g., 'user.name', 'items[0].id')"
                },
                "value": {
                    "type": "string",
                    "description": "New value as JSON string (for 'set' action)"
                }
            },
            "required": ["action", "json"]
        })
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("get");
        let json_str = params["json"].as_str().unwrap_or("");

        if json_str.is_empty() {
            return ToolResult::error("JSON string cannot be empty");
        }

        let data: Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => return ToolResult::error(format!("Invalid JSON: {}", e)),
        };

        match action {
            "get" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return ToolResult::success(
                        serde_json::to_string_pretty(&data).unwrap_or_default(),
                    );
                }

                match get_value_by_path(&data, path) {
                    Some(value) => ToolResult::success_with_data(
                        serde_json::to_string_pretty(&value).unwrap_or_default(),
                        json!({
                            "path": path,
                            "value": value
                        }),
                    ),
                    None => ToolResult::error(format!("Path '{}' not found", path)),
                }
            }
            "set" => {
                let path = params["path"].as_str().unwrap_or("");
                let value_str = params["value"].as_str().unwrap_or("null");

                if path.is_empty() {
                    return ToolResult::error("Path is required for 'set' action");
                }

                let new_value: Value = match serde_json::from_str(value_str) {
                    Ok(v) => v,
                    Err(e) => return ToolResult::error(format!("Invalid value JSON: {}", e)),
                };

                let mut modified = data.clone();
                if set_value_by_path(&mut modified, path, new_value) {
                    ToolResult::success_with_data(
                        serde_json::to_string_pretty(&modified).unwrap_or_default(),
                        json!({
                            "path": path,
                            "modified": true
                        }),
                    )
                } else {
                    ToolResult::error(format!("Failed to set path '{}'", path))
                }
            }
            "format" => {
                ToolResult::success(serde_json::to_string_pretty(&data).unwrap_or_default())
            }
            "validate" => ToolResult::success_with_data(
                "Valid JSON".to_string(),
                json!({
                    "valid": true,
                    "type": match data {
                        Value::Object(_) => "object",
                        Value::Array(_) => "array",
                        Value::String(_) => "string",
                        Value::Number(_) => "number",
                        Value::Bool(_) => "boolean",
                        Value::Null => "null",
                    }
                }),
            ),
            _ => ToolResult::error(format!("Unknown action: {}", action)),
        }
    }
}

fn get_value_by_path(data: &Value, path: &str) -> Option<Value> {
    let mut current = data;

    for segment in path.split('.') {
        // Handle array access: items[0]
        if let Some(bracket_pos) = segment.find('[') {
            // Invalid bracket syntax, e.g. "items[" or "items]".
            if !segment.ends_with(']') || bracket_pos + 1 >= segment.len() {
                return None;
            }
            let key = &segment[..bracket_pos];
            let idx_str = &segment[bracket_pos + 1..segment.len() - 1];

            if !key.is_empty() {
                current = current.get(key)?;
            }

            let idx: usize = idx_str.parse().ok()?;
            current = current.get(idx)?;
        } else {
            current = current.get(segment)?;
        }
    }

    Some(current.clone())
}

fn set_value_by_path(data: &mut Value, path: &str, value: Value) -> bool {
    let parts: Vec<&str> = path.split('.').collect();

    if parts.is_empty() {
        return false;
    }

    // Use recursion or a loop with parent tracking instead of mutable reference juggling
    let mut current = data;

    for (i, segment) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        // Handle array access
        if let Some(bracket_pos) = segment.find('[') {
            // Invalid bracket syntax, e.g. "items[" or "items]".
            if !segment.ends_with(']') || bracket_pos + 1 >= segment.len() {
                return false;
            }
            let key = &segment[..bracket_pos];
            let idx_str = &segment[bracket_pos + 1..segment.len() - 1];
            let idx: usize = match idx_str.parse() {
                Ok(n) => n,
                Err(_) => return false,
            };

            if !key.is_empty() {
                if !current.is_object() {
                    return false;
                }
                // Create nested object if missing
                if !current.get(key).is_some_and(|_| true) {
                    if let Some(obj) = current.as_object_mut() {
                        obj.insert(key.to_string(), json!({}));
                    } else {
                        return false;
                    }
                }
                current = match current.get_mut(key) {
                    Some(v) => v,
                    None => return false,
                };
            }

            if is_last {
                if let Some(arr) = current.as_array_mut() {
                    if idx < arr.len() {
                        arr[idx] = value;
                        return true;
                    }
                }
                return false;
            }

            current = match current.get_mut(idx) {
                Some(v) => v,
                None => return false,
            };
        } else if is_last {
            if let Some(obj) = current.as_object_mut() {
                obj.insert(segment.to_string(), value);
                return true;
            }
            return false;
        } else {
            if !current.is_object() {
                return false;
            }
            // Create nested object if missing
            if !current.get(*segment).is_some_and(|_| true) {
                if let Some(obj) = current.as_object_mut() {
                    obj.insert(segment.to_string(), json!({}));
                } else {
                    return false;
                }
            }
            current = match current.get_mut(*segment) {
                Some(v) => v,
                None => return false,
            };
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_json_get() {
        let tool = JsonQueryTool;
        let params = json!({
            "action": "get",
            "json": r#"{"user": {"name": "Alice", "age": 30}}"#,
            "path": "user.name"
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("Alice"));
    }

    #[tokio::test]
    async fn test_json_get_array() {
        let tool = JsonQueryTool;
        let params = json!({
            "action": "get",
            "json": r#"{"items": [{"id": 1}, {"id": 2}]}"#,
            "path": "items[0].id"
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("1"));
    }

    #[tokio::test]
    async fn test_json_format() {
        let tool = JsonQueryTool;
        let params = json!({
            "action": "format",
            "json": r#"{"a":1,"b":2}"#
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("\"a\": 1"));
    }

    #[tokio::test]
    async fn test_json_validate() {
        let tool = JsonQueryTool;
        let params = json!({
            "action": "validate",
            "json": r#"{"valid": true}"#
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("Valid JSON"));
    }
}
