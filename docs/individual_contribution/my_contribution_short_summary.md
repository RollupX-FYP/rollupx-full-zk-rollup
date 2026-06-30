# Individual Contribution Summary

In this final year project, my primary focus was architecting the L1 interaction layer and constructing the rigorous benchmarking infrastructure necessary to evaluate our modular ZK-rollup prototype. 

I led the implementation of the Submitter service, a critical daemon responsible for committing L2 state transitions and proofs to the L1 network. I significantly refactored the Submitter (`submitter/src/application/orchestrator.rs`) to follow Domain-Driven Design principles. By introducing a mockable `BridgeClient` trait (`submitter/src/infrastructure/ethereum_adapter.rs`), I allowed for robust unit testing of different Data Availability (DA) strategies, increasing verifiable test coverage from 41% to 88%. I implemented support for EIP-4844 Blobs (`da_blob.rs`), Calldata (`da_calldata.rs`), and Off-chain DA, and fixed critical reliability bugs (e.g., catching silent L1 transaction EVM reverts). I also developed the corresponding L1 smart contracts (`ZKRollupBridge.sol`, DA interfaces, and Verifiers) to handle settlement.

Beyond the core system architecture, I engineered the comprehensive `benchmark-suite`. This included a configurable workload generator (`benchmark-suite/workload/poisson_generator.py`) capable of simulating mathematically grounded transaction traffic (Poisson arrival processes). I developed shell and Python orchestration scripts (`run_matrix.sh`) that automate the execution of complex experiment matrices, fixing critical race conditions during Docker startup (`wait_for_sequencer.sh`).

To make sense of the distributed system logs, I built the `data-tools` pipeline. This suite of Python scripts (`data-tools/aggregate.py`, `data-tools/plots/latency_cdf.py`) aggregates JSONL metrics from the Sequencer, Executor, and Submitter. It calculates critical statistics like achieved throughput, latency percentiles, and cost breakdowns, directly powering the analytical figures in our final report.

Ultimately, my work transformed isolated services into a cohesive, end-to-end reproducible research pipeline: from workload generation and L2 sequencing, through L1 submission and verification, to automated data analysis.
