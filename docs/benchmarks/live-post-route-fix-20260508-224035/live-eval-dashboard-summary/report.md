# Live Eval Report: live-eval-dashboard-summary

- Run id: `post-route-fix-20260508-224035`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/post-route-fix-20260508-224035/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/post-route-fix-20260508-224035/live-eval-dashboard-summary/env`
- Test status: `failed`
- Generated: `2026-05-08 22:45:00 +0800`

## Git Status

```text
```

## Diff Stat

```text
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
summary mode is not implemented yet
[exit status: 2]

$ cargo test -q -- --test-threads=1

running 1118 tests
....................................................................................... 87/1118
....................................................................................... 174/1118
....................................................................................... 261/1118
....................................................................................... 348/1118
....................................................................................... 435/1118
....................................................................................... 522/1118
....................................................................................... 609/1118
....................................................................................... 696/1118
....................................................................................... 783/1118
....................................................................................... 870/1118
....................................................................................... 957/1118
....................................................................................... 1044/1118
..........................................................................
test result: ok. 1118 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.00s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-post-route-fix-20260508-224035/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-post-route-fix-20260508-224035/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 6
tool_execution_progress: 1
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 1159
diff_chars: 0
tool_executions: 6
first_write_tool_index: none
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 57
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
runtime_diet: prompt=5363 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=not_verified
adaptive_triggers: required_validation,repeated_no_code_progress
trace_event_types: workflow.fallback,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,workflow.fallback,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: true
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: no_code_diff
warning: action_checkpoint_no_patch
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 4/7
memory_sync_events: 3
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: required_validation,repeated_no_code_progress
latest_top_priority: P0
latest_top_importance_score: 0.8250000476837158
latest_top_weight_share: 0.27227723598480225
acceptance_accepted: missing
closeout_status: not_verified
runtime_diet: prompt=5363 tool_schema=2641 tools=12 workflow=guarded
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
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
