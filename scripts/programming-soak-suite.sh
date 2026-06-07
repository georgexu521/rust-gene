#!/usr/bin/env bash
# Programming parity soak suite — real-programming release gate.
#
# Runs real programming tasks through CLI/TUI/API and collects
# pass/fail, failure_owner, cost, cache, and provider health metadata.
#
# Slice F of the opencode programming parity plan.
#
# Usage: bash scripts/programming-soak-suite.sh [all|task_name...]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BASELINE_DIR="$PROJECT_DIR/target/soak-bundles"
RUN_ID="soak-$(date +%Y%m%d-%H%M%S)"
BUNDLE_DIR="$BASELINE_DIR/$RUN_ID"

TASKS=(
  "single-file-bug-fix"
  "multi-file-refactor"
  "frontend-component-change"
  "failing-test-repair"
  "permission-deny-safe-retry"
  "long-shell-output-paging"
  "revert-last-assistant-turn"
  "provider-slow-tail-classification"
)

echo "Programming Parity Soak Suite"
echo "Run ID: $RUN_ID"
echo "Tasks: ${#TASKS[@]}"
echo "Output: $BUNDLE_DIR"
echo ""

mkdir -p "$BUNDLE_DIR"

# Artifact bundle per task
run_task() {
  local task="$1"
  local task_dir="$BUNDLE_DIR/$task"
  mkdir -p "$task_dir"

  echo "  [$task] starting..."

  # Record prompt
  cat > "$task_dir/prompt.txt" << 'ENDOFPROMPT'
(real programming task prompt — replace with actual task description)
ENDOFPROMPT

  # Record the run metadata
  cat > "$task_dir/run.json" << ENDOFJSON
{
  "run_id": "$RUN_ID",
  "task": "$task",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "status": "not_run",
  "failure_owner": null,
  "model": "",
  "provider": "",
  "turns": 0,
  "tool_calls": 0,
  "file_changes": 0,
  "prompt_tokens": 0,
  "completion_tokens": 0,
  "cost_usd": 0.0,
  "cache_hit_ratio": 0.0,
  "closeout_status": null,
  "provider_health": null,
  "notes": ""
}
ENDOFJSON

  echo "  [$task] artifact bundle at $task_dir"
}

{
  echo "# Programming Soak Baseline $RUN_ID"
  echo ""
  echo "| # | Task | Status | Failure Owner | Turns | Files | Cost | Notes |"
  echo "|---|------|--------|---------------|-------|-------|------|-------|"
} > "$BUNDLE_DIR/baseline.md"

for i in "${!TASKS[@]}"; do
  task="${TASKS[$i]}"
  run_task "$task"
  echo "  | $((i+1)) | $task | not_run | | | | | |" >> "$BUNDLE_DIR/baseline.md"
done

{
  echo ""
  echo "## Summary"
  echo ""
  echo "- run_id: $RUN_ID"
  echo "- tasks: ${#TASKS[@]}"
  echo "- artifact_bundles: $BUNDLE_DIR"
  echo ""
  echo "## Soak Scenarios"
  echo ""
  echo "1. **single-file-bug-fix**: Edit one file, run targeted test, verify closeout"
  echo "2. **multi-file-refactor**: Edit 2+ files with interdependent changes"
  echo "3. **frontend-component-change**: Edit TSX/TS file, run lint check"
  echo "4. **failing-test-repair**: Diagnose failing test, repair, verify pass"
  echo "5. **permission-deny-safe-retry**: Blocked bash mutation → use file_edit → pass"
  echo "6. **long-shell-output-paging**: Generate >32KB output, page via tool-output API"
  echo "7. **revert-last-assistant-turn**: Create change, revert, verify restore, unrevert"
  echo "8. **provider-slow-tail-classification**: Run with slow provider, check timeout diagnosis"
  echo ""
  echo "## Release Blockers"
  echo ""
  echo "- Data loss (session parts/events not recoverable after crash)"
  echo "- False verified closeout"
  echo "- Provider timeout with no visible diagnosis"
  echo "- Permission hard gate bypass"
  echo "- API schema drift without version bump"
} >> "$BUNDLE_DIR/baseline.md"

echo ""
echo "Baseline saved to $BUNDLE_DIR/baseline.md"
echo "Artifact bundles: $BUNDLE_DIR"
echo ""
echo "To run a real task, populate prompt.txt and execute the agent, then"
echo "update run.json with the actual outcome."
