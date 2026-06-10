# Flow Stabilization Test Run - 2026-05-27

## 1) Scope

Executed Phase 0 baseline run from
`docs/FLOW_STABILIZATION_TEST_PLAN_2026-05-27.md`, then started Phase 1
targeted reruns to separate flow bugs from model mistakes.

## 2) Commands Executed

```bash
cargo fmt --check
cargo check -q
bash scripts/coding-workflow-gates.sh quick

bash scripts/run_live_eval.sh --case mvp-weighted-agent --mode agent-run --run-tests --run-id flow-mva-20260527-083214 --label flow-mva
bash scripts/run_live_eval.sh --mode summary --run-id flow-mva-20260527-083214

bash scripts/run_live_eval.sh --case real-project-coding --mode agent-run --run-tests --run-id flow-real-20260527-084801 --label flow-real
bash scripts/run_live_eval.sh --mode summary --run-id flow-real-20260527-084801

bash scripts/run_live_eval.sh --case minimum-agent-verification-repair --mode agent-run --run-tests --run-id flow-rerun-minver-20260527-091108 --label flow-rerun-minver
bash scripts/run_live_eval.sh --mode summary --run-id flow-rerun-minver-20260527-091108

bash scripts/run_live_eval.sh --case core-permission-rejection-recovery --mode agent-run --run-tests --run-id flow-rerun-perm-20260527-091212 --label flow-rerun-perm
bash scripts/run_live_eval.sh --mode summary --run-id flow-rerun-perm-20260527-091212

bash scripts/run_live_eval.sh --case core-terminal-install-run --mode agent-run --run-tests --run-id flow-rerun-terminal-20260527-091311 --label flow-rerun-terminal
bash scripts/run_live_eval.sh --mode summary --run-id flow-rerun-terminal-20260527-091311

bash scripts/run_live_eval.sh --case backend-todo-api-crud --mode agent-run --run-tests --run-id flow-rerun-backend-20260527-095340 --label flow-rerun-backend
bash scripts/run_live_eval.sh --mode summary --run-id flow-rerun-backend-20260527-095340

bash scripts/run_live_eval.sh --case core-inspection-grounding --mode agent-run --run-tests --run-id flow-rerun-inspection-20260527-095628 --label flow-rerun-inspection
bash scripts/run_live_eval.sh --mode summary --run-id flow-rerun-inspection-20260527-095628

bash scripts/run_live_eval.sh --case core-long-output-artifact --mode agent-run --run-tests --run-id flow-rerun-longout-20260527-095732 --label flow-rerun-longout
bash scripts/run_live_eval.sh --mode summary --run-id flow-rerun-longout-20260527-095732

bash scripts/run_live_eval.sh --case core-provider-roundtrip --mode agent-run --run-tests --run-id flow-rerun-provider-20260527-100711 --label flow-rerun-provider
bash scripts/run_live_eval.sh --mode summary --run-id flow-rerun-provider-20260527-100711

bash scripts/run_live_eval.sh --case core-rollback-product-path --mode agent-run --run-tests --run-id flow-rerun-rollback-20260527-101217 --label flow-rerun-rollback
bash scripts/run_live_eval.sh --mode summary --run-id flow-rerun-rollback-20260527-101217

bash scripts/run_live_eval.sh --case core-provider-roundtrip --mode agent-run --run-tests --run-id flow-fix-provider2-20260527-104742 --label flow-fix-provider2
bash scripts/run_live_eval.sh --mode summary --run-id flow-fix-provider2-20260527-104742

bash scripts/run_live_eval.sh --case core-rollback-product-path --mode agent-run --run-tests --run-id flow-fix-rollback3-20260527-105638 --label flow-fix-rollback3
bash scripts/run_live_eval.sh --mode summary --run-id flow-fix-rollback3-20260527-105638
```

## 3) Baseline Results

### Deterministic gate

- `cargo fmt --check`: pass
- `cargo check -q`: pass
- `coding-workflow-gates.sh quick`: pass

### Live suites

| Run id | Suite | Result |
| --- | --- | --- |
| `flow-mva-20260527-083214` | `mvp-weighted-agent` | `6/7` pass (`1` fail) |
| `flow-real-20260527-084801` | `real-project-coding` | `4/11` pass (`7` fail) |

`flow-real` failure owner distribution:

- `agent_flow=4`
- `llm_reasoning=1`
- `mixed=2`

## 4) Phase 1 Rerun Results (Targeted)

