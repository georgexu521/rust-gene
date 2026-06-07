# Next Phase Opencode Core Alignment Plan

Date: 2026-06-06

Status: implemented with follow-up hardening

Implementation review: 2026-06-07

The main slices have landed in code:

- `ToolOutputStore` stores long tool output behind `tool-output://...` and now
  enforces session-scoped reads.
- `SessionEventWriter` and `StreamEventMirror` now capture text, reasoning,
  tool input, tool lifecycle, usage, closeout, permission, diagnostics, errors,
  and completion events through the shared `RuntimeController` path used by TUI
  and desktop.
- `SessionPart` projection can rebuild assistant text, reasoning, tool parts,
  permission, compaction, and closeout from `session_events`.
- `SessionRunCoordinator` now prevents overlapping TUI runs and persists
  queued follow-up input into `session_inputs`; completed runs drain the next
  queued message.
- `/revert last-turn` no longer reports successful restore/remove operations
  as errors.
- Tool lifecycle settlement gaps now flow into final closeout and downgrade
  verified closeout when a tool remains pending/running.

2026-06-07 completion update:

- `session_parts` is now a persisted projection table refreshed from
  `session_events`, with query APIs for desktop/TUI reload.
- CLI shell turn submission now goes through `RuntimeController`, aligning the
  CLI path with TUI/desktop controller semantics.
- Desktop `resume_session` returns persisted `session_parts`; the React
  transcript reload path maps assistant text, reasoning, tool, permission,
  compaction, and closeout parts back into UI items.
- TUI session restore rebuilds completed tool runs from persisted tool/shell
  parts when available, while retaining message history as the text source.
- Desktop exposes paged tool-output list/page Tauri commands, typed frontend
  APIs, and a drawer viewer for session-scoped long-output reads.
- `/revert last-turn` now records assistant `message_id` and `part_id` on file
  changes, groups file-change rounds by assistant message before falling back to
  legacy tool round IDs, and preserves checkpoint safety.

Remaining follow-up hardening:

- Add the desktop button/view for "revert this assistant turn" once revert
  events are exported; the TUI command and backend mapping are in place, but
  desktop revert UX is still API-ready rather than product-visible.
- Emit a typed revert event into `session_events`/diagnostic export after a
  revert succeeds or partially succeeds.

## 1. Purpose

This plan follows the completed opencode programming-chain and agent-engine
alignment work:

- `docs/OPENCODE_PROGRAMMING_CHAIN_GAP_PLAN_2026-06-05.md`
- `docs/OPENCODE_AGENT_ENGINE_ALIGNMENT_PLAN_2026-06-06.md`

The prior phases made Priority Agent much closer to opencode on tool semantics,
file mutation safety, permissions, TUI diagnostics, usage accounting, cache
stability, todos, and provider runtime metadata.

This next phase should not add broad new agent features. The remaining useful
gap is architectural product polish: opencode is moving toward a durable,
event-projected session engine where text, reasoning, tool input, tool output,
usage, errors, queues, compaction, and replay all share one session model.

Priority Agent already has stronger local safety primitives than opencode:
checkpoint-backed writes, read-before-edit, proof-gated closeout, memory review
gates, failure-owner classification, and usage JSONL/SQLite accounting. The
next step is to make those primitives feel like one coherent, replayable product
loop across CLI, TUI, and desktop.

## 2. Evidence Reviewed

opencode source reviewed under `/Users/georgexu/Downloads/opencode-dev`:

- `packages/core/src/session/run-coordinator.ts`
- `packages/core/src/session/runner/llm.ts`
- `packages/core/src/session/runner/publish-llm-event.ts`
- `packages/core/src/session/event.ts`
- `packages/core/src/session/projector.ts`
- `packages/core/src/session/message-updater.ts`
- `packages/core/src/session/input.ts`
- `packages/core/src/session/sql.ts`
- `packages/core/src/tool-output-store.ts`
- `packages/opencode/src/session/compaction.ts`
- `packages/opencode/src/session/revert.ts`
- `packages/opencode/src/session/todo.ts`
- `packages/opencode/src/session/tools.ts`

