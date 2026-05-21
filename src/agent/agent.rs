//! Agent 实现

use crate::agent::envelope::{AgentArtifact, AgentTaskEnvelope};
use crate::agent::manager::AgentResult;
use crate::agent::roles::AgentRole;
use crate::agent::types::{AgentId, AgentMessage, AgentMessageType, AgentStatus};
use crate::engine::QueryEngine;
use crate::services::api::Message;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tracing::{debug, error, info, warn};

/// Agent 配置
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Agent 名称
    pub name: String,
    /// Agent 描述
    pub description: String,
    /// 系统提示词
    pub system_prompt: String,
    /// 父 Agent ID（如果是子 Agent）
    pub parent_id: Option<AgentId>,
    /// 最大工具调用次数
    pub max_tool_calls: usize,
    /// 最大对话轮次（工具循环上限）
    pub max_turns: usize,
    /// 超时时间（秒）
    pub timeout_secs: u64,
    /// 最大成本预算（美元，按单次 agent 任务增量计算）
    pub max_cost_usd: Option<f64>,
    /// Agent 角色
    pub role: AgentRole,
    /// 允许的工具白名单（None 表示不限制）
    pub allowed_tools: Option<Vec<String>>,
    /// Initial context inherited by forked subagents.
    pub context_messages: Vec<Message>,
    /// Optional isolated working directory for this agent.
    pub working_dir: Option<PathBuf>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "Agent".to_string(),
            description: String::new(),
            system_prompt: "You are a helpful assistant.".to_string(),
            parent_id: None,
            max_tool_calls: 50,
            max_turns: 10,
            timeout_secs: 600,
            max_cost_usd: None,
            role: AgentRole::default(),
            allowed_tools: None,
            context_messages: Vec::new(),
            working_dir: None,
        }
    }
}

impl AgentConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn as_child_of(mut self, parent_id: AgentId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    pub fn with_role(mut self, role: AgentRole) -> Self {
        self.role = role;
        // 自动从角色设置系统提示词前缀（如果当前是默认提示词）
        if self.system_prompt == "You are a helpful assistant." {
            self.system_prompt = role.system_prompt_prefix().to_string();
        } else {
            // 否则将角色前缀追加到现有提示词
            self.system_prompt =
                format!("{}\n\n{}", role.system_prompt_prefix(), self.system_prompt);
        }
        self
    }

    /// 从角色名构建配置
    pub fn from_role(role: AgentRole) -> Self {
        Self::new(role.display_name()).with_role(role)
    }

    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns.max(1);
        self
    }

    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        if !tools.is_empty() {
            self.allowed_tools = Some(tools);
        }
        self
    }

    pub fn with_context_messages(mut self, messages: Vec<Message>) -> Self {
        self.context_messages = messages;
        self
    }

    pub fn with_working_dir(mut self, working_dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(working_dir.into());
        self
    }

    pub fn with_max_cost_usd(mut self, max_cost_usd: f64) -> Self {
        if max_cost_usd > 0.0 {
            self.max_cost_usd = Some(max_cost_usd);
        }
        self
    }

    /// 构建完整的系统提示词（角色前缀 + 自定义内容）
    pub fn build_system_prompt(&self) -> String {
        let prefix = self.role.system_prompt_prefix();
        format!("{}\n\n{}", prefix, self.system_prompt)
    }
}

/// Agent 句柄
#[derive(Debug, Clone)]
pub struct AgentHandle {
    pub id: AgentId,
    pub config: AgentConfig,
    /// 共享状态 - 通过 watch channel 订阅状态变化
    pub status: watch::Receiver<AgentStatus>,
}

/// Agent 实例
pub struct Agent {
    /// Agent ID
    pub id: AgentId,
    /// 配置
    pub config: AgentConfig,
    /// 状态发送端
    status_tx: watch::Sender<AgentStatus>,
    /// 状态接收端（与 AgentHandle 共享同一个 watch channel）
    status_rx: watch::Receiver<AgentStatus>,
    /// 消息接收通道
    pub receiver: mpsc::Receiver<AgentMessage>,
    /// 消息发送通道（用于向父 Agent 发送消息）
    pub sender: Option<mpsc::Sender<AgentMessage>>,
    /// 结果发送通道（用于向 AgentManager 报告结果）
    pub result_sender: Option<mpsc::Sender<AgentResult>>,
    /// 查询引擎
    pub query_engine: Arc<QueryEngine>,
    /// 任务历史
    pub task_history: Vec<String>,
    /// 最后一次执行结果（实际 LLM 输出）
    pub last_result: Option<String>,
}

impl Agent {
    /// 创建新的 Agent
    pub fn new(
        id: AgentId,
        config: AgentConfig,
        receiver: mpsc::Receiver<AgentMessage>,
        sender: Option<mpsc::Sender<AgentMessage>>,
        query_engine: Arc<QueryEngine>,
    ) -> Self {
        info!("Creating new agent: {} ({})", id, config.name);

        let (status_tx, status_rx) = watch::channel(AgentStatus::Pending);
        Self {
            id,
            config,
            status_tx,
            status_rx,
            receiver,
            sender,
            result_sender: None,
            query_engine,
            task_history: Vec::new(),
            last_result: None,
        }
    }

