use super::*;
use async_trait::async_trait;
use futures::StreamExt;
use std::collections::VecDeque;
use std::sync::Mutex as StdMutex;

struct MockProvider;

#[async_trait]
impl LlmProvider for MockProvider {
    async fn chat(
        &self,
        _request: crate::services::api::ChatRequest,
    ) -> anyhow::Result<crate::services::api::ChatResponse> {
        unimplemented!()
    }

    async fn chat_stream(
        &self,
        _request: crate::services::api::ChatRequest,
    ) -> anyhow::Result<async_openai::types::ChatCompletionResponseStream> {
        unimplemented!()
    }

    fn base_url(&self) -> &str {
        "mock://local"
    }

    fn default_model(&self) -> &str {
        "mock-a"
    }
}

struct NamedMockProvider {
    base_url: &'static str,
    model: &'static str,
}

#[async_trait]
impl LlmProvider for NamedMockProvider {
    async fn chat(
        &self,
        _request: crate::services::api::ChatRequest,
    ) -> anyhow::Result<crate::services::api::ChatResponse> {
        unimplemented!()
    }

    async fn chat_stream(
        &self,
        _request: crate::services::api::ChatRequest,
    ) -> anyhow::Result<async_openai::types::ChatCompletionResponseStream> {
        unimplemented!()
    }

    fn base_url(&self) -> &str {
        self.base_url
    }

    fn default_model(&self) -> &str {
        self.model
    }
}

struct ToolTurnProvider {
    responses: StdMutex<VecDeque<crate::services::api::ChatResponse>>,
}

#[async_trait]
impl LlmProvider for ToolTurnProvider {
    async fn chat(
        &self,
        _request: crate::services::api::ChatRequest,
    ) -> anyhow::Result<crate::services::api::ChatResponse> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("no mock response left"))
    }

    async fn chat_stream(
        &self,
        _request: crate::services::api::ChatRequest,
    ) -> anyhow::Result<async_openai::types::ChatCompletionResponseStream> {
        Err(anyhow::anyhow!(
            "stream not used for MiniMax-compatible tool turns"
        ))
    }

    fn base_url(&self) -> &str {
        "https://api.minimaxi.com/v1"
    }

    fn default_model(&self) -> &str {
        "MiniMax-M2.7"
    }
}

struct RecordingToolProvider {
    requests: StdMutex<Vec<crate::services::api::ChatRequest>>,
}

#[async_trait]
impl LlmProvider for RecordingToolProvider {
    async fn chat(
        &self,
        request: crate::services::api::ChatRequest,
    ) -> anyhow::Result<crate::services::api::ChatResponse> {
        let mut requests = self.requests.lock().unwrap();
        requests.push(request);
        if requests.len() == 1 {
            Ok(crate::services::api::ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![crate::services::api::ToolCall {
                    id: "call_read".to_string(),
                    name: "file_read".to_string(),
                    arguments: serde_json::json!({ "path": "marker.txt" }),
                }]),
                usage: None,
                tool_call_repair: None,
            })
        } else {
            Ok(crate::services::api::ChatResponse {
                content: "Done.".to_string(),
                tool_calls: None,
                usage: None,
                tool_call_repair: None,
            })
        }
    }

    async fn chat_stream(
        &self,
        _request: crate::services::api::ChatRequest,
    ) -> anyhow::Result<async_openai::types::ChatCompletionResponseStream> {
        Err(anyhow::anyhow!(
            "stream not used for MiniMax-compatible tool turns"
        ))
    }

    fn base_url(&self) -> &str {
        "https://api.minimaxi.com/v1"
    }

    fn default_model(&self) -> &str {
        "MiniMax-M2.7"
    }
}

#[test]
fn test_stream_event_creation() {
    let event = StreamEvent::TextChunk("Hello".to_string());
    assert!(matches!(event, StreamEvent::TextChunk(_)));
}

#[test]
fn test_runtime_model_switch_updates_label() {
    let engine = StreamingQueryEngine::new(
        Arc::new(MockProvider),
        Arc::new(ToolRegistry::new()),
        "mock-a",
    );
    assert_eq!(engine.model_name(), "mock-a");
    engine.set_model("mock-b");
    assert_eq!(engine.model_name(), "mock-b");
}

#[test]
fn test_runtime_provider_switch_updates_provider_and_model() {
    let engine = StreamingQueryEngine::new(
        Arc::new(NamedMockProvider {
            base_url: "mock://a",
            model: "model-a",
        }),
        Arc::new(ToolRegistry::new()),
        "model-a",
    );

    engine.set_provider(
        Arc::new(NamedMockProvider {
            base_url: "mock://b",
            model: "model-b",
        }),
        "model-b",
    );

    assert_eq!(engine.provider_base_url(), "mock://b");
    assert_eq!(engine.model_name(), "model-b");
    assert_eq!(engine.provider().default_model(), "model-b");
}

