//! Agent Swarm - 多 Agent 并行编排
//!
//! 类似 Claude Code 的 Agent Swarm 架构：
//! - 主 Agent 可以 spawn 多个子 Agent 并行执行
//! - 每个子 Agent 在隔离的上下文中工作
//! - 通过消息通道通信
//! - Coordinator 收集结果并汇报

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

/// Agent 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    /// 任务 ID
    pub id: String,
    /// 任务描述
    pub description: String,
    /// 系统提示词（可选，覆盖默认）
    pub system_prompt: Option<String>,
    /// 用户消息
    pub user_message: String,
    /// 最大迭代次数
    pub max_iterations: usize,
}

/// Agent 任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// 任务 ID
    pub task_id: String,
    /// Agent ID
    pub agent_id: String,
    /// 是否成功
    pub success: bool,
    /// 结果内容
    pub content: String,
    /// 错误信息（如果有）
    pub error: Option<String>,
    /// token 使用量
    pub tokens_used: Option<u64>,
}

/// Agent 状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentState {
    /// 等待中
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

/// Swarm Agent 实例
pub struct SwarmAgent {
    /// Agent ID
    pub id: String,
    /// 关联的任务
    pub task: AgentTask,
    /// 当前状态
    pub state: AgentState,
    /// 结果（完成后填充）
    pub result: Option<AgentResult>,
}

/// Swarm 协调器 - 管理多个并行 Agent
pub struct SwarmCoordinator {
    /// 所有 Agent
    agents: Arc<RwLock<HashMap<String, SwarmAgent>>>,
    /// LLM Provider
    provider: Arc<dyn crate::services::api::LlmProvider>,
    /// 模型名称
    model: String,
    /// 最大并发 Agent 数
    max_concurrent: usize,
    /// 工具注册表（子 Agent 共享）
    tool_registry: Option<Arc<crate::tools::ToolRegistry>>,
}

