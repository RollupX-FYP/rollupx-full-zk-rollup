#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Auto-setup METRICS_DIR to the most recent run if not provided ───────────────
if [[ -z "${METRICS_DIR:-}" ]]; then
    MOST_RECENT=$(ls -td "${PROJECT_ROOT}/benchmark-suite/metrics/run_"* 2>/dev/null | head -n 1 || true)
    if [[ -n "$MOST_RECENT" ]]; then
        export METRICS_DIR="$MOST_RECENT"
        echo "======================================================================"
        echo " No METRICS_DIR provided. Using the most recent session directory:"
        echo "   $METRICS_DIR"
        echo "======================================================================"
        echo ""
    else
        echo "Error: No METRICS_DIR provided and no previous run directories found."
        exit 1
    fi
fi

cd "$PROJECT_ROOT"
docker compose --profile report build data-tools --no-cache
docker compose --profile report run -T --rm data-tools
