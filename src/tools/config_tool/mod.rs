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
        "View and modify agent configuration. Actions: list, get, set, schema, export, doctor"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get", "set", "list", "schema", "export", "doctor"],
                    "description": "Action to perform"
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
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");
        let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let value = params.get("value").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "get" => {
                if key.is_empty() {
                    return ToolResult::success("Usage: config action=get key=<key>");
                }
                match crate::services::config::AppConfig::load() {
                    Ok(config) => crate::services::config::get_config_value(&config, key)
                        .map(|v| ToolResult::success(format!("{} = {}", key, v)))
                        .unwrap_or_else(|| {
                            ToolResult::error(format!("Unknown config key: {}", key))
                        }),
                    Err(e) => ToolResult::error(format!("Failed to load config: {}", e)),
                }
            }
            "set" => {
                if key.is_empty() {
                    return ToolResult::success("Usage: config action=set key=<key> value=<value>");
                }
                if value.is_empty() {
                    return ToolResult::success("Usage: config action=set key=<key> value=<value>");
                }
                match crate::services::config::AppConfig::load() {
                    Ok(mut config) => {
                        match crate::services::config::set_config_value(&mut config, key, value) {
                            Ok(()) => match config.save() {
                                Ok(()) => ToolResult::success(format!(
                                    "Updated {} = {} and saved to config.toml",
                                    key, value
                                )),
                                Err(e) => {
                                    ToolResult::error(format!("Failed to save config: {}", e))
                                }
                            },
                            Err(e) => ToolResult::error(e),
                        }
                    }
                    Err(e) => ToolResult::error(format!("Failed to load config: {}", e)),
                }
            }
            "schema" => ToolResult::success(crate::services::config::format_config_schema_text()),
            "export" => match crate::services::config::AppConfig::load() {
                Ok(config) => {
                    let cwd =
                        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                    let export = crate::services::config::redacted_config_export(&config, &cwd);
                    match serde_json::to_string_pretty(&export) {
                        Ok(text) => ToolResult::success(text),
                        Err(e) => {
                            ToolResult::error(format!("Failed to serialize config export: {}", e))
                        }
                    }
                }
                Err(e) => ToolResult::error(format!("Failed to load config: {}", e)),
            },
            "doctor" => match crate::services::config::AppConfig::load() {
                Ok(config) => {
                    let issues = crate::services::config::validate_config(&config);
                    if issues.is_empty() {
                        ToolResult::success("Config doctor: ok")
                    } else {
                        ToolResult::success(format!(
                            "Config doctor: warning\n- {}",
                            issues.join("\n- ")
                        ))
                    }
                }
                Err(e) => ToolResult::error(format!("Failed to load config: {}", e)),
            },
            _ => match crate::services::config::AppConfig::load() {
                Ok(config) => {
                    ToolResult::success(crate::services::config::format_config_summary(&config))
                }
                Err(e) => ToolResult::error(format!("Failed to load config: {}", e)),
            },
        }
    }
}
