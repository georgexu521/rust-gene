# LabRun And General Agent Integration Plan
Status: Implemented P0/P1 slice; P2 proof/trace added; Postdoc audit deferred
Created: 2026-06-24

## Purpose

This document turns the latest external runtime review into a focused next
development plan for Priority Agent.

The review does not argue that the project needs a new agent architecture. The
main takeaway is the opposite: the general coding-agent runtime is already a
real closed loop, and LabRun is already a useful long-running project
governance layer. The next improvement should make the two modes feel more
coherent to the user and make LabRun's state, next actions, and evidence easier
to act on from normal agent turns.

## Executive Conclusion

The review is useful and mostly correct.

Current state:

- The general agent mode is a valid programming agent, not just an LLM with a
  shell tool. It has routing, context assembly, route-scoped tools, permission
  review, ordered tool execution, observations, repair, verification proof,
  claim gating, trace, memory, and closeout.
- LabRun is also valid, but it should be named accurately: it is a structured,
  auditable, long-running project workflow with agent execution, not an
  autonomous theatrical swarm.
- The best next step is not to rewrite the shared runtime. It is to improve the
  seam between normal full-agent turns and LabRun state.

Important correction from repo inspection:

- LabRun context injection is not missing from the runtime. `RequestPreparation`
  already injects a `<lab-context>` block when `lab_context_enabled` is true,
  and tests cover stage, artifact/gate evidence refs, compression decisions,
  and current LabRun IDs.
- The remaining issue is product quality: the injected LabRun context should
  become more actionable, and LabRun should tell users the next safe command or
  state transition instead of making them remember a large command surface.

## Implementation Progress

Completed in the first implementation slice:

- Added deterministic LabRun next-action recommendations in
  `src/lab/next_action.rs`.
- Added `/lab next` and `/lab next --json`.
- Reused the same recommendation in `<lab-context>` injection as a compact
  `next_safe_actions` block.
- Added LabRun help text that distinguishes plain Lab Mode text, `/lab`
  commands, provider-backed `/lab step llm`, and scoped `/lab task run`.
- Added stage-aware tool exposure advisory in
  `TurnIterationSetupController`. This records recommended/missing tools by
  task stage without filtering tools yet.
- Added user-facing permission presets:
  `fast-coding`, `safe-coding`, `review-only`, and `labrun`.
- Added `/lab proof` for persisted verification proof rollup.
- Added `/lab trace [limit]` for recent persisted LabRun event trace.

Explicitly deferred:

- Postdoc read-only code-aware audit is deferred to a separate risk-gated
  implementation. The first slice should define a `PostdocAuditTask`, a
  read-only tool contract, provider/tool execution boundaries, and an output
  schema before enabling it. Postdoc still must not mutate code.
- `/lab evidence graph` is deferred until `/lab proof` has been dogfooded. The
  proof view now exposes the underlying evidence needed for a later graph.

## What To Borrow From The Review

### 1. Preserve The Two-Layer Product Model

Priority Agent should keep two complementary modes:

- General agent mode: fast, direct, task-local coding loop.
- LabRun mode: slower, structured, evidence-based governance loop for long
  projects.

LabRun should continue to reuse the general agent for scoped graduate execution.
It should not fork a second tool runtime or invent a separate permission model.

### 2. Treat LabRun As Governance, Not Swarm Theater

The Professor/Postdoc/Graduate roles are valuable because they encode
responsibility boundaries:

- Professor: direction, risk, non-goals, strategic review.
- Postdoc: technical decomposition, integration, audit, synthesis.
- Graduate: narrow scoped implementation with required validation.

The value is not that three agents "chat". The value is stage, artifact, scope,
validation, review, evidence, and recoverability.

### 3. Make LabRun State Actionable In Normal Agent Turns

The runtime already has LabRun context injection, but the content should help
the model decide what to do next. A LabRun context packet should expose:

- current stage
- current owner
- gate requirement
- open graduate tasks
- blocker state
- latest artifact
- evidence refs
- meeting recommendation
- next safe actions
- recommended command

### 4. Add A LabRun Navigation Command

The `/lab` command surface is powerful but large. A user should not need to
memorize the command tree to make progress.

Add:

```text
/lab next
/lab next --json
```

This command should summarize the current LabRun state and recommend the next
safe command.

### 5. Make Tool Exposure More Stage-Aware

Route-level tool scoping already exists. The next refinement is stage-level
scoping in the iteration setup layer:

```text
Understand -> read/search tools
Edit       -> file write/edit/patch tools after sufficient evidence
Validate   -> bash/run_tests/git diff/status tools
Closeout   -> diff/trace/cost/session evidence tools
```

This should reduce premature writes, premature shell execution, and repeated
search churn without weakening the main loop.

### 6. Productize Permission Presets

The low-level permission modes already exist:

- `AutoAll`
- `AutoLowRisk`
- `ReadOnly`
- `Default`
- `Once`

The next product layer should expose user-facing presets:

- `fast-coding`: developer dogfood, maps to AutoAll plus existing high-risk
  confirmation.
- `safe-coding`: normal safer coding, maps to AutoLowRisk.
- `review-only`: audit/review sessions, maps to ReadOnly.
- `labrun`: stage/role-aware LabRun behavior.

This is a UX and policy composition layer, not a reason to weaken hard
permissions.

### 7. Add A Conservative Postdoc Code-Aware Audit Lane

Postdoc should stay conservative. It should not get broad write access. The
valuable upgrade is read-only code awareness after GraduateResult:

```text
PostdocAuditTask
  tools: file_read, grep, git_diff, validation-only bash
  input: GraduateResult + changed files + required validation + evidence refs
  output: PostdocIntegrationSummary
```

This lets Postdoc verify enough of the diff and evidence to produce a stronger
integration summary without becoming an uncontrolled coding agent.

### 8. Unify LabRun Evidence With Runtime Proof Surfaces

The general agent has trace, verification proof, evidence ledger, and closeout.
LabRun has artifacts, gates, evidence refs, reports, and graduate dispatch
records. They should be viewable together.

Potential commands:

```text
/lab proof
/lab trace
/lab evidence graph
```

This is useful, but it is lower priority than `/lab next` and context action
quality.

## Non-Goals

- Do not rewrite `RuntimeController`, `StreamingQueryEngine`, or
  `ConversationLoop`.
- Do not create a separate LabRun tool executor.
- Do not turn Professor/Postdoc/Graduate into an unbounded chat swarm.
- Do not weaken permissions, checkpoints, scope checks, or validation gates.
- Do not let Postdoc mutate code in the first audit implementation.
- Do not make `/lab next` silently advance state. It should recommend; state
  changes still go through explicit commands or bounded scheduler steps.

## P0: LabRun Usability And Context Actionability

### P0.1 Implement `/lab next`

Problem:

- `/lab help` exposes many commands.
- Users need a single "what should I do now?" command.
- The scheduler already has enough stage/gate/blocker state to produce a next
  recommendation.

Plan:

1. Add `/lab next`.
2. Add `/lab next --json`.
3. Compute a deterministic recommendation from the latest LabRun:
   - no run
   - proposal awaiting approval
   - paused/recoverable
   - needs user
   - blocked gate
   - queued graduate task
   - graduate result ready for integration
   - postdoc/professor review boundary
   - user report / closeout boundary
4. Include:
   - current stage
   - owner
   - run status
   - current blocker or missing gate item
   - recommended command
   - short reason
   - safe alternatives
5. Keep output deterministic and easy to test.

Example output:

```text
Current: graduate_work
Owner: Graduate
State: active
Next: /lab task run gradtask_123
Why: queued GraduateTask has scope and validation commands
Alternatives: /lab task envelope gradtask_123, /lab context graduate
```

Acceptance:

- `/lab next` works when no run exists.
- `/lab next` handles active, paused, blocked, needs-user, and completed states.
- `/lab next --json` produces stable machine-readable fields.
- Tests cover at least:
  - no LabRun
  - proposal pending approval
  - active run at professor gate
  - queued graduate task
  - blocked graduate stage

### P0.2 Enrich LabRun Context Injection With Next Safe Actions

Problem:

- LabRun context is already injected, but it is not yet action-oriented enough
  for normal full-agent turns.

Plan:

1. Reuse the same deterministic recommendation logic from `/lab next`.
2. Add a compact `next_safe_actions` section to the `<lab-context>` block.
3. Include only stable, high-signal fields:
   - recommended command
   - why this command is safe
   - current blocker
   - open task count
   - current gate requirement
4. Keep the context block bounded and token-conscious.
5. Record a trace event or context-zone fact that a LabRun next-action hint was
   injected.

Acceptance:

- Existing Lab context injection tests still pass.
- New test proves the injected `<lab-context>` contains a recommended next
  command when a LabRun has a queued/blocked/active next step.
- Context injection remains disabled when `lab_context_enabled` is false.

### P0.3 Document The LabRun Interaction Boundary

Problem:

- Users can reasonably confuse normal text turns, `/lab` commands, provider
  draft/review steps, strict scheduler steps, and graduate subagent execution.

Plan:

Add a concise section to LabRun docs and help text:

```text
Plain text in lab mode: normal full-agent turn with LabRun context.
/lab commands: deterministic LabRun state/artifact/scheduler operations.
/lab task run: scoped Graduate agent execution through existing AgentTool.
/lab step llm: provider-backed Professor/Postdoc artifact draft/review.
/lab run hybrid: provider stages plus strict graduate scheduler.
```

Acceptance:

- `/lab help` or `/lab next` explains these boundaries without making the help
  wall much longer.
- Docs explain that LabRun is an auditable governance workflow, not an
  autonomous swarm.

## P1: Runtime Policy Refinement

### P1.1 Add Stage-Aware Tool Exposure In Iteration Setup

Problem:

