# ZK Rollup Bridge Contracts

[![CI](https://github.com/RollupX-FYP/contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/RollupX-FYP/contracts/actions/workflows/ci.yml)
[![Security](https://github.com/RollupX-FYP/contracts/actions/workflows/security.yml/badge.svg)](https://github.com/RollupX-FYP/contracts/actions/workflows/security.yml)
[![Docker](https://github.com/RollupX-FYP/contracts/actions/workflows/docker-publish.yml/badge.svg)](https://github.com/RollupX-FYP/contracts/actions/workflows/docker-publish.yml)
[![codecov](https://codecov.io/gh/RollupX-FYP/contracts/branch/main/graph/badge.svg)](https://codecov.io/gh/RollupX-FYP/contracts)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Solidity](https://img.shields.io/badge/Solidity-0.8.24-e6e6e6?logo=solidity&logoColor=black)](https://docs.soliditylang.org/en/v0.8.24/)
[![Node.js](https://img.shields.io/badge/Node.js-18%20|%2020-339933?logo=nodedotjs&logoColor=white)](https://nodejs.org/)

This repository contains the Solidity smart contracts for an L1 ZK Rollup Bridge, supporting both traditional Calldata Data Availability (DA) and EIP-4844 Blob DA.

## 📚 Documentation

Detailed documentation is available in the `docs/` folder:

- **[System Architecture](docs/ARCHITECTURE.md)**: Overview of the rollup components (Sequencer, Prover, Relayer) and data flow.
- **[API Reference](docs/API.md)**: Detailed specifications of smart contract functions and events.
- **[Integration Guide](docs/INTEGRATION.md)**: How to communicate with the contracts using JSON-RPC and `ethers.js`.
- **[Best Practices](BEST_PRACTICES.md)**: Architectural decisions and security standards.
- **[Local Testing](../LOCAL_TESTING.md)**: Guide to running the full pipeline locally.

## 🚀 Quick Start (Docker)

The easiest way to verify the contracts is using Docker.

```bash
# 1. Build the image
docker build -t zk-rollup-contracts .

# 2. Run tests inside the container
docker run --rm zk-rollup-contracts
```

## 🛠 Local Development

### Prerequisites
- Node.js v20 (LTS)
- npm or yarn

### Installation

```bash
npm install
```

### Compilation

```bash
npx hardhat compile
```

### Testing

Run the full test suite (100% coverage):

```bash
npx hardhat test
```

Generate coverage report:
```bash
npx hardhat coverage
```

## 🛠 Local Simulation & Integration

We provide scripts to simulate the full L2-to-L1 pipeline locally. This involves deploying a test network, mocking the L2 executor output, and validating the state transitions.

1.  **Deploy Test Network:**
    ```bash
    npx hardhat run scripts/deploy-test-net.ts --network localhost
    ```
    Deploys `ZKRollupBridge`, `MockVerifier`, and `TestBlobDA` (for mocking EIP-4844 blobs locally).

2.  **Generate Mock Batch:**
    ```bash
    npx hardhat run scripts/generate-mock-batch.ts --network localhost
    ```
    Generates a valid batch and configures the Rust Submitter.

3.  **Verify State:**
    ```bash
    npx hardhat run scripts/verify-state.ts --network localhost
    ```
    Asserts that the Bridge contract state matches the submitted batch.

See `../LOCAL_TESTING.md` for the complete end-to-end guide.

## 📦 Deployment

### Configuration
1. Create a `.env` file:
   ```env
   PRIVATE_KEY=0x...
   SEPOLIA_RPC_URL=https://sepolia.infura.io...
   ```
2. Configure `hardhat.config.ts` if targeting other networks.

### Deploy Command
```bash
npx hardhat run scripts/deploy.ts --network sepolia
```
*(Note: You may need to create a deployment script first, as this repo focuses on contract logic/tests)*

## 🧩 Features

- **ZKRollupBridge**: 
  - **Modular DA**: Pluggable strategies for Calldata or Blob DA.
  - **Permissionless Mode**: Set sequencer to `address(0)` for open access during dev.
  - **State Verification**: Groth16 proof verification on BN254.
- **Security**:
  - Ownable (2-step transfer).
  - Immutable verifiers.
  - Strict input validation (scalar field reduction).

## 📄 Contract ABIs

Auto-generated ABIs are hosted on GitHub Pages:
- [ZKRollupBridge.json](https://rollupx-fyp.github.io/contracts/abis/ZKRollupBridge.json)
- [BlobDA.json](https://rollupx-fyp.github.io/contracts/abis/BlobDA.json)
- [RealVerifier.json](https://rollupx-fyp.github.io/contracts/abis/RealVerifier.json)

You can generate them locally via:
```bash
npm run export-abis
```
