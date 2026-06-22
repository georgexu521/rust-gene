//! Priority Agent 特色功能
//!
//! 智能任务分配、优先级调度

use crate::agent::AgentManager;
use priority_core::weight_engine::types::{Project, Task};
use priority_core::weight_engine::{WeightAnalysisResult, WeightAnalysisTool};
use std::sync::Arc;

/// 智能任务调度器
pub struct PriorityScheduler {
    weight_analyzer: WeightAnalysisTool,
    agent_manager: Option<Arc<AgentManager>>,
}

impl PriorityScheduler {
    pub fn new() -> Self {
        Self {
            weight_analyzer: WeightAnalysisTool::new(),
            agent_manager: None,
        }
    }

    pub fn with_agent_manager(mut self, manager: Arc<AgentManager>) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    /// 为任务推荐最佳执行者
    pub async fn recommend_executor(&self, task: &Task) -> ExecutorRecommendation {
        // 分析任务特征
        let task_type = self.classify_task(task);

        // 根据类型推荐执行者
        let recommended = match task_type {
            TaskType::Analysis => "parent",
            TaskType::Code => "sub-agent",
            TaskType::Research => "sub-agent",
            TaskType::Test => "parallel-agents",
        };

        ExecutorRecommendation {
            task_name: task.name.clone(),
            task_type,
            recommended_executor: recommended.to_string(),
            reasoning: format!(
                "Task '{}' classified as {:?}, best handled by {}",
                task.name, task_type, recommended
            ),
        }
    }

    /// 任务分类
    fn classify_task(&self, task: &Task) -> TaskType {
        let name = task.name.to_lowercase();
        let _desc = task.description.to_lowercase();

        if name.contains("test") || name.contains("测试") {
            TaskType::Test
        } else if name.contains("分析") || name.contains("analyze") || name.contains("review") {
            TaskType::Analysis
        } else if name.contains("研究") || name.contains("research") || name.contains("调查") {
            TaskType::Research
        } else {
            TaskType::Code
        }
    }

    /// 分析项目并制定执行计划
    pub fn create_execution_plan(&self, project: &Project) -> ExecutionPlan {
        let analysis = self.weight_analyzer.analyze_project(project);

        // 按权重排序任务
        let mut tasks: Vec<(&Task, f64)> = project
            .all_tasks()
            .into_iter()
            .map(|t| (t, self.get_task_weight(t, &analysis)))
            .collect();

        tasks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        ExecutionPlan {
            project_name: project.name.clone(),
            total_tasks: tasks.len(),
            high_priority_tasks: tasks.iter().take(3).map(|(t, _)| t.name.clone()).collect(),
            estimated_completion: self.estimate_completion(project),
        }
    }

    fn get_task_weight(&self, task: &Task, analysis: &WeightAnalysisResult) -> f64 {
        analysis
            .weights
            .iter()
            .find(|(id, _)| id.as_str() == task.id.0.as_str())
            .map(|(_, w)| *w)
            .unwrap_or(0.0)
    }

    fn estimate_completion(&self, project: &Project) -> String {
        let progress = project.overall_progress();
        let remaining = 1.0 - progress;

        // 粗略估计：假设每天完成 10% 的工作量
        let days_remaining = (remaining / 0.1).ceil() as i32;

        if days_remaining <= 1 {
            "今天内".to_string()
        } else if days_remaining <= 3 {
            "3天内".to_string()
        } else if days_remaining <= 7 {
            "本周内".to_string()
        } else {
            format!("约 {} 天", days_remaining)
        }
    }
}

impl Default for PriorityScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// 执行者推荐
#[derive(Debug, Clone)]
pub struct ExecutorRecommendation {
    pub task_name: String,
    pub task_type: TaskType,
    pub recommended_executor: String,
    pub reasoning: String,
}

/// 任务类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    Analysis,
    Code,
    Research,
    Test,
}

/// 执行计划
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub project_name: String,
    pub total_tasks: usize,
    pub high_priority_tasks: Vec<String>,
    pub estimated_completion: String,
}

impl ExecutionPlan {
    pub fn format(&self) -> String {
        format!(
            r#"执行计划: {}
=============
总任务数: {}
高优先级任务: {}
预计完成: {}
"#,
            self.project_name,
            self.total_tasks,
            self.high_priority_tasks.join(", "),
            self.estimated_completion
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler() {
        let scheduler = PriorityScheduler::new();

        let project = Project::new("test", "Test Project");
        let plan = scheduler.create_execution_plan(&project);

        assert_eq!(plan.project_name, "Test Project");
    }
}