impl SwarmCoordinator {
    /// 创建新的协调器
    pub fn new(provider: Arc<dyn crate::services::api::LlmProvider>, model: String) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            provider,
            model,
            max_concurrent: 4,
            tool_registry: None,
        }
    }

    /// 设置工具注册表
    pub fn with_tool_registry(mut self, registry: Arc<crate::tools::ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    /// 设置最大并发数
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max.max(1);
        self
    }

    /// Spawn 一个子 Agent 执行任务
    pub async fn spawn_agent(&self, task: AgentTask) -> String {
        let agent_id = format!("agent-{}", &Uuid::new_v4().to_string()[..8]);

        let agent = SwarmAgent {
            id: agent_id.clone(),
            task: task.clone(),
            state: AgentState::Pending,
            result: None,
        };

        self.agents.write().await.insert(agent_id.clone(), agent);

        info!("Spawned agent {} for task: {}", agent_id, task.description);
        agent_id
    }

    /// Spawn 多个 Agent 并行执行
    pub async fn spawn_parallel(&self, tasks: Vec<AgentTask>) -> Vec<String> {
        let mut ids = Vec::new();
        for task in tasks {
            let id = self.spawn_agent(task).await;
            ids.push(id);
        }
        ids
    }

    /// 执行所有 Pending 状态的 Agent（并行）
    pub async fn execute_all(&self) -> Vec<AgentResult> {
        // 获取所有 pending 的 agent IDs 和 tasks
        let pending: Vec<(String, AgentTask)> = {
            let agents = self.agents.read().await;
            agents
                .values()
                .filter(|a| a.state == AgentState::Pending)
                .map(|a| (a.id.clone(), a.task.clone()))
                .collect()
        };

        if pending.is_empty() {
            return Vec::new();
        }

        info!("Executing {} agents in parallel", pending.len());

        // 用信号量控制并发
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_concurrent));
        let mut handles = Vec::new();

        for (agent_id, task) in pending {
            let provider = self.provider.clone();
            let model = self.model.clone();
            let agents = self.agents.clone();
            let sem = semaphore.clone();

            let handle = tokio::spawn(async move {
                // 获取并发许可
                let _permit = sem.acquire().await.unwrap();

                // 更新状态为 Running
                {
                    let mut map = agents.write().await;
                    if let Some(agent) = map.get_mut(&agent_id) {
                        agent.state = AgentState::Running;
                    }
                }

                info!("Agent {} started: {}", agent_id, task.description);

                // 执行任务
                let result =
                    Self::run_agent_task(&agent_id, &task, provider.as_ref(), &model).await;

                // 更新状态和结果
                {
                    let mut map = agents.write().await;
                    if let Some(agent) = map.get_mut(&agent_id) {
                        agent.state = if result.success {
                            AgentState::Completed
                        } else {
                            AgentState::Failed
                        };
                        agent.result = Some(result.clone());
                    }
                }

                info!(
                    "Agent {} {}: {}",
                    agent_id,
                    if result.success {
                        "completed"
                    } else {
                        "failed"
                    },
                    task.description
                );

                result
            });

            handles.push(handle);
        }

        // 等待所有 Agent 完成
        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => warn!("Agent task panicked: {}", e),
            }
        }

        results
    }

    /// 执行单个 Agent 任务
    async fn run_agent_task(
        agent_id: &str,
        task: &AgentTask,
        provider: &dyn crate::services::api::LlmProvider,
        model: &str,
    ) -> AgentResult {
        use crate::services::api::{ChatRequest, Message};

        let system_prompt = task.system_prompt.clone().unwrap_or_else(|| {
            format!(
                "你是子 Agent {}。你的任务是：{}\n\n\
                 请直接完成任务，给出简洁但完整的回答。不要闲聊。",
                agent_id, task.description
            )
        });

        let request = ChatRequest::new(model)
            .with_messages(vec![
                Message::system(&system_prompt),
                Message::user(&task.user_message),
            ])
            .with_temperature(0.6);

        match provider.chat(request).await {
            Ok(response) => AgentResult {
                task_id: task.id.clone(),
                agent_id: agent_id.to_string(),
                success: true,
                content: response.content,
                error: None,
                tokens_used: response.usage.as_ref().map(|u| u.total_tokens as u64),
            },
            Err(e) => AgentResult {
                task_id: task.id.clone(),
                agent_id: agent_id.to_string(),
                success: false,
                content: String::new(),
                error: Some(format!("{}", e)),
                tokens_used: None,
            },
        }
    }

    /// 获取所有 Agent 结果
    pub async fn get_results(&self) -> Vec<AgentResult> {
        let agents = self.agents.read().await;
        agents.values().filter_map(|a| a.result.clone()).collect()
    }

    /// 获取特定 Agent 的结果
    pub async fn get_result(&self, agent_id: &str) -> Option<AgentResult> {
        let agents = self.agents.read().await;
        agents.get(agent_id).and_then(|a| a.result.clone())
    }

    /// 获取所有 Agent 状态
    pub async fn get_states(&self) -> Vec<(String, AgentState)> {
        let agents = self.agents.read().await;
        agents
            .values()
            .map(|a| (a.id.clone(), a.state.clone()))
            .collect()
    }

    /// 综合所有结果为一份报告
    pub async fn synthesize_results(&self) -> String {
        let results = self.get_results().await;
        if results.is_empty() {
            return "No agent results available.".to_string();
        }

        let mut output = String::new();
        output.push_str("## Swarm Results\n\n");

        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success).count();
        let total_tokens: u64 = results.iter().filter_map(|r| r.tokens_used).sum();

        output.push_str(&format!(
            "Total: {} agents ({} succeeded, {} failed)\n",
            results.len(),
            successful,
            failed
        ));
        if total_tokens > 0 {
            output.push_str(&format!("Tokens used: {}\n", total_tokens));
        }
        output.push('\n');

        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!("### Agent {}: {}\n", i + 1, result.agent_id));
            if result.success {
                output.push_str(&result.content);
            } else {
                output.push_str(&format!(
                    "**FAILED**: {}\n",
                    result.error.as_deref().unwrap_or("unknown error")
                ));
            }
            output.push_str("\n---\n\n");
        }

        output
    }

    /// 取消所有运行中的 Agent
    pub async fn cancel_all(&self) {
        let mut agents = self.agents.write().await;
        for agent in agents.values_mut() {
            if agent.state == AgentState::Running || agent.state == AgentState::Pending {
                agent.state = AgentState::Cancelled;
            }
        }
    }

    /// 清除已完成的 Agent
    pub async fn clear_completed(&self) {
        let mut agents = self.agents.write().await;
        agents.retain(|_, a| {
            a.state != AgentState::Completed
                && a.state != AgentState::Failed
                && a.state != AgentState::Cancelled
        });
    }
}

// ── Swarm 工具接口 ──────────────────────────────────────

/// Swarm 工具 - 让 agent 创建和管理 Agent Swarm
pub struct SwarmTool;

#[async_trait::async_trait]
impl crate::tools::Tool for SwarmTool {
    fn name(&self) -> &str {
        "swarm"
    }

