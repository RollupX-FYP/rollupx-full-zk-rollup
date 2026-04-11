# Executor (Current)

This folder contains the active executor package `zksync_state_machine` implemented in [src](src).

Legacy package location `state_machine/` has been removed.

## Inputs and Outputs

### Runtime Input
- `batch_output.json` from sequencer bridge flow (via [src/bridge.rs](src/bridge.rs)).
- System contract artifacts under [contracts/system-contracts](contracts/system-contracts).
- L1 contract artifacts under [contracts/l1-contracts/zkout](contracts/l1-contracts/zkout).

### Runtime Output
- `executor_prover_output.json` containing root hash, pubdata, witness metadata.
- Optional compatibility output `batch_output.executor.json` for downstream tooling.

## Build and Test

### Prerequisites
```bash
rustup install nightly-2025-03-19
sudo apt install -y clang libclang-dev cmake pkg-config libssl-dev
```

### One Script for Build/Test/Run
Use [scripts/executor_ctl.sh](scripts/executor_ctl.sh):

```bash
# Compile + sync contract artifacts from zksync-era
executor/scripts/executor_ctl.sh build-contracts

# Run package tests
executor/scripts/executor_ctl.sh test

# Build contracts + run tests
executor/scripts/executor_ctl.sh verify

# Run binary
executor/scripts/executor_ctl.sh run
```

Optional env:
```bash
export ZKSYNC_ERA_ROOT=/path/to/zksync-era
export CXXFLAGS='-include cstdint'
```

### Direct Cargo Commands
```bash
CXXFLAGS='-include cstdint' cargo +nightly-2025-03-19 test \
    --manifest-path executor/Cargo.toml \
    -p zksync_state_machine \
    --all-features --ignore-rust-version -- --nocapture
```

## Current Structure

```text
executor/
├── src/                        # active package (lib.rs, executor.rs, bridge.rs, main.rs)
├── scripts/executor_ctl.sh     # unified build/test/run script
├── contracts/                  # system + l1 artifacts
├── lib/                        # era libs
├── node/                       # shared metrics + node crates used by workspace
├── patches/                    # pinned compile patches
├── Cargo.toml                  # workspace root, zksync_state_machine -> src
└── SYSTEM_DESIGN.md
```

See [SYSTEM_DESIGN.md](SYSTEM_DESIGN.md) for architecture and data flow details.
