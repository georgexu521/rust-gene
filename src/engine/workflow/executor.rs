//! Executor — 按权重顺序执行计划步骤
//!
//! M1 范围：
//! - 串行执行（非并行）
//! - 依赖检查：未满足依赖的步骤先等待
//! - 失败重试：第 1 次失败重试，第 2 次失败标记 [重构]
//! - 状态更新：执行后更新 PlanStep.status
//! - 执行报告：产出 ExecutionRecord 列表

use crate::engine::plan_mode::{Plan, PlanStep, StepStatus};
use async_trait::async_trait;
use std::time::Instant;

/// 执行结果类型
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionOutcome {
    Success(String),
    Failed(String),
    Skipped(String),
    NeedsRefactor(String),
}

/// 单步执行记录
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub step_index: usize,
    pub description: String,
    pub tool: Option<String>,
    pub outcome: ExecutionOutcome,
    pub duration_ms: u64,
    pub retry_count: usize,
}

/// 步骤执行器 trait（便于 mock 测试）
#[async_trait]
pub trait StepExecutor: Send + Sync {
    /// 执行单个步骤，返回成功结果或错误原因
    async fn execute_step(&self, step: &PlanStep) -> Result<String, String>;
}

/// Workflow 执行器
pub struct WorkflowExecutor;

impl WorkflowExecutor {
    pub fn new() -> Self {
        Self
    }

    /// 执行整个计划
    ///
    /// # 执行策略
    /// 1. 每次从所有 Pending 步骤中，选择**依赖已满足**且**权重最高**的步骤
    /// 2. 执行该步骤（含重试逻辑）
    /// 3. 更新步骤状态
    /// 4. 重复直到无步骤可执行
    ///
    /// # M1 限制
    /// - 串行执行（非并行）
    /// - 不实现微思考（M2）
    /// - 不实现验证策略（M2）
    pub async fn execute(
        &self,
        plan: &mut Plan,
        step_executor: &dyn StepExecutor,
    ) -> Result<Vec<ExecutionRecord>, String> {
        let mut records = Vec::new();

        loop {
            match self.find_next_executable(plan) {
                Some(idx) => {
                    let record = self
                        .execute_single_step(plan, idx, step_executor)
                        .await;
                    records.push(record);
                }
                None => break,
            }
        }

        Ok(records)
    }

    // ============================================================================
    // 内部方法
    // ============================================================================

