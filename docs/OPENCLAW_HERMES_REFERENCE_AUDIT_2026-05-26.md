# OpenClaw and Hermes Reference Audit

Date: 2026-05-26

Local sources inspected:

- `/Users/georgexu/Downloads/openclaw-main`
- `/Users/georgexu/Downloads/hermes-agent-main`
- `/Users/georgexu/Desktop/rust-agent`

Online references checked as cross-checks:

- `https://docs.openclaw.ai/reference/templates/SOUL`
- `https://hermes-agent.nousresearch.com/docs/user-guide/features/memory`
- `https://hermes-agent.nousresearch.com/docs/user-guide/features/memory-providers/`

## Executive Conclusion

Both projects are useful, but they should not be copied wholesale.

OpenClaw is most useful as a reference for prompt-file product design:
`SOUL.md`, `USER.md`, `TOOLS.md`, context-file ordering, skills roots,
prompt cache boundaries, and a built-in hybrid memory index.

Hermes is most useful as a reference for memory runtime design:
provider lifecycle hooks, strict memory scoping, frozen snapshots, external
provider isolation, memory-context fencing, and tests around session switch and
user/chat identity propagation.

Priority Agent already has strong foundations after the 2026-05-25 memory
alignment work: typed memory candidates, evidence references, records, retrieval
context, frozen memory/user snapshots, and a `MemoryProvider` trait. The next
best work is not another broad rewrite. The highest-value gap is wiring those
pieces into cleaner runtime boundaries.

## What To Borrow From OpenClaw

### 1. Separate Persona From Runtime Rules

OpenClaw keeps `SOUL.md` for voice/personality and keeps operational rules in
`AGENTS.md`. Its SOUL guidance is short and explicit: tone, boundaries,
default bluntness, continuity, and opinionated behavior belong there; security
rules, project changelogs, and large behavior manuals do not.

For Priority Agent, this maps cleanly to:

- `AGENTS.md`: runtime and project operating constraints.
- `SOUL.md`: Liz/Priority Agent voice, taste, and communication style.
- `USER.md`: compact user profile if present, but not a replacement for memory.
- `TOOLS.md`: project-local tool hints only when they are stable and actionable.

This is a direct product fit because Priority Agent is meant to be narrow,
personal, and verifiable. It also reduces pressure to stuff persona into the
main system prompt or long memory blocks.

Recommended implementation:

- Extend `src/instructions/mod.rs` beyond AGENTS-only loading.
- Add a strict file order: `AGENTS.md`, `SOUL.md`, `USER.md`, `TOOLS.md`.
- Keep per-file and total prompt budgets small.
- Add tests proving `SOUL.md` does not override runtime constraints.
- Keep `SOUL.md` optional; absence should preserve current behavior.

### 2. Make Prompt Cache Boundaries More Operational

OpenClaw has a clear stable-prefix versus dynamic-tail model. Stable identity,
tooling, skills, and static docs sit above the cache boundary; volatile session
state, runtime status, memory retrieval, and current turn context sit below it.

Priority Agent already has similar infrastructure in
`src/engine/context_assembly.rs` and `src/engine/prompt_context.rs`, but the live
request path still behaves like a legacy prompt renderer in places. The
`relevant_material`, `recent_observation`, and `current_decision_request` zones
exist conceptually, but the active rendering path is not using the full zone
model as the primary request shape.

Recommended implementation:

- Render all context zones into the model request, not only stable prefix and
  task state.
- Keep retrieval and recent observations out of the stable fingerprint.
- Replace fallback user-message memory injection with explicit fenced context
  zones where possible.
- Add snapshot tests for stable-prefix fingerprint stability.

### 3. Improve Local Memory Retrieval Infrastructure

OpenClaw's built-in memory docs describe a SQLite-backed memory engine with
FTS5 BM25, optional vector search, hybrid merge, CJK trigram support, session
transcript indexing, optional temporal decay, and MMR.

Priority Agent currently has typed records and retrieval/rerank logic, but it
still relies heavily on records, text files, lexical matching, and LLM rerank.
That is good enough for the current stage, but weaker for long-running Chinese
and cross-project recall.

