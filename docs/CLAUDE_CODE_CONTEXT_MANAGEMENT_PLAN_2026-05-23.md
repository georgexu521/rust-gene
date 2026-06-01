# Claude Code Context Management Plan

Date: 2026-05-23

Status: created after inspecting local Claude source and Priority Agent's
current context/runtime implementation.

Progress through 2026-05-24:

- Track A is implemented at the runtime layer with provider/model context
  profiles, output reserve, auto-compact thresholds, warning thresholds, and
  hard-block thresholds.
- Track B is implemented with a canonical context usage snapshot that combines
  history, prompt, memory, tools, usage, and cache pressure signals.
- Track C is implemented with durable compact boundary persistence and desktop
  restored-session binding through compacted session restore.
- Track D is implemented with a session context ledger for file reads and
  read-only bash facts. The earlier repeated-read runtime closeout that
  answered from ledger/cache facts was removed on 2026-06-02 as part of runtime
  diet; ledger facts should inform prompt context and trace, not generate final
  answers outside the LLM.
- Track E has its first product layer: tool-result normalization now applies
  context policy, keeps model-facing summaries factual, and marks ledger-fact
  eligible results.
- Track F has its first product layer: preflight, streaming pre-query,
  provider reactive retry, and manual compact now share
  `CompactionAttemptRecord` state with no-gain/circuit-open detection; manual
  compact is exposed through CLI `/compact` and a desktop Tauri command.
- Track G has started: desktop now has a runtime context snapshot API, a topbar
  context meter, environment-popover context details, and a manual compact
  action wired to shared runtime compaction. Restored sessions also render
  compact boundary cards in the transcript.
  Trace drawer now includes context pressure and compact-state summary.
  Responses that avoid repeated reads through ledger reuse now get an explicit
  transcript card.
  Track G is now complete at the planned first release level.
- Track H has started: deterministic long-session compaction now verifies
  boundary persistence, restored desktop runtime snapshots expose compact
  boundary state, and `scripts/release-dogfood-gate.sh quick` passes.

Goal: make long desktop coding sessions reliable by turning context window
tracking, prompt cache awareness, automatic compaction, restored-session
continuity, and read/tool-result reuse into explicit runtime state.

This plan is Claude Code-informed. It must not copy Claude source bodies,
vendor prompt text, analytics strings, or UI copy. The target is to reproduce
the behavior shape with Priority Agent's Rust runtime and desktop session
model.

## Why This Matters

Desktop users expect one conversation to keep working until the model context is
near full, then continue after automatic compaction without losing the active
task. If this is weak, the agent will:

1. repeatedly read the same files because it is unsure whether prior tool output
   is still available;
2. forget prior decisions, modified files, validation results, and user
   corrections after long runs;
3. hit provider context-limit errors instead of compacting and retrying;
4. restore an old desktop session in a visually convincing way while runtime
   state is actually incomplete.

Priority Agent already has many pieces. The next work is to make them one
coherent product-level state machine.

## Claude Source Inspected

Primary local reference files:

- `/Users/georgexu/Desktop/claude/src/query.ts`
- `/Users/georgexu/Desktop/claude/src/services/compact/autoCompact.ts`
- `/Users/georgexu/Desktop/claude/src/services/compact/compact.ts`
- `/Users/georgexu/Desktop/claude/src/services/compact/microCompact.ts`
- `/Users/georgexu/Desktop/claude/src/services/compact/sessionMemoryCompact.ts`
- `/Users/georgexu/Desktop/claude/src/services/compact/postCompactCleanup.ts`
- `/Users/georgexu/Desktop/claude/src/services/compact/prompt.ts`
- `/Users/georgexu/Desktop/claude/src/query/tokenBudget.ts`
- `/Users/georgexu/Desktop/claude/src/utils/tokens.ts`

Important behavior shape extracted:

1. Request assembly starts from messages after the latest compact boundary, then
   applies reducers in a deliberate order: tool-result budget, snip,
   microcompact, context collapse projection, auto compact, then API call.
2. Auto compact uses model-specific context windows, output-token reserve, warn
   and hard-block thresholds, settings/env switches, and a consecutive-failure
   circuit breaker.
3. Token accounting prefers the latest API usage and estimates only messages
   added after that. It also treats cache read/create tokens as part of context
   pressure when appropriate.
4. Compaction produces post-compact messages: a boundary marker, summary
   message, preserved recent tail, attachments, and session-start hook context.
5. Session memory compaction can use extracted session memory and a last
   summarized message id, while preserving enough recent messages and avoiding
   broken tool-use/tool-result pairs.
