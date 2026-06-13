#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${1:-target/tui-tool-turn-spine-matrix}"

export PRIORITY_AGENT_TUI_DISABLE_DB_FINAL_RECOVERY="${PRIORITY_AGENT_TUI_DISABLE_DB_FINAL_RECOVERY:-1}"
export PRIORITY_AGENT_DEFAULT_PROVIDER="${PRIORITY_AGENT_DEFAULT_PROVIDER:-deepseek}"
export PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS="${PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS:-45}"

cargo build -q

common=(
  python3 scripts/tui_pty_smoke.py
  --size 120x35
  --assert-session-events
  --assert-terminal-contract
  --assert-persistence
)

run_case() {
  local case_name="$1"
  shift
  local case_dir="$OUT_DIR/$case_name"
  mkdir -p "$case_dir"
  "$@" --out-dir "$case_dir" | tee "$case_dir/result.json"
}

run_case tool-pwd \
  "${common[@]}" \
  --prompt tool-pwd \
  --timeout 90 \
  --settle 4 \
  --expect-outcome completed \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 1 \
  --expect-tool-part-count 1

run_case tool-fail \
  "${common[@]}" \
  --prompt tool-fail \
  --timeout 90 \
  --settle 4 \
  --expect-outcome completed \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 1 \
  --expect-tool-part-count 1

run_case tool-long \
  "${common[@]}" \
  --prompt tool-long \
  --timeout 120 \
  --settle 4 \
  --expect-outcome completed \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 1 \
  --expect-tool-part-count 1

run_case tool-sleep-interrupt \
  "${common[@]}" \
  --prompt tool-sleep \
  --timeout 40 \
  --settle 3 \
  --interrupt-after 5 \
  --interrupt-key esc \
  --expect-outcome interrupted \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 0 \
  --expect-tool-part-count 1

python3 scripts/tui_readiness_report.py \
  --matrix "real=$OUT_DIR" \
  --out-dir "$OUT_DIR/_readiness" >/dev/null