Recommended implementation:

- Add a local search index beside `records.jsonl`, likely SQLite FTS5 first.
- Index `MEMORY.md`, topic memory files, `USER.md`, and structured records.
- Add CJK-friendly tokenization or trigram indexing before relying on vector
  search.
- Keep embeddings optional and provider-neutral.
- Preserve evidence refs and source paths in search results.

This is a P1 item, after provider lifecycle and scoping are cleaned up.

### 4. Strengthen Skills As Procedural Memory

OpenClaw's skill system has clearer root precedence, plugin-bundled skills,
per-agent allowlists, and scanner logic for third-party skills. Priority Agent's
current skill loader is much simpler: bundled skills plus configured external
paths/URLs.

Recommended implementation:

- Support workspace skill roots such as `.agents/skills` or `skills`.
- Add a per-agent allowlist before broad discovery.
- Add a scanner before loading URL or third-party skills.
- Treat skills as procedural memory: stable workflows, commands, repair loops,
  and domain operating guides belong there instead of in user memory.

## What To Borrow From Hermes

### 1. Finish The Memory Provider Boundary

Hermes has a clean provider lifecycle: initialize, system prompt block, prefetch,
queued prefetch, sync turn, session end, session switch, pre-compress,
delegation, write mirroring, tool schemas, and dispatch. It also isolates
provider failures and allows built-in memory plus at most one external provider.

Priority Agent already has the trait shape in `src/memory/provider.rs`, but
`LocalMemoryProvider` is still mostly an adapter marker and `MemoryManager`
still owns most local behavior directly.

Recommended implementation:

- Turn `LocalMemoryProvider` into the real local implementation.
- Make `MemoryManager` the orchestrator over built-in plus optional external
  providers.
- Enforce built-in plus at most one external provider until there is a concrete
  need for more.
- Add tests for provider fanout, provider failure isolation, and tool dispatch.

This is the highest-value Hermes-derived change.

### 2. Propagate Memory Scope Everywhere

Hermes tests focus on session switch, stale session writes, user identity, chat
identity, and provider session routing. That is the right kind of test surface.

Priority Agent has a rich `MemoryScope` type, but some internal paths still
default to unbound scope. For example, generic content-to-candidate flows can
fall back to `MemoryScope::local("unbound-session")`, and background extraction
can reconstruct a manager from base paths instead of carrying the active
project/session/user scope through the lifecycle.

Recommended implementation:

- Build one authoritative `MemoryScope` at conversation-loop turn start.
- Pass that scope into tool memory writes, auto extraction, background learning,
  retrieval, sync, and provider hooks.
- Remove or quarantine unbound-scope defaults.
- Add tests for project root, session id, parent session id, and agent context.

This should be paired with the provider-boundary work.

### 3. Keep Frozen Snapshots And Add Stronger Leak Guards

Hermes uses a frozen memory snapshot for the session prompt, while writes update
disk/live state but do not mutate the current system prompt. It also strips or
scrubs leaked `<memory-context>` blocks from provider output.

Priority Agent already has frozen memory/user snapshots and fenced retrieval
context. The remaining check is whether pre-existing or externally edited memory
files are sanitized as strictly on load as new writes are sanitized on save, and
whether streaming output can leak internal memory-context fences.

Recommended implementation:

- Audit load-time sanitization for `MEMORY.md`, `USER.md`, topic files, and
  structured records.
- Add tests for hostile persisted memory content.
- Add a small streaming/output scrubber only for internal memory/debug fences.
- Keep user-visible final answers natural and concise.

### 4. Tighten Memory Tool Semantics

Hermes distinguishes durable facts, preferences, and environment quirks from
task progress, PR status, one-off completed work, and procedures. Procedures
belong in skills, not memory.

Priority Agent has `MemoryKind`, evidence refs, and quality policy, so this can
be implemented mostly as schema/tool-contract tightening rather than a prompt
rewrite.

Recommended implementation:

- Tighten memory-save schema language around what not to save.
- Route repeatable procedures toward skills.
- Route task progress and command history toward traces/session search instead
  of long-term user memory.
