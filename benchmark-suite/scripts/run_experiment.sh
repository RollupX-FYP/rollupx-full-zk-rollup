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
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
RUN_ID_WITH_TIMESTAMP="${RUN_ID}_${TIMESTAMP}"
ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"

# ── defaults for env vars (override via environment) ──────────────────────────
export MAX_BATCH_SIZE=${MAX_BATCH_SIZE:-50}
export TIMEOUT_MS=${TIMEOUT_MS:-30000}
export MIN_BATCH_SIZE=${MIN_BATCH_SIZE:-1}
export BATCH_POLICY=${BATCH_POLICY:-fixed}
export ADAPTIVE_LOW_LOAD_THRESHOLD=${ADAPTIVE_LOW_LOAD_THRESHOLD:-50}
export ADAPTIVE_MEDIUM_LOAD_THRESHOLD=${ADAPTIVE_MEDIUM_LOAD_THRESHOLD:-200}
export ADAPTIVE_SMALL_BATCH_SIZE=${ADAPTIVE_SMALL_BATCH_SIZE:-50}
export ADAPTIVE_MEDIUM_BATCH_SIZE=${ADAPTIVE_MEDIUM_BATCH_SIZE:-100}
export ADAPTIVE_LARGE_BATCH_SIZE=${ADAPTIVE_LARGE_BATCH_SIZE:-500}
export BLOB_TARGET_BYTES=${BLOB_TARGET_BYTES:-131072}
export BLOB_FILL_TARGET=${BLOB_FILL_TARGET:-0.90}
export POLICY=${POLICY:-FCFS}
export DA_MODE=${DA_MODE:-calldata}
export PROVER=${PROVER:-groth16}
export REQUIRE_REAL_PROOFS=${REQUIRE_REAL_PROOFS:-true}
export ETH_PRICE_USD=${ETH_PRICE_USD:-2500}
export REGULAR_GAS_PRICE_GWEI=${REGULAR_GAS_PRICE_GWEI:-2}
export BLOB_GAS_PRICE_GWEI=${BLOB_GAS_PRICE_GWEI:-0.001}
export RATE_TPS=${RATE_TPS:-10}
export DURATION_S=${DURATION_S:-120}
export WARMUP_S=${WARMUP_S:-15}
export WORKLOAD_CONCURRENCY=${WORKLOAD_CONCURRENCY:-1}
export WORKLOAD_TARGET_TXS=${WORKLOAD_TARGET_TXS:-0}
export WORKLOAD_ACCOUNT_COUNT=${WORKLOAD_ACCOUNT_COUNT:-1}
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
export HARDHAT_MINING_INTERVAL=${HARDHAT_MINING_INTERVAL:-12000}
export SEQUENCER_EXECUTOR_PUBLISH_RETRIES=${SEQUENCER_EXECUTOR_PUBLISH_RETRIES:-5}
export SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS=${SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS:-10000}
export HARDHAT_DEV_ADDRESSES=${HARDHAT_DEV_ADDRESSES:-"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266,0x70997970C51812dc3A010C7d01b50e0d17dc79C8,0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC,0x90F79bf6EB2c4f870365E785982E1f101E93b906,0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65,0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc,0x976EA74026e726554db657fa54763AbBf14479A3,0x14dC79964da2C08b23698b3D3cc7Ca32193d9955,0x23618e81e3E31A58A0A5fF5C9D8f6F2368D4F2c6,0xa0Ee7A142d267C1f36714e4a8F75612F20a79720"}

sender_addresses_for_count() {
    local count="${1:-1}"
    python3 - "$count" "$HARDHAT_DEV_ADDRESSES" <<'PY'
import sys
count = max(1, int(sys.argv[1]))
addresses = [s.strip() for s in sys.argv[2].split(",") if s.strip()]
if count > len(addresses):
    raise SystemExit(f"WORKLOAD_ACCOUNT_COUNT={count} exceeds available seeded addresses={len(addresses)}")
print(",".join(addresses[:count]))
PY
}

