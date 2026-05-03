#!/usr/bin/env bash
# scripts/run_infra_matrix.sh — Automated infrastructure factor experiments.
#
# This script runs ON THE HOST (not inside a container). It automates the
# tear-down → reconfigure → restart → benchmark cycle for infrastructure
# factors that require restarting the Docker Compose stack.
#
# Infrastructure factors: batch_size, timeout, policy, da_mode, prover
# (Workload factors like rate_tps and tx_mix are handled by run_matrix.sh
#  inside the benchmark container and do NOT need this script.)
#
# Usage:
#   bash scripts/run_infra_matrix.sh [--dry-run] [--filter <factor>] [--skip-baseline]
#
# Options:
#   --dry-run         Print commands without executing
#   --filter <f>      Only run experiments where factor == <f>
#                     e.g. --filter batch_size, --filter policy
#   --skip-baseline   Skip the baseline run (useful for resuming)
#
# Prerequisites:
#   - Docker Compose must be available on the host
#   - Python 3.11+ must be available on the host (for tomllib)
#   - The benchmark image should already be built:
#       docker compose --profile bench build benchmark --no-cache

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

DRY_RUN=false
FILTER=""
SKIP_BASELINE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)        DRY_RUN=true; shift ;;
        --filter)         FILTER="$2"; shift 2 ;;
        --skip-baseline)  SKIP_BASELINE=true; shift ;;
        *) echo "Unknown flag: $1"; exit 1 ;;
    esac
done

# ── Auto-setup METRICS_DIR if not provided ──────────────────────────────────────
if [[ -z "${METRICS_DIR:-}" ]]; then
    export METRICS_DIR="${PROJECT_ROOT}/benchmark-suite/metrics/run_$(date +%Y%m%d_%H%M%S)"
    mkdir -p "$METRICS_DIR"
    echo "======================================================================"
    echo " No METRICS_DIR provided. Creating new session directory:"
    echo "   $METRICS_DIR"
    echo "======================================================================"
    echo ""
fi

# ── Infra factors that require a stack restart ────────────────────────────────
INFRA_FACTORS="batch_size timeout policy da_mode prover"

echo ""
echo "======================================================================"
echo "  RollupX Infrastructure Factor Matrix"
echo "  Host-side orchestrator (tear-down → reconfigure → restart → run)"
echo "======================================================================"
echo ""

# ── Parse experiments.toml using Python ───────────────────────────────────────
EXPERIMENTS_JSON=$(python3 - "$FILTER" "$SKIP_BASELINE" <<'PYEOF'
import sys, json, tomllib

filter_factor  = sys.argv[1] if sys.argv[1] else None
skip_baseline  = sys.argv[2] == "True"

infra_factors = {"batch_size", "timeout", "policy", "da_mode", "prover"}

with open("benchmark-suite/config/experiments.toml", "rb") as f:
    cfg = tomllib.load(f)

baseline = cfg["baseline"]
seeds    = baseline["seeds"]
repeats  = baseline["repeats"]

runs = []

# Baseline (uses default env vars)
if not skip_baseline:
    runs.append({
        "id":           "baseline",
        "factor":       "baseline",
        "batch_size":   baseline["batch_size"],
        "timeout_ms":   baseline["timeout_ms"],
        "policy":       baseline["policy"],
        "da_mode":      baseline["da_mode"],
        "prover":       baseline["prover"],
        "rate_tps":     baseline["rate_tps"],
        "duration_s":   baseline["duration_s"],
        "warmup_s":     baseline.get("warmup_s", 15),
        "tx_mix":       baseline["tx_mix"],
        "seeds":        seeds[:repeats],
        "repeats":      repeats,
    })

# Only infrastructure factor experiments
for exp in cfg["experiments"]:
    factor = exp.get("factor", "")
    if factor not in infra_factors:
        continue
    if filter_factor and factor != filter_factor:
        continue
    merged = {**baseline, **exp}
    runs.append({
        "id":           merged["id"],
        "factor":       factor,
        "batch_size":   merged["batch_size"],
        "timeout_ms":   merged["timeout_ms"],
        "policy":       merged["policy"],
        "da_mode":      merged["da_mode"],
        "prover":       merged["prover"],
        "rate_tps":     merged["rate_tps"],
        "duration_s":   merged["duration_s"],
        "warmup_s":     merged.get("warmup_s", 15),
        "tx_mix":       merged["tx_mix"],
        "seeds":        seeds[:repeats],
        "repeats":      repeats,
    })

print(json.dumps(runs))
PYEOF
)

