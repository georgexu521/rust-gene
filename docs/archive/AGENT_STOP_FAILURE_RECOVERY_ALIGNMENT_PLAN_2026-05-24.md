# Agent Stop Failure Recovery Alignment Plan

Date: 2026-05-24

Source note: `/Users/georgexu/Downloads/08-agent_stop_failure_recovery_notes.md`

Repo scope: `/Users/georgexu/Desktop/rust-agent`, current branch `claude`,
current head `f3c7d67f Harden checkpoints and action boundaries`.

Related active plan:
`docs/AGENT_OBSERVER_ALIGNMENT_PLAN_2026-05-24.md`.

## 0. Conclusion

Implementation status after the 2026-05-24 alignment pass: substantially
implemented in the runtime. The plan remains as design context, but the active
code now has a unified terminal stop contract, Observer-fed stop counters,
typed recovery metadata, rollback candidates, model-output failure stop traces,
closeout stop-report fields, and live-eval assertions for terminal stop and
recovery behavior.

Conservative decision: rollback is recommended and traced by default, not
auto-executed. Pre-action ActionReview denials/revisions now emit shared stop
events, while existing permission/checkpoint gates remain the source of truth.

The project already has a real base for stop and recovery behavior. It is not a
blank slate:

- `src/engine/stop_checker.rs` exists and is wired into the turn loop.
- `ActionReview` and permission checks already block or ask for user input
  before risky tools.
- repeated tool failures and duplicate read-only loops are stopped.
- final closeout is guarded by validation proof and tool evidence.
- file mutation paths create checkpoint-backed recovery data and fail closed
  when rollback would be unavailable.
- `rewind` can restore checkpoint-backed file changes or tool rounds.
- trace and live-eval reporting already expose stop checks, recovery plans,
  closeout, checkpoint, validation, and action-review facts.

The gap is that these are still several adjacent mechanisms, not one coherent
Stop Checker and Failure Recovery contract.

The note asks for a runtime that can answer:

```text
Should the agent continue, finish, ask the user, stop as partial, mark blocked,
declare failed, or roll back?
```

Current `priority-agent` can answer parts of that question, but not as one
stable end-state model. The next line of work should turn the existing pieces
into a single stop/recovery decision layer, while preserving the current
checkpoint and action-review safety work.

## 1. Current Alignment

