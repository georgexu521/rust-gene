# Live Eval Report: core-multi-file-edit

- Run id: `core-quality-real-rerun-20260517-091952`
- Sample: `evalsets/live_tasks/core-multi-file-edit.yaml`
- Worktree: `target/live-evals/core-quality-real-rerun-20260517-091952/core-multi-file-edit/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/core-quality-real-rerun-20260517-091952/core-multi-file-edit/env`
- Test status: `ok`
- Generated: `2026-05-17 09:21:28 +0800`

## Git Status

```text
 M fixtures/core_quality/multifile/cart.py
 M fixtures/core_quality/multifile/pricing.md
?? fixtures/core_quality/multifile/__pycache__/
```

## Diff Stat

```text
 fixtures/core_quality/multifile/cart.py    | 2 +-
 fixtures/core_quality/multifile/pricing.md | 2 +-
 2 files changed, 2 insertions(+), 2 deletions(-)
```

## Required Commands

```text
$ python3 fixtures/core_quality/multifile/test_cart.py
.
----------------------------------------------------------------------
Ran 1 test in 0.000s

OK
[exit status: 0]

$ rg 'TAX_RATE = 0.0825' fixtures/core_quality/multifile/cart.py
TAX_RATE = 0.0825
[exit status: 0]

$ rg 'tax rate: 0.0825' fixtures/core_quality/multifile/pricing.md
tax rate: 0.0825
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-core-quality-real-rerun-20260517-091952/core-multi-file-edit/agent-output.md`
- Events: `docs/benchmarks/live-core-quality-real-rerun-20260517-091952/core-multi-file-edit/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 3
tool_execution_progress: 1
tool_execution_start: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 938
diff_chars: 592
diff_files_changed: 2
tool_executions: 3
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 38
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=2335 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:5/5
adaptive_triggers: required_validation,first_code_change
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
memory_sync_events: 2
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 3
agent_required_commands: 3
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
adaptive_trigger_events: 2
adaptive_triggers: required_validation,first_code_change
latest_top_priority: P3
latest_top_importance_score: 0.3099999725818634
latest_top_weight_share: 0.3084576725959778
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=2335 tool_schema=3186 tools=15 workflow=guarded
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
