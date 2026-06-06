//! Plan Mode - 先规划再执行
//!
//! 工作流程：
//! 1. 用户提出任务
//! 2. Agent 进入 Planning 状态，生成执行计划
//! 3. TUI 展示计划，等待用户审批
//! 4. 用户批准后，Agent 按计划逐步执行
//! 5. 执行过程中可以跳过/修改步骤

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tracing::info;

/// 全局 PlanModeManager 实例
/// 用于在 ToolRegistry 和 TUI 之间共享同一个计划状态
pub static GLOBAL_PLAN_MANAGER: once_cell::sync::Lazy<Arc<PlanModeManager>> =
    once_cell::sync::Lazy::new(|| Arc::new(PlanModeManager::new()));

/// 计划模式状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanModeState {
    /// 关闭（普通模式）
    Off,
    /// 正在生成计划
    Generating,
    /// 正在向用户澄清需求（交互式提问中）
    Clarifying { question: String },
    /// 等待用户审批
    WaitingApproval,
    /// 正在执行计划
    Executing { current_step: usize },
    /// 计划完成
    Completed,
    /// 计划被拒绝
    Rejected,
}

/// 计划步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// 步骤描述
    pub description: String,
    /// 预期使用的工具
    pub tool: Option<String>,
    /// 步骤状态
    pub status: StepStatus,
    /// 权重分数（归一化到 [0, 100]）
    #[serde(default)]
    pub weight: u32,
    /// 权重解释
    #[serde(default)]
    pub weight_explanation: String,
    /// 依赖的步骤索引
    #[serde(default)]
    pub dependent_step_indices: Vec<usize>,
    /// 递归深度（0 = 顶层，L3 时达到 max_depth）
    #[serde(default)]
    pub depth: u32,
}

impl PlanStep {
    pub fn new(description: impl Into<String>, tool: Option<String>) -> Self {
        Self {
            description: description.into(),
            tool,
            status: StepStatus::Pending,
            weight: 0,
            weight_explanation: String::new(),
            dependent_step_indices: Vec::new(),
            depth: 0,
        }
    }

    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Skipped,
    Failed(String),
}

/// 执行计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// 计划标题
    pub title: String,
    /// 总体目标
    pub goal: String,
    /// 执行步骤
    pub steps: Vec<PlanStep>,
    /// 预估复杂度
    pub estimated_complexity: String,
    /// 递归深度（0 = 顶层用户任务）
    #[serde(default)]
    pub depth: u32,
    /// 最大允许递归深度（环境变量可覆盖，默认 3）
    #[serde(default)]
    pub max_depth: u32,
}

