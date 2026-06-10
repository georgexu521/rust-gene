//! Config tool
//!
//! View and modify agent configuration.

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolPermissionLevel, ToolResult};
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
            "required": [],
            "additionalProperties": false
        })
    }

    fn requires_confirmation(&self, params: &Value) -> bool {
        config_action(params) == "set"
    }

    fn confirmation_prompt(&self, params: &Value) -> Option<String> {
        if !self.requires_confirmation(params) {
            return None;
        }
        let key = params
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or("config key");
        Some(format!("Update persistent agent configuration for {key}?"))
    }

    fn operation_kind(&self, params: &Value) -> ToolOperationKind {
        match config_action(params) {
            "set" => ToolOperationKind::Write,
            "list" => ToolOperationKind::List,
            "get" | "schema" | "export" | "doctor" => ToolOperationKind::Read,
            _ => ToolOperationKind::Read,
        }
    }

    fn permission_level(&self) -> ToolPermissionLevel {
        ToolPermissionLevel::HighRisk
    }

    fn is_concurrency_safe(&self, params: &Value) -> bool {
        !self.requires_confirmation(params)
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn tool_use_summary(&self, params: &Value) -> Option<String> {
        let action = config_action(params);
        let key = params.get("key").and_then(Value::as_str).unwrap_or("");
        if key.is_empty() {
            Some(action.to_string())
        } else {
            Some(format!("{action} {key}"))
        }
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let action = config_action(&params);
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
                                Ok(()) => {
                                    crate::services::config::init_runtime_config(config);
                                    ToolResult::success(format!(
                                        "Updated {} = {} and refreshed runtime config",
                                        key, value
                                    ))
                                }
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

fn config_action(params: &Value) -> &str {
    params
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_tool_contract_is_parameter_sensitive() {
        let tool = ConfigTool;
        let read = json!({"action": "get", "key": "model"});
        let set = json!({"action": "set", "key": "model", "value": "test-model"});

        assert_eq!(tool.operation_kind(&read), ToolOperationKind::Read);
        assert!(!tool.requires_confirmation(&read));
        assert!(tool.is_concurrency_safe(&read));

        assert_eq!(tool.operation_kind(&set), ToolOperationKind::Write);
        assert!(tool.requires_confirmation(&set));
        assert!(tool.confirmation_prompt(&set).is_some());
        assert!(!tool.is_concurrency_safe(&set));
        assert_eq!(tool.permission_level(), ToolPermissionLevel::HighRisk);
        assert!(tool.strict_schema());
    }
}
