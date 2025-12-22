# ZK Rollup Bridge Contracts

[![CI](https://github.com/RollupX-FYP/contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/RollupX-FYP/contracts/actions/workflows/ci.yml)
[![Security](https://github.com/RollupX-FYP/contracts/actions/workflows/security.yml/badge.svg)](https://github.com/RollupX-FYP/contracts/actions/workflows/security.yml)
[![Docker](https://github.com/RollupX-FYP/contracts/actions/workflows/docker-publish.yml/badge.svg)](https://github.com/RollupX-FYP/contracts/actions/workflows/docker-publish.yml)
[![codecov](https://codecov.io/gh/RollupX-FYP/contracts/branch/main/graph/badge.svg)](https://codecov.io/gh/RollupX-FYP/contracts)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Solidity](https://img.shields.io/badge/Solidity-0.8.24-e6e6e6?logo=solidity&logoColor=black)](https://docs.soliditylang.org/en/v0.8.24/)
[![Node.js](https://img.shields.io/badge/Node.js-18%20|%2020-339933?logo=nodedotjs&logoColor=white)](https://nodejs.org/)

This repository contains the Solidity smart contracts for an L1 ZK Rollup Bridge, supporting both traditional Calldata Data Availability (DA) and EIP-4844 Blob DA. It includes a complete testing suite with 100% code coverage.

## Features

- **ZKRollupBridge**: The core contract managing state roots, batch commitments, and proof verification.
  - **Architecture**: Built using Domain-Driven Design (DDD) and SOLID principles.
  - **Modular DA**: Uses the Strategy Pattern to support multiple DA Providers (Calldata, Blob).
  - **Security**: Allowlisted DA providers, immutable verifier, and strict state transition boundaries.
  - **Ownable (2-step)** for secure administration.
  - **Sequencer Modes**: Supports a restricted mode (only `sequencer` can submit) and a permissionless dev mode (if `sequencer` is `address(0)`).
- **RealVerifier**: A Groth16 verifier implementation (BN254 curve) for production use.
- **Test Utilities**:
  - `MockVerifier`: For simulating proof verification results.
  - `TestRealVerifier`: Wraps the pairing library to verify elliptic curve operations.
  - `TestBlobDA`: Mocks `blobhash` opcode for testing on non-Cancun environments.
  - `TestZKRollupBridge`: Wrapper around the bridge to expose internal functions if needed for testing.

## Documentation

- **[BEST_PRACTICES.md](BEST_PRACTICES.md)**: Detailed guide on the architectural decisions, DDD boundaries, SOLID principles, and security standards used in this project.
- **[AGENTS.md](AGENTS.md)**: Instructions and context for AI agents working on this codebase.

## Prerequisites

- Node.js (v18 or v20 LTS recommended)
- npm or yarn

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

- **Solidity Version**: 0.8.24
- **EVM Version**: `cancun` (required for `blobhash`)
- **Networks**:
  - `hardhat`: Configured with `cancun` hardfork.
  - `sepolia`: configured via `.env` (see below).

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

The project follows a modular structure based on DDD layers:

- `contracts/`
  - `bridge/`: **Aggregate Root** (`ZKRollupBridge.sol`) - Core settlement logic.
  - `interfaces/`: **Abstractions** (`IVerifier.sol`, `IDAProvider.sol`) - Dependency inversion.
  - `da/`: **Strategies** (`CalldataDA.sol`, `BlobDA.sol`) - Data Availability implementations.
  - `verifiers/`: **Domain Services** (`RealVerifier.sol`, `MockVerifier.sol`) - Cryptographic verification.
  - `libraries/`: **Shared Logic** (`Pairing.sol`) - Elliptic curve math.
- `test/`: Hardhat tests (TypeScript).
  - `ZKRollupBridge.test.ts`: Integration tests for the bridge and DA strategies.
  - `RealVerifier.test.ts`: Unit tests for the verifier and pairing library.