| Run id | Case | Result | Owner |
| --- | --- | --- | --- |
| `flow-rerun-minver-20260527-091108` | `minimum-agent-verification-repair` | pass | `none` |
| `flow-rerun-perm-20260527-091212` | `core-permission-rejection-recovery` | fail | `llm_reasoning` |
| `flow-rerun-terminal-20260527-091311` | `core-terminal-install-run` | fail | `mixed` |
| `flow-rerun-backend-20260527-095340` | `backend-todo-api-crud` | pass | `none` |
| `flow-rerun-inspection-20260527-095628` | `core-inspection-grounding` | fail | `agent_flow` |
| `flow-rerun-longout-20260527-095732` | `core-long-output-artifact` | fail | `mixed` |
| `flow-rerun-provider-20260527-100711` | `core-provider-roundtrip` | fail | `agent_flow` |
| `flow-rerun-rollback-20260527-101217` | `core-rollback-product-path` | fail | `agent_flow` |

Key signal:

- `minimum-agent-verification-repair` moved from failed (in suite) to single-case
  pass, so that specific failure is not yet a stable flow regression.
- `backend-todo-api-crud` moved from suite failure to single-case pass, so it is
  also not a stable regression.
- stable `agent_flow` failures after rerun:
  `core-inspection-grounding`, `core-provider-roundtrip`,
  `core-rollback-product-path`.
- `core-permission-rejection-recovery` is stable `llm_reasoning` failure with
  honest `not_verified` closeout.
- stable `mixed` failures:
  `core-terminal-install-run`, `core-long-output-artifact`.

## 5) Immediate Next Steps

1. Start fixing stable `agent_flow` cases first:
   - `core-inspection-grounding`
   - `core-provider-roundtrip`
   - `core-rollback-product-path`
2. Keep `llm_reasoning` lane (`core-permission-rejection-recovery`) under strict
   closeout policy and improve repair evidence quality only.
3. Split `mixed` lane for harness/env vs model behavior:
   - `core-terminal-install-run`
   - `core-long-output-artifact`
4. After `agent_flow` fixes land, rerun `real-project-coding` full suite.

## 6) Policy Update Confirmed (2026-05-27)

Stabilization policy is updated to no hard wall-clock kill for agent runs.
Execution control should rely on liveness evidence and explicit state
classification (`running/waiting_model/waiting_tool/stalled/looping`) instead
of fixed-duration termination.

## 7) Agent-Flow Patch Batch A (2026-05-27)

### Code changes

1. `tool_exposure_plan`:
   - allow validation tools in action-checkpoint mode when
     `required_validation_commands_present=true` even before file changes.
2. `run_tests` tool:
   - normalize `cd <current_workdir> && <safe validation command>` into
     `<safe validation command>` before classification/execution;
   - keep rejecting `cd` to other directories;
   - add regression tests for both paths.

### Focused deterministic tests

- `cargo test -q action_checkpoint_keeps_validation_tools_when_required_commands_exist_without_changes` ✅
- `cargo test -q run_tests_accepts_safe_command_with_workdir_cd_prefix` ✅
- `cargo test -q run_tests_rejects_cd_to_different_absolute_directory` ✅
- `cargo fmt --check` ✅

## 8) Post-patch reruns

| Run id | Case | Result | Owner | Key signal |
| --- | --- | --- | --- | --- |
| `flow-fix-inspection-20260527-102551` | `core-inspection-grounding` | pass | `none` | `verification_proof_status=verified` |
| `flow-fix-provider-20260527-102908` | `core-provider-roundtrip` | fail | `agent_flow` | `required validation missing 1/1` |
| `flow-fix-rollback2-20260527-103737` | `core-rollback-product-path` | fail | `agent_flow` | `required validation missing 2/2` |

Interpretation:

- Batch A fixed one stable flow defect (`inspection` now green).
- `provider/rollback` no longer show the previous `run_tests` invalid-params
  signature from `cd <workdir> && ...`, but still fail because required
  validation is never actually executed by the agent turn.

## 9) Infra incident and mitigation

- Incident: one rollback rerun failed with `No space left on device`
  (`flow-fix-rollback-20260527-103347`) and produced incomplete artifacts.
- Mitigation performed: cleared transient workspace artifacts under
  `target/live-evals/*`.
  - Before: `target/live-evals=26G`, `target=44G`
  - After: `target/live-evals=0B`, `target=18G`

## 10) Agent-Flow Patch Batch B Verification (2026-05-27)

### Code change

- `post_change_workflow_controller` now runs `required_validation_commands`
  even when `changed_files` is empty, records command evidence into
  `evidence_ledger` (`source=required_validation`), updates
  `successful_required_validation_commands`, and feeds failures back into
  context/tool text.

### Deterministic regression test

- `cargo test -q runs_required_validation_even_without_changed_files` ✅

### Post-Batch-B reruns

| Run id | Case | Result | Owner | Key signal |
| --- | --- | --- | --- | --- |
| `flow-fix-provider2-20260527-104742` | `core-provider-roundtrip` | pass | `none` | `required validation passed 1/1` |
| `flow-fix-rollback3-20260527-105638` | `core-rollback-product-path` | pass | `none` | `required validation passed 2/2` |

