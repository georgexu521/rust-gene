//! AI 权重分析器
//!
//! 支持两种模式：
//! - LLM 模式：调用 QueryEngine 让 LLM 根据项目上下文分析优先级（推荐）
//! - 启发式模式：使用关键词规则匹配（LLM 不可用时的 fallback）

use crate::engine::QueryEngine;
use crate::internal::ai_analyzer::heuristics::{HeuristicResult, WeightHeuristics};
use priority_core::weight_engine::types::{Project, Task, TaskId, Weight};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

/// AI 权重分析结果
#[derive(Debug, Clone)]
pub struct AiAnalysisResult {
    /// 任务ID
    pub task_id: TaskId,
    /// 建议权重
    pub suggested_weight: Weight,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f64,
    /// 启发式分析结果（如果有）
    pub heuristics: Option<HeuristicResult>,
    /// LLM 分析结果（如果有）
    pub llm_analysis: Option<LlmTaskAnalysis>,
    /// 分析总结
    pub summary: String,
    /// 建议
    pub recommendations: Vec<String>,
}

/// LLM 对单个任务的分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTaskAnalysis {
    pub task_id: String,
    pub weight: f64,
    pub priority_level: String,
    pub reasoning: String,
    pub blocks: Vec<String>,
    pub blocked_by: Vec<String>,
}

/// LLM 完整分析响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAnalysisResponse {
    pub tasks: Vec<LlmTaskAnalysis>,
    pub overall_strategy: String,
    pub suggested_order: Vec<String>,
}

impl AiAnalysisResult {
    pub fn new(task_id: TaskId) -> Self {
        Self {
            task_id,
            suggested_weight: Weight::new(0.5),
            confidence: 0.0,
            heuristics: None,
            llm_analysis: None,
            summary: String::new(),
            recommendations: Vec::new(),
        }
    }

    /// 格式化输出
    pub fn format(&self) -> String {
        let mut output = String::new();

        let source = if self.llm_analysis.is_some() {
            "🤖 AI"
        } else {
            "📊 规则"
        };

        output.push_str(&format!("{} 分析结果: {}\n", source, self.task_id));
        output.push_str(&format!(
            "建议权重: {:.1}%\n",
            self.suggested_weight.as_percentage()
        ));
        output.push_str(&format!("置信度: {:.0}%\n", self.confidence * 100.0));

        if let Some(ref llm) = self.llm_analysis {
            output.push_str(&format!("优先级: {}\n", llm.priority_level));
            output.push_str(&format!("分析: {}\n", llm.reasoning));
            if !llm.blocks.is_empty() {
                output.push_str(&format!("阻塞了: {}\n", llm.blocks.join(", ")));
            }
            if !llm.blocked_by.is_empty() {
                output.push_str(&format!("被阻塞于: {}\n", llm.blocked_by.join(", ")));
            }
        }

        if !self.summary.is_empty() {
            output.push_str(&format!("\n{}\n", self.summary));
        }

        if !self.recommendations.is_empty() {
            output.push_str("\n💡 建议:\n");
            for rec in &self.recommendations {
                output.push_str(&format!("  • {}\n", rec));
            }
        }

        output
    }
}

/// 项目上下文信息（喂给 LLM 的数据）
#[derive(Debug, Clone, Default)]
pub struct ProjectContext {
    /// Git 最近提交
    pub recent_commits: Vec<String>,
    /// Open Issues
    pub open_issues: Vec<String>,
    /// 失败的测试
    pub failed_tests: Vec<String>,
    /// TODO/FIXME
    pub todos: Vec<String>,
    /// CI 状态
    pub ci_status: Option<String>,
    /// 自由文本描述
    pub description: Option<String>,
}

