# API Reference

This document provides a comprehensive specification of the `ZKRollupBridge` and related contracts.

## 1. ZKRollupBridge

The entry point for L2 Sequencers and Users.

### State Variables

| Type | Name | Description |
| :--- | :--- | :--- |
| `IVerifier` | `verifier` | Immutable address of the Groth16 Verifier contract. |
| `bytes32` | `stateRoot` | The current Merkle Root of the L2 state. |
| `address` | `sequencer` | The authorized sequencer. If `address(0)`, the bridge is in **Permissionless Mode**. |
| `uint256` | `nextBatchId` | Counter for batches (starts at 1). |
| `uint256` | `forcedInclusionDelay` | Immutable configuration (blocks) defining the censorship resistance window. |

### Data Structures

#### `Groth16Proof`
Structure representing a zk-SNARK proof.
```solidity
struct Groth16Proof {
    uint256[2] a;
    uint256[2][2] b;
    uint256[2] c;
}
```

### Functions

#### `commitBatch`
Submits a state transition proof and Data Availability commitment.

```solidity
function commitBatch(
    uint8 daId,
    bytes calldata batchData,
    bytes calldata daMeta,
    bytes32 newRoot,
    Groth16Proof calldata proof
) external
```

*   **Requirements:**
    *   Caller must be the `sequencer` (unless permissionless).
    *   `daId` must be a registered and enabled DA Provider.
    *   `newRoot` must not be zero.
    *   Proof verification must succeed.
    *   DA Provider validation must succeed.

#### `forceTransaction`
Enables the "Escape Hatch" for users being censored by the sequencer.

```solidity
function forceTransaction(bytes32 _txHash) external
```

*   **Behavior:**
    *   Calculates `deadline = block.number + forcedInclusionDelay`.
    *   Stores `forcedTxTimestamps[_txHash] = deadline`.
    *   Emits `ForcedTransactionEnqueued`.

#### `setDAProvider` (Admin)
Registers a new Data Availability strategy contract.

```solidity
function setDAProvider(uint8 daId, address provider, bool enabled) external onlyOwner
```

### Events

| Name | Signature | Description |
| :--- | :--- | :--- |
| `BatchFinalized` | `event BatchFinalized(uint256 indexed batchId, bytes32 indexed daCommitment, bytes32 oldRoot, bytes32 newRoot, uint8 daMode)` | Emitted when a batch is successfully verified and settled. |
| `ForcedTransactionEnqueued` | `event ForcedTransactionEnqueued(bytes32 indexed txHash, uint256 deadlineBlock)` | Emitted when a user initiates a forced inclusion. |
| `SequencerUpdated` | `event SequencerUpdated(address indexed newSequencer)` | Emitted when the sequencer address changes. |
| `DAProviderSet` | `event DAProviderSet(uint8 indexed daId, address provider, bool enabled)` | Emitted when a DA strategy is configured. |

### Errors

| Name | Description |
| :--- | :--- |
| `NotSequencer` | Caller is not the authorized sequencer. |
| `InvalidNewRoot` | The proposed `newRoot` is `bytes32(0)`. |
| `DAProviderNotEnabled` | The requested `daId` points to `address(0)` or is disabled. |
| `InvalidProof` | The Verifier contract rejected the Groth16 proof. |
| `DAProviderAlreadySet` | Attempted to overwrite an existing enabled provider without disabling it first. |
| `InvalidVerifier` | Constructor argument was `address(0)`. |

---

## 2. DA Providers (IDAProvider)

Interface for modular Data Availability strategies.

### `computeCommitment`
Calculates the commitment hash for the batch data.

```solidity
function computeCommitment(bytes calldata batchData, bytes calldata daMeta) external pure returns (bytes32)
```

### `validateDA`
Verifies that the data is available according to the strategy's rules.

```solidity
function validateDA(bytes32 commitment, bytes calldata daMeta) external view
```

### Implementations

*   **CalldataDA**:
    *   `computeCommitment`: Returns `keccak256(batchData)`.
    *   `validateDA`: No-op (presence in calldata is sufficient).
*   **BlobDA** (EIP-4844):
    *   `computeCommitment`: Extracts `versionedHash` from `daMeta`.
    *   `validateDA`: Checks `blobhash(index) == versionedHash`. Requires `cancun` EVM.