Priority Agent source reviewed:

- `src/session_store/`
- `src/engine/streaming.rs`
- `src/engine/conversation_loop/`
- `src/desktop_runtime/mod.rs`
- `src/shell.rs`
- `src/tui/`
- `src/engine/checkpoint.rs`
- `src/cost_tracker/`
- `apps/desktop/src/app/runEventState.ts`
- `apps/desktop/src-tauri/src/lib.rs`

## 3. Current Assessment

Priority Agent is now strong enough to run real programming tasks. The remaining
gap is not "can it edit, run tools, validate, remember, and account for cost";
it can. The gap is how stable and replayable the whole runtime path is when a
task becomes long, interrupted, resumed, rendered in desktop, or debugged from
a daily baseline.

opencode is still ahead in four product-engineering areas:

1. Durable run coordination: one session has one active drain, queued user input
   is coalesced, and wakeups are explicit.
2. Event-projected session state: provider text, reasoning, tool input, tool
   result, usage, shell output, and compaction are all events projected into a
   replayable message table.
3. Tool output storage: huge outputs are stored behind a managed URI with safe
   paging instead of only truncated inline previews.
4. Message/part-level UX: UI can render assistant text, reasoning, tool calls,
   tool results, errors, and compaction as typed parts rather than one flat
   assistant string plus volatile tool cards.

Priority Agent should borrow these product surfaces without replacing its
existing runtime spine. The right approach is additive projection first, then
frontends gradually consume the projection.

## 4. Gap 1: Durable Session Event Ledger And Projection

### opencode behavior

opencode core defines typed session events such as:

- `Step.Started`, `Step.Ended`, `Step.Failed`
- `Text.Started`, `Text.Delta`, `Text.Ended`
- `Reasoning.Started`, `Reasoning.Delta`, `Reasoning.Ended`
- `Tool.Input.Started`, `Tool.Input.Ended`, `Tool.Called`, `Tool.Success`,
  `Tool.Failed`, `Tool.Progress`
- `Shell.Started`, `Shell.Ended`
- `Compaction.Started`, `Compaction.Ended`

`SessionProjector` applies those events into `session_message` rows, and
`SessionMessageUpdater` keeps assistant messages coherent even when events
arrive in fragments.

### Priority Agent status

Priority Agent has `StreamEvent`, `TurnTrace`, `learning_events`, `messages`,
and desktop/TUI event transforms. Those are useful, but they are still split:

- `StreamEvent` is live transport, not durable replay.
- `messages` stores flat user/assistant/tool rows.
- `TurnTrace` stores runtime facts, but it is not the canonical UI projection.
- Desktop reconstructs run state from streamed events and local UI state.

### Risk

After interruption, resume, or desktop reload, the product cannot always replay
exactly what happened: partial tool input, reasoning, tool output progress,
provider failure, and final settlement may be lost or reduced to generic text.

### Plan

Add a small event-projection layer before any broad rewrite.

Work items:

- Add `session_events` table:
  - `id`, `session_id`, `seq`, `event_type`, `timestamp_ms`, `payload_json`.
  - Use per-session sequence for deterministic replay.
  - Keep it append-only except test cleanup/session deletion.
- Add `session_projection` or `session_parts` table:
  - message id, part id, role/type, parent assistant id, status, payload, created
    and completed timestamps.
- Create `SessionEventWriter` with helpers for live runtime:
  - provider step started/ended/failed;
  - assistant text delta/end;
  - reasoning delta/end;
  - tool input/called/progress/success/failure;
  - usage and closeout summary.
- In the first slice, mirror existing `StreamEvent` into the durable event log
  without changing CLI/TUI behavior.
- Add a projector that can rebuild typed session parts from the event log.
- Add tests proving replay is stable when events arrive in normal and partial
  order.

Acceptance:

- A completed turn can be reconstructed from `session_events` without relying
  on in-memory TUI state.
- Tool lifecycle and assistant text are replayable after process restart.
- Existing `messages` and `turn_traces` remain intact during migration.

Suggested gates:

