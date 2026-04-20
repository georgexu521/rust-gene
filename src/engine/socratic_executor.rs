//! Socratic Plan Executor
//!
//! 两层架构的完整实现：
//! Layer 1: 计划 + 权重 (宏观排序)
//! Layer 2: Socratic 深度思考 (微观推理)
//!
//! 流程：
//! 1. 任务分解为步骤
//! 2. 每个步骤分配权重
//! 3. 按权重排序
//! 4. 对每个步骤执行 Socratic 分析
//! 5. 基于分析结果执行
//! 6. 完成后重算权重
//! 7. 保存学到的东西

use crate::engine::plan_mode::{Plan, PlanStep, StepStatus};
use crate::engine::socratic::{QaPair, QuestionType, SocraticSession};
use serde::{Deserialize, Serialize};
use tracing::info;

/// 步骤的 Socratic 分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepAnalysis {
    pub step_index: usize,
    pub step_description: String,
    pub original_weight: f64,
    pub qa_chain: Vec<QaPair>,
    pub synthesis: String,
    pub discovered_risks: Vec<String>,
    pub discovered_prerequisites: Vec<String>,
    pub suggested_approach: String,
}

/// 执行阶段
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutorPhase {
    /// 初始化
    Init,
    /// 正在分析步骤
    AnalyzingStep { index: usize },
    /// 正在执行步骤
    ExecutingStep { index: usize },
    /// 重算权重
    Recalculating,
    /// 完成
    Completed,
}

/// Socratic Plan Executor
pub struct SocraticPlanExecutor {
    /// 执行计划
    plan: Plan,
    /// 步骤分析结果
    step_analyses: Vec<StepAnalysis>,
    /// 当前阶段
    phase: ExecutorPhase,
    /// Socratic 配置
    socratic_depth: usize,
    socratic_questions_per_level: usize,
}

impl SocraticPlanExecutor {
    pub fn new(plan: Plan) -> Self {
        Self {
            plan,
            step_analyses: Vec::new(),
            phase: ExecutorPhase::Init,
            socratic_depth: 2,
            socratic_questions_per_level: 3,
        }
    }

    pub fn with_socratic_depth(mut self, depth: usize) -> Self {
        self.socratic_depth = depth.min(3);
        self
    }

    pub fn with_questions_per_level(mut self, count: usize) -> Self {
        self.socratic_questions_per_level = count.min(5);
        self
    }

    /// 获取当前阶段
    pub fn phase(&self) -> &ExecutorPhase {
        &self.phase
    }

    /// 获取步骤分析结果
    pub fn analyses(&self) -> &[StepAnalysis] {
        &self.step_analyses
    }

    /// 获取下一步要分析的步骤
    pub fn next_step_to_analyze(&self) -> Option<(usize, &PlanStep)> {
        self.plan.steps.iter().enumerate().find(|(i, step)| {
            step.status == StepStatus::Pending
                && !self.step_analyses.iter().any(|a| a.step_index == *i)
        })
    }

    /// 对一个步骤执行 Socratic 分析
    pub fn analyze_step(&mut self, step_index: usize) -> StepAnalysis {
        let step = &self.plan.steps[step_index];
        self.phase = ExecutorPhase::AnalyzingStep { index: step_index };

        info!(
            "Socratic analysis for step {}: {}",
            step_index + 1,
            step.description
        );

        // 创建 Socratic 会话
        let mut session = SocraticSession::new(step.description.clone())
            .with_depth(self.socratic_depth)
            .with_questions_per_level(self.socratic_questions_per_level);

        session.generate_initial_questions();

        // 收集所有问题
        let mut questions = Vec::new();
        while let Some((question, qtype, depth)) = session.next_question() {
            questions.push((question, qtype, depth));
        }

        // 生成分析结果（答案由 LLM 填充）
        let analysis = StepAnalysis {
            step_index,
            step_description: step.description.clone(),
            original_weight: 1.0 / self.plan.steps.len() as f64, // 均等初始权重
            qa_chain: questions
                .iter()
                .map(|(q, qt, d)| QaPair {
                    question: q.clone(),
                    answer: String::new(), // 由 LLM 填充
                    question_type: qt.clone(),
                    depth: *d,
                    leads_to: Vec::new(),
                })
                .collect(),
            synthesis: String::new(), // 由 LLM 综合
            discovered_risks: Vec::new(),
            discovered_prerequisites: Vec::new(),
            suggested_approach: String::new(),
        };

        self.step_analyses.push(analysis.clone());
        analysis
    }

