# Opencode Third Alignment Investigation And Next Phase Plan

Date: 2026-06-07

Status: implemented with follow-up hardening notes

## 1. Purpose

This is the third focused comparison between Priority Agent and the local
opencode source under `/Users/georgexu/Downloads/opencode-dev`.

The previous opencode alignment plans have mostly been implemented:

- `docs/OPENCODE_PROGRAMMING_CHAIN_GAP_PLAN_2026-06-05.md`
- `docs/OPENCODE_AGENT_ENGINE_ALIGNMENT_PLAN_2026-06-06.md`
- `docs/NEXT_PHASE_OPENCODE_CORE_ALIGNMENT_PLAN_2026-06-06.md`

Priority Agent now has the core pieces needed for real programming tasks:
checkpoint-backed edits, read-before-edit protection, route-scoped tools,
permission evidence, usage ledger, runtime controller paths, persisted
`session_events`, persisted `session_parts`, paged tool output, desktop/TUI
session reload, and assistant-turn revert events.

The remaining gap is no longer "missing primitives". The gap is product
hardening: opencode keeps more runtime behavior behind durable, typed,
incrementally projected contracts. Priority Agent should now make its existing
safety primitives cheaper to replay, easier to debug, and more predictable
during long sessions, restarts, slow providers, and desktop/TUI reloads.

## 2. Evidence Reviewed

opencode source reviewed:

- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/session/sql.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/session/event.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/session/message.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/session/projector.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/session/input.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/session/run-coordinator.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/revert.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/message-v2.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/processor.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/truncate.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/shell.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/permission/schema.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/provider/provider.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/config/config.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/server/server.ts`

Priority Agent source reviewed:

- `src/session_store/event_store.rs`
- `src/session_store/session_parts.rs`
- `src/session_store/mod.rs`
- `src/migrations/v10_add_session_inputs.rs`
- `src/migrations/v11_add_session_parts.rs`
- `src/engine/run_coordinator.rs`
- `src/engine/runtime_controller.rs`
- `src/engine/conversation_loop/session_processor.rs`
- `src/engine/conversation_loop/runtime_timeouts.rs`
- `src/tool_output_store/mod.rs`
- `src/permissions/mod.rs`
- `src/services/api/provider.rs`
- `src/services/api/provider_protocol.rs`
- `src/diagnostics/provider_health.rs`
- `apps/desktop/src-tauri/src/desktop_state.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src/app/runEventState.ts`
- `apps/desktop/src/app/components/ToolOutputDrawer.tsx`

## 3. Current Assessment

Priority Agent is now closer to opencode than it was in the first two rounds.
Several earlier gaps are no longer open:

- Long tool output is stored behind `tool-output://...` with session-scoped
  reads and desktop/TUI access.
- TUI, desktop, and CLI paths have moved toward the shared
  `RuntimeController`.
- `session_events` and `session_parts` can persist and reload assistant text,
  reasoning, tool parts, permission, closeout, and revert events.
- Desktop can reload persisted parts and exposes a visible assistant-turn
  revert control.
- Revert results are typed events and appear in diagnostic export.
- Provider timeout and slow-tail handling are much stronger than before.

opencode is still ahead in these areas:

- edit tools have stronger fallback matching when `oldString` is slightly
  stale, whitespace-shifted, or only partially anchored;
- DeepSeek/provider message transforms are more specialized around reasoning
  and interleaved message formats;
- session projections are treated as a canonical typed read model, not only a
  rebuilt helper table;
- event projection is incremental and sequence-driven;
- prompt admission has explicit ids, conflict detection, and promoted sequence;
- streamed deltas are live transport while replayable final values are stored
  at ended boundaries;
- revert supports unrevert and cleans projected messages/parts after the
  revert point;
- tool-output truncation has configurable max lines/bytes and cleanup policy;
- permission rules are persisted as product state and exposed through a small
  user-facing contract;
- shell command parsing extracts paths and command structure more precisely;
- provider/config/server surfaces are more formalized for external clients.

The right next phase is not to copy opencode wholesale. Priority Agent should
keep its stronger local safety model and personal-agent scope, then borrow the
parts that make long real coding sessions more durable and observable.

Implementation update on 2026-06-07:

- `file_edit` now has deterministic edit recovery candidates for exact,
  line-trimmed, indentation-normalized, block-anchor, and safe
  whitespace-normalized matches.
