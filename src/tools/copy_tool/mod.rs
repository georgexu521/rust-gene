//! Copy tool
//!
//! Copies text to clipboard.

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct CopyTool;

#[async_trait]
impl Tool for CopyTool {
    fn name(&self) -> &str {
        "copy"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Write
    }

    fn description(&self) -> &str {
        "Copy text to clipboard. Takes 'text' parameter with the content to copy."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to copy to clipboard"
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");

        // Use platform-specific clipboard copy
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let result = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn();

            if let Ok(mut child) = result {
                use std::io::Write;
                let write_ok = if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(text.as_bytes()).is_ok()
                } else {
                    true
                };
                if write_ok && child.wait().is_ok() {
                    return ToolResult::success("Text copied to clipboard");
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            // Try xclip first, then xsel
            let result = Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .spawn();

            if let Ok(mut child) = result {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    if stdin.write_all(text.as_bytes()).is_err() {
                        continue;
                    }
                }
                if child.wait().is_ok() {
                    return ToolResult::success("Text copied to clipboard");
                }
            }

            // Try xsel as fallback
            let result = Command::new("xsel")
                .args(["--clipboard", "--input"])
                .stdin(std::process::Stdio::piped())
                .spawn();

            if let Ok(mut child) = result {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    if stdin.write_all(text.as_bytes()).is_err() {
                        continue;
                    }
                }
                if child.wait().is_ok() {
                    return ToolResult::success("Text copied to clipboard");
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            let result = Command::new("cmd")
                .args(["/c", "echo", text, "|", "clip"])
                .spawn();

            if let Ok(mut child) = result {
                if child.wait().is_ok() {
                    return ToolResult::success("Text copied to clipboard");
                }
            }
        }

        ToolResult {
            success: false,
            content: String::new(),
            error: Some("No clipboard utility available (tried pbcopy/xclip/xsel)".to_string()),
            data: None,
            duration_ms: None,
            ..Default::default()
        }
    }
}
