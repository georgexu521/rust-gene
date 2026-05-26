#!/usr/bin/env bash
# Fast deterministic gate for runtime-spine work.
#
# This is narrower than scripts/coding-workflow-gates.sh. It exists so P0a
# runtime-spine changes can be checked frequently without running live evals.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run_step() {
  local label="$1"
  shift
  echo
  echo "=== $label ==="
  "$@"
}

run_step "format" cargo fmt --check
run_step "runtime spine matrix" cargo test -q scenario_matrix
run_step "gate outcome derivation" cargo test -q gate_outcome
run_step "intent router" cargo test -q intent_router
run_step "route scoped tools" cargo test -q route_scoped_tools
run_step "task mode score" cargo test -q task_mode_score
run_step "closeout" cargo test -q closeout
run_step "evidence ledger" cargo test -q evidence_ledger
run_step "verification proof" cargo test -q verification_proof
run_step "runtime spine behavior" cargo test -q runtime_spine_behavior
run_step "live eval report parser syntax" \
  python3 -m py_compile scripts/live_eval_report_parser.py
run_step "live eval summary smoke" \
  bash scripts/live-eval-summary-smoke.sh