- Keep evidence requirements strict for user preferences.

## What Not To Borrow

- Do not copy OpenClaw's broader mobile, gateway, or multi-surface platform
  shape. Priority Agent should stay focused on local coding workflow.
- Do not add Honcho, Supermemory, or mem0 as required dependencies. External
  memory should remain optional behind the provider trait.
- Do not turn `SOUL.md` into a large personality wall. Short beats long.
- Do not allow agent-created or downloaded skills without scanner/review gates.
- Do not weaken validation, checkpoint, permission, or closeout gates to make
  weaker providers pass.

## Recommended Implementation Order

### P0: Runtime Boundary Cleanup

1. Add optional `SOUL.md`, `USER.md`, and `TOOLS.md` loading with strict budgets
   and tests.
2. Convert the existing memory provider trait into the real orchestration
   boundary.
3. Propagate `MemoryScope` through all memory writes, extraction, retrieval, and
   provider hooks.
4. Render all context zones in the live request path and keep volatile retrieval
   below the stable cache boundary.

### P1: Retrieval And Skills

1. Add a local SQLite FTS index for memory records and memory files.
2. Add CJK-friendly tokenization or trigram search before vector search.
3. Add workspace skill roots, allowlists, and third-party skill scanning.
4. Tighten memory-save schema guidance around durable facts versus procedures.

### P2: Optional Advanced Memory

1. Add an active-memory sub-agent only for interactive persistent sessions.
2. Add an optional external memory provider bridge.
3. Add cadence/backoff controls for expensive provider prefetch.

## Detailed Follow-Up Modification Plan

This section turns the audit into an implementation roadmap. The plan favors
small, testable batches because the affected surface spans prompt assembly,
memory, skills, retrieval, and conversation-loop request preparation.

### Guiding Constraints

- Keep the runtime narrow, deep, personal, and verifiable.
- Preserve the LLM/runtime boundary: the LLM owns semantic judgment; runtime
  code owns deterministic screening, state, evidence, and closeout gates.
- Keep always-on prompts short. Put detailed behavior in tool contracts,
  runtime checks, memory retrieval, or skills.
- Do not weaken validation, permissions, checkpoints, or high-risk gates.
- Preserve existing user or prior-agent work in the dirty tree.
- Prefer staged compatibility over broad rewrites. The first version of each
  batch should preserve current behavior unless the plan explicitly says
  otherwise.

### Phase 0: Baseline And Guardrails

Goal: establish a known baseline before changing prompt and memory boundaries.

Primary files:

- `docs/OPENCLAW_HERMES_REFERENCE_AUDIT_2026-05-26.md`
- `docs/PROJECT_STATUS.md`
- `docs/AGENT_MEMORY_SYSTEM_ALIGNMENT_PLAN_2026-05-25.md`
- `src/instructions/mod.rs`
- `src/engine/prompt_context.rs`
- `src/engine/context_assembly.rs`
- `src/memory/provider.rs`
- `src/memory/manager.rs`
- `src/tools/memory_tool/mod.rs`
- `src/skills/loader.rs`

Implementation steps:

1. Capture the current test baseline before behavior changes.
2. Add or refresh narrow tests only where the next phase needs a safety net.
3. Record any known failing tests separately instead of broadening the scope.
4. Confirm whether the existing `turn_iteration_controller.rs` dirty change is
   related before touching nearby conversation-loop code.

Suggested validation:

```bash
cargo check -q
cargo test -q instructions
cargo test -q prompt_context
cargo test -q closeout
```

Acceptance criteria:

- The current behavior is understood before Phase 1 starts.
- Any existing unrelated dirty work remains untouched.
- The next phase has enough tests to detect prompt-order or memory-boundary
  regressions.

Risks:

- Prompt and memory tests can become brittle if they snapshot too much text.
  Prefer asserting structure, ordering, budgets, and boundaries over exact full
  prompt bodies.

### Phase 1: Root Context Files (`SOUL.md`, `USER.md`, `TOOLS.md`)

Goal: borrow OpenClaw's strongest idea by separating persona, user profile, and
tool hints from runtime rules.

