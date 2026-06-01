#!/usr/bin/env bash
# Run the canonical agent runtime without building or launching the desktop app.
#
# This is the fast path for complex agent-flow dogfood. It exercises the same
# StreamingQueryEngine used by CLI/TUI and desktop full turns, then leaves
# desktop testing to UI/bridge smoke checks.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROMPT_FILE="$ROOT_DIR/tests/fixtures/agent_runtime/complex_read_only_closeout.md"
OUT_DIR="$ROOT_DIR/target/agent-runtime-dogfood/$(date +%Y%m%d-%H%M%S)"
TIMEOUT_SECS="${TIMEOUT_SECS:-900}"

usage() {
  cat <<'EOF'
Usage:
  scripts/agent-runtime-dogfood.sh [--prompt-file PATH] [--out-dir PATH]

Runs one real non-interactive agent turn through priority-agent --eval-run.
Use this before desktop packaging when validating tool-loop and closeout flow.

Environment:
  TIMEOUT_SECS  Max wall-clock seconds for the run (default: 900)
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prompt-file)
      PROMPT_FILE="$2"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

mkdir -p "$OUT_DIR"
OUTPUT_FILE="$OUT_DIR/final.md"
EVENTS_FILE="$OUT_DIR/events.jsonl"
STDOUT_FILE="$OUT_DIR/stdout.log"
STDERR_FILE="$OUT_DIR/stderr.log"

echo "==> Building runtime binary"
cargo build -q

echo "==> Running unified runtime dogfood"
echo "prompt: $PROMPT_FILE"
echo "output: $OUTPUT_FILE"
echo "events: $EVENTS_FILE"

python3 - "$TIMEOUT_SECS" "$ROOT_DIR/target/debug/priority-agent" "$PROMPT_FILE" "$OUTPUT_FILE" "$EVENTS_FILE" "$STDOUT_FILE" "$STDERR_FILE" <<'PY'
import subprocess
import sys
from pathlib import Path

timeout_secs = int(sys.argv[1])
binary = sys.argv[2]
prompt_file = sys.argv[3]
output_file = sys.argv[4]
events_file = sys.argv[5]
stdout_file = Path(sys.argv[6])
stderr_file = Path(sys.argv[7])

cmd = [
    binary,
    "--eval-run",
    "--prompt-file",
    prompt_file,
    "--output",
    output_file,
    "--events",
    events_file,
]

with stdout_file.open("wb") as stdout, stderr_file.open("wb") as stderr:
    try:
        result = subprocess.run(
            cmd,
            stdout=stdout,
            stderr=stderr,
            timeout=timeout_secs,
            check=False,
        )
    except subprocess.TimeoutExpired:
        print(f"runtime dogfood timed out after {timeout_secs}s", file=sys.stderr)
        print(f"stderr: {stderr_file}", file=sys.stderr)
        sys.exit(124)

if result.returncode != 0:
    print(f"runtime dogfood failed with exit status {result.returncode}", file=sys.stderr)
    print(f"stderr: {stderr_file}", file=sys.stderr)
    sys.exit(result.returncode)
PY

python3 - "$EVENTS_FILE" "$OUTPUT_FILE" <<'PY'
import json
import re
import sys
from pathlib import Path

events_path = Path(sys.argv[1])
output_path = Path(sys.argv[2])

events = []
for line in events_path.read_text(encoding="utf-8").splitlines():
    if line.strip():
        events.append(json.loads(line))

names = [event.get("event") for event in events]
errors = [event for event in events if event.get("event") == "error"]
tool_completions = [event for event in events if event.get("event") == "tool_execution_complete"]
trace_summaries = [event for event in events if event.get("event") == "trace_summary"]
output = output_path.read_text(encoding="utf-8")

failures = []
if errors:
    failures.append(f"stream error events: {len(errors)}")
if "complete" not in names:
    failures.append("missing complete event")
if not trace_summaries:
    failures.append("missing trace_summary event")
if not tool_completions:
    failures.append("no tool completions recorded")
if len(tool_completions) >= 50:
    failures.append(f"tool completions reached loop cap boundary: {len(tool_completions)}")
if re.search(r"Closeout:\s*-\s*Status:\s*(failed|not_verified)", output, re.I):
    failures.append("read-only run produced failed/not_verified code-change closeout")

if trace_summaries:
    trace = trace_summaries[-1].get("trace") or {}
    status = str(trace_summaries[-1].get("status") or trace.get("status") or "").lower()
    if status and "completed" not in status:
        failures.append(f"trace status is not completed: {status}")

if failures:
    print("runtime dogfood assertions failed:", file=sys.stderr)
    for failure in failures:
        print(f"- {failure}", file=sys.stderr)
    sys.exit(1)

print(
    "runtime dogfood passed: "
    f"events={len(events)} tools={len(tool_completions)} "
    f"trace_events={trace_summaries[-1].get('event_count') if trace_summaries else 0}"
)
PY

echo "agent runtime dogfood passed"
