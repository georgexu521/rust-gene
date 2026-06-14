#!/usr/bin/env bash
set -euo pipefail

FIXTURE_OUT_DIR="${FIXTURE_OUT_DIR:-target/tui-tool-turn-spine-fixture-matrix}"
REAL_OUT_DIR="${REAL_OUT_DIR:-target/tui-tool-turn-spine-matrix}"
NIGHTLY_OUT_DIR="${NIGHTLY_OUT_DIR:-target/tui-tool-turn-spine-nightly}"
REPORT_OUT_DIR="${REPORT_OUT_DIR:-target/tui-readiness-report}"

bash scripts/tui_tool_turn_spine_fixture_matrix.sh "$FIXTURE_OUT_DIR"

report_args=(--matrix "fixture=$FIXTURE_OUT_DIR")

if [[ -d "$NIGHTLY_OUT_DIR" ]]; then
  latest_nightly_link="$NIGHTLY_OUT_DIR/latest"
  if [[ -L "$latest_nightly_link" ]]; then
    latest_nightly_dir="$(readlink -f "$latest_nightly_link")"
    round_index=1
    while [[ -d "$latest_nightly_dir/round-$round_index" ]]; do
      report_args+=(--matrix "nightly-round-$round_index=$latest_nightly_dir/round-$round_index")
      round_index=$((round_index + 1))
    done
  fi
fi

if [[ "${RUN_REAL_PROVIDER:-0}" == "1" || "${RUN_REAL_PROVIDER:-0}" == "true" ]]; then
  bash scripts/tui_tool_turn_spine_matrix.sh "$REAL_OUT_DIR"
  report_args+=(--matrix "real=$REAL_OUT_DIR")
elif [[ -n "$(find "$REAL_OUT_DIR" -maxdepth 2 -name result.json -print -quit 2>/dev/null)" ]]; then
  report_args+=(--matrix "real=$REAL_OUT_DIR")
fi

python3 scripts/tui_readiness_report.py \
  "${report_args[@]}" \
  --out-dir "$REPORT_OUT_DIR"

echo "readiness: $REPORT_OUT_DIR/readiness.md"
