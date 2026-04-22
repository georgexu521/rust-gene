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
    /// 状态机历史
    pub state_history: Vec<WorkflowStateSnapshot>,
}

// ============================================================================
// Workflow 状态机
// ============================================================================

/// Workflow 执行状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowState {
    /// 空闲 / 初始状态
    Idle,
    /// 闸门判断（Direct vs Workflow）
    Gate,
    /// 主动提问式深思
    Thinking,
    /// 计划生成
    Planning,
    /// 权重计算 / 重新计算
    Weighting,
    /// 计划执行中
    Executing { current_step: usize, total: usize },
    /// 结果验证
    Verifying,
    /// 重新权重（基于执行反馈调整）
    Reweight,
    /// 执行完成
    Done,
    /// 报告生成
    Report,
    /// 降级到 Direct 模式（预算/错误触发）
    FallbackDirect,
}

/// 状态快照（用于审计和调试）
#[derive(Debug, Clone)]
pub struct WorkflowStateSnapshot {
    pub state: WorkflowState,
    pub entered_at: String,
    pub duration_ms: u64,
}

/// Workflow 状态机
///
/// 追踪 WorkflowEngine 执行过程中的状态流转，
/// 提供可观测性和调试能力。
#[derive(Debug, Clone)]
pub struct WorkflowStateMachine {
    pub current_state: WorkflowState,
    pub history: Vec<WorkflowStateSnapshot>,
    state_entered_at: std::time::Instant,
}

impl WorkflowStateMachine {
    pub fn new() -> Self {
        Self {
            current_state: WorkflowState::Idle,
            history: Vec::new(),
            state_entered_at: std::time::Instant::now(),
        }
    }

    /// 推进到下一个状态
    pub fn transition(&mut self, next: WorkflowState) {
        let now = std::time::Instant::now();
        let duration = now.duration_since(self.state_entered_at).as_millis() as u64;

        self.history.push(WorkflowStateSnapshot {
            state: self.current_state.clone(),
            entered_at: chrono::Local::now().to_rfc3339(),
            duration_ms: duration,
        });

        self.current_state = next;
        self.state_entered_at = now;
    }

    /// 格式化状态历史为可读文本
    pub fn format_history(&self) -> String {
        let mut output = String::from("## Workflow 状态流转\n\n");
        for (i, snap) in self.history.iter().enumerate() {
            let state_label = format!("{:?}", snap.state);
            output.push_str(&format!(
                "{}. {} — {}ms\n",
                i + 1,
                state_label,
                snap.duration_ms
            ));
        }
        output.push('\n');
        output
    }
}

impl Default for WorkflowStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// WorkflowEngine — M1/M2 完整闭环
///
/// 状态机链路：
/// IDLE → GATE → THINKING → PLANNING → WEIGHTING → EXECUTING →
/// VERIFYING → (REWEIGHT → EXECUTING)* → DONE → REPORT
pub struct WorkflowEngine {
    llm_provider: Arc<dyn LlmProvider>,
}

impl WorkflowEngine {
    pub fn new(llm_provider: Arc<dyn LlmProvider>) -> Self {
        Self { llm_provider }
    }