impl Plan {
    pub fn new(title: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            goal: goal.into(),
            steps: Vec::new(),
            estimated_complexity: "medium".to_string(),
            depth: 0,
            max_depth: Self::default_max_depth(),
        }
    }

    pub fn add_step(mut self, description: impl Into<String>, tool: Option<&str>) -> Self {
        self.steps
            .push(PlanStep::new(description, tool.map(String::from)));
        self
    }

    pub fn with_complexity(mut self, complexity: impl Into<String>) -> Self {
        self.estimated_complexity = complexity.into();
        self
    }

    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }

    pub fn with_max_depth(mut self, max_depth: u32) -> Self {
        self.max_depth = max_depth;
        self
    }

    pub fn default_max_depth() -> u32 {
        std::env::var("PRIORITY_AGENT_WORKFLOW_MAX_DEPTH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3)
    }

    pub fn human_review_request(&self) -> crate::engine::human_review::HumanReviewRequest {
        crate::engine::human_review::HumanReviewRequest::plan_approval(
            &self.title,
            &self.goal,
            self.steps.len(),
            &self.estimated_complexity,
        )
    }

    /// 格式化为可读文本
    pub fn format(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("Plan: {}\n", self.title));
        output.push_str(&format!("Goal: {}\n", self.goal));
        output.push_str(&format!("Complexity: {}\n", self.estimated_complexity));
        output.push_str(&format!("Steps ({}):\n", self.steps.len()));
        output.push_str("---\n");

        for (i, step) in self.steps.iter().enumerate() {
            let status_icon = match step.status {
                StepStatus::Pending => "[ ]",
                StepStatus::InProgress => "[~]",
                StepStatus::Completed => "[x]",
                StepStatus::Skipped => "[s]",
                StepStatus::Failed(_) => "[!]",
            };
            let tool_info = step
                .tool
                .as_deref()
                .map(|t| format!(" (via {})", t))
                .unwrap_or_default();
            output.push_str(&format!(
                "  {} {}. {}{}\n",
                status_icon,
                i + 1,
                step.description,
                tool_info
            ));
        }

        output.push_str("---\n");
        output.push_str("Approve this plan? (y/n/modify)\n");
        output
    }

    /// 获取待执行的下一步
    pub fn next_pending_step(&self) -> Option<(usize, &PlanStep)> {
        self.steps
            .iter()
            .enumerate()
            .find(|(_, s)| s.status == StepStatus::Pending)
    }

    /// 是否所有步骤都完成
    pub fn is_complete(&self) -> bool {
        self.steps
            .iter()
            .all(|s| matches!(s.status, StepStatus::Completed | StepStatus::Skipped))
    }

    /// 进度统计
    pub fn progress(&self) -> (usize, usize, usize) {
        let completed = self
            .steps
            .iter()
            .filter(|s| s.status == StepStatus::Completed)
            .count();
        let skipped = self
            .steps
            .iter()
            .filter(|s| s.status == StepStatus::Skipped)
            .count();
        let total = self.steps.len();
        (completed + skipped, total, completed)
    }
}

/// 计划审批通道
pub struct PlanApprovalChannel {
    pending_plan: Arc<Mutex<Option<PendingPlan>>>,
}

struct PendingPlan {
    plan: Plan,
    response_tx: oneshot::Sender<PlanApproval>,
}

#[derive(Debug, Clone)]
pub enum PlanApproval {
    Approved,
    Rejected,
    Modified(String),
}

impl PlanApprovalChannel {
    pub fn new() -> Self {
        Self {
            pending_plan: Arc::new(Mutex::new(None)),
        }
    }

    /// 提交计划等待审批（60 秒超时）
    pub async fn submit_plan(&self, plan: Plan) -> Result<PlanApproval, String> {
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_plan.lock().await;
            *pending = Some(PendingPlan {
                plan,
                response_tx: tx,
            });
        }
        match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
            Ok(result) => result.map_err(|_| "Approval channel closed".to_string()),
            Err(_) => Err("Plan approval timed out after 60 seconds".to_string()),
        }
    }

    /// TUI 取出待审批的计划
    pub async fn take_pending(&self) -> Option<(Plan, oneshot::Sender<PlanApproval>)> {
        let mut pending = self.pending_plan.lock().await;
        pending.take().map(|p| (p.plan, p.response_tx))
    }

    /// 是否有待审批的计划
    pub async fn has_pending(&self) -> bool {
        self.pending_plan.lock().await.is_some()
    }
}

impl Default for PlanApprovalChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Plan Mode 管理器
pub struct PlanModeManager {
    state: Arc<Mutex<PlanModeState>>,
    current_plan: Arc<Mutex<Option<Plan>>>,
    approval_channel: Arc<PlanApprovalChannel>,
}