```bash
cargo test -q session_store
cargo test -q streaming
cargo test -q desktop_runtime
cargo check -q
```

## 5. Gap 2: Run Coordinator And Durable Input Queue

### opencode behavior

opencode `SessionRunCoordinator` guarantees one active drain chain per session.
`run` starts or joins an explicit drain; `wake` coalesces follow-up work; queued
inputs are promoted in order. `SessionInput` has durable `steer` and `queue`
delivery modes.

### Priority Agent status

Priority Agent can run one live stream in TUI/desktop and has cancellation and
runtime facade state. But user input during an active run is still mostly a UI
concern, not a durable session input queue. Desktop/TUI can diverge in how they
handle follow-up prompts, stop, and resume.

### Risk

Real users often type follow-up steering while an agent is still running. If
this is not durable, a desktop reload or runtime restart can lose intent or
cause duplicate turns.

### Plan

Add a minimal session run coordinator.

Work items:

- Add `session_inputs` table:
  - `id`, `session_id`, `delivery` (`steer` or `queue`), `content`, optional
    attachments/references JSON, `created_at`, `promoted_at`.
- Add `SessionRunCoordinator` service:
  - at most one active run per session in-process;
  - `run(session_id)` joins current explicit run;
  - `wake(session_id)` schedules one coalesced follow-up;
  - `await_idle(session_id)` for tests and desktop.
- Add TUI/desktop API path:
  - if run is active, new prompt is admitted as `steer` or `queue`;
  - runtime promotes steering before the next model step;
  - queued prompts run after current activity settles.
- Persist busy/idle/interrupted status in session metadata.
- Keep current synchronous CLI path initially; add queue behavior only for TUI
  and desktop first.

Acceptance:

- If a user submits a second prompt while a run is active, it is not dropped.
- Duplicate wakeups coalesce.
- Restart/reload can show pending queued input.
- Tests prove no two model drains run concurrently for the same session.

Suggested gates:

```bash
cargo test -q session_store
cargo test -q runtime_facade
cargo test -q desktop_runtime
cargo test -q tui
cargo check -q
```

## 6. Gap 3: Tool Output Store With Paging

### opencode behavior

opencode `ToolOutputStore` stores large outputs behind `tool-output://...`
resources. It keeps bounded inline previews and allows safe session-scoped
reads with offset/limit. It also has retention cleanup and UTF-8 boundary
handling.

### Priority Agent status

Priority Agent truncates output for desktop previews and TUI cards. Some tools
return structured metadata, but there is no shared managed output store for
large shell/test/build/log output.

### Risk

Long test output and server logs are either too large for the prompt/UI or too
truncated for debugging. This directly hurts real coding tasks.

### Plan

Add a local managed tool-output store.

Work items:

- Add `ToolOutputStore` under `src/tool_output_store/`:
  - managed directory under data dir;
  - metadata JSON with session id, tool call id, mime, name, size, created time;
  - content file with bounded retention;
  - URI format such as `tool-output://<id>`.
- Add APIs:
  - `truncate_or_store(session_id, tool_call_id, content)`;
  - `read_page(session_id, uri, offset, limit)`;
  - `cleanup_old_outputs()`.
- Wire into bash, run_tests, grep, and long tool outputs:
  - model-facing output gets head/tail preview plus URI marker;
  - TUI/desktop can open full output page by page;
  - `/diagnostic` includes referenced output resources.
- Add security checks:
  - session-scoped read;
  - invalid URI rejection;
  - UTF-8 safe offsets.

Acceptance:

- A long failing test output is not lost; user can page through it.
- Prompt stays bounded.
- Desktop and TUI show "full output available" instead of only a truncated blob.

Suggested gates:

```bash
cargo test -q tool_output_store
cargo test -q bash_tool
cargo test -q tui
cargo test -q desktop_runtime
cargo check -q
```

## 7. Gap 4: Typed Tool Lifecycle For UI And Replay

### opencode behavior

opencode publishes tool input start/delta/end, tool called, tool progress,
success/failure. `SessionMessageUpdater` projects those into a typed assistant
tool part with state `pending`, `running`, `completed`, or `error`.

