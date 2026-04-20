//! 事件系统 - 用于状态变更通知
//!
//! 模仿 React 的重新渲染机制

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// 状态事件
#[derive(Debug, Clone)]
pub enum StateEvent {
    /// 状态已更新
    StateUpdated,
    /// 新消息
    NewMessage { id: String },
    /// 消息更新
    MessageUpdated { id: String },
    /// 新任务
    NewTask { id: String },
    /// 任务更新
    TaskUpdated { id: String },
    /// 开始查询
    QueryStarted,
    /// 查询结束
    QueryEnded,
    /// 工具调用开始
    ToolUseStarted { id: String },
    /// 工具调用结束
    ToolUseEnded { id: String },
    /// 错误
    Error { message: String },
    /// 退出应用
    Exit,
}

/// 事件处理器类型
type EventHandler = Box<dyn Fn(StateEvent) + Send + Sync>;
/// 事件订阅者映射（共享引用）
type SubscriberMap = Arc<Mutex<HashMap<String, EventHandler>>>;

/// 事件总线
pub struct EventBus {
    subscribers: SubscriberMap,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 订阅事件
    pub fn subscribe<F>(&self, callback: F) -> Subscription
    where
        F: Fn(StateEvent) + Send + Sync + 'static,
    {
        let id = Uuid::new_v4().to_string();
        self.subscribers
            .lock()
            .unwrap()
            .insert(id.clone(), Box::new(callback));

        Subscription {
            id,
            bus: self.subscribers.clone(),
        }
    }

    /// 发布事件
    pub fn emit(&self, event: StateEvent) {
        let subscribers = self.subscribers.lock().expect("subscribers mutex poisoned");
        for callback in subscribers.values() {
            callback(event.clone());
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// 订阅句柄 - 当 drop 时自动取消订阅
pub struct Subscription {
    id: String,
    bus: SubscriberMap,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.bus
            .lock()
            .expect("bus mutex poisoned")
            .remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus() {
        let bus = EventBus::new();
        let received = Arc::new(Mutex::new(false));

        let r = received.clone();
        let _sub = bus.subscribe(move |event| {
            if matches!(event, StateEvent::StateUpdated) {
                *r.lock().unwrap() = true;
            }
        });

        bus.emit(StateEvent::StateUpdated);

        assert!(*received.lock().unwrap());
    }
}
