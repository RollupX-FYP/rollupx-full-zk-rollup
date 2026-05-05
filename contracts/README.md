# RollupX Smart Contracts

This directory contains the Layer-1 Solidity smart contracts for the RollupX prototype. The system uses a modular design to support research into different Data Availability (DA) strategies and ZK-proof verification systems.

## Overview

The core of the system is the `ZKRollupBridge.sol`, which orchestrates state transitions, deposits, and withdrawals.

### Components

- **Bridge**: `contracts/bridge/ZKRollupBridge.sol` — The aggregate root managing the L2 state root and batch finalization.
- **DA Providers**: `contracts/da/` — Strategy contracts for different DA layers:
    - `CalldataDA`: Standard L1 calldata.
    - `BlobDA`: EIP-4844 Blob transactions.
    - `OffChainDA`: Data stored off-chain with an on-chain commitment.
- **Verifiers**: `contracts/verifiers/` — Support for multiple ZK backends:
    - `Groth16Verifier`: BN254 SNARKs (Standard).
    - `MockVerifier`: No-op verifier for rapid prototyping.

## Key Features

- **Multi-Verifier Support**: Hot-swap between different proof systems via an admin registry.
- **Modular DA**: Strategy pattern for switching DA modes without bridge modification.
- **Censorship Resistance**: Forced inclusion mechanism with a "Bridge Freezing" state if the sequencer fails to include a user's transaction.
- **Optimistic Fallback**: Support for an optimistic state transition mode with a challenge period.

## Development

This project uses both **Foundry** (for unit testing and gas snapshots) and **Hardhat** (for deployment and local node simulation).

### Prerequisites

- [Foundry](https://book.getfoundry.sh/getting-started/installation)
- Node.js & npm

### Usage

**Compile:**
```bash
forge build
# or
npx hardhat compile
```

**Run Unit Tests:**
```bash
forge test
```

**Local Deployment (Hardhat):**
```bash
# In one terminal
npx hardhat node

# In another
npx hardhat run scripts/deploy-local.ts --network localhost
```

**Gas Reporting:**
```bash
npx hardhat test --network hardhat
```
See `reports/gas-report.txt` for detailed function-level measurements.

## Configuration

Research-specific configurations (block times, reference gas prices) are defined in `hardhat.config.ts`.
