#!/usr/bin/env bash
# Programming parity soak suite — real-programming release gate.
#
# Runs real programming tasks through the agent eval-run lane and collects
# pass/fail, failure_owner, cost/cache/provider hints, and runtime evidence.
#
# Usage:
#   bash scripts/programming-soak-suite.sh [all|task_name...]
#   PRIORITY_AGENT_SOAK_DRY_RUN=1 bash scripts/programming-soak-suite.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BASELINE_DIR="${PRIORITY_AGENT_SOAK_BASELINE_DIR:-$PROJECT_DIR/target/soak-bundles}"
RUN_ID="${PRIORITY_AGENT_SOAK_RUN_ID:-soak-$(date +%Y%m%d-%H%M%S)}"
BUNDLE_DIR="$BASELINE_DIR/$RUN_ID"
DRY_RUN="${PRIORITY_AGENT_SOAK_DRY_RUN:-0}"

ALL_TASKS=(
  "single-file-bug-fix"
  "multi-file-refactor"
  "frontend-component-change"
  "failing-test-repair"
  "permission-deny-safe-retry"
  "long-shell-output-paging"
  "revert-last-assistant-turn"
  "provider-slow-tail-classification"
)

selected_tasks() {
  if [[ "$#" -eq 0 || "${1:-}" == "all" ]]; then
    printf '%s\n' "${ALL_TASKS[@]}"
  else
    printf '%s\n' "$@"
  fi
}

task_prompt() {
  local task="$1"
  case "$task" in
    single-file-bug-fix)
      cat <<'EOF'
In this repository, inspect the smallest relevant code path for a narrow bug-risk fix.
Make at most one source-file change, run the narrowest relevant validation, and close out with evidence.
Do not change docs for this soak task unless a test requires it.
EOF
      ;;
    multi-file-refactor)
      cat <<'EOF'
In this repository, find a small duplicated helper or adjacent responsibility that can be refactored across two files.
Keep behavior unchanged, preserve existing boundaries, run targeted validation, and report exact changed files.
EOF
      ;;
    frontend-component-change)
      cat <<'EOF'
In this repository, inspect the desktop or TUI frontend-facing code and make one small usability improvement.
Run the narrowest compile or static check that covers the changed surface, and avoid unrelated redesign.
EOF
      ;;
    failing-test-repair)
      cat <<'EOF'
In this repository, run one narrow test that is likely to expose a real issue.
If it fails because of project logic, repair the project logic and rerun it.
If it fails because of environment/provider weakness, classify honestly without weakening validation.
EOF
      ;;
    permission-deny-safe-retry)
      cat <<'EOF'
In this repository, make a tiny code-comment or text-file change using the safe file mutation tools.
Do not use shell redirection, sed -i, perl -pi, or other bash-based workspace writes.
Run a narrow validation or explain why validation is not applicable.
EOF
      ;;
    long-shell-output-paging)
      cat <<'EOF'
In this repository, run a command that can produce substantial output, then inspect only the relevant evidence.
Use paging or bounded reads where available, avoid flooding the transcript, and close out with the exact proof used.
EOF
      ;;
    revert-last-assistant-turn)
      cat <<'EOF'
In this repository, make a tiny reversible file change, verify the changed file, then use the project's revert capability if available.
Confirm the final file state and report whether revert was complete or partial.
EOF
      ;;
    provider-slow-tail-classification)
      cat <<'EOF'
In this repository, exercise a narrow provider-backed or runtime-status path and report provider timeout/slow-tail classification evidence if present.
Do not mask timeout or cancellation evidence; classify environment versus project defects honestly.
EOF
      ;;
    *)
      cat <<EOF
Run a real programming-agent soak task named "$task" in this repository.
Keep the scope narrow, collect tool evidence, run targeted validation, and classify any failure_owner honestly.
EOF
      ;;
  esac
}

agent_command() {
  if [[ -n "${PRIORITY_AGENT_SOAK_BINARY:-}" ]]; then
    printf '%s\0' "$PRIORITY_AGENT_SOAK_BINARY"
  else
    printf '%s\0' cargo run --quiet --
  fi
}

