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
- `metadata_out`: integrity metadata (sha256 hashes, sizes, public input hash)

## Wire into executor

Set environment variables for executor process:

- `RISC0_HOST_BIN=<path-to-rollup_host binary>`
- `RISC0_GUEST_ELF=<path-to-compiled-guest ELF>`
- `RISC0_WORK_DIR=tmp/risc0` (optional)

Executor validates host metadata before publishing batches. If metadata or artifact hashes mismatch, batch publication fails fast.