| Note requirement | Current project state | Alignment | Main gap |
| --- | --- | --- | --- |
| LLM final answer should not blindly mean completed | Final closeout uses runtime validation proof and tool evidence before marking closeout status. | Strong partial | Final status taxonomy is still validation-oriented, not task-outcome oriented. |
| Verification passing is the strongest completion signal | `CloseoutEvaluator` uses `VerificationProof` and required validation commands. | Strong | Good foundation; should feed a unified terminal status. |
| Max step or tool budget stops infinite loops | Conversation loop has `max_iterations`; resource policy has `max_tool_calls`; closeout appends a budget stop message. | Partial | Budget stop is a message/fallback, not a structured `partial` stop outcome. |
| Consecutive failures should stop and trigger review | `StopChecker` handles repeated failed tools; `ToolFailureStopController` stops repeated/noisy failed tool attempts. | Partial | It tracks only narrow tool failure patterns, not consecutive failure families across validation/edit/output/permission. |
| No uncertainty reduction should stop or replan | Action checkpoint and no-code-progress counters exist; Observer now has `reduced_uncertainty`. | Partial | `reduced_uncertainty` and candidate focus are not yet inputs to `StopChecker`. |
| High risk should become `needs_user` | `ActionReview`, permission, destructive scope, and goal drift can ask/deny/revise. | Partial | StopChecker does not receive the final action-review risk verdict as a stop outcome. |
| Goal drift should stop or ask user | `GoalDriftDetector` traces medium/high drift and can require approval through permission evaluation. | Partial | It is still mostly action-level advisory, not part of terminal stop status. |
| User interruption or new instruction should stop/turn | Tool interrupt metadata, cancel tools, and retry/session flows exist. | Partial | No durable `stopped_by_user` or redirected-goal stop record in task state/closeout. |
| Tool failure should produce typed recovery | `RecoveryPlan::tool_failure` exists and trace records recovery plans. | Partial | Recovery categories are mostly `ToolErrorCode` labels, not domain-specific failure types like `file_not_found`, `old_string_not_found`, `test_assertion_failed`. |
| Command failure should distinguish environment vs test failure | Bash/run-tests metadata and Observer validation findings exist. | Partial | Recovery planning does not yet separate command-not-found, timeout, failing tests, environment missing, or dependency install failures as first-class categories. |
| Edit failure should reread or use smaller patch | `file_edit` returns rich failure metadata; action checkpoint pushes repair corrections. | Strong partial | Good local behavior, but the recovery plan does not uniformly consume edit failure metadata. |
| Model output failure should have bounded output repair | Workflow JSON parsers accept fenced JSON/JSON5 and mark recoverable parse errors. | Partial | There is no explicit retry budget and final stop reason for repeated model-output repair failure. |
| Permission failure should suggest alternatives | `ActionReview.model_recovery`, permission-denial recovery traces, and tool result metadata provide guidance. | Strong partial | Alternatives are not always normalized into a shared `allowed_alternatives` recovery field. |
| Destructive changes should have checkpoints and rollback | File writes/edits/format create checkpoints; `rewind` restores file changes and tool rounds. | Strong | Automatic rollback recommendation is not yet tied to stop/recovery decisions. |
| Weight system should influence whether continuing is worth it | Workflow feedback and adaptive triggers exist. | Partial | There is no `continue_score = value + uncertainty - risk - cost` stop metric. |
| Failures should enter memory as lessons | Turn outcomes and recovery plans are persisted in traces/learning events. | Partial | Failed strategies and better strategies are not extracted as durable structured memory inputs. |

## 2. Evidence From Current Code

- `StopChecker` is intentionally small and currently returns only
  `Continue`, `Checkpoint`, or `Stop` with reasons such as no progress,
  repeated tool failure, duplicate read-only, and verification ready
  (`src/engine/stop_checker.rs:1`, `src/engine/stop_checker.rs:12`,
  `src/engine/stop_checker.rs:36`).
- `StopCheckStatus` has only `Continue`, `Checkpoint`, and `Stop`; it does not
  model `completed`, `partial`, `blocked`, `failed`, `needs_user`, or
  `rolled_back` directly (`src/engine/task_context.rs:84`).
- `StopCheckRecord` stores stop-check facts, but not terminal status, failure
  family, recovery plan id, rollback candidate, or user-facing next action
  (`src/engine/task_context.rs:181`).
- The turn loop records stop checks after tool rounds, using tool success,
  validation success, no-code-progress rounds, action checkpoint state,
  repeated failed tools, and duplicate read-only tools
  (`src/engine/conversation_loop/turn_iteration_controller.rs:278`).
- Repeated failed tool handling can stop the turn with plain messages like
  `[Stopped repeated failed tool attempts: ...]`
  (`src/engine/conversation_loop/tool_failure_stop_controller.rs:17`).
- `RecoveryPlan` exists, but `tool_failure` mostly derives category from
  generic error codes and suggested commands
  (`src/engine/recovery_plan.rs:20`, `src/engine/recovery_plan.rs:90`).
- Closeout already uses validation proof and runtime labels, and downgrades
  unsafe verified closeout when proof blocks it
  (`src/engine/conversation_loop/closeout_controller.rs:37`,
  `src/engine/conversation_loop/closeout_controller.rs:105`).
- Iteration budget exhaustion is currently appended as a plain stop message and
  `WorkflowFallback`, not a structured `partial` stop decision
  (`src/engine/conversation_loop/closeout_controller.rs:217`).
- File mutation rollback safety is strong: file mutations refuse to write when
  checkpoint creation fails (`src/tools/file_tool/mod.rs:1626`).
