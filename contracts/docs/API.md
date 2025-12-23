# Smart Contract API Reference

This document provides detailed API specifications for the core smart contracts.

## ZKRollupBridge

The `ZKRollupBridge` is the Aggregate Root contract managing the rollup state.

### `commitBatch`

Commits a new batch of transactions to L1, updating the state root and verifying the ZK proof.

```solidity
function commitBatch(
    uint8 daId,
    bytes calldata batchData,
    bytes calldata daMeta,
    bytes32 newRoot,
    Groth16Proof calldata proof
) external
```

**Parameters:**

- `daId` (uint8): The ID of the DA provider strategy to use.
    - `0`: CalldataDA
    - `1`: BlobDA
- `batchData` (bytes):
    - For **CalldataDA**: The raw compressed batch data.
    - For **BlobDA**: Empty `0x`.
- `daMeta` (bytes): Metadata required by the DA provider.
    - For **CalldataDA**: Empty `0x`.
    - For **BlobDA**: ABI encoded `(bytes32 expectedVersionedHash, uint8 blobIndex)`.
- `newRoot` (bytes32): The new state root after applying the batch.
- `proof` (Groth16Proof): The Zero-Knowledge proof verifying the transition.
    - Struct: `struct Groth16Proof { uint256[2] a; uint256[2][2] b; uint256[2] c; }`

**Access Control:**
- If `sequencer` is set (non-zero): Only callable by `sequencer`.
- If `sequencer` is `address(0)`: Callable by anyone (Permissionless Dev Mode).

### `setSequencer`

Updates the sequencer address.

```solidity
function setSequencer(address newSequencer) external onlyOwner
```

- `newSequencer`: The new address. Set to `address(0)` to enable permissionless mode.

### `setDAProvider`

Registers or updates a DA provider strategy.

```solidity
function setDAProvider(uint8 daId, address provider, bool enabled) external onlyOwner
```

- Note: To change the address of an enabled provider, you must first disable it (2-step process).

### Events

- `BatchFinalized(uint256 indexed batchId, bytes32 indexed daCommitment, bytes32 oldRoot, bytes32 newRoot, uint8 daMode)`
- `SequencerUpdated(address indexed newSequencer)`
- `DAProviderSet(uint8 indexed daId, address provider, bool enabled)`

---

## DA Strategies

### CalldataDA (ID: 0)

Uses Ethereum calldata for data availability.
- **Commitment**: `keccak256(batchData)`
- **Validation**: None (implicit).

### BlobDA (ID: 1)

Uses EIP-4844 Blobs for data availability.
- **Commitment**: The KZG versioned hash of the blob.
- **Validation**: Checks `blobhash(index)` matches the commitment.
- **Metadata**: `abi.encode(bytes32 versionedHash, uint8 blobIndex)`

---

## Verifier

### RealVerifier

Implements the Groth16 verification logic on the BN254 curve.
- **Public Inputs**: The bridge passes public inputs in the following order:
  1. `daCommitment` (reduced to field element)
  2. `oldRoot` (reduced to field element)
  3. `newRoot` (reduced to field element)
