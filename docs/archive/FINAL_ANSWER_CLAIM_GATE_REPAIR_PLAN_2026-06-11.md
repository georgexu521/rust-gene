# Final Answer Claim Gate And Bounded Repair Plan
Status: Proposed

Last updated: 2026-06-11

## Goal

Prevent weak models from falsely claiming completion in the final answer, while
still giving the agent a chance to recover and actually finish the work.

The target failure looks like this:

```text
User: Fix the bug and run tests.
Model: I fixed it and tests passed.
Reality: no file changed, no validation command ran.
```

The correct product behavior is not just to rewrite the final answer to
`not_verified`. The correct behavior is:

1. Detect the unsupported claim before sending it to the user.
2. Convert the mismatch into structured runtime evidence.
3. Re-enter the model once with a focused repair prompt when budget and safety
   allow it.
4. Only close out to the user when repair succeeds, reaches a hard boundary, or
   exhausts the bounded retry budget.

## Non-Goals

- Do not ask the LLM to "double check itself" as the main defense. Weak models
  can hallucinate the double check too.
- Do not add another broad always-on prompt rule.
- Do not weaken closeout, permissions, checkpoints, destructive action gates, or
  validation requirements.
- Do not make final-answer validation depend on hidden semantic judgment by the
  runtime. The runtime should inspect concrete claims against concrete evidence.
- Do not run arbitrary validation commands invented by the final-answer gate.
  It may request repair, but command choice stays in the normal agent/tool flow.

## Existing Project Strengths

Priority Agent already has most of the proof boundary needed for this feature:

- `EvidenceLedger` records tool results, changed files, validation results, and
  closeout-relevant facts.
- `VerificationProof` derives verified, partial, failed, not-run, blocked,
  unavailable, or user-deferred proof status from ledger evidence.
- `CloseoutEvaluator` reads `EvidenceLedger` and `VerificationProof`, not model
  summary text.
- `FinalCloseoutController` appends a structured `Closeout:` block and records
  `FinalCloseoutPrepared` trace events.
- `AssistantResponseRetryController` already retries some unsupported no-tool
  claims, such as false bash-unavailable or local filesystem claims without a
  tool.
- Goal mode now reads closeout/proof events through `GoalDecisionEngine`, so an
  unsupported completion can become another goal turn instead of a false
  `Complete`.

The missing piece is a final-answer-specific claim gate that checks the model's
natural-language completion claims before they reach the user.

## Design Summary

Add a deterministic claim gate between the model's final text and final
delivery:

```text
assistant final text
  -> FinalAnswerClaimExtractor
  -> FinalAnswerClaimVerifier
  -> decision:
       pass through
       repair once
       downgrade closeout
       block impossible claim
  -> final response or bounded repair turn
```

The gate should treat the final answer as untrusted narration. Only runtime
evidence can support claims about file changes, tests, commits, tool execution,
or completion.

## Claim Classes

Start with narrow, high-value claim classes. Do not try to parse every possible
sentence.

### 1. Mutation Claims

Examples:

- "I modified `src/foo.rs`."
- "I fixed the bug."
- "I created the file."
- "The implementation is complete."

Required support:

- `EvidenceLedger.changed_files()` includes at least one relevant file; or
- tool records include successful file mutation tools; or
- git/checkpoint diff shows effective changes for this turn.

Unsupported when:

- `changed_files` is empty for a code-change or bug-fix route;
- no file mutation tool succeeded;
- no effective diff was recorded after an edit attempt.

### 2. Validation Claims

Examples:

- "Tests passed."
- "`cargo test -q` passed."
- "I ran the validation."
- "Clippy is clean."

Required support:

- `EvidenceLedger` contains a successful validation record for the claimed
  command; or
- `VerificationProof` is `Verified` and its proof kind supports the claim; or
- closeout validation lines include the exact required command pass.

Unsupported when:

- proof status is `NotRun`, `Unavailable`, `Blocked`, or `UserDeferred`;
- the command mentioned by the model does not appear in ledger evidence;
- only a compacted summary or model narration claims the test passed.

### 3. Commit Claims

Examples:

- "I committed the changes."
- "The commit is ready."
- "Pushed to GitHub."