Current gap:

- `src/instructions/mod.rs` primarily loads `AGENTS.md` and injects only the
  runtime guidance section.
- Persona and user-specific guidance currently compete with runtime rules,
  memory, and project docs.

Target behavior:

- `AGENTS.md` remains the authoritative runtime/project instruction file.
- `SOUL.md` is optional and limited to assistant voice, tone, communication
  style, judgment posture, and personality.
- `USER.md` is optional and limited to compact user profile facts. It must not
  replace long-term memory retrieval.
- `TOOLS.md` is optional and limited to stable project-local tool hints.
- File order is deterministic: `AGENTS.md`, `SOUL.md`, `USER.md`, `TOOLS.md`.
- Runtime constraints always outrank persona and user-profile context.

Implementation steps:

1. Introduce a small root-context model in `src/instructions/mod.rs`, for
   example a `RootContextKind` or equivalent local type for `Agents`, `Soul`,
   `User`, and `Tools`.
2. Extend workspace discovery to look for optional root context files beside
   existing AGENTS discovery.
3. Apply separate budgets:
   - `AGENTS.md`: preserve current runtime-guidance extraction behavior.
   - `SOUL.md`: small cap, likely lower than AGENTS.
   - `USER.md`: small cap, no large dossier.
   - `TOOLS.md`: small cap, stable commands only.
4. Render each file into a labeled section so traces and tests can identify the
   source.
5. Add explicit precedence language: `SOUL.md`, `USER.md`, and `TOOLS.md` cannot
   override runtime, sandbox, permission, validation, or tool-safety rules.
6. Add warnings or trace metadata when a file is skipped because of size,
   unreadable content, or invalid placement.
7. Keep the feature disabled by absence. Repos with only `AGENTS.md` should
   behave the same as today.

Tests:

```bash
cargo test -q instructions
cargo test -q prompt_context
```

Test cases to add:

- Loads files in deterministic order.
- Missing optional files do not change current behavior.
- `SOUL.md` is included as persona/style, not as runtime policy.
- `SOUL.md` cannot override AGENTS/runtime constraints in rendered ordering.
- Oversized optional files are truncated or skipped according to policy.
- `USER.md` and memory retrieval remain separate sources.

Acceptance criteria:

- A workspace can define Liz/Priority Agent persona in `SOUL.md` without
  editing `AGENTS.md`.
- Existing AGENTS-only workspaces keep the same effective instructions.
- Prompt budget stays within current project limits.
- The runtime can explain which root context files were loaded.

Risks and mitigations:

- Risk: `USER.md` becomes a second memory system.
  Mitigation: keep it compact and static; durable evolving facts still go
  through memory.
- Risk: `SOUL.md` grows into a personality wall.
  Mitigation: small cap and docs that say short beats long.
- Risk: tool hints become stale.
  Mitigation: `TOOLS.md` should contain stable commands only; dynamic tool
  availability still comes from runtime.

### Phase 2: Live Context Zones And Prompt Cache Boundary

Goal: make Priority Agent's existing context-zone model operational in the live
request path.

Current gap:

- `src/engine/context_assembly.rs` has zones such as stable prefix, task state,
  relevant material, recent observation, and current decision request.
- Some live request paths still render a legacy shape or append retrieval
  context into the user message as a fallback.

Target behavior:

- Stable identity, root instructions, static tool guidance, and static skill
  summaries stay in the stable prefix.
- Memory retrieval, project/session retrieval, tool observations, validation
  failures, and recent runtime evidence stay below the cache boundary.
- The model request makes trust boundaries visible: user text, repo docs,
  memory retrieval, tool output, and runtime observations should not be blended
  into one unlabelled string.

Implementation steps:

1. Trace every caller that renders a model request through
   `src/engine/prompt_context.rs` and conversation-loop request preparation.
2. Make `ContextAssemblyPlan` the primary request-shaping object instead of a
   mostly diagnostic structure.
3. Populate `relevant_material` with memory/project retrieval output.
4. Populate `recent_observation` with tool failures, validation output,
   checkpoint/diff evidence, and other runtime observations.
