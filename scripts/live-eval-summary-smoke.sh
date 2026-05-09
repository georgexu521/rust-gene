#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

RUN_ID="summary-smoke-$$"
RUN_DIR="docs/benchmarks/live-$RUN_ID"

cleanup() {
  rm -rf "$RUN_DIR"
}
trap cleanup EXIT

mkdir -p "$RUN_DIR/task-code-pass" "$RUN_DIR/task-plan-pass" "$RUN_DIR/task-seeded-fail"

cat >"$RUN_DIR/task-code-pass/report.md" <<'EOF'
# Live Eval Report: task-code-pass

Quality signals:

```text
eval_intent: seeded_code_change
required_command_status: ok
closeout_status: passed
adaptive_triggers: required_validation,first_code_change
first_write_tool_index: 2
memory_active: true
memory_recalled_items: 2
memory_conflicts: 1
memory_changed_plan: true
```
EOF
cat >"$RUN_DIR/task-code-pass/agent-quality-status.txt" <<'EOF'
status=ok
failure_owner=none
EOF
echo "ok" >"$RUN_DIR/task-code-pass/test-status.txt"
echo " scripts/run_live_eval.sh | 2 +-" >"$RUN_DIR/task-code-pass/diff-stat.txt"
: >"$RUN_DIR/task-code-pass/agent-events.jsonl"

cat >"$RUN_DIR/task-plan-pass/report.md" <<'EOF'
# Live Eval Report: task-plan-pass

Quality signals:

```text
eval_intent: audit_or_regression_check
required_command_status: skipped
closeout_status: missing
first_write_tool_index: none
skill_active: true
skill_tool_calls: 1
skill_usage_events: 2
skill_promotion_evidence: true
```
EOF
echo "skipped" >"$RUN_DIR/task-plan-pass/test-status.txt"
echo "status=ok" >"$RUN_DIR/task-plan-pass/plan-lint.txt"
touch "$RUN_DIR/task-plan-pass/minimax-plan.md"

cat >"$RUN_DIR/task-seeded-fail/report.md" <<'EOF'
# Live Eval Report: task-seeded-fail

Quality signals:

```text
eval_intent: seeded_code_change
required_command_status: failed
closeout_status: not_verified
adaptive_triggers: repeated_no_code_progress
first_write_tool_index: none
action_checkpoint_no_patch: true
warning: action_checkpoint_no_patch
```
EOF
cat >"$RUN_DIR/task-seeded-fail/agent-quality-status.txt" <<'EOF'
status=failed
failure_owner=agent_flow
failure=expected_code_diff_missing
warning=no_code_diff
EOF
echo "failed" >"$RUN_DIR/task-seeded-fail/test-status.txt"
: >"$RUN_DIR/task-seeded-fail/agent-events.jsonl"

summary_path="$(scripts/run_live_eval.sh --mode summary --run-id "$RUN_ID")"

grep -q 'Pass rate: `2/3` (66.7%)' "$summary_path"
grep -q 'Real code-change passes: `1`' "$summary_path"
grep -q 'Plan-only passes: `1`' "$summary_path"
grep -q 'Seeded no-diff failures: `1`' "$summary_path"
grep -q 'Memory active tasks: `1`' "$summary_path"
grep -q 'Memory changed-plan tasks: `1`' "$summary_path"
grep -q 'Memory recalled items: `2`' "$summary_path"
grep -q 'Skill active tasks: `1`' "$summary_path"
grep -q 'Skill promotion-evidence tasks: `1`' "$summary_path"
grep -q '`expected_code_diff_missing`: `1`' "$summary_path"
grep -q '`warning:no_code_diff`: `1`' "$summary_path"
grep -q '`warning:action_checkpoint_no_patch`: `1`' "$summary_path"
grep -q '| real_code_change_passed | 1 |' "$summary_path"
grep -q '| plan_only_passed | 1 |' "$summary_path"
grep -q '| seeded_no_diff_failed | 1 |' "$summary_path"
grep -q '| memory_active_tasks | 1 | Tasks where retrieval, sync, or memory tools were active. |' "$summary_path"
grep -q '| skill_promotion_evidence_tasks | 1 | Tasks with promotion-related skill evidence. |' "$summary_path"
grep -q '| task-code-pass | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | missing | required_validation,first_code_change | 2 | yes | active=true, recalled=2, conflicts=1, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |' "$summary_path"
grep -q '| task-plan-pass | passed | audit_or_regression_check | missing | skipped | ok | plan-only | unknown | missing | missing | none | none | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=true, tool_calls=1, usage_events=2, promotion=true | none |' "$summary_path"
grep -q '| task-seeded-fail | failed | seeded_code_change | agent_flow | failed | none | agent-run | failed | not_verified | missing | repeated_no_code_progress | none | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,action_checkpoint_no_patch |' "$summary_path"

aggregate_path="$RUN_DIR/aggregate-summary.md"
LIVE_EVAL_AGGREGATE_REFRESH_SUMMARIES=0 \
  LIVE_EVAL_AGGREGATE_RUN_GLOB="live-$RUN_ID" \
  bash scripts/live-eval-aggregate-summary.sh "$aggregate_path" >/dev/null

grep -q 'Runs scanned: `1`' "$aggregate_path"
grep -q 'Task reports scanned: `3`' "$aggregate_path"
grep -q 'Pass rate: `2/3` (66.7%)' "$aggregate_path"
grep -q '| instrumented_task_reports | 3 | 100.0% |' "$aggregate_path"
grep -q '| passed | 2 | 66.7% |' "$aggregate_path"
grep -q '| failed | 1 | 33.3% |' "$aggregate_path"
grep -q '| agent_flow | 1 | 33.3% |' "$aggregate_path"
grep -q '| warning:action_checkpoint_no_patch | 1 |' "$aggregate_path"
grep -q '| warning:no_code_diff | 1 |' "$aggregate_path"
grep -q '| memory_active_tasks | 1 | 33.3% |' "$aggregate_path"
grep -q '| memory_recalled_items | 2 | n/a |' "$aggregate_path"
grep -q '| skill_active_tasks | 1 | 33.3% |' "$aggregate_path"
grep -q '| skill_promotion_evidence_tasks | 1 | 33.3% |' "$aggregate_path"
grep -q '| task-seeded-fail | seeded_code_change | agent_flow | agent_flow | failed | failed | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | repeated_no_code_progress | no_code_diff,action_checkpoint_no_patch |' "$aggregate_path"

echo "live eval summary smoke passed: $summary_path"
