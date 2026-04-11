# Executor (Current)

This folder contains the active executor package `zksync_state_machine` implemented in [src](src).

Legacy package location `state_machine/` has been removed.

## Inputs and Outputs

### Runtime Input

- gRPC `PublishBatch` requests from sequencer (default mode).
- `batch_output.json` from sequencer bridge flow (legacy mode via [src/bridge.rs](src/bridge.rs)).
- System contract artifacts under [contracts/system-contracts](contracts/system-contracts).
- L1 contract artifacts under [contracts/l1-contracts/zkout](contracts/l1-contracts/zkout).

### Runtime Output

- gRPC `StreamBatches` stream consumed by submitter (default mode).
- `executor_prover_output.json` containing root hash, pubdata, witness metadata (legacy mode).
- Optional compatibility output `batch_output.executor.json` for downstream tooling.

## Build and Test

### Prerequisites

**Ubuntu / Linux:**
```bash
rustup install nightly-2025-03-19
sudo apt install -y clang libclang-dev cmake pkg-config libssl-dev
```

**Windows:**
```powershell
rustup install nightly-2025-03-19
choco install llvm cmake pkgconfiglite openssl # Run in Admin PowerShell
```
*(Ensure LLVM/Clang and OpenSSL are in your system PATH. Running the `.sh` scripts below requires Git Bash or WSL).*

### One Script for Build/Test/Run

Use [scripts/executor_ctl.sh](scripts/executor_ctl.sh):
*(On Windows, you can run these commands using Git Bash or WSL)*

```bash
# Run package tests
executor/scripts/executor_ctl.sh test

# Run tests and start binary
executor/scripts/executor_ctl.sh verify

# Run binary
executor/scripts/executor_ctl.sh run
```

### gRPC Runtime (Default)

**Linux / macOS / Windows (Git Bash or WSL):**
```bash
# Start executor gRPC server on 127.0.0.1:50051
export CXXFLAGS='-include cstdint'
EXECUTOR_MODE=grpc EXECUTOR_GRPC_ADDR=127.0.0.1:50051 executor/scripts/executor_ctl.sh run

# Optional legacy bridge mode
EXECUTOR_MODE=bridge executor/scripts/executor_ctl.sh run
```

**Windows (PowerShell direct):**
```powershell
# Start executor gRPC server on 127.0.0.1:50051
$env:CXXFLAGS="-include cstdint"
$env:EXECUTOR_MODE="grpc"
$env:EXECUTOR_GRPC_ADDR="127.0.0.1:50051"
cargo +nightly-2025-03-19 run --manifest-path executor/Cargo.toml -p zksync_state_machine --ignore-rust-version

# Optional legacy bridge mode
$env:EXECUTOR_MODE="bridge"
cargo +nightly-2025-03-19 run --manifest-path executor/Cargo.toml -p zksync_state_machine --ignore-rust-version
```

Optional env:

**Linux / macOS / Windows (Git Bash):**
```bash
export CXXFLAGS='-include cstdint'
```

**Windows (PowerShell):**
```powershell
$env:CXXFLAGS="-include cstdint"
```

### Direct Cargo Commands

**Linux / macOS / Windows (Git Bash):**
```bash
CXXFLAGS='-include cstdint' cargo +nightly-2025-03-19 test \
    --manifest-path executor/Cargo.toml \
    -p zksync_state_machine \
    --all-features --ignore-rust-version -- --nocapture
```

**Windows (PowerShell direct):**
```powershell
$env:CXXFLAGS="-include cstdint"
cargo +nightly-2025-03-19 test `
    --manifest-path executor/Cargo.toml `
    -p zksync_state_machine `
    --all-features --ignore-rust-version -- --nocapture
```

If you run `cargo check` directly, use:

```bash
cargo +nightly-2025-03-19 check --manifest-path executor/Cargo.toml -p zksync_state_machine --ignore-rust-version
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