- `rewind` is a high-risk, confirmation-required tool that can restore latest
  file change, latest tool round, target checkpoint, file change id, or path
  (`src/tools/rewind_tool/mod.rs:20`, `src/tools/rewind_tool/mod.rs:51`).
- Trace already has `StopCheckEvaluated`, `RecoveryPlan`, and
  `FinalCloseoutPrepared`, but they are separate events rather than one final
  stop/recovery report (`src/engine/trace.rs:181`,
  `src/engine/trace.rs:436`, `src/engine/trace.rs:467`).

## 3. Remaining Problems

### P0. Stop outcome taxonomy is too coarse

The note's core model has at least:

```text
completed, partial, blocked, failed, needs_user, rolled_back
```

Current runtime stop status is:

```text
continue, checkpoint, stop
```

Closeout status separately has:

```text
passed, partial, failed, not_verified
```

Verification status separately has:

```text
not_required, pending, verified, failed, blocked, user_deferred, unavailable
```

These are useful internal statuses, but no single object says what happened to
the user task. That makes reports and evals infer status from several partial
signals.

Risk:

- A loop can stop for repeated failure, budget exhaustion, or duplicate reads
  without a durable user-facing terminal status.
- Eval can see `closeout_not_successful`, but cannot always tell whether the
  correct answer was `partial`, `blocked`, `failed`, `needs_user`, or
  `rolled_back`.

### P1. StopChecker is post-tool and narrow

The current `StopChecker` mainly consumes tool-round facts:

- any tool success;
- successful write;
- successful validation command;
- no-code-progress rounds;
- action checkpoint state;
- repeated failed tools;
- duplicate read-only tools.

It does not directly consume:

- user interruption;
- latest Observer confidence or uncertainty reduction;
- action-review `ask_user`, `deny`, or `revise`;
- risk score / side-effect class;
- goal drift verdict;
- max edit attempts;
- consecutive validation failures;
- model-output parse repair attempts.

Risk:

- Pre-action "do not do this" decisions stay in ActionReview/permission, while
  post-action stop decisions stay in StopChecker. The agent can be safe, but
  the runtime cannot explain all stop causes through one contract.

### P2. Failure recovery is generic compared with the note

`RecoveryPlan` records source, category, primary error, action, retryable,
safe_retry, suggested command, and status. That is useful.

But the note wants recovery by failure type:

- file not found -> search/list alternatives;
- command not found -> inspect package manager/project setup;
- test assertion failure -> fix code;
- timeout -> narrow command or ask user;
- old text not found -> reread/smaller patch;
- permission denied -> allowed alternatives;
- model output invalid -> bounded repair, then stop.

Current recovery is often one of:

```text
invalid_params, permission_denied, not_found, timeout, dangerous_blocked,
unavailable, unknown
```

Risk:

- The recovery plan may be technically true but not operationally specific
  enough to prevent repeated bad actions.

### P3. Command failure and validation failure are not separated enough

The Observer work can now produce validation findings, failed test names, first
diagnostics, and command status. But recovery planning still does not treat
these as different failure families.

Important distinctions:

- command failed to execute;
- command timed out;
- dependency missing;
- test failed and gave useful bug evidence;
- test failed because harness/setup is broken;
- install failed;
- background task failed or is still running.

Risk:

- StopChecker may count useful failing tests as generic failure, or allow
  repeated validation failures without recognizing no improvement.

### P4. Rollback is available but not a stop/recovery decision

Checkpoint/rewind implementation is strong:

- file mutations create checkpoints;
- failed checkpoint creation blocks mutation;
- `rewind` can restore file changes/tool rounds.

But the runtime does not yet produce a structured rollback recommendation when:

- tests got worse after a recent edit;
- formatting changed files and then failed;
- patch partially failed;
- action scope drifted;
- user asks to undo;
- the agent marks a strategy as wrong.

Risk:

- The user or model can manually call rewind, but the runtime does not yet say
  "this is a rollback-worthy failure" with the exact safe target and rationale.

