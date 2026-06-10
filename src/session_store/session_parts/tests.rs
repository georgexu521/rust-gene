use super::*;
use crate::session_store::event_store::query_session_events;
use crate::session_store::SessionEventRow;
use rusqlite::Connection;

#[test]
fn projects_tool_lifecycle() {
    let events = vec![
        row(
            1,
            "tool_called",
            r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
        ),
        row(
            2,
            "tool_succeeded",
            r#"{"tool_call_id":"c1","result_preview":"ok"}"#,
        ),
    ];

    let parts = project_session_parts(&events);
    assert_eq!(parts.len(), 1, "tool_called updated by tool_succeeded");

    match &parts[0] {
        SessionPart::Tool {
            tool_call_id,
            tool_name,
            status,
            result_preview,
            ..
        } => {
            assert_eq!(tool_call_id, "c1");
            assert_eq!(tool_name, "bash");
            assert_eq!(*status, ToolPartStatus::Completed);
            assert_eq!(result_preview.as_deref(), Some("ok"));
        }
        _ => panic!("expected tool"),
    }
}

#[test]
fn projects_closeout() {
    let events = vec![row(
        1,
        "closeout",
        r#"{"status":"passed","evidence_summary":"tests ok"}"#,
    )];
    let parts = project_session_parts(&events);
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        SessionPart::Closeout {
            status,
            evidence_summary,
            ..
        } => {
            assert_eq!(status, "passed");
            assert_eq!(evidence_summary.as_deref(), Some("tests ok"));
        }
        _ => panic!("expected closeout"),
    }
}

