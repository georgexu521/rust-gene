//! 任务分析器 - 分析任务结构和关键路径

use crate::weight_engine::types::{Project, Task, TaskId, Weight};
use crate::task_analyzer::dependency_graph::DependencyGraph;

/// 关键路径
#[derive(Debug, Clone)]
pub struct CriticalPath {
    /// 路径上的任务ID列表
    pub task_ids: Vec<TaskId>,
    /// 路径总权重
    pub total_weight: Weight,
    /// 路径长度（任务数）
    pub length: usize,
}

impl CriticalPath {
    pub fn new(task_ids: Vec<TaskId>, total_weight: f64) -> Self {
        let length = task_ids.len();
        Self {
            task_ids,
            total_weight: Weight::new(total_weight),
            length,
        }
    }
}

/// 分析结果
#[derive(Debug)]
pub struct AnalysisResult {
    /// 任务总数
    pub total_tasks: usize,
    /// 最大深度（层级数）
    pub max_depth: usize,
    /// 平均分支因子
    pub avg_branching_factor: f64,
    /// 关键路径
    pub critical_paths: Vec<CriticalPath>,
    /// 孤立任务（无依赖也无被依赖）
    pub isolated_tasks: Vec<TaskId>,
    /// 瓶颈任务（被多个任务依赖）
    pub bottleneck_tasks: Vec<(TaskId, usize)>,
    /// 风险评分 (0-1)
    pub risk_score: f64,
}

/// 任务分析器
pub struct TaskAnalyzer {
    dependency_graph: DependencyGraph,
}

impl TaskAnalyzer {
    pub fn new() -> Self {
        Self {
            dependency_graph: DependencyGraph::new(),
        }
    }

    /// 分析项目
    pub fn analyze(&mut self, project: &Project) -> AnalysisResult {
        self.dependency_graph = DependencyGraph::from_tasks(&project.root_tasks);

        let all_tasks = project.all_tasks();
        let total_tasks = all_tasks.len();

        // 计算最大深度
        let max_depth = self.calculate_max_depth(&all_tasks);

        // 计算平均分支因子
        let avg_branching = self.calculate_avg_branching(&all_tasks);

        // 找出关键路径
        let critical_paths = self.find_critical_paths(&all_tasks);

        // 找出孤立任务
        let isolated_tasks = self.find_isolated_tasks(&all_tasks);

        // 找出瓶颈任务
        let bottleneck_tasks = self.find_bottleneck_tasks(&all_tasks);

        // 计算风险评分
        let risk_score = self.calculate_risk_score(
            total_tasks,
            max_depth,
            &critical_paths,
            &bottleneck_tasks,
        );

        AnalysisResult {
            total_tasks,
            max_depth,
            avg_branching_factor: avg_branching,
            critical_paths,
            isolated_tasks,
            bottleneck_tasks,
            risk_score,
        }
    }

    /// 计算任务树的最大深度
    fn calculate_max_depth(&self, tasks: &[&Task]) -> usize {
        tasks
            .iter()
            .map(|t| self.task_depth(t))
            .max()
            .unwrap_or(0)
    }

    fn task_depth(&self, task: &Task) -> usize {
        if task.children.is_empty() {
            1
        } else {
            1 + task
                .children
                .iter()
                .map(|c| self.task_depth(c))
                .max()
                .unwrap_or(0)
        }
    }

    /// 计算平均分支因子
    fn calculate_avg_branching(&self, tasks: &[&Task]) -> f64 {
        let mut total_branches = 0;
        let mut task_count = 0;

        for task in tasks {
            if !task.children.is_empty() {
                total_branches += task.children.len();
                task_count += 1;
            }
        }

        if task_count == 0 {
            0.0
        } else {
            total_branches as f64 / task_count as f64
        }
    }

    /// 查找关键路径
    fn find_critical_paths(&self, tasks: &[&Task]) -> Vec<CriticalPath> {
        let mut paths = Vec::new();

        for task in tasks {
            let path = self.find_heaviest_path(task);
            if !path.task_ids.is_empty() {
                paths.push(path);
            }
        }

        // 按权重排序
        paths.sort_by(|a, b| {
            b.total_weight
                .value()
                .partial_cmp(&a.total_weight.value())
                .unwrap()
        });

        // 只返回前3条关键路径
        paths.truncate(3);
        paths
    }

    /// 找到从某个任务开始的最重路径
    fn find_heaviest_path(&self, task: &Task) -> CriticalPath {
        if task.children.is_empty() {
            return CriticalPath::new(
                vec![task.id.clone()],
                task.local_weight.value(),
            );
        }

        // 找到权重最大的子路径
        let heaviest_child = task
            .children
            .iter()
            .max_by(|a, b| {
                a.local_weight
                    .value()
                    .partial_cmp(&b.local_weight.value())
                    .unwrap()
            })
            .unwrap();

        let mut child_path = self.find_heaviest_path(heaviest_child);
        let total_weight = task.local_weight.value() + child_path.total_weight.value();

        let mut path_ids = vec![task.id.clone()];
        path_ids.append(&mut child_path.task_ids);

        CriticalPath::new(path_ids, total_weight)
    }

