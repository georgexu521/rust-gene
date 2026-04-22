//! Planner — 从 ThinkingResult 生成带权重的执行计划
//!
//! M1 范围：
//! - 从 ThinkingResult.extract_steps() 提取步骤
//! - 推断每个步骤的工具
//! - 推断步骤间依赖关系
//! - 调用 WeightEngine 计算每步权重
//! - 输出带 weight / weight_explanation / dependent_step_indices 的 Plan

use crate::engine::plan_mode::{Plan, PlanStep, StepStatus};
use crate::engine::workflow::questioning::ThinkingResult;
use crate::engine::workflow::weights::{StepContext, WeightEngine};

/// Workflow 计划生成器
pub struct WorkflowPlanner {
    weight_engine: WeightEngine,
}

impl WorkflowPlanner {
    pub fn new() -> Self {
        Self {
            weight_engine: WeightEngine::default(),
        }
    }

    /// 从 ThinkingResult 生成带权重的 Plan
    ///
    /// # M1 行为
    /// - 至少产出 1 个步骤（P-01）
    /// - 每个步骤有 weight（P-02）和 weight_explanation（P-03）
    /// - 依赖识别正确（P-04）
    /// - 无循环依赖（P-05）：所有依赖只指向前序步骤，天然无环
    pub fn plan(&self, thinking_result: &ThinkingResult, mainline_goal: &str) -> Plan {
        let descriptions = thinking_result.extract_steps();

        if descriptions.is_empty() {
            return self.build_single_step_plan(&thinking_result.problem_statement, mainline_goal);
        }

        // 推断每个步骤的工具
        let tools: Vec<Option<String>> =
            descriptions.iter().map(|d| Self::infer_tool(d)).collect();

        // 推断依赖关系
        let dependencies = Self::infer_dependencies(&descriptions, &tools);

        // 构建带权重的步骤
        let mut steps = Vec::new();
        for (i, desc) in descriptions.iter().enumerate() {
            let unlocks = dependencies
                .iter()
                .enumerate()
                .filter(|(_, deps)| deps.contains(&i))
                .count();

            let ctx = StepContext {
                description: desc.clone(),
                tool: tools[i].clone(),
                step_index: i,
                total_steps: descriptions.len(),
                mainline_goal: mainline_goal.to_string(),
                completed_steps: vec![],
                dependent_steps: dependencies[i].clone(),
                unlocks_count: unlocks,
            };

            let weighted = self.weight_engine.compute(&ctx);

            steps.push(PlanStep {
                description: desc.clone(),
                tool: tools[i].clone(),
                status: StepStatus::Pending,
                weight: weighted.normalized_score,
                weight_explanation: weighted.explanation,
                dependent_step_indices: dependencies[i].clone(),
            });
        }

        Plan {
            title: format!("Plan for: {}", truncate(mainline_goal, 40)),
            goal: thinking_result.problem_statement.clone(),
            steps,
            estimated_complexity: Self::estimate_complexity(descriptions.len()),
        }
    }

    /// 使用 WeightEngine 对 Plan 中的步骤重新计算权重
    ///
    /// 用于执行过程中状态变化后（如某些步骤已完成）的重新排序。
    pub fn reweight(&self, plan: &mut Plan, mainline_goal: &str) {
        let completed: Vec<usize> = plan
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.status == StepStatus::Completed || s.status == StepStatus::Skipped)
            .map(|(i, _)| i)
            .collect();