### Priority Agent status

Priority Agent has `ToolRunView`, `StreamEvent::ToolExecution*`,
`ToolResult.data`, `mutation_result`, and desktop `runEventState`. This is
enough for live rendering, but not yet a durable typed projection.

### Risk

The UI can show one state live, but after resume the session store mainly has
flat assistant text and trace summaries. Tool cards are not guaranteed to be
replayable or identical across TUI and desktop.

### Plan

Unify tool lifecycle into typed session parts.

Work items:

- Define `SessionPart` enum:
  - `assistant_text`, `reasoning`, `tool`, `shell`, `permission`,
    `compaction`, `closeout`.
- Convert `ToolRunView` from owning state to rendering projected parts where
  possible.
- Persist tool input, status, structured result, compact content, full output
  resource URI, mutation result, and provider-executed metadata.
- Add a compatibility adapter from existing `StreamEvent` to `SessionPart`
  updates.
- Keep old TUI live view until projection has enough coverage.

Acceptance:

- TUI and desktop can render the same completed tool card from session store.
- A resumed session preserves tool status and structured output.
- Mutation results, diagnostics, and rewind hints survive restart.

Suggested gates:

```bash
cargo test -q tool_metadata
cargo test -q file_tool
cargo test -q desktop_runtime
cargo test -q tui
cargo check -q
```

## 8. Gap 5: Anchored Compaction And Output Pruning

### opencode behavior

opencode compaction keeps recent tail turns, maintains anchored summaries,
selects hidden history, prunes old tool outputs, and publishes compaction
events. It distinguishes compaction messages from normal assistant text.

### Priority Agent status

Priority Agent has context compression, compact boundaries, stable prefix work,
memory fences, and reactive compaction. The system is strong, but compaction is
still more runtime-internal than product-visible.

### Risk

Long sessions can become hard to reason about: users may not know what was
summarized, what remains exact, and what tool output was pruned or retained.

### Plan

Productize compaction visibility without adding prompt bloat.

Work items:

- Add a typed `compaction` session event/part:
  - reason, selected parent message, tail start id, summary, included range.
- Store compact boundaries in the same projection path used by session replay.
- Add `/context compact status` or extend `/context`:
  - stable prefix hash;
  - exact tail turns;
  - summarized turns;
  - pruned tool output count and retained output resources.
- Keep memory separate from compaction: compaction is session history hygiene,
  memory is long-term personal/project knowledge.

Acceptance:

- After compaction, user can see what was summarized and what remains exact.
- Daily baseline can include compaction decisions.
- Cache diagnostic can distinguish compaction-driven misses from memory/tool
  schema misses.

Suggested gates:

```bash
cargo test -q context_compressor
cargo test -q prompt_context
cargo test -q cache_stability
cargo test -q session_store
cargo check -q
```

## 9. Gap 6: Message/Round-Centric Revert In The Main UI

### opencode behavior

opencode revert works by `messageID` or `partID`, restores snapshots, applies
patch reversions, recomputes session diff, and publishes a diff event.

### Priority Agent status

Priority Agent has stronger low-level checkpoint safety and `/rewind`, but the
user-facing default is still more checkpoint/tool-round centric than
message/assistant-turn centric.

### Risk

When users ask "undo what the last agent turn did", the product should not make
them reason about checkpoint IDs.

### Plan

Map current checkpoint rounds to session parts.

Work items:

- Link `FileChangeRoundSummary` to assistant message id / tool call id in the
  session projection.
- Add a "revert this assistant turn" command/UX path:
  - TUI command: `/revert last-turn`;
  - desktop button on the last mutation group.
- After revert, emit a typed revert event with restored paths and diff summary.
- Keep `/rewind checkpoint_id` for advanced recovery.

Acceptance:

- Last assistant turn can be reverted with one command.
- Revert result is visible in session history and diagnostic export.
- Checkpoint safety remains mandatory.

Suggested gates:

```bash
cargo test -q checkpoint
cargo test -q rewind
cargo test -q file_tool
cargo test -q desktop_runtime
cargo check -q
```

