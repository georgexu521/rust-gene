//! 权重计算引擎 - Priority Agent 的核心
//!
//! 提供分层权重系统的计算和管理

pub mod calculator;
pub mod types;

pub use calculator::WeightCalculator;
pub use types::{Task, TaskId, Weight};

/// 权重分析工具 (stub)
pub struct WeightAnalysisTool;

impl WeightAnalysisTool {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze_project(&self, _project: &types::Project) -> WeightAnalysisResult {
        WeightAnalysisResult {
            weights: std::collections::HashMap::new(),
        }
    }
}

/// 权重分析结果 (stub)
pub struct WeightAnalysisResult {
    pub weights: std::collections::HashMap<String, f64>,
}
