# Workload Generator / Benchmark Suite

## Workload Generator Abstract Architecture
**Purpose:** Show how the workload generator orchestrates tests.
**Evidence from code:** `benchmark-suite/README.md`, `benchmark-suite/workload/poisson_generator.py`

```mermaid
flowchart TD
    WG[Poisson Generator Script]
    CONF[Config / Experiments Matrix]
    SEQ[Target Sequencer]
    METR[Local Metrics Files]

    CONF --> WG
    WG -->|HTTP JSON-RPC| SEQ
    WG -->|Write| METR
```
**Explanation:** The generator reads experiment configurations, blasts the Sequencer with synthetic HTTP traffic based on a Poisson distribution, and dumps the resulting telemetry into a local JSON file.

## Workload Generator Detailed Architecture
**Purpose:** Detail internal modules of the workload generator.
**Evidence from code:** `benchmark-suite/workload/poisson_generator.py`

```mermaid
flowchart LR
    CLI[ArgParser] --> INIT[Load Environment & Seed]
    INIT --> LOOP[Main Traffic Loop]
    
    subgraph Traffic_Generation
        LOOP --> TX_GEN[hash_tx / eth_account sign]
        TX_GEN --> HTTP[urllib request JSON-RPC]
    end
    
    HTTP --> LOOP
    LOOP -->|Finish Duration| STATS[Compile Metrics]
    STATS --> DISK[(workload_exp.json)]
```
**Explanation:** A synchronous python loop generates ECDSA-signed mock Ethereum transactions and sends them sequentially/concurrently via HTTP, calculating throughput and latency.

## Workload Generator Sequence Diagram
**Purpose:** Runtime behavior of the generator.
**Evidence from code:** `benchmark-suite/workload/poisson_generator.py`

```mermaid
sequenceDiagram
    participant Main
    participant RNG
    participant HTTP
    participant Disk

    Main->>RNG: Initialize Seed
    loop Duration
        Main->>RNG: Sleep (Poisson distribution delay)
        Main->>Main: Generate Tx Payload & Sign
        Main->>HTTP: POST sendTransaction
        HTTP-->>Main: Response (Accept/Reject)
        Main->>Main: Record Latency
    end
    Main->>Disk: Write workload_<exp_id>.json
```
**Explanation:** The core loop sleeps based on a mathematical distribution to simulate real-world arrival times before dispatching requests.
