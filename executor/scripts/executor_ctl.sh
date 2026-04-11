#!/usr/bin/env bash
set -euo pipefail

EXECUTOR_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ERA_ROOT="${ZKSYNC_ERA_ROOT:-${EXECUTOR_ROOT}/../zksync-era}"

export CXXFLAGS="${CXXFLAGS:--include cstdint}"

usage() {
  cat <<'EOF'
Usage:
  executor/scripts/executor_ctl.sh <command>

Commands:
  build-contracts   Compile and sync system contracts from zksync-era
  test              Run executor package tests (zksync_state_machine)
  run               Run zksync_state_machine binary
  verify            build-contracts + test
  all               build-contracts + test + run

Environment:
  ZKSYNC_ERA_ROOT   Path to zksync-era checkout (default: ../zksync-era)
  CXXFLAGS          C++ flags for rocksdb builds (default: -include cstdint)
EOF
}

build_contracts() {
  if [[ ! -d "${ERA_ROOT}/contracts/system-contracts" ]]; then
    echo "error: missing zksync-era contracts at ${ERA_ROOT}/contracts/system-contracts" >&2
    exit 1
  fi

  pushd "${ERA_ROOT}/contracts/system-contracts" >/dev/null

  echo "[executor] compiling Solidity contracts"
  yarn hardhat compile

  echo "[executor] compiling Yul precompiles"
  yarn compile-yul compile-precompiles

  echo "[executor] building bootloader"
  yarn build:bootloader

  popd >/dev/null

  local dst="${EXECUTOR_ROOT}/contracts/system-contracts"
  mkdir -p "${dst}/zkout" "${dst}/artifacts-zk" "${dst}/contracts-preprocessed" "${dst}/bootloader/build" "${dst}/bootloader/tests"

  echo "[executor] syncing contract artifacts"
  rsync -a "${ERA_ROOT}/contracts/system-contracts/zkout/" "${dst}/zkout/"
  rsync -a "${ERA_ROOT}/contracts/system-contracts/artifacts-zk/" "${dst}/artifacts-zk/"
  rsync -a "${ERA_ROOT}/contracts/system-contracts/contracts-preprocessed/" "${dst}/contracts-preprocessed/"
  rsync -a "${ERA_ROOT}/contracts/system-contracts/bootloader/build/" "${dst}/bootloader/build/"
  rsync -a "${ERA_ROOT}/contracts/system-contracts/bootloader/tests/" "${dst}/bootloader/tests/"

  mkdir -p "${dst}/contracts-preprocessed/bootloaderartifacts"
  rsync -a "${dst}/bootloader/build/artifacts/" "${dst}/contracts-preprocessed/bootloaderartifacts/"

  for d in "${dst}/contracts-preprocessed/bootloaderartifacts"/*.yul; do
    [[ -d "$d" ]] || continue
    n="$(basename "$d" .yul)"
    if [[ -f "$d/Bootloader.zbin" && ! -f "$d/${n}.yul.zbin" ]]; then
      cp "$d/Bootloader.zbin" "$d/${n}.yul.zbin"
    fi
  done

  mkdir -p "${EXECUTOR_ROOT}/contracts/l1-contracts/zkout"
  rsync -a "${ERA_ROOT}/contracts/l1-contracts/zkout/" "${EXECUTOR_ROOT}/contracts/l1-contracts/zkout/"
}

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
    --bin zksync_state_machine
}

cmd="${1:-}"
case "$cmd" in
  build-contracts)
    build_contracts
    ;;
  test)
    test_executor
    ;;
  run)
    run_executor
    ;;
  verify)
    build_contracts
    test_executor
    ;;
  all)
    build_contracts
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
