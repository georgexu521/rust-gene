#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

RUNS=3
PORT=18787
LABEL="baseline"
OUTPUT=""
COMPARE_REPORT=""
SKIP_BUILD=0
ENABLE_LONG_CHAT=0
LONG_CHAT_TURNS=100

usage() {
  cat <<'EOF'
Usage:
  scripts/benchmark.sh [options]

Options:
  --runs N                 Number of repeated runs for each metric (default: 3)
  --port PORT              API server port for benchmark calls (default: 18787)
  --label NAME             Label to include in report filename (default: baseline)
  --output FILE            Output markdown file path
  --compare REPORT.md      Compare current metrics with previous markdown report
  --skip-build             Skip cargo build step
  --enable-long-chat       Run 100-turn chat benchmark (may consume API quota)
  --long-chat-turns N      Turn count for long chat benchmark (default: 100)
  -h, --help               Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs) RUNS="${2:-}"; shift 2 ;;
    --port) PORT="${2:-}"; shift 2 ;;
    --label) LABEL="${2:-}"; shift 2 ;;
    --output) OUTPUT="${2:-}"; shift 2 ;;
    --compare) COMPARE_REPORT="${2:-}"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --enable-long-chat) ENABLE_LONG_CHAT=1; shift ;;
    --long-chat-turns) LONG_CHAT_TURNS="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "${OUTPUT}" ]]; then
  mkdir -p docs/benchmarks
  ts="$(date +%Y%m%d-%H%M%S)"
  OUTPUT="docs/benchmarks/report-${LABEL}-${ts}.md"
fi

if ! [[ "$RUNS" =~ ^[0-9]+$ ]] || [[ "$RUNS" -lt 1 ]]; then
  echo "--runs must be a positive integer" >&2
  exit 1
fi

if ! [[ "$LONG_CHAT_TURNS" =~ ^[0-9]+$ ]] || [[ "$LONG_CHAT_TURNS" -lt 1 ]]; then
  echo "--long-chat-turns must be a positive integer" >&2
  exit 1
fi

BIN="target/release/priority-agent"
SERVER_PID=""
SERVER_LOG=""

now_ms() {
  perl -MTime::HiRes=time -e 'printf("%.0f\n", time()*1000)'
}

avg_ms() {
  awk '{s+=$1} END { if (NR==0) { print "0.0" } else { printf("%.1f", s/NR) } }'
}

p95_from_values() {
  local values="$1"
  local total
  total="$(printf '%s\n' "$values" | awk 'NF>0 {c++} END {print c+0}')"
  if [[ -z "$total" || "$total" -eq 0 ]]; then
    echo "0"
    return
  fi
  local idx=$(( (total * 95 + 99) / 100 ))
  if [[ "$idx" -lt 1 ]]; then idx=1; fi
  printf '%s\n' "$values" | sort -n | sed -n "${idx}p"
}

measure_command_ms() {
  local command="$1"
  local out=()
  local i
  for ((i=1; i<=RUNS; i++)); do
    local start end
    start="$(now_ms)"
    if eval "$command" >/dev/null 2>&1; then
      end="$(now_ms)"
      out+=("$((end - start))")
    else
      return 1
    fi
  done
  printf '%s\n' "${out[@]}"
}

cleanup() {
  if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" >/dev/null 2>&1; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if [[ "$SKIP_BUILD" -eq 0 ]]; then
  cargo build --release --features experimental-api-server >/dev/null
fi

if [[ ! -x "$BIN" ]]; then
  echo "Binary not found: $BIN" >&2
  exit 1
fi

has_llm_key=0
if [[ -n "${MINIMAX_API_KEY:-}" || -n "${OPENAI_API_KEY:-}" || -n "${MOONSHOT_API_KEY:-}" ]]; then
  has_llm_key=1
fi

startup_runs=""
startup_status="ok"
startup_note=""
if startup_runs="$(measure_command_ms "env -u OPENAI_API_KEY -u MOONSHOT_API_KEY -u MINIMAX_API_KEY \"$BIN\" --help")"; then
  :
