#!/usr/bin/env bash
# Active memory graduation baseline test.
#
# Measures active memory behavior in interactive sessions.
# Run this to collect data for the graduation decision.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "=== Active Memory Graduation Baseline ==="
echo

run_cargo_gate() {
  local label="$1"
  local filter="$2"
  shift 2
  local matched
  matched="$(cargo test -- --list 2>/dev/null | { grep -F "$filter" || true; } | wc -l | tr -d ' ')"
  if [[ "$matched" -eq 0 ]]; then
    echo "  Status: FAIL (${label} matched no tests)"
    return 1
  fi
  echo "  Matched tests: $matched"

  local output
  local status=0
  output="$("$@" 2>&1)" || status=$?

  if [[ "$status" -ne 0 ]]; then
    echo "$output" | tail -40
    echo "  Status: FAIL"
    return "$status"
  fi
  echo "  Status: PASS"
}

# Test 1: Active memory disabled (baseline)
echo "Test 1: Active memory disabled"
echo "  PRIORITY_AGENT_ACTIVE_MEMORY=0"
echo "  Running cargo test -q memory::active::tests::..."
run_cargo_gate "active memory disabled" "memory::active::tests::" env PRIORITY_AGENT_ACTIVE_MEMORY=0 cargo test -q memory::active::tests:: -- --test-threads=1
echo

# Test 2: Active memory enabled
echo "Test 2: Active memory enabled"
echo "  PRIORITY_AGENT_ACTIVE_MEMORY=1"
echo "  Running cargo test -q memory::active::tests::..."
run_cargo_gate "active memory enabled" "memory::active::tests::" env PRIORITY_AGENT_ACTIVE_MEMORY=1 cargo test -q memory::active::tests:: -- --test-threads=1
echo

# Test 3: Memory retrieval context
echo "Test 3: Memory retrieval context"
echo "  Running cargo test -q turn_retrieval_context_controller..."
run_cargo_gate "memory retrieval context" "turn_retrieval_context_controller" cargo test -q turn_retrieval_context_controller -- --test-threads=1
echo

# Test 4: Request preparation
echo "Test 4: Request preparation"
echo "  Running cargo test -q request_preparation_controller..."
run_cargo_gate "request preparation" "request_preparation_controller" cargo test -q request_preparation_controller -- --test-threads=1
echo

# Test 5: Memory doctor
echo "Test 5: Memory doctor"
echo "  Running cargo test -q memory_doctor..."
run_cargo_gate "memory doctor" "memory_doctor" cargo test -q memory_doctor -- --test-threads=1
echo

echo "=== Summary ==="
echo
echo "If all tests pass, active memory is mechanically safe for a comparison run."
echo "This smoke gate alone is not graduation evidence."
echo "Run with PRIORITY_AGENT_ACTIVE_MEMORY=1 to collect an enabled baseline."
echo
echo "Graduation rule (from plan):"
echo "  If it improves at least two memory evals or real resume tasks"
echo "  without causing cache instability or confusing user-facing answers,"
echo "  expose it as /memory control active on."
echo
echo "Current recommendation: Run daily gate or targeted memory evals with"
echo "active memory enabled and compare results with a disabled baseline."
