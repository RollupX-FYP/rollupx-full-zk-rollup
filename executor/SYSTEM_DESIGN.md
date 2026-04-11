# Executor System Design (Current)

This document describes the active executor package hosted in [src](src) and wired as `zksync_state_machine` in [Cargo.toml](Cargo.toml).

## Scope

The executor is responsible for:
1. Loading a sequencer batch input.
2. Executing transactions in EraVM-compatible mode.
3. Falling back to tolerant synthetic execution in research mode when strict VM preconditions are not met.
4. Producing Merkle root and witness-ready output.

## High-Level Flow

```text
sequencer batch_output.json
        |
        v
bridge::run_from_env
  - parse tx list
  - build envs
  - load artifacts
        |
        v
BatchProcessor
  - StateMachine::init
  - execute txs
  - seal batch
        |
        v
TreeProcessor
  - apply storage logs
  - produce root + witness
        |
        v
executor_prover_output.json (+ optional compatibility output)
```

## Inputs

1. Sequencer JSON file:
- [batch_output.json](../batch_output.json)

2. Contract artifacts:
- [contracts/system-contracts](contracts/system-contracts)
- [contracts/l1-contracts/zkout](contracts/l1-contracts/zkout)

3. Runtime env from bridge mode:
- `SEQUENCER_OUTPUT`
- `EXECUTOR_OUTPUT`
- `EXECUTOR_PROVER_OUTPUT`
- `EXECUTOR_DB_PATH`

## Outputs

1. Prover payload JSON with:
- `batch_id`
- `root_hash`
- `pubdata`
- `witness`
- `storage_log_count`

2. Compatibility batch output JSON (legacy shape for downstream tooling).

## Components

1. [src/bridge.rs](src/bridge.rs)
- Ingests sequencer output.
- Builds `BatchInput`.
- Writes final output artifacts.

2. [src/executor.rs](src/executor.rs)
- `BatchProcessor`: orchestrates execution + tree processing.
- `StateMachine`: VM wrapper + tolerant fallback.
- `TreeProcessor`: applies storage logs to Merkle tree.

3. [src/lib.rs](src/lib.rs)
- Shared input/output structs (`BatchInput`, `BatchOutput`).

4. [src/main.rs](src/main.rs)
- Entrypoint; calls `bridge::run_from_env`.

## Execution Modes

1. `StrictEra`
- Fails fast if VM preconditions are not satisfied.

2. `TolerantResearch`
- Uses synthetic fallback for compatibility in reduced local setups.

## Build and Run

Use one script:
- [scripts/executor_ctl.sh](scripts/executor_ctl.sh)

Examples:

```bash
executor/scripts/executor_ctl.sh build-contracts
executor/scripts/executor_ctl.sh test
executor/scripts/executor_ctl.sh run
```

Direct test command:

```bash
CXXFLAGS='-include cstdint' cargo +nightly-2025-03-19 test \
  --manifest-path executor/Cargo.toml \
  -p zksync_state_machine \
  --all-features --ignore-rust-version -- --nocapture
```

## Repository Hygiene

1. Active package path is [src](src).
2. Legacy package path `state_machine/` is removed.
3. Build outputs are ignored via [executor/.gitignore](.gitignore).
