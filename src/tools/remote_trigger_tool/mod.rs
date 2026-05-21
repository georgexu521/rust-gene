//! Remote Trigger 工具 - 远程会话管理
//!
//! 支持列出、获取、创建和运行远程触发器

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

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

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        remote_trigger_requires_confirmation(params)
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        remote_trigger_requires_confirmation(params)
            .then(|| remote_trigger_permission_prompt(params))
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let facts = remote_trigger_permission_metadata(params);
        format!(
            "remote_trigger(action={}, target={}, risk={})",
            facts["action"].as_str().unwrap_or("unknown"),
            facts["target"].as_str().unwrap_or("none"),
            facts["risk_level"].as_str().unwrap_or("unknown")
        )
    }

    fn operation_kind(&self, params: &serde_json::Value) -> ToolOperationKind {
        match remote_trigger_action(params) {
            "list" => ToolOperationKind::List,
            "get" | "status" | "replay" => ToolOperationKind::Read,
            "sync" if !remote_trigger_persist_cursor(params) => ToolOperationKind::Read,
            "sync" => ToolOperationKind::Write,
            "create" | "run" => ToolOperationKind::Task,
            _ => ToolOperationKind::Network,
        }
    }

    fn is_read_only(&self, params: &serde_json::Value) -> bool {
        matches!(
            remote_trigger_action(params),
            "list" | "get" | "status" | "replay"
        ) || (remote_trigger_action(params) == "sync" && !remote_trigger_persist_cursor(params))
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let action = remote_trigger_action(params);
        if action.is_empty() {
            return None;
        }
        let target = remote_trigger_target(params).unwrap_or_else(|| "no target".to_string());
        Some(format!("{} {}", action, target))
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");

        let bridge_url = crate::bridge::resolve_bridge_url().ok_or_else(|| {
                ToolResult::error(
                    "Bridge URL not set. Pass --bridge-url or set PRIORITY_AGENT_BRIDGE_URL/BRIDGE_URL.",
                )
            });
        let bridge_url = match bridge_url {
            Ok(url) => url,
            Err(e) => return e,
        };

        let auth_token = crate::bridge::resolve_bridge_auth_token();
        let tenant_id = crate::bridge::resolve_bridge_tenant_id();
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

pub(crate) fn remote_trigger_permission_metadata(params: &Value) -> Value {
    let snapshot = crate::bridge::runtime_snapshot();
    let action = remote_trigger_action(params).to_string();
    let target = remote_trigger_target(params);
    let persist_cursor = remote_trigger_persist_cursor(params);
    let use_saved_cursor = params["use_saved_cursor"].as_bool().unwrap_or(true);
    let risk_level = remote_trigger_risk_level(&action, persist_cursor);
    let remote_effect = remote_trigger_effect(&action, persist_cursor);
    let permission_summary = remote_trigger_permission_summary(
        &action,
        target.as_deref(),
        risk_level,
        remote_effect,
        snapshot.bridge_url_source.as_deref(),
        snapshot.auth_token_configured,
        snapshot.tenant_id.is_some(),
    );

    json!({
        "surface": "bridge",
        "tool_name": "remote_trigger",
        "action": action,
        "target": target,
        "risk_level": risk_level,
        "requires_confirmation": remote_trigger_requires_confirmation(params),
        "remote_effect": remote_effect,
        "permission_summary": permission_summary,
        "bridge_url_configured": snapshot.bridge_url.is_some(),
        "bridge_url_source": snapshot.bridge_url_source,
        "auth_token_configured": snapshot.auth_token_configured,
        "tenant_configured": snapshot.tenant_id.is_some(),
        "cursor": {
            "use_saved_cursor": use_saved_cursor,
            "persist_cursor": persist_cursor,
            "cursor_path": snapshot.cursor_path.display().to_string(),
            "known_cursor_sessions": snapshot.cursor_count,
        }
    })
}

fn remote_trigger_action(params: &Value) -> &str {
    params["action"].as_str().unwrap_or("")
}

fn remote_trigger_target(params: &Value) -> Option<String> {
    params["id"]
        .as_str()
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
}

fn remote_trigger_persist_cursor(params: &Value) -> bool {
    remote_trigger_action(params) == "sync" && params["persist_cursor"].as_bool().unwrap_or(true)
}

fn remote_trigger_requires_confirmation(params: &Value) -> bool {
    matches!(remote_trigger_action(params), "create" | "run")
        || remote_trigger_persist_cursor(params)
}

fn remote_trigger_risk_level(action: &str, persist_cursor: bool) -> &'static str {
    match action {
        "run" => "high",
        "create" => "medium",
        "sync" if persist_cursor => "medium",
        "sync" => "low",
        "list" | "get" | "status" | "replay" => "low",
        _ => "unknown",
    }
}

