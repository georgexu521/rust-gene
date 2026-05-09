# Live Eval Report: live-eval-dashboard-summary

- Run id: `dashboard-deterministic-fix-20260509-185127`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/dashboard-deterministic-fix-20260509-185127/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/dashboard-deterministic-fix-20260509-185127/live-eval-dashboard-summary/env`
- Test status: `ok`
- Generated: `2026-05-09 18:56:59 +0800`

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
Summary written to docs/benchmarks/live-live-summary-smoke/summary.md
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1148 tests
....................................................................................... 87/1148
....................................................................................... 174/1148
....................................................................................... 261/1148
....................................................................................... 348/1148
....................................................................................... 435/1148
....................................................................................... 522/1148
....................................................................................... 609/1148
....................................................................................... 696/1148
....................................................................................... 783/1148
....................................................................................... 870/1148
....................................................................................... 957/1148
....................................................................................... 1044/1148
....................................................................................... 1131/1148
.................
test result: ok. 1148 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.36s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-dashboard-deterministic-fix-20260509-185127/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-dashboard-deterministic-fix-20260509-185127/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 4
tool_execution_progress: 1
tool_execution_start: 4
trace_summary: 1
```

Quality signals:

```text
output_chars: 521
diff_chars: 3796
tool_executions: 4
first_write_tool_index: 4
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 47
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: None
closeout_status: passed
runtime_diet: prompt=4104 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=passed
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
trace_event_types: tool.start,tool.done,workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
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
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 3/7
memory_sync_events: 3
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 3
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: passed
runtime_diet: prompt=4104 tool_schema=2641 tools=12 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-09T10:53:00.093299Z  WARN priority_agent::engine::conversation_loop: Workflow judgment analysis failed: key must be a string at line 1 column 2
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
