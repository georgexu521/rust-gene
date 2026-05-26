# Agent Memory System Alignment Plan

Date: 2026-05-25

Source note: `/Users/georgexu/Downloads/09-agent_memory_system_notes.md`

Repo scope: `/Users/georgexu/Desktop/rust-agent`, current branch `claude`.

Related prior design:

- `docs/HERMES_MEMORY_SELF_EVOLUTION_REVIEW.md`
- `docs/MEMORY_CONTROLLED_SELF_EVOLUTION_DESIGN.md`
- `docs/AGENT_OBSERVER_ALIGNMENT_PLAN_2026-05-24.md`
- `docs/AGENT_STOP_FAILURE_RECOVERY_ALIGNMENT_PLAN_2026-05-24.md`

## 0. Conclusion

Priority Agent already has a real memory system. It is not just appending chat
history into the prompt. The current runtime has:

- frozen memory snapshots;
- user/project/topic/agent memory namespaces;
- quality and safety gates for memory writes;
- background and session-end memory extraction;
- retrieval context with provenance, conflict lowering, and LLM rerank fallback;
- session-store learning events and typed `ExperienceRecord` payloads;
- memory doctor/review/search/explain tooling;
- live-eval reporting for memory activity, recall, conflict, and changed-plan
  signals.

The project is therefore ahead of the note's simplest `memory.json` version.
The main remaining problem is different: the memory system has several strong
pieces, but they are not yet one coherent, typed, evidence-backed lifecycle.

The note's target model is:

```text
Context = current prompt workspace
State   = current task record
Memory  = long-term cross-task experience
```

The repo mostly respects that separation now, especially after the Observer and
Stop/Recovery work. But durable memory still loses important metadata, write
paths are fragmented, and failure lessons do not yet reliably become structured
strategy/failure memories that influence later action weights.

The next stage should keep the existing Markdown memory UX, but add a typed
record/index layer and route every write through one memory candidate pipeline.

## 0.1 Implementation Status

2026-05-26 OpenClaw/Hermes follow-up batch started the provider-boundary
cleanup:

- Added a `MemoryProviderRegistry` with built-in local provider ownership,
  optional single external provider registration, provider-name reporting, and
  lifecycle fanout for initialize, system prompt blocks, prefetch,
  queue-prefetch, turn sync, session end, pre-compress, write notifications, and
  shutdown.
- Added structured provider call outcomes so unavailable providers are skipped
  and failing providers report errors without stopping unrelated providers.
- `MemoryManager` now owns the provider registry and exposes provider lifecycle
  wrapper methods. Local Markdown/record storage still stays in `MemoryManager`
  for this first batch; moving it behind `LocalMemoryProvider` remains the next
  provider-boundary step.
- Turn retrieval context is now injected as an explicit `<relevant_material>`
  zone before the current user request, and dynamic zone system messages are
  excluded from the stable-prefix context trace.
- `MemoryManager` now carries an active `MemoryScope`, and manager-owned
  candidate creation uses that scope instead of defaulting to
  `unbound-session`. Conversation turn bootstrap sets the active scope from the
  current CLI session id and working directory before retrieval and later
  memory operations run.

2026-05-25 implementation batch completed the planned memory-system alignment
slice across all eight phases:

- Added typed `MemoryCandidate`, `MemoryEvidenceRef`, `MemoryStrategyMetadata`,
  enriched `MemoryRecord`, and durable `memory/records.jsonl` sidecar support.
- Routed manual `memory_save`, normal learning writes, topic writes, background
  LLM extraction, trailing extraction, and stop/recovery candidate memories
  through `MemoryManager::submit_candidate`.
- Kept Markdown memory files as readable accepted-record projections with
  `memory-id` comments, and added best-effort legacy Markdown import without
  rewriting user-edited files.
- Enforced kind-specific evidence policy: project/tool facts and
  failure/success strategy memories need non-inferred evidence to become
  accepted; unsupported candidates remain proposed.
- Changed LLM reflection prompts/parsing from free-form bullets to structured
  JSON memory candidates with type, evidence, confidence, importance, tags, and
  strategy fields.
