//! Integration test: SQLite session persistence.
//!
//! Validates the SessionStore lifecycle: create session → add messages → reload → search.

use priority_agent::session_store::SessionStore;
use uuid::Uuid;

fn temp_db(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pa-int-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("sessions.db")
}

#[test]
fn session_create_and_retrieve() {
    let db_path = temp_db("session-create");
    let store = SessionStore::open(&db_path).expect("open db");

    let id = Uuid::new_v4().to_string();
    store
        .create_session(&id, "Test Session", "mock-model")
        .expect("create session");

    let session = store
        .get_session(&id)
        .expect("get session")
        .expect("session should exist");

    assert_eq!(session.title, "Test Session");
    assert_eq!(session.model, "mock-model");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn session_add_and_list_messages() {
    let db_path = temp_db("session-messages");
    let store = SessionStore::open(&db_path).expect("open db");

    let id = Uuid::new_v4().to_string();
    store
        .create_session(&id, "Chat Session", "mock-model")
        .expect("create session");

    store
        .add_message(
            &id,
            "user",
            "Hello, how do I use Rust lifetimes?",
            None,
            None,
        )
        .expect("add user message");

    store
        .add_message(
            &id,
            "assistant",
            "Rust lifetimes use the 'a syntax for annotations.",
            None,
            None,
        )
        .expect("add assistant message");

    let messages = store.get_messages(&id).expect("list messages");

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert!(messages[0].content.contains("lifetimes"));
    assert_eq!(messages[1].role, "assistant");
    assert!(messages[1].content.contains("'a"));

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn session_fts_search_finds_messages() {
    let db_path = temp_db("session-fts");
    let store = SessionStore::open(&db_path).expect("open db");

    let id = Uuid::new_v4().to_string();
    store
        .create_session(&id, "Dev Session", "mock-model")
        .expect("create session");

    store
        .add_message(
            &id,
            "user",
            "The TUI uses ratatui for terminal rendering",
            None,
            None,
        )
        .expect("add message");

    store
        .add_message(
            &id,
            "assistant",
            "Yes, ratatui is a Rust library for building TUIs",
            None,
            None,
        )
        .expect("add message");

    // FTS5 search should find messages containing "ratatui".
    let results = store
        .search_messages("ratatui", 10)
        .expect("search messages");

    assert!(!results.is_empty(), "FTS search should find 'ratatui'");
    assert!(
        results.iter().any(|m| m.content.contains("ratatui")),
        "results should contain the search term"
    );

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn session_list_recent() {
    let db_path = temp_db("session-list");
    let store = SessionStore::open(&db_path).expect("open db");

    let id1 = Uuid::new_v4().to_string();
    let id2 = Uuid::new_v4().to_string();
    store
        .create_session(&id1, "First Session", "mock-model")
        .expect("create session 1");
    store
        .create_session(&id2, "Second Session", "mock-model")
        .expect("create session 2");

    let sessions = store.list_sessions(10).expect("list sessions");
    assert!(sessions.len() >= 2, "should list at least 2 sessions");

    let _ = std::fs::remove_file(&db_path);
}
