#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TASK_DIR="evalsets/live_tasks"
MODE="list"
CASE_ID=""
LABEL="live-eval"
RUN_ID=""
WORK_ROOT="target/live-evals"
WORKDIR=""
REPORT_DIR="docs/benchmarks"
SKIP_BUILD=0
RUN_TESTS=0

usage() {
  cat <<'EOF'
Usage:
  scripts/run_live_eval.sh --list
  scripts/run_live_eval.sh --case <id|all> --mode <prepare|api-plan|collect|full> [options]

Modes:
  list      List live task samples.
  prepare   Create a git worktree, prompt.txt, and RUNBOOK.md.
  api-plan  Prepare a worktree, start the API server, and ask MiniMax for a plan.
  collect   Collect diff/test output from an existing worktree.
  full      prepare + api-plan + optional collect.

Options:
  --case ID          Live task id, or "all".
  --mode MODE        list, prepare, api-plan, collect, or full.
  --workdir DIR      Existing task worktree for collect mode.
  --label LABEL      Report/run label (default: live-eval).
  --run-id ID        Stable run id (default: timestamp).
  --run-tests        Run acceptance.required_commands during collect/full.
  --skip-build       Reuse target/release/priority-agent for api-plan.
  -h, --help         Show this help.

MiniMax:
  api-plan/full intentionally require MINIMAX_API_KEY. The script starts the
  local API server with MiniMax as the provider and writes the LLM planning
  response to the live eval report directory.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --list) MODE="list"; shift ;;
    --case) CASE_ID="${2:-}"; shift 2 ;;
    --mode) MODE="${2:-}"; shift 2 ;;
    --workdir) WORKDIR="${2:-}"; shift 2 ;;
    --label) LABEL="${2:-}"; shift 2 ;;
    --run-id) RUN_ID="${2:-}"; shift 2 ;;
    --run-tests) RUN_TESTS=1; shift ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$RUN_ID" ]]; then
  RUN_ID="${LABEL}-$(date +%Y%m%d-%H%M%S)"
fi

need_yaml() {
  python3 - <<'PY'
try:
    import yaml  # noqa: F401
except Exception as exc:
    raise SystemExit(f"PyYAML is required for live eval parsing: {exc}")
PY
}

yaml_get() {
  local file="$1" path="$2" default="${3:-}"
  python3 - "$file" "$path" "$default" <<'PY'
import sys, yaml
file, path, default = sys.argv[1], sys.argv[2], sys.argv[3]
with open(file, "r", encoding="utf-8") as fh:
    data = yaml.safe_load(fh) or {}
cur = data
for part in path.split("."):
    if isinstance(cur, dict) and part in cur:
        cur = cur[part]
    else:
        print(default)
        raise SystemExit(0)
if cur is None:
    print(default)
elif isinstance(cur, (dict, list)):
    import json
    print(json.dumps(cur, ensure_ascii=False))
else:
    print(cur)
PY
}

yaml_list() {
  local file="$1" path="$2"
  python3 - "$file" "$path" <<'PY'
import sys, yaml
file, path = sys.argv[1], sys.argv[2]
with open(file, "r", encoding="utf-8") as fh:
    data = yaml.safe_load(fh) or {}
cur = data
for part in path.split("."):
    cur = cur.get(part, {}) if isinstance(cur, dict) else {}
if isinstance(cur, list):
    for item in cur:
        print(item)
PY
}