#[tokio::test]
async fn streaming_history_does_not_persist_completed_tool_calls_as_final_assistant_calls() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(dir.path().join("marker.txt"), "marker-content\n")
        .await
        .unwrap();
    let provider = Arc::new(ToolTurnProvider {
        responses: StdMutex::new(VecDeque::from(vec![
            crate::services::api::ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![crate::services::api::ToolCall {
                    id: "call_read".to_string(),
                    name: "file_read".to_string(),
                    arguments: serde_json::json!({
                        "path": "marker.txt"
                    }),
                }]),
                usage: None,
                tool_call_repair: None,
            },
            crate::services::api::ChatResponse {
                content: "Done.".to_string(),
                tool_calls: None,
                usage: None,
                tool_call_repair: None,
            },
            crate::services::api::ChatResponse {
                content: "Done.".to_string(),
                tool_calls: None,
                usage: None,
                tool_call_repair: None,
            },
            crate::services::api::ChatResponse {
                content: "Done.".to_string(),
                tool_calls: None,
                usage: None,
                tool_call_repair: None,
            },
        ])),
    });
    let mut registry = ToolRegistry::new();
    registry.register(crate::tools::file_tool::FileReadTool);
    let engine = StreamingQueryEngine::new(provider, Arc::new(registry), "MiniMax-M2.7")
        .with_working_dir(dir.path())
        .with_max_iterations(5);

    let mut stream = engine.query_stream("请读取 marker 文件").await;
    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::Complete => break,
            StreamEvent::Error(error) => panic!("stream failed: {error}"),
            _ => {}
        }
    }

    let history = engine.get_history().await;
    assert!(history
        .iter()
        .any(|message| matches!(message, Message::User { .. })));
    assert!(
        history.iter().any(|message| matches!(
            message,
            Message::Assistant {
                tool_calls: None,
                ..
            }
        )),
        "final assistant should be persisted without stale tool calls: {history:?}"
    );
    assert!(
        history.iter().all(|message| !matches!(
            message,
            Message::Assistant {
                tool_calls: Some(calls),
                ..
            } if !calls.is_empty()
        )),
        "completed tool calls must not be persisted as pending provider tool calls: {history:?}"
    );
}

