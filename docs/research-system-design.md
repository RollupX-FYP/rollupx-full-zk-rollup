# RollupX Research System Design

This document describes the current RollupX implementation as a research benchmark system. It is scoped to the repository implementation, not to a production rollup deployment.

## System Purpose

RollupX is a single-node experimental ZK-rollup pipeline used to study batching, scheduling, simplified execution, proof generation, data availability, L1 submission, and measurement design under controlled workloads.

The implemented end-to-end path is:

```text
benchmark workload
  -> sequencer HTTP API
  -> sequencer validation, nonce cache, mempool
  -> batch scheduler and batch orchestrator
  -> executor gRPC service
  -> simplified state transition and RISC0 proof path
  -> submitter DA/L1 path
  -> local bridge contracts
  -> raw JSON/JSONL/CSV metrics
  -> analysis report and data-tools aggregation
```

The strongest intended use is comparative engineering research: testing how controlled changes to batch size, scheduling policy, DA mode, offered load, transaction mix, or sender concurrency affect this prototype.

## Component Map

| Component | Main paths | Role | Primary outputs |
|---|---|---|---|
| Workload generator | `benchmark-suite/workload/` | Generates signed synthetic transactions with controlled rate, mix, seed, sender count, and concurrency. | `workload_<experiment_id>.json`, `tx_log_<run_id>.csv`, `run_status.json` |
| Benchmark harness | `benchmark-suite/scripts/` | Resets state, starts Docker stack, runs workload, waits for metrics, validates run, generates report. | `run.log`, `run_metadata.json`, diagnostics, analysis report |
| Sequencer | `sequencer/src/` | Accepts transactions, validates signature/nonce/balance, maintains mempool, seals batches, publishes to executor. | `sequencer_batch_metrics.jsonl`, registry metadata |
| Executor | `executor/src/` | Executes simplified transfer-centric STF, persists traces, invokes proof backend, emits enriched batches. | `executor_batch_metrics.jsonl`, `executor_<experiment_id>.json`, traces/proof artifacts |
| Prover | `risc0_prover/` | RISC Zero guest/host path for state-diff replay proof. | proof bytes, journal bytes, proof metadata |
| Submitter | `submitter/src/` | Applies DA mode, submits batches to local bridge, records settlement and cost metrics. | `submitter_metrics.json` JSONL rows |
| Contracts | `contracts/contracts/` | Local bridge, DA provider, verifier interfaces, forced inclusion surfaces. | L1 state/events/receipts |
| Data tools | `data-tools/` | Aggregates run directories and plots derived results. | `all_results.csv`, `all_batch_results.csv`, plots |
| UI | `zk-rollup-ui/` | Dashboard/dApp shell, not central to benchmark validity. | UI assets |

## Sequencer Design

The sequencer has four main responsibilities:

1. Ingestion: `sequencer/src/api/server.rs` exposes JSON-RPC and REST `POST /tx`; the benchmark uses REST.
2. Validation: `sequencer/src/validation/validator.rs` checks signature, nonce, and balance against the state cache.
3. Pooling/scheduling: `sequencer/src/pool/`, `sequencer/src/scheduler/`, and `sequencer/src/batch/`.
4. Publishing: `sequencer/src/batch/orchestrator.rs` serializes sealed batches and publishes them to executor.

Accepted transactions are converted to `PooledTransaction` and carry arrival/pool timestamps used for wait-time and ordering metrics. The sequencer pessimistically updates nonce and balance in `StateCache` during admission, so scheduling sees a locally consistent transaction stream as long as the cache has been seeded correctly.

Batch sealing can be triggered by:

| Trigger | Meaning |
|---|---|
| Size threshold | Pending normal transactions reach the current fixed/adaptive target. |
| Timeout | Pending transactions waited longer than `TIMEOUT_MS`. |
| Forced queue | Forced L1 transactions should be sealed immediately. |

Scheduling policies:

| Policy | Behavior |
|---|---|
| `FCFS` | Keeps pool arrival order. |
| `FeePriority` | Sorts by gas price descending. |
| `TimeBoost` | Uses time window, boost bid, and gas price. |
| `FairBFT` | Single-node timestamp ordering approximation. |
| `BlobPacking` | Uses nonce-safe fill-first greedy selection for blob-sized batches. |

### BlobPacking Design

`TransactionPool::take_blob_packed_nonce_safe` implements the current robust blob scheduler:

1. Drain a pool snapshot with original indexes.
2. Group by sender address.
3. Sort each sender group by nonce and arrival index.
4. Mark only the contiguous prefix from the expected state-cache nonce as eligible.
5. Treat nonce gaps as ineligible and keep those transactions pending.
6. Sort eligible transactions by estimated encoded bytes descending, then gas price descending, then arrival time/index.
7. Greedily select transactions that fit `max_count` and `blob_target_bytes`.
8. Restore unselected/ineligible transactions to the pool in original arrival order.
9. Return selected transactions in original arrival order for deterministic, nonce-safe execution.

The selector returns metrics including selected bytes, eligible bytes, eligible transaction count, nonce-gap count, truncated sender count, and low-fill reason.

## Executor Design

The executor service in `executor/src/service.rs` accepts sealed batches over gRPC. It parses transactions, queues work, executes a simplified transfer-centric state transition, persists traces, invokes the proof path, and forwards enriched output for submission.

The core STF in `executor/src/tx_engine.rs` does:

1. Signature verification.
2. Nonce and balance checks.
3. Transfer-style state update.
4. State-diff generation.
5. Lightweight root computation.
6. Execution trace construction with phase timings.

This is not an EVM-equivalent execution engine. It is a controlled synthetic STF for measuring this pipeline.

## Prover Design

The proof integration spans `executor/src/proof.rs` and `risc0_prover/`. The executor writes a converted trace, invokes the RISC0 host, then verifies proof/journal metadata before publishing the enriched batch.

The proof statement is narrow: the guest checks replay of the simplified state diffs from initial root to final root. It does not prove arbitrary smart-contract execution or a production rollup circuit.

## Submitter Design

The benchmark submitter path is mainly in `submitter/src/daemon.rs`. It receives executor-enriched batches, formats proof/data, selects a DA strategy, submits to the bridge, and writes settlement/cost rows.

DA modes:

| Mode | Behavior |
|---|---|
| `calldata` | Posts batch data through calldata-style path. |
| `blob` | Uses blob-like compressed/archive path and records whether real EIP-4844 receipt data exists. |
| `offchain` | Stores local data and submits pointer/commitment; simulated DA. |

Cost metrics must be interpreted with provenance fields: `cost_source`, `blob_cost_source`, `real_eip4844_blob`, and `cost_breakdown_is_estimated`.

## Contracts Design

`contracts/contracts/bridge/ZKRollupBridge.sol` is the local L1 integration target. It supports deposits, withdrawals, configurable DA providers, verifier ids, batch commitment, forced transaction surfaces, and state-root updates.

In local benchmark runs, this validates integration mechanics and local gas receipts, not public Ethereum economics.

## Benchmark Harness Design

`benchmark-suite/scripts/run_experiment.sh` is the main run orchestrator. It:

1. Resolves environment variables.
2. Creates a timestamped metrics directory.
3. Optionally resets local state.
4. Recreates the Docker core stack.
5. Seeds dev sender addresses according to `WORKLOAD_ACCOUNT_COUNT`.
6. Runs warmup and measured workload phases.
7. Polls component metric files.
8. Validates workload/component/L1 status according to mode.
9. Collects diagnostics and generates `analysis_report.md`.

The harness now supports two validation styles:

| Mode | Environment | Intended use |
|---|---|---|
| Fast smoke | `REQUIRE_COMPONENT_METRICS=0`, `REQUIRE_L1_VALIDATION=0` | Quickly verify workload and sequencer behavior. |
| Strict pipeline | `STRICT_PIPELINE_CATCHUP=1`, `REQUIRE_COMPONENT_METRICS=1`, `REQUIRE_L1_VALIDATION=1` | Validate executor/submitter/L1 catch-up; can take much longer with real proof work. |

## Main Design Boundaries

Credible research questions:

- Relative effects of batch size, batch policy, and offered load under local conditions.
- Scheduling-policy effects on wait-time, ordering, and blob-packing diagnostics.
- Prototype pipeline bottlenecks across sequencing, execution, proof, and submission.
- Local comparative cost behavior separated by cost provenance.

Weak claims:

- Mainnet cost prediction.
- EVM-equivalent execution performance.
- Production-grade ZK proof/security.
- Decentralized sequencer fairness.
- Real EIP-4844 blob-market behavior unless receipt-backed blob fields prove it.