- Provider transform diagnostics now report DeepSeek/OpenAI-compatible reasoning
  metadata, output cap, schema mode, DSML stripping, and interrupted tool-call
  repair.
- `session_events` now update `session_parts` incrementally by sequence, while
  full rebuild remains available as a repair/debug path.
- Assistant text and reasoning now have final-value completion events for replay.
- `ToolOutputPolicy` now controls the actual storage threshold, preview
  direction, preview line budget, retention cleanup, and diagnostic export.
- Remaining work is hardening and broader end-to-end coverage, not missing core
  primitives.

### 3.1 Accuracy-Oriented Follow-Up: Four Places opencode Still Leads

This follow-up is specifically about why opencode can feel more accurate than
Reasonix on the same DeepSeek v4 pro model. The conclusion is that opencode's
advantage is mostly outside the model: edit recovery, provider/message
transforms, replay contracts, and output policy give the model cleaner evidence
and fewer chances to spiral.

Priority Agent already behaves more like opencode than Reasonix in the broad
programming loop: it has read-before-write, checkpoint-backed mutation,
structured mutation results, LSP diagnostics, provider capability detection,
tool-call repair, proof-gated closeout, `tool-output://` paging, and typed
session parts. The remaining work is to close four sharper gaps.

#### Follow-up A: Edit fallback matching

Current status:

- Priority Agent gives strong repair instructions after `file_edit` misses an
  anchor, and it encourages line-based or exact current-content edits.
- opencode goes further by trying multiple edit matchers inside the edit tool
  before returning failure.

Optimization plan:

- Add an internal `EditMatcher` pipeline for `file_edit`:
  - exact match;
  - line-trimmed match;
  - indentation-flexible match;
  - block-anchor match using first/last line anchors;
  - whitespace-normalized match for safe single-candidate cases.
- Keep the safety rule: fallback may apply only when it produces exactly one
  candidate and the resulting diff is small enough to review.
- Return structured metadata:
  - `match_strategy`;
  - `candidate_count`;
  - `anchor_confidence`;
  - `fallback_used`;
  - `fallback_reason`.
- When fallback is rejected, return a targeted model-facing recovery message
  with the nearest line range and the exact next safe tool form.

Acceptance:

- Common stale-anchor failures become successful safe edits instead of extra
  model turns.
- Non-unique or low-confidence fallbacks still fail closed.
- TUI/desktop show when an edit used fallback matching.

Suggested gates:

```bash
cargo test -q file_tool
cargo test -q file_edit
cargo test -q action_checkpoint
cargo check -q
```

#### Follow-up B: DeepSeek/provider message transform parity

Current status:

- Priority Agent detects provider capabilities, normalizes tool-result
  adjacency, repairs malformed/truncated tool-call arguments, tracks reasoning
  and cached tokens, and defaults DeepSeek to `deepseek-v4-pro`.
- opencode still has more provider-specific transform surface, especially for
  DeepSeek reasoning/interleaved message fields and max-output option mapping.

Optimization plan:

- Add a DeepSeek-specific provider transform report:
  - whether reasoning/interleaved fields were present;
  - whether empty reasoning parts were inserted or preserved;
  - whether provider-native reasoning options were sent;
  - effective output cap;
  - effective tool schema mode.
- Add tests with captured DeepSeek-like transcripts:
  - assistant with text only;
  - assistant with reasoning only;
  - assistant with tool calls and empty reasoning;
  - interrupted tool-call pair;
  - DSML leaked function call.
- Keep transforms at the provider boundary, not in the main loop, so runtime
  permission, checkpoint, validation, and closeout gates still see normal
  `ToolCall` values.
- Expose the transform report in `/diagnostic` and provider status.

Acceptance:

- DeepSeek requests are reproducible from diagnostic export without logging
  secrets.
- Tool-call and reasoning normalization is tested separately from the agent
  loop.
- Provider-specific fixes do not leak into generic runtime logic.

Suggested gates:

```bash
cargo test -q provider_protocol
cargo test -q tool_call_repair
cargo test -q openai_compat
cargo test -q diagnostic
cargo check -q
```

#### Follow-up C: Incremental canonical session projection

Current status:

- Priority Agent has `session_events` and persisted `session_parts`.
- The normal event writer now incrementally applies events after the highest
  projected sequence instead of rebuilding the whole projection after each event.
- Full rebuild remains available for repair/debug.
- opencode treats the projected session read model as a canonical incremental
  product contract.

