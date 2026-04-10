//! 上下文管理器 - 管理会话状态和上下文
//!
//! 提供状态持久化、历史记录和上下文恢复功能

pub mod state;
pub mod history;
pub mod persistence;

pub use state::{SessionState, ContextSnapshot};
pub use history::{HistoryManager, HistoryEntry};
pub use persistence::{PersistenceManager, StorageBackend};
