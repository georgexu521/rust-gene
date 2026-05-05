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
first_write_tool_index: 2
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
first_write_tool_index: none
```
EOF
cat >"$RUN_DIR/task-seeded-fail/agent-quality-status.txt" <<'EOF'
status=failed
failure_owner=llm_reasoning
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
grep -q '`expected_code_diff_missing`: `1`' "$summary_path"
grep -q '`warning:no_code_diff`: `1`' "$summary_path"
grep -q '| real_code_change_passed | 1 |' "$summary_path"
grep -q '| plan_only_passed | 1 |' "$summary_path"
grep -q '| seeded_no_diff_failed | 1 |' "$summary_path"

echo "live eval summary smoke passed: $summary_path"
