use super::StreamEvent;
use tokio::sync::mpsc;

pub(super) async fn emit_turn_timeout_failure(
    tx: &mpsc::Sender<StreamEvent>,
    stage: &str,
    timeout: std::time::Duration,
) {
    let message = format!("{stage} execution timed out after {}s", timeout.as_secs());
    let _ = tx
        .send(StreamEvent::RuntimeDiagnostic {
            diagnostic: serde_json::json!({
                "schema": "turn_timeout.v1",
                "stage": stage,
                "status": "timed_out",
                "timeout_secs": timeout.as_secs(),
                "message": message,
            }),
        })
        .await;
    let _ = tx
        .send(StreamEvent::Closeout {
            status: "timed_out".to_string(),
            evidence_summary: Some(message.clone()),
        })
        .await;
    let _ = tx.send(StreamEvent::Error(message)).await;
}

pub(super) async fn emit_turn_cancelled_failure(tx: &mpsc::Sender<StreamEvent>, stage: &str) {
    let message = format!("{stage} cancelled by caller");
    let _ = tx
        .send(StreamEvent::RuntimeDiagnostic {
            diagnostic: serde_json::json!({
                "schema": "turn_cancellation.v1",
                "stage": stage,
                "status": "cancelled",
                "cancellation_boundaries": cancellation_boundary_map(),
                "message": message,
            }),
        })
        .await;
    let _ = tx
        .send(StreamEvent::Closeout {
            status: "cancelled".to_string(),
            evidence_summary: Some(message.clone()),
        })
        .await;
    let _ = tx.send(StreamEvent::Error(message)).await;
}

fn cancellation_boundary_map() -> serde_json::Value {
    serde_json::json!([
        {
            "boundary": "provider_request_future",
            "behavior": "drops_future_only",
            "detail": "The active provider future is dropped through the selected turn future; provider SDK transport-specific abort hooks are best-effort follow-up work."
        },
        {
            "boundary": "streaming_engine",
            "behavior": "cancellable",
            "detail": "Streaming turn preflight and query execution observe the shared CancellationToken."
        },
        {
            "boundary": "tool_execution_controller",
            "behavior": "drops_future_only",
            "detail": "Long-running tool futures are interrupted at the runtime selection boundary unless the specific tool owns a process-level cancellation hook."
        },
        {
            "boundary": "bash_local_process_tools",
            "behavior": "external_process_killed",
            "detail": "Required validation child processes use kill-on-drop plus explicit process termination when cancellation is wired into that runner."
        },
        {
            "boundary": "required_validation_runner",
            "behavior": "external_process_killed",
            "detail": "Required validation stops the child process group and returns Interrupted on cancellation."
        },
        {
            "boundary": "desktop_lightweight_provider_lane",
            "behavior": "cancellable",
            "detail": "Desktop lightweight turns select on the same CancellationToken before emitting completion."
        }
    ])
}
