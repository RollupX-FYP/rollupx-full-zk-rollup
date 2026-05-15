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

### Run the full written-plan staged set

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

## Full Plan Stage-By-Stage Commands

Use these commands when you want to cover the full benchmark plan one stage at a time and keep each stage in a named output directory.

### Quick validation first

```bash
cd benchmark-suite
bash scripts/run_plan_benchmark.sh --profile smoke --stage stage0 --analytics --session-name stage0_validation
```

### Pilot run for checking plots and report wiring

```bash
cd benchmark-suite
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage0 --analytics --session-name pilot_stage0_validation
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage1 --analytics --session-name pilot_stage1_fixed_batching
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage2 --analytics --session-name pilot_stage2_adaptive_batching
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage3 --analytics --session-name pilot_stage3_policy
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage4 --analytics --session-name pilot_stage4_da
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage5 --analytics --session-name pilot_stage5_proofs
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage6 --analytics --session-name pilot_stage6_l1
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage7 --analytics --session-name pilot_stage7_reliability
bash scripts/run_plan_benchmark.sh --profile pilot --stage stage8 --analytics --session-name pilot_stage8_final_comparison
```

### Final report-quality run

Use `--repeats 3` or higher for report-quality comparisons. `--profile final` uses longer runs, so this can take a long time.

```bash
cd benchmark-suite
bash scripts/run_plan_benchmark.sh --profile final --stage stage0 --repeats 3 --analytics --session-name final_stage0_validation
bash scripts/run_plan_benchmark.sh --profile final --stage stage1 --repeats 3 --analytics --session-name final_stage1_fixed_batching
bash scripts/run_plan_benchmark.sh --profile final --stage stage2 --repeats 3 --analytics --session-name final_stage2_adaptive_batching
bash scripts/run_plan_benchmark.sh --profile final --stage stage3 --repeats 3 --analytics --session-name final_stage3_policy
bash scripts/run_plan_benchmark.sh --profile final --stage stage4 --repeats 3 --analytics --session-name final_stage4_da
bash scripts/run_plan_benchmark.sh --profile final --stage stage5 --repeats 3 --analytics --session-name final_stage5_proofs
bash scripts/run_plan_benchmark.sh --profile final --stage stage6 --repeats 3 --analytics --session-name final_stage6_l1
bash scripts/run_plan_benchmark.sh --profile final --stage stage7 --repeats 3 --analytics --session-name final_stage7_reliability
bash scripts/run_plan_benchmark.sh --profile final --stage stage8 --repeats 3 --analytics --session-name final_stage8_final_comparison
```

### One-command full plan

This runs `stage0`, `baseline`, and `stage1` through `stage8` in one session.

```bash
cd benchmark-suite
bash scripts/run_plan_benchmark.sh --profile final --stage all --repeats 3 --analytics --session-name final_full_plan
```

### Where outputs are saved

Each command above writes a session under:

```text
benchmark-suite/metrics/<session-name>/
```

For example:

```text
benchmark-suite/metrics/final_stage4_da/
```

Inside each session:

- `plan_manifest.csv` records every case, stage, repeat count, and env override.
- `<experiment_id>/<run_id>/` contains raw per-run JSON, CSV, logs, diagnostics, and `analysis_report.md`.
- `analysis/all_results.csv` is the main run-level aggregate.
- `analysis/all_batch_results.csv` is the batch-level aggregate used for batch/proof/DA feasibility plots.
- `analysis/stats_summary.csv` contains grouped summary statistics.
- `analysis/sensitivity_matrix.csv` contains percentage deltas versus baseline.
- `analysis/thesis_summary.md` is the session-level generated Markdown report.
- `figures/` contains generated `.png` plots.
- `latest/` is the shared metrics handoff directory used during the most recent run.

### Regenerate graphs and reports

If raw runs already exist and you only need to rebuild analytics:

```bash
cd benchmark-suite
bash scripts/generate_plan_artifacts.sh metrics/<session-name> local
```

Use Docker-based analytics instead of local Python with:

```bash
cd benchmark-suite
bash scripts/generate_plan_artifacts.sh metrics/<session-name> docker
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
- `WORKLOAD_BURST_ENABLED=0`
- `WORKLOAD_BURST_RATE_TPS=0`
- `WORKLOAD_BURST_PERIOD_S=30`
- `WORKLOAD_BURST_DUTY_CYCLE=0.25`
- `TX_MIX=balanced`
- `HARDHAT_MINING_INTERVAL=12000`
- `SEQUENCER_EXECUTOR_PUBLISH_RETRIES=3`
- `SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS=5000`
- `COMM_MODE=grpc`
- `USE_DOCKER_STACK=1`

---

## 5. Implemented Stages

The plan runner supports the written plan stages `stage0` through `stage8`, plus the reusable `baseline`.

### `stage0` — Instrumentation validation

- `s0_validation`

Purpose:

- verify that the harness records timestamps, per-run status, per-batch metrics, proof metrics, DA/L1 metrics, and cost fields
- run a 5 TPS transfer-only validation workload with proof fallback allowed

### `baseline`

- `baseline`

Purpose:

- provide the reference fixed-batch, FCFS, calldata configuration used by sensitivity and comparison reports

### `stage1` — Fixed batching sweeps

- Batch-size cases: `s1_bs_0025`, `s1_bs_0050`, `s1_bs_0100`, `s1_bs_0200`, `s1_bs_0500`, `s1_bs_1000`
- Timeout cases: `s1_to_00500`, `s1_to_01000`, `s1_to_02000`, `s1_to_05000`, `s1_to_10000`
- Workload cases: `s1_wl_normal`, `s1_wl_transfer`, `s1_wl_heavy`

Purpose:

- batch-size trade-offs
- timeout trade-offs
- workload sensitivity
- throughput, latency, cost, and proof-time behavior

### `stage2` — Adaptive batching comparison

- Load comparison cases: `s2_fixed_low`, `s2_adaptive_low`, `s2_fixed_medium`, `s2_adaptive_medium`, `s2_fixed_high`, `s2_adaptive_high`, `s2_fixed_burst`, `s2_adaptive_burst`
- Threshold cases: `s2_adapt_l10_m50`, `s2_adapt_l25_m100`, `s2_adapt_l50_m150`

Purpose:

- compare fixed vs adaptive batching
- compare low, medium, high, and burst traffic
- test adaptive threshold and adaptive batch-size choices

### `stage3` — Sequencer policy comparison

- Policy cases: `s3_pol_fcfs`, `s3_pol_feepriority`, `s3_pol_timeboost`, `s3_pol_fairbft`, `s3_pol_blobpacking`
- Burst cases: `s3_burst_feepriority`, `s3_burst_timeboost`, `s3_burst_fairbft`

Purpose:

- scheduling policy trade-offs
- fairness vs latency
- starvation and reordering behavior
- blob-aware packing effects

### `stage4` — DA mode and blob packing

- DA mode cases: `s4_da_calldata`, `s4_da_blob`, `s4_da_offchain`, `s4_da_blobpacking`
- Blob target cases: `s4_blob_target_32768`, `s4_blob_target_65536`, `s4_blob_target_98304`, `s4_blob_target_120000`
- Blob fill cases: `s4_blob_fill_050`, `s4_blob_fill_070`, `s4_blob_fill_080`, `s4_blob_fill_090`, `s4_blob_fill_095`

Purpose:

- compare calldata, blob, and offchain DA
- measure blob target and fill-target sensitivity
- compare blob mode with and without blob-aware scheduling

### `stage5` — Prover backend and real proof behavior

- Real-proof batch-size cases: `s5_real_bs_0050`, `s5_real_bs_0100`, `s5_real_bs_0200`, `s5_real_bs_0500`
- Proof-mode cases: `s5_proof_mock`, `s5_proof_real`, `s5_proof_strict`, `s5_heavy_real`

Purpose:

- compare mock/fallback-friendly mode against real proof mode
- compare strict real-proof requirements against permissive fallback
- measure proof-time, memory, and finality impact across batch sizes and heavy-state workload

### `stage6` — Gas limit and L1 submission sensitivity

- Mining interval cases: `s6_l1_interval_1000`, `s6_l1_interval_3000`, `s6_l1_interval_12000`, `s6_l1_interval_30000`
- Gas price cases: `s6_gas_regular_5_blob_01`, `s6_gas_regular_10_blob_1`, `s6_gas_regular_30_blob_5`, `s6_gas_regular_100_blob_20`

Purpose:

- compare hard finality under different L1 mining intervals
- compare calldata/blob cost behavior under different gas price assumptions
- separate soft confirmation from hard finality behavior

### `stage7` — Reliability and publish behavior

- Retry cases: `s7_retry_0`, `s7_retry_1`, `s7_retry_3`, `s7_retry_5`
- Timeout cases: `s7_timeout_1000`, `s7_timeout_3000`, `s7_timeout_5000`, `s7_timeout_10000`
- Communication mode cases: `s7_comm_grpc`, `s7_comm_file`

Purpose:

- compare retry settings
- compare publish timeout settings
- compare available communication modes
- observe reliability/latency behavior under burst load

### `stage8` — Final best configuration comparison

- Final configurations: `baseline`, `best_fixed`, `best_adaptive`, `best_fairness`, `best_cost`, `best_realproof`
- Final workloads: `normal`, `burst`, `heavy`, `da_heavy`
- Case IDs use the form `s8_<configuration>_<workload>`

Purpose:

- compare the baseline against representative best configurations
- produce the final configuration recommendation matrix
- evaluate every final configuration across normal, burst, heavy-state, and DA-heavy workloads

Note: `stage8` uses representative best configurations based on the plan defaults and recommended values. If earlier stages identify better values, update the `stage8` overrides in `benchmark-suite/scripts/run_plan_benchmark.py` before the final dissertation run.

### Stage aliases

- `minimum`: runs `stage0 + baseline + stage1 + stage3 + stage4 + stage5`
- `all`: runs `stage0 + baseline + stage1 + stage2 + stage3 + stage4 + stage5 + stage6 + stage7 + stage8`

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
- `WORKLOAD_BURST_ENABLED`
- `WORKLOAD_BURST_RATE_TPS`
- `WORKLOAD_BURST_PERIOD_S`
- `WORKLOAD_BURST_DUTY_CYCLE`
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
- `transfer`
- `da_heavy`

The runner currently defaults to:

- `TX_MIX=balanced`

Burst cases use a timed workload where the generator switches between base `RATE_TPS` and `WORKLOAD_BURST_RATE_TPS` according to the configured burst period and duty cycle.

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
- `batch_policy`
- `min_batch_size`
- `adaptive_*`
- `blob_target_bytes`
- `blob_fill_target`
- `workload_burst_*`
- `hardhat_mining_interval`
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
- `batch_data_bytes_vs_tx_count.png`
- `state_diff_count_vs_tx_count.png`
- `unique_touched_accounts_vs_tx_count.png`
- `execution_time_vs_tx_count.png`
- `proof_time_vs_tx_count.png`
- `l1_gas_used_vs_tx_count.png`
- `blob_utilization_vs_tx_count.png`
- `l1_latency_vs_tx_count.png`

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

### 3. Plan-specific env vars are propagated

The implementation propagates the plan knobs used by the staged matrix, including:

- `PROVER_BACKEND`
- `REQUIRE_REAL_PROOFS`
- `ALLOW_PROOF_FALLBACK`
- `ALLOW_UNSIGNED_USER_TXS`
- `COMM_MODE`
- `BATCH_POLICY`
- `ADAPTIVE_*`
- `BLOB_TARGET_BYTES`
- `BLOB_FILL_TARGET`
- `WORKLOAD_BURST_*`
- `HARDHAT_MINING_INTERVAL`

### 4. `stage8` is representative, not auto-optimized

`stage8` compares representative best configurations from the plan. It does not automatically read earlier stage results and rewrite itself. For final dissertation-quality claims, inspect stages 1-7 first, then adjust the `stage8` overrides in `benchmark-suite/scripts/run_plan_benchmark.py` if your measured winners differ.

### 5. `stage7` is a harness-level reliability sweep

`stage7` covers retry count, publish timeout, communication mode, and burst-load recovery behavior. It does not currently stop containers mid-run or inject RPC outages; add those as separate fault-injection hooks if you need hard failure-recovery experiments beyond parameter stress.

### 6. `smoke` profile is for correctness, not research-quality data

Because `smoke` uses:

- `WORKLOAD_TARGET_TXS=1`

it is mainly a pipeline validation profile.

Use `pilot` or `final` for real plots and comparisons.

### 7. Existing worktree caveat

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

- a staged benchmark runner aligned to stages 0-8 of the written plan
- reusable per-run execution via `run_experiment.sh`
- automatic session manifests
- automatic aggregation and statistics
- automatic plot generation
- automatic Markdown summary report generation
- organized session output directories for reproducibility
