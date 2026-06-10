# Hermes Memory And Self-Evolution Review

Date: 2026-04-27

This document compares Hermes Agent's memory and self-evolution design with
Priority Agent's current implementation. It is both a planning artifact and an
implementation tracker.

Current implementation status:

- 2026-04-27: Phase 1 foundation started. Added typed memory contracts
  (`MemoryScope`, `MemoryRecord`, provenance, kind/status/sensitivity enums),
  a `MemoryProvider` trait plus `LocalMemoryProvider` adapter marker, memory
  safety scanning, memory quality scoring, and atomic local memory writes.
- 2026-04-27: Automatic memory writes now pass through safety and quality gates.
  Explicit `memory_save` calls run safety scanning and return quality metadata.
  Background LLM memory extraction also respects safety and quality checks.
- 2026-04-27: Automatic memory decisions are now recorded to a lightweight
  `decisions.jsonl` journal so accepted, proposed, rejected, and blocked writes
  can be inspected by `/memory doctor` and future LearningEvent integration.
- 2026-05-27: Closeout-time and lifecycle-flush memory sync now default to a
  review-only boundary. `MemoryProposal`, skipped-review-only flush records, and
  execution/progress evidence remain visible, but legacy turn/session extraction
  no longer silently persists long-term memory unless explicitly enabled with
  `PRIORITY_AGENT_AUTO_MEMORY_WRITE=legacy`. A narrower experimental policy,
  `PRIORITY_AGENT_AUTO_MEMORY_WRITE=narrow`, only allows explicit user
  preference statements to auto-persist during turn closeout; manual flushes
  remain a user-confirmed write path.
- 2026-05-27: The Hermes-style provider lifecycle is now inspectable through
  the memory doctor JSON/text panel: provider name/kind/availability, active
  scope, external provider, and lifecycle hooks are exposed without calling
  provider hooks. Controlled self-evolution is also normalized around
  `proposal -> eval -> accept/apply -> rollback`: improvement proposals now
  record eval status and rollback refs, `/improvements apply` requires a passed
  eval, and `/evolution status` summarizes improvement/skill evolution state.
- 2026-05-27: The next memory/self-evolution slice landed the first cleaner
  ownership boundaries: `MemoryManager` delegates local typed-record reads to
  `LocalMemoryProvider`, closeout memory proposals are persisted to a review
  queue, `/memory-proposals` supports `list/show/accept/reject/apply`,
  `/active-task` provides one combined progress panel, and improvement proposals
  can bind named evalsets before apply.

## Sources Reviewed

Online sources:

