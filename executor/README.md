# Standalone ZKsync State Machine

This repository is a decoupled, standalone extraction of the `zksync_state_machine` module from the ZKsync Era monorepo. It provides a complete batch processing pipeline for ZKsync Era.

## 🚀 Build & Test Instructions

Everything you need to build and verify the state machine in one go.

### 1. Prerequisites
- **RocksDB**: Used for persistent state storage.
- **Environment Variables**: You **MUST** include the following `CXXFLAGS` for successful compilation of RocksDB:
  ```bash
  export CXXFLAGS="-include cstdint"
  ```

### 2. Full Assembly & Build
From the root of this repository, run:
```bash
# Export the mandatory build flags
export CXXFLAGS="-include cstdint"

# Build the main state machine crate
cargo build -p zksync_state_machine
```

### 3. Run Integration Tests
To verify the full batch processing pipeline (VM execution + Merkle tree updates):
```bash
export CXXFLAGS="-include cstdint"
cargo test -p zksync_state_machine --all-targets --all-features
```

## 🏗️ Architecture Overview

### Main Module: `state_machine/`
The core logic resides in the `state_machine/` directory (promoted to the root for accessibility).

- **`BatchProcessor`**: Orchestrates the VM and the Merkle tree.
- **`StateMachine`**: A simplified wrapper around the ZKsync EraVM.
- **`TreeProcessor`**: Manages the ZKsync Merkle tree, leaf indexing, and witness generation.

### Inputs & Outputs
- **Input**: A batch of transactions, L1/L2 block environment, and system configuration.
- **Output**: Final state root, ZK witness data for the prover, and pubdata for L1 Data Availability (DA).

## 📂 Project Structure

- `state_machine/`: The main batch processing crate.
- `lib/`: Internal ZKsync library dependencies (30+ crates, e.g., `multivm`, `merkle_tree`, `dal`).
- `node/`: Internal node components (e.g., `shared_metrics`).
- `contracts/`: System and L1 contracts required for VM execution and testing.

---
*Extracted and minimized from the ZKsync Era repository.*