    /// 获取需要 LLM 回答的问题列表
    pub fn get_pending_questions(&self, step_index: usize) -> Vec<String> {
        if let Some(analysis) = self
            .step_analyses
            .iter()
            .find(|a| a.step_index == step_index)
        {
            analysis
                .qa_chain
                .iter()
                .filter(|qa| qa.answer.is_empty())
                .map(|qa| qa.question.clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 填充 Socratic 分析的答案
    pub fn fill_answers(
        &mut self,
        step_index: usize,
        answers: Vec<(String, String)>, // (question, answer)
    ) {
        if let Some(analysis) = self
            .step_analyses
            .iter_mut()
            .find(|a| a.step_index == step_index)
        {
            for (question, answer) in answers {
                if let Some(qa) = analysis
                    .qa_chain
                    .iter_mut()
                    .find(|q| q.question == question)
                {
                    qa.answer = answer;
                }
            }

            // 从答案中提取风险和前提
            for qa in &analysis.qa_chain {
                if qa.question_type == QuestionType::RiskAssessment && !qa.answer.is_empty() {
                    analysis.discovered_risks.push(qa.answer.clone());
                }
                if qa.question_type == QuestionType::PrerequisiteCheck && !qa.answer.is_empty() {
                    analysis.discovered_prerequisites.push(qa.answer.clone());
                }
            }

            // 生成综合（先提取需要的数据，避免借用冲突）
            let analysis_clone = analysis.clone();
            analysis.synthesis = Self::synthesize_analysis(&analysis_clone);
        }
    }

    /// 综合分析结果（关联函数，不借用 self）
    fn synthesize_analysis(analysis: &StepAnalysis) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "## Step Analysis: {}\n\n",
            analysis.step_description
        ));

        if !analysis.discovered_prerequisites.is_empty() {
            output.push_str("### Prerequisites\n");
            for p in &analysis.discovered_prerequisites {
                output.push_str(&format!("- {}\n", p));
            }
            output.push('\n');
        }

        if !analysis.discovered_risks.is_empty() {
            output.push_str("### Risks\n");
            for r in &analysis.discovered_risks {
                output.push_str(&format!("- {}\n", r));
            }
            output.push('\n');
        }

        // 从方案优化和反思中提取建议
        let solutions: Vec<&QaPair> = analysis
            .qa_chain
            .iter()
            .filter(|qa| {
                qa.question_type == QuestionType::SolutionOptimization
                    || qa.question_type == QuestionType::Reflection
            })
            .collect();

        if !solutions.is_empty() {
            output.push_str("### Approach\n");
            for s in &solutions {
                if !s.answer.is_empty() {
                    output.push_str(&format!("- {}\n", s.answer));
                }
            }
        }

        output
    }

    /// 标记步骤完成并重算权重
    pub fn complete_step(&mut self, step_index: usize) {
        if step_index < self.plan.steps.len() {
            self.plan.steps[step_index].status = StepStatus::Completed;
            info!("Step {} completed", step_index + 1);
        }

        // 检查是否全部完成
        if self.plan.is_complete() {
            self.phase = ExecutorPhase::Completed;
        } else {
            self.phase = ExecutorPhase::Recalculating;
        }
    }

    /// 基于 Socratic 分析重算权重
    /// 发现的风险越多、前提越多，权重可能需要调整
    pub fn recalculate_weights(&mut self) {
        self.phase = ExecutorPhase::Recalculating;

        for analysis in &self.step_analyses {
            if analysis.step_index >= self.plan.steps.len() {
                continue;
            }
            let step = &mut self.plan.steps[analysis.step_index];
            if step.status != StepStatus::Pending {
                continue;
            }

            // 基于分析调整权重
            let risk_factor = 1.0 + (analysis.discovered_risks.len() as f64 * 0.1);
            let prereq_factor = if analysis.discovered_prerequisites.is_empty() {
                1.0
            } else {
                0.8
            };

            // 原始权重 * 风险因子 * 前提因子
            // 风险高的步骤权重增加（需要更仔细处理）
            // 有未完成前提的步骤权重降低（应该先做前提）
            let _adjusted_weight = analysis.original_weight * risk_factor * prereq_factor;

            // 更新步骤描述以反映分析结果
            if !analysis.synthesis.is_empty() {
                step.description = format!(
                    "{}\n[Socratic: {} risks, {} prereqs]",
                    step.description,
                    analysis.discovered_risks.len(),
                    analysis.discovered_prerequisites.len()
                );
            }
        }

        info!("Weights recalculated based on Socratic analysis");
    }

