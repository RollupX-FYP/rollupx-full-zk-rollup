# Best Practices & Architecture

This repository adheres to **Domain-Driven Design (DDD)** and **SOLID** principles to ensure a secure, modular, and maintainable L1 Settlement layer for the ZK Rollup.

## 1. Architecture & DDD Boundaries

### Bounded Context: "Settlement"
The L1 contracts operate strictly within the **Settlement** context. Their purpose is limited to:
- **Finality**: Verifying proofs and updating state roots.
- **Data Availability (DA) Binding**: ensuring transaction data is committed (via Calldata or Blobs).
- **Minimal State**: Storing only essential aggregates (`stateRoot`, `batchCommitment`).

### Aggregate Root: `ZKRollupBridge`
- The `ZKRollupBridge` acts as the **Aggregate Root**.
- **Responsibility**: It orchestrates the state transition. It validates inputs via Domain Services (Verifier, DA Provider) and updates the internal state.
- **State Transition**: State updates happen *only* via the strict `_finalizeBatch` transition function, ensuring consistency.

### Domain Services
- **Verifier**: Stateless cryptographic verification (`IVerifier`).
- **DA Provider**: Validates data availability strategies (`IDAProvider`).

---

## 2. SOLID Principles

### Single Responsibility Principle (SRP)
- **Bridge**: Orchestration and state management.
- **Verifier**: Pure cryptography (pairing checks).
- **DA Providers**: Validation of specific DA media (Calldata vs Blobs).

### Open/Closed Principle (OCP)
- The system is **open for extension** but **closed for modification**.
- New DA strategies can be added by deploying a new contract implementing `IDAProvider` and registering it in the Bridge.
- The core Bridge logic does not need to change to support new DA types.

### Liskov Substitution Principle (LSP)
- All DA providers implement `IDAProvider` and can be used interchangeably by the Bridge without breaking logic.
- Verifiers implement `IVerifier` and can be swapped (via deployment configuration) without altering the Bridge.

### Interface Segregation Principle (ISP)
- Interfaces (`IVerifier`, `IDAProvider`) are minimal and focused on specific needs.

### Dependency Inversion Principle (DIP)
- High-level modules (`ZKRollupBridge`) depend on abstractions (`IVerifier`, `IDAProvider`), not concrete implementations (`RealVerifier`, `BlobDA`).

---

## 3. Design Patterns

### Strategy Pattern (Data Availability)
We use the Strategy Pattern to support multiple DA modes:
- **CalldataDA**: Validates `keccak256(batchData)`.
- **BlobDA**: Validates EIP-4844 versioned hashes and ensures blobs are attached via `blobhash` opcode checks.

### Adapter Pattern (Verifier)
The `RealVerifier` contract adapts the `Pairing` library and low-level elliptic curve operations to the clean `IVerifier` interface expected by the Bridge.
- **Implementation Detail**: The `vk.IC` array in `RealVerifier` is sized to 4. This aligns with the 1 constant + 3 public inputs (Commitment, OldRoot, NewRoot) required by the protocol.

---

## 4. Directory Structure

The codebase is organized to reflect these architectural layers:

```
contracts/
├── bridge/         # Aggregate Roots & Core Logic (ZKRollupBridge)
├── interfaces/     # Abstractions (IVerifier, IDAProvider)
├── da/             # DA Strategies (CalldataDA, BlobDA)
├── verifiers/      # Cryptographic implementations (RealVerifier, MockVerifier)
└── libraries/      # Shared logic (Pairing)
```

---

## 5. Security & Testing Standards

### Security
- **Registry Pattern**: DA providers must be explicitly allowlisted (`daEnabled`) by the owner. Arbitrary addresses are blocked.
- **Immutable Verifier**: The verifier address is immutable to prevent malicious swaps after deployment. (Upgrades should use a Router pattern if needed).
- **Strict Boundaries**: The Bridge never parses batch data; it only stores commitments.
- **Sequencer Permissionless Mode**: If the `sequencer` is set to `address(0)`, the bridge enters a dev-only permissionless mode where any address can submit batches. This must be managed carefully.

### Testing
- **100% Coverage**: All branches, including error states and edge cases, must be covered.
- **Mocking**:
  - **Blobhash**: `TestBlobDA` overrides the virtual `_getBlobHash` to simulate Cancun opcode behavior in test environments.
  - **Verifier**: `MockVerifier` is used for integration tests to isolate Bridge logic from cryptographic complexity.
