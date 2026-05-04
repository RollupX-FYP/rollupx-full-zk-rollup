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
ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"

# ── defaults for env vars (override via environment) ──────────────────────────
export MAX_BATCH_SIZE=${MAX_BATCH_SIZE:-50}
export TIMEOUT_MS=${TIMEOUT_MS:-30000}
export MIN_BATCH_SIZE=${MIN_BATCH_SIZE:-1}
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
export EXPERIMENT_NAME=${EXPERIMENT_NAME:-$EXP_ID}
export VALIDITY_ENVIRONMENT=${VALIDITY_ENVIRONMENT:-local_hardhat}
export VALIDITY_NETWORK_MODEL=${VALIDITY_NETWORK_MODEL:-single_node_local}
export VALIDITY_EXECUTION_SCOPE=${VALIDITY_EXECUTION_SCOPE:-transfer_centric_stf}
export VALIDITY_PROOF_MODE_POLICY=${VALIDITY_PROOF_MODE_POLICY:-groth16_only}
export VALIDITY_COST_INTERPRETATION=${VALIDITY_COST_INTERPRETATION:-comparative_not_market_representative}
export CLEAN_STATE_BEFORE_RUN=${CLEAN_STATE_BEFORE_RUN:-1}
export CLEAN_METRICS_BEFORE_RUN=${CLEAN_METRICS_BEFORE_RUN:-1}
export USE_DOCKER_STACK=${USE_DOCKER_STACK:-1}

METRICS_ROOT="${METRICS_ROOT:-metrics}/${EXP_ID}/${RUN_ID}"
export METRICS_ROOT
SHARED_METRICS_DIR="${SHARED_METRICS_DIR:-metrics/latest}"
export SHARED_METRICS_DIR

should_use_docker_stack() {
    if [[ "$USE_DOCKER_STACK" == "1" || "$USE_DOCKER_STACK" == "true" ]]; then
        return 0
    fi
    if [[ "$USE_DOCKER_STACK" == "0" || "$USE_DOCKER_STACK" == "false" ]]; then
        return 1
    fi
    [[ -f "${ROOT_DIR}/docker-compose.yml" ]] && command -v docker >/dev/null 2>&1
}

restart_docker_stack_for_run() {
    local metrics_abs
    metrics_abs="$(cd "$(dirname "$METRICS_ROOT")" && pwd)/$(basename "$METRICS_ROOT")"

    echo "[docker] recreating core stack for ${RUN_ID}"
    echo "[docker] metrics dir: ${metrics_abs}"
    (
        cd "$ROOT_DIR"
        METRICS_DIR="$metrics_abs" \
        EXPERIMENT_ID="$EXP_ID" \
        EXPERIMENT_NAME="$EXPERIMENT_NAME" \
        SEQUENCER_BATCH_MAX_SIZE="$MAX_BATCH_SIZE" \
        SEQUENCER_BATCH_TIMEOUT_MS="$TIMEOUT_MS" \
        SEQUENCER_BATCH_MIN_SIZE="$MIN_BATCH_SIZE" \
        SEQUENCER_POLICY="$POLICY" \
        SUBMITTER_DA_MODE="$DA_MODE" \
        SUBMITTER_PROOF_BACKEND="$PROVER" \
        docker compose --profile core down -v --remove-orphans

        if [[ "${DOCKER_UP_BUILD:-1}" == "1" || "${DOCKER_UP_BUILD:-1}" == "true" ]]; then
            METRICS_DIR="$metrics_abs" \
            EXPERIMENT_ID="$EXP_ID" \
            EXPERIMENT_NAME="$EXPERIMENT_NAME" \
            SEQUENCER_BATCH_MAX_SIZE="$MAX_BATCH_SIZE" \
            SEQUENCER_BATCH_TIMEOUT_MS="$TIMEOUT_MS" \
            SEQUENCER_BATCH_MIN_SIZE="$MIN_BATCH_SIZE" \
            SEQUENCER_POLICY="$POLICY" \
            SUBMITTER_DA_MODE="$DA_MODE" \
            SUBMITTER_PROOF_BACKEND="$PROVER" \
            docker compose --profile core up -d --force-recreate --build
        else
            METRICS_DIR="$metrics_abs" \
            EXPERIMENT_ID="$EXP_ID" \
            EXPERIMENT_NAME="$EXPERIMENT_NAME" \
            SEQUENCER_BATCH_MAX_SIZE="$MAX_BATCH_SIZE" \
            SEQUENCER_BATCH_TIMEOUT_MS="$TIMEOUT_MS" \
            SEQUENCER_BATCH_MIN_SIZE="$MIN_BATCH_SIZE" \
            SEQUENCER_POLICY="$POLICY" \
            SUBMITTER_DA_MODE="$DA_MODE" \
            SUBMITTER_PROOF_BACKEND="$PROVER" \
            docker compose --profile core up -d --force-recreate
        fi
    )
    bash "$(dirname "$0")/wait_for_sequencer.sh" "$SEQ_HOST" "$SEQ_PORT" 60
    collect_docker_diagnostics "after_start"
}