- Route-scoped tools are already useful, but the per-iteration exposure plan is
  still mostly "base tools plus recovery expansion".
- The runtime already tracks task stage. It can use that to reduce premature
  actions.

Plan:

1. Extend `TurnIterationSetupController` to receive route/workflow/task-stage
   context where needed.
2. Define stage tool groups:
   - Understand: `project_list`, `grep`, `glob`, `file_read`, `symbol_query`
   - Edit: `file_edit`, `file_write`, `file_patch`
   - Validate: `bash`, `run_tests`, `git_diff`, `git_status`, `format`
   - Closeout: `git_diff`, `git_status`, `trace`, `cost`, session evidence
3. Start advisory or test-gated before making it hard-blocking:
   - log intended stage exposure
   - compare with route exposure
   - add tests
4. Preserve route recovery that exposes safe read/search tools after a bad
   route.
5. Do not hide required validation tools during validation/repair.

Acceptance:

- Build/code-change routes still expose write tools at the right stage.
- Plan/review/explore modes keep write tools out unless explicitly switched.
- Repair rounds can still expose tools needed to fix a validated failure.
- Existing route-scoped tool tests pass.

### P1.2 Add User-Facing Permission Presets

Problem:

- Low-level permission modes exist, but users think in work styles.

Plan:

1. Add preset mapping:
   - `fast-coding` -> `AutoAll`
   - `safe-coding` -> `AutoLowRisk`
   - `review-only` -> `ReadOnly`
   - `labrun` -> stage/role-aware preset, initially conservative around
     graduate/postdoc boundaries
2. Expose presets in TUI/CLI status and permission help.
3. Keep existing explicit permission mode commands for advanced users.
4. Document that high-risk gates remain active under fast-coding.

Acceptance:

- Preset changes update the actual permission mode or policy overlay.
- Existing permission mode tests still pass.
- UI/status text distinguishes agent mode from permission preset.

## P1/P2: LabRun Review Strengthening

### P1.3 Add Optional Postdoc Read-Only Audit

Problem:

- Postdoc currently relies heavily on supplied artifacts/evidence.
- That is safe, but integration review can be stronger if Postdoc can inspect
  the actual diff and key files after GraduateResult.

Plan:

1. Add a Postdoc audit task type.
2. Restrict tools to read-only plus validation-only shell:
   - `file_read`
   - `grep`
   - `glob`
   - `git_diff`
   - selected validation commands
3. Feed in GraduateResult, changed files, allowed scope, validation summary,
   and evidence refs.
4. Output a PostdocIntegrationSummary or audit section.
5. Start opt-in; later consider default-on for high-risk graduate tasks.

Acceptance:

- Postdoc audit cannot write files.
- Audit output cites files/diff/validation evidence it actually inspected.
- Missing evidence produces a blocker or "not verified" audit status.

### P2.1 Add LabRun Proof And Trace Views

Problem:

- LabRun evidence and general runtime proof are both strong, but they are split
  across views and files.

Plan:

1. Add `/lab proof` as a concise proof rollup:
   - latest graduate dispatch
   - changed files
   - validation commands
   - artifact gate result
   - postdoc/professor review status
2. Add `/lab trace` or extend existing dashboard with runtime trace links.
3. Consider `/lab evidence graph` only after proof rollup is useful.

Acceptance:

- A user can see why a LabRun stage is verified, blocked, or not verified
  without opening multiple artifact files manually.
- The view uses existing persisted evidence rather than model claims.

## Suggested Execution Order

1. Build a small deterministic LabRun next-action recommender.
2. Wire it to `/lab next` and `/lab next --json`.
3. Reuse the recommender inside LabRun context injection.
4. Update LabRun help/docs to explain plain text vs `/lab` commands vs
   scheduler/provider/graduate paths.
5. Add stage-aware tool exposure as an advisory/tested controller change.
6. Add permission presets on top of existing permission modes.
7. Add optional Postdoc read-only audit.
8. Add LabRun proof/trace rollup.

## Validation Plan

Focused commands:

```bash
cargo fmt --check
git diff --check
cargo test -q lab::commands --lib
cargo test -q lab::context --lib
cargo test -q request_preparation_controller --lib
cargo test -q turn_iteration_setup_controller --lib
```

Broader commands after shared runtime/tool-policy changes:

```bash
cargo check -q
cargo check --features legacy-cli -q
cargo check --features experimental-api-server -q
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
bash scripts/validate_docs.sh
```

## Done Definition

This plan is complete when:

- `/lab next` and `/lab next --json` exist and are tested.
- LabRun context injection includes bounded next-action guidance.
- LabRun docs/help clearly explain normal full-agent turns vs `/lab` commands
  vs provider-backed steps vs graduate subagent execution.
- Stage-aware tool exposure has at least advisory implementation and tests.
- Permission presets are exposed without weakening existing permission modes.
- Postdoc audit is available as a read-only path or explicitly deferred with a
  tracked reason.
- LabRun proof/trace view is available or explicitly deferred after P0/P1
  completion.
