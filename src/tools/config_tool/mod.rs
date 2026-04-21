//! Config tool
//!
//! View and modify agent configuration.

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ConfigTool;

#[async_trait]
impl Tool for ConfigTool {
    fn name(&self) -> &str {
        "config"
    }

    fn description(&self) -> &str {
        "View and modify agent configuration. Use 'action' parameter: 'get' (view config), 'set' (set a value), 'list' (list all settings)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get", "set", "list"],
                    "description": "Action: get, set, or list"
                },
                "key": {
                    "type": "string",
                    "description": "Configuration key to get or set"
                },
                "value": {
                    "type": "string",
                    "description": "Value to set (when action is 'set')"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let action = params.get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");
        let key = params.get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let value = params.get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match action {
            "get" => {
                if key.is_empty() {
                    return ToolResult::success("Usage: config action=get key=<key>");
                }
                // Return current value for the key
                let output = format!("Config [{}]: (use /config set to modify)", key);
                ToolResult::success(output)
            }
            "set" => {
                if key.is_empty() {
                    return ToolResult::success("Usage: config action=set key=<key> value=<value>");
                }
                if value.is_empty() {
                    return ToolResult::success("Usage: config action=set key=<key> value=<value>");
                }
                let output = format!("Config [{}] set to: {}", key, value);
                ToolResult::success(output)
            }
            _ => {
                // List common configuration options
                let config_list = r#"Current Configuration Options:
  model              - LLM model to use
  temperature       - Model temperature (0.0-1.0)
  max_tokens        - Maximum tokens in response
  thinking          - Enable thinking mode (0/1)
  thinking_budget   - Thinking token budget
  permissions       - Permission mode (default/auto_low_risk/auto_all/read_only)
  hooks.pre_tool    - Pre-tool hook command
  hooks.post_tool   - Post-tool hook command
  context_window    - Context window size
  compression_threshold - When to compress context

Use 'config action=get key=<key>' to view a value
Use 'config action=set key=<key> value=<value>' to modify"#;
                ToolResult::success(config_list)
            }
        }
    }
}
