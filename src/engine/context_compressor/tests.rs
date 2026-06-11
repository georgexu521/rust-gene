use super::*;

/// 防止编译器过度优化的简单 black_box
fn black_box<T>(x: T) -> T {
    std::hint::black_box(x)
}

#[test]
fn test_token_budget() {
    let budget = TokenBudget::new(128_000);
    assert_eq!(budget.available_for_history(), 128_000 - 4096 - 2000 - 1000);
    assert!(budget.needs_compression(100_000));
    assert!(!budget.needs_compression(50_000));
}

#[test]
fn test_tail_soft_ceiling() {
    let budget = TokenBudget::new(128_000);
    let target = budget.target_tokens();
    let ceiling = budget.tail_soft_ceiling();
    assert!(ceiling > target);
    assert_eq!(ceiling, target * 150 / 100);
}

#[test]
fn test_estimate_tokens() {
    assert_eq!(estimate_tokens("hello"), 2); // 5 chars / 4 = 1.25 → 2
    assert_eq!(estimate_tokens("1234"), 1); // 4 chars / 4 = 1
    assert_eq!(estimate_tokens(""), 0);
}

#[test]
fn estimate_messages_tokens_counts_assistant_tool_calls() {
    let content_only = vec![Message::assistant("")];
    let with_tool_call = vec![Message::assistant_with_tools(
        "",
        vec![ToolCall {
            id: "call-1".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "a".repeat(80),
                "new_string": "b".repeat(80)
            }),
        }],
    )];

    assert!(estimate_messages_tokens(&with_tool_call) > estimate_messages_tokens(&content_only));
}

#[test]
fn test_structured_summary_8_sections() {
    let mut s = StructuredSummary::new();
    s.goal = "Build auth".to_string();
    s.constraints.push("Must use JWT".to_string());
    s.progress_done.push("Login done".to_string());
    s.decisions.push("Use bcrypt".to_string());
    s.files_modified.push("auth.rs".to_string());
    s.next_steps.push("Add OAuth".to_string());
    s.critical_context.push("API key in .env".to_string());
    s.tools_patterns.push("grep before edit".to_string());

    let text = s.to_text();

    assert!(text.contains("## Goal"));
    assert!(text.contains("## Constraints"));
    assert!(text.contains("## Progress"));
    assert!(text.contains("## Key Decisions"));
    assert!(text.contains("## Relevant Files"));
    assert!(text.contains("## Next Steps"));
    assert!(text.contains("## Critical Context"));
    assert!(text.contains("## Tools & Patterns"));
}

#[test]
fn test_structured_summary_merge() {
    let mut s1 = StructuredSummary::new();
    s1.goal = "Build auth".to_string();
    s1.progress_done.push("Login done".to_string());
    s1.files_modified.push("auth.rs".to_string());
    s1.critical_context.push("JWT secret in env".to_string());

    let mut s2 = StructuredSummary::new();
    s2.goal = "Build auth v2".to_string();
    s2.progress_done.push("Signup done".to_string());
    s2.next_steps.push("Add OAuth".to_string());
    s2.critical_context.push("Rate limit: 100/min".to_string());

    s1.merge(&s2);

    assert_eq!(s1.goal, "Build auth v2"); // goal 被更新
    assert_eq!(s1.progress_all().len(), 2); // progress 累积
    assert_eq!(s1.files_modified.len(), 1); // files 保留
    assert_eq!(s1.next_steps.len(), 1); // next_steps 被更新
    assert_eq!(s1.critical_context.len(), 2); // critical_context 累积
}

#[test]
fn test_summary_from_text() {
    let text = r#"## Goal
实现用户认证

## Constraints
- 必须使用 JWT
- 密码用 bcrypt

## Progress
- 完成了登录 API
- 添加了 JWT 支持

## Key Decisions
- 选择 bcrypt 而非 argon2

## Relevant Files
- src/auth.rs

## Next Steps
- 添加 OAuth

## Critical Context
- API key 存放在 .env 文件中

## Tools & Patterns
- 先 grep 再 edit"#;

    let summary = StructuredSummary::from_text(text);
    assert_eq!(summary.goal, "实现用户认证");
    assert_eq!(summary.constraints.len(), 2);
    assert_eq!(summary.progress_all().len(), 2);
    assert_eq!(summary.decisions.len(), 1);
    assert_eq!(summary.files_modified.len(), 1);
    assert_eq!(summary.next_steps.len(), 1);
    assert_eq!(summary.critical_context.len(), 1);
    assert_eq!(summary.tools_patterns.len(), 1);
}

#[test]
fn test_compress_preserves_head_and_tail() {
    let mut compressor = ContextCompressor::new(1000);

    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Hello"),
        Message::assistant("Hi!"),
        Message::user("How are you?"),
        Message::assistant("I'm fine, thanks!"),
        Message::user("What's 2+2?"),
        Message::assistant("4"),
    ];

    let compressed = compressor.compress(&messages);

    // 头部 system prompt 应该保留
    assert!(matches!(&compressed[0], Message::System { .. }));

    // 应该有摘要或尾部消息
    assert!(compressed.len() >= 2);

    // 统计
    let stats = compressor.stats();
    assert_eq!(stats.compression_count, 1);
}

#[test]
fn test_sanitize_tool_pairs_removes_orphans() {
    let messages = vec![
        Message::user("Run ls"),
        Message::assistant_with_tools(
            "Running...",
            vec![ToolCall {
                id: "call_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "ls"}),
            }],
        ),
        Message::tool("call_1", "file1.txt\nfile2.txt"),
        // 孤立的 tool result（没有对应的 call）
        Message::tool("call_orphan", "some result"),
    ];

    let sanitized = ContextCompressor::sanitize_tool_pairs(messages);
    // 孤立的 tool result 应该被移除
    assert_eq!(sanitized.len(), 3);
}

