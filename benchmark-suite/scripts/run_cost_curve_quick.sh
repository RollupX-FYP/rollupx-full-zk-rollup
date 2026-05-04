#!/usr/bin/env bash
set -euo pipefail

# Quick cost-curve run for the economic-sealer report.
# Designed for a ~30 minute window by using one repeat and high enough traffic
# to fill larger batches more often than the default 10 TPS feasibility run.

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT_DIR/benchmark-suite"

export REPEATS_OVERRIDE=${REPEATS_OVERRIDE:-1}
export DURATION_S_OVERRIDE=${DURATION_S_OVERRIDE:-45}
export WARMUP_S_OVERRIDE=${WARMUP_S_OVERRIDE:-5}
export RATE_TPS_OVERRIDE=${RATE_TPS_OVERRIDE:-80}
export SUBMITTER_WAIT_MAX=${SUBMITTER_WAIT_MAX:-80}
export DOCKER_UP_BUILD=${DOCKER_UP_BUILD:-0}
export RUN_BATCH_SIZES=${RUN_BATCH_SIZES:-"25 50 100 250 500"}

index=0
for batch_size in $RUN_BATCH_SIZES; do
  exp_id="$(printf 'exp_cc_%03d_batch_size_bs%03d_calldata_balanced_%stps' "$index" "$batch_size" "$RATE_TPS_OVERRIDE")"
  export MAX_BATCH_SIZE="$batch_size"
  export TIMEOUT_MS="${TIMEOUT_MS_OVERRIDE:-30000}"
  export MIN_BATCH_SIZE="${MIN_BATCH_SIZE_OVERRIDE:-1}"
  export POLICY="${POLICY_OVERRIDE:-FCFS}"
  export DA_MODE="${DA_MODE_OVERRIDE:-calldata}"
  export PROVER="${PROVER_OVERRIDE:-groth16}"
  export RATE_TPS="$RATE_TPS_OVERRIDE"
  export DURATION_S="$DURATION_S_OVERRIDE"
  export WARMUP_S="$WARMUP_S_OVERRIDE"
  export TX_MIX="${TX_MIX_OVERRIDE:-balanced}"
  export SEED="${SEED_OVERRIDE:-42}"
  export EXPERIMENT_NAME="$exp_id"

  echo
  echo "======================================================================"
  echo "  COST CURVE ${index}: batch_size=${batch_size}"
  echo "======================================================================"
  bash scripts/run_experiment.sh "$exp_id" 1
  index=$((index + 1))
done

python3 scripts/analyze_cost_curve.py metrics \
  --out metrics/cost_curve_quick_analysis \
  --prover-hour-usd "${PROVER_HOUR_USD:-30.0}" \
  --l1-gas-gwei "${L1_GAS_GWEI:-25}" \
  --eth-usd "${ETH_USD:-3000}" \
  --calibrated-prover-ms-per-tx "${CALIBRATED_PROVER_MS_PER_TX:-250}" \
  --calibrated-prover-quadratic-ms "${CALIBRATED_PROVER_QUADRATIC_MS:-0.2}"

echo
echo "[DONE] cost curve analysis:"
echo "  metrics/cost_curve_quick_analysis/cost_curve_report.md"
echo "  metrics/cost_curve_quick_analysis/batch_size_cost_summary.csv"
echo "  metrics/cost_curve_quick_analysis/figures/"
