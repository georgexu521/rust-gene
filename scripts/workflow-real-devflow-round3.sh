#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[round3] running real devflow acceptance from this week's commits"
cargo test test_workflow_real_devflow_round3_acceptance -- --nocapture

echo "[round3] report generated at docs/workflow/real-devflow-round3-report.md"
