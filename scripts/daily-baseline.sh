#!/usr/bin/env bash
# Daily baseline: deterministic checks that do not require a provider.
#
# Run: bash scripts/daily-baseline.sh
#
# This covers: compilation, formatting, runtime-spine tests, cache-stability,
# controller contract, route-scoped tools, permission logic, checkpoint safety,
# and script syntax. All gates are deterministic (no live eval spend).
#
# For live-eval runs (require provider), see scripts/run_live_eval.sh.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PASS=0
FAIL=0

gate() {
  local name="$1"
  shift
  echo "[gate] $name"
  if "$@"; then
    echo "[gate] $name: PASS"
    PASS=$((PASS + 1))
  else
    echo "[gate] $name: FAIL"
    FAIL=$((FAIL + 1))
  fi
  echo
}

# ---- Compilation ----
gate "cargo-check"       cargo check -q
gate "cargo-check-api"   cargo check --features experimental-api-server -q

# ---- Formatting ----
gate "cargo-fmt"         cargo fmt --check

# ---- Runtime-spine tests ----
gate "instructions"      cargo test --lib -q instructions -- --test-threads=1
gate "cache-stability"   cargo test --lib -q cache_stability -- --test-threads=1
gate "controller"        cargo test --lib -q runtime_controller -- --test-threads=1
gate "route-tools"       cargo test --lib -q route_scoped_tools -- --test-threads=1
gate "closeout"          cargo test --lib -q closeout -- --test-threads=1
gate "permissions"       cargo test --lib -q permissions -- --test-threads=1
gate "checkpoint"        cargo test --lib -q checkpoint -- --test-threads=1
gate "file-tool"         cargo test --lib -q file_tool -- --test-threads=1
gate "desktop-runtime"   cargo test --lib -q desktop_runtime -- --test-threads=1
gate "usage-ledger"      cargo test --lib -q usage_ledger -- --test-threads=1
gate "cost-tracker"      cargo test --lib -q cost_tracker -- --test-threads=1
gate "edit-match"        cargo test --lib -q edit_match -- --test-threads=1

# ---- Broad test ----
gate "full-test"         cargo test --lib -q -- --test-threads=1

# ---- Code size stewardship (Phase 8) ----
gate "file-size-report"  bash scripts/file-size-report.sh --threshold 1200 --top 25

# ---- File size hard limit (Phase 0) ----
gate "file-size-hard-limit" bash -c '
  OVER_LIMIT=$(find src -name "*.rs" -not -name "tests.rs" -exec wc -l {} + | awk "{if(\$1>1500 && \$2 != \"total\") print \$1, \$2}")
  if [ -n "$OVER_LIMIT" ]; then
    echo "Files exceeding 1500 lines:"
    echo "$OVER_LIMIT"
    exit 1
  fi
  echo "All production files are within 1500-line limit."
'

# ---- Documentation health (Phase 4) ----
gate "doc-health" bash scripts/doc_health_check.sh

# ---- Script syntax ----
gate "eval-script-syntax"  bash -n scripts/run_live_eval.sh
gate "parser-syntax"       python3 -m py_compile scripts/live_eval_report_parser.py
gate "daily-baseline-syntax" bash -n scripts/daily-baseline.sh

# ---- Summary ----
TOTAL=$((PASS + FAIL))
echo "========================================"
echo "Daily baseline: $PASS/$TOTAL passed"
if [ "$FAIL" -gt 0 ]; then
  echo "$FAIL gate(s) failed. See above for details."
  exit 1
else
  echo "All gates passed. Ready for live eval or push."
fi
