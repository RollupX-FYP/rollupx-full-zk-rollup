# RollupX Stage 2 Benchmark Analysis Report: Adaptive Batching Comparison

This document provides a comprehensive analysis of the performance metrics gathered during **Stage 2 (Adaptive Batching Comparison)** of the RollupX benchmarking plan. It outlines the experimental configurations, details findings and trade-offs, reveals critical orchestration and design anomalies, and proposes visualizations to support the findings.

---

## 1. Executive Summary

Stage 2 benchmarks evaluate the performance of the **Adaptive Batching Policy** compared to the **Fixed Batching Policy** under varying offered load profiles (low, medium, high, and burst) and adaptive threshold configurations. Analysis of the raw data reveals three primary insights:

1. **Orchestration Mismatch (Configuration Anomaly)**: Due to a manual environment-variable propagation bug during the high-timeout reruns on May 19, all low-load, medium-load, and threshold sweep experiments (`s2_adaptive_low`, `s2_adaptive_medium`, `s2_adapt_l10_m50`, `s2_adapt_l25_m100`, `s2_adapt_l50_m150`) were executed using the `fixed` batch policy instead of the planned `adaptive` policy. Consequently, their performance is identical to their fixed-batching counterparts.
2. **The Hysteresis Threshold Trap (Design Limitation)**: A mathematical audit of the batch trigger logic in [trigger.rs](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/sequencer/src/batch/trigger.rs#L158-L176) shows that if `adaptive_small_batch_size >= adaptive_low_load_threshold`, the sequencer's size-threshold trigger will **never** fire at the smaller batch size under steady transaction arrival. The target size immediately jumps to a larger size before the mempool accumulates enough transactions to seal at the smaller size.
3. **High-Load Policy Convergence**: Under sustained high offered load (60 TPS), the mempool depth consistently exceeds the medium and large thresholds. As a result, the adaptive policy's target batch size scales up to the maximum batch size limit (`max_batch_size = 100`), behaving identically to fixed batching.
4. **Burst Load Equivalence**: Under the burst workload (8 TPS base, 80 TPS burst), the adaptive policy behaves identically to the fixed policy. It seals on timeout during the base rate (since mempool depth never reaches the low threshold before the timeout) and seals on size at 100 during the burst rate (since it crosses the low threshold in ~300 ms, shifting the target size to 100).

---

## 2. Overview of Stage 2 & Experimental Setup

Stage 2 sweeps evaluate the RollupX sequencer's ability to adaptively adjust batch sealing sizes dynamically based on mempool depth.

### Sealing Mechanics

The sequencer's batch trigger evaluates three trigger conditions in priority order:
1. **Forced L1 Transactions**: Sealed immediately.
2. **Size Threshold**: Sealed when `pool_size >= target_batch_size`.
3. **Timeout**: Sealed when `time_elapsed >= timeout_interval_ms` (if `pool_size >= min_batch_size`).

Under the **Fixed Policy**, the target batch size is constant and equal to `max_batch_size`.
Under the **Adaptive Policy**, the target batch size varies dynamically based on the number of pending transactions in the mempool:
* $\text{pending\_count} < \text{adaptive\_low\_load\_threshold} \implies \text{target} = \text{adaptive\_small\_batch\_size}$
* $\text{pending\_count} \le \text{adaptive\_medium\_load\_threshold} \implies \text{target} = \text{adaptive\_medium\_batch\_size}$
* Otherwise $\implies \text{target} = \text{adaptive\_large\_batch\_size}$

*(Note: Target size is always capped at `max_batch_size`)*

### Experimental Matrix in Stage 2
* **Low Load (10 TPS)**: Fixed vs. Adaptive with a high timeout (30.0s) to allow size-driven sealing under low arrival rates.
* **Medium Load (10 TPS actual / 25 TPS planned)**: Fixed vs. Adaptive under a moderate arrival rate.
* **High Load (60 TPS)**: Fixed vs. Adaptive under a backlogged rate.
* **Burst Load**: Base 8 TPS, bursting to 80 TPS (30s period, 25% duty cycle).
* **Threshold Sweeps**: Sweeping low/medium load thresholds (`s2_adapt_l10_m50`, `s2_adapt_l25_m100`, `s2_adapt_l50_m150`) to observe sensitivity to transition boundaries.

---

## 3. Key Findings & Strong Deductions

### Deduction 1: The Orchestration Configuration Bug
In [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage2_adaptive_batching/analysis/all_results.csv), the columns `batch_policy` and `timeout_ms` show a clear anomaly:
* The runs on May 19 (`s2_adaptive_low`, `s2_adaptive_medium`, and all `s2_adapt_*` threshold sweeps) show `batch_policy = fixed` in their metadata, despite `plan_manifest.csv` requesting `adaptive`.
* The reason lies in the manual run command for these high-timeout reruns. In [run_experiment.sh](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/scripts/run_experiment.sh#L30), the script sets `export BATCH_POLICY=${BATCH_POLICY:-fixed}`. Because the manual run command did not explicitly set `BATCH_POLICY=adaptive` in the environment shell, it defaulted to `fixed`.
* Consequently, **these "adaptive" runs were actually executed as fixed-batching runs**, leading to identical results with their fixed counterparts:
  * `s2_adaptive_low` vs `s2_fixed_low`: Both committed ~9.38 TPS, average batch size ~99, L1 gas/tx ~19.6k, wait time ~7.0s.
  * `s2_adaptive_medium` vs `s2_fixed_medium`: Both committed ~9.38 TPS, average batch size ~99, L1 gas/tx ~19.6k, wait time ~7.0s.

### Deduction 2: The Adaptive Hysteresis Trap (Mathematical Proof)
A fundamental design flaw exists in the sequencer's dynamic target computation in [trigger.rs](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/sequencer/src/batch/trigger.rs#L158-L176).

Let $L_t$ be `adaptive_low_load_threshold` and $S_b$ be `adaptive_small_batch_size`.
Suppose transactions arrive steadily. For the sequencer to seal a batch of size $S_b$ based on the size threshold, the mempool pending count $P$ must satisfy:
$$P \ge \text{target\_batch\_size\_for\_depth}(P)$$

For the target size to equal $S_b$, we must satisfy the low load condition:
$$P < L_t$$

Therefore, a size-driven seal at the small batch size requires:
$$S_b \le P < L_t$$

This inequality can only be satisfied if:
$$S_b < L_t$$

If $S_b \ge L_t$ (i.e., the small batch size is greater than or equal to the low load threshold), then:
* When $P < L_t$, the target size is $S_b$. But since $P < L_t \le S_b$, the condition $P \ge S_b$ is never met.
* The moment $P$ reaches $L_t$, the condition $P < L_t$ becomes false. The target size immediately jumps to `adaptive_medium_batch_size` (usually 100 or higher, capped at `max_batch_size`).
* Since the new target is much larger, the sequencer does not seal. The pending count continues to accumulate towards the larger target.

**Implication**: In all configured experiments:
* `s2_adaptive_low`: $S_b = 50, L_t = 50 \implies S_b \ge L_t$
* `s2_adapt_l10_m50`: $S_b = 25, L_t = 10 \implies S_b > L_t$
* `s2_adapt_l25_m100`: $S_b = 50, L_t = 25 \implies S_b > L_t$
* `s2_adapt_l50_m150`: $S_b = 50, L_t = 50 \implies S_b \ge L_t$

Under all these configurations, the sequencer is mathematically blocked from ever sealing at the smaller batch size under steady load. It will always bypass the small batch size and accumulate transactions until it hits the medium batch size or the maximum batch size (100). Thus, **even if the adaptive policy had been active, the results would still have converged to fixed batching at size 100**.

### Deduction 3: High-Load Policy Convergence
Under high offered load (60 TPS, `s2_adaptive_high` vs `s2_fixed_high`):
* The arrival rate is high enough that the mempool pending count is almost always greater than the medium load threshold ($L_m = 100$).
* In this regime, the target batch size is computed as `adaptive_large_batch_size` (500) capped at `max_batch_size` (100).
* Since both policies use a target batch size of 100, the adaptive policy's behavior converges completely to the fixed policy.
* This is visible in the metrics:
  * Average batch size: **97.12** (adaptive) vs **97.93** (fixed)
  * Committed TPS: **65.29** for both
  * L1 gas/tx: **19,941** (adaptive) vs **19,713** (fixed)
  * Avg L2->L1 latency: **7031 ms** (adaptive) vs **7028 ms** (fixed)

### Deduction 4: Burst Load Mechanics
Under burst load (8 TPS base, 80 TPS burst, `s2_adaptive_burst` vs `s2_fixed_burst`):
* During the **base period (22.5s)**: Transactions arrive at 8 TPS. With a 2s timeout, only ~16 transactions accumulate. Since 16 is below the target batch size (100 for fixed, 25 for adaptive), both policies seal on timeout with a batch size of 16.
* During the **burst period (7.5s)**: Transactions arrive at 80 TPS. The mempool depth crosses the low threshold (25) in 312 ms, which shifts the adaptive target to 100. Both policies accumulate 100 transactions and seal on size threshold in 1.25s.
* Because both policies behave identically in both phases, their overall averages are equivalent:
  * Committed TPS: **29.31** (adaptive) vs **29.09** (fixed)
  * Average batch size: **49.77** (adaptive) vs **49.41** (fixed)
  * L1 gas/tx: **21,046** (adaptive) vs **21,081** (fixed)
  * Avg L2->L1 latency: **7026 ms** (adaptive) vs **7025 ms** (fixed)

---

## 4. Summary Table of Stage 2 Results

The table below summarizes the key metrics extracted from [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage2_adaptive_batching/analysis/all_results.csv).

| Experiment ID | Batch Policy (Planned) | Batch Policy (Actual) | Rate TPS (Offered) | Timeout (ms) | Total Batches | Average Batch Size | Committed TPS | Avg L1 Gas/Tx | Jain's Fairness | Avg L2->L1 (ms) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **baseline** | fixed | fixed | 25 | 2,000 | 97 | 36.86 | 19.86 | 20,741.68 | 0.76 | 7,024.54 |
| **s2_fixed_low** | fixed | fixed | 10 | 30,000 | 17 | 98.88 | 9.34 | 19,671.49 | 0.77 | 7,033.18 |
| **s2_adaptive_low** | adaptive | **fixed (bug)** | 10 | 30,000 | 17 | 99.35 | 9.38 | 19,667.11 | 0.76 | 7,034.65 |
| **s2_fixed_medium** | fixed | fixed | 25 | 30,000 | 17 | 98.94 | 9.34 | 19,665.76 | 0.76 | 7,035.00 |
| **s2_adaptive_medium** | adaptive | **fixed (bug)** | 25 | 30,000 | 17 | 99.35 | 9.38 | 19,671.80 | 0.77 | 7,034.24 |
| **s2_fixed_high** | fixed | fixed | 60 | 2,000 | 120 | 97.93 | 65.29 | 19,713.93 | 0.76 | 7,028.56 |
| **s2_adaptive_high** | adaptive | adaptive | 60 | 2,000 | 121 | 97.12 | 65.29 | 19,941.66 | 0.76 | 7,031.02 |
| **s2_fixed_burst** | fixed | fixed | 8 | 2,000 | 106 | 49.41 | 29.09 | 21,081.18 | 0.75 | 7,025.05 |
| **s2_adaptive_burst** | adaptive | adaptive | 8 | 2,000 | 106 | 49.77 | 29.31 | 21,046.18 | 0.77 | 7,026.43 |
| **s2_adapt_l10_m50** | adaptive | **fixed (bug)** | 10 | 30,000 | 17 | 99.12 | 9.36 | 19,665.77 | 0.76 | 7,034.82 |
| **s2_adapt_l25_m100** | adaptive | **fixed (bug)** | 10 | 30,000 | 17 | 99.00 | 9.35 | 19,669.27 | 0.77 | 7,033.76 |
| **s2_adapt_l50_m150** | adaptive | **fixed (bug)** | 10 | 30,000 | 17 | 99.00 | 9.35 | 19,664.91 | 0.76 | 7,035.65 |

---

## 5. Proposed Visualizations & Plots

To present these insights in academic papers or presentations, we propose the following three plots:

### Plot 1: Policy Performance Convergence under Load (Grouped Bar Chart)
* **Rationale**: Shows how committed TPS and L1 gas efficiency converge between Fixed and Adaptive policies as offered load increases, proving policy convergence.
* **X-Axis**: Offered Load (Low (10 TPS), Medium (25 TPS), High (60 TPS))
* **Y-Axis**: Committed TPS (`tps_committed`) / Average Gas per Transaction (`avg_gas_per_tx`)
* **Visual Elements**: Side-by-side bars comparing Fixed vs. Adaptive configurations.
* **Data Sources**:
  * X-axis: `rate_tps` column in [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage2_adaptive_batching/analysis/all_results.csv)
  * Y1-axis: `tps_committed` in [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage2_adaptive_batching/analysis/all_results.csv)
  * Y2-axis: `avg_gas_per_tx` in [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage2_adaptive_batching/analysis/all_results.csv)
  * Filter runs: `s2_fixed_low`, `s2_adaptive_low`, `s2_fixed_medium`, `s2_adaptive_medium`, `s2_fixed_high`, `s2_adaptive_high`.

### Plot 2: Burst Load Batch Size and Trigger Reason Timeline (Scatter Plot)
* **Rationale**: Illustrates the dynamic transition of trigger reasons (timeout vs size threshold) and batch sizes over time under burst loads, confirming the two-phase burst behavior.
* **X-Axis**: Batch Submission Time (or Batch ID)
* **Y-Axis**: Batch Size (`tx_count`)
* **Color/Marker Style**: Grouped by trigger reason (`seal_reason` = `Timeout` vs `SizeThreshold`).
* **Data Sources**:
  * X-axis: `batch_id` in [all_batch_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage2_adaptive_batching/analysis/all_batch_results.csv)
  * Y-axis: `tx_count` in [all_batch_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage2_adaptive_batching/analysis/all_batch_results.csv)
  * Color: `seal_reason` in [all_batch_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage2_adaptive_batching/analysis/all_batch_results.csv)
  * Filter runs: `s2_adaptive_burst` and `s2_fixed_burst`.

### Plot 3: The Hysteresis Trap Step-Function (Conceptual/Line Plot)
* **Rationale**: Visualizes the target batch size function compared to the mempool depth, graphically showing why $S_b \ge L_t$ prevents the sequencer from sealing at $S_b$.
* **X-Axis**: Pending Transactions in Pool ($P$)
* **Y-Axis**: Target Batch Size ($T$)
* **Visual Elements**:
  * Step function representing $\text{target\_batch\_size\_for\_depth}(P)$.
  * A diagonal line $T = P$ representing the seal threshold.
  * Highlight the intersection (or lack thereof) in the low-load region ($P < L_t$). Show that the target step function is always strictly above the diagonal when $S_b \ge L_t$, preventing size-based sealing.

---

## 6. Recommendations & Parameter Tuning

To fix the observed design and orchestration issues in future benchmark stages:

1. **Fix Orchestration Scripts**: Update [run_plan_benchmark.py](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/scripts/run_plan_benchmark.py#L445-L463) or the manual execution environment to ensure that `BATCH_POLICY` is always explicitly exported when running the docker stack, avoiding reliance on defaults in `run_experiment.sh`.
2. **Resolve the Hysteresis Trap**: Modify the adaptive batching configuration to ensure that the small batch size is strictly less than the low load threshold:
   $$\text{adaptive\_small\_batch\_size} < \text{adaptive\_low\_load\_threshold}$$
   For example, set:
   * `adaptive_low_load_threshold = 50`
   * `adaptive_small_batch_size = 25`
   This configuration will allow size-driven sealing to trigger at 25 transactions under low load, highlighting the true benefits of adaptive batching compared to fixed batching.