TOTAL_EXPERIMENTS=$(echo "$EXPERIMENTS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d))")
echo "[matrix] Found $TOTAL_EXPERIMENTS infrastructure experiment(s) to run"
echo ""

if [[ "$TOTAL_EXPERIMENTS" -eq 0 ]]; then
    echo "[matrix] Nothing to run. Check your --filter flag."
    exit 0
fi

# ── Map experiment factors to Docker Compose environment variables ────────────
# This function takes a JSON experiment object and exports the right env vars.
run_single_experiment() {
    local EXP_JSON="$1"
    local EXP_INDEX="$2"

    local EXP_ID=$(echo "$EXP_JSON"    | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['id'])")
    local FACTOR=$(echo "$EXP_JSON"     | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['factor'])")
    local BATCH=$(echo "$EXP_JSON"      | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['batch_size'])")
    local TIMEOUT=$(echo "$EXP_JSON"    | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['timeout_ms'])")
    local POLICY=$(echo "$EXP_JSON"     | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['policy'])")
    local DA_MODE=$(echo "$EXP_JSON"    | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['da_mode'])")
    local PROVER=$(echo "$EXP_JSON"     | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['prover'])")
    local RATE=$(echo "$EXP_JSON"       | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['rate_tps'])")
    local DURATION=$(echo "$EXP_JSON"   | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['duration_s'])")
    local WARMUP=$(echo "$EXP_JSON"     | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['warmup_s'])")
    local TX_MIX=$(echo "$EXP_JSON"     | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['tx_mix'])")
    local REPEATS=$(echo "$EXP_JSON"    | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['repeats'])")
    local SEEDS_JSON=$(echo "$EXP_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(json.dumps(d['seeds']))")

    echo ""
    echo "======================================================================"
    echo "  [$EXP_INDEX/$TOTAL_EXPERIMENTS] Experiment: $EXP_ID  (factor: $FACTOR)"
    echo "  batch=$BATCH  timeout=${TIMEOUT}ms  policy=$POLICY"
    echo "  da=$DA_MODE  prover=$PROVER  rate=${RATE}tps  mix=$TX_MIX"
    echo "======================================================================"

    if [[ "$DRY_RUN" == "true" ]]; then
        echo "  [DRY-RUN] Would tear down stack, restart with:"
        echo "    SEQUENCER_BATCH_MAX_SIZE=$BATCH"
        echo "    SEQUENCER_BATCH_TIMEOUT_MS=$TIMEOUT"
        echo "    SEQUENCER_POLICY=$POLICY"
        echo "    SUBMITTER_DA_MODE=$DA_MODE"
        echo "    SUBMITTER_PROOF_BACKEND=$PROVER"
        echo "  [DRY-RUN] Then run $REPEATS repeat(s) of $EXP_ID"
        return 0
    fi

    # ── Step 1: Tear down existing stack (keep volumes for metrics) ───────
    echo "[step 1/4] Tearing down existing stack..."
    docker compose down --remove-orphans 2>/dev/null || true
    # Remove per-service volumes but keep metrics_data
    docker volume rm -f \
        "$(docker volume ls -q | grep sequencer_db)" \
        "$(docker volume ls -q | grep executor_state)" \
        "$(docker volume ls -q | grep executor_traces)" \
        "$(docker volume ls -q | grep executor_risc0)" \
        "$(docker volume ls -q | grep submitter_data)" \
        "$(docker volume ls -q | grep runtime_config)" \
        2>/dev/null || true

    # ── Step 2: Restart core stack with infrastructure env vars ───────────
    echo "[step 2/4] Starting core stack with infra config..."
    export SEQUENCER_BATCH_MAX_SIZE="$BATCH"
    export SEQUENCER_BATCH_TIMEOUT_MS="$TIMEOUT"
    export SEQUENCER_POLICY="$POLICY"
    export SUBMITTER_DA_MODE="$DA_MODE"
    export SUBMITTER_PROOF_BACKEND="$PROVER"

    docker compose --profile core up -d --build

    # ── Step 3: Wait for stack health ─────────────────────────────────────
    echo "[step 3/4] Waiting for stack to become healthy..."
    # Wait for Docker's own healthchecks (up to 3 minutes)
    local MAX_WAIT=180
    local ELAPSED=0
    while [[ $ELAPSED -lt $MAX_WAIT ]]; do
        if docker compose ps --format json 2>/dev/null | python3 -c "
import sys, json
lines = sys.stdin.read().strip().split('\n')
services = [json.loads(l) for l in lines if l.strip()]
core = [s for s in services if s.get('Service') in ('hardhat','sequencer','executor','submitter')]
if not core:
    sys.exit(1)
unhealthy = [s for s in core if s.get('Health','') != 'healthy']
sys.exit(0 if not unhealthy else 1)
" 2>/dev/null; then
            echo "  All core services healthy."
            break
        fi
        sleep 5
        ELAPSED=$((ELAPSED + 5))
        echo "  Waiting... (${ELAPSED}s / ${MAX_WAIT}s)"
    done

    if [[ $ELAPSED -ge $MAX_WAIT ]]; then
        echo "  [FAIL] Stack did not become healthy within ${MAX_WAIT}s. Skipping $EXP_ID."
        return 1
    fi

    # ── Step 4: Run benchmark repeats ─────────────────────────────────────
    echo "[step 4/4] Running $REPEATS repeat(s)..."
    local FAILED=0
    for i in $(seq 1 "$REPEATS"); do
        local SEED=$(echo "$SEEDS_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin)[$((i-1))])")
        local RUN_ID="${EXP_ID}_r$(printf '%02d' "$i")"
        echo ""
        echo "  -- Run $i/$REPEATS  seed=$SEED  run_id=$RUN_ID"

        docker compose --profile bench run -T --rm \
            -e RATE_TPS="$RATE" \
            -e DURATION_S="$DURATION" \
            -e WARMUP_S="$WARMUP" \
            -e TX_MIX="$TX_MIX" \
            -e SEED="$SEED" \
            -e RUN_ID="$RUN_ID" \
            -e EXPERIMENT_ID="$EXP_ID" \
            -e MAX_BATCH_SIZE="$BATCH" \
            -e TIMEOUT_MS="$TIMEOUT" \
            -e POLICY="$POLICY" \
            -e DA_MODE="$DA_MODE" \
            -e PROVER="$PROVER" \
            benchmark bash /app/scripts/run_experiment.sh "$EXP_ID" "$i" \
            || { echo "  [FAIL] $RUN_ID"; FAILED=$((FAILED + 1)); }
    done

    if [[ $FAILED -gt 0 ]]; then
        echo "  [$EXP_ID] $FAILED/$REPEATS runs failed."
        return 1
    fi
    echo "  [$EXP_ID] All $REPEATS runs completed."
    return 0
}

# ── Main loop: iterate over each infrastructure experiment ────────────────────
FAILURES=0
INDEX=0

while read -r ROW; do
    if [[ -z "$ROW" ]]; then continue; fi
    INDEX=$((INDEX + 1))
    run_single_experiment "$ROW" "$INDEX" || FAILURES=$((FAILURES + 1))
done <<< "$(echo "$EXPERIMENTS_JSON" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for item in data:
    print(json.dumps(item))
")"

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "======================================================================"
if [[ $FAILURES -gt 0 ]]; then
    echo "[INFRA-MATRIX] Completed with $FAILURES failure(s) out of $TOTAL_EXPERIMENTS experiments."
    exit 1
else
    echo "[INFRA-MATRIX] All $TOTAL_EXPERIMENTS experiments completed successfully."
fi
echo "======================================================================"
