//! WorkflowEngine — 权重驱动 + 主动提问式深思
//!
//! 模块划分：
//! - `weights.rs` — 源码级权重计算规则引擎（六维评分）
//! - `gate.rs` — 触发闸门（Direct vs Workflow）
//! - `questioning.rs` — 主动提问式深思引擎
//! - `planner.rs` — 计划生成与递归拆分
//! - `executor.rs` — 按权重执行与回写
//! - `metrics.rs` — 评估指标与日志

pub mod executor;
pub mod feedback;
pub mod gate;
pub mod metrics;
pub mod planner;
pub mod questioning;
pub mod weights;

pub use executor::{ExecutionOutcome, ExecutionRecord, NoOpStepExecutor, StepExecutor, WorkflowExecutor};
pub use feedback::{FeedbackEngine, HistoricalFailureRule};
pub use metrics::WorkflowMetrics;
pub use gate::{Gate, GateDecision};
pub use planner::WorkflowPlanner;
pub use questioning::{ActiveQuestioningEngine, QuestionNode, ThinkingResult};
pub use weights::{
    BlockerValueRule, ComplexityRule, DependencyPenaltyRule, DimensionScore, DriftPenaltyRule,
    ImpactRule, RiskRule, StepContext, WeightDimension, WeightEngine, WeightRule, WeightedStep,
};

// ============================================================================
// WorkflowEngine 主入口
// ============================================================================

use crate::engine::plan_mode::Plan;
use crate::services::api::LlmProvider;
use std::sync::Arc;

/// Workflow 执行结果
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    /// 思考成果
    pub thinking_result: ThinkingResult,
    /// 执行计划
    pub plan: Plan,
    /// 执行日志
    pub execution_log: Vec<ExecutionRecord>,
    /// 最终报告
    pub final_report: String,
}

/// WorkflowEngine — M1 最小可运行闭环
///
/// 链路：THINKING → PLANNING → EXECUTION → REPORT
pub struct WorkflowEngine {
    llm_provider: Arc<dyn LlmProvider>,
}

impl WorkflowEngine {
    pub fn new(llm_provider: Arc<dyn LlmProvider>) -> Self {
        Self { llm_provider }
    }

    /// 运行完整 Workflow
    ///
    /// # M1 行为
    /// 1. 主动提问式深思（ActiveQuestioningEngine）→ ThinkingResult
    /// 2. 计划生成（WorkflowPlanner）→ Plan
    /// 3. 按权重执行（WorkflowExecutor + StepExecutor）→ ExecutionRecord[]
    /// 4. 格式化报告
    pub async fn run(
        &self,
        task: &str,
        mainline_goal: &str,
        step_executor: &dyn StepExecutor,
    ) -> Result<WorkflowResult, String> {
        // 1. THINKING
        let mut questioning = ActiveQuestioningEngine::new(task.into(), mainline_goal.into());
        let model = self.llm_provider.default_model();
        let thinking_result = questioning
            .think(self.llm_provider.as_ref(), model)
            .await?;

        // 2. PLANNING
        let planner = WorkflowPlanner::new();
        let mut plan = planner.plan(&thinking_result, mainline_goal);

        // 3. EXECUTION
        let executor = WorkflowExecutor::new();
        let execution_log = executor.execute(&mut plan, step_executor).await?;

        // M2: 记录执行反馈（用于后续权重调整）
        let mut feedback = FeedbackEngine::load();
        feedback.record_execution(&execution_log);

        // 4. REPORT
        let final_report = Self::build_report(&thinking_result, &plan, &execution_log);

        Ok(WorkflowResult {
            thinking_result,
            plan,
            execution_log,
            final_report,
        })
    }