6. Reactive compaction handles real prompt-too-long or media-too-large failures
   and retries once through explicit state transitions.
7. Post-compact cleanup resets caches that are invalidated by compaction while
   intentionally preserving expensive state that should survive.
8. Tool-use summaries and tool-result shrinking reduce future context pressure
   without forcing a full conversation summary every time.

## Priority Agent Current Anchors

Runtime and storage:

- `src/engine/context_compressor.rs`
- `src/engine/context_collapse.rs`
- `src/engine/streaming.rs`
- `src/engine/conversation_loop/preflight_compression_controller.rs`
- `src/engine/conversation_loop/api_request_controller.rs`
- `src/engine/conversation_loop/context_budget_controller.rs`
- `src/session_store/mod.rs`
- `src/engine/evidence_ledger.rs`
- `src/tools/file_cache.rs`

Desktop/UI:

- `src/desktop_runtime/`
- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/src/app/runEventState.ts`
- `apps/desktop/src/app/components/Transcript.tsx`
- `apps/desktop/src/app/components/DiagnosticsPanel.tsx`

Existing strengths to preserve:

- `StreamingQueryEngine` has a shared `ContextCompressor`.
- Preflight compression and provider context-too-long reactive retry exist.
- Compaction runtime records include strategy, boundary, retained items, token
  delta, and provenance.
- Session store has message persistence, FTS, learning events, and helper
  methods for deleting or rewriting messages.
- Tool metadata, trace events, desktop timeline, and file cache already provide
  the raw material for a durable context ledger.

## Gaps To Close

### Gap 1: Static Context Window

Current desktop/streaming default is `128_000` tokens. Provider/model changes
reset to the same static value unless explicitly overridden. This is not enough
for provider-specific models, output reserve, or auto-compact thresholds.

Target:

- add a `ModelContextProfile` registry with:
  - provider;
  - model pattern;
  - context window;
  - reserved output tokens;
  - auto-compact threshold;
  - warning and hard-block thresholds;
  - cache accounting behavior;
  - safe fallback defaults.
- feed this profile into `ContextCompressor`, diagnostics, desktop settings,
  and trace.

### Gap 2: Token Accounting Is Too Approximate

Priority Agent mostly estimates tokens from text length and tool schema size.
Claude's loop treats the last API response usage as the best known context
anchor, then estimates only the messages added after it.

Target:

- introduce `ContextUsageSnapshot` as the canonical budget input;
- use provider usage fields already captured by `Usage`:
  `prompt_tokens`, `completion_tokens`, `reasoning_tokens`, `cached_tokens`;
- keep separate:
  - request pressure tokens;
  - billing tokens;
  - cache read/create tokens when available;
  - estimated additions after the latest usage-bearing assistant message;
  - reserved output tokens.
- make all context decisions use the same snapshot.

### Gap 3: Compaction Is Not Durable Enough

Compaction can update in-memory history, but desktop session persistence still
mostly stores user/assistant messages as append-only records. A restored session
needs to resume from the post-compact state, not reconstruct pressure from a
large raw transcript.

Target:

- add durable compact boundary records to session storage;
- persist the post-compact message set or a compacted segment record;
- preserve transcript access for older raw details without sending all old
  details back to the model;
- store `before_tokens`, `after_tokens`, `strategy`, `trigger`,
  `boundary_id`, `preserved_tail_count`, and retained facts;
- make `DesktopRuntime::bind_session` restore compacted runtime state.

### Gap 4: Read/Tool Output Reuse Is Per-Turn, Not Session-Level

The current duplicate read guard stops repeated successful read-only tools in a
single run. The `phageGPT/README.md` case shows that this does not yet feel
like true context reuse to the model.

Target:

- add a session-level `ContextLedger`;
- record stable facts for:
  - file reads: path, content hash, line count, char count, excerpt summary,
    read timestamp, project root, dirty-state relation;
  - bash reads/searches: command, cwd, exit status, key output facts;
  - diffs and file edits: changed files, hunks, validation result;
  - diagnostics and provider state.
- inject compact ledger facts into prompts before allowing redundant read-only
  calls;
- only require re-read when file hash/mtime changed, path is outside ledger
  scope, or user explicitly asks for exact current content.

### Gap 5: Tool Result Shrinking Needs A Formal Pipeline

Claude applies tool-result budget and microcompact before full auto compact.
Priority Agent has truncation and evidence records, but not one explicit
pipeline that decides what the model sees, what UI shows, and what remains as
an artifact.

Target:

- define `ToolResultContextPolicy`:
  - provider-visible preview;
  - desktop-visible detail;
  - trace/debug payload;
  - durable artifact path when large;
  - ledger facts for reuse;
  - compaction eligibility;
  - protected recent tail.
- shrink old read/search/bash outputs before full summary compaction;
- preserve tool-use/tool-result pairs and insert explicit stubs when content is
  intentionally omitted.

### Gap 6: Auto/Reactive Compact State Machine Is Incomplete

Preflight and reactive compaction exist, but they should share a single state
model: considered, skipped, compacted, no-gain, failed, circuit-open, retrying,
and recovered.

Target:

- introduce `CompactionDecision` and `CompactionAttemptRecord`;
- enforce consecutive failure limits;
- avoid repeated no-gain compactions;
- add explicit triggers:
  - preflight threshold;
  - provider context error;
  - restored session over threshold;
  - manual compact;
  - large tool-result pressure;
  - cache-stale/time-based cleanup;
- emit the same record to trace, desktop events, session store, and logs.

### Gap 7: Desktop Does Not Explain Context State

The desktop app can show token usage and trace details, but context management
is not yet a visible, understandable part of the product.

Target:

- add a small context meter in the title/composer area;
- add compact boundary cards to transcript when compaction happens;
- add a trace drawer section for context:
  - model context profile;
  - current usage snapshot;
  - attached context;
  - recent ledger facts;
  - compaction history;
  - cache token usage when provider reports it;
- add a manual "Compact conversation" action in desktop settings or composer
  menu.

## Implementation Tracks

## Progress Update: 2026-05-23

Implemented in this pass:

- Track A first layer: `ModelContextProfile` now detects provider/model context
  windows and output reserves for MiniMax, Claude/Anthropic-like,
  Kimi, OpenAI-compatible, reasoning-capable, and safe fallback models.
- Track B first layer: `ContextUsageSnapshot` is now the canonical request
  pressure estimate used by the conversation-loop budget controller.
- Track C first layer: compact boundary records are durable in session storage
  and are persisted from preflight, streaming pre-query, and provider
  context-error reactive compaction. Session storage now also has
  `rewrite_session_messages_after_compact` and `restore_compacted_messages`
  APIs for a compacted runtime continuation surface, and desktop session
  restore now uses that restoration path.
- Track D first layer: `ContextLedger` records successful `file_read` facts
  plus read/list/search bash facts as structured session events and injects
  recent ledger facts into request prompts so the model can reuse prior reads
  instead of rereading whole files.
- Superseded Track D follow-up: repeated read-only closeout was briefly
  ledger-aware, but that path is now removed. Exact duplicate read loops should
  be bounded by the shared storm guard and iteration budget; useful prior-read
  facts should enter as context ledger evidence.
- Track E first layer: `ToolResultContextPolicy` is now attached during tool
  result normalization and marks provider-visible size, desktop-visible size,
  trace payload availability, durable artifacts, ledger eligibility,
  compaction eligibility, and protected recent-tail status.

Still remaining before this plan is fully done:

- deepen `ToolResultContextPolicy` into old-result shrinking and artifact-backed
  provider stubs;
- unify compaction attempts into a state machine with no-gain/circuit-breaker
  records;
- expose context profile, usage, ledger facts, and compact boundaries in the
  desktop UI;
- add the long-session release gate.

### Track A: Model Context Profiles

Priority: P0.

Tasks:

1. Add `src/engine/model_context.rs`.
2. Define `ModelContextProfile` and lookup by provider/model.
3. Replace hard-coded `128_000` defaults in streaming/provider switching.
4. Expose profile through diagnostics and desktop API.
5. Add tests for MiniMax, OpenAI-compatible, Kimi, Claude-compatible, and
   unknown fallback models.

Acceptance:

```bash
cargo test -q model_context -- --test-threads=1
cargo test -q streaming -- --test-threads=1
```

### Track B: Canonical Context Usage Snapshot

Priority: P0.

Tasks:

1. Add `ContextUsageSnapshot` and make `context_usage_report` use it.
2. Include actual provider usage when available.
3. Count tool schema, memory snapshot, system prompt, history, and reserved
   output as separate fields.
4. Update preflight compression to use this snapshot.
5. Add trace events for usage snapshots before request and after response.

Acceptance:

```bash
cargo test -q context_budget_controller -- --test-threads=1
cargo test -q context_compressor -- --test-threads=1
```

### Track C: Durable Compact Boundaries

Priority: P0.

Tasks:

1. Add a session-store migration for `compact_boundaries`.
2. Add APIs:
   - `add_compact_boundary`;
   - `list_compact_boundaries`;
   - `rewrite_session_messages_after_compact`;
   - `restore_compacted_messages`.
3. Persist compact records from preflight, reactive, and manual compaction.
4. Ensure restored desktop sessions bind to compacted runtime history.
5. Add a transcript path or raw-message archive pointer for details that are no
   longer sent to the model.

Acceptance:

```bash
cargo test -q session_store -- --test-threads=1
cargo test -q streaming -- --test-threads=1
```

### Track D: Session Context Ledger

Priority: P0.

Tasks:

1. Add `src/engine/context_ledger.rs`.
2. Record file-read facts from `file_read` and read-like bash/search commands.
3. Store ledger facts in session store as structured learning/context events.
4. Inject recent relevant ledger facts into `PromptContextAssembler`.
5. Keep prior read facts model-facing through context ledger evidence. Do not
   reintroduce a runtime-generated final answer path for repeated reads.

Acceptance:

```bash
cargo test -q file_cache -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q turn_iteration_controller -- --test-threads=1
```

### Track E: Tool Result Context Policy

Priority: P1.

Tasks:

1. Define `ToolResultContextPolicy`.
2. Apply it in tool result normalization before provider-visible append.
3. Store large raw outputs as artifacts.
4. Keep compact model-facing summaries stable and factual.
5. Add protected recent tail rules so the last useful tool results remain
   visible verbatim.

Acceptance:

```bash
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q tool_batch_result_processor -- --test-threads=1
```

### Track F: Unified Compaction State Machine

Priority: P1.

Tasks:

1. Done: add `CompactionDecision` and `CompactionAttemptRecord`.
2. Done: unify preflight, streaming-level pre-query compression, and API
   reactive compression through the same attempt recorder.
3. Done: add no-gain detection and consecutive-failure circuit breaker.
4. Done: add manual compact entrypoint usable by CLI and desktop.
5. Partly done: manual compact now clears transient file/read cache while
   preserving durable ledger facts. Remaining work is to route richer desktop
   context projections through the same post-compact cleanup hook once Track G
   adds them.

Acceptance:

```bash
cargo test -q context_compressor -- --test-threads=1
cargo test -q streaming -- --test-threads=1
cargo test -q api_request_controller -- --test-threads=1
```

### Track G: Desktop Context UX

Priority: P1.

Tasks:

1. Done: add context meter and compact state to desktop runtime snapshot.
2. Done: render compact boundary cards in transcript.
3. Done: add context section to trace drawer.
4. Done: add manual compact action.
5. Done: show when a response reused ledger facts instead of re-reading a file.

Acceptance:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

### Track H: Long Session Release Gate

Priority: P0 before claiming done.

Tasks:

1. Done: add a deterministic long-session fixture that:
   - reads several files;
   - edits one file;
   - runs validation;
   - triggers compaction with a low test threshold;
   - continues the task after compaction;
   - asks about a previously read README without causing repeated reads.
2. Done: add a restored desktop session case:
   - load compacted session;
   - verify context meter and compact boundary;
   - continue coding without losing prior facts.
3. Remaining: add a provider context-too-long simulation that proves reactive
   compact retry succeeds once and then records failure if still over limit.

Acceptance:

```bash
cargo test -q context_long_session -- --test-threads=1
cargo test -q desktop_runtime -- --test-threads=1
scripts/release-dogfood-gate.sh quick
```

## Implementation Order

1. Track A: model context profiles.
2. Track B: canonical usage snapshot.
3. Track C: durable compact boundaries and restored-session binding.
4. Track D: context ledger evidence. Repeated exact reads are bounded by the
   shared storm guard; the context ledger should not synthesize final answers.
5. Track F: unified compaction state machine.
6. Track E: tool-result context policy.
7. Track G: desktop context UX.
8. Track H: long-session release gate.

The first four tracks should land before judging long-session read reuse solved.
The current policy is deliberately narrow: prior read facts should be durable,
visible, and model-facing, while exact duplicate read loops are bounded by the
shared storm guard and the iteration cap.

## Non-Goals

- Do not copy Claude prompt templates or source code.
- Do not add more always-on prompt rules when runtime state can enforce the
  behavior.
- Do not make context compaction only a desktop UI feature; it must live in the
  shared runtime.
- Do not claim parity from unit tests alone. A restored long desktop session
  must pass before this is considered product-ready.

## Done Definition

This plan is done when:

1. context windows are provider/model-aware;
2. usage snapshots are canonical and visible in trace/desktop;
3. compaction boundaries persist across app restarts;
4. restored sessions continue from compacted runtime history;
5. repeated read-only tools can be answered from unchanged ledger facts;
6. provider context-too-long errors compact and retry without losing state;
7. desktop shows context state clearly without turning the transcript into logs;
8. the long-session release gate passes.