- Added typed-record retrieval first, usage telemetry (`use_count`,
  `last_used_at`), stale project-fact demotion, projection drift reporting, and
  verified-record supersede handling.
- Promoted stop/recovery `ExperienceRecord.candidate_memories` into durable
  `strategy-failures` memory records with runtime evidence and trace linkage.
- Added bounded memory modifiers to `ActionDecision` so failure memories raise
  risk and successful diagnostic memories raise value without overriding
  permissions/checkpoints.
- Extended live-eval parser, aggregate reporting, and run reports with memory
  contract fields, and added live task samples for failure-lesson promotion and
  stale project-fact demotion.

Validated:

```bash
cargo test -q memory -- --test-threads=1
cargo test -q memory_tool -- --test-threads=1
cargo test -q retrieval_context -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q turn_recording -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q experience_ledger -- --test-threads=1
cargo fmt --check
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh
ruby -ryaml -e 'ARGV.each { |p| YAML.load_file(p); puts p }' evalsets/live_tasks/memory-failure-lesson-promotion.yaml evalsets/live_tasks/memory-stale-project-fact-demotion.yaml
cargo check -q
cargo test -q
cargo clippy --all-features -- -D warnings
```

## 1. Current Alignment

| Note requirement | Current project state | Alignment | Main gap |
| --- | --- | --- | --- |
| Context, State, Memory must be separate | `RetrievalContext`, `<task-state>`, `MemoryManager`, context ledger, and learning events are separate runtime surfaces. | Strong partial | Retrieval is now zoned as `<relevant_material>`; remaining work is to fully render every live request from the zone plan instead of mixed controller-level system messages. |
| Memory is not all history in prompt | Frozen snapshot, relevant memory prefetch, top-k retrieval, and retrieval policy gating exist. | Strong | Good foundation; tune ranking and metadata rather than replace it. |
| Store user preference, project fact, strategy, failure lesson, stable constraint | `MemoryKind` covers preferences, project facts, conventions, tool quirks, failure patterns, successful fixes, decisions, skill candidates, and notes. | Partial | Markdown output does not preserve typed kind/evidence/importance/verification fields consistently. |
| Do not store logs, stale facts, secrets, unverified guesses | `scan_memory_content`, quality scoring, duplicate gates, prompt-injection fences, and memory calibration tests exist. | Strong partial | Evidence and verified-vs-inferred state are not first-class in stored records. Staleness is mostly file-level maintenance, not per-memory lifecycle. |
| Memory layers: user, project, task, strategy | `USER.md`, `MEMORY.md`, topic files, pending session learnings, agent role JSON, and session learning events exist. | Partial | Strategy memory is implicit in categories and learning events, not a first-class durable layer with success/failure counts. |
| Lifecycle: generate, verify, store, retrieve, use, update, decay/delete | Generation, store, retrieve, use, flush, archive, and conflict hints exist. | Partial | Verification, use telemetry, update/supersede, last_verified, and per-record decay are incomplete. |
| Memory should carry evidence, confidence, last_verified | `MemoryRecord` has confidence/provenance/status/tags; `LearningEventRecord` has confidence/payload. | Partial | `MemoryRecord` is not the canonical local store, and it lacks evidence, importance, last_verified, last_used, and use_count. |
| Retrieval can be simple: keyword/tag/project/importance/recency top 3-5 | Keyword search, semantic aliases, topic files, LLM rerank, conflicts, and top-k retrieval exist. | Partial | Retrieval does not yet use structured tags, project id, importance, last_used/use_count, or verified freshness. |
| Write only on important events | Memory sync is throttled; manual `memory_save`, flush, background extraction, and session-end extraction exist. | Partial | Write surfaces do not share one typed candidate contract, and task-end reflection is still bullet-oriented. |
| LLM may propose candidates, system filters | Quality/safety gate exists and background LLM extraction is gated. | Partial | LLM output is free-form bullets, not structured memory candidates with type/evidence/confidence/importance/tags. |
| Memory should affect action weights | Learning-aware routing/planning and live-eval `memory_changed_plan` exist. | Partial | Action decision/review does not consume strategy/failure memory as explicit value/risk modifiers. |
| Memory should support self-evolution | Learning events, `ExperienceRecord`, improvements, skill proposals, skill fitness, and evolution controller exist. | Partial | Candidate memories from failed turns are recorded in experience payloads but not promoted through the long-term memory pipeline. |

