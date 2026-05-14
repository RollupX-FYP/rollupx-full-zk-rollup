# RollupX Benchmark Plan Implementation Guide

This guide explains the benchmark workflow implemented for `rollupx_benchmarking_plan.md`.

It covers:

- how to run the benchmark plan
- which scripts are used
- which stages and experiment groups are implemented
- what metrics are collected
- where raw results, plots, and reports are saved
- how to regenerate analytics from an existing benchmark session

---

## 1. Main Entry Points

The implemented plan uses these scripts:

- `benchmark-suite/scripts/run_plan_benchmark.sh`
- `benchmark-suite/scripts/run_plan_benchmark.py`
- `benchmark-suite/scripts/run_experiment.sh`
- `benchmark-suite/scripts/generate_plan_artifacts.sh`

### Execution flow

```text
run_plan_benchmark.sh
  -> run_plan_benchmark.py
    -> for each selected case:
       env overrides + bash scripts/run_experiment.sh <experiment_id> <repeat>
         -> restart stack / run workload / validate / save per-run metrics
    -> optionally:
       generate_plan_artifacts.sh
         -> aggregate.py
         -> stats.py
         -> plots/*.py
         -> report/generate_md.py
```

---

## 2. How To Run

Run all commands from:

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite
```

### Minimal smoke-style plan run

```bash
bash scripts/run_plan_benchmark.sh --profile smoke --stage minimum --analytics
```

This is the fastest validation run of the implemented plan.

### Recommended short benchmark run

```bash
bash scripts/run_plan_benchmark.sh --profile pilot --stage minimum --analytics
```

This is the best starting point if you want usable plots and summaries without committing to a very long session.

### Run a specific stage

```bash
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage1 --analytics
```

### Run multiple stages

```bash
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage1 --stage stage3 --stage stage4 --analytics
```

### Run the full implemented staged set

```bash
bash scripts/run_plan_benchmark.sh --profile pilot --stage all --analytics
```

### Dry run only

This prints the planned session directory and case manifest without executing Docker or workloads.

```bash
bash scripts/run_plan_benchmark.sh --profile smoke --stage minimum --dry-run
```

### Regenerate plots and reports for an existing session

```bash
bash scripts/generate_plan_artifacts.sh metrics/<session_dir> local
```

Example:

```bash
bash scripts/generate_plan_artifacts.sh metrics/plan_pilot_20260515_120000 local
```

---

## 3. Profiles

The plan runner supports three execution profiles.

### `smoke`

- `RATE_TPS=1`
- `DURATION_S=5`
- `WARMUP_S=0`
- `WORKLOAD_TARGET_TXS=1`
- intended for fast pipeline validation

### `pilot`

- `RATE_TPS=25`
- `DURATION_S=60`
- `WARMUP_S=5`
- `WORKLOAD_TARGET_TXS=0`
- intended for short exploratory benchmarking

### `final`

- `RATE_TPS=50`
- `DURATION_S=600`
- `WARMUP_S=60`
- `WORKLOAD_TARGET_TXS=0`
- intended for long report-quality sessions

---

## 4. Implemented Baseline

The benchmark runner starts from a baseline environment and then applies per-case overrides.

### Baseline environment

- `MAX_BATCH_SIZE=100`
- `MIN_BATCH_SIZE=10`
- `TIMEOUT_MS=2000`
- `BATCH_POLICY=fixed`
- `ADAPTIVE_LOW_LOAD_THRESHOLD=25`
- `ADAPTIVE_MEDIUM_LOAD_THRESHOLD=100`
- `ADAPTIVE_SMALL_BATCH_SIZE=25`
- `ADAPTIVE_MEDIUM_BATCH_SIZE=100`
- `ADAPTIVE_LARGE_BATCH_SIZE=500`
- `BLOB_TARGET_BYTES=120000`
- `BLOB_FILL_TARGET=0.80`
- `POLICY=FCFS`
- `DA_MODE=calldata`
- `PROVER=groth16`
- `PROVER_BACKEND=risc0`
- `REQUIRE_REAL_PROOFS=true`
- `ALLOW_PROOF_FALLBACK=1`
- `ALLOW_UNSIGNED_USER_TXS=0`
- `ETH_PRICE_USD=3000`
- `REGULAR_GAS_PRICE_GWEI=10`
- `BLOB_GAS_PRICE_GWEI=1`
- `TX_MIX=balanced`
- `HARDHAT_MINING_INTERVAL=12000`
- `SEQUENCER_EXECUTOR_PUBLISH_RETRIES=3`
- `SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS=5000`
- `COMM_MODE=grpc`
- `USE_DOCKER_STACK=1`

---

## 5. Implemented Stages

The implemented plan runner currently supports these stage groups.

### `baseline`

- `baseline`

### `stage1` — Fixed batching sweeps

- `bs_025`
- `bs_050`
- `bs_100`
- `bs_200`
- `bs_500`
- `to_0500`
- `to_1000`
- `to_2000`
- `to_5000`

Purpose:

- batch-size trade-offs
- timeout trade-offs
- throughput vs latency
- gas amortization

### `stage2` — Adaptive batching comparison

- `ab_fixed_low`
- `ab_adaptive_low`
- `ab_fixed_high`
- `ab_adaptive_high`

Purpose:

- compare fixed vs adaptive batching
- compare low-load and high-load behavior

### `stage3` — Sequencer policy comparison

- `pol_fcfs`
- `pol_feepriority`
- `pol_blobpacking`

Purpose:

- scheduling policy trade-offs
- fairness vs latency
- blob-aware packing effects

### `stage4` — DA mode comparison

- `da_calldata`
- `da_blob`
- `da_offchain`
- `da_blobpacking`

Purpose:

- compare calldata, blob, and offchain DA
- compare blob mode with and without blob-aware scheduling

### `stage5` — Proof mode comparison

- `proof_real`
- `proof_mock`
- `proof_strict`

Purpose:

- compare real proof enforcement vs fallback/mock-friendly mode
- compare strict proof requirements vs permissive fallback

### `stage6` — L1 timing sensitivity

- `l1_fast`
- `l1_normal`
- `l1_slow`

Purpose:

- compare different `HARDHAT_MINING_INTERVAL` values
- study hard finality sensitivity to L1 timing

### `stage7` — Reliability and publish behavior

- `rel_retry0`
- `rel_retry1`
- `rel_retry3`
- `rel_to1000`
- `rel_to5000`

Purpose:

- compare retry settings
- compare publish timeout settings
- observe reliability/latency trade-offs

### Stage aliases

- `minimum`
  - runs `baseline + stage1 + stage3 + stage4 + stage5`
- `all`
  - runs `baseline + stage1 + stage2 + stage3 + stage4 + stage5 + stage6 + stage7`

---

## 6. What Actually Runs Per Experiment

Each experiment case uses the same core harness:

```bash
bash scripts/run_experiment.sh <experiment_id> <repeat>
```

That script performs:

1. creates a run-specific metrics directory
2. records run metadata
3. writes a sequencer config snapshot
4. restarts the Docker core stack with per-run env vars
5. waits for sequencer readiness
6. runs `workload/poisson_generator.py`
7. waits for sequencer, executor, and submitter metrics to flush
8. validates component metrics and L1 state
9. saves resource metrics
10. generates a per-run `analysis_report.md`

---

## 7. Workload Shape

The workload generator uses:

- `RATE_TPS`
- `DURATION_S`
- `WARMUP_S`
- `WORKLOAD_TARGET_TXS`
- `WORKLOAD_CONCURRENCY`
- `TX_MIX`
- `SEED`

### Current transaction mix implementation

The current benchmark generator uses transaction classes:

- `A` light
- `B` medium
- `C` heavy

Current mix presets:

- `balanced`
- `light`
- `heavy`

The runner currently defaults to:

- `TX_MIX=balanced`

---

## 8. What Is Measured

The implementation collects metrics at several levels.

### Run-level metadata

Saved in:

- `run_metadata.json`

Includes:

- git commit
- start/end timestamp
- machine/runtime info
- config snapshot
- proof/DA/retry/communication settings

### Workload-level metrics

Saved in:

- `workload_<experiment_id>.json`
- `tx_log_<run_id>.csv`
- `run_status.json`

Includes:

- total transactions submitted
- successful and failed transactions
- success rate
- average user action latency
- per-transaction timestamp, type, latency, status, error

### Sequencer metrics

Saved in:

- `sequencer_batch_metrics.jsonl`

Includes:

- batch ID
- tx count per batch
- seal reason
- scheduling policy
- batch policy
- mempool depth
- estimated batch bytes
- blob utilization
- queue wait statistics
- gas limit utilization
- Jain fairness index
- reordering events

### Executor metrics

Saved in:

- `executor_batch_metrics.jsonl`

Includes:

- execution time
- proof generation time
- proof mode
- proof bytes
- journal bytes
- state diff counts and bytes
- touched account counts
- prover wall-clock timing

### Submitter / L1 metrics

Saved in:

- `submitter_metrics.json`

Includes:

- submission status
- DA mode
- tx hash
- L1 gas used
- regular/blob gas usage
- submission latency
- soft commit latency
- hard finality latency
- finality gain
- total cost in wei and USD
- cost per tx
- blob utilization
- cost source and blob cost source

### Resource metrics

Saved in:

- `resource_metrics.json`

Includes:

- peak memory usage summary recorded by the harness

---

## 9. Derived Analytics Generated

The analytics pipeline derives higher-level benchmark outputs from the raw run directories.

### Aggregated CSV outputs

Generated by:

- `data-tools/aggregate.py`
- `data-tools/stats.py`

Outputs:

- `all_results.csv`
- `all_batch_results.csv`
- `stats_summary.csv`
- `sensitivity_matrix.csv`

### Derived metrics available in aggregation

The updated aggregation layer computes or carries forward:

- `tps_offered`
- `tps_accepted`
- `tps_committed`
- `goodput_tps`
- `avg_l2_l1_ms`
- `p50_l2_l1_ms`
- `p95_l2_l1_ms`
- `p99_l2_l1_ms`
- `avg_exec_ms`
- `p50_exec_ms`
- `p95_exec_ms`
- `p99_exec_ms`
- `avg_prove_ms`
- `p50_prove_ms`
- `p95_prove_ms`
- `p99_prove_ms`
- `avg_gas_per_batch`
- `avg_gas_per_tx`
- `avg_total_cost_wei`
- `avg_cost_per_tx_wei`
- `avg_total_cost_usd`
- `avg_cost_per_tx_usd`
- `avg_blob_utilization`
- `avg_soft_commit_ms`
- `avg_hard_finality_ms`
- `avg_finality_gain_ms`
- `avg_comp_ratio`
- `avg_compressed_bytes`
- `avg_calldata_bytes`
- `jains_fairness`
- `starvation_count`
- `p95_latency_typeA_ms`
- `p95_latency_typeB_ms`
- `p95_latency_typeC_ms`
- `failed_batches`
- `total_retries`
- `max_memory_usage_mb`
- `max_memory_usage_gb`

---

## 10. Plots Generated

When analytics are enabled, the artifact script generates plots into the session `figures/` directory.

### Main figures

- `pareto_cost_latency.png`
- `pareto_throughput_latency.png`
- `pareto_prove_gas.png`
- `pareto_da_comparison.png`
- `throughput_by_policy.png`
- `throughput_by_batch_size.png`
- `throughput_by_da_mode.png`
- `throughput_by_rate.png`
- `latency_cdf_all.png`
- `latency_boxplot_batch_size.png`
- `latency_boxplot_timeout.png`
- `latency_boxplot_policy.png`
- `latency_boxplot_da_mode.png`
- `fairness_jains.png`
- `fairness_per_class.png`
- `starvation.png`
- `cost_heatmap_gas_per_tx.png`
- `cost_heatmap_comp_ratio.png`
- `cost_heatmap_latency.png`
- `sensitivity_heatmap.png`
- `sensitivity_*.png`

Actual output depends on which experiment groups were run and which columns are present.

---

## 11. Reports Generated

### Per-run report

Each individual `run_experiment.sh` invocation generates:

- `analysis_report.md`

Location:

```text
metrics/<session>/<experiment_id>/<run_id>/analysis_report.md
```

### Session-level report

When `--analytics` is used, the artifact pipeline generates:

- `thesis_summary.md`

Location:

```text
metrics/<session>/analysis/thesis_summary.md
```

This report summarizes:

- overview
- full result table
- rankings
- comparison vs baseline
- hypotheses section
- threats to validity
- embedded/generated figure references

---

## 12. Where Results Are Saved

Every `run_plan_benchmark.sh` session creates a new session directory:

```text
benchmark-suite/metrics/plan_<profile>_<timestamp>/
```

Example:

```text
benchmark-suite/metrics/plan_pilot_20260515_120000/
```

### Inside a session directory

```text
benchmark-suite/metrics/plan_<profile>_<timestamp>/
├── plan_manifest.csv
├── latest/
├── baseline/
│   └── baseline_r01_<timestamp>/
├── bs_025/
│   └── bs_025_r01_<timestamp>/
├── ...
├── analysis/
│   ├── all_results.csv
│   ├── all_batch_results.csv
│   ├── stats_summary.csv
│   ├── sensitivity_matrix.csv
│   └── thesis_summary.md
└── figures/
    ├── pareto_cost_latency.png
    ├── fairness_jains.png
    ├── throughput_by_policy.png
    └── ...
