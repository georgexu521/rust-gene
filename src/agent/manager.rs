//! Agent 管理器
//!
//! 管理所有 Agent 的生命周期

use crate::agent::agent::{Agent, AgentConfig, AgentHandle};
use crate::agent::progress::AgentProgressEvent;
use crate::agent::types::{AgentId, AgentMessage, AgentStatus};
use crate::engine::QueryEngine;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info};

/// Agent 结果
#[derive(Debug, Clone)]
pub struct AgentResult {
    pub agent_id: AgentId,
    pub status: AgentStatus,
    pub content: String,
    pub completed_at: std::time::Instant,
    /// 执行的工具列表
    pub tools_used: Vec<String>,
    /// 置信度评分 (0.0 - 1.0)
    pub confidence: f32,
    /// 冲突标记
    pub has_conflict: bool,
}

/// In-memory lifecycle counters for active sub-agents and retained outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentManagerStats {
    pub active_agents: usize,
    pub terminal_agents: usize,
    pub message_channels: usize,
    pub completion_receivers: usize,
    pub cached_results: usize,
}

/// DAG 节点状态
#[derive(Debug, Clone)]
pub struct DagNode {
    pub agent_id: AgentId,
    pub status: AgentStatus,
    pub dependencies: Vec<AgentId>,
    pub dependents: Vec<AgentId>,
    pub result: Option<AgentResult>,
}

/// Agent 编排 DAG
#[derive(Debug)]
pub struct AgentDag {
    nodes: HashMap<AgentId, DagNode>,
}

impl AgentDag {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// 添加节点及其依赖
    pub fn add_node(&mut self, agent_id: AgentId, dependencies: Vec<AgentId>) {
        let node = DagNode {
            agent_id: agent_id.clone(),
            status: AgentStatus::Pending,
            dependencies: dependencies.clone(),
            dependents: Vec::new(),
            result: None,
        };

        // 添加到 dependents 列表
        for dep in &dependencies {
            if let Some(dep_node) = self.nodes.get_mut(dep) {
                dep_node.dependents.push(agent_id.clone());
            }
        }

        self.nodes.insert(agent_id, node);
    }