## 2. Evidence From Current Code

- `src/memory/manager.rs` owns the local lifecycle: `MEMORY.md`, `USER.md`,
  topic memory files, frozen snapshots, prefetch, LLM rerank, turn sync,
  session flush, memory decision logs, flush logs, and maintenance.
- `src/memory/types.rs` defines `MemoryScope`, `MemoryKind`, `MemoryStatus`,
  `SensitivityLevel`, `MemoryProvenance`, and `MemoryRecord`.
- `src/memory/quality.rs`, `src/memory/scoring.rs`, `src/memory/safety.rs`,
  and `src/memory/recall.rs` already implement write, keep, safety, and recall
  scoring contracts.
- `src/memory/provider.rs` defines a provider lifecycle trait and
  `MemoryProviderRegistry` for local plus optional single external provider
  fanout. The current `LocalMemoryProvider` is still an adapter marker;
  `MemoryManager` owns the registry and still owns local storage directly.
- `src/engine/retrieval_context.rs` turns memory matches into provenance-bearing
  retrieval items and lowers confidence for conflicts.
- `src/engine/conversation_loop/memory_snapshot_controller.rs` injects frozen
  memory as fenced background system context.
- `src/engine/conversation_loop/turn_retrieval_context_controller.rs` builds
  memory retrieval context, records trace events, and merges memory with
  project/session retrieval.
- `src/engine/conversation_loop/request_preparation_controller.rs` records
  context-zone materialization and keeps dynamic zone system messages out of
  the stable-prefix trace. Its memory prefetch fallback now injects fenced
  `<relevant_material>` when a turn retrieval context was not already built.
- `src/engine/conversation_loop/memory_sync_controller.rs` performs heuristic or
  LLM memory sync after the turn and records `MemorySynced`.
- `src/session_store/mod.rs` persists `learning_events` with kind, source,
  summary, confidence, payload, and created_at.
- `src/engine/experience_ledger.rs` stores typed `ExperienceRecord` payloads and
  can attach `candidate_memories` for failed strategies after stop checks.
- `src/tools/memory_tool/mod.rs` provides `memory_save`, `memory_load`,
  `memory_clear`, doctor/review/search/conflict/explain surfaces, but it
  duplicates parts of local file write logic instead of calling
  `MemoryManager`.
- `src/agent/memory.rs` has a separate agent role memory store under
  `~/.priority-agent/memory/agents/*.json`, using key/value/tags but not the
  main `MemoryRecord` contract.
- `scripts/live_eval_report_parser.py` already reports memory-active,
  recalled-item, conflict, changed-plan, and behavior assertion signals.

## 3. Remaining Problems

### P0. Typed memory records are not the canonical local store

`MemoryRecord` exists, but the active local store is still Markdown sections and
topic Markdown files:

```text
## [CATEGORY] timestamp
content
```

That keeps memory human-readable, which is good, but it loses the note's core
metadata:

- stable `id`;
- exact `type`;
- `importance`;
- `evidence`;
- `last_verified`;
- `last_used`;
- `use_count`;
- supersedes/superseded-by relationships;
- confidence history.

Risk:

- Retrieval can only infer kind/trust from text shape and file path.
- Stale facts cannot be revalidated or demoted precisely.
- `/memory explain` can show why text matched, but not why that memory is still
  trusted.
- Failure lessons cannot accumulate success/failure counts cleanly.

### P0. Memory write paths are fragmented

There are multiple write paths:

- `MemoryManager::add_learning*`;
- `MemoryManager::sync_turn*`;
- background LLM bullet writes;
- session flush/trailing extraction;
- `memory_save` tool;
- agent role memory writes;
- `ExperienceRecord.candidate_memories`;
- session-store `learning_events`.

Several paths share gates, but not one typed `MemoryCandidate -> decision ->
record -> render/index` flow. `memory_save` also duplicates file write logic and
does not consistently emit the same decision journal shape as `MemoryManager`.

