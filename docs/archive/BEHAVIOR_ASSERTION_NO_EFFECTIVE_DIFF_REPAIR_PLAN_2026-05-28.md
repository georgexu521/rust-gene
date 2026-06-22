# Behavior Assertion / No Effective Diff Repair Plan
Status: Active

Date: 2026-05-28
Branch: `codex/behavior-assertion-repair-loop`

## Goal

Make coding tasks recover faster when the model claims progress, runs checks, or
keeps inspecting, but no real code diff exists and required behavior assertions
still fail.

This is deliberately not a main-loop rewrite. The runtime should keep the
existing strict validation and closeout gates, then feed a sharper observation
back to the LLM so the next step is a patch or a bounded focused lookup before a
patch.

## Problem Seen In Recent Tests

The latest real coding gauntlet showed that the framework was mostly doing the
right thing: it blocked false closeout when required validation failed or no code
diff existed. The remaining failures were mostly owned by `llm_reasoning`.

The weak pattern was:

- The task required a behavioral change.
- Required validation or sample behavior assertions failed.
- The worktree had no effective code diff, or patch synthesis produced no
  change.
- The model continued with ordinary reasoning instead of quickly entering a
  concrete repair path.

Framework responsibility is not to mark this as success. That already works.
The next framework responsibility is to make the failure easier for the model to
repair by turning it into an explicit, structured observation.

## Design

### Detection

The first implementation detects the most reliable runtime-owned signal:

```text
programming workflow
+ no changed files recorded
+ required validation ran
+ at least one required validation command failed
=> no_effective_diff repair observation
```

Live-eval behavior assertions are usually represented as required validation
commands. Treating failed required validation with an empty diff as a
`no_effective_diff` repair case keeps the implementation deterministic and
does not require the runtime to semantically judge the code.

### Runtime Action

When the detection fires, the runtime now:

- appends a `[No-effective-diff repair observation]` system message;
- includes failed required validation commands and a compact evidence preview;
- activates focused repair mode;
- blocks further pre-patch validation by setting
  `action_checkpoint_requires_patch_before_validation`;
- reserves one repair round so a model at the iteration edge can still patch;
- records a trace fallback event for observability.

If the same pattern repeats, the runtime escalates the observation with
`repair_escalation=focused_patch_required` and raises the no-change checkpoint so
existing patch-synthesis recovery can engage sooner.

### LLM Responsibility

The LLM still owns semantic repair:

- choose the target file and actual code change;
- decide whether one narrow lookup is needed;
- patch using current evidence;
- rerun the failed required validation after a file change.

### Framework Responsibility

The framework owns the guardrails:

- do not accept empty-diff closeout for code-change workflows;
- keep failed required validation visible in context;
- prevent validation loops before a patch;
- preserve trace evidence and failure ownership;
- keep bounded recovery honest if the model still cannot patch.

## Implementation Slice

Files changed:

- `src/engine/conversation_loop/post_change_workflow_controller.rs`
- `src/engine/conversation_loop/patch_synthesis_executor.rs`
- `src/engine/conversation_loop/turn_runtime_state.rs`

Core state added:

```rust
FocusedRepairRuntimeState::no_effective_diff_repair_rounds
```

Core behavior added:

```text
NoEffectiveDiffRepairController::apply(...)
```

The controller is only invoked from the no-changed-files post-change branch,
after required validation evidence has been recorded and appended.

Follow-up from the first live run:

- The new `no_effective_diff` observation fired correctly.
- The model entered focused repair and deterministic patch synthesis.
- Deterministic patch synthesis then produced valid patch actions, but
  `file_edit` rejected them because the target files had only been partially read
  by model tools.
- This was framework friction: patch synthesis had already validated the patch
  against current file content, but did not register that read with the file
  state tracker.

The executor now marks synthesized `file_edit` targets as read using the current
file content and mtime immediately before executing the synthesized batch. This
keeps read-before-edit discipline intact while preventing validated
runtime-synthesized patches from being blocked as stale or partial.

## Acceptance Criteria

- A direct/non-programming turn with no changes and no required validation stays
  silent.
- A no-diff turn with passing required validation stays accepted as validation
  evidence.
- A code-change turn with no diff and failed required validation emits
  `status=no_effective_diff`.
- The same failure activates focused repair and disables pre-patch validation.
- The runtime does not mark the task verified unless the normal validation and
  closeout gates pass later.

## Validation

Completed:

```bash
cargo fmt --check
cargo test -q post_change_workflow_controller -- --test-threads=1
cargo test -q patch_synthesis_executor -- --test-threads=1
cargo test -q required_validation -- --test-threads=1
cargo check -q
```

Diagnostic live run:

```bash
bash scripts/run_live_eval.sh --case memory-save-quality-gate --mode agent-run --run-tests --run-id behavior-repair-memory-save-20260528-145633 --label behavior-repair --overlay-working-tree
bash scripts/run_live_eval.sh --mode summary --run-id behavior-repair-memory-save-20260528-145633
```

Result: failed, but useful. It confirmed `no effective diff repair observation
emitted after required validation failure round=1` in the trace, then exposed the
patch-synthesis/read-state friction fixed in this slice.

Rerun after the patch-synthesis read-state fix:

```bash
bash scripts/run_live_eval.sh --case memory-save-quality-gate --mode agent-run --run-tests --run-id behavior-repair-memory-save-rerun-20260528-151753 --label behavior-repair --overlay-working-tree
bash scripts/run_live_eval.sh --mode summary --run-id behavior-repair-memory-save-rerun-20260528-151753
```

Result: passed.

- Summary: `docs/benchmarks/live-behavior-repair-memory-save-rerun-20260528-151753/summary.md`
- Report: `docs/benchmarks/live-behavior-repair-memory-save-rerun-20260528-151753/memory-save-quality-gate/report.md`
- Pass rate: `1/1`
- Real code-change passes: `1`
- Behavior assertions passed: `1/1`
- Required validation: `4/4`
- Full harness tests: `2076 passed`
- Patch synthesis no-change: `false`
- Tool failures: `0`
- Failure owner: `none`

Observed runtime behavior:

- The first no-diff validation failure emitted the structured repair observation.
- Focused repair entered patch synthesis.
- Patch synthesis applied three successful `file_edit` calls.
- The isolated worktree ended with a real diff in `src/memory/quality.rs` and
  `src/tui/app.rs`.
- Closeout was verified by `command_passed,required_validation_passed`.

## Failure Ownership Policy

Framework bug:

- failed required validation is hidden from the next model turn;
- empty diff is treated as verified;
- closeout is allowed after failed behavior assertions;
- focused repair mode permits validation loops before a patch;
- trace lacks evidence that the no-effective-diff repair path fired.

LLM bug, framework handled correctly:

- model receives the structured observation but still edits the wrong file;
- model refuses to patch and keeps inspecting within allowed focused lookup;
- model patches but the behavior assertion still fails and closeout is blocked.

## Follow-Up

After this slice, the next useful improvement is empirical rather than
architectural: rerun one or two previously failed behavior-assertion cases and
compare whether the model reaches a real diff faster. If pass rate still does not
move, improve the repair observation text or patch-synthesis evidence packet
before touching the main loop.