    /// 找到下一个可执行的步骤
    ///
    /// 可执行条件：
    /// - status == Pending
    /// - 所有依赖步骤都已完成（Completed 或 Skipped）
    ///
    /// 在可执行步骤中选择权重最高的（E-01）。
    fn find_next_executable(&self, plan: &Plan) -> Option<usize> {
        plan.steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.status == StepStatus::Pending)
            .filter(|(_, s)| {
                s.dependent_step_indices.iter().all(|dep_idx| {
                    if let Some(dep_step) = plan.steps.get(*dep_idx) {
                        dep_step.status == StepStatus::Completed
                            || dep_step.status == StepStatus::Skipped
                    } else {
                        true // 依赖索引越界视为已满足（防御性）
                    }
                })
            })
            .max_by_key(|(_, s)| s.weight)
            .map(|(idx, _)| idx)
    }

    /// 执行单个步骤（含重试逻辑）
    ///
    /// # 重试策略（M1）
    /// - 首次执行失败 → 重试 1 次（E-04）
    /// - 重试仍失败 → 标记 [重构]（E-05），返回 NeedsRefactor
    async fn execute_single_step(
        &self,
        plan: &mut Plan,
        step_index: usize,
        step_executor: &dyn StepExecutor,
    ) -> ExecutionRecord {
        let start = Instant::now();
        let mut retry_count = 0;

        // 首次执行
        let first_result = step_executor
            .execute_step(&plan.steps[step_index])
            .await;

        let (outcome, final_status) = match first_result {
            Ok(output) => {
                let duration = start.elapsed().as_millis() as u64;
                plan.steps[step_index].status = StepStatus::Completed;
                (
                    ExecutionOutcome::Success(output),
                    StepStatus::Completed,
                )
            }
            Err(err1) => {
                // 第 1 次失败 → 重试（E-04）
                retry_count = 1;
                let retry_result = step_executor
                    .execute_step(&plan.steps[step_index])
                    .await;

                match retry_result {
                    Ok(output) => {
                        let duration = start.elapsed().as_millis() as u64;
                        plan.steps[step_index].status = StepStatus::Completed;
                        (
                            ExecutionOutcome::Success(output),
                            StepStatus::Completed,
                        )
                    }
                    Err(err2) => {
                        // 第 2 次失败 → 标记 [重构]（E-05）
                        let duration = start.elapsed().as_millis() as u64;
                        let step = &mut plan.steps[step_index];
                        if !step.description.starts_with("[重构]") {
                            step.description = format!("[重构] {}", step.description);
                        }
                        // M1 中标记为 Pending 并附带 NeedsRefactor 结果
                        // 不设置为 Failed，以便上层 WorkflowEngine 可决定重新 plan
                        (
                            ExecutionOutcome::NeedsRefactor(format!(
                                "首次: {}; 重试: {}",
                                err1, err2
                            )),
                            StepStatus::Pending,
                        )
                    }
                }
            }
        };

        // 应用最终状态（如果上面分支没设置的话）
        if plan.steps[step_index].status != final_status {
            plan.steps[step_index].status = final_status;
        }

        let duration = start.elapsed().as_millis() as u64;
        let step = &plan.steps[step_index];

        ExecutionRecord {
            step_index,
            description: step.description.clone(),
            tool: step.tool.clone(),
            outcome,
            duration_ms: duration,
            retry_count,
        }
    }

    /// 执行报告格式化
    pub fn format_report(records: &[ExecutionRecord]) -> String {
        let mut output = String::new();
        output.push_str("## 执行报告\n\n");

        let success_count = records
            .iter()
            .filter(|r| matches!(r.outcome, ExecutionOutcome::Success(_)))
            .count();
        let refactor_count = records
            .iter()
            .filter(|r| matches!(r.outcome, ExecutionOutcome::NeedsRefactor(_)))
            .count();
        let total_duration: u64 = records.iter().map(|r| r.duration_ms).sum();

        output.push_str(&format!(
            "总步骤: {} | 成功: {} | 需重构: {} | 总耗时: {}ms\n\n",
            records.len(),
            success_count,
            refactor_count,
            total_duration
        ));

        for (i, record) in records.iter().enumerate() {
            let icon = match &record.outcome {
                ExecutionOutcome::Success(_) => "✅",
                ExecutionOutcome::Failed(_) => "❌",
                ExecutionOutcome::Skipped(_) => "⏭️",
                ExecutionOutcome::NeedsRefactor(_) => "🔄",
            };
            output.push_str(&format!(
                "{} Step {}: {} ({}ms, {} retries)\n",
                icon,
                record.step_index,
                record.description,
                record.duration_ms,
                record.retry_count
            ));
        }

        output
    }
}

impl Default for WorkflowExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// 空操作步骤执行器 — M1 集成占位
///
/// 不执行实际操作，仅返回模拟成功结果。
/// 用于 ConversationLoop 集成阶段，避免在 M1 中引入完整的工具参数解析。
pub struct NoOpStepExecutor;