Required support:

- a successful git commit/push operation was recorded in tool evidence; or
- the runtime has a known commit SHA / push result for this turn.

Unsupported when:

- no git command/tool output exists;
- the final answer says "committed" but the only evidence is staged or unstaged
  changes.

### 4. Read/Inspection Claims

Examples:

- "I checked the file."
- "I reviewed the diff."
- "I inspected the logs."

Required support:

- file read/search/tool records show the inspected resource; or
- trace/tool evidence includes diff/log inspection.

Unsupported when:

- the answer states current local filesystem facts without corresponding tool
  output.

This overlaps with existing `AssistantResponseRetryController` filesystem
grounding. The final-answer gate should reuse that detector rather than create
two incompatible implementations.

### 5. Completion Claims

Examples:

- "Done."
- "All set."
- "Everything is fixed."

Required support depends on workflow:

- read-only/direct answer: answer may be complete if no tools or validation were
  required.
- code-change/bug-fix: requires proof-compatible closeout, or no-diff audit
  evidence when no file change was expected.
- goal mode: requires `GoalDecision::Complete`, not just final-answer prose.

Unsupported when:

- closeout status is `partial`, `not_verified`, or `failed`;
- verification proof is not run and validation was required;
- goal decision is `Continue`, `NeedsUser`, `Blocked`, or `Failed`.

## Runtime Decision Model

Introduce a small deterministic decision enum:

```rust
pub enum FinalAnswerClaimGateDecision {
    Pass,
    Repair {
        observation: FinalAnswerClaimObservation,
    },
    Downgrade {
        observation: FinalAnswerClaimObservation,
        user_visible_status: &'static str,
    },
}
```

`Pass` means all detected claims are supported or no relevant claim was found.

`Repair` means the model made an unsupported completion claim and the task can
still continue safely within budget.

`Downgrade` means repair is not allowed or not useful. The runtime should append
an honest closeout such as `not_verified`, `needs_user`, `blocked`, or `failed`
with the unsupported claim evidence.

## Repair Eligibility

The gate should trigger a repair turn only when all are true:

- The route is code-change, bug-fix, goal, or another action workflow where
  continued work is expected.
- The user requested an actionable outcome, not just an explanation.
- The unsupported claim is about mutation, validation, commit, or completion.
- No hard safety boundary is active.
- No permission/credential/network blocker requires user input.
- The repair budget has not been exhausted.
- The assistant has not already received the same claim-gate repair observation
  in this turn/session.

Do not repair when:

- The user asked a lightweight side question.
- The answer is a read-only explanation and no tool-grounding was required.
- The claim is unsupported but harmless and no action was requested; downgrade
  the answer instead.
- The same unsupported claim repeats after repair.
- The model is at max iteration/tool budget.

## Bounded Repair Loop

Add a budget specifically for claim-gate repair:

```rust
pub struct FinalAnswerClaimGateBudget {
    pub max_repairs_per_turn: u8,      // default 1
    pub max_repairs_per_goal_step: u8, // default 1
}
```

The first implementation should allow only one repair per turn. This is enough
to catch the common "I did it" hallucination without creating a runaway loop.

When the gate chooses `Repair`, append a structured observation to the next model
request:

```text
<recent_observation>
Final answer claim gate failed.

Unsupported claims:
- mutation_completed: assistant claimed code was changed, but changed_files=0
- validation_passed: assistant claimed `cargo test -q` passed, but proof=not_run

Runtime evidence:
- route=bug_fix
- changed_files=0
- validation_proof=not_run
- required_validation=["cargo test -q"]
- successful_mutation_tools=0
- failed_tools=0

Required next action:
- Continue the task.
- Inspect the target files if needed.
- Make an actual focused change or explain why no change is required.
- Run or request the required validation.
- Do not claim completion until the evidence supports it.
</recent_observation>
```

This is not a user-facing final answer. It is runtime feedback to the next model
turn.

## Integration Points

### Assistant Response Retry

Likely file:

- `src/engine/conversation_loop/assistant_response_retry_controller.rs`

Extend the existing unsupported-claim retry mechanism instead of creating a
parallel retry path.