#[test]
fn test_sanitize_tool_pairs_inserts_stubs() {
    let messages = vec![
        Message::user("Run ls"),
        Message::assistant_with_tools(
            "Running...",
            vec![ToolCall {
                id: "call_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "ls"}),
            }],
        ),
        // 没有 tool result — 应该插入 stub
        Message::user("Next question"),
    ];

    let sanitized = ContextCompressor::sanitize_tool_pairs(messages);
    // 应该有 4 条消息（插入了 stub）
    assert_eq!(sanitized.len(), 4);
    // stub 应该是 tool result
    if let Message::Tool {
        tool_call_id,
        content,
    } = &sanitized[2]
    {
        assert_eq!(tool_call_id, "call_1");
        assert!(content.contains("lost"));
    } else {
        panic!("Expected stub tool result at index 2");
    }
}

#[test]
fn test_cooldown() {
    let mut compressor = ContextCompressor::new(1000);
    assert!(!compressor.is_in_cooldown());

    compressor.record_failure();
    assert!(compressor.is_in_cooldown());
}

#[test]
fn test_preflight_check() {
    let compressor = ContextCompressor::new(10_000);
    let messages = vec![Message::user("x".repeat(5000))];

    // 不超阈值
    assert!(!compressor.preflight_check(&messages, 100, 100));

    // 超阈值
    assert!(compressor.preflight_check(&messages, 5000, 5000));
}

#[test]
fn compaction_attempt_records_open_circuit_after_repeated_no_gain() {
    let mut compressor = ContextCompressor::new(10_000);

    let first = compressor.record_compaction_decision(
        CompactionAttemptInput::new(
            "test",
            ContextCompactionStrategy::AutoCompact,
            CompactionDecision::NoGain,
            1_000,
            4,
            "no reduction",
        )
        .with_after(Some(1_000), Some(4)),
    );
    assert!(!first.circuit_open);

    let second = compressor.record_compaction_decision(
        CompactionAttemptInput::new(
            "test",
            ContextCompactionStrategy::AutoCompact,
            CompactionDecision::NoGain,
            1_000,
            4,
            "no reduction",
        )
        .with_after(Some(1_000), Some(4)),
    );
    assert!(second.circuit_open);
    assert!(compressor.compaction_circuit_open());
    assert_eq!(compressor.compaction_attempt_records().len(), 2);
    assert_eq!(
        compressor.compaction_attempt_records()[1].decision,
        CompactionDecision::NoGain
    );
}

#[test]
fn successful_compaction_attempt_resets_circuit_counters() {
    let mut compressor = ContextCompressor::new(10_000);
    compressor.record_compaction_decision(
        CompactionAttemptInput::new(
            "test",
            ContextCompactionStrategy::AutoCompact,
            CompactionDecision::NoGain,
            1_000,
            4,
            "no reduction",
        )
        .with_after(Some(1_000), Some(4)),
    );
    let compacted = compressor.record_compaction_decision(
        CompactionAttemptInput::new(
            "test",
            ContextCompactionStrategy::AutoCompact,
            CompactionDecision::Compacted,
            1_000,
            4,
            "reduced",
        )
        .with_after(Some(500), Some(2))
        .with_boundary_id(Some("boundary-1".to_string())),
    );
    assert_eq!(compacted.consecutive_no_gain, 0);
    assert_eq!(compacted.consecutive_failures, 0);
    assert!(!compacted.circuit_open);
}

#[test]
fn test_align_boundary_forward_skips_orphan_tools() {
    // 头部之后有孤立的 tool results（被 summarize 后残留）
    let messages = vec![
        Message::system("You are helpful"),
        Message::tool("call_orphan_1", "old result 1"),
        Message::tool("call_orphan_2", "old result 2"),
        Message::user("What's next?"),
        Message::assistant("Let me check"),
    ];

    // align_boundary_forward 应该跳过孤立 tool results
    let aligned = ContextCompressor::align_boundary_forward(&messages, 1);
    assert_eq!(aligned, 3); // 跳过 index 1, 2（两个 tool messages）
}

#[test]
fn test_align_boundary_forward_no_tools() {
    // 没有孤立 tool results，idx 不变
    let messages = vec![
        Message::system("You are helpful"),
        Message::user("Hello"),
        Message::assistant("Hi!"),
    ];

    let aligned = ContextCompressor::align_boundary_forward(&messages, 0);
    assert_eq!(aligned, 0); // 第一条就是 user，不变
}

#[test]
fn test_summary_prefix_in_output() {
    let mut compressor = ContextCompressor::new(1000);

    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Hello"),
        Message::assistant("Hi!"),
        Message::user("How are you?"),
        Message::assistant("I'm fine!"),
        Message::user("What's 2+2?"),
        Message::assistant("4"),
    ];

    let compressed = compressor.compress(&messages);

    // 找到摘要消息，应该包含 SUMMARY_PREFIX
    let has_prefix = compressed.iter().any(|m| {
        let content = m.content();
        content.contains("[CONTEXT COMPACTION]")
    });
    assert!(
        has_prefix,
        "Compressed output should contain SUMMARY_PREFIX"
    );
}

#[test]
fn test_prune_keeps_more_context_for_critical_tool_output() {
    let mut messages = vec![Message::user("start"), Message::assistant("ok")];
    for i in 0..6 {
        let content = if i == 0 {
            format!("Result: ERROR\n{}\n", "x".repeat(1500))
        } else {
            "Result: OK\nsmall output".to_string()
        };
        messages.push(Message::tool(format!("call_{}", i), content));
    }

    let pruned = ContextCompressor::prune_old_tool_results(&messages);
    let first_tool = pruned
        .iter()
        .find_map(|m| match m {
            Message::Tool {
                tool_call_id,
                content,
            } if tool_call_id == "call_0" => Some(content.clone()),
            _ => None,
        })
        .expect("missing call_0");

    assert!(
        first_tool.len() > 200,
        "critical tool output should preserve more context"
    );
}

