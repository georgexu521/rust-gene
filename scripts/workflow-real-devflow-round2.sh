#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[round2] running real devflow acceptance replay"
cargo test test_workflow_real_devflow_round2_acceptance -- --nocapture

echo "[round2] report generated at docs/workflow/real-devflow-round2-report.md"
