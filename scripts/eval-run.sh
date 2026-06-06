#!/usr/bin/env bash
# Priority Agent Eval Runner
# Usage: ./scripts/eval-run.sh [tier-1|tier-2|tier-3|tier-4|tier-5|all|help]

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

EVALSETS_DIR="$ROOT_DIR/evalsets"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

print_header() {
    echo ""
    echo -e "${BLUE}════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Priority Agent Eval Runner${NC}"
    echo -e "${BLUE}════════════════════════════════════════════════${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}→ $1${NC}"
}

print_result() {
    local task="$1"
    local status="$2"
    if [ "$status" = "PASS" ]; then
        echo -e "  ${GREEN}✓${NC} $task"
    else
        echo -e "  ${RED}✗${NC} $task"
    fi
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_progress() {
    echo -e "${CYAN}[$1]${NC} $2"
}

# ─── Real-time Progress Monitor ───────────────────────────────
# Watches events.jsonl and prints human-readable progress lines.
# Usage: monitor_progress <events_file> <task_name> [verbose]
#   verbose: if non-empty, shows full tool args/results and thinking

monitor_progress() {
    local events_file="$1"
    local task_name="$2"
    local verbose="${3:-}"

    python3 - "$events_file" "$task_name" "$verbose" <<'PY' &
import json
import sys
import time
import os
import textwrap

events_file, task_name, verbose = sys.argv[1], sys.argv[2], sys.argv[3]
verbose_mode = bool(verbose and verbose.strip())

# ── state ──
last_pos = 0
iteration = 0
max_iterations = 150  # updated from runtime_diagnostic if present
tool_args_buffer = {}   # id -> accumulated args
tool_name_map = {}      # id -> name
current_tool = None
last_tool_result = None
total_prompt = 0
total_completion = 0
start_time = time.time()

GREEN = '\033[0;32m'
YELLOW = '\033[1;33m'
CYAN = '\033[0;36m'
GRAY = '\033[0;90m'
RED = '\033[0;31m'
RESET = '\033[0m'

def fmt(msg, color=""):
    return f"{color}{msg}{RESET}"

def print_tool_call(name, args_summary):
    prefix = fmt("  →", CYAN)
    print(f"{prefix} {fmt(name, YELLOW)} {args_summary}", flush=True)

def print_tool_result(success, preview):
    icon = fmt("✓", GREEN) if success else fmt("✗", RED)
    # Truncate preview to ~200 chars for normal, 500 for verbose
    limit = 500 if verbose_mode else 200
    if len(preview) > limit:
        preview = preview[:limit] + " …"
    lines = preview.split('\n')
    if len(lines) > 3 and not verbose_mode:
        preview = '\n'.join(lines[:3]) + "\n…"
    print(f"    {icon} {preview}", flush=True)

def print_iteration(it, max_it):
    pct = int(100 * it / max_it) if max_it else 0
    bar_len = 20
    filled = int(bar_len * it / max_it) if max_it else 0
    bar = '█' * filled + '░' * (bar_len - filled)
    print(f"  {fmt('[' + bar + ']', GRAY)} {fmt(str(it) + '/' + str(max_it), CYAN)} ({pct}%)  iteration", flush=True)

def print_thinking(text):
    # Show a compact indicator that LLM is thinking
    text = text.strip()
    if not text:
        return
    # Only show first line, truncated
    first_line = text.split('\n')[0][:80]
    print(f"  {fmt('💭', GRAY)} {fmt(first_line, GRAY)}", flush=True)

def print_usage():
    print(f"  {fmt('Tokens:', GRAY)} prompt={total_prompt} completion={total_completion} total={total_prompt+total_completion}", flush=True)

# Wait for file
while not os.path.exists(events_file):
    time.sleep(0.5)

while True:
    try:
        current_size = os.path.getsize(events_file)
    except OSError:
        time.sleep(0.5)
        continue

    if current_size > last_pos:
        with open(events_file, 'r') as f:
            f.seek(last_pos)
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    continue

                ev = event.get("event", "")

                # ── lifecycle ──
                if ev == "start":
                    print(f"\n{fmt('═' * 50, CYAN)}")
                    print(f"  {fmt('▶ Agent started:', CYAN)} {task_name}")
                    print(f"{fmt('═' * 50, CYAN)}")

                elif ev == "complete":
                    elapsed = int(time.time() - start_time)
                    print(f"\n  {fmt('■ Agent complete', GREEN)}  ({elapsed}s elapsed)")
                    print(f"{fmt('═' * 50, CYAN)}\n")

                elif ev == "error":
                    msg = event.get("message", "unknown error")
                    print(f"\n  {fmt('✗ Error:', RED)} {msg}\n")

                # ── iteration / model roundtrip ──
                elif ev == "runtime_diagnostic":
                    diag = event.get("diagnostic", {})
                    stage = diag.get("stage", "")

                    if stage == "conversation_loop_starting":
                        timeout = diag.get("timeout_secs", "?")
                        print(f"\n  {fmt('⏱  Timeout:', YELLOW)} {timeout}s")

                    elif stage == "api_request_started":
                        it = diag.get("iteration", 0)
                        if it and it != iteration:
                            iteration = it
                            print_iteration(iteration, max_iterations)
                        model = diag.get("model", "")
                        tools = diag.get("tools", "")
                        streaming = "stream" if diag.get("streaming") else "batch"
                        shape = diag.get("request_shape", "")
                        print(f"    {fmt('⟳', CYAN)} LLM request  model={model}  tools={tools}  {streaming}  {shape}")

                    elif stage == "provider_request_completed":
                        ms = diag.get("elapsed_ms", "?")
                        success = "ok" if diag.get("success") else "FAIL"
                        print(f"    {fmt('↳', CYAN)} Response     {ms}ms  [{success}]")

                    elif stage == "messages_built":
                        count = diag.get("messages", "?")
                        print(f"    {fmt('📨', GRAY)} Messages built: {count}")

                    elif stage == "preflight_compression_checked":
                        print(f"    {fmt('🗜', GRAY)} Compression checked")

                    elif verbose_mode:
                        # In verbose, show all other diagnostics
                        print(f"    {fmt('🔧', GRAY)} diag: {stage}")

                # ── tool calls (arguments streaming in) ──
                elif ev == "tool_call_start":
                    tid = event.get("id", "")
                    name = event.get("name", "unknown")
                    tool_name_map[tid] = name
                    tool_args_buffer[tid] = ""
                    current_tool = name

                elif ev == "tool_call_args":
                    tid = event.get("id", "")
                    delta = event.get("args_delta", "")
                    if tid in tool_args_buffer:
                        tool_args_buffer[tid] += delta

                elif ev == "tool_call_complete":
                    tid = event.get("id", "")
                    name = tool_name_map.get(tid, current_tool or "unknown")
                    args = tool_args_buffer.get(tid, "")
                    # Pretty-print args if valid JSON
                    args_summary = ""
                    if args:
                        try:
                            a = json.loads(args)
                            if isinstance(a, dict):
                                # Show key=value pairs, compact
                                pairs = []
                                for k, v in a.items():
                                    sv = str(v)
                                    if len(sv) > 60:
                                        sv = sv[:57] + "…"
                                    pairs.append(f"{k}={sv}")
                                args_summary = "  ".join(pairs)
                            else:
                                args_summary = str(a)[:80]
                        except json.JSONDecodeError:
                            args_summary = args[:80]
                    if args_summary:
                        print_tool_call(name, args_summary)
                    else:
                        print_tool_call(name, "")
                    # Clean up
                    tool_args_buffer.pop(tid, None)
                    tool_name_map.pop(tid, None)

                # ── tool execution ──
                elif ev == "tool_execution_start":
                    meta = event.get("metadata", {})
                    tool = meta.get("tool", event.get("name", "unknown"))
                    path = meta.get("path", "")
                    if path:
                        print(f"    {fmt('▸', GRAY)} exec: {tool}  path={path}")

                elif ev == "tool_execution_progress":
                    prog = event.get("progress", "")
                    if prog and verbose_mode:
                        print(f"    {fmt('…', GRAY)} {prog}")

                elif ev == "tool_execution_complete":
                    meta = event.get("metadata", {})
                    success = meta.get("success", True)
                    preview = event.get("result_preview", "")
                    chars = event.get("result_chars", 0)
                    if preview:
                        print_tool_result(success, preview)
                    elif chars:
                        print(f"    {fmt('✓', GREEN) if success else fmt('✗', RED)}  ({chars} chars)")
                    else:
                        print(f"    {fmt('✓', GREEN) if success else fmt('✗', RED)}")

                # ── thinking ──
                elif ev == "thinking_start":
                    if verbose_mode:
                        print(f"  {fmt('🧠', GRAY)} Thinking…")

                elif ev == "thinking_chunk":
                    # Actual thinking text is not in the event (only char count)
                    # In verbose mode we at least acknowledge it
                    if verbose_mode:
                        chars = event.get("chars", 0)
                        print(f"    {fmt('·', GRAY)} {chars} chars of reasoning")

                elif ev == "thinking_complete":
                    if verbose_mode:
                        print(f"  {fmt('✓', GRAY)} Thinking complete")

                # ── usage ──
                elif ev == "usage":
                    pt = event.get("prompt_tokens", 0) or 0
                    ct = event.get("completion_tokens", 0) or 0
                    rt = event.get("reasoning_tokens", 0) or 0
                    if pt:
                        total_prompt += pt
                    if ct:
                        total_completion += ct
                    if verbose_mode and (pt or ct):
                        print(f"    {fmt('📊', GRAY)} usage  prompt={pt}  completion={ct}  reasoning={rt}")

                # ── closeout ──
                elif ev == "closeout":
                    status = event.get("status", "?")
                    ev_sum = event.get("evidence_summary", "")
                    color = GREEN if status == "completed" else (RED if status == "failed" else YELLOW)
                    print(f"\n  {fmt('📝 Closeout:', color)} {status}")
                    if ev_sum and verbose_mode:
                        for line in textwrap.wrap(ev_sum, width=70):
                            print(f"      {fmt(line, GRAY)}")

                # ── text output ──
                elif ev == "text_chunk":
                    if verbose_mode:
                        chars = event.get("chars", 0)
                        print(f"    {fmt('📝', GRAY)} text +{chars} chars")

                elif ev == "output_truncated":
                    print(f"  {fmt('⚠ Output truncated', YELLOW)}")

            last_pos = f.tell()

    time.sleep(0.5)
PY
}

# Extract field from YAML task file using Python
yaml_get() {
    local task_file="$1"
    local field="$2"
    python3 - "$task_file" "$field" <<'PY' 2>/dev/null
import yaml
import sys

task_file, field = sys.argv[1], sys.argv[2]
with open(task_file) as f:
    data = yaml.safe_load(f) or {}

value = data
for key in field.split('.'):
    if value is None:
        break
    value = value.get(key) if isinstance(value, dict) else None

if isinstance(value, list):
    for item in value:
        print(item)
elif isinstance(value, (str, int, float, bool)):
    print(value)
PY
}

yaml_list_has_items() {
    local task_file="$1"
    local field="$2"
    [ -n "$(yaml_get "$task_file" "$field" | sed '/^[[:space:]]*$/d')" ]
}

write_task_prompt() {
    local task_file="$1"
    local prompt_file="$2"
    python3 - "$task_file" "$prompt_file" <<'PY'
import yaml
import sys

task_file, prompt_file = sys.argv[1], sys.argv[2]
with open(task_file) as f:
    task = yaml.safe_load(f) or {}

acceptance = task.get("acceptance") or {}
diff_constraints = acceptance.get("diff_constraints") or {}

def list_block(title, values, empty="(none)"):
    lines = [f"## {title}"]
    if not isinstance(values, list) or not values:
        lines.append(empty)
    else:
        lines.extend(f"- {item}" for item in values)
    return lines

lines = [
    f"# Eval task: {task.get('title') or task.get('id') or 'unknown'}",
    "",
    f"- Task id: `{task.get('id') or 'unknown'}`",
    f"- Eval intent: `{task.get('eval_intent') or 'seeded_code_change'}`",
    f"- Risk: `{task.get('risk') or 'unknown'}`",
    f"- Complexity: `{task.get('complexity') or 'unknown'}`",
    "",
    "## User task",
    "",
    str(task.get("prompt") or "").strip(),
    "",
]
lines.extend(list_block("Allowed tools", task.get("allowed_tools") or []))
lines.append("")
lines.extend(list_block("Forbidden tools", task.get("forbidden_tools") or []))
lines.append("")
lines.extend(list_block("Expected behavior", task.get("expected_behavior") or []))
lines.extend(["", "## Acceptance checks", ""])

required = acceptance.get("required_commands") or []
if required:
    lines.append("Before final closeout, run every required command below. If a command fails, repair and rerun it; do not claim verified completion while any required command is failing.")
    lines.extend(f"- `{cmd}`" for cmd in required)
else:
    lines.append("- No agent-run required command is defined for this task.")

harness = acceptance.get("harness_commands") or []
if harness:
    lines.extend([
        "",
        "Harness-only commands will run after the agent turn. Do not spend the agent loop on them unless a focused failure points there.",
    ])

lines.extend([
    "",
    "## Diff constraints",
    "",
    f"- Max files changed: `{diff_constraints.get('max_files_changed', 'unspecified')}`",
])
for forbidden in diff_constraints.get("forbidden_paths") or []:
    lines.append(f"- Do not change path: `{forbidden}`")

intent = str(task.get("eval_intent") or "seeded_code_change")
lines.extend(["", "## Closeout requirements", ""])
if intent == "direct_answer":
    lines.append("- This is a direct-answer eval. Do not inspect or mutate the repository unless the task explicitly requires it.")
elif intent == "read_only_audit":
    lines.append("- This is a read-only local evidence eval. Inspect narrowly and do not modify files.")
elif intent in {"audit_or_regression_check", "stale_or_already_satisfied"}:
    lines.append("- This is an audit/regression eval. If behavior is already present, prove it instead of forcing an arbitrary edit.")
else:
    lines.append("- This is a real code-change eval. Fix the issue and provide verification evidence.")

with open(prompt_file, "w") as out:
    out.write("\n".join(lines).rstrip() + "\n")
PY
}

capture_status_paths() {
    local output_file="$1"
    {
        git diff --name-only
        git ls-files --others --exclude-standard
    } | grep -v '^target/' | grep -v '/target/' | grep -v '^\.git/' | sed '/^[[:space:]]*$/d' | sort -u > "$output_file"
}

print_status_path_delta() {
    local baseline_file="$1"
    local current_file="$2"
    comm -13 "$baseline_file" "$current_file" || true
}

# Prepare task fixtures by running prepare_commands from YAML
prepare_task() {
    local task_file="$1"
    local task_name
    task_name=$(basename "$task_file" .yaml)

    print_info "Preparing fixtures for: $task_name"

    # Use Python to write each command to a separate file, then execute
    local temp_dir
    temp_dir=$(mktemp -d)

    python3 -c "
import yaml
import sys

with open('$task_file') as f:
    data = yaml.safe_load(f)

repo = data.get('repo', {})
commands = repo.get('prepare_commands', [])

if not commands:
    sys.exit(0)

for i, cmd in enumerate(commands):
    with open('$temp_dir/cmd_{i}.sh', 'w') as out:
        out.write('#!/bin/bash\nset -e\n')
        out.write(cmd)
        out.write('\n')
    print(i)
" 2>/dev/null

    local count=0
    for cmd_file in "$temp_dir"/cmd_*.sh; do
        [ -f "$cmd_file" ] || continue
        count=$((count + 1))
        chmod +x "$cmd_file"
        echo "  [prepare $count] Running multi-line command..."
        if bash "$cmd_file" >/dev/null 2>&1; then
            echo -e "  ${GREEN}✓${NC} prepare command $count succeeded"
        else
            echo -e "  ${RED}✗${NC} prepare command $count failed"
            rm -rf "$temp_dir"
            return 1
        fi
    done

    rm -rf "$temp_dir"

    if [ "$count" -eq 0 ]; then
        print_info "No prepare_commands found, skipping fixture setup"
        return 0
    fi

    print_success "Fixture preparation complete ($count commands)"
}

# Show task summary after completion
show_task_summary() {
    local task_name="$1"
    local output_file="$2"
    local events_file="$3"

    echo ""
    echo -e "${CYAN}═══ Task Summary: $task_name ═══${NC}"

    # Show response preview
    if [ -f "$output_file" ] && [ -s "$output_file" ]; then
        echo ""
        echo "Response preview:"
        local preview
        preview=$(head -8 "$output_file" 2>/dev/null || echo "<empty output>")
        echo "$preview" | sed 's/^/  /'
        local total_lines
        total_lines=$(wc -l < "$output_file" 2>/dev/null || echo 0)
        if [ "$total_lines" -gt 8 ]; then
            echo "  ... ($((total_lines - 8)) more lines)"
        fi
    else
        echo ""
        echo "  <no output file>"
    fi

    # Show event statistics
    if [ -f "$events_file" ] && [ -s "$events_file" ]; then
        echo ""
        echo "Event statistics:"
        local tool_calls
        tool_calls=$(rg -c '"event":\s*"tool_call_start"' "$events_file" 2>/dev/null || echo 0)
        echo "  Tool calls: $tool_calls"

        local text_chunks
        text_chunks=$(rg -c '"event":\s*"text_chunk"' "$events_file" 2>/dev/null || echo 0)
        echo "  Text chunks: $text_chunks"

        local thinking_chunks
        thinking_chunks=$(rg -c '"event":\s*"thinking_chunk"' "$events_file" 2>/dev/null || echo 0)
        if [ "$thinking_chunks" -gt 0 ]; then
            echo "  Thinking chunks: $thinking_chunks"
        fi

        # Check for errors
        if rg -q '"event":\s*"error"' "$events_file" 2>/dev/null; then
            echo -e "  ${RED}⚠ Errors detected in events${NC}"
        fi

        # Check for closeout
        local closeout_status
        closeout_status=$(rg '"status":\s*"([^"]+)"' -o -r '$1' "$events_file" 2>/dev/null | tail -1)
        if [ -n "$closeout_status" ]; then
            echo -e "  Closeout status: ${CYAN}$closeout_status${NC}"
        fi
    fi

    # Check for iteration limit warning
    if [ -f "$output_file" ] && rg -q "iteration limit" "$output_file" 2>/dev/null; then
        echo ""
        print_warning "Iteration limit was reached"
    fi

    echo ""
    echo "Output files:"
    echo "  Report: $output_file"
    echo "  Events: $events_file"
}

# ── Cleanup stale processes from previous runs ──
cleanup_stale_processes() {
    local task_name="$1"
    # Kill any lingering cargo test/build/run processes that might hold locks
    if pgrep -f "cargo test" > /dev/null 2>&1; then
        print_warning "Killing stale cargo processes from previous runs…"
        pkill -9 -f "cargo test" 2>/dev/null || true
        pkill -9 -f "cargo run" 2>/dev/null || true
        pkill -9 -f "cargo build" 2>/dev/null || true
        sleep 1
    fi
}

# Run a single eval task YAML file
run_task_file() {
    local task_file="$1"
    local task_name
    task_name=$(basename "$task_file" .yaml)

    echo ""
    echo -e "${BLUE}═══ Task: $task_name ═══${NC}"

    # Step 0: Cleanup stale processes
    cleanup_stale_processes "$task_name"

    # Step 1: Prepare fixtures
    if ! prepare_task "$task_file"; then
        print_error "Fixture preparation failed: $task_name"
        return 1
    fi

    # Step 2: Ensure the YAML has a prompt.
    if [ -z "$(yaml_get "$task_file" "prompt")" ]; then
        print_error "Could not extract prompt from $task_file"
        return 1
    fi

    # Step 3: Check if eval-run mode is available
    if [ ! -f "./target/debug/priority-agent" ]; then
        print_info "Building priority-agent..."
        if ! cargo build -q; then
            print_error "Failed to build priority-agent"
            return 1
        fi
    fi

    # Step 4: Write enriched prompt to temp file
    local prompt_file
    prompt_file=$(mktemp)
    write_task_prompt "$task_file" "$prompt_file"

    # Step 5: Setup output paths
    local timestamp
    timestamp=$(date +%Y%m%d-%H%M%S)
    local output_file="$ROOT_DIR/target/eval-reports/${task_name}-${timestamp}.md"
    local events_file="$ROOT_DIR/target/eval-reports/${task_name}-${timestamp}.jsonl"
    local baseline_status_file="$ROOT_DIR/target/eval-reports/${task_name}-${timestamp}.baseline-paths"
    mkdir -p "$ROOT_DIR/target/eval-reports"
    capture_status_paths "$baseline_status_file"

    print_info "Running eval task..."

    # Step 6: Set eval environment
    local eval_intent
    eval_intent=$(yaml_get "$task_file" "eval_intent")
    if [ -z "${PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS:-}" ]; then
        if [ "$eval_intent" = "seeded_code_change" ]; then
            export PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS=1
        else
            export PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS=0
        fi
    fi
    export PRIORITY_AGENT_AUTO_APPROVE="${PRIORITY_AGENT_AUTO_APPROVE:-1}"
    # Increase iteration limit for eval tasks (default 50 may not be enough for complex changes)
    export PRIORITY_AGENT_ENGINE_MAX_ITERATIONS="${PRIORITY_AGENT_ENGINE_MAX_ITERATIONS:-150}"

    # Step 7: Start real-time progress monitor
    local monitor_pid=""
    if [ "${EVAL_RUN_MONITOR:-1}" != "0" ]; then
        monitor_progress "$events_file" "$task_name" "${VERBOSE:-}" &
        monitor_pid=$!
    fi

    # Step 8: Run the eval task
    local exit_code=0
    local stderr_file="$ROOT_DIR/target/eval-reports/${task_name}-${timestamp}.stderr.log"
    if ./target/debug/priority-agent \
        --eval-run \
        --prompt-file "$prompt_file" \
        --output "$output_file" \
        --events "$events_file" \
        2>"$stderr_file"; then
        exit_code=0
    else
        exit_code=$?
    fi

    # Stop progress monitor
    if [ -n "$monitor_pid" ] && kill -0 "$monitor_pid" 2>/dev/null; then
        kill "$monitor_pid" 2>/dev/null
        wait "$monitor_pid" 2>/dev/null || true
    fi

    if [ $exit_code -eq 0 ]; then
        print_success "Task completed: $task_name"
    else
        print_error "Task failed or hit limit: $task_name (exit: $exit_code)"
        if [ -s "$stderr_file" ]; then
            echo "  Stderr preview:"
            head -5 "$stderr_file" | sed 's/^/    /'
        fi
    fi

    rm -f "$prompt_file"

    # Step 8: Show summary
    show_task_summary "$task_name" "$output_file" "$events_file"

    # Step 9: Check acceptance criteria
    local acceptance_exit=0
    if check_acceptance "$task_file" "$events_file" "$baseline_status_file"; then
        acceptance_exit=0
    else
        acceptance_exit=$?
    fi

    # Task passes only if both agent succeeded AND acceptance criteria passed
    if [ $exit_code -ne 0 ] || [ $acceptance_exit -ne 0 ]; then
        return 1
    fi
    return 0
}

# Check acceptance criteria from task definition
check_acceptance() {
    local task_file="$1"
    local events_file="$2"
    local baseline_status_file="$3"

    echo ""
    echo -e "${CYAN}═══ Acceptance Criteria ═══${NC}"

    ACCEPTANCE_TOTAL=0
    ACCEPTANCE_PASSED=0
    ACCEPTANCE_FAILED=0

    run_acceptance_commands "$task_file" "acceptance.required_commands" "Required"
    run_acceptance_commands "$task_file" "acceptance.harness_commands" "Harness"

    check_tool_constraints "$task_file" "$events_file"
    check_diff_constraints "$task_file" "$baseline_status_file"

    if [ "$ACCEPTANCE_TOTAL" -eq 0 ]; then
        echo "  No executable acceptance criteria defined"
        return 0
    fi

    echo ""
    echo "  Total: $ACCEPTANCE_TOTAL, Passed: $ACCEPTANCE_PASSED, Failed: $ACCEPTANCE_FAILED"

    if [ "$ACCEPTANCE_FAILED" -eq 0 ]; then
        print_success "All acceptance criteria passed"
        return 0
    fi

    print_error "Some acceptance criteria failed"
    return 1
}

run_acceptance_commands() {
    local task_file="$1"
    local field="$2"
    local label="$3"

    local commands
    commands=$(yaml_get "$task_file" "$field")

    if [ -z "$commands" ]; then
        echo "  No $label commands defined"
        return 0
    fi

    while IFS= read -r cmd; do
        [ -z "$cmd" ] && continue
        ACCEPTANCE_TOTAL=$((ACCEPTANCE_TOTAL + 1))
        # Run each command in a subshell so `cd` doesn't affect the parent shell
        if (eval "$cmd") >/dev/null 2>&1; then
            print_result "$label: $cmd" "PASS"
            ACCEPTANCE_PASSED=$((ACCEPTANCE_PASSED + 1))
        else
            print_result "$label: $cmd" "FAIL"
            ACCEPTANCE_FAILED=$((ACCEPTANCE_FAILED + 1))
        fi
    done <<< "$commands"
}

check_tool_constraints() {
    local task_file="$1"
    local events_file="$2"

    if ! yaml_list_has_items "$task_file" "allowed_tools" && ! yaml_list_has_items "$task_file" "forbidden_tools"; then
        echo "  No tool constraints defined"
        return 0
    fi

    ACCEPTANCE_TOTAL=$((ACCEPTANCE_TOTAL + 1))
    if python3 - "$task_file" "$events_file" <<'PY'
import json
import sys
import yaml

task_file, events_file = sys.argv[1], sys.argv[2]
with open(task_file) as f:
    task = yaml.safe_load(f) or {}

allowed = task.get("allowed_tools")
allowed_set = {str(tool).strip() for tool in allowed or [] if str(tool).strip()}
forbidden_set = {str(tool).strip() for tool in task.get("forbidden_tools") or [] if str(tool).strip()}
used = []

try:
    with open(events_file) as f:
        for line in f:
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("event") == "tool_execution_start":
                name = str(event.get("name") or "").strip()
                if name and name not in used:
                    used.append(name)
except FileNotFoundError:
    print("missing events file")
    sys.exit(1)

violations = []
for name in used:
    if name in forbidden_set:
        violations.append(f"forbidden tool used: {name}")
    if allowed is not None and name not in allowed_set:
        violations.append(f"tool outside allowed_tools used: {name}")

if violations:
    for violation in violations:
        print(violation)
    sys.exit(1)
PY
    then
        print_result "Tool constraints" "PASS"
        ACCEPTANCE_PASSED=$((ACCEPTANCE_PASSED + 1))
    else
        print_result "Tool constraints" "FAIL"
        ACCEPTANCE_FAILED=$((ACCEPTANCE_FAILED + 1))
    fi
}

check_diff_constraints() {
    local task_file="$1"
    local baseline_status_file="$2"

    local max_files
    max_files=$(yaml_get "$task_file" "acceptance.diff_constraints.max_files_changed")
    local forbidden_paths
    forbidden_paths=$(yaml_get "$task_file" "acceptance.diff_constraints.forbidden_paths")

    if [ -z "$max_files" ] && [ -z "$forbidden_paths" ]; then
        echo "  No diff constraints defined"
        return 0
    fi

    local current_status_file
    current_status_file=$(mktemp)
    capture_status_paths "$current_status_file"

    local changed_after_prepare
    changed_after_prepare=$(print_status_path_delta "$baseline_status_file" "$current_status_file")

    if [ -n "$max_files" ] && [[ "$max_files" =~ ^[0-9]+$ ]]; then
        local changed_count
        changed_count=$(printf "%s\n" "$changed_after_prepare" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')
        ACCEPTANCE_TOTAL=$((ACCEPTANCE_TOTAL + 1))
        if [ "$changed_count" -le "$max_files" ]; then
            print_result "Max changed paths since prepare <= $max_files" "PASS"
            ACCEPTANCE_PASSED=$((ACCEPTANCE_PASSED + 1))
        else
            print_result "Max changed paths since prepare <= $max_files (actual: $changed_count)" "FAIL"
            ACCEPTANCE_FAILED=$((ACCEPTANCE_FAILED + 1))
        fi
    fi

    if [ -n "$forbidden_paths" ]; then
        ACCEPTANCE_TOTAL=$((ACCEPTANCE_TOTAL + 1))
        local forbidden_hit=0
        while IFS= read -r forbidden; do
            [ -z "$forbidden" ] && continue
            while IFS= read -r changed_path; do
                [ -z "$changed_path" ] && continue
                case "$changed_path" in
                    "$forbidden"|"$forbidden"*) forbidden_hit=1 ;;
                esac
            done <<< "$changed_after_prepare"
        done <<< "$forbidden_paths"
        if [ "$forbidden_hit" -eq 0 ]; then
            print_result "Forbidden paths unchanged since prepare" "PASS"
            ACCEPTANCE_PASSED=$((ACCEPTANCE_PASSED + 1))
        else
            print_result "Forbidden paths unchanged since prepare" "FAIL"
            ACCEPTANCE_FAILED=$((ACCEPTANCE_FAILED + 1))
        fi
    fi

    rm -f "$current_status_file"
    return 0
}

# Run all tasks in a tier directory
run_tier() {
    local tier="$1"
    local tier_dir="$EVALSETS_DIR/$tier"

    if [ ! -d "$tier_dir" ]; then
        print_error "Tier directory not found: $tier_dir"
        return 1
    fi

    echo -e "${BLUE}═══ Running $tier ═══${NC}"

    local tasks
    tasks=$(find "$tier_dir" -name "*.yaml" -o -name "*.yml" | sort)

    if [ -z "$tasks" ]; then
        print_info "No tasks found in $tier"
        return 0
    fi

    local total=0
    local passed=0

    while IFS= read -r task_file; do
        [ -z "$task_file" ] && continue
        total=$((total + 1))
        if run_task_file "$task_file"; then
            passed=$((passed + 1))
        fi
    done <<< "$tasks"

    echo ""
    echo -e "${BLUE}═══ $tier Summary ═══${NC}"
    echo "  Total: $total"
    echo -e "  Passed: ${GREEN}$passed${NC}"
    echo -e "  Failed: ${RED}$((total - passed))${NC}"

    if [ "$passed" -eq "$total" ]; then
        print_success "All tasks in $tier passed"
        return 0
    else
        print_error "Some tasks in $tier failed"
        return 1
    fi
}

# List available tasks
list_tasks() {
    echo "Available eval tasks:"
    echo ""
    for tier_dir in "$EVALSETS_DIR"/tier-*; do
        [ -d "$tier_dir" ] || continue
        local tier_name
        tier_name=$(basename "$tier_dir")
        echo -e "${BLUE}$tier_name${NC}"
        find "$tier_dir" -name "*.yaml" -o -name "*.yml" | while read -r f; do
            local task_name
            task_name=$(basename "$f" .yaml)
            echo "  - $task_name"
        done
        echo ""
    done

    echo "Legacy live tasks:"
    find "$EVALSETS_DIR/live_tasks" -name "*.yaml" | while read -r f; do
        local task_name
        task_name=$(basename "$f" .yaml)
        echo "  - $task_name"
    done
}

show_help() {
    cat << 'EOF'
Priority Agent Eval Runner

Usage:
  ./scripts/eval-run.sh [OPTIONS] [COMMAND]

Commands:
  tier-1      Run tier-1 foundation tasks (alias: tier-1-foundations)
  tier-2      Run tier-2 single-file tasks (alias: tier-2-single-file)
  tier-3      Run tier-3 multi-file tasks (alias: tier-3-multi-file)
  tier-4      Run tier-4 integration tasks (alias: tier-4-integration)
  tier-5      Run tier-5 edge-case tasks (alias: tier-5-edge-cases)
  all         Run all tiers (takes ~1 hour)
  list        List all available tasks
  help        Show this help

Options:
  -v, --verbose    Show detailed tool arguments, results, and reasoning

Examples:
  ./scripts/eval-run.sh tier-1              # Quick sanity check (~5 min)
  ./scripts/eval-run.sh -v tier-4           # Verbose monitoring for tier-4
  ./scripts/eval-run.sh --verbose all       # Full suite with full detail

Environment:
  PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS=1  Auto-approve mutating tools.
                                         Defaults to 1 only for seeded_code_change tasks.
  PRIORITY_AGENT_AUTO_APPROVE=1          Auto-approve ask_user tool.
  PRIORITY_AGENT_ROUTE_SCOPED_TOOLS=0    Optional override; not set by this runner.
  EVAL_RUN_MONITOR=0                     Disable real-time progress monitor.

EOF
}

# ── Argument parsing ──
VERBOSE=""
COMMAND=""

for arg in "$@"; do
    case "$arg" in
        -v|--verbose)
            VERBOSE=1
            ;;
        -h|--help|help)
            show_help
            exit 0
            ;;
        tier-*|all|list)
            COMMAND="$arg"
            ;;
    esac