Optimization plan:

- Implement the P0 projection work in this document first:
  - incremental apply from last projected sequence;
  - stable part ids;
  - cursor APIs;
  - full rebuild only as repair/debug.
- Add projection performance tests with thousands of events.
- Add DB lock regression tests around append-event plus desktop resume.
- Make desktop/TUI consume cursor reads for large sessions.

Acceptance:

- Long coding sessions stay responsive while streaming.
- Desktop resume does not require a full projection rebuild.
- Diagnostics can identify projection lag or repair rebuilds.

Suggested gates:

```bash
cargo test -q session_store
cargo test -q session_parts
cargo test -q desktop_runtime
cargo check -q
```

#### Follow-up D: Configurable tool-output policy

Current status:

- Priority Agent already stores long outputs in `ToolOutputStore` and exposes
  session-scoped paging through TUI/desktop.
- opencode still has a more complete product policy: configurable max
  lines/bytes, retention, and preview direction.

Optimization plan:

- Add config keys:
  - `tool_output.max_bytes`;
  - `tool_output.max_lines`;
  - `tool_output.preview_direction`;
  - `tool_output.retention_days`.
- Use head-tail or tail preview by default for validation/test failures where
  the useful error is usually near the end.
- Add startup cleanup and `/tool-output clean`.
- Record the active policy in diagnostic export and desktop status.
- Add model-facing hints telling the agent to inspect stored output by search or
  offset instead of re-dumping full logs into context.

Acceptance:

- Large test logs do not pollute context, but remain inspectable.
- Users can tune output policy without recompiling.
- TUI and desktop show identical output metadata and paging behavior.

Suggested gates:

```bash
cargo test -q tool_output_store
cargo test -q config
cargo test -q tui
cd apps/desktop && pnpm test -- desktop-ui-smoke
cargo check -q
```

## 4. Gap 1: Session Parts Are Useful But Not Yet Canonical Enough

### opencode behavior

opencode stores a typed `session_message` projection with `session_id`, `type`,
`seq`, timestamps, and JSON data. The projector writes by synchronized event
sequence and updates assistant/shell/compaction messages through a typed
updater. Consumers can page or filter the read model without replaying every raw
event.

### Priority Agent status

Priority Agent now has `session_events` and `session_parts`. This is a good
foundation. `SessionEventWriter::write_event` now calls
`incremental_refresh_session_parts`, which reads events after
`MAX(projected_to_seq)` and updates the projected read model without deleting
and reinserting the whole session.

The main hardening target is now equivalence and load behavior: incremental
projection must remain byte-for-byte compatible with full rebuild for mixed
assistant text, reasoning, tool, permission, closeout, and revert events, and it
should stay responsive for long sessions.

### Plan

Priority: P0.

Work items:

- Add an incremental projection path that reads `MAX(projected_to_seq)` for the
  session and applies only new events. Done.
- Keep the full rebuild function as a repair/debug command, not the default
  write path. Done.
- Add stable projected ids derived from assistant message id, tool call id, and
  event family instead of `part_1`, `part_2`, etc. Done for current part
  families.
- Add cursor APIs. Done:
  - `get_session_parts_after(session_id, part_index, limit)`;
  - `get_session_events_after(session_id, seq, limit)`;
  - desktop/TUI can use these for paged reload and diagnostics.
- Add indexes needed for cursor reads and session/kind/status filters. Done in
  the session-parts migration for the current schema.
- Add a migration if the current schema is not sufficient. Not currently needed.
- Keep tests proving full rebuild and incremental projection stay equivalent
  when assistant text is separated by tool parts.

Acceptance:

- A long session can append thousands of text/tool events without full
  projection rebuild per event.
- Desktop resume can page projected parts instead of loading all parts at once.
- A repair command can still rebuild a corrupted projection from raw events.
- Tests prove projection is idempotent and sequence ordered.

Suggested gates:

```bash
cargo test -q session_store
cargo test -q session_parts
cargo test -q desktop_runtime
cargo check -q
```

## 5. Gap 2: Replayable Final-Value Boundaries Are Still Too Weak

### opencode behavior

opencode treats `Text.Delta` and `Reasoning.Delta` as live stream updates.
Replay relies on ended/final records that contain the full value. Tool input,
tool output, shell output, and assistant step settlement also have typed
completion boundaries.

### Priority Agent status