5. Use `current_decision_request` for concise per-turn objective and constraints
   when available.
6. Replace fallback user-message memory injection with explicit fenced zones.
7. Ensure stable-prefix fingerprints do not include volatile retrieval,
   timestamps, tool output, or session-only state.
8. Add trace output that records which zones were included and their token
   estimates.

Tests:

```bash
cargo test -q prompt_context
cargo test -q route_scoped_tools
cargo test -q closeout
```

Test cases to add:

- Memory retrieval appears in the relevant-material zone, not stable prefix.
- Tool/validation failures appear as recent observations.
- Stable prefix fingerprint is unchanged when only retrieval changes.
- User text is not merged into retrieved memory or tool observations.
- MVA/minimal profiles still skip or shrink memory context intentionally.

Acceptance criteria:

- The live request path uses the same zone model described in code.
- Prompt cache stability improves because volatile material is outside the
  stable prefix.
- Debug traces can explain why a retrieved memory or observation was present.

Risks and mitigations:

- Risk: changing request shape changes model behavior.
  Mitigation: keep labels concise, run narrow prompt/context tests first, then
  run one or two representative live-eval cases.
- Risk: too many zones inflate prompts.
  Mitigation: preserve token budgets per zone and emit truncation metadata.

### Phase 3: Memory Provider Orchestration Boundary

Goal: turn the existing `MemoryProvider` trait into the real runtime boundary
instead of leaving it as mostly a future-facing shape.

Current gap:

- `src/memory/provider.rs` already contains the Hermes-like lifecycle shape.
- `src/memory/manager.rs` still owns most local memory behavior directly.
- `LocalMemoryProvider` is still closer to an adapter marker than a full local
  provider.

Target behavior:

- `MemoryManager` orchestrates providers.
- `LocalMemoryProvider` owns local file/record storage behavior.
- The runtime supports built-in local memory plus at most one external provider
  until there is a concrete reason to support more.
- Provider failures are non-fatal unless the failed provider owns a required
  local operation.

Implementation steps:

1. Add a provider registry inside `MemoryManager` without moving storage yet.
   This creates the orchestration surface first.
2. Implement fanout for lifecycle hooks:
   - `initialize`
   - `system_prompt_block`
   - `prefetch`
   - `queue_prefetch`
   - `sync_turn`
   - `on_session_end`
   - `on_pre_compress`
   - `on_memory_write`
   - `shutdown`
3. Add failure isolation and structured provider errors.
4. Enforce one local provider plus zero or one external provider.
5. Move local file/record operations behind `LocalMemoryProvider` in a second
   commit-sized batch.
6. Keep public `MemoryManager` methods stable while internals move.
7. Add a fake provider for tests.

Tests:

```bash
cargo test -q memory
cargo test -q prompt_context
cargo test -q closeout
```

Test cases to add:

- Initialization fans out to all providers.
- Prefetch merges local and external provider context with source labels.
- One provider failure is recorded but does not crash unrelated providers.
- `on_memory_write` mirrors local writes to external provider hooks.
- Registering a second external provider is rejected.
- Provider shutdown is called once.

Acceptance criteria:

- Existing local memory behavior still works.
- Provider lifecycle is observable in tests.
- External provider support becomes a small adapter problem, not a manager
  rewrite.

Risks and mitigations:

- Risk: moving storage behind `LocalMemoryProvider` causes broad churn.
  Mitigation: split registry/fanout from storage migration.
- Risk: provider context becomes untrusted prompt material.
  Mitigation: render provider output as fenced, labelled, non-authoritative
  context unless the provider explicitly returns trusted runtime metadata.

### Phase 4: Authoritative `MemoryScope` Propagation

Goal: remove unbound memory writes and ensure every memory operation knows its
project, session, user/profile, and agent context.

Current gap:

- `MemoryScope` is rich enough, but some paths still fall back to local
  unbound session defaults.
- Background extraction and generic candidate creation can lose active
  project/session scope.

Target behavior:

- A single authoritative `MemoryScope` is built at conversation-loop turn start.
- All memory writes, retrieval, sync, extraction, and provider hooks receive
  that scope.
