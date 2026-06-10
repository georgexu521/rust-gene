# Message Persistence Plan — Implementation

Date: 2026-06-10
Status: Active
Parent: `docs/MESSAGE_PERSISTENCE_PLAN_2026-06-08.md`

## Current State (code audit)

### Already working

- SQLite `messages` table with full schema: id, session_id, role, content,
  tool_calls, tool_call_id, reasoning, created_at + FTS5 search index.
- `SessionStore::add_message()` — persists a single message.
- `SessionStore::get_messages()` — reads all messages for a session.
- `SessionStore::restore_history()` — loads persisted messages as API `Message`
  objects (called by `StreamingQueryEngine::execute_query` on first turn of a
  resumed session).
- `SessionStore::replace_session_messages()` — bulk delete + re-insert for
  compaction.
- TUI session resume: loads messages via `load_api_messages()` and pushes
  them into the engine.
- Desktop `resume_session`: same pattern.

### What's missing

The conversation loop adds tool results and assistant responses to the in-memory
`Vec<Message>` but **never persists them to SQLite**. The persistence gap:

| Event | In-memory | SQLite |
|-------|-----------|--------|
| User submits message | ✅ | ✅ (`add_message` in streaming.rs) |
| Assistant text response | ✅ | ❌ |
| Tool call dispatched | ✅ | ❌ |
| Tool result received | ✅ | ❌ |
| Compaction deletes old messages | ✅ | ❌ (messages lost) |

This means:
- On crash: anything after the user's initial message is gone.
- On compaction: deleted tool results are unrecoverable.
- On session export: `get_messages()` returns only the user's first message
  and whatever the loop explicitly persisted (which is nothing beyond that).

opencode persists every message turn to an SQLite-backed session store and
loads full history on resume. The same approach fits Priority Agent with
minimal changes since the storage layer already exists.

## Implementation Plan

### Step 1: Persist assistant + tool messages during the loop

**File**: `src/engine/conversation_loop/mod.rs`,
`src/engine/conversation_loop/turn_completion_controller.rs`

After each turn completes (assistant response received, tool calls executed),
persist the new messages to the session store.

Tasks:
- In `TurnCompletionController::complete()` or at the end of the tool iteration
  loop, call `session_store.add_message()` for each new message.
- Only persist if `session_store` is `Some` (it's optional in the loop).
- Persist: assistant responses (with `tool_calls` JSON if present) and tool
  result messages (with `tool_call_id`).
- Skip persistence for system messages (they're reprovisioned on each turn from
  the engine config) and for the initial user message (already persisted in
  streaming.rs).

### Step 2: Persist compaction results

**File**: `src/engine/context_compressor/compressor.rs`

When compaction replaces old messages:
- Call `session_store.replace_session_messages()` with the compacted message
  list (or the retained subset) so future session resumes get the compacted
  history.

This is a natural extension point — compaction already touches the message list
and should write the result back to durable storage.

### Step 3: Load full history on conversation loop init

**File**: `src/engine/streaming.rs` (already partially done)

The streaming engine already calls `restore_history()` when the session has
existing messages. Verify this covers:
- First turn of a resumed session → loads full history ✅ (already works)
- New user message appended and persisted ✅ (already works)
- Subsequent turns → incremental persistence from Step 1 keeps DB in sync

No code changes needed for this step — just verification.

### Step 4: Session export reads from DB

**File**: `src/session_store/export.rs`

After Steps 1-2, `get_messages()` returns the complete history for any session.
The export path already reads from the store, so it will naturally include all
messages. No code changes needed — just verify the export output after the
persistence changes land.

## Verification

```bash
cargo test -q session_store
cargo test -q conversation_loop
cargo test -q streaming
cargo test -q
bash scripts/daily-baseline.sh
```

Manual smoke test:
1. Start a TUI session, send a message, observe tool calls
2. Kill the process (Ctrl+C)
3. Restart, resume the session
4. Verify full history (not just user message) is restored

## Non-Goals

- No schema changes — the existing `messages` table is sufficient.
- No per-message update — only append + bulk-replace (compaction).
- No token-count-per-message tracking — that's a separate compaction concern.
- No changes to the API routes — they already read from the store correctly.
