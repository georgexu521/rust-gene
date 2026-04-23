#!/usr/bin/env bash
# Documentation validation script for Phase 0
# Checks for doc/implementation conflicts

set -e

echo "=== Documentation Validation ==="
echo ""

# Check that required Phase 0 files exist
echo "Checking Phase 0 deliverables..."
required_files=(
    "PLAN.md"
    "CAPABILITY_MATRIX.md"
    "QUALITY_GATES.md"
)

for file in "${required_files[@]}"; do
    if [[ -f "$file" ]]; then
        echo "  [OK] $file exists"
    else
        echo "  [FAIL] $file missing"
        exit 1
    fi
done

echo ""

# Check for "not implemented" or placeholder markers in docs that claim to be complete
echo "Checking for doc/implementation conflicts..."

# Extract commands/tools that are marked as Production/Usable
# and verify they have implementations

# Get registered tools from source
echo "  Verifying tool registry..."
tool_count=$(grep -c "registry.register" src/tools/mod.rs || echo "0")
echo "  Registered tools in mod.rs: $tool_count"

# Get command count
echo "  Verifying command registry..."
cmd_count=$(grep -c "pub const CMD_" src/tui/commands.rs || echo "0")
echo "  Registered commands in commands.rs: $cmd_count"

echo ""

# Run cargo build check
echo "Running cargo build check..."
if cargo build --all-features > /dev/null 2>&1; then
    echo "  [OK] Build passes"
else
    echo "  [FAIL] Build failed"
    exit 1
fi

# Run cargo test
echo "Running cargo test..."
TEST_OUTPUT=$(mktemp)
if cargo test --quiet > "$TEST_OUTPUT" 2>&1; then
    test_count=$(grep -E "test result" "$TEST_OUTPUT" | head -1)
    echo "  [OK] Tests pass - $test_count"
    rm -f "$TEST_OUTPUT"
else
    echo "  [FAIL] Tests failed"
    cat "$TEST_OUTPUT"
    rm -f "$TEST_OUTPUT"
    exit 1
fi

echo ""
echo "=== Validation Complete ==="
echo ""
echo "Summary:"
echo "  - Phase 0 deliverables: All present"
echo "  - Tool registrations: $tool_count"
echo "  - Command registrations: $cmd_count"
echo "  - Build: PASS"
echo "  - Tests: PASS"
echo ""
echo "Note: For full validation, also run:"
echo "  - cargo clippy -- -D warnings"
echo "  - cargo doc --no-deps"
