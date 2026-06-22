# TUI Test Coverage And Clone Reduction Plan

Date: 2026-06-10
Status: Active

## Goal

Improve two separate quality surfaces without changing runtime behavior:

1. Add focused tests around TUI business logic that already has pure formatting,
   parsing, and diagnostic helper seams.
2. Reduce clone overhead in hot runtime paths only where the current ownership
   contract makes the clone clearly unnecessary.

These are incremental hardening tasks, not an architecture rewrite. Test
coverage should land first; clone reduction should stay profiling-driven and
avoid weakening trace, hook, provider, or async task boundaries.

## Code Audit Findings

### Finding 1: TUI coverage is not zero, but it is uneven

The earlier draft overstated the gap by saying `learning.rs` had no tests. The
current tree already has:

| File | Lines | Current tests |
|------|-------|---------------|
| `src/tui/slash_handler/learning.rs` | 1473 | `learning/tests.rs` has 9 tests |
| `src/tui/slash_handler/agents.rs` | 1440 | `agents/tests.rs` has 12 tests |
| `src/tui/slash_handler/session/actions.rs` | 1335 | no local tests found |
| `src/tui/app.rs` | 1308 | has split tests under `src/tui/app/` |
| `src/tui/mod.rs` | 1274 | has module-level tests |

The real gap is narrower: existing tests cover memory proposal, improvement,
agent doctor, exposure, and readiness behavior, but many pure label/formatter
helpers in `learning.rs` and session action parser/diagnostic helpers in
`actions.rs` remain untested.

### Finding 2: Keep tests near existing module seams

`src/tui/slash_handler/learning/tests.rs` and
`src/tui/slash_handler/agents/tests.rs` already exist. New tests should extend
those files rather than creating a second parallel test module.

For `src/tui/slash_handler/session/actions.rs`, add a local `#[cfg(test)] mod
tests` at the bottom of the file unless the session slash-handler tree gets a
dedicated split test module first.

### Finding 3: Some listed clone reductions need interface changes

The original clone plan was too optimistic in three places:

- `api_request_controller.rs`: many `request.model.clone()` calls populate
  owned trace events, retry diagnostics, or JSON values. Binding
  `let model = request.model.as_str()` does not remove those allocations when
  the downstream type owns `String` or `serde_json::Value`.
- `session_processor.rs`: `fallback_messages` and `fallback_tools` are cloned
  before `provider.chat_stream(request)` because `chat_stream` consumes
  `ChatRequest`. Deferring those clones is not a small move unless
  `ChatRequest` or the provider interface changes.
- `session_processor.rs`: wrapping parsed JSON args in `Arc<Value>` does not
  remove all data clones because `Tool::execute` and hook `ToolCall` records
  currently take owned `Value`.

These can still be improved, but they should be treated as measured refactors,
not quick mechanical substitutions.

### Finding 4: There are safe clone targets, but they are smaller

Two low-risk clone surfaces remain worth addressing:

- Avoid full-history clones for memory flushing in `src/engine/streaming.rs`
  where a bounded helper-owned snapshot or tail snapshot is enough. Do not hold
  the shared history lock across an async memory flush.
- Reuse per-iteration owned labels in `api_request_controller.rs` only where the
  same owned value is immediately duplicated for multiple local emissions and
  tests can prove trace contents are unchanged.

If profiling shows large JSON tool args are a real cost, handle that as a
second slice by changing hook/tool argument contracts intentionally.

## Implementation Plan

### Step 1: Add missing learning formatter tests

File:

- `src/tui/slash_handler/learning/tests.rs`

Add tests for the pure helpers that are not covered by the existing learning
test file:

| Function | File |
|----------|------|
| `compact_inline` | `learning.rs` |
| `goal_drift_count_label` | `learning.rs` |
| `format_goal_drift_report` | `learning.rs` |
| `count_debug_values` | `learning.rs` |
| `format_counts` | `learning.rs` |
| `is_evolution_learning_event` | `learning.rs` |
| `format_learning_event_detail` | `learning.rs` |
| `format_experience_event` | `learning.rs` |
| `latest_resource_policy_label` | `learning.rs` |
| `latest_contract_state_label` | `learning.rs` |
| `latest_retrieval_context_label` | `learning.rs` |
| `latest_reflection_label` | `learning.rs` |
| `latest_stage_validation_label` | `learning.rs` |
| `latest_workflow_plan_label` | `learning.rs` |
| `latest_closeout_label` | `learning.rs` |
| `latest_acceptance_label` | `learning.rs` |
| `latest_guided_debugging_label` | `learning.rs` |