    fn description(&self) -> &str {
        "Spawn and manage a swarm of parallel agents. Use this to divide a complex task \
         into independent subtasks that can be executed concurrently by multiple agents. \
         Each agent runs in isolation and returns its result."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["spawn", "execute", "status", "results", "clear"],
                    "description": "spawn: create agents. execute: run pending agents. \
                                   status: check agent states. results: get all results. \
                                   clear: remove completed agents."
                },
                "tasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "description": { "type": "string" },
                            "user_message": { "type": "string" },
                            "max_iterations": { "type": "integer", "default": 5 }
                        },
                        "required": ["id", "description", "user_message"]
                    },
                    "description": "List of tasks to spawn agents for (for 'spawn' action)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        context: crate::tools::ToolContext,
    ) -> crate::tools::ToolResult {
        let action = params["action"].as_str().unwrap_or("status");

        // 获取 provider
        let provider = match &context.llm_provider {
            Some(p) => p.clone(),
            None => {
                return crate::tools::ToolResult::error(
                    "SwarmTool requires LLM provider. Set OPENAI_API_KEY or MOONSHOT_API_KEY.",
                );
            }
        };
        let model = if context.model.is_empty() {
            "kimi-k2.5".to_string()
        } else {
            context.model.clone()
        };

        // 使用 session_id 作为 coordinator key，存入全局缓存
        use std::sync::OnceLock;
        static COORDINATORS: OnceLock<
            std::sync::Arc<
                tokio::sync::RwLock<std::collections::HashMap<String, SwarmCoordinator>>,
            >,
        > = OnceLock::new();
        let coordinators = COORDINATORS.get_or_init(|| {
            std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()))
        });

        // 获取或创建此 session 的 coordinator
        let session_key = context.session_id.clone();
        {
            let mut map = coordinators.write().await;
            if !map.contains_key(&session_key) {
                map.insert(
                    session_key.clone(),
                    SwarmCoordinator::new(provider.clone(), model.clone()),
                );
            }
        }

        match action {
            "spawn" => {
                let tasks_array = params["tasks"].as_array();
                if tasks_array.is_none() || tasks_array.unwrap().is_empty() {
                    return crate::tools::ToolResult::error("Tasks required for 'spawn' action");
                }

                let mut tasks = Vec::new();
                for t in tasks_array.unwrap() {
                    tasks.push(AgentTask {
                        id: t["id"].as_str().unwrap_or("unnamed").to_string(),
                        description: t["description"].as_str().unwrap_or("").to_string(),
                        system_prompt: None,
                        user_message: t["user_message"].as_str().unwrap_or("").to_string(),
                        max_iterations: t["max_iterations"].as_u64().unwrap_or(5) as usize,
                    });
                }

                // Spawn + 立即执行
                let ids = {
                    let map = coordinators.read().await;
                    let coord = map.get(&session_key).unwrap();
                    let ids = coord.spawn_parallel(tasks).await;
                    let _results = coord.execute_all().await;
                    ids
                };

                // 获取结果
                let report = {
                    let map = coordinators.read().await;
                    let coord = map.get(&session_key).unwrap();
                    coord.synthesize_results().await
                };

                crate::tools::ToolResult::success(format!(
                    "Spawned and executed {} agents:\n\n{}",
                    ids.len(),
                    report
                ))
            }

            "status" => {
                let map = coordinators.read().await;
                let coord = map.get(&session_key).unwrap();
                let states = coord.get_states().await;
                if states.is_empty() {
                    crate::tools::ToolResult::success(
                        "No agents. Use action='spawn' to create.".to_string(),
                    )
                } else {
                    let status: Vec<String> = states
                        .iter()
                        .map(|(id, state)| format!("  {}: {:?}", id, state))
                        .collect();
                    crate::tools::ToolResult::success(format!(
                        "Agent status:\n{}",
                        status.join("\n")
                    ))
                }
            }

            "results" => {
                let map = coordinators.read().await;
                let coord = map.get(&session_key).unwrap();
                let report = coord.synthesize_results().await;
                crate::tools::ToolResult::success(report)
            }

            "clear" => {
                let map = coordinators.read().await;
                let coord = map.get(&session_key).unwrap();
                coord.clear_completed().await;
                crate::tools::ToolResult::success("Cleared completed agents.".to_string())
            }

            "execute" => crate::tools::ToolResult::success(
                "Agents auto-execute on spawn. Use action='spawn' to create and run agents."
                    .to_string(),
            ),

            _ => crate::tools::ToolResult::error(format!(
                "Unknown action: {}. Use spawn, status, results, clear",
                action
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_task_creation() {
        let task = AgentTask {
            id: "task-1".to_string(),
            description: "Analyze code".to_string(),
            system_prompt: None,
            user_message: "Review main.rs".to_string(),
            max_iterations: 5,
        };
        assert_eq!(task.id, "task-1");
        assert_eq!(task.max_iterations, 5);
    }

    #[test]
    fn test_agent_result_serialization() {
        let result = AgentResult {
            task_id: "t1".to_string(),
            agent_id: "a1".to_string(),
            success: true,
            content: "Done".to_string(),
            error: None,
            tokens_used: Some(100),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Done"));
    }

    #[test]
    fn test_agent_state_transitions() {
        assert_eq!(AgentState::Pending, AgentState::Pending);
        assert_ne!(AgentState::Pending, AgentState::Running);
    }
}
