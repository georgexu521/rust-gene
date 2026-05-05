# Scripts Guide

This directory mixes installer, CI helpers, and workflow research scripts.
Use this file as the source of truth for what is on the critical path.

## Core (user-facing)

- `install.sh`: one-command local install (`make install` calls this).

## CI/Automation Path

- `lint-check.sh`: used by CI lint/check validation workflows.
- `workflow-production-gates.sh`: orchestrates workflow gate scripts.
- `workflow-m1-acceptance.sh`: acceptance checks for workflow milestones.
- `workflow-gate-replay.sh`: gate replay harness.
- `workflow-param-replay.sh`: parameter replay harness.
- `workflow-weekly-report.sh`: weekly metrics reporting.
- `workflow-real-devflow-round2.sh`: replay fixture wrapper.
- `workflow-real-devflow-round3.sh`: replay fixture wrapper.
- `run_live_eval.sh`: semi-automatic live coding task regression harness. It can
  prepare task worktrees, ask MiniMax for a planning response, and collect
  diff/test/report artifacts. Agent-run reports include a `Specialty signals`
  section that summarizes memory, automation/validation, guided debugging,
  guided reasoning, weighted planning, and closeout activity for real-task
  review. Task YAML parsing uses the system Ruby YAML/JSON stdlib, so
  prepare/collect/report paths do not require installing PyYAML.
- `live-eval-summary-smoke.sh`: deterministic fixture test for
  `run_live_eval.sh --mode summary`; validates pass rates, plan-only separation,
  real code-change pass classification, and seeded no-diff failure modes without
  running an LLM.
- `coding-workflow-gates.sh`: layered coding-agent workflow gates. Use `quick`
  for focused deterministic edit/validation/repair/closeout contracts,
  `standard` before workflow/tool commits, `full` for docs/build/full-test
  closeout, and `live-smoke` only when the real agent path needs a live check.

## Manual Dev/Ops Utilities

- `health-check.sh`: API health probe for local/manual ops.
- `audit-api-smoke.sh`: audit endpoint smoke test.
- `benchmark.sh`: local performance benchmark helper.
- `validate_docs.sh`: manual docs/build consistency checker.
- `generate-gate-replay-v2.sh`: regenerate replay samples json.

## Policy

- Prefer adding new scripts to one of the three sections above.
- If a script is manual-only, keep it out of CI and document it here.
- If a script is obsolete, remove it instead of leaving it undocumented.
