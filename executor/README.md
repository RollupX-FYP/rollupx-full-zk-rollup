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
- `REQUIRE_REAL_PROOFS=1` (Strictly enforce Groth16 proofs)
- `ALLOW_UNSIGNED_USER_TXS=1` (For synthetic research experiments)

---

## Observability & Metrics

The executor tracks granular performance data for state transition execution and zero-knowledge proving.

### Metrics Storage

Metrics are persisted to the `METRICS_ROOT` directory:
- **`executor_batch_metrics.jsonl`**: Per-batch breakdown of execution phases and prover metadata.
- **`executor_{EXPERIMENT_ID}.json`**: Aggregate session stats including Mean/Min/Max latencies.

### Execution Phase Breakdown

Every batch execution times the following phases (recorded in `ms`):
- `signature_verify_ms`: ECDSA recovery and address matching.
- `nonce_balance_check_ms`: State lookups and validity rules.
- `state_transition_ms`: Balance arithmetic and nonce increments.
- `merkle_update_ms`: Sparse Merkle Tree (SMT) path updates and root recomputation.
- `state_diff_computation_ms`: Serialization of state changes for DA.

### Prover Metrics

If `PROVER_BACKEND=risc0` is used, the executor records:
- `total_prover_wall_ms`: End-to-end latency of the RISC0 host.
- `proof_mode`: `groth16` (real) or `fake` (mock).
- `proof_bytes`: Size of the resulting SNARK artifact.

---

## Benchmarking Suite Integration

The executor is a core target for the `RollupX Benchmark Suite`.

1. **State Isolation**: The runner (`run_matrix.sh`) ensures `STATE_DB_PATH` is unique or cleared between experiment variants to avoid cache poisoning.
2. **Prover Enforcement**: The `REQUIRE_REAL_PROOFS` flag is used during "Production Baseline" experiments to ensure timing results reflect actual ZK overhead.
3. **Trace Verification**: The suite uses the `TRACE_ROOT/index.jsonl` to verify that every batch submitted to L1 has a corresponding, verified execution trace.

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