Risk:

- Manual saves, background saves, failure lessons, and agent memories can drift
  in behavior.
- New metadata fields would need to be patched into several places.
- Candidate memories produced by stop/failure recovery can remain trapped in
  learning-event payloads instead of becoming durable strategy memory.

### P0. Evidence and verification are not hard requirements for factual memory

The quality gate scores stability, future utility, specificity, relevance,
volatility, sensitivity risk, and duplication. That is useful, but it does not
require a factual candidate to carry evidence.

Risk:

- An LLM-extracted bullet can become a fact if its text looks useful enough.
- Project facts and environment facts can outlive the evidence that justified
  them.
- The system cannot distinguish "verified from package.json" from "summarized
  from assistant prose" at retrieval time.

### P1. Project and agent scoping is still implicit

`MemoryScope` contains project root, session id, parent session id, agent
context, profile, user id, and platform. But the default CLI bootstrap creates
`MemoryManager::new()`, which points to the global `~/.priority-agent` root.
Project memory and user memory are namespaces, but the project identity is not
yet a durable routing key in the local records.

Agent role memory is another separate JSON system. It is visible to
`memory_load`, but it is not governed by the same scope/safety/verification
contract.

Risk:

- Memories from one repo can influence another repo.
- Subagent or eval/test context can pollute user/project memory unless every
  caller is careful.
- Project-specific failure lessons cannot be reliably filtered by project id.

### P1. Strategy and failure memory are not first-class enough

The note's key self-evolution point is strategy memory:

```text
strategy -> success_count / failure_count -> action weight changes
```

Current code has `FailurePattern`, `SuccessfulFix`, learning events,
improvement proposals, skill proposals, and candidate memories from stop
checks. But the long-term memory layer does not have a structured
`StrategyMemory` with:

- contexts/tags;
- success_count/failure_count;
- risk/value modifiers;
- evidence links;
- originating experience ids;
- last verified outcome.

Risk:

- The agent can record that something failed, but later action selection does
  not have a stable numerical signal to prefer the better strategy.

### P1. Retrieval does not update usage telemetry

`RetrievalContext` has provenance and scores, and `MemoryRecord` has confidence,
utility, and status. But memory retrieval does not update per-memory `last_used`
or `use_count` because the retrieved units are snippets, not canonical records.

Risk:

- Maintenance cannot tell which memories are actually useful.
- Recall scoring's `prior_usefulness` is still a proxy.
- Unused or harmful memories do not lose rank based on real usage.

### P1. Memory context injection should have one background path

The normal retrieval-context path is fenced as system/background context. The
request-preparation fallback currently appends formatted retrieval context onto
the user message.

Risk:

- The XML fence helps, but the message role still makes the memory look closer
  to user input than it should.
- It blurs the note's Context/State/Memory boundary and makes prompt-debugging
  harder.

### P1. Lifecycle maintenance is file-level, not record-level

`maintain_memory` removes duplicate sections and archives large topic files.
`MemoryKeepScore` exists. But there is no canonical per-record state machine for:

- active;
- proposed;
- stale;
- needs revalidation;
- superseded;
- archived;
- blocked.

Risk:

- Old project facts can remain active after code changes.
- Conflicts can be detected, but resolution is not tied to a specific record
  update.

### P2. Memory-to-action weighting is still shallow

Learning-aware routing/planning already exists and live-eval can detect
`memory_changed_plan`. But the action boundary work has richer surfaces now:
ActionDecision, ActionReview, Observer uncertainty, StopChecker outcomes, and
recovery plans.

Memory should feed those surfaces as bounded modifiers, not as broad prompt
guidance.

Risk:

- A known failed strategy may still be proposed because the model saw it only as
  background text.
- A known successful diagnostic strategy may not increase the value score of the
  right low-risk action.

### P2. Observability is good, but not note-complete

Current reports expose memory activity, recall counts, conflicts, and changed
plans. They do not yet assert the full note contract:

- candidate type/evidence/confidence/importance;
- project/user/agent scope correctness;
- last_used/use_count mutation after recall;
- failure lesson promotion;
- stale memory demotion;
- blocked sensitive candidate audit;
- action-risk/value modifier from memory.

