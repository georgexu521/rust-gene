//! 上下文管理器 - 管理会话状态和上下文
//!
//! 提供状态持久化、历史记录和上下文恢复功能

pub mod history;
pub mod persistence;
pub mod state;

pub use history::{HistoryEntry, HistoryManager};
pub use persistence::{PersistenceManager, StorageBackend};
pub use state::{ContextSnapshot, SessionState};
