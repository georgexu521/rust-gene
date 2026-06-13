use super::*;
use crate::session_store::SessionStore;
use crate::state::MessageRole;
use std::collections::HashMap;
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
fn test_message_metadata_round_trips() {
    let mut manager = TuiSessionManager::in_memory().unwrap();
    let session_id = manager
        .start_session("Metadata Session", "deepseek-v4-flash")
        .unwrap();
    let metadata = HashMap::from([
        ("elapsed_ms".to_string(), "2730".to_string()),
        ("validation_status".to_string(), "passed".to_string()),
        ("model_label".to_string(), "deepseek-v4-flash".to_string()),
    ]);

    manager
        .add_message_with_metadata(MessageRole::Assistant, "Done", &metadata)
        .unwrap();

    let messages = manager.load_messages(&session_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].metadata.get("elapsed_ms"),
        Some(&"2730".to_string())
    );
    assert_eq!(
        messages[0].metadata.get("validation_status"),
        Some(&"passed".to_string())
    );
    assert_eq!(
        messages[0].metadata.get("model_label"),
        Some(&"deepseek-v4-flash".to_string())
    );
}

#[test]
fn test_export_session_preserves_tool_success_and_failure_stats() {
    let mut manager = TuiSessionManager::in_memory().unwrap();
    let session_id = manager
        .start_session("Export Tool Stats", "test-fixture-model")
        .unwrap();
    manager
        .add_message(MessageRole::User, "run partial tools")
        .unwrap();
    manager
        .add_message(MessageRole::Assistant, "partial complete")
        .unwrap();

    manager
        .write_session_event(
            &session_id,
            "tool_started",
            &serde_json::json!({
                "tool_call_id": "call_ok",
                "tool_name": "bash",
                "message_id": "user_export_1"
            }),
        )
        .unwrap();
    manager
        .write_session_event(
            &session_id,
            "tool_result_completed",
            &serde_json::json!({"tool_call_id": "call_ok", "result": "Result: OK\npartial-ok"}),
        )
        .unwrap();
    manager
        .write_session_event(
            &session_id,
            "tool_succeeded",
            &serde_json::json!({"tool_call_id": "call_ok", "result_preview": "partial-ok"}),
        )
        .unwrap();
    manager
        .write_session_event(
            &session_id,
            "tool_started",
            &serde_json::json!({"tool_call_id": "call_fail", "tool_name": "bash"}),
        )
        .unwrap();
    manager
        .write_session_event(
            &session_id,
            "tool_result_completed",
            &serde_json::json!({"tool_call_id": "call_fail", "result": "Result: ERROR\npartial-fail"}),
        )
        .unwrap();
    manager
        .write_session_event(
            &session_id,
            "tool_failed",
            &serde_json::json!({"tool_call_id": "call_fail", "error": "partial-fail"}),
        )
        .unwrap();

    let export_json = manager.export_session(&session_id).unwrap();
    let export: serde_json::Value = serde_json::from_str(&export_json).unwrap();
    let final_event_seq = manager
        .load_session_events(&session_id)
        .unwrap()
        .last()
        .map(|event| event.seq);

    assert_eq!(export["tool_stats"]["calls"]["bash"], 2);
    assert_eq!(export["tool_stats"]["successes"]["bash"], 1);
    assert_eq!(export["tool_stats"]["failures"]["bash"], 1);
    let statuses = export["parts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|part| part["status"].as_str())
        .collect::<Vec<_>>();
    assert!(statuses.contains(&"completed"));
    assert!(statuses.contains(&"failed"));
    assert!(export["unresolved_settlement"]
        .as_array()
        .unwrap()
        .is_empty());
    let ok_part = export["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["tool_call_id"].as_str() == Some("call_ok"))
        .expect("successful tool part is exported");
    assert_eq!(ok_part["part_id"].as_str(), Some("tool_call_ok"));
    assert_eq!(ok_part["message_id"].as_str(), Some("user_export_1"));
    assert_eq!(ok_part["projected_to_seq"].as_i64(), final_event_seq);
    assert!(
        ok_part["updated_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "exported part should include an update timestamp"
    );
}

#[test]
fn test_settle_unfinished_tool_parts_marks_running_tool_failed() {
    let mut manager = TuiSessionManager::in_memory().unwrap();
    let session_id = manager
        .start_session("Interrupted Tool", "test-fixture-model")
        .unwrap();
    manager
        .write_session_event(
            &session_id,
            "tool_started",
            &serde_json::json!({"tool_call_id": "call_running", "tool_name": "bash"}),
        )
        .unwrap();

    let settled = manager
        .settle_unfinished_tool_parts(&session_id, "Run interrupted")
        .unwrap();

    assert_eq!(settled, 1);
    let parts = manager.load_session_parts(&session_id).unwrap();
    assert_eq!(parts[0].status.as_deref(), Some("failed"));
    let events = manager.load_session_events(&session_id).unwrap();
    assert!(events.iter().any(|event| event.event_type == "tool_failed"
        && event.payload.contains("Run interrupted before settlement")));
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
