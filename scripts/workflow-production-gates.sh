#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run_gate() {
  local name="$1"
  local cmd="$2"
  echo "[gate] $name"
  eval "$cmd"
  echo "[gate] $name: PASS"
}

run_gate "cargo-check" "cargo check -q"
run_gate "cargo-clippy" "cargo clippy -q -- -D warnings"
run_gate "workflow-param-replay" "bash scripts/workflow-param-replay.sh"
run_gate "workflow-weekly-report" "bash scripts/workflow-weekly-report.sh 8"
run_gate "workflow-m1-acceptance" "bash scripts/workflow-m1-acceptance.sh"
run_gate "workflow-gate-replay" "bash scripts/workflow-gate-replay.sh"
run_gate "workflow-real-devflow-round2" "bash scripts/workflow-real-devflow-round2.sh"
run_gate "workflow-real-devflow-round3" "bash scripts/workflow-real-devflow-round3.sh"

echo "All production workflow gates passed."
