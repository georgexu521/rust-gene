//! Agent 实现

use crate::agent::manager::AgentResult;
use crate::agent::roles::AgentRole;
use crate::agent::types::{AgentId, AgentMessage, AgentMessageType, AgentStatus};
use crate::engine::QueryEngine;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
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
        self
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
    /// 共享状态 - Agent 和 Manager 都持有同一个 Arc
    pub status: Arc<RwLock<AgentStatus>>,
}

/// Agent 实例
pub struct Agent {
    /// Agent ID
    pub id: AgentId,
    /// 配置
    pub config: AgentConfig,
    /// 共享状态（与 AgentHandle 共享同一个 Arc）
    status: Arc<RwLock<AgentStatus>>,
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

        Self {
            id,
            config,
            status: Arc::new(RwLock::new(AgentStatus::Pending)),
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

    /// 获取句柄（共享同一个状态 Arc）
    pub fn handle(&self) -> AgentHandle {
        AgentHandle {
            id: self.id.clone(),
            config: self.config.clone(),
            status: self.status.clone(),
        }
    }

    /// 设置状态
    async fn set_status(&self, status: AgentStatus) {
        let mut s = self.status.write().await;
        *s = status;
    }

    /// 启动 Agent
    pub async fn run(mut self) {
        info!("Agent {} starting...", self.id);
        self.set_status(AgentStatus::Running).await;

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

        // 主循环
        // TODO: 将 status 的 RwLock 替换为 watch::channel，以消除这里的轮询
        let check_interval = tokio::time::Duration::from_millis(500);
        loop {
            tokio::select! {
                Some(msg) = self.receiver.recv() => {
                    if let Err(e) = self.handle_message(msg).await {
                        error!("Agent {} failed to handle message: {}", self.id, e);
                    }
                }
                _ = tokio::time::sleep(check_interval) => {
                    // 周期性检查，防止外部直接设置 terminal 状态后卡在 recv()
                }
                else => {
                    // 通道关闭
                    break;
                }
            }

            if self.status.read().await.is_terminal() {
                break;
            }
        }

        // 发送最终结果给 AgentManager
        let final_status = *self.status.read().await;
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
                confidence: 1.0,      // 默认置信度
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
                    self.set_status(AgentStatus::Cancelled).await;
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
        info!("Agent {} executing task: {}", self.id, task);
        self.task_history.push(task.to_string());
        self.set_status(AgentStatus::Running).await;

        // 使用 QueryEngine 的 query_with_tools 执行任务（子 Agent 也能使用工具）
        let mut options = crate::engine::query_engine::QueryOptions::default()
            .with_max_iterations(self.config.max_turns.min(self.config.max_tool_calls));
        if let Some(ref allowed) = self.config.allowed_tools {
            options = options.with_allowed_tools(allowed.clone());
        }
        let agent_system_prompt = self.config.build_system_prompt();
        let cost_before = self.query_engine.estimated_cost_usd().await;

        let result = self
            .query_engine
            .query_with_tools_with_system_prompt(task, options, Some(&agent_system_prompt))
            .await;
        let cost_after = self.query_engine.estimated_cost_usd().await;
        let cost_delta = (cost_after - cost_before).max(0.0);

        // 发送结果给父 Agent
        let result_content = match result {
            Ok(query_result) => {
                if let Some(limit) = self.config.max_cost_usd {
                    if cost_delta > limit {
                        self.set_status(AgentStatus::Failed).await;
                        let content = format!(
                            "Task exceeded cost budget: used ${:.4}, limit ${:.4}\nPartial result:\n{}",
                            cost_delta, limit, query_result.content
                        );
                        self.last_result = Some(content.clone());
                        content
                    } else {
                        self.set_status(AgentStatus::Completed).await;
                        let content = format!("Task completed:\n{}", query_result.content);
                        self.last_result = Some(query_result.content);
                        content
                    }
                } else {
                    self.set_status(AgentStatus::Completed).await;
                    let content = format!("Task completed:\n{}", query_result.content);
                    self.last_result = Some(query_result.content);
                    content
                }
            }
            Err(e) => {
                self.set_status(AgentStatus::Failed).await;
                let content = format!("Task failed: {}", e);
                self.last_result = Some(content.clone());
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
