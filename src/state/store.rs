//! 状态存储
//!
//! 管理状态更新和持久化

use crate::state::{
    select_runtime_mcp, select_runtime_permission, select_runtime_status, select_runtime_tools,
    AppState, EventBus, RuntimeMcpState, RuntimePermissionState, RuntimeStatusSnapshot,
    RuntimeToolUse, StateEvent,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 状态存储
pub struct StateStore {
    state: RwLock<AppState>,
    event_bus: Arc<EventBus>,
}

impl StateStore {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            state: RwLock::new(AppState::new()),
            event_bus,
        }
    }

    /// 获取当前状态
    pub async fn get_state(&self) -> AppState {
        self.state.read().await.clone()
    }

    /// 更新状态
    pub async fn update<F>(&self, updater: F)
    where
        F: FnOnce(&mut AppState),
    {
        {
            let mut state = self.state.write().await;
            updater(&mut state);
        }
        // 发送状态更新事件
        self.event_bus.emit(StateEvent::StateUpdated);
    }

    /// 替换整个状态
    pub async fn replace(&self, new_state: AppState) {
        {
            let mut state = self.state.write().await;
            *state = new_state;
        }
        self.event_bus.emit(StateEvent::StateUpdated);
    }

    pub async fn runtime_status(&self) -> RuntimeStatusSnapshot {
        let state = self.state.read().await;
        select_runtime_status(&state)
    }

    pub async fn runtime_tools(&self) -> Vec<RuntimeToolUse> {
        let state = self.state.read().await;
        select_runtime_tools(&state)
    }

    pub async fn runtime_permission(&self) -> RuntimePermissionState {
        let state = self.state.read().await;
        select_runtime_permission(&state)
    }

    pub async fn runtime_mcp(&self) -> RuntimeMcpState {
        let state = self.state.read().await;
        select_runtime_mcp(&state)
    }
}

/// 状态更新类型
#[allow(clippy::large_enum_variant)]
pub enum StateUpdate {
    Full(AppState),
    Partial(Box<dyn FnOnce(&mut AppState)>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_update() {
        let event_bus = Arc::new(EventBus::new());
        let store = StateStore::new(event_bus);

        store
            .update(|state| {
                state.add_user_message("Hello");
            })
            .await;

        let state = store.get_state().await;
        assert_eq!(state.messages.len(), 1);
    }
}
