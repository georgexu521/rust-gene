//! Desktop tool
//!
//! Desktop integration - opening URLs, files, and applications.

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct DesktopTool;

#[async_trait]
impl Tool for DesktopTool {
    fn name(&self) -> &str {
        "desktop"
    }

    fn description(&self) -> &str {
        "Open URLs, files, or applications in the desktop environment. Use 'action' parameter: 'open' (open URL/file), 'reveal' (show in finder)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["open", "reveal"],
                    "description": "Action: open or reveal"
                },
                "target": {
                    "type": "string",
                    "description": "URL or file path to open/reveal"
                }
            },
            "required": ["target"]
        })
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("open");
        let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("");

        if target.is_empty() {
            return ToolResult {
                success: false,
                content: String::new(),
                error: Some("Missing 'target' parameter".to_string()),
                data: None,
                duration_ms: None,
                ..Default::default()
            };
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            match action {
                "open" => {
                    let result = Command::new("open").arg(target).spawn();
                    if result.is_ok() {
                        return ToolResult::success(format!("Opened: {}", target));
                    }
                }
                "reveal" => {
                    let result = Command::new("open")
                        .args(["-R", target]) // -R = reveal in Finder
                        .spawn();
                    if result.is_ok() {
                        return ToolResult::success(format!("Revealed in Finder: {}", target));
                    }
                }
                _ => {}
            }
        }

        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            match action {
                "open" => {
                    // Try xdg-open first
                    let result = Command::new("xdg-open").arg(target).spawn();
                    if result.is_ok() {
                        return ToolResult::success(format!("Opened: {}", target));
                    }
                }
                "reveal" => {
                    // Try nautilus or other file managers
                    let result = Command::new("nautilus").arg(target).spawn();
                    if result.is_ok() {
                        return ToolResult::success(format!("Revealed: {}", target));
                    }
                    // Fallback to xdg-open
                    let result = Command::new("xdg-open")
                        .arg(
                            std::path::Path::new(target)
                                .parent()
                                .unwrap_or(std::path::Path::new(target)),
                        )
                        .spawn();
                    if result.is_ok() {
                        return ToolResult::success(format!("Revealed: {}", target));
                    }
                }
                _ => {}
            }
        }

        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            match action {
                "open" => {
                    let result = Command::new("cmd")
                        .args(["/c", "start", "", target])
                        .spawn();
                    if result.is_ok() {
                        return ToolResult::success(format!("Opened: {}", target));
                    }
                }
                "reveal" => {
                    let result = Command::new("explorer").args(["/select,", target]).spawn();
                    if result.is_ok() {
                        return ToolResult::success(format!("Revealed: {}", target));
                    }
                }
                _ => {}
            }
        }

        ToolResult {
            success: false,
            content: String::new(),
            error: Some(format!(
                "Failed to {} '{}' - platform not supported",
                action, target
            )),
            data: None,
            duration_ms: None,
            ..Default::default()
        }
    }
}