Interpretation:

- The remaining `agent_flow` failure signature (`required validation missing`)
  is cleared.
- The three previously stable `agent_flow` cases are now all green:
  `core-inspection-grounding`, `core-provider-roundtrip`,
  `core-rollback-product-path`.
- In no-hard-timeout mode, long validation commands still complete with liveness
  evidence (`agent-stderr.log` periodic progress lines), so they should be
  treated as running instead of stalled.

## 11) Updated Next Steps

1. Re-run `real-project-coding` full suite after Patch B to measure aggregate
   owner distribution shift.
2. Continue `mixed` lane triage:
   - `core-terminal-install-run`
   - `core-long-output-artifact`
3. Keep `llm_reasoning` lane (`core-permission-rejection-recovery`) under strict
   verification gate and improve repair/closeout evidence only.

## 12) Real Suite Re-run After Patch B (2026-05-27)

### Commands

```bash
bash scripts/run_live_eval.sh --case real-project-coding --mode agent-run --run-tests --run-id flow-real-postb-20260527-110105 --label flow-real-postb
bash scripts/run_live_eval.sh --mode summary --run-id flow-real-postb-20260527-110105
```

### Result snapshot

| Run id | Suite | Result |
| --- | --- | --- |
| `flow-real-postb-20260527-110105` | `real-project-coding` | `7/11` pass (`4` fail) |

Owner distribution:

- `none=7`
- `mixed=2`
- `llm_reasoning=1`
- `eval_harness=1`
- `agent_flow=0`

Delta vs baseline `flow-real-20260527-084801`:

- pass rate: `4/11 -> 7/11`
- `agent_flow` failures: `4 -> 0`

### Case-level highlights

- `core-inspection-grounding`: pass (was stable `agent_flow` fail before fixes)
- `core-provider-roundtrip`: pass (required validation `1/1`)
- `core-rollback-product-path`: pass (required validation `2/2`)
- remaining stable fails are non-`agent_flow` lanes:
  - `core-terminal-install-run` (`mixed`)
  - `core-long-output-artifact` (`mixed`)
  - `core-permission-rejection-recovery` (`llm_reasoning`)

## 13) Infra Incident During Post-B Suite

- Incident time: during `flow-real-postb-20260527-110105` second half.
- Symptom: repeated `No space left on device` while writing reports/worktrees.
- Immediate impact: task artifacts for later tasks were partially degraded; run
  still produced `summary.md` for the 11-task suite.

Mitigation executed:

- `rm -rf target/live-evals/*`
- capacity recovered from `Avail=116Mi` to `Avail=25Gi`
- post-cleanup footprint:
  - `target/live-evals=0B`
  - `docs/benchmarks=32M`

## 14) Mixed-Lane Patch C Verification (2026-05-27)

### Code changes

1. `tool_exposure_plan`:
   - keep `bash`/`run_tests` exposed in `Edit` stage when required validation
     commands exist.
2. `action_policy`:
   - fix live-eval `worktree` root path classification
     (`.../worktree` and `.../worktree/...` both treated as workspace path).
3. `validation_runner`:
   - do not delete `.venv` after required-validation preflight;
   - for `core_terminal_demo`, add/repair preflight via local `.pth` injection;
   - run repair path even when `.venv` already exists but marker is missing.
4. `run_live_eval.sh`:
   - exclude `.venv/` files from `max_files_changed` counting.

### Deterministic checks

- `cargo test -q programming_edit_stage_keeps_validation_tools_when_required_commands_exist` ✅
- `cargo test -q live_eval_worktree_root_path_is_workspace_not_dependency_path` ✅
- `cargo test -q preflight_` ✅ (`10` passed)
- `bash -n scripts/run_live_eval.sh` ✅

### Post-Patch-C reruns

```bash
bash scripts/run_live_eval.sh --case core-long-output-artifact --mode agent-run --run-tests --run-id flow-fix-longout-preflight-20260527-123237 --label flow-fix-longout-preflight
bash scripts/run_live_eval.sh --mode summary --run-id flow-fix-longout-preflight-20260527-123237

bash scripts/run_live_eval.sh --case core-terminal-install-run --mode agent-run --run-tests --run-id flow-fix-terminal-preflight7-20260527-132300 --label flow-fix-terminal-preflight7
bash scripts/run_live_eval.sh --mode summary --run-id flow-fix-terminal-preflight7-20260527-132300
```

| Run id | Case | Result | Owner | Key signal |
| --- | --- | --- | --- | --- |
| `flow-fix-longout-preflight-20260527-123237` | `core-long-output-artifact` | pass | `none` | `required validation passed 3/3` |
| `flow-fix-terminal-preflight7-20260527-132300` | `core-terminal-install-run` | pass | `none` | `required validation passed 2/2` |