#[test]
fn test_summarize_middle_extracts_command_and_error_lines() {
    let mut compressor = ContextCompressor::new(1000);
    let middle = vec![
        Message::assistant_with_tools(
            "run checks",
            vec![ToolCall {
                id: "call_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "cargo check && cargo test"}),
            }],
        ),
        Message::tool(
            "call_1",
            "cargo check\nerror[E0425]: cannot find value `x` in this scope\nfailed to compile",
        ),
    ];

    let summary = compressor.summarize_middle(&middle);
    assert!(summary.contains("Command: cargo check && cargo test"));
    assert!(summary.to_lowercase().contains("error"));
}

#[test]
fn test_role_alternation_no_consecutive_same() {
    let mut compressor = ContextCompressor::new(1000);

    // 构造一个会触发压缩的消息序列
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Hello"),
        Message::assistant("Hi!"),
        Message::user("How are you?"),
        Message::assistant("I'm fine!"),
        Message::user("What's 2+2?"),
        Message::assistant("4"),
    ];

    let compressed = compressor.compress(&messages);

    // 检查没有连续相同角色（除了 system 开头 + tool 消息）
    for i in 1..compressed.len() {
        let prev_role = match &compressed[i - 1] {
            Message::User { .. } => "user",
            Message::Assistant { .. } => "assistant",
            Message::System { .. } => "system",
            Message::Tool { .. } => "tool",
        };
        let curr_role = match &compressed[i] {
            Message::User { .. } => "user",
            Message::Assistant { .. } => "assistant",
            Message::System { .. } => "system",
            Message::Tool { .. } => "tool",
        };
        // 不允许 user-user 或 assistant-assistant 连续
        if prev_role == "user" || prev_role == "assistant" {
            assert_ne!(
                prev_role,
                curr_role,
                "Found consecutive {} messages at index {}-{}",
                prev_role,
                i - 1,
                i
            );
        }
    }
}

// ── Micro-benchmarks ──

#[test]
fn bench_compress_heuristic() {
    let mut messages = vec![Message::system("You are a helpful assistant.")];
    for i in 0..100 {
        messages.push(Message::user(format!("User message number {}", i)));
        messages.push(Message::assistant(format!("Assistant reply number {}", i)));
    }
    // 添加一些 tool 消息对
    for i in 0..20 {
        messages.push(Message::user(format!("Run command {}", i)));
        messages.push(Message::assistant_with_tools(
            "Running...",
            vec![ToolCall {
                id: format!("call_{}", i),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "ls"}),
            }],
        ));
        messages.push(Message::tool(format!("call_{}", i), "file.txt\n"));
    }

    let iterations = 500;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let mut compressor = ContextCompressor::new(2000);
        let result = compressor.compress(&messages);
        let _ = black_box(result);
    }
    let elapsed = start.elapsed();
    let avg_us = elapsed.as_micros() as f64 / iterations as f64;
    println!(
        "bench_compress_heuristic: {} iterations, avg {:.1} μs/iter",
        iterations, avg_us
    );
}

#[test]
fn bench_sanitize_tool_pairs() {
    let mut messages = vec![Message::user("Start")];
    for i in 0..50 {
        messages.push(Message::assistant_with_tools(
            "Running...",
            vec![ToolCall {
                id: format!("call_{}", i),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "ls"}),
            }],
        ));
        messages.push(Message::tool(format!("call_{}", i), "result"));
    }
    // 添加孤立的 tool result
    messages.push(Message::tool("orphan", "orphan result"));

    let iterations = 5000;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let result = ContextCompressor::sanitize_tool_pairs(messages.clone());
        let _ = black_box(result);
    }
    let elapsed = start.elapsed();
    let avg_us = elapsed.as_micros() as f64 / iterations as f64;
    println!(
        "bench_sanitize_tool_pairs: {} iterations, avg {:.1} μs/iter",
        iterations, avg_us
    );
}

#[test]
fn bench_estimate_messages_tokens() {
    let mut messages = vec![Message::system("You are a helpful assistant.")];
    for i in 0..200 {
        messages.push(Message::user(format!("User message number {}", i)));
        messages.push(Message::assistant(format!("Assistant reply number {}", i)));
    }

    let iterations = 10_000;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let tokens = estimate_messages_tokens(&messages);
        let _ = black_box(tokens);
    }
    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
    println!(
        "bench_estimate_messages_tokens: {} iterations, avg {:.0} ns/iter",
        iterations, avg_ns
    );
}

// ── LLM 压缩测试 ───────────────────────────────────────────────────────────────────

use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, Usage};
use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;

struct MockLlmProvider {
    response: Option<String>,
}

#[async_trait]
impl LlmProvider for MockLlmProvider {
    async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
        match self.response.as_ref() {
            Some(content) => Ok(ChatResponse {
                content: content.clone(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
                tool_call_repair: None,
            }),
            None => Err(anyhow::anyhow!("Mock LLM error")),
        }
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        unimplemented!()
    }

    fn base_url(&self) -> &str {
        "http://localhost"
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }
}

struct CapturingLlmProvider {
    requests: std::sync::Mutex<Vec<ChatRequest>>,
    response: String,
}

#[async_trait]
impl LlmProvider for CapturingLlmProvider {
    async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
        self.requests.lock().unwrap().push(request);
        Ok(ChatResponse {
            content: self.response.clone(),
            tool_calls: None,
            usage: Some(Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                reasoning_tokens: None,
                cached_tokens: Some(80),
            }),
            tool_call_repair: None,
        })
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream> {
        unimplemented!()
    }

    fn base_url(&self) -> &str {
        "http://localhost"
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }
}

