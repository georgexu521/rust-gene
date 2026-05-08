# Manual Smoke Report: UX Runtime

Generated: 2026-05-08 21:05 +0800

Purpose: run the manual UX smoke layer from
`docs/AGENT_TESTING_MATRIX_2026-05-08.md` against the current checkout, then
compare it with one representative live agent eval.

## Local Deterministic Gates

| Gate | Result | Notes |
| --- | --- | --- |
| `scripts/coding-workflow-gates.sh standard` | passed after fix | Initial failure was a stale smoke assertion. `summary.md` now includes `runtime_diet`, but `scripts/live-eval-summary-smoke.sh` still expected the old row shape. |
| `cargo clippy --all-features -- -D warnings` | passed | Completed in dev profile without warnings. |
| `scripts/coding-workflow-gates.sh full` | passed | `cargo test`: 1110 passed, 0 failed. |

Fix made during testing:

- Updated `scripts/live-eval-summary-smoke.sh` to expect the current
  `runtime_diet` column in summary task rows.

## Manual UX Smoke Results

Artifacts are under `target/manual-smoke/results/`.

| Smoke | Output file | Tools used | Result | Finding |
| --- | --- | --- | --- | --- |
| Desktop truth check | `desktop_truth.out.md` | `file_read` | partial | Correctly proved `/Users/georgexu/Desktop/gex/` exists, but final answer appended code-change closeout text. |
| Terminal Python check | `terminal_python_check.out.md` | `bash` | partial | Correctly checked `pygame 2.6.1`, but final answer appended code-change closeout text. |
| Simple code creation | `simple_code_creation.out.md` | `file_write` plus auto verification | partial | Wrote `hello.py` and auto validation passed, but final answer was only mechanical closeout and did not name the validation command. |
| Exact delete scope | `delete_scope.out.md` | `bash` | passed | Deleted only the requested temp file and kept the parent directory. |
| No-code error explanation | `no_code_answer.out.md` | none | partial | Correctly avoided tools and explained the MiniMax tool-call-order error, but final answer appended code-change closeout text. |

## Root Cause Signals

The repeated user-facing issue is not the model choosing the wrong tool. It is
runtime routing and closeout applying a code-change workflow to non-code-change
turns.

Trace evidence:

- Desktop truth check routed as `CodeChange` with reason `prompt asks for code or
  product changes`, despite being a filesystem inspection request.
- Terminal Python check routed as `Debugging -> BugFix`, despite being a
  read-only environment check.
- No-code error explanation routed as `Debugging -> BugFix`, despite explicit
  instruction to avoid tools and only explain the error.
- These misroutes recorded `implementation_intent_recorded` and
  `final_closeout_prepared`, which appended `Done with caveats... no changed
  files recorded...` to otherwise good answers.

This matches the live-use concern: too much workflow structure is still leaking
into simple turns.

## Post-fix Rerun

Routing and closeout were tightened after the initial smoke:

- Chinese creation words such as "创建时间" no longer count as a code creation
  request unless they appear as a natural artifact-creation phrase.
- "工具输出" no longer counts as a code artifact just because it contains
  "工具".
- Error-explanation prompts route to direct answer when the user asks "为什么" or
  "怎么回事" without requesting a fix.
- Planning signals are checked before code-creation/local-inspection signals so
  roadmap/design prompts are not stolen by broad action words.
- Non-code direct routes suppress deterministic closeout; code-change routes
  still keep concise validation closeout.
- Code-change concise closeout now preserves a short command-level validation
  evidence item, preferring evidence that names the changed file.

Artifacts:

| Smoke | Output file | Route | Tools used | Result | Finding |
| --- | --- | --- | --- | --- | --- |
| Desktop truth check | `desktop_truth.final.out.md` | `DirectAnswer / Direct / Light` | `file_read` | passed | Correctly reported that `/Users/georgexu/Desktop/gex/` exists; no fabricated metadata and no closeout leakage. |
| Terminal Python check | `terminal_python_check.fixed.out.md` | `DirectAnswer / Direct / Light` | `bash` | passed | Actually executed the import/version check and reported Python 3.11.8 plus pygame 2.6.1; no closeout leakage. |
| No-code error explanation | `no_code_answer.final.out.md` | `DirectAnswer / Direct / Light` | none | passed | Explained the MiniMax tool-call-order error without using tools and without bug-fix closeout. |
| Simple code creation | `simple_code_creation.final.out.md` | `CodeChange / CodeChange / Project` | `file_write`, `bash`, auto validation | passed | Wrote `hello.py` and the concise closeout now names `python3 -m py_compile /Users/georgexu/Desktop/rust-agent/target/manual-smoke/simple-code/hello.py`. |

## Representative Live Eval

Command:

```bash
scripts/run_live_eval.sh \
  --case code-change-verification-repair-loop \
  --mode agent-run \
  --run-tests \
  --timeout 1800 \
  --idle-timeout 300 \
  --label capability-now
```

Report:

- `docs/benchmarks/live-capability-now-20260508-205418/code-change-verification-repair-loop/report.md`
- `docs/benchmarks/live-capability-now-20260508-205418/summary.md`

Result:

| Metric | Value |
| --- | --- |
| Status | passed |
| Real code diff | yes |
| Required commands | all passed |
| First write tool index | 6 |
| Closeout status | passed |
| Failure owner | none |
| Runtime diet | `prompt=5483 tool_schema=2930 tools=12 workflow=strict closeout=full validation=passed` |

Important positive signal:

- The agent did not repeat the previous "inspect but no edit" failure mode.
  It used `grep`, `file_read`, `file_edit`, and `bash`, produced a focused diff,
  and required validation passed.

## Current Assessment

The current runtime now handles the previously failing read-only/support turns
without code-change workflow leakage, while preserving real code-change
validation.

Strong signals:

- Deterministic gates pass.
- Clippy passes.
- Full local validation passes.
- Destructive-scope smoke passed.
- The representative live code-change eval passed with a real diff and required
  validation.
- Post-fix manual reruns pass for filesystem truth, terminal environment check,
  no-tool error explanation, and simple code creation.

Weak signals:

- The full five-case live suite has not been rerun after this focused
  routing/closeout patch; the representative code-change live eval from this
  session remains passed.

## Recommended Next Fix

Run the full five-case live suite and use it as the next product-level signal:

1. Add regression assertions for the four post-fix smoke routes so broad Chinese
   words such as "创建时间" and "工具输出" do not reintroduce code-change routing.
2. Run the full five-case live suite after the closeout wording fix.
3. If any case still fails, classify it as routing, tool exposure, provider
   protocol, or model reasoning before changing prompts again.

Suggested tests:

```bash
cargo test -q intent_router
cargo test -q runtime_diet
cargo test -q closeout
cargo test -q prompt_context
scripts/coding-workflow-gates.sh standard
```
