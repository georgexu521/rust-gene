# Live Eval Report: live-eval-dashboard-summary

- Run id: `post-commit-suite-20260508-222022`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/post-commit-suite-20260508-222022/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/post-commit-suite-20260508-222022/live-eval-dashboard-summary/env`
- Test status: `failed`
- Generated: `2026-05-08 22:23:34 +0800`

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

running 1115 tests
....................................................................................... 87/1115
....................................................................................... 174/1115
....................................................................................... 261/1115
....................................................................................... 348/1115
....................................................................................... 435/1115
....................................................................................... 522/1115
....................................................................................... 609/1115
....................................................................................... 696/1115
....................................................................................... 783/1115
....................................................................................... 870/1115
....................................................................................... 957/1115
....................................................................................... 1044/1115
.......................................................................
test result: ok. 1115 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.89s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-post-commit-suite-20260508-222022/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-post-commit-suite-20260508-222022/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
text_chunk: 1
trace_summary: 1
```

Quality signals:

```text
output_chars: 2200
diff_chars: 0
tool_executions: 0
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: true
trace_status: Completed
trace_events: 14
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: missing
adaptive_triggers: required_validation
trace_event_types: resource.policy,retrieval.context,workflow.trigger,workflow.fallback,workflow.judgment,workflow.plan,task.context,reflection.pass,goal,workflow.route,workflow.done,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: no_code_diff
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: llm_reasoning
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 3/7
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P3
latest_top_importance_score: 0.05000000074505806
latest_top_weight_share: 0.20000000298023224
acceptance_accepted: missing
closeout_status: missing
runtime_diet: missing
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
