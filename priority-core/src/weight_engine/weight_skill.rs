//! 权重分析 Skill
//!
//! 将原有的权重系统作为 Skill 集成到新架构

use crate::skills::{Skill, SkillMeta};
use crate::weight_engine::calculator::WeightCalculator;
use crate::weight_engine::types::Project;
use std::path::PathBuf;

/// 创建权重分析 Skill（程序化注册，不从文件加载）
pub fn weight_analysis_skill() -> Skill {
    Skill {
        meta: SkillMeta {
            name: "weight_analysis".to_string(),
            description: "Analyze task priorities using weighted priority system".to_string(),
            version: "1.0.0".to_string(),
            author: "priority-agent".to_string(),
            triggers: vec![
                "weight".to_string(),
                "priority".to_string(),
                "analyze".to_string(),
            ],
            required_env: Vec::new(),
        },
        content: r#"## Weight Analysis Skill

Use the WeightCalculator to analyze task priorities.

### Steps:
1. Load the project task list
2. Calculate absolute weights for each task
3. Sort by weight (highest first)
4. Return the top tasks with their weight percentages

### Commands:
- analyze: Analyze all tasks and return weight distribution
- next_task: Get the next recommended task based on weight
- calculate_progress: Calculate project completion progress"#
            .to_string(),
        raw_content: String::new(),
        skill_dir: PathBuf::from("builtin"),
        modified: None,
    }
}

/// 权重分析工具 - 集成到工具系统
pub struct WeightAnalysisTool;

impl WeightAnalysisTool {
    pub fn new() -> Self {
        Self
    }

    /// 分析项目权重
    pub fn analyze_project(&self, project: &Project) -> WeightAnalysisResult {
        let calculator = WeightCalculator::new();
        let weights = calculator.calculate_absolute_weights(project);

        let mut weight_list: Vec<(_, _)> = weights.iter().collect();
        weight_list.sort_by(|a, b| {
            b.1.value()
                .partial_cmp(&a.1.value())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        WeightAnalysisResult {
            project_name: project.name.clone(),
            total_tasks: weights.len(),
            weights: weight_list
                .into_iter()
                .map(|(id, w)| (id.0.clone(), w.value()))
                .collect(),
            next_task: calculator.next_task(project).map(|t| t.task_name),
        }
    }
}

/// 权重分析结果
#[derive(Debug, Clone)]
pub struct WeightAnalysisResult {
    pub project_name: String,
    pub total_tasks: usize,
    pub weights: Vec<(String, f64)>,
    pub next_task: Option<String>,
}

impl WeightAnalysisResult {
    pub fn format(&self) -> String {
        let mut output = format!(
            "权重分析报告: {}\n总任务数: {}\n\n",
            self.project_name, self.total_tasks
        );

        output.push_str("任务权重分布 (前 10):\n");
        for (task_id, weight) in self.weights.iter().take(10) {
            output.push_str(&format!("  {}: {:.1}%\n", task_id, weight * 100.0));
        }

        if let Some(ref next) = self.next_task {
            output.push_str(&format!("\n🎯 推荐任务: {}\n", next));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_skill() {
        let skill = weight_analysis_skill();
        assert_eq!(skill.meta.name, "weight_analysis");
        assert!(skill.meta.triggers.contains(&"weight".to_string()));
        assert!(skill.content.contains("WeightCalculator"));
    }
}
