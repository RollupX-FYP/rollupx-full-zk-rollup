# RISC Zero Prover Integration

This directory provides a real proof path for the RollupX executor.

## Crates

- `rollup_core`: shared proof input types (`BlockTrace`, `StateDiff`) and lightweight SMT verifier logic.
- `guest`: RISC Zero guest program (the circuit) that verifies state diffs and commits `(initial_root, final_root)`.
- `host`: host prover binary that reads `BlockTrace` JSON + guest ELF and produces proof/journal/metadata artifacts.

## Build

1. Build guest ELF using RISC Zero tooling.
2. Build host binary:
   - `cargo build --release -p rollup_host`

## Run host manually

`rollup_host <block_trace.json> <guest_elf_path> [proof_out] [journal_out] [metadata_out]`

Host writes:
- `proof_out`: proof bytes
- `journal_out`: zkVM journal bytes
- `metadata_out`: `ProofRunMetadata` JSON

---

## Observability & Metrics

The host prover captures granular telemetry about the ZK proving process.

### `ProofRunMetadata` Structure

The metadata JSON contains the following key fields:

| Category | Metric | Description |
| :--- | :--- | :--- |
| **Integrity** | `trace_sha256`, `proof_sha256` | Hashes to verify artifact consistency. |
| | `public_inputs_hash` | SHA256 of `(initial_root, final_root)`. |
| **Timing (ms)** | `witness_generation_ms` | Time to serialize input and setup environment. |
| | `zkvm_execution_ms` | Pure VM execution time. |
| | `proof_compression_ms` | Time to generate the Groth16 SNARK. |
| | `total_prover_wall_ms` | End-to-end host latency. |
| **Resources** | `total_cycles` | Aggregate RISC0 cycles (computational cost). |
| | `total_segments` | Number of VM segments produced. |

### Proving Modes

- **`groth16`**: Full SNARK proof (production).
- **`journal_fallback`**: Mock/Fast mode where only the journal is verified.

---

## Guest Verification Logic

The guest program (`risc0_prover/guest`) acts as the RollupX "Circuit":

1. **Input**: Receives a `BlockTrace` via the VM's input channel.
2. **Replay**: Uses the `LightweightSMT` from `rollup_core` to sequentially apply `StateDiff` entries starting from `initial_root`.
3. **Verification**: Asserts that the final recomputed root matches the `final_root` provided in the trace.
4. **Commit**: Commits the `(initial_root, final_root)` pair to the journal as public output.

---

## Benchmarking Suite Integration

Prover performance is a primary metric for RollupX research.

1. **Cycle Tracking**: The suite uses `total_cycles` to measure how the complexity of transaction types (Class A vs C) impacts ZK resource consumption.
2. **Throughput Bottlenecking**: Prover wall time often dictates the maximum sustainable batch frequency.
3. **Cost Modeling**: Cycle counts are used to project L1 verification costs for various prover configurations.

## Wire into executor

Set environment variables for executor process:

- `RISC0_HOST_BIN=<path-to-rollup_host binary>`
- `RISC0_GUEST_ELF=<path-to-compiled-guest ELF>`
- `RISC0_WORK_DIR=tmp/risc0` (optional)

Executor validates host metadata before publishing batches. If metadata or artifact hashes mismatch, batch publication fails fast.
