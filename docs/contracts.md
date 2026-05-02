# Smart Contracts

## Contracts Abstract Architecture
**Purpose:** High-level contract relationships.
**Evidence from code:** `contracts/AGENTS.md`

```mermaid
flowchart TD
    Bridge[ZKRollupBridge.sol] --> DA[IDAProvider]
    Bridge --> Verif[MockVerifier.sol]
    
    DA_Impl1[CalldataDA] -.->|Implements| DA
    DA_Impl2[BlobDA] -.->|Implements| DA
```
**Explanation:** The Bridge acts as the aggregate root. It delegates data availability checks to swappable DA providers and delegates proof verification to a verifier contract.

## Contracts Detailed Architecture
**Purpose:** Contract interactions and state transitions.
**Evidence from code:** `contracts/AGENTS.md`

```mermaid
flowchart LR
    SUB[Submitter] -->|submitBatch| BR[ZKRollupBridge]
    BR -->|Check Commitment| DA[DA Provider]
    BR -->|verifyProof| VER[Verifier Router]
    
    VER -->|True| BR
    BR -->|Update state_root| STATE[(Contract State)]
```
**Explanation:** The bridge orchestrates. It does not parse blobs; it simply takes the commitment, queries the DA provider to ensure it matches, verifies the proof, and finalizes the batch.

## Contracts Sequence Diagram
**Purpose:** On-chain batch finalization.
**Evidence from code:** `contracts/AGENTS.md`

```mermaid
sequenceDiagram
    participant Submitter
    participant Bridge
    participant DA_Contract
    participant Verifier

    Submitter->>Bridge: submitBatch(batchData, proof, da_commitment)
    Bridge->>DA_Contract: verifyCommitment(da_commitment)
    DA_Contract-->>Bridge: true
    Bridge->>Verifier: verify(proof, public_inputs)
    Verifier-->>Bridge: true
    Bridge->>Bridge: _finalizeBatch(update state)
```
**Explanation:** Synchronous execution within a single Ethereum transaction.