#[tokio::test]
async fn test_compress_async_with_llm_success() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_LLM_COMPACTION", "1");
    let summary_text = "## Goal\nTest goal\n\n## Constraints\n\n## Progress\n\n## Key Decisions\n\n## Relevant Files\n\n## Next Steps\n\n## Critical Context\n\n## Tools & Patterns\n";
    let provider = std::sync::Arc::new(MockLlmProvider {
        response: Some(summary_text.to_string()),
    });

    let mut compressor = ContextCompressor::new(1000).with_llm_provider(provider, "mock-model");

    let mut messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Hello"),
        Message::assistant("Hi!"),
        Message::user("How are you?"),
        Message::assistant("I'm fine, thanks!"),
        Message::user("What's 2+2?"),
        Message::assistant("4"),
    ];
    for i in 0..24 {
        messages.push(Message::user(format!(
            "Long context item {i}: {}",
            "project detail ".repeat(80)
        )));
    }

    let compressed = compressor.compress_async(&messages).await;

    // 应该生成摘要消息
    let has_summary = compressed.iter().any(|m| {
        let content = m.content();
        content.contains("[CONTEXT COMPACTION]")
    });
    assert!(
        has_summary,
        "LLM compression should produce a summary message"
    );

    let stats = compressor.stats();
    assert_eq!(stats.compression_count, 1);
    assert_eq!(stats.llm_compression_attempts, 1);
    assert_eq!(stats.llm_compression_failures, 0);
    assert!(stats.total_tokens_before > 0);
    assert!(stats.total_tokens_after > 0);
}

#[tokio::test]
async fn llm_summary_request_reuses_main_agent_stable_prefix() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_LLM_COMPACTION", "1");
    let summary_text = "## Goal\nTest goal\n\n## Constraints\n\n## Progress\n\n## Key Decisions\n\n## Relevant Files\n\n## Next Steps\n\n## Critical Context\n\n## Tools & Patterns\n";
    let provider = std::sync::Arc::new(CapturingLlmProvider {
        requests: std::sync::Mutex::new(Vec::new()),
        response: summary_text.to_string(),
    });
    let mut compressor =
        ContextCompressor::new(1000).with_llm_provider(provider.clone(), "mock-model");
    compressor.set_llm_summary_stable_prefix("MAIN AGENT STABLE PREFIX");

    let messages = vec![
        Message::user("First task"),
        Message::assistant("First answer"),
        Message::user("Second task"),
    ];
    let summary = compressor.llm_summarize_middle(&messages, None).await;

    assert!(summary.is_some());
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(matches!(
        &requests[0].messages[0],
        Message::System { content } if content == "MAIN AGENT STABLE PREFIX"
    ));
    assert!(matches!(
        &requests[0].messages[1],
        Message::User { content } if content.contains("Create a new anchored summary")
    ));
}

#[tokio::test]
async fn llm_summary_prefix_from_messages_skips_dynamic_context_zones() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
    env.set("PRIORITY_AGENT_LLM_COMPACTION", "1");
    let summary_text = "## Goal\nTest goal\n\n## Constraints\n\n## Progress\n\n## Key Decisions\n\n## Relevant Files\n\n## Next Steps\n\n## Critical Context\n\n## Tools & Patterns\n";
    let provider = std::sync::Arc::new(CapturingLlmProvider {
        requests: std::sync::Mutex::new(Vec::new()),
        response: summary_text.to_string(),
    });
    let mut compressor =
        ContextCompressor::new(1000).with_llm_provider(provider.clone(), "mock-model");
    compressor.set_llm_summary_stable_prefix_from_messages(&[
        Message::system("<context_zones>\n<task-state>volatile</task-state>\n</context_zones>"),
        Message::system("MAIN AGENT STABLE PREFIX"),
        Message::user("Compress this"),
    ]);

    let summary = compressor
        .llm_summarize_middle(&[Message::user("large tail")], None)
        .await;

    assert!(summary.is_some());
    let requests = provider.requests.lock().unwrap();
    assert!(matches!(
        &requests[0].messages[0],
        Message::System { content } if content == "MAIN AGENT STABLE PREFIX"
    ));
}

#[tokio::test]
async fn test_compress_async_falls_back_when_llm_fails() {
    let provider = std::sync::Arc::new(MockLlmProvider { response: None });

    let mut compressor = ContextCompressor::new(1000).with_llm_provider(provider, "mock-model");

    let mut messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Hello"),
        Message::assistant("Hi!"),
        Message::user("How are you?"),
        Message::assistant("I'm fine, thanks!"),
        Message::user("What's 2+2?"),
        Message::assistant("4"),
    ];
    for i in 0..24 {
        messages.push(Message::user(format!(
            "Long context item {i}: {}",
            "project detail ".repeat(80)
        )));
    }

    let compressed = compressor.compress_async(&messages).await;

    // 即使 LLM 失败，也应该有压缩输出（启发式）
    assert!(!compressed.is_empty());

    let stats = compressor.stats();
    assert_eq!(stats.llm_compression_attempts, 1);
    assert_eq!(stats.llm_compression_failures, 1);
    assert!(stats.in_cooldown);
}

#[tokio::test]
async fn test_compress_async_without_provider_uses_heuristic() {
    let mut compressor = ContextCompressor::new(1000);

    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Hello"),
        Message::assistant("Hi!"),
        Message::user("How are you?"),
        Message::assistant("I'm fine, thanks!"),
        Message::user("What's 2+2?"),
        Message::assistant("4"),
    ];

    let compressed = compressor.compress_async(&messages).await;

    assert!(!compressed.is_empty());

    let stats = compressor.stats();
    assert_eq!(stats.llm_compression_attempts, 0);
    assert_eq!(stats.llm_compression_failures, 0);
}

// ─── Long Session Stress Tests ─────────────────────────────────────────────