copy_component_metrics() {
    local copied=0
    local src
    for src in \
        "${SHARED_METRICS_DIR}/sequencer_batch_metrics.jsonl" \
        "${SHARED_METRICS_DIR}/executor_batch_metrics.jsonl" \
        "${SHARED_METRICS_DIR}/submitter_metrics.json"; do
        if [[ -f "$src" ]]; then
            cp "$src" "${METRICS_ROOT}/$(basename "$src")"
            copied=$((copied + 1))
        fi
    done
    if [[ "$copied" -gt 0 ]]; then
        echo "[metrics] copied ${copied} component metric file(s) from ${SHARED_METRICS_DIR}"
    else
        echo "[metrics] no component metric files found in ${SHARED_METRICS_DIR}"
    fi
}

component_metrics_size() {
    local total=0
    local src
    for src in \
        "${METRICS_ROOT}/sequencer_batch_metrics.jsonl" \
        "${METRICS_ROOT}/executor_batch_metrics.jsonl" \
        "${METRICS_ROOT}/submitter_metrics.json"; do
        if [[ -f "$src" ]]; then
            total=$((total + $(wc -c < "$src")))
        fi
    done
    echo "$total"
}

metric_rows() {
    local src="$1"
    if [[ -f "$src" ]]; then
        wc -l < "$src"
    else
        echo 0
    fi
}

component_metric_counts() {
    local seq exe sub
    seq=$(metric_rows "${METRICS_ROOT}/sequencer_batch_metrics.jsonl")
    exe=$(metric_rows "${METRICS_ROOT}/executor_batch_metrics.jsonl")
    sub=$(metric_rows "${METRICS_ROOT}/submitter_metrics.json")
    echo "$seq $exe $sub"
}

component_metrics_caught_up() {
    local seq exe sub
    read -r seq exe sub < <(component_metric_counts)

    [[ "$seq" -gt 0 ]] || return 1
    [[ "$exe" -gt 0 ]] || return 1
    [[ "$sub" -gt 0 ]] || return 1
    [[ "$exe" -ge "$seq" ]] || return 1
    [[ "$sub" -ge "$exe" ]] || return 1
}

summarize_component_metrics() {
    local missing=0
    local src
    echo "[metrics] component metric files:"
    for src in \
        "${METRICS_ROOT}/sequencer_batch_metrics.jsonl" \
        "${METRICS_ROOT}/executor_batch_metrics.jsonl" \
        "${METRICS_ROOT}/submitter_metrics.json"; do
        if [[ -f "$src" ]]; then
            echo "  [OK] $(basename "$src") ($(wc -l < "$src") rows, $(wc -c < "$src") bytes)"
        else
            echo "  [MISS] $(basename "$src")"
            missing=$((missing + 1))
        fi
    done
    if [[ "$missing" -gt 0 ]]; then
        echo "[metrics] WARNING: ${missing} component metric file(s) missing; inspect docker compose logs for executor/submitter pipeline errors."
    fi
}

