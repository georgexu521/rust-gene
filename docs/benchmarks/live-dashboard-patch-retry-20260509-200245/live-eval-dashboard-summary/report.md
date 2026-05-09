# Live Eval Report: live-eval-dashboard-summary

- Run id: `dashboard-patch-retry-20260509-200245`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/dashboard-patch-retry-20260509-200245/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/dashboard-patch-retry-20260509-200245/live-eval-dashboard-summary/env`
- Test status: `ok`
- Generated: `2026-05-09 20:18:18 +0800`

## Git Status

```text
 M scripts/run_live_eval.sh
?? docs/benchmarks/live-live-summary-smoke/
```

## Diff Stat

```text
 scripts/run_live_eval.sh | 214 +++++++++++++++++++++++++++++++++++++++++++++--
 1 file changed, 209 insertions(+), 5 deletions(-)
```

## Required Commands

```text
$ bash -n scripts/run_live_eval.sh
[exit status: 0]

$ scripts/run_live_eval.sh --list
id                                   type         eval_intent                risk       title
--                                   ----         -----------                ----       -----
backend-todo-api-crud                feature      seeded_code_change         medium     implement a tiny stdlib todo API backend
cli-scrollback-polish                ux           seeded_code_change         medium     interactive CLI should feel smooth and readable
code-change-verification-repair-loop feature      seeded_code_change         high       failed verification should trigger repair before closeout
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
No task reports found in docs/benchmarks/live-live-summary-smoke
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1151 tests
....................................................................................... 87/1151
....................................................................................... 174/1151
....................................................................................... 261/1151
....................................................................................... 348/1151
....................................................................................... 435/1151
....................................................................................... 522/1151
....................................................................................... 609/1151
....................................................................................... 696/1151
....................................................................................... 783/1151
....................................................................................... 870/1151
....................................................................................... 957/1151
....................................................................................... 1044/1151
....................................................................................... 1131/1151
....................
test result: ok. 1151 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.45s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-dashboard-patch-retry-20260509-200245/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-dashboard-patch-retry-20260509-200245/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 10
tool_execution_progress: 6
tool_execution_start: 10
trace_summary: 1
```

Quality signals:

```text
output_chars: 1276
diff_chars: 8390
tool_executions: 10
first_write_tool_index: 5
tool_errors: 1
tool_failures: 4
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 125
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=17416 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
trace_event_types: tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: tool_errors_seen
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
failure_owner: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 7/7
memory_sync_events: 9
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
required_command_status: ok
validation_events: 5
stage_validation_events: 5
tool_progress_events: 6
guided_debugging_events: 4
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P0
latest_top_importance_score: 0.9900000095367432
latest_top_weight_share: 0.24116933345794678
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=17416 tool_schema=2641 tools=12 workflow=strict
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 60s] cargo test -q -- --test-threads=1
[required validation still running after 90s] cargo test -q -- --test-threads=1
[required validation still running after 120s] cargo test -q -- --test-threads=1
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 30s] cargo test -q -- --test-threads=1
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
