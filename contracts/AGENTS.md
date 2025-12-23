# Agent Instructions (`AGENTS.md`)

This file contains instructions and context for AI agents working on this repository.

## 1. Context & Architecture
This project implements an **L1 ZK Rollup Bridge** using **Domain-Driven Design (DDD)** and **SOLID** principles.
- **Aggregate Root**: `ZKRollupBridge` (in `contracts/bridge/`). It manages the state and orchestrates validation.
- **DA Strategies**: Data Availability logic is delegated to `IDAProvider` implementations (in `contracts/da/`).
- **Boundaries**: The bridge **must not** contain business logic for specific DA types (e.g., parsing blobs). It only handles commitments.

## 2. Mandatory Checks
Before submitting any changes, you **MUST**:
1.  **Run Tests**: `npx hardhat test` (All tests must pass).
2.  **Verify Coverage**: Ensure 100% branch and line coverage.
    - *Note*: If `npx hardhat coverage` is not available, manually verify that your tests cover all conditional branches.
3.  **Verify Docker Build**: Ensure the application builds correctly in a container.
    - Run: `docker build -t zk-rollup-contracts .` (if environment permits) or verify `npm ci` and `npx hardhat compile` run cleanly.

## 3. Coding Guidelines
- **Adding new DA types**:
    - Create a new contract in `contracts/da/` implementing `IDAProvider`.
    - Do **not** modify `ZKRollupBridge.sol` logic.
    - Add the new provider to the registry in tests.
- **Modifying the Bridge**:
    - Only modify `contracts/bridge/ZKRollupBridge.sol` if the core settlement logic changes.
    - Ensure state transitions only occur in `_finalizeBatch`.
- **Verifier**:
    - Keep the verifier reference `immutable`.
    - If upgrading, use a Router pattern, not a setter.
- **Constants**:
    - Use `contracts/libraries/Constants.sol` for shared cryptographic constants (SNARK scalar field, Prime Q).

## 4. Testing Pattern
- **Blobhash Mocking**:
    - The `BlobDA` contract uses a virtual `_getBlobHash` function.
    - In tests, use `TestBlobDA` (in `contracts/test/`) which overrides this function to return mock hashes.
    - **Do not** attempt to rely on actual `blobhash` opcode in standard Hardhat networks unless specifically configured.

## 5. Documentation
- Refer to `BEST_PRACTICES.md` for detailed architectural decisions.
- Keep `README.md` updated if directory structure changes.

## 6. Technical Constraints
- **Node.js Version**: The `crytic/slither-action` requires `node-version: 20` to prevent execution errors.
- **CodeQL**: GitHub Actions workflows utilizing CodeQL must use `v4` actions (`github/codeql-action/*@v4`).
- **Contract Names**: Duplicate contract names in different files (e.g., `contracts/MockVerifier.sol` vs `contracts/verifiers/MockVerifier.sol`) must be avoided to prevent Hardhat artifact collision errors.
- **Dependencies**: Peer dependencies for `@nomicfoundation/hardhat-toolbox` must be explicitly installed in `devDependencies`.
- **Package Lock**: `package-lock.json` must be explicitly unignored in `.gitignore` and committed.
