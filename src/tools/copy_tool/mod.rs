//! Copy tool
//!
//! Copies text to clipboard.

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct CopyTool;

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn write_stdin_and_wait(mut child: std::process::Child, text: &str) -> bool {
    use std::io::Write;

    let write_ok = match child.stdin.take() {
        Some(mut stdin) => stdin.write_all(text.as_bytes()).is_ok(),
        None => true,
    };

    let status_ok = child.wait().map(|status| status.success()).unwrap_or(false);
    write_ok && status_ok
}

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

            if let Ok(child) = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                if write_stdin_and_wait(child, text) {
                    return ToolResult::success("Text copied to clipboard");
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            use std::process::Command;

            fn try_copy_with(command: &str, args: &[&str], text: &str) -> bool {
                Command::new(command)
                    .args(args)
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .map(|child| write_stdin_and_wait(child, text))
                    .unwrap_or(false)
            }

            // Try xclip first, then xsel
            if try_copy_with("xclip", &["-selection", "clipboard"], text)
                || try_copy_with("xsel", &["--clipboard", "--input"], text)
            {
                return ToolResult::success("Text copied to clipboard");
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
