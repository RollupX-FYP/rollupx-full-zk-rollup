# Benchmark Variables and Experimental Design

This document describes the benchmark matrix encoded in `benchmark-suite/config/experiments.toml` and the execution harness in `benchmark-suite/scripts/`.

## Baseline Configuration

The current baseline is:

| Variable | Baseline |
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
| `repeats` | `5` |
| `seeds` | `[42, 43, 44, 45, 46]` |
| `eth_price_usd` | `2500` |
| `regular_gas_price_gwei` | `2` |
| `blob_gas_price_gwei` | `0.001` |

The baseline is a single-node local experiment. It is intended for comparative benchmarking, not market-representative Ethereum cost measurement.

## Independent Variables

The configured sweeps are one-factor-at-a-time around the baseline.

### Batch Size

| Experiment | Value |
|---|---|
| `exp_001_batch_size_bs010_calldata_fcfs_10tps` | `10` |
| `exp_002_batch_size_bs050_calldata_fcfs_10tps` | `50` |
| `exp_003_batch_size_bs100_calldata_fcfs_10tps` | `100` |
| `exp_004_batch_size_bs500_calldata_fcfs_10tps` | `500` |
| `exp_005_batch_size_bs1000_calldata_fcfs_10tps` | `1000` |

Purpose: evaluate latency/cost/throughput tradeoffs as fixed target batch size changes.

### Scheduling Policy

| Experiment | Value |
|---|---|
| `exp_010_policy_fcfs_bs100_calldata_10tps` | `FCFS` |
| `exp_011_policy_feepriority_bs100_calldata_10tps` | `FeePriority` |
| `exp_012_policy_blobpacking_bs100_calldata_10tps` | `BlobPacking` |

Purpose: compare ordering effects on fee proxy, wait-time distribution, and blob-size proxy behavior. The configured DA mode remains calldata unless overridden, so `BlobPacking` in this sweep is mostly a scheduling heuristic test, not necessarily an actual blob-DA result.

### DA Mode

| Experiment | Value |
|---|---|
| `exp_020_da_calldata_bs100_fcfs_10tps` | `calldata` |
| `exp_021_da_blob_bs100_fcfs_10tps` | `blob` |
| `exp_022_da_offchain_bs100_fcfs_10tps` | `offchain` |

Purpose: compare settlement/DA paths. Interpret blob and offchain rows through submitter provenance fields such as `real_eip4844_blob`, `cost_source`, and `da_mode_is_simulated`.

### Batch Policy

| Experiment | Configuration |
|---|---|
| `exp_030_batch_policy_fixed100_bs100_calldata_fcfs_10tps` | fixed, max `100` |
| `exp_031_batch_policy_fixed500_bs500_calldata_fcfs_10tps` | fixed, max `500` |
| `exp_032_batch_policy_adaptive_bs500_calldata_fcfs_10tps` | adaptive, max `500` |
| `exp_033_batch_policy_adaptive_blob_bs500_blob_blobpacking_10tps` | adaptive, blob DA, blob packing |

Purpose: validate adaptive batching and its combination with blob-aware selection.

Adaptive thresholds:

| Parameter | Baseline |
|---|---|
| `adaptive_low_load_threshold` | `50` |
| `adaptive_medium_load_threshold` | `200` |
| `adaptive_small_batch_size` | `50` |
| `adaptive_medium_batch_size` | `100` |
| `adaptive_large_batch_size` | `500` |
| `blob_target_bytes` | `131072` |
| `blob_fill_target` | `0.90` |

## Workload Variables

The workload generator supports transaction mixes:

| Mix | Type A | Type B | Type C |
|---|---:|---:|---:|
| `balanced` | 70% | 20% | 10% |
| `light` | 95% | 4% | 1% |
| `heavy` | 20% | 30% | 50% |

Transaction classes:

| Type | Intended profile | Gas limit | Gas price | Extra calldata |
|---|---|---:|---:|---:|
| A | light transfer | 21,000 | 1 gwei | 0 bytes |
| B | medium swap-like tx | 65,000 | 2 gwei | 200 bytes |
| C | heavy contract-like call | 200,000 | 3 gwei | 500 bytes |

In the current sequencer/executor path, the transaction object used by `UserTransaction` does not fully consume the synthetic `calldata` field as execution input. The gas limit, gas price, value, and destination still affect batching and fee proxies.

## Dependent Variables

Primary dependent variables:

- accepted TPS,
- sealed batch count,
- actual per-batch `tx_count`,
- wait-time percentiles,
- executor execution time,
- prover wall time,
- proof bytes and journal bytes,
- L1 gas used,
- estimated/measured blob gas,
- total cost and cost per transaction,
- blob utilization,
- finality/latency fields,
- failure/retry/gas-bump incidence.

Secondary diagnostics:

- mempool depth at batch,
- pool growth rate,
- gas limit utilization,
- state diff count/bytes,
- touched-account count,
- proof mode,
- cost provenance,
- Docker/container diagnostics.

## Execution Phases

`run_matrix.sh` defines common phases:

| Phase | Filter | Repeats | Duration | Warmup |
|---|---|---:|---:|---:|
| `smoke` | batch size | 1 | 30s | 5s |
| `feasibility-lite` | batch size | 3 | 90s | 5s |
| `feasibility` | batch size | 5 | 120s | 15s |
| `model-quality` | batch size | 30 | 120s | 15s |

Each run gets a seed from the baseline seed list. If repeats exceed the seed list length, the current matrix logic only uses `seeds[:repeats]`; ensure enough seeds are configured before relying on high-repeat phases.

## Run Isolation

For Docker mode, each run recreates the core stack with the run-specific environment:

- sequencer batch size and timeout,
- batch policy and adaptive thresholds,
- scheduling policy,
- DA mode,
- proof backend,
- gas/ETH price references,
- experiment id/name,
- metrics directory.

The harness calls `docker compose down -v` before `up -d --force-recreate`, then waits for sequencer health. It also resets local runtime state when `CLEAN_STATE_BEFORE_RUN=1`.

## Recommended Analysis Design

Use the benchmark as a comparative factorial study with controlled local conditions:

1. Treat the baseline as the control.
2. Analyze one-factor sweeps separately.
3. Use identical seeds across compared configurations where possible.
4. Analyze actual `tx_count`, not only configured batch size.
5. Separate rows by `cost_source`, `blob_cost_source`, and `real_eip4844_blob`.
6. Exclude or label gas-bumped rows.
7. Report confidence intervals across repeats.
8. Inspect raw JSONL for schema consistency before plotting.

## Threats to Design Quality

The current matrix is useful but incomplete for strong research claims:

- one-factor sweeps miss interactions between rate, mix, batch size, DA mode, and proof mode;
- only one offered rate is configured in the current TOML;
- warmup traffic is not component-tagged;
- local Hardhat timing does not represent public L1 finality;
- local blob mode may be hybrid/estimated rather than real EIP-4844;
- the STF is transfer-centric and does not exercise real contract execution;
- aggregation contains some historical field names that should be reconciled with current emitters.

