use super::*;
use crate::session_store::SessionStore;
use crate::state::MessageRole;
use std::sync::Arc;

#[test]
fn test_session_lifecycle() {
    let mut manager = TuiSessionManager::in_memory().unwrap();

    // 开始新会话
    let _session_id = manager.start_session("Test Session", "gpt-4").unwrap();
    assert!(manager.current_session_id().is_some());

    // 添加消息
    manager.add_message(MessageRole::User, "Hello").unwrap();
    manager
        .add_message(MessageRole::Assistant, "Hi there")
        .unwrap();

    // 验证消息
    let session_id = manager.current_session_id().unwrap();
    let messages = manager.load_messages(session_id).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[1].role, MessageRole::Assistant);

    // 验证会话列表
    let sessions = manager.list_sessions(100).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Test Session");
}

#[test]
fn test_from_store_reuses_existing_session() {
    let store = Arc::new(SessionStore::in_memory().unwrap());
    store
        .create_session("shared-session", "Shared", "mock-model")
        .unwrap();

    let manager =
        TuiSessionManager::from_store(store.clone(), "shared-session", "Shared", "mock-model")
            .unwrap();

    assert_eq!(manager.current_session_id(), Some("shared-session"));
    assert!(manager.is_current_session("shared-session"));
    manager.add_message(MessageRole::User, "hello").unwrap();
    assert_eq!(store.get_messages("shared-session").unwrap().len(), 1);
}

#[test]
fn test_recent_traces_use_current_session_scope() {
    let store = Arc::new(SessionStore::in_memory().unwrap());
    store
        .create_session("shared-session", "Shared", "mock-model")
        .unwrap();
    store
        .create_session("other-session", "Other", "mock-model")
        .unwrap();

    for turn_index in 1..=2 {
        let mut trace =
            crate::engine::trace::TurnTrace::new("shared-session", turn_index, "shared trace");
        trace.finish(crate::engine::trace::TurnStatus::Completed);
        store.add_turn_trace(&trace).unwrap();
    }
    let mut other_trace = crate::engine::trace::TurnTrace::new("other-session", 9, "other trace");
    other_trace.finish(crate::engine::trace::TurnStatus::Completed);
    store.add_turn_trace(&other_trace).unwrap();

    let manager =
        TuiSessionManager::from_store(store, "shared-session", "Shared", "mock-model").unwrap();
    let traces = manager.recent_traces(10).unwrap();

    assert_eq!(traces.len(), 2);
    assert!(traces
        .iter()
        .all(|trace| trace.session_id == "shared-session"));
    assert_eq!(traces[0].turn_index, 2);
    assert_eq!(traces[1].turn_index, 1);
}
