//! 团队协作工具
//!
//! 让 agent 可以通过 TeammateMailbox 与其他 agent 发送/接收消息。

use crate::team::{MessageKind, MessagePriority, TeammateMailbox};
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// 团队协作工具
pub struct TeamTool;

#[async_trait]
impl Tool for TeamTool {
    fn name(&self) -> &str {
        "team"
    }

    fn description(&self) -> &str {
        "Collaborate with other agents via the team mailbox. \
Actions: 'send' (message an agent), 'receive' (get unread messages), \
'poll' (check unread count), 'broadcast' (notify all teammates), \
'mark_read' (mark messages as read), 'list' (show recent messages). \
Use this to coordinate work, hand off tasks, or request help from specialist agents."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["send", "receive", "poll", "broadcast", "mark_read", "list"],
                    "description": "The team collaboration action to perform"
                },
                "to": {
                    "type": "string",
                    "description": "Recipient agent ID for 'send' action"
                },
                "content": {
                    "type": "string",
                    "description": "Message content for 'send' or 'broadcast' action"
                },
                "priority": {
                    "type": "string",
                    "enum": ["high", "normal", "low"],
                    "description": "Message priority (default: normal)",
                    "default": "normal"
                },
                "kind": {
                    "type": "string",
                    "enum": ["request", "response", "notify", "broadcast"],
                    "description": "Message kind (default: notify)",
                    "default": "notify"
                },
                "from": {
                    "type": "string",
                    "description": "Filter by sender for 'receive' action"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max messages to return (default: 10)",
                    "default": 10
                },
                "message_id": {
                    "type": "string",
                    "description": "Message ID for 'mark_read' action"
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

        let mailbox = TeammateMailbox::new("agent");

        match action {
            "send" => {
                let to = match params["to"].as_str() {
                    Some(t) => t,
                    None => return ToolResult::error("Missing required parameter: to"),
                };
                let content = match params["content"].as_str() {
                    Some(c) => c,
                    None => return ToolResult::error("Missing required parameter: content"),
                };
                let priority = parse_priority(params["priority"].as_str());
                let kind = parse_kind(params["kind"].as_str());
                let msg = mailbox.send(to, content, priority, kind, None);
                ToolResult::success(format!(
                    "Sent message [{}] to {} with priority {:?}",
                    msg.id, msg.to, msg.priority
                ))
            }
            "receive" => {
                let limit = params["limit"].as_u64().unwrap_or(10) as usize;
                let msgs = if let Some(from) = params["from"].as_str() {
                    mailbox.receive_from(from, limit)
                } else {
                    mailbox.receive(limit)
                };
                if msgs.is_empty() {
                    return ToolResult::success("No unread messages.");
                }
                let lines: Vec<String> = msgs
                    .iter()
                    .map(|m| {
                        format!(
                            "[{}] {} -> {} ({:?}, {:?}): {}",
                            m.id, m.from, m.to, m.priority, m.kind, m.content
                        )
                    })
                    .collect();
                ToolResult::success(lines.join("\n"))
            }
            "poll" => {
                let summary = mailbox.unread_summary();
                let data = serde_json::to_value(&summary).unwrap_or_default();
                ToolResult::success_with_data(
                    format!(
                        "Unread messages: {} total. By sender: {:?}. By priority: {:?}",
                        summary.total, summary.by_sender, summary.by_priority
                    ),
                    data,
                )
            }
            "broadcast" => {
                let content = match params["content"].as_str() {
                    Some(c) => c,
                    None => return ToolResult::error("Missing required parameter: content"),
                };
                let priority = parse_priority(params["priority"].as_str());
                let msg = mailbox.broadcast(content, priority);
                ToolResult::success(format!(
                    "Broadcast message [{}] with priority {:?}",
                    msg.id, msg.priority
                ))
            }
            "mark_read" => {
                if let Some(id) = params["message_id"].as_str() {
                    if mailbox.mark_read(id) {
                        ToolResult::success(format!("Marked {} as read", id))
                    } else {
                        ToolResult::error(format!("Message {} not found", id))
                    }
                } else {
                    let count = mailbox.mark_all_read();
                    ToolResult::success(format!("Marked {} messages as read", count))
                }
            }
            "list" => {
                let limit = params["limit"].as_u64().unwrap_or(10) as usize;
                let msgs = mailbox.list_messages(limit);
                if msgs.is_empty() {
                    return ToolResult::success("No messages in mailbox.");
                }
                let lines: Vec<String> = msgs
                    .iter()
                    .map(|m| {
                        let status = if m.read { "read" } else { "unread" };
                        format!(
                            "[{}] {} -> {} [{}] ({:?}): {}",
                            m.id, m.from, m.to, status, m.priority, m.content
                        )
                    })
                    .collect();
                ToolResult::success(lines.join("\n"))
            }
            _ => ToolResult::error(format!("Unknown team action: {}", action)),
        }
    }
}

fn parse_priority(s: Option<&str>) -> MessagePriority {
    match s {
        Some("high") => MessagePriority::High,
        Some("low") => MessagePriority::Low,
        _ => MessagePriority::Normal,
    }
}

fn parse_kind(s: Option<&str>) -> MessageKind {
    match s {
        Some("request") => MessageKind::Request,
        Some("response") => MessageKind::Response,
        Some("broadcast") => MessageKind::Broadcast,
        _ => MessageKind::Notify,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_team_tool_params() {
        let tool = TeamTool;
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
    }

    #[test]
    fn test_team_tool_name() {
        let tool = TeamTool;
        assert_eq!(tool.name(), "team");
    }

    #[test]
    fn test_parse_priority() {
        assert_eq!(parse_priority(Some("high")), MessagePriority::High);
        assert_eq!(parse_priority(Some("low")), MessagePriority::Low);
        assert_eq!(parse_priority(Some("normal")), MessagePriority::Normal);
        assert_eq!(parse_priority(None), MessagePriority::Normal);
    }

    #[test]
    fn test_parse_kind() {
        assert_eq!(parse_kind(Some("request")), MessageKind::Request);
        assert_eq!(parse_kind(Some("response")), MessageKind::Response);
        assert_eq!(parse_kind(Some("broadcast")), MessageKind::Broadcast);
        assert_eq!(parse_kind(None), MessageKind::Notify);
    }
}
