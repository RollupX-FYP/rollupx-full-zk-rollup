# RollupX Architecture

RollupX is an experimental, modular Layer-2 ZK-Rollup prototype. Its primary design goal is high throughput, custom transaction scheduling, and comprehensive observability across various components. 

The system implements the full transaction lifecycle from inception to execution. However, **the Zero-Knowledge proving subsystem (Prover) is currently entirely mocked/stubbed**, and data availability (DA) is posted to a local L1 (Ethereum/Hardhat) with verification checks that utilize mock verification responses. 

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

See [USAGE.md](USAGE.md) for a comprehensive guide on starting the system components, generating workloads, running automated benchmarks, and using the data tools.
