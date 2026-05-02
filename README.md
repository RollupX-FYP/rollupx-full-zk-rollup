# RollupX Architecture

RollupX is an experimental, modular Layer-2 ZK-Rollup prototype. Its primary design goal is high throughput, custom transaction scheduling, and comprehensive observability across various components. 

The system implements a full transaction lifecycle from ingestion to execution, trace persistence, RISC0-based proof artifact generation, and gRPC publication to the submitter.

## Current Runtime Status

- `Sequencer` accepts user transactions over JSON-RPC and seals batches.
- `Executor` runs STF execution, persists execution traces, verifies trace hashes, invokes RISC0 host proving, and publishes enriched batch payloads over gRPC.
- `Submitter` consumes gRPC batch stream and performs settlement flow to the local L1 test deployment.
- `risc0_prover` is now in the repository root and is used as the real proof path backend for executor (`PROVER_BACKEND=risc0`).

Known scope limits:
- STF is transfer-centric research logic, not full EVM-equivalent execution.
- Results are intended for controlled benchmarking and tuning experiments.

## Documentation Map

- [docs/system-overview.md](docs/system-overview.md): High-level overview of the system architecture and components.
- [docs/e2e-flow.md](docs/e2e-flow.md): End-to-end transaction lifecycle from generation to L1 settlement.
- [docs/ui.md](docs/ui.md): Overview of the partially implemented Next.js frontend dashboard.
- [docs/workload-generator.md](docs/workload-generator.md): Details on the Python-based workload generator.
- [docs/sequencer.md](docs/sequencer.md): Deep dive into the Sequencer microservice.
- [docs/executor.md](docs/executor.md): Details on the Executor microservice.
- [docs/prover.md](docs/prover.md): Information regarding the mocked Prover subsystem.
- [docs/submitter.md](docs/submitter.md): Documentation on the Submitter microservice.
- [docs/contracts.md](docs/contracts.md): Details on the L1 Solidity smart contracts.
- [docs/data-tools.md](docs/data-tools.md): Guide to the data analysis and visualization pipeline.
- [docs/runtime-integration.md](docs/runtime-integration.md): Information on how the components integrate during runtime.
- [docs/known-gaps.md](docs/known-gaps.md): Known limitations, mocked areas, and incomplete flows.

## Recommended Reading Order

1. [docs/system-overview.md](docs/system-overview.md)
2. [docs/e2e-flow.md](docs/e2e-flow.md)
3. Component-specific documentation as needed (e.g., [docs/sequencer.md](docs/sequencer.md), [docs/executor.md](docs/executor.md))
4. [docs/data-tools.md](docs/data-tools.md)
5. [docs/known-gaps.md](docs/known-gaps.md)

## Usage

See [USAGE.md](USAGE.md) for:
- new-environment setup (toolchains and dependencies),
- full local stack bring-up (Hardhat + deploy + executor + submitter + sequencer),
- E2E workload replay,
- benchmark and analysis pipeline steps.