Current controller already handles:

- no-tool response after tool use;
- pseudo shell command claims;
- false bash-unavailable claims;
- local filesystem claims without evidence;
- continuation-only responses.

Add:

- unsupported mutation/completion claim detection;
- unsupported validation claim detection;
- unsupported commit/push claim detection;
- a new retry flag such as `claim_gate_repair_used`.

This file is the right first integration point for no-tool final answers.

### Closeout Controller

Likely file:

- `src/engine/conversation_loop/closeout_controller/mod.rs`

Add a final claim-gate pass before appending the final closeout text, or just
before final content is emitted.

The closeout layer has access to:

- final content;
- final tool calls;
- iterations used / max iterations;
- `EvidenceLedger`;
- `VerificationProof`;
- required validation commands;
- settlement gaps;
- task route and task context.

That is enough evidence to determine whether a final claim is supported.

Do not let `CloseoutEvaluator` parse model text. Keep evaluation proof-driven.
The claim gate should be a sibling layer that can trigger repair or downgrade
when text and proof disagree.

### Turn Loop

Likely files:

- `src/engine/conversation_loop/turn_iteration_loop_controller.rs`
- `src/engine/conversation_loop/mod.rs`

If the gate chooses `Repair`, the turn loop must:

- push the assistant's unsupported final answer into context as recent content;
- push the claim-gate observation as a system/runtime observation;
- continue one more iteration if budget allows;
- record a trace event so the UI and evals can see the repair path.

Avoid returning a user-visible answer before the repair attempt.

### Evidence Ledger

Likely file:

- `src/engine/evidence_ledger.rs`

Add or expose helper methods only if current APIs are insufficient:

```rust
pub struct FinalAnswerEvidenceSnapshot {
    pub changed_files: Vec<String>,
    pub successful_mutation_tools: usize,
    pub validation_records: Vec<ValidationEvidence>,
    pub verification_status: VerificationProofStatus,
    pub git_commit_records: Vec<String>,
    pub git_push_records: Vec<String>,
}
```

Keep this as a read-only snapshot. Do not mutate ledger state from the claim
gate.

### Trace

Likely files:

- `src/engine/trace/event.rs`
- `src/engine/trace/event_summary.rs`

Add an event:

```rust
TraceEvent::FinalAnswerClaimGate {
    decision: String,
    unsupported_claims: usize,
    repair_attempt: u32,
    changed_files: usize,
    verification_proof_status: Option<String>,
    summary: String,
}
```

This lets `/trace`, desktop, and live eval reports distinguish:

- weak model hallucination;
- runtime caught and repaired it;
- runtime caught it and downgraded because repair was impossible.

### Goal Mode

Likely files:

- `src/engine/goal/decision.rs`
- `src/engine/goal/runner.rs`
- `src/tui/app.rs`

Goal mode should treat claim-gate failure as evidence that the goal cannot
complete yet.

Mapping:

- `Repair` -> goal remains `Active` and schedules another continuation.
- `Downgrade` with hard blocker -> `NeedsUser`, `Blocked`, or `Failed`.
- Unsupported completion claim must never become `GoalDecision::Complete`.

The goal step should store a summary such as:

```text
claim_gate=repair unsupported=validation_passed,mutation_completed proof=not_run changed_files=0
```

## Claim Extraction Strategy

Use deterministic pattern matching first. This should be intentionally boring.

### English Patterns

Mutation:

- `I fixed`
- `I've fixed`
- `I changed`
- `I updated`
- `I created`
- `I implemented`
- `all set`
- `done`

Validation:

- `tests passed`
- `test passed`
- `cargo test`
- `clippy passed`
- `validation passed`
- `checks passed`
- `build passed`

Commit:

- `committed`
- `created commit`
- `pushed`
- `pushed to GitHub`

### Chinese Patterns

Mutation:

- `我修好了`
- `已经修复`
- `已经修改`
- `我改了`
- `已经实现`
- `做完了`
- `完成了`

Validation:

- `测试通过`
- `验证通过`
- `跑过测试`
- `cargo test 通过`
- `clippy 通过`
- `检查通过`

Commit:

- `已经提交`
- `提交好了`
- `已经推送`
- `推送到 GitHub`

### Avoid Over-Matching

Do not trigger repair for:

- "You can fix this by..."
- "The test should pass after..."
- "I would change..."
- "I cannot verify..."
- "No changes were made."
- "This is a plan, not an implementation."

The extractor should emit claim spans and claim types, but it does not need
full natural-language understanding.

## User-Facing Behavior

### Case A: Repair Succeeds

User sees only the final successful response:

```text
Fixed the parser fallback and added a regression test.

Validation: `cargo test -q parser_fallback --lib` passed.
```

Trace shows the claim-gate repair happened.

### Case B: Repair Fails Or Budget Exhausts

User sees an honest closeout:

```text
I could not verify completion.

The previous answer claimed the fix and tests were done, but runtime evidence
still shows changed_files=0 and validation_proof=not_run after one repair
attempt. The task remains not_verified.
```

### Case C: Needs User

```text
I could not continue because validation needs credentials/network approval.
No verified completion claim was emitted.
```

### Case D: Read-Only Question

If the user only asked for an explanation and no local state is claimed, the gate
passes through.

## Implementation Phases

### Phase 0 - Tests First: Unsupported Claim Fixtures

Status: next

Add tests that capture the failure before implementation.

Test cases:

- model says "I fixed it" with `changed_files=0` -> repair decision.
- model says "tests passed" with `verification_proof=not_run` -> repair
  decision.
- model says "committed" with no git evidence -> repair or downgrade.
- model says "I did not modify files" with `changed_files=0` -> pass.
- read-only answer with no local-state claims -> pass.
- Chinese "已经修复，测试通过" with no evidence -> repair decision.
- claim-gate repair already used once -> downgrade, not infinite retry.

Likely files:

- `src/engine/conversation_loop/assistant_response_retry_controller.rs`
- `src/engine/conversation_loop/closeout_controller/tests.rs`
- new `src/engine/conversation_loop/final_answer_claim_gate.rs`

Validation:

```bash
cargo test -q final_answer_claim_gate --lib
cargo test -q assistant_response_retry_controller --lib
cargo test -q closeout_controller --lib
```

### Phase 1 - Claim Extractor And Evidence Snapshot

Status: next

Add a small module:

```text
src/engine/conversation_loop/final_answer_claim_gate.rs
```

Types:

```rust
pub enum FinalAnswerClaimKind {
    MutationCompleted,
    ValidationPassed,
    CommitCreated,
    Pushed,
    FileInspected,
    TaskCompleted,
}

pub struct FinalAnswerClaim {
    pub kind: FinalAnswerClaimKind,
    pub span_preview: String,
    pub command: Option<String>,
    pub path: Option<String>,
}

pub struct FinalAnswerClaimGateInput<'a> {
    pub content: &'a str,
    pub route: &'a IntentRoute,
    pub evidence_ledger: &'a EvidenceLedger,
    pub verification_proof: &'a VerificationProof,
    pub required_validation_commands: &'a [String],
    pub repair_used: bool,
    pub iterations_used: usize,
    pub max_iterations: usize,
}
```

Keep the API small and independent enough to test directly.

Validation:

```bash
cargo test -q final_answer_claim_gate --lib
cargo fmt --check
```

### Phase 2 - Retry Integration

Status: follow-up

Extend `AssistantResponseRetryController`:

- add `claim_gate_repair_used` to request/application contexts;
- evaluate unsupported final claims after existing filesystem/tool retry checks;
- emit a `<recent_observation>` with structured claim-gate evidence;
- return `NoToolAssistantResponseFlow::Retry`.

This catches no-tool hallucinated answers early, before closeout.

Validation:

```bash
cargo test -q assistant_response_retry_controller --lib
cargo test -q pseudo_tool --lib
```

### Phase 3 - Closeout Integration

Status: follow-up

Add claim-gate evaluation in final closeout handling for cases where:

- the model used tools;
- final content exists;
- closeout proof disagrees with final answer claims.

If repair is still possible, return control to the turn loop rather than
immediately appending the final closeout.

If repair is not possible, append a clear closeout downgrade and trace event.

Validation:

```bash
cargo test -q closeout_controller --lib
cargo test -q turn_iteration_loop_controller --lib
```

### Phase 4 - Goal Mode Integration

Status: follow-up

Feed claim-gate outcomes into goal steps:

- record unsupported claim summary in the latest goal step;
- keep active goal running when repair is eligible;
- block complete decisions when unsupported completion claims exist.

Validation:

```bash
cargo test -q goal --lib
cargo test -q goal_runner --lib
```

### Phase 5 - UI And Trace Visibility

Status: follow-up

Expose the event without making the UI noisy.

TUI:

- `/trace last` should show claim-gate repair or downgrade.
- `/quick` can include a compact "Claim gate: repaired" line if it happened.

Desktop:

- transcript can show a small runtime diagnostic only when repair fails or
  downgrades;
- successful repair should stay mostly invisible except in trace/debug views.

Validation:

```bash
cargo test -q trace --lib
cargo test -q quick --lib
corepack pnpm --dir apps/desktop build
```

### Phase 6 - Live Eval Coverage

Status: follow-up

Add or update live eval cases:

- weak model says "done" without edit;
- weak model says "tests passed" without validation;
- weak model says "committed" without git evidence;
- model recovers after claim-gate observation and actually edits/tests;
- repair budget exhausted produces honest `not_verified`.

Likely locations:

- `evalsets/live_tasks/`
- `scripts/live_eval_report_parser.py`
- `docs/archive/REAL_TASK_SOAK_TEST_PLAN_2026-06-07.md`

Validation:

```bash
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/run_live_eval.sh --case <claim-gate-case> --mode agent-run --run-tests
```

## Acceptance Criteria

This feature is done when:

- Unsupported "I changed/fixed/implemented" claims with no mutation evidence do
  not reach the user as successful final answers.
- Unsupported "tests passed" claims with no validation evidence trigger repair
  or honest `not_verified`.
- Unsupported commit/push claims require git evidence.
- One repair attempt can recover by making a real change and running validation.
- Repeated unsupported claims stop after budget and produce a clear `not_verified`
  closeout.
- Goal mode never marks `Complete` from unsupported final-answer claims.
- Trace records whether the gate passed, repaired, or downgraded.
- Existing closeout tests still pass.
- `cargo clippy --all-targets --all-features -- -D warnings` passes.

## Risks

### False Positives

The gate may block harmless phrases like "done" in a read-only answer. Mitigate
by scoping strict checks to code-change, bug-fix, goal, and local-state routes.

### Repair Loops

The model may repeat the same unsupported claim. Mitigate with one repair per
turn and trace-backed repeat detection.

### Over-Trusting Git Diff

A diff alone does not prove the right fix. It only supports mutation claims.
Validation claims still require validation evidence.

### Over-Trusting Validation

A passing command does not prove every acceptance criterion. The gate should
support specific claims, not upgrade the whole task to complete unless closeout
also supports it.

### Prompt Bloat

The repair observation should be compact and structured. Do not add large
always-on prompt sections.

## Suggested First Slice

Start with the smallest high-impact slice:

1. Add `final_answer_claim_gate.rs` with deterministic claim extraction and
   evidence snapshot checks.
2. Add tests for English and Chinese unsupported mutation/validation claims.
3. Integrate only with `AssistantResponseRetryController` for no-tool final
   answers.
4. Add one closeout regression test proving summary text cannot override
   `EvidenceLedger`.
5. Run:

```bash
cargo test -q final_answer_claim_gate --lib
cargo test -q assistant_response_retry_controller --lib
cargo test -q closeout_controller --lib
cargo check -q
```

After that slice passes, wire the gate into final closeout and goal mode.

## Open Questions

- Should the first repair attempt consume the normal iteration budget, or have a
  reserved single claim-gate repair budget?
- Should commit/push claims be supported through generic bash evidence first, or
  should git operations become typed evidence in `EvidenceLedger`?
- Should `changed_files=0` plus no-diff audit evidence support "done" for
  review-only tasks? The likely answer is yes, but only on read-only routes.
- Should the desktop transcript show a claim-gate repair that later succeeds, or
  keep it only in trace/debug views?