- Unbound scope is only allowed in explicit maintenance or migration paths.

Implementation steps:

1. Add a helper near conversation-loop state creation to build the active
   `MemoryScope`.
2. Include:
   - workspace/project root
   - session id
   - parent session id when available
   - profile or user id when available
   - agent context
   - platform/runtime source
3. Thread that scope through:
   - memory tool writes
   - auto learning
   - background extraction
   - retrieval context prefetch
   - provider hooks
   - session end and pre-compress hooks
4. Replace `MemoryScope::local("unbound-session")` defaults with scoped
   constructors or explicit maintenance-only calls.
5. Add trace metadata for scope on memory decisions.

Tests:

```bash
cargo test -q memory
cargo test -q closeout
```

Test cases to add:

- Tool memory write preserves working directory and session id.
- Auto extraction preserves working directory and session id.
- Background extraction does not downgrade to unbound scope.
- Retrieval can filter or rank by project/session scope.
- Session switch updates provider scope.

Acceptance criteria:

- Normal interactive turns no longer create unbound-session memory records.
- Memory records can be traced back to project/session/source evidence.
- Provider hooks receive enough context to support external memory safely later.

Risks and mitigations:

- Risk: too much scope filtering hides useful cross-project memory.
  Mitigation: separate hard scope from ranking preference. User preferences can
  remain broad; project facts should be scoped more tightly.

### Phase 5: Memory Safety, Frozen Snapshot, And Leak Guards

Goal: preserve Hermes-like frozen snapshot safety while tightening hostile
persisted-memory and internal-context leak handling.

Current baseline:

- Priority Agent already has frozen memory/user snapshots and evidence-backed
  memory writes.
- The remaining uncertainty is whether all persisted memory sources are scanned
  on load as strictly as new writes are screened on save.

Target behavior:

- Session prompt memory is frozen for the session.
- Writes update disk/records and become visible according to the next snapshot
  or explicit retrieval path, not by silently mutating stable prompt state.
- Hostile persisted memory content is detected and fenced or skipped.
- Internal memory/debug fences do not leak into final user-facing output.

Implementation steps:

1. Audit load paths for:
   - `MEMORY.md`
   - `USER.md`
   - topic files
   - structured `records.jsonl`
2. Apply the same or stricter prompt-injection scan on load as on write.
3. Add source-aware quarantine metadata for skipped hostile entries.
4. Add a minimal output scrubber for internal memory/debug fence markers if no
   equivalent guard already exists.
5. Keep scrubber narrow so it does not rewrite normal user content.

Tests:

```bash
cargo test -q memory
cargo test -q prompt_context
```

Test cases to add:

- Hostile `MEMORY.md` content is not injected as trusted instruction.
- Hostile `USER.md` content is fenced or skipped.
- Structured memory records preserve evidence while blocking promptware.
- Final output does not contain internal memory-context tags.
- Normal user-authored content that happens to mention memory is not stripped.

Acceptance criteria:

- Persisted memory is treated as contextual evidence, not instruction.
- Frozen snapshot semantics remain clear.
- The user does not see internal memory-context scaffolding in normal replies.

Risks and mitigations:

- Risk: over-aggressive scanning drops useful memory.
  Mitigation: quarantine with reason and expose via memory doctor/review.

### Phase 6: Local Memory Search Index

Goal: improve long-term recall quality, especially for Chinese and cross-project
history, without making embeddings or external services mandatory.

Current baseline:

- Priority Agent has typed records and retrieval/rerank.
- OpenClaw's built-in memory design is stronger for lexical search, hybrid
  retrieval, and CJK-friendly lookup.

Target behavior:

- Local memory records and memory files are indexed into a queryable local
  search store.
- Search results keep source path, evidence refs, scope, kind, timestamp, and
  confidence/importance metadata.
- The first version uses local lexical search. Embeddings remain optional.

Implementation steps:

1. Check existing dependencies before adding new ones.
2. Prefer SQLite FTS5 if it fits the dependency and packaging constraints.
3. Define index rows for:
   - source kind
   - source path
   - memory id or record id
   - scope fields
   - text chunk
   - evidence refs
   - timestamp
   - tags/kind
