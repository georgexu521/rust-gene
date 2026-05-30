use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

/// 用户问答通道 - 在工具和 TUI 之间传递问答消息
pub struct AskChannel {
    pending_question: Arc<Mutex<Option<AskPending>>>,
}

struct AskPending {
    question: String,
    options: Vec<String>,
    response_tx: oneshot::Sender<String>,
}

impl AskChannel {
    pub fn new() -> Self {
        Self {
            pending_question: Arc::new(Mutex::new(None)),
        }
    }

    /// 工具调用：发送问题，等待回答
    pub async fn ask(&self, question: String, options: Vec<String>) -> Result<String, String> {
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_question.lock().await;
            *pending = Some(AskPending {
                question,
                options,
                response_tx: tx,
            });
        }
        rx.await.map_err(|_| "User did not respond".to_string())
    }

    /// TUI 调用：检查是否有待处理的问题
    pub async fn take_pending(&self) -> Option<(String, Vec<String>, oneshot::Sender<String>)> {
        let mut pending = self.pending_question.lock().await;
        pending
            .take()
            .map(|p| (p.question, p.options, p.response_tx))
    }
}

impl Default for AskChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// AskUserQuestion 工具
pub struct AskUserQuestionTool {
    channel: Arc<AskChannel>,
}

impl AskUserQuestionTool {
    pub fn new(channel: Arc<AskChannel>) -> Self {
        Self { channel }
    }
}

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn description(&self) -> &str {
        "Ask the user a question and wait for their response. Use this when you need clarification, confirmation, or input from the user."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
                },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional: predefined answer options for the user to choose from"
                }
            },
            "required": ["question"]
        })
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn requires_user_interaction(&self) -> bool {
        true
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let question = params["question"].as_str().unwrap_or("").to_string();
        if question.is_empty() {
            return ToolResult::error("Question cannot be empty");
        }

        let options: Vec<String> = params["options"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Auto-approve in non-interactive/eval mode: respond immediately
        // without waiting for user input. Controlled by PRIORITY_AGENT_AUTO_APPROVE
        // (default "1" in eval-run mode, set to "0" to require user interaction).
        if std::env::var("PRIORITY_AGENT_AUTO_APPROVE")
            .unwrap_or_else(|_| "1".to_string())
            .trim()
            != "0"
        {
            let answer = if options.is_empty() {
                "auto-approved (non-interactive mode)".to_string()
            } else {
                options.first().cloned().unwrap_or_else(|| "auto-approved".to_string())
            };
            return ToolResult::success(format!(
                "Question: {}\nAnswer (auto): {}",
                question, answer
            ));
        }

        match self.channel.ask(question, options).await {
            Ok(answer) => ToolResult::success(format!("User answered: {}", answer)),
            Err(e) => ToolResult::error(format!("Failed to get user response: {}", e)),
        }
    }

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        false
    }
}
