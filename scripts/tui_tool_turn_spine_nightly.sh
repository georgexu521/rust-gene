#!/usr/bin/env bash
# Repeat the real-provider TUI tool-turn matrix and emit one readiness report.
#
# This is intentionally a thin wrapper around tui_tool_turn_spine_matrix.sh so
# nightly/soak runs use the same PTY contract output as one-off real-provider
# checks.

set -euo pipefail

OUT_DIR="${1:-target/tui-tool-turn-spine-nightly}"
ROUNDS="${TUI_TOOL_TURN_SPINE_NIGHTLY_ROUNDS:-3}"
RUN_ID="${TUI_TOOL_TURN_SPINE_NIGHTLY_RUN_ID:-nightly-$(date +%Y%m%d-%H%M%S)}"
RUN_DIR="$OUT_DIR/$RUN_ID"
REPORT_OUT_DIR="$RUN_DIR/_readiness"

if ! [[ "$ROUNDS" =~ ^[0-9]+$ ]] || [[ "$ROUNDS" -lt 1 ]]; then
  echo "TUI_TOOL_TURN_SPINE_NIGHTLY_ROUNDS must be a positive integer" >&2
  exit 2
fi

mkdir -p "$RUN_DIR"

python3 - "$RUN_DIR/manifest.json" "$RUN_ID" "$ROUNDS" <<'PY'
import json
import os
import pathlib
import sys
from datetime import datetime, timezone

import subprocess

path = pathlib.Path(sys.argv[1])
run_id = sys.argv[2]
rounds = int(sys.argv[3])
manifest = {
    "schema": "tui_tool_turn_spine_nightly.v1",
    "run_id": run_id,
    "generated_at": datetime.now(timezone.utc).isoformat(),
    "rounds": rounds,
    "provider": os.environ.get("PRIORITY_AGENT_DEFAULT_PROVIDER", "deepseek"),
    "model": os.environ.get("PRIORITY_AGENT_DEFAULT_MODEL"),
    "base_url_family": os.environ.get("PRIORITY_AGENT_LLM_BASE_URL_FAMILY"),
    "request_timeout_secs": os.environ.get(
        "PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "45"
    ),
    "disable_db_final_recovery": os.environ.get(
        "PRIORITY_AGENT_TUI_DISABLE_DB_FINAL_RECOVERY", "1"
    ),
    "git_sha": subprocess.run(
        ["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=False
    ).stdout.strip(),
}
path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
PY

report_args=()
failed_rounds=0
for round in $(seq 1 "$ROUNDS"); do
  round_dir="$RUN_DIR/round-$round"
  mkdir -p "$round_dir"
  echo "==> TUI tool-turn real-provider round $round/$ROUNDS"
  set +e
  bash scripts/tui_tool_turn_spine_matrix.sh "$round_dir"
  status=$?
  set -e
  if [[ "$status" -ne 0 ]]; then
    failed_rounds=$((failed_rounds + 1))
    printf '%s\n' "$status" > "$round_dir/exit-code.txt"
  fi
  report_args+=(--matrix "real-round-$round=$round_dir")
done

set +e
python3 scripts/tui_readiness_report.py "${report_args[@]}" --out-dir "$REPORT_OUT_DIR"
report_status=$?
set -e

python3 - "$RUN_DIR/manifest.json" "$failed_rounds" "$report_status" <<'PY'
import json
import pathlib
import sys
from datetime import datetime, timezone

path = pathlib.Path(sys.argv[1])
manifest = json.loads(path.read_text(encoding="utf-8"))
manifest["failed_rounds"] = int(sys.argv[2])
manifest["readiness_exit_code"] = int(sys.argv[3])
manifest["closed_at"] = datetime.now(timezone.utc).isoformat()
manifest["status"] = "passed" if manifest["failed_rounds"] == 0 and manifest["readiness_exit_code"] == 0 else "failed"
path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
PY

echo "manifest: $RUN_DIR/manifest.json"
echo "readiness: $REPORT_OUT_DIR/readiness.md"

if [[ "$failed_rounds" -ne 0 || "$report_status" -ne 0 ]]; then
  exit 1
fi
