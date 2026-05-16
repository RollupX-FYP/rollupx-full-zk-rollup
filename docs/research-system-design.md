# RollupX Research System Design

This document describes the implementation as it exists in this repository. It is written for research and benchmarking use, so it emphasizes component boundaries, state transitions, observability points, and known interpretation limits.

## System Purpose

RollupX is an experimental single-node ZK-rollup pipeline for controlled batching, scheduling, proof, data-availability, and L1 settlement experiments. It is not a production-equivalent rollup. The implemented execution scope is transfer-centric and benchmark-oriented; the benchmark harness is designed to compare configurations under repeatable local conditions.

The end-to-end path is:

```text
benchmark workload
  -> sequencer HTTP API
  -> sequencer mempool + batcher
  -> executor gRPC service
  -> RISC0 proof artifact path
  -> submitter DA/L1 path
  -> local bridge contracts
  -> JSON/JSONL metrics
  -> data-tools aggregation
```

## Component Map

| Component | Main paths | Runtime role | Primary outputs |
|---|---|---|---|
| Workload generator | `benchmark-suite/workload/` | Generates signed synthetic transactions with Poisson arrivals or fixed-count bursts. | `workload_<experiment_id>.json`, `tx_log_<run_id>.csv`, `run_status.json` |
| Sequencer | `sequencer/src/` | Accepts transactions, validates against a pessimistic cache, pools transactions, seals batches, publishes them to executor. | `sequencer_batch_metrics.jsonl`, batch metadata registry |
| Executor | `executor/src/` | Receives sealed batches, executes transfer STF, persists traces, invokes RISC0 host, streams enriched payloads. | `executor_batch_metrics.jsonl`, `executor_<experiment_id>.json`, trace/proof artifacts |
| Prover | `risc0_prover/` | RISC Zero host/guest path that verifies state-diff replay and emits proof/journal metadata. | proof bytes, journal bytes, proof metadata JSON |
| Submitter | `submitter/src/` | Receives executor payloads, applies DA strategy, submits to bridge, writes settlement/cost metrics. | `submitter_metrics.json`, CSV rows in some paths |
| Contracts | `contracts/contracts/` | Local L1 bridge, DA provider, verifier, deposit/withdraw/forced inclusion surface. | L1 state updates and events |
| Data tools | `data-tools/` | Merges run, batch, executor, submitter, and workload outputs for analysis/plots. | `all_results.csv`, `all_batch_results.csv`, plots |
| UI | `zk-rollup-ui/` | Minimal dApp/dashboard shell. | Not central to current benchmark path |

## Sequencer Design

The sequencer is built around four responsibilities:

1. Ingestion: `sequencer/src/api/server.rs` exposes JSON-RPC `sendTransaction` and REST `POST /tx`. The benchmark generator uses `/tx`.
2. Validation: `sequencer/src/validation/validator.rs` checks signature, nonce, and balance against an in-memory state cache.
3. Pooling and batching: `sequencer/src/pool/tx_pool.rs`, `sequencer/src/pool/forced_queue.rs`, and `sequencer/src/batch/`.
4. Publishing: `sequencer/src/batch/orchestrator.rs` serializes sealed batches and publishes them to executor gRPC.

The normal transaction path records `arrived_at`, `pool_entry_at`, and `validation_latency_ms` in `PooledTransaction`. After a transaction validates, the sequencer pessimistically deducts balance and increments nonce in the cache before inserting into the pool. This supports concurrent ingestion, but it also means the sequencer's validation model is only as valid as the cache initialization/reset discipline.

Batch sealing supports these trigger classes:

| Trigger | Implementation | Meaning |
|---|---|---|
| Forced transactions | `BatchTrigger::should_seal` | L1-forced operations seal immediately. |
| Size threshold | `target_batch_size_for_depth` | Seal when pending normal transactions reach the configured fixed/adaptive target. |
| Timeout | `timeout_interval_ms` | Seal partial batches after timeout if any transactions are waiting. |

Scheduling policies are implemented in `sequencer/src/scheduler/policies.rs`:

| Policy | Behavior |
|---|---|
| `FCFS` | Preserves pool order. |
| `FeePriority` | Sorts by gas price descending. |
| `TimeBoost` | Sorts by time window, boost bid, then gas price. |
| `FairBFT` | Sorts by transaction timestamp in a single-node approximation. |
| `BlobPacking` | Favors larger estimated encoded payloads, then higher gas price, then earlier timestamp. |

Adaptive batching is threshold-based. If `batch_policy = "adaptive"`, the target is selected from small, medium, and large target sizes according to current mempool depth. For blob packing, the pool has a separate greedy selector that chooses transactions fitting within `blob_target_bytes` and preserves unselected transactions in arrival order.

