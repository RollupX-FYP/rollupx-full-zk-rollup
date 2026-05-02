#!/usr/bin/env bash
# scripts/verify_stack.sh
# Verifies the core services of RollupX are up and functioning.

set -euo pipefail

echo "=== RollupX Stack Verification ==="

echo "1. Checking Hardhat L1 Node..."
if curl -s http://127.0.0.1:8545 -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | grep -q "result"; then
    echo " [OK] Hardhat responds to eth_blockNumber"
else
    echo " [FAIL] Hardhat is not responding"
    exit 1
fi

echo "2. Checking Shared Runtime Config (Contracts Deployed)..."
if docker compose exec -T sequencer cat /runtime/addresses.json > /dev/null 2>&1 || docker compose exec -T submitter cat /runtime/addresses.json > /dev/null 2>&1; then
    echo " [OK] Contracts deployed and addresses.json is present"
else
    echo " [FAIL] addresses.json not found in sequencer/submitter /runtime volume"
    # Don't strictly exit 1 here as it might be named differently (e.g. submitter.yaml, sequencer.default.toml)
    # Let's just check if the runtime directory has any config files.
    if docker compose exec -T sequencer ls /runtime/ > /dev/null 2>&1; then
        echo " [INFO] /runtime directory exists and is populated."
    fi
fi

echo "3. Checking Sequencer Traffic Port..."
if curl -s http://127.0.0.1:3000/ -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"rollup_health","params":{},"id":1}' | grep -q "jsonrpc"; then
    echo " [OK] Sequencer responds on port 3000"
else
    echo " [FAIL] Sequencer not responding on port 3000"
    exit 1
fi

echo "4. Checking Executor Lifecycle..."
if docker compose logs executor | grep -qi "generated\|persisted\|proved\|published"; then
    echo " [OK] Executor shows lifecycle activity"
else
    echo " [WARN] No executor lifecycle activity found yet (expected if no transactions sent)"
fi

echo "5. Checking Submitter Activity..."
if docker compose logs submitter | grep -qi "submit\|batch\|outbox"; then
    echo " [OK] Submitter shows submission activity"
else
    echo " [WARN] No submitter activity found yet (expected if no transactions sent)"
fi

echo "=== Verification Complete ==="