- [NousResearch/hermes-agent GitHub repository](https://github.com/NousResearch/hermes-agent)
- [Hermes memory provider plugin docs](https://hermes-agent.nousresearch.com/docs/developer-guide/memory-provider-plugin/)
- [Hermes working with skills docs](https://hermes-agent.nousresearch.com/docs/guides/work-with-skills/)

Local Hermes source:

- `/Users/georgexu/Desktop/hermes-agent-main/MEMORY_EVOLUTION_ANALYSIS.md`
- `/Users/georgexu/Desktop/hermes-agent-main/agent/memory_manager.py`
- `/Users/georgexu/Desktop/hermes-agent-main/agent/memory_provider.py`
- `/Users/georgexu/Desktop/hermes-agent-main/tools/memory_tool.py`
- `/Users/georgexu/Desktop/hermes-agent-main/tools/rl_training_tool.py`
- `/Users/georgexu/Desktop/hermes-agent-main/plugins/memory/supermemory/README.md`
- `/Users/georgexu/Desktop/hermes-agent-main/plugins/memory/honcho/README.md`
- `/Users/georgexu/Desktop/hermes-agent-main/skills/dogfood/SKILL.md`
- Hermes tests under `tests/agent`, `tests/tools`, `tests/gateway`, and
  `tests/plugins/memory`.

Priority Agent source:

- `src/memory/manager.rs`
- `src/tools/memory_tool/mod.rs`
- `src/session_store/mod.rs`
- `src/engine/intent_router.rs`
- `src/engine/retrieval_context.rs`
- `src/engine/reflection_pass.rs`
- `src/skills/`
- `docs/CODING_AGENT_WORKFLOW_DISCUSSION.md`
- `docs/PROJECT_STATUS.md`
- `docs/REMAINING_CLOSURE_PLAN.md`

## Executive Summary

Priority Agent already has more memory infrastructure than a simple coding CLI:
frozen memory snapshots, pre-turn memory prefetch, LLM-assisted extraction,
topic memory files, namespace search, conflict hints, session FTS search,
LearningEvent records, RetrievalContext, workflow reflection, and skills.
The default write posture is now intentionally stricter than Hermes: execution
closeout proposes memory and records progress evidence, while durable automatic
long-term writes require an explicit policy opt-in or an explicit `memory_save`
tool call.
The provider lifecycle and self-evolution state are now inspectable rather than
implicit: `memory_load` doctor output reports the memory provider panel, and
`/evolution status` shows the shared proposal/eval/apply/rollback control loop.

The main gap is that these pieces are not yet a productized self-evolution
system. Hermes treats memory as a provider lifecycle with hooks, quality
controls, setup/doctor commands, profile isolation, skill evolution, and
behavioral evaluation loops. Priority Agent has many of the raw ingredients,
but the lifecycle is still mostly local-file oriented and the feedback loop is
not yet strong enough to reliably improve future coding behavior.

The useful target is not "copy Hermes". The useful target is:

```text
Observed behavior -> quality-gated memory -> retrieval with provenance
  -> weighted planning/routing adjustment -> eval-backed skill or prompt update
  -> measured improvement or rollback
```

## What Hermes Gets Right

### 1. Provider-Oriented Memory Lifecycle

Hermes defines memory as a provider contract rather than one hardcoded storage
path. The public docs describe a `MemoryProvider` abstraction with lifecycle
methods and hooks:

- `initialize`
- `system_prompt_block`
- `prefetch`
- `queue_prefetch`
- `sync_turn`
- `on_session_end`
- `on_pre_compress`
- `on_memory_write`
- `shutdown`

This is important because memory is not just storage. It is a turn lifecycle:

```text
session start -> static prompt block
turn start -> recall / prefetch
turn end -> sync observation
compression -> preserve insights before discard
session end -> final extraction
manual write -> mirror to external providers
```

Hermes also intentionally allows the built-in provider plus at most one external
provider. That avoids tool schema bloat and conflicting memory sources while
still allowing Honcho, Supermemory, Mem0, Hindsight, Byterover, OpenViking, and
other provider styles.

Priority Agent currently has `MemoryManager`, but not a clean provider trait
with lifecycle hooks. External provider support would be difficult to add
without touching the core manager.

### 2. Memory Safety And Prompt-Cache Discipline

Hermes' built-in `MEMORY.md` / `USER.md` store uses several pragmatic controls:

- A frozen snapshot is captured at session start and injected into the system
  prompt, while mid-session writes update disk but do not mutate the prompt.
- Memory entries are bounded by character limits.
- Writes use locks and atomic replace.
- Memory content is scanned for injection and exfiltration patterns before it
  can be injected into future prompts.
- Prefetched memory is fenced as context, not user instruction.

Priority Agent already has frozen snapshots and XML fences. It has basic
deduplication and topic routing. The weaker parts are atomic writes, prompt
injection scanning, quality scoring, conflict resolution policy, and durable
flush/retry tracking.

### 3. Profile, User, Thread, And Workspace Scoping

Hermes is careful about scoping:

- `HERMES_HOME` is profile-aware.
- Providers receive `hermes_home`, `platform`, `agent_context`, `user_id`,
  `parent_session_id`, and identity/workspace information.
- Supermemory supports profile-scoped containers and whitelisted custom
  containers.
- Honcho supports host/profile-specific config, user and AI peer identities,
  session strategies, and per-peer observation settings.

This matters because a coding agent can easily pollute memory:

- one project leaks into another project
- a subagent writes incomplete observations into user memory
- cron/system turns alter the user's profile
- a resumed session reuses the wrong memory namespace

Priority Agent has project/user/topic/agent memory namespaces, but scoping is
not yet formalized as a durable contract. It needs a first-class `MemoryScope`.

### 4. Skills As Procedural Memory

Hermes treats skills as reusable procedural memory, not just documentation.
Important patterns:

- `SKILL.md` with YAML frontmatter.
- Progressive disclosure: list metadata first, load full skill only when used,
  load reference files only when needed.
- Installed skills become slash commands.
- Skills can declare required config.
- Skills can be installed from hubs or URLs with provenance tracking.
- Agent-created skills are possible, but need trust and review controls.
- Periodic nudges remind the model to persist reusable procedures as skills.

Priority Agent already has a skill system and has imported some behavior
skills, but the self-evolution path is still incomplete:

```text
failure or repeated task -> candidate skill update -> review/test -> trusted install
```

### 5. Self-Evolution Uses Evaluation, Not Just Memory

Hermes' public README describes a closed learning loop: agent-curated memory,
periodic nudges, autonomous skill creation after complex tasks, skill
self-improvement during use, FTS5 session search, and research-ready trajectory
generation/compression. The local repo also includes `rl_training_tool.py`,
trajectory tooling, and dogfood/QA skills.

The important lesson is that self-evolution should not mean "the agent edits
itself whenever it feels like it." It should mean:

```text
1. capture behavior trace
2. classify failure or success pattern
3. propose memory / prompt / skill / routing update
4. run a behavior eval or targeted regression
5. accept, reject, or rollback the change
6. store the outcome as learning
```

Priority Agent already has EvalSet and LearningEvent foundations, but the loop
from runtime failure to proposed improvement to measured adoption is not yet
productized.

## Priority Agent Current State

Already implemented or partially implemented:

- `MEMORY.md`, `USER.md`, topic memory files, and agent JSON memory search.
- Frozen memory snapshot at session start.
- `<memory-context>` and `<relevant-memory>` style fenced injection.
- Local memory prefetch and LLM rerank.
- Heuristic and LLM-assisted learning extraction.
- Background memory extraction and optional trailing session extraction.
- Namespace search and conflict hints in memory tooling.
- SQLite sessions with WAL and FTS5 search.
- LearningEvent persistence and learning-aware routing.
- Unified RetrievalContext with provenance-bearing retrieval items.
- ReflectionPass and workflow contracts.
- Skills with `SKILL.md` style loading.
- Recent `/resume` work now restores prior CLI sessions into live
  conversation history.

Key gaps:

- No `MemoryProvider` trait with provider lifecycle hooks.
- No provider setup/doctor UX.
- No first-class `MemoryRecord` schema with scope, provenance, quality,
  confidence, lifecycle state, and source.
- Memory writes are not consistently atomic or file-locked.
- Memory write security scanning is incomplete.
- No durable async flush queue with stale-write guards and retry limits.
- No stable quality gate for deciding whether a memory should be saved.
- Conflict detection exists, but conflict resolution is not part of a workflow.
- Self-evolution is not yet gated by evals and rollback rules.
- Skill improvement is not yet driven by runtime failures or recurring
  successful procedures.
- Learned events do not yet strongly recalibrate weighted planning factors.

## Recommended Architecture

### MemoryProvider Contract

Introduce a provider layer without deleting the existing local memory behavior.
The existing `MemoryManager` should become the orchestrator.

```rust
#[async_trait::async_trait]
pub trait MemoryProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    async fn initialize(&self, scope: &MemoryScope) -> anyhow::Result<()>;
    async fn system_prompt_block(&self, scope: &MemoryScope) -> anyhow::Result<Option<String>>;
    async fn prefetch(&self, query: &str, scope: &MemoryScope) -> anyhow::Result<Vec<MemoryRecord>>;
    async fn queue_prefetch(&self, query: &str, scope: &MemoryScope) -> anyhow::Result<()>;
    async fn sync_turn(&self, turn: &MemoryTurn, scope: &MemoryScope) -> anyhow::Result<()>;
    async fn on_session_end(&self, transcript: &[MemoryTurn], scope: &MemoryScope) -> anyhow::Result<()>;
    async fn on_pre_compress(&self, messages: &[MemoryTurn], scope: &MemoryScope) -> anyhow::Result<Vec<MemoryRecord>>;
    async fn on_memory_write(&self, record: &MemoryRecord, scope: &MemoryScope) -> anyhow::Result<()>;
    async fn shutdown(&self) -> anyhow::Result<()>;
}
```

First implementation should be `LocalMemoryProvider`, wrapping current
`MEMORY.md`, `USER.md`, and topic memory behavior.

External providers can come later. The provider contract is useful even before
adding Supermemory or Honcho because it clarifies lifecycle boundaries.

### MemoryRecord Schema

Memory should be a typed record before it becomes Markdown.

```rust
pub struct MemoryRecord {
    pub id: String,
    pub scope: MemoryScope,
    pub kind: MemoryKind,
    pub content: String,
    pub summary: String,
    pub provenance: MemoryProvenance,
    pub confidence: f32,
    pub utility: f32,
    pub sensitivity: SensitivityLevel,
    pub status: MemoryStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
}
```

Suggested enums:

```text
MemoryKind:
  user_preference | project_fact | workflow_convention | tool_quirk |
  failure_pattern | successful_fix | decision | skill_candidate

MemoryStatus:
  proposed | accepted | rejected | superseded | archived

SensitivityLevel:
  public | local_only | secret_like | unsafe
```

### MemoryScope Contract

```rust
pub struct MemoryScope {
    pub user_id: Option<String>,
    pub profile: String,
    pub project_root: Option<PathBuf>,
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub agent_context: AgentContext,
    pub platform: String,
}
```

`AgentContext` should distinguish:

```text
primary | subagent | cron | flush | eval | test
```

Only `primary` should freely write user/project memory. Subagents should write
observations back to the parent as proposed records unless explicitly allowed.
Eval/test contexts should not pollute production memory.

### Quality Gate

Every proposed memory should pass through a quality gate:

```text
quality_score =
  stable_fact * 0.25
+ future_utility * 0.25
+ specificity * 0.20
+ user_or_project_relevance * 0.20
- volatility * 0.15
- sensitivity_risk * 0.20
- duplication * 0.15
```

Suggested behavior:

- `score >= 0.70`: accept automatically if safety scan passes.
- `0.45 <= score < 0.70`: keep as proposed memory and surface in `/memory review`.
- `score < 0.45`: reject or keep only in session trace.
- `sensitivity_risk == unsafe`: block.

The model should judge the semantic factors; the software should calculate the
score, clamp values, record provenance, and enforce safety boundaries.

### Retrieval Ranking

Memory retrieval should rank records with a mixed score:

```text
retrieval_score =
  semantic_relevance * 0.35
+ lexical_overlap * 0.15
+ scope_match * 0.20
+ confidence * 0.10
+ utility * 0.10
+ recency_decay * 0.05
+ frequency_boost * 0.05
- conflict_penalty * 0.15
- sensitivity_penalty * 0.20
```

This should feed `RetrievalContext` directly, with provenance visible in trace
and optionally in CLI debug panels.

### Self-Evolution Loop

Define self-evolution as a gated workflow:

```text
Runtime trace / eval failure / user correction
  -> LearningEvent
  -> FailurePattern or SuccessPattern
  -> ImprovementProposal
  -> target: memory | skill | prompt | routing | tool guidance
  -> validation plan
  -> eval / targeted tests
  -> accept, revise, or reject
  -> store outcome
```

`ImprovementProposal` should be explicit:

```rust
pub struct ImprovementProposal {
    pub id: String,
    pub trigger_event_ids: Vec<i64>,
    pub target: ImprovementTarget,
    pub proposed_change: String,
    pub expected_benefit: String,
    pub risk: RiskLevel,
    pub validation: Vec<String>,
    pub status: ProposalStatus,
}
```

This avoids uncontrolled self-modification.

### Skill Evolution

A skill should be updated only when there is enough evidence:

```text
one-off fact -> memory
repeated stable procedure -> skill candidate
tool/model failure pattern -> prompt/tool guidance candidate
project-specific convention -> project memory or project skill
```

Proposed workflow:

1. Detect repeated successful procedure or repeated failure.
2. Generate `SkillProposal`.
3. Run a skill quality checklist:
   - clear trigger condition
   - scoped tools
   - concrete workflow
   - validation instructions
   - no secrets
   - no brittle project-only assumptions unless project-scoped
4. Save as untrusted/proposed skill.
5. User or eval accepts it.
6. Promote to trusted active skill.

## Implementation Plan

### Phase 1: Memory Reliability Foundation

Goal: make current memory durable, scoped, safe, and inspectable.

Status: implemented for the local provider path. Core contracts, safety scanner,
quality gate, atomic local writes, memory decision journaling, and `/memory
doctor` decision counts have been implemented. Remaining product polish is
richer `/memory review` UX and direct LearningEvent persistence for
accepted/proposed/rejected memory decisions.

Tasks:

1. Add `MemoryScope`, `MemoryRecord`, `MemoryProvenance`, and `MemoryQuality`
   contracts.
2. Add `LocalMemoryProvider` behind a `MemoryProvider` trait.
3. Move current local file writes behind provider methods.
4. Add atomic writes and lock protection for local memory files.
5. Add memory safety scanner for prompt injection, invisible Unicode, secret-like
   content, and shell exfiltration patterns.
6. Add a quality gate before accepting automatic memories.
7. Persist accepted/proposed/rejected memory decisions as LearningEvents.

Acceptance criteria:

- Existing memory tests still pass.
- Auto memory writes produce typed records before Markdown output.
- Unsafe memory content is blocked with a useful error.
- Duplicate or low-quality memory is rejected or marked proposed.
- `/memory` can show accepted/proposed/rejected counts.

### Phase 2: Memory Lifecycle And Flush Queue

Goal: make memory extraction reliable across turn, compression, resume, and exit.

Status: implemented for the interactive CLI/TUI engine path. Memory flushes now
write an append-only durable lifecycle log at
`~/.priority-agent/memory/flush_queue.jsonl`, including session id, reason,
message hash, status, attempt count, and completion timestamp. The engine
flushes on session end, compression preflight, `/clear`, `/new`, `/resume`, and
exit. Duplicate flushes for the same session/reason/message hash are marked
`skipped_duplicate`, and `/memory doctor` reports flush counts.

Tasks:

1. Add lifecycle hooks to the memory orchestrator. ✅
2. Add durable async flush queue with retry count, stale guard, and completion
   marker. ✅
3. Flush before context compression through the engine pre-compress lifecycle
   hook. ✅
4. Flush on CLI exit, `/new`, `/clear`, and `/resume` session switch. ✅
5. Ensure resumed sessions use the correct session binding before continuing. ✅
6. Add `/memory doctor` for path, provider, pending flush, conflict, and safety
   status. ✅ for local provider counts and lifecycle status.

Acceptance criteria:

- A session cannot be flushed twice unless explicitly retried. ✅
- Failed flushes are visible and bounded. ✅ status contract exists; retry
  worker is intentionally deferred until provider failures return structured
  errors.
- `/resume` does not write memories to the wrong session/project scope. ✅
- Memory flush does not block normal streaming response. ✅ session-end flush is
  still spawned after response completion; pre-compress flush runs only when the
  context is about to be rewritten.

### Phase 3: Retrieval And Observability

Goal: make memory useful at the exact moment it is needed.

Status: implemented for memory retrieval. Memory prefetch now builds individual
`RetrievalItem` records instead of one opaque memory block. Each item includes a
stable retrieval id, provenance, reason, trust level, score, and conflict flag.
Memory ranking uses lexical match, scope, confidence, recency, and semantic
proxy factors; conflicts reduce confidence and are visible in trace and
`/memory` commands. Prompt injection boundaries now explicitly mark retrieval
context as background context, not user instructions.

Tasks:

1. Route memory retrieval through `RetrievalContext` as individual provenance
   items, not one opaque block. ✅
2. Add ranking using semantic/lexical/scope/confidence/recency factors. ✅
3. Emit trace events for selected memories and discarded high-score conflicts. ✅
4. Add `/memory search`, `/memory conflicts`, `/memory review`, and `/memory
   explain <id>`. ✅
5. Add prompt fences that distinguish memory from user instructions. ✅

Acceptance criteria:

- Trace shows which memories were injected and why. ✅
- Conflicting memories reduce retrieval confidence. ✅
- The model can cite memory provenance internally without treating it as
  instruction. ✅

### Phase 4: Self-Evolution Proposal System

Goal: turn repeated behavior into controlled improvement proposals.

Status: implemented for proposal generation and user-gated lifecycle. Runtime
LearningEvents can now be scanned into `ImprovementProposal` records stored in
`~/.priority-agent/improvements.jsonl`. Proposals target memory, skill, prompt,
routing, or tool guidance; include trigger event ids, evidence, expected
benefit, risk, validation plan, and optional evalset bindings; and can be
listed, shown, accepted, rejected, bound to evalsets, or applied through
`/improvements`. Applying requires prior explicit acceptance and passed
evaluation, so high-risk or behavior-changing suggestions are never applied
automatically. Proposal outcomes are persisted back as LearningEvents.

Tasks:

1. Add `ImprovementProposal` and `ImprovementTarget`. ✅
2. Convert user corrections, failed validations, repeated tool failures, and
   successful fixes into candidate proposals. ✅ initial runtime-event rules for
   repeated tool failures, recovery patterns, and corrections.
3. Add `/improvements` commands:
   - list ✅
   - show ✅
   - accept ✅
   - reject ✅
   - bind-eval ✅
   - apply ✅
4. Require validation plans for prompt, skill, routing, and tool guidance
   changes. ✅
5. Store proposal outcomes as LearningEvents. ✅

Acceptance criteria:

- The agent can propose a memory, skill, or routing improvement without applying
  it automatically. ✅
- High-risk changes require user approval. ✅
- Accepted proposals are tied to evidence, validation results, and optional
  bound evalsets. ✅

### Phase 5: Skill Evolution

Goal: convert reusable procedures into reviewed skills.

Tasks:

1. Add `SkillProposal` generated from repeated successful procedures. ✅
2. Add skill quality checklist and safety scan. ✅
3. Add proposed/untrusted/trusted skill states. ✅
4. Add skill eval harness for selected skills. ✅ lightweight proposal eval
   backed by the quality checklist.
5. Add native slash-command activation for accepted skills if not already
   present in the current CLI path. ✅ `/skill-proposals apply` writes a reviewed
   skill into the user skill path and reloads runtime skills.

Acceptance criteria:

- Repeated project workflows can become project-scoped skill candidates. ✅
- Skills are not activated until reviewed or validated. ✅
- Skill updates preserve user edits and record provenance. ✅ existing skill
  files are not overwritten; generated `SKILL.md` includes proposal provenance.

Implementation notes:

- Skill candidates are stored in `~/.priority-agent/skill_proposals.jsonl`.
- `/skill-proposals scan` reads recent `LearningEvent`s and groups repeated
  successful procedures.
- `/skill-proposals eval <id>` runs trigger, workflow, validation, scoped-tool,
  safety, and destructive-action checks.
- `/skill-proposals accept <id>` moves a candidate to untrusted review state.
- `/skill-proposals apply <id>` requires acceptance, refuses to overwrite
  existing user edits, writes `~/.priority-agent/skills/<name>/SKILL.md`, reloads
  skills, and records a `skill_proposal` learning event.

### Phase 6: Learning-To-Planning Feedback

Goal: make memory and learning affect future coding decisions.

Tasks:

1. Feed relevant LearningEvents and high-confidence MemoryRecords into
   weighted planning factors. ✅ via `learning_planning` using recent
   `LearningEvent`s and high-confidence retrieved memory items.
2. Adjust priority weights when past failures indicate risk.
   ✅ failed workflows, recovery plans, and failed tools raise
   verification/recovery factors.
3. Adjust IntentRouter when past tasks show a query type needs more retrieval,
   validation, or user clarification. ✅ `route_with_learning` escalates
   retrieval/reasoning/risk and adjusts recommended tools.
4. Record before/after planning decisions for audit. ✅ `planning_adjustment`
   LearningEvents and `workflow.learning` trace events store before/after
   summaries and adjustment reasons.

Acceptance criteria:

- A previously failed tool or workflow increases verification/recovery weight. ✅
- Repeated successful patterns reduce unnecessary exploration. ✅
- The agent can explain why memory changed the current plan. ✅ audit payload
  records source, affected step, factor deltas, before/after top step, and reason.

Implementation notes:

- `src/engine/learning_planning.rs` applies bounded factor adjustments after the
  model produces the workflow judgment. This keeps AI semantic judgment primary
  while allowing software to stabilize and audit the control signal.
- The adjustment layer can only modify factor values; it does not rewrite the
  user's request, plan text, or acceptance contract.
- Planning adjustments are persisted as `planning_adjustment` learning events so
  future turns can inspect and improve the feedback loop.

## Priority Recommendation

The highest value next step is not external provider integration. The highest
value next step is Phase 1 plus Phase 2:

```text
typed memory records
quality gate
safety scanner
provider trait
durable flush queue
scope correctness
```

After that, external providers such as Supermemory or Honcho become adapters
rather than architectural rewrites.

## Open Questions

1. Should proposed memories be visible by default, or only in an advanced
   `/memory review` view?
2. Should project memory live under `.priority-agent/` in the repo, the global
   user config, or both?
3. Should self-evolution proposals be allowed to modify prompts automatically
   after passing evals, or should every prompt/skill change require user review?
4. Should external memory providers be supported in the first productized
   version, or should the first version be local-only with a stable trait?
5. Should user profile memory and coding workflow memory have separate safety
   policies?