## 4. Design Decision

Do not replace the current Markdown memory store with a single `memory.json`.

The current Markdown files are useful because gex can inspect and edit them by
hand. The better first product version is:

```text
human-readable Markdown files
  + typed memory record sidecar/index
  + one MemoryCandidate write pipeline
```

Recommended local shape:

```text
~/.priority-agent/
  USER.md
  MEMORY.md
  memory/
    *.md
    records.jsonl
    decisions.jsonl
    flush_queue.jsonl
    archive/
```

Each accepted memory should have a typed record in `records.jsonl`, and Markdown
should be treated as a readable projection of accepted records. Existing files
can be migrated gradually by assigning generated ids and best-effort metadata.

## 5. Implementation Plan

### Phase 1: Canonical memory contract and sidecar index

Goal: make a long-term memory item addressable, typed, scoped, and auditable.

Changes:

1. Extend the memory model with `MemoryCandidate` and richer `MemoryRecord`
   fields:
   - `importance: u8`;
   - `evidence: Vec<MemoryEvidenceRef>`;
   - `last_verified_at: Option<DateTime<Utc>>`;
   - `last_used_at: Option<DateTime<Utc>>`;
   - `use_count: u64`;
   - `success_count: u64`;
   - `failure_count: u64`;
   - `supersedes: Vec<String>`;
   - `superseded_by: Option<String>`.
2. Add a durable `records.jsonl` store under the memory directory.
3. Keep `MEMORY.md`, `USER.md`, and topic files as projections for accepted
   records.
4. Add best-effort import for existing Markdown sections into typed records
   without deleting or rewriting user-edited content.
5. Teach `/memory doctor_json` to report record counts, records missing
   evidence, stale records, and projection drift.

Acceptance:

- A saved memory has a stable id and survives restart.
- Markdown remains readable.
- `/memory doctor_json` exposes record metadata.
- Existing memory tests still pass.

Suggested validation:

```bash
cargo test -q memory -- --test-threads=1
cargo test -q memory_tool -- --test-threads=1
cargo test -q retrieval_context -- --test-threads=1
```

### Phase 2: Unified write pipeline

Goal: every memory write should go through the same candidate gate.

Changes:

1. Add `MemoryManager::submit_candidate(candidate, source)` as the only durable
   write entrypoint.
2. Route these callers through it:
   - manual `memory_save`;
   - heuristic turn sync;
   - LLM memory extraction;
   - session flush/trailing extraction;
   - workflow decisions;
   - stop/failure candidate memories;
   - agent role memory writes that should be shared with the main runtime.
3. Persist every accepted/proposed/rejected/blocked candidate to
   `decisions.jsonl` with:
   - candidate id;
   - source;
   - scope;
   - kind;
   - score;
   - evidence status;
   - safety status;
   - reason.
4. Keep low-confidence useful items as `Proposed` records instead of silently
   dropping them when they are good enough for review but not injection.
5. Make `memory_save` use `MemoryManager` instead of duplicated file logic.

Acceptance:

- Manual save, background extraction, and session flush produce the same
  decision shape.
- Duplicates and secrets are blocked consistently in every write path.
- Candidate memories from stop/failure recovery are visible in `/memory review`
  or equivalent doctor output.

Suggested validation:

```bash
cargo test -q memory -- --test-threads=1
cargo test -q memory_tool -- --test-threads=1
cargo test -q memory_sync_controller -- --test-threads=1
cargo test -q experience_ledger -- --test-threads=1
```

### Phase 3: Evidence-backed memory reflection

Goal: let the LLM propose memories, but require system-verifiable evidence.

Changes:

1. Replace free-form LLM memory bullets with structured JSON candidates:
   - `type`;
   - `content`;
   - `evidence`;
   - `confidence`;
   - `importance`;
   - `tags`;
   - `scope`;
   - `source_turn`.
2. Enforce evidence policy by kind:
   - user preferences can use explicit user statement evidence;
   - project facts require file/tool/trace evidence;
   - failure lessons require failed outcome/recovery evidence;
   - successful strategies require success/validation evidence;
   - guesses remain task state, not long-term facts.
