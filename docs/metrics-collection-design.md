# Metrics Collection Design

This document describes how metrics are produced and validated in the current RollupX benchmark implementation.

## Run Directory Contract

Each benchmark run writes to:

```text
metrics/<experiment_id>/<run_id_timestamp>/
```

Expected files:

| File | Producer | Format | Meaning |
|---|---|---|---|
| `run.log` | Harness | text | Full orchestration log. |
| `run_metadata.json` | `collect_env.sh` | JSON | Git/env/machine/config snapshot. |
| `run_status.json` | Workload generator | JSON | Pass/fail based on HTTP transaction submission success. |
| `workload_<experiment_id>.json` | Workload generator | JSON | Offered load, accepted count, latency summary, mix. |
| `tx_log_<run_id>.csv` | Workload generator | CSV | Per-transaction client status/error/latency. |
| `sequencer_batch_metrics.jsonl` | Sequencer | JSONL | One row per sealed batch. |
| `executor_batch_metrics.jsonl` | Executor | JSONL | One row per executed/proved batch. |
| `executor_<experiment_id>.json` | Executor | JSON | Run-level executor summary. |
| `submitter_metrics.json` | Submitter | JSONL | One row per submitted batch despite `.json` suffix. |
| `resource_metrics_timeseries.csv` | Harness sampler | CSV | Time-series process/container resource sampling. |
| `analysis_report.md` | Report script | Markdown | Human-readable run summary. |
| `diagnostics/` | Harness | logs/text | Docker logs, stats, config snapshots. |

## Workload Metrics

`benchmark-suite/workload/poisson_generator.py` emits signed transactions. It supports:

- timed Poisson workload: `--rate`, `--duration`;
- warmup phase: `--warmup`;
- fixed-count burst: `--target_txs`;
- sender concurrency: `--concurrency`;
- multiple dev accounts: `--account_count`;
- deterministic seed streams.

The workload now selects a sender index first and passes that explicit `sender_index` to `TxFactory.make`. This keeps `from`, signature key, sender nonce, and logged sender metadata aligned. That is essential for nonce-valid multi-account experiments.

`tx_log_<run_id>.csv` records:

- `tx_id`,
- `tx_type`,
- `sender_index`,
- `sender_nonce`,
- `from`,
- phase,
- UTC timestamp,
- HTTP latency,
- success/error,
- rejection/error detail.

## Sequencer Metrics

Sequencer rows are appended by `BatchOrchestrator::append_batch_metrics_row` to `sequencer_batch_metrics.jsonl`.

Important field groups:

| Group | Fields |
|---|---|
| Identity/timing | `batch_id`, `experiment_id`, `sealed_at_ms`, `batch_created_time_ms`, `time_since_last_seal_ms` |
| Policy | `batch_policy`, `scheduling_policy`, `scheduler_config`, `seal_reason` |
| Composition | `tx_count`, `forced_tx_count`, `normal_tx_count`, `mempool_depth_at_batch` |
| Capacity proxies | `total_gas_limit`, `gas_limit_max`, `gas_limit_utilization`, `estimated_batch_bytes`, `blob_utilization` |
| Fee proxies | `total_gas_price_wei`, `fee_proxy_wei`, `actual_batch_fee_wei`, `optimal_batch_fee_wei`, `ordering_efficiency` |
| Waiting/fairness | `wait_time_p50_ms`, `wait_time_p95_ms`, `wait_time_p99_ms`, `wait_time_mean_ms`, `jains_fairness_index` |
| Pool/cache | `pool_depth_at_seal`, `pool_depth_after_seal`, `pool_growth_rate_tps`, `cache_hit_rate`, `stale_nonce_rejections`, `cache_age_ms` |
| BlobPacking | `blob_selected_bytes`, `blob_eligible_bytes`, `blob_eligible_tx_count`, `blob_ineligible_nonce_gap_count`, `blob_nonce_chain_truncated_senders`, `blob_low_fill_reason` |

Blob fields are meaningful when `scheduling_policy = "BlobPacking"`. Under other policies they are expected to be zero/null.

`blob_low_fill_reason` can be:

- `nonce_gaps`,
- `insufficient_eligible_bytes`,
- `count_cap`,
- `null`.

## Executor Metrics

Executor rows are appended in `executor/src/service.rs` to `executor_batch_metrics.jsonl`.

Important field groups:

