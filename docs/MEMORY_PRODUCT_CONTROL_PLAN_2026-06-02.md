# Memory Product Control Plan

Date: 2026-06-02
Status: Proposed next implementation plan

This plan follows `docs/MEMORY_SYSTEM_SIMPLIFICATION_PLAN_2026-06-02.md`.
The simplification work made the memory contract understandable:

- pinned memory is compact stable prompt context;
- recall is dynamic per-turn background context;
- learning proposals are review-first candidates.

The next step is to make memory easier for gex to control in real use. The
goal is not to make memory more automatic. The goal is to make memory more
predictable, inspectable, and useful.

## 1. Reference Product Lessons

### 1.1 Codex

Codex treats memory as a local recall aid, not as project policy.

Useful lessons:

- memory usage and future memory generation should be separately controllable;
- long-lived project rules belong in `AGENTS.md` or project docs, not inferred
  memories;
- memory generation should be conservative, background, and sensitive-data
  aware;
- short-lived or active-thread details should not automatically become durable
  memory.

Priority Agent should learn from this by exposing explicit thread/session
controls:

- use existing memory or ignore it;
- allow this session to generate future memory candidates or do not;
- keep rule-like content in project docs, not generated memory.

### 1.2 OpenClaw

OpenClaw's Active Memory design is useful because it makes recall explicit.
A memory sub-agent gets one bounded chance to search before the main response,
and may return `NONE`.

Useful lessons:

- active recall should be opt-in or policy-gated;
- recall should support modes such as strict, balanced, and preference-only;
- recall should use narrow tools, not a broad tool surface;
- no relevant memory is a valid result;
- background, one-shot, helper, and sub-agent paths should not all run memory
  recall by default.

Priority Agent should learn from this by turning recall behavior into an
explicit product setting instead of a hidden heuristic.

### 1.3 Hermes

Hermes is strongest as a personal-agent memory product. Its core lesson is that
the always-present memory should be small and curated, while larger history
belongs behind search.

Useful lessons:

- core memory should stay small, stable, and always inspectable;
- preferences, profile facts, and stable project conventions are good memory;
- workflows and repeatable procedures should become skills, not memory blobs;
- periodic review can propose memory or skill updates, but should not silently
  rewrite the agent's personality or project rules;
- transcript/session search and durable memory must stay separate.

Priority Agent should keep the current review-required boundary and improve the
human review loop.

## 2. Current Priority Agent Position

Priority Agent is now in a good middle position:

- more structured and auditable than a simple Markdown-only memory system;
- more conservative than always-on active memory;
- more local-personal than a generic coding assistant memory layer;
- already split into pinned memory, recall, and proposals.

The remaining product gaps are mostly control and ergonomics:

- gex cannot yet clearly toggle "use memory" and "generate memory" per session;
- active recall modes are not exposed as a simple user-facing policy;
- prompt-cache behavior is not yet expressed as a first-class memory policy;
- proposal review exists, but needs a smoother accept/reject/merge workflow;
- memory file selection is traceable, but not yet easy enough to inspect from
  one command;
- skill candidates and memory candidates are still too easy to blur together.

## 2.1 Static Prefix Risk Audit

Priority Agent already moved many dynamic context blocks toward a
Reasonix-style static-prefix shape. Most request-time context now goes near the
last user message through `prepend_to_last_user_message`, which is the right
direction for prompt caching.

However, the current system is not yet a pure stable-prefix design. The risks
below should be fixed before treating memory and runtime context as
cache-stable.

### Risk 1: Task Focus Is Still Added To The System Prompt

Current risk:

- `PromptContextAssembler::build_for_turn()` uses the current user message and
  history to infer task type.
- `prompt_builder` then appends `Task Focus: Coding`, `Debugging`, `Review`, or
  `Architecture` to the system prompt.
- This means two turns in the same session can produce different system prompt
  fingerprints when the user's task type changes.

Recommendation:

- Keep the base system prompt static.
- Move task-focus guidance into a dynamic tail zone such as `<task-state>` or
  `<current_decision_request>`.
- Track task-focus as dynamic routing/context, not as stable system policy.

Acceptance checks:

- A coding turn followed by a review/debugging turn keeps the same stable
  system fingerprint.
- The task-focus text is still available to the model through dynamic context.

### Risk 2: Pinned Memory Snapshot Is Not Always Stable Across Turns

Current risk:

- Pinned memory is compact and session-frozen, which is cache-friendly.
- But `MemorySnapshotController` skips pinned snapshot injection when dynamic
  memory recall exists.