### P5. User interruption and new instruction are not terminal-state aware

The system has interrupt behaviors, cancel tools, `/retry`, and session flows.
But StopChecker and closeout do not preserve a user-facing status such as:

```text
stopped_by_user
redirected_by_user
needs_user
```

Risk:

- A user correction can be handled operationally, while task state and evals
  still look like a generic incomplete or failed turn.

### P6. Output repair is present only in parser tolerance

Workflow contract parsing can accept fenced JSON/JSON5 and detect recoverable
parse errors. But the note asks for a clear bounded flow:

```text
first bad output -> ask model to repair format
second bad output -> parser repair/fallback
third bad output -> stop and report unstable model output
```

Risk:

- Model-output failures can be swallowed as generic workflow fallback or API
  failure instead of becoming a typed stop/recovery path.

### P7. Memory learns outcomes, not failed strategies

The project persists turn outcomes and recovery-plan traces. It also has
workflow learning events.

Missing piece:

```json
{
  "failed_strategy": "...",
  "reason": "...",
  "better_strategy": "..."
}
```

Risk:

- The system can remember that a turn failed, but not always why the strategy
  was bad or what should be tried next time.

## 4. Development Plan

### Phase 0 - Baseline Stop/Recovery Tests

Goal: make current behavior measurable before changing the contract.

Add focused tests for:

- max iteration budget produces a structured stop report;
- repeated failed `file_edit` stops as failed/blocked rather than generic stop;
- duplicate read-only loop stops as partial with preserved evidence;
- successful validation after edit maps to completed;
- permission/action-review ask-user maps to needs_user;
- checkpoint creation failure maps to blocked with rollback unavailable;
- successful `rewind` maps to rolled_back;
- user interruption/new instruction maps to stopped_by_user or redirected.

Likely files:

- `src/engine/stop_checker.rs`
- `src/engine/task_context.rs`
- `src/engine/conversation_loop/turn_iteration_controller.rs`
- `src/engine/conversation_loop/closeout_controller.rs`
- `src/engine/trace.rs`
- `scripts/live_eval_report_parser.py`

Validation:

```bash
cargo test -q stop_checker -- --test-threads=1
cargo test -q turn_iteration_controller -- --test-threads=1
cargo test -q closeout_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
```

### Phase 1 - Unified Stop Outcome Contract

Goal: introduce one durable runtime answer for "what happened to the task?"

Add a new contract, either in `stop_checker.rs` or a small adjacent module:

```rust
pub enum TaskTerminalStatus {
    Completed,
    Partial,
    Blocked,
    Failed,
    NeedsUser,
    RolledBack,
    StoppedByUser,
}

pub enum StopAction {
    Continue,
    Closeout,
    AskUser,
    Replan,
    Recover,
    RecommendRollback,
    Stop,
}

pub struct StopOutcome {
    status: Option<TaskTerminalStatus>,
    action: StopAction,
    reason: StopCheckReason,
    summary: String,
    evidence: Vec<String>,
    recovery_plan_id: Option<String>,
    rollback_candidate: Option<RollbackCandidate>,
}
```

Compatibility rules:

- Keep existing `StopCheckStatus` for low-risk migration, or map it into
  `StopAction`.
- Keep `StageValidationStatus` as workflow validation state, not task terminal
  state.
- Keep `VerificationStatus` as evidence/proof state, not final user status.
- Add serde defaults so old traces and tests remain readable.

Validation:

```bash
cargo test -q stop_checker -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

### Phase 2 - Feed Observer State Into StopChecker

Goal: let the StopChecker know whether continuing is still useful.

Use the Observer fields added in the previous plan:

- `result_kind`;
- `confidence`;
- `reduced_uncertainty`;
- `risk_note`;
- `candidate_focus`;
- `next_attention`;
- failed validation findings.

Add bounded counters to task state:

- `uncertainty_not_reduced_steps`;
- `consecutive_validation_failures`;
- `consecutive_edit_failures`;
- `consecutive_command_failures`;
- `consecutive_permission_blocks`;
- `last_failure_family`;
- `last_progress_signal`.

Rules:

- successful validation with required commands -> `Completed`;
- no uncertainty reduction for N high-cost tool rounds -> `Blocked` or
  `Partial`;
- same validation failure after N repair attempts -> `Failed` or
  `NeedsUser`;
- repeated search/read with no new evidence -> `Partial` with evidence summary;
- high-risk next action with no approval -> `NeedsUser`.

Validation:

```bash
cargo test -q stop_checker -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q tool_result_controller -- --test-threads=1
```

### Phase 3 - Pre-Action Stop And Risk Gating

Goal: connect ActionReview, permission, goal drift, and StopChecker.

Current pre-action checks are strong but dispersed. Add an adapter from
`ActionReview` and goal drift into stop outcomes:

- `ActionReviewDecision::AskUser` -> `NeedsUser`;
- `ActionReviewDecision::Deny` -> `Blocked`;
- `ActionReviewDecision::Revise` -> `Recover` or `Replan`;
- destructive scope violation -> `NeedsUser` or `Blocked`;
- high goal drift -> `NeedsUser`;
- max tool-call budget before scheduling -> `Partial`;
- checkpoint unavailable before mutation -> `Blocked`.

Do not weaken existing permission behavior. This phase only makes the decision
visible in a shared stop/recovery contract.

Validation:

```bash
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q goal_drift -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

### Phase 4 - Typed Failure Recovery Families

Goal: make recovery specific enough to prevent repeated bad actions.

Extend `RecoveryPlan` with:

```rust
failure_type: String,
recovery_kind: RecoveryKind,
allowed_alternatives: Vec<String>,
retry_budget: Option<usize>,
side_effect_uncertain: bool,
requires_user_decision: bool,
```

Add deterministic classifiers:

- file read/write/edit:
  `file_not_found`, `old_string_not_found`,
  `old_string_occurrence_mismatch`, `stale_read_conflict`,
  `checkpoint_creation_failed`, `generated_or_dependency_target`;
- command:
  `command_not_found`, `dependency_missing`, `timeout`, `test_failed`,
  `install_failed`, `environment_unavailable`;
- permission:
  `path_outside_workspace`, `permission_denied`,
  `dangerous_blocked`, `network_requires_confirmation`;
- model output:
  `invalid_json`, `schema_validation_failed`, `tool_name_invalid`,
  `tool_arguments_invalid`;
- context:
  `context_overflow`, `provider_protocol`, `payload_too_large`.

Recovery examples:

- `old_string_not_found` -> reread target file, use smaller exact patch;
- `test_failed` -> inspect first diagnostic and fix current code;
- `command_not_found` -> inspect project package manager/toolchain;
- `permission_denied` -> ask user or choose listed safe alternative;
- `checkpoint_creation_failed` -> stop, do not mutate.

Validation:

```bash
cargo test -q recovery_plan -- --test-threads=1
cargo test -q file_tool -- --test-threads=1
cargo test -q bash_tool -- --test-threads=1
cargo test -q tool_result_controller -- --test-threads=1
```

### Phase 5 - Rollback Recommendation Path

Goal: make rollback a controlled runtime decision, not only a manual tool.

Add a `RollbackCandidate` / `RollbackRecommendation` object:

```rust
pub struct RollbackCandidate {
    checkpoint_id: Option<String>,
    file_change_id: Option<String>,
    tool_round_id: Option<String>,
    paths: Vec<String>,
    reason: String,
    confidence: u8,
    auto_allowed: bool,
}
```

Initial policy:

- default to recommend, not auto-rollback;
- auto-rollback only for agent-owned latest tool round when the write was
  partially applied and the tool already knows rollback is safe;
- never use `git reset --hard`;
- never roll back user-preexisting dirty changes;
- always trace rollback candidate and actual rollback result;
- after rollback, mark failed strategy in task state.