Priority Agent persists live delta events and now also writes final-value events
for assistant text and reasoning. Projection prefers the completed value when it
is present, while deltas still support live UI streaming.

The remaining durability gap is large tool input/result final values. Tool
output is stored behind `tool-output://...`, but tool input completion,
shell-output completion, and canonical replay source metadata still need more
hardening.

### Plan

Priority: P0.

Work items:

- Add final-value events:
  - `assistant_text_completed`; done.
  - `reasoning_completed`; done.
  - `tool_input_completed`;
  - `tool_result_completed`;
  - `shell_output_completed`.
- Keep deltas for live UI only; projection should prefer completed values when
  present. Done for assistant text and reasoning.
- For large final values, store content behind `tool-output://...` or a
  session-content store and keep only URI + metadata in the event payload.
- Update diagnostic export to show whether a part was rebuilt from deltas or
  completed from final value.
- Add tests for missing delta plus completed event, truncated preview plus full
  stored output, and interrupted tool input.

Acceptance:

- Restart/reload can reconstruct final assistant text/tool input even if live
  deltas were partial.
- Diagnostic export can prove whether replay came from final events or deltas.
- No durable event relies on a preview-only payload for canonical content.

Suggested gates:

```bash
cargo test -q session_store
cargo test -q streaming
cargo test -q tool_output_store
cargo test -q diagnostic
cargo check -q
```

## 6. Gap 3: Durable Prompt Admission Needs Idempotency

### opencode behavior

opencode `session_input` stores a prompt id, durable prompt payload, delivery
mode (`steer` or `queue`), and `promoted_seq`. Reusing the same prompt id with
the same prompt is a retry/reconciliation path; reusing it with a different
prompt is rejected.

### Priority Agent status

Priority Agent now has `session_inputs`, `InputDelivery`, prompt idempotency,
prompt hashes, `promoted_seq`, and state tracking through the v12 session-input
migration and `run_coordinator` helpers. Same `prompt_id` plus same prompt is an
idempotent retry; same `prompt_id` plus different prompt is a conflict; reserved
internal ids are rejected.

The remaining gap is product visibility and recovery polish: queued/pending
inputs should be easier to inspect and cancel from TUI/desktop, and crash
recovery should make interrupted steer inputs explicit.

### Plan

Priority: P1.

Work items:

- Extend `session_inputs` with:
  - `prompt_id`; done.
  - `prompt_hash`; done.
  - `attachments_json`; done.
  - `promoted_seq`; done.
  - `state` (`pending`, `promoted`, `cancelled`, `conflict`); schema support done.
  - `error`; done.
- Add `admit_session_input(session_id, prompt_id, prompt, delivery)` with
  idempotency. Done:
  - same id + same hash returns existing row;
  - same id + different hash fails loudly;
  - reserved internal ids are rejected.
- Promote input by setting `promoted_seq` to the current session event sequence.
  Done.
- Add `/sessions pending` or desktop status display for queued prompts.
- Add crash recovery policy:
  - pending queue inputs survive;
  - in-flight steer inputs are marked interrupted unless promoted safely.

Acceptance:

- Double submit from desktop cannot produce duplicate turns.
- Retry after frontend/network failure can safely reuse the same prompt id.
- Queued prompts are visible and can be cancelled.
- Tests prove no overlapping drains for one session.

Suggested gates:

```bash
cargo test -q run_coordinator
cargo test -q session_store
cargo test -q desktop_runtime
cargo test -q tui
cargo check -q
```

## 7. Gap 4: Revert Is Present But Not Yet A Full Session State Machine

### opencode behavior

opencode revert can target a message or part, assert the session is not busy,
capture or restore snapshots, revert file patches, compute diff summaries,
publish a diff event, store revert metadata on the session, support unrevert,
and clean up messages/parts after the revert point.

### Priority Agent status

Priority Agent now has an important user-facing path:

- desktop has a visible Revert button;
- TUI has assistant-turn revert;
- TUI has `/unrevert` while a snapshot checkpoint still exists;
- file restore/remove results are correctly separated from errors;
- typed `revert` and `unrevert` events are persisted and projected;
- diagnostic export includes revert events;
- revert/unrevert are blocked while a session run is busy.

The remaining gap is session consistency after revert. The file system can be
restored, but the projected conversation is not yet trimmed or marked as
reverted at message/part granularity. Revert metadata exists in checkpoint
history and typed events, but there is not yet a persisted `session_reverts`
table or projected `reverted_after` marker comparable to opencode's session
state.