METRICS_ROOT="${METRICS_ROOT:-metrics}/${EXP_ID}/${RUN_ID_WITH_TIMESTAMP}"
export METRICS_ROOT
SHARED_METRICS_DIR="${SHARED_METRICS_DIR:-metrics/latest}"
export SHARED_METRICS_DIR
RISC0_HOST_WORK_DIR="${RISC0_HOST_WORK_DIR:-${ROOT_DIR}/benchmark-suite/risc0_work}"
export RISC0_HOST_WORK_DIR

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
        SEQUENCER_BATCH_POLICY="$BATCH_POLICY" \
        SEQUENCER_ADAPTIVE_LOW_LOAD_THRESHOLD="$ADAPTIVE_LOW_LOAD_THRESHOLD" \
        SEQUENCER_ADAPTIVE_MEDIUM_LOAD_THRESHOLD="$ADAPTIVE_MEDIUM_LOAD_THRESHOLD" \
        SEQUENCER_ADAPTIVE_SMALL_BATCH_SIZE="$ADAPTIVE_SMALL_BATCH_SIZE" \
        SEQUENCER_ADAPTIVE_MEDIUM_BATCH_SIZE="$ADAPTIVE_MEDIUM_BATCH_SIZE" \
        SEQUENCER_ADAPTIVE_LARGE_BATCH_SIZE="$ADAPTIVE_LARGE_BATCH_SIZE" \
        SEQUENCER_BLOB_TARGET_BYTES="$BLOB_TARGET_BYTES" \
        SEQUENCER_BLOB_FILL_TARGET="$BLOB_FILL_TARGET" \
        SEQUENCER_POLICY="$POLICY" \
        SUBMITTER_DA_MODE="$DA_MODE" \
        SUBMITTER_PROOF_BACKEND="$PROVER" \
        REQUIRE_REAL_PROOFS="$REQUIRE_REAL_PROOFS" \
        ETH_PRICE_USD="$ETH_PRICE_USD" \
        REGULAR_GAS_PRICE_GWEI="$REGULAR_GAS_PRICE_GWEI" \
        BLOB_GAS_PRICE_GWEI="$BLOB_GAS_PRICE_GWEI" \
        SEQUENCER_EXECUTOR_PUBLISH_RETRIES="$SEQUENCER_EXECUTOR_PUBLISH_RETRIES" \
        SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS="$SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS" \
        RISC0_HOST_WORK_DIR="$RISC0_HOST_WORK_DIR" \
        docker compose --profile core down -v --remove-orphans

        if [[ "${DOCKER_UP_BUILD:-1}" == "1" || "${DOCKER_UP_BUILD:-1}" == "true" ]]; then
            METRICS_DIR="$metrics_abs" \
            EXPERIMENT_ID="$EXP_ID" \
            EXPERIMENT_NAME="$EXPERIMENT_NAME" \
            SEQUENCER_BATCH_MAX_SIZE="$MAX_BATCH_SIZE" \
            SEQUENCER_BATCH_TIMEOUT_MS="$TIMEOUT_MS" \
            SEQUENCER_BATCH_MIN_SIZE="$MIN_BATCH_SIZE" \
            SEQUENCER_BATCH_POLICY="$BATCH_POLICY" \
            SEQUENCER_ADAPTIVE_LOW_LOAD_THRESHOLD="$ADAPTIVE_LOW_LOAD_THRESHOLD" \
            SEQUENCER_ADAPTIVE_MEDIUM_LOAD_THRESHOLD="$ADAPTIVE_MEDIUM_LOAD_THRESHOLD" \
            SEQUENCER_ADAPTIVE_SMALL_BATCH_SIZE="$ADAPTIVE_SMALL_BATCH_SIZE" \
            SEQUENCER_ADAPTIVE_MEDIUM_BATCH_SIZE="$ADAPTIVE_MEDIUM_BATCH_SIZE" \
            SEQUENCER_ADAPTIVE_LARGE_BATCH_SIZE="$ADAPTIVE_LARGE_BATCH_SIZE" \
            SEQUENCER_BLOB_TARGET_BYTES="$BLOB_TARGET_BYTES" \
            SEQUENCER_BLOB_FILL_TARGET="$BLOB_FILL_TARGET" \
            SEQUENCER_POLICY="$POLICY" \
            SUBMITTER_DA_MODE="$DA_MODE" \
            SUBMITTER_PROOF_BACKEND="$PROVER" \
            REQUIRE_REAL_PROOFS="$REQUIRE_REAL_PROOFS" \
            ETH_PRICE_USD="$ETH_PRICE_USD" \
            REGULAR_GAS_PRICE_GWEI="$REGULAR_GAS_PRICE_GWEI" \
            BLOB_GAS_PRICE_GWEI="$BLOB_GAS_PRICE_GWEI" \
            SEQUENCER_EXECUTOR_PUBLISH_RETRIES="$SEQUENCER_EXECUTOR_PUBLISH_RETRIES" \
            SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS="$SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS" \
            RISC0_HOST_WORK_DIR="$RISC0_HOST_WORK_DIR" \
            docker compose --profile core up -d --force-recreate --build
        else
            METRICS_DIR="$metrics_abs" \
            EXPERIMENT_ID="$EXP_ID" \
            EXPERIMENT_NAME="$EXPERIMENT_NAME" \
            SEQUENCER_BATCH_MAX_SIZE="$MAX_BATCH_SIZE" \
            SEQUENCER_BATCH_TIMEOUT_MS="$TIMEOUT_MS" \
            SEQUENCER_BATCH_MIN_SIZE="$MIN_BATCH_SIZE" \
            SEQUENCER_BATCH_POLICY="$BATCH_POLICY" \
            SEQUENCER_ADAPTIVE_LOW_LOAD_THRESHOLD="$ADAPTIVE_LOW_LOAD_THRESHOLD" \
            SEQUENCER_ADAPTIVE_MEDIUM_LOAD_THRESHOLD="$ADAPTIVE_MEDIUM_LOAD_THRESHOLD" \
            SEQUENCER_ADAPTIVE_SMALL_BATCH_SIZE="$ADAPTIVE_SMALL_BATCH_SIZE" \
            SEQUENCER_ADAPTIVE_MEDIUM_BATCH_SIZE="$ADAPTIVE_MEDIUM_BATCH_SIZE" \
            SEQUENCER_ADAPTIVE_LARGE_BATCH_SIZE="$ADAPTIVE_LARGE_BATCH_SIZE" \
            SEQUENCER_BLOB_TARGET_BYTES="$BLOB_TARGET_BYTES" \
            SEQUENCER_BLOB_FILL_TARGET="$BLOB_FILL_TARGET" \
            SEQUENCER_POLICY="$POLICY" \
            SUBMITTER_DA_MODE="$DA_MODE" \
            SUBMITTER_PROOF_BACKEND="$PROVER" \
            REQUIRE_REAL_PROOFS="$REQUIRE_REAL_PROOFS" \
            ETH_PRICE_USD="$ETH_PRICE_USD" \
            REGULAR_GAS_PRICE_GWEI="$REGULAR_GAS_PRICE_GWEI" \
            BLOB_GAS_PRICE_GWEI="$BLOB_GAS_PRICE_GWEI" \
            SEQUENCER_EXECUTOR_PUBLISH_RETRIES="$SEQUENCER_EXECUTOR_PUBLISH_RETRIES" \
            SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS="$SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS" \
            RISC0_HOST_WORK_DIR="$RISC0_HOST_WORK_DIR" \
            docker compose --profile core up -d --force-recreate
        fi
    )
    bash "$(dirname "$0")/wait_for_sequencer.sh" "$SEQ_HOST" "$SEQ_PORT" 60
    collect_docker_diagnostics "after_start"

    copy_l1_deployment
}