else
  startup_status="error"
  startup_note="help path failed"
fi

first_token_status="skipped"
first_token_runs=""
first_token_note="requires API server + LLM key"
tool_call_status="skipped"
tool_call_runs=""
tool_call_note="requires API server + LLM key"
long_chat_status="skipped"
long_chat_runs=""
long_chat_note="disabled by default (use --enable-long-chat)"

if [[ "$has_llm_key" -eq 1 ]]; then
  SERVER_LOG="$(mktemp -t priority-agent-bench-api.XXXXXX.log)"
  "$BIN" --api --port "$PORT" >"$SERVER_LOG" 2>&1 &
  SERVER_PID="$!"

  ready=0
  for _ in {1..60}; do
    if curl -fsS "http://127.0.0.1:${PORT}/api/health" >/dev/null 2>&1; then
      ready=1
      break
    fi
    sleep 0.25
  done

  if [[ "$ready" -eq 1 ]]; then
    first_token_note="POST /api/chat"
    tool_call_note="POST /api/tools/call (calculate)"

    measure_chat_once() {
      curl -fsS -X POST "http://127.0.0.1:${PORT}/api/chat" \
        -H "Content-Type: application/json" \
        -d '{"message":"Reply with exactly OK.","stream":false}' >/dev/null
    }

    measure_tool_once() {
      curl -fsS -X POST "http://127.0.0.1:${PORT}/api/tools/call" \
        -H "Content-Type: application/json" \
        -d '{"tool":"calculate","params":{"expression":"2+2"}}' >/dev/null
    }

    if first_token_runs="$(measure_command_ms "measure_chat_once")"; then
      first_token_status="ok"
    else
      first_token_status="error"
      first_token_note="chat endpoint call failed"
    fi

    if tool_call_runs="$(measure_command_ms "measure_tool_once")"; then
      tool_call_status="ok"
    else
      tool_call_status="error"
      tool_call_note="tool call endpoint failed"
    fi

    if [[ "$ENABLE_LONG_CHAT" -eq 1 ]]; then
      long_chat_note="${LONG_CHAT_TURNS} turns over /api/chat in one session"
      long_chat_status="ok"
      long_chat_tmp="$(mktemp -t priority-agent-bench-longchat.XXXXXX.txt)"
      : >"$long_chat_tmp"
      for ((i=1; i<=RUNS; i++)); do
        session_id="bench-${LABEL}-${i}-$(date +%s)"
        start="$(now_ms)"
        ok=1
        for ((turn=1; turn<=LONG_CHAT_TURNS; turn++)); do
          prompt="Turn ${turn}. Reply with: ACK ${turn}."
          if ! curl -fsS -X POST "http://127.0.0.1:${PORT}/api/chat" \
            -H "Content-Type: application/json" \
            -d "{\"message\":\"${prompt}\",\"session_id\":\"${session_id}\",\"stream\":false}" \
            >/dev/null; then
            ok=0
            break
          fi
        done
        end="$(now_ms)"
        if [[ "$ok" -eq 1 ]]; then
          echo "$((end - start))" >>"$long_chat_tmp"
        else
          long_chat_status="error"
          long_chat_note="failed during turn loop"
          break
        fi
      done
      if [[ "$long_chat_status" == "ok" ]]; then
        long_chat_runs="$(cat "$long_chat_tmp")"
      fi
      rm -f "$long_chat_tmp"
    fi
  else
    first_token_status="error"
    first_token_note="API server did not become healthy"
    tool_call_status="error"
    tool_call_note="API server did not become healthy"
    if [[ "$ENABLE_LONG_CHAT" -eq 1 ]]; then
      long_chat_status="error"
      long_chat_note="API server did not become healthy"
    fi
  fi
fi

metric_line() {
  local name="$1" status="$2" values="$3" note="$4"
  local avg="N/A" p95="N/A" raw="N/A"
  if [[ "$status" == "ok" ]]; then
    avg="$(printf '%s\n' "$values" | avg_ms)"
    p95="$(p95_from_values "$values")"
    raw="$(printf '%s\n' "$values" | tr '\n' ',' | sed 's/,$//')"
  fi
  printf '| %s | %s | %s | %s | %s |\n' "$name" "$status" "$avg" "$p95" "$note"
  printf '| %s raw runs | - | - | - | `%s` |\n' "$name" "$raw"
}

