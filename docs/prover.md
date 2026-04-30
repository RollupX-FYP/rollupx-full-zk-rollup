# Prover

RollupX uses a real RISC0 host/guest proving path through `risc0_prover/`.

## Components

- `risc0_prover/rollup_core`
  - Shared proof input model (`BlockTrace`, `StateDiff`)
  - Lightweight state-diff verifier logic used by guest
- `risc0_prover/host/guest`
  - Guest zkVM program
  - Reads `BlockTrace`, applies diffs, commits `(initial_root, final_root)`
- `risc0_prover/host`
  - Host prover binary (`rollup_host`)
  - Produces:
    - proof bytes
    - journal bytes
    - metadata json with integrity hashes/sizes

## Host CLI

```bash
rollup_host <block_trace.json> [guest_elf_path] [proof_out] [journal_out] [metadata_out]
```

## Executor Integration

Executor invokes host binary from `executor/src/proof.rs` and validates metadata before publishing batches.

Environment:
- `PROVER_BACKEND=risc0`
- `RISC0_HOST_BIN=/abs/path/to/rollup_host`
- `RISC0_GUEST_ELF` (optional when host embeds guest ELF)
- `RISC0_WORK_DIR` (optional)
- `ALLOW_PROOF_FALLBACK=1` (optional override; default is strict `groth16`-only acceptance in executor)

## Artifact Contract

For each batch, executor expects:
- `trace_<batch_id>.json`
- `proof_<batch_id>.bin`
- `journal_<batch_id>.bin`
- `proof_meta_<batch_id>.json`

Metadata must satisfy:
- `status == "ok"`
- `proof_mode == "groth16"` by default (unless `ALLOW_PROOF_FALLBACK=1`)
- trace hash agreement
- expected public input hash agreement
- proof/journal hash+size agreement