write_agent_prompt() {
  local file="$1" out="$2"
  python3 - "$file" "$out" <<'PY'
import sys, yaml

sample_path, out_path = sys.argv[1], sys.argv[2]
with open(sample_path, "r", encoding="utf-8") as fh:
    sample = yaml.safe_load(fh) or {}

def list_block(title, values, empty="(none)"):
    lines = [f"## {title}"]
    if values:
        lines.extend(f"- {item}" for item in values)
    else:
        lines.append(empty)
    return lines

acceptance = sample.get("acceptance") or {}
diff_constraints = acceptance.get("diff_constraints") or {}

lines = [
    f"# Live coding regression task: {sample.get('title', sample.get('id', 'unknown'))}",
    "",
    f"- Task id: `{sample.get('id', 'unknown')}`",
    f"- Type: `{sample.get('type', 'unknown')}`",
    f"- Risk: `{sample.get('risk', 'unknown')}`",
    f"- Complexity: `{sample.get('complexity', 'unknown')}`",
    "",
    "## User task",
    "",
    str(sample.get("prompt", "")).strip(),
    "",
]

lines.extend(list_block("Allowed tools", sample.get("allowed_tools") or []))
lines.append("")
lines.extend(list_block("Forbidden tools", sample.get("forbidden_tools") or []))
lines.append("")
lines.extend(list_block("Expected behavior", sample.get("expected_behavior") or []))
lines.append("")
lines.extend([
    "## Acceptance checks",
    "",
    "Before your final response, run every required command below. If any command fails, inspect the failure, repair the code, and rerun the relevant command. Do not claim completion while required commands are failing.",
])
required = acceptance.get("required_commands") or []
if required:
    lines.extend(f"- `{cmd}`" for cmd in required)
else:
    lines.append("- (none)")
lines.append("")
lines.extend([
    "## Diff constraints",
    "",
    f"- Max files changed: `{diff_constraints.get('max_files_changed', 'unspecified')}`",
])
for forbidden in diff_constraints.get("forbidden_paths") or []:
    lines.append(f"- Do not change path: `{forbidden}`")
lines.append("")
lines.extend([
    "## Closeout requirements",
    "",
    "- Summarize files changed and why.",
    "- List validation commands you ran and their pass/fail status.",
    "- Mention any remaining risk or blocker explicitly.",
])

with open(out_path, "w", encoding="utf-8") as fh:
    fh.write("\n".join(lines).rstrip() + "\n")
PY
}

json_payload() {
  local prompt_file="$1" system_file="$2" context_file="$3"
  python3 - "$prompt_file" "$system_file" "$context_file" <<'PY'
import json, sys
prompt = open(sys.argv[1], encoding="utf-8").read()
system = open(sys.argv[2], encoding="utf-8").read()
context = open(sys.argv[3], encoding="utf-8").read()
message = prompt
if context.strip():
    message += "\n\n---\nRepository context for planning:\n" + context
print(json.dumps({
    "message": message,
    "system_prompt": system,
    "stream": False,
    "temperature": 0.2,
}, ensure_ascii=False))
PY
}

task_keywords() {
  local file="$1"
  python3 - "$file" <<'PY'
import re, sys, yaml
with open(sys.argv[1], encoding="utf-8") as fh:
    data = yaml.safe_load(fh) or {}
text = "\n".join(str(data.get(k, "")) for k in ("id", "title", "type", "prompt"))
stop = {
    "this", "that", "with", "from", "into", "should", "must", "only", "when",
    "true", "false", "mode", "task", "code", "file", "files", "test", "tests",
    "要求", "修复", "问题", "新增", "更新", "项目", "任务", "代码", "测试",
}
terms = []
for term in re.findall(r"[A-Za-z_][A-Za-z0-9_]{3,}|[\u4e00-\u9fff]{2,}", text):
    normalized = term.strip().lower()
    if normalized in stop:
        continue
    if normalized not in terms:
        terms.append(normalized)
for term in terms[:14]:
    print(term)
PY
}

build_repo_context() {
  local file="$1" task_workdir="$2" out="$3"
  local id title
  id="$(yaml_get "$file" id)"
  title="$(yaml_get "$file" title)"
  {
    echo "Task id: $id"
    echo "Task title: $title"
    echo "Repo language: Rust"
    echo "Current ref: $(git -C "$task_workdir" rev-parse --short HEAD 2>/dev/null || echo unknown)"
    echo
    echo "High-signal repository files:"
    (cd "$task_workdir" && find src -name '*.rs' -type f | sort | sed -n '1,180p') 2>/dev/null || true
    echo
    echo "Keyword hits:"
    local term
    while IFS= read -r term; do
      [[ -z "$term" ]] && continue
      echo
      echo "## $term"
      (cd "$task_workdir" && rg -n -m 8 --glob '*.rs' --glob '*.md' "$term" src docs Cargo.toml 2>/dev/null | sed -n '1,12p') || true
    done < <(task_keywords "$file")
  } >"$out"
}

