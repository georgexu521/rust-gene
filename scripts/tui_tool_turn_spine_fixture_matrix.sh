#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${1:-target/tui-tool-turn-spine-fixture-matrix}"

cargo build -q

common=(
  python3 scripts/tui_pty_smoke.py
  --size 120x35
  --assert-session-events
  --assert-terminal-contract
  --assert-persistence
  --assert-projection
  --expect-provider-label "Custom / test-fixture-model"
)

run_fixture() {
  local scenario="$1"
  shift
  env \
    PRIORITY_AGENT_TUI_DISABLE_DB_FINAL_RECOVERY=1 \
    PRIORITY_AGENT_TEST_PROVIDER_SCENARIO="$scenario" \
    PRIORITY_AGENT_DEFAULT_PROVIDER=test-fixture \
    "$@"
}

run_fixture_case() {
  local scenario="$1"
  local case_name="$2"
  shift 2
  local case_dir="$OUT_DIR/$case_name"
  mkdir -p "$case_dir"
  run_fixture "$scenario" "$@" --out-dir "$case_dir" | tee "$case_dir/result.json"
}

run_fixture_case tool-pwd tool-pwd \
  "${common[@]}" \
  --prompt tool-pwd \
  --timeout 60 \
  --settle 3 \
  --expect-outcome completed \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 1 \
  --expect-tool-part-count 1

run_fixture_case tool-fail tool-fail \
  "${common[@]}" \
  --prompt tool-fail \
  --timeout 60 \
  --settle 3 \
  --expect-outcome completed \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 1 \
  --expect-tool-part-count 1

run_fixture_case tool-long tool-long \
  "${common[@]}" \
  --prompt tool-long \
  --timeout 60 \
  --settle 3 \
  --expect-outcome completed \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 1 \
  --expect-tool-part-count 1

run_fixture_case tool-invalid-args tool-invalid-args \
  "${common[@]}" \
  --prompt tool-invalid-args \
  --timeout 60 \
  --settle 3 \
  --expect-outcome completed \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 1 \
  --expect-tool-part-count 1

run_fixture_case tool-multi tool-multi \
  "${common[@]}" \
  --prompt tool-multi \
  --timeout 60 \
  --settle 3 \
  --expect-outcome completed \
  --expect-tool-start-count 2 \
  --expect-tool-result-count 2 \
  --expect-tool-part-count 2

run_fixture_case tool-partial tool-partial \
  "${common[@]}" \
  --prompt tool-partial \
  --timeout 60 \
  --settle 3 \
  --expect-outcome completed \
  --expect-tool-start-count 2 \
  --expect-tool-result-count 2 \
  --expect-tool-part-count 2

run_fixture_case tool-malformed tool-malformed \
  "${common[@]}" \
  --prompt tool-malformed \
  --timeout 60 \
  --settle 3 \
  --expect-outcome completed \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 1 \
  --expect-tool-part-count 1 \
  --assert-provider-repair-diagnostic

run_fixture_case tool-sleep tool-sleep-interrupted \
  "${common[@]}" \
  --prompt tool-sleep \
  --timeout 35 \
  --settle 2 \
  --interrupt-after 2 \
  --interrupt-key esc \
  --expect-outcome interrupted \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 0 \
  --expect-tool-part-count 1

case_dir="$OUT_DIR/provider-timeout-after-result"
mkdir -p "$case_dir"
env \
  PRIORITY_AGENT_TUI_DISABLE_DB_FINAL_RECOVERY=1 \
  PRIORITY_AGENT_TEST_PROVIDER_SCENARIO=tool-timeout-after-result \
  PRIORITY_AGENT_TEST_PROVIDER_SLEEP_SECS=35 \
  PRIORITY_AGENT_DEFAULT_PROVIDER=test-fixture \
  PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS=30 \
  "${common[@]}" \
  --prompt tool-pwd \
  --timeout 55 \
  --settle 3 \
  --expect-outcome provider-timeout \
  --expect-tool-start-count 1 \
  --expect-tool-result-count 1 \
  --expect-tool-part-count 1 \
  --out-dir "$case_dir" | tee "$case_dir/result.json"

python3 scripts/tui_readiness_report.py \
  --matrix "fixture=$OUT_DIR" \
  --out-dir "$OUT_DIR/_readiness" >/dev/null