Trigger candidates:

- validation became worse after last edit;
- format/check tool changed file then failed;
- multi-file patch partially failed;
- action exceeded allowed scope;
- user explicitly asked to undo;
- Observer risk note says command side effects are unclear.

Validation:

```bash
cargo test -q checkpoint -- --test-threads=1
cargo test -q rewind -- --test-threads=1
cargo test -q stop_checker -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

### Phase 6 - Bounded Model Output Repair

Goal: handle bad model control outputs without infinite retry.

Implement a small repair controller for workflow JSON/control outputs:

```text
attempt 1: parse strict JSON / JSON5 / fenced JSON
attempt 2: request format repair with schema and original error
attempt 3: stop as failed or blocked with model_output_invalid
```

Apply only to structured control outputs:

- workflow judgment;
- acceptance review;
- guided debugging;
- future observer fallback JSON if enabled.

Rules:

- never repair tool execution results as proof;
- never let repaired output invent validation success;
- trace parse error, repair attempt count, and final stop status.

Validation:

```bash
cargo test -q workflow_contract -- --test-threads=1
cargo test -q recovery_plan -- --test-threads=1
cargo test -q trace -- --test-threads=1
```

### Phase 7 - Stop Report And Closeout Integration

Goal: every non-continue terminal path should produce a concise, inspectable
report.

Add a `StopReport` rendered into closeout/final responses:

```text
Status: completed | partial | blocked | failed | needs_user | rolled_back
Why stopped: ...
Evidence: ...
Changed files: ...
Validation: ...
Recovery tried: ...
Rollback: unavailable | recommended | completed
Next action: ...
```

Integrate with:

- `FinalCloseoutPrepared` trace event;
- runtime diet closeout/validation labels;
- `scripts/run_live_eval.sh` summary fields;
- desktop/TUI trace panel when available.

Validation:

```bash
cargo test -q closeout_controller -- --test-threads=1
cargo test -q turn_completion_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh
```

### Phase 8 - Failure Memory And Eval Diagnostics

Goal: make failures useful for future runs and measurable in live evals.

Persist structured learning events when a stop outcome is terminal:

```json
{
  "terminal_status": "blocked",
  "failed_strategy": "repeated exact file_edit old_string",
  "reason": "old_string_not_found after stale read",
  "better_strategy": "reread the target range and patch with a narrower anchor",
  "recovery_plan_id": "...",
  "rollback_status": "not_needed"
}
```

Add live-eval assertions and summary fields:

- `stop_terminal_status`;
- `stop_reason`;
- `failure_type`;
- `recovery_plan`;
- `rollback_recommended`;
- `rollback_completed`;
- `needs_user`;
- `stopped_by_user`;
- `uncertainty_not_reduced`.

Validation:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh
bash scripts/live-eval-summary-smoke.sh
cargo test -q trace -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
```

## 5. Suggested Execution Order

1. Phase 0 first: pin current behavior and avoid breaking closeout/tool-call
   protocol.
2. Phase 1 next: create the terminal stop outcome contract while preserving
   current fields.
3. Phase 2 next: feed Observer progress and uncertainty into StopChecker.
4. Phase 3 after that: unify pre-action ActionReview/permission decisions with
   stop outcomes.
5. Phase 4 next: make failure recovery typed and operationally specific.
6. Phase 5 after typed failures: add rollback recommendation, not default
   automatic rollback.
7. Phase 6 can run in parallel with Phase 4 if model-output parse failures are
   hurting evals.
8. Phase 7 and Phase 8 should be added as each behavior lands, so traces and
   evals remain explainable.

## 6. Non-Goals

- Do not replace `ActionReview`, permission checks, checkpoint manager, or
  `rewind`; this plan should connect them through a shared stop/recovery
  contract.
- Do not auto-rollback broad changes by default.
- Do not use destructive git rollback commands such as `git reset --hard`.
- Do not let LLM-generated recovery summaries override runtime validation,
  permission, or checkpoint facts.
