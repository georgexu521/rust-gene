#!/usr/bin/env bash
# Documentation validation script
# Checks current documentation anchors, registry smoke counts, file-size policy,
# advisory rustdoc coverage, compile wiring, and workflow-enabled tests.

set -e

echo "=== Documentation Validation ==="
echo ""

# Check that required status/planning files exist
echo "Checking required documentation..."
required_files=(
    "README.md"
    "QUICKSTART.md"
    "PLAN.md"
    "CAPABILITY_MATRIX.md"
    "QUALITY_GATES.md"
    "docs/PROJECT_STATUS.md"
    "docs/PROJECT_MAP.md"
    "docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md"
    "docs/RELEASE_STRUCTURE_CLEANUP_RECOMMENDATIONS_2026-06-22.md"
    "docs/REMAINING_STRUCTURE_REFINEMENT_PLAN_2026-06-22.md"
    "docs/CODE_DOCUMENTATION_PLAN_2026-06-22.md"
    "docs/NEXT_PRIORITY_CORE_WEIGHT_REFINEMENT_PLAN_2026-06-24.md"
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

echo "Checking registry smoke counts and source policy..."

# Get registered tools from source
echo "  Verifying tool registry..."
tool_count=$(grep -c "registry.register" src/tools/registry.rs || true)
echo "  Registered tools in registry.rs: $tool_count"

# Get command count
echo "  Verifying command registry..."
cmd_count=$(grep -c "CommandDef::new" src/tui/commands/catalog.rs || true)
echo "  Registered commands in commands.rs: $cmd_count"

echo "  Verifying source file line ceiling..."
bash scripts/check_source_file_sizes.sh

echo "  Running advisory rustdoc audit..."
python3 scripts/audit_rust_docs.py --limit 0

echo ""

# Run cargo check using the same broad target/feature wiring as CI.
echo "Running cargo check..."
if cargo check --workspace --all-targets --all-features > /dev/null 2>&1; then
    echo "  [OK] Check passes"
else
    echo "  [FAIL] Check failed"
    exit 1
fi

# Run cargo test using the current workflow-enabled baseline.
echo "Running cargo test..."
TEST_OUTPUT=$(mktemp)
if env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1 > "$TEST_OUTPUT" 2>&1; then
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
echo "  - Required docs: All present"
echo "  - Tool registrations: $tool_count"
echo "  - Command registrations: $cmd_count"
echo "  - Check: PASS"
echo "  - Tests: PASS"
echo ""
echo "Note: For full validation, also run:"
echo "  - cargo clippy --workspace --all-targets --all-features -- -D warnings"
echo "  - cargo doc --workspace --all-features --no-deps"