task_env_base() {
  local id="$1"
  echo "$ROOT_DIR/$WORK_ROOT/$RUN_ID/$id/env"
}

ensure_task_env() {
  local id="$1" env_base
  env_base="$(task_env_base "$id")"
  mkdir -p \
    "$env_base/home" \
    "$env_base/xdg-config" \
    "$env_base/xdg-data" \
    "$env_base/xdg-state"
}

find_free_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()'
}

task_files() {
  find "$TASK_DIR" -maxdepth 1 -type f -name '*.yaml' | sort
}

find_task_file() {
  local id="$1"
  local file task_id
  for file in $(task_files); do
    task_id="$(yaml_get "$file" id)"
    if [[ "$task_id" == "$id" ]]; then
      echo "$file"
      return 0
    fi
  done
  return 1
}

list_tasks() {
  need_yaml
  printf '%-36s %-12s %-10s %s\n' "id" "type" "risk" "title"
  printf '%-36s %-12s %-10s %s\n' "--" "----" "----" "-----"
  local file
  for file in $(task_files); do
    printf '%-36s %-12s %-10s %s\n' \
      "$(yaml_get "$file" id)" \
      "$(yaml_get "$file" type)" \
      "$(yaml_get "$file" risk)" \
      "$(yaml_get "$file" title)"
  done
}

resolve_ref() {
  local requested="$1"
  if git rev-parse --verify --quiet "${requested}^{commit}" >/dev/null; then
    echo "$requested"
  else
    echo "HEAD"
  fi
}

prepare_task() {
  local file="$1"
  local id title base_ref resolved_ref task_workdir prompt_file runbook metadata env_base
  id="$(yaml_get "$file" id)"
  title="$(yaml_get "$file" title)"
  base_ref="$(yaml_get "$file" repo.base_ref HEAD)"
  resolved_ref="$(resolve_ref "$base_ref")"
  task_workdir="$WORK_ROOT/$RUN_ID/$id/worktree"
  prompt_file="$WORK_ROOT/$RUN_ID/$id/prompt.txt"
  runbook="$WORK_ROOT/$RUN_ID/$id/RUNBOOK.md"
  metadata="$WORK_ROOT/$RUN_ID/$id/metadata.json"
  env_base="$(task_env_base "$id")"

  mkdir -p "$(dirname "$task_workdir")"
  ensure_task_env "$id"
  if [[ ! -d "$task_workdir/.git" && ! -f "$task_workdir/.git" ]]; then
    git worktree add --force --detach "$task_workdir" "$resolved_ref" >/dev/null
  fi

  write_agent_prompt "$file" "$prompt_file"
  python3 - "$file" "$metadata" "$task_workdir" "$resolved_ref" "$env_base" <<'PY'
import json, sys, yaml
sample_path, metadata_path, workdir, resolved_ref, env_base = sys.argv[1:6]
with open(sample_path, encoding="utf-8") as fh:
    sample = yaml.safe_load(fh) or {}
sample["_sample_path"] = sample_path
sample["_workdir"] = workdir
sample["_resolved_ref"] = resolved_ref
sample["_env_base"] = env_base
with open(metadata_path, "w", encoding="utf-8") as fh:
    json.dump(sample, fh, ensure_ascii=False, indent=2)
PY

  {
    echo "# Live Eval Runbook: $id"
    echo
    echo "- Title: $title"
    echo "- Sample: $file"
    echo "- Requested base ref: $base_ref"
    echo "- Resolved base ref: $resolved_ref"
    echo "- Worktree: $task_workdir"
    echo "- Isolated env: $env_base"
    echo "- MiniMax model: ${MINIMAX_MODEL:-MiniMax-M2.7}"
    echo
    echo "## Prompt"
    echo
    cat "$prompt_file"
    echo
    echo "## Required Commands"
    echo
    local cmd
    while IFS= read -r cmd; do
      [[ -z "$cmd" ]] && continue
      echo "- \`$cmd\`"
    done < <(yaml_list "$file" acceptance.required_commands)
    echo
    echo "## Manual Agent Command"
    echo
    echo '```bash'
    echo "cd \"$task_workdir\""
    echo "mkdir -p \"$env_base/home\" \"$env_base/xdg-config\" \"$env_base/xdg-data\" \"$env_base/xdg-state\""
    echo "HOME=\"$env_base/home\" \\"
    echo "XDG_CONFIG_HOME=\"$env_base/xdg-config\" \\"
    echo "XDG_DATA_HOME=\"$env_base/xdg-data\" \\"
    echo "XDG_STATE_HOME=\"$env_base/xdg-state\" \\"
    echo "PRIORITY_AGENT_A2A_TRANSCRIPT_PATH=\"$env_base/a2a-transcript.jsonl\" \\"
    echo "MINIMAX_API_KEY=\"\${MINIMAX_API_KEY:?}\" \\"
    echo "MINIMAX_BASE_URL=\"\${MINIMAX_BASE_URL:-}\" \\"
    echo "MINIMAX_MODEL=\"\${MINIMAX_MODEL:-MiniMax-M2.7}\" \\"
    echo "OPENAI_API_KEY=\"\" \\"
    echo "MOONSHOT_API_KEY=\"\" \\"
    echo "\"$ROOT_DIR/target/release/priority-agent\""
    echo '```'
    echo
    echo "After the agent run, collect results with:"
    echo
    echo '```bash'
    echo "scripts/run_live_eval.sh --case $id --mode collect --workdir \"$task_workdir\" --run-tests"
    echo '```'
  } >"$runbook"

  echo "$task_workdir"
}

