# Executor

Active executor crate: `zksync_state_machine` in `executor/src`.

## What It Does

The executor is not a pass-through relay. In gRPC mode it performs:

1. batch transaction normalization,
2. STF execution against local state,
3. trace persistence + hash verification,
4. RISC0 proof artifact generation,
5. enriched batch publication for submitter consumption.

Implementation entrypoint:
- `executor/src/main.rs` -> `service::run_server_from_env()`

## Runtime Environment

Required:
- `PROVER_BACKEND=risc0`
- `RISC0_HOST_BIN=<abs-path-to-rollup_host>`

Common:
- `EXECUTOR_GRPC_ADDR=127.0.0.1:50051`
- `TRACE_ROOT=executor/tmp/traces`
- `STATE_DB_PATH=executor/tmp/state_db`
- `RISC0_WORK_DIR=executor/tmp/risc0` (optional)

## Run

```bash
PROVER_BACKEND=risc0 \
RISC0_HOST_BIN=/abs/path/to/risc0_prover/target/debug/rollup_host \
EXECUTOR_GRPC_ADDR=127.0.0.1:50051 \
TRACE_ROOT=/abs/path/to/executor/tmp/traces \
STATE_DB_PATH=/abs/path/to/executor/tmp/state_db \
/abs/path/to/executor/target/debug/zksync_state_machine
```

## Tests

```bash
CXXFLAGS='-include cstdint' cargo +nightly-2025-03-19 test \
  --manifest-path executor/Cargo.toml \
  -p zksync_state_machine \
  --ignore-rust-version
```

## Notes

- Legacy files (`src/bridge.rs`, `src/executor.rs`, `src/grpc.rs`) were removed.
- Prover crate was moved to repo root: `risc0_prover/`.