#[async_trait]
impl StepExecutor for NoOpStepExecutor {
    async fn execute_step(&self, step: &PlanStep) -> Result<String, String> {
        Ok(format!("[NoOp] 模拟执行: {}", step.description))
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan_mode::{Plan, PlanStep, StepStatus};

    /// 基于计数器的 Mock 执行器
    struct MockStepExecutor {
        results: std::sync::Mutex<Vec<Result<String, String>>>,
    }

    impl MockStepExecutor {
        fn new(results: Vec<Result<String, String>>) -> Self {
            Self {
                results: std::sync::Mutex::new(results),
            }
        }
    }

    #[async_trait]
    impl StepExecutor for MockStepExecutor {
        async fn execute_step(&self, _step: &PlanStep) -> Result<String, String> {
            let mut results = self.results.lock().unwrap();
            if results.is_empty() {
                Ok("default".into())
            } else {
                results.remove(0)
            }
        }
    }

    fn make_plan_with_weights(steps: Vec<(String, u32, Vec<usize>)>) -> Plan {
        let plan_steps = steps
            .into_iter()
            .enumerate()
            .map(|(i, (desc, weight, deps))| PlanStep {
                description: desc,
                tool: None,
                status: StepStatus::Pending,
                weight,
                weight_explanation: format!("weight={}", weight),
                dependent_step_indices: deps,
            })
            .collect();

        Plan {
            title: "Test Plan".into(),
            goal: "Test".into(),
            steps: plan_steps,
            estimated_complexity: "low".into(),
        }
    }

    #[tokio::test]
    async fn test_execute_by_weight_order() {
        // E-01: 按权重排序执行
        let executor = WorkflowExecutor::new();
        let mut plan = make_plan_with_weights(vec![
            ("低权重步骤".into(), 10, vec![]),
            ("高权重步骤".into(), 90, vec![]),
            ("中权重步骤".into(), 50, vec![]),
        ]);

        let mock = MockStepExecutor::new(vec![
            Ok("ok1".into()),
            Ok("ok2".into()),
            Ok("ok3".into()),
        ]);

        let records = executor.execute(&mut plan, &mock).await.unwrap();

        // 执行顺序应该是：高权重(90) → 中权重(50) → 低权重(10)
        assert_eq!(records[0].step_index, 1, "Highest weight should execute first");
        assert_eq!(records[1].step_index, 2);
        assert_eq!(records[2].step_index, 0);
    }

    #[tokio::test]
    async fn test_dependency_executed_first() {
        // E-02: 依赖未满足时，依赖步骤先执行
        let executor = WorkflowExecutor::new();
        let mut plan = make_plan_with_weights(vec![
            ("实现功能 A".into(), 90, vec![]),
            ("测试功能 A".into(), 80, vec![0]), // 依赖步骤 0
        ]);

        let mock = MockStepExecutor::new(vec![Ok("impl_ok".into()), Ok("test_ok".into())]);

        let records = executor.execute(&mut plan, &mock).await.unwrap();

        // 步骤 1 权重更高（80 vs 无），但它依赖步骤 0
        // 所以步骤 0 必须先执行
        assert_eq!(records[0].step_index, 0, "Dependency should execute first");
        assert_eq!(records[1].step_index, 1);

        // 状态验证（E-03）
        assert_eq!(plan.steps[0].status, StepStatus::Completed);
        assert_eq!(plan.steps[1].status, StepStatus::Completed);
    }

    #[tokio::test]
    async fn test_retry_on_first_failure() {
        // E-04: 失败 1 次后重试，成功后状态 Completed
        let executor = WorkflowExecutor::new();
        let mut plan = make_plan_with_weights(vec![
            ("可能失败的步骤".into(), 50, vec![]),
        ]);

        let mock = MockStepExecutor::new(vec![
            Err("第一次失败".into()),
            Ok("重试成功".into()),
        ]);

        let records = executor.execute(&mut plan, &mock).await.unwrap();

        assert_eq!(records[0].retry_count, 1, "Should have retried once");
        assert!(
            matches!(records[0].outcome, ExecutionOutcome::Success(_)),
            "Should succeed after retry"
        );
        assert_eq!(plan.steps[0].status, StepStatus::Completed);
    }

    #[tokio::test]
    async fn test_refactor_on_second_failure() {
        // E-05: 失败 2 次后标记 [重构]
        let executor = WorkflowExecutor::new();
        let mut plan = make_plan_with_weights(vec![
            ("连续失败的步骤".into(), 50, vec![]),
        ]);

        let mock = MockStepExecutor::new(vec![
            Err("第一次失败".into()),
            Err("第二次失败".into()),
        ]);

        let records = executor.execute(&mut plan, &mock).await.unwrap();

        assert_eq!(records[0].retry_count, 1);
        assert!(
            matches!(records[0].outcome, ExecutionOutcome::NeedsRefactor(_)),
            "Should mark as NeedsRefactor after 2 failures"
        );
        assert!(
            plan.steps[0].description.starts_with("[重构]"),
            "Step description should be prefixed with [重构]"
        );
    }

    #[tokio::test]
    async fn test_skip_non_executable() {
        // 混合场景：部分已完成，部分有未满足依赖
        let executor = WorkflowExecutor::new();
        let mut plan = make_plan_with_weights(vec![
            ("步骤 0".into(), 100, vec![]),
            ("步骤 1".into(), 90, vec![0]),
            ("步骤 2".into(), 80, vec![1]), // 依赖步骤 1
        ]);

        // 预设步骤 0 已完成
        plan.steps[0].status = StepStatus::Completed;

        let mock = MockStepExecutor::new(vec![Ok("ok1".into()), Ok("ok2".into())]);

        let records = executor.execute(&mut plan, &mock).await.unwrap();

        // 步骤 0 已完成，步骤 1 可执行（依赖 0 已完成），步骤 2 依赖 1 尚未完成
        assert_eq!(records[0].step_index, 1);
        assert_eq!(records[1].step_index, 2);
        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn test_format_report() {
        let records = vec![
            ExecutionRecord {
                step_index: 0,
                description: "步骤 A".into(),
                tool: Some("bash".into()),
                outcome: ExecutionOutcome::Success("done".into()),
                duration_ms: 100,
                retry_count: 0,
            },
            ExecutionRecord {
                step_index: 1,
                description: "[重构] 步骤 B".into(),
                tool: None,
                outcome: ExecutionOutcome::NeedsRefactor("error".into()),
                duration_ms: 200,
                retry_count: 1,
            },
        ];

        let report = WorkflowExecutor::format_report(&records);
        assert!(report.contains("成功: 1"));
        assert!(report.contains("需重构: 1"));
        assert!(report.contains("步骤 A"));
        assert!(report.contains("[重构] 步骤 B"));
    }
}
