#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUT_DIR="${OUT_DIR:-target/test-lane-profiles}"
mkdir -p "$OUT_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
REPORT="$OUT_DIR/profile-$STAMP.md"

run_profile() {
  local name="$1"
  shift
  local log="$OUT_DIR/$STAMP-$name.log"
  echo "[profile] $name: $*"
  {
    echo "## $name"
    echo
    echo '```text'
    echo "$ $*"
    /usr/bin/time -p "$@" 2>&1
    echo '```'
    echo
  } | tee "$log" >> "$REPORT"
}

{
  echo "# Test Lane Profile"
  echo
  echo "- generated_at_utc: $STAMP"
  echo "- cwd: $ROOT_DIR"
  echo
} > "$REPORT"

run_profile "workflow" cargo test -q workflow
run_profile "memory" cargo test -q memory
run_profile "streaming" cargo test -q streaming
run_profile "memory-doctor-text" cargo test -q tools::memory_tool::tests::test_format_memory_doctor_includes_conflicts_and_counts -- --exact
run_profile "memory-doctor-json" cargo test -q tools::memory_tool::tests::test_memory_doctor_json_includes_calibration_and_gates -- --exact
run_profile "project-partner-closeout-memory-proposal" cargo test -q engine::conversation_loop::closeout_controller::tests::project_partner_profile_surfaces_review_only_memory_proposal -- --exact
run_profile "streaming-history-tool-calls" cargo test -q engine::streaming::tests::streaming_history_does_not_persist_completed_tool_calls_as_final_assistant_calls -- --exact

echo "[profile] report: $REPORT"
