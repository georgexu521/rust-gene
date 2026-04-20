//! Remote Trigger 工具 - 远程会话管理
//!
//! 支持列出、获取、创建和运行远程触发器

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Remote Trigger 工具
pub struct RemoteTriggerTool;

#[async_trait]
impl Tool for RemoteTriggerTool {
    fn name(&self) -> &str {
        "remote_trigger"
    }

    fn description(&self) -> &str {
        "Manage remote sessions via a bridge server. Actions: 'list', 'get', 'create', 'run', 'status', 'replay', 'sync'. Sync supports persistent replay cursor."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "create", "run", "status", "replay", "sync"],
                    "description": "The remote trigger action to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Session or trigger ID for 'get'/'run'/'status'/'replay'/'sync' actions"
                },
                "prompt": {
                    "type": "string",
                    "description": "Prompt for 'create' action"
                },
                "body": {
                    "type": "object",
                    "description": "Optional JSON body for 'run' action"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max messages for 'replay' action (default 100)"
                },
                "since_id": {
                    "type": "integer",
                    "description": "Only return messages with id > since_id for incremental replay"
                },
                "use_saved_cursor": {
                    "type": "boolean",
                    "description": "For sync: when since_id is absent, load local saved cursor (default true)"
                },
                "persist_cursor": {
                    "type": "boolean",
                    "description": "For sync: persist next_since_id after successful replay (default true)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");

        let bridge_url = crate::bridge::get_bridge_url()
            .cloned()
            .or_else(|| std::env::var("BRIDGE_URL").ok())
            .ok_or_else(|| {
                ToolResult::error(
                    "BRIDGE_URL not set. Pass --bridge-url or set the environment variable.",
                )
            });
        let bridge_url = match bridge_url {
            Ok(url) => url,
            Err(e) => return e,
        };

        let auth_token = std::env::var("BRIDGE_TOKEN").ok();
        let tenant_id = std::env::var("BRIDGE_TENANT_ID").ok();
        let client = crate::bridge::BridgeClient::with_tenant(bridge_url, auth_token, tenant_id);

        match action {
            "list" => match client.list_sessions().await {
                Ok(data) => {
                    ToolResult::success_with_data("Remote sessions listed successfully", data)
                }
                Err(e) => ToolResult::error(format!("Failed to list sessions: {}", e)),
            },
            "get" => {
                let id = params["id"].as_str().unwrap_or("");
                if id.is_empty() {
                    return ToolResult::error("Missing 'id' parameter for get action");
                }
                match client.get_session(id).await {
                    Ok(data) => {
                        ToolResult::success_with_data(format!("Session {} retrieved", id), data)
                    }
                    Err(e) => ToolResult::error(format!("Failed to get session: {}", e)),
                }
            }
            "create" => {
                let prompt = params["prompt"].as_str().unwrap_or("");
                if prompt.is_empty() {
                    return ToolResult::error("Missing 'prompt' parameter for create action");
                }
                match client.create_session(prompt).await {
                    Ok(data) => {
                        ToolResult::success_with_data("Remote session created successfully", data)
                    }
                    Err(e) => ToolResult::error(format!("Failed to create session: {}", e)),
                }
            }
            "run" => {
                let id = params["id"].as_str().unwrap_or("");
                if id.is_empty() {
                    return ToolResult::error("Missing 'id' parameter for run action");
                }
                let body = params.get("body").cloned();
                match client.run_trigger(id, body).await {
                    Ok(data) => {
                        ToolResult::success_with_data(format!("Trigger {} executed", id), data)
                    }
                    Err(e) => ToolResult::error(format!("Failed to run trigger: {}", e)),
                }
            }
            "status" => {
                let id = params["id"].as_str().unwrap_or("");
                if id.is_empty() {
                    return ToolResult::error("Missing 'id' parameter for status action");
                }
                match client.get_session_status(id).await {
                    Ok(data) => ToolResult::success_with_data(
                        format!("Session {} status retrieved", id),
                        data,
                    ),
                    Err(e) => ToolResult::error(format!("Failed to get session status: {}", e)),
                }
            }
            "replay" => {
                let id = params["id"].as_str().unwrap_or("");
                if id.is_empty() {
                    return ToolResult::error("Missing 'id' parameter for replay action");
                }
                let limit = params["limit"].as_u64().and_then(|v| u32::try_from(v).ok());
                let since_id = params["since_id"].as_i64();
                match client.get_session_messages(id, limit, since_id).await {
                    Ok(data) => ToolResult::success_with_data(
                        format!("Session {} messages replayed", id),
                        data,
                    ),
                    Err(e) => {
                        ToolResult::error(format!("Failed to replay session messages: {}", e))
                    }
                }
            }
            "sync" => {
                let id = params["id"].as_str().unwrap_or("");
                if id.is_empty() {
                    return ToolResult::error("Missing 'id' parameter for sync action");
                }
                let limit = params["limit"].as_u64().and_then(|v| u32::try_from(v).ok());
                let use_saved_cursor = params["use_saved_cursor"].as_bool().unwrap_or(true);
                let persist_cursor = params["persist_cursor"].as_bool().unwrap_or(true);
                let since_id = params["since_id"].as_i64().or_else(|| {
                    if use_saved_cursor {
                        crate::bridge::load_replay_cursor(id)
                    } else {
                        None
                    }
                });
                let status = client.get_session_status(id).await;
                let replay = client.get_session_messages(id, limit, since_id).await;
                match (status, replay) {
                    (Ok(status), Ok(replay)) => {
                        let next_since_id = replay.get("next_since_id").and_then(|v| v.as_i64());
                        let mut persisted = false;
                        if persist_cursor {
                            if let Some(next) = next_since_id {
                                if crate::bridge::save_replay_cursor(id, next).is_ok() {
                                    persisted = true;
                                }
                            }
                        }
                        ToolResult::success_with_data(
                            format!("Session {} synchronized", id),
                            json!({
                                "status": status,
                                "replay": replay,
                                "cursor": {
                                    "since_id_used": since_id,
                                    "next_since_id": next_since_id,
                                    "persisted": persisted
                                }
                            }),
                        )
                    }
                    (Err(e), _) => ToolResult::error(format!("Failed to sync status: {}", e)),
                    (_, Err(e)) => ToolResult::error(format!("Failed to sync replay: {}", e)),
                }
            }
            _ => ToolResult::error(format!("Unknown remote trigger action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_trigger_params() {
        let tool = RemoteTriggerTool;
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
    }
}
