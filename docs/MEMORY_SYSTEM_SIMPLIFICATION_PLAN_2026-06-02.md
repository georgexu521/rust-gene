# Memory System Simplification Plan

Date: 2026-06-02
Status: Draft plan after Hermes/Reasonix comparison

Implementation update:

- 2026-06-02: First code slice implemented. Memory doctor now exposes the
  pinned memory / recall / learning proposals contract, runtime budget labels
  stable prompt memory as `pinned_memory`, and `MemorySnapshotController` skips
  pinned snapshot injection when the current turn already has dynamic memory
  recall.
- 2026-06-02: Phase 2 compact pinned snapshot slice implemented. Stable prompt
  memory now uses index-style entries for `MEMORY.md` and `USER.md` plus the
  topic memory manifest. Full Markdown bodies stay available to search and
  dynamic recall instead of being injected into the stable prompt snapshot.
- 2026-06-02: Phase 3 boundary slice started. Project progress records selected
  by the retrieval pipeline now surface as `Project` retrieval context rather
  than durable `Memory`, while session-search results remain `Session` context.
  Tests cover that project progress recall does not create or write `USER.md`.

This plan narrows Priority Agent's memory system without discarding the useful
parts already built. The goal is not to make memory less capable. The goal is
to make the product contract simple enough that the model, runtime, and user all
agree on what memory is allowed to mean.

Reference sources:

- Priority Agent memory code under `src/memory/` and retrieval injection under
  `src/engine/conversation_loop/`.
- Existing project plans:
  `docs/HERMES_MEMORY_FEATURE_FOLLOWUP_PLAN_2026-05-27.md`,
  `docs/HERMES_MEMORY_SELF_EVOLUTION_REVIEW.md`, and
  `docs/RUNTIME_SIMPLIFICATION_PLAN_2026-06-02.md`.
- Reasonix local source:
  `/Users/georgexu/Downloads/DeepSeek-Reasonix-main`.
- Hermes local source:
  `/Users/georgexu/Desktop/hermes-agent-main`.

## 1. Current Assessment

Priority Agent already has a serious memory foundation:

- typed memory records, scope identity, trust boundary, evidence, status,
  sensitivity, quality, and proposal metadata;
- a Hermes-style `MemoryProvider` lifecycle with local plus optional external
  provider registration;
- frozen local snapshots, FTS search, retrieval ranking, retrieval budgets, and
  `MemoryRetrievalTrace`;
- project progress records separated from user profile memory;
- review-required proposal queues and explicit accept/reject/apply paths;
- memory doctor, snapshot, records, proposal, search, and eval surfaces.

The issue is product shape, not raw capability. The memory stack now covers too
many responsibilities at once:

- durable user/project memory;
- past conversation recall;
- active project progress;
- typed records and Markdown projections;
- retrieval context assembly;
- background learning candidates;
- provider lifecycle orchestration;
- active-memory prototype context;
- memory-related self-evolution evidence.

This makes the system harder to reason about than Reasonix, and less cleanly
productized than Hermes.

## 2. Reference Lessons

### 2.1 Reasonix

Reasonix keeps memory simple and cache-friendly:

- project memory is loaded from a small ordered set of project files;
- user memory lives under a private memory directory with one deterministic
  index;
- the stable prompt gets compact memory/index material, not an unbounded recall
  dump;
- memory bodies remain explicit files, and the index is what enters the prompt
  by default.

Priority Agent should copy this default feel: small, predictable, and easy to
inspect.

### 2.2 Hermes

Hermes is stronger on lifecycle and product boundaries:

- built-in memory is a frozen prompt snapshot;
- prior transcript recall is handled by session search, not long-term memory;
- providers have clear lifecycle hooks and failure isolation;
- plugins can provide semantic memory, but only through a bounded provider
  surface;
- local memory writes are locked, bounded, scanned, and atomically persisted.

Priority Agent should keep this architecture direction, especially provider
lifecycle, session-search separation, profile/project scoping, and safety gates.

## 3. Target Product Contract

Priority Agent memory should have three visible layers.

### 3.1 Pinned Memory

Pinned memory is the small, stable material allowed into the stable prompt
prefix:

- user preferences and durable collaboration style;
- project conventions and long-lived local environment facts;
- compact indexes that point to larger memory bodies;
- manually curated or explicitly accepted records only.

Pinned memory must stay small and deterministic. It should behave more like
Reasonix's memory index than a broad semantic retrieval dump.

### 3.2 Recall

Recall is dynamic, per-turn retrieved context:

- selected memory records;
- project progress relevant to the current task;
- session history search results;
- project/file retrieval results;
- optional external provider hits.

Recall should be injected as an untrusted context zone with provenance, score,
freshness, and "why recalled" trace. It should not become a hidden instruction
surface.

### 3.3 Learning Proposals

