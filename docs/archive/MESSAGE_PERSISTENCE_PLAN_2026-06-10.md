# Message Persistence Plan — Runtime Continuation Messages

Date: 2026-06-10
Status: Active
Parent: `docs/MESSAGE_PERSISTENCE_PLAN_2026-06-08.md`

## Goal

Make session resume, export, search, and crash recovery reflect the model-visible
conversation, including tool-call assistant messages and tool results. The
`messages` table should be the durable continuation surface for future turns,
while `session_events`, `session_parts`, trace records, and tool-output artifacts
remain the richer replay and UI projection surfaces.

This plan does not replace `session_events` or `session_parts`. It closes the
gap where some `Vec<Message>` entries used by the model never reach SQLite.

## Current State From Code Audit

### Already Working

- SQLite `messages` has the fields needed for model-visible messages:
  `session_id`, `role`, `content`, `tool_calls`, `tool_call_id`, `reasoning`,
  `created_at`, plus FTS search.
- `SessionStore::add_message()` persists one message and updates the session
  timestamp.
- `SessionStore::get_messages()` reads ordered message records for a session.
- `SessionStore::restore_history()` converts persisted records back to API
  `Message` values.
- `SessionStore::replace_session_messages()` can rewrite a session with API
  `Message` values.
- `SessionStore::rewrite_session_messages_after_compact()` already describes
  the intended compaction contract: rewrite the model-visible continuation
  surface while raw transcript details stay in trace/artifact records.
- Streaming startup restores history on the first turn of a resumed session when
  the shared history is empty.
- User messages are persisted before each request in `src/engine/streaming.rs`.
- Final assistant text for non-tool or completed tool turns is already
  persisted by the streaming wrapper after `ConversationLoop` returns.
- Compact boundaries and compaction events are already recorded in
  `compact_boundaries` and `session_events`.
- Desktop `resume_session` reads `messages`, compact boundaries, and
  `session_parts`; export already includes both messages and projected parts.

### Findings To Correct

The original version of this plan overstated the gap by saying all assistant
responses were missing from SQLite. Current code already persists final
assistant text in `src/engine/streaming.rs`. The remaining gap is narrower and
more specific:

| Event | In-memory `Vec<Message>` | SQLite `messages` |
|-------|---------------------------|-------------------|
| User submits message | yes | yes |
| Final assistant text | yes | mostly yes |
| Assistant tool-call envelope | yes | no |
| Tool result message | yes | no |
| Compacted continuation history | yes | partially/no |

The important missing cases are:

1. `Message::assistant_with_tools(...)` is appended in
   `src/engine/conversation_loop/tool_round_controller.rs` but not persisted.
2. `Message::tool(...)` is appended in
   `src/engine/conversation_loop/tool_result_controller.rs` but not persisted.
3. Preflight and manual compaction persist boundary/event metadata, but they do
   not consistently rewrite the `messages` table to the compacted continuation
   message list.
4. Adding persistence in `TurnCompletionController::complete()` would risk
   duplicating final assistant text, because the streaming wrapper already
   writes that message.
5. `src/engine/context_compressor/` is not the right place to write SQLite. It
   should remain a pure compression/decision component; persistence belongs at
   the caller sites or in a small session-store helper.

## Implementation Plan

### Step 1: Add A Shared Runtime Message Persistence Helper

Files:

- `src/session_store/message_ops.rs`
- `src/engine/conversation_loop/`

Add a narrow helper that converts a `Message` into a persisted `messages` row
using the existing schema:

- `Message::Assistant { tool_calls: Some(...) }` -> role `assistant`, content,
  serialized `tool_calls`, `tool_call_id = None`.
- `Message::Tool { tool_call_id, content }` -> role `tool`, content,
  `tool_calls = None`, `tool_call_id`.
- `Message::User` remains handled by streaming.
- `Message::System` should not be persisted by this helper.

The helper should return a normal `Result<i64, rusqlite::Error>` so callers can
warn on failure without silently claiming resume/export is complete.

### Step 2: Persist Tool-Call Assistant Messages At The Append Site

File: `src/engine/conversation_loop/tool_round_controller.rs`

When the loop appends `Message::assistant_with_tools(content, tool_calls)`, also
persist that exact message if `conversation.session_store` is available.

This is better than persisting at turn completion because it is local to the
message append and avoids double-writing the final assistant text already
handled by `src/engine/streaming.rs`.

Acceptance details:

- Persist even when `content` is empty; some providers return a pure tool-call
  assistant message.
- Preserve the original tool-call ids, names, and arguments in `tool_calls`.
- Add a focused regression test with an in-memory `SessionStore`.

### Step 3: Persist Tool Result Messages At The Append Site

Files:

- `src/engine/conversation_loop/tool_result_controller.rs`
- `src/engine/conversation_loop/tool_batch_result_processor.rs` if plumbing is
  needed

When `append_provider_tool_result(...)` appends `Message::tool(call_id,
model_content)`, persist the same tool result message with the matching
`tool_call_id`.

Acceptance details:

- Persist normalized model-visible content, not the full UI artifact payload.
- Large outputs should continue to rely on tool-output artifacts and metadata;
  the `messages` row should mirror what the provider sees on the next request.
- Failed, denied, invalid-param, and permission-recovery tool results should all
  follow the same path if they become `Message::tool`.
- Add tests that restore the session with `restore_history()` and verify the
  assistant-with-tools -> tool-result sequence is reconstructable.

### Step 4: Rewrite The Continuation Surface After Compaction

Files:

- `src/engine/streaming.rs`
- `src/engine/conversation_loop/preflight_compression_controller.rs`
- optionally a helper in `src/session_store/message_ops.rs`

Do not write from `context_compressor` itself. At the caller sites that already
replace in-memory history with compressed messages, rewrite the `messages` table
to the compacted continuation history.

Use `rewrite_session_messages_after_compact()` unless there is a specific reason
to keep `replace_session_messages()`. That method already states the intended
contract: raw transcript details stay in trace/artifacts, while `messages`
becomes the continuation surface.

Caller sites to audit:

- Streaming preflight compression in `src/engine/streaming.rs`.
- Manual compact in `StreamingQueryEngine::compact_context_manually()`.
- Conversation-loop preflight compression in
  `src/engine/conversation_loop/preflight_compression_controller.rs`.
- Reactive context retry paths if they mutate shared history.

Acceptance details:

- Only rewrite when compaction actually changes the continuation history.
- Do not persist dynamic per-turn system/context-zone messages that are
  regenerated every request.
- Preserve compact summary messages and retained recent tail messages.
- Compact boundary/event persistence should remain in place.

### Step 5: Verify Resume And Export Semantics

Files:

- `src/tui/session_manager/mod.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src-tauri/src/desktop_state.rs`
- `src/session_store/export.rs`

Confirm these paths consume the updated `messages` table correctly:

- TUI restore through `load_api_messages()` and engine history binding.
- Desktop `resume_session`, which reads messages, compact boundaries, and
  session parts, then initializes a runtime for the same session id.
- Session export, which should include complete `messages` plus projected
  `parts`.

If a path displays messages but does not seed runtime history directly, document
that it relies on `StreamingQueryEngine` `restore_history()` on the first resumed
turn.

## Verification

Targeted tests:

```bash
cargo test -q session_store
cargo test -q conversation_loop -- --test-threads=1
cargo test -q streaming -- --test-threads=1
```

Required regression coverage:

- A tool-call turn persists:
  1. user message,
  2. assistant message with `tool_calls`,
  3. tool result message with `tool_call_id`,
  4. final assistant text.
- `restore_history()` reconstructs that sequence as API `Message` values.
- A compacted session rewrites `messages` to the compacted continuation surface
  while preserving compact boundary/event records.
- Desktop/TUI resume and export see the persisted tool-call/tool-result
  messages.

Manual smoke test:

1. Start a TUI or desktop session.
2. Send a prompt that triggers at least one tool call.
3. Kill the process after tool execution or after final answer.
4. Restart and resume the same session.
5. Verify history includes the assistant tool-call context, tool result, and
   final assistant answer, not just the user message.
6. Export the session and verify messages and parts are both present.

Broader gate after targeted tests pass:

```bash
cargo check -q
cargo test -q
bash scripts/daily-baseline.sh
```

## Non-Goals

- No schema migration unless tests prove the current `messages` columns cannot
  represent the provider-visible sequence.
- No full replay replacement for `session_events` or `session_parts`.
- No per-token accounting in `messages`; compaction and context-budget logic can
  keep using existing token estimators.
- No raw tool-output blob duplication in `messages`; large outputs stay in
  tool-output artifacts, with normalized provider-visible content in messages.
- No persistence from `src/engine/context_compressor/`; keep that module free of
  storage side effects.
