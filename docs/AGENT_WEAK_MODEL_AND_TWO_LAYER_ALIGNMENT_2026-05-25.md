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

## 2. Product Thesis and Differentiation

This direction should be treated as a major product differentiator, not as a
minor runtime implementation detail.

The market already has many coding agents. Shipping another tool that can read
files, edit code, run shell commands, and summarize diffs is unlikely to stand
out. The stronger product claim is:

```text
Priority Agent is a long-term local AI project partner:
it remembers the user and project,
helps shape vague ideas into small buildable steps,
executes with coding-agent rigor,
and keeps proof, memory, and project progress auditable.
```

The useful synthesis is:

```text
OpenClaw-style long-term partner feel
+ Claude Code / Codex-style engineering execution
+ local project memory, verification, audit, and replay
```

The external positioning should avoid saying only "Soul-powered coding agent."
That sounds like personality layered on top of tools. The sharper positioning
is:

```text
a rigorous project-landing partner with long-term memory
```

In Chinese product language:

```text
不是又一个会写代码的 Agent，
而是长期陪你把项目做成的本地 AI 伙伴。
```

This means the visible product should make four capabilities obvious:

1. Project Soul: a compact project-partner constitution for how the agent works
   with the user, asks questions, controls MVP scope, and decides when execution
   is needed.
2. Project Memory: scoped, evidence-backed memory for project goals, stack,
   decisions, failures, validation commands, and user preferences.
3. Verified Executor: the strict execution spine for reading files, editing
   code, running commands, validating, reporting, and blocking false success.
4. Project Pulse: a project-progress heartbeat that suggests the next concrete
   step from real project state, never random companionship.

These capabilities should be product surfaces, not just internal names. Users
should be able to see what the agent remembered, why it remembered it, what
contract it is executing, what proof it has, and what next project step it
recommends.

Keep the core names stable unless there is a strong reason to change them:

```text
TaskContract = what to do, what boundaries apply, and how success is judged
ContextPack = what execution is allowed to know right now
ExecutionReport = what actually happened and what evidence exists

Project Soul = partner constitution
Project Memory = scoped project/user/task memory
Verified Executor = strict local execution spine
Project Pulse = state-backed project progress heartbeat
```

## 3. Main Architecture Decision

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

## 4. Layer Definitions

### 4.1 Partner Layer

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

### 4.2 Execution Layer

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

## 5. Task Routing Matrix

The routing boundary is not "code vs non-code."

The real boundary is whether the task touches project state, filesystem state,
command execution, deployment, validation responsibility, or durable memory.

| User task | Owner | Reason |
| --- | --- | --- |
| Pure discussion, brainstorming, product positioning, or tradeoff analysis | Partner Layer | No project state mutation or validation responsibility. |
| Generate a copyable Markdown draft in chat | Partner Layer | Draft output only; no filesystem side effect. |
| Create, modify, move, delete, or save a project file | TaskContract + Execution Layer | Filesystem state changes require scope, permissions, and report evidence. |
| Update `README.md`, docs, changelog, or project notes in the repo | TaskContract + Execution Layer | Docs are project state even when they are not code. |
| Generate API docs from current code | TaskContract + Execution Layer | Requires reading project files and preserving consistency. |
| Run tests, install, deploy, inspect environment, or execute shell commands | Execution Layer | Commands require tool policy, observation, and validation evidence. |
| High-risk or destructive operation | Execution Layer + `human_confirm` profile | Safety and permission policy outrank convenience. |
| Propose memory, skill, or rule update | Partner Layer proposes; memory/eval gates decide | Long-term state changes need evidence and scope. |

The Partner Layer can draft and reason freely in chat. Once a task needs real
project execution, it must hand off through a `TaskContract`.

## 6. The Bridge: Task Contract

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
assumptions:
  - assumption: string
    source: user_explicit | partner_inferred | project_memory | default_policy
    confidence: low | medium | high
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

Assumptions are first-class because ordinary users often leave important
details implicit. Examples:

- default to a local web tool;
- default to no login or cloud sync in MVP;
- default to minimal dependencies;
- default to a non-programmer-friendly explanation;
- default to validation before verified closeout.

If an assumption later proves wrong, the execution report can point to the
specific assumption instead of hiding the mistake inside a vague planning
failure.

## 7. Authority Matrix

When instructions, memory, inference, and safety disagree, the runtime needs a
clear authority order.

Priority order:

```text
1. Safety, permission, legal, and destructive-action policy
2. TaskContract scope and acceptance criteria
3. Current explicit user instruction
4. Current project state and tool evidence
5. Project memory with provenance
6. Partner-layer assumptions and inferences
7. Default preferences and product heuristics
```

Notes:

- A current user instruction can revise the task contract, but execution should
  first update the contract rather than silently ignore its old scope.
- Project memory cannot override the user's current explicit instruction.
- Partner inference cannot override task scope.
- Soul text cannot override permission, validation, or checkpoint policy.
- If authority is ambiguous, stop and ask or downgrade to a safer profile.

## 8. Context Pack Rule

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

ContextPack also needs budgets, not only allow/deny categories. Without budgets,
memory and project history can grow until execution context becomes noisy again.

Example budget:

```yaml
context_budget:
  max_user_facts: 5
  max_project_facts: 10
  max_recent_observations: 8
  max_failure_summaries: 3
  max_memory_records: 5
  max_skill_summaries: 2
  max_total_estimated_tokens: 4000
```

The budget should be relevance-based. A small number of high-provenance facts is
better than a large amount of loosely related memory.

## 9. Weak-model Compensation as a Runtime Profile

Weak-model compensation should not be a pile of always-on prompt rules.

The goal is not to make weak models perform like strong models. The goal is to
constrain weak-model failure into small, observable, reversible steps.

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

## 10. Why This Is Not "No Two Layers"

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

## 11. Product Surfaces To Build Toward

The architecture becomes a sellable product only when users can feel and inspect
the four surfaces below.

### 11.1 Project Soul

Project Soul is not unrestricted roleplay. It is a compact constitution for the
partner layer.

It should include:

- the agent's working relationship with the user;
- how it asks clarifying questions;
- how it controls MVP scope;
- when it should proceed with reasonable assumptions;
- when it must create a task contract and enter execution;
- when it must stop and ask for confirmation;
- how it proposes memory, skill, or rule updates.

It should not include:

- long diaries;
- emotional chat history;
- broad motivational language;
- detailed coding rules that belong in tools, permissions, or validation;
- one-off model mistake workarounds.

Rule: if a constraint can be enforced by runtime, do not encode it only in
Project Soul.

Examples:

- file deletion belongs in permission policy;
- verified closeout belongs in completion contract;
- weak-model narrow edits belong in constrained profile;
- tool safety belongs in tool contracts and review gates.

Project Soul should carry values, relationship, and routing behavior. Hard
constraints belong in runtime.

### 11.2 Project Memory

Project Memory should be visible, scoped, and evidence-backed.

Memory records need:

- content;
- source evidence;
- confidence;
- scope: user, project, task, skill, or global;
- freshness or expiry policy;
- conflict and duplicate status;
- whether the user accepted, rejected, or edited it.

This is the difference between "the agent remembers me" and "the agent
silently accumulates stale assumptions."

### 11.3 Verified Executor

Verified Executor is the trust anchor. It is what keeps the product from
becoming a personality wrapper over unreliable automation.

It must expose:

- task contract;
- context pack;
- changed files;
- commands run;
- validation proof;
- stop or repair reason;
- final status: `success`, `partial`, `failed`, or `not_verified`.

The user-facing answer can stay concise, but the evidence must remain available
in traces, reports, and replay artifacts.

### 11.4 Project Pulse

Project Pulse should be a project-progress heartbeat, not a social heartbeat.

Good pulse examples:

- "This project has been idle for three days; the last blocker was validation
  failure in the install step. The smallest next task is to rerun the fixed
  command."
- "The last session produced a memory candidate about pnpm version mismatch.
  Review it before the next install task."
- "The MVP scope now includes login and cloud sync, but the current project has
  no persistence layer. Consider cutting login from the first milestone."

Bad pulse examples:

- generic encouragement;
- random check-ins without project state;
- emotional diary updates;
- automatic scope expansion;
- reminders that cannot be traced to project memory, task state, or validation
  evidence.

Project Pulse should be user-configurable and easy to disable.

Pulse should be pull-first before push-first.

Early product surfaces should be commands or panels such as:

- "show next project step";
- "resume last project state";
- "generate today's smallest progress task";
- "explain why this project stalled";
- "review pending memory proposals."

Push reminders can come later, after project memory and execution reports are
stable enough to make reminders useful rather than annoying.

## 12. Evolution Without Losing Control

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

Separate memory evolution from behavior evolution:

```text
Memory can evolve frequently with scoped evidence.
Behavior should evolve slowly with review and eval proof.
```

Memory evolution can record:

- user preferences;
- project decisions;
- failed approaches;
- environment constraints;
- validation commands;
- known local setup facts.

Capability evolution is higher risk. New skills, new global rules, new workflow
steps, new profile triggers, or new eval cases should require review, evidence,
or a passing eval path before becoming default behavior.

Example user-visible proposal:

```text
Proposed project memory:
This project should check pnpm version before dependency installation.

Evidence:
The last install task failed because the active pnpm version did not match the
workspace expectation.

Scope:
Current project only.

Action:
Accept, edit, reject, or keep as task-local note.
```

The agent can feel like it is learning, but the user and the runtime can still
see exactly what changed and why.

## 13. Example Flow: Vague Idea To Local Tool

User:

```text
I want a small tool to record bacterial strains and phage information.
```

Partner Layer:

- recognizes a local research-tool task;
- asks only route-changing questions, such as fields, export needs, and whether
  the MVP should stay local;
- defaults away from login, cloud sync, permissions, and collaboration unless
  the user explicitly needs them;
- produces a TaskContract with assumptions and acceptance criteria.

TaskContract:

```yaml
task_type: code_change
objective: create a local web tool for recording strains and phage entries
assumptions:
  - assumption: first version is local-only with no login or cloud sync
    source: partner_inferred
    confidence: high
  - assumption: user prefers simple operation over full backend complexity
    source: partner_inferred
    confidence: medium
acceptance_criteria:
  - add entries
  - search entries
  - persist locally
  - export data
validation:
  required_commands:
    - npm test or equivalent project check
```

Execution Layer:

- creates or edits the project files inside the allowed scope;
- runs the required checks;
- returns an ExecutionReport with changed files, validation status, and any
  known limitations.

Partner Layer:

- explains what was built in ordinary language;
- tells the user how to run it;
- suggests the smallest next improvement;
- proposes memory such as "for this project, prefer simple local tools before
  login/cloud features," with evidence and project-only scope.

## 14. External Reference Lessons: OpenClaw and Hermes

OpenClaw and Hermes are useful references because they made the same promise
legible to users from different angles:

- OpenClaw made the promise emotional and visible: a local assistant that lives
  on the user's devices, channels, voice surfaces, workspace files, skills, and
  heartbeat loop.
- Hermes made the promise operational: a self-improving agent with bounded
  memory, searchable sessions, progressive-disclosure skills, cron, model
  switching, approvals, security layers, and migration from OpenClaw assets.

The lesson is not to copy their whole surface area. The lesson is to make
continuity, growth, and execution trust visible as product primitives.

Source discipline matters here. Official docs should be used for capability and
architecture facts. Reddit, Chinese guides, marketing comparisons, and media
articles are useful mainly for adoption signals, pain points, and messaging
language, not as proof that a mechanism is safe or robust.

### 14.1 What OpenClaw Did Right

