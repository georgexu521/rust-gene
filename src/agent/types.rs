//! Agent 类型定义

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

/// Agent ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new() -> Self {
        Self(format!("agent_{}", &Uuid::new_v4().to_string()[..8]))
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Agent 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// 待启动
    Pending,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

impl AgentStatus {
    /// 是否处于终态
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentStatus::Completed | AgentStatus::Failed | AgentStatus::Cancelled
        )
    }
}

/// Agent 间消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub from: AgentId,
    pub to: AgentId,
    pub content: String,
    pub timestamp: SystemTime,
    pub msg_type: AgentMessageType,
}

impl AgentMessage {
    pub fn new(
        from: AgentId,
        to: AgentId,
        content: impl Into<String>,
        msg_type: AgentMessageType,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from,
            to,
            content: content.into(),
            timestamp: SystemTime::now(),
            msg_type,
        }
    }
}

/// 消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageType {
    /// 任务分配
    Task,
    /// 结果汇报
    Result,
    /// 状态更新
    Status,
    /// 询问
    Query,
    /// 控制命令
    Control,
}
