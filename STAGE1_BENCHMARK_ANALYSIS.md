# RollupX Stage 1 Benchmark Analysis Report: Fixed Batching Sweeps

This document provides a comprehensive analysis of the performance metrics gathered during **Stage 1 (Fixed Batching Sweeps)** of the RollupX benchmarking plan. It outlines the experimental configurations, details key performance trade-offs, reveals critical statistical anomalies in the aggregation framework, and proposes visualizations to support these findings.

---

## 1. Executive Summary

Stage 1 sweeps evaluate the performance of a **Fixed Batching Policy** under varying constraints of maximum batch size, timeout limits, and workload shapes. Analysis of the raw data reveals two primary insights:
1. **The Latency-Cost Trade-Off**: Larger batch sizes reduce per-transaction L1 gas costs by amortizing fixed submission overhead, but increase mempool queuing delays.
2. **Methodological Anomalies**: The reported metrics in `all_results.csv` suffer from severe boundary-effect distortion at the end of finite test runs:
   * **Tail Latency Inflation**: The lack of an end-of-test mempool flush causes the final batch to wait for the entire timeout duration (up to 200s), distorting the average queue wait time.
   * **Empty Batch Cost Inflation**: Empty batches submitted at the end of runs are aggregated using `max(tx_count, 1)` in unweighted simple averages, artificially inflating the reported gas per transaction for large batch configs (e.g., from an actual ~19.7k gas/tx to 83.6k gas/tx for `s1_bs_1000`).

> [!IMPORTANT]
> **Comparability Note**: USD cost metrics should not be compared across runs directly because sweeps from May 18 (timeouts, workloads) and May 19 (batch sizes) used different environment parameters:
> * **May 18**: `ETH_PRICE_USD` = $3000, `REGULAR_GAS_PRICE_GWEI` = 10.0
> * **May 19**: `ETH_PRICE_USD` = $2500, `REGULAR_GAS_PRICE_GWEI` = 2.0
>
> All comparative efficiency analysis must use **Gas Units** (L1 Gas Used or Gas per Transaction), which remain constant and independent of market conditions.

---

## 2. Overview of Stage 1 & Experimental Setup

Stage 1 sweeps test the boundaries of the sequencer's fixed batching scheduler. Under this policy, a batch is sealed and sent for execution when either:
1. The number of transactions in the mempool reaches `MAX_BATCH_SIZE`.
2. The time elapsed since the last batch seal reaches `TIMEOUT_MS`.

### Experimental Sweeps
* **Baseline**: Reference configuration (`MAX_BATCH_SIZE=100`, `TIMEOUT_MS=2000` (2s), 25 offered TPS, balanced workload mix).
* **Batch Size Sweeps (`s1_bs_*`)**: Capped sizes from 25 to 1000 transactions, with the timeout disabled (`TIMEOUT_MS=200000` or 200s) to force size-driven sealing.
* **Timeout Sweeps (`s1_to_*`)**: Timeout limits from 500 ms to 10,000 ms (10s), with `MAX_BATCH_SIZE=100` and rate = 25 TPS.
* **Workload Sweeps (`s1_wl_*`)**: Arrival rates and mixes representing different transaction payloads:
  * **Normal**: Balanced mix, 25 offered TPS, concurrency = 1 (achieves ~18.5 actual TPS due to gRPC sequential client bottlenecks).
  * **Transfer**: Simple transfers, 40 offered TPS, concurrency = 2 (achieves ~40.2 actual TPS).
  * **Heavy**: Complex execution, 25 offered TPS, concurrency = 2 (achieves ~25.1 actual TPS).

---

## 3. Key Findings & Strong Deductions

