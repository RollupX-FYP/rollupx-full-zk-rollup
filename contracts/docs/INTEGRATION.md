# Integration Guide

This guide explains how to communicate with the ZK Rollup Bridge contracts using JSON-RPC and standard libraries like `ethers.js`.

## Communicating with the Bridge

To submit batches, you need to call the `commitBatch` function on the `ZKRollupBridge` contract.

### JSON-RPC Interface

You will primarily use the `eth_sendTransaction` method.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "eth_sendTransaction",
  "params": [{
    "from": "0xSequencerAddress...",
    "to": "0xBridgeAddress...",
    "data": "0x...", // ABI encoded function call
    "value": "0x0"
  }],
  "id": 1
}
```

### Response

- **Success**: Returns the transaction hash.
- **Error**: Returns a revert reason. Common errors:
  - `NotSequencer()`: Sender is not the authorized sequencer.
  - `InvalidProof()`: The ZK proof failed verification.
  - `DAProviderNotEnabled()`: Invalid `daId`.

---

## Example: submitting a batch using ethers.js

### Prerequisites
- Node.js
- `ethers` library installed (`npm install ethers`)
- The Bridge ABI (available in `docs/abis/ZKRollupBridge.json`)

### 1. Calldata DA (Traditional)

```javascript
const { ethers } = require("ethers");
const fs = require("fs");

async function submitCalldataBatch() {
    const provider = new ethers.JsonRpcProvider("YOUR_RPC_URL");
    const wallet = new ethers.Wallet("YOUR_PRIVATE_KEY", provider);
    
    const bridgeAbi = JSON.parse(fs.readFileSync("./ZKRollupBridge.json"));
    const bridge = new ethers.Contract("BRIDGE_ADDRESS", bridgeAbi, wallet);

    // Data
    const batchData = ethers.toUtf8Bytes("compressed_tx_data");
    const newRoot = "0x..."; // New state root
    const proof = { a: [0, 0], b: [[0,0],[0,0]], c: [0,0] }; // Your real proof

    // Call commitBatch
    // daId = 0 (Calldata)
    // daMeta = "0x"
    const tx = await bridge.commitBatch(0, batchData, "0x", newRoot, proof);
    console.log("Tx Hash:", tx.hash);
    await tx.wait();
}
```

### 2. Blob DA (EIP-4844)

Submitting blobs requires creating a blob-carrying transaction. Ethers v6 supports this natively.

```javascript
async function submitBlobBatch() {
    // ... setup provider and wallet ...

    // Create a Blob
    const blobData = new Uint8Array(131072).fill(1); // Your data
    // Compute Commitment/Hash (requires kzg-wasm library usually)
    const kzg = await import("kzg-wasm"); // Example
    await kzg.loadTrustedSetup(); 
    const commitment = kzg.blobToKzgCommitment(blobData);
    const versionedHash = kzg.commitmentToVersionedHash(commitment);

    // Metadata for Bridge
    const blobIndex = 0; // Index of blob in this tx
    const daMeta = ethers.AbiCoder.defaultAbiCoder().encode(
        ["bytes32", "uint8"], 
        [versionedHash, blobIndex]
    );

    // Populate transaction
    const txData = await bridge.commitBatch.populateTransaction(
        1, // daId = 1 (Blob)
        "0x", // batchData empty
        daMeta,
        newRoot,
        proof
    );

    // Send Blob Transaction
    const tx = await wallet.sendTransaction({
        to: bridge.target,
        data: txData.data,
        blobs: [blobData],
        kzg: kzg, // Pass the KZG library
        type: 3 // Blob transaction type
    });

    console.log("Blob Tx Hash:", tx.hash);
}
```

## Other Services Needed

To run a complete rollup, you will need:
1. **L2 Node**: To execute transactions.
2. **Prover**: A service (e.g., running `rapidsnark`) to generate the Groth16 proofs.
3. **Database**: To store the Merkle Tree state.
