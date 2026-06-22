//! 上下文管理器 - 管理会话状态和上下文
//!
//! 提供状态持久化、历史记录和上下文恢复功能

pub mod history;
pub mod persistence;
pub mod state;

#[allow(unused_imports)]
pub use history::{HistoryEntry, HistoryManager};
#[allow(unused_imports)]
pub use persistence::{PersistenceManager, StorageBackend};
#[allow(unused_imports)]
pub use state::{ContextSnapshot, SessionState};