build_binary() {
  if [[ "$SKIP_BUILD" -eq 0 ]]; then
    cargo build --release --features experimental-api-server >/dev/null
  fi
  if [[ ! -x "$ROOT_DIR/target/release/priority-agent" ]]; then
    echo "Missing binary: $ROOT_DIR/target/release/priority-agent" >&2
    exit 1
  fi
}

api_plan_task() {
  local file="$1" task_workdir="$2"
  local id report_dir server_log system_file response_file plan_file payload_file context_file headers_file lint_file env_base port server_pid ready
  id="$(yaml_get "$file" id)"
  report_dir="$REPORT_DIR/live-$RUN_ID/$id"
  mkdir -p "$report_dir"
  server_log="$report_dir/api-server.log"
  system_file="$report_dir/system-prompt.txt"
  response_file="$report_dir/api-response.json"
  plan_file="$report_dir/minimax-plan.md"
  payload_file="$report_dir/request.json"
  context_file="$report_dir/repo-context.txt"
  headers_file="$report_dir/api-response.headers"
  lint_file="$report_dir/plan-lint.txt"
  env_base="$(task_env_base "$id")"

  if [[ -z "${MINIMAX_API_KEY:-}" ]]; then
    echo "MINIMAX_API_KEY is required for api-plan/full mode." >&2
    exit 1
  fi

  build_binary
  ensure_task_env "$id"

  cat >"$system_file" <<'EOF'
You are evaluating Priority Agent on a live coding regression task.
Use MiniMax as the model under test.
This repository is a Rust coding-agent project, not a Python project.
This API endpoint cannot execute tools. Do not emit TOOL_CALL blocks, XML tags,
<think> blocks, hidden reasoning tags, or pretend that files were inspected or
edited.
Do not claim that you edited files. Produce a concise engineering plan only:
1. classify task type/risk,
2. identify first blocker,
3. list likely files to inspect,
4. list acceptance checks,
5. note memory/skill/evolution risks if relevant.
Any XML-like tag, pseudo tool call, or "let me inspect/run/edit" action text is
an evaluation failure for this plan-only mode. Stop after the plan.
Do not include an implementation preamble, a "ready to proceed" sentence, or a
closing sentence about running commands. The response must end immediately after
the fifth plan section.
If you are uncertain, say what should be inspected next instead of fabricating
tool results.
EOF

  build_repo_context "$file" "$task_workdir" "$context_file"
  json_payload "$WORK_ROOT/$RUN_ID/$id/prompt.txt" "$system_file" "$context_file" >"$payload_file"
  port="$(find_free_port)"
  (
    cd "$task_workdir"
    env \
      HOME="$env_base/home" \
      XDG_CONFIG_HOME="$env_base/xdg-config" \
      XDG_DATA_HOME="$env_base/xdg-data" \
      XDG_STATE_HOME="$env_base/xdg-state" \
      PRIORITY_AGENT_A2A_TRANSCRIPT_PATH="$env_base/a2a-transcript.jsonl" \
      MINIMAX_API_KEY="$MINIMAX_API_KEY" \
      MINIMAX_BASE_URL="${MINIMAX_BASE_URL:-}" \
      MINIMAX_MODEL="${MINIMAX_MODEL:-MiniMax-M2.7}" \
      OPENAI_API_KEY="" \
      MOONSHOT_API_KEY="" \
      "$ROOT_DIR/target/release/priority-agent" --api --port "$port"
  ) >"$server_log" 2>&1 &
  server_pid="$!"

  cleanup_server() {
    if kill -0 "$server_pid" >/dev/null 2>&1; then
      kill "$server_pid" >/dev/null 2>&1 || true
      wait "$server_pid" >/dev/null 2>&1 || true
    fi
  }

  ready=0
  for _ in {1..80}; do
    if curl -fsS "http://127.0.0.1:$port/api/health" >/dev/null 2>&1; then
      ready=1
      break
    fi
    sleep 0.25
  done
  if [[ "$ready" -ne 1 ]]; then
    cleanup_server
    echo "API server did not become healthy. See $server_log" >&2
    exit 1
  fi

  local curl_status=0 http_status
  curl -sS -D "$headers_file" -o "$response_file" \
    -X POST "http://127.0.0.1:$port/api/chat" \
    -H "Content-Type: application/json" \
    --data-binary "@$payload_file" || curl_status=$?
  cleanup_server
  if [[ "$curl_status" -ne 0 ]]; then
    echo "API chat request failed. See $server_log and $payload_file" >&2
    exit "$curl_status"
  fi
  http_status="$(awk 'NR==1{print $2}' "$headers_file")"
  if [[ ! "$http_status" =~ ^2[0-9][0-9]$ ]]; then
    echo "API chat request returned HTTP $http_status. See $response_file, $headers_file, and $payload_file" >&2
    exit 1
  fi

  python3 - "$response_file" "$plan_file" "$lint_file" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as fh:
    data = json.load(fh)
content = data.get("content", "")
with open(sys.argv[2], "w", encoding="utf-8") as fh:
    fh.write(content)
    if not content.endswith("\n"):
        fh.write("\n")

forbidden = ["<think", "</think>", "<thinking", "</thinking>", "TOOL_CALL", "<tool_call"]
forbidden_phrases = [
    "let me start",
    "let me inspect",
    "let me run",
    "let me edit",
    "ready to proceed with implementation",
]
lower_content = content.lower()
violations = [marker for marker in forbidden if marker.lower() in lower_content]
violations.extend(
    f"action_text:{phrase}" for phrase in forbidden_phrases if phrase in lower_content
)
with open(sys.argv[3], "w", encoding="utf-8") as fh:
    if violations:
        fh.write("status=failed\n")
        fh.write("reason=forbidden markers in API plan output\n")
        for marker in violations:
            fh.write(f"violation={marker}\n")
    else:
        fh.write("status=ok\n")
PY
  if grep -q '^status=failed' "$lint_file"; then
    echo "MiniMax plan failed lint. See $lint_file and $plan_file" >&2
    exit 1
  fi

  echo "$plan_file"
}

