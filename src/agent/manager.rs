//! Agent 管理器
//!
//! 管理所有 Agent 的生命周期

use crate::agent::agent::{Agent, AgentConfig, AgentHandle};
use crate::agent::types::{AgentId, AgentMessage, AgentStatus};
use crate::engine::QueryEngine;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};

/// Agent 结果
#[derive(Debug, Clone)]
pub struct AgentResult {
    pub agent_id: AgentId,
    pub status: AgentStatus,
    pub content: String,
    pub completed_at: std::time::Instant,
}

/// Agent 管理器
#[derive(Debug)]
pub struct AgentManager {
    /// 所有 Agent
    agents: Arc<RwLock<HashMap<AgentId, AgentHandle>>>,
    /// 消息通道
    message_channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<AgentMessage>>>>,
    /// QueryEngine 引用
    query_engine: Option<Arc<QueryEngine>>,
    /// Agent 结果存储（用于收集子 Agent 结果）
    results: Arc<RwLock<HashMap<AgentId, AgentResult>>>,
    /// 完成通知接收器（用于 wait_for_result 的事件驱动等待）
    completion_receivers:
        Arc<RwLock<HashMap<AgentId, tokio::sync::oneshot::Receiver<AgentResult>>>>,
}

impl AgentManager {
    /// 创建新的管理器（自动启动 reaper 任务）
    pub fn new() -> Self {
        let agents: Arc<RwLock<HashMap<AgentId, AgentHandle>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let message_channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<AgentMessage>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let results: Arc<RwLock<HashMap<AgentId, AgentResult>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let completion_receivers: Arc<
            RwLock<HashMap<AgentId, tokio::sync::oneshot::Receiver<AgentResult>>>,
        > = Arc::new(RwLock::new(HashMap::new()));

        // 启动周期性清理任务（每 30 秒回收已终止的 Agent 及其结果）
        let agents_for_reaper = agents.clone();
        let channels_for_reaper = message_channels.clone();
        let receivers_for_reaper = completion_receivers.clone();
        let results_for_reaper = results.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                let mut agents = agents_for_reaper.write().await;
                let mut channels = channels_for_reaper.write().await;
                let mut receivers = receivers_for_reaper.write().await;
                let mut results_map = results_for_reaper.write().await;

                let completed: Vec<_> = agents
                    .iter()
                    .filter_map(|(id, handle)| {
                        if handle
                            .status
                            .try_read()
                            .map(|s| s.is_terminal())
                            .unwrap_or(false)
                        {
                            Some(id.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                for id in completed {
                    agents.remove(&id);
                    channels.remove(&id);
                    receivers.remove(&id);
                    results_map.remove(&id);
                    debug!("Reaper cleaned up agent: {}", id);
                }

                // 若 results 积压过多（>200），保留最近的 100 条
                if results_map.len() > 200 {
                    let mut sorted: Vec<_> = results_map.drain().collect();
                    sorted.sort_by_key(|(_, r)| r.completed_at);
                    let keep = sorted.into_iter().rev().take(100);
                    results_map.extend(keep);
                    debug!("Reaper capped results to 100 entries");
                }
            }
        });

        Self {
            agents,
            message_channels,
            query_engine: None,
            results,
            completion_receivers,
        }
    }

    /// 设置 QueryEngine
    pub fn with_query_engine(mut self, engine: Arc<QueryEngine>) -> Self {
        self.query_engine = Some(engine);
        self
    }

    /// 创建并启动子 Agent
    pub async fn spawn(
        &self,
        config: AgentConfig,
        parent_id: Option<AgentId>,
    ) -> anyhow::Result<AgentId> {
        let (tx, rx) = mpsc::channel::<AgentMessage>(100);

        let agent_id = AgentId::new();

        // 获取父 Agent 的发送通道
        let parent_sender = if let Some(ref pid) = parent_id {
            self.message_channels.read().await.get(pid).cloned()
        } else {
            None
        };

        let query_engine = self
            .query_engine
            .clone()
            .ok_or_else(|| anyhow::anyhow!("QueryEngine not set"))?;

        // 创建结果收集通道
        let (result_tx, mut result_rx) = tokio::sync::mpsc::channel::<AgentResult>(1);

        // 创建完成通知通道
        let (completion_tx, completion_rx) = tokio::sync::oneshot::channel::<AgentResult>();
        {
            let mut receivers = self.completion_receivers.write().await;
            receivers.insert(agent_id.clone(), completion_rx);
        }

        // 创建 Agent
        let mut config = config;
        config.parent_id = parent_id;

        let agent = Agent::new(agent_id.clone(), config, rx, parent_sender, query_engine)
            .with_result_sender(result_tx);

        let handle = agent.handle();

        // 记录 Agent
        {
            let mut agents = self.agents.write().await;
            agents.insert(agent_id.clone(), handle);
        }

        // 记录消息通道
        {
            let mut channels = self.message_channels.write().await;
            channels.insert(agent_id.clone(), tx);
        }

        info!("Spawned agent: {}", agent_id);

        let results = self.results.clone();
        let agent_id_for_task = agent_id.clone();

        // 启动 Agent
        tokio::spawn(async move {
            agent.run().await;
        });

        // 启动结果收集任务
        tokio::spawn(async move {
            // 等待 Agent 完成的消息
            if let Some(msg) = result_rx.recv().await {
                let mut results_map = results.write().await;
                results_map.insert(agent_id_for_task.clone(), msg.clone());
                // 通知等待者
                let _ = completion_tx.send(msg);
            }
        });

        Ok(agent_id)
    }

    /// 获取 Agent 结果
    pub async fn get_result(&self, agent_id: &AgentId) -> Option<AgentResult> {
        let results = self.results.read().await;
        results.get(agent_id).cloned()
    }

    /// 等待 Agent 完成并返回结果（带超时，使用 oneshot 事件驱动）
    pub async fn wait_for_result(
        &self,
        agent_id: &AgentId,
        timeout_secs: u64,
    ) -> anyhow::Result<AgentResult> {
        // 快速路径：结果已存在
        if let Some(result) = self.get_result(agent_id).await {
            return Ok(result);
        }

        // 获取并移除通知接收器
        let rx = {
            let mut receivers = self.completion_receivers.write().await;
            receivers.remove(agent_id)
        };

        match rx {
            Some(rx) => {
                match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx).await {
                    Ok(Ok(result)) => Ok(result),
                    Ok(Err(_)) => {
                        // 通道关闭，尝试从结果缓存读取
                        if let Some(result) = self.get_result(agent_id).await {
                            Ok(result)
                        } else {
                            Err(anyhow::anyhow!(
                                "Agent {} result channel closed without result",
                                agent_id
                            ))
                        }
                    }
                    Err(_) => Err(anyhow::anyhow!(
                        "Timeout waiting for agent {} result after {}s",
                        agent_id,
                        timeout_secs
                    )),
                }
            }
            None => {
                // 没有接收器了，再检查一次结果缓存
                if let Some(result) = self.get_result(agent_id).await {
                    Ok(result)
                } else {
                    Err(anyhow::anyhow!("Agent {} not found", agent_id))
                }
            }
        }
    }