#[tokio::test]
async fn streaming_engine_uses_working_dir_for_relative_tool_paths() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS", "0");
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(dir.path().join("marker.txt"), "marker-content\n")
        .await
        .unwrap();
    let provider = Arc::new(RecordingToolProvider {
        requests: StdMutex::new(Vec::new()),
    });
    let mut registry = ToolRegistry::new();
    registry.register(crate::tools::file_tool::FileReadTool);
    let engine = StreamingQueryEngine::new(provider.clone(), Arc::new(registry), "MiniMax-M2.7")
        .with_working_dir(dir.path())
        .with_max_iterations(3);

    let mut stream = engine.query_stream("read marker").await;
    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::Complete => break,
            StreamEvent::Error(error) => panic!("stream failed: {error}"),
            _ => {}
        }
    }

    let requests = provider.requests.lock().unwrap();
    assert!(
        requests.iter().any(|request| request.messages.iter().any(
            |message| matches!(message, Message::System { content } if content.contains(&dir.path().display().to_string()))
        )),
        "system prompt should be assembled for selected working dir"
    );
    let tool_messages = requests
        .iter()
        .flat_map(|request| request.messages.iter())
        .filter_map(|message| match message {
            Message::Tool { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        tool_messages
            .iter()
            .any(|content| content.contains("marker-content")),
        "relative file_read should resolve inside selected working dir; tool messages: {tool_messages:?}"
    );
}

#[tokio::test]
async fn reactive_context_retry_compacts_history_before_rebuild() {
    let history = Arc::new(tokio::sync::Mutex::new(vec![
        Message::user("please inspect the large output"),
        Message::assistant(&"tool output ".repeat(500)),
        Message::user("continue"),
    ]));
    let compressor = Arc::new(tokio::sync::Mutex::new(
        crate::engine::context_compressor::ContextCompressor::new(120),
    ));
    let before_tokens = {
        let hist = history.lock().await;
        crate::engine::context_compressor::estimate_messages_tokens(&hist)
    };

    let retry_messages = reactive_context_retry_messages(
        history.clone(),
        compressor.clone(),
        "System prompt.",
        "continue",
        crate::engine::agent_mode::AgentMode::Build,
        None,
        None,
    )
    .await
    .expect("reactive context retry should rebuild messages after compaction");

    let after_tokens = {
        let hist = history.lock().await;
        crate::engine::context_compressor::estimate_messages_tokens(&hist)
    };
    assert!(after_tokens < before_tokens);
    assert!(matches!(
        retry_messages.first(),
        Some(Message::System { .. })
    ));
    let runtime_records = compressor.lock().await.compaction_records().to_vec();
    assert!(runtime_records
        .iter()
        .any(|record| record.strategy.label() == "reactive_compact"));
}

#[tokio::test]
async fn manual_compact_records_attempt_and_updates_history() {
    let provider = Arc::new(ToolTurnProvider {
        responses: StdMutex::new(VecDeque::from([crate::services::api::ChatResponse {
            content: "Large tool output was inspected.".to_string(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
        }])),
    });
    let registry = Arc::new(ToolRegistry::new());
    let engine = StreamingQueryEngine::new(provider, registry, "mock-a").with_max_context(120);
    engine
        .set_history(vec![
            Message::user("please inspect the large output"),
            Message::assistant(&"tool output ".repeat(500)),
            Message::user("continue"),
        ])
        .await;
    let before_tokens =
        crate::engine::context_compressor::estimate_messages_tokens(&engine.get_history().await);

    let attempt = engine
        .compact_context_manually()
        .await
        .expect("manual compact should record an attempt");
    let after_history = engine.get_history().await;
    let after_tokens = crate::engine::context_compressor::estimate_messages_tokens(&after_history);

    assert_eq!(
        attempt.strategy,
        crate::engine::context_collapse::ContextCompactionStrategy::SessionMemoryCompact
    );
    assert_eq!(
        attempt.decision,
        crate::engine::context_collapse::CompactionDecision::Compacted
    );
    assert!(after_tokens < before_tokens);
    let attempts = engine
        .compressor()
        .expect("compressor")
        .lock()
        .await
        .compaction_attempt_records()
        .to_vec();
    assert!(attempts
        .iter()
        .any(|record| record.decision.label() == "considered"));
    assert!(attempts
        .iter()
        .any(|record| record.decision.label() == "compacted"));
}

#[tokio::test]
async fn context_usage_report_does_not_initialize_memory_manager() {
    let provider = Arc::new(ToolTurnProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let registry = Arc::new(ToolRegistry::new());
    let engine = StreamingQueryEngine::new(provider, registry, "mock-model");

    assert!(engine.memory_manager().is_none());
    let usage = engine.context_usage_report().await;

    assert_eq!(usage.memory_snapshot_tokens, 0);
    assert!(usage.relevant_memories.is_empty());
    assert!(engine.memory_manager().is_none());
}

#[test]
fn lazy_session_binding_records_current_model() {
    let provider = Arc::new(ToolTurnProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let registry = Arc::new(ToolRegistry::new());
    let engine = StreamingQueryEngine::new(provider, registry, "mock-model");
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    assert!(engine.session_store.set(Some(store.clone())).is_ok());

    let (_bound_store, session_id) = engine.session_binding().expect("session binding");
    let session = store
        .get_session(&session_id)
        .unwrap()
        .expect("created session");

    assert_eq!(session.model, "mock-model");
}

#[tokio::test]
async fn agent_manager_is_constructed_on_demand() {
    let provider = Arc::new(ToolTurnProvider {
        responses: StdMutex::new(VecDeque::new()),
    });
    let registry = Arc::new(ToolRegistry::new());
    let query_engine = Arc::new(crate::engine::QueryEngine::new(
        provider.clone(),
        registry.clone(),
        "mock-model",
    ));
    let engine = StreamingQueryEngine::new(provider, registry, "mock-model")
        .with_agent_query_engine(query_engine);

    assert!(engine.agent_manager().is_none());
    assert!(engine.agent_manager_or_init().is_some());
    assert!(engine.agent_manager().is_some());
}

#[tokio::test]
async fn context_long_session_manual_compact_persists_boundary_for_restore() {
    let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session("long-session", "Long Session", "MiniMax-M2.7")
        .unwrap();
    let provider = Arc::new(ToolTurnProvider {
        responses: StdMutex::new(VecDeque::from([crate::services::api::ChatResponse {
            content: "README and validation facts were summarized.".to_string(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
        }])),
    });
    let registry = Arc::new(ToolRegistry::new());
    let engine = StreamingQueryEngine::new(provider, registry, "MiniMax-M2.7")
        .with_session_store(store.clone(), "long-session".to_string())
        .with_max_context(120);
    engine
        .set_history(vec![
            Message::user("read README, inspect src/lib.rs, and run cargo test"),
            Message::assistant(&"README contents and src/lib.rs details. ".repeat(220)),
            Message::user("edit config and continue"),
            Message::assistant("Edited config. cargo test passed."),
            Message::user("what did the README say earlier?"),
        ])
        .await;

    let attempt = engine
        .compact_context_manually()
        .await
        .expect("manual compaction attempt");

    assert_eq!(
        attempt.decision,
        crate::engine::context_collapse::CompactionDecision::Compacted
    );
    let boundary = store
        .latest_compact_boundary("long-session")
        .unwrap()
        .expect("compact boundary persisted");
    assert_eq!(boundary.strategy, "session_memory_compact");
    assert!(boundary.before_tokens > boundary.after_tokens);
    assert!(engine
        .get_history()
        .await
        .iter()
        .any(|message| matches!(message, Message::User { content } if content.contains("README"))));
}

#[test]
fn progressive_text_chunks_keep_short_text_single() {
    assert_eq!(progressive_text_chunks("hello"), vec!["hello".to_string()]);
}

#[test]
fn progressive_text_chunks_split_long_text_on_boundaries() {
    let text = "这是一段比较长的回答，用来模拟 non-streaming provider 返回完整文本后，桌面 UI 仍然需要渐进显示的体验。"
        .repeat(3);
    let chunks = progressive_text_chunks(&text);

    assert!(chunks.len() > 1);
    assert_eq!(chunks.concat(), text);
    assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 96));
}
