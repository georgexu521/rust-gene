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

# Extract field from YAML task file using Python
yaml_get() {
    local task_file="$1"
    local field="$2"
    python3 -c "
import yaml
with open('$task_file') as f:
    data = yaml.safe_load(f)
    # Support nested paths like 'repo.prepare_commands'
    keys = '$field'.split('.')
    value = data
    for key in keys:
        if value is None:
            break
        value = value.get(key) if isinstance(value, dict) else None
    
    if isinstance(value, list):
        for item in value:
            print(item)
    elif isinstance(value, str):
        print(value)
    " 2>/dev/null
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
            echo -e "  ${YELLOW}⚠${NC} prepare command $count may have partial failure (continuing)"
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

# Run a single eval task YAML file
run_task_file() {
    local task_file="$1"
    local task_name
    task_name=$(basename "$task_file" .yaml)
    
    echo ""
    echo -e "${BLUE}═══ Task: $task_name ═══${NC}"
    
    # Step 1: Prepare fixtures
    prepare_task "$task_file"
    
    # Step 2: Extract prompt from YAML
    local prompt
    prompt=$(yaml_get "$task_file" "prompt")
    
    if [ -z "$prompt" ]; then
        print_error "Could not extract prompt from $task_file"
        return 1
    fi
    
    # Step 3: Check if eval-run mode is available
    if [ ! -f "./target/debug/priority-agent" ]; then
        print_info "Building priority-agent..."
        if ! cargo build -q 2>/dev/null; then
            print_error "Failed to build priority-agent"
            return 1
        fi
    fi
    
    # Step 4: Write prompt to temp file
    local prompt_file
    prompt_file=$(mktemp)
    echo "$prompt" > "$prompt_file"
    
    # Step 5: Setup output paths
    local timestamp
    timestamp=$(date +%Y%m%d-%H%M%S)
    local output_file="$ROOT_DIR/target/eval-reports/${task_name}-${timestamp}.md"
    local events_file="$ROOT_DIR/target/eval-reports/${task_name}-${timestamp}.jsonl"
    mkdir -p "$ROOT_DIR/target/eval-reports"
    
    print_info "Running eval task..."
    
    # Step 6: Set eval environment
    export PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS=1
    export PRIORITY_AGENT_AUTO_APPROVE=1
    
    # Step 7: Run the eval task
    local exit_code=0
    if ./target/debug/priority-agent \
        --eval-run \
        --prompt-file "$prompt_file" \
        --output "$output_file" \
        --events "$events_file" \
        2>/dev/null; then
        print_success "Task completed: $task_name"
    else
        print_error "Task failed or hit limit: $task_name"
        exit_code=1
    fi
    
    rm -f "$prompt_file"
    
    # Step 8: Show summary
    show_task_summary "$task_name" "$output_file" "$events_file"
    
    # Step 9: Check acceptance criteria
    check_acceptance "$task_file"
    
    return $exit_code
}

# Check acceptance criteria from task definition
check_acceptance() {
    local task_file="$1"
    
    echo ""
    echo -e "${CYAN}═══ Acceptance Criteria ═══${NC}"
    
    local commands
    commands=$(yaml_get "$task_file" "acceptance.required_commands")
    
    if [ -z "$commands" ]; then
        echo "  No required commands defined"
        return 0
    fi
    
    local total=0
    local passed=0
    
    while IFS= read -r cmd; do
        [ -z "$cmd" ] && continue
        total=$((total + 1))
        if eval "$cmd" >/dev/null 2>&1; then
            print_result "Required: $cmd" "PASS"
            passed=$((passed + 1))
        else
            print_result "Required: $cmd" "FAIL"
        fi
    done <<< "$commands"
    
    echo ""
    echo "  Total: $total, Passed: $passed, Failed: $((total - passed))"
    
    if [ "$passed" -eq "$total" ] && [ "$total" -gt 0 ]; then
        print_success "All acceptance criteria passed"
        return 0
    elif [ "$total" -gt 0 ]; then
        print_error "Some acceptance criteria failed"
        return 1
    fi
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
  ./scripts/eval-run.sh [COMMAND] [OPTIONS]

Commands:
  tier-1      Run tier-1 foundation tasks (tool health)
  tier-2      Run tier-2 single-file tasks
  tier-3      Run tier-3 multi-file tasks
  tier-4      Run tier-4 integration tasks
  tier-5      Run tier-5 edge-case tasks
  all         Run all tiers (takes ~1 hour)
  list        List all available tasks
  help        Show this help

Examples:
  ./scripts/eval-run.sh tier-1          # Quick sanity check (~5 min)
  ./scripts/eval-run.sh tier-2          # Single-file changes (~10 min)
  ./scripts/eval-run.sh all             # Full suite (~1 hour)

Environment:
  PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS=1  Auto-approve mutating tools
  PRIORITY_AGENT_AUTO_APPROVE=1          Auto-approve ask_user tool

EOF
}

# Main
print_header

case "${1:-help}" in
    tier-1)
        run_tier "tier-1-foundations"
        ;;
    tier-2)
        run_tier "tier-2-single-file"
        ;;
    tier-3)
        run_tier "tier-3-multi-file"
        ;;
    tier-4)
        run_tier "tier-4-integration"
        ;;
    tier-5)
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
