//! PowerShell 工具 - 跨平台 PowerShell 执行
//!
//! 支持在 Windows/Linux/macOS 上执行 PowerShell 命令和脚本。

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::process::{Command as StdCommand, Stdio};
use tokio::process::Command;

/// PowerShell 工具
pub struct PowerShellTool;

fn normalize_dash_variants(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2013}' | '\u{2014}' | '\u{2015}' => '-',
            _ => c,
        })
        .collect()
}

fn is_high_risk_powershell_command(cmd: &str) -> Option<&'static str> {
    let normalized = normalize_dash_variants(cmd).to_ascii_lowercase();
    let compact = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

    let risk_checks = [
        (
            compact.contains("invoke-expression") || compact.contains(" iex "),
            "uses Invoke-Expression (arbitrary code execution)",
        ),
        (
            compact.contains("-encodedcommand")
                || compact.contains(" -enc ")
                || compact.starts_with("-enc "),
            "uses encoded command payload",
        ),
        (
            (compact.contains("invoke-webrequest")
                || compact.contains(" iwr ")
                || compact.contains("invoke-restmethod")
                || compact.contains(" irm ")
                || compact.contains("curl "))
                && (compact.contains("| iex")
                    || compact.contains("| invoke-expression")
                    || compact.contains("| powershell")
                    || compact.contains("| pwsh")),
            "contains download-and-execute pattern",
        ),
        (
            compact.contains("start-process") && compact.contains("-verb runas"),
            "requests elevated process launch (RunAs)",
        ),
        (
            compact.contains("powershell -command")
                || compact.contains("pwsh -command")
                || compact.contains("powershell -file")
                || compact.contains("pwsh -file"),
            "spawns nested PowerShell process",
        ),
    ];

    risk_checks
        .into_iter()
        .find_map(|(matched, reason)| if matched { Some(reason) } else { None })
}

#[async_trait]
impl Tool for PowerShellTool {
    fn name(&self) -> &str {
        "powershell"
    }

    fn description(&self) -> &str {
        "Execute PowerShell commands or scripts cross-platform. \
         Works on Windows, Linux (pwsh), and macOS (pwsh). \
         Actions: 'execute' (run command/script), 'version' (check PowerShell version)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["execute", "version"],
                    "description": "The PowerShell action to perform"
                },
                "command": {
                    "type": "string",
                    "description": "PowerShell command to execute (for 'execute')"
                },
                "script_path": {
                    "type": "string",
                    "description": "Path to .ps1 script file (for 'execute', alternative to command)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 60)",
                    "default": 60
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for command execution"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");

        match action {
            "version" => self.check_version().await,
            "execute" => {
                let command = params["command"].as_str();
                let script_path = params["script_path"].as_str();
                let timeout = params["timeout"].as_u64().unwrap_or(60);
                let work_dir = params["working_dir"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| context.working_dir.to_string_lossy().to_string());

                if command.is_none() && script_path.is_none() {
                    return ToolResult::error(
                        "Either 'command' or 'script_path' is required for execute".to_string(),
                    );
                }

                if let Some(ps_cmd) = command {
                    if let Some(reason) = is_high_risk_powershell_command(ps_cmd) {
                        if !context.permissions.allow_all_bash {
                            return ToolResult::error(format!(
                                "High-risk PowerShell command blocked: {}. \
                                 Enable explicit approval/override before running it.",
                                reason
                            ));
                        }
                    }
                }

                self.execute_ps(command, script_path, &work_dir, timeout)
                    .await
            }
            _ => ToolResult::error(format!("Unknown PowerShell action: {}", action)),
        }
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        match params["action"].as_str().unwrap_or("") {
            "execute" => {
                if params["script_path"].as_str().is_some() {
                    return true;
                }
                if let Some(cmd) = params["command"].as_str() {
                    return is_high_risk_powershell_command(cmd).is_some();
                }
                false
            }
            _ => false,
        }
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        if params["action"].as_str().unwrap_or("") != "execute" {
            return None;
        }
        if let Some(script) = params["script_path"].as_str() {
            return Some(format!(
                "PowerShell script execution requires confirmation:\n{}\nAllow execution?",
                script
            ));
        }
        params["command"].as_str().map(|cmd| {
            if let Some(reason) = is_high_risk_powershell_command(cmd) {
                format!(
                    "High-risk PowerShell command detected ({})\n{}\nAllow execution?",
                    reason, cmd
                )
            } else {
                format!("Allow PowerShell command execution?\n{}", cmd)
            }
        })
    }
}