collect_task() {
  local file="$1" task_workdir="$2"
  local id report_dir report diff_stat diff_patch cmd_log test_status env_base
  id="$(yaml_get "$file" id)"
  report_dir="$REPORT_DIR/live-$RUN_ID/$id"
  mkdir -p "$report_dir"
  report="$report_dir/report.md"
  diff_stat="$report_dir/diff-stat.txt"
  diff_patch="$report_dir/diff.patch"
  cmd_log="$report_dir/required-commands.log"
  test_status="skipped"
  env_base="$(task_env_base "$id")"

  git -C "$task_workdir" status --short >"$report_dir/git-status.txt" || true
  git -C "$task_workdir" diff --stat >"$diff_stat" || true
  git -C "$task_workdir" diff >"$diff_patch" || true

  : >"$cmd_log"
  if [[ "$RUN_TESTS" -eq 1 ]]; then
    test_status="ok"
    local cmd
    while IFS= read -r cmd; do
      [[ -z "$cmd" ]] && continue
      (
        set +e
        echo "\$ $cmd"
        (cd "$task_workdir" && bash -lc "$cmd")
        status=$?
        echo "[exit status: $status]"
        echo
        exit "$status"
      ) >>"$cmd_log" 2>&1 || test_status="failed"
    done < <(yaml_list "$file" acceptance.required_commands)
  fi

  {
    echo "# Live Eval Report: $id"
    echo
    echo "- Run id: \`$RUN_ID\`"
    echo "- Sample: \`$file\`"
    echo "- Worktree: \`$task_workdir\`"
    echo "- Isolated env: \`$env_base\`"
    echo "- Test status: \`$test_status\`"
    echo "- Generated: \`$(date '+%Y-%m-%d %H:%M:%S %z')\`"
    echo
    echo "## Git Status"
    echo
    echo '```text'
    cat "$report_dir/git-status.txt"
    echo '```'
    echo
    echo "## Diff Stat"
    echo
    echo '```text'
    cat "$diff_stat"
    echo '```'
    echo
    echo "## Required Commands"
    echo
    echo '```text'
    cat "$cmd_log"
    echo '```'
    echo
    echo "## Human Review"
    echo
    echo "- accepted: TODO"
    echo "- task_success: TODO"
    echo "- mainline_hit: TODO"
    echo "- plan_coverage: TODO"
    echo "- rework_count: TODO"
    echo "- tool_efficiency: TODO"
    echo "- diff_discipline: TODO"
    echo "- closeout_accuracy: TODO"
    echo "- notes: TODO"
  } >"$report"

  echo "$report"
}

