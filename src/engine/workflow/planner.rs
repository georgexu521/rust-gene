//! Planner — 从 ThinkingResult 生成带权重的执行计划
//!
//! M1 范围：
//! - 从 ThinkingResult.extract_steps() 提取步骤
//! - 推断每个步骤的工具
//! - 推断步骤间依赖关系
//! - 调用 WeightEngine 计算每步权重
//! - 输出带 weight / weight_explanation / dependent_step_indices 的 Plan

use crate::engine::plan_mode::{Plan, PlanStep, StepStatus};
use crate::engine::workflow::policy::WorkflowPolicy;
use crate::engine::workflow::questioning::ThinkingResult;
use crate::engine::workflow::weights::{StepContext, WeightEngine};
use crate::services::api::{ChatRequest, LlmProvider, Message};
use std::sync::Arc;

/// Workflow 计划生成器
pub struct WorkflowPlanner {
    weight_engine: WeightEngine,
    llm_provider: Option<Arc<dyn LlmProvider>>,
}

impl WorkflowPlanner {
    pub fn new() -> Self {
        Self {
            weight_engine: WeightEngine::default(),
            llm_provider: None,
        }
    }

    /// 创建带 LLM 增强的 Planner（M2）
    pub fn with_llm(llm_provider: Arc<dyn LlmProvider>) -> Self {
        Self {
            weight_engine: WeightEngine::default(),
            llm_provider: Some(llm_provider),
        }
    }

    pub fn with_policy(policy: &WorkflowPolicy) -> Self {
        Self {
            weight_engine: WeightEngine::from_multipliers(&policy.weights),
            llm_provider: None,
        }
    }

