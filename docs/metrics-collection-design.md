# Metrics Collection Design

This document explains how metrics are produced, synchronized, and aggregated in the current RollupX benchmark implementation.

## Metrics Contract

The benchmark harness sets `METRICS_ROOT` for each run. Components append files inside that directory:

| File | Producer | Format | Meaning |
|---|---|---|---|
| `workload_<experiment_id>.json` | Workload generator | JSON | Run-level offered load and accepted transaction counts. |
| `tx_log_<run_id>.csv` | Workload generator | CSV | Per-submission client-side status and HTTP latency. |
| `run_status.json` | Workload generator | JSON | Pass/fail based on transaction submission success. |
| `run_metadata.json` | `collect_env.sh` | JSON | Git commit, machine, runtime, config snapshot, validity envelope. |
| `sequencer_batch_metrics.jsonl` | Sequencer | JSONL | One row per sealed batch. |
| `executor_batch_metrics.jsonl` | Executor | JSONL | One row per executed/proved batch. |
| `executor_<experiment_id>.json` | Executor | JSON | Run-level executor summary. |
| `submitter_metrics.json` | Submitter | JSONL despite `.json` name | One row per submitted/simulated batch. |
| `resource_metrics.json` | Harness | JSON | Docker memory snapshot/proxy. |

## Workload Metrics

The workload generator is `benchmark-suite/workload/poisson_generator.py`.

It emits synthetic transactions using seeded random streams:

- inter-arrival RNG,
- transaction-type RNG,
- transaction-value RNG.

Modes:

- timed Poisson phase: `--rate`, `--duration`,
- warmup phase: `--warmup`,
- fixed-count burst: `--target_txs > 0`,
- optional concurrent HTTP senders: `--concurrency`.

The JSON summary records:

- configured rate,
- duration,
- total transactions sent during measured phase,
- successful/failed submissions,
- client-side average action latency,
- tx type counts,
- seed and tx mix.

The CSV log records:

- `tx_id`,
- `tx_type`,
- UTC timestamp,
- HTTP submission latency,
- success/error status,
- error string.

Important limitation: warmup transactions are not recorded by the workload JSON, but they are still sent into the live system. Unless the system drains fully before measurement or the components tag warmup traffic, component-level metrics may include warmup-originated batches.

## Sequencer Batch Metrics

Sequencer metrics are emitted in `sequencer/src/batch/orchestrator.rs` by appending `SequencerBatchMetricsRow` to `sequencer_batch_metrics.jsonl`.

Major field groups:

| Group | Example fields |
|---|---|
| Identity/timing | `batch_id`, `experiment_id`, `sealed_at_ms`, `batch_created_time_ms`, `time_since_last_seal_ms` |
| Policy | `batch_policy`, `scheduling_policy`, `scheduler_config`, `seal_reason` |
| Batch composition | `tx_count`, `forced_tx_count`, `normal_tx_count`, `mempool_depth_at_batch` |
| Resource proxies | `total_gas_limit`, `gas_limit_utilization`, `estimated_batch_bytes`, `blob_utilization` |
| Fee proxies | `total_gas_price_wei`, `fee_proxy_wei` |
| Wait-time/fairness | `wait_time_p50_ms`, `wait_time_p95_ms`, `wait_time_p99_ms`, `wait_time_mean_ms`, `jains_fairness_index` |
| Ordering diagnostics | `actual_batch_fee_wei`, `optimal_batch_fee_wei`, `ordering_efficiency`, `reordering_events`, `max_reorder_distance` |
| Pool/cache diagnostics | `pool_depth_at_seal`, `pool_depth_after_seal`, `pool_growth_rate_tps`, `cache_hit_rate`, `cache_age_ms` |

Interpretation notes:

- `blob_utilization` is based on estimated serialized bytes divided by configured target bytes.
- `fee_proxy_wei` is a gas-price times gas-limit proxy, not actual paid L1 fee.
- `ordering_efficiency` currently compares sums of included gas prices, so it can be weak as a measure of MEV/revenue optimality when the selected set is unchanged.
- Percentiles are computed per batch from in-batch wait times, not across the full experiment.

## Executor Metrics

Executor batch metrics are emitted in `executor/src/service.rs` to `executor_batch_metrics.jsonl`.

Major fields:

| Group | Example fields |
|---|---|
| Identity | `experiment_id`, `batch_id`, `trace_id` |
| Batch size | `tx_count`, `batch_data_bytes` |
| State/proof workload | `state_diff_count`, `state_diff_bytes`, `unique_touched_accounts`, `repeated_touched_accounts` |
| Gas/fee proxies | `total_gas_limit`, `total_gas_price_wei`, `fee_proxy_wei` |
| Execution phases | `signature_verify_ms`, `nonce_balance_check_ms`, `state_transition_ms`, `merkle_update_ms`, `state_diff_computation_ms`, `trace_serialization_ms`, `total_execution_ms` |
| Prover metadata | `witness_generation_ms`, `zkvm_execution_ms`, `proof_compression_ms`, `total_prover_wall_ms`, `total_cycles`, `total_segments`, `proof_mode` |
| I/O | `trace_write_ms`, `proof_read_ms`, `proof_bytes`, `journal_bytes` |

The executor also writes `executor_<experiment_id>.json`, but aggregation code should be checked before relying on older fields; some historical field names no longer match the current emitted schema.

## Submitter Metrics

Submitter metrics are emitted in `submitter/src/daemon.rs` by appending `SubmitterMetrics` rows to `submitter_metrics.json`.

Major field groups:

| Group | Example fields |
|---|---|
| Status | `submission_status`, `error`, `batch_id`, `tx_hash`, `da_mode`, `da_mode_is_simulated` |
| Latency | `submission_latency_ms`, `l2_l1_latency_ms`, `soft_commit_ms`, `hard_finality_ms`, `finality_gain_ms` |
| Proof | `prover_rtt_ms`, `proof_generation_ms`, `proof_metadata_hash`, `proof_bytes` |
| Payload/DA | `batch_data_bytes`, `compressed_bytes`, `compression_ratio`, `blob_count`, `blob_utilization` |
| Gas and cost | `l1_gas_used`, `regular_gas_used`, `blob_gas_used`, `blob_base_fee_wei`, `total_cost_wei`, `cost_per_tx_wei`, `total_cost_usd`, `cost_per_tx_usd` |
| Cost provenance | `cost_source`, `blob_cost_source`, `real_eip4844_blob`, `cost_model_version`, `cost_breakdown_is_estimated` |
| Gas bumping | `gas_bumped`, `gas_bump_count`, `original_gas_price_gwei`, `final_gas_price_gwei` |

Cost model provenance is critical:

- `measured`: receipt-level gas/fee data is available.
- `hybrid`: measured regular gas plus estimated blob gas.
- `estimated`: model-derived values without full receipt backing.
- `real_eip4844_blob = false`: do not treat blob results as observed EIP-4844 market outcomes.

## Harness Synchronization

`benchmark-suite/scripts/run_experiment.sh` coordinates each run:

1. Create/clean `METRICS_ROOT`.
2. Reset local state if configured.
3. Write metadata.
4. Recreate Docker core stack with experiment env.
5. Run workload generator.
6. Poll component metric files.
7. Require sequencer, executor, and submitter rows.
8. Require row parity: executor rows >= sequencer rows, submitter rows >= executor rows.
9. Validate workload status.
10. Validate L1/submission state.
11. Write diagnostics and resource metrics.

This is a useful guard against early termination, but it is not a substitute for experimental isolation. Row parity only shows that components processed at least as many unique batch ids as upstream, not that those rows correspond exactly to measured-phase transactions.

## Aggregation

`data-tools/aggregate.py` produces:

- run-level `all_results.csv`,
- batch-level `all_batch_results.csv`.

It joins rows primarily by `batch_id` and run directory. For valid analysis, inspect the emitted schemas before trusting all derived columns. Known examples to verify:

- executor emits `total_execution_ms` and `total_proof_ms`, while aggregation contains historical `execution_time_ms` and `proof_time_ms` batch fields;
- sequencer emits wait-time fields named `wait_time_*`, while aggregation references `oldest_tx_wait_ms`;
- sequencer rows do not include `batch_data_bytes`; executor/submitter do.

Recommended practice: treat raw JSONL as the source of truth, and regenerate/patch aggregate fields before producing research figures.

## Metric Validity Checklist

Before analyzing a run, confirm:

- `run_status.json` has `"status": "pass"`.
- `run_metadata.json` includes the expected config snapshot and git commit.
- All three component metrics exist and have nonzero unique batch ids.
- Submitter rows have `submission_status = "submitted"` or an explicitly simulated status that you intend to include.
- Blob rows are separated by `real_eip4844_blob`, `cost_source`, and `blob_cost_source`.
- Gas-bumped rows are filtered or analyzed separately.
- Warmup rows are absent or explicitly accounted for.
- Aggregation columns match current raw schemas.

