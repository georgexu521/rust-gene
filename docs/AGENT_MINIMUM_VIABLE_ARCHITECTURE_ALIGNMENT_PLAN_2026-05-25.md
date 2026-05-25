# Agent Minimum Viable Architecture Alignment Plan

Date: 2026-05-25

Source note:
`/Users/georgexu/Downloads/11-agent_minimum_viable_architecture_notes.md`

Repo scope: `/Users/georgexu/Desktop/rust-agent`, current branch `claude`,
current head `f3c7d67f`.

Related active plans:

- `docs/AGENT_OBSERVER_ALIGNMENT_PLAN_2026-05-24.md`
- `docs/AGENT_STOP_FAILURE_RECOVERY_ALIGNMENT_PLAN_2026-05-24.md`
- `docs/AGENT_MEMORY_SYSTEM_ALIGNMENT_PLAN_2026-05-25.md`
- `docs/AGENT_WEIGHT_SYSTEM_ALIGNMENT_PLAN_2026-05-25.md`
- `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`
- `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`

## 0. What This Note Is Really About

The note describes the smallest loop that deserves to be called an Agent:

```text
user task
  -> route task
  -> initialize state
  -> build context
  -> ask LLM for next action candidates
  -> score and rank actions
  -> check permission and safety
  -> execute one tool/action
  -> observe the result
  -> update state
  -> stop/checkpoint/continue/close out
```

For this project, the note should not be interpreted as "build a first Agent
from scratch." Priority Agent already has much more than the minimum:
stateful routing, task state, tool exposure, permission review, checkpointing,
Observer, StopChecker, memory, action scoring, trace diagnostics, desktop run
diagnostics, and live-eval reporting.

The real lesson is architectural compression:

```text
The runtime should expose one understandable, testable minimal loop.
Advanced features can exist, but they must hang off that loop instead of
turning the control flow into a pile of parallel frameworks.
```

So this plan is not a feature-expansion plan. It is a loop-alignment plan: make
the existing modules behave and report like one clean minimum viable Agent
architecture.

## 1. Current Alignment

| Note module | Current project implementation | Alignment | Main concern |
| --- | --- | --- | --- |
| Task Router | `IntentRouter`, `TaskModeScore`, route risk/retrieval/reasoning, light/full/high-risk mode scoring. | Strong | Good foundation, but routing/mode/stage evidence is spread across traces and task state. |
| State Manager | `TaskContextBundle` and `AgentTaskState` track goal, mode, stage, scope, observations, findings, hypotheses, focus, risks, verification, stop checks, and action-score history. | Strong | State is rich, but there is no compact "MVA state contract" for the loop as a whole. |
| Context Builder | `PromptContextAssembler`, `ContextAssemblyPlan`, request preparation, context ledger hints, retrieval/memory injection, task-state zone. | Partial | Five zones exist, but only stable prefix + task state are rendered through the legacy prompt path; relevant material, recent observation, and current decision request are mostly diagnostic/ad hoc. |
| LLM Planner | The model returns normal assistant text and tool calls through `TurnModelStepController` and `TurnAssistantResponseController`. | Partial | The model proposes a chosen tool call, not a ranked candidate set, except for the gated candidate-action contract that is not wired into ordinary model steps. |
| Weight Scorer | `ActionDecision` computes value/risk/uncertainty/cost/reversibility/scope-fit/action-score with stage formulas and Observer/Memory modifiers. | Strong | Runtime scores the chosen action; true alternative ranking is still mostly future/gated. |
| Permission Checker | `ActionReview`, permission context, destructive scope checks, action policy, checkpoint policy, resource policy. | Strong | Safety gates are real; the plan should preserve these as runtime-owned, not prompt-owned. |
| Tool Executor | `ToolExecutionController`, tool lifecycle recovery, read-only concurrency, checkpoint-managed mutations, tool result normalization. | Strong | The minimum loop says "one action per round"; current runtime can execute batches and parallel read-only calls. That is useful, but hard to audit as the minimum loop. |
| Observer + StopChecker | `ToolObservation`, context ledger, `AgentTaskState`, `StopChecker`, stop traces, recovery plans, rollback candidates. | Strong | Stop/observe pieces exist, but the trace does not yet present a single loop-step record tying chosen action -> review -> observation -> state update -> stop decision together. |