### Deduction 1: Decoupling L2 Execution from L1 Gas
In ZK-rollups, the L1 contract does not execute L2 transactions; it only verifies the validity proof and stores the state transition data (DA). 
* The L1 gas cost of a batch submission is composed of a fixed verifier/bridge overhead (~29,000 gas with mock verification) and a linear calldata data availability cost (~40.13 gas per byte of batch data).
* Comparing **`s1_wl_heavy`** (heavy execution) and **`s1_wl_normal`** (balanced mix): despite the heavy workload requiring more L2 virtual machine steps, its L1 gas cost per transaction is actually slightly lower (19,997 gas vs 20,196 gas) because the transaction payload (calldata bytes) is slightly smaller. 
* **Takeaway**: On-chain L1 gas cost is completely decoupled from L2 execution complexity and depends strictly on the serialized calldata footprint.

### Deduction 2: The Queueing Latency Model
Under uniform transaction arrival rate $\lambda$ (TPS), the time to accumulate a batch of size $B$ is $B/\lambda$. The average wait time of transactions in a sealed batch is half the sealing interval:
$$W_q \approx \frac{1}{2} \min\left(T_{\text{timeout}}, \frac{B_{\text{max}}}{\lambda}\right)$$

This formula fits the experimental data with high accuracy:
* **`s1_to_00500`** (0.5s timeout): Theoretical wait = 250 ms. Actual = **256.4 ms**.
* **`s1_to_01000`** (1.0s timeout): Theoretical wait = 500 ms. Actual = **516.3 ms**.
* **`s1_to_02000`** (2.0s timeout): Theoretical wait = 1000 ms. Actual = **1017.2 ms**.
* **`s1_to_05000`** (5.0s timeout): Theoretical wait = 2500 ms. Actual = **2487.0 ms**.
* **`s1_to_10000`** (10.0s timeout): The batch fills to 100 txs before the timeout. Sealing interval = $100 / 18.5 \approx 5.4$s. Theoretical wait = 2700 ms. Actual = **2937.1 ms**.

> [!NOTE]
> For `s1_to_10000`, the timeout is never reached because the batch size cap of 100 acts as a ceiling, stopping latency accumulation once the arrival rate is high enough.

### Deduction 3: L1 Confirmation Latency is Dominated by L1 Mining
The L2-to-L1 transaction latency (`avg_l2_l1_ms`) remains constant at ~7.0 seconds across all Stage 1 sweeps.
* This is because the test environment uses a Hardhat mining interval of 12 seconds (`HARDHAT_MINING_INTERVAL=12000`).
* When the submitter sends a batch, the transaction waits in the L1 mempool for the next block. The average wait is half the block time (6s) plus RPC and submission overhead (~1s), resulting in ~7.0s latency regardless of L2 parameters.
* **Takeaway**: L1 block intervals mask the L2 finality improvements of smaller or faster batches.

### Deduction 4: Tail Batch Latency Distortion
The reported queue wait time for `s1_bs_0100` is 8,212.60 ms, which is far higher than the expected ~2,700 ms.
* Analysis of the raw JSONL shows that batches 1-35 had a wait time of ~2,700 ms. However, Batch 36 (the final batch containing 80 transactions) waited for **198,247 ms** (nearly 200 seconds) because the workload generator stopped, and the sequencer held the transactions in the mempool until the 200s timeout expired.
* Because the aggregator computes a simple arithmetic mean of the batch averages, this single tail batch skews the entire run metric:
$$\text{Reported Mean} = \frac{35 \times 2700\text{ ms} + 198247\text{ ms}}{36} \approx 8212\text{ ms}$$
* **Takeaway**: High timeouts combined with finite test runs create severe tail-end latency outliers that distort simple averages.

### Deduction 5: Empty Batch Cost Distortion
For `s1_bs_1000`, the reported gas per transaction is **83,626 gas**, while smaller batch sizes report ~19,600 gas.
* Raw metrics show that the run produced 6 batches. Batches 1 and 2 were full (605 and 661 txs), Batch 3 had 1 tx, and Batches 4, 5, and 6 had **0 txs** (empty).
* The aggregator uses `max(tx_count, 1)` to avoid division by zero when calculating per-batch gas ratios:
  * Batch 4 (empty): $115,132\text{ gas} / 1\text{ tx} = 115,132\text{ gas/tx}$.
  * Batch 5 (empty): $112,332\text{ gas} / 1\text{ tx} = 112,332\text{ gas/tx}$.
