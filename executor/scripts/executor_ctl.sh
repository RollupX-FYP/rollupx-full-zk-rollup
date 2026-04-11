#!/usr/bin/env bash
set -euo pipefail

export CXXFLAGS="${CXXFLAGS:--include cstdint}"

usage() {
  cat <<'EOF'
Usage:
  executor/scripts/executor_ctl.sh <command>

Commands:
  test              Run executor package tests (zksync_state_machine)
  run               Run zksync_state_machine binary
  verify            Same as test

Environment:
  CXXFLAGS          C++ flags for rocksdb builds (default: -include cstdint)
EOF
}

EXECUTOR_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

test_executor() {
  cargo +nightly-2025-03-19 test \
    --manifest-path "${EXECUTOR_ROOT}/Cargo.toml" \
    -p zksync_state_machine \
    --all-features \
    --ignore-rust-version \
    -- --nocapture
}

run_executor() {
  cargo +nightly-2025-03-19 run \
    --manifest-path "${EXECUTOR_ROOT}/src/Cargo.toml" \
    --bin zksync_state_machine \
    --ignore-rust-version
}

cmd="${1:-}"
case "$cmd" in
  test)
    test_executor
    ;;
  run)
    run_executor
    ;;
  verify|all)
    test_executor
    run_executor
    ;;
  ""|-h|--help|help)
    usage
    ;;
  *)
    echo "error: unknown command '$cmd'" >&2
    usage
    exit 1
    ;;
esac
