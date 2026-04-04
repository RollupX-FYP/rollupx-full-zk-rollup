#!/usr/bin/env bash
# collect_env.sh — write run_metadata.json to METRICS_ROOT
# Usage: bash collect_env.sh <run_id> <experiment_id>
# All experiment parameters are expected as environment variables.

set -euo pipefail

RUN_ID=${1:-${RUN_ID:-unknown}}
EXP_ID=${2:-${EXPERIMENT_ID:-unknown}}
METRICS_ROOT=${METRICS_ROOT:-metrics}

mkdir -p "$METRICS_ROOT"

# ── gather info ───────────────────────────────────────────────────────────────

GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# CPU / RAM
OS_TYPE=$(uname)

if [[ "$OS_TYPE" == "Darwin" ]]; then
    CPU_MODEL=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown")
    CPU_CORES=$(sysctl -n hw.logicalcpu 2>/dev/null || echo "unknown")
    RAM_BYTES=$(sysctl -n hw.memsize 2>/dev/null || echo 0)
    RAM_GB=$(python3 -c "print(round($RAM_BYTES / 1073741824, 1))" 2>/dev/null || echo "unknown")
elif [[ "$OS_TYPE" == "Linux" ]]; then
    CPU_MODEL=$(grep "model name" /proc/cpuinfo 2>/dev/null | head -1 | cut -d: -f2 | xargs || echo "unknown")
    CPU_CORES=$(nproc 2>/dev/null || echo "unknown")
    if [[ -f /proc/meminfo ]]; then
        RAM_KB=$(grep MemTotal /proc/meminfo | awk '{print $2}')
        RAM_GB=$(echo "scale=1; $RAM_KB / 1048576" | bc 2>/dev/null || echo "unknown")
    else
        RAM_GB="unknown"
    fi
else
    CPU_MODEL="unknown"; CPU_CORES="unknown"; RAM_GB="unknown"
fi
OS_INFO=$(uname -srm 2>/dev/null || echo "unknown")

# language runtimes
PYTHON_VERSION=$(python3 --version 2>/dev/null || echo "unknown")
RUST_VERSION=$(rustc --version 2>/dev/null || echo "unknown")

# ── write JSON ────────────────────────────────────────────────────────────────

cat > "$METRICS_ROOT/run_metadata.json" <<EOF
{
  "run_id":        "$RUN_ID",
  "experiment_id": "$EXP_ID",
  "git_commit":    "$GIT_COMMIT",
  "timestamp_start": "$TIMESTAMP",
  "timestamp_end":   "pending",
  "machine": {
    "cpu_model":  "$CPU_MODEL",
    "cpu_cores":  $CPU_CORES,
    "ram_gb":     "$RAM_GB",
    "os":         "$OS_INFO"
  },
  "runtimes": {
    "python": "$PYTHON_VERSION",
    "rust":   "$RUST_VERSION"
  },
  "config_snapshot": {
    "experiment_id": "$EXP_ID",
    "batch_size":    "${MAX_BATCH_SIZE:-unknown}",
    "timeout_ms":    "${TIMEOUT_MS:-unknown}",
    "policy":        "${POLICY:-unknown}",
    "da_mode":       "${DA_MODE:-unknown}",
    "prover":        "${PROVER:-unknown}",
    "rate_tps":      "${RATE_TPS:-unknown}",
    "duration_s":    "${DURATION_S:-unknown}",
    "warmup_s":      "${WARMUP_S:-unknown}",
    "tx_mix":        "${TX_MIX:-unknown}",
    "seed":          "${SEED:-unknown}"
  }
}
EOF

echo "[collect_env] written → $METRICS_ROOT/run_metadata.json"
