//! 状态管理
//!
//! 模仿 React 风格的 AppState + setAppState 模式
//! 使用事件系统实现状态变更通知

pub mod app_state;
pub mod events;
pub mod store;

pub use app_state::{AppState, MessageItem, MessageRole, TaskItem, TaskStatus, TaskType};
pub use events::{EventBus, StateEvent};
pub use store::StateStore;

use std::sync::Arc;
use tokio::sync::RwLock;

/// 应用状态上下文
#[derive(Clone)]
pub struct AppContext {
    store: Arc<RwLock<StateStore>>,
    event_bus: Arc<EventBus>,
}

impl AppContext {
    pub fn new() -> Self {
        let event_bus = Arc::new(EventBus::new());
        let store = Arc::new(RwLock::new(StateStore::new(event_bus.clone())));

        Self { store, event_bus }
    }

    /// 获取当前状态的副本
    pub async fn get_state(&self) -> AppState {
        self.store.read().await.get_state().await
    }

    /// 更新状态（模仿 setAppState）
    pub async fn set_state<F>(&self, updater: F)
    where
        F: FnOnce(&mut AppState),
    {
        self.store.write().await.update(updater).await;
    }

    /// 订阅状态变更事件
    pub fn subscribe<F>(&self, callback: F) -> events::Subscription
    where
        F: Fn(StateEvent) + Send + Sync + 'static,
    {
        self.event_bus.subscribe(callback)
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}