* Taking the simple mean of these ratios results in 83,626 gas. The true weighted average (total gas / total txs) is actually **19,771 gas/tx**, which aligns perfectly with the amortization curve.
* **Takeaway**: Empty batches submitted at the end of runs penalize efficiency metrics when using unweighted ratio averages.

---

## 4. Summary Table of Reported vs. True Weighted Metrics

Below is the comparative summary of the 15 runs. The "True Weighted Gas/Tx" column filters out the statistical bias introduced by the `max(tx_count, 1)` aggregation floor.

| Experiment ID | Total Batches | Empty Batches | Total Transactions | Total L1 Gas Used | Reported Gas/Tx (Distorted) | True Weighted Gas/Tx | Avg Queue Wait (ms) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **baseline** | 97 | 0 | 3,599 | 72,692,381 | 20,516.63 | **20,197.94** | 1,005.89 |
| **s1_bs_0025** | 144 | 0 | 3,594 | 73,854,810 | 20,550.06 | **20,549.47** | 1,392.44 |
| **s1_bs_0050** | 72 | 0 | 3,589 | 71,669,600 | 19,969.61 | **19,969.24** | 2,738.93 |
| **s1_bs_0100** | 36 | 0 | 3,580 | 70,460,650 | 19,682.34 | **19,681.75** | 8,212.60 |
| **s1_bs_0200** | 19 | 0 | 3,604 | 70,478,002 | 20,443.81 | **19,555.49** | 10,448.34 |
| **s1_bs_0500** | 8 | 0 | 3,606 | 70,170,660 | 19,482.66 | **19,459.42** | 74,209.82 |
| **s1_bs_1000** | 6 | 3 | 1,267 | 25,050,047 | 83,626.43 | **19,771.15** | 115,429.28 |
| **s1_to_00500** | 381 | 0 | 3,580 | 81,957,835 | 23,549.75 | **22,893.25** | 256.48 |
| **s1_to_01000** | 193 | 0 | 3,580 | 75,143,868 | 21,390.83 | **20,989.91** | 516.32 |
| **s1_to_02000** | 97 | 0 | 3,588 | 72,471,336 | 20,737.59 | **20,198.25** | 1,017.27 |
| **s1_to_05000** | 40 | 0 | 3,580 | 70,557,320 | 19,741.49 | **19,708.75** | 2,487.02 |
| **s1_to_10000** | 36 | 0 | 3,572 | 70,297,420 | 19,681.09 | **19,680.13** | 2,937.16 |
| **s1_wl_normal** | 97 | 0 | 3,575 | 72,201,096 | 20,516.94 | **20,196.11** | 999.84 |
| **s1_wl_transfer**| 98 | 0 | 7,820 | 154,597,325| 20,999.14 | **19,769.48** | 1,015.18 |
| **s1_wl_heavy** | 97 | 0 | 4,860 | 97,185,346 | 20,104.12 | **19,996.98** | 1,002.42 |

---

## 5. Proposed Visualizations & Plots

To illustrate these deductions in academic reports or presentations, the following four plots are proposed.

### Plot 1: Batch Size vs. Queue Latency & Gas Cost (Dual-Axis)
* **Rationale**: Shows the core batch size trade-off. As batch size grows, gas efficiency increases (diminishing returns) while queue latency grows.
* **X-Axis**: Batch Size (`batch_size`)
* **Left Y-Axis (Line/Scatter)**: Median/Mean Queue Wait Time (`p50_queue_wait_ms` or `avg_queue_wait_ms`). *Note: Exclude the tail batch outlier or use the median to prevent skew.*
* **Right Y-Axis (Line/Scatter)**: True Gas per Transaction (computed from `total_gas / total_txs`).
* **Data Sources**:
  * X-axis: `batch_size` column in [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage1_fixed_batching/analysis/all_results.csv)
  * Y1-axis: `p50_queue_wait_ms` in [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage1_fixed_batching/analysis/all_results.csv)
  * Y2-axis: `true_gas_per_tx` column in [gas_check_analysis.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage1_fixed_batching/analysis/gas_check_analysis.csv)
  * Filter runs: `s1_bs_0025` through `s1_bs_1000`.