```

### Per-run directory contents

Each run directory typically contains:

- `run.log`
- `run_metadata.json`
- `run_status.json`
- `workload_<experiment_id>.json`
- `tx_log_<run_id>.csv`
- `sequencer_batch_metrics.jsonl`
- `executor_batch_metrics.jsonl`
- `submitter_metrics.json`
- `resource_metrics.json`
- `l1_deployment.json`
- `l1_state_validation.json`
- `analysis_report.md`
- `diagnostics/`

---

## 13. Manifest File

Each session writes:

- `plan_manifest.csv`

This file records:

- selected profile
- selected stage
- experiment ID
- description
- repeat count
- env overrides used for that case

This is useful for reproducibility and for mapping result directories back to the intended benchmark plan.

---

## 14. Analytics Modes

The plan runner supports:

- `--analytics-mode local`
- `--analytics-mode docker`

### `local`

Uses the local Python environment to run:

- `aggregate.py`
- `stats.py`
- plotting scripts
- `generate_md.py`

Example:

```bash
bash scripts/run_plan_benchmark.sh --profile pilot --stage minimum --analytics --analytics-mode local
```

### `docker`

Uses the existing Docker `report` profile.

Example:

```bash
bash scripts/run_plan_benchmark.sh --profile pilot --stage minimum --analytics --analytics-mode docker
```

---

## 15. Important Notes

### 1. The runner uses env overrides, not `experiments.toml`

This plan runner does not depend on `benchmark-suite/config/experiments.toml`.

It directly sets environment variables for each case and calls:

```bash
bash scripts/run_experiment.sh <experiment_id> <repeat>
```

### 2. Docker stack is used by default

The runner assumes:

- `USE_DOCKER_STACK=1`

So each run recreates the core stack with the selected configuration.

### 3. Proof-related env vars are now propagated

The implementation updates the harness so these settings are passed through:

- `PROVER_BACKEND`
- `REQUIRE_REAL_PROOFS`
- `ALLOW_PROOF_FALLBACK`
- `ALLOW_UNSIGNED_USER_TXS`
- `COMM_MODE`

### 4. `smoke` profile is for correctness, not research-quality data

Because `smoke` uses:

- `WORKLOAD_TARGET_TXS=1`

it is mainly a pipeline validation profile.

Use `pilot` or `final` for real plots and comparisons.

### 5. Existing worktree caveat

This guide describes the implemented benchmark scripts and analytics workflow only.

If your repository already has unrelated local changes, those are outside this benchmark guide.

---

## 16. Recommended Usage

### Quick validation

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite
bash scripts/run_plan_benchmark.sh --profile smoke --stage minimum --analytics
```

### Best short benchmark session

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite
bash scripts/run_plan_benchmark.sh --profile pilot --stage minimum --analytics
```

### Broader staged evaluation

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite
bash scripts/run_plan_benchmark.sh --profile pilot --stage all --analytics
```

### Regenerate only analytics

```bash
cd ~/rollupx-full-zk-rollup/benchmark-suite
bash scripts/generate_plan_artifacts.sh metrics/plan_pilot_20260515_120000 local
```

---

## 17. Summary

The implemented benchmark plan provides:

- a staged benchmark runner aligned to the written plan
- reusable per-run execution via `run_experiment.sh`
- automatic session manifests
- automatic aggregation and statistics
- automatic plot generation
- automatic Markdown summary report generation
- organized session output directories for reproducibility

If you want, the next useful step is to add a second Markdown file with:

- recommended command sets for each FYP chapter/result section
- which figures to use in the final dissertation
- which `stage` combinations best answer each research question