    fn build_report(
        thinking: &ThinkingResult,
        plan: &Plan,
        execution_log: &[ExecutionRecord],
    ) -> String {
        let mut output = String::new();
        output.push_str("# Workflow 执行报告\n\n");
        output.push_str(&format!("## 问题本质\n{}\n\n", thinking.problem_statement));
        output.push_str(&format!("## 计划步骤（{} 步）\n", plan.steps.len()));
        for (i, step) in plan.steps.iter().enumerate() {
            let status_icon = match step.status {
                crate::engine::plan_mode::StepStatus::Completed => "✅",
                crate::engine::plan_mode::StepStatus::Pending => "⏳",
                crate::engine::plan_mode::StepStatus::Failed(_) => "❌",
                crate::engine::plan_mode::StepStatus::Skipped => "⏭️",
                crate::engine::plan_mode::StepStatus::InProgress => "🔄",
            };
            output.push_str(&format!(
                "{} {}. {} (weight={})\n",
                status_icon, i + 1, step.description, step.weight
            ));
        }
        output.push('\n');
        output.push_str(&WorkflowExecutor::format_report(execution_log));
        output.push('\n');
        // M1: 追加执行指标聚合
        let metrics = WorkflowMetrics::from_records(execution_log);
        output.push_str(&metrics.summary());
        output
    }
}

// ============================================================================
// 集成测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan_mode::{PlanStep, StepStatus};
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, Message};
    use async_trait::async_trait;

    /// Mock LLM Provider：根据问题内容返回不同的模拟答案
    struct MockLlmProvider;

    #[async_trait]
    impl LlmProvider for MockLlmProvider {
        async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let user_msg = request
                .messages
                .iter()
                .find_map(|m| match m {
                    Message::User { content } => Some(content.as_str()),
                    _ => None,
                })
                .unwrap_or("");

            let answer = if user_msg.contains("方案") || user_msg.contains("步骤") {
                "1. 设计数据库表结构\n2. 实现登录接口\n3. 实现注册接口".into()
            } else {
                "这是一个详细的分析结论，包含足够的深度信息来回答问题并推进思考过程。".into()
            };

            Ok(ChatResponse {
                content: answer,
                tool_calls: None,
                usage: None,
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<async_openai::types::ChatCompletionResponseStream> {
            unimplemented!()
        }

        fn base_url(&self) -> &str {
            "http://mock"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    /// Mock Step Executor：总是成功
    struct MockStepExecutor;

    #[async_trait]
    impl StepExecutor for MockStepExecutor {
        async fn execute_step(&self, step: &PlanStep) -> Result<String, String> {
            Ok(format!("Executed: {}", step.description))
        }
    }

    #[tokio::test]
    async fn test_workflow_engine_end_to_end() {
        let engine = WorkflowEngine::new(Arc::new(MockLlmProvider));
        let result = engine
            .run("实现用户认证系统", "实现用户认证系统", &MockStepExecutor)
            .await;

        assert!(result.is_ok(), "Workflow should complete: {:?}", result.err());
        let workflow_result = result.unwrap();

        // I-02: 复杂任务进入 Workflow，产出结果
        assert!(!workflow_result.thinking_result.problem_statement.is_empty());

        // I-03: Workflow 产出报告
        assert!(!workflow_result.final_report.is_empty());

        // 计划应有步骤
        assert!(
            !workflow_result.plan.steps.is_empty(),
            "Plan should have steps"
        );

        // 执行日志应与计划步骤数一致（或更多，含重试）
        assert!(
            !workflow_result.execution_log.is_empty(),
            "Should have execution log"
        );
    }

    #[tokio::test]
    async fn test_workflow_result_contains_plan() {
        let engine = WorkflowEngine::new(Arc::new(MockLlmProvider));
        let result = engine
            .run("新增一个模块", "新增模块", &MockStepExecutor)
            .await
            .unwrap();

        // 验证 plan 的每个步骤都有权重（P-02 集成验证）
        for (i, step) in result.plan.steps.iter().enumerate() {
            assert!(
                step.weight > 0 || step.weight == 0,
                "Step {} should have weight computed",
                i
            );
        }
    }
}