4. Add incremental indexing for `records.jsonl` and memory files.
5. Add rebuild/doctor command support for corrupt or stale indexes.
6. Add CJK-friendly tokenization or trigram-like fallback before adding vector
   complexity.
7. Feed indexed results into existing retrieval/rerank rather than replacing
   policy and evidence gates.

Tests:

```bash
cargo test -q memory
cargo test -q route_scoped_tools
```

Test cases to add:

- Index builds from records and memory files.
- Search returns source labels and evidence refs.
- Deleted or changed memory files are reindexed correctly.
- Chinese queries retrieve relevant Chinese memory.
- Retrieval respects scope ranking without hiding global preferences.

Acceptance criteria:

- Memory search quality improves without external services.
- Existing memory APIs remain compatible.
- Index corruption can be repaired without data loss.

Risks and mitigations:

- Risk: index state becomes another source of truth.
  Mitigation: treat the index as rebuildable cache; records/files remain
  canonical.
- Risk: database dependency complicates install.
  Mitigation: verify current dependency tree and feature availability first.

### Phase 7: Skills As Procedural Memory

Goal: make repeatable workflows learnable and reusable without polluting user
memory or always-on prompt text.

Current gap:

- `src/skills/loader.rs` supports bundled skills and configured external
  locations, but it does not yet match OpenClaw's stronger root precedence,
  allowlist, plugin-bundled skills, or scanner posture.

Target behavior:

- Skills capture procedures, commands, repair loops, provider quirks, and
  project workflows.
- User memory captures durable facts and preferences.
- Session traces capture transient task progress and validation history.

Implementation steps:

1. Define skill root precedence:
   - workspace `.agents/skills`
   - workspace `skills`
   - user-level configured skills
   - bundled skills
2. Add per-agent or per-project allowlist support before broad discovery.
3. Add a scanner for third-party or URL-loaded skills.
4. Add metadata for source, trust level, and load reason.
5. Add a trace event when a skill is considered, loaded, skipped, or rejected.
6. Tighten memory-save guidance so procedures are saved as skills instead of
   long-term memory entries.

Tests:

```bash
cargo test -q skills
cargo test -q prompt_context
```

Test cases to add:

- Workspace skill roots load in deterministic precedence order.
- Allowlist blocks unapproved skills.
- Scanner rejects dangerous skill content.
- Bundled skills remain available.
- Skill summaries stay compact until a skill is selected.

Acceptance criteria:

- Repeatable workflows have a first-class home.
- Third-party skill loading has trust and scanner boundaries.
- Always-on prompt remains short.

Risks and mitigations:

- Risk: workspace skills can smuggle prompt injection.
  Mitigation: scanner plus trust labels plus compact summaries, and full skill
  loading only when selected.

### Phase 8: Optional Active Memory And External Providers

Goal: add advanced memory only after local provider, scope, retrieval, and
skills are clean.

Target behavior:

- Active memory sub-agent runs only in eligible interactive persistent sessions.
- It does not run in one-shot evals, internal subagents, heartbeat automation,
  or headless maintenance paths.
- External providers are optional adapters behind the provider trait.

Implementation steps:

1. Define eligibility gates:
   - interactive persistent CLI or desktop session
   - user-facing main agent only
   - memory enabled
   - timeout budget available
   - not an eval/minimal/automation/internal path
2. Give the active-memory worker a tiny tool surface:
   - search memory
   - inspect memory record
   - return concise summary or `NONE`
3. Mark its output as untrusted contextual evidence.
4. Add timeout, max summary length, and failure isolation.
5. Add optional external provider adapter experiments only after local provider
   tests are solid.

Tests:

```bash
cargo test -q memory
cargo test -q closeout
bash -n scripts/run_live_eval.sh
```

Test cases to add:

- Active memory is skipped in eval/headless/internal sessions.
- Timeout returns no context rather than blocking the main turn.
- Output is fenced as context, not instruction.
- External provider failure does not block local memory.

Acceptance criteria:

- Advanced memory improves recall in long interactive work without becoming
  always-on prompt bloat.
