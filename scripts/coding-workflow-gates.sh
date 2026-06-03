#!/usr/bin/env bash
# Layered coding workflow gates.
#
# Use this instead of manually remembering which targeted tests prove the
# coding-agent edit/validation/repair/closeout loop is still healthy.

set -euo pipefail

MODE="${1:-quick}"
LIVE_CASE="${LIVE_CASE:-code-change-verification-repair-loop}"
LIVE_TIMEOUT="${LIVE_TIMEOUT:-1800}"
LIVE_IDLE_TIMEOUT="${LIVE_IDLE_TIMEOUT:-300}"

usage() {
  cat <<'EOF'
Usage:
  scripts/coding-workflow-gates.sh [quick|standard|full|live-smoke|help]

Modes:
  quick       Focused deterministic tests for coding workflow contracts.
  standard    quick + cargo check -q.
  full        scripts/validate_docs.sh (docs + build + full tests).
  live-smoke  One opt-in live eval agent run. Defaults to code-change-verification-repair-loop.

Environment:
  LIVE_CASE          Live eval case for live-smoke (default: code-change-verification-repair-loop)
  LIVE_TIMEOUT       Agent timeout seconds for live-smoke (default: 1800)
  LIVE_IDLE_TIMEOUT  Idle timeout seconds for live-smoke (default: 300)

Notes:
  - quick/standard/full never run live LLM/API evals.
  - live-smoke intentionally runs the real agent path and may take minutes.
EOF
}

run_step() {
  local label="$1"
  shift
  echo
  echo "=== $label ==="
  "$@"
}

quick_gate() {
  run_step "file size no-regression gate" \
    scripts/file-size-report.sh --threshold 3000 --fail-over 3000
  run_step "file size watchlist" \
    scripts/file-size-report.sh --threshold 1500 --top 20
  run_step "closeout evidence contract" \
    cargo test -q closeout -- --test-threads=1
  run_step "tool progress labels" \
    cargo test -q tool_execution_start_progress -- --test-threads=1
  run_step "bash command classification" \
    cargo test -q command_classifier -- --test-threads=1
  run_step "git tool summary/recovery semantics" \
    cargo test -q git_tool -- --test-threads=1
  run_step "eval report/trend helpers" \
    cargo test -q eval_report -- --test-threads=1
  run_step "live eval summary smoke" \
    bash scripts/live-eval-summary-smoke.sh
  run_step "deterministic coding replay matrix" \
    cargo test -q bundled_coding_replay_matrix_passes -- --test-threads=1
  run_step "tool/file reliability gauntlet" \
    cargo test -q bundled_tool_file_reliability_gauntlet_passes -- --test-threads=1
}

standard_gate() {
  quick_gate
  run_step "cargo check" cargo check -q
}

full_gate() {
  run_step "docs/build/full local validation" bash scripts/validate_docs.sh
}

live_smoke_gate() {
  echo "Live smoke uses the real agent path and may take minutes."
  echo "Case: $LIVE_CASE"
  run_step "live coding workflow smoke" \
    bash scripts/run_live_eval.sh \
      --case "$LIVE_CASE" \
      --mode agent-run \
      --run-tests \
      --timeout "$LIVE_TIMEOUT" \
      --idle-timeout "$LIVE_IDLE_TIMEOUT"
}

case "$MODE" in
  quick)
    quick_gate
    ;;
  standard)
    standard_gate
    ;;
  full)
    full_gate
    ;;
  live-smoke)
    live_smoke_gate
    ;;
  help|-h|--help)
    usage
    ;;
  *)
    echo "Unknown mode: $MODE" >&2
    usage >&2
    exit 2
    ;;
esac