- That avoids duplicate memory context, but it also means some turns have the
  pinned memory index in the stable prefix and some do not.

Recommendation:

- Treat compact pinned memory as the stable map of available memory.
- When `memory.use=on`, inject the same frozen pinned memory index into the
  stable prefix for every turn in the session.
- Keep detailed dynamic recall in the user tail / `relevant_material` zone.
- Do not refresh pinned memory mid-session after writes unless the user starts
  a new session or explicitly asks to refresh memory.

Acceptance checks:

- Dynamic memory recall changing between turns does not change the stable
  prefix fingerprint.
- Mid-session memory writes do not mutate the pinned memory snapshot.
- Doctor reports both pinned-memory fingerprint and dynamic recall fingerprint.

### Risk 3: RetrievalPromptController Still Inserts Dynamic System Messages

Current risk:

- `RetrievalPromptController` wraps retrieval in `<relevant_material>`, but
  still inserts it as `Message::system(...)` before the user message.
- Cache diagnostics can classify this as dynamic context, but the message shape
  is inconsistent with the newer user-tail strategy.

Recommendation:

- Route retrieval prompt injection through the same user-tail path as
  `RequestPreparationController`.
- Keep retrieval fenced as `<relevant_material>`, but do not add it as a
  separate system message.

Acceptance checks:

- Retrieval context appears before the latest user content, inside the user
  message tail.
- No new system message is created for retrieval context.

### Risk 4: Runtime Corrections And Guards Are Bare System Messages

Current risk:

- Several runtime feedback paths push plain `Message::system(...)` during or
  after tool execution.
- Examples include:
  - `tool_batch_result_processor.rs`
  - `assistant_response_retry_controller.rs`
  - `post_edit_verification_controller.rs`
  - `tool_failure_guided_debugging.rs`
- These are dynamic repair hints, guardrails, or observations. If they remain
  in history as untagged system messages, later cache diagnostics may count
  them as stable system material.

Recommendation:

- Convert these to tagged dynamic context, usually `<recent_observation>` or
  `<task-state>`.
- Prefer user-tail injection or tool/session evidence over bare system
  insertion.
- If a system message is unavoidable, it must use a recognized dynamic prefix
  so `is_dynamic_context_system_message` excludes it from stable-prefix
  accounting.

Acceptance checks:

- Tool failure correction, destructive-scope guard, retry correction, and
  post-edit verification hints no longer appear as untagged system messages.
- Cache diagnostics list zero unexpected dynamic system messages after a tool
  failure turn.

### Risk 5: Route-Scoped Tool Schemas Can Still Change The Cached Prefix

Current risk:

- Tool schema is part of the cached prefix shape.
- The project already canonicalizes tool ordering, which is good.
- But route-specific tool exposure can still change the tool list and schema
  fingerprint across turns.

Recommendation:

- Keep a small static core tool surface when cache stability matters.
- Use deferred/route-scoped tools for expensive or rarely needed tools, but
  make the cache tradeoff explicit.
- Report tool list/schema changes as first-class cache-miss reasons.
- Do not solve this by always exposing every tool if that would create broad
  schema noise and waste prompt tokens.

Acceptance checks:

- Cache diagnostics distinguish `system`, `tools`, `dynamic_tail`, and
  `message_count` causes.
- Route changes that alter tool schemas are visible in doctor/trace output.
- Core memory controls do not add broad external provider tools by default.

## 3. Product Rules To Preserve

These rules should not be weakened while adding controls:

- Do not silently auto-write durable memory by default.
- Do not let memory override `AGENTS.md`, project docs, permissions,
  checkpoints, validation gates, or tool contracts.
- Do not store task progress, command logs, or temporary tool output as user
  memory.
- Do not store repeatable procedures as memory when they should become skills.
- Do not let project-scoped records cross project identities.
- Do not inject large memory bodies into the stable prompt.
- Do keep the stable prompt prefix cache-friendly: pinned memory may enter the
  stable prefix only as compact indexes that remain fixed for the session.
- Do keep dynamic recall in the user-tail/relevant-material zone so it does not
  churn the cached stable prefix.
- Do not expose external provider tools by default.

## 4. Proposed User-Facing Model

The user should be able to explain memory like this:

1. `Pinned memory`
   Small stable indexes from `MEMORY.md`, `USER.md`, and topic files. These help
   the agent know what memory exists, but they are not hidden instructions.