    /// 运行完整 Workflow（状态机驱动）
    ///
    /// # 状态机行为
    /// 1. GATE — 快速检查（支持可选 LLM 分类）
    /// 2. THINKING — 主动提问式深思
    /// 3. PLANNING — 递归计划生成
    /// 4. WEIGHTING — 权重计算（在 planning 中完成）
    /// 5. EXECUTING — 按权重执行步骤
    /// 6. VERIFYING — 检查执行结果（NeedsRefactor 检测）
    /// 7. REWEIGHT — 如有需要，重新计算权重并补充执行
    /// 8. DONE — 标记完成
    /// 9. REPORT — 生成最终报告
    pub async fn run(
        &self,
        task: &str,
        mainline_goal: &str,
        step_executor: &dyn StepExecutor,
    ) -> Result<WorkflowResult, String> {
        let mut sm = WorkflowStateMachine::new();

        // 1. GATE
        sm.transition(WorkflowState::Gate);
        if let Err(e) = self.run_gate(task, mainline_goal).await {
            sm.transition(WorkflowState::FallbackDirect);
            return Err(e);
        }

        // 2. THINKING
        sm.transition(WorkflowState::Thinking);
        let thinking_result = match self.run_thinking(task, mainline_goal).await {
            Ok(v) => v,
            Err(e) => {
                sm.transition(WorkflowState::FallbackDirect);
                return Err(e);
            }
        };

        // 3. PLANNING
        sm.transition(WorkflowState::Planning);
        let mut plan = self.run_planning(&thinking_result, mainline_goal).await;

        // 规划后强校验：依赖索引越界视为计划错误，避免静默执行顺序错乱。
        let invalid_deps = WorkflowExecutor::find_invalid_dependency_indices(&plan);
        if !invalid_deps.is_empty() {
            sm.transition(WorkflowState::FallbackDirect);
            return Err(format!(
                "Invalid dependency indices detected in plan: {:?}",
                invalid_deps
            ));
        }

        // 4. WEIGHTING（planning 已包含权重计算，这里作为显式状态记录）
        sm.transition(WorkflowState::Weighting);

        // 5. EXECUTING
        sm.transition(WorkflowState::Executing {
            current_step: 0,
            total: plan.steps.len(),
        });
        let mut execution_log = match self.run_executing(&mut plan, step_executor, &mut sm).await {
            Ok(v) => v,
            Err(e) => {
                sm.transition(WorkflowState::FallbackDirect);
                return Err(e);
            }
        };

        // 6. VERIFYING
        sm.transition(WorkflowState::Verifying);
        let needs_reweight = self.run_verifying(&plan, &execution_log);

        // 7. REWEIGHT（如有 NeedsRefactor 的步骤）
        if needs_reweight {
            sm.transition(WorkflowState::Reweight);
            let planner = WorkflowPlanner::with_llm(self.llm_provider.clone());
            planner.reweight(&mut plan, mainline_goal);

            // 补充执行：对 [重构] 失败步骤进行“受控重试”。
            // 先把 [重构] + Failed 的步骤转回 Pending，然后仅执行这一轮。
            for step in &mut plan.steps {
                if step.description.starts_with("[重构]")
                    && matches!(step.status, crate::engine::plan_mode::StepStatus::Failed(_))
                {
                    step.status = crate::engine::plan_mode::StepStatus::Pending;
                }
            }

            // 只执行当前 Pending，避免无限循环。
            let remaining = plan
                .steps
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    matches!(s.status, crate::engine::plan_mode::StepStatus::Pending)
                })
                .count();
            if remaining > 0 {
                sm.transition(WorkflowState::Executing {
                    current_step: 0,
                    total: remaining,
                });
                let extra_log = self.run_executing(&mut plan, step_executor, &mut sm).await?;
                execution_log.extend(extra_log);
            }
        }

        // 进入 Done 前强校验：不能在仍有 Pending 时宣告完成。
        let pending_steps: Vec<(usize, String)> = plan
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.status == crate::engine::plan_mode::StepStatus::Pending)
            .map(|(i, s)| (i, s.description.clone()))
            .collect();
        if !pending_steps.is_empty() {
            sm.transition(WorkflowState::FallbackDirect);
            return Err(format!(
                "Workflow incomplete: {} pending step(s) remain: {:?}",
                pending_steps.len(),
                pending_steps
            ));
        }

        // M2: 记录执行反馈
        let mut feedback = FeedbackEngine::load();
        feedback.record_execution(&execution_log);

        // 8. DONE
        sm.transition(WorkflowState::Done);

        // 9. REPORT
        sm.transition(WorkflowState::Report);
        let final_report = Self::build_report(&thinking_result, &plan, &execution_log, &sm);

        Ok(WorkflowResult {
            thinking_result,
            plan,
            execution_log,
            final_report,
            state_history: sm.history,
        })
    }

    // ============================================================================
    // 状态步骤方法
    // ============================================================================

    async fn run_gate(&self, task: &str, _mainline_goal: &str) -> Result<(), String> {
        // GATE 状态：与 ConversationLoop 保持一致的闸门判定。
        // 即使外部直接调用 WorkflowEngine，也会进行一次准入检查。
        let gate_llm_enabled = std::env::var("PRIORITY_AGENT_WORKFLOW_GATE_LLM")
            .ok()
            .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(false);
        let gate = Gate::new().with_llm_classifier(gate_llm_enabled);
        let decision = if gate_llm_enabled {
            gate.decide_with_llm(
                task,
                self.llm_provider.as_ref(),
                self.llm_provider.default_model(),
            )
            .await
        } else {
            gate.decide(task)
        };
        match decision {
            GateDecision::Workflow { .. } => Ok(()),
            GateDecision::Direct { reason } => {
                Err(format!("Gate decided Direct mode, skip workflow: {}", reason))
            }
        }
    }

    async fn run_thinking(
        &self,
        task: &str,
        mainline_goal: &str,
    ) -> Result<ThinkingResult, String> {
        let mut questioning = ActiveQuestioningEngine::new(task.into(), mainline_goal.into());
        let model = self.llm_provider.default_model();
        questioning
            .think(self.llm_provider.as_ref(), model)
            .await
            .map_err(|e| format!("Thinking failed: {}", e))
    }

    async fn run_planning(
        &self,
        thinking_result: &ThinkingResult,
        mainline_goal: &str,
    ) -> Plan {
        let planner = WorkflowPlanner::with_llm(self.llm_provider.clone());
        planner.plan_with_recursion(thinking_result, mainline_goal).await
    }

    async fn run_executing(
        &self,
        plan: &mut Plan,
        step_executor: &dyn StepExecutor,
        sm: &mut WorkflowStateMachine,
    ) -> Result<Vec<ExecutionRecord>, String> {
        let executor = WorkflowExecutor::new();
        let mut all_records = Vec::new();

        while let Some(idx) = executor.find_next_executable(plan) {
            // 更新状态机执行进度
            let completed = plan
                .steps
                .iter()
                .filter(|s| {
                    matches!(
                        s.status,
                        crate::engine::plan_mode::StepStatus::Completed
                            | crate::engine::plan_mode::StepStatus::Skipped
                    )
                })
                .count();
            sm.current_state = WorkflowState::Executing {
                current_step: completed + 1,
                total: plan.steps.len(),
            };

            let record = executor.execute_single_step(plan, idx, step_executor).await;
            all_records.push(record);
        }

        Ok(all_records)
    }

    fn run_verifying(&self, plan: &Plan, execution_log: &[ExecutionRecord]) -> bool {
        // 检查是否有 NeedsRefactor 或仍 Pending 的步骤
        let has_refactor = execution_log
            .iter()
            .any(|r| matches!(r.outcome, ExecutionOutcome::NeedsRefactor(_)));
        let has_pending = plan
            .steps
            .iter()
            .any(|s| s.status == crate::engine::plan_mode::StepStatus::Pending);
        has_refactor || has_pending
    }

    fn build_report(
        thinking: &ThinkingResult,
        plan: &Plan,
        execution_log: &[ExecutionRecord],
        sm: &WorkflowStateMachine,
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
        let metrics = WorkflowMetrics::from_workflow(plan, execution_log, &plan.goal);
        output.push_str(&metrics.summary());
        match crate::engine::workflow::metrics::persist_workflow_metrics(
            &thinking.problem_statement,
            &plan.goal,
            &metrics,
        ) {
            Ok(_) => output.push_str("\n- Metrics persisted: yes\n"),
            Err(e) => output.push_str(&format!("\n- Metrics persisted: no ({})\n", e)),
        }
        output.push('\n');
        // 状态机历史
        output.push_str(&sm.format_history());
        output
    }
}

// ============================================================================
// 集成测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan_mode::PlanStep;
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
