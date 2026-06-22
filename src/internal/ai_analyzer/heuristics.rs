//! 权重启发式算法
//!
//! 基于任务特征计算权重分数

use priority_core::weight_engine::types::Task;

/// 启发式分析结果
#[derive(Debug, Clone)]
pub struct HeuristicResult {
    /// 关键词匹配分数 (0.0 - 1.0)
    pub keyword_score: f64,
    /// 复杂度分数 (0.0 - 1.0)
    pub complexity_score: f64,
    /// 紧急度分数 (0.0 - 1.0)
    pub urgency_score: f64,
    /// 阻塞性分数 (0.0 - 1.0)
    pub blocking_score: f64,
    /// 描述质量分数 (0.0 - 1.0)
    pub description_quality_score: f64,
    /// 综合权重 (0.0 - 1.0)
    pub combined_weight: f64,
    /// 分析理由
    pub reasoning: Vec<String>,
}

impl HeuristicResult {
    pub fn new() -> Self {
        Self {
            keyword_score: 0.0,
            complexity_score: 0.0,
            urgency_score: 0.0,
            blocking_score: 0.0,
            description_quality_score: 0.0,
            combined_weight: 0.0,
            reasoning: Vec::new(),
        }
    }

    /// 添加分析理由
    pub fn add_reason(&mut self, reason: impl Into<String>) {
        self.reasoning.push(reason.into());
    }
}

impl Default for HeuristicResult {
    fn default() -> Self {
        Self::new()
    }
}

/// 权重启发式计算器
pub struct WeightHeuristics;

impl WeightHeuristics {
    /// 创建新的启发式计算器
    pub fn new() -> Self {
        Self
    }

    /// 分析任务并计算启发式分数
    pub fn analyze(&self, task: &Task) -> HeuristicResult {
        let mut result = HeuristicResult::new();

        // 1. 关键词分析
        result.keyword_score = self.analyze_keywords(task);

        // 2. 复杂度分析
        result.complexity_score = self.analyze_complexity(task);

        // 3. 紧急度分析
        result.urgency_score = self.analyze_urgency(task);

        // 4. 阻塞性分析
        result.blocking_score = self.analyze_blocking(task);

        // 5. 描述质量分析
        result.description_quality_score = self.analyze_description_quality(task);

        // 计算综合权重
        result.combined_weight = self.calculate_combined_weight(&result);

        result
    }

    /// 关键词分析
    fn analyze_keywords(&self, task: &Task) -> f64 {
        let name_lower = task.name.to_lowercase();
        let desc_lower = task.description.to_lowercase();
        let text = format!("{} {}", name_lower, desc_lower);

        let mut score: f64 = 0.0;

        // 高优先级关键词
        let high_priority: Vec<(&str, f64)> = vec![
            ("核心", 0.9),
            ("关键", 0.9),
            ("基础", 0.85),
            ("架构", 0.85),
            ("安全", 0.9),
            ("认证", 0.85),
            ("数据库", 0.8),
            ("api", 0.8),
            ("接口", 0.75),
            ("核心功能", 0.95),
            ("mvp", 0.9),
            ("最小可用", 0.9),
            ("阻塞", 0.95),
            ("依赖", 0.8),
            ("必须先", 0.9),
        ];

        // 中等优先级关键词
        let medium_priority: Vec<(&str, f64)> = vec![
            ("功能", 0.6),
            ("模块", 0.55),
            ("页面", 0.5),
            ("ui", 0.5),
            ("界面", 0.5),
            ("样式", 0.4),
            ("优化", 0.55),
            ("改进", 0.5),
            ("重构", 0.6),
        ];

        // 低优先级关键词
        let low_priority: Vec<(&str, f64)> = vec![
            ("文档", 0.3),
            ("注释", 0.25),
            ("测试", 0.35),
            ("日志", 0.3),
            ("调试", 0.25),
            ("修复", 0.4),
            ("bug", 0.4),
            ("样式调整", 0.2),
            ("美化", 0.2),
        ];

        for (keyword, weight) in &high_priority {
            if text.contains(keyword) {
                score += weight;
            }
        }

        for (keyword, weight) in &medium_priority {
            if text.contains(keyword) {
                score += weight * 0.6;
            }
        }

        for (keyword, weight) in &low_priority {
            if text.contains(keyword) {
                score += weight * 0.3;
            }
        }

        // 归一化到 0-1
        f64::min(score, 1.0)
    }

    /// 复杂度分析
    fn analyze_complexity(&self, task: &Task) -> f64 {
        let mut score: f64 = 0.0;
        let text = format!("{} {}", task.name, task.description).to_lowercase();

        // 基于描述长度判断复杂度
        let desc_len = task.description.len();
        if desc_len > 200 {
            score += 0.3;
        } else if desc_len > 100 {
            score += 0.2;
        } else if desc_len > 50 {
            score += 0.1;
        }

        // 复杂度关键词
        let complexity_indicators: Vec<(&str, f64)> = vec![
            ("复杂", 0.3),
            ("困难", 0.3),
            ("挑战", 0.25),
            ("大量", 0.2),
            ("多个", 0.15),
            ("集成", 0.2),
            ("迁移", 0.25),
            ("重构", 0.2),
            ("设计", 0.15),
            ("实现", 0.1),
        ];

        for (indicator, weight) in complexity_indicators {
            if text.contains(indicator) {
                score += weight;
            }
        }

        // 子任务数量也影响复杂度
        let child_count = task.children.len();
        if child_count > 5 {
            score += 0.3;
        } else if child_count > 3 {
            score += 0.2;
        } else if child_count > 0 {
            score += 0.1;
        }

        f64::min(score, 1.0)
    }

