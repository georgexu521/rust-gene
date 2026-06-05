#!/usr/bin/env bash
# Classify live_tasks into tiers based on task characteristics

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "Classifying live_tasks into tiers..."
echo ""

# Define classification rules
# Format: pattern|tier|reason
classify_task() {
    local filename="$1"
    local basename=$(basename "$filename" .yaml)
    
    # Check if task has tier field already
    local existing_tier
    existing_tier=$(python3 -c "
import yaml
with open('$filename') as f:
    data = yaml.safe_load(f)
    print(data.get('tier', ''))
" 2>/dev/null)
    
    if [ -n "$existing_tier" ]; then
        echo "$existing_tier"
        return
    fi
    
    # Classification logic based on task name patterns
    case "$basename" in
        # Tier 1: Tool verification and basic capabilities
        minimum-agent-direct-answer|minimum-agent-light-inspection|\
        core-inspection-grounding|core-provider-roundtrip|core-terminal-install-run)
            echo "tier-1-foundations"
            ;;
        
        # Tier 2: Single file modifications
        core-simple-stale-edit|core-multi-file-edit|cli-scrollback-polish|\
        desktop-ui-smoke-polish)
            echo "tier-2-single-file"
            ;;
        
        # Tier 3: Multi-file coordination
        backend-todo-api-crud|frontend-book-notes-localstorage|\
        core-rust-multi-file-refactor|code-change-verification-repair-loop|\
        live-eval-dashboard-summary|persistent-memory-planning-context)
            echo "tier-3-multi-file"
            ;;
        
        # Tier 5: Edge cases and boundaries (most runtime/memory/permission tasks)
        core-permission-rejection-recovery|core-rollback-product-path|\
        core-long-output-artifact|permission-default-open-dangerous-guard|\
        memory-*|minimum-agent-*|runtime-spine-p0b-*|project-partner-*|\
        skill-promotion-gate|resume-session-picker)
            echo "tier-5-edge-cases"
            ;;
        
        # Default: Check complexity field
        *)
            local complexity
            complexity=$(python3 -c "
import yaml
with open('$filename') as f:
    data = yaml.safe_load(f)
    print(data.get('complexity', 'medium'))
" 2>/dev/null)
            
            case "$complexity" in
                low)
                    echo "tier-2-single-file"
                    ;;
                medium)
                    echo "tier-3-multi-file"
                    ;;
                high)
                    echo "tier-5-edge-cases"
                    ;;
                *)
                    echo "tier-3-multi-file"
                    ;;
            esac
            ;;
    esac
}

# Process each task
total=0
moved=0
skipped=0

for task_file in "$ROOT_DIR"/evalsets/live_tasks/*.yaml; do
    [ -f "$task_file" ] || continue
    total=$((total + 1))
    
    basename=$(basename "$task_file")
    tier=$(classify_task "$task_file")
    target_dir="$ROOT_DIR/evalsets/$tier"
    
    # Check if already in correct tier
    if [ -f "$target_dir/$basename" ]; then
        echo "  SKIP: $basename (already in $tier)"
        skipped=$((skipped + 1))
        continue
    fi
    
    # Create tier directory if needed
    mkdir -p "$target_dir"
    
    # Copy task file
    cp "$task_file" "$target_dir/$basename"
    
    # Add tier field to YAML if not present
    python3 << EOF
import yaml

with open('$target_dir/$basename') as f:
    data = yaml.safe_load(f)

# Add tier field
data['tier'] = '$tier'

with open('$target_dir/$basename', 'w') as f:
    yaml.dump(data, f, default_flow_style=False, allow_unicode=True, sort_keys=False)

EOF
    
    echo "  MOVE: $basename -> $tier"
    moved=$((moved + 1))
done

echo ""
echo "Classification complete:"
echo "  Total tasks: $total"
echo "  Moved: $moved"
echo "  Skipped: $skipped"
echo ""

# Show tier summaries
echo "Tier summaries:"
for tier_dir in "$ROOT_DIR"/evalsets/tier-*; do
    [ -d "$tier_dir" ] || continue
    tier_name=$(basename "$tier_dir")
    count=$(ls "$tier_dir"/*.yaml 2>/dev/null | wc -l)
    echo "  $tier_name: $count tasks"
done
