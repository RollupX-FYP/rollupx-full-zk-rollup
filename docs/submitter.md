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
    participant CB as Circuit Breaker
    participant Prover
    participant L1

    Stream->>Saga: New Batch
    Saga->>DB: Save State (Discovered)
    
    rect rgb(200, 220, 255)
    Note over Saga, Prover: Proving Phase
    Saga->>CB: Request Proof
    alt Circuit CLOSED
        CB->>Prover: HTTP POST /prove
        Prover-->>CB: 200 OK (Proof)
        CB-->>Saga: proof bytes
    else Circuit OPEN (Load Shedding)
        CB-->>Saga: Error (CircuitOpen)
        Saga->>Saga: Backoff & Retry
    end
    end

    Saga->>DB: Save State (Proved)
    Saga->>L1: eth_sendRawTransaction
    L1-->>Saga: Receipt
    Saga->>DB: Save State (Confirmed)
```
**Explanation:** The Submitter implements a robust Saga loop with a built-in Circuit Breaker to prevent overwhelming a struggling prover. Every state transition is persisted to the database, allowing the service to resume seamlessly after a crash.

## Research & Metrics Mapping

| Research Goal | Submitter Metric | Interpretation |
| :--- | :--- | :--- |
| **System Finality** | `batch_e2e_duration_seconds` | Primary measure of L2 -> L1 settlement latency. |
| **System Stability** | `prover_circuit_tripped_total` | Frequency of prover outages or overload events. |
| **DA Efficiency** | `tx_submitted_total` | Comparative analysis of Calldata vs. Blob frequency. |
| **Operational Cost** | `gas_used` (logs) | Direct projection of L1 operational overhead. |
| **Fault Tolerance** | `batches_failed_permanent_total`| System reliability floor (Dead Letter Queue rate). |
