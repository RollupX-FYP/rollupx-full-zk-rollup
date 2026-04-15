# RollupX System Overview

RollupX is an experimental, modular Layer-2 ZK-Rollup prototype. Its primary design goal is high throughput, custom transaction scheduling, and comprehensive observability across various components.

The system implements a layered architecture starting from user transaction generation, moving through sequencing and execution, simulating a cryptographic proof stage, and finally submitting state updates to an Ethereum Layer 1 (L1) bridge via smart contracts.

The system relies heavily on Domain-Driven Design (DDD) principles in its Rust-based microservices (Sequencer, Submitter) and includes extensive testing, benchmarking, and data visualization tools to evaluate different rollup configurations (such as First-Come-First-Served vs. Time-Boost scheduling, and Calldata vs. EIP-4844 Blob Data Availability).

## Core Components & Implementation Status

1. **UI (`zk-rollup-ui/`)**
   - **Responsibility:** Provide a frontend to deposit, transfer, withdraw, and inspect L2 state.
   - **Status:** **Partially implemented / Minimal.** Contains a `README.md` describing it as a "Minimal dApp", but full source code is not present in the main source tree.

2. **Workload Generator / Benchmark Suite (`benchmark-suite/`)**
   - **Responsibility:** Orchestrate controlled experiments by generating synthetic traffic via Poisson processes, varying transaction types, and configuring sequencer/prover behaviors.
   - **Status:** **Real / Fully Implemented.**

3. **Sequencer (`sequencer/`)**
   - **Responsibility:** Ingest transactions via JSON-RPC, validate them against a pessimistic state cache, and batch them based on size, timeout, or forced L1 triggers. It includes swappable scheduling policies.
   - **Status:** **Real / Fully Implemented.**

4. **Executor (`executor/`)**
   - **Responsibility:** Receive sealed batches from the Sequencer via gRPC, apply state transitions using a VM, and produce outputs containing root hashes and witness metadata.
   - **Status:** **Real / Fully Implemented.** *Note: In default `grpc` mode, it acts as a pass-through relay and emits relay metrics to `executor_<exp_id>.json`. It currently fails tests locally because pre-compiled system-contract artifacts (`Bootloader.zbin`) are missing.*

5. **Prover Subsystem (`Zk-Prover/`, `submitter/src/infrastructure/prover_mock.rs`)**
   - **Responsibility:** Generate Zero-Knowledge proofs for the executed batches.
   - **Status:** **Mocked / Stubbed.** The system heavily simulates this stage. The submitter uses mock providers to simulate proof generation delay and return dummy proof payloads.

6. **Submitter (`submitter/`)**
   - **Responsibility:** Ingest finalized executor batches and submit them to the Ethereum L1 bridge. It handles DA (Data Availability) via Calldata or EIP-4844 blobs, maintains an internal database for retry loops and idempotency, and uses a Saga workflow.
   - **Status:** **Real / Fully Implemented.**

7. **Smart Contracts (`contracts/`)**
   - **Responsibility:** Act as the L1 settlement layer. Includes the `ZKRollupBridge` which orchestrates validation, and distinct DA providers.
   - **Status:** **Real / Fully Implemented.** *Note: Verifier contracts are mocked for local testing.*

8. **Data Tools (`data-tools/`)**
   - **Responsibility:** Consume JSON/CSV metrics from the benchmark suite to compute statistics and plot Pareto frontiers, throughput bars, and latency CDFs.
   - **Status:** **Real / Fully Implemented.**

## Abstract Architecture Diagram

```mermaid
flowchart LR
    UI[UI Frontend\n(Minimal)] --> SEQ
    WG[Workload Generator\n/ Benchmark Suite] -->|Synthetic Tx Load| SEQ

    subgraph Rollup_L2 [Rollup L2 Node]
        SEQ[Sequencer] -->|Sealed Batch| EXEC[Executor\n(gRPC Relay)]
        EXEC -->|Root/Witness| PROV[Mocked Prover\n(Simulated Delay)]
        PROV -->|Mock Proof| SUB[Submitter]
    end

    SUB -->|DA & Proof| L1[Ethereum L1 / Local Hardhat]
    L1 -.->|L1 Events| SEQ

    WG -.->|Metrics| DT[Data Tools / Analytics]
    SEQ -.->|Metrics| DT
    EXEC -.->|Metrics| DT
    SUB -.->|Metrics| DT
```

**Key takeaways:**
- Traffic from the workload generator hits the Sequencer.
- The Sequencer forwards batches to the Executor via gRPC.
- The Executor (currently acting as a relay) forwards them to the Submitter.
- The Submitter gets a mock proof and posts data to the L1 contracts.
- Metrics are collected from the Workload Generator, Executor, and Submitter and aggregated by the Data Tools pipeline.
