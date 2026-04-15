# Sequencer

## Sequencer Abstract Architecture
**Purpose:** High-level view of Sequencer responsibilities.
**Evidence from code:** `sequencer/src/main.rs`, `sequencer/README.md`

```mermaid
flowchart TD
    API[JSON-RPC API] --> Pool[Tx Pools]
    L1[L1 Listener] --> Pool
    Pool --> Orch[Batch Orchestrator]
    Orch --> Exec[Executor Client]
```
**Explanation:** The Sequencer ingests from users and L1, queues them, orders them via a scheduler, and dispatches batches to the Executor.

## Sequencer Detailed Architecture
**Purpose:** Internal breakdown of the Sequencer.
**Evidence from code:** `sequencer/src/main.rs`, `sequencer/src/pool/`, `sequencer/src/scheduler/`

```mermaid
flowchart TD
    subgraph Sequencer_Process
        API[API Server] --> VAL[Validity Checker]
        VAL <--> CACHE[(State Cache)]
        VAL --> NORM[Normal Tx Pool]
        
        L1[L1 Event Listener] --> FORC[Forced Tx Queue]
        
        NORM --> ORCH[Batch Orchestrator]
        FORC --> ORCH
        
        ORCH <--> TRIG{Triggers:\nSize/Timeout/Forced}
        ORCH --> SCHED[Scheduler Policies:\nFCFS/TimeBoost/etc.]
        SCHED --> DB[(SQLite Registry)]
    end
```
**Explanation:** The Validty Checker strictly uses pessimistic balance tracking against the State Cache. Triggers dictate when the Orchestrator pulls from the queues and applies the active Strategy pattern scheduling policy.

## Sequencer Sequence Diagram
**Purpose:** Transaction lifecycle inside the Sequencer.
**Evidence from code:** `sequencer/src/main.rs`

```mermaid
sequenceDiagram
    participant User
    participant API
    participant Cache
    participant Pool
    participant Orch
    participant Exec

    User->>API: sendTransaction
    API->>Cache: Check Nonce & Balance
    Cache-->>API: OK (deduct balance)
    API->>Pool: Push to Normal Queue
    API-->>User: Accepted

    loop Background Task
        Orch->>Orch: Check Triggers (Timeout?)
        Orch->>Pool: Pull Txs
        Orch->>Orch: Apply Scheduling Policy
        Orch->>Exec: PublishBatch
    end
```
**Explanation:** User interaction is isolated from batch creation. Validation is instantaneous, while batching happens asynchronously.
