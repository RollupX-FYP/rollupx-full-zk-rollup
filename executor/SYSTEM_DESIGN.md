# Executor System Design

## Scope

`zksync_state_machine` is the active executor and proving coordinator.

It owns:
- execution of normalized sequencer transactions,
- state transition trace construction,
- trace integrity persistence,
- RISC0 proof artifact generation,
- enriched batch publication to submitter.

## High-Level Flow

```text
Sequencer PublishBatch (gRPC)
        |
        v
Executor service::publish_batch
  - parse + normalize txs
  - execute STF (tx_engine + state manager)
  - build ExecutionTraceV1
  - persist trace and verify sha256
  - invoke RISC0 host prover
  - verify artifact metadata/hashes
  - publish enriched BatchPayload to stream subscribers
        |
        v
Submitter stream consumer
```

## Core Modules

- `src/service.rs`: gRPC server (`publish_batch`, `stream_batches`) and orchestration.
- `src/tx_engine.rs`: STF execution, tx checks, state diff formation, trace public inputs.
- `src/state.rs`: RocksDB state manager.
- `src/trace.rs`: trace persistence, hash verification, lifecycle append.
- `src/proof.rs`: prover backend integration (`risc0`) and artifact integrity checks.
- `src/block_constructor.rs`: build enriched payload for downstream submitter.

## Lifecycle States

Trace lifecycle entries are appended in order:
- `generated`
- `persisted`
- `proved`
- `published`

Stored in `TRACE_ROOT/index.jsonl`.

## Design Guarantees

- Deterministic trace public input construction for a given batch input.
- Persisted trace hash is immediately re-verified.
- Proof/journal metadata is cryptographically checked before publish.
- Publish fails fast on metadata mismatch.