write_task_summary() {
  local task="$1"
  local task_dir="$2"
  local exit_code="$3"
  python3 - "$RUN_ID" "$task" "$task_dir" "$exit_code" <<'PY'
import json
import pathlib
import sys
from datetime import datetime, timezone

run_id, task, task_dir_raw, exit_code_raw = sys.argv[1:5]
task_dir = pathlib.Path(task_dir_raw)
exit_code = int(exit_code_raw)
events_path = task_dir / "events.jsonl"
output_path = task_dir / "agent-output.md"

events = []
if events_path.exists():
    for line in events_path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            pass

def text_of(path):
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except FileNotFoundError:
        return ""

output_text = text_of(output_path)
combined = "\n".join(json.dumps(e, ensure_ascii=False) for e in events) + "\n" + output_text
lower = combined.lower()

tool_calls = sum(
    1
    for event in events
    if str(event.get("event", "")).startswith("tool")
    or "tool" in str(event.get("type", "")).lower()
)
file_changes = sum(1 for marker in ("file_change", "mutation_result", "changed_files") if marker in lower)
turns = sum(1 for event in events if event.get("event") in {"turn_started", "turn_completed", "trace_summary"})

status = "passed" if exit_code == 0 else "failed"
failure_owner = None
if status == "failed":
    if "api key" in lower or "provider" in lower or "timeout" in lower:
        failure_owner = "provider_or_environment"
    elif "permission" in lower or "blocked" in lower:
        failure_owner = "permission_or_policy"
    else:
        failure_owner = "agent_or_project"

usage = {}
for event in events:
    payload = event.get("usage") or event.get("provider_usage") or {}
    if isinstance(payload, dict):
        for key in ("prompt_tokens", "completion_tokens", "cached_tokens", "cost_usd"):
            if key in payload and key not in usage:
                usage[key] = payload[key]

trace = next((event for event in reversed(events) if event.get("event") == "trace_summary"), {})
closeout_status = trace.get("status") or trace.get("terminal_status")

run = {
    "run_id": run_id,
    "task": task,
    "timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "status": status,
    "exit_code": exit_code,
    "failure_owner": failure_owner,
    "model": None,
    "provider": None,
    "turns": turns,
    "tool_calls": tool_calls,
    "file_changes": file_changes,
    "prompt_tokens": usage.get("prompt_tokens", 0),
    "completion_tokens": usage.get("completion_tokens", 0),
    "cached_tokens": usage.get("cached_tokens", 0),
    "cost_usd": usage.get("cost_usd", 0.0),
    "cache_hit_ratio": None,
    "closeout_status": closeout_status,
    "provider_health": None,
    "notes": "real eval-run" if events else "no events captured",
}
(task_dir / "run.json").write_text(json.dumps(run, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
(task_dir / "baseline-row.md").write_text(
    f"| {task} | {status} | {failure_owner or ''} | {turns} | {file_changes} | {run['cost_usd']} | {run['notes']} |\n",
    encoding="utf-8",
)
PY
}

run_task() {
  local task="$1"
  local task_dir="$BUNDLE_DIR/$task"
  local prompt_file="$task_dir/prompt.txt"
  local output_file="$task_dir/agent-output.md"
  local events_file="$task_dir/events.jsonl"
  local stdout_file="$task_dir/stdout.log"
  local stderr_file="$task_dir/stderr.log"

  mkdir -p "$task_dir"
  task_prompt "$task" >"$prompt_file"

  echo "  [$task] starting..."
  if [[ "$DRY_RUN" == "1" ]]; then
    cat >"$task_dir/run.json" <<EOF
{
  "run_id": "$RUN_ID",
  "task": "$task",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "status": "not_run",
  "failure_owner": null,
  "notes": "dry_run"
}
EOF
    echo "| $task | not_run | | 0 | 0 | 0.0 | dry_run |" >"$task_dir/baseline-row.md"
    echo "  [$task] dry-run bundle at $task_dir"
    return
  fi

  local -a cmd
  while IFS= read -r -d '' part; do
    cmd+=("$part")
  done < <(agent_command)
  cmd+=("--eval-run" "--prompt-file" "$prompt_file" "--output" "$output_file" "--events" "$events_file")

  set +e
  (
    cd "$PROJECT_DIR"
    PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS="${PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS:-1}" \
    PRIORITY_AGENT_AUTO_TEST="${PRIORITY_AGENT_AUTO_TEST:-1}" \
      "${cmd[@]}"
  ) >"$stdout_file" 2>"$stderr_file"
  local exit_code=$?
  set -e

  write_task_summary "$task" "$task_dir" "$exit_code"
  echo "  [$task] exit=$exit_code bundle=$task_dir"
}

TASKS=()
while IFS= read -r task; do
  TASKS+=("$task")
done < <(selected_tasks "$@")

echo "Programming Parity Soak Suite"
echo "Run ID: $RUN_ID"
echo "Tasks: ${#TASKS[@]}"
echo "Output: $BUNDLE_DIR"
echo "Dry run: $DRY_RUN"
echo ""

mkdir -p "$BUNDLE_DIR"

{
  echo "# Programming Soak Baseline $RUN_ID"
  echo ""
  echo "| Task | Status | Failure Owner | Turns | Files | Cost | Notes |"
  echo "|------|--------|---------------|-------|-------|------|-------|"
} >"$BUNDLE_DIR/baseline.md"

for task in "${TASKS[@]}"; do
  run_task "$task"
  cat "$BUNDLE_DIR/$task/baseline-row.md" >>"$BUNDLE_DIR/baseline.md"
done

{
  echo ""
  echo "## Summary"
  echo ""
  echo "- run_id: $RUN_ID"
  echo "- tasks: ${#TASKS[@]}"
  echo "- artifact_bundles: $BUNDLE_DIR"
  echo "- dry_run: $DRY_RUN"
  echo ""
  echo "## Release Blockers"
  echo ""
  echo "- Data loss: session parts/events are not recoverable after crash"
  echo "- False verified closeout"
  echo "- Provider timeout with no visible diagnosis"
  echo "- Permission hard gate bypass"
  echo "- API schema drift without version bump"
} >>"$BUNDLE_DIR/baseline.md"

echo ""
echo "Baseline saved to $BUNDLE_DIR/baseline.md"
echo "Artifact bundles: $BUNDLE_DIR"
