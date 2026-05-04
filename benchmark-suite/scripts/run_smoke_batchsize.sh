#!/usr/bin/env bash
set -euo pipefail

# 1 repeat, 30s duration, batch-size factor only
export DURATION_S=${DURATION_S:-30}
export WARMUP_S=${WARMUP_S:-5}

bash scripts/run_matrix.sh --filter batch_size
