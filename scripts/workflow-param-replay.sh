#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[param replay] running parameter planner replay tests"
cargo test -q test_tool_specific_params_file_write_glob_project_list -- --nocapture
cargo test -q test_param_planner_replay_samples -- --nocapture

SAMPLE_COUNT="$(rg -n '\"tool\"' docs/workflow/param-replay-samples.json | wc -l | tr -d ' ')"
NOW_UTC="$(date -u '+%Y-%m-%d %H:%M:%S UTC')"
mkdir -p docs/workflow
cat > docs/workflow/param-replay-report.md <<EOF
# Param Planner Replay Report

- Generated at: ${NOW_UTC}
- Sample count: ${SAMPLE_COUNT}
- Status: PASS
- Test cases:
  - test_tool_specific_params_file_write_glob_project_list
  - test_param_planner_replay_samples
EOF

echo "[param replay] PASS"