impl PowerShellTool {
    /// 检测 PowerShell 可执行文件路径
    fn get_powershell_path() -> String {
        // Windows: 先尝试 powershell.exe，再尝试 pwsh.exe
        // Linux/macOS: 只尝试 pwsh
        if cfg!(target_os = "windows") {
            // 检查 PowerShell 7+ (pwsh) 是否存在
            if StdCommand::new("pwsh")
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok()
            {
                return "pwsh".to_string();
            }
            // 回退到 Windows PowerShell
            "powershell.exe".to_string()
        } else {
            // Linux/macOS 使用 pwsh (PowerShell Core)
            "pwsh".to_string()
        }
    }

    /// 检查 PowerShell 版本
    async fn check_version(&self) -> ToolResult {
        let ps_path = Self::get_powershell_path();

        match Command::new(&ps_path)
            .args(["-Command", "$PSVersionTable.PSVersion.ToString()"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    ToolResult::success_with_data(
                        format!("PowerShell version: {}", version),
                        json!({
                            "version": version,
                            "path": ps_path
                        }),
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    ToolResult::error(format!("PowerShell version check failed: {}", stderr))
                }
            }
            Err(e) => ToolResult::error(format!(
                "PowerShell not found ({}): {}. \
                 On Linux/macOS, install with: brew install powershell/tap/powershell",
                ps_path, e
            )),
        }
    }

    /// 执行 PowerShell 命令或脚本
    async fn execute_ps(
        &self,
        command: Option<&str>,
        script_path: Option<&str>,
        work_dir: &str,
        timeout_secs: u64,
    ) -> ToolResult {
        let ps_path = Self::get_powershell_path();
        let work_path = std::path::Path::new(work_dir);

        let mut cmd = Command::new(&ps_path);
        cmd.current_dir(work_path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(script) = script_path {
            // 执行脚本文件
            let resolved = match crate::tools::file_tool::resolve_path(script, work_path) {
                Ok(p) => p,
                Err(e) => return ToolResult::error(e.to_string()),
            };

            if !resolved.exists() {
                return ToolResult::error(format!("Script file not found: {}", resolved.display()));
            }

            cmd.args(["-File", &resolved.to_string_lossy()]);
        } else if let Some(ps_command) = command {
            // 执行命令
            cmd.args(["-Command", ps_command]);
        }

        // 设置超时
        let timeout = std::time::Duration::from_secs(timeout_secs);

        match tokio::time::timeout(timeout, cmd.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                if output.status.success() {
                    let result = if stdout.is_empty() {
                        "Command executed successfully (no output)".to_string()
                    } else {
                        stdout.clone()
                    };
                    ToolResult::success_with_data(
                        result,
                        json!({
                            "stdout": stdout,
                            "stderr": stderr,
                            "exit_code": exit_code
                        }),
                    )
                } else {
                    let error_msg = if stderr.is_empty() {
                        format!("Command failed with exit code {}", exit_code)
                    } else {
                        format!("Command failed (exit {}): {}", exit_code, stderr)
                    };
                    ToolResult::error_with_content(
                        error_msg,
                        format!("stdout: {}\nstderr: {}", stdout, stderr),
                    )
                }
            }
            Ok(Err(e)) => ToolResult::error(format!("Failed to execute PowerShell: {}", e)),
            Err(_) => ToolResult::error(format!(
                "PowerShell command timed out after {} seconds",
                timeout_secs
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_powershell_version() {
        let tool = PowerShellTool;
        let result = tool
            .execute(
                json!({ "action": "version" }),
                ToolContext::new(".", "test"),
            )
            .await;
        // PowerShell 可能未安装，但应该返回结果
        assert!(
            result.success || result.error.is_some(),
            "Expected either success or error"
        );
    }

    #[test]
    fn test_powershell_tool_name() {
        let tool = PowerShellTool;
        assert_eq!(tool.name(), "powershell");
    }
}