| Group | Fields |
|---|---|
| Identity | `experiment_id`, `batch_id`, `trace_id` |
| Batch size | `tx_count`, `batch_data_bytes` |
| State workload | `state_diff_count`, `state_diff_bytes`, `unique_touched_accounts`, `repeated_touched_accounts` |
| Gas/fee proxies | `total_gas_limit`, `total_gas_price_wei`, `fee_proxy_wei` |
| Execution phases | `signature_verify_ms`, `nonce_balance_check_ms`, `state_transition_ms`, `merkle_update_ms`, `state_diff_computation_ms`, `trace_serialization_ms`, `total_execution_ms` |
| Proof | `prover_metrics`, `total_proof_ms`, `proof_bytes`, `journal_bytes`, `proof_mode`, cycles/segments when present |
| I/O | `trace_write_ms`, `proof_read_ms` |

Executor metrics can lag sequencer metrics substantially when real proof work is enabled. A run can have a passing workload and complete sequencer metrics before executor/submitter have caught up.

## Submitter Metrics

Submitter rows are written by `submitter/src/daemon.rs` to `submitter_metrics.json` as JSONL.

Important field groups:

| Group | Fields |
|---|---|
| Status | `submission_status`, `error`, `batch_id`, `tx_hash`, `da_mode`, `da_mode_is_simulated` |
| Latency | `submission_latency_ms`, `l2_l1_latency_ms`, `soft_commit_ms`, `hard_finality_ms`, `finality_gain_ms` |
| Proof | `prover_rtt_ms`, `proof_generation_ms`, `proof_metadata_hash`, `proof_bytes` |
| Payload/DA | `batch_data_bytes`, `compressed_bytes`, `compression_ratio`, `blob_count`, `blob_utilization` |
| Gas/cost | `l1_gas_used`, `regular_gas_used`, `blob_gas_used`, `total_cost_wei`, `cost_per_tx_wei`, `total_cost_usd`, `cost_per_tx_usd` |
| Provenance | `cost_source`, `blob_cost_source`, `real_eip4844_blob`, `cost_model_version`, `cost_breakdown_is_estimated` |
| Gas bumping | `gas_bumped`, `gas_bump_count`, `original_gas_price_gwei`, `final_gas_price_gwei` |

Cost provenance is mandatory for interpretation:

| Field pattern | Interpretation |
|---|---|
| `cost_source = measured` | receipt-backed regular gas/cost path. |
| `cost_source = hybrid` | measured regular gas plus estimated blob component. |
| `cost_source = estimated` | model-derived cost. |
| `real_eip4844_blob = false` | do not claim observed real blob-market behavior. |

## Harness Synchronization

`run_experiment.sh` polls component metrics using unique `batch_id` counts.

Current validation controls:

| Variable | Default/use |
|---|---|
| `SUBMITTER_WAIT_MAX` | Maximum 3-second polling iterations; defaults longer for groth16/real proof modes. |
| `COMPONENT_STABLE_POLLS` | Number of stable-size polls before the wait loop can exit. |
| `STRICT_PIPELINE_CATCHUP` | If true, require executor batch count >= sequencer and submitter >= executor before considering metrics caught up. |
| `REQUIRE_COMPONENT_METRICS` | If true, missing sequencer/executor/submitter metrics fail the run. |
| `REQUIRE_L1_VALIDATION` | If true, validate submitter/L1 progress. |

Fast smoke runs may intentionally skip executor/submitter/L1 validation. Such runs are useful for checking workload and sequencer behavior, but they are not full end-to-end proof/settlement evidence.

## Aggregation

`data-tools/aggregate.py` scans run directories and writes run-level and batch-level CSVs. Raw JSONL should be treated as the source of truth, because aggregate code can lag behind emitter schema changes.

Before publication, verify these mappings:

- executor emits `total_execution_ms` and `total_proof_ms`;
- sequencer emits `wait_time_*` rather than old wait field names;
- sequencer does not emit executor/submitter payload-size fields;
- submitter `submitter_metrics.json` is JSONL, not a single JSON document.

## Metric Validity Checklist

Before analyzing results:

- Confirm `run_status.json` has `"status": "pass"`.
- Confirm `run_metadata.json` has the intended git commit and environment.
- Confirm `scheduling_policy` and `da_mode` match the research question.
- For full pipeline claims, require nonzero sequencer, executor, and submitter batch ids.
- For strict end-to-end claims, use `STRICT_PIPELINE_CATCHUP=1`, `REQUIRE_COMPONENT_METRICS=1`, and `REQUIRE_L1_VALIDATION=1`.
- Separate blob rows by `real_eip4844_blob`, `cost_source`, and `blob_cost_source`.
- Treat smoke runs with missing executor/submitter metrics as partial validation only.
- Archive raw JSONL/CSV, diagnostics, and exact command/environment with every figure.

