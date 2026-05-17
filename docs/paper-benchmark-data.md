# Paper Benchmark Data Pack

This file collects the information needed to write a defensible paper from the RollupX benchmark. It is scoped to the current implementation: a local, single-node prototype with synthetic workloads, a transfer-centric executor, a RISC0 state-diff proof path, and local L1 settlement.

## Working Title

Controlled Benchmarking of Batching, Scheduling, and Data Availability Tradeoffs in a Local ZK-Rollup Prototype

## Core Claim

The benchmark supports comparative analysis within this implementation under local conditions. It can compare how batch size, batch policy, scheduling policy, offered load, and data availability mode affect local latency, batching behavior, component bottlenecks, and cost metrics when measured/proxy/estimated provenance is reported.

It should not be used to claim production rollup throughput, Ethereum mainnet economics, decentralized sequencer fairness, EVM-equivalent execution, or production-grade ZK proof security.

## Research Questions

| ID | Question | Supported? | Notes |
|---|---|---:|---|
| RQ1 | How do batch size, batch policy, and offered load affect local latency and pipeline behavior? | Yes | Strong local comparative claim. |
| RQ2 | How do scheduling policies affect wait time, ordering proxies, and BlobPacking diagnostics? | Yes, with limits | Wait time and blob diagnostics are meaningful; ordering/MEV metrics are proxies. |
| RQ3 | Where do bottlenecks appear across sequencing, execution, proof, and submission? | Yes | Requires strict mode and nonzero component rows. |
| RQ4 | How do local cost metrics compare across DA modes when separated by provenance? | Yes, local only | Must split by `cost_source`, `blob_cost_source`, and `real_eip4844_blob`. |

## Valid Variables

| Variable | Values | Use In Main Claims? | Reason |
|---|---|---:|---|
| `batch_size` | `10`, `50`, `100`, `500`, `1000` | Yes | Directly controls fixed batch trigger/selection. |
| `batch_policy` | `fixed`, `adaptive` | Yes | Implemented in `BatchTrigger::target_batch_size_for_depth`. |
| `policy` | `FCFS`, `FeePriority`, `BlobPacking` | Yes, with limits | FCFS/FeePriority are direct ordering policies; BlobPacking uses estimated size diagnostics. |
| `rate_tps` | `5`, `10`, `25`, `50` | Yes | Poisson generator changes offered load. |
| `da_mode` | `calldata`, `blob`, `offchain` | Yes, local only | Valid for local DA/cost behavior with provenance. |
| `repeats` / `seed` | `42` to `46` | Yes | Needed for repeated seeded comparisons. |
| `duration_s` | recommended `90` or `120` | Yes | Longer runs expose lag and tail behavior. |
| `warmup_s` | recommended `10` or `15` | Yes | Reduces startup effects, though warmup leakage must still be considered. |

## Weak Variables Or Claims

| Item | Status | Why |
|---|---|---|
| `tx_mix` as calldata-heavy workload | Exclude from primary claims | Generator creates `calldata`, but sequencer `UserTransaction` drops unknown fields. |
| `tx_mix` as execution complexity | Exclude from primary claims | Executor is transfer-centric; A/B/C do not execute different contract logic. |
| MEV resistance | Do not claim | `ordering_efficiency` is a fee proxy; `reordering_events` is currently hardcoded to `0`. |
| Real EIP-4844 economics | Claim only if receipt-backed | Requires `real_eip4844_blob = true` and measured blob gas fields. |
| Production throughput | Do not claim | Single-node local VM and prototype stack. |
| EVM-equivalent execution | Do not claim | Executor is not a full EVM. |

## Primary Metrics

| Component | File | Metrics To Use |
|---|---|---|
| Workload generator | `workload_<experiment_id>.json`, `tx_log_<run_id>.csv` | offered rate, success count, client latency, seed, sender counts |
| Sequencer | `sequencer_batch_metrics.jsonl` | `tx_count`, `seal_reason`, `wait_time_*`, `jains_fairness_index`, `pool_depth_*`, `gas_limit_utilization`, `blob_*` |
| Executor | `executor_batch_metrics.jsonl`, `executor_<experiment_id>.json` | `total_execution_ms`, phase timings, `total_proof_ms`, `proof_bytes`, `journal_bytes`, state diff counts |
| Submitter | `submitter_metrics.json` | submission latency, DA mode, gas/cost fields, cost provenance fields |
| Harness | `run_metadata.json`, `run_status.json`, diagnostics | git commit, machine config, run status, strict validation context |

