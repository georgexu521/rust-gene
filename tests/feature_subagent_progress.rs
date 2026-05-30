//! Integration test: Sub-agent Progress Events (Phase B #5).

use priority_agent::agent::manager::AgentManager;
use priority_agent::agent::progress::AgentProgressEvent;
use priority_agent::agent::types::{AgentId, AgentStatus};

#[tokio::test]
async fn progress_channel_created_on_subscribe() {
    let manager = AgentManager::new();
    let agent_id = AgentId("test-progress".to_string());
    let rx = manager.subscribe_progress(&agent_id).await;
    drop(rx);
}

#[tokio::test]
async fn emit_progress_does_not_panic_without_subscribers() {
    let manager = AgentManager::new();
    let agent_id = AgentId("no-sub".to_string());
    manager
        .emit_progress(
            &agent_id,
            AgentProgressEvent::Started {
                agent_id: "no-sub".into(),
                task: "test task".into(),
            },
        )
        .await;
}

#[tokio::test]
async fn progress_events_received_by_subscriber() {
    let manager = AgentManager::new();
    let agent_id = AgentId("sub-test".to_string());

    let mut rx = manager.subscribe_progress(&agent_id).await;

    manager
        .emit_progress(
            &agent_id,
            AgentProgressEvent::Started {
                agent_id: "sub-test".into(),
                task: "analyze codebase".into(),
            },
        )
        .await;

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout")
        .expect("should receive event");

    match event {
        AgentProgressEvent::Started { task, .. } => {
            assert_eq!(task, "analyze codebase");
        }
        _ => panic!("expected Started event"),
    }
}

#[test]
fn progress_event_summaries_are_human_readable() {
    let started = AgentProgressEvent::Started {
        agent_id: "a".into(),
        task: "read files".into(),
    };
    assert!(started.summary().contains("read files"));

    let completed = AgentProgressEvent::Completed {
        agent_id: "a".into(),
        result: priority_agent::agent::manager::AgentResult {
            agent_id: AgentId("a".into()),
            status: AgentStatus::Completed,
            content: "done".into(),
            completed_at: std::time::Instant::now(),
            tools_used: vec![],
            confidence: 1.0,
            has_conflict: false,
        },
    };
    assert_eq!(completed.summary(), "Completed");

    let failed = AgentProgressEvent::Failed {
        agent_id: "a".into(),
        error: "timeout".into(),
    };
    assert!(failed.summary().contains("timeout"));
}