OpenClaw's official positioning is simple and memorable: a personal assistant
that runs on your devices, answers on channels you already use, supports voice
and live surfaces, and treats the gateway as the control plane rather than the
product itself
([OpenClaw README](https://github.com/openclaw/openclaw)).

The strongest product choices are:

- **Local-first identity.** Users can understand "my assistant on my machine"
  faster than a generic agent framework.
- **Many surfaces, one assistant.** Messaging, desktop, voice, canvas, nodes,
  and web chat create the feeling that the agent is present in the user's life.
- **Inspectable agent assets.** `SOUL.md`, `AGENTS.md`, `TOOLS.md`,
  `HEARTBEAT.md`, and `skills/<skill>/SKILL.md` turn hidden behavior into files
  that users can read, edit, copy, and share.
- **Heartbeat as product memory.** Heartbeat gives the system a visible
  "still watching the project" loop, but the docs also show the right restraint:
  `target: "none"` can run without outbound messages, OK-only acknowledgments
  can be suppressed, `lightContext` can reduce context, and `isolatedSession`
  can avoid sending full chat history
  ([OpenClaw heartbeat](https://docs.openclaw.ai/gateway/heartbeat)).
- **Skills as community currency.** ClawHub and workspace skills make workflows
  portable; skill allowlists and load-time filters keep that ecosystem from
  becoming one giant prompt
  ([OpenClaw skills](https://docs.openclaw.ai/tools/skills)).
- **Security is part of the product story.** OpenClaw explicitly says personal
  assistant deployment is a one-trust-boundary model, recommends smallest
  access first, and documents sandboxing plus security audit checks
  ([OpenClaw security](https://docs.openclaw.ai/gateway/security),
  [OpenClaw sandboxing](https://docs.openclaw.ai/gateway/sandboxing)).

The community and media reaction also explain the viral loop: the "lobster"
identity created meme value, GitHub/social/media attention reinforced it, and
community-written tutorials/tools let the project spread beyond the original
repo. Examples include Chinese media/self-media framing OpenClaw as the move
from "chat" to "doing work," PANews describing the "lobster phenomenon" as a
mix of meme culture and community self-organization, and Reddit discussions
splitting OpenClaw's raw controllability from Hermes' smoother productization
([TechRadar](https://www.techradar.com/pro/why-is-openclaw-so-popular-in-china),
[PANews](https://www.panewslab.com/zh/articles/019cbdca-2361-7748-adf2-01b5f575bbe8),
[Reddit comparison thread](https://www.reddit.com/r/openclaw/comments/1swc620/openclaw_vs_hermes/)).
That is a product lesson: the agent needs a shareable object, not just a feature
list.

For Priority Agent, the borrowable part is the inspectable local identity:
Project Soul, Project Memory, Project Pulse, and project skills should feel like
real local assets. The non-borrowable part is broad surface sprawl before the
strict coding/project loop is proven.

### 14.2 What Hermes Did Right

Hermes makes the "agent that grows with you" claim more concrete. Its README
claims a built-in learning loop: create skills from experience, improve them
during use, persist knowledge, search past conversations, and build a user model
across sessions
([Hermes README](https://github.com/NousResearch/hermes-agent)).

The strongest engineering choices are:

- **Bounded memory.** Hermes documents small `MEMORY.md` and `USER.md` limits,
  frozen prompt snapshots, consolidation pressure, and clear save/skip guidance
  ([Hermes memory](https://hermes-agent.nousresearch.com/docs/user-guide/features/memory)).
- **Procedural memory via skills.** Skills are loaded on demand through
  progressive disclosure instead of stuffing all procedures into every prompt
  ([Hermes skills](https://hermes-agent.nousresearch.com/docs/user-guide/features/skills)).
- **One runtime across surfaces.** CLI and messaging platforms share commands,
  sessions, skills, and model switching; users can move between terminal and
  chat without mentally switching products.
- **Migration as adoption leverage.** Hermes can import OpenClaw `SOUL.md`,
  memories, skills, command allowlists, messaging settings, API keys, TTS
  assets, and workspace instructions. That turns competitor momentum into an
  onboarding path.
- **Safety is layered.** Hermes presents approval modes, dangerous-command
  patterns, user authorization, container isolation, MCP credential filtering,
  context-file scanning, cross-session isolation, and input sanitization as one
  defense-in-depth model
  ([Hermes security](https://hermes-agent.nousresearch.com/docs/user-guide/security)).
- **Cron is powerful but fenced.** Scheduled tasks can deliver to chats, local
  files, or platform targets, but cron sessions cannot recursively create more
  cron jobs
  ([Hermes cron](https://hermes-agent.nousresearch.com/docs/user-guide/features/cron)).

Community discussion around Hermes is useful because it surfaces both the draw
and the skepticism. Users praise smoother setup, memory depth, skills, and
"works out of the box" UX, but they also complain about tiny memory, snapshot
lag, self-learning that still needs review, opaque abstraction, and cost growth
when context accumulates
([Reddit skills discussion](https://www.reddit.com/r/hermesagent/comments/1smlqdt/how_skills_work_in_hermes_agent/),
[Hermes review](https://utilo.io/en/home/blog/hermes-agent-review-2026),
[Hermes Chinese guide](https://hermes.cocoloop.cn/hermes-agent-guide/index.html)).

For Priority Agent, the borrowable part is the disciplined memory/skill loop:
memory can be compact and evidence-backed; procedures can become reviewed
project skills; skills should load only when relevant. The non-borrowable part
is automatic behavior evolution without eval proof.

### 14.3 What This Changes For Our Design

The current alignment doc is pointed in the right direction. The external
references suggest tightening five places:

1. **Make local state inspectable.** Project Soul, Project Memory, Project
   Pulse, TaskContract history, and accepted project skills should have obvious
   local representations. Users should be able to see what the agent believes
   and what it learned.
2. **Treat skills as reviewed procedural memory.** A repeated successful
   workflow can propose a project skill, but the proposal needs provenance,
   diff, scanner output, owner approval, and ideally an eval or replay case.
3. **Keep pulse quiet by default.** Project Pulse should start as pull-first
   project status. Push behavior should require explicit opt-in, active hours,
   OK suppression, small context, and no direct memory writes from external
   untrusted content.
4. **Productize setup and diagnostics.** The difference between a research
   agent and a product is often `doctor`, status, config validation, permission
   explanation, migration preview, rollback, and clear reports.
5. **Avoid surface-area inflation.** OpenClaw and Hermes win attention with
   many channels, but Priority Agent should first win a narrower claim:
   long-term project landing with rigorous local execution.

This leads to a sharper product line:

```text
Not another coding agent.
A local project partner that remembers the project, turns vague intent into a
typed execution contract, executes through a verifiable runtime, and evolves
only from reviewed evidence.
```

### 14.4 Extra Guardrails From The Reference Systems

Add these constraints to the design backlog:

- **Source provenance everywhere.** Memory, skills, assumptions, and pulse facts
  must record whether they came from explicit user instruction, repo evidence,
  execution proof, external channel content, or partner inference.
- **Memory is not a dump.** Compact facts and decisions belong in memory; long
  transcripts, raw logs, and stale plans belong in searchable history or reports.
- **Heartbeat must not pollute foreground context.** A pulse run should use a
  scoped ContextPack, write proposals rather than memory, and never let
  untrusted external content silently alter project behavior.
- **Skill evolution needs a quarantine path.** Suspicious skill proposals should
  be retained for review but not loaded or applied.
- **Migration is a later growth lever.** After MVP-0 works, consider importing
  project facts from existing Codex/Claude/OpenClaw/Hermes files, but only
  through a previewable migration plan.
- **Cost is part of alignment.** Context budgets and profile routing are not
  only safety features; they also keep the product usable on weaker and cheaper
  models.

## 15. Current Repo Fit

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

## 16. Recommended Implementation Path

### MVP-0: Prove the smallest loop first

Do not start with a desktop companion, multi-agent orchestration, project
heartbeat automation, or a rich Soul UI.

MVP-0 should prove only this loop:

```text
natural user request
  -> Partner framing
  -> TaskContract
  -> ContextPack
  -> existing executor
  -> ExecutionReport
  -> Partner explanation
  -> memory proposal, not auto-write
```

MVP-0 includes:

- Partner prompt/policy for framing and assumptions;
- TaskContract generation in Markdown/JSON;
- ContextPack materialization with budgets;
- existing executor consuming the contract;
- ExecutionReport returned from real tool evidence;
- memory proposal generated but not written without a gate.

MVP-0 excludes:

- desktop avatar or companion UI;
- push heartbeat;
- autonomous self-editing;
- many specialized subagents;
- global behavior changes from one task.

Implementation progress on 2026-05-25:

- Added typed runtime projections for `TaskContract`, `ContextPack`, and
  `ExecutionReport`.
- Materialized `task.contract` and `context.pack` trace events from the existing
  `TaskContextBundle`.
- Materialized `execution.report` from closeout evidence without creating a
  second agent loop.
- Added the first profile selector for `standard`, `constrained`,
  `review_required`, and `human_confirm` based on current risk, verification,
  stop-check, uncertainty, and action-score signals.
- Injected compact `TaskContract` and `ContextPack` zones into executor requests
  for contract-worthy tasks, so the executor consumes the typed handoff instead
  of relying only on informal task-state prose.
- Connected `model_profile` to route/phase-scoped tool exposure: `standard`
  keeps the existing surface, `constrained` and `review_required` remove broad
  mutation/environment tools, and `human_confirm` stays read/inspect/confirm
  until a safer handoff is available.
- Added a review-only `MemoryProposal` surface derived from `ExecutionReport`;
  it can propose `successful_fix` or `failure_pattern` candidates with evidence
  but records `write_performed=false`.
- Added `memory.proposal` trace output and closeout memory boundary diagnostics
  so future memory-review UI can distinguish proposal generation from legacy
  heuristic/LLM memory sync.
- Added focused tests for assumptions, scope, validation commands, context
  budgets, executor context injection, weak-model profiles, profile-scoped tool
  exposure, execution report status mapping, and review-only memory proposals.

### Phase 0: Document the contract shape

Deliverables:

- add typed `TaskContract`, `ContextPack`, and `ExecutionReport` design notes;
- map each field to existing runtime owners;
- identify which fields are already traceable and which are only implied.
- include assumption provenance, context budgets, and authority ordering.

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

### Phase 4: Add Project Soul and Project Memory surfaces

Deliverables:

- compact project soul template;
- memory review surface for accepted, rejected, edited, and stale records;
- closeout-time memory proposal flow;
- scoped memory records with evidence and freshness.

Acceptance:

- users can inspect why the agent remembers something;
- execution context still excludes unrelated soul and memory;
- global behavior changes remain gated by review or eval evidence.

### Phase 5: Project heartbeat later, not first

Deliverables:

- only after task contracts and reports are stable, add project-level heartbeat
  suggestions based on real project state.
- start pull-first through explicit commands/panels before push reminders.

Acceptance:

- off by default or user-configurable;
- never random companionship;
- always tied to a concrete project next step, stale task, failed validation, or
  risk.

### Phase 6: Demo the differentiator

Deliverables:

- demo 1: vague idea to local MVP with scope control and task contract;
- demo 2: resume a project days later using project memory and previous
  execution reports;
- demo 3: fail, learn a scoped lesson, rerun with the lesson applied, and show
  the memory proposal.

Acceptance:

- the demos show long-term partner behavior and rigorous execution evidence;
- no demo depends on hidden prompt magic;
- each demo has a replayable trace or benchmark artifact.

## 17. Eval Additions

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

Add product-differentiation evals:

- a fuzzy non-programmer request must become a small task contract before file
  changes;
- assumptions must distinguish user-explicit facts from partner inferences;
- context budget must exclude low-relevance memory even when memory exists;
- authority order must block project memory or partner inference from
  overriding current user instruction or task scope;
- a resumed project must cite prior memory and execution evidence, not invent
  project state;
- a memory proposal must include source evidence and scope;
- a pulse suggestion must be traceable to project state, stale task, failed
  validation, or a known decision;
- a project-soul instruction must affect partner-layer behavior without leaking
  unrelated persona text into execution context.

## 18. Non-goals

- Do not create a separate Soul runtime that can call tools independently.
- Do not pass full personality or long-term memory into execution.
- Do not solve weak models by adding broad permanent prompt rules.
- Do not split into many subagents before one executor contract is proven.
- Do not weaken validation, checkpointing, permissions, or high-risk gates to
  make a weak provider pass.
- Do not market the product as a broad Claude Code replacement.
- Do not sell "soul" as a substitute for proof, validation, and clear project
  progress.
- Do not let self-evolution automatically rewrite global behavior from one
  failed task.
- Do not make Project Pulse a generic reminder or companionship feature.
- Do not enforce runtime-enforceable constraints only through Soul text.
- Do not skip the small MVP-0 loop by starting with broad desktop or heartbeat
  features.
- Do not copy OpenClaw/Hermes channel breadth before proving the narrow local
  project loop.
- Do not let Project Pulse, imported memories, or generated skills change agent
  behavior without provenance and review gates.

## 19. Open Questions

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
6. What is the minimum Project Soul template that feels personal but stays short
   enough to keep runtime context clean?
7. Should Project Pulse ship as a CLI command first, a desktop card first, or an
   automation only after project memory is stable?
8. Which assumptions should be shown to the user before execution, and which can
   remain in trace/report only?
9. What default ContextPack budgets preserve enough memory value without making
   weak models worse?
10. Should authority conflicts become hard stops, automatic profile downgrades,
    or user clarification prompts?
11. What local files should become the canonical inspectable surfaces for
    Project Soul, Project Memory, Project Pulse, TaskContract history, and
    reviewed project skills?
12. What minimum scanner/quarantine rules are required before generated skills
    can be proposed?

## 20. Bottom Line

Priority Agent should have a warm partner face and a strict execution spine.

The product can remember gex, understand the project, ask good questions, and
evolve over time. But file changes, shell commands, memory writes, validation,
and verified closeout must stay under typed runtime control.

The defensible product claim is not just "we can write code." It is:

```text
long-term project memory and partner guidance
+ rigorous local execution
+ evidence-backed evolution
```

That is the practical synthesis of the two notes:

```text
one partner relationship
one execution runtime spine
typed handoff contracts
profile-based weak-model tightening
evidence-gated memory and evolution
```

The next design step is to make the abstract boundary concrete:

```text
Task Routing Matrix
+ Authority Matrix
+ TaskContract assumptions
+ ContextPack budgets
+ MVP-0 loop
```