- Eval and automation paths remain deterministic.

Risks and mitigations:

- Risk: active memory increases latency.
  Mitigation: strict timeout, cadence, and skip gates.
- Risk: active memory becomes hidden autonomous judgment.
  Mitigation: it only retrieves/summarizes context; the main LLM still owns
  semantic judgment.

### Documentation Updates

Update docs only when behavior changes, not for every internal refactor.

Expected doc updates by phase:

- Phase 1: update `docs/PROJECT_STATUS.md` and possibly add a short
  `docs/SOUL_USER_TOOLS_CONTEXT.md` if the feature needs user-facing guidance.
- Phase 2: update context/prompt status in `docs/PROJECT_STATUS.md`.
- Phase 3 and Phase 4: update
  `docs/AGENT_MEMORY_SYSTEM_ALIGNMENT_PLAN_2026-05-25.md` or add a successor
  note if the provider boundary materially changes.
- Phase 6: document index rebuild/doctor behavior.
- Phase 7: document skill root precedence and trust/scanner behavior.

### Suggested Commit Boundaries

1. `docs: add OpenClaw Hermes follow-up plan`
2. `prompt: load optional soul user tools context`
3. `prompt: render live request context zones`
4. `memory: introduce provider registry fanout`
5. `memory: move local storage behind provider boundary`
6. `memory: propagate scoped memory context`
7. `memory: harden persisted memory load and leak guards`
8. `memory: add local search index`
9. `skills: add workspace roots allowlists and scanner`
10. `memory: add gated active memory prototype`

### Overall Exit Criteria

The whole optimization line is complete when:

- `SOUL.md` can shape Liz's voice without weakening runtime rules.
- `AGENTS.md` remains the place for project/runtime constraints.
- Memory writes and retrievals carry project/session/user scope.
- The provider trait is a real boundary, not future-only scaffolding.
- Volatile memory and observations stay below the stable prompt cache boundary.
- Long-term recall improves for Chinese and cross-project work.
- Procedures move toward skills, not user memory.
- Live eval failures still report honest `not_verified`, `failed`, or
  `partial` instead of weakening gates.

## Source Evidence Index

OpenClaw files that informed this audit:

- `docs/concepts/soul.md`
- `docs/reference/templates/SOUL.md`
- `docs/reference/templates/USER.md`
- `docs/concepts/memory-builtin.md`
- `docs/concepts/memory-search.md`
- `docs/concepts/active-memory.md`
- `docs/concepts/system-prompt.md`
- `src/agents/system-prompt.ts`
- `src/plugins/memory-state.ts`
- `extensions/memory-core/src/prompt-section.ts`
- `docs/tools/skills.md`
- `src/security/skill-scanner.ts`
- `src/security/audit-workspace-skills.ts`

Hermes files that informed this audit:

- `hermes_cli/default_soul.py`
- `docker/SOUL.md`
- `agent/system_prompt.py`
- `agent/prompt_builder.py`
- `agent/memory_provider.py`
- `agent/memory_manager.py`
- `tools/memory_tool.py`
- `plugins/memory/honcho/README.md`
- `plugins/memory/honcho/__init__.py`
- `plugins/memory/supermemory/README.md`
- `plugins/memory/mem0/README.md`
- `tests/agent/test_memory_session_switch.py`
- `tests/agent/test_memory_user_id.py`

Priority Agent files that were compared:

- `docs/PROJECT_STATUS.md`
- `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`
- `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`
- `docs/AGENT_MEMORY_SYSTEM_ALIGNMENT_PLAN_2026-05-25.md`
- `docs/HERMES_MEMORY_SELF_EVOLUTION_REVIEW.md`
- `src/memory/provider.rs`
- `src/memory/types.rs`
- `src/memory/manager.rs`
- `src/tools/memory_tool/mod.rs`
- `src/engine/prompt_context.rs`
- `src/engine/context_assembly.rs`
- `src/instructions/mod.rs`
- `src/engine/conversation_loop/turn_retrieval_context_controller.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/skills/loader.rs`
- `src/skills/parser.rs`
