#!/usr/bin/env bash
# scripts/verify_artifacts.sh
# Verifies that the expected artifacts are generated after a benchmark run.

set -euo pipefail

EXP_ID=${1:-smoke_test}
RUN_ID="${EXP_ID}_r01"
# The docker volume "metrics_data" is mapped. Since we can't easily read docker named volumes from the host without docker run,
# we use a temporary alpine container attached to the volume.
echo "Verifying benchmark artifacts for $EXP_ID in metrics volume..."

ARTIFACTS=(
    "metrics/$EXP_ID/$RUN_ID/workload_$EXP_ID.json"
    "metrics/$EXP_ID/$RUN_ID/tx_log_$RUN_ID.csv"
    "metrics/$EXP_ID/$RUN_ID/run_status.json"
)

MISSING=0

for artifact in "${ARTIFACTS[@]}"; do
    if docker run --rm -v rollupx-full-zk-rollup_metrics_data:/var/lib/rollupx/metrics alpine test -f "/var/lib/rollupx/metrics/$artifact"; then
        echo " [OK] Found $artifact"
    else
        # Try checking if there's a different volume name or prefix
        # docker-compose prepends directory name to volumes by default.
        # Fallback check assuming the volume is just named metrics_data or we can just use `docker compose run`
        if docker compose --profile bench run --rm benchmark test -f "/var/lib/rollupx/metrics/$artifact" 2>/dev/null; then
            echo " [OK] Found $artifact"
        else
            echo " [FAIL] Missing $artifact"
            MISSING=$((MISSING+1))
        fi
    fi
done

if [ $MISSING -eq 0 ]; then
    echo "All expected benchmark artifacts are present."
else
    echo "Missing $MISSING expected artifacts."
    exit 1
fi