impl PlanModeManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PlanModeState::Off)),
            current_plan: Arc::new(Mutex::new(None)),
            approval_channel: Arc::new(PlanApprovalChannel::new()),
        }
    }

    /// 获取审批通道
    pub fn approval_channel(&self) -> Arc<PlanApprovalChannel> {
        self.approval_channel.clone()
    }

    /// 获取当前状态
    pub async fn get_state(&self) -> PlanModeState {
        self.state.lock().await.clone()
    }

    /// 获取当前计划
    pub async fn get_plan(&self) -> Option<Plan> {
        self.current_plan.lock().await.clone()
    }

    /// 进入计划模式
    pub async fn enter_plan_mode(&self) {
        let mut state = self.state.lock().await;
        *state = PlanModeState::Generating;
        info!("Entered plan mode");
    }

    /// 提交计划
    pub async fn submit_plan(&self, plan: Plan) -> Result<PlanApproval, String> {
        {
            let mut state = self.state.lock().await;
            *state = PlanModeState::WaitingApproval;
        }
        {
            let mut current = self.current_plan.lock().await;
            *current = Some(plan.clone());
        }

        info!("Plan submitted, waiting for approval: {}", plan.title);
        self.approval_channel.submit_plan(plan).await
    }

    /// 开始执行
    pub async fn start_execution(&self) {
        let mut state = self.state.lock().await;
        *state = PlanModeState::Executing { current_step: 0 };
        info!("Plan execution started");
    }

    /// 标记步骤完成
    pub async fn complete_step(&self, step_index: usize) {
        let mut plan = self.current_plan.lock().await;
        if let Some(ref mut p) = *plan {
            if step_index < p.steps.len() {
                p.steps[step_index].status = StepStatus::Completed;
            }
        }

        // 检查是否全部完成
        if let Some(ref p) = *plan {
            if p.is_complete() {
                let mut state = self.state.lock().await;
                *state = PlanModeState::Completed;
                info!("Plan completed");
            } else {
                let mut state = self.state.lock().await;
                *state = PlanModeState::Executing {
                    current_step: step_index + 1,
                };
            }
        }
    }

    /// 标记步骤失败
    pub async fn fail_step(&self, step_index: usize, error: String) {
        let mut plan = self.current_plan.lock().await;
        if let Some(ref mut p) = *plan {
            if step_index < p.steps.len() {
                p.steps[step_index].status = StepStatus::Failed(error);
            }
        }
    }

    /// 跳过步骤
    pub async fn skip_step(&self, step_index: usize) {
        let mut plan = self.current_plan.lock().await;
        if let Some(ref mut p) = *plan {
            if step_index < p.steps.len() {
                p.steps[step_index].status = StepStatus::Skipped;
            }
        }
    }

    /// 退出计划模式
    pub async fn exit(&self, rejected: bool) {
        let mut state = self.state.lock().await;
        *state = if rejected {
            PlanModeState::Rejected
        } else {
            PlanModeState::Off
        };
        let mut plan = self.current_plan.lock().await;
        *plan = None;
        info!("Exited plan mode (rejected={})", rejected);
    }

    /// 开始澄清提问
    pub async fn start_clarifying(&self, question: &str) {
        let mut state = self.state.lock().await;
        *state = PlanModeState::Clarifying {
            question: question.to_string(),
        };
        info!("Started clarifying question: {}", question);
    }

    /// 完成澄清提问，回到生成计划状态
    pub async fn finish_clarifying(&self) {
        let mut state = self.state.lock().await;
        *state = PlanModeState::Generating;
        info!("Finished clarifying, back to generating");
    }
}

impl Default for PlanModeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Plan 工具 - Agent 用来提交执行计划
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;

pub struct PlanTool {
    manager: Arc<PlanModeManager>,
}

