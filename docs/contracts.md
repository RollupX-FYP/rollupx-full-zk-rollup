# Smart Contracts

## Contracts Abstract Architecture
**Purpose:** High-level contract relationships.
**Evidence from code:** `contracts/contracts/bridge/ZKRollupBridge.sol`

```mermaid
flowchart TD
    Bridge[ZKRollupBridge.sol] --> DA[IDAProvider]
    Bridge --> Verif[IVerifier Mapping]
    
    DA_Impl1[CalldataDA] -.->|Implements| DA
    DA_Impl2[BlobDA] -.->|Implements| DA
    
    V_Impl1[Groth16Verifier] -.->|In Registry| Verif
    V_Impl2[MockVerifier] -.->|In Registry| Verif
```
**Explanation:** The Bridge acts as the aggregate root. It delegates data availability checks to swappable DA providers and delegates proof verification to a registry of verifier contracts.

## Contracts Detailed Architecture
**Purpose:** Contract interactions and state transitions.
**Evidence from code:** `ZKRollupBridge.sol`

```mermaid
flowchart LR
    SUB[Submitter] -->|commitBatch| BR[ZKRollupBridge]
    BR -->|validateDA| DA[DA Provider]
    BR -->|verifyProof| VER[Verifier Registry]
    
    VER -->|True| BR
    BR -->|Update latestStateRoot| STATE[(Contract State)]
```
**Explanation:** The bridge orchestrates. It does not parse blobs; it simply takes the commitment, queries the DA provider to ensure it matches, verifies the proof using the selected backend, and finalizes the batch.

## Contracts Sequence Diagram
**Purpose:** On-chain batch finalization.
**Evidence from code:** `ZKRollupBridge.sol`

```mermaid
sequenceDiagram
    participant Submitter
    participant Bridge
    participant DA_Contract
    participant Verifier
 
    Submitter->>Bridge: commitBatch(daId, verifierId, data, meta, root, proof)
    Bridge->>DA_Contract: validateDA(commitment, meta)
    DA_Contract-->>Bridge: void (success)
    Bridge->>Verifier: verifyProof(a, b, c, inputs)
    Verifier-->>Bridge: true
    Bridge->>Bridge: _finalizeBatch(update latestStateRoot)
```
**Explanation:** Synchronous execution within a single Ethereum transaction.
