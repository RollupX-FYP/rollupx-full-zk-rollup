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
- **L1 Contracts -> Sequencer:** WebSocket Events. The Sequencer's `L1 Listener` monitors the Bridge contract for `Deposit` and `ForcedTransaction` events to populate the high-priority `Forced Queue`.
- **Sequencer -> Executor:** gRPC (`PublishBatch`). The Sequencer batches transactions and pushes the payload forward.
- **Executor -> Submitter:** gRPC (`StreamBatches`). The Submitter polls the Executor's broadcast channel to ingest pending payloads.
- **Submitter -> L1 Contracts:** Standard Web3 RPC (`eth_sendRawTransaction`).

## Metrics Synchronization & Stabilization

For high-accuracy benchmarking, the system employs a **stabilization loop** within `run_experiment.sh`:
1. **Poll Metrics:** The orchestrator polls the `sequencer_batches_<exp>.jsonl`, `executor_metrics.jsonl`, and `submitter_metrics.json` files.
2. **Verify Integrity:** It ensures that every `batch_id` produced by the Sequencer has a corresponding execution entry in the Executor and a settlement receipt in the Submitter.
3. **Wait for Quiescence:** The experiment only exits when the metrics files stop growing for a configurable number of seconds (`COMPONENT_STABLE_POLLS`), ensuring that late-arriving settlement data is captured.

## Important Runtime Distinctions

**The Pass-Through Executor:**
In the standard benchmarking environment (i.e., `EXECUTOR_MODE=grpc`), the Executor does *not* invoke the EraVM. It operates as a stateless pass-through relay. It logs performance metrics (`batch_count`, `duration_s`) and instantly broadcasts the `BatchPayload` to the Submitter. Any true state-transition execution logic is bypassed to facilitate rapid data-availability (DA) and networking testing.

**The Mocked Prover:**
Similarly, the Zero-Knowledge proving subsystem is stubbed. The Submitter interacts with a `MockProofProvider` that simulates computational delay but ultimately returns a dummy proof string (`0x00...00`). The L1 smart contracts (`MockVerifier.sol`) are configured to blindly accept these dummy proofs during benchmark execution.
