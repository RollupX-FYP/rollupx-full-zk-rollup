# Benchmark Variables and Experimental Design

This document describes the current benchmark matrix in `benchmark-suite/config/experiments.toml` and the execution controls in `benchmark-suite/scripts/run_experiment.sh`.

## Baseline

| Variable | Baseline value |
|---|---|
| `id` | `exp_000_baseline_fixed100_calldata_fcfs_10tps` |
| `batch_size` | `100` |
| `timeout_ms` | `30000` |
| `min_batch_size` | `1` |
| `batch_policy` | `fixed` |
| `policy` | `FCFS` |
| `da_mode` | `calldata` |
| `prover` | `groth16` |
| `rate_tps` | `10` |
| `duration_s` | `120` |
| `warmup_s` | `15` |
| `tx_mix` | `balanced` |
| `workload_concurrency` | `1` |
| `workload_account_count` | `1` |
| `repeats` | `5` |
| `seeds` | `[42, 43, 44, 45, 46]` |
| `eth_price_usd` | `2500` |
| `regular_gas_price_gwei` | `2` |
| `blob_gas_price_gwei` | `0.001` |

The baseline is local, single-node, and comparative. It is not a mainnet cost benchmark.

## Independent Variables

### Batch Size Sweep

| Experiment | Value |
|---|---:|
| `exp_001_batch_size_bs010_calldata_fcfs_10tps` | 10 |
| `exp_002_batch_size_bs050_calldata_fcfs_10tps` | 50 |
| `exp_003_batch_size_bs100_calldata_fcfs_10tps` | 100 |
| `exp_004_batch_size_bs500_calldata_fcfs_10tps` | 500 |
| `exp_005_batch_size_bs1000_calldata_fcfs_10tps` | 1000 |

Purpose: measure latency, batch count, gas/cost proxy, and proof/submission behavior as fixed batch target changes.

### Scheduling Policy Sweep

| Experiment | Policy |
|---|---|
| `exp_010_policy_fcfs_bs100_calldata_10tps` | `FCFS` |
| `exp_011_policy_feepriority_bs100_calldata_10tps` | `FeePriority` |
| `exp_012_policy_blobpacking_bs100_calldata_10tps` | `BlobPacking` |

Purpose: compare ordering/composition behavior. `BlobPacking` now uses nonce-safe eligibility and fill-first greedy packing. If `da_mode` remains `calldata`, this tests scheduling and blob-size proxies, not real blob submission economics.

### DA Mode Sweep

| Experiment | DA mode |
|---|---|
| `exp_020_da_calldata_bs100_fcfs_10tps` | `calldata` |
| `exp_021_da_blob_bs100_fcfs_10tps` | `blob` |
| `exp_022_da_offchain_bs100_fcfs_10tps` | `offchain` |

Purpose: compare local DA/submission/cost paths. Blob/offchain results must be separated by provenance fields.

### Batch Policy Sweep

| Experiment | Configuration |
|---|---|
| `exp_030_batch_policy_fixed100_bs100_calldata_fcfs_10tps` | fixed, 100 |
| `exp_031_batch_policy_fixed500_bs500_calldata_fcfs_10tps` | fixed, 500 |
| `exp_032_batch_policy_adaptive_bs500_calldata_fcfs_10tps` | adaptive, max 500 |
| `exp_033_batch_policy_adaptive_blob_bs500_blob_blobpacking_10tps` | adaptive + blob DA + BlobPacking |

Adaptive parameters:

| Parameter | Value |
|---|---:|
| `adaptive_low_load_threshold` | 50 |
| `adaptive_medium_load_threshold` | 200 |
| `adaptive_small_batch_size` | 50 |
| `adaptive_medium_batch_size` | 100 |
| `adaptive_large_batch_size` | 500 |
| `blob_target_bytes` | 131072 |
| `blob_fill_target` | 0.90 |

### Rate Sweep

| Experiment | Offered rate |
|---|---:|
| `exp_040_rate_005tps_bs100_calldata_fcfs_balanced` | 5 TPS |
| `exp_041_rate_025tps_bs100_calldata_fcfs_balanced` | 25 TPS |
| `exp_042_rate_050tps_bs100_calldata_fcfs_balanced` | 50 TPS |