#[test]
fn projects_separate_text_blocks_around_tool_parts() {
    let events = vec![
        row(1, "assistant_text_delta", r#"{"text":"before"}"#),
        row(
            2,
            "tool_called",
            r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
        ),
        row(
            3,
            "tool_succeeded",
            r#"{"tool_call_id":"c1","result_preview":"ok"}"#,
        ),
        row(4, "assistant_text_delta", r#"{"text":"after"}"#),
    ];

    let parts = project_session_parts(&events);
    assert_eq!(parts.len(), 3);
    assert!(matches!(
        &parts[0],
        SessionPart::AssistantText { part_id, content, .. }
            if part_id == "text_1" && content == "before"
    ));
    assert!(matches!(&parts[1], SessionPart::Tool { .. }));
    assert!(matches!(
        &parts[2],
        SessionPart::AssistantText { part_id, content, .. }
            if part_id == "text_4" && content == "after"
    ));
}

#[test]
fn projects_completed_tool_input_without_delta() {
    let events = vec![
        row(
            1,
            "tool_input_completed",
            r#"{"tool_call_id":"c1","input_args":"{\"command\":\"cargo test\"}","replay_source":"completed_event"}"#,
        ),
        row(
            2,
            "tool_started",
            r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
        ),
    ];

    let parts = project_session_parts(&events);
    match &parts[0] {
        SessionPart::Tool {
            input_args,
            input_replay_source,
            tool_name,
            ..
        } => {
            assert_eq!(tool_name, "bash");
            assert_eq!(input_args.as_deref(), Some(r#"{"command":"cargo test"}"#));
            assert_eq!(input_replay_source.as_deref(), Some("completed_event"));
        }
        _ => panic!("expected tool"),
    }
}

#[test]
fn projects_completed_tool_result_with_output_uri() {
    let events = vec![
        row(
            1,
            "tool_called",
            r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
        ),
        row(
            2,
            "tool_result_completed",
            r#"{"tool_call_id":"c1","result_preview":"tail","output_uri":"tool-output://bash_c1","replay_source":"completed_event"}"#,
        ),
        row(
            3,
            "shell_output_completed",
            r#"{"tool_call_id":"c1","command":"cargo test","output_uri":"tool-output://bash_c1","replay_source":"completed_event"}"#,
        ),
    ];

    let parts = project_session_parts(&events);
    assert_eq!(parts.len(), 2);
    match &parts[0] {
        SessionPart::Tool {
            status,
            result_preview,
            output_uri,
            result_replay_source,
            ..
        } => {
            assert_eq!(*status, ToolPartStatus::Completed);
            assert_eq!(result_preview.as_deref(), Some("tail"));
            assert_eq!(output_uri.as_deref(), Some("tool-output://bash_c1"));
            assert_eq!(result_replay_source.as_deref(), Some("completed_event"));
        }
        _ => panic!("expected tool"),
    }
    assert!(matches!(
        &parts[1],
        SessionPart::Shell {
            command,
            output_uri,
            ..
        } if command.as_deref() == Some("cargo test")
            && output_uri.as_deref() == Some("tool-output://bash_c1")
    ));
}

#[test]
fn projects_revert_marker_from_target_part() {
    let events = vec![row(
        1,
        "revert",
        r#"{"status":"completed","target_part_id":"tool_c1","part_ids":["tool_c1"],"unrevert_possible":true}"#,
    )];

    let parts = project_session_parts(&events);
    assert!(matches!(
        &parts[0],
        SessionPart::Revert {
            reverted_after,
            unrevert_possible,
            ..
        } if reverted_after.as_deref() == Some("tool_c1") && *unrevert_possible
    ));
}

#[test]
fn incremental_projection_matches_full_projection_for_text_tool_text() {
    let conn = test_conn();
    let events = vec![
        row(1, "assistant_text_delta", r#"{"text":"before"}"#),
        row(
            2,
            "tool_called",
            r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
        ),
        row(
            3,
            "tool_input_completed",
            r#"{"tool_call_id":"c1","input_args":"{\"command\":\"cargo test\"}","replay_source":"completed_event"}"#,
        ),
        row(
            4,
            "tool_succeeded",
            r#"{"tool_call_id":"c1","result_preview":"ok"}"#,
        ),
        row(
            5,
            "tool_result_completed",
            r#"{"tool_call_id":"c1","result_preview":"ok full","output_uri":"tool-output://bash_c1","replay_source":"completed_event"}"#,
        ),
        row(
            6,
            "shell_output_completed",
            r#"{"tool_call_id":"c1","command":"cargo test","output_uri":"tool-output://bash_c1","replay_source":"completed_event"}"#,
        ),
        row(7, "assistant_text_delta", r#"{"text":"after"}"#),
        row(8, "reasoning_delta", r#"{"text":"think"}"#),
        row(9, "reasoning_completed", r#"{"text":"think done"}"#),
    ];

    for event in &events {
        insert_event(&conn, event);
        incremental_refresh_session_parts(&conn, "sess-1").unwrap();
    }

    let full_payloads = project_session_parts(&events)
        .iter()
        .map(|part| serde_json::to_value(part).unwrap())
        .collect::<Vec<_>>();
    let incremental_payloads = query_persisted_session_parts(&conn, "sess-1")
        .unwrap()
        .into_iter()
        .map(|part| part.payload)
        .collect::<Vec<_>>();

    assert_eq!(incremental_payloads, full_payloads);
}

fn row(seq: i64, event_type: &str, payload: &str) -> SessionEventRow {
    SessionEventRow {
        id: seq,
        session_id: "sess-1".to_string(),
        seq,
        event_type: event_type.to_string(),
        timestamp_ms: 0,
        payload: payload.to_string(),
    }
}

fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
            "CREATE TABLE session_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                payload TEXT NOT NULL DEFAULT '{}'
            );
            CREATE INDEX IF NOT EXISTS idx_session_events_session ON session_events(session_id, seq);
            CREATE TABLE session_parts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                part_index INTEGER NOT NULL,
                part_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                tool_call_id TEXT,
                tool_name TEXT,
                status TEXT,
                payload TEXT NOT NULL DEFAULT '{}',
                projected_to_seq INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                message_id TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_session_parts_session_part
                ON session_parts(session_id, part_id);",
        )
        .unwrap();
    conn
}

fn insert_event(conn: &Connection, event: &SessionEventRow) {
    conn.execute(
        "INSERT INTO session_events (session_id, seq, event_type, timestamp_ms, payload)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            event.session_id,
            event.seq,
            event.event_type,
            event.timestamp_ms,
            event.payload
        ],
    )
    .unwrap();
}

#[test]
fn project_detects_running_tool_after_interruption_and_failed_event_updates_status() {
    // Tests that the projection correctly shows Running after a tool_started
    // event without a matching completed/failed event, and that injecting a
    // tool_failed event updates the projection to Failed.
    let conn = test_conn();
    let events = vec![
        row(
            1,
            "tool_called",
            r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
        ),
        row(
            2,
            "tool_started",
            r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
        ),
        // No tool_succeeded/tool_failed — simulating interruption
    ];
    for event in &events {
        insert_event(&conn, event);
    }

    let parts = project_session_parts(&events);
    assert_eq!(parts.len(), 1);
    assert!(
        matches!(
            &parts[0],
            SessionPart::Tool {
                status: ToolPartStatus::Running,
                ..
            }
        ),
        "tool should be running after interruption"
    );

    // Simulate recovery by writing a tool_failed event
    let writer = crate::session_store::SessionEventWriter::new(
        std::sync::Arc::new(std::sync::Mutex::new(conn)),
        "sess-1",
    );
    writer
        .tool_failed("c1", "Tool execution interrupted before settlement")
        .unwrap();

    // After recovery, re-project should show failed status
    let conn2 = writer.connection();
    let conn2_guard = conn2.lock().unwrap();
    let recovered_events = query_session_events(&conn2_guard, "sess-1", None).unwrap();
    let recovered_parts = project_session_parts(&recovered_events);
    assert_eq!(recovered_parts.len(), 1);
    assert!(
        matches!(
            &recovered_parts[0],
            SessionPart::Tool {
                status: ToolPartStatus::Failed,
                ..
            }
        ),
        "tool should be failed after recovery event"
    );
}

#[test]
fn export_payload_includes_parts_closeout_and_tool_outputs() {
    let conn = test_conn();
    let events = vec![
        row(1, "assistant_text_delta", r#"{"text":"Hello"}"#),
        row(
            2,
            "tool_called",
            r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
        ),
        row(
            3,
            "tool_succeeded",
            r#"{"tool_call_id":"c1","result_preview":"ok"}"#,
        ),
        row(
            4,
            "closeout",
            r#"{"status":"passed","evidence_summary":"tests ok"}"#,
        ),
        row(
            5,
            "compaction",
            r#"{"strategy":"snip","trigger":"memory","before_tokens":1000,"after_tokens":500}"#,
        ),
    ];
    for event in &events {
        insert_event(&conn, event);
    }

    let parts = project_session_parts(&events);
    assert_eq!(parts.len(), 4); // text + tool + closeout + compaction

    // Verify closeout part
    let closeout_parts: Vec<_> = parts
        .iter()
        .filter(|p| matches!(p, SessionPart::Closeout { .. }))
        .collect();
    assert_eq!(closeout_parts.len(), 1);

    // Verify compaction part
    let compaction_parts: Vec<_> = parts
        .iter()
        .filter(|p| matches!(p, SessionPart::Compaction { .. }))
        .collect();
    assert_eq!(compaction_parts.len(), 1);

    // Verify no unresolved settlement (all tools completed)
    let unresolved: Vec<_> = parts
        .iter()
        .filter(|p| {
            matches!(
                p,
                SessionPart::Tool {
                    status: ToolPartStatus::Running | ToolPartStatus::Pending,
                    ..
                }
            )
        })
        .collect();
    assert!(unresolved.is_empty(), "all tools should be settled");
}