- Do not make StopChecker a large prompt. The hard stop conditions should be
  runtime logic, with the model seeing concise reasons and safe alternatives.
- Do not stop useful failing tests too early. A failed validation can be
  progress when it reduces uncertainty.

## 7. Definition Of Done

This line is done when:

- every stop path produces a structured `StopOutcome`;
- user-facing terminal status can be one of completed, partial, blocked,
  failed, needs_user, rolled_back, or stopped_by_user;
- StopChecker consumes Observer progress and uncertainty signals;
- ActionReview/permission/goal-drift decisions map into stop outcomes;
- failure recovery records typed failure families and safe alternatives;
- rollback is recommended or performed only with explicit checkpoint-backed
  scope;
- closeout/final answers include a compact stop report when not simply
  completed;
- traces and live-eval summaries expose terminal status, failure type, recovery
  plan, rollback status, and needs-user state;
- failed strategies can be persisted as structured learning events.

## 8. Implementation Notes

Completed in this pass:

- Added `TaskTerminalStatus`, `StopAction`, richer `StopCheckReason`,
  `RollbackCandidate`, and `FailedStrategyRecord` in
  `src/engine/task_context.rs`.
- Extended `StopCheckRecord` and `AgentTaskState` with terminal status,
  evidence, failure type, recovery plan id, rollback candidate, next action,
  failure counters, last progress signal, rollback candidates, and failed
  strategies.
- Fed Observer/tool ledger state into task state counters:
  uncertainty-not-reduced, validation/edit/command/permission failures,
  progress reset, failure family, and checkpoint-backed rollback candidates.
- Expanded `StopChecker` to produce terminal status and action decisions for:
  completed validation, partial budget/duplicate-read stops, repeated tool
  failure, consecutive validation/edit/command/permission failures,
  uncertainty not reduced, model-output invalid, user interruption,
  ActionReview ask/deny/revise, and rollback recommendation.
- Wired stop outcomes into turn-loop traces and task-state updates.
- Added pre-action stop trace emission for ActionReview denial/revision paths
  without weakening existing permission or checkpoint gates.
- Extended `RecoveryPlan` with typed `failure_type`, `recovery_kind`,
  `allowed_alternatives`, `retry_budget`, `side_effect_uncertain`, and
  `requires_user_decision`.
- Added deterministic recovery classification for old-string edit failures,
  stale reads, checkpoint creation failures, permission blocks, timeouts,
  command-not-found/target-not-found, failed tests, invalid params, and
  unavailable tools/remotes.
- Propagated typed recovery metadata into tool result metadata, tool
  observations, context ledger entries, traces, and TUI learning output.
- Added rollback recommendation candidates from failed checkpointed
  observations; default policy remains recommendation-only.
- Added structured budget-exhaustion and bounded patch-synthesis failure stop
  trace events.
- Extended final closeout trace fields with terminal status, stop reason,
  stop action, failure type, recovery plan id, and rollback status.
- Added failed-strategy memory candidates to `ExperienceRecord` turn outcome
  payloads.
- Extended `scripts/live_eval_report_parser.py` with assertions and summary
  fields for stop terminal status, stop action, failure type, typed recovery,
  rollback recommended/completed, needs-user, stopped-by-user,
  uncertainty-not-reduced, and model-output-invalid.

Validation run:

```bash
cargo check -q
python3 -m py_compile scripts/live_eval_report_parser.py
cargo fmt --check
cargo test -q stop_checker -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
cargo test -q runtime_spine_behavior -- --test-threads=1
cargo test -q recovery_plan -- --test-threads=1
cargo test -q closeout_controller -- --test-threads=1
cargo test -q turn_completion_controller -- --test-threads=1
cargo check --features experimental-api-server -q
cargo clippy --all-features -- -D warnings
cargo test -q
bash -n scripts/run_live_eval.sh
```

Final full-test result: `1829 passed; 0 failed`, plus 3 auxiliary tests passed.
