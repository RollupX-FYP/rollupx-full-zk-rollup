#!/bin/sh
set -eu

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    echo "RollupX executor container"
    echo "Environment defaults:"
    echo "  EXECUTOR_GRPC_ADDR=${EXECUTOR_GRPC_ADDR:-0.0.0.0:50051}"
    echo "  TRACE_ROOT=${TRACE_ROOT:-/var/lib/rollupx/executor/traces}"
    echo "  STATE_DB_PATH=${STATE_DB_PATH:-/var/lib/rollupx/executor/state_db}"
    echo "  RISC0_HOST_BIN=${RISC0_HOST_BIN:-/usr/local/bin/rollup_host}"
    exit 0
fi

exec /usr/local/bin/zksync_state_machine
