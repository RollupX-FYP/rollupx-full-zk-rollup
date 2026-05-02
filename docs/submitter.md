# Submitter

## Submitter Abstract Architecture
**Purpose:** Overview of the batch submission process.
**Evidence from code:** `submitter/README.md`, `submitter/src/submitter.rs`

```mermaid
flowchart TD
    EXEC[Executor Stream] --> ING[Ingestion]
    ING --> SAGA[Saga Workflow Engine]
    SAGA --> DA[DA Formatting]
    SAGA --> L1[L1 Broadcaster]
```
**Explanation:** The Submitter pulls execution results and uses a robust Saga pattern to ensure Data Availability and Proofs are securely and reliably posted to L1.

## Submitter Detailed Architecture
**Purpose:** Internal DDD structure of the Submitter.
**Evidence from code:** `submitter/src/infrastructure/`

```mermaid
flowchart TD
    subgraph Submitter_Daemon
        GRPC[gRPC Client] --> DOM[Domain Layer / Saga]
        
        DOM <--> DB[(Postgres / SQLite)]
        DOM --> PROV[ProofProvider Interface]
        
        DOM --> DA_FMT[DA Formatter]
        DA_FMT -->|Calldata| COMP[Zlib Compression]
        DA_FMT -->|EIP-4844| BLOB[Blob Formatter]
        
        COMP --> ETH[Ethereum Adapter]
        BLOB --> ETH
    end
```
**Explanation:** Follows Hexagonal Architecture. The domain logic dictates the workflow (fetch proof -> format DA -> send tx), utilizing adapters for storage, proving, and Ethereum RPC.

## Submitter Sequence Diagram
**Purpose:** The Saga execution loop.
**Evidence from code:** `submitter/src/infrastructure/prover_http.rs`, `submitter/README.md`

```mermaid
sequenceDiagram
    participant Stream
    participant Saga
    participant DB
    participant Prover
    participant L1

    Stream->>Saga: New Batch
    Saga->>DB: Save State (PendingProof)
    Saga->>Prover: get_proof()
    Prover-->>Saga: proof bytes
    Saga->>DB: Save State (PendingSubmit)
    Saga->>L1: eth_sendRawTransaction
    L1-->>Saga: Receipt
    Saga->>DB: Save State (Finalized)
```
**Explanation:** Every step of the pipeline is checkpointed to the database, ensuring crash-recovery and preventing double-submission.
