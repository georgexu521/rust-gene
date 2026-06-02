#!/bin/bash
# Priority Agent TUI Dogfood Test Suite
#
# Runs the TUI agent against real projects and captures results.
# Requires MINIMAX_API_KEY or equivalent in the environment.
#
# Usage:
#   export MINIMAX_API_KEY=your-key
#   bash scripts/tui-dogfood-test.sh          # all tests
#   bash scripts/tui-dogfood-test.sh explore  # just exploration
#   bash scripts/tui-dogfood-test.sh edit     # just editing
#
# Output: /tmp/priority-agent-dogfood/<timestamp>/

set -euo pipefail

AGENT_BIN="${AGENT_BIN:-./target/debug/priority-agent}"
RESULTS_DIR="/tmp/priority-agent-dogfood/$(date +%Y%m%d-%H%M%S)"
PROMPTS_DIR="$RESULTS_DIR/prompts"
OUTPUTS_DIR="$RESULTS_DIR/outputs"

mkdir -p "$PROMPTS_DIR" "$OUTPUTS_DIR"

red()  { printf '\033[31m%s\033[0m\n' "$*"; }
green(){ printf '\033[32m%s\033[0m\n' "$*"; }
cyan() { printf '\033[36m%s\033[0m\n' "$*"; }
bold() { printf '\033[1m%s\033[0m\n' "$*"; }

# macOS compat: timeout / gtimeout fallback
if ! command -v timeout >/dev/null 2>&1 && command -v gtimeout >/dev/null 2>&1; then
  timeout() { gtimeout "$@"; }
elif ! command -v timeout >/dev/null 2>&1; then
  timeout() {
    local secs="$1"; shift
    perl -e 'alarm shift; exec @ARGV' "$secs" "$@"
  }
fi

# ═══════════════════════════════════════════════════════════
# Test prompts
# ═══════════════════════════════════════════════════════════

write_prompt() {
  local id="$1"; shift
  printf '%s\n' "$*" > "$PROMPTS_DIR/$id.txt"
}

# Exploration tests
write_prompt explore-src \
  "请查看 src/ 目录结构，告诉我下面有哪些子目录，每个子目录大概负责什么功能。只读，不要修改文件。"

write_prompt explore-engine \
  "请查看 src/engine/conversation_loop/ 目录，列出所有 .rs 文件，说明主要的控制器文件分别是做什么的。"

write_prompt explore-tools \
  "请查看 src/tools/ 目录，列出有哪些工具模块（子目录或文件）。只读。"

# Reading tests
write_prompt read-cargo \
  "请读取 priority-core/Cargo.toml 的 [package] 段，告诉我 package.version 和 package.edition 的值。只读，不要检查其他文件。"

write_prompt read-main \
  "请读取 src/main.rs，总结一下程序的入口逻辑。"

# Code understanding
write_prompt understand-loop \
  "这是只读理解任务，不要修改文件。请用 grep 或窄范围 file_read 查看 src/engine/conversation_loop/turn_iteration_controller.rs 里调用 StopChecker 的位置，以及 src/engine/stop_checker.rs 里 StopChecker::evaluate 的分支；最后用一段话概括它们的关系，并列出 stop checker 当前的主要停止条件。"

# Simple editing (requires --focus edit)
write_prompt simple-edit \
  "请在 src/engine/conversation_loop/force_summary.rs 的 ForceSummaryReason 枚举的 Stuck 变体注释上，把 'Iteration limit hit' 改成 'Iteration limit reached'。只改这一处，改完验证编译。"

# ═══════════════════════════════════════════════════════════
# Runner
# ═══════════════════════════════════════════════════════════

run_test() {
  local id="$1" expect="$2" timeout="${3:-120}"
  local prompt_file="$PROMPTS_DIR/$id.txt"
  local out_file="$OUTPUTS_DIR/$id.out"
  local err_file="$OUTPUTS_DIR/$id.err"

  cyan "▶ $id"
  echo "  prompt: $(head -1 "$prompt_file" | cut -c1-80)..."

  local start_ts=$(date +%s)
  set +e
  timeout "$timeout" "$AGENT_BIN" --eval-run \
    --prompt-file "$prompt_file" \
    --output "$out_file" \
    --events "$OUTPUTS_DIR/$id.events.jsonl" \
    2>"$err_file"
  local exit_code=$?
  set -e
  local elapsed=$(($(date +%s) - start_ts))

  local test_passed=true
  local notes=""

  if [ $exit_code -eq 124 ] || [ $exit_code -eq 142 ]; then
    test_passed=false
    notes="$notes [TIMEOUT]"
  elif [ $exit_code -ne 0 ]; then
    test_passed=false
    notes="$notes [EXIT:$exit_code]"
  fi

  if [ ! -s "$out_file" ]; then
    test_passed=false
    notes="$notes [EMPTY]"
  fi

  if [ -n "$expect" ] && ! grep -qi "$expect" "$out_file" 2>/dev/null; then
    test_passed=false
    notes="$notes [MISSING:$expect]"
  fi

  if $test_passed; then
    green "  ✅ PASS (${elapsed}s)"
  else
    red "  ❌ FAIL (${elapsed}s)$notes"
    echo "  --- output preview ---"
    tail -10 "$out_file" 2>/dev/null | sed 's/^/    /'
    echo "  --- stderr preview ---"
    tail -5 "$err_file" 2>/dev/null | sed 's/^/    /'
  fi
  echo ""
}

# ═══════════════════════════════════════════════════════════
# Main
# ═══════════════════════════════════════════════════════════

case "${1:-all}" in
  explore)
    bold "═══ Exploration Tests ═══"
    run_test explore-src "engine\|tools\|agent" 120
    run_test explore-engine "controller\|mod.rs" 120
    run_test explore-tools "file_tool\|bash_tool\|agent_tool" 120
    ;;
  read)
    bold "═══ Reading Tests ═══"
    run_test read-cargo "version\|edition" 120
    run_test read-main "main\|cli\|tui" 120
    ;;
  understand)
    bold "═══ Understanding Tests ═══"
    run_test understand-loop "UserInterrupted\|BudgetExhausted\|VerificationReady\|NoIssue" 240
    ;;
  edit)
    bold "═══ Editing Tests ═══"
    run_test simple-edit "Stuck\|reached" 180
    ;;
  all)
    bold "═══ Full Test Suite ═══"
    run_test explore-src "engine\|tools\|agent" 120
    run_test explore-engine "controller\|mod.rs" 120
    run_test read-cargo "version\|edition" 120
    run_test read-main "main\|cli\|tui" 120
    run_test understand-loop "UserInterrupted\|BudgetExhausted\|VerificationReady\|NoIssue" 240
    ;;
  *)
    echo "Usage: bash scripts/tui-dogfood-test.sh [explore|read|understand|edit|all]"
    exit 1
    ;;
esac

echo "═══ Done ═══"
echo "prompts: $PROMPTS_DIR"
echo "outputs: $OUTPUTS_DIR"