fn remote_trigger_effect(action: &str, persist_cursor: bool) -> &'static str {
    match action {
        "run" => "remote_execution",
        "create" => "remote_session_create",
        "sync" if persist_cursor => "remote_read_and_local_cursor_write",
        "sync" => "remote_read",
        "replay" => "remote_message_read",
        "status" => "remote_status_read",
        "get" => "remote_session_read",
        "list" => "remote_session_list",
        _ => "unknown",
    }
}

fn remote_trigger_permission_summary(
    action: &str,
    target: Option<&str>,
    risk_level: &str,
    remote_effect: &str,
    bridge_url_source: Option<&str>,
    auth_token_configured: bool,
    tenant_configured: bool,
) -> String {
    format!(
        "remote trigger action={} target={} risk={} effect={} bridge_source={} auth_token={} tenant={}",
        action,
        target.unwrap_or("none"),
        risk_level,
        remote_effect,
        bridge_url_source.unwrap_or("none"),
        auth_token_configured,
        tenant_configured
    )
}

fn remote_trigger_permission_prompt(params: &Value) -> String {
    let facts = remote_trigger_permission_metadata(params);
    let action = facts["action"].as_str().unwrap_or("unknown");
    let target = facts["target"].as_str().unwrap_or("none");
    let effect = facts["remote_effect"].as_str().unwrap_or("unknown");
    let risk = facts["risk_level"].as_str().unwrap_or("unknown");
    let bridge_source = facts["bridge_url_source"].as_str().unwrap_or("none");
    match action {
        "run" => format!(
            "Run remote trigger '{}' through the bridge?\nRisk: high remote execution; it may mutate remote/project state and is not automatically safe to retry.\nBridge: source={}, auth_token_configured={}, tenant_configured={}\nEffect: {}\nAllow?",
            target,
            bridge_source,
            facts["auth_token_configured"].as_bool().unwrap_or(false),
            facts["tenant_configured"].as_bool().unwrap_or(false),
            effect
        ),
        "create" => {
            let prompt_chars = params["prompt"].as_str().map(str::chars).map(Iterator::count).unwrap_or(0);
            format!(
                "Create a remote bridge session with a {} character prompt?\nRisk: medium remote session creation; prompt content will be sent to the configured bridge.\nBridge: source={}, auth_token_configured={}, tenant_configured={}\nAllow?",
                prompt_chars,
                bridge_source,
                facts["auth_token_configured"].as_bool().unwrap_or(false),
                facts["tenant_configured"].as_bool().unwrap_or(false)
            )
        }
        "sync" => format!(
            "Synchronize remote session '{}' and persist the replay cursor locally?\nRisk: {} {}; this reads remote state and updates the local bridge cursor.\nBridge: source={}\nAllow?",
            target, risk, effect, bridge_source
        ),
        _ => format!(
            "Run remote trigger action '{}' for target '{}'?\nRisk: {} {}\nAllow?",
            action, target, risk, effect
        ),
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

    #[test]
    fn remote_trigger_run_requires_confirmation_with_high_risk_facts() {
        let tool = RemoteTriggerTool;
        let params = json!({"action": "run", "id": "session-1"});

        assert!(tool.requires_confirmation(&params));
        assert_eq!(tool.operation_kind(&params), ToolOperationKind::Task);
        assert!(!tool.is_concurrency_safe(&params));

        let facts = remote_trigger_permission_metadata(&params);
        assert_eq!(facts["risk_level"], "high");
        assert_eq!(facts["remote_effect"], "remote_execution");
        assert!(facts["permission_summary"]
            .as_str()
            .unwrap()
            .contains("remote trigger action=run"));
    }

    #[test]
    fn remote_trigger_sync_cursor_is_not_read_only() {
        let tool = RemoteTriggerTool;
        let persist = json!({"action": "sync", "id": "session-1"});
        let no_persist = json!({
            "action": "sync",
            "id": "session-1",
            "persist_cursor": false
        });

        assert!(tool.requires_confirmation(&persist));
        assert!(!tool.is_read_only(&persist));
        assert_eq!(tool.operation_kind(&persist), ToolOperationKind::Write);
        assert!(!tool.requires_confirmation(&no_persist));
        assert!(tool.is_read_only(&no_persist));
        assert_eq!(tool.operation_kind(&no_persist), ToolOperationKind::Read);
    }
}
