#!/usr/bin/env bash
set -euo pipefail

echo "[lint-check] rustc: $(rustc --version)"
echo "[lint-check] cargo: $(cargo --version)"

TOTAL_WARNINGS=0

run_check() {
  local label="$1"
  shift
  echo
  echo "[lint-check] >>> ${label}"
  local tmp
  tmp="$(mktemp)"

  if "$@" 2>&1 | tee "${tmp}"; then
    local warnings
    warnings="$(grep -c '^warning:' "${tmp}" || true)"
    TOTAL_WARNINGS=$((TOTAL_WARNINGS + warnings))
    echo "[lint-check] warnings(${label}) = ${warnings}"
    rm -f "${tmp}"
  else
    rm -f "${tmp}"
    return 1
  fi
}

run_check "default check" cargo check -q
run_check "feature check: experimental-api-server" cargo check -q --features experimental-api-server
run_check "feature check: experimental-priority" cargo check -q --features experimental-priority
run_check "feature check: experimental-task-analyzer" cargo check -q --features experimental-task-analyzer

echo
echo "[lint-check] total warnings = ${TOTAL_WARNINGS}"
echo "[lint-check] all checks passed"