## Executor Design

The executor gRPC server lives in `executor/src/service.rs`. It accepts `BatchPayload` from the sequencer, parses transactions, queues work asynchronously, and emits a processed/enriched stream for submitter consumption.

The execution engine in `executor/src/tx_engine.rs` is intentionally simple:

1. Verify ECDSA signatures.
2. Check sender nonce and balance.
3. Apply transfer-style state updates.
4. Generate sender/receiver state diffs.
5. Fold diffs into a lightweight hash-derived root.
6. Build `ExecutionTraceV1` with public inputs, outcomes, state diffs, and phase timings.

This is not an EVM-equivalent STF. It is useful for controlled comparisons where the research question is about batching, DA, proof overheads, and pipeline behavior, not arbitrary smart-contract execution.

The executor persists traces, verifies trace hashes, invokes the RISC0 proof backend, verifies artifact metadata, and writes per-batch metrics. It also stores run-level executor stats keyed by experiment id.

## Prover Design

The prover integration is in `executor/src/proof.rs` and `risc0_prover/`.

The executor writes a converted `rollup_core::BlockTrace`, invokes the RISC0 host binary, then reads:

- proof bytes,
- journal bytes,
- proof metadata,
- proof/journal hashes,
- timing/cycle metadata.

The host metadata must satisfy integrity checks before the executor publishes the enriched payload. The accepted proof mode is `groth16` unless fallback is explicitly allowed by environment. The guest verifies that state diffs replay from the initial root to the final root and commits the public root pair.

Research interpretation: this proves consistency of the repository's lightweight state-diff model, not the validity of a full L2 VM.

## Submitter Design

The submitter has two paths:

1. A domain/application orchestrator in `submitter/src/application/orchestrator.rs` for batch lifecycle state transitions.
2. The benchmark daemon in `submitter/src/daemon.rs`, which is the main path for gRPC batch ingestion and metric emission.

The benchmark daemon receives executor batches, writes batch data to a file, formats proof bytes, applies a DA strategy, submits to the bridge, and records final metrics.

DA strategies:

| Mode | Path | Behavior |
|---|---|---|
| Calldata | `submitter/src/infrastructure/da_calldata.rs` | Sends batch data directly in `commitBatch`. |
| Blob | `submitter/src/infrastructure/da_blob.rs` | Compresses/archives payload data and submits a blob metadata commitment path. Local mode may use hybrid estimated blob costs. |
| Offchain | `submitter/src/infrastructure/da_offchain.rs` | Stores data in a local directory and sends only a pointer/commitment. Marked simulated. |

The submitter also contains retry/gas-bump state in `submitter/src/saga.rs`, which matters for latency validity because gas-bumped rows are not directly comparable to non-bumped rows.

## Contract Design

The bridge contract is `contracts/contracts/bridge/ZKRollupBridge.sol`. It supports:

- ETH/ERC20 deposits,
- withdrawals via Merkle proof against `latestStateRoot`,
- sequencer-controlled `commitBatch`,
- configurable DA providers,
- configurable verifier ids,
- forced transaction queue/freeze logic,
- optimistic mode stub.

`commitBatch` computes a DA commitment via the selected DA provider, validates DA, verifies proof through the selected verifier, emits soft commit/final commit events, and updates `latestStateRoot`. Local experiments usually run against Hardhat-style local deployments, so the contract path validates integration mechanics more than public-network economics.

## Data Tools Design

`data-tools/aggregate.py` scans benchmark run directories, loads metadata, workload status, executor summaries, sequencer JSONL, executor JSONL, and submitter JSONL, and writes merged CSVs.

The intended run layout is:

```text
benchmark-suite/metrics/<experiment_id>/<run_id_timestamp>/
  run.log
  run_metadata.json
  run_status.json
  workload_<experiment_id>.json
  tx_log_<run_id>.csv
  sequencer_batch_metrics.jsonl
  executor_batch_metrics.jsonl
  submitter_metrics.json
  diagnostics/
```

## Main Design Boundaries

The strongest supported research questions are comparative:

- How do fixed vs adaptive batch targets affect batch size, wait time, throughput, and cost proxies under identical local workloads?
- How do FCFS, fee-priority, and blob-packing policies change batch composition and DA utilization proxies?
- How do calldata, blob-like, and offchain DA strategies compare under the repository's local cost model?
- Where does the local pipeline spend time across sequencing, execution, proof, and submission?

The weakest supported claims are external-validity claims:

- real mainnet cost predictions,
- EVM-equivalent rollup performance,
- decentralized sequencer fairness,
- production-grade proof/security guarantees,
- public-network blob market behavior.

