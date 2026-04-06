# System Design — Standalone ZKsync State Machine

This document describes the architecture, component responsibilities, and data flows of the standalone ZKsync state machine.

---

## Overview

The state machine implements one half of a ZK rollup: it **executes** a batch of L2 transactions through the ZKsync EraVM, writes the resulting state changes into a ZK Merkle tree, and produces the cryptographic outputs needed by a prover.

```mermaid
graph TD
    TX["L2 Transactions (signed EIP-712)"]
    ENV["Batch Environment (L1 block number, timestamp, fee params, chain ID)"]
    SM["StateMachine (EraVM)"]
    TREE["TreeProcessor (ZK Merkle Tree)"]
    OUT["BatchOutput (root_hash, witness, pubdata)"]
    PROVER["ZK Prover (external)"]
    L1["L1 Contract (settlement)"]

    TX --> SM
    ENV --> SM
    SM -- "StorageLogs" --> TREE
    TREE --> OUT
    OUT -- "witness" --> PROVER
    OUT -- "root_hash + pubdata" --> L1
```

---

## Core Components

### 1. `BatchProcessor` (`executor.rs`)

Top-level orchestrator. Owns both the `StateMachine` and the `TreeProcessor`, and sequences their interaction for a full batch.

```mermaid
graph TD
    subgraph BatchProcessor
        BP_new["new(storage, envs, db_path)"]
        BP_proc["process_batch(BatchInput)"]
        BP_state["state_machine_mut()"]
    end

    subgraph StateMachine
        SM_new["new(storage, envs)"]
        SM_exec["execute_transaction(tx)"]
        SM_seal["seal_batch()"]
    end

    subgraph TreeProcessor
        TP_new["new(db_path)"]
        TP_proc["process_batch(logs)"]
    end

    BatchProcessor --> StateMachine
    BatchProcessor --> TreeProcessor
```

---

### 2. Batch Execution Flow

```mermaid
sequenceDiagram
    participant Caller
    participant BatchProcessor
    participant StateMachine
    participant EraVM
    participant TreeProcessor
    participant MerkleTree

    Caller->>BatchProcessor: process_batch(BatchInput)

    loop For each Transaction
        BatchProcessor->>StateMachine: execute_transaction(tx)
        StateMachine->>EraVM: push_transaction(tx)
        EraVM-->>StateMachine: VmExecutionResultAndLogs
        StateMachine-->>BatchProcessor: result + storage_logs
    end

    BatchProcessor->>StateMachine: seal_batch()
    StateMachine->>EraVM: finish_batch()
    EraVM-->>StateMachine: FinishedL1Batch
    StateMachine-->>BatchProcessor: deduplicated_storage_logs

    BatchProcessor->>TreeProcessor: process_batch(storage_logs)
    TreeProcessor->>MerkleTree: apply writes
    MerkleTree-->>TreeProcessor: root_hash + witness
    TreeProcessor-->>BatchProcessor: TreeOutput

    BatchProcessor-->>Caller: BatchOutput (root_hash, pubdata, witness, finished_batch)
```

---

### 3. Storage Architecture

```mermaid
graph LR
    TX["Transaction execution"] -- "writes StorageLogs" --> SV

    subgraph In-Process Storage
        SV["StorageView (write cache)"]
        BASE["InMemoryStorage (system contracts + initial state)"]
        SV -- "read-through" --> BASE
    end

    SV -- "deduplicated writes at batch seal" --> TREE

    subgraph On-Disk
        TREE["ZK Merkle Tree (RocksDB)"]
    end
```

**`InMemoryStorage`** pre-loads all ZKsync system contracts (AccountCodeStorage, ContractDeployer, L1Messenger, etc.) on construction. It acts as the read-only base layer.

**`StorageView`** wraps the base storage and buffers every write during VM execution. At batch seal, these writes are deduplicated and forwarded to the Merkle tree.

---

### 4. Merkle Tree & Witness Generation

```mermaid
graph TD
    LOGS["Deduplicated StorageLogs (key: StorageKey, value: H256)"]
    TREE["ZK Sparse Merkle Tree (256-bit keys, 32-byte leaves)"]
    ROOT["State Root Hash (H256)"]
    WITNESS["ZK Witness (Merkle paths)"]
    PUBDATA["Pubdata (compressed state diffs)"]

    LOGS --> TREE
    TREE --> ROOT
    TREE --> WITNESS
    LOGS --> PUBDATA
```

The Merkle tree uses a **sparse binary tree** with 256-bit keys (derived from `keccak(address, slot)`). Each batch application produces:

- **Root hash** — committed to L1 to prove state transition
- **Witness** — Merkle opening proofs fed to the ZK prover circuit
- **Pubdata** — compressed state diff posted to L1 for data availability

---

### 5. Transaction Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Pending
    Pending --> Pushed : vm.push_transaction
    Pushed --> Executing : EraVM processing

    Executing --> Success : tx complete
    Executing --> Reverted : explicit revert or out of gas

    Success --> LogsEmitted : state changes produced
    Reverted --> LogsEmitted : state changes produced

    LogsEmitted --> Sealed : seal_batch called
    Sealed --> TreeApplied : Merkle tree updated
    TreeApplied --> [*]
```

---

### 6. Dependency Graph (Major Crates)

```mermaid
graph BT
    SM["zksync_state_machine"]

    SM --> MULTI["zksync_multivm"]
    SM --> TREE["zksync_merkle_tree"]
    SM --> VMIF["zksync_vm_interface"]
    SM --> TYPES["zksync_types"]
    SM --> CONTRACTS["zksync_contracts"]
    SM --> STATE["zksync_state"]
    SM --> STORE["zksync_storage"]

    MULTI --> VMIF
    MULTI --> TYPES
    TREE --> TYPES
    STATE --> VMIF
    STATE --> TYPES
    STORE --> TYPES
    CONTRACTS --> TYPES

    TYPES --> BTYPE["zksync_basic_types"]
    VMIF --> TYPES
    STORE --> RDB["rocksdb"]
    TREE --> RDB
```

---

### 7. Key Data Structures

```mermaid
graph LR
    subgraph Input
        BI["BatchInput"]
        BI --> L1E["L1BatchEnv"]
        BI --> SE["SystemEnv"]
        BI --> TXS["Vec&lt;Transaction&gt;"]
    end

    subgraph Output
        BO["BatchOutput"]
        BO --> RH["root_hash: H256"]
        BO --> PD["pubdata: Vec&lt;u8&gt;"]
        BO --> WIT["witness: Option&lt;WitnessBlockState&gt;"]
        BO --> FB["finished_batch: FinishedL1Batch"]
    end
```

---

## Design Decisions

| Decision                        | Rationale                                                                                                      |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| **Standalone workspace**        | Enables independent development and testing without the full 200+ crate monorepo                               |
| **`StorageView` write cache**   | Avoids partial tree writes mid-batch; all changes applied atomically at seal                                   |
| **RocksDB for the Merkle tree** | Proven key-value store, already used by ZKsync Era nodes in production                                         |
| **`LegacyVmInstance`**          | Pinned VM version matches the protocol version in the system contracts; ensures consistent execution semantics |
| **Local `svm-rs` patches**      | Minimal targeted fix to unblock the nightly-2025-03-19 toolchain without forking or upgrading upstream         |