impl ProjectContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// 从项目目录自动收集上下文
    pub async fn collect_from_dir(dir: &str) -> Self {
        let mut ctx = Self::new();

        // 收集 git log
        if let Ok(output) = tokio::process::Command::new("git")
            .args(["log", "--oneline", "-20"])
            .current_dir(dir)
            .output()
            .await
        {
            ctx.recent_commits = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
        }

        // 收集 TODO/FIXME
        if let Ok(output) = tokio::process::Command::new("grep")
            .args([
                "-rn",
                "--include=*.rs",
                "--include=*.py",
                "--include=*.js",
                "--include=*.ts",
                "-E",
                "TODO|FIXME|HACK|XXX",
                ".",
            ])
            .current_dir(dir)
            .output()
            .await
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            ctx.todos = stdout.lines().take(30).map(|s| s.to_string()).collect();
        }

        ctx
    }

    /// 格式化为 LLM 可读的文本
    fn format_for_llm(&self, tasks: &[&Task]) -> String {
        let mut ctx = String::new();

        // 任务列表
        ctx.push_str("## 当前任务列表\n");
        for (i, task) in tasks.iter().enumerate() {
            ctx.push_str(&format!("{}. [{}] {}", i + 1, task.id, task.name));
            if !task.description.is_empty() {
                ctx.push_str(&format!(" - {}", task.description));
            }
            if !task.children.is_empty() {
                ctx.push_str(&format!(" ({} 个子任务)", task.children.len()));
            }
            if !task.dependencies.is_empty() {
                ctx.push_str(&format!(" (依赖: {:?})", task.dependencies));
            }
            ctx.push('\n');
        }

        // Git 活动
        if !self.recent_commits.is_empty() {
            ctx.push_str("\n## 最近 Git 提交\n");
            for commit in &self.recent_commits {
                ctx.push_str(&format!("- {}\n", commit));
            }
        }

        // TODO
        if !self.todos.is_empty() {
            ctx.push_str("\n## TODO/FIXME\n");
            for todo in self.todos.iter().take(15) {
                ctx.push_str(&format!("- {}\n", todo));
            }
        }

        // Issues
        if !self.open_issues.is_empty() {
            ctx.push_str("\n## Open Issues\n");
            for issue in &self.open_issues {
                ctx.push_str(&format!("- {}\n", issue));
            }
        }

        // 额外描述
        if let Some(ref desc) = self.description {
            ctx.push_str(&format!("\n## 项目描述\n{}\n", desc));
        }

        ctx
    }
}

/// AI 权重分析器
pub struct AiWeightAnalyzer {
    heuristics: WeightHeuristics,
    /// LLM 引擎（可选）
    query_engine: Option<Arc<QueryEngine>>,
}

impl AiWeightAnalyzer {
    /// 创建新的分析器（仅启发式）
    pub fn new() -> Self {
        Self {
            heuristics: WeightHeuristics::new(),
            query_engine: None,
        }
    }

    /// 创建带 LLM 引擎的分析器
    pub fn with_engine(engine: Arc<QueryEngine>) -> Self {
        Self {
            heuristics: WeightHeuristics::new(),
            query_engine: Some(engine),
        }
    }

    /// 分析项目并自动分配权重
    /// 优先使用 LLM，不可用时回退到启发式
    pub async fn analyze_project(
        &self,
        project: &Project,
        context: Option<&ProjectContext>,
    ) -> Vec<AiAnalysisResult> {
        if let Some(ref engine) = self.query_engine {
            // 使用 LLM 分析
            match self.analyze_with_llm(project, context, engine).await {
                Ok(results) => return results,
                Err(e) => {
                    warn!("LLM analysis failed, falling back to heuristics: {}", e);
                }
            }
        }

        // Fallback: 启发式分析
        self.analyze_with_heuristics(project)
    }