        let total_steps = plan.steps.len();
        for (i, step) in plan.steps.iter_mut().enumerate() {
            let unlocks = step
                .dependent_step_indices
                .iter()
                .filter(|dep_idx| !completed.contains(dep_idx))
                .count();

            let ctx = StepContext {
                description: step.description.clone(),
                tool: step.tool.clone(),
                step_index: i,
                total_steps,
                mainline_goal: mainline_goal.to_string(),
                completed_steps: completed.clone(),
                dependent_steps: step.dependent_step_indices.clone(),
                unlocks_count: unlocks,
            };

            let weighted = self.weight_engine.compute(&ctx);
            step.weight = weighted.normalized_score;
            step.weight_explanation = weighted.explanation;
        }
    }

    // ============================================================================
    // 工具推断
    // ============================================================================

    fn infer_tool(description: &str) -> Option<String> {
        let d = description.to_lowercase();

        // 读取类
        if d.contains("读取") || d.contains("查看") || d.contains("read") || d.contains("cat ") {
            return Some("file_read".into());
        }
        if d.contains("搜索") || d.contains("grep") || d.contains("find") {
            return Some("grep".into());
        }
        if d.contains("glob") || d.contains("列出文件") || d.contains("list files") {
            return Some("glob".into());
        }

        // 写入类
        if d.contains("创建") || d.contains("新建") || d.contains("write") || d.contains("生成文件")
        {
            return Some("file_write".into());
        }

        // 编辑类
        if d.contains("修改") || d.contains("修复") || d.contains("fix") || d.contains("edit")
            || d.contains("update") || d.contains("改")
        {
            return Some("file_edit".into());
        }

        // 执行类
        if d.contains("测试") || d.contains("test") || d.contains("编译") || d.contains("build")
            || d.contains("cargo ") || d.contains("运行") || d.contains("run")
            || d.contains("执行") || d.contains("deploy") || d.contains("启动")
            || d.contains("git ")
        {
            return Some("bash".into());
        }

        // Agent 类
        if d.contains("agent") || d.contains("spawn") || d.contains("子任务") {
            return Some("agent".into());
        }

        // Web 类
        if d.contains("web") || d.contains("fetch") || d.contains("url") || d.contains("网页")
        {
            return Some("web_fetch".into());
        }

        // MCP
        if d.contains("mcp") {
            return Some("mcp".into());
        }

        None
    }

    // ============================================================================
    // 依赖推断
    // ============================================================================

    /// 推断步骤间依赖关系
    ///
    /// M1 简化规则：
    /// 1. 含有"然后/之后/接着/next/then/after"的步骤依赖前一步
    /// 2. 测试/验证步骤依赖最近的实现/编写/修改步骤
    /// 3. 编辑/写入步骤依赖最近的读取步骤（了解上下文）
    ///
    /// 所有依赖只指向更小的索引，天然无环（P-05 保证）。
    fn infer_dependencies(
        descriptions: &[String],
        tools: &[Option<String>],
    ) -> Vec<Vec<usize>> {
        let n = descriptions.len();
        let mut deps: Vec<Vec<usize>> = vec![vec![]; n];
        let lowered: Vec<String> = descriptions.iter().map(|d| d.to_lowercase()).collect();

        for i in 0..n {
            let desc = &lowered[i];

            // 规则 1：顺序词 → 依赖前一步
            if desc.contains("然后")
                || desc.contains("之后")
                || desc.contains("接着")
                || desc.contains("next")
                || desc.contains("then")
                || desc.contains("after")
                || desc.contains("finally")
                || desc.contains("最后")
            {
                if i > 0 && !deps[i].contains(&(i - 1)) {
                    deps[i].push(i - 1);
                }
            }

            // 规则 2：测试/验证 → 依赖最近的实现类步骤
            let is_test = desc.contains("测试")
                || desc.contains("验证")
                || desc.contains("test")
                || desc.contains("verify")
                || desc.contains("check")
                || desc.contains("validate");
            if is_test {
                for j in (0..i).rev() {
                    let prev = &lowered[j];
                    let is_impl = prev.contains("实现")
                        || prev.contains("编写")
                        || prev.contains("修改")
                        || prev.contains("添加")
                        || prev.contains("新增")
                        || prev.contains("create")
                        || prev.contains("implement")
                        || prev.contains("add")
                        || prev.contains("fix")
                        || prev.contains("edit")
                        || prev.contains("write")
                        || prev.contains("设计")
                        || prev.contains("design");
                    if is_impl {
                        if !deps[i].contains(&j) {
                            deps[i].push(j);
                        }
                        break;
                    }
                }
            }

            // 规则 3：编辑/写入 → 依赖最近的读取步骤
            let tool = tools[i].as_deref();
            let is_write = tool == Some("file_edit") || tool == Some("file_write");
            if is_write {
                for j in (0..i).rev() {
                    let prev_tool = tools[j].as_deref();
                    if prev_tool == Some("file_read") || prev_tool == Some("grep") {
                        if !deps[i].contains(&j) {
                            deps[i].push(j);
                        }
                        break;
                    }
                }
            }
        }

        deps
    }

    // ============================================================================
    // 辅助方法
    // ============================================================================

    fn build_single_step_plan(&self, problem: &str, mainline_goal: &str) -> Plan {
        let ctx = StepContext {
            description: problem.into(),
            tool: None,
            step_index: 0,
            total_steps: 1,
            mainline_goal: mainline_goal.to_string(),
            completed_steps: vec![],
            dependent_steps: vec![],
            unlocks_count: 0,
        };
        let weighted = self.weight_engine.compute(&ctx);

        let step = PlanStep {
            description: problem.into(),
            tool: None,
            status: StepStatus::Pending,
            weight: weighted.normalized_score,
            weight_explanation: weighted.explanation,
            dependent_step_indices: vec![],
        };

        Plan {
            title: format!("Plan for: {}", truncate(mainline_goal, 40)),
            goal: problem.into(),
            steps: vec![step],
            estimated_complexity: "low".into(),
        }
    }

    fn estimate_complexity(step_count: usize) -> String {
        match step_count {
            0..=2 => "low",
            3..=5 => "medium",
            _ => "high",
        }
        .to_string()
    }
}