2. `Recall`
   Per-turn search results selected for the current request. Recall can include
   typed records, topic files, project progress, session search, and optional
   external provider context.

3. `Proposals`
   Candidate memories discovered during work. They wait for review before
   becoming durable memory.

4. `Skills`
   Repeatable workflows, procedures, and operational playbooks. These are not
   memory and should have their own review/test path.

## 5. Implementation Phases

### Phase 0: Make Memory Cache Policy Explicit

Goal: make memory useful without making every turn pay for a changing prompt
prefix.

Current implementation is partly cache-friendly:

- pinned memory snapshots are compact index-style context;
- snapshots are frozen for a session;
- dynamic recall is prepended near the last user message rather than changing
  the earliest stable prompt;
- cache diagnostics already track stable-prefix fingerprints.

The remaining policy gap is that pinned snapshot injection can be skipped when
dynamic memory recall exists. That avoids duplicate memory context, but it also
means the stable prefix does not always carry the same memory index across the
session.

Preferred policy:

- freeze pinned memory before the first model request in a session;
- inject the compact pinned memory index into the stable prefix consistently
  for the whole session when `memory.use=on`;
- never inject large memory bodies into that stable prefix;
- keep dynamic recall in `relevant_material` or the last-user-message tail;
- move task-focus guidance out of the system prompt and into dynamic tail
  context;
- convert retrieval prompt injection to user-tail insertion instead of dynamic
  system messages;
- convert runtime repair hints, retry corrections, and guard messages from bare
  system messages into tagged dynamic context;
- do not refresh pinned memory mid-session after a memory write unless the user
  explicitly asks for a memory refresh/new session;
- record the pinned-memory fingerprint, stable-prefix fingerprint, and dynamic
  recall fingerprint separately.

This preserves prompt-cache locality while still letting the model know that
memory exists. The stable prefix carries the small map; dynamic recall carries
the selected details.

Likely files:

- `src/engine/conversation_loop/memory_snapshot_controller.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/conversation_loop/turn_request_bootstrap_controller.rs`
- `src/engine/conversation_loop/retrieval_prompt_controller.rs`
- `src/engine/prompt_builder.rs`
- `src/engine/cache_stability.rs`
- `src/memory/manager.rs`

Acceptance checks:

- With `memory.use=on`, the same frozen pinned memory index is present in the
  stable prefix across turns in the same session.
- Dynamic recall changes do not change the stable-prefix fingerprint.
- Task type changes do not change the stable-prefix fingerprint.
- Retrieval context does not create a separate dynamic system message.
- Runtime repair/guard feedback is tagged as dynamic context rather than bare
  system prompt material.
- A mid-session memory write does not mutate the pinned prefix until a refresh
  or new session.
- When `memory.use=off`, neither pinned memory nor dynamic recall is injected.
- Doctor reports pinned-memory fingerprint separately from dynamic recall trace.

### Phase 1: Add Session Memory Controls

Goal: make memory use and memory generation explicit.

Add controls equivalent to:

- `memory.use`: whether existing memory can be used in this session;
- `memory.generate`: whether this session may create future memory proposals;
- `memory.active_recall`: whether pre-response recall runs automatically;
- `memory.write_policy`: `review_only`, `narrow`, or `legacy`.

Suggested command surface:

```text
/memory control
/memory control use on|off
/memory control generate on|off
/memory control recall strict|balanced|preference-only|off
/memory control write review-only|narrow
```

Likely files:

- `src/services/config.rs`
- `src/tools/memory_tool/mod.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/conversation_loop/memory_sync_controller.rs`

Acceptance checks:

- Turning memory use off prevents pinned snapshot and dynamic recall injection.
- Turning generation off still allows using existing memory but suppresses new
  memory proposals from this session.
- Default remains use-on, generate-review-only, write-review-only.

### Phase 2: Productize Recall Modes

Goal: make recall behavior predictable.

Add explicit recall modes:

- `off`: no dynamic memory recall;
- `strict`: only high-confidence, directly relevant memory;
- `balanced`: current default behavior;
- `preference-only`: only user preferences and durable collaboration style;
- `debug`: include trace-heavy recall output for inspection.

The mode should affect:

- candidate budget;
- source types allowed;
- score threshold;
- whether project progress/session search are included;
- trace verbosity.

Likely files:

- `src/memory/retrieval.rs`
- `src/memory/ranking.rs`
- `src/engine/retrieval_context.rs`
- `src/engine/conversation_loop/turn_retrieval_context_controller.rs`

