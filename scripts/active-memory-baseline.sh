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

# Test 1: Active memory disabled (baseline)
echo "Test 1: Active memory disabled"
echo "  PRIORITY_AGENT_ACTIVE_MEMORY=0"
echo "  Running cargo test -q active_memory..."
if PRIORITY_AGENT_ACTIVE_MEMORY=0 cargo test -q active_memory 2>&1 | tail -3; then
  echo "  Status: PASS"
else
  echo "  Status: FAIL"
fi
echo

# Test 2: Active memory enabled
echo "Test 2: Active memory enabled"
echo "  PRIORITY_AGENT_ACTIVE_MEMORY=1"
echo "  Running cargo test -q active_memory..."
if PRIORITY_AGENT_ACTIVE_MEMORY=1 cargo test -q active_memory 2>&1 | tail -3; then
  echo "  Status: PASS"
else
  echo "  Status: FAIL"
fi
echo

# Test 3: Memory retrieval context
echo "Test 3: Memory retrieval context"
echo "  Running cargo test -q turn_retrieval_context_controller..."
if cargo test -q turn_retrieval_context_controller 2>&1 | tail -3; then
  echo "  Status: PASS"
else
  echo "  Status: FAIL"
fi
echo

# Test 4: Request preparation
echo "Test 4: Request preparation"
echo "  Running cargo test -q request_preparation_controller..."
if cargo test -q request_preparation_controller 2>&1 | tail -3; then
  echo "  Status: PASS"
else
  echo "  Status: FAIL"
fi
echo

# Test 5: Memory doctor
echo "Test 5: Memory doctor"
echo "  Running cargo test -q memory_doctor..."
if cargo test -q memory_doctor 2>&1 | tail -3; then
  echo "  Status: PASS"
else
  echo "  Status: FAIL"
fi
echo

echo "=== Summary ==="
echo
echo "If all tests pass, active memory is safe to graduate."
echo "Run with PRIORITY_AGENT_ACTIVE_MEMORY=1 to enable."
echo
echo "Graduation rule (from plan):"
echo "  If it improves at least two memory evals or real resume tasks"
echo "  without causing cache instability or confusing user-facing answers,"
echo "  expose it as /memory control active on."
echo
echo "Current recommendation: Run daily gate with active memory enabled"
echo "and compare results with disabled baseline."
