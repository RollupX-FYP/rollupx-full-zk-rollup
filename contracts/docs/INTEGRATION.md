# Integration Guide

This guide explains how to interact with the ZK Rollup contracts using `ethers.js` or similar libraries.

## 1. Connecting to the Bridge

### ABI
Load the `ZKRollupBridge.json` artifact.

```typescript
const bridge = new ethers.Contract(BRIDGE_ADDRESS, ABI, signer);
```

### Reading State
```typescript
const stateRoot = await bridge.stateRoot();
const nextBatchId = await bridge.nextBatchId();
const sequencer = await bridge.sequencer();
```

## 2. Committing a Batch

The `commitBatch` function requires constructing a `Groth16Proof` struct and formatting the input data correctly.

### Proof Structure
```typescript
const proof = {
  a: ["0x...", "0x..."],       // uint256[2]
  b: [["0x..","0x.."], ...],   // uint256[2][2]
  c: ["0x...", "0x..."]        // uint256[2]
};
```

### Submission (Calldata Mode)
```typescript
const DA_ID_CALLDATA = 0;
const batchData = ethers.toUtf8Bytes("transaction-data");
const daMeta = "0x"; // Empty for calldata

const tx = await bridge.commitBatch(
  DA_ID_CALLDATA,
  batchData,
  daMeta,
  newRootHash,
  proof
);
await tx.wait();
```

### Submission (Blob Mode)
Requires a transaction with EIP-4844 blobs attached. In `ethers.js` v6:

```typescript
const DA_ID_BLOB = 1;
const blobs = [ ... ]; // Blob data
const kzg = ...;       // KZG library

// 1. Calculate Versioned Hash
const versionedHash = ethers.hexlify(...);

// 2. Encode Metadata: (bytes32 hash, uint8 index)
const daMeta = ethers.AbiCoder.defaultAbiCoder().encode(
    ["bytes32", "uint8"],
    [versionedHash, 0] // Index 0
);

// 3. Send Transaction
const tx = await bridge.commitBatch(
  DA_ID_BLOB,
  "0x", // Empty batchData
  daMeta,
  newRootHash,
  proof,
  {
    blobs: blobs,
    kzg: kzg,
    maxFeePerBlobGas: ...
  }
);
```

## 3. Handling Forced Transactions

### Listening for Events
To detect if a user is trying to force a transaction:

```typescript
bridge.on("ForcedTransactionEnqueued", (txHash, deadline) => {
    console.log(`User forced tx ${txHash}. Must include by block ${deadline}`);
});
```

### Sending a Forced Request
Any user can call this:

```typescript
const txHash = ethers.keccak256("0xmytransactiondata...");
await bridge.forceTransaction(txHash);
```
