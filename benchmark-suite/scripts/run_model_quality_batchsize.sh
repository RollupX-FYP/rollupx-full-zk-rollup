#!/usr/bin/env bash
set -euo pipefail

# 30 repeats, 120s duration
export REPEATS_OVERRIDE=${REPEATS_OVERRIDE:-30}
export DURATION_S_OVERRIDE=${DURATION_S_OVERRIDE:-120}
export WARMUP_S_OVERRIDE=${WARMUP_S_OVERRIDE:-15}

bash scripts/run_matrix.sh --filter batch_size
