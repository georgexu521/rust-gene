# Live Eval Report: memory-save-sensitive-hard-block

- Run id: `batch6-provider-gated-fix-20260511-104114`
- Sample: `evalsets/live_tasks/memory-save-sensitive-hard-block.yaml`
- Worktree: `target/live-evals/batch6-provider-gated-fix-20260511-104114/memory-save-sensitive-hard-block/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-provider-gated-fix-20260511-104114/memory-save-sensitive-hard-block/env`
- Test status: `skipped`
- Generated: `2026-05-11 10:41:35 +0800`

## Provider Health Preflight

Provider health failed before the agent run, so required commands were not run for this task.

```json
{
  "status": "failed",
  "model": "MiniMax-M2.7",
  "base_url": "https://api.minimaxi.com/v1",
  "timeout_secs": 45,
  "duration_ms": 16734,
  "steps": [
    {
      "name": "plain_chat",
      "status": "ok",
      "duration_ms": 9598,
      "detail": "content_chars=107"
    },
    {
      "name": "tool_call",
      "status": "failed",
      "duration_ms": 7135,
      "error": "provider returned no tool call",
      "error_category": "provider_semantics"
    }
  ]
}
```

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
```

## Agent Run

- Exit status: `126`
- Output: `docs/benchmarks/live-batch6-provider-gated-fix-20260511-104114/memory-save-sensitive-hard-block/agent-output.md`
- Events: `docs/benchmarks/live-batch6-provider-gated-fix-20260511-104114/memory-save-sensitive-hard-block/agent-events.jsonl`

Event counts:

```text
error: 1
eval_started: 1
provider_health_preflight: 1
```

Quality signals:

```text
output_chars: 0
diff_chars: 0
tool_executions: 0
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: missing
trace_events: 0
test_status: skipped
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
eval_intent: audit_or_regression_check
warning: empty_agent_output
warning: no_code_diff
warning: missing_trace_summary
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: environment
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
required_commands: 3
required_command_status: skipped
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
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
provider unavailable: provider health preflight failed before agent-run
Health status: docs/benchmarks/live-batch6-provider-gated-fix-20260511-104114/provider-health-status.txt
Health report: docs/benchmarks/live-batch6-provider-gated-fix-20260511-104114/provider-health.json

2026-05-11T02:41:35.302435Z ERROR priority_agent: Provider health failed: provider health failed: tool_call: provider returned no tool call
Provider health failed: provider health failed: tool_call: provider returned no tool call
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
