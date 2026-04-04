#!/usr/bin/env bash
# wait_for_sequencer.sh — poll /health until sequencer is ready or timeout
# Usage: bash wait_for_sequencer.sh [host] [port] [max_retries]

set -euo pipefail

HOST=${1:-${SEQ_HOST:-localhost}}
PORT=${2:-${SEQ_PORT:-3000}}
MAX=${3:-30}

echo "[wait_for_sequencer] polling http://$HOST:$PORT/health (max ${MAX} attempts)"

for i in $(seq 1 "$MAX"); do
    if curl -sf --max-time 2 "http://$HOST:$PORT/health" > /dev/null 2>&1; then
        echo "[wait_for_sequencer] ready after ${i} attempt(s)"
        exit 0
    fi
    echo "  attempt $i/$MAX — not ready yet, sleeping 2s ..."
    sleep 2
done

echo "[wait_for_sequencer] ERROR: sequencer not ready after $((MAX * 2))s" >&2
exit 1
