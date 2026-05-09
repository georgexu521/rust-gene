# Live Eval Report: live-eval-dashboard-summary

- Run id: `focused-lookup-budget-20260509-212938`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/focused-lookup-budget-20260509-212938/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/focused-lookup-budget-20260509-212938/live-eval-dashboard-summary/env`
- Test status: `ok`
- Generated: `2026-05-09 21:35:52 +0800`

## Git Status

```text
 M scripts/run_live_eval.sh
?? docs/benchmarks/live-live-summary-smoke/
```

## Diff Stat

```text
 scripts/run_live_eval.sh | 169 +++++++++++++++++++++++++++++++++++++++++++++--
 1 file changed, 164 insertions(+), 5 deletions(-)
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
Summary written: docs/benchmarks/live-live-summary-smoke/summary.md
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1155 tests
....................................................................................... 87/1155
....................................................................................... 174/1155
....................................................................................... 261/1155
....................................................................................... 348/1155
....................................................................................... 435/1155
....................................................................................... 522/1155
....................................................................................... 609/1155
....................................................................................... 696/1155
....................................................................................... 783/1155
....................................................................................... 870/1155
....................................................................................... 957/1155
....................................................................................... 1044/1155
....................................................................................... 1131/1155
........................
test result: ok. 1155 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.51s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-focused-lookup-budget-20260509-212938/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-focused-lookup-budget-20260509-212938/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 6
tool_execution_progress: 1
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 898
diff_chars: 5780
tool_executions: 6
first_write_tool_index: 6
tool_errors: 0
tool_failures: 3
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 83
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=8264 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=passed
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
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
memory_sync_events: 6
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
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
latest_top_priority: P2
latest_top_importance_score: 0.5950000286102295
latest_top_weight_share: 0.28537172079086304
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=8264 tool_schema=2641 tools=12 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 60s] cargo test -q -- --test-threads=1
[required validation still running after 90s] cargo test -q -- --test-threads=1
[required validation still running after 120s] cargo test -q -- --test-threads=1
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
