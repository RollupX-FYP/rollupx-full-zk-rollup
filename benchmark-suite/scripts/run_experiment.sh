#!/usr/bin/env bash
# run_experiment.sh — orchestrate one complete experiment run end-to-end.
#
# Usage:
#   bash run_experiment.sh <experiment_id> <repeat_index>
#
# All experiment parameters are expected as environment variables (set by
# run_matrix.sh or manually). See PLAN.md §14 for the full list.
#
# Outputs:
#   metrics/<exp_id>/<run_id>/workload_<exp_id>.json
#   metrics/<exp_id>/<run_id>/run_metadata.json
#   metrics/<exp_id>/<run_id>/run_status.json
#   metrics/<exp_id>/<run_id>/tx_log_<run_id>.csv

set -euo pipefail

# ── args ──────────────────────────────────────────────────────────────────────
EXP_ID=${1:?Usage: run_experiment.sh <experiment_id> <repeat_index>}
REPEAT=${2:?Usage: run_experiment.sh <experiment_id> <repeat_index>}
RUN_ID="${EXP_ID}_r$(printf '%02d' "$REPEAT")"

# ── defaults for env vars (override via environment) ──────────────────────────
export MAX_BATCH_SIZE=${MAX_BATCH_SIZE:-50}
export TIMEOUT_MS=${TIMEOUT_MS:-5000}
export POLICY=${POLICY:-FCFS}
export DA_MODE=${DA_MODE:-calldata}
export PROVER=${PROVER:-groth16}
export RATE_TPS=${RATE_TPS:-10}
export DURATION_S=${DURATION_S:-120}
export WARMUP_S=${WARMUP_S:-15}
export TX_MIX=${TX_MIX:-balanced}
export SEED=${SEED:-42}
export SEQ_HOST=${SEQ_HOST:-localhost}
export SEQ_PORT=${SEQ_PORT:-3000}
export L1_RPC_URL=${L1_RPC_URL:-https://sepolia.infura.io/v3/YOUR_KEY}
export BRIDGE_ADDRESS=${BRIDGE_ADDRESS:-0x0000000000000000000000000000000000000000}
export START_BLOCK=${START_BLOCK:-0}
export RUN_ID="$RUN_ID"
export EXPERIMENT_ID="$EXP_ID"

METRICS_ROOT="metrics/${EXP_ID}/${RUN_ID}"
export METRICS_ROOT

# ── traps — always clean up sequencer ─────────────────────────────────────────
SEQ_PID=""
cleanup() {
    if [[ -n "$SEQ_PID" ]]; then
        echo "[cleanup] killing sequencer PID $SEQ_PID"
        kill "$SEQ_PID" 2>/dev/null || true
        wait "$SEQ_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# ── 1. Prepare output directory ───────────────────────────────────────────────
mkdir -p "$METRICS_ROOT"
LOGFILE="$METRICS_ROOT/run.log"
exec > >(tee -a "$LOGFILE") 2>&1

echo "======================================================================"
echo "  RUN: $RUN_ID"
echo "  Exp: $EXP_ID  |  Repeat: $REPEAT  |  Seed: $SEED"
echo "  batch_size=$MAX_BATCH_SIZE  timeout=${TIMEOUT_MS}ms  policy=$POLICY"
echo "  da=$DA_MODE  prover=$PROVER  rate=${RATE_TPS}tps  mix=$TX_MIX"
echo "======================================================================"

# ── 2. Collect environment metadata ──────────────────────────────────────────
bash "$(dirname "$0")/collect_env.sh" "$RUN_ID" "$EXP_ID"

# ── 3. Write sequencer config from template ──────────────────────────────────
SEQ_CONFIG="/tmp/seq_config_${RUN_ID}.toml"
if [[ ! -f "config/sequencer.template.toml" ]]; then
    echo "[WARN] config/sequencer.template.toml not found — skipping config write"
else
    envsubst < "config/sequencer.template.toml" > "$SEQ_CONFIG"
    echo "[config] written → $SEQ_CONFIG"
fi

# ── 4. (Re)start sequencer ────────────────────────────────────────────────────
# Adjust SEQUENCER_BIN to your actual binary path.
SEQUENCER_BIN=${SEQUENCER_BIN:-./target/release/rollup_sequencer}

if [[ -x "$SEQUENCER_BIN" ]]; then
    echo "[sequencer] stopping any existing instance ..."
    pkill -f "$(basename "$SEQUENCER_BIN")" 2>/dev/null || true
    sleep 1

    echo "[sequencer] starting with config $SEQ_CONFIG ..."
    ROLLUPX_CONFIG="$SEQ_CONFIG" "$SEQUENCER_BIN" \
        > "$METRICS_ROOT/sequencer.log" 2>&1 &
    SEQ_PID=$!
    echo "[sequencer] PID=$SEQ_PID"

    bash "$(dirname "$0")/wait_for_sequencer.sh" "$SEQ_HOST" "$SEQ_PORT" 30
else
    echo "[WARN] Sequencer binary not found at $SEQUENCER_BIN"
    echo "       Assuming sequencer is already running and correctly configured."
fi

# ── 5. Run workload generator ─────────────────────────────────────────────────
echo "[workload] starting ..."
python3 workload/poisson_generator.py \
    --experiment_id "$EXP_ID" \
    --run_id        "$RUN_ID" \
    --rate          "$RATE_TPS" \
    --duration      "$DURATION_S" \
    --warmup        "$WARMUP_S" \
    --seed          "$SEED" \
    --tx_mix        "$TX_MIX" \
    --prover_backend "$PROVER" \
    --host          "$SEQ_HOST" \
    --port          "$SEQ_PORT"

# ── 6. Wait for submitter to flush final batch ────────────────────────────────
# Poll for submitter idle: check that submitter_metrics.json stops growing.
echo "[wait] waiting for submitter to flush ..."
SUBMITTER_METRICS="${METRICS_ROOT}/submitter_metrics.json"
PREV_SIZE=0
STABLE_COUNT=0

SUBMITTER_WAIT_MAX=${SUBMITTER_WAIT_MAX:-40}
for _ in $(seq 1 "$SUBMITTER_WAIT_MAX"); do
    sleep 3
    if [[ -f "$SUBMITTER_METRICS" ]]; then
        CURR_SIZE=$(wc -c < "$SUBMITTER_METRICS")
        if [[ "$CURR_SIZE" -eq "$PREV_SIZE" ]]; then
            STABLE_COUNT=$((STABLE_COUNT + 1))
            if [[ "$STABLE_COUNT" -ge 5 ]]; then
                echo "[wait] submitter appears idle (stable for 15s)"
                break
            fi
        else
            STABLE_COUNT=0
            PREV_SIZE="$CURR_SIZE"
        fi
    fi
done

# ── 7. Update timestamp_end in metadata ───────────────────────────────────────
END_TS=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
METADATA_FILE="$METRICS_ROOT/run_metadata.json"
if command -v jq &>/dev/null && [[ -f "$METADATA_FILE" ]]; then
    tmp=$(mktemp)
    jq --arg ts "$END_TS" '.timestamp_end = $ts' "$METADATA_FILE" > "$tmp" && mv "$tmp" "$METADATA_FILE"
fi

echo "[DONE] $RUN_ID  →  $METRICS_ROOT"
