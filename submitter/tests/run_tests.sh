#!/bin/bash
# Comprehensive Submitter Test Suite Runner
# Runs all test suites with appropriate configurations

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=================================="
echo "Submitter Component Test Suite"
echo "=================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test categories
echo -e "${BLUE}Available test suites:${NC}"
echo "  1. Quick unit tests (no --ignore)"
echo "  2. DA strategy tests (--ignored)"
echo "  3. L1 testnet tests (--ignored, requires env vars)"
echo "  4. Batch lifecycle tests (no --ignore)"
echo "  5. Resilience tests (no --ignore)"
echo "  6. E2E flow tests (--ignored)"
echo "  7. Config & performance tests (no --ignore)"
echo "  8. Run all tests"
echo ""

# Default: run quick tests
RUN_SUITE="${1:-0}"

run_quick_tests() {
    echo -e "${GREEN}Running quick unit tests...${NC}"
    cd "$PROJECT_ROOT/submitter"
    
    cargo test --test test_batch_lifecycle -- --nocapture
    cargo test --test test_resilience -- --nocapture
    cargo test --test test_config_performance -- --nocapture
}

run_da_strategy_tests() {
    echo -e "${GREEN}Running DA strategy tests...${NC}"
    cd "$PROJECT_ROOT/submitter"
    
    cargo test test_calldata_strategy -- --ignored --nocapture
    cargo test test_blob_strategy -- --ignored --nocapture
    cargo test test_offchain_strategy -- --ignored --nocapture
    cargo test test_compression -- --ignored --nocapture
}

run_l1_testnet_tests() {
    echo -e "${GREEN}Running L1 testnet tests...${NC}"
    
    # Check for required environment variables
    if [ -z "$TESTNET_RPC_URL" ]; then
        echo -e "${YELLOW}⚠ TESTNET_RPC_URL not set. Skipping L1 testnet tests.${NC}"
        echo "  Set env vars to run:"
        echo "    export TESTNET_RPC_URL=https://sepolia.infura.io/v3/YOUR_KEY"
        echo "    export TESTNET_PRIVATE_KEY=0x..."
        echo "    export BRIDGE_CONTRACT_ADDRESS=0x..."
        return
    fi
    
    cd "$PROJECT_ROOT/submitter"
    
    cargo test test_l1_testnet_bridge_deployment -- --ignored --nocapture
    cargo test test_l1_testnet_batch_submission -- --ignored --nocapture
    cargo test test_l1_state_root_reading -- --ignored --nocapture
    cargo test test_l1_gas_price_estimation -- --ignored --nocapture
}

run_batch_lifecycle_tests() {
    echo -e "${GREEN}Running batch lifecycle tests...${NC}"
    cd "$PROJECT_ROOT/submitter"
    
    cargo test test_batch_status_transitions -- --nocapture
    cargo test test_batch_retry -- --nocapture
    cargo test test_batch_expiration -- --nocapture
}

run_resilience_tests() {
    echo -e "${GREEN}Running resilience tests...${NC}"
    cd "$PROJECT_ROOT/submitter"
    
    cargo test test_submission_retry -- --nocapture
    cargo test test_circuit_breaker -- --nocapture
    cargo test test_exponential_backoff -- --nocapture
    cargo test test_timeout_protection -- --nocapture
}

run_e2e_tests() {
    echo -e "${GREEN}Running E2E flow tests...${NC}"
    
    if [ -z "$TESTNET_RPC_URL" ]; then
        echo -e "${YELLOW}⚠ E2E tests require L1 testnet setup. Using mocked flows.${NC}"
    fi
    
    cd "$PROJECT_ROOT/submitter"
    
    cargo test test_end_to_end_calldata_submission -- --ignored --nocapture
    cargo test test_end_to_end_blob_submission -- --ignored --nocapture
    cargo test test_end_to_end_multiple_batches_concurrent -- --ignored --nocapture
}

run_config_tests() {
    echo -e "${GREEN}Running configuration & performance tests...${NC}"
    cd "$PROJECT_ROOT/submitter"
    
    cargo test test_config_validation -- --nocapture
    cargo test test_performance -- --nocapture
}

run_all_tests() {
    echo -e "${GREEN}Running all test suites...${NC}"
    cd "$PROJECT_ROOT/submitter"
    
    echo -e "${BLUE}[1/4] Quick tests...${NC}"
    run_quick_tests
    
    echo ""
    echo -e "${BLUE}[2/4] Batch lifecycle...${NC}"
    run_batch_lifecycle_tests
    
    echo ""
    echo -e "${BLUE}[3/4] DA strategies...${NC}"
    run_da_strategy_tests
    
    echo ""
    echo -e "${BLUE}[4/4] Resilience...${NC}"
    run_resilience_tests
    
    if [ -n "$TESTNET_RPC_URL" ]; then
        echo ""
        echo -e "${BLUE}[5/4] L1 Testnet (optional)...${NC}"
        run_l1_testnet_tests
    fi
}

# Run selected suite
case $RUN_SUITE in
    1)
        run_quick_tests
        ;;
    2)
        run_da_strategy_tests
        ;;
    3)
        run_l1_testnet_tests
        ;;
    4)
        run_batch_lifecycle_tests
        ;;
    5)
        run_resilience_tests
        ;;
    6)
        run_e2e_tests
        ;;
    7)
        run_config_tests
        ;;
    8)
        run_all_tests
        ;;
    *)
        run_quick_tests
        ;;
esac

echo ""
echo -e "${GREEN}✓ Test suite complete${NC}"
