# Memory Product Control Plan

Date: 2026-06-02
Status: Product-control baseline implemented on 2026-06-02

This plan follows `docs/MEMORY_SYSTEM_SIMPLIFICATION_PLAN_2026-06-02.md`.
The simplification work made the memory contract understandable:

- pinned memory is compact stable prompt context;
- recall is dynamic per-turn background context;
- learning proposals are review-first candidates.

The next step is to make memory easier for gex to control in real use. The
goal is not to make memory more automatic. The goal is to make memory more
predictable, inspectable, and useful.

Implementation snapshot:

- Stable-prefix policy is now explicit in the main request path. Task focus is
  moved into the user tail in the streaming path, pinned memory is injected as
  a compact session-frozen index when memory use is enabled, retrieval context
  is merged into `<relevant_material>` in the latest user message, and runtime
  repair/correction messages are tagged as `<recent_observation>`.
- Session controls now separate memory use from memory generation:
  `/memory control use on|off`, `/memory control generate on|off`, and
  `/memory control recall off|strict|balanced|preference-only`.
- Recall modes are product-visible. `off` disables dynamic memory recall,
  `strict` applies a higher relevance threshold, `preference-only` keeps
  preference-like memory, and `balanced` remains the default.
- Cache and route diagnostics are visible through `/cache miss-report` and
  `/doctor`, including route-scoped tool schema fingerprints and inferred
  prompt-cache miss reasons.
- Proposal review remains review-first and command-driven through
  `/memory-proposals` list/show/accept/reject/edit/apply/batch/resolve flows.

Remaining non-blocking maintenance:

- expose a separate `memory.write_policy` knob if we decide to make write policy
  user-configurable beyond the current review-only behavior;
- keep reducing `MemoryManager` responsibility in small slices;
- keep refining deterministic routing between memory candidates, skill
  candidates, project progress, and trace-only evidence.

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

The original product gaps were mostly control and ergonomics. The important
baseline gaps are now closed:

- gex can toggle "use memory" and "generate memory" independently per session;
- active recall modes are exposed as a user-facing policy;
- prompt-cache behavior is visible through first-class diagnostics;
- proposal review is available from commands rather than JSON hand edits;
- memory selection can be inspected through doctor/explain/why surfaces.

The remaining long-term gap is sharper routing between skill candidates,
project progress, trace evidence, and durable memory. That should continue as
bounded follow-up work, not as another broad memory rewrite.

## 2.1 Static Prefix Risk Audit

Priority Agent already moved many dynamic context blocks toward a
Reasonix-style static-prefix shape. Most request-time context now goes near the
last user message through `prepend_to_last_user_message`, which is the right
direction for prompt caching.

The risks below were the cache-stability audit items for this implementation
slice. Their baseline status is now recorded inline.

### Risk 1: Task Focus Is Still Added To The System Prompt

Original risk:

- `PromptContextAssembler::build_for_turn()` uses the current user message and
  history to infer task type.
- `prompt_builder` then appends `Task Focus: Coding`, `Debugging`, `Review`, or
  `Architecture` to the system prompt.
- This means two turns in the same session can produce different system prompt
  fingerprints when the user's task type changes.

Implemented baseline:

- The main streaming request path uses the prompt assembly plan and keeps the
  stable prefix separate from task focus.
- Task focus is prepended to the latest user message as `<task-focus>` dynamic
  context.
- A stable-prefix test covers task-focus changes.
- Legacy/helper prompt-builder APIs still render task focus for compatibility;
  they are not the primary cache-sensitive request path.

Acceptance checks:

- [x] A coding turn followed by a review/debugging turn keeps the same stable
  system fingerprint.
- [x] The task-focus text is still available to the model through dynamic
  context.

### Risk 2: Pinned Memory Snapshot Is Not Always Stable Across Turns

Original risk:

- Pinned memory is compact and session-frozen, which is cache-friendly.
- But `MemorySnapshotController` skips pinned snapshot injection when dynamic
  memory recall exists.
- That avoids duplicate memory context, but it also means some turns have the
  pinned memory index in the stable prefix and some do not.

Implemented baseline:

- `MemorySnapshotController` now injects the compact pinned snapshot whenever
  memory is enabled and the retrieval policy allows memory context.
- Dynamic recall no longer suppresses pinned snapshot injection.
- `memory.use=off` gates both pinned snapshot use and dynamic recall.
- Mid-session writes do not refresh the frozen snapshot unless the session or
  explicit refresh path changes it.

Acceptance checks:

- [x] Dynamic memory recall changing between turns does not change the stable
  prefix fingerprint.