    /// 紧急度分析
    fn analyze_urgency(&self, task: &Task) -> f64 {
        let mut score: f64 = 0.0;
        let text = format!("{} {}", task.name, task.description).to_lowercase();

        // 紧急关键词
        let urgency_keywords: Vec<(&str, f64)> = vec![
            ("紧急", 0.9),
            ("立即", 0.9),
            ("马上", 0.85),
            ("尽快", 0.7),
            ("截止", 0.8),
            ("deadline", 0.8),
            ("今天", 0.75),
            ("明天", 0.6),
            ("本周", 0.5),
            ("阻塞", 0.9),
            ("卡住", 0.85),
            ("无法继续", 0.9),
            ("必须先", 0.8),
        ];

        for (keyword, weight) in urgency_keywords {
            if text.contains(keyword) {
                score += weight;
            }
        }

        // 检查元数据中是否有截止日期
        if task.metadata.contains_key("due_date") {
            score += 0.3;
        }

        f64::min(score, 1.0)
    }

    /// 阻塞性分析
    fn analyze_blocking(&self, task: &Task) -> f64 {
        let mut score: f64 = 0.0;
        let text = format!("{} {}", task.name, task.description).to_lowercase();

        // 阻塞关键词
        let blocking_keywords: Vec<(&str, f64)> = vec![
            ("阻塞", 0.95),
            ("依赖", 0.8),
            ("必须先", 0.9),
            ("前置", 0.85),
            ("基础", 0.75),
            ("核心", 0.7),
            ("其他任务需要", 0.8),
            ("后续", 0.6),
        ];

        for (keyword, weight) in blocking_keywords {
            if text.contains(keyword) {
                score += weight;
            }
        }

        // 依赖数量
        let dep_count = task.dependencies.len();
        if dep_count == 0 {
            // 没有依赖的任务可能是基础任务
            score += 0.1;
        }

        f64::min(score, 1.0)
    }

    /// 描述质量分析
    fn analyze_description_quality(&self, task: &Task) -> f64 {
        let desc = &task.description;
        let mut score: f64 = 0.0;

        // 描述长度
        let len = desc.len();
        if len > 100 {
            score += 0.4;
        } else if len > 50 {
            score += 0.3;
        } else if len > 20 {
            score += 0.2;
        } else if len > 0 {
            score += 0.1;
        }

        // 检查描述中是否包含关键信息
        let desc_lower = desc.to_lowercase();
        let quality_indicators: Vec<(&str, f64)> = vec![
            ("目的", 0.1),
            ("目标", 0.1),
            ("步骤", 0.1),
            ("需要", 0.05),
            ("包括", 0.05),
            ("例如", 0.1),
        ];

        for (indicator, weight) in quality_indicators {
            if desc_lower.contains(indicator) {
                score += weight;
            }
        }

        f64::min(score, 1.0)
    }

    /// 计算综合权重
    fn calculate_combined_weight(&self, result: &HeuristicResult) -> f64 {
        // 权重系数
        const KEYWORD_WEIGHT: f64 = 0.25;
        const COMPLEXITY_WEIGHT: f64 = 0.20;
        const URGENCY_WEIGHT: f64 = 0.25;
        const BLOCKING_WEIGHT: f64 = 0.20;
        const QUALITY_WEIGHT: f64 = 0.10;

        let combined = result.keyword_score * KEYWORD_WEIGHT
            + result.complexity_score * COMPLEXITY_WEIGHT
            + result.urgency_score * URGENCY_WEIGHT
            + result.blocking_score * BLOCKING_WEIGHT
            + result.description_quality_score * QUALITY_WEIGHT;

        // 归一化并稍微调整，使得分数分布更合理
        let adjusted = f64::min(combined * 1.2, 1.0);

        // 确保最小权重
        f64::max(adjusted, 0.05)
    }
}

impl Default for WeightHeuristics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_high_priority_task() {
        let heuristics = WeightHeuristics::new();
        let task = Task::new("t1", "实现核心认证系统")
            .with_description("这是项目的基础功能，其他模块都依赖它");

        let result = heuristics.analyze(&task);

        assert!(result.keyword_score > 0.5);
        assert!(result.blocking_score > 0.5);
        assert!(result.combined_weight > 0.3);
    }

    #[test]
    fn test_analyze_low_priority_task() {
        let heuristics = WeightHeuristics::new();
        let task = Task::new("t2", "更新文档注释").with_description("补充一些代码注释");

        let result = heuristics.analyze(&task);

        assert!(result.combined_weight < 0.5);
    }
}
