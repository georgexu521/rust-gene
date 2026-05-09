# Live Eval Report: live-eval-dashboard-summary

- Run id: `checkpoint-default-patch-20260509-093727`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/checkpoint-default-patch-20260509-093727/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-default-patch-20260509-093727/live-eval-dashboard-summary/env`
- Test status: `ok`
- Generated: `2026-05-09 10:37:21 +0800`

## Git Status

```text
 M scripts/run_live_eval.sh
?? docs/benchmarks/live-live-summary-smoke/
```

## Diff Stat

```text
 scripts/run_live_eval.sh | 145 +++++++++++++++++++++++++++++++++++++++++++++--
 1 file changed, 140 insertions(+), 5 deletions(-)
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
Summary written to: docs/benchmarks/live-live-summary-smoke/summary.md
[exit status: 0]

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
test result: ok. 1118 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.65s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-checkpoint-default-patch-20260509-093727/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-checkpoint-default-patch-20260509-093727/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 8
tool_execution_progress: 1
tool_execution_start: 8
trace_summary: 1
```

Quality signals:

```text
output_chars: 1148
diff_chars: 5190
tool_executions: 8
first_write_tool_index: 7
tool_errors: 0
tool_failures: 3
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 83
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=9574 tool_schema=2641 tools=12 workflow=strict closeout=full validation=failed
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
trace_event_types: workflow.fallback,api.start,workflow.fallback,api.done,tool.start,tool.done,workflow.fallback,workflow.fallback,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: true
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: action_checkpoint_invalid_tools
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
warning: recovered_action_checkpoint_invalid_tools
warning: recovered_closeout_not_successful
warning: recovered_acceptance_review_rejected
warning: recovered_stage_validation_failed
warning: recovered_verification_failed
failure_owner: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
memory_sync_events: 3
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P0
latest_top_importance_score: 0.9350000619888306
latest_top_weight_share: 0.31375840306282043
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=9574 tool_schema=2641 tools=12 workflow=strict
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 60s] cargo test -q -- --test-threads=1
[required validation still running after 90s] cargo test -q -- --test-threads=1
[required validation still running after 120s] cargo test -q -- --test-threads=1
[required validation still running after 150s] cargo test -q -- --test-threads=1
[required validation still running after 180s] cargo test -q -- --test-threads=1
[required validation still running after 210s] cargo test -q -- --test-threads=1
[required validation still running after 240s] cargo test -q -- --test-threads=1
[required validation still running after 270s] cargo test -q -- --test-threads=1
[required validation still running after 300s] cargo test -q -- --test-threads=1
[required validation still running after 330s] cargo test -q -- --test-threads=1
[required validation still running after 360s] cargo test -q -- --test-threads=1
[required validation still running after 390s] cargo test -q -- --test-threads=1
[required validation still running after 420s] cargo test -q -- --test-threads=1
[required validation still running after 450s] cargo test -q -- --test-threads=1
[required validation still running after 480s] cargo test -q -- --test-threads=1
[required validation still running after 510s] cargo test -q -- --test-threads=1
[required validation still running after 540s] cargo test -q -- --test-threads=1
[required validation still running after 570s] cargo test -q -- --test-threads=1
[required validation still running after 600s] cargo test -q -- --test-threads=1
[required validation still running after 630s] cargo test -q -- --test-threads=1
[required validation still running after 660s] cargo test -q -- --test-threads=1
[required validation still running after 690s] cargo test -q -- --test-threads=1
[required validation still running after 720s] cargo test -q -- --test-threads=1
[required validation still running after 750s] cargo test -q -- --test-threads=1
[required validation still running after 780s] cargo test -q -- --test-threads=1
[required validation still running after 810s] cargo test -q -- --test-threads=1
[required validation still running after 840s] cargo test -q -- --test-threads=1
2026-05-09T01:59:25.070722Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-default-patch-20260509-093727/live-eval-dashboard-summary/worktree/scripts/run_live_eval.sh; refusing inexact multi-line replacement
2026-05-09T02:01:07.830427Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-default-patch-20260509-093727/live-eval-dashboard-summary/worktree/scripts/run_live_eval.sh; refusing inexact multi-line replacement
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
