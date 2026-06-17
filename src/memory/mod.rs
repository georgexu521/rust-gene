//! 记忆系统模块
//!
//! 提供长期记忆的存储、检索和管理能力。记忆系统让 agent 能够：
//! - 跨会话保留重要信息（用户偏好、项目知识、历史决策）
//! - 基于相关性召回记忆，增强上下文
//! - 自动提取和组织新记忆
//! - 检测和处理矛盾记忆
//!
//! ## 核心组件
//! - [`MemoryManager`] - 记忆管理器，提供统一的读写接口
//! - [`provider`] - 记忆存储后端（本地文件、向量数据库等）
//! - [`retrieval`] - 记忆检索和排序
//! - [`extraction`] - 从对话中提取记忆
//! - [`quality`] - 记忆质量评估

pub mod active;
pub mod background_review;
pub mod calibration;
pub mod contradiction;
pub mod eval;
pub mod extraction;
pub(super) mod files;
pub mod manager;
pub mod persistence;
pub mod provider;
pub mod provider_ops;
pub mod quality;
pub mod ranking;
pub mod recall;
pub mod reports;
pub mod retrieval;
pub mod safety;
pub mod scoring;
pub mod search_index;
pub mod types;

pub use calibration::{
    built_in_memory_calibration_samples, run_memory_calibration_samples, MemoryCalibrationActual,
    MemoryCalibrationExpectation, MemoryCalibrationResult, MemoryCalibrationSample,
};
pub use eval::{run_memory_eval_suite, MemoryEvalFailureOwner, MemoryEvalReport, MemoryEvalResult};
pub use manager::{
    MemoryFlushReason, MemoryFlushRecord, MemoryFlushStatus, MemoryFlushSummary, MemoryManager,
    MemoryMigrationFileReport, MemoryMigrationReport, MemoryProductContractReport,
    MemoryRecordSummary, MemorySnapshotReport, MemoryWriteTarget,
};
pub use provider::{
    LocalMemoryProvider, LocalMemoryRecordWriteStatus, MemoryOperationJournalEntry, MemoryProvider,
    MemoryProviderCallOutcome, MemoryProviderCallStatus, MemoryProviderCapabilities,
    MemoryProviderLifecycleEntry, MemoryProviderLifecycleReport, MemoryProviderRegistry,
    NoNetworkMemoryProvider, MEMORY_PROVIDER_LIFECYCLE_HOOKS,
};
pub use quality::{assess_memory_candidate, MemoryQualityAssessment};
pub use recall::{score_recall, RecallDecision, RecallFactors, RecallScore};
pub use reports::MemoryWriteScoringTrace;
pub use safety::{scan_memory_content, MemorySafetyIssue};
pub use scoring::{
    memory_keep_factors_from_document, memory_write_factors_from_signals, score_memory_keep,
    score_memory_write, MemoryKeepDecision, MemoryKeepFactors, MemoryMaintenanceAction,
    MemoryWriteDecision, MemoryWriteFactors,
};
pub use types::{
    AgentContext, MemoryCandidate, MemoryEvidenceKind, MemoryEvidenceRef, MemoryKind,
    MemoryProjection, MemoryProvenance, MemoryRecord, MemoryScope, MemoryStatus,
    MemoryStrategyMetadata, SensitivityLevel,
};
