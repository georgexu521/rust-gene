//! SendMessage 工具 - 向其他 Agent 发送消息

use crate::agent::types::{AgentId, AgentMessage, AgentMessageType};
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// 发送消息工具
pub struct SendMessageTool;

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str {
        "send_message"
    }

    fn description(&self) -> &str {
        "Send a message to another agent by ID. Use this for inter-agent communication."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Target agent ID"
                },
                "message": {
                    "type": "string",
                    "description": "Message content"
                },
                "msg_type": {
                    "type": "string",
                    "enum": ["task", "result", "status", "query", "control"],
                    "default": "message",
                    "description": "Message type"
                }
            },
            "required": ["to", "message"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let to = params["to"].as_str().unwrap_or("");
        let message = params["message"].as_str().unwrap_or("");
        let msg_type = params["msg_type"].as_str().unwrap_or("task");

        if to.is_empty() || message.is_empty() {
            return ToolResult::error("to and message are required");
        }

        let manager = match context.agent_manager {
            Some(m) => m,
            None => return ToolResult::error("Agent manager not available"),
        };

        let target_id = AgentId(to.to_string());
        let msg_type = match msg_type {
            "result" => AgentMessageType::Result,
            "status" => AgentMessageType::Status,
            "query" => AgentMessageType::Query,
            "control" => AgentMessageType::Control,
            _ => AgentMessageType::Task,
        };

        let agent_msg = AgentMessage::new(AgentId::new(), target_id.clone(), message, msg_type);

        match manager.send_message(&target_id, agent_msg).await {
            Ok(()) => ToolResult::success(format!("Message sent to agent {}", to)),
            Err(e) => ToolResult::error(format!("Failed to send message: {}", e)),
        }
    }

    fn is_available(&self, context: &ToolContext) -> bool {
        context.agent_manager.is_some()
    }

    fn unavailable_reason(&self, _context: &ToolContext) -> Option<String> {
        Some("Agent manager not configured".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_message_no_manager() {
        let tool = SendMessageTool;
        let result = tool
            .execute(
                json!({"to": "agent_123", "message": "hello"}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(!result.success);
        assert!(result
            .error
            .unwrap()
            .contains("Agent manager not available"));
    }
}
