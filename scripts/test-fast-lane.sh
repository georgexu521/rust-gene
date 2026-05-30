#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run_gate() {
  local name="$1"
  shift
  echo "[fast-lane] $name"
  "$@"
  echo "[fast-lane] $name: PASS"
}

run_gate "fmt" cargo fmt --check
run_gate "check" cargo check -q
run_gate "clippy" cargo clippy --all-features -- -D warnings
run_gate "workflow-tests" cargo test -q workflow
run_gate "streaming-integration" cargo test -q --test streaming_query
run_gate "api-feature-check" cargo check --features experimental-api-server -q

echo "[fast-lane] all gates passed"