Acceptance checks:

- `strict` returns fewer results than `balanced`.
- `preference-only` does not inject project progress or topic workflow files.
- `off` produces no dynamic recall but still permits explicit `/memory search`.
- Each mode writes a clear `MemoryRetrievalTrace`.

### Phase 3: Improve Proposal Review UX

Goal: make review-required memory useful instead of hidden.

Add a clearer review flow:

```text
/memory proposals
/memory proposal show <id>
/memory proposal accept <id>
/memory proposal reject <id>
/memory proposal edit <id>
/memory proposal merge <id> <existing-record-id>
```

Review UI should show:

- proposed memory content;
- target surface: user, project, topic, skill, or reject;
- evidence;
- reason;
- sensitivity;
- conflicts;
- duplicate candidates;
- expected file/projection path.

Likely files:

- `src/tools/memory_tool/mod.rs`
- `src/engine/task_contract.rs`
- `src/memory/manager.rs`
- `src/memory/reports.rs`

Acceptance checks:

- A closeout/background proposal can be accepted without hand-editing JSON.
- A rejected proposal is recorded and does not reappear as a duplicate.
- A conflict can be merged into an existing accepted record.
- Accepted proposal writes `records.jsonl` and the correct Markdown projection.

### Phase 4: Add A Memory Routing Doctor

Goal: answer "why did this memory get used or not used?"

Add a command that explains routing for a query:

```text
/memory why "query text"
```

It should show:

- pinned sources available;
- recall mode;
- candidate sources searched;
- selected records;
- skipped records and reasons;
- project scope gate decisions;
- stale/conflict decisions;
- budget cuts;
- external provider status.

Likely files:

- `src/tools/memory_tool/mod.rs`
- `src/memory/retrieval.rs`
- `src/engine/retrieval_context.rs`

Acceptance checks:

- A user can see why a topic file was selected.
- A user can see why a cross-project record was skipped.
- A user can see when memory was disabled by session control.

### Phase 5: Separate Memory Candidates From Skill Candidates

Goal: prevent procedures from becoming memory blobs.

Add deterministic routing rules:

- stable preference -> memory;
- stable project convention -> memory;
- current status -> project progress;
- repeatable workflow -> skill proposal;
- tool failure pattern -> skill or workflow learning event;
- one-off evidence -> trace/session evidence only.

Likely files:

- `src/engine/task_contract.rs`
- `src/engine/conversation_loop/closeout_controller.rs`
- `src/engine/conversation_loop/workflow_runtime.rs`
- `src/memory/reports.rs`

Acceptance checks:

- A multi-step workflow candidate becomes a skill proposal, not memory.
- A user preference remains a memory proposal.
- A failed validation command becomes evidence, not durable user memory.

### Phase 6: Keep Splitting MemoryManager

Goal: continue reducing central manager responsibility without a risky rewrite.

Next low-risk slices:

- proposal review operations module;
- retrieval orchestration module;
- Markdown projection writer module;
- maintenance/migration module;
- provider configuration module.

Acceptance checks:

- Public `/memory` behavior stays stable.
- `cargo test -q memory` remains green after each slice.
- No storage format migration happens without a migration report and rollback
  test.

## 6. Suggested First Slice

The best first implementation slice is Phase 0 plus Phase 1 and a small part of
Phase 4:

1. Make pinned memory injection consistently stable-prefix and session-frozen.
2. Add session-level `memory.use` and `memory.generate` state.
3. Make request preparation respect `memory.use`.
4. Make closeout/background proposal creation respect `memory.generate`.
5. Add doctor output that reports these controls plus pinned/dynamic
   fingerprints.
6. Add tests proving memory can be used without generating new proposals, and
   proposals can be disabled without deleting existing memory.

This gives gex immediate control without touching ranking quality or storage
format, and it makes memory cheaper by keeping the stable prefix cacheable.

## 7. Final Acceptance Criteria

This product-control work is done when:

- gex can turn memory use and memory generation on/off per session;
- pinned memory remains fixed in the stable prefix for the session;
- dynamic recall changes do not churn the stable-prefix fingerprint;
- recall mode is visible and testable;
- memory review can be done from commands without editing JSON;
- `/memory why` explains selected and skipped memory;
- workflows route to skills instead of memory;
- stable prompt remains compact;
- default write policy remains review-only;
- `cargo fmt --check`, `cargo check -q`, and `cargo test -q memory` pass.
