#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Auto-setup METRICS_DIR if not provided ──────────────────────────────────────
if [[ -z "${METRICS_DIR:-}" ]]; then
    export METRICS_DIR="${PROJECT_ROOT}/benchmark-suite/metrics/run_$(TZ="Asia/Colombo" date +%Y%m%d_%H%M%S)"
    mkdir -p "$METRICS_DIR"
    echo "======================================================================"
    echo " No METRICS_DIR provided. Creating new session directory:"
    echo "   $METRICS_DIR"
    echo "======================================================================"
    echo ""
fi

cd "$PROJECT_ROOT"
docker compose --profile core --profile bench run -T --rm benchmark bash scripts/run_matrix.sh "$@"