/// Helper to create a long conversation with many turns
fn create_long_conversation(turns: usize) -> Vec<Message> {
    let mut messages = vec![Message::system("You are a helpful coding assistant.")];
    for i in 0..turns {
        messages.push(Message::user(format!("Task {}: Implement feature X", i)));
        messages.push(Message::assistant(format!(
            "I'll implement feature X for task {}. Here's my approach...",
            i
        )));
        // Add some tool calls
        messages.push(Message::assistant_with_tools(
            format!("Tool use for task {}", i),
            vec![crate::services::api::ToolCall {
                id: format!("call_{}", i),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "echo done"}),
            }],
        ));
        messages.push(Message::tool(
            format!("call_{}", i),
            "Command executed successfully".to_string(),
        ));
    }
    messages
}

#[test]
fn test_long_session_50_turns_stability() {
    // 50 turns stress test - should remain stable
    let messages = create_long_conversation(50);
    let tokens = estimate_messages_tokens(&messages);

    // With 50 turns, we should have token usage (threshold adjusted for estimation method)
    assert!(
        tokens > 1000,
        "50 turns should use >1000 tokens, got {}",
        tokens
    );

    // Test that micro_compress doesn't panic and produces valid output
    let mut compressor = ContextCompressor::new(128_000);
    let compressed = compressor.micro_compress(&messages);

    // Compressed messages should still be valid
    assert!(!compressed.is_empty());

    // Check stats show micro_compress ran
    let stats = compressor.stats();
    assert!(stats.total_tokens_before > 0, "Should track tokens before");
}

#[test]
fn test_long_session_100_turns_stability() {
    // 100 turns stress test - compression should trigger
    let messages = create_long_conversation(100);
    let tokens = estimate_messages_tokens(&messages);

    // With 100 turns, significant token usage
    assert!(
        tokens > 2000,
        "100 turns should use >2000 tokens, got {}",
        tokens
    );

    // Test micro_compress
    let mut compressor = ContextCompressor::new(128_000);
    let compressed = compressor.micro_compress(&messages);

    assert!(!compressed.is_empty());
    // micro_compress trims tool results but doesn't remove messages

    let stats = compressor.stats();
    assert!(stats.total_tokens_before > 0);
}

#[test]
fn test_long_session_200_turns_stability() {
    // 200 turns stress test - aggressive compression
    let messages = create_long_conversation(200);
    let tokens = estimate_messages_tokens(&messages);

    // With 200 turns, very high token usage
    assert!(
        tokens > 4000,
        "200 turns should use >4000 tokens, got {}",
        tokens
    );

    // Test micro_compress handles large inputs
    let mut compressor = ContextCompressor::new(128_000);
    let compressed = compressor.micro_compress(&messages);

    assert!(!compressed.is_empty());

    // Multiple micro_compress calls should be stable
    let recompressed = compressor.micro_compress(&compressed);
    assert!(!recompressed.is_empty());
}

#[test]
fn test_micro_compress_quality_preservation() {
    // Verify that micro_compress preserves critical content
    let mut messages = vec![Message::system("You are a helpful assistant.")];
    messages.push(Message::user(
        "Remember: the API endpoint is at localhost:8080".to_string(),
    ));
    messages.push(Message::assistant(
        "I'll remember that the API is at localhost:8080".to_string(),
    ));

    // Add many filler messages
    for i in 0..50 {
        messages.push(Message::user(format!("Turn {}", i)));
        messages.push(Message::assistant(format!("Response {}", i)));
    }

    // Critical info should be preserved - check in original messages
    let api_reference = "localhost:8080";
    let has_critical = messages.iter().any(|m| match m {
        Message::User { content, .. } | Message::Assistant { content, .. } => {
            content.contains(api_reference)
        }
        _ => false,
    });
    assert!(
        has_critical,
        "Original messages should contain critical info"
    );

    let mut compressor = ContextCompressor::new(128_000);
    let compressed = compressor.micro_compress(&messages);

    // After compression, the critical info should still be present
    // (micro_compress doesn't remove content, just trims tool results)
    let preserved = compressed.iter().any(|m| match m {
        Message::User { content, .. } | Message::Assistant { content, .. } => {
            content.contains(api_reference)
        }
        _ => false,
    });
    assert!(
        preserved,
        "Compressed messages should preserve critical info"
    );
}

#[test]
fn test_compress_50_turns_stability() {
    // 50 turns: full compression pipeline should remain stable
    let messages = create_long_conversation(50);
    let tokens_before = estimate_messages_tokens(&messages);

    let mut compressor = ContextCompressor::new(32_000);
    let compressed = compressor.compress(&messages);
    let tokens_after = estimate_messages_tokens(&compressed);

    assert!(!compressed.is_empty());
    assert!(
        tokens_after < tokens_before,
        "Compression should reduce tokens: {} -> {}",
        tokens_before,
        tokens_after
    );

    let stats = compressor.stats();
    assert!(
        stats.compression_count >= 1,
        "Should have compressed at least once"
    );
}

#[test]
fn test_compress_100_turns_stability() {
    // 100 turns: aggressive compression
    let messages = create_long_conversation(100);
    let tokens_before = estimate_messages_tokens(&messages);

    let mut compressor = ContextCompressor::new(32_000);
    let compressed = compressor.compress(&messages);
    let tokens_after = estimate_messages_tokens(&compressed);

    assert!(!compressed.is_empty());
    assert!(
        tokens_after < tokens_before,
        "Compression should reduce tokens: {} -> {}",
        tokens_before,
        tokens_after
    );

    // Multiple compressions should be stable
    let recompressed = compressor.compress(&compressed);
    assert!(!recompressed.is_empty());

    let stats = compressor.stats();
    assert!(stats.compression_count >= 2);
}

