# New Feature Eval Plan — Batch D

Date: 2026-06-01

This batch tests the 6 agent-skill optimizations added in Phase A/B/C.  
Unlike the existing Batch A/B/C which test general coding capability,  
these tasks specifically verify the new runtime features work end-to-end.

## Principles

- Each task targets ONE new feature. Overlap is noted but not required.
- Tasks are designed to be runnable with `cargo test` integration tests  
  AND as manual agent-run eval scenarios.
- If a manual agent run is not available, the integration test serves as  
  the pass/fail gate.

---

## Scorecard

| # | Task | Feature Under Test | Type |
|---|---|---|---|
| D1 | Read-before-Edit Block | Phase A #2 | integration test |
| D2 | Stale Edit + Re-read | Phase A #2 | agent-run |
| D3 | User Steer Self-Correction | Phase A #3 | integration test |
| D4 | Force Summary Wrap-up | Phase A #4 | integration test |
| D5 | Parallel Read Dispatch | Phase B #1 | integration test |
| D6 | Storm Breaker Suppression | repair #1 | integration test |
| D7 | Truncation Repair | repair #2 | integration test |
| D8 | Git Rollback on Failure | repair #3 | agent-run |
| D9 | Sub-agent Progress Events | Phase B #5 | integration test |
| D10 | Decision Timeline Replay | Phase C #6 | integration test |

---

## Task Details

### D1 — Read-before-Edit Block

**Feature**: Phase A #2 — `check_read_before_write` enforcement

**Integration test**:

```rust
// tests/feature_read_before_edit.rs
#[tokio::test]
async fn write_without_read_is_blocked() {
    let mut env = EnvGuard::acquire().await;
    env.set("PRIORITY_AGENT_READ_BEFORE_EDIT", "1");

    // Create tool context with fresh session
    let ctx = ToolContext {
        session_id: "test-read-before-edit".into(),
        working_dir: temp_dir(),
        ..Default::default()
    };

    // Try to write a file without reading it first
    let tool = FileWriteTool::new();
    let params = json!({
        "path": "new_file.txt",
        "content": "should be blocked"
    });
    let result = tool.execute(params, ctx).await;

    // Should return error about not reading the file first
    assert!(!result.success);
    assert!(result.content.contains("has not been read"));
}
```

**Verification**: `cargo test feature_read_before_edit`

---

### D2 — Stale Edit + Re-read

**Feature**: Phase A #2 — Read-before-Edit with re-read flow

**Agent-run**:

```
Task: Add a doc comment to src/engine/repair/storm.rs::StormState::check().
      But: the file has been externally modified since the agent last read it.
      The agent must re-read before editing.
```

**Setup**: Before running, modify `storm.rs` externally (add a comment line).  
Then ask the agent to edit. The stale-read detection should trigger.

**Expected behavior**:
1. Agent reads the file (first read — stale state)
2. Agent tries to edit → Read-before-Edit allows (file was read)
3. But the FileStateTracker detects the file was modified externally → warns

**Pass**: Agent successfully edits after re-reading OR produces a clear warning about stale state.

---

### D3 — User Steer Self-Correction

**Feature**: Phase A #3 — `replace_last_assistant_message`

**Integration test**:

```rust
// tests/feature_self_correction.rs
#[test]
fn drift_signal_replaces_last_assistant_message() {
    let mut messages = vec![
        Message::user("Write a function in PascalCase"),
        Message::assistant("Here is the function in PascalCase: ..."),
    ];

    // User corrects — "跑偏了" is a drift signal
    messages.push(Message::user("跑偏了，用 snake_case"));

    replace_last_assistant_message(&mut messages, "用 snake_case");

    // The last assistant message should now contain the correction
    let last_assistant = messages.iter().rfind(|m| matches!(m, Message::Assistant { .. }));
    assert!(last_assistant.unwrap().content().contains("snake_case"));
    // The old "PascalCase" content should be gone
    assert!(!messages.iter().any(|m| m.content().contains("PascalCase")));
}
```

**Verification**: `cargo test feature_self_correction`

---

### D4 — Force Summary Wrap-up

**Feature**: Phase A #4 — `force_summary` injection

**Integration test**:

```rust
#[test]
fn force_summary_injected_in_last_two_iterations() {
    assert!(should_force_summary(8, 10));   // last-2
    assert!(should_force_summary(9, 10));   // last-1
    assert!(!should_force_summary(7, 10));  // not yet
}

#[test]
fn force_summary_message_is_system_with_wrap_up() {
    let msg = force_summary_message();
    assert!(matches!(msg, Message::System { .. }));
    assert!(msg.content().contains("wrap-up"));
    assert!(msg.content().contains("summarize"));
}
```

**Agent-run**:

```
Task: Set PRIORITY_AGENT_MAX_ITERATIONS=3, then ask the agent to
      "write a complete REST API with user auth, rate limiting, and
       database migrations for a todo app".

      Because the task is too large for 3 iterations, the force-summary
      should trigger in iteration 2 and produce a wrap-up.
```

**Pass**: Agent produces a summary of what was accomplished (not an error or empty output).

---

### D5 — Parallel Read Dispatch

**Feature**: Phase B #1 — Two-phase parallel dispatch

**Integration test**:

```rust
use std::time::Instant;

#[tokio::test]
async fn parallel_reads_faster_than_sequential() {
    // Create a MockProvider that returns 3 file_read tool calls
    let provider = MockProvider::with_streams(vec![
        stream_tool_call_response("c1", "file_read", json!({"path": "a.txt"})),
        stream_tool_call_response("c2", "file_read", json!({"path": "b.txt"})),
        stream_tool_call_response("c3", "file_read", json!({"path": "c.txt"})),
    ]);
    // ... (full streaming engine setup)

    let start = Instant::now();
    let events = run_streaming_query(&engine, "read three files").await;
    let elapsed = start.elapsed();

    // Three parallel reads should be faster than 3 × single read time
    // Each read takes ~50ms in mock, so parallel ~50ms vs sequential ~150ms
    assert!(elapsed.as_millis() < 120, "parallel reads should be fast, got {:?}", elapsed);
}
```

