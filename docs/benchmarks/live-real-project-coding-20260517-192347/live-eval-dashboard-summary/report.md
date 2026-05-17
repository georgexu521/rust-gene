# Live Eval Report: live-eval-dashboard-summary

- Run id: `real-project-coding-20260517-192347`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-192347/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-192347/live-eval-dashboard-summary/env`
- Test status: `ok`
- Generated: `2026-05-17 19:48:51 +0800`

## Git Status

```text
 M scripts/run_live_eval.sh
?? docs/benchmarks/live-live-summary-smoke/
?? scripts/__pycache__/
```

## Diff Stat

```text
 scripts/run_live_eval.sh | 94 +++++++++++++++++++++++++++++++++++++++++++++---
 1 file changed, 89 insertions(+), 5 deletions(-)
```

## Required Commands

```text
$ bash -n scripts/run_live_eval.sh
[exit status: 0]

$ scripts/run_live_eval.sh --list
id                                   type         eval_intent                risk       title
--                                   ----         -----------                ----       -----
backend-todo-api-crud                feature      seeded_code_change         medium     implement a tiny stdlib todo API backend
cli-scrollback-polish                ux           audit_or_regression_check  medium     interactive CLI should feel smooth and readable
code-change-verification-repair-loop feature      seeded_code_change         high       failed verification should trigger repair before closeout
core-inspection-grounding            audit        audit_or_regression_check  low        inspect filesystem facts without hallucinating metadata
core-long-output-artifact            runtime      audit_or_regression_check  medium     preserve and inspect long command output
core-multi-file-edit                 feature      seeded_code_change         medium     coordinate a two-file code and docs edit
core-permission-rejection-recovery   bug_fix      seeded_code_change         high       recover after user rejects a destructive cleanup
core-provider-roundtrip              protocol     audit_or_regression_check  medium     verify provider tool-call and tool-result protocol support
core-rollback-product-path           audit        audit_or_regression_check  medium     verify file history rollback is a product path
core-simple-stale-edit               bug_fix      seeded_code_change         medium     read before a focused single-file edit
core-terminal-install-run            runtime      audit_or_regression_check  medium     install a local Python package and run it through the terminal
frontend-book-notes-localstorage     feature      seeded_code_change         medium     build a small book notes frontend with search, tags, and persistence
live-eval-dashboard-summary          feature      seeded_code_change         medium     live eval reports should summarize pass rates and failure modes
memory-recall-conflict-precision     bug_fix      audit_or_regression_check  high       memory recall should demote only relevant conflicts
memory-save-duplicate-demotion       bug_fix      audit_or_regression_check  medium     duplicate memory candidates should not pollute long-term memory
memory-save-quality-gate             bug_fix      seeded_code_change         high       memory_save should respect quality gates
memory-save-sensitive-hard-block     bug_fix      audit_or_regression_check  high       explicit memory saves must not persist sensitive data
permission-default-open-dangerous-guard bug_fix      audit_or_regression_check  high       default-open permissions should still guard destructive operations
persistent-memory-planning-context   bug_fix      seeded_code_change         high       persistent memory should affect workflow planning
resume-session-picker                feature      seeded_code_change         medium     interactive CLI should support Claude-style resume
skill-promotion-gate                 bug_fix      seeded_code_change         medium     skill apply should require promotion evidence
[exit status: 0]

$ scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke
Summary written to docs/benchmarks/live-live-summary-smoke/summary.md
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1450 tests
....................................................................................... 87/1450
....................................................................................... 174/1450
....................................................................................... 261/1450
....................................................................................... 348/1450
....................................................................................... 435/1450
....................................................................................... 522/1450
....................................................................................... 609/1450
....................................................................................... 696/1450
....................................................................................... 783/1450
....................................................................................... 870/1450
....................................................................................... 957/1450
....................................................................................... 1044/1450
....................................................................................... 1131/1450
....................................................................................... 1218/1450
....................................................................................... 1305/1450
....................................................................................... 1392/1450
..........................................................
test result: ok. 1450 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.90s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-real-project-coding-20260517-192347/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-192347/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 5
tool_execution_progress: 1
tool_execution_start: 5
trace_summary: 1
```

Quality signals:

```text
output_chars: 980
diff_chars: 3796
diff_files_changed: 1
tool_executions: 5
first_write_tool_index: 5
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 56
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=6050 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:5/5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
warning: tool_errors_seen
failure_owner: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
memory_sync_events: 4
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
agent_required_commands: 4
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 3
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
latest_top_priority: P3
latest_top_importance_score: 0.05000000074505806
latest_top_weight_share: 0.20000000298023224
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=6050 tool_schema=3186 tools=15 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 60s] cargo test -q -- --test-threads=1
[required validation still running after 90s] cargo test -q -- --test-threads=1
```

## Human Review

- accepted: TODO
- task_success: TODO
- mainline_hit: TODO
- plan_coverage: TODO
- rework_count: TODO
- tool_efficiency: TODO
- diff_discipline: TODO
- closeout_accuracy: TODO
- notes: TODO
