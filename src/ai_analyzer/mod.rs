//! AI 权重分析器
//!
//! 支持 LLM 驱动的智能权重分配和启发式 fallback

pub mod analyzer;
pub mod heuristics;

pub use analyzer::{AiWeightAnalyzer, ProjectContext};
