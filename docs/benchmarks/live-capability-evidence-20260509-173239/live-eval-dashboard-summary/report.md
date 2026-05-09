# Live Eval Report: live-eval-dashboard-summary

- Run id: `capability-evidence-20260509-173239`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/capability-evidence-20260509-173239/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/capability-evidence-20260509-173239/live-eval-dashboard-summary/env`
- Test status: `ok`
- Generated: `2026-05-09 17:48:55 +0800`

## Git Status

```text
 M scripts/run_live_eval.sh
?? docs/benchmarks/live-live-summary-smoke/
```

## Diff Stat

```text
 scripts/run_live_eval.sh | 81 ++++++++++++++++++++++++++++++++++++++++++------
 1 file changed, 72 insertions(+), 9 deletions(-)
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

running 1147 tests
....................................................................................... 87/1147
....................................................................................... 174/1147
....................................................................................... 261/1147
....................................................................................... 348/1147
....................................................................................... 435/1147
....................................................................................... 522/1147
....................................................................................... 609/1147
....................................................................................... 696/1147
....................................................................................... 783/1147
....................................................................................... 870/1147
....................................................................................... 957/1147
....................................................................................... 1044/1147
....................................................................................... 1131/1147
................
test result: ok. 1147 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.03s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-capability-evidence-20260509-173239/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-capability-evidence-20260509-173239/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 5
tool_execution_progress: 1
tool_execution_start: 5
trace_summary: 1
```

Quality signals:

```text
output_chars: 1227
diff_chars: 3270
tool_executions: 5
first_write_tool_index: 4
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 75
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=7686 tool_schema=2641 tools=12 workflow=strict closeout=full validation=failed
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
latest_top_importance_score: 0.8250000476837158
latest_top_weight_share: 0.24087592959403992
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=7686 tool_schema=2641 tools=12 workflow=strict
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 60s] cargo test -q -- --test-threads=1
[required validation still running after 90s] cargo test -q -- --test-threads=1
[required validation still running after 120s] cargo test -q -- --test-threads=1
2026-05-09T09:47:29.688161Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: run_live_eval.sh patch contains Markdown emphasis markers (`**`), likely copied from highlighted tool output rather than source code; patch synthesis declined without a reason
2026-05-09T09:48:06.164786Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: patch synthesis declined without a reason; patch synthesis declined without a reason
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