copy_l1_deployment() {
    if [[ -f "${ROOT_DIR}/runtime/contracts.json" ]]; then
        cp "${ROOT_DIR}/runtime/contracts.json" "${METRICS_ROOT}/l1_deployment.json"
        echo "[metrics] copied l1_deployment.json from shared volume"
    elif command -v docker >/dev/null 2>&1 && docker exec rollupx-full-zk-rollup-submitter-1 ls /runtime/contracts.json >/dev/null 2>&1; then
        docker cp rollupx-full-zk-rollup-submitter-1:/runtime/contracts.json "${METRICS_ROOT}/l1_deployment.json"
        echo "[metrics] copied l1_deployment.json from submitter container"
    else
        echo "[metrics] WARNING: unable to find /runtime/contracts.json for l1_deployment.json"
    fi
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

metric_unique_batch_ids() {
    local src="$1"
    if [[ ! -f "$src" ]]; then
        echo 0
        return 0
    fi
    python3 - "$src" <<'PY'
import json
import sys

path = sys.argv[1]
batch_ids = set()
with open(path, "r", encoding="utf-8") as handle:
    for line in handle:
        line = line.strip()
        if not line:
            continue
        try:
            row = json.loads(line)
        except Exception:
            continue
        batch_id = row.get("batch_id")
        if batch_id is not None:
            batch_ids.add(str(batch_id))
print(len(batch_ids))
PY
}

component_metric_counts() {
    local seq exe sub
    seq=$(metric_unique_batch_ids "${METRICS_ROOT}/sequencer_batch_metrics.jsonl")
    exe=$(metric_unique_batch_ids "${METRICS_ROOT}/executor_batch_metrics.jsonl")
    sub=$(metric_unique_batch_ids "${METRICS_ROOT}/submitter_metrics.json")
    echo "$seq $exe $sub"
}

strict_pipeline_catchup_required() {
    local strict="${STRICT_PIPELINE_CATCHUP:-0}"
    [[ "$strict" == "1" || "$strict" == "true" ]]
}

component_metrics_caught_up() {
    local seq exe sub
    read -r seq exe sub < <(component_metric_counts)

    [[ "$seq" -gt 0 ]] || return 1
    [[ "$exe" -gt 0 ]] || return 1
    [[ "$sub" -gt 0 ]] || return 1
    if strict_pipeline_catchup_required; then
        [[ "$exe" -ge "$seq" ]] || return 1
        [[ "$sub" -ge "$exe" ]] || return 1
    fi
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
            echo "  [OK] $(basename "$src") ($(wc -l < "$src") rows, $(metric_unique_batch_ids "$src") batch_ids, $(wc -c < "$src") bytes)"
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
    if strict_pipeline_catchup_required; then
        if [[ "$exe" -lt "$seq" ]]; then
            echo "[metrics] ERROR: executor metrics lag sequencer metrics (${exe} < ${seq})"
            failed=1
        fi
        if [[ "$sub" -lt "$exe" ]]; then
            echo "[metrics] ERROR: submitter metrics lag executor metrics (${sub} < ${exe})"
            failed=1
        fi
    else
        if [[ "$exe" -lt "$seq" ]]; then
            echo "[metrics] WARN: executor metrics lag sequencer metrics (${exe} < ${seq}) in non-strict mode"
        fi
        if [[ "$sub" -lt "$exe" ]]; then
            echo "[metrics] WARN: submitter metrics lag executor metrics (${sub} < ${exe}) in non-strict mode"
        fi
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

wait_for_component_metrics_flush() {
    echo "[wait] waiting for component metrics to flush ..."
    if strict_pipeline_catchup_required; then
        echo "[wait] mode=strict (require executor>=sequencer and submitter>=executor)"
    else
        echo "[wait] mode=non-strict (require non-zero sequencer/executor/submitter metrics and file idle)"
    fi
    local prev_size=0
    local stable_count=0
    if [[ -z "${SUBMITTER_WAIT_MAX:-}" ]]; then
        if [[ "$PROVER" == "groth16" || "${REQUIRE_REAL_PROOFS:-}" == "1" || "${REQUIRE_REAL_PROOFS:-}" == "true" ]]; then
            SUBMITTER_WAIT_MAX=600
        else
            SUBMITTER_WAIT_MAX=120
        fi
    fi
    COMPONENT_STABLE_POLLS=${COMPONENT_STABLE_POLLS:-$((TIMEOUT_MS / 3000 + 5))}
    for poll in $(seq 1 "$SUBMITTER_WAIT_MAX"); do
        sleep 3
        local curr_size
        curr_size=$(component_metrics_size)
        read -r SEQ_ROWS EXE_ROWS SUB_ROWS < <(component_metric_counts)
        echo "[wait] poll=${poll}/${SUBMITTER_WAIT_MAX} rows: sequencer=${SEQ_ROWS} executor=${EXE_ROWS} submitter=${SUB_ROWS} bytes=${curr_size}"

        if [[ "$curr_size" -eq "$prev_size" ]]; then
            stable_count=$((stable_count + 1))
            if [[ "$stable_count" -ge "$COMPONENT_STABLE_POLLS" ]] && component_metrics_caught_up; then
                echo "[wait] component metrics caught up and idle (stable for $((COMPONENT_STABLE_POLLS * 3))s)"
                break
            fi
        else
            stable_count=0
            prev_size="$curr_size"
        fi
    done
}

validate_l1_state() {
    echo "[validation] verifying L1 state and submitter progress ..."
    local sub_metrics="${METRICS_ROOT}/submitter_metrics.json"
    local l1_deploy="${METRICS_ROOT}/l1_deployment.json"
    
    if [[ ! -f "$sub_metrics" ]]; then
        echo "[validation] ERROR: submitter_metrics.json missing"
        return 1
    fi

    if [[ ! -f "$l1_deploy" ]]; then
        copy_l1_deployment
    fi
    if [[ ! -f "$l1_deploy" ]]; then
        echo "[validation] ERROR: l1_deployment.json missing"
        return 1
    fi

    python3 - "$sub_metrics" "$DA_MODE" <<'PYEOF'
import json
import sys

path, da_mode = sys.argv[1], sys.argv[2].lower()
rows = []
with open(path, "r", encoding="utf-8") as fh:
    for line_no, line in enumerate(fh, start=1):
        line = line.strip()
        if not line:
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise SystemExit(f"[validation] ERROR: invalid submitter JSON on line {line_no}: {exc}")

submitted = [row for row in rows if row.get("submission_status") == "submitted"]
if not submitted:
    raise SystemExit("[validation] ERROR: submitter confirmed/submitted zero batches")

missing_tx = [row.get("batch_id", "<unknown>") for row in submitted if not row.get("tx_hash")]
if missing_tx:
    raise SystemExit(f"[validation] ERROR: submitted rows missing tx_hash: {missing_tx[:5]}")

missing_gas = [
    row.get("batch_id", "<unknown>")
    for row in submitted
    if not isinstance(row.get("l1_gas_used"), int) or row.get("l1_gas_used", 0) <= 0
]
if missing_gas:
    raise SystemExit(f"[validation] ERROR: submitted rows missing positive l1_gas_used: {missing_gas[:5]}")

if da_mode == "blob":
    real_blob_rows = [
        row for row in submitted
        if row.get("real_eip4844_blob") is True
        and isinstance(row.get("measured_blob_gas_used"), int)
        and row.get("measured_blob_gas_used", 0) > 0
        and isinstance(row.get("blob_gas_price_wei"), int)
        and row.get("blob_gas_price_wei", 0) > 0
    ]
    hybrid_blob_rows = [
        row for row in submitted
        if row.get("real_eip4844_blob") is False
        and row.get("cost_source") == "hybrid"
        and row.get("blob_cost_source") == "estimated"
        and isinstance(row.get("estimated_blob_gas_used"), int)
        and row.get("estimated_blob_gas_used", 0) > 0
        and int(row.get("total_cost_wei", "0") or "0") > 0
        and int(row.get("cost_per_tx_wei", "0") or "0") > 0
    ]
    if real_blob_rows:
        print(f"[validation] [OK] blob mode has {len(real_blob_rows)} receipt-level EIP-4844 blob row(s)")
    elif hybrid_blob_rows:
        print("[validation] [OK] blob mode uses local hybrid cost model: measured regular gas plus estimated blob gas")
    else:
        sample = submitted[-1]
        raise SystemExit(
            "[validation] ERROR: blob mode lacks either real blob receipt fields or valid hybrid cost fields; "
            f"last_row={{cost_source:{sample.get('cost_source')}, "
            f"blob_cost_source:{sample.get('blob_cost_source')}, "
            f"real_eip4844_blob:{sample.get('real_eip4844_blob')}, "
            f"estimated_blob_gas_used:{sample.get('estimated_blob_gas_used')}}}"
        )

print(f"[validation] [OK] submitter submitted {len(submitted)} batch(es) with receipt gas")
PYEOF

    if [[ "$USED_DOCKER_STACK" == "1" ]] && command -v docker >/dev/null 2>&1; then
        echo "[validation] verifying Hardhat bridge state ..."
        (
            cd "$ROOT_DIR"
            docker compose --profile core run --rm --no-deps \
                -e DEPLOYMENT_FILE=/runtime/contracts.json \
                -e EXPECT_MIN_NEXT_BATCH_ID=2 \
                -e EXPECT_STATE_ROOT_CHANGED=1 \
                -e RUNTIME_VALIDATION_OUT=/runtime/l1_state_validation.json \
                contracts-deployer \
                npx hardhat run scripts/verify-runtime.ts --network host_docker
        )
        docker cp rollupx-full-zk-rollup-submitter-1:/runtime/l1_state_validation.json "${METRICS_ROOT}/l1_state_validation.json" >/dev/null 2>&1 || true
        echo "[validation] [OK] Hardhat bridge state advanced"
    else
        echo "[validation] WARNING: docker stack unavailable; skipped on-chain bridge state check"
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
        
        # Capture Docker memory stats
        echo "[diagnostics] collecting docker memory stats..."
        docker compose --profile core stats --no-stream > "${diag_dir}/docker_stats.txt" 2>&1 || true
    )
}

RESOURCE_SAMPLER_PID=""

start_resource_sampler() {
    if ! command -v docker >/dev/null 2>&1; then
        return
    fi
    local out_csv="${METRICS_ROOT}/resource_metrics_timeseries.csv"
    echo "timestamp_utc,container_name,container_id,cpu_pct,mem_usage_raw,mem_pct,net_io,block_io,pids" > "$out_csv"
    (
        while true; do
            ts="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
            docker compose --profile core stats --no-stream --format '{{.Name}},{{.ID}},{{.CPUPerc}},{{.MemUsage}},{{.MemPerc}},{{.NetIO}},{{.BlockIO}},{{.PIDs}}' \
                2>/dev/null | awk -v ts="$ts" 'BEGIN{FS=","; OFS=","} {print ts,$0}' >> "$out_csv" || true
            sleep 5
        done
    ) &
    RESOURCE_SAMPLER_PID=$!
    echo "[metrics] started resource sampler pid=${RESOURCE_SAMPLER_PID}"
}

stop_resource_sampler() {
    if [[ -n "${RESOURCE_SAMPLER_PID:-}" ]]; then
        kill "$RESOURCE_SAMPLER_PID" 2>/dev/null || true
        wait "$RESOURCE_SAMPLER_PID" 2>/dev/null || true
        RESOURCE_SAMPLER_PID=""
    fi
}

save_resource_metrics() {
    local metrics_file="${METRICS_ROOT}/resource_metrics.json"
    local timeseries_file="${METRICS_ROOT}/resource_metrics_timeseries.csv"
    python3 - "$timeseries_file" "$metrics_file" <<'PY'
import csv
import json
import re
import sys
from datetime import datetime, timezone

timeseries, out = sys.argv[1], sys.argv[2]
rows = []
try:
    with open(timeseries, "r", encoding="utf-8") as fh:
        rows = list(csv.DictReader(fh))
except FileNotFoundError:
    rows = []

def parse_pct(raw: str) -> float:
    try:
        return float((raw or "0").replace("%", "").strip())
    except Exception:
        return 0.0

def parse_mem_mb(mem_usage_raw: str) -> float:
    if not mem_usage_raw:
        return 0.0
    head = mem_usage_raw.split("/")[0].strip()
    m = re.match(r"([0-9]+(?:\.[0-9]+)?)\s*([KMG]i?)?B", head, flags=re.IGNORECASE)
    if not m:
        return 0.0
    value = float(m.group(1))
    unit = (m.group(2) or "").lower()
    if unit in ("ki", "k"):
        return value / 1024.0
    if unit in ("mi", "m"):
        return value
    if unit in ("gi", "g"):
        return value * 1024.0
    return value / (1024.0 * 1024.0)

max_memory_mb = 0.0
max_cpu_pct = 0.0
samples = 0
per_container_peak_memory_mb = {}
for row in rows:
    samples += 1
    name = row.get("container_name", "unknown")
    mem_mb = parse_mem_mb(row.get("mem_usage_raw", ""))
    cpu_pct = parse_pct(row.get("cpu_pct", ""))
    max_memory_mb = max(max_memory_mb, mem_mb)
    max_cpu_pct = max(max_cpu_pct, cpu_pct)
    if mem_mb > per_container_peak_memory_mb.get(name, 0.0):
        per_container_peak_memory_mb[name] = mem_mb

payload = {
    "timestamp": datetime.now(timezone.utc).isoformat(),
    "sample_count": samples,
    "timeseries_file": "resource_metrics_timeseries.csv",
    "max_memory_usage_mb": round(max_memory_mb, 3),
    "max_memory_usage_gb": round(max_memory_mb / 1024.0, 6),
    "max_cpu_pct": round(max_cpu_pct, 3),
    "per_container_peak_memory_mb": {
        k: round(v, 3) for k, v in sorted(per_container_peak_memory_mb.items())
    },
}
with open(out, "w", encoding="utf-8") as fh:
    json.dump(payload, fh, indent=2)
PY
    
    echo "[metrics] saved resource metrics → $metrics_file"
}

# ── traps — always clean up sequencer ─────────────────────────────────────────
SEQ_PID=""
cleanup() {
    stop_resource_sampler
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
mkdir -p "$RISC0_HOST_WORK_DIR"
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
echo "  da=$DA_MODE  prover=$PROVER  rate=${RATE_TPS}tps  mix=$TX_MIX  concurrency=$WORKLOAD_CONCURRENCY  target_txs=$WORKLOAD_TARGET_TXS  account_count=$WORKLOAD_ACCOUNT_COUNT"
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

export SEQUENCER_DEV_SEED_ADDRS="$(sender_addresses_for_count "$WORKLOAD_ACCOUNT_COUNT")"

# ── 4. (Re)start sequencer ────────────────────────────────────────────────────
# Adjust SEQUENCER_BIN to your actual binary path.
SEQUENCER_BIN=${SEQUENCER_BIN:-./target/release/sequencer}
USED_DOCKER_STACK=0

if should_use_docker_stack; then
    USED_DOCKER_STACK=1
    restart_docker_stack_for_run
    start_resource_sampler
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
echo "[workload] starting warmup phase ..."
if [[ "$WARMUP_S" -gt 0 ]]; then
    WARMUP_DIR="${METRICS_ROOT}/warmup"
    mkdir -p "$WARMUP_DIR"
    METRICS_ROOT="$WARMUP_DIR" python3 "${ROOT_DIR}/benchmark-suite/workload/poisson_generator.py" \
        --experiment_id "$EXP_ID" \
        --run_id        "${RUN_ID}_warmup" \
        --rate          "$RATE_TPS" \
        --duration      "$WARMUP_S" \
        --warmup        0 \
        --seed          "$SEED" \
        --tx_mix        "$TX_MIX" \
        --prover_backend "$PROVER" \
        --host          "$SEQ_HOST" \
        --port          "$SEQ_PORT" \
        --concurrency   "$WORKLOAD_CONCURRENCY" \
        --target_txs    0 \
        --account_count "$WORKLOAD_ACCOUNT_COUNT" \
        --phase         warmup \
        --start_nonce   0
    wait_for_component_metrics_flush
fi

WARMUP_TXS=0
if [[ -f "${METRICS_ROOT}/warmup/workload_${EXP_ID}.json" ]]; then
    WARMUP_TXS=$(python3 - "${METRICS_ROOT}/warmup/workload_${EXP_ID}.json" <<'PY'
import json
import sys
with open(sys.argv[1], "r", encoding="utf-8") as fh:
    payload = json.load(fh)
print(payload.get("details", {}).get("total_txs", 0))
PY
)
fi
echo "[workload] warmup complete (total_txs=${WARMUP_TXS}); resetting measured metric files"
rm -f "${METRICS_ROOT}/sequencer_batch_metrics.jsonl" \
      "${METRICS_ROOT}/executor_batch_metrics.jsonl" \
      "${METRICS_ROOT}/submitter_metrics.json" \
      "${METRICS_ROOT}/run_status.json" \
      "${METRICS_ROOT}/workload_${EXP_ID}.json" \
      "${METRICS_ROOT}/tx_log_${RUN_ID}.csv" \
      "${SHARED_METRICS_DIR}/sequencer_batch_metrics.jsonl" \
      "${SHARED_METRICS_DIR}/executor_batch_metrics.jsonl" \
      "${SHARED_METRICS_DIR}/submitter_metrics.json"

echo "[workload] starting measured phase ..."
python3 "${ROOT_DIR}/benchmark-suite/workload/poisson_generator.py" \
    --experiment_id "$EXP_ID" \
    --run_id        "$RUN_ID" \
    --rate          "$RATE_TPS" \
    --duration      "$DURATION_S" \
    --warmup        0 \
    --seed          "$SEED" \
    --tx_mix        "$TX_MIX" \
    --prover_backend "$PROVER" \
    --host          "$SEQ_HOST" \
    --port          "$SEQ_PORT" \
    --concurrency   "$WORKLOAD_CONCURRENCY" \
    --target_txs    "$WORKLOAD_TARGET_TXS" \
    --account_count "$WORKLOAD_ACCOUNT_COUNT" \
    --phase         measured \
    --start_nonce   "$WARMUP_TXS"

# ── 6. Wait for submitter to flush final batch ────────────────────────────────
# Poll component metrics until executor/submitter have caught up and files stop growing.
wait_for_component_metrics_flush

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
validate_l1_state

# ── 7. Update timestamp_end in metadata ───────────────────────────────────────
END_TS=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
METADATA_FILE="$METRICS_ROOT/run_metadata.json"
if command -v jq &>/dev/null && [[ -f "$METADATA_FILE" ]]; then
    tmp=$(mktemp)
    jq --arg ts "$END_TS" '.timestamp_end = $ts' "$METADATA_FILE" > "$tmp" && mv "$tmp" "$METADATA_FILE"
fi

# ── 7b. Collect resource metrics ───────────────────────────────────────────────
stop_resource_sampler
save_resource_metrics

# ── 8. Generate analysis report ────────────────────────────────────────────────
if command -v python3 &>/dev/null; then
    echo "[report] generating analysis report ..."
    python3 "$(dirname "$0")/generate_analysis_report.py" "$METRICS_ROOT"
else
    echo "[report] WARNING: python3 not found; skipping analysis report generation"
fi

echo "[DONE] $RUN_ID  →  $METRICS_ROOT"