Keep the tests data-oriented. Build minimal `TurnTrace` or
`LearningEventRecord` fixtures and assert stable user-visible strings.

Verification:

```bash
cargo test -q tui::slash_handler::learning --lib
```

### Step 2: Add session action parser and diagnostic tests

File:

- `src/tui/slash_handler/session/actions.rs`

Add local tests for:

| Function | Purpose |
|----------|---------|
| `parse_export_format` | accepted aliases and rejected unknown values |
| `parse_export_privacy` | accepted privacy modes and rejected unknown values |
| `diagnostic_failed_tool_names` | failed tool name extraction and de-duplication |
| `diagnostic_validation_status` | validation pass/fail/unknown summary |
| `tool_run_looks_like_validation` | command/name/content validation detection |

This is the highest-value uncovered TUI surface because it backs user-visible
diagnostic/export commands and currently has no local tests.

Verification:

```bash
cargo test -q tui::slash_handler::session::actions --lib
```

### Step 3: Extend existing agent tests only where gaps remain

Files:

- `src/tui/slash_handler/agents/tests.rs`
- `src/tui/slash_handler/agents/doctor_formatting.rs`

`agents/tests.rs` already covers doctor readiness, route exposure, terminal task
counts, prompt-cache diagnostics, and MCP repair formatting. Add only the gaps
that are still untested:

| Function | Coverage target |
|----------|-----------------|
| `handle_voice` | installed/missing command messaging stays clear |
| `handle_telemetry` | telemetry command text remains stable |
| `format_provider_status_summary` | provider/model status line is present |
| `format_effective_config_summary` | effective config summary includes key runtime knobs |

Verification:

```bash
cargo test -q tui::slash_handler::agents --lib
```

### Step 4: Profile clone hotspots before editing runtime ownership

Files:

- `src/engine/conversation_loop/api_request_controller.rs`
- `src/engine/conversation_loop/session_processor.rs`
- `src/engine/streaming.rs`

Before changing clone-heavy code, record a small baseline:

```bash
rg -n "\.clone\(\)" src/engine/conversation_loop/api_request_controller.rs \
  src/engine/conversation_loop/session_processor.rs src/engine/streaming.rs
cargo test -q conversation_loop -- --test-threads=1
cargo test -q streaming -- --test-threads=1
```

Use this baseline to separate cosmetic clone-count reductions from meaningful
hot-path work.

### Step 5: Remove only low-risk clones first

Start with clones that do not cross API ownership boundaries:

- In `src/engine/streaming.rs`, avoid cloning the full conversation history for
  memory flush if a bounded helper-owned snapshot or tail snapshot is enough.
  Preserve the existing rule that the history lock must not be held across the
  async flush.
- In `src/engine/conversation_loop/api_request_controller.rs`, reuse local
  provider/model/request-shape labels only when the target still receives the
  same owned value and trace/event payloads remain byte-for-byte equivalent.

Do not change these in the first slice:

- `Provider::chat_stream(ChatRequest)` ownership.
- `Tool::execute(Value, ToolContext)` ownership.
- hook `ToolCall { arguments: Value }` ownership.

Those are broader interface changes and need separate tests.

Verification:

```bash
cargo test -q conversation_loop -- --test-threads=1
cargo test -q streaming -- --test-threads=1
```

### Step 6: Optional second slice for large tool args

Only do this if profiling or a real session trace shows JSON argument cloning is
material:

- Introduce a borrowed or shared argument view for hook pre/post checks.
- Keep provider-visible `ToolCall` serialization unchanged.
- Keep `Tool::execute` behavior unchanged unless the whole tool trait is
  intentionally migrated.

This is not part of the first cleanup batch.

## Validation

Narrow gates:

```bash
cargo test -q tui::slash_handler::learning --lib
cargo test -q tui::slash_handler::session::actions --lib
cargo test -q tui::slash_handler::agents --lib
cargo test -q conversation_loop -- --test-threads=1
cargo test -q streaming -- --test-threads=1
```

Broader gate after code changes:

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q
bash scripts/daily-baseline.sh
```

## Expected Outcome

- TUI: focused tests for the main uncovered formatter/parser/diagnostic helpers,
  added to existing module test seams.
- Runtime: smaller, proven clone reductions without changing provider, hook, or
  tool ownership contracts.
- No user-visible behavior changes.
- No weakening of trace, diagnostic, permission, validation, or session evidence
  paths.