    /// 使用 LLM 分析项目优先级
    async fn analyze_with_llm(
        &self,
        project: &Project,
        context: Option<&ProjectContext>,
        engine: &QueryEngine,
    ) -> anyhow::Result<Vec<AiAnalysisResult>> {
        let tasks: Vec<&Task> = project.all_tasks();
        if tasks.is_empty() {
            return Ok(Vec::new());
        }

        // 构建上下文
        let ctx = context.cloned().unwrap_or_default();
        let ctx_text = ctx.format_for_llm(&tasks);

        // 构建 prompt
        let prompt = format!(
            r#"你是一个项目优先级分析专家。你的任务是分析项目中所有任务，并为每个任务分配权重（优先级）。

{ctx_text}

## 分析要求

请分析以上所有任务，考虑以下因素：
1. **业务价值**：这个任务对项目成功有多重要？
2. **依赖关系**：哪些任务阻塞了其他任务？
3. **紧急程度**：是否有截止日期或紧迫需求？
4. **复杂度**：任务的工作量和难度
5. **当前状态**：从 Git 活动看，哪些模块正在活跃开发？

## 输出格式

请严格输出以下 JSON 格式（不要输出其他内容）：

```json
{{
  "tasks": [
    {{
      "task_id": "任务ID",
      "weight": 0.85,
      "priority_level": "P0-关键",
      "reasoning": "为什么这个权重",
      "blocks": ["被此任务阻塞的任务ID"],
      "blocked_by": ["此任务依赖的任务ID"]
    }}
  ],
  "overall_strategy": "整体执行策略描述",
  "suggested_order": ["按推荐执行顺序排列的任务ID"]
}}
```

注意：
- weight 范围 0.0-1.0，所有任务的 weight 之和不需要等于 1
- priority_level 可选: P0-关键, P1-高, P2-中, P3-低
- 确保每个任务都有分析结果"#
        );

        info!("Sending {} tasks to LLM for priority analysis", tasks.len());

        // 调用 LLM
        let response = engine.query_simple(&prompt).await?;

        // 解析 JSON（从 markdown code block 中提取）
        let json_str = extract_json(&response)?;
        let analysis: LlmAnalysisResponse = serde_json::from_str(json_str).map_err(|e| {
            anyhow::anyhow!("Failed to parse LLM response: {}. Raw: {}", e, json_str)
        })?;

        info!(
            "LLM analysis complete: {} tasks analyzed, strategy: {}",
            analysis.tasks.len(),
            &analysis.overall_strategy[..analysis.overall_strategy.len().min(50)]
        );

        // 转换为 AiAnalysisResult
        let mut results = Vec::new();
        for llm_task in &analysis.tasks {
            let task_id = TaskId::new(&llm_task.task_id);
            let mut result = AiAnalysisResult::new(task_id.clone());
            result.suggested_weight = Weight::new(llm_task.weight);
            result.confidence = 0.85; // LLM 分析置信度较高
            result.llm_analysis = Some(llm_task.clone());
            result.summary = format!("{}: {}", llm_task.priority_level, llm_task.reasoning);

            if !llm_task.blocks.is_empty() {
                result.recommendations.push(format!(
                    "此任务阻塞了 {} 个其他任务，建议优先完成",
                    llm_task.blocks.len()
                ));
            }

            results.push(result);
        }

        // 补充 LLM 没覆盖到的任务（用启发式）
        let analyzed_ids: std::collections::HashSet<String> =
            analysis.tasks.iter().map(|t| t.task_id.clone()).collect();

        for task in &tasks {
            if !analyzed_ids.contains(&task.id.0) {
                let mut result = self.analyze_task_with_heuristics(task);
                result.summary = format!("（LLM 未覆盖，规则分析）{}", result.summary);
                result.confidence *= 0.6; // 降低置信度
                results.push(result);
            }
        }

        // 按权重排序
        results.sort_by(|a, b| {
            b.suggested_weight
                .value()
                .partial_cmp(&a.suggested_weight.value())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    /// 启发式分析（无 LLM fallback）
    fn analyze_with_heuristics(&self, project: &Project) -> Vec<AiAnalysisResult> {
        let mut results = Vec::new();

        for task in project.all_tasks() {
            results.push(self.analyze_task_with_heuristics(task));
        }

        results.sort_by(|a, b| {
            b.suggested_weight
                .value()
                .partial_cmp(&a.suggested_weight.value())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// 分析单个任务（启发式）
    fn analyze_task_with_heuristics(&self, task: &Task) -> AiAnalysisResult {
        let mut result = AiAnalysisResult::new(task.id.clone());

        let heuristic_result = self.heuristics.analyze(task);
        result.suggested_weight = Weight::new(heuristic_result.combined_weight);
        result.confidence = self.calculate_confidence(&heuristic_result);
        result.summary = self.generate_summary(task, &heuristic_result);
        result.recommendations = self.generate_recommendations(task, &heuristic_result);
        result.heuristics = Some(heuristic_result);

        result
    }

    /// 建议权重分配
    pub async fn suggest_weights(
        &self,
        project: &Project,
        context: Option<&ProjectContext>,
    ) -> Vec<(TaskId, Weight, String)> {
        let results = self.analyze_project(project, context).await;

        results
            .into_iter()
            .map(|r| {
                let reasoning = if let Some(ref llm) = r.llm_analysis {
                    llm.reasoning.clone()
                } else if let Some(ref h) = r.heuristics {
                    if h.reasoning.is_empty() {
                        "基于规则分析".to_string()
                    } else {
                        h.reasoning.join("; ")
                    }
                } else {
                    "无分析数据".to_string()
                };
                (r.task_id, r.suggested_weight, reasoning)
            })
            .collect()
    }

    fn calculate_confidence(&self, heuristics: &HeuristicResult) -> f64 {
        let scores = [
            heuristics.keyword_score,
            heuristics.complexity_score,
            heuristics.urgency_score,
            heuristics.blocking_score,
            heuristics.description_quality_score,
        ];

        let avg = scores.iter().sum::<f64>() / scores.len() as f64;
        let variance = scores.iter().map(|s| (s - avg).powi(2)).sum::<f64>() / scores.len() as f64;

        let consistency = 1.0 - variance.sqrt();
        (avg * 0.6 + consistency * 0.4).clamp(0.0, 1.0)
    }

    fn generate_summary(&self, _task: &Task, heuristics: &HeuristicResult) -> String {
        let mut parts = Vec::new();

        let weight_desc = if heuristics.combined_weight > 0.8 {
            "🔴 极高优先级"
        } else if heuristics.combined_weight > 0.6 {
            "🟠 高优先级"
        } else if heuristics.combined_weight > 0.4 {
            "🟡 中等优先级"
        } else if heuristics.combined_weight > 0.2 {
            "🟢 低优先级"
        } else {
            "⚪ 极低优先级"
        };
        parts.push(weight_desc.to_string());

        let mut factors = Vec::new();
        if heuristics.keyword_score > 0.5 {
            factors.push("关键词匹配度高".to_string());
        }
        if heuristics.urgency_score > 0.5 {
            factors.push("紧急度高".to_string());
        }
        if heuristics.blocking_score > 0.5 {
            factors.push("阻塞其他任务".to_string());
        }
        if heuristics.complexity_score > 0.5 {
            factors.push("复杂度较高".to_string());
        }

        if !factors.is_empty() {
            parts.push(format!("主要因素: {}", factors.join("、")));
        }

        parts.join("\n")
    }

    fn generate_recommendations(&self, task: &Task, heuristics: &HeuristicResult) -> Vec<String> {
        let mut recs = Vec::new();

        if heuristics.description_quality_score < 0.3 {
            recs.push("建议添加更详细的任务描述".to_string());
        }
        if heuristics.complexity_score > 0.7 && task.children.is_empty() {
            recs.push("任务较复杂，建议拆分为子任务".to_string());
        }
        if heuristics.blocking_score > 0.7 {
            recs.push("这是关键路径任务，建议优先处理".to_string());
        }
        if heuristics.urgency_score > 0.7 {
            recs.push("紧急任务，建议立即开始".to_string());
        }

        recs
    }
}

impl Default for AiWeightAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// 从 LLM 响应中提取 JSON（处理 markdown code block）
fn extract_json(response: &str) -> anyhow::Result<&str> {
    // 尝试直接解析
    if serde_json::from_str::<serde_json::Value>(response.trim()).is_ok() {
        return Ok(response.trim());
    }

    // 尝试从 ```json ... ``` 中提取
    if let Some(start) = response.find("```json") {
        let after = &response[start + 7..];
        if let Some(end) = after.find("```") {
            return Ok(after[..end].trim());
        }
    }

    // 尝试从 ``` ... ``` 中提取
    if let Some(start) = response.find("```") {
        let after = &response[start + 3..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            if inner.starts_with('{') {
                return Ok(inner);
            }
        }
    }

    // 尝试找到 JSON 对象
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            if end > start {
                return Ok(&response[start..=end]);
            }
        }
    }

    Err(anyhow::anyhow!(
        "Could not extract JSON from LLM response: {}",
        &response[..response.len().min(200)]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_code_block() {
        let response = r#"Here is the analysis:

```json
{"tasks": [], "overall_strategy": "test", "suggested_order": []}
```

Hope this helps!"#;

        let json = extract_json(response).unwrap();
        let parsed: LlmAnalysisResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.overall_strategy, "test");
    }

    #[test]
    fn test_extract_json_direct() {
        let response = r#"{"tasks": [], "overall_strategy": "direct", "suggested_order": []}"#;
        let json = extract_json(response).unwrap();
        let parsed: LlmAnalysisResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.overall_strategy, "direct");
    }

    #[test]
    fn test_analyze_with_heuristics() {
        let analyzer = AiWeightAnalyzer::new();
        let task = Task::new("auth", "实现核心认证系统")
            .with_description("这是项目的基础，所有其他功能都依赖这个模块。必须先完成。");

        let result = analyzer.analyze_task_with_heuristics(&task);
        assert!(result.suggested_weight.value() > 0.5);
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_project_context_format() {
        let mut ctx = ProjectContext::new();
        ctx.recent_commits.push("abc123 feat: add auth".to_string());
        ctx.todos
            .push("src/main.rs:10 TODO: add error handling".to_string());

        let task = Task::new("t1", "Login");
        let tasks = vec![&task];
        let formatted = ctx.format_for_llm(&tasks);

        assert!(formatted.contains("## 当前任务列表"));
        assert!(formatted.contains("## 最近 Git 提交"));
        assert!(formatted.contains("## TODO/FIXME"));
    }
}