#[test]
fn test_compression_level_auto_select() {
    // Low usage -> Light
    let level = CompressionLevel::auto_select(0.5, 0, 0, true);
    assert_eq!(level, CompressionLevel::Light);

    // Medium usage with LLM -> Medium
    let level = CompressionLevel::auto_select(0.75, 0, 0, true);
    assert_eq!(level, CompressionLevel::Medium);

    // Medium usage without LLM -> Light
    let level = CompressionLevel::auto_select(0.75, 0, 0, false);
    assert_eq!(level, CompressionLevel::Light);

    // High usage with LLM, first compression -> Heavy
    let level = CompressionLevel::auto_select(0.9, 0, 0, true);
    assert_eq!(level, CompressionLevel::Heavy);

    // High usage with LLM, already compressed -> Medium
    let level = CompressionLevel::auto_select(0.9, 3, 0, true);
    assert_eq!(level, CompressionLevel::Medium);

    // High usage with LLM failures -> Medium
    let level = CompressionLevel::auto_select(0.9, 0, 5, true);
    assert_eq!(level, CompressionLevel::Medium);
}

#[test]
fn test_compress_with_level_none() {
    let messages = create_long_conversation(10);
    let mut compressor = ContextCompressor::new(128_000);
    let compressed = compressor.compress_with_level(&messages, CompressionLevel::None);
    assert_eq!(compressed.len(), messages.len());
}

#[test]
fn test_compress_with_level_light() {
    let messages = create_long_conversation(20);
    let tokens_before = estimate_messages_tokens(&messages);
    let mut compressor = ContextCompressor::new(128_000);
    let compressed = compressor.compress_with_level(&messages, CompressionLevel::Light);
    let tokens_after = estimate_messages_tokens(&compressed);
    assert!(!compressed.is_empty());
    // Light compression trims tool results but doesn't summarize
    assert!(tokens_after <= tokens_before);
}

#[test]
fn test_snip_tool_results_records_strategy() {
    let messages = vec![
        Message::tool("call_1", "x".repeat(500)),
        Message::tool("call_2", "recent"),
        Message::tool("call_3", "recent"),
        Message::tool("call_4", "recent"),
    ];
    let mut compressor = ContextCompressor::new(128_000);

    let compressed = compressor.snip_tool_results(&messages);
    let record = compressor.latest_compaction_record().unwrap();

    assert_eq!(record.strategy, ContextCompactionStrategy::Snip);
    assert_eq!(record.messages_before, messages.len());
    assert_eq!(record.messages_after, compressed.len());
    assert_eq!(record.stage_order, vec!["snip_tool_results".to_string()]);
    assert_eq!(
        record.token_delta,
        i64::try_from(record.tokens_after).unwrap() - i64::try_from(record.tokens_before).unwrap()
    );
    assert_eq!(record.token_pressure, Some(ContextTokenPressure::Low));
    assert!(record
        .retained_items
        .contains(&"recent_tool_results:last_3".to_string()));
    assert!(record.provenance.iter().any(|p| p == "tool_result_snip"));
}

#[test]
fn test_micro_compress_records_strategy_and_provenance() {
    let messages = create_long_conversation(5);
    let mut compressor = ContextCompressor::new(128_000);

    let compressed = compressor.micro_compress(&messages);
    let record = compressor.latest_compaction_record().unwrap();

    assert_eq!(record.strategy, ContextCompactionStrategy::MicroCompact);
    assert_eq!(record.level.as_deref(), Some("light"));
    assert_eq!(record.messages_after, compressed.len());
    assert_eq!(
        record.stage_order,
        vec![
            "snip_tool_results".to_string(),
            "sanitize_tool_pairs".to_string()
        ]
    );
    assert!(record
        .retained_items
        .contains(&"tool_call_pairs:sanitized".to_string()));
    assert!(record.provenance.iter().any(|p| p == "tool_pair_sanitize"));
}

#[test]
fn test_compress_with_level_medium() {
    let messages = create_long_conversation(50);
    let tokens_before = estimate_messages_tokens(&messages);
    let mut compressor = ContextCompressor::new(32_000);
    let compressed = compressor.compress_with_level(&messages, CompressionLevel::Medium);
    let tokens_after = estimate_messages_tokens(&compressed);
    assert!(!compressed.is_empty());
    assert!(
        tokens_after < tokens_before,
        "Medium compression should reduce tokens: {} -> {}",
        tokens_before,
        tokens_after
    );
}

#[test]
fn test_time_based_compression_triggers() {
    use std::time::Duration;

    let config = TimeBasedConfig {
        session_duration_threshold_secs: 1, // 1 second threshold
        message_count_threshold: 5,
        ..TimeBasedConfig::default()
    };

    let mut compressor = ContextCompressor::new(128_000);
    compressor.time_config = config;

    // Create a session start time in the past
    compressor.session_start = std::time::Instant::now() - Duration::from_secs(10);

    // Should trigger time-based compression
    let messages: Vec<Message> = (0..3)
        .map(|i| Message::user(format!("Message {}", i)))
        .collect();

    assert!(compressor.needs_time_based_compression(&messages));
}

#[test]
fn test_compression_warning_levels() {
    let compressor = ContextCompressor::new(100_000); // Small window

    // 50% usage - should be None or Approaching
    let low_messages = vec![
        Message::system("System"),
        Message::user("Hi"),
        Message::assistant("Hello"),
    ];

    // With small window, even few messages might approach limit
    let warning = compressor.warning_level(&low_messages);
    assert!(matches!(
        warning,
        CompressionWarning::None | CompressionWarning::Approaching
    ));
}

#[test]
fn test_compact_boundary_marker() {
    let meta = CompactMetadata {
        sequence: 1,
        boundary_id: "cb-test-123".to_string(),
        preserved_tail_count: 3,
        messages_before: 20,
        messages_after: 5,
        tokens_before: 8000,
        tokens_after: 3000,
        timestamp: "2026-04-23T10:00:00+08:00".to_string(),
    };

    let marker = meta.to_boundary_marker();
    assert!(marker.contains("COMPACT_BOUNDARY"));
    assert!(marker.contains("seq=1"));
    assert!(marker.contains("id=cb-test-123"));

    // Parse it back
    let (parsed, clean) =
        CompactMetadata::parse_from_text(&format!("Summary text{}", marker)).unwrap();
    assert_eq!(parsed.sequence, 1);
    assert_eq!(parsed.boundary_id, "cb-test-123");
    assert_eq!(parsed.preserved_tail_count, 3);
    assert_eq!(parsed.messages_before, 20);
    assert_eq!(parsed.tokens_before, 8000);
    assert!(clean.starts_with("Summary text"));
}