## Real, Proxy, And Estimated Metrics

| Class | Examples | How To Report |
|---|---|---|
| Real/local observed | batch counts, wait times, component timings, proof bytes, local receipt gas when present | Can be used directly as local measurements. |
| Proxy | `fee_proxy_wei`, `ordering_efficiency`, `jains_fairness_index`, estimated blob byte diagnostics | Label as proxy metrics. |
| Estimated/hybrid | `estimated_batch_bytes`, `estimated_da_bytes_pre_enrichment`, `estimated_blob_gas_used`, `total_cost_usd` | Report with model/provenance fields. |
| Provenance fields | `cost_source`, `blob_cost_source`, `real_eip4844_blob`, `cost_breakdown_is_estimated` | Always group or filter cost results by these fields. |

## Recommended Benchmark Design

Use a staged design. This reduces time while preserving scientific value.

### Stage 1: Screening

Purpose: broad signal and sanity checking. Do not use as final evidence.

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite

export METRICS_ROOT="$HOME/rollupx-results/stage1_screening_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$METRICS_ROOT"

export STRICT_PIPELINE_CATCHUP=1
export REQUIRE_COMPONENT_METRICS=1
export REQUIRE_L1_VALIDATION=1
export USE_DOCKER_STACK=1
export DOCKER_UP_BUILD=1

bash scripts/run_matrix.sh \
  --filter all \
  --repeats 1 \
  --duration 45 \
  --warmup 5
```

Estimated time on the target 16-vCPU / 122 GiB RAM VM:

| Case | Time |
|---|---:|
| Best case | 3-5 hours |
| Realistic | 6-12 hours |
| Slow strict | 12-24 hours |

### Stage 2: Core Variables

Purpose: repeated measurements for variables worth claiming.

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite

export METRICS_ROOT="$HOME/rollupx-results/stage2_core_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$METRICS_ROOT"

export STRICT_PIPELINE_CATCHUP=1
export REQUIRE_COMPONENT_METRICS=1
export REQUIRE_L1_VALIDATION=1
export USE_DOCKER_STACK=1
export DOCKER_UP_BUILD=0

for factor in batch_size policy rate da_mode batch_policy; do
  bash scripts/run_matrix.sh \
    --filter "$factor" \
    --repeats 3 \
    --duration 90 \
    --warmup 10
done
```

Estimated time:

| Case | Time |
|---|---:|
| Best case | 10-18 hours |
| Realistic | 1-2.5 days |
| Slow strict | 3-5 days |

### Stage 3: Confirmation

Purpose: stronger evidence for final contrasts only.

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite

export METRICS_ROOT="$HOME/rollupx-results/stage3_confirmation_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$METRICS_ROOT"

export STRICT_PIPELINE_CATCHUP=1
export REQUIRE_COMPONENT_METRICS=1
export REQUIRE_L1_VALIDATION=1
export USE_DOCKER_STACK=1
export DOCKER_UP_BUILD=0

CONFIGS=(
  exp_000_baseline_fixed100_calldata_fcfs_10tps
  exp_001_batch_size_bs010_calldata_fcfs_10tps
  exp_005_batch_size_bs1000_calldata_fcfs_10tps
  exp_011_policy_feepriority_bs100_calldata_10tps
  exp_012_policy_blobpacking_bs100_calldata_10tps
  exp_021_da_blob_bs100_fcfs_10tps
  exp_032_batch_policy_adaptive_bs500_calldata_fcfs_10tps
  exp_040_rate_005tps_bs100_calldata_fcfs_balanced
  exp_042_rate_050tps_bs100_calldata_fcfs_balanced
)

for id in "${CONFIGS[@]}"; do
  bash scripts/run_matrix.sh \
    --only "$id" \
    --repeats 5 \
    --duration 120 \
    --warmup 15
done
```

Estimated time:

| Case | Time |
|---|---:|
| Best case | 8-16 hours |
| Realistic | 1-2 days |
| Slow strict | 2-4 days |

## Output Collection

Aggregate each stage after it finishes:

```bash
cd ~/rollupx-full-zk-rollup

python3 data-tools/aggregate.py \
  --metrics_root "$METRICS_ROOT" \
  --output "$METRICS_ROOT/all_results.csv" \
  --include_failed