## 2. What The Project Already Does Better Than The Note

- It has route-scoped and phase-scoped tool exposure, so the model does not
  always see the full tool surface.
- It has a richer stage model than the note:
  `Understand`, `Plan`, `Edit`, `Validate`, `Repair`, `Closeout`, `Done`.
- It has deterministic permission and destructive-scope enforcement instead of
  relying on prompt instructions.
- It has checkpoint-aware mutation boundaries and rollback candidate tracking.
- It has semantic Observer output with key findings, evidence, candidate focus,
  goal impact, uncertainty reduction, and model-visibility policy.
- It has action economics: `scope_fit`, final `action_score`, formula version,
  and bounded Observer/Memory score modifiers.
- It has verification proof and closeout status, including `partial` and
  `not_verified` paths when validation evidence is missing.
- It has live-eval/reporting hooks that can assert runtime-spine behavior.

This means the next optimization should be integration quality, not adding a
second "minimal agent" stack.

## 3. Remaining Problems

### P0. The minimal loop is implemented, but not represented as one contract

The current runtime has the loop pieces, but they are spread across:

- `src/engine/conversation_loop/turn_iteration_controller.rs`
- `src/engine/conversation_loop/turn_model_step_controller.rs`
- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/tool_result_controller.rs`
- `src/engine/action_decision.rs`
- `src/engine/action_review.rs`
- `src/engine/task_context.rs`
- `src/engine/stop_checker.rs`
- `src/engine/trace.rs`

The project has runtime-spine tests, but there is not yet one canonical
"AgentLoopStep" contract that shows, for each iteration:

```text
route/mode/stage
context zones used
candidate/chosen action
score/review result
permission/checkpoint decision
tool observation
state delta
stop decision
```

Risk: future features can attach to the loop in inconsistent places. The code
can still work, but architecture review becomes harder because there is no
single runtime artifact that proves the minimum loop stayed intact.

### P0. Candidate-action architecture is not wired end to end

The weight-system batch added:

- `src/engine/candidate_action.rs`
- `CandidateActionMode`
- candidate JSON parsing/ranking helpers
- `CandidateActionsEvaluated` trace event type
- `candidate_action_request` debug payload in `ActionReview`

But normal model interaction still works like this:

```text
model chooses tool_call -> runtime scores/reviews that chosen action
```

The note's architecture is:

```text
model proposes candidate_actions -> runtime scores/ranks -> selected action
```

The current project intentionally kept that off by default, which is correct
for product feel. The remaining gap is a gated integration path:

- shadow ranking when the runtime asks for candidate actions;
- gated replacement only after repeated action revision, repeated low action
  scores, high-risk low-value actions, or no uncertainty reduction;
- trace coverage for rejected candidates and selected runtime-ranked action.

Risk: without this, the runtime can reject weak actions, but it cannot yet
actively select the better alternative when the model is stuck.

### P0. Context zones exist, but the request path is still partially legacy

`ContextAssemblyPlan` names the five zones from the note:

```text
stable_prefix
task_state
relevant_material
recent_observation
current_decision_request
```

However, `render_legacy_system_prompt()` only emits stable prefix plus the task
state tail. Later, `RequestPreparationController` injects task state, context
ledger hints, focused repair prompts, and memory prefetch as separate system
messages.

That works, but it means the architecture is only partially realized:

- the zone names are traceable;
- `<task-state>` is injected;
- context ledger and memory are useful;
- but recent observations and relevant material are not first-class zones in
  the final model request.

Risk: the note's central idea, "Context Builder chooses the right current
material," can regress into scattered system-message injection.

### P1. Stage transitions are useful but decentralized

Stage changes currently happen in several places:

- context-ledger observation handling;
- tool-round observation;
- validation result handling;
- stop-check application;
- closeout logic;
- focused repair.

This is natural for a mature runtime, but the minimum architecture wants a
clear state manager answer to:

```text
why did the task move from diagnosis to implementation?
why did validation move back to repair?
why is finalization allowed now?
```

Risk: traces show events, but stage transitions do not have one canonical
policy object or transition event with before/after/reason/evidence.

### P1. Tool exposure is better than "too many tools," but there is no MVA audit mode

The note recommends a tiny first tool set:

```text
list_files, read_file, search_code, edit_file, run_tests, git_diff, final_answer
```

Priority Agent should keep its richer tool surface because this is a real
coding CLI. But for architecture validation, we need a narrow audit profile
that proves the minimum loop works without relying on advanced tools.

Current phase-scoped exposure still allows advanced capabilities in some
stages, for example git, format, dev server, dependency install, delegation,
MCP, memory, and shell depending on route/profile. That is fine for real use,
but it makes the minimum loop harder to test.

Risk: a live-eval can pass because an advanced recovery path saves the turn,
while the basic loop is still weak.

### P1. The runtime can execute batches, while the minimum loop is single-action

The note's first version executes one action per loop. Priority Agent supports
multiple tool calls and read-only concurrency. That is useful, but it means:

- one model response can contain several actions;
- one StopChecker decision may summarize a batch;
- one Observer/state update can combine several tool results;
- candidate ranking cannot cleanly select one action unless it sits before
  tool execution.

Risk: batch execution makes loop evidence less crisp and can hide which action
actually reduced uncertainty or caused failure.

### P1. Completion semantics are strong for code changes, but not unified as an MVA rule

The project has verification proof and closeout status, including partial and
not-verified states. That is stronger than the note.

The remaining gap is unification:

```text
direct answer -> explicit answer is enough
light task -> observation plus concise response is enough
full coding task -> diff or explicit no-diff outcome + verification attempt/proof
high risk -> explicit user decision or blocked/partial closeout
```

This rule exists in pieces across workflow closeout, StopChecker, evidence
ledger, direct-task behavior, and final response handling. It should become a
small shared completion contract and a trace/debug payload.

Risk: future routes can accidentally mark a full task as complete with a plain
assistant response and thin evidence.

### P2. Memory is more advanced than minimum, but the minimum memory contract is unclear

The note recommends simple memory:

```text
read user/project preferences at task start
write a task summary at task end
```

Priority Agent has richer memory retrieval, write gates, trust/conflict/stale
signals, retention, and score modifiers. That is aligned with the product
direction, but the minimum memory guarantee is not easy to test:

- was preference memory considered at the start?
- did stale/conflicting memory stay fenced?
- did task-end learning write only a useful summary?
- did memory affect action scoring only within bounded limits?

Risk: memory quality can be evaluated in specialized tests, while the minimum
loop has no simple "memory read/write happened at the right boundary" signal.

### P2. Live evals assert runtime pieces, but not the MVA loop as a scenario set

Existing live-eval reporting can assert many runtime-spine signals. The missing
piece is a small scenario suite that mirrors the note's core validation
questions:

```text
can the loop run through?
can the model propose/select a stable next action?
does scoring reduce bad actions?
does permission stop high-risk actions?
does Observer create useful next-state evidence?
does StopChecker prevent repeated dead loops?
```

Risk: we can keep adding powerful subsystems without a small evergreen test
that says the basic Agent loop remains healthy.

## 4. Recommended Direction

Do not build a new "minimal agent" runtime beside the current one.

Instead, create a thin MVA contract layer over the current runtime:

```text
existing route/state/context/model/tool/observer/stop modules
  -> small AgentLoopStep diagnostic contract
  -> gated candidate-action path
  -> materialized context zones
  -> MVA audit tool profile and eval set
