# Live Eval Report: live-eval-dashboard-summary

- Run id: `capability-dashboard-summary-rerun-20260503-235256`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/capability-dashboard-summary-rerun-20260503-235256/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/capability-dashboard-summary-rerun-20260503-235256/live-eval-dashboard-summary/env`
- Test status: `failed`
- Generated: `2026-05-03 23:56:27 +0800`

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
PyYAML is required for live eval parsing: No module named 'yaml'
[exit status: 1]

$ cargo test -q -- --test-threads=1

running 1057 tests
....................................................................................... 87/1057
....................................................................................... 174/1057
....................................................................................... 261/1057
....................................................................................... 348/1057
....................................................................................... 435/1057
....................................................................................... 522/1057
....................................................................................... 609/1057
....................................................................................... 696/1057
....................................................................................... 783/1057
....................................................................................... 870/1057
....................................................................................... 957/1057
....................................................................................... 1044/1057
.............
test result: ok. 1057 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.65s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-capability-dashboard-summary-rerun-20260503-235256/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-capability-dashboard-summary-rerun-20260503-235256/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 13
tool_execution_progress: 2
tool_execution_start: 13
trace_summary: 1
```

Quality signals:

```text
output_chars: 1084
diff_chars: 0
tool_executions: 13
first_write_tool_index: none
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 83
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
trace_event_types: tool.done,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,workflow.fallback,workflow.fallback,workflow.fallback,closeout,assistant
stale_edit_warnings: 0
eval_intent: seeded_code_change
warning: no_code_diff
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: llm_reasoning
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: false
active_specialty_signals: 4/6
memory_sync_events: 7
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 2
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
latest_top_priority: P0
latest_top_importance_score: 0.8500000238418579
latest_top_weight_share: 0.2396053522825241
acceptance_accepted: missing
closeout_status: not_verified
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

## Human Review

- accepted: false
- task_success: false
- mainline_hit: missed
- plan_coverage: partial
- rework_count: 0
- tool_efficiency: poor
- diff_discipline: failed
- closeout_accuracy: accurate
- notes: Rerun used the refreshed seeded fixture from commit `e896cbb`; the
  fixture removed summary support and required `--mode summary` to be restored.
  The agent again inspected the script and metadata but never called
  `file_edit`, so no diff was produced. The harness correctly failed
  `scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke` while
  full Rust tests still passed (`1057 passed; 0 failed`).
