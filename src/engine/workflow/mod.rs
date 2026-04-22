//! WorkflowEngine — 权重驱动 + 主动提问式深思
//!
//! 模块划分：
//! - `weights.rs` — 源码级权重计算规则引擎（六维评分）
//! - `gate.rs` — 触发闸门（Direct vs Workflow）
//! - `questioning.rs` — 主动提问式深思引擎
//! - `planner.rs` — 计划生成与递归拆分
//! - `executor.rs` — 按权重执行与回写
//! - `metrics.rs` — 评估指标与日志

pub mod weights;

pub use weights::{
    BlockerValueRule, ComplexityRule, DependencyPenaltyRule, DimensionScore, DriftPenaltyRule,
    ImpactRule, RiskRule, StepContext, WeightDimension, WeightEngine, WeightRule, WeightedStep,
};
