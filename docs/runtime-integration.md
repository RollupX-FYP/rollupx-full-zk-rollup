# Runtime Integration

The end-to-end integration is primarily driven by the `benchmark-suite/scripts/run_experiment.sh` orchestration script.

## The Benchmark Orchestrator

The benchmark orchestration automates configuration matrix testing by seamlessly managing the runtime lifecycle of the system.

### Integration Flow
1. **Prepare Output Directory:** It sets up an isolated directory for the metrics, e.g., `benchmark-suite/metrics/<experiment_id>/<run_id>/`.
2. **Collect Environment Metadata:** It invokes `collect_env.sh` to record the exact git commit, machine configuration (CPU/RAM/OS), and language versions (Python/Rust). This is stored in `run_metadata.json`.
3. **Configure Sequencer:** It writes a dynamic `seq_config.toml` customized for the parameters of the test (batch size, timeout, scheduling policy).
4. **Start Sequencer:** It spins up the `rollup_sequencer` process in the background. It utilizes a polling script (`wait_for_sequencer.sh`) to ensure the REST and gRPC endpoints are healthy before proceeding.
5. **Run Workload Generator:** It executes the Python `poisson_generator.py` script. This script connects to the Sequencer via HTTP POST and simulates synthetic transactions at the configured TPS rate and type mix (e.g., balanced, heavy).
6. **Wait for Submitter:** It actively monitors the `submitter_metrics.json` output file to ensure that all processed batches have correctly landed on the Ethereum L1 node.
7. **Cleanup:** It records the final timestamp in `run_metadata.json` and kills the Sequencer background process.

### Runtime Boundaries
The runtime components communicate across clear boundaries:
- **Workload -> Sequencer:** JSON-RPC over HTTP (`POST /tx`).
- **Sequencer -> Executor:** gRPC (`PublishBatch`). The Sequencer batches transactions and pushes the payload forward.
- **Executor -> Submitter:** gRPC (`StreamBatches`). The Submitter polls the Executor's broadcast channel to ingest pending payloads.
- **Submitter -> L1 Contracts:** Standard Web3 RPC (`eth_sendRawTransaction`).

## Important Runtime Distinctions

**The Pass-Through Executor:**
In the standard benchmarking environment (i.e., `EXECUTOR_MODE=grpc`), the Executor does *not* invoke the EraVM. It operates as a stateless pass-through relay. It logs performance metrics (`batch_count`, `duration_s`) and instantly broadcasts the `BatchPayload` to the Submitter. Any true state-transition execution logic is bypassed to facilitate rapid data-availability (DA) and networking testing.

**The Mocked Prover:**
Similarly, the Zero-Knowledge proving subsystem is stubbed. The Submitter interacts with a `MockProofProvider` that simulates computational delay but ultimately returns a dummy proof string (`0x00...00`). The L1 smart contracts (`MockVerifier.sol`) are configured to blindly accept these dummy proofs during benchmark execution.