3. Add a final task-end memory reflection step that reads the compact
   `AgentTaskState`, Observer findings, stop status, recovery plans, and
   validation proof, then emits candidates only.
4. Keep the system gate responsible for accepting, proposing, or rejecting.

Acceptance:

- An unsupported project fact is rejected or left proposed.
- A user-explicit preference can be accepted with conversation evidence.
- A failed strategy from a stop/recovery event becomes a proposed or accepted
  failure-pattern/strategy memory.
- Sensitive evidence is redacted or blocked before storage.

Suggested validation:

```bash
cargo test -q memory -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q stop_checker -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

### Phase 4: Scoped retrieval with usage telemetry

Goal: retrieve fewer, better memories and learn which memories are useful.

Changes:

1. Build memory retrieval from typed records first, falling back to Markdown
   snippets only for unimported legacy content.
2. Rank by:
   - project/user/agent scope match;
   - tags;
   - lexical/semantic match;
   - importance;
   - confidence;
   - freshness and last_verified;
   - prior usefulness;
   - token cost;
   - conflict status.
3. Record `MemoryUsed` telemetry when a memory enters context:
   - memory id;
   - query;
   - score;
   - reason;
   - conflict status;
   - retrieval policy;
   - trace id/session id.
4. Increment `use_count` and update `last_used_at` only for records actually
   injected into context.
5. Normalize request preparation so memory retrieval is always inserted as
   background retrieval context, never appended to user content.

Acceptance:

- `/memory explain <query>` can name record ids, scores, and reasons.
- A recalled record's `use_count` changes.
- Conflicting or stale records are capped below high-confidence injection.
- Light/Web/None routes still skip stale memory context.

Suggested validation:

```bash
cargo test -q retrieval_context -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q turn_retrieval_context_controller -- --test-threads=1
cargo test -q context_budget_controller -- --test-threads=1
```

### Phase 5: Strategy and failure memory promotion

Goal: turn stop/failure recovery into reusable behavior memory.

Changes:

1. Convert `ExperienceRecord.candidate_memories` into real `MemoryCandidate`
   inputs.
2. Add structured strategy/failure fields:
   - failed_strategy;
   - better_strategy;
   - context tags;
   - triggering error/failure type;
   - recovery plan id;
   - success/failure counts.
3. Promote validated successful recovery patterns into `SuccessfulFix` or
   strategy records.
4. Link memories back to learning-event ids and trace ids.
5. Surface strategy/failure candidates in `/memory review` and improvement
   proposals without auto-applying high-risk behavior changes.

Acceptance:

- A repeated tool/validation/model-output failure can produce a proposed
  failure-pattern memory.
- A later successful recovery updates success_count/failure_count or supersedes
  the old lesson.
- The record is project-scoped unless it is clearly user/global behavior.

Suggested validation:

```bash
cargo test -q experience_ledger -- --test-threads=1
cargo test -q recovery_plan -- --test-threads=1
cargo test -q turn_recording -- --test-threads=1
cargo test -q closeout -- --test-threads=1
```

### Phase 6: Memory-aware action weighting

Goal: let memory affect behavior through bounded runtime scores, not broad
prompt pressure.

Changes:

1. Add a memory modifier layer for action scoring/review:
   - positive value boost for successful strategies;
   - risk boost for failure patterns;
   - ask-user/deny hints for stable constraints;
   - lower confidence for stale/conflicting memories.
2. Feed only top relevant strategy/failure/policy records into
   `ActionDecision`/`ActionReview` as explicit factors.
3. Trace the modifier:
   - memory id;
   - action id/tool;
   - value/risk delta;
   - reason;
   - final effect.
4. Keep modifiers bounded so one memory cannot dominate runtime safety gates.

Acceptance:

- A known failed strategy increases action risk in trace.
- A known successful diagnostic strategy increases action value in trace.
- Memory cannot override permission, checkpoint, or user instruction rules.

Suggested validation:

```bash
cargo test -q action_decision -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

### Phase 7: Lifecycle maintenance and stale memory handling