### Plan

Priority: P1.

Work items:

- Add `session_reverts` or session metadata fields:
  - target assistant message id;
  - target part id;
  - snapshot/checkpoint id;
  - changed paths;
  - diff summary;
  - status;
  - timestamp.
- Add a projected `reverted_after` marker so desktop/TUI can hide, dim, or
  label assistant parts after the revert point.
- Add cleanup behavior for "confirm revert":
  - remove projected parts after target;
  - preserve raw events for audit unless user deletes session;
  - keep a typed revert marker.
- Add optional unrevert if the checkpoint/snapshot still exists. Done for TUI.
- Block revert while a session run is busy. Done for TUI revert/unrevert.

Acceptance:

- After revert, desktop/TUI no longer makes reverted assistant parts look
  current.
- Diagnostic export distinguishes active parts from reverted parts.
- A successful revert can be undone while its snapshot is still available.
- Raw audit trail remains available for debugging.

Suggested gates:

```bash
cargo test -q checkpoint
cargo test -q session_store
cargo test -q tui
cd apps/desktop && pnpm test -- run-event-state
cargo check -q
```

## 8. Gap 5: Tool Output Store Needs Configurable Product Policy

### opencode behavior

opencode has `tool_output.max_lines` and `tool_output.max_bytes`, a default
retention window, head/tail previews, stored full output, and user-facing hints
for reading/searching large output.

### Priority Agent status

Priority Agent has `ToolOutputStore`, `tool-output://` URIs, session-scoped
reads, desktop drawer, and TUI `/tool-output`. `ToolOutputPolicy` now controls
the actual storage threshold, preview byte budget, preview line budget, preview
direction, and retention cleanup. The remaining gap is product polish:
project-level config files and startup scheduling should eventually sit above
the current environment-variable policy.

### Plan

Priority: P1.

Work items:

- Add config keys:
  - `tool_output.max_bytes`;
  - `tool_output.max_lines`;
  - `tool_output.preview_direction` (`head`, `tail`, `head_tail`);
  - `tool_output.retention_days`.
- Make shell/test output default to tail or head-tail when failures are more
  likely at the end of output. Done: default preview is tail.
- Add a scheduled cleanup path on app startup and a manual `/tool-output clean`.
  Manual clean is done; startup cleanup remains follow-up.
- Include output policy in `/diagnostic` and desktop status. Diagnostic export is
  done; desktop surfacing remains follow-up if a richer settings/status view is
  added.
- Add a prompt hint that tells the model to inspect by offset/search, not to
  dump full logs into context.

Acceptance:

- Users can tune long-output behavior without recompiling.
- Huge logs do not bloat prompt context or UI state.
- TUI and desktop show the same stored-output metadata.

Suggested gates:

```bash
cargo test -q tool_output_store
cargo test -q config
cargo test -q tui
cd apps/desktop && pnpm test -- desktop-ui-smoke
cargo check -q
```

## 9. Gap 6: Provider Layer Needs More Formal Runtime Contracts

### opencode behavior

opencode provider code has a more formal provider/config surface: bundled and
custom providers, model metadata, provider transforms, auth/base URL merging,
OpenAI-compatible request differences, header timeout, SSE read timeout, and
provider status exposed to clients.

### Priority Agent status

Priority Agent has improved significantly:

- provider env specs;
- provider families and capabilities;
- MiniMax/OpenAI-compatible differences;
- request timeouts and stream idle timeout;
- provider health diagnostics;
- runtime facade provider lifecycle status.

The gap is product polish and consistency:

- slow-tail timeout settings are still spread across runtime env, provider
  profile, and diagnostics;
- model capability/status is not yet a first-class persisted table or API;
- desktop status can show runtime state, but provider health history is still
  mostly diagnostic rather than operational.

### Plan

Priority: P1.

Work items:

- Add a provider runtime profile snapshot:
  - provider id;
  - model id;
  - protocol family;
  - tool-call mode;
  - request timeout;
  - stream idle timeout;
  - last health result;
  - last timeout category.
- Persist recent provider health runs in a small table or JSONL ledger.
- Add `/provider status --json` and desktop status fields backed by the same
  profile snapshot.