    pub fn with_llm_and_policy(
        llm_provider: Arc<dyn LlmProvider>,
        policy: &WorkflowPolicy,
    ) -> Self {
        Self {
            weight_engine: WeightEngine::from_multipliers(&policy.weights),
            llm_provider: Some(llm_provider),
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
        let tools: Vec<Option<String>> = descriptions.iter().map(|d| Self::infer_tool(d)).collect();

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
                depth: 0,
            });
        }

        Plan {
            title: format!("Plan for: {}", truncate(mainline_goal, 40)),
            goal: thinking_result.problem_statement.clone(),
            steps,
            estimated_complexity: Self::estimate_complexity(descriptions.len()),
            depth: 0,
            max_depth: Plan::default_max_depth(),
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

    /// M2: LLM 增强计划生成
    ///
    /// 对复杂计划（>5 步或存在低置信度依赖）调用 LLM 辅助推断依赖关系。
    pub async fn plan_enhanced(
        &self,
        thinking_result: &ThinkingResult,
        mainline_goal: &str,
    ) -> Plan {
        let mut plan = self.plan(thinking_result, mainline_goal);

        // 简单计划跳过 LLM 增强
        if plan.steps.len() <= 3 {
            return plan;
        }

        // 检查是否存在低置信度依赖（孤岛步骤）
        let has_islands = plan
            .steps
            .iter()
            .enumerate()
            .any(|(i, s)| i > 0 && s.dependent_step_indices.is_empty());

        if !has_islands && plan.steps.len() <= 5 {
            return plan;
        }

        // 尝试 LLM 增强
        if let Some(ref provider) = self.llm_provider {
            if let Ok(enhanced_deps) = self
                .infer_dependencies_with_llm(provider, &plan.steps, mainline_goal)
                .await
            {
                // 合并 LLM 推断的依赖（只添加不删除，保守策略）
                for (i, extra_deps) in enhanced_deps.iter().enumerate() {
                    if i < plan.steps.len() {
                        for dep in extra_deps {
                            if *dep < i && !plan.steps[i].dependent_step_indices.contains(dep) {
                                plan.steps[i].dependent_step_indices.push(*dep);
                            }
                        }
                    }
                }
            }
        }

        plan
    }

    // ============================================================================
    // 递归计划拆分（Gap #1）
    // ============================================================================

    /// 递归计划生成：当计划复杂时自动拆分，达到 max_depth 时强制原子化
    ///
    /// 规则：
    /// 1. 先生成初始计划（plan_enhanced）
    /// 2. 若满足拆分条件且 depth < max_depth，调用 LLM 细化步骤 → depth + 1
    /// 3. 若 depth >= max_depth，调用 flatten_to_atomic 强制原子化
    pub async fn plan_with_recursion(
        &self,
        thinking_result: &ThinkingResult,
        mainline_goal: &str,
    ) -> Plan {
        let mut plan = self.plan_enhanced(thinking_result, mainline_goal).await;
        let max_depth = plan.max_depth;

        // 递归细化：最多 max_depth 层
        while plan.depth < max_depth && Self::needs_recursive_split(&plan) {
            if let Some(ref provider) = self.llm_provider {
                match self.refine_plan_with_llm(provider, &plan).await {
                    Ok(refined) => {
                        plan = refined;
                    }
                    Err(_) => break,
                }
            } else {
                break;
            }
        }

        // L3（或 max_depth）强制原子化
        if plan.depth >= max_depth.saturating_sub(1) {
            Self::flatten_to_atomic(&mut plan);
        }

        plan
    }

    /// 判断计划是否需要递归拆分
    ///
    /// 触发条件（满足任一）：
    /// - 引用文件数 >= 5
    /// - 预估复杂度为 high
    /// - 涉及 >= 2 个子领域（通过关键词简单推断）
    fn needs_recursive_split(plan: &Plan) -> bool {
        // 简单计划不拆分
        if plan.steps.len() <= 3 {
            return false;
        }

        // 条件 1：引用文件数 >= 5
        let total_files: usize = plan
            .steps
            .iter()
            .map(|s| Self::extract_file_references(&s.description).len())
            .sum();
        if total_files >= 5 {
            return true;
        }

        // 条件 2：预估复杂度为 high
        if plan.estimated_complexity == "high" {
            return true;
        }

        // 条件 3：涉及 >= 2 个子领域
        if Self::count_subdomains(plan) >= 2 {
            return true;
        }

        false
    }

    /// 简单子领域计数（通过领域关键词推断）
    fn count_subdomains(plan: &Plan) -> usize {
        let domains = [
            (
                "数据库",
                vec!["数据库", "db", "sql", "表", "schema", "migration"],
            ),
            (
                "前端",
                vec!["前端", "ui", "界面", "react", "vue", "html", "css"],
            ),
            (
                "后端",
                vec!["后端", "api", "接口", "路由", "controller", "handler"],
            ),
            ("测试", vec!["测试", "test", "unit", "integration", "mock"]),
            (
                "安全",
                vec!["安全", "auth", "认证", "加密", "permission", "login"],
            ),
            ("部署", vec!["部署", "deploy", "docker", "ci", "cd", "k8s"]),
        ];

        let mut matched = std::collections::HashSet::new();
        for step in &plan.steps {
            let desc = step.description.to_lowercase();
            for (domain_name, keywords) in &domains {
                for kw in keywords {
                    if desc.contains(kw) {
                        matched.insert(*domain_name);
                        break;
                    }
                }
            }
        }
        matched.len()
    }

    /// 使用 LLM 细化计划：将复杂步骤拆分为更细粒度的子步骤
    ///
    /// 返回的新 plan depth 自动 +1。
    async fn refine_plan_with_llm(
        &self,
        provider: &Arc<dyn LlmProvider>,
        plan: &Plan,
    ) -> anyhow::Result<Plan> {
        let step_list: String = plan
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                format!(
                    "{}: {} (tool: {:?}, deps: {:?})",
                    i, s.description, s.tool, s.dependent_step_indices
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "请将以下执行计划细化为更具体的步骤。主线目标: {}\n\n\
            当前计划（{} 步）:\n{}\n\n\
            请输出细化后的步骤列表（JSON 格式），要求：\n\
            1. 保持原有步骤的逻辑顺序\n\
            2. 将每个复杂步骤拆分为 2-4 个更细粒度的原子步骤\n\
            3. 输出格式为 JSON 字符串数组，例如：[\"步骤1\", \"步骤2\", \"步骤3\"]\n\
            4. 只输出 JSON，不输出其他文字\n\
            5. 步骤总数应在 {} ~ {} 之间\n\n\
            输出:",
            plan.goal,
            plan.steps.len(),
            step_list,
            plan.steps.len() + 1,
            plan.steps.len() * 3
        );

        let request = ChatRequest {
            model: provider.default_model().to_string(),
            messages: vec![Message::User { content: prompt }],
            tools: None,
            temperature: Some(0.1),
            max_tokens: Some(1000),
            thinking_budget: None,
        };

        let response = provider.chat(request).await?;
        let descriptions: Vec<String> = serde_json::from_str(response.content.trim())?;

        if descriptions.is_empty() {
            return Err(anyhow::anyhow!("LLM returned empty step list"));
        }

        // 用新的描述重新构建计划
        let tools: Vec<Option<String>> = descriptions.iter().map(|d| Self::infer_tool(d)).collect();
        let dependencies = Self::infer_dependencies(&descriptions, &tools);

        let mut steps = Vec::new();
        for (i, desc) in descriptions.iter().enumerate() {
            let ctx = StepContext {
                description: desc.clone(),
                tool: tools[i].clone(),
                step_index: i,
                total_steps: descriptions.len(),
                mainline_goal: plan.goal.clone(),
                completed_steps: vec![],
                dependent_steps: dependencies[i].clone(),
                unlocks_count: 0,
            };

            let weighted = self.weight_engine.compute(&ctx);

            steps.push(PlanStep {
                description: desc.clone(),
                tool: tools[i].clone(),
                status: StepStatus::Pending,
                weight: weighted.normalized_score,
                weight_explanation: weighted.explanation,
                dependent_step_indices: dependencies[i].clone(),
                depth: plan.depth + 1,
            });
        }

        Ok(Plan {
            title: plan.title.clone(),
            goal: plan.goal.clone(),
            steps,
            estimated_complexity: Self::estimate_complexity(descriptions.len()),
            depth: plan.depth + 1,
            max_depth: plan.max_depth,
        })
    }

    /// L3 原子化展平：将读/搜类步骤合并，写/改类保持原子
    ///
    /// 规则：
    /// - file_read / grep / glob → 合并为单个 "combined_read" 步骤
    /// - file_write / file_edit → 保持为独立原子步骤
    /// - 其他工具 → 保持为独立步骤
    fn flatten_to_atomic(plan: &mut Plan) {
        let mut read_steps: Vec<String> = Vec::new();
        let mut atomic_steps: Vec<PlanStep> = Vec::new();

        for step in plan.steps.drain(..) {
            match step.tool.as_deref() {
                Some("file_read") | Some("grep") | Some("glob") => {
                    read_steps.push(step.description.clone());
                }
                _ => {
                    atomic_steps.push(step);
                }
            }
        }

        // 标记是否有读/搜步骤（在 read_steps 被消费前记录）
        let has_read_steps = !read_steps.is_empty();

        // 如果有读/搜步骤，合并为一个
        if has_read_steps {
            let combined_desc = if read_steps.len() == 1 {
                read_steps.into_iter().next().unwrap()
            } else {
                format!("合并读取/搜索: {}", read_steps.join("; "))
            };

            let read_step = PlanStep {
                description: combined_desc,
                tool: Some("file_read".into()),
                status: StepStatus::Pending,
                weight: 50,
                weight_explanation: "L3 合并读取步骤".into(),
                dependent_step_indices: vec![],
                depth: plan.depth,
            };
            atomic_steps.insert(0, read_step);
        }

        // 重新计算依赖（简单策略：写/改依赖合并后的 read 步骤）
        let read_step_idx = if has_read_steps { Some(0usize) } else { None };
        for (i, step) in atomic_steps.iter_mut().enumerate() {
            if matches!(step.tool.as_deref(), Some("file_write") | Some("file_edit")) {
                if let Some(read_idx) = read_step_idx {
                    if i != read_idx && !step.dependent_step_indices.contains(&read_idx) {
                        step.dependent_step_indices.push(read_idx);
                    }
                }
            }
            // 确保 depth 为 max_depth（原子层）
            step.depth = plan.max_depth;
        }

        plan.steps = atomic_steps;
        plan.depth = plan.max_depth;
    }

    /// M2: 使用 LLM 推断依赖关系
    ///
    /// 返回每个步骤额外依赖的步骤索引列表（仅含 LLM 补充的依赖）。
    async fn infer_dependencies_with_llm(
        &self,
        provider: &Arc<dyn LlmProvider>,
        steps: &[PlanStep],
        mainline_goal: &str,
    ) -> anyhow::Result<Vec<Vec<usize>>> {
        let step_list: String = steps
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}: {}", i, s.description))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "分析以下任务步骤的依赖关系。主线目标: {}\n\n步骤列表:\n{}\n\n\
            请输出每个步骤依赖的前序步骤索引（JSON 数组的数组格式，只输出 JSON）。\
            规则：\n\
            1. 只输出合法的 JSON，不输出其他文字\n\
            2. 每个步骤只依赖索引更小的步骤\n\
            3. 如果没有额外依赖，用空数组 []\n\
            4. 格式示例：[[],[0],[0,1],[1]]\n\n\
            输出:",
            mainline_goal, step_list
        );

        let request = ChatRequest {
            model: provider.default_model().to_string(),
            messages: vec![Message::User { content: prompt }],
            tools: None,
            temperature: Some(0.0),
            max_tokens: Some(500),
            thinking_budget: None,
        };

        let response = provider.chat(request).await?;
        let deps: Vec<Vec<usize>> = serde_json::from_str(response.content.trim())?;

        // 安全校验：只保留合法的依赖（索引更小且无越界）
        let validated: Vec<Vec<usize>> = deps
            .iter()
            .enumerate()
            .map(|(i, step_deps)| {
                step_deps
                    .iter()
                    .filter(|dep| **dep < i && **dep < steps.len())
                    .copied()
                    .collect()
            })
            .collect();

        Ok(validated)
    }

    // ============================================================================
    // 工具推断
    // ============================================================================

    fn infer_tool(description: &str) -> Option<String> {
        let d = description.to_lowercase();
        let is_dir_op = d.contains("文件夹")
            || d.contains("目录")
            || d.contains("folder")
            || d.contains("directory")
            || d.contains("mkdir");
        let is_edit = d.contains("修改")
            || d.contains("修复")
            || d.contains("fix")
            || d.contains("edit")
            || d.contains("update")
            || d.contains("改");
        let is_search = d.contains("搜索") || d.contains("grep") || d.contains("find");

        // 读取类
        if d.contains("读取") || d.contains("查看") || d.contains("read") || d.contains("cat ")
        {
            return Some("file_read".into());
        }
        if d.contains("glob") || d.contains("列出文件") || d.contains("list files") {
            return Some("glob".into());
        }

        // 目录类操作（优先于 file_write，避免“新建文件夹”误判为写文件）
        if is_dir_op {
            return Some("bash".into());
        }

        // 写入类
        if d.contains("创建") || d.contains("新建") || d.contains("write") || d.contains("生成文件")
        {
            return Some("file_write".into());
        }

        // 编辑类
        if is_edit {
            return Some("file_edit".into());
        }

        // 搜索类（优先级低于编辑类，避免“find + fix”落到 grep）
        if is_search {
            return Some("grep".into());
        }

        // 执行类
        if d.contains("测试")
            || d.contains("test")
            || d.contains("编译")
            || d.contains("build")
            || d.contains("cargo ")
            || d.contains("运行")
            || d.contains("run")
            || d.contains("执行")
            || d.contains("deploy")
            || d.contains("启动")
            || d.contains("git ")
        {
            return Some("bash".into());
        }

        // Agent 类
        if d.contains("agent") || d.contains("spawn") || d.contains("子任务") {
            return Some("agent".into());
        }

        // Web 类
        if d.contains("web") || d.contains("fetch") || d.contains("url") || d.contains("网页") {
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
    /// M1 简化规则 + M2 增强：
    /// 1. 含有"然后/之后/接着/next/then/after"的步骤依赖前一步
    /// 2. 测试/验证步骤依赖最近的实现/编写/修改步骤
    /// 3. 编辑/写入步骤依赖最近的读取步骤（了解上下文）
    /// 4. (M2) 同文件引用 → 步骤间隐含依赖
    /// 5. (M2) 语义关键词：基于/依赖/requires/depends on/build on
    ///
    /// 所有依赖只指向更小的索引，天然无环（P-05 保证）。
    fn infer_dependencies(descriptions: &[String], tools: &[Option<String>]) -> Vec<Vec<usize>> {
        let n = descriptions.len();
        let mut deps: Vec<Vec<usize>> = vec![vec![]; n];
        let lowered: Vec<String> = descriptions.iter().map(|d| d.to_lowercase()).collect();

        // M2: 预提取每个步骤提到的文件名
        let file_refs: Vec<Vec<String>> = lowered
            .iter()
            .map(|d| Self::extract_file_references(d))
            .collect();

        for i in 0..n {
            let desc = &lowered[i];

            // 规则 1：顺序词 → 依赖前一步
            if (desc.contains("然后")
                || desc.contains("之后")
                || desc.contains("接着")
                || desc.contains("next")
                || desc.contains("then")
                || desc.contains("after")
                || desc.contains("finally")
                || desc.contains("最后"))
                && i > 0
                && !deps[i].contains(&(i - 1))
            {
                deps[i].push(i - 1);
            }

            // M2 规则 5：语义依赖词 → 依赖前一步（强顺序暗示）
            let has_semantic_dep = desc.contains("基于")
                || desc.contains("依赖")
                || desc.contains("requires")
                || desc.contains("depends on")
                || desc.contains("build on")
                || desc.contains("建立在");
            if has_semantic_dep && i > 0 && !deps[i].contains(&(i - 1)) {
                deps[i].push(i - 1);
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

            // M2 规则 4：同文件引用 → 隐含依赖
            for j in 0..i {
                let shared_files: Vec<_> = file_refs[i]
                    .iter()
                    .filter(|f| file_refs[j].contains(f))
                    .collect();
                if !shared_files.is_empty() && !deps[i].contains(&j) {
                    deps[i].push(j);
                }
            }
        }

        deps
    }

    /// M2: 从描述中提取文件名引用
    ///
    /// 简单启发式：匹配 `xxx.rs`, `xxx.toml`, `xxx.md`, `xxx.json` 等常见文件扩展名。
    fn extract_file_references(description: &str) -> Vec<String> {
        let mut refs = Vec::new();
        let exts = [
            ".rs", ".toml", ".md", ".json", ".yaml", ".yml", ".js", ".ts", ".py", ".go",
        ];

        for word in description.split_whitespace() {
            let trimmed = word.trim_matches(|c: char| c.is_ascii_punctuation());
            for ext in &exts {
                if trimmed.ends_with(ext) && trimmed.len() > ext.len() {
                    refs.push(trimmed.to_lowercase());
                    break;
                }
            }
        }
        refs
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
            depth: 0,
        };

        Plan {
            title: format!("Plan for: {}", truncate(mainline_goal, 40)),
            goal: problem.into(),
            steps: vec![step],
            estimated_complexity: "low".into(),
            depth: 0,
            max_depth: Plan::default_max_depth(),
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
            decision_basis:
                "1. 设计数据库表结构\n2. 实现登录接口\n3. 实现注册接口\n4. 编写测试验证".into(),
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
        thinking.decision_basis =
            "1. 读取现有代码\n2. 然后修改登录逻辑\n3. 最后编写测试验证".into();
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
        assert_eq!(
            WorkflowPlanner::infer_tool("读取配置文件"),
            Some("file_read".into())
        );
        assert_eq!(
            WorkflowPlanner::infer_tool("修改登录逻辑"),
            Some("file_edit".into())
        );
        assert_eq!(
            WorkflowPlanner::infer_tool("创建新模块"),
            Some("file_write".into())
        );
        assert_eq!(
            WorkflowPlanner::infer_tool("在桌面新建一个 gex 文件夹"),
            Some("bash".into())
        );
        assert_eq!(
            WorkflowPlanner::infer_tool("运行 cargo test"),
            Some("bash".into())
        );
        assert_eq!(
            WorkflowPlanner::infer_tool("搜索 TODO"),
            Some("grep".into())
        );
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

    // ============================================================================
    // M2 增强测试
    // ============================================================================

    #[test]
    fn test_extract_file_references() {
        assert_eq!(
            WorkflowPlanner::extract_file_references("读取 auth.rs 和 config.toml"),
            vec!["auth.rs", "config.toml"]
        );
        assert_eq!(
            WorkflowPlanner::extract_file_references("修改 main.js 中的逻辑"),
            vec!["main.js"]
        );
        assert!(WorkflowPlanner::extract_file_references("随便做点什么").is_empty());
    }

    #[test]
    fn test_file_based_dependency_inference() {
        let planner = WorkflowPlanner::new();
        let thinking = ThinkingResult {
            problem_statement: "重构代码".into(),
            key_uncertainties: vec![],
            // 步骤 0 和步骤 2 都提到 auth.rs → 步骤 2 应隐含依赖步骤 0
            decision_basis: "1. 读取 auth.rs 了解现状\n2. 设计新接口\n3. 基于 auth.rs 实现新逻辑"
                .into(),
            question_chain: vec![],
            total_token_cost: 100,
            convergence_reason: "test".into(),
        };
        let plan = planner.plan(&thinking, "重构认证模块");

        if plan.steps.len() >= 3 {
            // 步骤 2 因同文件引用应依赖步骤 0
            assert!(
                plan.steps[2].dependent_step_indices.contains(&0),
                "Step 2 should depend on step 0 due to shared file auth.rs"
            );
        }
    }

    #[test]
    fn test_semantic_dependency_keywords() {
        let planner = WorkflowPlanner::new();
        let thinking = ThinkingResult {
            problem_statement: "构建系统".into(),
            key_uncertainties: vec![],
            decision_basis: "1. 设计数据库表\n2. 基于上述设计实现 API".into(),
            question_chain: vec![],
            total_token_cost: 100,
            convergence_reason: "test".into(),
        };
        let plan = planner.plan(&thinking, "构建 API 系统");

        if plan.steps.len() >= 2 {
            // 步骤 1 含"基于"语义词，应依赖步骤 0
            assert!(
                plan.steps[1].dependent_step_indices.contains(&0),
                "Step 1 should depend on step 0 due to semantic keyword '基于'"
            );
        }
    }

    // ============================================================================
    // 递归拆分测试 (Gap #1)
    // ============================================================================

    #[test]
    fn test_needs_recursive_split_simple_plan() {
        let plan =
            Plan::new("简单任务", "修一个 bug").add_step("修改一行代码", Some("file_edit".into()));
        assert!(
            !WorkflowPlanner::needs_recursive_split(&plan),
            "Simple plan should not split"
        );
    }

    #[test]
    fn test_needs_recursive_split_high_complexity() {
        let mut plan = Plan::new("复杂重构", "重构整个模块").with_complexity("high");
        plan.steps = (0..6)
            .map(|i| PlanStep::new(format!("步骤 {}", i), None))
            .collect();
        assert!(
            WorkflowPlanner::needs_recursive_split(&plan),
            "High complexity plan should split"
        );
    }

    #[test]
    fn test_needs_recursive_split_many_files() {
        let mut plan = Plan::new("多文件改动", "改很多文件");
        plan.steps = vec![
            PlanStep::new("读取 auth.rs", Some("file_read".into())),
            PlanStep::new("读取 config.toml", Some("file_read".into())),
            PlanStep::new("读取 main.rs", Some("file_read".into())),
            PlanStep::new("读取 lib.rs", Some("file_read".into())),
            PlanStep::new("读取 utils.rs", Some("file_read".into())),
            PlanStep::new("修改 auth.rs", Some("file_edit".into())),
        ];
        assert!(
            WorkflowPlanner::needs_recursive_split(&plan),
            "Plan with >=5 files should split"
        );
    }

    #[test]
    fn test_needs_recursive_split_multi_domain() {
        let mut plan = Plan::new("全栈任务", "前后端一起改");
        plan.steps = vec![
            PlanStep::new("设计数据库表", Some("bash".into())),
            PlanStep::new("实现 API 接口", Some("file_write".into())),
            PlanStep::new("编写前端组件", Some("file_write".into())),
            PlanStep::new("部署到 Docker", Some("bash".into())),
        ];
        assert!(
            WorkflowPlanner::needs_recursive_split(&plan),
            "Multi-domain plan should split"
        );
    }

    #[test]
    fn test_flatten_to_atomic_merges_read_steps() {
        let mut plan = Plan::new("测试", "测试").with_max_depth(3);
        plan.depth = 3;
        plan.steps = vec![
            PlanStep::new("读取 auth.rs", Some("file_read".into())),
            PlanStep::new("搜索 TODO", Some("grep".into())),
            PlanStep::new("列出文件", Some("glob".into())),
            PlanStep::new("修改 auth.rs", Some("file_edit".into())),
            PlanStep::new("创建新文件", Some("file_write".into())),
        ];
        for step in &mut plan.steps {
            step.depth = 3;
        }

        WorkflowPlanner::flatten_to_atomic(&mut plan);

        // Read/grep/glob should be merged into 1 step
        assert_eq!(
            plan.steps.len(),
            3,
            "Expected 3 atomic steps: 1 read + 2 write/edit"
        );

        // First step should be combined read
        assert_eq!(plan.steps[0].tool, Some("file_read".into()));
        assert!(plan.steps[0].description.contains("合并读取/搜索"));

        // Write/edit should remain atomic
        assert_eq!(plan.steps[1].tool, Some("file_edit".into()));
        assert_eq!(plan.steps[2].tool, Some("file_write".into()));

        // All steps should have depth == max_depth
        for step in &plan.steps {
            assert_eq!(step.depth, 3, "Atomic step depth should be max_depth");
        }
    }

    #[test]
    fn test_flatten_to_atomic_adds_read_dependency() {
        let mut plan = Plan::new("测试", "测试").with_max_depth(3);
        plan.depth = 3;
        plan.steps = vec![
            PlanStep::new("读取 auth.rs", Some("file_read".into())),
            PlanStep::new("修改 auth.rs", Some("file_edit".into())),
        ];
        for step in &mut plan.steps {
            step.depth = 3;
        }
        // Clear initial dependencies
        plan.steps[1].dependent_step_indices.clear();

        WorkflowPlanner::flatten_to_atomic(&mut plan);

        // Edit step should now depend on combined read step (index 0)
        assert!(
            plan.steps[1].dependent_step_indices.contains(&0),
            "Edit step should depend on read step after flattening"
        );
    }

    #[test]
    fn test_flatten_to_atomic_no_read_steps() {
        let mut plan = Plan::new("测试", "测试").with_max_depth(3);
        plan.depth = 3;
        plan.steps = vec![
            PlanStep::new("修改 auth.rs", Some("file_edit".into())),
            PlanStep::new("创建新文件", Some("file_write".into())),
        ];
        for step in &mut plan.steps {
            step.depth = 3;
        }

        WorkflowPlanner::flatten_to_atomic(&mut plan);

        // No read steps to merge, should keep both
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].tool, Some("file_edit".into()));
        assert_eq!(plan.steps[1].tool, Some("file_write".into()));
    }
}
