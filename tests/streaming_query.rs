//! Integration test: streaming query pipeline.

use futures::StreamExt;
use priority_agent::engine::streaming::StreamEvent;
use std::sync::Arc;

mod common;

#[tokio::test]
async fn streaming_query_with_text_response() {
    let provider = Arc::new(common::MockProvider::with_streams(vec![
        common::stream_text_response("Hello from mock!"),
    ]));
    let tool_registry = common::tool_registry();

    let engine = priority_agent::engine::streaming::StreamingQueryEngine::new(
        provider.clone(),
        tool_registry,
        "mock-model",
    );

    let mut stream = engine.query_stream("say hello").await;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Quiet direct turns intentionally skip the run-card Start event; the
    // streaming contract still emits text and completion.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::TextChunk(_))),
        "expected TextChunk event"
    );
    assert!(
        events.iter().any(|e| matches!(e, StreamEvent::Complete)),
        "expected Complete event"
    );
    assert!(
        !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
        "unexpected error event: {:?}",
        events
    );

    let text = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::TextChunk(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(text, "Hello from mock!");

    assert_eq!(provider.call_count(), 1);
}

#[tokio::test]
async fn streaming_query_with_tool_call_roundtrip() {
    let mut env = common::EnvGuard::acquire().await;
    env.set("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS", "0");

    let provider = Arc::new(common::MockProvider::with_streams(vec![
        common::calculate_tool_call_stream(),
        common::stream_text_response("Tool said: 5"),
    ]));

    let tool_registry = common::tool_registry();

    let engine = priority_agent::engine::streaming::StreamingQueryEngine::new(
        provider.clone(),
        tool_registry,
        "mock-model",
    );

    let mut stream = engine.query_stream("calculate 2 + 3").await;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    assert!(
        !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
        "unexpected error event: {:?}",
        events
    );
    assert!(
        events.iter().any(|e| matches!(e, StreamEvent::Complete)),
        "expected Complete event, got: {:?}",
        events
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            StreamEvent::ToolExecutionStart { id, name, .. }
                if id == "call_1" && name == "calculate"
        )),
        "expected calculate tool execution start, got: {:?}",
        events
    );

    let tool_result = events.iter().find_map(|event| match event {
        StreamEvent::ToolExecutionComplete { id, result, .. } if id == "call_1" => {
            Some(result.as_str())
        }
        _ => None,
    });
    assert!(
        tool_result.is_some_and(|result| result.contains("2 + 3 = 5")),
        "expected successful calculate result, got: {:?}",
        tool_result
    );

    let text = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::TextChunk(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert!(text.contains("Tool said: 5"), "final text was: {text}");
    assert_eq!(provider.call_count(), 2);
}