```

This keeps the product direction intact: narrow, deep, personal, and
verifiable. It also avoids reintroducing prompt-heavy workflow control.

## 4.1 Implementation Status

2026-05-25 implementation batch completed the MVA alignment slice without
creating a second agent loop.

Phase coverage: Phase 0 through Phase 8 are implemented in this batch.

Implemented:

- Added `AgentLoopStepEvaluated` trace events so each tool round can show the
  route, mode, stage before/after, exposed tool count, selected tool-call
  count, action-score history, observation/state delta, and StopChecker result
  in one compact runtime artifact.
- Added `ContextZonesMaterialized` trace events and moved dynamic request
  material toward the five-zone shape:
  `<task-state>`, `<relevant_material>`, `<recent_observation>`, and the user
  message as current decision request, while keeping the stable prefix
  cacheable.
- Wrapped memory/retrieval prefetch and context-ledger hints as
  relevant/recent-observation material instead of a generic untyped hint.
- Wired `PRIORITY_AGENT_CANDIDATE_ACTIONS=off|shadow|gated` into the model-step
  path. Shadow/gated mode ranks parsed candidate JSON when present, otherwise
  ranks the model's proposed tool calls; gated replacement only narrows to the
  runtime-selected existing tool call when score/no-progress state indicates the
  model is stuck.
- Added an MVA audit tool profile with `PRIORITY_AGENT_MVA_AUDIT_TOOLS=1`,
  preserving normal product tools while letting evals exercise a small
  list/read/search/edit/test/diff/ask surface.
- Added `TaskStageTransition` history in `AgentTaskState` and routed the main
  stage changes through a transition helper with source/reason/evidence counts.
- Added `CompletionContractEvaluated` trace/runtime diagnostic output so direct,
  light, full coding, and high-risk completion can be checked as a mode-aware
  contract instead of only inferred from the final answer.
- Extended live-eval parser/reporting metrics with context-zone, agent-loop,
  stage-transition, candidate-ranking, and completion-contract signals.
- Added six `evalsets/live_tasks/minimum-agent-*.yaml` cases for direct answer,
  light inspection, full edit, verification repair, high-risk block, and
  low-value search stop/replan coverage.

Validation for this batch should include:

```bash
cargo check -q
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q turn_model_step_controller -- --test-threads=1
cargo test -q tool_exposure_plan -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q stop_checker -- --test-threads=1
cargo test -q turn_completion_controller -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q candidate_action -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh
cargo fmt --check
cargo clippy --all-features -- -D warnings
```

## 5. Implementation Plan

### Phase 0 - Freeze the current MVA map

Purpose: make the existing architecture reviewable before changing behavior.

Tasks:

1. Add a compact runtime map in this document's implementation section or a
   follow-up `docs/AGENT_RUNTIME_MVA_MAP_2026-05-25.md`.
2. Map each note module to its current code owner:
   router, state, context, planner, scorer, permission, executor, observer,
   stop, closeout.
3. Add a short "do not duplicate these modules" note to prevent a parallel
   agent-loop subsystem.
4. Record current behavior for direct, light, full, and high-risk mode scoring.

Likely files:

- `docs/AGENT_MINIMUM_VIABLE_ARCHITECTURE_ALIGNMENT_PLAN_2026-05-25.md`
- `docs/PROJECT_STATUS.md` only if implementation materially changes the
  validated baseline.

Validation:

```bash
cargo test -q task_mode_score -- --test-threads=1
cargo test -q lightweight_planner -- --test-threads=1
```

### Phase 1 - Add an `AgentLoopStep` diagnostic contract

Purpose: expose one per-iteration artifact that proves the minimum loop ran.

Tasks:

1. Add a small serializable loop-step diagnostic object with:
   - route intent/workflow/risk;
   - task mode and stage before/after;
   - exposed tool count and phase-scoped tool count;
   - selected tool call count;
   - action score/review decision summary;
   - permission/checkpoint/destructive-scope summary;
   - observation count/key-finding count;
   - state delta summary;
   - stop decision.
2. Record it after each tool round, using existing trace/state data.
3. Add a `TraceEvent` such as `AgentLoopStepEvaluated`.
4. Surface a compact version in `/trace` runtime-spine summary and live-eval
   parser metrics.

Likely files:

- `src/engine/trace.rs`
- `src/engine/conversation_loop/turn_iteration_controller.rs`
- `src/engine/conversation_loop/turn_recording.rs`
- `scripts/live_eval_report_parser.py`

Validation:

```bash
cargo test -q trace -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
```

### Phase 2 - Materialize the five context zones in the actual request path

Purpose: make Context Builder match the note's architecture instead of only
reporting the zone names.

Tasks:

1. Keep the stable prefix cacheable.
2. Move current `<task-state>` into the `task_state` zone explicitly.
3. Move selected context-ledger evidence into `relevant_material` or
   `recent_observation` instead of a generic ad hoc system hint.
4. Move memory/retrieval prefetch into a bounded `relevant_material` sub-block
   with provenance/trust/conflict summaries.
5. Keep focused repair prompts as targeted dynamic decision-request material.
6. Add request-budget metrics by zone:
   stable prefix tokens, task-state tokens, relevant-material tokens,
   recent-observation tokens, current-decision-request tokens.

Important constraint:

- Do not make every turn output JSON.
- Do not add a heavy planner prompt to normal turns.
- The zones should structure context, not turn the model into a form filler.

Likely files:

- `src/engine/context_assembly.rs`
- `src/engine/prompt_context.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/conversation_loop/context_budget_controller.rs`
- `src/engine/runtime_spine_behavior_tests.rs`

Validation:

```bash
cargo test -q prompt_context -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q context_budget_controller -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
```

### Phase 3 - Wire candidate-action ranking behind the existing gate

Purpose: complete the note's LLM Planner + Weight Scorer shape without forcing
candidate JSON every turn.

Tasks:

1. Keep `PRIORITY_AGENT_CANDIDATE_ACTIONS=off` as the default.
2. In `shadow` mode, when `ActionReview` emits a triggered
   `candidate_action_request`, ask the model for up to three candidate actions
   before the next tool round.
3. Parse candidates with `parse_candidate_actions`.
4. Rank candidates with `rank_candidate_actions` against exposed tools and the
   current `ActionDecisionInput`.
5. Record `CandidateActionsEvaluated` with rejected reasons.
6. In `gated` mode, allow the runtime-ranked selected candidate to replace the
   next tool call only when the current stop/review reason is score-driven:
   repeated low score, low scope fit, high-risk low-value, or no uncertainty
   reduction.
7. Add explicit fallback behavior when candidate JSON is absent or invalid:
   continue normal tool-call path and record a protocol failure, not a user
   visible crash.

Likely files:

- `src/engine/candidate_action.rs`
- `src/engine/action_review.rs`
- `src/engine/conversation_loop/turn_model_step_controller.rs`
- `src/engine/conversation_loop/assistant_response_retry_controller.rs`
- `src/engine/conversation_loop/turn_iteration_controller.rs`
- `src/engine/trace.rs`
- `scripts/live_eval_report_parser.py`

Validation:

```bash
cargo test -q candidate_action -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
```

### Phase 4 - Add an MVA audit tool profile

Purpose: test the minimum architecture with a small first-version tool surface
without reducing the normal product surface.

Tasks:

1. Add a profile or env flag for MVA audit mode.
2. Map note tools to current tool names:
   - `list_files` -> `glob` / `project_list`
   - `read_file` -> `file_read`
   - `search_code` -> `grep`
   - `edit_file` -> `file_edit` / `file_patch`
   - `run_tests` -> `run_tests` / focused safe `bash`
   - `git_diff` -> `git_diff` / `diff`
   - `final_answer` -> model closeout, not a tool
3. In audit mode, hide deploy/network/database/email/browser/MCP/delegation and
   broad shell actions unless the route is explicitly testing those systems.
4. Keep permission and checkpoint gates active.
5. Add route/phase exposure tests for the audit profile.

Likely files:

- `src/tools/mod.rs`
- `src/engine/conversation_loop/tool_orchestrator.rs`
- `src/engine/conversation_loop/tool_exposure_plan.rs`
- `src/engine/tool_exposure.rs`
- `src/engine/conversation_loop/route_scoped_tools_tests.rs`

Validation:

```bash
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q tool_exposure -- --test-threads=1
cargo test -q tool_exposure_plan -- --test-threads=1
```

### Phase 5 - Centralize stage transition reasoning

Purpose: make state transitions understandable and auditable.

Tasks:

1. Add a small transition helper that records:
   - previous stage;
   - next stage;
   - reason;
   - source event;
   - evidence count or summary.
2. Route existing stage changes through this helper where practical.
3. Keep stage names as the current richer set:
   `Understand`, `Plan`, `Edit`, `Validate`, `Repair`, `Closeout`, `Done`.
4. Add a display mapping to the note's four stage families:
   - diagnosis: `Understand`, `Plan`
   - implementation: `Edit`, `Repair`
   - verification: `Validate`
   - finalization: `Closeout`, `Done`
5. Add trace summary for stage transitions.

Likely files:

- `src/engine/task_context.rs`
- `src/engine/trace.rs`
- `src/engine/conversation_loop/turn_iteration_controller.rs`
- `src/engine/runtime_spine_behavior_tests.rs`

Validation:

```bash
cargo test -q task_context -- --test-threads=1
cargo test -q stop_checker -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
```

### Phase 6 - Add a unified completion contract

Purpose: make "done / partial / blocked / needs user" consistent across modes.

Tasks:

1. Add a shared completion evaluator for:
   - direct answer;
   - light task;
   - full coding task;
   - high-risk task.
2. For full coding tasks, require one of:
   - changed files plus verification proof;
   - no-diff audit with explicit required-validation proof;
   - partial/not-verified closeout with the missing evidence stated.
3. For high-risk tasks, require user decision evidence or blocked/needs-user
   terminal status.
4. Ensure plain assistant final responses on full/high-risk routes are checked
   against this contract before being marked complete.
5. Record the completion contract result in trace and runtime diagnostics.

Likely files:

- `src/engine/conversation_loop/turn_assistant_response_controller.rs`
- `src/engine/conversation_loop/turn_completion_controller.rs`
- `src/engine/conversation_loop/closeout_controller.rs`
- `src/engine/verification_proof.rs`
- `src/engine/trace.rs`

Validation:

```bash
cargo test -q closeout -- --test-threads=1
cargo test -q turn_completion_controller -- --test-threads=1
cargo test -q direct_task_behavior -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
```

### Phase 7 - Add minimum-loop eval coverage

Purpose: validate the note's "first version" questions as a stable regression
suite.

Tasks:

1. Add a small eval set for:
   - direct answer with no tools;
   - light local inspection;
   - full small code edit;
   - verification failure then repair;
   - high-risk destructive action requiring user/blocked state;
   - repeated low-value action causing replan/stop.
2. Add required runtime assertions:
   - task routed;
   - task state initialized;
   - context zones recorded;
   - action score recorded;
   - action review recorded;
   - tool observation recorded;
   - state transition recorded;
   - stop check recorded;
   - completion contract recorded.
3. Add parser aliases and aggregate summary fields for MVA health.

Likely files:

- `evalsets/live_tasks/*.yaml`
- `scripts/live_eval_report_parser.py`
- `scripts/run_live_eval.sh`
- `scripts/live-eval-aggregate-summary.sh`

Validation:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh
```

Optional live gate:

```bash
bash scripts/run_live_eval.sh --case minimum-agent-loop --mode summary
```

### Phase 8 - Refresh docs and status

Purpose: keep the repo's durable architecture story accurate after the
implementation phases.

Tasks:

1. Add an implementation status section to this document.
2. Refresh `docs/PROJECT_STATUS.md` only after the code and tests actually
   land.
3. Link the MVA loop contract from runtime-spine docs or trace/debug docs.
4. Record which behavior is default and which behavior is gated by env flags.

Validation:

```bash
cargo fmt --check
cargo check -q
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
```

## 6. Acceptance Criteria

This plan is complete when all of the following are true:

- A trace/debug artifact can show one complete loop step from route to stop.
- Context zones are materialized in the actual request path, not only in a
  report object.
- Candidate-action ranking is wired in shadow/gated mode and remains off by
  default for normal turns.
- An MVA audit profile can run with the small first-version tool surface.
- Stage transitions carry before/after/reason/evidence.
- Completion status is mode-aware and cannot silently mark full/high-risk work
  complete with thin evidence.
- A small minimum-loop eval set asserts router, state, context, scorer,
  permission, executor, observer, stop, and completion signals together.

## 7. Explicit Non-Goals

- Do not replace the existing conversation loop with a second minimal loop.
- Do not force JSON candidate actions on every normal turn.
- Do not remove advanced tools from the real product surface.
- Do not simplify memory by deleting the current trust/conflict/write-gate
  work; instead, add a small boundary contract that proves memory read/write
  happens at the right time.
- Do not make prompts heavier. This plan should move evidence into runtime
  contracts and traces, not into always-on model instructions.

## 8. Priority Order

Recommended implementation order:

1. Phase 1: `AgentLoopStep` diagnostic contract.
2. Phase 2: real five-zone context materialization.
3. Phase 3: gated candidate-action ranking integration.
4. Phase 5: centralized stage transition reasoning.
5. Phase 6: unified completion contract.
6. Phase 7: minimum-loop eval coverage.
7. Phase 4: MVA audit tool profile, if the eval work shows too much advanced
   tool leakage.
8. Phase 8: docs/status refresh.

Reason: the first three phases expose the current loop clearly and complete
the note's main architecture. Stage/completion/eval work then makes the loop
harder to regress. The audit profile is useful, but it should be driven by
evidence from the loop diagnostics instead of introduced as premature
restriction.