- [x] Mid-session memory writes do not mutate the pinned memory snapshot.
- [x] Doctor reports memory snapshot state and prompt-cache diagnostics; dynamic
  recall remains traceable through memory doctor/explain output.

### Risk 3: RetrievalPromptController Still Inserts Dynamic System Messages

Original risk:

- `RetrievalPromptController` wraps retrieval in `<relevant_material>`, but
  still inserts it as `Message::system(...)` before the user message.
- Cache diagnostics can classify this as dynamic context, but the message shape
  is inconsistent with the newer user-tail strategy.

Implemented baseline:

- `RetrievalPromptController` now routes retrieval blocks through
  `prepend_to_last_user_message`.
- Retrieval remains fenced as `<relevant_material>`.
- The controller test asserts that no extra system message is created.

Acceptance checks:

- [x] Retrieval context appears before the latest user content, inside the user
  message tail.
- [x] No new system message is created for retrieval context.

### Risk 4: Runtime Corrections And Guards Are Bare System Messages

Original risk:

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

Implemented baseline:

- Runtime corrections, retry hints, post-edit verification prompts, workflow
  guard messages, focused repair hints, and recovery notes are tagged as
  `<recent_observation>` dynamic context.
- `RequestPreparationController` preserves known dynamic context separately
  from stable system material and moves context zones into the user tail.
- Remaining dynamic system insertions are recognized dynamic fences rather than
  bare stable-policy text.

Acceptance checks:

- [x] Tool failure correction, destructive-scope guard, retry correction, and
  post-edit verification hints no longer appear as untagged system messages.
- [x] Cache diagnostics list zero unexpected dynamic system messages after a tool
  failure turn.

### Risk 5: Route-Scoped Tool Schemas Can Still Change The Cached Prefix

Original risk:

- Tool schema is part of the cached prefix shape.
- The project already canonicalizes tool ordering, which is good.
- But route-specific tool exposure can still change the tool list and schema
  fingerprint across turns.

Implemented baseline:

- Route-scoped tools remain allowed because they keep the default surface
  smaller and avoid broad schema noise.
- Tool schema changes are reported as first-class cache-miss reasons through
  prompt-cache diagnostics.
- `/doctor` now reports a `route_tool_schema_cache` matrix with per-route tool
  counts and schema fingerprints.
- Core memory controls do not add broad external provider tools by default.

Acceptance checks:

- [x] Cache diagnostics distinguish `system`, `tools`, `dynamic_tail`, and
  `message_count` causes.
- [x] Route changes that alter tool schemas are visible in doctor/trace output.
- [x] Core memory controls do not add broad external provider tools by default.

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

Status: implemented for the main request path.

Current implementation is cache-friendly in the product-control baseline:

- pinned memory snapshots are compact index-style context;
- snapshots are frozen for a session;
- dynamic recall is prepended near the last user message rather than changing
  the earliest stable prompt;
- cache diagnostics already track stable-prefix fingerprints.

Implemented policy:

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

- [x] With `memory.use=on`, the same frozen pinned memory index is present in the
  stable prefix across turns in the same session.
- [x] Dynamic recall changes do not change the stable-prefix fingerprint.
- [x] Task type changes do not change the stable-prefix fingerprint in the main
  streaming request path.
- [x] Retrieval context does not create a separate dynamic system message.
- [x] Runtime repair/guard feedback is tagged as dynamic context rather than bare
  system prompt material.
- [x] A mid-session memory write does not mutate the pinned prefix until a refresh
  or new session.
- [x] When `memory.use=off`, neither pinned memory nor dynamic recall is injected.
- [x] Doctor reports memory/cache state and cache-miss reasons; dynamic recall
  remains traceable through memory doctor/explain output.

### Phase 1: Add Session Memory Controls

Goal: make memory use and memory generation explicit.

Status: implemented for `use`, `generate`, and `recall`; `write_policy` remains
non-blocking follow-up because default proposal writing is already review-only.

Implemented controls:

- `memory.use`: whether existing memory can be used in this session;
- `memory.generate`: whether this session may create future memory proposals;
- `memory.recall`: whether and how pre-response recall runs automatically.

Command surface:

```text
/memory control
/memory control use on|off
/memory control generate on|off
/memory control recall off|strict|balanced|preference-only
```

Likely files:

- `src/services/config.rs`
- `src/tools/memory_tool/mod.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/conversation_loop/memory_sync_controller.rs`

Acceptance checks:

- [x] Turning memory use off prevents pinned snapshot and dynamic recall injection.
- [x] Turning generation off still allows using existing memory but suppresses new
  memory proposals from this session.
- [x] Default remains use-on, generate-on, write-review-only.