    /// 找出孤立任务
    fn find_isolated_tasks(&self, tasks: &[&Task]) -> Vec<TaskId> {
        tasks
            .iter()
            .filter(|t| t.dependencies.is_empty() && self.dependency_graph.get_dependents(&t.id).is_none())
            .map(|t| t.id.clone())
            .collect()
    }

    /// 找出瓶颈任务（被多个任务依赖）
    fn find_bottleneck_tasks(&self, tasks: &[&Task]) -> Vec<(TaskId, usize)> {
        let mut bottlenecks: Vec<(TaskId, usize)> = tasks
            .iter()
            .filter_map(|t| {
                self.dependency_graph
                    .get_dependents(&t.id)
                    .map(|deps| (t.id.clone(), deps.len()))
            })
            .filter(|(_, count)| *count > 1)
            .collect();

        // 按依赖数排序
        bottlenecks.sort_by(|a, b| b.1.cmp(&a.1));
        bottlenecks
    }

    /// 计算项目风险评分
    fn calculate_risk_score(
        &self,
        total_tasks: usize,
        max_depth: usize,
        critical_paths: &[CriticalPath],
        bottleneck_tasks: &[(TaskId, usize)],
    ) -> f64 {
        let mut score: f64 = 0.0;

        // 任务数量风险
        if total_tasks > 50 {
            score += 0.2;
        } else if total_tasks > 20 {
            score += 0.1;
        }

        // 深度风险
        if max_depth > 5 {
            score += 0.2;
        } else if max_depth > 3 {
            score += 0.1;
        }

        // 关键路径风险
        if let Some(critical) = critical_paths.first() {
            if critical.length > 5 {
                score += 0.2;
            }
        }

        // 瓶颈风险
        if !bottleneck_tasks.is_empty() {
            let max_bottleneck = bottleneck_tasks[0].1;
            if max_bottleneck > 5 {
                score += 0.3;
            } else if max_bottleneck > 3 {
                score += 0.2;
            } else {
                score += 0.1;
            }
        }

        score.min(1.0)
    }

    /// 生成分析报告
    pub fn generate_report(&self, result: &AnalysisResult) -> String {
        let mut report = String::new();

        report.push_str("# 项目分析报告\n\n");

        report.push_str(&format!("## 概览\n"));
        report.push_str(&format!("- 任务总数: {}\n", result.total_tasks));
        report.push_str(&format!("- 最大深度: {}\n", result.max_depth));
        report.push_str(&format!(
            "- 平均分支因子: {:.2}\n",
            result.avg_branching_factor
        ));
        report.push_str(&format!(
            "- 风险评分: {:.0}%\n",
            result.risk_score * 100.0
        ));

        report.push_str("\n## 关键路径\n");
        for (i, path) in result.critical_paths.iter().enumerate() {
            report.push_str(&format!(
                "{}. 权重: {:.1}%, 长度: {}\n",
                i + 1,
                path.total_weight.as_percentage(),
                path.length
            ));
            report.push_str("   路径: ");
            for (j, task_id) in path.task_ids.iter().enumerate() {
                if j > 0 {
                    report.push_str(" -> ");
                }
                report.push_str(&task_id.to_string());
            }
            report.push('\n');
        }

        if !result.bottleneck_tasks.is_empty() {
            report.push_str("\n## 瓶颈任务\n");
            for (task_id, count) in &result.bottleneck_tasks {
                report.push_str(&format!("- {} (被 {} 个任务依赖)\n", task_id, count));
            }
        }

        if !result.isolated_tasks.is_empty() {
            report.push_str("\n## 孤立任务\n");
            for task_id in &result.isolated_tasks {
                report.push_str(&format!("- {}\n", task_id));
            }
        }

        report
    }
}

impl Default for TaskAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_simple_project() {
        let mut project = Project::new("test", "Test Project");

        let mut task_a = Task::new("a", "Task A").with_weight(0.5);
        let task_a1 = Task::new("a1", "Subtask A1").with_weight(0.3);
        let task_a2 = Task::new("a2", "Subtask A2").with_weight(0.2);

        task_a.add_child(task_a1);
        task_a.add_child(task_a2);

        project.add_task(task_a);

        let mut analyzer = TaskAnalyzer::new();
        let result = analyzer.analyze(&project);

        assert_eq!(result.total_tasks, 3);
        assert_eq!(result.max_depth, 2);
    }
}
