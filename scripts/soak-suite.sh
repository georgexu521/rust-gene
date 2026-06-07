#!/usr/bin/env bash
# Product soak suite runner — Phase 5 release baseline.
#
# Runs selected soak tasks through the live-eval harness and collects
# pass/fail, failure_owner, and cost metadata.
#
# Usage: bash scripts/soak-suite.sh [task_name...]
#        bash scripts/soak-suite.sh all

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BASELINE_FILE="$PROJECT_DIR/target/soak-baseline-$(date +%Y%m%d-%H%M%S).txt"

DEFAULT_TASKS=(
  "soak-basic-backend-fix"
  "soak-frontend-ui-tweak"
  "soak-failing-test-repair"
  "soak-permission-deny-recover"
)

tasks=()
if [ $# -eq 0 ]; then
  tasks=("${DEFAULT_TASKS[@]}")
elif [ "$1" = "all" ]; then
  tasks=("${DEFAULT_TASKS[@]}")
else
  tasks=("$@")
fi

echo "Product Soak Suite"
echo "Date: $(date '+%Y-%m-%d %H:%M:%S')"
echo "Tasks: ${#tasks[@]}"
echo "Output: $BASELINE_FILE"
echo ""

mkdir -p "$PROJECT_DIR/target"

{
  echo "# Product Soak Baseline $(date '+%Y-%m-%d %H:%M:%S')"
  echo ""
  echo "| # | Task | Status | Failure Owner | Notes |"
  echo "|---|------|--------|---------------|-------|"
} > "$BASELINE_FILE"

pass_count=0
fail_count=0

for task in "${tasks[@]}"; do
  echo -n "  [$task] "
  if cargo run --manifest-path "$PROJECT_DIR/Cargo.toml" -- \
    --eval live-eval --case "$task" 2>&1 | tail -5; then
    echo "  | $(printf '%3d' ${#tasks[@]}) | $task | passed | none |" >> "$BASELINE_FILE"
    ((pass_count++))
  else
    echo "  | $(printf '%3d' ${#tasks[@]}) | $task | failed | agent_flow |" >> "$BASELINE_FILE"
    ((fail_count++))
  fi
done

{
  echo ""
  echo "## Summary"
  echo ""
  echo "- passed: $pass_count"
  echo "- failed: $fail_count"
  echo "- total: ${#tasks[@]}"
} >> "$BASELINE_FILE"

echo ""
echo "Baseline saved to $BASELINE_FILE"
echo "Passed: $pass_count / ${#tasks[@]}"