**Pass**: 3 file_read calls complete in roughly the time of 1 (not 3).

---

### D6 — Storm Breaker Suppression

**Feature**: Repair #1 — `StormState` duplicate detection

**Existing test** (already passing): `src/engine/repair/storm.rs`

```rust
#[test]
fn storm_suppresses_repeated_calls() {
    let mut state = StormState::default();
    let args = json!({"path": "/tmp/a.txt"});
    // First 3 — allowed
    state.check("file_read", &args); // Allow
    state.check("file_read", &args); // Allow
    state.check("file_read", &args); // Allow
    // 4th — suppressed
    assert!(matches!(state.check("file_read", &args), StormDecision::Suppress(_)));
}
```

**Agent-run** (hard to trigger intentionally): Skip — integration test is sufficient.

---

### D7 — Truncation Repair

**Feature**: Repair #2 — `repair_truncated_json`

**Existing test** (already passing): `src/engine/repair/truncation.rs`

```rust
#[test]
fn missing_closing_brace() {
    let raw = r#"{"path": "/tmp/test.txt", "content": "hello"#;
    let result = repair_truncated_json(raw).unwrap();
    assert!(result.is_object());
}

#[test]
fn unterminated_string_closed() {
    let raw = r#"{"query": "hello world"#;
    let result = repair_truncated_json(raw).unwrap();
    assert_eq!(result["query"], "hello world");
}
```

---

### D8 — Git Rollback on Failure

**Feature**: Repair #3 — `GitRollbackGuard`

**Agent-run**:

```
Setup: The repo must be clean. Set PRIORITY_AGENT_GIT_ROLLBACK=on.

Task: "Edit src/engine/repair/storm.rs to add a syntax error
       (remove a closing brace), then run cargo check and fix it."

Expected: The agent makes the edit → cargo check fails → the agent
         tries to fix → eventually succeeds.

Git rollback test: After the agent finishes successfully, run
`git stash list` — there should be at least one stash entry
from the auto-checkpoint.

Undo the stashes: `git stash pop`
```

**Pass**: `git stash list` shows Priority Agent's checkpoint stash.

---

### D9 — Sub-agent Progress Events

**Feature**: Phase B #5 — `AgentProgressEvent` + `subscribe_progress`

**Integration test**:

```rust
#[tokio::test]
async fn progress_events_stream_from_sub_agent() {
    let manager = AgentManager::new();

    let config = AgentConfig {
        id: "test-sub".into(),
        task: "read Cargo.toml".into(),
        system_prompt: "You are a test agent.".into(),
        ..AgentConfig::task("read Cargo.toml")
    };

    let (handle, mut rx) = manager.spawn_with_progress(config).await?;

    // Wait for progress events
    let mut events = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Ok(event) = rx.recv().await {
            events.push(event);
        }
    }).await.ok();

    // Should receive at least Started and either Completed or Failed
    assert!(events.iter().any(|e| matches!(e, AgentProgressEvent::Started { .. })));
}
```

**Pass**: Sub-agent progress events are received.

---

### D10 — Decision Timeline Replay

**Feature**: Phase C #6 — `DecisionTimeline::from_events`

**Existing test** (already passing): `src/engine/trace_replay.rs`

```rust
#[test]
fn tool_events_are_tracked() {
    let events = vec![
        TraceEvent::ToolStarted { tool: "file_read".into(), call_id: "c1".into(), parallel: false, pre_executed: false },
        TraceEvent::ToolCompleted { tool: "file_read".into(), call_id: "c1".into(), success: true, duration_ms: 42, output_chars: 100 },
    ];
    let timeline = DecisionTimeline::from_events(1, &events);
    assert_eq!(timeline.decisions.len(), 2);
    assert!(timeline.decisions[0].summary.contains("file_read"));
}

#[test]
fn timeline_json_is_valid() {
    // ... verifies JSON export works
}
```

---

## Run Plan

### Quick Gate (unit tests, < 1s)

```bash
cargo test engine::repair::            # D6, D7 (16 tests)
cargo test engine::trace_replay         # D10 (2 tests)
cargo test engine::conversation_loop::force_summary  # D4 (3 tests)
```

### Integration Tests (need env setup)

```bash
cargo test feature_read_before_edit     # D1
cargo test feature_self_correction      # D3
cargo test feature_parallel_dispatch    # D5
cargo test feature_subagent_progress    # D9
```

### Agent-run Tasks (manual, need LLM)

```bash
# D2 — Stale edit + re-read
# D8 — Git rollback on failure
```

---

## Expected Results

Based on the design of each feature:

| Task | Expected | Risk |
|---|---|---|
| D1 Read-before-Edit | PASS — gate blocks write | Low |
| D2 Stale Edit | PASS — agent re-reads and edits | Medium |
| D3 Self-Correction | PASS — message replaced | Low |
| D4 Force Summary | PASS — wrap-up injected | Low |
| D5 Parallel Read | PASS — reads < 120ms | Medium |
| D6 Storm Breaker | PASS — already tested | None |
| D7 Truncation | PASS — already tested | None |
| D8 Git Rollback | PASS — stash created | Medium |
| D9 Progress Events | PASS — events streamed | Medium |
| D10 Timeline | PASS — already tested | None |

All 10 tasks should pass. Any failure indicates a regression or design flaw in the feature.
