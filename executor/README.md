# Standalone ZKsync State Machine

A standalone extraction of the `zksync_state_machine` crate from the [ZKsync Era](https://github.com/matter-labs/zksync-era) monorepo. Provides a self-contained batch processing pipeline — execute L2 transactions through the ZKsync EraVM, update the ZK Merkle tree, and produce a state root + ZK witness.

---

## Prerequisites

```bash
# Rust nightly toolchain (pinned via rust-toolchain.toml)
rustup install nightly-2025-03-19

# System libraries (Ubuntu/Debian)
sudo apt install -y clang libclang-dev cmake pkg-config libssl-dev
```

---

## Build

```bash
export CXXFLAGS="-include cstdint"
cargo build -p zksync_state_machine --ignore-rust-version
```

> First build compiles RocksDB, EraVM, and ZK crypto from source — expect **5–10 minutes**. Subsequent builds are incremental.

---

## Run Tests

```bash
export CXXFLAGS="-include cstdint"
cargo test -p zksync_state_machine --all-features --ignore-rust-version
```

Expected output:
```
running 2 tests
test tests::test_sequential_transactions ... ok
test tests::test_batch_processor ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in ~4s
```

---

## Repository Structure

```
executor/
├── rust-toolchain.toml         # Pins nightly-2025-03-19
├── state_machine/              # Main crate (BatchProcessor, StateMachine, TreeProcessor)
│   └── src/
│       ├── lib.rs              # StateMachine wrapper + integration tests
│       ├── executor.rs         # BatchProcessor orchestrator
│       ├── tree.rs             # TreeProcessor (Merkle tree + witness)
│       └── types.rs            # BatchInput / BatchOutput types
├── lib/
│   ├── multivm/                # ZKsync EraVM implementation
│   ├── merkle_tree/            # ZK sparse Merkle tree + witness generation
│   ├── types/                  # Core types (transactions, keys, H256, Address)
│   ├── contracts/              # System contract ABI & bytecode loaders
│   ├── dal/                    # Database abstraction (RocksDB-backed)
│   ├── state/                  # Storage state management
│   ├── vm_interface/           # VM traits and in-memory storage
│   ├── crypto_primitives/      # K256 keys, signing utilities
│   └── ...                     # 20+ additional internal library crates
├── node/
│   └── shared_metrics/         # Prometheus metrics stubs
├── contracts/
│   └── system-contracts/       # Compiled system contract artifacts (required at runtime)
└── patches/
    ├── svm-rs/                 # Patched to compile on nightly-2025-03-19
    └── svm-rs-builds/          # Same patch for companion crate
```

See [`SYSTEM_DESIGN.md`](./SYSTEM_DESIGN.md) for architecture diagrams and data flow.