run_one() {
  local file="$1" id task_workdir plan_path report_path
  id="$(yaml_get "$file" id)"
  case "$MODE" in
    prepare)
      task_workdir="$(prepare_task "$file")"
      echo "Prepared $id: $task_workdir"
      ;;
    api-plan)
      task_workdir="$(prepare_task "$file")"
      if ! plan_path="$(api_plan_task "$file" "$task_workdir")"; then
        return 1
      fi
      echo "MiniMax plan for $id: $plan_path"
      ;;
    collect)
      if [[ -z "$WORKDIR" ]]; then
        echo "--workdir is required for collect mode" >&2
        exit 1
      fi
      report_path="$(collect_task "$file" "$WORKDIR")"
      echo "Collected $id: $report_path"
      ;;
    full)
      task_workdir="$(prepare_task "$file")"
      if ! plan_path="$(api_plan_task "$file" "$task_workdir")"; then
        return 1
      fi
      report_path="$(collect_task "$file" "$task_workdir")"
      echo "MiniMax plan for $id: $plan_path"
      echo "Collected $id: $report_path"
      ;;
    *)
      echo "Unknown mode: $MODE" >&2
      exit 1
      ;;
  esac
}

main() {
  need_yaml
  if [[ "$MODE" == "list" ]]; then
    list_tasks
    exit 0
  fi
  if [[ -z "$CASE_ID" ]]; then
    echo "--case is required unless --list is used." >&2
    usage
    exit 1
  fi

  mkdir -p "$REPORT_DIR" "$WORK_ROOT/$RUN_ID"

  if [[ "$CASE_ID" == "all" ]]; then
    local file failures=0
    for file in $(task_files); do
      if ! run_one "$file"; then
        echo "Live eval task failed: $(yaml_get "$file" id)" >&2
        failures=$((failures + 1))
      fi
    done
    if [[ "$failures" -gt 0 ]]; then
      echo "Live eval completed with $failures failed task(s)." >&2
      exit 1
    fi
  else
    local file
    if ! file="$(find_task_file "$CASE_ID")"; then
      echo "No live task found for id: $CASE_ID" >&2
      exit 1
    fi
    run_one "$file"
  fi
}

main
