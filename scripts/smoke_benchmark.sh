#!/usr/bin/env bash
# scripts/smoke_benchmark.sh
# Runs a short benchmark to ensure end-to-end functionality in a containerized setup.

set -euo pipefail

echo "========================================"
echo " Running Smoke Benchmark (Docker)"
echo "========================================"

# Check if benchmark service image is built
if ! docker images | grep -q "rollupx/benchmark"; then
    echo "Building benchmark image..."
    docker compose --profile bench build benchmark
fi

# Run the benchmark runner container, overriding env vars for a short smoke test
docker compose --profile core --profile bench run --rm \
    -e DURATION_S=15 \
    -e RATE_TPS=2 \
    -e WARMUP_S=5 \
    -e TX_MIX=transfer_only \
    -e EXP_ID=smoke_test \
    -e REPEAT=1 \
    -e POLICY=FCFS \
    -e PROVER=groth16 \
    -e DA_MODE=calldata \
    benchmark bash /app/scripts/run_experiment.sh smoke_test 1

echo "========================================"
echo " Smoke benchmark finished."
echo " Check the local /var/lib/rollupx/metrics (if mapped) or the docker volumes for metrics/smoke_test/smoke_test_r01/ artifacts."
echo "========================================"
