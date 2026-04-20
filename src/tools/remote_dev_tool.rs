//! 远程开发工具
//!
//! 检测远程环境、管理远程会话、执行远程命令。

use crate::remote::{RemoteEnvDetector, RemoteSessionConfig, RemoteSessionManager, RemoteAuth};
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;

/// 远程开发工具
pub struct RemoteDevTool;

#[async_trait]
impl Tool for RemoteDevTool {
    fn name(&self) -> &str {
        "remote_dev"
    }

    fn description(&self) -> &str {
        "Detect remote development environments and manage remote SSH sessions. \
Actions: 'detect' (check current environment), 'list' (list saved sessions), \
'create' (save a new SSH session config), 'remove' (delete a session), \
'ssh' (build SSH command), 'exec' (execute command on remote host). \
Use this when working in containers, WSL, Codespaces, or over SSH."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["detect", "list", "create", "remove", "ssh", "exec"],
                    "description": "The remote dev action to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Session ID for 'remove', 'ssh', or 'exec' actions"
                },
                "name": {
                    "type": "string",
                    "description": "Friendly name for 'create' action"
                },
                "host": {
                    "type": "string",
                    "description": "SSH host for 'create' action"
                },
                "port": {
                    "type": "integer",
                    "description": "SSH port for 'create' action (default: 22)",
                    "default": 22
                },
                "username": {
                    "type": "string",
                    "description": "SSH username for 'create' action (default: current user)"
                },
                "key_file": {
                    "type": "string",
                    "description": "Path to SSH private key for 'create' action (default: use SSH agent)"
                },
                "command": {
                    "type": "string",
                    "description": "Command to execute for 'exec' action"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        if action.is_empty() {
            return ToolResult::error("Missing required parameter: action");
        }

        match action {
            "detect" => {
                let info = RemoteEnvDetector::detect();
                let env_type = info.env_type.to_string();
                let summary = format!(
                    "Environment: {}\nRemote: {}\nHostname: {}\nUser: {}\nCWD: {}\nDetected signals: {}",
                    env_type,
                    info.is_remote,
                    info.hostname,
                    info.username,
                    info.working_dir.display(),
                    info.detected_env_vars.join(", ")
                );
                let data = serde_json::to_value(info).unwrap_or_default();
                ToolResult::success_with_data(summary, data)
            }
            "list" => {
                let manager = RemoteSessionManager::new();
                let sessions = manager.list_sessions();
                if sessions.is_empty() {
                    return ToolResult::success("No saved remote sessions. Use 'create' to add one.");
                }
                let lines: Vec<String> = sessions
                    .iter()
                    .map(|s| {
                        format!(
                            "[{}] {} ({}@{}:{}) - {:?}",
                            s.id, s.config.name, s.config.username, s.config.host, s.config.port, s.status
                        )
                    })
                    .collect();
                ToolResult::success(lines.join("\n"))
            }
            "create" => {
                let name = params["name"].as_str().unwrap_or("unnamed");
                let host = match params["host"].as_str() {
                    Some(h) => h,
                    None => return ToolResult::error("Missing required parameter: host"),
                };
                let port = params["port"].as_u64().unwrap_or(22) as u16;
                let username = params["username"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        std::env::var("USER")
                            .or_else(|_| std::env::var("USERNAME"))
                            .unwrap_or_else(|_| "user".to_string())
                    });
                let auth = if let Some(key) = params["key_file"].as_str() {
                    RemoteAuth::KeyFile {
                        path: PathBuf::from(key),
                    }
                } else {
                    RemoteAuth::Agent
                };

                let config = RemoteSessionConfig {
                    name: name.to_string(),
                    host: host.to_string(),
                    port,
                    username,
                    auth,
                    remote_working_dir: None,
                    ssh_options: Vec::new(),
                };

                let manager = RemoteSessionManager::new();
                let session = manager.create_session(config);
                ToolResult::success(format!(
                    "Created remote session [{}] {} ({}@{}:{})",
                    session.id, session.config.name, session.config.username, session.config.host, session.config.port
                ))
            }
            "remove" => {
                let id = match params["id"].as_str() {
                    Some(i) => i,
                    None => return ToolResult::error("Missing required parameter: id"),
                };
                let manager = RemoteSessionManager::new();
                if manager.remove_session(id) {
                    ToolResult::success(format!("Removed session {}", id))
                } else {
                    ToolResult::error(format!("Session {} not found", id))
                }
            }
            "ssh" => {
                let id = match params["id"].as_str() {
                    Some(i) => i,
                    None => return ToolResult::error("Missing required parameter: id"),
                };
                let manager = RemoteSessionManager::new();
                match manager.build_ssh_command(id) {
                    Some(cmd) => {
                        let args: Vec<String> = cmd
                            .get_args()
                            .map(|s| s.to_string_lossy().to_string())
                            .collect();
                        let cmd_line = format!("ssh {}", args.join(" "));
                        ToolResult::success(format!("SSH command built. Run:\n\n{}", cmd_line))
                    }
                    None => ToolResult::error(format!("Session {} not found", id)),
                }
            }
            "exec" => {
                let id = match params["id"].as_str() {
                    Some(i) => i,
                    None => return ToolResult::error("Missing required parameter: id"),
                };
                let command = match params["command"].as_str() {
                    Some(c) => c,
                    None => return ToolResult::error("Missing required parameter: command"),
                };
                let manager = RemoteSessionManager::new();
                match manager.execute_remote(id, command).await {
                    Ok((stdout, stderr, code)) => {
                        let mut output = format!("Exit code: {}\n\n", code);
                        if !stdout.is_empty() {
                            output.push_str("--- stdout ---\n");
                            output.push_str(&stdout);
                            output.push('\n');
                        }
                        if !stderr.is_empty() {
                            output.push_str("--- stderr ---\n");
                            output.push_str(&stderr);
                            output.push('\n');
                        }
                        if code == 0 {
                            ToolResult::success(output)
                        } else {
                            ToolResult::error(output)
                        }
                    }
                    Err(e) => ToolResult::error(format!("Remote execution failed: {}", e)),
                }
            }
            _ => ToolResult::error(format!("Unknown remote_dev action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_dev_tool_params() {
        let tool = RemoteDevTool;
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
        let actions = params["properties"]["action"]["enum"].as_array().unwrap();
        assert!(!actions.is_empty());
    }

    #[test]
    fn test_remote_dev_tool_name() {
        let tool = RemoteDevTool;
        assert_eq!(tool.name(), "remote_dev");
    }
}
