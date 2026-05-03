#!/usr/bin/env bash
# scripts/run_full_suite.sh
#
# This script automates the entire benchmark execution process and saves all
# outputs (raw metrics, logs, aggregated reports, and figures) into a newly
# created, timestamped folder on the host machine.
#
# Usage:
#   bash scripts/run_full_suite.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── 1. Create a unique folder for this round of experiments ──────────────────
TIMESTAMP=$(TZ="Asia/Colombo" date +%Y%m%d_%H%M%S)
SESSION_DIR="${PROJECT_ROOT}/benchmark-suite/metrics/run_${TIMESTAMP}"
mkdir -p "$SESSION_DIR"

# Export METRICS_DIR so docker-compose.yml uses it for the bind mount
export METRICS_DIR="$SESSION_DIR"

echo "======================================================================"
echo " Starting Full Benchmark Suite"
echo " All results will be saved to:"
echo "   $METRICS_DIR"
echo "======================================================================"
echo ""

cd "$PROJECT_ROOT"

# ── 2. Build images ──────────────────────────────────────────────────────────
echo "[step 1/4] Building benchmark image..."
docker compose --profile bench build benchmark --no-cache

# ── 3. Run workload experiments ──────────────────────────────────────────────
echo ""
echo "[step 2/4] Running workload experiments (rate, tx_mix)..."
docker compose --profile core --profile bench run -T --rm benchmark bash scripts/run_matrix.sh --workload

# ── 4. Run infrastructure experiments ────────────────────────────────────────
echo ""
echo "[step 3/4] Running infrastructure experiments (batch_size, policy, etc.)..."
bash scripts/run_infra_matrix.sh

# ── 5. Generate analytics reports ────────────────────────────────────────────
echo ""
echo "[step 4/4] Generating analytics reports..."
docker compose --profile report build data-tools --no-cache
docker compose --profile report run -T --rm data-tools

echo ""
echo "======================================================================"
echo " ✅ Full suite completed successfully!"
echo " All raw metrics, stats_summary.csv, and figures/ have been saved to:"
echo "   $METRICS_DIR"
echo "======================================================================"