Learning proposals are candidate durable memories:

- generated by closeout, background review, explicit user commands, or provider
  callbacks;
- validated by deterministic runtime gates;
- defaulted to `review_required`;
- persisted only after explicit acceptance and successful gate checks.

The LLM may propose what is worth remembering. The runtime decides whether the
candidate is safe, scoped, evidenced, deduplicated, and allowed to persist.

## 4. Hard Boundaries

The simplification work must preserve these rules:

- Do not silently auto-write durable memory unless an explicit project policy
  enables a narrow path.
- Do not persist secrets, unsafe instructions, prompt-injection patterns, or
  broad local/private data without minimization and review.
- Do not let project memory leak across project identities, monorepo subpaths,
  users, agents, or profiles.
- Do not let memory content override runtime policy, sandbox policy,
  permissions, checkpoint behavior, validation gates, or tool contracts.
- Do not use memory to hide failed validation or manufacture verified closeout.
- Do not weaken existing review, evidence, sensitivity, stale/conflict, or
  duplicate gates.

## 5. Boundary Cleanup

The main conceptual cleanup is to route information into the right surface.

| Information | Target surface | Notes |
| --- | --- | --- |
| Stable user preference | Memory | Durable, scoped to user/profile, reviewable if inferred |
| Stable project convention | Memory | Durable, scoped to project identity |
| Local machine fact | Memory | Minimized, private, reviewed unless explicit |
| Current task status | Project progress | Not user profile memory |
| Past conversation details | Session search | Searchable transcript recall, not durable memory |
| Reusable workflow | Skill | Procedural memory, reviewed and testable |
| Temporary evidence | Retrieval context | Per-turn only |
| Failed validation output | Tool/session evidence | Can feed repair and proposals, not direct memory |
| Prompt/routing improvement | Improvement proposal | Must bind eval before apply |

## 6. Implementation Phases

### Phase 1: Declare The Contract In Code And UI

Goal: make the memory product model visible before changing storage internals.

Work:

- Add a short developer-facing contract near the memory entrypoints explaining
  pinned memory, recall, and learning proposals.
- Update `/memory` and `/memory-proposals` help text to use those terms.
- Make doctor output clearly separate:
  - pinned snapshot state;
  - recall/search state;
  - proposal/review state;
  - provider lifecycle state.
- Ensure project progress commands do not describe progress as long-term user
  memory.

Likely files:

- `src/tools/memory_tool/`
- `src/tui/slash_handler/learning.rs`
- `src/memory/manager.rs`
- `src/memory/reports.rs`

Validation:

```bash
cargo fmt --check
cargo check -q
cargo test -q memory
```

### Phase 2: Simplify Prompt Injection

Goal: avoid injecting both broad frozen memory and selected recall as competing
memory context.

Work:

- Decide one stable prefix surface for pinned memory.
- Keep dynamic recall in the per-turn retrieval context zone.
- Change `MemorySnapshotController` so it injects only pinned/index-level
  memory, not broad recall material.
- Ensure turn-level retrieval can still select relevant memory records with
  provenance and budget.
- Trace prompt memory as two separate counters:
  - pinned memory chars;
  - recalled memory chars.

Likely files:

- `src/engine/conversation_loop/memory_snapshot_controller.rs`
- `src/engine/conversation_loop/turn_retrieval_context_controller.rs`
- `src/engine/retrieval_context.rs`
- `src/memory/provider.rs`
- `src/memory/retrieval.rs`

Acceptance checks:

- A memory accepted during a session is persisted, but does not silently mutate
  the stable prompt snapshot mid-session.
- A relevant accepted memory can still appear in dynamic recall with a trace
  decision.
- No prompt contains two large `<memory-context>` blocks for the same turn.

Validation:

```bash
cargo fmt --check
cargo check -q
cargo test -q memory_snapshot
cargo test -q retrieval_context
cargo test -q memory
```

### Phase 3: Separate Session Search From Durable Memory

Goal: make Hermes' distinction explicit: past transcript recall is not the same
thing as long-term memory.

Work:

- Audit current memory retrieval candidates and mark which ones are true durable
  memory, project progress, or session recall.
- Route prior conversation lookups through session search/retrieval source
  `Session`.
- Keep project progress as `Project` or a distinct progress lane, not user
  memory.
- Add tests proving task progress and prior transcript details do not pollute
  user/profile memory.

Likely files:

- `src/engine/retrieval_context.rs`
- `src/engine/conversation_loop/turn_retrieval_context_controller.rs`
- `src/memory/retrieval.rs`
- `src/session_store/`
- `src/memory/project_progress.rs`

Acceptance checks:

- "What did we do last time?" can recall session/project progress without
  writing a user memory.