    /// 向 Agent 发送消息
    pub async fn send_message(&self, to: &AgentId, message: AgentMessage) -> anyhow::Result<()> {
        let channels = self.message_channels.read().await;

        if let Some(sender) = channels.get(to) {
            sender
                .send(message)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Agent {} not found", to))
        }
    }

    /// 获取 Agent 状态
    pub async fn get_status(&self, agent_id: &AgentId) -> Option<AgentStatus> {
        let agents = self.agents.read().await;
        if let Some(handle) = agents.get(agent_id) {
            Some(*handle.status.read().await)
        } else {
            None
        }
    }

    /// 列出所有 Agent
    pub async fn list_agents(&self) -> Vec<AgentHandle> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// 终止 Agent
    pub async fn kill(&self, agent_id: &AgentId) -> anyhow::Result<()> {
        // 发送停止命令
        let stop_msg = AgentMessage::new(
            AgentId::new(),
            agent_id.clone(),
            "stop",
            crate::agent::types::AgentMessageType::Control,
        );

        self.send_message(agent_id, stop_msg).await?;

        // 移除通道
        {
            let mut channels = self.message_channels.write().await;
            channels.remove(agent_id);
        }

        info!("Killed agent: {}", agent_id);
        Ok(())
    }

    /// 等待 Agent 完成
    pub async fn wait_for(
        &self,
        agent_id: &AgentId,
        timeout_secs: u64,
    ) -> anyhow::Result<AgentStatus> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);

        loop {
            if let Some(status) = self.get_status(agent_id).await {
                if status.is_terminal() {
                    return Ok(status);
                }
            } else {
                return Err(anyhow::anyhow!("Agent {} not found", agent_id));
            }

            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for agent {}", agent_id));
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// 清理已完成的 Agent
    pub async fn cleanup(&self) {
        let mut agents = self.agents.write().await;
        let mut channels = self.message_channels.write().await;
        let mut receivers = self.completion_receivers.write().await;

        let mut completed = Vec::new();
        for (id, handle) in agents.iter() {
            if handle.status.read().await.is_terminal() {
                completed.push(id.clone());
            }
        }

        for id in completed {
            agents.remove(&id);
            channels.remove(&id);
            receivers.remove(&id);
            debug!("Cleaned up agent: {}", id);
        }
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_manager() {
        let manager = AgentManager::new();

        // 由于没有 QueryEngine，spawn 会失败
        let result = manager.spawn(AgentConfig::new("test-agent"), None).await;

        assert!(result.is_err());
    }
}
