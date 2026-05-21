//! 远程开发工具
//!
//! 检测远程环境、管理远程会话、执行远程命令。

use crate::remote::{RemoteAuth, RemoteEnvDetector, RemoteSessionConfig, RemoteSessionManager};
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
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

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        remote_dev_requires_confirmation(params)
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        remote_dev_requires_confirmation(params).then(|| remote_dev_permission_prompt(params))
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let facts = remote_dev_permission_metadata(params);
        format!(
            "remote_dev(action={}, target={}, risk={}, command={})",
            facts["action"].as_str().unwrap_or("unknown"),
            facts["target"].as_str().unwrap_or("none"),
            facts["risk_level"].as_str().unwrap_or("unknown"),
            facts["command_preview"].as_str().unwrap_or("none")
        )
    }

    fn operation_kind(&self, params: &serde_json::Value) -> ToolOperationKind {
        match remote_dev_action(params) {
            "detect" => ToolOperationKind::Read,
            "list" => ToolOperationKind::List,
            "ssh" => ToolOperationKind::Read,
            "exec" => ToolOperationKind::Shell,
            "create" | "remove" => ToolOperationKind::Write,
            _ => ToolOperationKind::Network,
        }
    }

    fn is_read_only(&self, params: &serde_json::Value) -> bool {
        matches!(remote_dev_action(params), "detect" | "list" | "ssh")
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let action = remote_dev_action(params);
        if action.is_empty() {
            return None;
        }
        let target = remote_dev_target(params).unwrap_or_else(|| "no target".to_string());
        Some(format!("{} {}", action, target))
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
                    return ToolResult::success(
                        "No saved remote sessions. Use 'create' to add one.",
                    );
                }
                let lines: Vec<String> = sessions
                    .iter()
                    .map(|s| {
                        format!(
                            "[{}] {} ({}@{}:{}) - {:?}",
                            s.id,
                            s.config.name,
                            s.config.username,
                            s.config.host,
                            s.config.port,
                            s.status
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
                    session.id,
                    session.config.name,
                    session.config.username,
                    session.config.host,
                    session.config.port
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

pub(crate) fn remote_dev_permission_metadata(params: &Value) -> Value {
    let action = remote_dev_action(params).to_string();
    let target = remote_dev_target(params);
    let risk_level = remote_dev_risk_level(&action);
    let remote_effect = remote_dev_effect(&action);
    let command_preview = params["command"].as_str().map(short_preview);
    let host = params["host"]
        .as_str()
        .filter(|host| !host.trim().is_empty());
    let key_file_configured = params["key_file"]
        .as_str()
        .is_some_and(|key| !key.trim().is_empty());
    let permission_summary = remote_dev_permission_summary(
        &action,
        target.as_deref(),
        host,
        command_preview.as_deref(),
        risk_level,
        remote_effect,
    );

    json!({
        "surface": "remote_dev",
        "tool_name": "remote_dev",
        "action": action,
        "target": target,
        "host": host,
        "risk_level": risk_level,
        "requires_confirmation": remote_dev_requires_confirmation(params),
        "remote_effect": remote_effect,
        "command_preview": command_preview,
        "key_file_configured": key_file_configured,
        "permission_summary": permission_summary,
    })
}

fn remote_dev_action(params: &Value) -> &str {
    params["action"].as_str().unwrap_or("")
}

fn remote_dev_target(params: &Value) -> Option<String> {
    params["id"]
        .as_str()
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            params["host"]
                .as_str()
                .filter(|host| !host.trim().is_empty())
                .map(str::to_string)
        })
}

fn remote_dev_requires_confirmation(params: &Value) -> bool {
    matches!(remote_dev_action(params), "create" | "remove" | "exec")
}

fn remote_dev_risk_level(action: &str) -> &'static str {
    match action {
        "exec" => "high",
        "create" | "remove" => "medium",
        "detect" | "list" | "ssh" => "low",
        _ => "unknown",
    }
}

fn remote_dev_effect(action: &str) -> &'static str {
    match action {
        "exec" => "remote_ssh_execution",
        "create" => "local_remote_session_create",
        "remove" => "local_remote_session_delete",
        "ssh" => "ssh_command_preview",
        "list" => "local_remote_session_list",
        "detect" => "environment_detection",
        _ => "unknown",
    }
}

fn remote_dev_permission_summary(
    action: &str,
    target: Option<&str>,
    host: Option<&str>,
    command_preview: Option<&str>,
    risk_level: &str,
    remote_effect: &str,
) -> String {
    format!(
        "remote dev action={} target={} host={} risk={} effect={} command={}",
        action,
        target.unwrap_or("none"),
        host.unwrap_or("none"),
        risk_level,
        remote_effect,
        command_preview.unwrap_or("none")
    )
}

fn remote_dev_permission_prompt(params: &Value) -> String {
    let facts = remote_dev_permission_metadata(params);
    let action = facts["action"].as_str().unwrap_or("unknown");
    let target = facts["target"].as_str().unwrap_or("none");
    let effect = facts["remote_effect"].as_str().unwrap_or("unknown");
    let risk = facts["risk_level"].as_str().unwrap_or("unknown");
    match action {
        "exec" => format!(
            "Execute a command on remote session '{}'?\nCommand: {}\nRisk: high remote SSH execution; it can mutate files/processes outside the local workspace and may not be safely retryable.\nEffect: {}\nAllow?",
            target,
            facts["command_preview"].as_str().unwrap_or(""),
            effect
        ),
        "create" => format!(
            "Save remote SSH session '{}'?\nRisk: medium local session configuration; host/key metadata will be stored for future remote use.\nEffect: {}\nAllow?",
            target, effect
        ),
        "remove" => format!(
            "Remove saved remote SSH session '{}'?\nRisk: medium local session deletion; future remote commands for that session will fail until recreated.\nAllow?",
            target
        ),
        _ => format!(
            "Run remote dev action '{}' for target '{}'?\nRisk: {} {}\nAllow?",
            action, target, risk, effect
        ),
    }
}

fn short_preview(value: &str) -> String {
    const MAX_CHARS: usize = 120;
    let mut preview = value.chars().take(MAX_CHARS).collect::<String>();
    if value.chars().count() > MAX_CHARS {
        preview.push_str("...");
    }
    preview
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

    #[test]
    fn remote_dev_exec_requires_confirmation_with_command_facts() {
        let tool = RemoteDevTool;
        let params = json!({
            "action": "exec",
            "id": "prod-shell",
            "command": "cargo test -q"
        });

        assert!(tool.requires_confirmation(&params));
        assert_eq!(tool.operation_kind(&params), ToolOperationKind::Shell);
        assert!(!tool.is_concurrency_safe(&params));

        let facts = remote_dev_permission_metadata(&params);
        assert_eq!(facts["risk_level"], "high");
        assert_eq!(facts["remote_effect"], "remote_ssh_execution");
        assert_eq!(facts["command_preview"], "cargo test -q");
    }

    #[test]
    fn remote_dev_detect_is_read_only_but_serial_for_permission_trace() {
        let tool = RemoteDevTool;
        let params = json!({"action": "detect"});

        assert!(!tool.requires_confirmation(&params));
        assert!(tool.is_read_only(&params));
        assert!(!tool.is_concurrency_safe(&params));
        assert_eq!(tool.operation_kind(&params), ToolOperationKind::Read);
    }
}
