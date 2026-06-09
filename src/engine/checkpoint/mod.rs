//! File Checkpointing 系统
//!
//! 对标 Claude Code 的 fileHistory.ts：
//! - 每次文件修改前自动创建快照
//! - 最多保留 MAX_CHECKPOINTS (100) 个快照
//! - 支持 diff 对比任意两个版本
//! - 支持恢复到任意历史状态
//! - 存储在 ~/.priority-agent/checkpoints/<session_id>/

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod manager;
#[cfg(test)]
mod tests;
pub mod types;

pub use types::*;

/// 全局 CheckpointManager 缓存（按 session_id）
static CHECKPOINT_MANAGERS: once_cell::sync::Lazy<
    std::sync::Mutex<HashMap<String, Arc<Mutex<CheckpointManager>>>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

/// 获取或创建 CheckpointManager
pub async fn get_checkpoint_manager(
    session_id: impl Into<String>,
) -> Arc<Mutex<CheckpointManager>> {
    let session_id = session_id.into();
    {
        let managers = CHECKPOINT_MANAGERS
            .lock()
            .expect("checkpoint managers lock poisoned");
        if let Some(mgr) = managers.get(&session_id) {
            return mgr.clone();
        }
    }

    let mgr = Arc::new(Mutex::new(CheckpointManager::new(&session_id).await));
    {
        let mut managers = CHECKPOINT_MANAGERS
            .lock()
            .expect("checkpoint managers lock poisoned");
        managers.insert(session_id, mgr.clone());
    }
    mgr
}
