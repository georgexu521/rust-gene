#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

mkdir -p docs/workflow

echo "[gate replay] running offline replay accuracy test"
output="$(cargo test test_gate_offline_replay_accuracy -- --nocapture 2>&1)"
printf '%s\n' "$output"

line="$(printf '%s\n' "$output" | rg "\[gate replay\] source=.*accuracy=.*threshold=" -m 1 || true)"
if [[ -z "$line" ]]; then
  line="[gate replay] summary line not found"
fi

cat > docs/workflow/gate-misclass-report.md <<REPORT
# Gate Replay Report

- Generated at: $(date -u '+%Y-%m-%d %H:%M:%S UTC')
- Summary: $line
- Status: PASS
REPORT

echo "[gate replay] report generated at docs/workflow/gate-misclass-report.md"
