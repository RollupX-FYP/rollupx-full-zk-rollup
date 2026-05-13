# RollupX Benchmarking Plan

**Project:** CS4203 — Improving Blockchain Scalability: Throughput Enhancement in ZK-Rollups  
**Prototype:** RollupX — modular Rust-based ZK-rollup on Ethereum Sepolia testnet  
**Purpose of this document:** Complete specification for a coding agent to implement, configure, and execute the full benchmarking suite.

---

## Table of Contents

1. [Goals and Research Questions](#1-goals-and-research-questions)
2. [Global Constraints and Fixed Decisions](#2-global-constraints-and-fixed-decisions)
3. [Workload Generator Specification](#3-workload-generator-specification)
4. [Metrics Collection Specification](#4-metrics-collection-specification)
5. [Statistical Analysis Plan](#5-statistical-analysis-plan)
6. [Experiment 1 — Batch Size Sweep](#6-experiment-1--batch-size-sweep)
7. [Experiment 2 — Scheduling Policy Comparison](#7-experiment-2--scheduling-policy-comparison)
8. [Experiment 3 — Data Availability Mode Comparison](#8-experiment-3--data-availability-mode-comparison)
9. [Experiment 4 — Cross-Factor Interaction (Novel Contribution)](#9-experiment-4--cross-factor-interaction-novel-contribution)
10. [Experiment 5 — Adaptive vs Fixed Batching Under Bursty Load](#10-experiment-5--adaptive-vs-fixed-batching-under-bursty-load)
11. [Experiment Sequencing and Dependencies](#11-experiment-sequencing-and-dependencies)
12. [Harness Implementation Notes](#12-harness-implementation-notes)
13. [Output Artefacts and File Structure](#13-output-artefacts-and-file-structure)

---

## 1. Goals and Research Questions

The benchmarking suite is designed to produce evidence addressing three research questions:

| ID  | Research Question                                                                                                                                                               |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| RQ2 | How do batch size & frequency, sequencing policies, and DA modes (calldata / EIP-4844 blobs / offchain) affect end-to-end performance (TPS, latency, gas/tx, proof time)?       |
| RQ3 | To what extent can benchmark-driven tuning improve the scalability of a modular ZK-rollup?                                                                                      |
| Gap | The literature lacks controlled studies of how batch size, scheduling, and DA mode **interact** to affect performance simultaneously. Experiment 4 directly addresses this gap. |

Each experiment maps to one or more of these:

| Experiment                     | RQs addressed |
| ------------------------------ | ------------- |
| 1 — Batch size sweep           | RQ2, RQ3      |
| 2 — Scheduling policy          | RQ2, RQ3      |
| 3 — DA mode                    | RQ2, RQ3      |
| 4 — Cross-factor interaction   | RQ2, RQ3, Gap |
| 5 — Adaptive vs fixed batching | RQ3           |

---

## 2. Global Constraints and Fixed Decisions

These settings apply to **all** experiments unless explicitly overridden in the experiment specification.

### 2.1 Proof mode

```
REQUIRE_REAL_PROOFS=false
ALLOW_PROOF_FALLBACK=true
PROVER_BACKEND=mock
```

**Rationale:** Full Groth16/Plonky2 proof generation is the dominant latency source (orders of magnitude slower than batching/scheduling/DA operations). Enabling real proofs would drown out the signal from the variables under study. All five experiments are about batching, scheduling, and DA — not proving overhead. If proving latency is desired as a separate characterization, it must be a standalone micro-benchmark outside this suite.

### 2.2 Network and testnet

```
network.rpc_url = <Sepolia RPC>
network.chain_id = 11155111
HARDHAT_MINING_INTERVAL = 2000   # 2s block time, approximating Sepolia
```

For experiments requiring a deterministic local L1 (Experiments 1, 2, 5), a local Hardhat node is preferred to eliminate public testnet congestion variance. For Experiment 3 (DA mode, which measures real gas costs) and Experiment 4, use Sepolia.

### 2.3 Unsigned transactions

```
ALLOW_UNSIGNED_USER_TXS=true
```

Required for the synthetic workload generator to submit transactions without managing a wallet keystore per sender.

### 2.4 Run duration and warm-up

- **Warm-up period:** 30 seconds. Discard all metrics collected during this window. The system must reach steady-state transaction processing before measurement begins.
- **Measurement window:** 300 seconds (5 minutes) of steady-state operation per run.
- **Repetitions:** Minimum 3 independent runs per configuration. Each run restarts the sequencer and executor from clean state (empty mempool, reset state tree).
- **Random seed:** Fix the workload generator seed per experiment (not per run) so each repetition uses the same transaction sequence. This allows direct comparison across configurations for the same workload.

### 2.5 Compute environment

Record the following in every results file:

- CPU model, core count, RAM
- OS and kernel version
- Rust toolchain version (`rustc --version`)
- RollupX git commit hash
- Timestamp (UTC ISO-8601)

---

## 3. Workload Generator Specification

### 3.1 Transaction distribution

Transactions are generated according to a **Poisson arrival process** with configurable rate parameter λ (transactions per second). The inter-arrival time between consecutive transactions is exponentially distributed with mean 1/λ seconds.

Default steady-state λ: **50 tx/s** (used in all experiments unless the experiment section specifies otherwise).

### 3.2 Transaction type mix

| Type       | Proportion | Description                         |
| ---------- | ---------- | ----------------------------------- |
| Transfer   | 70%        | Token transfer between two accounts |
| Deposit    | 15%        | L1→L2 deposit                       |
| Withdrawal | 15%        | L2→L1 withdrawal                    |

### 3.3 Fee distribution (for scheduling policy experiments)

For Experiment 2 (scheduling), the workload must contain fee-varied transactions to give the FeePriority and TimeBoost policies meaningful signal:

| Tier         | Proportion | Fee multiplier |
| ------------ | ---------- | -------------- |
| Low fee      | 40%        | 1× base fee    |
| Normal fee   | 30%        | 2× base fee    |
| High fee     | 20%        | 5× base fee    |
| Priority fee | 10%        | 10× base fee   |

For all other experiments, use uniform fees (1× base) since fee differentiation would confound the variable under study.

### 3.4 Bursty workload (Experiment 5 only)

The bursty pattern cycles through three phases, repeated 3 times per run:

| Phase    | Duration | λ (tx/s) |
| -------- | -------- | -------- |
| Low load | 30s      | 10       |
| Burst    | 15s      | 200      |
| Low load | 30s      | 10       |

Total pattern duration: 225s × 3 repetitions = total run of 225s per run (no separate warm-up needed; measure from the start of the first low-load phase).

### 3.5 Sender accounts

Pre-fund a pool of 1,000 synthetic accounts before each experiment run. Each account must have sufficient balance to complete all transactions assigned to it during the run. The workload generator assigns senders uniformly at random from the pool.

---

## 4. Metrics Collection Specification

All metrics are collected via the **Prometheus endpoint** at `localhost:3312`. The harness must scrape this endpoint at 1-second intervals throughout the measurement window.

### 4.1 Primary metrics

| Metric name                  | Unit      | Description                                                   | Source                     |
| ---------------------------- | --------- | ------------------------------------------------------------- | -------------------------- |
| `rollupx_tps_observed`       | tx/s      | Transactions finalized per second (sliding 10s window)        | Prometheus                 |
| `rollupx_latency_p50_ms`     | ms        | 50th percentile end-to-end latency (tx submit → batch sealed) | Prometheus                 |
| `rollupx_latency_p95_ms`     | ms        | 95th percentile end-to-end latency                            | Prometheus                 |
| `rollupx_latency_p99_ms`     | ms        | 99th percentile end-to-end latency                            | Prometheus                 |
| `rollupx_gas_per_tx`         | gas units | Mean gas consumed per transaction in submitted batch          | Prometheus / submitter log |
| `rollupx_da_bytes_per_batch` | bytes     | Raw bytes posted to DA layer per batch                        | Submitter log              |
| `rollupx_batch_size_actual`  | tx count  | Actual number of transactions in sealed batch                 | Sequencer log              |
| `rollupx_queue_depth`        | tx count  | Current sequencer mempool size at scrape time                 | Prometheus                 |
| `rollupx_batch_seal_rate`    | batches/s | Rate of batch sealing events                                  | Prometheus                 |

### 4.2 Cost metrics (Experiments 3 and 4)

These require the submitter to emit gas cost events. Compute at analysis time using:

```
cost_usd_per_tx = (gas_per_tx × REGULAR_GAS_PRICE_GWEI × 1e-9 × ETH_PRICE_USD)
                  + (da_bytes_per_batch / txs_per_batch) × blob_cost_factor
```

Where `blob_cost_factor` is computed from `BLOB_GAS_PRICE_GWEI` for blob mode, and `0` for offchain mode.

Report:

- `cost_usd_per_tx_execution` — gas for state transition verification only
- `cost_usd_per_tx_da` — gas for data availability posting only
- `cost_usd_per_tx_total` — sum

### 4.3 Fairness metric (Experiment 2 only)

**Jain's fairness index** over per-transaction latency within each batch:

```
J = (Σ xᵢ)² / (n × Σ xᵢ²)
```

Where `xᵢ` is the latency of transaction `i` in the batch and `n` is the batch size. J = 1.0 means all transactions experience identical latency; J → 1/n means one transaction dominates.

Also record:

- `high_fee_priority_ratio`: fraction of high-fee transactions (top 10% by fee) that are included in the next batch after arrival, vs. the global inclusion rate. Values > 1.0 indicate fee-priority bias.

### 4.4 Queue depth instrumentation

The sequencer MPSC channel fill level (number of `SealedBatch` items pending in the channel between sequencer and executor) must be emitted as a Prometheus gauge at 1-second intervals:

```
rollupx_sequencer_executor_channel_depth  (gauge, units: batches)
rollupx_mempool_tx_count                  (gauge, units: transactions)
```

This is the empirical backpressure signal. Even if backpressure is not yet fully implemented, recording this metric shows where the system saturates, which is a publishable bottleneck characterization.

### 4.5 Raw data format

Each scrape interval produces one row in a CSV file. Required columns:

```
timestamp_utc, experiment_id, run_id, config_hash,
max_batch_size, policy_type, da_mode, batch_policy,
tps_observed, latency_p50_ms, latency_p95_ms, latency_p99_ms,
gas_per_tx, da_bytes_per_batch, batch_size_actual,
queue_depth, batch_seal_rate,
[fairness_jain, high_fee_priority_ratio]  # Experiment 2 only
[cost_usd_per_tx_execution, cost_usd_per_tx_da, cost_usd_per_tx_total]  # Experiments 3+4
```

`config_hash` is the SHA-256 of the serialised configuration key-value pairs for that run, truncated to 8 hex characters. This allows unambiguous identification of configurations across runs.

---

## 5. Statistical Analysis Plan

### 5.1 Central tendency and spread

- Report **median** (not mean) for all throughput and latency metrics. Metrics are non-normally distributed and right-skewed; the mean is misleading.
- Report **IQR (interquartile range)** as the primary spread measure.
- Report **P95** separately as the tail latency indicator.
- For cost metrics, report mean ± standard deviation (cost distributions are closer to normal after log-transform).

### 5.2 Significance testing

With 3 repetitions per configuration:

| Test                                            | When to apply                                                                          |
| ----------------------------------------------- | -------------------------------------------------------------------------------------- |
| Kruskal-Wallis H-test                           | Compare ≥3 groups on a single metric (e.g., TPS across 6 batch sizes)                  |
| Dunn's post-hoc test with Bonferroni correction | Pairwise comparisons after significant Kruskal-Wallis result                           |
| Mann-Whitney U test                             | Compare exactly 2 groups (e.g., adaptive vs fixed in Experiment 5)                     |
| Two-way ANOVA                                   | Experiment 4 interaction effects (after confirming approximate normality of residuals) |

Significance threshold: α = 0.05 throughout.

### 5.3 Effect size

Report **η² (eta-squared)** for each factor tested:

- η² < 0.06: small effect
- 0.06 ≤ η² < 0.14: medium effect
- η² ≥ 0.14: large effect

For Experiment 4, report the partial η² for each main effect and the interaction term separately.

### 5.4 Plots required per experiment

| Experiment | Required plots                                                                                                                                                                                                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1          | TPS vs batch size (line + CI ribbon); P95 latency vs batch size; gas/tx vs batch size                                                                                                                                |
| 2          | Box-and-violin plots of latency per policy; fairness index bar chart; high-fee priority ratio per policy                                                                                                             |
| 3          | Gas/tx by DA mode (grouped bars with cost breakdown); DA bytes/batch by DA mode; effective USD/tx by DA mode                                                                                                         |
| 4          | TPS response surface (batch size × DA mode heatmap, one per policy); Pareto frontier (TPS vs cost vs latency, 3D or projected 2D); interaction plot (mean TPS, lines = DA mode, x-axis = batch size, panel = policy) |
| 5          | Time-series of queue depth and TPS over one burst cycle (fixed vs adaptive overlaid); P95 latency CDF comparison                                                                                                     |

All plots must be exported as both PNG (300 DPI) and PDF (vector) for publication use.

---

## 6. Experiment 1 — Batch Size Sweep

### 6.1 Purpose

Characterise the relationship between `max_batch_size` and TPS, latency, and gas cost in isolation. Identify the **optimal batch size** — defined as the smallest `max_batch_size` at which observed TPS is within 10% of the maximum observed TPS across all configurations. This value is used as the controlled batch size in Experiments 2–5.

### 6.2 Independent variable

`max_batch_size` ∈ {10, 25, 50, 100, 200, 500}

### 6.3 Controlled parameters

| Parameter             | Value            |
| --------------------- | ---------------- |
| `min_batch_size`      | 1                |
| `timeout_interval_ms` | 5000             |
| `max_gas_limit`       | default          |
| `batch_policy`        | fixed            |
| `policy_type`         | FCFS             |
| `da.mode`             | calldata         |
| `proof.backend`       | mock             |
| `REQUIRE_REAL_PROOFS` | false            |
| Workload λ            | 50 tx/s, Poisson |
| Fee distribution      | uniform          |

### 6.4 Configuration matrix

6 values × 3 repetitions = **18 runs**.

### 6.5 Execution method

Use `run_matrix.sh`. No Docker Compose restart needed between runs (DA mode is constant). Vary `max_batch_size` via environment variable override:

```bash
MAX_BATCH_SIZE=10  run_matrix.sh
MAX_BATCH_SIZE=25  run_matrix.sh
# ...
```

### 6.6 Expected findings

- TPS increases with batch size up to an inflection point, then plateaus or decreases as batching latency dominates.
- P95 latency increases monotonically with batch size (transactions must wait longer to be included in a larger batch).
- Gas per transaction decreases with batch size (fixed costs amortised across more transactions).
- The knee-point in the TPS vs latency trade-off curve is the primary result of this experiment.

### 6.7 Derived output

After analysis, define:

```
OPTIMAL_BATCH_SIZE = min{ b : TPS(b) ≥ 0.9 × max_b(TPS(b)) }
```

Record this value. It becomes the controlled `max_batch_size` in all subsequent experiments.

---

## 7. Experiment 2 — Scheduling Policy Comparison

### 7.1 Purpose

Compare the four configurable scheduling policies on throughput, latency distribution, and fairness. Characterise the trade-off between fee-maximising and fairness-maximising policies.

### 7.2 Independent variable

`policy_type` ∈ {FCFS, FeePriority, TimeBoost, FairBFT}

Note: `BlobPacking` is excluded from this experiment as it conflates scheduling with DA optimisation; it belongs in Experiment 3 or 4 if supported.

### 7.3 Controlled parameters

| Parameter             | Value                                    |
| --------------------- | ---------------------------------------- |
| `max_batch_size`      | `OPTIMAL_BATCH_SIZE` (from Experiment 1) |
| `min_batch_size`      | 1                                        |
| `timeout_interval_ms` | 5000                                     |
| `batch_policy`        | fixed                                    |
| `da.mode`             | calldata                                 |
| `proof.backend`       | mock                                     |
| `REQUIRE_REAL_PROOFS` | false                                    |
| Workload λ            | 50 tx/s, Poisson                         |
| Fee distribution      | tiered (see §3.3)                        |

The tiered fee distribution is **mandatory** for this experiment. Without fee variation, FeePriority and TimeBoost behave identically to FCFS and the experiment produces no useful signal.

### 7.4 TimeBoost parameter

`time_window_ms` must be set to a value that produces meaningful time-priority differentiation. Recommended: `time_window_ms = 1000` (transactions submitted within the same 1-second window compete on fee; across windows, earlier submission wins).

### 7.5 Configuration matrix

4 policies × 3 repetitions = **12 runs**.

### 7.6 Execution method

Use `run_matrix.sh`. No Docker Compose restart needed.

```bash
POLICY_TYPE=FCFS        run_matrix.sh
POLICY_TYPE=FeePriority run_matrix.sh
POLICY_TYPE=TimeBoost   run_matrix.sh
POLICY_TYPE=FairBFT     run_matrix.sh
```

### 7.7 Expected findings

| Policy      | Expected TPS                       | Expected P95 latency | Expected fairness |
| ----------- | ---------------------------------- | -------------------- | ----------------- |
| FCFS        | Moderate                           | Low                  | High (J ≈ 1.0)    |
| FeePriority | Similar to FCFS                    | High for low-fee tx  | Low (J < 0.7)     |
| TimeBoost   | Similar to FCFS                    | Moderate             | Moderate          |
| FairBFT     | Slightly lower (ordering overhead) | Low                  | High              |

TPS is expected to be approximately equal across all policies since the bottleneck is batch sealing throughput, not ordering. The differentiating dimension is latency distribution and fairness.

---

## 8. Experiment 3 — Data Availability Mode Comparison

### 8.1 Purpose

Quantify the cost and latency impact of the three DA modes: calldata (Mode A), EIP-4844 blobs (Mode B), and offchain (Mode C). Produce per-transaction cost breakdowns in gas units and USD.

### 8.2 Independent variable

`da.mode` ∈ {calldata, blob, offchain}

### 8.3 Controlled parameters

| Parameter                 | Value                                    |
| ------------------------- | ---------------------------------------- |
| `max_batch_size`          | `OPTIMAL_BATCH_SIZE` (from Experiment 1) |
| `policy_type`             | FCFS                                     |
| `batch_policy`            | fixed                                    |
| `proof.backend`           | mock                                     |
| `REQUIRE_REAL_PROOFS`     | false                                    |
| `proof.verification_mode` | offchainonly                             |
| `da.blob_binding`         | opcode (for blob mode)                   |
| `ETH_PRICE_USD`           | fix at a representative value, e.g. 3000 |
| `REGULAR_GAS_PRICE_GWEI`  | fix at 20                                |
| `BLOB_GAS_PRICE_GWEI`     | fix at 1                                 |
| Workload λ                | 50 tx/s, Poisson                         |

Fix gas prices as constants rather than fetching live values. This ensures comparability across runs and across time. Document the fixed values in the paper.

### 8.4 Why this experiment requires `run_infra_matrix.sh`

Switching `da.mode` between calldata, blob, and offchain requires a full Docker Compose teardown and restart because:

- The submitter daemon initialises DA-mode-specific internal state at startup
- Blob mode requires a different mock blob commitment store
- Offchain mode requires the archiver service (`da.archiver_url`) to be running

Each DA mode configuration must start from a clean Docker Compose environment.

### 8.5 Configuration matrix

3 DA modes × 3 repetitions = **9 runs**, each requiring a Docker Compose restart.

### 8.6 Batch size sub-sweep (optional but recommended)

To show cost scaling behaviour, repeat the DA mode comparison at three batch sizes: 25, `OPTIMAL_BATCH_SIZE`, and 500. This produces the key result showing that blobs are inefficient for small batches (fixed 128 KB blob size underutilised) but highly efficient at large batch sizes.

If included: 3 DA modes × 3 batch sizes × 3 repetitions = **27 runs**.

### 8.7 Expected findings

| DA mode  | Gas/tx                      | DA bytes/batch   | USD/tx  | Notes                             |
| -------- | --------------------------- | ---------------- | ------- | --------------------------------- |
| calldata | Highest                     | High             | Highest | 16 gas per non-zero byte          |
| blob     | Low at large batch sizes    | Fixed (128 KB)   | Low     | Inefficient for small batches     |
| offchain | Near zero (only commitment) | ~32 bytes (hash) | Lowest  | Security assumption: DA committee |

The crossover batch size at which blobs become cheaper than calldata is a key quantitative result. Literature suggests this is around batch size 50–100; validate against RollupX's actual encoding.

---

## 9. Experiment 4 — Cross-Factor Interaction (Novel Contribution)

### 9.1 Purpose

This is the **primary novel contribution** of the project. Perform a full-factorial experiment simultaneously varying batch size, DA mode, and scheduling policy. Produce response surfaces and a Pareto frontier mapping the TPS–cost–latency trade-off space. Test whether the factors interact (i.e., whether the effect of batch size on TPS depends on which DA mode is active).

This directly addresses the literature gap identified in §2.4 of the project proposal: _"Lack of controlled studies on how batch size, scheduling, and DA strategies interact to affect throughput, latency, and cost."_

### 9.2 Independent variables (varied jointly)

| Factor           | Levels                   |
| ---------------- | ------------------------ |
| `max_batch_size` | 25, 100, 500             |
| `da.mode`        | calldata, blob, offchain |
| `policy_type`    | FCFS, FeePriority        |

`policy_type` is limited to 2 levels (not 4) to keep the design tractable. FCFS and FeePriority are the most informative contrast: they represent the fairness-maximising and revenue-maximising extremes.

### 9.3 Design

Full factorial: 3 × 3 × 2 = **18 configurations**  
3 repetitions each = **54 runs total**

Full configuration table:

| Config ID | max_batch_size | da.mode  | policy_type |
| --------- | -------------- | -------- | ----------- |
| C01       | 25             | calldata | FCFS        |
| C02       | 25             | calldata | FeePriority |
| C03       | 25             | blob     | FCFS        |
| C04       | 25             | blob     | FeePriority |
| C05       | 25             | offchain | FCFS        |
| C06       | 25             | offchain | FeePriority |
| C07       | 100            | calldata | FCFS        |
| C08       | 100            | calldata | FeePriority |
| C09       | 100            | blob     | FCFS        |
| C10       | 100            | blob     | FeePriority |
| C11       | 100            | offchain | FCFS        |
| C12       | 100            | offchain | FeePriority |
| C13       | 500            | calldata | FCFS        |
| C14       | 500            | calldata | FeePriority |
| C15       | 500            | blob     | FCFS        |
| C16       | 500            | blob     | FeePriority |
| C17       | 500            | offchain | FCFS        |
| C18       | 500            | offchain | FeePriority |

### 9.4 Execution method

Use `run_infra_matrix.sh` for all 18 configurations (DA mode changes require Docker Compose restarts). Within each DA mode group, batch size and policy type can be varied without restart. Recommended execution order: iterate over DA mode outermost (3 restarts total per repetition), then batch size, then policy type.

```
for da_mode in calldata blob offchain:
    docker-compose down && docker-compose up -d
    for batch_size in 25 100 500:
        for policy in FCFS FeePriority:
            run trial with (da_mode, batch_size, policy)
            collect metrics
            save to results/exp4/run_{config_id}_{rep}.csv
```

### 9.5 Analysis: interaction test

Fit a two-way ANOVA model with main effects and interaction terms:

```
TPS ~ batch_size + da_mode + policy_type
      + batch_size:da_mode
      + batch_size:policy_type
      + da_mode:policy_type
```

Report:

- F-statistic and p-value for each main effect
- F-statistic and p-value for each interaction term
- Partial η² for each term

A statistically significant `batch_size:da_mode` interaction term is itself a publishable finding — it confirms that these two factors cannot be optimised independently.

### 9.6 Analysis: Pareto frontier

For each of the 18 configurations, compute the median values of:

- TPS (maximise)
- cost_usd_per_tx_total (minimise)
- latency_p95_ms (minimise)

Plot all 18 points in 3D objective space. Identify the Pareto-optimal subset (configurations not dominated on all three objectives simultaneously). The Pareto frontier plot, with each point labelled by its config ID and described in a companion table, is a key publication figure.

**Projected 2D plots** (easier to read in a paper):

- TPS vs cost (2D), with latency encoded as point size
- TPS vs latency (2D), with cost encoded as colour
- Cost vs latency (2D), with TPS encoded as point size

### 9.7 Expected findings

- `batch_size:da_mode` interaction will be statistically significant: blob mode benefits more from large batch sizes than calldata does (because blob cost is fixed regardless of batch size, but calldata cost scales with bytes).
- `batch_size:policy_type` interaction is expected to be non-significant or small: scheduling policy primarily affects latency distribution, not throughput, so batch size and policy are approximately independent.
- Pareto frontier will show 3–5 non-dominated configurations, providing concrete tuning guidance: e.g., "for minimum cost, use batch_size=500, blob mode, FCFS; for minimum latency, use batch_size=25, offchain, FCFS; for maximum TPS, use batch_size=100, offchain, FeePriority."

---

## 10. Experiment 5 — Adaptive vs Fixed Batching Under Bursty Load

### 10.1 Purpose

Evaluate whether the `adaptive` batch policy (`batch_policy=adaptive`) outperforms the `fixed` policy under a non-stationary (bursty) arrival process. Characterise recovery time after a burst and tail latency behaviour during sustained bursts.

### 10.2 Independent variable

`batch_policy` ∈ {fixed, adaptive}

### 10.3 Adaptive policy parameters

```toml
batch_policy = "adaptive"
adaptive_low_load_threshold = 15       # tx/s; below this, use small batches
adaptive_medium_load_threshold = 80    # tx/s; below this, use medium batches
adaptive_small_batch_size = 20
adaptive_medium_batch_size = 75
adaptive_large_batch_size = 250
```

The fixed policy baseline:

```toml
batch_policy = "fixed"
max_batch_size = OPTIMAL_BATCH_SIZE    # from Experiment 1
```

### 10.4 Controlled parameters

| Parameter             | Value    |
| --------------------- | -------- |
| `policy_type`         | FCFS     |
| `da.mode`             | calldata |
| `proof.backend`       | mock     |
| `REQUIRE_REAL_PROOFS` | false    |
| `timeout_interval_ms` | 2000     |

### 10.5 Workload pattern

See §3.4. The burst pattern is:

- 30s at λ = 10 tx/s (low load)
- 15s at λ = 200 tx/s (burst — 4× the steady-state rate used in other experiments)
- 30s at λ = 10 tx/s (recovery)

Repeat 3 times per run. Total run duration: 225 seconds.

### 10.6 Configuration matrix

2 policies × 3 repetitions = **6 runs**.

### 10.7 Execution method

Use `run_matrix.sh`. No Docker Compose restart required.

### 10.8 Key derived metrics

- **Burst P95 latency:** P95 latency computed only over transactions arriving during the 15-second burst window.
- **Recovery time:** Time from end of burst until queue depth returns to ≤ 5 transactions. Lower is better.
- **Low-load gas/tx:** Mean gas per transaction computed only over the low-load windows. Adaptive policy should produce smaller batches here, potentially increasing gas/tx vs fixed.
- **Queue depth time-series:** Export 1-second queue depth samples aligned to the burst pattern phases for the time-series plot.

### 10.9 Expected findings

- Adaptive policy reduces burst P95 latency by sealing smaller, faster batches during burst (small batch fills quickly → faster inclusion).
- Adaptive policy incurs slightly higher gas/tx during low load (small batches = less amortisation), but this is the intended trade-off.
- Fixed policy shows a larger queue depth spike during burst and longer recovery time.

---

## 11. Experiment Sequencing and Dependencies

Experiments must be run in the following order due to dependencies:

```
Experiment 1 (Batch Size Sweep)
    │
    ▼  Produces: OPTIMAL_BATCH_SIZE
    │
    ├──► Experiment 2 (Scheduling) — uses OPTIMAL_BATCH_SIZE
    │
    ├──► Experiment 3 (DA Mode) — uses OPTIMAL_BATCH_SIZE
    │
    ├──► Experiment 4 (Cross-Factor) — uses batch sizes {25, 100, 500}
    │    (independent of OPTIMAL_BATCH_SIZE; uses fixed levels)
    │
    └──► Experiment 5 (Adaptive Batching) — uses OPTIMAL_BATCH_SIZE for fixed baseline
```

Experiments 2, 3, and 5 can proceed in parallel after Experiment 1 completes.  
Experiment 4 can begin concurrently with Experiments 2 and 3 since its batch size levels are fixed (not derived from Experiment 1).

---

## 12. Harness Implementation Notes

### 12.1 `experiments.toml` structure

Each experiment should be representable as a section in `experiments.toml`. The harness reads this file and generates the run matrix. Example structure:

```toml
[global]
warmup_seconds = 30
measurement_seconds = 300
repetitions = 3
workload_lambda = 50
workload_seed = 42
prover_backend = "mock"
require_real_proofs = false
allow_unsigned_txs = true

[experiment.1_batch_size_sweep]
variable = "max_batch_size"
levels = [10, 25, 50, 100, 200, 500]
controlled.policy_type = "FCFS"
controlled.da_mode = "calldata"
controlled.batch_policy = "fixed"

[experiment.2_scheduling_policy]
variable = "policy_type"
levels = ["FCFS", "FeePriority", "TimeBoost", "FairBFT"]
controlled.max_batch_size = "{{OPTIMAL_BATCH_SIZE}}"  # resolved after Exp 1
controlled.da_mode = "calldata"
controlled.batch_policy = "fixed"
workload.fee_distribution = "tiered"

[experiment.3_da_mode]
variable = "da_mode"
levels = ["calldata", "blob", "offchain"]
controlled.max_batch_size = "{{OPTIMAL_BATCH_SIZE}}"
controlled.policy_type = "FCFS"
controlled.batch_policy = "fixed"
requires_infra_restart = true

[experiment.4_cross_factor]
variables = ["max_batch_size", "da_mode", "policy_type"]
levels.max_batch_size = [25, 100, 500]
levels.da_mode = ["calldata", "blob", "offchain"]
levels.policy_type = ["FCFS", "FeePriority"]
requires_infra_restart = true

[experiment.5_adaptive_batching]
variable = "batch_policy"
levels = ["fixed", "adaptive"]
controlled.max_batch_size = "{{OPTIMAL_BATCH_SIZE}}"
controlled.policy_type = "FCFS"
controlled.da_mode = "calldata"
workload.pattern = "bursty"
```

### 12.2 Environment variable override convention

The harness sets parameters via environment variables following the convention already established in RollupX. Key mappings:

| Config key            | Environment variable  |
| --------------------- | --------------------- |
| `max_batch_size`      | `MAX_BATCH_SIZE`      |
| `policy_type`         | `POLICY_TYPE`         |
| `da.mode`             | `DA_MODE`             |
| `batch_policy`        | `BATCH_POLICY`        |
| `REQUIRE_REAL_PROOFS` | `REQUIRE_REAL_PROOFS` |
| `PROVER_BACKEND`      | `PROVER_BACKEND`      |

### 12.3 Clean state between runs

For all experiments, each run must start from clean state:

1. Stop sequencer and executor processes.
2. Clear the sequencer transaction pool.
3. Reset the executor's state tree (RocksDB) to the genesis state.
4. Clear Prometheus metrics (or account for the reset in analysis by using deltas, not absolute counters).
5. For DA mode changes: full `docker-compose down -v && docker-compose up -d` (the `-v` flag removes volumes including the RocksDB state).

### 12.4 Prometheus scraping

The harness must run a sidecar scraper that:

1. Polls `http://localhost:3312/metrics` every 1 second.
2. Parses the Prometheus text format.
3. Appends a row to the current run's CSV file (see §4.5).
4. Tags each row with `experiment_id`, `run_id`, and `config_hash`.

Discard rows collected during the first 30 seconds (warm-up). Begin recording at `t = 30s` relative to process start.

### 12.5 Detecting run failure

A run has failed and must be discarded if any of the following occur:

- Observed TPS drops to zero for more than 10 consecutive seconds during the measurement window (process crash or deadlock).
- The sequencer exits with a non-zero status code.
- The `rollupx_sequencer_executor_channel_depth` gauge exceeds 50 for more than 30 consecutive seconds (persistent backpressure saturation, not a burst — the system is overloaded and not measuring the variable of interest).
- Fewer than 1,000 transactions are processed during the measurement window (insufficient statistical sample).

Failed runs must be logged and replaced with an additional repetition.

---

## 13. Output Artefacts and File Structure

```
results/
├── exp1_batch_size_sweep/
│   ├── run_bs010_rep1.csv
│   ├── run_bs010_rep2.csv
│   ├── run_bs010_rep3.csv
│   ├── run_bs025_rep1.csv
│   │   ...
│   ├── summary_exp1.csv          # one row per config; median/IQR/P95 aggregated
│   └── optimal_batch_size.txt    # single integer, consumed by later experiments
│
├── exp2_scheduling_policy/
│   ├── run_fcfs_rep1.csv
│   │   ...
│   └── summary_exp2.csv
│
├── exp3_da_mode/
│   ├── run_calldata_rep1.csv
│   │   ...
│   └── summary_exp3.csv
│
├── exp4_cross_factor/
│   ├── run_C01_rep1.csv
│   │   ...
│   ├── summary_exp4.csv
│   └── pareto_frontier.csv       # config_id, tps_median, cost_median, latency_p95_median, is_pareto_optimal
│
├── exp5_adaptive_batching/
│   ├── run_fixed_rep1.csv
│   │   ...
│   └── summary_exp5.csv
│
└── environment.json               # CPU, RAM, OS, Rust version, git commit, ETH price, gas prices used
```

### 13.1 Summary CSV schema

Each `summary_expN.csv` has one row per configuration:

```
config_hash, [factor columns], tps_median, tps_iqr, tps_p95,
latency_p50_median, latency_p95_median, latency_p95_iqr,
gas_per_tx_median, da_bytes_per_batch_median,
[cost_usd_per_tx_median],  # Experiments 3+4
[fairness_jain_median],    # Experiment 2
repetitions_completed, repetitions_failed
```

### 13.2 Plots

Each experiment directory should also contain a `plots/` subdirectory with all required figures (see §5.4), exported as both `.png` (300 DPI) and `.pdf`.

---

## Appendix A — Parameter Reference

The following table lists every RollupX configurable parameter, its role in the benchmarking suite, and whether it is varied, controlled, or irrelevant in each experiment.

| Parameter                        | Exp 1               | Exp 2               | Exp 3             | Exp 4             | Exp 5               | Notes                                   |
| -------------------------------- | ------------------- | ------------------- | ----------------- | ----------------- | ------------------- | --------------------------------------- |
| `max_batch_size`                 | **VARY**            | controlled          | controlled        | **VARY**          | controlled          | Core throughput lever                   |
| `timeout_interval_ms`            | controlled=5000     | controlled=5000     | controlled=5000   | controlled=5000   | controlled=2000     | Lower in Exp 5 for burst responsiveness |
| `min_batch_size`                 | controlled=1        | controlled=1        | controlled=1      | controlled=1      | controlled=1        |                                         |
| `max_gas_limit`                  | default             | default             | default           | default           | default             | Not a study variable                    |
| `batch_policy`                   | controlled=fixed    | controlled=fixed    | controlled=fixed  | controlled=fixed  | **VARY**            |                                         |
| `adaptive_low_load_threshold`    | —                   | —                   | —                 | —                 | controlled=15       | Exp 5 only                              |
| `adaptive_medium_load_threshold` | —                   | —                   | —                 | —                 | controlled=80       | Exp 5 only                              |
| `adaptive_small_batch_size`      | —                   | —                   | —                 | —                 | controlled=20       | Exp 5 only                              |
| `adaptive_medium_batch_size`     | —                   | —                   | —                 | —                 | controlled=75       | Exp 5 only                              |
| `adaptive_large_batch_size`      | —                   | —                   | —                 | —                 | controlled=250      | Exp 5 only                              |
| `blob_target_bytes`              | —                   | —                   | controlled        | controlled        | —                   | Blob mode only                          |
| `blob_fill_target`               | —                   | —                   | controlled        | controlled        | —                   | Blob mode only                          |
| `policy_type`                    | controlled=FCFS     | **VARY**            | controlled=FCFS   | **VARY**          | controlled=FCFS     |                                         |
| `time_window_ms`                 | —                   | controlled=1000     | —                 | —                 | —                   | TimeBoost only                          |
| `PROVER_BACKEND`                 | mock                | mock                | mock              | mock              | mock                | Fixed globally                          |
| `REQUIRE_REAL_PROOFS`            | false               | false               | false             | false             | false               | Fixed globally                          |
| `ALLOW_PROOF_FALLBACK`           | true                | true                | true              | true              | true                | Fixed globally                          |
| `da.mode`                        | controlled=calldata | controlled=calldata | **VARY**          | **VARY**          | controlled=calldata |                                         |
| `da.blob_binding`                | —                   | —                   | controlled=opcode | controlled=opcode | —                   | Blob mode only                          |
| `proof.backend`                  | mock                | mock                | mock              | mock              | mock                | Fixed globally                          |
| `proof.verification_mode`        | offchainonly        | offchainonly        | offchainonly      | offchainonly      | offchainonly        |                                         |
| `ETH_PRICE_USD`                  | —                   | —                   | controlled=3000   | controlled=3000   | —                   | Cost experiments only                   |
| `REGULAR_GAS_PRICE_GWEI`         | —                   | —                   | controlled=20     | controlled=20     | —                   |                                         |
| `BLOB_GAS_PRICE_GWEI`            | —                   | —                   | controlled=1      | controlled=1      | —                   |                                         |
| `ALLOW_UNSIGNED_USER_TXS`        | true                | true                | true              | true              | true                | Fixed globally                          |
| `HARDHAT_MINING_INTERVAL`        | 2000                | 2000                | 2000              | 2000              | 2000                |                                         |

---

## Appendix B — Metrics to Literature Mapping

This table maps each metric collected in this suite to the closest precedent in the cited literature, enabling direct comparison in the results section.

| Metric                                | Literature precedent                                              | Expected RollupX value                          |
| ------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------- |
| TPS (token transfers)                 | Gogol et al. 2025: >14,000 TPS theoretical; 71 TPS for DeFi swaps | 100–2,000 TPS (NoProofs mode; simpler tx types) |
| P50 confirmation latency              | Chaliasos et al. 2024: zkSync Era <2.5s soft finality for >50% tx | Target <1s in NoProofs mode                     |
| DA cost reduction (blobs vs calldata) | Gogol et al. 2025: EIP-4844 blobs cut DA costs ~10–100×           | Quantify crossover batch size                   |
| DA cost as % of total cost            | Chaliasos et al. 2024: DA was ~80% of costs pre-EIP-4844          | Validate post-EIP-4844 shift                    |
| Merkle tree computation overhead      | Gogol et al. 2025: up to 2.44s per batch                          | Measure per batch in NoProofs mode to isolate   |
| Scheduling fairness                   | Motepalli et al. 2023: FCFS vs fee-priority trade-offs            | Quantify with Jain's index                      |

---

_Document version: 1.0 — generated for RollupX benchmarking implementation._  
_Corresponds to project proposal submitted 2025-09-24, CS4203, University of Moratuwa._
