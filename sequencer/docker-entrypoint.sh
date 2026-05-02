#!/bin/sh
set -eu

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    echo "RollupX sequencer container"
    echo "Starts /usr/local/bin/sequencer with /app/config/default.toml."
    exit 0
fi

exec /usr/local/bin/sequencer "$@"