- "I prefer concise Chinese replies" can become a reviewed durable user memory.
- Completed task details do not appear in `USER.md` or durable user records
  unless explicitly accepted as a preference/convention.

### Phase 4: Make Local Memory Feel Reasonix-Simple

Goal: keep canonical typed records, but make the default local memory surface
small and inspectable.

Work:

- Treat human-readable Markdown as pinned projections/indexes.
- Keep `records.jsonl` as canonical typed durable memory.
- Limit stable prompt memory to compact user/project summaries or indexes.
- Ensure topic/body files are recall candidates, not automatic prompt-pinned
  bodies.
- Add a deterministic "memory index" view that is stable enough for prompt
  caching and easy for the user to read.

Likely files:

- `src/memory/provider.rs`
- `src/memory/manager.rs`
- `src/memory/reports.rs`
- `src/memory/search_index.rs`

Acceptance checks:

- The prompt-pinned memory size is bounded and predictable.
- Large memory bodies are discoverable through recall/search.
- Doctor can show exactly which pinned projections entered the prompt.

### Phase 5: Productize Provider Modes

Goal: keep the Hermes provider architecture, but expose a simple contract.

Work:

- Support at most one external provider.
- Make external provider modes explicit:
  - `off`;
  - `context`;
  - `tools`;
  - `hybrid`.
- Default local memory remains available without external provider setup.
- Provider failures remain non-fatal and visible in doctor/trace.
- External writes stay disabled unless an explicit reviewed mirror policy is
  added later.

Likely files:

- `src/memory/provider.rs`
- `src/services/config.rs`
- `src/memory/reports.rs`

Acceptance checks:

- Misconfigured external provider does not block local memory.
- Doctor reports provider mode, capabilities, failures, and last successful
  lifecycle hook.
- External provider configuration does not add broad tool schema noise by
  default.

### Phase 6: Continue Splitting `MemoryManager`

Goal: reduce the central manager without changing behavior in one large rewrite.

Suggested boundaries:

- local store/provider ownership;
- proposal review and gates;
- retrieval orchestration;
- project progress integration;
- flush/sync queue;
- diagnostics/reports;
- maintenance/migration.

This should proceed as small gateable slices. Each slice should preserve public
slash/tool behavior and keep memory tests green.

Validation lane:

```bash
cargo fmt --check
cargo check -q
cargo test -q memory
bash scripts/test-fast-lane.sh
```

### Phase 7: Strengthen Retrieval Without Making Defaults Heavy

Goal: improve recall quality while keeping Reasonix-like default simplicity.

Work:

- Keep local retrieval deterministic by default:
  - FTS candidates;
  - lexical/Jaccard matching;
  - scope match;
  - trust/importance/status;
  - recency and stale/conflict penalty;
  - user-pinned bonus.
- Keep LLM rerank opt-in or reserved for high-recall policies.
- Make "why recalled" visible through existing `MemoryRetrievalTrace`.
- Add eval fixtures for paraphrase recall, project-scope isolation, and
  stale-conflict suppression.

Likely files:

- `src/memory/ranking.rs`
- `src/memory/retrieval.rs`
- `src/engine/retrieval_context.rs`

## 7. Suggested First Code Slice

The safest first implementation slice is Phase 2 plus a small part of Phase 1:

1. Add explicit terminology in memory reports/help: pinned memory, recall,
   proposals.
2. Split prompt accounting into pinned memory vs recalled memory.
3. Adjust `MemorySnapshotController` so stable injection is pinned/index-only.
4. Ensure dynamic memory retrieval remains available and trace-backed.
5. Add tests proving no duplicate broad memory context injection.

This slice attacks the biggest product ambiguity while touching a narrow set of
files.

## 8. Open Decisions

These should be decided before broad implementation:

1. Should local durable memory be initialized by default for all interactive
   sessions, or only when memory/retrieval policy asks for it?
2. Should project progress appear under retrieval source `Project`, `Memory`, or
   a new explicit `Progress` source?
3. Should `MEMORY.md`/`USER.md` remain prompt projections, or should the stable
   prompt use only a generated deterministic index?
4. Which external provider mode should be supported first: read-only context or
   explicit tools?
5. Should active memory remain an opt-in prototype until the pinned/recall split
   is stable?

## 9. Final Acceptance Criteria

The memory system is considered simplified when:

- a user can explain the system as pinned memory, recall, and proposals;
- stable prompt memory is compact, bounded, and inspectable;
- dynamic recall has provenance and "why recalled" trace;
- prior conversation search does not pollute durable memory;
- project progress remains separate from user/profile memory;
- automatic learning remains proposal-first and review-required by default;
- provider failures are visible but non-fatal;
- `MemoryManager` is no longer the home for every memory responsibility;
- targeted memory tests cover prompt injection, secrets, scope isolation,
  duplicate recall, stale conflicts, proposal gates, and provider failure.