Goal: memory should age, be revalidated, superseded, archived, or deleted.

Changes:

1. Add record-level lifecycle actions:
   - keep active;
   - revalidate;
   - supersede;
   - archive;
   - reject/delete candidate.
2. Use file hashes, package/config evidence, project root, and last_verified to
   detect project facts that need revalidation.
3. Add conflict resolution:
   - new verified record supersedes old unverified record;
   - equal confidence conflicts go to review;
   - stale records are visible but not high-confidence injected.
4. Extend `/memory doctor` and `/memory review` with actionable stale/conflict
   lists.

Acceptance:

- A stale project fact is not injected as high confidence.
- A new verified fact can supersede an old one without deleting audit history.
- Archive keeps records inspectable but lowers retrieval rank.

Suggested validation:

```bash
cargo test -q memory -- --test-threads=1
cargo test -q memory_tool -- --test-threads=1
cargo test -q retrieval_context -- --test-threads=1
```

### Phase 8: Evals and reporting

Goal: make the memory contract testable at behavior level.

Changes:

1. Add deterministic tests for:
   - structured candidate acceptance/rejection;
   - evidence-required project fact;
   - secret/prompt-injection block;
   - project/user/agent scope separation;
   - retrieval usage telemetry;
   - stale/superseded record demotion;
   - failure lesson promotion;
   - memory action weight modifier.
2. Extend live-eval parser/report fields:
   - `memory_candidate_typed`;
   - `memory_candidate_has_evidence`;
   - `memory_record_used`;
   - `memory_use_count_updated`;
   - `memory_failure_lesson_promoted`;
   - `memory_action_weight_changed`;
   - `memory_stale_demoted`;
   - `memory_scope_correct`.
3. Add at least two live-eval cases:
   - failure lesson from one task improves the next task;
   - stale project fact is demoted after contradictory verified evidence.

Acceptance:

- Behavior reports prove memory changed action choice for the right reason.
- Sensitive/unsupported memories are visibly blocked.
- Memory contract regressions fail in deterministic tests before live eval.

Suggested validation:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh
cargo test -q runtime_spine_behavior -- --test-threads=1
cargo test -q
cargo clippy --all-features -- -D warnings
```

## 6. Recommended Execution Order

Implement in this order:

1. Phase 1: typed sidecar/index.
2. Phase 2: unified write pipeline.
3. Phase 3: evidence-backed reflection.
4. Phase 4: scoped retrieval and usage telemetry.
5. Phase 5: strategy/failure promotion.
6. Phase 6: action weighting.
7. Phase 7: lifecycle maintenance.
8. Phase 8: eval/reporting.

Reason:

- Without Phase 1 and Phase 2, every later feature has to patch multiple write
  surfaces.
- Without Phase 3, memory can still store plausible but unsupported facts.
- Without Phase 4, lifecycle and usefulness cannot be measured.
- Phase 5 and Phase 6 are where the note's "failure becomes experience" goal
  becomes real behavior.

## 7. Non-Goals

Do not do these in the next slice:

- Do not dump all session history into prompt.
- Do not replace current Markdown files with opaque JSON only.
- Do not add a vector database before typed local retrieval is working.
- Do not let the LLM write accepted memory without system gates.
- Do not let memory override permissions, checkpoints, user instructions, or
  runtime safety rules.
- Do not auto-apply prompt/skill/self-evolution changes from memory alone.

## 8. First Patch Slice

The smallest valuable implementation slice is:

```text
Phase 1 + part of Phase 2
```

Concrete patch target:

1. Add `MemoryCandidate`, `MemoryEvidenceRef`, and extended `MemoryRecord`.
2. Add local `records.jsonl` append/read helpers.
3. Update `MemoryManager::add_learning_async` to create a typed record before
   writing Markdown.
4. Update `memory_save` to call `MemoryManager` instead of duplicating write
   logic.
5. Extend `/memory doctor_json` with typed-record counts.
6. Add focused tests for saved record metadata, duplicate block, secret block,
   and Markdown projection.

This patch does not need to solve action weighting yet. It creates the durable
substrate that the rest of the plan depends on.