Purpose: identify load-dependent behavior and proof/submission lag.

### Transaction Mix Sweep

| Experiment | Mix |
|---|---|
| `exp_050_mix_light_bs100_calldata_fcfs_10tps` | light |
| `exp_051_mix_balanced_bs100_calldata_fcfs_10tps` | balanced |
| `exp_052_mix_heavy_bs100_calldata_fcfs_10tps` | heavy |

### Workload Sender Sweep

| Experiment | Accounts | HTTP concurrency |
|---|---:|---:|
| `exp_060_workload_accounts1_conc1_bs100_calldata_fcfs_10tps` | 1 | 1 |
| `exp_061_workload_accounts4_conc4_bs100_calldata_fcfs_10tps` | 4 | 4 |
| `exp_062_workload_accounts8_conc8_bs100_calldata_fcfs_10tps` | 8 | 8 |

Purpose: test nonce handling, sender contention, and multi-account validity.

## Workload Model

Transaction classes:

| Type | Intended profile | Gas limit | Gas price | Extra calldata |
|---|---|---:|---:|---:|
| A | light transfer | 21,000 | 1 gwei | 0 bytes |
| B | medium swap-like | 65,000 | 2 gwei | 200 bytes |
| C | heavy contract-like | 200,000 | 3 gwei | 500 bytes |

Mix presets:

| Mix | A | B | C |
|---|---:|---:|---:|
| `balanced` | 70% | 20% | 10% |
| `light` | 95% | 4% | 1% |
| `heavy` | 20% | 30% | 50% |

The synthetic calldata contributes to payload-size estimates and DA stress, but execution remains transfer-centric.

## Dependent Variables

Primary:

- workload success rate and client latency;
- sealed batch count and per-batch `tx_count`;
- wait-time percentiles and fairness proxy;
- blob selected/eligible bytes and low-fill reason;
- executor execution/proof timings;
- proof and journal bytes;
- submitter latency, gas, and cost;
- resource time series.

Secondary:

- mempool depth;
- pool growth rate;
- cache hit/stale-nonce diagnostics;
- ordering efficiency;
- gas bumping;
- cost provenance;
- real/simulated DA provenance.

## Run Modes

The same script supports two important modes:

| Mode | Required environment | Meaning |
|---|---|---|
| Smoke | `REQUIRE_COMPONENT_METRICS=0`, `REQUIRE_L1_VALIDATION=0`, shorter `SUBMITTER_WAIT_MAX` | Validates workload/sequencer quickly; executor/submitter may be missing. |
| Strict | `STRICT_PIPELINE_CATCHUP=1`, `REQUIRE_COMPONENT_METRICS=1`, `REQUIRE_L1_VALIDATION=1` | Validates full pipeline catch-up and L1 submission; can take long with real proofs. |

Use smoke mode for debugging. Use strict mode for research evidence.

## Recommended Experimental Design

For defensible comparisons:

1. Use the same seeds across compared configurations.
2. Report actual emitted batch counts, not only configured batch size.
3. Separate smoke and strict runs.
4. Separate `calldata`, `blob`, and `offchain` results.
5. Separate `measured`, `hybrid`, `estimated`, and simulated cost rows.
6. Use confidence intervals across repeats.
7. Analyze raw JSONL before relying on aggregate CSVs.
8. Include rate and mix sweeps when making general claims.
9. Run BlobPacking with `POLICY=BlobPacking` and verify nonzero blob-selection fields.
10. Use multi-account sweeps for fairness/nonce claims.

## Design Threats

- Local Hardhat settlement is not public Ethereum.
- The STF is simplified and not EVM-equivalent.
- Blob mode may be estimated/hybrid rather than real EIP-4844.
- Warmup traffic can contaminate component metrics unless drained or filtered.
- Strict pipeline runs can take much longer than workload duration because proof/submission lag behind sequencing.
- Fairness and MEV fields are proxies, not full adversarial fairness or MEV measurement.
- Aggregation scripts must be checked against current raw schemas before publication.

