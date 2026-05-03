# Real Project Flow Observation Plan

Date: 2026-05-03

This plan uses `rust-agent` itself as the programming project. The feature is
small enough to finish in one focused batch, but real enough to exercise the
coding workflow and produce reusable evidence.

## Project Task

Add a specialty-observation section to live eval reports so each real agent run
shows whether the project's distinctive systems were active:

- Memory and retrieval signals.
- Automation and validation signals.
- Guided debugging signals.
- Workflow planning and weight/priority signals.
- Closeout and acceptance signals.

## Why This Task

The current live eval report already records raw quality signals. What is
missing is a compact view that answers: did our memory, automation, guided
debugging, and weighted planning systems actually participate in the task?

This is useful for every future real programming run and avoids subjective
manual review.

## Execution Plan

- [x] Inspect current `scripts/run_live_eval.sh` report generation.
- [x] Add a deterministic specialty-signal analyzer to the report path.
- [x] Document the report section in `scripts/README.md`.
- [x] Validate the script syntax and live-eval listing.
- [x] Run focused workflow gates after the change.
- [x] Run one real agent task or collect a representative report to inspect the
  new section.
- [x] Record observations and commit the result.

## Observation Checklist

During the run, inspect whether the report exposes:

- [x] `memory_active`: memory sync, retrieval, or memory tool evidence.
- [x] `automation_active`: required commands, stage validation, or validation
  progress evidence.
- [ ] `guided_debugging_active`: guided debugging trace evidence.
- [x] `weighted_planning_active`: workflow plan event with priority/importance
  fields.
- [x] `closeout_active`: final closeout and acceptance review evidence.

## First Observation

Representative report:

`docs/benchmarks/live-live-eval-20260503-152320/code-change-verification-repair-loop/report.md`

Observed specialty signals:

- `memory_active=true`: 6 memory sync events; retrieval used Project and
  Session sources.
- `automation_active=true`: 5 required commands, `required_command_status=ok`,
  one verification event, one stage validation event, and one progress event.
- `guided_reasoning_active=true`: workflow judgment required guided reasoning.
- `weighted_planning_active=true`: one weighted plan event exposed `P0`,
  `top_importance_score=0.9025000333786011`, and
  `top_weight_share=0.2397078275680542`.
- `closeout_active=true`: acceptance accepted and closeout status passed.
- `guided_debugging_active=false`: expected for this successful repair path;
  validate it with a deliberately failing/blocking live task in the next round.

## Fresh Agent Run Observation

Run:

`docs/benchmarks/live-realflow-memory-20260503-163910/persistent-memory-planning-context/report.md`

Why this task was selected:

- It is a real bug-fix task against `rust-agent`.
- The fixture removes persistent memory prefetch before workflow judgment.
- Acceptance requires memory/planning tests, retrieval-context tests, an
  ordering assertion that prefetch happens before learning application, and the
  full test suite.

Result:

- Status: passed.
- Required command status: ok.
- Full suite: `1053 passed; 0 failed`.
- Changed file: `src/engine/conversation_loop/mod.rs`.
- Diff size: 1 file, 28 lines.
- Tool executions: 12.
- Trace events: 73.

Specialty signals:

- `memory_active=true`: 6 memory sync events.
- `automation_active=true`: required commands, verification, stage validation,
  and progress events were present.
- `guided_reasoning_active=true`: workflow judgment used guided reasoning.
- `weighted_planning_active=true`: workflow plan exposed priority/importance
  fields.
- `closeout_active=true`: acceptance review accepted and closeout passed.
- `guided_debugging_active=false`: no guided-debug event fired because final
  validation passed; the one failed bash tool was handled by action-checkpoint
  patch synthesis rather than the guided-debugging contract.

Flow notes:

- This was a genuinely slow path: release build, model/tool loop, required
  command reruns, and full test validation.
- The report shows the planning weight surface, but the latest top priority was
  `P3` with low importance (`0.05000000074505806`). That is worth reviewing:
  the signal exists, but the weight ordering may not be intuitive for a high-risk
  memory/planning bug.
- Guided debugging still needs a dedicated failing/blocking run; successful
  repair paths do not necessarily trigger it.

## Follow-Up Fix

The `P3 / 0.05` weight observation was traced to model-supplied high-risk
workflow factors that can be all zero. Since factors take precedence over the
model priority label, all-zero factors collapse the computed importance to the
minimum clamp.

Fix applied:

- Treat all-zero high-risk factors as invalid and fall back to the model's
  priority label.
- If a high-risk plan still has no step at `P2` or above, promote the first
  step to a conservative `P2` floor.
- Add a regression test:
  `high_risk_zero_factor_plan_gets_actionable_priority_floor`.

Validated with:

```bash
cargo test -q workflow_contract -- --test-threads=1
scripts/coding-workflow-gates.sh quick
scripts/coding-workflow-gates.sh standard
```

## Guided Debugging Run Observation

Run:

`docs/benchmarks/live-realflow-guided-20260503-170614/memory-save-quality-gate/report.md`

Why this task was selected:

- It is a high-risk memory quality-gate bug.
- The task prompt explicitly tells the agent not to satisfy `/save` by changing
  display text elsewhere; it must fix `src/tui/app.rs` slash-command behavior.
- Historical runs showed this task can trigger guided debugging.

Result:

- Status: failed.
- Failure owner: `llm_reasoning`.
- Required command status: failed.
- Full suite still passed: `1054 passed; 0 failed`.
- Required acceptance grep failed because `src/tui/app.rs` still contained two
  `format!("Saved: {}")` call sites.
- Changed files: `src/memory/quality.rs`, `src/tools/memory_tool/mod.rs`.

Specialty signals:

- `memory_active=true`
- `automation_active=true`
- `guided_debugging_active=true`
- `guided_reasoning_active=true`
- `weighted_planning_active=true`
- `closeout_active=true`
- `active_specialty_signals=6/6`
- `guided_debugging_events=4`
- `latest_top_priority=P1`
- `latest_top_importance_score=0.7150000333786011`

Interpretation:

- The workflow systems did their job: validation failed, guided debugging
  triggered, acceptance rejected the result, and closeout stayed failed instead
  of claiming success.
- The model still failed to execute the full task surface: it repaired the
  memory tool and quality scorer but missed the explicitly required TUI `/save`
  path.
- This should not be fixed by adding more hidden runtime tuning. The right next
  improvement is to tell the AI more clearly how to enumerate and close every
  acceptance target before editing and before final closeout.

## Validation

Required local checks:

```bash
bash -n scripts/run_live_eval.sh
scripts/run_live_eval.sh --list
scripts/coding-workflow-gates.sh quick
```

Run broader gates only if the implementation touches Rust code or critical
workflow contracts.
