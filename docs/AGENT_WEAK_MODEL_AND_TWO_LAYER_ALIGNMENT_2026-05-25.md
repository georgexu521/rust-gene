# Weak-model Compensation and Two-layer Product Alignment - 2026-05-25

Source notes:

- `/Users/georgexu/Downloads/16-agent_compensating_weak_models_notes.md`
- `/Users/georgexu/Downloads/17-双层Agent架构设计记录.md`

Repo scope: `/Users/georgexu/Desktop/rust-agent`

Related repo docs:

- `docs/PROJECT_STATUS.md`
- `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`
- `docs/AGENT_MINIMUM_VIABLE_ARCHITECTURE_ALIGNMENT_PLAN_2026-05-25.md`
- `docs/AGENT_MINIMUM_VIABLE_ARCHITECTURE_FOLLOWUP_PLAN_2026-05-25.md`
- `docs/AGENT_EVALUATION_OBSERVABILITY_MVP_FOLLOWUP_PLAN_2026-05-25.md`
- `docs/LLM_FLOW_FAILURE_AUDIT_2026-05-25.md`

## 1. What These Two Notes Add

The two notes are about the same product direction from two angles.

Note 16 says: when the model is weak or unstable, the runtime must reduce the
space in which it can make unrecoverable mistakes. It should make tasks
smaller, context cleaner, tool exposure narrower, outputs more structured,
observations clearer, memory writes more conservative, stop checks stricter,
and verification more frequent.

Note 17 says: the product should not only be another coding agent. It should be
a project landing partner that helps ordinary users turn vague ideas into real
local tools and documents. That requires a warmer front surface for intent,
memory, clarification, prioritization, and explanation, plus a stricter
execution surface for files, code, commands, validation, and audit reports.

The combined lesson:

```text
Warmth belongs at the user/project relationship boundary.
Strictness belongs at the execution boundary.
The bridge between them must be a typed task contract.
```

## 2. Main Architecture Decision

Do not build two independent agent loops.

Priority Agent already has a stateful runtime spine: routing, task state,
context zones, tool exposure, action scoring, permission review, observer
records, stop checks, verification proof, traces, and live-eval reporting. A
second "Soul Agent loop" beside that spine would add coordination complexity,
state drift, duplicated memory rules, and prompt-heavy control.

The better design is one runtime spine with two responsibility layers:

```text
user-facing partner layer
  -> task contract and context pack
  -> existing execution runtime spine
  -> structured execution report
  -> partner explanation, memory proposal, next-step framing
```

This preserves the product goal: narrow, deep, personal, and verifiable. The
agent can feel personal without letting personality, stale memory, or reflection
text pollute the engineering context.

## 3. Layer Definitions

### 3.1 Partner Layer

The partner layer is the user-visible relationship and product-judgment layer.
It is allowed to be warm, personal, memory-aware, and proactive.

Responsibilities:

- understand vague user intent;
- ask only route-changing questions;
- make MVP and priority judgments;
- explain technical results in human terms;
- propose project memory updates;
- propose skill or rule updates after repeated evidence;
- decide when a task needs execution;
- produce the task contract and context pack.

Boundaries:

- it does not directly mutate project files;
- it does not run commands;
- it does not bypass execution permissions;
- it does not pass full soul, diary, or unrelated memory into execution;
- it can propose durable memory, but the memory write gate decides.

### 3.2 Execution Layer

The execution layer is not a separate personality. It is the existing runtime
spine, configured by route, task contract, risk, and model capability.

Responsibilities:

- expose only relevant tools for the current stage and profile;
- execute file, shell, memory, MCP, git, and agent actions through typed tools;
- enforce permission, checkpoint, and resource policy;
- observe raw tool output into compact evidence;
- update `AgentTaskState`;
- run or require validation;
- block false verified closeout;
- emit trace diagnostics and execution reports.

Boundaries:

- it does not carry the full partner persona;
- it does not expand the product scope without contract evidence;
- it does not write long-term memory just because a model summary sounds right;
- it reports `partial`, `failed`, or `not_verified` honestly when proof is
  missing.

## 4. The Bridge: Task Contract

The task contract is the typed interface between warm intent handling and strict
execution.

Minimum fields:

```yaml
task_type: code_change | doc_change | file_task | analysis | validation | deploy | data_task
objective: string
user_context:
  - only task-relevant user facts
project_context:
  - only task-relevant project facts
scope:
  files_allowed: []
  files_forbidden: []
  commands_allowed: []
  commands_forbidden: []
constraints:
  must_do: []
  must_not_do: []
acceptance_criteria:
  - observable result
validation:
  required_commands: []
  proof_required: true | false
risk:
  level: low | medium | high
  reasons: []
model_profile:
  mode: standard | constrained | review_required | human_confirm
report_schema:
  status: success | partial | failed | not_verified
  changed_files: []
  validation_evidence: []
  risks: []
  next_steps: []
```

The partner layer can create or revise this contract. The execution runtime
enforces it.

For direct chat or pure drafting tasks, the contract can be implicit and no file
execution is needed. For anything that reads project state, modifies files,
runs commands, deploys, or validates behavior, the contract should become
explicit enough to audit.

## 5. Context Pack Rule

The context pack is the execution-safe subset of memory and current task state.

It may include:

- task objective;
- current stage;
- allowed files and tools;
- acceptance criteria;
- relevant project facts with provenance;
- recent tool observations;
- failed validation summaries;
- known forbidden actions for this task;
- a small number of highly relevant memory facts.

It must not include:

- full soul/persona files;
- long chat history;
- unrelated user emotions or diary entries;
- stale project plans;
- old failed attempts that are not relevant to this task;
- broad skill bodies unless selected by evidence;
- unverified memory claims.

This is how the product can have warmth and memory without letting warmth and
memory contaminate code execution.

## 6. Weak-model Compensation as a Runtime Profile

Weak-model compensation should not be a pile of always-on prompt rules.

It should be a runtime profile selected from evidence:

```text
standard:
  normal model, normal risk, normal tool surface

constrained:
  weak model, high uncertainty, repeated low action score, or no-progress loop

review_required:
  mutation happened under weak/uncertain conditions, validation failed, or diff risk is high

human_confirm:
  destructive action, global environment change, secret/network/account/payment risk
```

Constrained profile behavior:

- smaller action batches;
- stage-scoped tools only;
- candidate-action shadow or gated ranking;
- read/search before edit unless there is current evidence;
- one small mutation before review;
- required diff summary after mutation;
- validation before verified closeout;
- stricter repeated-action and no-progress stop checks;
- memory writes require provenance and high confidence;
- failure owner classification separates model reasoning from runtime flow.

This matches the current runtime-diet direction: keep the base prompt short,
activate stricter repair text or policy only after concrete risk or failure
evidence.

## 7. Why This Is Not "No Two Layers"

There should be two product layers, but not two competing control loops.

Bad version:

```text
Soul Agent loop makes plans, stores memories, calls tools sometimes.
Executor Agent loop also makes plans, stores memories, calls tools sometimes.
Both can reinterpret task state.
```

Problems:

- duplicated authority;
- unclear source of truth;
- prompt pollution;
- memory conflict;
- harder evals;
- harder rollback;
- more chances for weak models to drift.

Preferred version:

```text
Partner layer owns user relationship and task framing.
Execution runtime owns tools, state transitions, proof, and audit.
TaskContract and ContextPack are the only handoff.
ExecutionReport is the return channel.
Memory updates are proposed, gated, and traceable.
```

This gives the user a partner without weakening the engineering boundary.

## 8. Evolution Without Losing Control

"Evolution" should mean evidence-backed improvement, not autonomous mutation of
core behavior.

Allowed evolution path:

1. execution produces a structured report;
2. partner layer summarizes the lesson in human terms;
3. memory system proposes a typed memory candidate;
4. write gate checks evidence, confidence, namespace, duplication, and conflict;
5. high-impact rules or skills require user confirmation or repo review;
6. evals prove the change improves behavior before it becomes a default.

Examples:

- A task-local failure can update task memory immediately.
- A repeated project-specific pitfall can propose project memory.
- A repeated workflow can propose a skill.
- A global rule needs strong evidence and review.
- A model-specific workaround should usually become a profile or repair trigger,
  not a universal prompt rule.

## 9. Current Repo Fit

This plan fits the current repo because the hard pieces already exist or are
partially in place:

- `AgentTaskState` can carry objective, stage, scope, observations,
  verification, stop checks, and stage history.
- context-zone work can become the concrete `ContextPack` surface.
- `TaskModeScore` and action scoring can help choose standard vs constrained
  execution.
- candidate-action shadow/gated ranking already gives a path for weak-model
  correction without replacing the model by default.
- permission, checkpoint, and resource policy already define execution
  boundaries.
