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
