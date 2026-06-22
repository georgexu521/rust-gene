use super::{config, context_scrubber::ContextScrubber, StreamEvent};
use crate::services::api::Message;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::warn;

pub(super) async fn flush_session_end_memory_best_effort(
    mem_mutex: Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>,
    session_id: String,
    flush_history: Vec<Message>,
    tx: mpsc::Sender<StreamEvent>,
) {
    let timeout = config::session_end_memory_flush_timeout();
    let timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
    let _ = tx
        .send(StreamEvent::RuntimeDiagnostic {
            diagnostic: serde_json::json!({
                "schema": "streaming_stage.v1",
                "stage": "session_end_memory_flush_start",
                "timeout_ms": timeout_ms,
            }),
        })
        .await;
    let started = std::time::Instant::now();
    let handle = tokio::task::spawn_blocking(move || {
        let mut mem = mem_mutex.blocking_lock();
        mem.flush_session_with_reason(
            session_id,
            crate::memory::MemoryFlushReason::SessionEnd,
            &flush_history,
        )
    });

    let (status, detail) = match tokio::time::timeout(timeout, handle).await {
        Ok(Ok(record)) => (format!("{:?}", record.status), record.reason.to_string()),
        Ok(Err(error)) => {
            let detail = error.to_string();
            warn!("session end memory flush join failed: {detail}");
            ("failed".to_string(), detail)
        }
        Err(_) => {
            warn!("session end memory flush exceeded {timeout_ms}ms; continuing stream close");
            (
                "timed_out".to_string(),
                "timed out; stream close continued".to_string(),
            )
        }
    };
    let _ = tx
        .send(StreamEvent::RuntimeDiagnostic {
            diagnostic: serde_json::json!({
                "schema": "streaming_stage.v1",
                "stage": "session_end_memory_flush_done",
                "status": status,
                "detail": detail,
                "duration_ms": started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                "timeout_ms": timeout_ms,
            }),
        })
        .await;
}

/// 清理完整文本中的记忆上下文标签区间。
fn scrub_memory_context(text: String) -> String {
    let mut scrubber = ContextScrubber::new();
    let visible = scrubber.feed(&text);
    let tail = scrubber.flush();
    if tail.is_empty() {
        visible
    } else {
        visible + &tail
    }
}

pub async fn emit_text_progressively(tx: &mpsc::Sender<StreamEvent>, text: String) {
    let text = scrub_memory_context(text);
    let chunks = progressive_text_chunks(&text);
    let chunk_count = chunks.len();
    for chunk in chunks {
        if tx.send(StreamEvent::TextChunk(chunk)).await.is_err() {
            break;
        }
        if chunk_count > 1 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

pub(super) fn progressive_text_chunks(text: &str) -> Vec<String> {
    if text.chars().count() <= 96 {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0usize;
    for ch in text.chars() {
        current.push(ch);
        current_chars += 1;
        let natural_boundary = ch.is_whitespace()
            || matches!(
                ch,
                '.' | ',' | ';' | ':' | '!' | '?' | '。' | '，' | '；' | '：' | '！' | '？'
            );
        if current_chars >= 96 || (current_chars >= 32 && natural_boundary) {
            chunks.push(std::mem::take(&mut current));
            current_chars = 0;
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}