    /// 设置结果发送通道
    pub fn with_result_sender(mut self, sender: mpsc::Sender<AgentResult>) -> Self {
        self.result_sender = Some(sender);
        self
    }

    /// 获取句柄（共享同一个 watch channel）
    pub fn handle(&self) -> AgentHandle {
        AgentHandle {
            id: self.id.clone(),
            config: self.config.clone(),
            status: self.status_rx.clone(),
        }
    }

    /// 设置状态
    fn set_status(&self, status: AgentStatus) {
        let _ = self.status_tx.send(status);
    }

    /// 启动 Agent
    pub async fn run(mut self) {
        info!("Agent {} starting...", self.id);
        self.set_status(AgentStatus::Running);

        // 发送就绪消息给父 Agent
        if let Some(ref sender) = self.sender {
            let ready_msg = AgentMessage::new(
                self.id.clone(),
                self.config.parent_id.clone().unwrap_or_default(),
                "Agent ready".to_string(),
                AgentMessageType::Status,
            );
            let _ = sender.send(ready_msg).await;
        }

        // 主循环 - 使用 watch channel 等待状态变化，消除轮询
        loop {
            // 先检查当前状态，如果是终态立即退出
            if self.status_rx.borrow().is_terminal() {
                break;
            }

            tokio::select! {
                Some(msg) = self.receiver.recv() => {
                    if let Err(e) = self.handle_message(msg).await {
                        error!("Agent {} failed to handle message: {}", self.id, e);
                    }
                }
                Ok(()) = self.status_rx.changed() => {
                    // 状态发生变化，循环会重新检查
                }
                else => {
                    // 通道关闭
                    break;
                }
            }
        }

        // 发送最终结果给 AgentManager
        let final_status = *self.status_rx.borrow();
        if let Some(ref result_sender) = self.result_sender {
            let result = AgentResult {
                agent_id: self.id.clone(),
                status: final_status,
                content: self
                    .last_result
                    .clone()
                    .unwrap_or_else(|| self.task_history.join("\n")),
                completed_at: std::time::Instant::now(),
                tools_used: Vec::new(), // 工具历史可在此扩展
                confidence: 1.0,        // 默认置信度
                has_conflict: false,    // 默认无冲突
            };
            let _ = result_sender.send(result).await;
        }

        info!("Agent {} finished with status: {:?}", self.id, final_status);
    }

    /// 处理消息
    async fn handle_message(&mut self, msg: AgentMessage) -> anyhow::Result<()> {
        debug!("Agent {} received message: {:?}", self.id, msg.msg_type);

        match msg.msg_type {
            AgentMessageType::Task => {
                // 执行任务
                self.execute_task(&msg.content).await?;
            }
            AgentMessageType::Control => {
                // 控制命令
                if msg.content == "stop" {
                    self.set_status(AgentStatus::Cancelled);
                }
            }
            AgentMessageType::Query => {
                // 回复查询
                self.reply_to(&msg, "Query received").await?;
            }
            _ => {
                // 其他消息类型暂不支持
                warn!("Unhandled message type: {:?}", msg.msg_type);
            }
        }

        Ok(())
    }