## 10. Gap 7: Provider And Tool Settlement Invariants

### opencode behavior

opencode fails unsettled tools if the provider ends unexpectedly, records
provider-executed tools, and only continues the loop after local tool fibers
settle. It also has a step limit.

### Priority Agent status

Priority Agent has bounded iterations, failure-owner classification, tool
batch processing, and closeout proof gates. Recent work now records provider
metadata into usage ledger entries. But provider/tool settlement is still
spread across several controllers and live stream events.

### Risk

If provider streaming ends after partial tool calls, or if a tool is interrupted
after being started, replay and closeout may not always have one canonical
settlement record.

### Plan

Add settlement assertions around the existing runtime.

Work items:

- Add `ToolSettlementLedger` per turn:
  - every tool input start must become called, success, failure, cancelled, or
    provider-executed;
  - every provider step must end as success, failure, timeout, or cancelled.
- Emit settlement gaps as runtime diagnostics and trace events.
- Add a gate that blocks verified closeout when settlement is incomplete.
- Add tests for partial provider stream with open tool call, interrupted tool,
  and provider-executed tool.

Acceptance:

- No tool call can remain silently pending after a turn completes.
- Diagnostic export includes settlement gaps.
- Closeout cannot claim verified when settlement is incomplete.

Suggested gates:

```bash
cargo test -q tool_batch_result_processor
cargo test -q closeout
cargo test -q runtime_spine
cargo test -q conversation_loop
cargo check -q
```

## 11. Recommended Implementation Order

### Phase 1: Tool Output Store

This is the smallest high-impact slice. It improves real debugging immediately
and does not require a session architecture rewrite.

Deliverables:

- `ToolOutputStore`
- bash/run_tests long-output integration
- TUI/desktop full-output paging
- diagnostic export references

### Phase 2: Session Event Mirror

Mirror existing `StreamEvent` into `session_events` and add a projector without
changing frontends yet.

Deliverables:

- `session_events` table
- append-only event writer
- replay/projector tests
- no UI behavior change required

### Phase 3: Typed Session Parts For Tool Cards

Start consuming the projection in TUI/desktop for completed runs.

Deliverables:

- `SessionPart` projection
- persisted `session_parts` table refreshed from `session_events`
- TUI tool cards render from persisted parts when available
- desktop reload shows persisted completed run parts

### Phase 4: Run Coordinator And Input Queue

Once replay is stable, make active-run input durable.

Deliverables:

- `session_inputs` table
- single-run coordinator
- queued/steering input for desktop and TUI
- busy/idle status persisted

### Phase 5: Compaction And Revert Productization

Wire compaction and revert into session parts after the event/projection base is
stable.

Deliverables:

- typed compaction part
- `/context compact status`
- turn-centric revert
- revert events in diagnostic export

2026-06-07 status: typed compaction parts and turn-centric revert mapping are
implemented. Revert diagnostic events and desktop revert UX remain follow-up
hardening.

## 12. Non-Goals

- Do not replace Priority Agent's memory system with opencode's lighter model.
- Do not weaken checkpoint, permission, or closeout proof gates to match
  opencode's simpler flow.
- Do not rewrite the whole runtime into event sourcing in one pass.
- Do not add broad plugin architecture work until core session replay is stable.
- Do not optimize for prettier UI before the underlying projection is durable.

## 13. Definition Of Done

This next phase is done when:

- long tool output can be safely recovered after truncation;
- completed runs can be replayed from durable events/parts;
- TUI and desktop share the same persisted tool-card data;
- active-run follow-up user input is durable and ordered;
- compaction and revert have typed persisted state and checkpoint-safe command
  behavior;
- daily diagnostic export can reconstruct what happened without relying on
  in-memory UI state.

2026-06-07 verification:

```bash
cargo fmt --check
cargo check -q
cargo test -q checkpoint --lib
cargo test -q session_parts --lib
cargo test -q event_store --lib
cargo test -q session_store --lib
cargo test -q tui --lib
cargo test -q desktop_runtime --lib
cargo test -q tool_output_store --lib
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
npx -y pnpm@11.2.2 build
```