done

# Default to help if no command given
if [ -z "$COMMAND" ]; then
    show_help
    exit 0
fi

# Main
print_header

case "$COMMAND" in
    tier-1|tier-1-foundations)
        run_tier "tier-1-foundations"
        ;;
    tier-2|tier-2-single-file)
        run_tier "tier-2-single-file"
        ;;
    tier-3|tier-3-multi-file)
        run_tier "tier-3-multi-file"
        ;;
    tier-4|tier-4-integration)
        run_tier "tier-4-integration"
        ;;
    tier-5|tier-5-edge-cases)
        run_tier "tier-5-edge-cases"
        ;;
    all)
        failed=0
        run_tier "tier-1-foundations" || failed=1
        run_tier "tier-2-single-file" || failed=1
        run_tier "tier-3-multi-file" || failed=1
        run_tier "tier-4-integration" || failed=1
        run_tier "tier-5-edge-cases" || failed=1
        if [ "$failed" -eq 0 ]; then
            print_success "All tiers passed"
        else
            print_error "Some tiers failed"
            exit 1
        fi
        ;;
    list)
        list_tasks
        ;;
    help|--help|-h)
        show_help
        exit 0
        ;;
    *)
        echo "Unknown command: $1"
        show_help
        exit 1
        ;;
esac