#[test]
fn test_compact_boundary_embedded_in_compression() {
    let mut compressor = ContextCompressor::new(2000);

    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::user("Task 1: do something"),
        Message::assistant("Done with task 1.".to_string()),
        Message::user("Task 2: do more"),
        Message::assistant("Done with task 2.".to_string()),
        Message::user("Task 3: do even more"),
        Message::assistant("Done with task 3.".to_string()),
        Message::user("Task 4: final task"),
    ];

    let compressed = compressor.compress(&messages);

    // 应该有 compact boundary 被嵌入
    let boundaries = extract_compact_boundaries(&compressed);
    assert_eq!(boundaries.len(), 1, "Should have one compact boundary");
    assert_eq!(boundaries[0].sequence, 1);
    assert!(boundaries[0].messages_before > 0);
    assert!(boundaries[0].tokens_before > 0);

    // compressor 应该记录了历史
    assert_eq!(compressor.compact_metadata_history.len(), 1);
    let record = compressor.latest_compaction_record().unwrap();
    assert_eq!(record.strategy, ContextCompactionStrategy::AutoCompact);
    assert!(record.boundary_id.is_some());
    assert_eq!(record.sequence, Some(1));
    assert!(record
        .retained_items
        .iter()
        .any(|item| item.starts_with("tail_messages:")));
    assert!(record
        .provenance
        .iter()
        .any(|p| p.starts_with("compact_boundary:")));
}

#[test]
fn test_session_memory_compact_analyze() {
    let messages = vec![
        Message::system("System prompt"),
        Message::user("Read src/main.rs and src/lib.rs"),
        Message::assistant("I read src/main.rs and src/lib.rs"),
        Message::tool("call_1", "Content of src/main.rs"),
        Message::user("Now read src/config.rs"),
        Message::assistant("I read src/config.rs and src/main.rs again"),
        Message::user("TODO: fix the bug in src/main.rs"),
    ];

    let smc = SessionMemoryCompact::analyze(&messages);

    // hot_files 应该包含出现频率高的文件
    assert!(!smc.hot_files.is_empty(), "Should detect hot files");
    assert!(
        smc.hot_files.iter().any(|f| f.contains("src/main.rs")),
        "Should detect main.rs"
    );

    // pending_tasks 应该包含 TODO
    assert!(!smc.pending_tasks.is_empty(), "Should detect pending tasks");
    assert!(
        smc.pending_tasks.iter().any(|t| t.contains("TODO")),
        "Should detect TODO"
    );
}

#[test]
fn test_session_memory_compact_inject() {
    let smc = SessionMemoryCompact {
        hot_files: vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
        user_preferences: vec!["Use concise output".to_string()],
        pending_tasks: vec!["TODO: fix bug".to_string()],
        tool_patterns: vec!["file_read".to_string()],
    };

    let mut summary = "Summary text".to_string();
    smc.inject_into_summary(&mut summary);

    assert!(summary.contains("User Preferences"));
    assert!(summary.contains("Use concise output"));
    assert!(summary.contains("Frequently Accessed Files"));
    assert!(summary.contains("src/main.rs"));
    assert!(summary.contains("Pending Tasks"));
    assert!(summary.contains("TODO: fix bug"));
    assert!(summary.contains("Common Tool Patterns"));
    assert!(summary.contains("file_read"));

    let tags = smc.provenance_tags();
    assert!(tags.contains(&"session_memory:user_preferences=1".to_string()));
    assert!(tags.contains(&"session_memory:hot_files=2".to_string()));
}

#[test]
fn test_runtime_continuity_facts_extract_labeled_state() {
    let messages = vec![
        Message::assistant(
            "Active objective: finish Phase 8 compaction survivability\n\
             Changed files: src/engine/context_compressor.rs, docs/PROJECT_STATUS.md\n\
             Tool round: round_abc checkpoint-backed 2 file changes\n\
             Validation passed: cargo test -q context_compressor\n\
             Terminal task: shell_background_1 output_path=.priority-agent/tool-results/out.txt status=running\n\
             Permission requested: bash risk_level=medium matched_rules=[git push]\n\
             Attached context: current_diff files=src/engine/context_compressor.rs\n\
             Diagnostics delta: improved errors -1 warnings 0\n\
             agent_id=agent_1 task_id=task_1 status=running worktree=/tmp/agent-worktree",
        ),
        Message::user("This unrelated sentence mentions objective but is not runtime state."),
    ];

    let facts = RuntimeContinuityFacts::analyze(&messages);

    assert_eq!(facts.active_objectives.len(), 1);
    assert_eq!(facts.changed_files.len(), 1);
    assert_eq!(facts.file_change_rounds.len(), 1);
    assert_eq!(facts.validation_states.len(), 1);
    assert_eq!(facts.terminal_task_states.len(), 1);
    assert_eq!(facts.permission_states.len(), 1);
    assert_eq!(facts.context_attachments.len(), 1);
    assert_eq!(facts.diagnostic_states.len(), 1);
    assert_eq!(facts.subagent_task_states.len(), 1);
    assert!(facts
        .retained_items()
        .contains(&"runtime_state_active_objectives:1".to_string()));
    assert!(facts
        .retained_items()
        .contains(&"runtime_state_subagent_tasks:1".to_string()));
    assert!(facts
        .retained_items()
        .contains(&"runtime_state_file_change_rounds:1".to_string()));
    assert!(facts
        .retained_items()
        .contains(&"runtime_state_terminal_tasks:1".to_string()));
    assert!(facts
        .retained_items()
        .contains(&"runtime_state_permissions:1".to_string()));
    assert!(facts
        .retained_items()
        .contains(&"runtime_state_context_attachments:1".to_string()));
    assert!(facts
        .retained_items()
        .contains(&"runtime_state_diagnostics:1".to_string()));
}