validate_component_metrics() {
    local require="${REQUIRE_COMPONENT_METRICS:-}"
    if [[ -z "$require" ]]; then
        require="$USED_DOCKER_STACK"
    fi
    if [[ "$require" != "1" && "$require" != "true" ]]; then
        return 0
    fi

    local seq exe sub
    read -r seq exe sub < <(component_metric_counts)
    local failed=0

    if [[ "$seq" -eq 0 ]]; then
        echo "[metrics] ERROR: missing sequencer batch metrics"
        failed=1
    fi
    if [[ "$exe" -eq 0 ]]; then
        echo "[metrics] ERROR: missing executor batch metrics"
        failed=1
    fi
    if [[ "$sub" -eq 0 ]]; then
        echo "[metrics] ERROR: missing submitter metrics"
        failed=1
    fi
    if [[ "$exe" -lt "$seq" ]]; then
        echo "[metrics] ERROR: executor metrics lag sequencer metrics (${exe} < ${seq})"
        failed=1
    fi
    if [[ "$sub" -lt "$exe" ]]; then
        echo "[metrics] ERROR: submitter metrics lag executor metrics (${sub} < ${exe})"
        failed=1
    fi

    return "$failed"
}

validate_workload_status() {
    local status_file="${METRICS_ROOT}/run_status.json"
    if [[ ! -f "$status_file" ]]; then
        echo "[workload] ERROR: missing run_status.json"
        return 1
    fi
    if ! grep -Eq '"status"[[:space:]]*:[[:space:]]*"pass"' "$status_file"; then
        echo "[workload] ERROR: workload status is not pass"
        return 1
    fi
}

collect_docker_diagnostics() {
    local phase="${1:-final}"
    local metrics_abs
    metrics_abs="$(cd "$METRICS_ROOT" && pwd)"
    local diag_dir="${metrics_abs}/diagnostics/${phase}"
    mkdir -p "$diag_dir"

    if ! command -v docker >/dev/null 2>&1; then
        echo "[diagnostics] docker not available; skipping docker diagnostics"
        return
    fi

    echo "[diagnostics] collecting docker diagnostics (${phase}) -> ${diag_dir}"
    (
        cd "$ROOT_DIR"
        docker compose --profile core ps > "${diag_dir}/compose_ps.txt" 2>&1 || true
        docker compose --profile core logs --no-color --tail=500 sequencer > "${diag_dir}/sequencer.log" 2>&1 || true
        docker compose --profile core logs --no-color --tail=500 executor > "${diag_dir}/executor.log" 2>&1 || true
        docker compose --profile core logs --no-color --tail=500 submitter > "${diag_dir}/submitter.log" 2>&1 || true
        docker compose --profile core logs --no-color --tail=300 contracts-deployer > "${diag_dir}/contracts-deployer.log" 2>&1 || true
        docker exec rollupx-full-zk-rollup-sequencer-1 sh -lc 'echo "METRICS_ROOT=$METRICS_ROOT"; echo "EXPERIMENT_ID=$EXPERIMENT_ID"; ls -lah /var/lib/rollupx/metrics' > "${diag_dir}/sequencer_metrics_env.txt" 2>&1 || true
        docker exec rollupx-full-zk-rollup-executor-1 sh -lc 'echo "METRICS_ROOT=$METRICS_ROOT"; echo "EXPERIMENT_ID=$EXPERIMENT_ID"; ls -lah /var/lib/rollupx/metrics' > "${diag_dir}/executor_metrics_env.txt" 2>&1 || true
        docker exec rollupx-full-zk-rollup-submitter-1 sh -lc 'echo "METRICS_ROOT=$METRICS_ROOT"; echo "EXPERIMENT_ID=$EXPERIMENT_ID"; ls -lah /var/lib/rollupx/metrics' > "${diag_dir}/submitter_metrics_env.txt" 2>&1 || true
    )
}

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
if [[ "$CLEAN_METRICS_BEFORE_RUN" == "1" || "$CLEAN_METRICS_BEFORE_RUN" == "true" ]]; then
    rm -rf "$METRICS_ROOT"