    /// 执行任务
    async fn execute_task(&mut self, task: &str) -> anyhow::Result<()> {
        let (mut envelope, executable_task) = extract_task_envelope(task);
        if let Some(env) = envelope.as_mut() {
            env.mark_running("agent accepted task");
            let _ = crate::agent::a2a_transcript::append_envelope(env);
        }

        info!("Agent {} executing task: {}", self.id, executable_task);
        self.task_history.push(executable_task.clone());
        self.set_status(AgentStatus::Running);

        // 使用 QueryEngine 的 query_with_tools 执行任务（子 Agent 也能使用工具）
        let mut options = crate::engine::query_engine::QueryOptions::default()
            .with_max_iterations(self.config.max_turns.min(self.config.max_tool_calls));
        if let Some(ref allowed) = self.config.allowed_tools {
            options = options.with_allowed_tools(allowed.clone());
        }
        let fork_context_active = crate::agent::forked_context::messages_contain_fork_boilerplate(
            &self.config.context_messages,
        );
        if !self.config.context_messages.is_empty() {
            options = options.with_context(self.config.context_messages.clone());
        }
        if let Some(working_dir) = self.config.working_dir.clone() {
            options = options.with_working_dir(working_dir);
        }
        let agent_system_prompt = self.config.build_system_prompt();
        let cost_before = self.query_engine.estimated_cost_usd().await;
        let user_task = if fork_context_active {
            "Execute the fork directive above. Do not spawn additional sub-agents.".to_string()
        } else {
            executable_task.clone()
        };

        let result = self
            .query_engine
            .query_with_tools_with_system_prompt(&user_task, options, Some(&agent_system_prompt))
            .await;
        let cost_after = self.query_engine.estimated_cost_usd().await;
        let cost_delta = (cost_after - cost_before).max(0.0);

        // 发送结果给父 Agent
        let result_content = match result {
            Ok(query_result) => {
                if let Some(limit) = self.config.max_cost_usd {
                    if cost_delta > limit {
                        self.set_status(AgentStatus::Failed);
                        let content = format!(
                            "Task exceeded cost budget: used ${:.4}, limit ${:.4}\nPartial result:\n{}",
                            cost_delta, limit, query_result.content
                        );
                        self.last_result = Some(content.clone());
                        if let Some(env) = envelope.as_mut() {
                            env.fail_with_error(
                                "agent_cost_budget_exceeded",
                                format!("used ${:.4}, limit ${:.4}", cost_delta, limit),
                                false,
                            );
                            let _ = crate::agent::a2a_transcript::append_envelope(env);
                        }
                        content
                    } else {
                        self.set_status(AgentStatus::Completed);
                        let content = format!("Task completed:\n{}", query_result.content);
                        self.last_result = Some(query_result.content);
                        if let Some(env) = envelope.as_mut() {
                            env.complete_with_artifact(AgentArtifact {
                                kind: "result".to_string(),
                                title: "Agent result".to_string(),
                                content: content.clone(),
                            });
                            let _ = crate::agent::a2a_transcript::append_envelope(env);
                        }
                        content
                    }
                } else {
                    self.set_status(AgentStatus::Completed);
                    let content = format!("Task completed:\n{}", query_result.content);
                    self.last_result = Some(query_result.content);
                    if let Some(env) = envelope.as_mut() {
                        env.complete_with_artifact(AgentArtifact {
                            kind: "result".to_string(),
                            title: "Agent result".to_string(),
                            content: content.clone(),
                        });
                        let _ = crate::agent::a2a_transcript::append_envelope(env);
                    }
                    content
                }
            }
            Err(e) => {
                self.set_status(AgentStatus::Failed);
                let content = format!("Task failed: {}", e);
                self.last_result = Some(content.clone());
                if let Some(env) = envelope.as_mut() {
                    env.fail_with_error("agent_task_failed", e.to_string(), true);
                    let _ = crate::agent::a2a_transcript::append_envelope(env);
                }
                content
            }
        };

        if let Some(ref sender) = self.sender {
            let parent_id = self.config.parent_id.clone().unwrap_or_default();

            let result_msg = AgentMessage::new(
                self.id.clone(),
                parent_id,
                result_content,
                AgentMessageType::Result,
            );
            sender.send(result_msg).await?;
        }

        Ok(())
    }

    /// 回复消息
    async fn reply_to(&self, to: &AgentMessage, content: impl Into<String>) -> anyhow::Result<()> {
        if let Some(ref sender) = self.sender {
            let reply = AgentMessage::new(
                self.id.clone(),
                to.from.clone(),
                content,
                AgentMessageType::Result,
            );
            sender.send(reply).await?;
        }
        Ok(())
    }
}

fn extract_task_envelope(task: &str) -> (Option<AgentTaskEnvelope>, String) {
    const OPEN: &str = "<agent-task-envelope>";
    const CLOSE: &str = "</agent-task-envelope>";

    let Some(start) = task.find(OPEN) else {
        return (None, task.to_string());
    };
    let after_open = start + OPEN.len();
    let Some(relative_end) = task[after_open..].find(CLOSE) else {
        return (None, task.to_string());
    };
    let end = after_open + relative_end;
    let envelope_json = task[after_open..end].trim();
    let remainder = task[end + CLOSE.len()..].trim().to_string();
    match serde_json::from_str::<AgentTaskEnvelope>(envelope_json) {
        Ok(envelope) => {
            let executable = if remainder.is_empty() {
                envelope.prompt.clone()
            } else {
                remainder
            };
            (Some(envelope), executable)
        }
        Err(err) => {
            warn!("Failed to parse agent task envelope: {}", err);
            (None, task.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_agent_task_envelope_and_remainder() {
        let envelope =
            AgentTaskEnvelope::new(AgentId("parent".to_string()), "review", "fallback prompt")
                .assign_to(AgentId("child".to_string()));
        let json = serde_json::to_string(&envelope).unwrap();
        let wrapped = format!(
            "<agent-task-envelope>\n{}\n</agent-task-envelope>\n\nreal prompt",
            json
        );

        let (parsed, executable) = extract_task_envelope(&wrapped);
        assert!(parsed.is_some());
        assert_eq!(executable, "real prompt");
    }

    #[test]
    fn envelope_prompt_is_used_when_no_remainder_exists() {
        let envelope =
            AgentTaskEnvelope::new(AgentId("parent".to_string()), "review", "fallback prompt")
                .assign_to(AgentId("child".to_string()));
        let json = serde_json::to_string(&envelope).unwrap();
        let wrapped = format!("<agent-task-envelope>\n{}\n</agent-task-envelope>", json);

        let (parsed, executable) = extract_task_envelope(&wrapped);
        assert!(parsed.is_some());
        assert_eq!(executable, "fallback prompt");
    }
}