#[test]
fn test_long_task_compaction_preserves_runtime_continuity_state() {
    let mut messages = vec![
        Message::system("You are a coding agent."),
        Message::user("Please continue the release plan."),
    ];
    for i in 0..30 {
        messages.push(Message::assistant(format!(
            "Implementation note {}: repeated middle context {}",
            i,
            "x".repeat(120)
        )));
    }
    messages.push(Message::assistant(
        "Active objective: finish Phase 8 compaction survivability\n\
         Changed files: src/engine/context_compressor.rs, docs/CLAUDE_CODE_PROGRAMMING_PARITY_RELEASE_PLAN_2026-05-22.md\n\
         Tool round: round_phase8 checkpoint-backed 2 file changes\n\
         Validation passed: cargo test -q context_compressor\n\
         Terminal task: shell_background_1 output_path=.priority-agent/tool-results/out.txt status=running\n\
         Permission pending: bash risk_level=medium recovery=approve once or reject\n\
         Attached context: current_diff files=src/engine/context_compressor.rs,docs/CLAUDE_CODE_TOOL_FILE_RELIABILITY_AUDIT_PLAN_2026-05-23.md\n\
         Diagnostics: cargo check improved after src/engine/context_compressor.rs\n\
         agent_id=agent_1 task_id=task_1 status=running worktree=/tmp/agent-worktree branch=codex/agent-1234",
    ));
    for i in 30..60 {
        messages.push(Message::assistant(format!(
            "Later implementation note {}: repeated middle context {}",
            i,
            "y".repeat(120)
        )));
    }
    messages.push(Message::user("Continue from the current state."));

    let mut compressor = ContextCompressor::new(3_000);
    let compressed =
        compressor.compress_with_summary(&messages, Some("## Goal\nContinue Phase 8\n"));
    let compacted_text = compressed
        .iter()
        .map(|msg| msg.content())
        .collect::<Vec<_>>()
        .join("\n");
    let record = compressor.latest_compaction_record().unwrap();

    assert!(compacted_text.contains("## Runtime Continuity"));
    assert!(compacted_text.contains("Active objective: finish Phase 8"));
    assert!(compacted_text.contains("Changed files: src/engine/context_compressor.rs"));
    assert!(compacted_text.contains("Tool round: round_phase8"));
    assert!(compacted_text.contains("Validation passed: cargo test -q context_compressor"));
    assert!(compacted_text.contains("Terminal task: shell_background_1"));
    assert!(compacted_text.contains("Permission pending: bash"));
    assert!(compacted_text.contains("Attached context: current_diff"));
    assert!(compacted_text.contains("Diagnostics: cargo check improved"));
    assert!(compacted_text.contains("agent_id=agent_1 task_id=task_1"));
    assert!(record
        .retained_items
        .contains(&"runtime_state_active_objectives:1".to_string()));
    assert!(record
        .retained_items
        .contains(&"runtime_state_changed_files:1".to_string()));
    assert!(record
        .retained_items
        .contains(&"runtime_state_validation:1".to_string()));
    assert!(record
        .retained_items
        .contains(&"runtime_state_file_change_rounds:1".to_string()));
    assert!(record
        .retained_items
        .contains(&"runtime_state_terminal_tasks:1".to_string()));
    assert!(record
        .retained_items
        .contains(&"runtime_state_permissions:1".to_string()));
    assert!(record
        .retained_items
        .contains(&"runtime_state_context_attachments:1".to_string()));
    assert!(record
        .retained_items
        .contains(&"runtime_state_diagnostics:1".to_string()));
    assert!(record
        .retained_items
        .contains(&"runtime_state_subagent_tasks:1".to_string()));
    assert!(record
        .provenance
        .iter()
        .any(|p| p == "summary_memory:runtime_continuity"));
    assert!(record
        .stage_order
        .contains(&"restore_runtime_continuity".to_string()));
    assert_eq!(
        record.token_delta,
        i64::try_from(record.tokens_after).unwrap() - i64::try_from(record.tokens_before).unwrap()
    );
}

#[test]
fn test_extract_compact_boundaries_from_messages() {
    let msg1 = Message::system("Normal system message");
    let msg2 = Message::user(format!(
        "User message with boundary{}\nmore text",
        CompactMetadata {
            sequence: 2,
            boundary_id: "cb-abc".to_string(),
            preserved_tail_count: 2,
            messages_before: 10,
            messages_after: 3,
            tokens_before: 5000,
            tokens_after: 2000,
            timestamp: "2026-04-23T10:00:00+08:00".to_string(),
        }
        .to_boundary_marker()
    ));

    let boundaries = extract_compact_boundaries(&[msg1, msg2]);
    assert_eq!(boundaries.len(), 1);
    assert_eq!(boundaries[0].sequence, 2);
    assert_eq!(boundaries[0].boundary_id, "cb-abc");
}

#[test]
fn summary_template_includes_all_required_sections() {
    let template = crate::engine::context_compressor::SUMMARY_TEMPLATE;
    // All 8 sections must be present.
    assert!(template.contains("## Goal"));
    assert!(template.contains("## Constraints"));
    assert!(template.contains("## Progress"));
    assert!(template.contains("## Key Decisions"));
    assert!(template.contains("## Relevant Files"));
    assert!(template.contains("## Next Steps"));
    assert!(template.contains("## Critical Context"));
    assert!(template.contains("## Tools & Patterns"));
    // Template must make it clear this is continuation context.
    let prefix = crate::engine::context_compressor::SUMMARY_PREFIX;
    assert!(prefix.contains("compacted"));
    assert!(prefix.contains("context space"));
}
