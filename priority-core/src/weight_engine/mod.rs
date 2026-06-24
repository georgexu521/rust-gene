//! Priority weighting models and analysis helpers.

pub mod calculator;
pub mod types;

pub use calculator::WeightCalculator;
pub use types::{Task, TaskId, Weight};

/// Computes a deterministic weight summary for a project.
#[derive(Debug, Clone, Default)]
pub struct WeightAnalysisTool;

impl WeightAnalysisTool {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze_project(&self, project: &types::Project) -> WeightAnalysisResult {
        let calculator = WeightCalculator::new();
        let absolute_weights = calculator.calculate_absolute_weights(project);
        let mut weights: Vec<_> = absolute_weights
            .into_iter()
            .map(|(task_id, weight)| (task_id.0, weight.value()))
            .collect();

        weights.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.cmp(&right.0))
        });

        WeightAnalysisResult {
            project_name: project.name.clone(),
            total_tasks: project.all_tasks().len(),
            weights,
            next_task: calculator.next_task(project).map(|task| task.task_name),
        }
    }
}

/// Deterministic project weight analysis result.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightAnalysisResult {
    pub project_name: String,
    pub total_tasks: usize,
    pub weights: Vec<(String, f64)>,
    pub next_task: Option<String>,
}

impl WeightAnalysisResult {
    pub fn weight_for_task(&self, task_id: &str) -> Option<f64> {
        self.weights
            .iter()
            .find(|(id, _)| id == task_id)
            .map(|(_, weight)| *weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weight_engine::types::Project;

    #[test]
    fn weight_analysis_returns_calculated_weights() {
        let mut project = Project::new("project", "Project");
        project.add_task(Task::new("small", "Small").with_weight(0.25));
        project.add_task(Task::new("large", "Large").with_weight(0.75));

        let result = WeightAnalysisTool::new().analyze_project(&project);

        assert_eq!(result.project_name, "Project");
        assert_eq!(result.total_tasks, 2);
        assert_eq!(result.next_task.as_deref(), Some("Large"));
        assert_eq!(result.weights[0].0, "large");
        assert_eq!(result.weight_for_task("large"), Some(0.75));
        assert_eq!(result.weight_for_task("small"), Some(0.25));
    }
}