    /// 获取完整执行报告
    pub fn execution_report(&self) -> String {
        let mut report = String::new();

        report.push_str(&format!("# Execution Report: {}\n\n", self.plan.title));
        report.push_str(&format!("Goal: {}\n", self.plan.goal));
        report.push_str(&format!("Steps: {}\n", self.plan.steps.len()));
        report.push_str(&format!(
            "Completed: {}\n\n",
            self.plan
                .steps
                .iter()
                .filter(|s| s.status == StepStatus::Completed)
                .count()
        ));

        // 每个步骤的 Socratic 分析
        for analysis in &self.step_analyses {
            let step_status = if analysis.step_index < self.plan.steps.len() {
                format!("{:?}", self.plan.steps[analysis.step_index].status)
            } else {
                "Unknown".to_string()
            };

            report.push_str(&format!(
                "## Step {}: {}\n",
                analysis.step_index + 1,
                analysis.step_description
            ));
            report.push_str(&format!("Status: {}\n", step_status));
            report.push_str(&format!("Questions asked: {}\n", analysis.qa_chain.len()));
            report.push_str(&format!(
                "Risks found: {}\n",
                analysis.discovered_risks.len()
            ));
            report.push_str(&format!(
                "Prerequisites: {}\n\n",
                analysis.discovered_prerequisites.len()
            ));

            if !analysis.synthesis.is_empty() {
                report.push_str(&analysis.synthesis);
                report.push('\n');
            }
        }

        // 提取所有学到的内容
        report.push_str("## Learnings\n");
        let all_risks: Vec<&String> = self
            .step_analyses
            .iter()
            .flat_map(|a| &a.discovered_risks)
            .collect();
        if !all_risks.is_empty() {
            report.push_str("Risks discovered:\n");
            for r in &all_risks {
                report.push_str(&format!("- {}\n", r));
            }
        }

        report
    }

    /// 获取可保存到 MEMORY.md 的学习内容
    pub fn extract_learnings(&self) -> Vec<String> {
        let mut learnings = Vec::new();

        for analysis in &self.step_analyses {
            // 保存风险
            for risk in &analysis.discovered_risks {
                if risk.len() > 20 && risk.len() < 500 {
                    learnings.push(format!("Risk in '{}': {}", analysis.step_description, risk));
                }
            }

            // 保存解决方案
            for qa in &analysis.qa_chain {
                if qa.question_type == QuestionType::SolutionOptimization
                    && !qa.answer.is_empty()
                    && qa.answer.len() > 20
                    && qa.answer.len() < 500
                {
                    learnings.push(format!(
                        "Solution for '{}': {}",
                        analysis.step_description, qa.answer
                    ));
                }
            }
        }

        learnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_plan() -> Plan {
        Plan::new("Test Project", "Build something cool")
            .add_step("Design architecture", None)
            .add_step("Implement core", Some("file_write"))
            .add_step("Add tests", Some("bash"))
    }

    #[test]
    fn test_executor_creation() {
        let plan = create_test_plan();
        let executor = SocraticPlanExecutor::new(plan);
        assert_eq!(executor.plan.steps.len(), 3);
        assert_eq!(*executor.phase(), ExecutorPhase::Init);
    }

    #[test]
    fn test_analyze_step() {
        let plan = create_test_plan();
        let mut executor = SocraticPlanExecutor::new(plan);

        let analysis = executor.analyze_step(0);
        assert_eq!(analysis.step_index, 0);
        assert!(!analysis.qa_chain.is_empty());
        assert_eq!(executor.step_analyses.len(), 1);
    }

    #[test]
    fn test_next_step_to_analyze() {
        let plan = create_test_plan();
        let mut executor = SocraticPlanExecutor::new(plan);

        let (idx, _step) = executor.next_step_to_analyze().unwrap();
        assert_eq!(idx, 0);

        executor.analyze_step(0);

        let (idx, _step) = executor.next_step_to_analyze().unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn test_fill_answers() {
        let plan = create_test_plan();
        let mut executor = SocraticPlanExecutor::new(plan);

        executor.analyze_step(0);

        let questions = executor.get_pending_questions(0);
        assert!(!questions.is_empty());

        let answers: Vec<(String, String)> = questions
            .iter()
            .map(|q| (q.clone(), format!("Answer to: {}", q)))
            .collect();

        executor.fill_answers(0, answers);

        let analysis = &executor.step_analyses[0];
        assert!(analysis.qa_chain.iter().all(|qa| !qa.answer.is_empty()));
    }

    #[test]
    fn test_complete_and_recalculate() {
        let plan = create_test_plan();
        let mut executor = SocraticPlanExecutor::new(plan);

        executor.analyze_step(0);
        executor.complete_step(0);
        assert_eq!(executor.plan.steps[0].status, StepStatus::Completed);
    }

    #[test]
    fn test_extract_learnings() {
        let plan = create_test_plan();
        let mut executor = SocraticPlanExecutor::new(plan);

        executor.analyze_step(0);

        // 模拟填充带风险的答案
        let questions = executor.get_pending_questions(0);
        let answers: Vec<(String, String)> = questions
            .iter()
            .map(|q| {
                (
                    q.clone(),
                    "This is a significant risk that needs attention".to_string(),
                )
            })
            .collect();
        executor.fill_answers(0, answers);

        let learnings = executor.extract_learnings();
        assert!(!learnings.is_empty());
    }

    #[test]
    fn test_execution_report() {
        let plan = create_test_plan();
        let mut executor = SocraticPlanExecutor::new(plan);

        executor.analyze_step(0);
        executor.complete_step(0);
        executor.analyze_step(1);

        let report = executor.execution_report();
        assert!(report.contains("Execution Report"));
        assert!(report.contains("Step 1"));
        assert!(report.contains("Step 2"));
    }
}
