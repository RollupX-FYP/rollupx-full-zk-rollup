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

export PATH="/root/.risc0/bin:${PATH}"
echo "[executor-entrypoint] PROVER_BACKEND=${PROVER_BACKEND:-unset}"
echo "[executor-entrypoint] RISC0_HOST_BIN=${RISC0_HOST_BIN:-unset}"
if [ -n "${RISC0_GUEST_ELF:-}" ]; then
    echo "[executor-entrypoint] RISC0_GUEST_ELF=${RISC0_GUEST_ELF}"
fi
if [ "${PROVER_BACKEND:-}" = "risc0" ]; then
    if [ ! -x "${RISC0_HOST_BIN:-/usr/local/bin/rollup_host}" ]; then
        echo "[executor-entrypoint] ERROR: RISC0 host binary is missing or not executable" >&2
        exit 1
    fi
    ls -lh "${RISC0_HOST_BIN:-/usr/local/bin/rollup_host}"
    if command -v r0vm >/dev/null 2>&1; then
        echo "[executor-entrypoint] r0vm=$(command -v r0vm)"
    else
        echo "[executor-entrypoint] ERROR: r0vm is not available on PATH" >&2
        exit 1
    fi
fi

exec /usr/local/bin/zksync_state_machine