    /// 获取可执行的节点（所有依赖都已完成）
    pub fn get_runnable(&self) -> Vec<AgentId> {
        self.nodes
            .iter()
            .filter(|(_, node)| {
                node.status == AgentStatus::Pending
                    && node.dependencies.iter().all(|dep_id| {
                        self.nodes
                            .get(dep_id)
                            .map(|n| n.status.is_terminal())
                            .unwrap_or(false)
                    })
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 更新节点状态
    pub fn update_status(&mut self, agent_id: &AgentId, status: AgentStatus) {
        if let Some(node) = self.nodes.get_mut(agent_id) {
            node.status = status;
        }
    }

    /// 设置节点结果
    pub fn set_result(&mut self, agent_id: &AgentId, result: AgentResult) {
        if let Some(node) = self.nodes.get_mut(agent_id) {
            node.result = Some(result);
        }
    }

    /// 检查是否有循环依赖
    pub fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node in self.nodes.values() {
            if self.detect_cycle_dfs(node, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        false
    }

    fn detect_cycle_dfs(
        &self,
        node: &DagNode,
        visited: &mut HashSet<AgentId>,
        rec_stack: &mut HashSet<AgentId>,
    ) -> bool {
        if rec_stack.contains(&node.agent_id) {
            return true;
        }
        if visited.contains(&node.agent_id) {
            return false;
        }

        visited.insert(node.agent_id.clone());
        rec_stack.insert(node.agent_id.clone());

        for dep in &node.dependencies {
            if let Some(dep_node) = self.nodes.get(dep) {
                if self.detect_cycle_dfs(dep_node, visited, rec_stack) {
                    return true;
                }
            }
        }

        rec_stack.remove(&node.agent_id);
        false
    }

    /// 获取拓扑排序
    pub fn topological_sort(&self) -> Vec<AgentId> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();

        for id in self.nodes.keys() {
            if !visited.contains(id) {
                self.ts_dfs(id, &mut visited, &mut result);
            }
        }

        result
    }

    fn ts_dfs(&self, id: &AgentId, visited: &mut HashSet<AgentId>, result: &mut Vec<AgentId>) {
        visited.insert(id.clone());
        if let Some(node) = self.nodes.get(id) {
            for dep in &node.dependencies {
                if !visited.contains(dep) {
                    self.ts_dfs(dep, visited, result);
                }
            }
            result.push(id.clone());
        }
    }

    /// 获取所有节点状态
    pub fn get_all_statuses(&self) -> HashMap<AgentId, AgentStatus> {
        self.nodes
            .iter()
            .map(|(id, node)| (id.clone(), node.status))
            .collect()
    }
}

impl Default for AgentDag {
    fn default() -> Self {
        Self::new()
    }
}

/// 结果融合器
#[derive(Debug, Clone)]
pub struct ResultFusion {
    pub conflict_threshold: f32,
}

impl ResultFusion {
    pub fn new() -> Self {
        Self {
            conflict_threshold: 0.7,
        }
    }

    /// 融合多个 Agent 的结果
    pub fn fuse(&self, results: Vec<AgentResult>) -> FusedResult {
        if results.is_empty() {
            return FusedResult {
                content: String::new(),
                confidence: 0.0,
                conflicts: Vec::new(),
                evidence: HashMap::new(),
            };
        }

        if results.len() == 1 {
            let r = &results[0];
            return FusedResult {
                content: r.content.clone(),
                confidence: r.confidence,
                conflicts: Vec::new(),
                evidence: HashMap::new(),
            };
        }

        // 检查冲突
        let mut conflicts = Vec::new();
        let mut evidence: HashMap<String, Vec<AgentId>> = HashMap::new();

        for result in &results {
            // 按内容分组作为简单冲突检测
            let key = result.content.chars().take(100).collect::<String>();
            evidence
                .entry(key)
                .or_default()
                .push(result.agent_id.clone());
        }

        // 检测是否有低置信度冲突
        let low_conf_results: Vec<_> = results
            .iter()
            .filter(|r| r.confidence < self.conflict_threshold)
            .collect();

        if low_conf_results.len() > 1 {
            conflicts.push("Multiple low-confidence results detected".to_string());
        }

        // 计算平均置信度
        let avg_confidence: f32 =
            results.iter().map(|r| r.confidence).sum::<f32>() / results.len() as f32;

        // 简单融合：选择最高置信度的结果
        let best = results
            .iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .cloned();

        FusedResult {
            content: best.map(|r| r.content).unwrap_or_default(),
            confidence: avg_confidence,
            conflicts,
            evidence,
        }
    }
}

impl Default for ResultFusion {
    fn default() -> Self {
        Self::new()
    }
}

/// 融合结果
#[derive(Debug, Clone)]
pub struct FusedResult {
    pub content: String,
    pub confidence: f32,
    pub conflicts: Vec<String>,
    pub evidence: HashMap<String, Vec<AgentId>>,
}

/// Agent 审计记录
#[derive(Debug, Clone)]
pub struct AgentAuditRecord {
    pub agent_id: AgentId,
    pub action: AgentAuditAction,
    pub timestamp: std::time::SystemTime,
    pub details: String,
}

#[derive(Debug, Clone)]
pub enum AgentAuditAction {
    Spawn,
    StatusChange,
    Message,
    Result,
    Kill,
    Error,
}

/// Agent 审计器
#[derive(Debug)]
pub struct AgentAuditor {
    records: Arc<RwLock<Vec<AgentAuditRecord>>>,
}

impl AgentAuditor {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn log(&self, agent_id: AgentId, action: AgentAuditAction, details: String) {
        let record = AgentAuditRecord {
            agent_id,
            action,
            timestamp: std::time::SystemTime::now(),
            details,
        };
        let mut records = self.records.write().await;
        records.push(record);
    }

    pub async fn get_records(&self, agent_id: Option<&AgentId>) -> Vec<AgentAuditRecord> {
        let records = self.records.read().await;
        match agent_id {
            Some(id) => records
                .iter()
                .filter(|r| &r.agent_id == id)
                .cloned()
                .collect(),
            None => records.clone(),
        }
    }

    pub async fn clear(&self) {
        let mut records = self.records.write().await;
        records.clear();
    }
}

impl Default for AgentAuditor {
    fn default() -> Self {
        Self::new()
    }
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
    /// Progress broadcast channels for sub-agents (one sender per agent).
    progress_senders: Arc<RwLock<HashMap<AgentId, broadcast::Sender<AgentProgressEvent>>>>,
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
        let progress_senders: Arc<RwLock<HashMap<AgentId, broadcast::Sender<AgentProgressEvent>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        // 启动周期性清理任务（每 30 秒回收已终止的 Agent 句柄和通道）。
        // Agent results remain available for resume/read until the capped result
        // cache prunes older entries below.
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
                        if handle.status.borrow().is_terminal() {
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
            progress_senders,
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

    /// Subscribe to progress events for a sub-agent.
    ///
    /// Returns a broadcast receiver that yields `AgentProgressEvent`s as the
    /// sub-agent executes. The channel is created on first subscription.
    /// Capacity: 64 events; lagging receivers will miss older events.
    pub async fn subscribe_progress(
        &self,
        agent_id: &AgentId,
    ) -> broadcast::Receiver<AgentProgressEvent> {
        let mut senders = self.progress_senders.write().await;
        if let Some(sender) = senders.get(agent_id) {
            sender.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(64);
            senders.insert(agent_id.clone(), tx);
            rx
        }
    }

    /// Emit a progress event for a sub-agent (no-op if no subscribers).
    pub async fn emit_progress(&self, agent_id: &AgentId, event: AgentProgressEvent) {
        let senders = self.progress_senders.read().await;
        if let Some(tx) = senders.get(agent_id) {
            let _ = tx.send(event);
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
        agents.get(agent_id).map(|handle| *handle.status.borrow())
    }

    /// 列出所有 Agent
    pub async fn list_agents(&self) -> Vec<AgentHandle> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Snapshot lifecycle counters for UI/debug surfaces.
    pub async fn stats(&self) -> AgentManagerStats {
        let agents = self.agents.read().await;
        let terminal_agents = agents
            .values()
            .filter(|handle| handle.status.borrow().is_terminal())
            .count();
        let message_channels = self.message_channels.read().await.len();
        let completion_receivers = self.completion_receivers.read().await.len();
        let cached_results = self.results.read().await.len();
        AgentManagerStats {
            active_agents: agents.len(),
            terminal_agents,
            message_channels,
            completion_receivers,
            cached_results,
        }
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
        let agents = self.agents.read().await;
        let mut status_rx = if let Some(handle) = agents.get(agent_id) {
            handle.status.clone()
        } else {
            return Err(anyhow::anyhow!("Agent {} not found", agent_id));
        };
        drop(agents); // 释放锁，避免持有锁等待

        // 先检查当前状态
        let current = *status_rx.borrow();
        if current.is_terminal() {
            return Ok(current);
        }

        // 使用 watch channel 等待状态变化，消除轮询
        let timeout = tokio::time::Duration::from_secs(timeout_secs);
        let result = tokio::time::timeout(timeout, async {
            loop {
                match status_rx.changed().await {
                    Ok(()) => {
                        let status = *status_rx.borrow();
                        if status.is_terminal() {
                            return Ok(status);
                        }
                    }
                    Err(_) => {
                        return Err(anyhow::anyhow!("Agent {} status channel closed", agent_id));
                    }
                }
            }
        })
        .await;

        match result {
            Ok(status) => status,
            Err(_) => Err(anyhow::anyhow!("Timeout waiting for agent {}", agent_id)),
        }
    }

    /// 清理已完成的 Agent
    pub async fn cleanup(&self) {
        let mut agents = self.agents.write().await;
        let mut channels = self.message_channels.write().await;
        let mut receivers = self.completion_receivers.write().await;

        let mut completed = Vec::new();
        for (id, handle) in agents.iter() {
            if handle.status.borrow().is_terminal() {
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

    #[tokio::test]
    async fn test_wait_for_result_returns_cached_completed_result() {
        let manager = AgentManager::new();
        let agent_id = AgentId::new();
        let result = AgentResult {
            agent_id: agent_id.clone(),
            status: AgentStatus::Completed,
            content: "done".to_string(),
            completed_at: std::time::Instant::now(),
            tools_used: vec!["file_read".to_string()],
            confidence: 0.95,
            has_conflict: false,
        };
        manager
            .results
            .write()
            .await
            .insert(agent_id.clone(), result.clone());

        let loaded = manager
            .wait_for_result(&agent_id, 1)
            .await
            .expect("cached result should be resumable");
        assert_eq!(loaded.agent_id, agent_id);
        assert_eq!(loaded.status, AgentStatus::Completed);
        assert_eq!(loaded.content, "done");
    }

    #[tokio::test]
    async fn test_wait_for_result_returns_cached_failed_result() {
        let manager = AgentManager::new();
        let agent_id = AgentId::new();
        manager.results.write().await.insert(
            agent_id.clone(),
            AgentResult {
                agent_id: agent_id.clone(),
                status: AgentStatus::Failed,
                content: "failed".to_string(),
                completed_at: std::time::Instant::now(),
                tools_used: Vec::new(),
                confidence: 0.0,
                has_conflict: false,
            },
        );

        let loaded = manager
            .wait_for_result(&agent_id, 1)
            .await
            .expect("failed result should still be resumable");
        assert_eq!(loaded.status, AgentStatus::Failed);
        assert_eq!(loaded.content, "failed");
    }

    #[tokio::test]
    async fn test_wait_for_result_times_out_pending_receiver() {
        let manager = AgentManager::new();
        let agent_id = AgentId::new();
        let (_tx, rx) = tokio::sync::oneshot::channel::<AgentResult>();
        manager
            .completion_receivers
            .write()
            .await
            .insert(agent_id.clone(), rx);

        let err = manager
            .wait_for_result(&agent_id, 0)
            .await
            .expect_err("pending receiver should time out");
        assert!(err.to_string().contains("Timeout waiting for agent"));
    }

    #[tokio::test]
    async fn test_kill_sends_cancel_message_and_removes_channel() {
        let manager = AgentManager::new();
        let agent_id = AgentId::new();
        let (tx, mut rx) = mpsc::channel::<AgentMessage>(1);
        manager
            .message_channels
            .write()
            .await
            .insert(agent_id.clone(), tx);

        manager
            .kill(&agent_id)
            .await
            .expect("kill should send stop");
        let msg = rx.recv().await.expect("stop message");
        assert_eq!(msg.to, agent_id);
        assert_eq!(msg.content, "stop");
        assert!(matches!(
            msg.msg_type,
            crate::agent::types::AgentMessageType::Control
        ));
        assert!(!manager
            .message_channels
            .read()
            .await
            .contains_key(&agent_id));
    }

    // ─── DAG Tests ─────────────────────────────────────────────────────────────

    #[test]
    fn test_dag_add_node() {
        let mut dag = AgentDag::new();
        let id1 = AgentId::new();
        let id2 = AgentId::new();

        dag.add_node(id1.clone(), Vec::new());
        dag.add_node(id2.clone(), vec![id1.clone()]);

        let statuses = dag.get_all_statuses();
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses.get(&id1), Some(&AgentStatus::Pending));
        assert_eq!(statuses.get(&id2), Some(&AgentStatus::Pending));
    }

    #[test]
    fn test_dag_get_runnable() {
        let mut dag = AgentDag::new();
        let id1 = AgentId::new();
        let id2 = AgentId::new();

        dag.add_node(id1.clone(), Vec::new());
        dag.add_node(id2.clone(), vec![id1.clone()]);

        // id1 has no dependencies, should be runnable
        let runnable = dag.get_runnable();
        assert!(runnable.contains(&id1));
        assert!(!runnable.contains(&id2));
    }

    #[test]
    fn test_dag_topological_sort() {
        let mut dag = AgentDag::new();
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        let id3 = AgentId::new();

        // id3 depends on id2, id2 depends on id1
        dag.add_node(id1.clone(), Vec::new());
        dag.add_node(id2.clone(), vec![id1.clone()]);
        dag.add_node(id3.clone(), vec![id2.clone()]);

        let sorted = dag.topological_sort();

        // id1 should come before id2, id2 before id3
        let id1_idx = sorted.iter().position(|x| x == &id1).unwrap();
        let id2_idx = sorted.iter().position(|x| x == &id2).unwrap();
        let id3_idx = sorted.iter().position(|x| x == &id3).unwrap();

        assert!(id1_idx < id2_idx);
        assert!(id2_idx < id3_idx);
    }

    #[test]
    fn test_dag_no_cycle() {
        let mut dag = AgentDag::new();
        let id1 = AgentId::new();
        let id2 = AgentId::new();

        dag.add_node(id1.clone(), Vec::new());
        dag.add_node(id2.clone(), vec![id1.clone()]);

        assert!(!dag.has_cycle());
    }

    // ─── Result Fusion Tests ─────────────────────────────────────────────────

    #[test]
    fn test_result_fusion_single() {
        let fusion = ResultFusion::new();
        let results = vec![AgentResult {
            agent_id: AgentId::new(),
            status: AgentStatus::Completed,
            content: "test content".to_string(),
            completed_at: std::time::Instant::now(),
            tools_used: vec!["bash".to_string()],
            confidence: 0.9,
            has_conflict: false,
        }];

        let fused = fusion.fuse(results);
        assert_eq!(fused.content, "test content");
        assert_eq!(fused.confidence, 0.9);
        assert!(fused.conflicts.is_empty());
    }

    #[test]
    fn test_result_fusion_multiple() {
        let fusion = ResultFusion::new();
        let results = vec![
            AgentResult {
                agent_id: AgentId::new(),
                status: AgentStatus::Completed,
                content: "result A".to_string(),
                completed_at: std::time::Instant::now(),
                tools_used: vec![],
                confidence: 0.8,
                has_conflict: false,
            },
            AgentResult {
                agent_id: AgentId::new(),
                status: AgentStatus::Completed,
                content: "result B".to_string(),
                completed_at: std::time::Instant::now(),
                tools_used: vec![],
                confidence: 0.6,
                has_conflict: false,
            },
        ];

        let fused = fusion.fuse(results);
        // Should pick highest confidence
        assert_eq!(fused.content, "result A");
        // Average confidence
        assert!((fused.confidence - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_result_fusion_empty() {
        let fusion = ResultFusion::new();
        let fused = fusion.fuse(Vec::new());
        assert_eq!(fused.content, "");
        assert_eq!(fused.confidence, 0.0);
    }

    // ─── Auditor Tests ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_auditor_log() {
        let auditor = AgentAuditor::new();
        let agent_id = AgentId::new();

        auditor
            .log(
                agent_id.clone(),
                AgentAuditAction::Spawn,
                "Test spawn".to_string(),
            )
            .await;

        let records = auditor.get_records(Some(&agent_id)).await;
        assert_eq!(records.len(), 1);
        assert!(matches!(records[0].action, AgentAuditAction::Spawn));
    }

    #[tokio::test]
    async fn test_auditor_clear() {
        let auditor = AgentAuditor::new();
        let agent_id = AgentId::new();

        auditor
            .log(agent_id, AgentAuditAction::Spawn, "test".to_string())
            .await;
        auditor.clear().await;

        let records = auditor.get_records(None).await;
        assert!(records.is_empty());
    }
}
