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

## Validation

Required local checks:

```bash
bash -n scripts/run_live_eval.sh
scripts/run_live_eval.sh --list
scripts/coding-workflow-gates.sh quick
```

Run broader gates only if the implementation touches Rust code or critical
workflow contracts.