```

Check that component metrics exist:

```bash
find "$METRICS_ROOT" -name run_status.json -print -exec cat {} \;
find "$METRICS_ROOT" -name sequencer_batch_metrics.jsonl -exec wc -l {} \;
find "$METRICS_ROOT" -name executor_batch_metrics.jsonl -exec wc -l {} \;
find "$METRICS_ROOT" -name submitter_metrics.json -exec wc -l {} \;
```

Find generated reports:

```bash
find "$METRICS_ROOT" -name analysis_report.md | sort
```

## Required Reporting Fields

Every figure or table should include or be traceable to:

| Field | Source |
|---|---|
| experiment id | `run_metadata.json`, metrics rows |
| run id | `run_metadata.json`, workload output |
| git commit | `run_metadata.json` |
| hardware | `run_metadata.json` or machine capture |
| `batch_size` | `run_metadata.json` |
| `batch_policy` | `run_metadata.json`, sequencer rows |
| `policy` | `run_metadata.json`, sequencer rows |
| `rate_tps` | `run_metadata.json`, workload output |
| `duration_s`, `warmup_s` | `run_metadata.json`, workload output |
| `da_mode` | `run_metadata.json`, submitter rows |
| `cost_source`, `blob_cost_source` | submitter rows |
| `real_eip4844_blob` | submitter rows |
| strict/smoke mode | environment variables and run logs |

## Paper Structure

1. Introduction: motivate component-level benchmarking instead of one throughput number.
2. Research Questions: use RQ1-RQ4 above.
3. System Design: describe workload, sequencer, executor, prover, submitter, local bridge.
4. Methodology: explain staged benchmark design and controlled variables.
5. Metrics and Provenance: define real, proxy, hybrid, and estimated metrics.
6. Results: organize by RQ.
7. Validity Threats: local Hardhat, simplified STF, narrow proof statement, tx mix limitation, cost provenance.
8. Conclusion: summarize local comparative findings without production claims.

## Recommended Wording

Use this for scope:

> These results measure a local, single-node RollupX prototype using synthetic seeded workloads, a transfer-centric state transition function, a RISC0 state-diff proof path, and local Hardhat settlement. Results are suitable for comparative analysis within this implementation when run mode, metric provenance, and raw schemas are reported.

Use this for workload:

> Primary experiments use a fixed balanced synthetic workload. Transaction-mix sensitivity is excluded from primary claims because the current transaction classes mainly affect gas-limit and gas-price fields, not full calldata or execution semantics.

Use this for costs:

> Cost results are grouped by provenance. Receipt-backed regular gas is reported separately from hybrid or estimated blob-cost rows, and rows with `real_eip4844_blob = false` are not interpreted as real blob-market measurements.

Use this for scheduling:

> Scheduling-policy results are interpreted as local wait-time, ordering-proxy, and blob-packing diagnostics. They do not establish decentralized fairness or MEV resistance.

## Results Tables To Create

| Table/Figure | Rows/Grouping | Metrics |
|---|---|---|
| Batch size sweep | `batch_size` | p50/p95/p99 wait, batch count, proof time, submitter latency |
| Scheduling sweep | `policy` | wait times, `ordering_efficiency`, blob diagnostics |
| Offered load sweep | `rate_tps` | success rate, wait times, pool depth, executor/submitter lag |
| Batch policy sweep | `batch_policy` | batch count, seal reason, wait times, proof/submission pressure |
| DA/cost sweep | `da_mode`, `cost_source`, `blob_cost_source` | gas, total cost, cost per tx, simulated/measured flags |
| Pipeline bottleneck table | component | sequencer rows, executor timings, proof timings, submitter latency |

## Minimum Validity Checklist

Before using any run in the paper:

1. `run_status.json` has `"status": "pass"`.
2. Sequencer, executor, and submitter metric files have nonzero rows for end-to-end claims.
3. Strict mode was enabled for final claims.
4. `run_metadata.json` records the expected configuration.
5. Cost results are separated by provenance.
6. Blob results report whether they are estimated or receipt-backed.
7. Raw JSONL fields are checked before relying on aggregate CSVs.
8. Figures do not mix smoke and strict runs.
9. `tx_mix` is not used for primary calldata/execution-complexity claims.
10. The paper states the local/prototype scope clearly.

