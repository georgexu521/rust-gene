# Live Eval Report: live-eval-dashboard-summary

- Run id: `checkpoint-isolated-target-20260509-103857`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/checkpoint-isolated-target-20260509-103857/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-isolated-target-20260509-103857/live-eval-dashboard-summary/env`
- Test status: `failed`
- Generated: `2026-05-09 10:49:24 +0800`

## Git Status

```text
 M scripts/run_live_eval.sh
```

## Diff Stat

```text
 scripts/run_live_eval.sh | 78 ++++++++++++++++++++++++++++++++++++++++++++----
 1 file changed, 73 insertions(+), 5 deletions(-)
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
docs/benchmarks/live-live-summary-smoke/summary.md
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1118 tests
...............................................................error: test failed, to rerun pass `--bin priority-agent`

Caused by:
  process didn't exit successfully: `/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-isolated-target-20260509-103857/live-eval-dashboard-summary/env/cargo-target/debug/deps/priority_agent-4e8ff203fb792cce --test-threads=1 --quiet` (signal: 9, SIGKILL: kill)
[exit status: 101]

```

## Agent Run

- Exit status: ``
- Events: `docs/benchmarks/live-checkpoint-isolated-target-20260509-103857/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
eval_started: 1
start: 1
tool_execution_complete: 12
tool_execution_progress: 2
tool_execution_start: 12
```

Quality signals:

```text
output_chars: 0
diff_chars: 3040
tool_executions: 12
first_write_tool_index: 12
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: missing
trace_events: 0
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: missing
adaptive_triggers: none
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: empty_agent_output
warning: tool_run_without_closeout
warning: missing_trace_summary
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: false
active_specialty_signals: 1/7
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: none
required_commands: 4
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 2
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 0
adaptive_triggers: none
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: missing
runtime_diet: missing
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-09T02:39:58.403602Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: response was not valid patch JSON; response was not valid patch JSON
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
