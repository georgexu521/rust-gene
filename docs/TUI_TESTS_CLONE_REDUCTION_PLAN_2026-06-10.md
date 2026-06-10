# TUI Test Coverage + Hot Path Clone Reduction Plan

Date: 2026-06-10
Status: Active

## Goal

Two independent quality improvements:
1. Add targeted tests for the most-impactful untested TUI business logic (6800+ lines)
2. Eliminate the top clone patterns in the conversation loop hot path (560+ clones)

Neither requires architectural changes. Both are safe, incremental wins.

## Issue 1: TUI Business Logic Tests

### Current State

Five files contain 6800+ lines of business logic with zero or near-zero tests:

| File | Lines | Tests |
|------|-------|-------|
| `src/tui/slash_handler/learning.rs` | 1473 | 0 |
| `src/tui/slash_handler/agents.rs` | 1440 | 332 (agents/tests.rs, narrow) |
| `src/tui/slash_handler/session/actions.rs` | 1335 | 0 |
| `src/tui/app.rs` | 1308 | ✅ has tests |
| `src/tui/mod.rs` | 1274 | ✅ has tests |

The good news: 10+ pure formatting/labeling functions in `learning.rs` take `&TurnTrace`
or `&LearningEventRecord` and return `String`/`Option<String>`. These are trivially
testable — zero TUI dependencies.

### What To Test

**Batch 1: Pure formatters (highest ROI, easiest)**

| Function | File | Line |
|----------|------|------|
| `compact_inline` | learning.rs | 533 |
| `goal_drift_count_label` | learning.rs | 469 |
| `format_goal_drift_report` | learning.rs | 489 |
| `count_debug_values` | learning.rs | 843 |
| `format_counts` | learning.rs | 851 |
| `is_evolution_learning_event` | learning.rs | 733 |
| `format_learning_event_detail` | learning.rs | 575 |
| `format_experience_event` | learning.rs | 862 |
| `latest_resource_policy_label` | learning.rs | 147 |
| `latest_contract_state_label` | learning.rs | 168 |
| `latest_retrieval_context_label` | learning.rs | 232 |
| `latest_reflection_label` | learning.rs | 257 |
| `latest_stage_validation_label` | learning.rs | 276 |
| `latest_workflow_plan_label` | learning.rs | 300 |
| `latest_closeout_label` | learning.rs | 331 |
| `latest_acceptance_label` | learning.rs | 364 |
| `latest_guided_debugging_label` | learning.rs | 392 |

Batch: ~17 pure functions, 50-80 lines of test code total.

**Batch 2: Session action helpers**

| Function | File | Line |
|----------|------|------|
| `parse_export_format` | actions.rs | 682 |
| `parse_export_privacy` | actions.rs | 690 |
| `diagnostic_failed_tool_names` | actions.rs | 490 |
| `diagnostic_validation_status` | actions.rs | 505 |
| `tool_run_looks_like_validation` | actions.rs | 535 |

Batch: 5 pure functions with real validation logic.

**Batch 3: Agent helpers**

| Function | File |
|----------|------|
| `handle_voice` | agents.rs:1172 |
| `handle_telemetry` | agents.rs:1205 |
| `format_provider_status_summary` | doctor_formatting.rs |
| `format_effective_config_summary` | doctor_formatting.rs |

Batch: 4 extractable functions (no app state needed).

**Total: ~26 functions, estimated 100-200 lines of test code.**

## Issue 2: Hot Path Clone Reduction

### Current State

560+ `.clone()` calls in `conversation_loop/`, 91 more in `streaming.rs`. The top
offenders:

| File | Clones | Category |
|------|--------|----------|
| `streaming.rs` | 91 | 60 Arc, 8 large Vec, 5 String |
| `api_request_controller.rs` | 46 | 30 String, 3 ChatRequest, 2 Vec |
| `session_processor.rs` | 43 | 15 String, 8 Value/Vec, 10 Arc |

### What To Fix

**Fix 1: Eliminate model-name string cloning in api_request_controller.rs**

The model name is computed once (`model.clone()` or `self.model.clone()`) then
re-cloned 10-16 times in trace events, diagnostic emissions, and retry observers.

Fix: bind `let model = self.model.as_str()` at the top of the main request
function. All downstream consumers already take `&str` or `impl Into<String>`
— the clones are unnecessary.

Lines affected: `api_request_controller.rs` 78, 145, 160, 179, 188, 202, 266,
279, 299, 311, 360, 371, 384, 396, 410, 422.

**Fix 2: Share parsed tool args with Arc in session_processor.rs**

`parsed_args` is `serde_json::Value`, cloned 3 times for three closures in the
same spawned task (pre-tool hook, execution, post-tool hook). For complex tools
with large arguments, this clones potentially large JSON.

Fix: wrap in `Arc<serde_json::Value>` after parsing, clone the `Arc` into each
closure (ref-count bump, not data copy).

Lines affected: `session_processor.rs` 428, 464, 484.

**Fix 3: Defer fallback clones in session_processor.rs**

`fallback_messages.clone()` and `fallback_tools.clone()` are taken BEFORE
the streaming attempt. They're only used if streaming fails.

Fix: move the clones inside the error handling branch.

Lines affected: `session_processor.rs` 246-247.

**Fix 4: Avoid full-history Vec clone in streaming.rs**

`hist.clone()` at line 1382 clones the entire conversation history for a memory
flush operation that only reads the last few messages.

Fix: pass `&[Message]` to the flush function or extract only the needed tail.

Lines affected: `streaming.rs` 1032, 1382.

**Total: ~25 unnecessary clones eliminated with minimal refactoring.**

## Implementation Plan

### Step 1: TUI Pure Formatters — Tests Only

Files: `src/tui/slash_handler/learning.rs`, new `src/tui/slash_handler/learning/tests.rs`

Add tests for the 17 `latest_*_label` functions and `compact_inline`/`count_debug_values`/`format_counts`. These all take pure data and return strings. Zero TUI state needed.

Verification:
```bash
cargo test -q learning --lib
```

### Step 2: TUI Session Actions — Tests Only

Files: `src/tui/slash_handler/session/actions.rs`

Add tests for the 5 parser/validation helpers.

Verification:
```bash
cargo test -q session_actions --lib
```

### Step 3: Eliminate model-name String clones

File: `src/engine/conversation_loop/api_request_controller.rs`

Replace `model.clone()` with `model.as_str()` references.

Verification:
```bash
cargo test -q api_request_controller --lib
```

### Step 4: Share parsed_args with Arc

File: `src/engine/conversation_loop/session_processor.rs`

Wrap `parsed_args` in `Arc` after parsing, clone the Arc into closures.

Verification:
```bash
cargo test -q session_processor --lib
```

### Step 5: Defer fallback clones

File: `src/engine/conversation_loop/session_processor.rs`

Move `fallback_messages` and `fallback_tools` clones into the error branch.

Verification:
```bash
cargo test -q session_processor --lib
```

### Step 6: Avoid full-history Vec clone

File: `src/engine/streaming.rs`

Change memory flush to accept `&[Message]` instead of cloning the full history.

Verification:
```bash
cargo test -q streaming --lib
```

## Validation

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q
bash scripts/daily-baseline.sh
```

## Expected Outcome

- TUI: 26 new tests covering previously untested pure formatting/parsing logic
- Clones: ~25 unnecessary `.clone()` calls eliminated in hot paths
- No behavioral changes, no API breaks
- Test suite grows from 2465 to ~2490 tests