### Plot 2: Timeout vs. Queue Latency & Batch Occupancy (Dual-Axis)
* **Rationale**: Visualizes how the timeout acts as the primary batch sealing trigger until the batch size cap (100) is reached.
* **X-Axis**: Timeout (ms) (`timeout_ms`)
* **Left Y-Axis (Line)**: Average Queue Wait Time (`avg_queue_wait_ms`)
* **Right Y-Axis (Bar)**: Average Batch Size (`avg_batch_tx_count`)
* **Data Sources**:
  * X-axis: `timeout_ms` in [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage1_fixed_batching/analysis/all_results.csv)
  * Y1-axis: `avg_queue_wait_ms` in [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage1_fixed_batching/analysis/all_results.csv)
  * Y2-axis: `avg_batch_tx_count` in [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage1_fixed_batching/analysis/all_results.csv)
  * Filter runs: `s1_to_00500` through `s1_to_10000`.

### Plot 3: Queue Wait Time Distribution (Box & Whisker)
* **Rationale**: Highlights the tail-latency skew caused by the boundary effects of finite test runs.
* **X-Axis**: Experiment ID (`s1_bs_0100`, `s1_bs_0200`, `s1_bs_0500`, `s1_bs_1000`)
* **Y-Axis (Log Scale)**: Queue Wait Time (ms)
* **Visual Elements**: Box plots showing P50, P95, P99, and Max wait times to highlight the massive distance between the median and the outliers.
* **Data Sources**:
  * Raw values for each batch from `wait_time_mean_ms` in [sequencer_batch_metrics.jsonl](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage1_fixed_batching/s1_bs_0100/s1_bs_0100_r01_20260519_155141/sequencer_batch_metrics.jsonl) (and corresponding paths for other batch sizes).

### Plot 4: Workload Mix vs. Gas per Transaction (Grouped Bar Chart)
* **Rationale**: Proves the decoupling of L2 VM execution from L1 gas costs.
* **X-Axis**: Workload Mix (`balanced`, `transfer`, `heavy`)
* **Y-Axis**: True Gas per Transaction
* **Data Sources**:
  * X-axis: `tx_mix` in [all_results.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage1_fixed_batching/analysis/all_results.csv)
  * Y-axis: `true_gas_per_tx` in [gas_check_analysis.csv](file:///c:/Lishan%20Dissanayake/4%29%20Projects/FYP/rollupx-full-zk-rollup/benchmark-suite/metrics/final_stage1_fixed_batching/analysis/gas_check_analysis.csv)
  * Filter runs: `s1_wl_normal`, `s1_wl_transfer`, `s1_wl_heavy`.

---

## 6. Threat to Validity: Mock Verification

A critical parameter in these experiments is the use of the **Mock Verifier**.
* The smart contract verifier for Groth16 (`verifierId=0`) was deployed using `MockVerifier.sol`, which performs a simple `return true` and consumes negligible gas (~1,000 gas).
* In production, a real Groth16 verifier contract performs several bilinear pairing operations, consuming a fixed ~250,000 gas per verification.
* If a real verifier were used, the fixed L1 overhead would increase from ~29,000 gas to ~279,000 gas. This would make the amortization benefit of larger batch sizes **significantly more pronounced**:
  * For Batch Size 25: Gas per tx would increase from ~20,549 to **~30,515 gas**.
  * For Batch Size 500: Gas per tx would increase from ~19,459 to **~20,060 gas**.
  * The actual cost reduction from scaling batch size would jump from **5%** (mock verifier) to **34%** (real verifier).
* **Recommendation**: When publishing these results, explicitly state that the baseline gas amortization curve represents a lower bound, and that real-world cryptographic verification will heavily penalize smaller batch sizes.
