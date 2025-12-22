# ZK Rollup Bridge Contracts

This repository contains the Solidity smart contracts for an L1 ZK Rollup Bridge, supporting both traditional Calldata Data Availability (DA) and EIP-4844 Blob DA. It includes a complete testing suite with 100% code coverage.

## Features

*   **ZKRollupBridge**: The core contract managing state roots, batch commitments, and proof verification.
    *   Supports **Calldata DA** (legacy).
    *   Supports **Blob DA** (Cancun/EIP-4844), with optional `blobhash` verification.
    *   **Ownable (2-step)** for secure administration.
    *   **Sequencer** role management.
*   **RealVerifier**: A Groth16 verifier implementation (BN254 curve) for production use.
*   **Test Utilities**:
    *   `MockVerifier`: For simulating proof verification results.
    *   `TestRealVerifier`: Wraps the pairing library to verify elliptic curve operations.
    *   `TestZKRollupBridge`: Mocks `blobhash` opcode for testing on non-Cancun environments.

## Prerequisites

*   Node.js (v18 or v20 LTS recommended)
*   npm or yarn

## Installation

```bash
npm install
```

## Compilation

Compile the smart contracts using Hardhat:

```bash
npx hardhat compile
```

## Testing

Run the full test suite:

```bash
npx hardhat test
```

### Coverage

Generate a code coverage report (targeting 100% branch and line coverage):

```bash
npx hardhat coverage
```

## Configuration

The project is configured in `hardhat.config.ts`.
*   **Solidity Version**: 0.8.24
*   **EVM Version**: `cancun` (required for `blobhash`)
*   **Networks**:
    *   `hardhat`: Configured with `cancun` hardfork.
    *   `sepolia`: configured via `.env` (see below).

### Environment Variables

To deploy to a live network, create a `.env` file in the root directory:

```env
PRIVATE_KEY=your_private_key_here
SEPOLIA_RPC_URL=https://sepolia.infura.io/v3/your_api_key
```

## Docker

This project includes a Docker setup for consistent build and testing environments.

### Build Image

```bash
docker build -t zk-rollup-contracts .
```

### Run Tests in Docker

```bash
docker run --rm zk-rollup-contracts
```

## GitHub Actions

A CI/CD pipeline is configured in `.github/workflows/docker-publish.yml`. It automatically builds and pushes the Docker image to the GitHub Container Registry (ghcr.io) on pushes to the `main` branch.

## Project Structure

*   `contracts/`: Solidity source files.
    *   `ZKRollupBridge.sol`: Main bridge contract.
    *   `RealVerifier.sol`: Groth16 verifier logic.
    *   `MockVerifier.sol`: Mock verifier for testing.
*   `test/`: Hardhat tests (TypeScript).
    *   `ZKRollupBridge.test.ts`: Tests for the bridge contract.
    *   `RealVerifier.test.ts`: Tests for the verifier logic.
*   `scripts/`: Deployment scripts (if any).
