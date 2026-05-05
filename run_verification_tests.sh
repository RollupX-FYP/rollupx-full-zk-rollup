#!/bin/bash
# Test Execution Script for Executor & Prover Components
# Runs comprehensive test suite to verify correctness before factorial experiment

set -e

echo "=========================================="
echo "RollupX Executor & Prover Test Suite"
echo "=========================================="
echo

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

EXECUTOR_ROOT="${EXECUTOR_ROOT:-.}"
PROVER_ROOT="${PROVER_ROOT:-.}"

# Track test results
PASSED=0
FAILED=0

run_test_suite() {
    local name=$1
    local path=$2
    local manifest=$3
    local package=$4
    
    echo -e "${YELLOW}▶ Running $name...${NC}"
    
    if [[ "$name" == *"executor"* ]]; then
        # Executor-specific environment variables
        export ALLOW_UNSIGNED_USER_TXS=1
        export EXECUTOR_BUILD_ID="test-build"
        
        if cargo +nightly-2025-03-19 test \
            --manifest-path "$manifest" \
            -p "$package" \
            --ignore-rust-version \
            --test-threads=1 \
            -- --nocapture --test-threads=1; then
            echo -e "${GREEN}✓ $name PASSED${NC}"
            ((PASSED++))
        else
            echo -e "${RED}✗ $name FAILED${NC}"
            ((FAILED++))
        fi
    else
        # Prover tests
        if cargo +nightly test \
            --manifest-path "$manifest" \
            -p "$package" \
            --test-threads=1 \
            -- --nocapture --test-threads=1; then
            echo -e "${GREEN}✓ $name PASSED${NC}"
            ((PASSED++))
        else
            echo -e "${RED}✗ $name FAILED${NC}"
            ((FAILED++))
        fi
    fi
    echo
}

# Run Executor Tests
echo "╔════════════════════════════════════════╗"
echo "║        EXECUTOR COMPONENT TESTS        ║"
echo "╚════════════════════════════════════════╝"
echo

cd "$EXECUTOR_ROOT"

run_test_suite \
    "executor_tx_engine_tests" \
    "executor/src/Cargo.toml" \
    "executor/src/Cargo.toml" \
    "zksync_state_machine"

run_test_suite \
    "executor_state_tests" \
    "executor/src/Cargo.toml" \
    "executor/src/Cargo.toml" \
    "zksync_state_machine"

run_test_suite \
    "executor_trace_tests" \
    "executor/src/Cargo.toml" \
    "executor/src/Cargo.toml" \
    "zksync_state_machine"

run_test_suite \
    "executor_integration_tests" \
    "executor/src/Cargo.toml" \
    "executor/src/Cargo.toml" \
    "zksync_state_machine"

# Run Prover Tests
echo "╔════════════════════════════════════════╗"
echo "║        PROVER COMPONENT TESTS          ║"
echo "╚════════════════════════════════════════╝"
echo

cd "$PROVER_ROOT"

run_test_suite \
    "risc0_guest_logic_tests" \
    "risc0_prover/rollup_core/Cargo.toml" \
    "risc0_prover/rollup_core/Cargo.toml" \
    "rollup_core"

# Summary
echo "╔════════════════════════════════════════╗"
echo "║            TEST SUMMARY                ║"
echo "╚════════════════════════════════════════╝"
echo
echo -e "${GREEN}✓ Passed: $PASSED${NC}"
echo -e "${RED}✗ Failed: $FAILED${NC}"
echo

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