fi
mkdir -p "$METRICS_ROOT"
mkdir -p "$SHARED_METRICS_DIR"
LOGFILE="$METRICS_ROOT/run.log"
exec > >(tee -a "$LOGFILE") 2>&1

# reset component metric files so each run gets isolated snapshots
rm -f "${METRICS_ROOT}/sequencer_batch_metrics.jsonl" \
      "${METRICS_ROOT}/executor_batch_metrics.jsonl" \
      "${METRICS_ROOT}/submitter_metrics.json" \
      "${SHARED_METRICS_DIR}/sequencer_batch_metrics.jsonl" \
      "${SHARED_METRICS_DIR}/executor_batch_metrics.jsonl" \
      "${SHARED_METRICS_DIR}/submitter_metrics.json"

# ── optional: reset local runtime state for controlled experiments ───────────
if [[ "$CLEAN_STATE_BEFORE_RUN" == "1" || "$CLEAN_STATE_BEFORE_RUN" == "true" ]]; then
    bash "$(dirname "$0")/reset_state.sh" "$RUN_ID"
fi

echo "======================================================================"
echo "  RUN: $RUN_ID"
echo "  Exp: $EXP_ID  |  Repeat: $REPEAT  |  Seed: $SEED"
echo "  Name: $EXPERIMENT_NAME"
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
SEQUENCER_BIN=${SEQUENCER_BIN:-./target/release/sequencer}
USED_DOCKER_STACK=0

if should_use_docker_stack; then
    USED_DOCKER_STACK=1
    restart_docker_stack_for_run
elif [[ -x "$SEQUENCER_BIN" ]]; then
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
# Poll component metrics until executor/submitter have caught up and files stop growing.
echo "[wait] waiting for component metrics to flush ..."
PREV_SIZE=0
STABLE_COUNT=0

SUBMITTER_WAIT_MAX=${SUBMITTER_WAIT_MAX:-120}
for poll in $(seq 1 "$SUBMITTER_WAIT_MAX"); do
    sleep 3
    CURR_SIZE=$(component_metrics_size)
    read -r SEQ_ROWS EXE_ROWS SUB_ROWS < <(component_metric_counts)
    echo "[wait] poll=${poll}/${SUBMITTER_WAIT_MAX} rows: sequencer=${SEQ_ROWS} executor=${EXE_ROWS} submitter=${SUB_ROWS} bytes=${CURR_SIZE}"

    if [[ "$CURR_SIZE" -eq "$PREV_SIZE" ]]; then
        STABLE_COUNT=$((STABLE_COUNT + 1))
        if [[ "$STABLE_COUNT" -ge 5 ]] && component_metrics_caught_up; then
            echo "[wait] component metrics caught up and idle (stable for 15s)"
            break
        fi
    else
        STABLE_COUNT=0
        PREV_SIZE="$CURR_SIZE"
    fi
done

# Copy component-level metrics from the legacy shared directory if a non-Docker run used it.
if [[ "$USED_DOCKER_STACK" != "1" && "$SHARED_METRICS_DIR" != "$METRICS_ROOT" ]]; then
    copy_component_metrics
fi
summarize_component_metrics
if [[ "$USED_DOCKER_STACK" == "1" ]]; then
    collect_docker_diagnostics "final"
fi
validate_component_metrics
validate_workload_status

# ── 7. Update timestamp_end in metadata ───────────────────────────────────────
END_TS=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
METADATA_FILE="$METRICS_ROOT/run_metadata.json"
if command -v jq &>/dev/null && [[ -f "$METADATA_FILE" ]]; then
    tmp=$(mktemp)
    jq --arg ts "$END_TS" '.timestamp_end = $ts' "$METADATA_FILE" > "$tmp" && mv "$tmp" "$METADATA_FILE"
fi

echo "[DONE] $RUN_ID  →  $METRICS_ROOT"