- observer records and evidence ledger already convert raw tool output into
  compact model-visible facts.
- verification proof and completion contracts already block fake success.
- live-eval reporting already tracks invalid actions, premature edits, scope
  drift, repeated actions, failed actions, and agent scores.

The missing piece is product-level naming and wiring: make the partner/executor
handoff visible as `TaskContract`, `ContextPack`, and `ExecutionReport` instead
of treating it as an informal prompt convention.

## 10. Recommended Implementation Path

### Phase 0: Document the contract shape

Deliverables:

- add typed `TaskContract`, `ContextPack`, and `ExecutionReport` design notes;
- map each field to existing runtime owners;
- identify which fields are already traceable and which are only implied.

Acceptance:

- no new agent loop;
- no new always-on prompt block;
- every proposed field has a runtime owner or is explicitly future work.

### Phase 1: Add weak-model capability profile

Deliverables:

- runtime profile enum for `standard`, `constrained`, `review_required`, and
  `human_confirm`;
- profile selection from model/provider, risk, uncertainty, no-progress, and
  validation failure signals;
- trace/report output showing why the profile was selected.

Acceptance:

- strong/normal model tasks keep normal flow;
- weak/high-risk tasks get smaller tool exposure and stricter review;
- profile changes are visible in traces and live reports.

### Phase 2: Make ContextPack visible

Deliverables:

- emit a compact `ContextPackMaterialized` diagnostic or extend existing
  context-zone diagnostics;
- report included memory/retrieval facts with provenance and exclusion reasons;
- ensure partner/persona material is absent from execution context unless it is
  explicitly task-relevant.

Acceptance:

- eval can assert relevant facts included and irrelevant soul/history excluded;
- memory boundary cases remain passing.

### Phase 3: Product-facing execution report

Deliverables:

- normalize execution report fields for changed files, commands, validation,
  proof, risks, and next steps;
- make the CLI/desktop surface able to show a human explanation while retaining
  the machine report.

Acceptance:

- user sees a concise explanation;
- traces retain enough evidence for debugging;
- failed tasks return useful `partial` or `not_verified` reports instead of
  vague apologies.

### Phase 4: Project heartbeat later, not first

Deliverables:

- only after task contracts and reports are stable, add project-level heartbeat
  suggestions based on real project state.

Acceptance:

- off by default or user-configurable;
- never random companionship;
- always tied to a concrete project next step, stale task, failed validation, or
  risk.

## 11. Eval Additions

Add a weak-model suite that measures behavior, not vibes:

- simple bug fix requiring read-before-edit;
- stale edit anchor requiring repair;
- noisy test output requiring observer filtering;
- low-value repeated search requiring stop/replan;
- high-risk destructive request requiring block/confirmation;
- memory write candidate requiring evidence;
- doc task that should not run code;
- project task that must create or update a file and validate path safety.

Metrics:

- success or honest non-success;
- premature edit count;
- repeated action count;
- invalid action count;
- verification proof status;
- scope drift;
- memory write evidence;
- token/tool cost;
- failure owner.

The goal is not to make weak models look strong. The goal is to make weak-model
failures bounded, understandable, recoverable when possible, and honest when
not recoverable.

## 12. Non-goals

- Do not create a separate Soul runtime that can call tools independently.
- Do not pass full personality or long-term memory into execution.
- Do not solve weak models by adding broad permanent prompt rules.
- Do not split into many subagents before one executor contract is proven.
- Do not weaken validation, checkpointing, permissions, or high-risk gates to
  make a weak provider pass.
- Do not market the product as a broad Claude Code replacement.

## 13. Open Questions

1. Should the partner layer live first as prompt/context policy inside the CLI,
   or as a desktop-facing product surface?
2. How explicit should `TaskContract` be for normal coding tasks before it feels
   too heavy?
3. Which providers should trigger constrained profile by default, and which
   should require empirical eval evidence first?
4. Should memory update proposals be shown inline at closeout, in a separate
   review queue, or both?
5. What is the smallest user-facing "warmth" that improves trust without adding
   token cost or execution ambiguity?

## 14. Bottom Line

Priority Agent should have a warm partner face and a strict execution spine.

The product can remember gex, understand the project, ask good questions, and
evolve over time. But file changes, shell commands, memory writes, validation,
and verified closeout must stay under typed runtime control.

That is the practical synthesis of the two notes:

```text
one partner relationship
one execution runtime spine
typed handoff contracts
profile-based weak-model tightening
evidence-gated memory and evolution
```