- Normalize timeout names and config/env precedence in docs and code.
- Add explicit tests for:
  - stream-open timeout;
  - stream idle timeout;
  - nonstreaming tool-call timeout;
  - provider health failure classification.

Acceptance:

- When a provider is slow or stuck, TUI/desktop can show which timeout fired and
  what profile was active.
- Daily baseline can include provider runtime profile without scraping logs.
- Provider capability changes are visible before the agent starts a long task.

Suggested gates:

```bash
cargo test -q provider_protocol
cargo test -q provider_health
cargo test -q runtime_facade
cargo test -q conversation_loop
cargo check -q
```

## 10. Gap 7: Permission Rules Need A Smaller Product Contract

### opencode behavior

opencode persists permission rules as product state and uses a small action /
resource / effect model. Users can reason about rules by scope and resource.

### Priority Agent status

Priority Agent's permission system is more safety-oriented and more powerful:
mode, rule source, once TTL, wildcard rules, action review, shell command
classifier, checkpoint requirement, high-risk paths, and permission evidence.

The risk is complexity. A powerful system is only useful if the user can
understand why a command was allowed or blocked and how to change it safely.

### Plan

Priority: P2.

Work items:

- Add a product-facing `PermissionRuleView`:
  - scope (`session`, `project`, `global`);
  - matcher key (`edit`, `bash:<normalized>`, `mcp/server/tool`);
  - effect (`allow`, `deny`, `ask`);
  - source;
  - expires_at for once/session rules.
- Persist project-scoped rules explicitly instead of only carrying in-memory
  session rules.
- Add `/permissions explain <tool-or-command>` that returns the exact matcher
  keys and winning rule.
- Desktop settings should show rule scope and risk reason, not only mode.
- Keep destructive/high-risk gates hard even if user adds broad allow rules.

Acceptance:

- Users can tell why a permission happened without reading traces.
- Project rules survive restart and are auditable.
- Broad allow rules cannot bypass high-risk checkpoint constraints.

Suggested gates:

```bash
cargo test -q permissions
cargo test -q action_review
cargo test -q conversation_loop
cd apps/desktop && pnpm test -- desktop-ui-smoke
cargo check -q
```

## 11. Gap 8: Shell Command Understanding Can Still Improve

### opencode behavior

opencode shell parsing extracts command structure, path arguments, dynamic
arguments, current-directory changes, and mutating command metadata. It does not
only scan strings.

### Priority Agent status

Priority Agent has a strong bash classifier and blocks dangerous shell write
patterns. It also routes programming edits toward file tools. This is safer
than simply allowing shell writes.

The remaining gap is precision:

- dynamic shell constructs can still be hard to explain;
- path extraction is not as complete as opencode's shell parser;
- permission prompts sometimes know the command risk but not all touched paths.

### Plan

Priority: P2.

Work items:

- Extend command classification with a `ShellCommandView`:
  - primary command;
  - normalized command;
  - detected path args;
  - cwd-changing segments;
  - dynamic/unknown args;
  - mutation family;
  - recommended tool alternative.
- Add more tests from opencode-style shell cases:
  - heredoc;
  - redirect;
  - `sed -i`;
  - `perl -pi`;
  - `find ... -delete`;
  - nested `sh -c`;
  - path with spaces;
  - env assignments.
- Use detected paths in permission and checkpoint evidence.

Acceptance:

- Permission prompt can say which paths a shell command appears to touch.
- Unknown dynamic shell writes are blocked or escalated with clear reason.
- Weak models get a clear file-tool recovery path after blocked shell writes.

Suggested gates:

```bash
cargo test -q bash_tool
cargo test -q permissions
cargo test -q action_review
cargo check -q
```

## 12. Gap 9: External API Surface Is Not Yet As Stable As The Core

### opencode behavior

opencode exposes stable session/message/tool-output/server APIs for external
clients. The app and external clients use the same session concepts.

### Priority Agent status

Priority Agent has a desktop Tauri API and an experimental API server feature,
but the newly improved session projection is not yet a clearly documented API
contract.

### Plan

Priority: P2.

Work items:

- Define stable API DTOs for:
  - session info;
  - session parts cursor;
  - session events cursor;
  - tool output index/page;
  - permission request/answer;
  - provider runtime profile;
  - diagnostic export.
- Make desktop consume these DTOs directly where possible.
- Add API schema docs under `docs/api/`.
- Keep experimental API feature-gated until the DTOs are tested.

Acceptance:

- TUI, desktop, and optional API use the same session part vocabulary.
- Adding a frontend does not require reinterpreting raw trace internals.
- API docs match tested DTOs.

Suggested gates:

```bash
cargo test -q session_store
cargo check --features experimental-api-server -q
cd apps/desktop && pnpm test -- run-event-state
cargo check -q
```

## 13. Non-Goals

Do not copy these opencode areas yet:

- broad plugin marketplace architecture;
- multi-user hosted server product;
- large provider ecosystem beyond the providers gex actually uses;
- replacing Priority Agent's memory system with opencode's lighter instruction
  model;
- weakening checkpoint, permission, high-risk path, or proof-gated closeout
  behavior for convenience.

Priority Agent's product direction is still personal, local, narrow, verifiable
coding assistance. opencode is useful as an engineering reference, not as a
complete product target.

## 14. Recommended Implementation Order

### Phase 0: Accuracy parity with opencode

Do before broad product expansion.

- Add safe `file_edit` fallback matching and expose `match_strategy`.
- Add DeepSeek/provider transform diagnostics and golden tests.
- Keep these improvements inside tool/provider contracts, not as broad prompt
  rules.
- Use real coding tasks to compare failed edit retries, tool-call repair counts,
  validation success, and final verified closeout rate.

### Phase 1: Projection correctness and performance

Do first.

- Incremental `session_parts` projection.
- Stable part ids.
- Cursor APIs for events and parts.
- Repair/full-rebuild command.
- Tests for long sessions and partial event order.

### Phase 2: Durable replay final values

Do immediately after Phase 1.

- Add completed/final-value events.
- Prefer completed events in projection.
- Store large final values by URI.
- Export replay source in diagnostics.

### Phase 3: Prompt admission idempotency

Do before more real desktop/TUI stress testing.

- Add prompt ids and hashes. Done.
- Add duplicate/conflict detection. Done.
- Add promoted sequence and pending/cancelled state. Promoted/pending support is
  done; cancellation and user-facing pending views remain follow-up.
- Show queued input in TUI/desktop.

### Phase 4: Revert state machine

Do after session projection is stable.

- Persist session revert metadata.
- Mark or trim reverted projected parts.
- Add unrevert while snapshot exists. Done for TUI.
- Block revert while busy. Done for TUI revert/unrevert.

### Phase 5: Tool output and provider productization

Can run partly in parallel after Phase 1.

- Configurable tool-output limits and cleanup.
- Provider runtime profile snapshot.
- Provider health history.
- Desktop/TUI status based on the same provider profile.
- Diagnostic export includes active output policy and provider transform report.

### Phase 6: Permission and shell explainability

Do after the next real-task testing cycle identifies the most confusing cases.

- Product-facing permission rule view.
- `/permissions explain`.
- Better shell path extraction and dynamic risk metadata.

### Phase 7: API contract cleanup

Do last.

- Stabilize session/event/tool-output/provider DTOs.
- Document `docs/api/`.
- Keep experimental API feature-gated until the contract is used by desktop and
  covered by tests.

## 15. Final Gate For This Next Phase

This phase should be considered complete only when the following are true:

- a long coding session can be stopped and reloaded in desktop without losing
  assistant text, reasoning, tool state, tool output links, closeout, or revert
  markers;
- `session_parts` projection does not rebuild the entire session on every event
  during normal streaming;
- submitted prompts have stable ids and duplicate submit cannot create duplicate
  turns;
- stale or whitespace-shifted `file_edit` anchors either apply through a
  single-candidate safe fallback or fail closed with targeted recovery metadata;
- DeepSeek/provider transforms are covered by golden tests and visible in
  diagnostics;
- provider timeout diagnostics show which provider/model/profile/timeout path
  fired;
- tool output policy is configurable and visible in diagnostics;
- permission and shell blocks explain both the reason and the correct recovery
  path.

Suggested full gate:

```bash
cargo fmt --check
cargo check -q
cargo test -q session_store
cargo test -q session_parts
cargo test -q run_coordinator
cargo test -q tool_output_store
cargo test -q provider_protocol
cargo test -q provider_health
cargo test -q permissions
cargo test -q bash_tool
cargo test -q checkpoint
cargo test -q tui
cd apps/desktop && pnpm test -- run-event-state
cd apps/desktop && pnpm test -- desktop-ui-smoke
cargo check --features experimental-api-server -q
```