impl Default for WorkflowPlanner {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max).collect::<String>())
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_thinking_result() -> ThinkingResult {
        ThinkingResult {
            problem_statement: "实现用户认证系统".into(),
            key_uncertainties: vec!["数据库选型不确定".into()],
            decision_basis: "1. 设计数据库表结构\n2. 实现登录接口\n3. 实现注册接口\n4. 编写测试验证".into(),
            question_chain: vec![],
            total_token_cost: 500,
            convergence_reason: "executable_plan_formed".into(),
        }
    }

    #[test]
    fn test_plan_from_thinking_result() {
        let planner = WorkflowPlanner::new();
        let thinking = sample_thinking_result();
        let plan = planner.plan(&thinking, "实现用户认证系统");

        // P-01: steps >= 1
        assert!(!plan.steps.is_empty(), "Plan should have at least 1 step");
        // 从 decision_basis 中应提取到 4 个步骤
        assert_eq!(plan.steps.len(), 4, "Expected 4 steps from numbered list");
    }

    #[test]
    fn test_steps_have_weight() {
        let planner = WorkflowPlanner::new();
        let thinking = sample_thinking_result();
        let plan = planner.plan(&thinking, "实现用户认证系统");

        for (i, step) in plan.steps.iter().enumerate() {
            // P-02: weight 有值（归一化到 [0,100]）
            assert!(
                step.weight <= 100,
                "Step {} weight should be <= 100, got {}",
                i,
                step.weight
            );
        }
    }

    #[test]
    fn test_steps_have_weight_explanation() {
        let planner = WorkflowPlanner::new();
        let thinking = sample_thinking_result();
        let plan = planner.plan(&thinking, "实现用户认证系统");

        for (i, step) in plan.steps.iter().enumerate() {
            // P-03: weight_explanation 非空
            assert!(
                !step.weight_explanation.is_empty(),
                "Step {} should have weight_explanation",
                i
            );
        }
    }

    #[test]
    fn test_dependency_inference() {
        let planner = WorkflowPlanner::new();
        let mut thinking = sample_thinking_result();
        // 构造一个包含顺序和测试依赖的描述
        thinking.decision_basis = "1. 读取现有代码\n2. 然后修改登录逻辑\n3. 最后编写测试验证".into();
        let plan = planner.plan(&thinking, "修复登录 bug");

        // "然后修改" 应该依赖步骤 0
        assert!(
            plan.steps[1].dependent_step_indices.contains(&0),
            "Step 1 should depend on step 0"
        );

        // "测试验证" 应该依赖实现步骤（步骤 1）
        assert!(
            plan.steps[2].dependent_step_indices.contains(&1),
            "Step 2 (test) should depend on step 1 (implementation)"
        );
    }

    #[test]
    fn test_no_circular_dependencies() {
        let planner = WorkflowPlanner::new();
        let thinking = sample_thinking_result();
        let plan = planner.plan(&thinking, "实现用户认证系统");

        // P-05: 检查无循环依赖
        // 由于所有依赖只指向更小的索引，只需验证这一点
        for (i, step) in plan.steps.iter().enumerate() {
            for &dep in &step.dependent_step_indices {
                assert!(
                    dep < i,
                    "Circular or forward dependency detected: step {} depends on {}",
                    i,
                    dep
                );
            }
        }
    }

    #[test]
    fn test_tool_inference() {
        assert_eq!(WorkflowPlanner::infer_tool("读取配置文件"), Some("file_read".into()));
        assert_eq!(WorkflowPlanner::infer_tool("修改登录逻辑"), Some("file_edit".into()));
        assert_eq!(WorkflowPlanner::infer_tool("创建新模块"), Some("file_write".into()));
        assert_eq!(WorkflowPlanner::infer_tool("运行 cargo test"), Some("bash".into()));
        assert_eq!(WorkflowPlanner::infer_tool("搜索 TODO"), Some("grep".into()));
        assert_eq!(WorkflowPlanner::infer_tool("随便什么步骤"), None);
    }

    #[test]
    fn test_empty_steps_fallback() {
        let planner = WorkflowPlanner::new();
        let thinking = ThinkingResult {
            problem_statement: "修复一个简单问题".into(),
            key_uncertainties: vec![],
            decision_basis: "没有明确步骤".into(),
            question_chain: vec![],
            total_token_cost: 100,
            convergence_reason: "budget_exhausted".into(),
        };
        let plan = planner.plan(&thinking, "修复问题");

        // 即使 extract_steps 返回空，也应有 fallback 单步骤
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].description, "修复一个简单问题");
    }

    #[test]
    fn test_reweight_updates_scores() {
        let planner = WorkflowPlanner::new();
        let thinking = sample_thinking_result();
        let mut plan = planner.plan(&thinking, "实现用户认证系统");

        let _original_weight = plan.steps[0].weight;

        // 标记第一步完成
        plan.steps[0].status = StepStatus::Completed;
        planner.reweight(&mut plan, "实现用户认证系统");

        // 重新计算后权重可能变化（因为 completed_steps 变了）
        // 主要是验证不 panic，且 explanation 被更新
        assert!(!plan.steps[0].weight_explanation.is_empty());
    }

    #[test]
    fn test_read_before_edit_dependency() {
        let planner = WorkflowPlanner::new();
        let thinking = ThinkingResult {
            problem_statement: "修改代码".into(),
            key_uncertainties: vec![],
            decision_basis: "1. 读取 auth.rs\n2. 修改登录函数".into(),
            question_chain: vec![],
            total_token_cost: 100,
            convergence_reason: "test".into(),
        };
        let plan = planner.plan(&thinking, "修改登录功能");

        // 步骤 1（修改）应该依赖步骤 0（读取）
        if plan.steps.len() >= 2 {
            assert!(
                plan.steps[1].dependent_step_indices.contains(&0),
                "Edit step should depend on read step"
            );
        }
    }
}