extract_metric_avg() {
  local report="$1"
  local metric="$2"
  awk -F'|' -v m="$metric" '
    $0 ~ /^\|/ {
      gsub(/^ +| +$/, "", $2)
      if ($2 == m) {
        gsub(/^ +| +$/, "", $4)
        print $4
        exit
      }
    }
  ' "$report"
}

compare_block=""
if [[ -n "$COMPARE_REPORT" ]] && [[ -f "$COMPARE_REPORT" ]]; then
  compare_block=$(
    {
      echo ""
      echo "## Comparison vs ${COMPARE_REPORT}"
      echo ""
      echo "| metric | previous avg_ms | current avg_ms | delta_ms |"
      echo "|---|---:|---:|---:|"
      for metric in startup_help first_token tool_call long_chat; do
        prev="$(extract_metric_avg "$COMPARE_REPORT" "$metric")"
        if [[ "$metric" == "startup_help" ]]; then
          cur_status="$startup_status"
          cur_vals="$startup_runs"
        elif [[ "$metric" == "first_token" ]]; then
          cur_status="$first_token_status"
          cur_vals="$first_token_runs"
        elif [[ "$metric" == "tool_call" ]]; then
          cur_status="$tool_call_status"
          cur_vals="$tool_call_runs"
        else
          cur_status="$long_chat_status"
          cur_vals="$long_chat_runs"
        fi
        if [[ "$cur_status" == "ok" ]]; then
          cur="$(printf '%s\n' "$cur_vals" | avg_ms)"
        else
          cur="N/A"
        fi
        if [[ -z "$prev" ]]; then
          prev="N/A"
        fi
        if [[ "$prev" != "N/A" && "$cur" != "N/A" ]]; then
          delta="$(awk -v a="$cur" -v b="$prev" 'BEGIN{printf("%.1f", a-b)}')"
        else
          delta="N/A"
        fi
        echo "| ${metric} | ${prev} | ${cur} | ${delta} |"
      done
    }
  )
fi

{
  echo "# Priority Agent Benchmark Report"
  echo ""
  echo "- label: \`${LABEL}\`"
  echo "- generated_at: \`$(date '+%Y-%m-%d %H:%M:%S %z')\`"
  echo "- runs_per_metric: \`${RUNS}\`"
  echo "- binary: \`${BIN}\`"
  echo "- api_port: \`${PORT}\`"
  echo "- llm_key_detected: \`${has_llm_key}\`"
  echo ""
  echo "## Metrics"
  echo ""
  echo "| metric | status | avg_ms | p95_ms | notes |"
  echo "|---|---|---:|---:|---|"
  metric_line "startup_help" "$startup_status" "$startup_runs" "$startup_note"
  metric_line "first_token" "$first_token_status" "$first_token_runs" "$first_token_note"
  metric_line "tool_call" "$tool_call_status" "$tool_call_runs" "$tool_call_note"
  metric_line "long_chat" "$long_chat_status" "$long_chat_runs" "$long_chat_note"
  echo ""
  echo "## Environment"
  echo ""
  echo "\`\`\`text"
  echo "uname: $(uname -a)"
  echo "rustc: $(rustc --version 2>/dev/null || echo 'N/A')"
  echo "cargo: $(cargo --version 2>/dev/null || echo 'N/A')"
  echo "\`\`\`"
  if [[ -n "$SERVER_LOG" && -f "$SERVER_LOG" ]]; then
    echo ""
    echo "## API Server Log Tail"
    echo ""
    echo "\`\`\`text"
    tail -n 40 "$SERVER_LOG" || true
    echo "\`\`\`"
  fi
  if [[ -n "$compare_block" ]]; then
    echo "$compare_block"
  fi
} >"$OUTPUT"

echo "Benchmark report written to: $OUTPUT"