### Phase 2: Productize Recall Modes

Goal: make recall behavior predictable.

Status: implemented as a first usable policy layer.

Implemented recall modes:

- `off`: no dynamic memory recall;
- `strict`: only high-confidence, directly relevant memory;
- `balanced`: current default behavior;
- `preference-only`: only user preferences and durable collaboration style;

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

- [x] `strict` applies a higher relevance threshold than `balanced`.
- [x] `preference-only` filters to preference/user-style memory.
- [x] `off` produces no dynamic recall but still permits explicit memory
  commands.
- [x] Retrieval traces remain available for inspection.

### Phase 3: Improve Proposal Review UX

Goal: make review-required memory useful instead of hidden.

Status: already available as the `/memory-proposals` review flow.

Command surface:

```text
/memory-proposals list
/memory-proposals show <id>
/memory-proposals accept <id>
/memory-proposals reject <id>
/memory-proposals edit <id> <content>
/memory-proposals apply <id>
/memory-proposals batch-accept [filters]
/memory-proposals batch-reject [filters]
/memory-proposals resolve-conflict <keep-id>
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

- [x] A closeout/background proposal can be accepted without hand-editing JSON.
- [x] A rejected proposal is recorded and can be filtered out of review.
- [x] A conflict can be resolved by keeping one proposal and rejecting the
  conflicting group.
- [x] Accepted/applied proposals write durable records and Markdown projections.

### Phase 4: Add A Memory Routing Doctor

Goal: answer "why did this memory get used or not used?"

Status: implemented baseline via `/memory why`, `memory_load action=why`,
memory doctor/explain output, `/cache miss-report`, and `/doctor` cache-route
diagnostics.

Command surface:

```text
/memory why <query> [--item <retrieval-id-or-source>]
/cache miss-report
/doctor
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

- [x] A user can inspect selected memory through why/explain output.
- [x] A user can inspect skipped/disabled memory through doctor/explain output.
- [x] A user can see when prompt-cache misses are caused by tool/schema shape
  changes.
- [x] A user can see when memory was disabled by session control.

### Phase 5: Separate Memory Candidates From Skill Candidates

Goal: prevent procedures from becoming memory blobs.

Status: baseline rule documented; deeper routing remains ongoing maintenance.

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

- [x] A user preference remains a memory proposal.
- [x] A failed validation command is treated as evidence/trace, not durable user
  memory by default.
- [ ] Continue tightening multi-step workflow routing into skill proposals.

### Phase 6: Keep Splitting MemoryManager

Goal: continue reducing central manager responsibility without a risky rewrite.

Status: ongoing maintenance, not a blocker for the product-control baseline.

Next low-risk slices:

- proposal review operations module;
- retrieval orchestration module;
- Markdown projection writer module;
- maintenance/migration module;
- provider configuration module.

Acceptance checks:

- [x] Public `/memory` behavior stays stable in this slice.
- [ ] `cargo test -q memory` remains the gate after each future slice.
- [x] No storage format migration happens without a migration report and rollback
  test.

## 6. Implemented Slice

The implemented product-control slice covered Phase 0 plus Phase 1, Phase 2
baseline, and the diagnostic part of Phase 4:

1. Make pinned memory injection consistently stable-prefix and session-frozen.
2. Add session-level `memory.use` and `memory.generate` state.
3. Make request preparation respect `memory.use`.
4. Make closeout/background proposal creation respect `memory.generate`.
5. Add doctor/cache output that reports route tool-schema fingerprints and
   prompt-cache miss reasons.
6. Add tests proving memory can be used without generating new proposals, and
   proposals can be disabled without deleting existing memory.

This gives gex immediate control without touching ranking quality or storage
format, and it makes memory cheaper by keeping the stable prefix cacheable.

## 7. Final Acceptance Criteria

The product-control baseline is done when:

- [x] gex can turn memory use and memory generation on/off per session;
- [x] pinned memory remains fixed in the stable prefix for the session;
- [x] dynamic recall changes do not churn the stable-prefix fingerprint;
- [x] recall mode is visible and testable;
- [x] memory review can be done from commands without editing JSON;
- [x] `/memory why` explains selected and skipped memory;
- [x] stable prompt remains compact;
- [x] default write policy remains review-only;
- [x] `cargo fmt`, `cargo check -q`, and targeted memory/cache tests pass.

Non-blocking follow-up:

- [ ] expose `memory.write_policy` only if gex wants a user-visible write-policy
  switch beyond review-only defaults;
- [ ] continue routing multi-step workflow candidates to skill proposals;
- [ ] continue splitting `MemoryManager` behind stable `/memory` behavior.
