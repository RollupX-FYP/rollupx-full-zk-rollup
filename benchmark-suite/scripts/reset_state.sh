#!/usr/bin/env bash
# reset_state.sh — clear local runtime artifacts before a controlled experiment run.
#
# Intended for local benchmarking reproducibility. Removes run-time state only.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
RUN_ID="${1:-}"

cd "$ROOT_DIR"

echo "[reset_state] clearing local runtime artifacts..."

# Sequencer DBs
rm -f sequencer/sequencer.db sequencer/sequencer.db-shm sequencer/sequencer.db-wal
if [[ -n "$RUN_ID" ]]; then
  rm -f "sequencer/sequencer_${RUN_ID}.db" "sequencer/sequencer_${RUN_ID}.db-shm" "sequencer/sequencer_${RUN_ID}.db-wal"
fi

# Executor runtime data
rm -rf executor/tmp/*
rm -rf tmp/*

# Submitter runtime data
rm -f submitter/outbox.db submitter/outbox.db-shm submitter/outbox.db-wal
rm -f submitter/data/submitter.db submitter/data/submitter.db-shm submitter/data/submitter.db-wal
rm -rf submitter/offchain_store/*
rm -rf offchain_store/*

echo "[reset_state] done"