Interpretation:

- Previously stable `mixed` cases are now both green and reclassified to
  `failure_owner=none`.
- Required validation commands are executing normally and producing verified
  proof (`required_command_status=ok`).
- Tool calls are functionally healthy for these two cases (no
  `tool_not_exposed`, no required-command missing, no closeout mismatch).

## 15) Real Suite Re-run After Patch C (2026-05-27)

### Commands

```bash
bash scripts/run_live_eval.sh --case real-project-coding --mode agent-run --run-tests --run-id flow-real-postc-20260527-134900 --label flow-real-postc
bash scripts/run_live_eval.sh --mode summary --run-id flow-real-postc-20260527-134900
```

### Result snapshot

| Run id | Suite | Result |
| --- | --- | --- |
| `flow-real-postc-20260527-134900` | `real-project-coding` | `9/14` pass (`5` fail) |

Owner distribution:

- `none=9`
- `llm_reasoning=5`
- `agent_flow=0`

Interpretation:

- `agent_flow` remains `0` after mixed-lane fixes.
- Remaining scored failures in this run are all `llm_reasoning` and share the
  same direct signature: required validation failed and closeout failed.
- mixed lane is now stable green inside the full suite:
  - `core-terminal-install-run`: pass, `required validation passed 2/2`
  - `core-long-output-artifact`: pass, `required validation passed 3/3`

### Failed scored tasks in this run

- `backend-todo-api-crud` (`failure_owner=llm_reasoning`)
- `code-change-verification-repair-loop` (`failure_owner=llm_reasoning`)
- `core-permission-rejection-recovery` (`failure_owner=llm_reasoning`)
- `memory-save-quality-gate` (`failure_owner=llm_reasoning`)
- `skill-promotion-gate` (`failure_owner=llm_reasoning`)

## 16) Infra Incident During Patch-C Suite

- Incident: disk filled again during post-14th-task memory lane execution.
- Primary error: `No space left on device`.
- Impact:
  - `persistent-memory-planning-context` report artifacts became partial;
  - subsequent memory cases could not fully materialize report directories.
- Immediate mitigation executed:
  - `rm -rf target/live-evals/*`
  - capacity recovered to approximately `22Gi` free.

Note: this incident is infra/harness-level and should not be attributed to
agent-flow logic or model reasoning quality for the affected non-scored tail
tasks.

## 17) Long-Run Liveness And Disk Hygiene Patch (2026-05-27)

### Code changes

1. Required validation liveness:
   - default required-validation execution no longer has a fixed wall-clock
     timeout;
   - `PRIORITY_AGENT_REQUIRED_VALIDATION_TIMEOUT_SECS` remains available as an
     explicit opt-in cap;
   - 30s heartbeat logging remains active while a validation command is still
     running.
2. Agent-run liveness:
   - live-eval `agent-run` defaults changed to no wall-clock timeout
     (`--timeout 0`);
   - idle kill is also opt-in (`--idle-timeout 0` by default);
   - each agent task now writes `agent-monitor.log` with periodic liveness
     records that include elapsed time, idle time, and output/event file sizes.
3. Disk and artifact hygiene:
   - live-eval now uses one shared Cargo target directory per run by default
     (`target/live-evals/<run-id>/shared-cargo-target`) instead of a full
     per-task target directory;
   - each task checks available disk before starting and fails fast when below
     `PRIORITY_AGENT_LIVE_EVAL_MIN_FREE_GB` (default `8`);
   - generated dependency paths such as `.venv/` are excluded from
     `diff_files_changed` summary counts as well as `max_files_changed`.

### Validation

- `bash -n scripts/run_live_eval.sh` ✅
- `cargo fmt --check` ✅
- `cargo check -q` ✅
- `cargo test -q preflight_` ✅ (`10` passed)
- `cargo test -q runs_required_validation_even_without_changed_files` ✅
- `cargo test -q test_required_validation_shell_strips_agent_runtime_env` ✅
- `cargo test -q programming_edit_stage_keeps_validation_tools_when_required_commands_exist` ✅
- Disk preflight smoke:
  `PRIORITY_AGENT_LIVE_EVAL_MIN_FREE_GB=999999 bash scripts/run_live_eval.sh --case core-inspection-grounding --mode prepare --run-id disk-preflight-smoke`
  failed early with a clear disk-preflight message, as expected.

Interpretation:

- Long validations should now be treated as running while they keep producing
  liveness evidence instead of being stopped by a fixed wall-clock cap.
- A genuinely quiet or suspicious run can still be bounded explicitly by passing
  `--idle-timeout <seconds>` or `--timeout <seconds>`.
- The next full suite should consume substantially less disk because Rust build
  artifacts are shared across tasks.