impl PlanTool {
    pub fn new(manager: Arc<PlanModeManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for PlanTool {
    fn name(&self) -> &str {
        "plan"
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> crate::tools::ToolOperationKind {
        crate::tools::ToolOperationKind::Task
    }

    fn description(&self) -> &str {
        concat!(
            "Create an execution plan for the current task. Use this when the task is complex and needs step-by-step planning before execution. The plan will be shown to the user for approval.\n\n",
            "IMPORTANT: Before submitting a plan, ensure you have clarified all ambiguous requirements. ",
            "If you are unsure about any aspect (e.g., auth method, UI framework, API choice, file naming convention), ",
            "use the `ask_user` tool FIRST to ask the user a clarifying question. ",
            "Only submit the plan after the user's response resolves the ambiguity."
        )
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short title for the plan"
                },
                "goal": {
                    "type": "string",
                    "description": "Overall goal of the plan"
                },
                "steps": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "description": { "type": "string" },
                            "tool": { "type": "string" }
                        },
                        "required": ["description"]
                    },
                    "description": "List of steps to execute"
                },
                "complexity": {
                    "type": "string",
                    "enum": ["low", "medium", "high"],
                    "default": "medium"
                }
            },
            "required": ["title", "goal", "steps"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let title = params["title"].as_str().unwrap_or("Untitled Plan");
        let goal = params["goal"].as_str().unwrap_or("");
        let complexity = params["complexity"].as_str().unwrap_or("medium");

        if goal.is_empty() {
            return ToolResult::error("Goal cannot be empty");
        }

        let mut plan = Plan::new(title, goal).with_complexity(complexity);

        if let Some(steps) = params["steps"].as_array() {
            for step in steps {
                let desc = step["description"].as_str().unwrap_or("");
                let tool = step["tool"].as_str();
                if !desc.is_empty() {
                    plan = plan.add_step(desc, tool);
                }
            }
        }

        if plan.steps.is_empty() {
            return ToolResult::error("Plan must have at least one step");
        }

        // 进入计划模式并提交计划
        self.manager.enter_plan_mode().await;

        match self.manager.submit_plan(plan.clone()).await {
            Ok(PlanApproval::Approved) => {
                self.manager.start_execution().await;
                ToolResult::success_with_data(
                    format!("Plan approved! {} steps to execute.", plan.steps.len()),
                    serde_json::to_value(&plan).unwrap_or(serde_json::Value::Null),
                )
            }
            Ok(PlanApproval::Rejected) => {
                self.manager.exit(true).await;
                ToolResult::error("Plan was rejected by the user")
            }
            Ok(PlanApproval::Modified(feedback)) => {
                self.manager.exit(true).await;
                ToolResult::error(format!("Plan needs modification: {}", feedback))
            }
            Err(e) => {
                self.manager.exit(true).await;
                ToolResult::error(format!("Failed to get plan approval: {}", e))
            }
        }
    }

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_format() {
        let plan = Plan::new("Fix auth bug", "Resolve login failures")
            .add_step("Read auth.rs", Some("file_read"))
            .add_step("Fix the bug", Some("file_edit"))
            .add_step("Run tests", Some("bash"));

        let formatted = plan.format();
        assert!(formatted.contains("Fix auth bug"));
        assert!(formatted.contains("[ ] 1."));
        assert!(formatted.contains("(via file_read)"));
    }

    #[test]
    fn test_plan_progress() {
        let mut plan = Plan::new("Test", "Test goal")
            .add_step("Step 1", None)
            .add_step("Step 2", None);

        assert_eq!(plan.progress(), (0, 2, 0));
        assert!(!plan.is_complete());

        plan.steps[0].status = StepStatus::Completed;
        assert_eq!(plan.progress(), (1, 2, 1));
        assert!(!plan.is_complete());

        plan.steps[1].status = StepStatus::Completed;
        assert_eq!(plan.progress(), (2, 2, 2));
        assert!(plan.is_complete());
    }

    #[test]
    fn test_next_pending() {
        let mut plan = Plan::new("Test", "Goal")
            .add_step("A", None)
            .add_step("B", None);

        let (idx, step) = plan.next_pending_step().unwrap();
        assert_eq!(idx, 0);
        assert_eq!(step.description, "A");

        plan.steps[0].status = StepStatus::Completed;
        let (idx, step) = plan.next_pending_step().unwrap();
        assert_eq!(idx, 1);
        assert_eq!(step.description, "B");
    }

    #[tokio::test]
    async fn test_plan_approval_channel() {
        let channel = PlanApprovalChannel::new();
        let plan = Plan::new("Test", "Goal").add_step("Step 1", None);

        assert!(!channel.has_pending().await);

        // 使用 Arc 来共享
        let ch = std::sync::Arc::new(channel);
        let p = plan.clone();
        let ch2 = ch.clone();
        let handle = tokio::spawn(async move { ch2.submit_plan(p).await });

        // 取出待审批的计划
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(ch.has_pending().await);

        let (pending_plan, tx) = ch.take_pending().await.unwrap();
        assert_eq!(pending_plan.title, "Test");

        // 批准
        tx.send(PlanApproval::Approved).unwrap();

        let result = handle.await.unwrap();
        assert!(matches!(result, Ok(PlanApproval::Approved)));
    }
}
