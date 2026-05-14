# RollupX Benchmarking Plan

**Project:** RollupX — Configurable ZK-Rollup Prototype  
**Goal:** Produce high-impact empirical results by showing how batching, sequencing policy, data availability mode, proof configuration, and L1 submission behavior affect throughput, latency, cost, reliability, and fairness.

---

## 1. Benchmarking Purpose

The purpose of the benchmark is not only to show that the rollup works. The benchmark should produce clear research claims such as:

1. **Dynamic batching improves the throughput-latency-cost frontier** compared with a fixed batch configuration.
2. **Blob-aware packing reduces DA cost per transaction** when batches are large enough to use blob capacity efficiently.
3. **The bottleneck shifts depending on workload intensity**: at low load the bottleneck is batching delay, at medium load it is proof/execution delay, and at high load it becomes prover capacity, DA posting, or L1 submission.
4. **Different sequencer policies optimize different goals**: FCFS is simple and fair, FeePriority reduces high-fee transaction latency, TimeBoost improves paid priority latency, FairBFT improves fairness, and BlobPacking improves DA efficiency.
5. **Real-proof mode changes the scalability story** because proof generation time and memory usage become visible bottlenecks instead of hidden assumptions.
6. **The best configuration is workload-dependent**, so the final output should be a configuration recommendation matrix rather than one single “best” value.

The benchmark should therefore measure the full rollup pipeline:

```text
User submission
    → Sequencer validation/admission
    → Mempool waiting
    → Batch formation
    → Executor state transition
    → Proof generation or proof fallback
    → Submitter packaging
    → DA publication
    → L1 transaction inclusion
    → Finalized rollup batch
```

---

## 2. Research Questions to Answer

The benchmark should answer these concrete questions.

### RQ1 — Batching

How do `max_batch_size`, `min_batch_size`, and `timeout_interval_ms` affect throughput, latency, proof overhead, and cost per transaction?

This is the main throughput-latency trade-off experiment. Larger batches usually improve amortized cost and throughput, but they can increase waiting time because transactions wait longer before a batch is sealed.

### RQ2 — Adaptive Batching

Does `batch_policy = adaptive` outperform `batch_policy = fixed` under variable load?

This is one of the highest-impact experiments because it can show that your rollup is not only configurable, but also self-tuning. The key is to test it under low, medium, high, and bursty traffic.

### RQ3 — Sequencer Ordering Policy

How do `policy_type = FCFS / FeePriority / TimeBoost / FairBFT / BlobPacking` affect throughput, latency, fairness, revenue/cost, and starvation?

This experiment should not only compare TPS. It should measure whether some users or transaction classes are delayed unfairly.

### RQ4 — Data Availability Mode

How do `da.mode = calldata / blob / offchain`, `blob_target_bytes`, and `blob_fill_target` affect gas cost, DA bytes, blob utilization, and finality latency?

This is the main cost-efficiency experiment. It should clearly show when calldata is better, when blobs are better, and when offchain mode is cheaper but weaker from a trust/security perspective.

### RQ5 — Proving Bottleneck

How do `PROVER_BACKEND`, `REQUIRE_REAL_PROOFS`, `ALLOW_PROOF_FALLBACK`, and `proof.backend` affect proof time, verification cost, peak memory, CPU usage, and batch finality?

This experiment separates “system throughput” from “cryptographically proven throughput.” That distinction is important because a rollup can appear fast if proof generation is skipped or mocked.

### RQ6 — L1 Submission and Reliability

How do `SEQUENCER_EXECUTOR_PUBLISH_RETRIES`, `SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS`, `COMM_MODE`, and `HARDHAT_MINING_INTERVAL` affect failed batches, retry rate, finality latency, and system stability?

This experiment shows whether your prototype is robust under executor delays, RPC delay, and L1 mining interval changes.

---

## 3. Baseline Configuration

Before changing any parameter, define one baseline. Every experiment should compare against this baseline.

Recommended baseline:

```toml
[sequencer]
max_batch_size = 100
min_batch_size = 10
timeout_interval_ms = 2000
max_gas_limit = 30000000
batch_policy = "fixed"
policy_type = "FCFS"
time_window_ms = 1000
blob_target_bytes = 120000
blob_fill_target = 0.80
SEQUENCER_EXECUTOR_PUBLISH_RETRIES = 3
SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS = 5000

[executor]
PROVER_BACKEND = "risc0"
REQUIRE_REAL_PROOFS = true
ALLOW_PROOF_FALLBACK = true
ALLOW_UNSIGNED_USER_TXS = false

[submitter]
da.mode = "calldata"
da.blob_binding = "opcode"
da.blob_index = 0
proof.backend = "groth16"
proof.verification_mode = "onchain"
ETH_PRICE_USD = 3000
REGULAR_GAS_PRICE_GWEI = 10
BLOB_GAS_PRICE_GWEI = 1
COMM_MODE = "grpc"
HARDHAT_MINING_INTERVAL = 12000
```

The exact values can be adjusted to match your implementation, but the baseline must be fixed and documented. Do not keep changing the baseline while analyzing results.

---

## 4. Workload Design

Benchmark results are only meaningful if the workload is controlled. Each experiment should run under multiple workload profiles.

### 4.1 Transaction Types

Use at least the following transaction categories.

| Transaction Type         | Description                                                                   | Expected Cost | Why It Matters                                    |
| ------------------------ | ----------------------------------------------------------------------------- | ------------: | ------------------------------------------------- |
| Light transfer           | Simple balance transfer between existing accounts                             |           Low | Measures best-case rollup throughput              |
| Medium transfer          | Transfer with nonce check, balance check, state update, and moderate DA bytes |        Medium | Represents normal rollup activity                 |
| Heavy transfer           | Larger payload, more state reads/writes, or more expensive proof trace        |          High | Exposes executor/prover bottlenecks               |
| Deposit                  | L1-originated user deposit event processed by rollup                          |        Medium | Tests bridge/listener path                        |
| Withdrawal / forced exit | Exit request that must eventually be included                                 |   Medium/High | Tests censorship-resistance and priority handling |

If the current prototype only supports transfers, simulate transaction complexity by varying payload size, number of touched accounts, state-write count, or proof trace size.

### 4.2 Traffic Profiles

Each major experiment should be run under these traffic profiles.

| Profile     | Arrival Pattern                     |            Suggested Rate | Purpose                                        |
| ----------- | ----------------------------------- | ------------------------: | ---------------------------------------------- |
| Low load    | Poisson arrivals                    |                  5–10 TPS | Measures timeout behavior and minimum latency  |
| Medium load | Poisson arrivals                    |                 25–50 TPS | Measures normal operating region               |
| High load   | Poisson arrivals                    |               100–200 TPS | Measures saturation behavior                   |
| Burst load  | Alternating quiet and spike periods |   10 TPS → 200 TPS spikes | Tests adaptive batching and queue recovery     |
| Overload    | Rate above system capacity          | 300+ TPS or until failure | Finds breaking point and backpressure behavior |

Do not only test constant traffic. A real sequencer receives bursty traffic, so the adaptive batching experiment must include burst load.

### 4.3 Workload Mixes

Use these workload mixes.

| Mix           | Light | Medium |                    Heavy | Deposit/Withdraw | Purpose                               |
| ------------- | ----: | -----: | -----------------------: | ---------------: | ------------------------------------- |
| Transfer-only |  100% |     0% |                       0% |               0% | Maximum theoretical throughput        |
| Normal        |   60% |    30% |                       5% |               5% | Main benchmark workload               |
| Heavy-state   |   20% |    40% |                      35% |               5% | Executor/prover stress                |
| Bridge-heavy  |   40% |    30% |                      10% |              20% | L1 bridge and forced-operation stress |
| DA-heavy      |   30% |    30% | 40% with larger payloads |               0% | DA cost and blob packing stress       |

The **Normal** mix should be the default for final result comparisons.

---

## 5. Metrics to Measure

The benchmark must collect metrics at transaction level, batch level, component level, and system level.

---

### 5.1 Throughput Metrics

| Metric              | Formula / Meaning                                        | Level      |
| ------------------- | -------------------------------------------------------- | ---------- |
| Submitted TPS       | Number of submitted transactions per second              | System     |
| Accepted TPS        | Transactions accepted by sequencer per second            | Sequencer  |
| Batched TPS         | Transactions included in sealed batches per second       | Sequencer  |
| Executed TPS        | Transactions executed by executor per second             | Executor   |
| Proven TPS          | Transactions covered by generated proofs per second      | Prover     |
| Finalized TPS       | Transactions included in L1-finalized batches per second | End-to-end |
| Goodput             | Successfully finalized valid transactions per second     | End-to-end |
| Rejection rate      | Invalid/rejected transactions ÷ submitted transactions   | Sequencer  |
| Backlog growth rate | Change in mempool size over time                         | Sequencer  |

The most important throughput number is **goodput**, not submitted TPS. Submitted TPS can be artificially high even if the rollup cannot process or finalize the transactions.

---

### 5.2 Latency Metrics

Measure latency as percentiles, not only averages. Always report P50, P90, P95, and P99.

| Metric                    | Definition                                                      |
| ------------------------- | --------------------------------------------------------------- |
| Admission latency         | Time from user submission to sequencer acceptance/rejection     |
| Queue waiting latency     | Time from sequencer acceptance to batch inclusion               |
| Batch sealing latency     | Time from first transaction in batch to batch sealed            |
| Execution latency         | Time taken by executor to apply state transition                |
| Proof latency             | Time taken to generate proof for the batch                      |
| Submitter latency         | Time from proof/batch availability to L1 transaction submission |
| L1 inclusion latency      | Time from L1 submission to L1 inclusion                         |
| Soft confirmation latency | Time from user submission to sequencer soft confirmation        |
| Hard finality latency     | Time from user submission to L1 batch finalization              |
| End-to-end latency        | Full time from user submission to finalized batch               |

For impact, plot **throughput vs P95 latency**. Average latency can hide bad tail latency.

---

### 5.3 Cost Metrics

| Metric                                 | Definition                                          |
| -------------------------------------- | --------------------------------------------------- |
| Gas per batch                          | Total L1 gas used by one submitted batch            |
| Gas per transaction                    | Batch gas ÷ transaction count                       |
| DA gas per transaction                 | DA-related gas ÷ transaction count                  |
| Proof verification gas per transaction | Verifier gas ÷ transaction count                    |
| Calldata bytes per batch               | Number of calldata bytes submitted                  |
| Calldata bytes per transaction         | Calldata bytes ÷ transaction count                  |
| Blob bytes per batch                   | Number of blob bytes used                           |
| Blob bytes per transaction             | Blob bytes ÷ transaction count                      |
| Blob fill ratio                        | Used blob bytes ÷ available blob bytes              |
| Blob waste ratio                       | Unused blob bytes ÷ available blob bytes            |
| L1 cost per transaction                | Gas and blob cost converted to ETH or USD           |
| Cost reduction vs baseline             | `(baseline_cost - experiment_cost) / baseline_cost` |

Use both ETH and USD cost if `ETH_PRICE_USD`, `REGULAR_GAS_PRICE_GWEI`, and `BLOB_GAS_PRICE_GWEI` are configurable.

---

### 5.4 Prover and Executor Metrics

| Metric                         | Definition                                 |
| ------------------------------ | ------------------------------------------ |
| Execution time per batch       | Time spent in state transition engine      |
| Execution time per transaction | Execution time ÷ transaction count         |
| Proof generation time          | Wall-clock time for proof generation       |
| Proof size                     | Size of generated proof in bytes           |
| Journal size                   | Size of public output/journal              |
| Peak memory usage              | Maximum RAM used during proof generation   |
| Average CPU usage              | CPU utilization during execution/proving   |
| Prover failure count           | Number of failed proof generations         |
| Proof fallback count           | Number of times fallback mode was used     |
| State root mismatch count      | Number of invalid state transition outputs |

If `REQUIRE_REAL_PROOFS = false`, clearly label the run as **mock/fallback proof mode**. Do not compare it directly against real-proof performance without separating the two.

---

### 5.5 Sequencer Policy and Fairness Metrics

These are important for showing that sequencing policy affects more than TPS.

| Metric                       | Definition                                                           |
| ---------------------------- | -------------------------------------------------------------------- |
| Per-class latency            | Latency grouped by fee level, user class, or transaction type        |
| Starvation count             | Number of valid transactions waiting longer than a threshold         |
| Max wait time                | Longest time any valid transaction waited in the mempool             |
| Reordering distance          | Difference between arrival order and final batch order               |
| High-fee latency improvement | Latency reduction for high-fee transactions under FeePriority        |
| Low-fee penalty              | Latency increase for low-fee transactions under FeePriority          |
| Fairness score               | Jain's fairness index over per-user inclusion rates or waiting times |
| Forced transaction delay     | Time taken to include forced L1-originated operations                |

Recommended fairness formula:

```text
Jain fairness = (sum(x_i)^2) / (n * sum(x_i^2))
```

Where `x_i` can be per-user inclusion rate or inverse latency. A value close to 1 means more fair behavior.

---

### 5.6 Reliability Metrics

| Metric                         | Definition                                    |
| ------------------------------ | --------------------------------------------- |
| Batch publish success rate     | Successful publishes ÷ attempted publishes    |
| Executor publish timeout count | Number of executor publish timeouts           |
| Retry count per batch          | Number of retries before success/failure      |
| Failed batch count             | Number of batches that failed permanently     |
| Duplicate publish count        | Number of duplicated batch submissions        |
| RPC error count                | Number of L1 RPC/submitter errors             |
| Recovery time                  | Time taken to recover after executor/L1 delay |

These metrics make your benchmark stronger because they show operational robustness, not only ideal-case performance.

---

## 6. Parameters to Change and Why

Not every parameter should be swept equally. Some are core experimental variables, some are controls, and some are environment constants.

---

### 6.1 Sequencer Parameters

| Parameter                               | Change?                   | Suggested Values                                   | Main Metrics                          | Purpose                           |
| --------------------------------------- | ------------------------- | -------------------------------------------------- | ------------------------------------- | --------------------------------- |
| `max_batch_size`                        | Yes                       | 25, 50, 100, 200, 500, 1000                        | TPS, latency, gas/tx, proof time      | Main batch-size trade-off         |
| `timeout_interval_ms`                   | Yes                       | 250, 500, 1000, 2000, 5000, 10000                  | P95 latency, batch fill, TPS          | Batch frequency trade-off         |
| `min_batch_size`                        | Yes                       | 1, 10, 25, 50, 100                                 | low-load latency, empty/waste batches | Prevent inefficient tiny batches  |
| `max_gas_limit`                         | Limited                   | 10M, 20M, 30M or local chain limit                 | failed batches, gas/batch             | Find max submit capacity          |
| `batch_policy`                          | Yes                       | fixed, adaptive                                    | TPS, P95 latency, cost/tx             | Main adaptive batching comparison |
| `adaptive_low_load_threshold`           | Yes                       | 10, 25, 50 TPS                                     | adaptive decisions, latency           | Define low-load behavior          |
| `adaptive_medium_load_threshold`        | Yes                       | 50, 100, 150 TPS                                   | adaptive decisions, TPS               | Define medium/high transition     |
| `adaptive_small_batch_size`             | Yes                       | 25, 50                                             | low-load latency                      | Fast confirmation at low load     |
| `adaptive_medium_batch_size`            | Yes                       | 100, 200                                           | normal-load performance               | Normal operating batch size       |
| `adaptive_large_batch_size`             | Yes                       | 500, 1000                                          | high-load TPS, cost/tx                | High-load amortization            |
| `blob_target_bytes`                     | Yes                       | 32KB, 64KB, 96KB, 120KB                            | blob fill ratio, cost/tx              | Blob packing efficiency           |
| `blob_fill_target`                      | Yes                       | 0.50, 0.70, 0.80, 0.90, 0.95                       | latency, blob waste, cost             | DA efficiency vs waiting          |
| `policy_type`                           | Yes                       | FCFS, FeePriority, TimeBoost, FairBFT, BlobPacking | fairness, latency, DA cost            | Sequencing policy comparison      |
| `time_window_ms`                        | Yes for TimeBoost/FairBFT | 100, 250, 500, 1000, 2000                          | fairness, P95 latency                 | Ordering window sensitivity       |
| `SEQUENCER_EXECUTOR_PUBLISH_RETRIES`    | Yes for reliability       | 0, 1, 3, 5                                         | success rate, retry latency           | Robustness under failures         |
| `SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS` | Yes for reliability       | 1000, 3000, 5000, 10000                            | timeout count, latency                | Executor communication stability  |

---

### 6.2 Executor Parameters

| Parameter                 | Change?                         | Suggested Values                             | Main Metrics                 | Purpose                                  |
| ------------------------- | ------------------------------- | -------------------------------------------- | ---------------------------- | ---------------------------------------- |
| `PROVER_BACKEND`          | Yes if multiple are implemented | risc0, mock, native, other available backend | proof time, memory           | Compare proving backend behavior         |
| `REQUIRE_REAL_PROOFS`     | Yes                             | false, true                                  | proven TPS, finality latency | Separate mock vs real proof mode         |
| `ALLOW_PROOF_FALLBACK`    | Yes                             | true, false                                  | fallback count, failure rate | Reliability vs correctness strictness    |
| `RISC0_HOST_BIN`          | No                              | fixed path                                   | reproducibility              | Keep constant                            |
| `RISC0_GUEST_ELF`         | No                              | fixed ELF                                    | reproducibility              | Keep constant                            |
| `RISC0_WORK_DIR`          | No                              | fixed work dir                               | reproducibility              | Keep constant                            |
| `RISC0_GUEST_METHOD_ID`   | No                              | fixed method ID                              | correctness                  | Keep constant                            |
| `EXECUTOR_GRPC_ADDR`      | No, unless testing deployment   | fixed                                        | RPC latency                  | Keep constant for fair comparison        |
| `ALLOW_UNSIGNED_USER_TXS` | Mostly no                       | false for final benchmark                    | validation latency, security | Use true only for synthetic stress tests |

Important recommendation: run the final benchmark with `ALLOW_UNSIGNED_USER_TXS = false`. If you use unsigned transactions for stress testing, label those runs as synthetic-only because signature verification affects sequencer admission cost.

---

### 6.3 Submitter Parameters

| Parameter                 | Change?                          | Suggested Values                | Main Metrics                        | Purpose                           |
| ------------------------- | -------------------------------- | ------------------------------- | ----------------------------------- | --------------------------------- |
| `da.mode`                 | Yes                              | calldata, blob, offchain        | cost/tx, DA bytes, finality latency | Main DA comparison                |
| `da.blob_binding`         | No unless alternatives exist     | opcode                          | correctness, compatibility          | Keep fixed                        |
| `da.blob_index`           | Limited                          | 0, 1 if multiple blobs          | blob success/failure                | Only test if multi-blob supported |
| `da.archiver_url`         | Yes for offchain reliability     | local archiver, remote archiver | availability latency, failure rate  | Offchain DA robustness            |
| `proof.backend`           | Yes if implemented               | groth16, plonky2, halo2         | proof time, verify gas              | Proof backend comparison          |
| `proof.verification_mode` | Yes                              | onchain, offchainonly           | gas/tx, security level, latency     | Cost vs security trade-off        |
| `proof.verifier_id`       | No                               | fixed                           | correctness                         | Keep fixed                        |
| `ETH_PRICE_USD`           | Sensitivity only                 | 1500, 3000, 5000                | USD cost/tx                         | Economic sensitivity              |
| `REGULAR_GAS_PRICE_GWEI`  | Sensitivity only                 | 5, 10, 30, 100                  | calldata cost/tx                    | Congestion sensitivity            |
| `BLOB_GAS_PRICE_GWEI`     | Sensitivity only                 | 0.1, 1, 5, 20                   | blob cost/tx                        | Blob market sensitivity           |
| `SUBMITTER_PRIVATE_KEY`   | No                               | fixed test key                  | none                                | Do not vary                       |
| `COMM_MODE`               | Yes if supported                 | grpc, http, ipc                 | submitter latency, failures         | Communication overhead            |
| `EXECUTOR_URL`            | No, unless deployment comparison | fixed                           | RPC latency                         | Keep fixed                        |
| `HARDHAT_MINING_INTERVAL` | Yes in local tests               | 1000, 3000, 12000               | L1 inclusion latency                | Simulate block-time effects       |

---

### 6.4 L1 Bridge Parameters

| Parameter           | Change? | Suggested Values           | Main Metrics               | Purpose                            |
| ------------------- | ------- | -------------------------- | -------------------------- | ---------------------------------- |
| `network.rpc_url`   | Limited | local Hardhat, Sepolia RPC | L1 latency, failure rate   | Local vs public testnet comparison |
| `network.chain_id`  | No      | fixed per network          | correctness                | Environment constant               |
| `contracts.bridge`  | No      | deployed bridge address    | correctness                | Environment constant               |
| `l1.rpc_url`        | Limited | local, Sepolia             | L1 inclusion latency       | Environment comparison             |
| `l1.bridge_address` | No      | fixed deployed address     | correctness                | Environment constant               |
| `l1.start_block`    | No      | deployment block           | event listener correctness | Keep fixed per deployment          |

The bridge parameters should not be treated as performance knobs except when comparing **local deterministic environment vs public testnet environment**.

---

## 7. Experimental Plan

The benchmark should be run in stages. Each stage answers one research question and produces one or more graphs/tables for the final report.

---

## Stage 0 — Instrumentation Validation

### Purpose

Before running real experiments, verify that all metrics are correctly recorded.

### Configuration

Use the baseline configuration.

### Workload

- 1-minute run
- 5 TPS
- transfer-only workload
- mock/fallback proof mode allowed

### What to Check

- Every transaction has timestamps for submission, admission, batch inclusion, execution, proof, submission, and finalization.
- Every batch has batch ID, transaction count, batch size in bytes, DA mode, proof mode, gas used, and status.
- Cost calculation is correct.
- Failed transactions are logged with reasons.
- Batch registry and L1 submitted events match.

### Output

A small validation report showing that the benchmark harness is reliable.

---

## Stage 1 — Fixed Batch Size and Timeout Sweep

### Purpose

Find how fixed batching affects throughput, latency, cost, and proof time.

### Parameters to Change

```toml
batch_policy = "fixed"
policy_type = "FCFS"
max_batch_size = [25, 50, 100, 200, 500, 1000]
timeout_interval_ms = [500, 1000, 2000, 5000, 10000]
min_batch_size = [1 or 10 fixed initially]
da.mode = "calldata"
REQUIRE_REAL_PROOFS = false initially, true in final subset
```

Do not run the full `6 × 5` matrix for every workload at first. Start with the Normal workload under medium load. Then choose the best candidates for low, high, and burst load.

### Workloads

- Normal mix at 25 TPS, 50 TPS, 100 TPS
- Transfer-only mix at 100 TPS
- Heavy-state mix at 50 TPS

### Metrics

- Goodput TPS
- P50/P95/P99 end-to-end latency
- Queue waiting latency
- Batch fill ratio
- Gas per transaction
- Proof time per batch
- Mempool backlog

### Expected Graphs

1. `max_batch_size` vs goodput TPS
2. `max_batch_size` vs P95 latency
3. `max_batch_size` vs gas/tx
4. `timeout_interval_ms` vs P95 latency
5. Throughput-latency Pareto frontier

### Impact Claim

This stage should produce a claim such as:

> Increasing fixed batch size improves gas amortization and throughput up to a point, but after the saturation point, P95 latency increases sharply and goodput no longer improves.

---

## Stage 2 — Adaptive Batching Experiment

### Purpose

Show whether adaptive batching improves performance under changing traffic.

### Parameters to Change

```toml
batch_policy = ["fixed", "adaptive"]
adaptive_low_load_threshold = [10, 25, 50]
adaptive_medium_load_threshold = [50, 100, 150]
adaptive_small_batch_size = [25, 50]
adaptive_medium_batch_size = [100, 200]
adaptive_large_batch_size = [500, 1000]
timeout_interval_ms = [1000, 2000, 5000]
```

Recommended adaptive configuration for final comparison:

```toml
batch_policy = "adaptive"
adaptive_low_load_threshold = 25
adaptive_medium_load_threshold = 100
adaptive_small_batch_size = 50
adaptive_medium_batch_size = 200
adaptive_large_batch_size = 500
timeout_interval_ms = 2000
```

### Workloads

- Low load: 10 TPS
- Medium load: 50 TPS
- High load: 150 TPS
- Burst load: 10 TPS base with 200 TPS spikes

### Baselines

Compare adaptive batching against three fixed baselines:

| Fixed Baseline     | Purpose                                    |
| ------------------ | ------------------------------------------ |
| Small fixed batch  | Low latency but higher cost                |
| Medium fixed batch | Balanced baseline                          |
| Large fixed batch  | High throughput but worse low-load latency |

### Metrics

- Goodput TPS
- P95 soft confirmation latency
- P95 hard finality latency
- Gas/tx
- Batch fill ratio
- Batch size selected over time
- Mempool backlog recovery time after burst

### Expected Graphs

1. Time-series graph: traffic rate vs selected adaptive batch size
2. P95 latency comparison: fixed-small vs fixed-medium vs fixed-large vs adaptive
3. Cost/tx comparison under low, medium, high, and burst load
4. Mempool backlog over time during burst workload

### Impact Claim

This is likely your strongest result. Aim for a claim like:

> Adaptive batching keeps low-load latency close to small fixed batches while achieving high-load cost efficiency close to large fixed batches.

---

## Stage 3 — Sequencer Policy Comparison

### Purpose

Measure the performance, fairness, and cost behavior of different ordering policies.

### Parameters to Change

```toml
policy_type = ["FCFS", "FeePriority", "TimeBoost", "FairBFT", "BlobPacking"]
time_window_ms = [100, 250, 500, 1000, 2000]
batch_policy = "fixed" initially, then repeat best policies with "adaptive"
max_batch_size = best value from Stage 1
timeout_interval_ms = best balanced value from Stage 1
```

### Workloads

Use a mixed-fee workload:

| User Class       | Share | Fee Level | Expected Behavior                                   |
| ---------------- | ----: | --------: | --------------------------------------------------- |
| Low-fee users    |   60% |        1x | Should not starve                                   |
| Medium-fee users |   30% |        2x | Normal priority                                     |
| High-fee users   |   10% |        5x | Should get faster inclusion under priority policies |

Also run a burst workload where high-fee transactions arrive during congestion.

### Metrics

- Goodput TPS
- P95 latency per fee class
- Starvation count
- Max wait time
- Reordering distance
- Jain fairness index
- Fee-weighted priority benefit
- Blob fill ratio for BlobPacking
- Cost/tx for BlobPacking vs non-BlobPacking

### Expected Graphs

1. Policy vs P95 latency per transaction class
2. Policy vs Jain fairness index
3. Policy vs starvation count
4. Policy vs reordering distance
5. BlobPacking vs FCFS: blob fill ratio and DA cost/tx

### Impact Claim

This stage should avoid saying one policy is universally best. The better claim is:

> Sequencer policy changes the performance-fairness-cost trade-off. FeePriority and TimeBoost reduce high-priority latency but can penalize low-fee users, while FairBFT improves fairness at the cost of additional ordering delay. BlobPacking improves DA efficiency when transaction payload sizes vary.

---

## Stage 4 — Data Availability Mode and Blob Packing Experiment

### Purpose

Compare calldata, blob, and offchain DA modes. This should produce clear cost-efficiency results.

### Parameters to Change

```toml
da.mode = ["calldata", "blob", "offchain"]
blob_target_bytes = [32768, 65536, 98304, 120000]
blob_fill_target = [0.50, 0.70, 0.80, 0.90, 0.95]
policy_type = ["FCFS", "BlobPacking"]
max_batch_size = [50, 100, 200, 500, 1000]
```

### Workloads

- Transfer-only workload
- DA-heavy workload
- Normal workload
- Low-load workload to expose blob waste
- High-load workload to expose blob efficiency

### Metrics

- DA bytes per transaction
- Gas per transaction
- Blob bytes used
- Blob fill ratio
- Blob waste ratio
- L1 cost per transaction in ETH and USD
- Hard finality latency
- Offchain archiver latency
- Offchain DA failure rate

### Expected Graphs

1. DA mode vs cost/tx
2. Batch size vs blob fill ratio
3. Blob fill target vs P95 latency
4. Blob fill target vs cost/tx
5. Calldata vs blob crossover point
6. BlobPacking vs FCFS under DA-heavy workload

### Important Analysis

Identify the **crossover point**:

```text
The smallest batch size or payload size where blob mode becomes cheaper than calldata mode.
```

This is a high-value result because it gives practical guidance.

### Impact Claim

Possible final claim:

> Blob mode is not automatically cheaper for every workload. It becomes cost-effective only when batch payloads are large enough to fill a meaningful fraction of blob capacity. BlobPacking improves this crossover point by increasing blob utilization.

---

## Stage 5 — Prover Backend and Real Proof Experiment

### Purpose

Measure the difference between mock/fallback proof mode and real proof mode.

### Parameters to Change

```toml
REQUIRE_REAL_PROOFS = [false, true]
ALLOW_PROOF_FALLBACK = [true, false]
PROVER_BACKEND = [available backends]
proof.backend = ["groth16", "plonky2", "halo2"] if implemented
proof.verification_mode = ["onchain", "offchainonly"]
```

Only test proof backends that are actually implemented and stable. If only RISC0 is implemented, compare:

1. Mock/fallback proof mode
2. Real RISC0 proof mode
3. Real proof with fallback disabled

### Workloads

- Transfer-only
- Normal
- Heavy-state

### Batch Sizes

Use selected batch sizes from previous stages:

```text
50, 100, 200, 500
```

Avoid very large values if real proofs take too long.

### Metrics

- Proof generation time
- Proof generation time per transaction
- Proof size
- Journal size
- Peak RAM
- CPU usage
- Prover failure count
- Proof fallback count
- Verification gas
- End-to-end finality latency
- Proven TPS

### Expected Graphs

1. Batch size vs proof generation time
2. Batch size vs peak RAM
3. Batch size vs proven TPS
4. Mock vs real proof finality latency
5. Onchain vs offchain verification cost/latency

### Impact Claim

This stage should produce a realistic bottleneck claim:

> Without real proofs, the system bottleneck appears to be batching or L1 submission. With real proofs enabled, proof generation becomes a dominant contributor to hard finality latency, especially for larger or heavier batches.

---

## Stage 6 — Gas Limit and L1 Submission Stress Test

### Purpose

Find where L1 submission becomes a bottleneck or failure source.

### Parameters to Change

```toml
max_gas_limit = [10000000, 20000000, 30000000]
HARDHAT_MINING_INTERVAL = [1000, 3000, 12000]
REGULAR_GAS_PRICE_GWEI = [5, 10, 30, 100]
BLOB_GAS_PRICE_GWEI = [0.1, 1, 5, 20]
```

### Workloads

- Normal workload
- High load
- DA-heavy workload

### Metrics

- L1 inclusion latency
- Failed batch count
- Gas per batch
- Gas per transaction
- Max batch size before failure
- Finalized TPS
- Cost per transaction under gas price changes

### Expected Graphs

1. Mining interval vs hard finality latency
2. Gas price vs cost/tx for calldata and blob
3. Batch size vs failed batch rate
4. Finalized TPS vs L1 mining interval

### Impact Claim

> Even when L2 execution is fast, hard finality is bounded by L1 inclusion behavior and DA submission cost. This separates soft confirmation performance from final settlement performance.

---

## Stage 7 — Communication and Failure Recovery Experiment

### Purpose

Test whether the sequencer-executor-submit pipeline is stable under delays and failures.

### Parameters to Change

```toml
SEQUENCER_EXECUTOR_PUBLISH_RETRIES = [0, 1, 3, 5]
SEQUENCER_EXECUTOR_PUBLISH_TIMEOUT_MS = [1000, 3000, 5000, 10000]
COMM_MODE = ["grpc", "http"] if both are supported
```

### Faults to Inject

- Executor response delay
- Executor temporary downtime
- Submitter RPC failure
- Archiver unavailability for offchain DA
- L1 RPC timeout

### Metrics

- Batch success rate
- Retry count per batch
- Recovery time
- Duplicate publish count
- Failed batch count
- End-to-end latency under failure
- Mempool backlog after recovery

### Expected Graphs

1. Publish timeout vs failed batch count
2. Retry count vs success rate
3. Failure duration vs recovery time
4. Communication mode vs publish latency

### Impact Claim

> Retry and timeout settings create a reliability-latency trade-off. Aggressive timeouts detect failures faster but can increase false failures, while longer timeouts reduce failure count but increase tail latency.

---

## Stage 8 — Final Best Configuration Comparison

### Purpose

After all sweeps, select the best configurations and compare them against the baseline.

### Configurations to Compare

| Configuration   | Description                                                           |
| --------------- | --------------------------------------------------------------------- |
| Baseline        | Fixed batch, FCFS, calldata, mock/fallback proof as initially defined |
| Best fixed      | Best fixed batch size and timeout from Stage 1                        |
| Best adaptive   | Best adaptive thresholds and batch sizes from Stage 2                 |
| Best fairness   | FairBFT or best fairness-preserving policy                            |
| Best cost       | BlobPacking + blob mode with best fill target                         |
| Best real-proof | Best configuration with `REQUIRE_REAL_PROOFS = true`                  |

### Workloads

Run all final configurations on:

- Normal workload
- Burst workload
- Heavy-state workload
- DA-heavy workload

### Metrics

- Goodput TPS
- P95 soft latency
- P95 hard finality latency
- Cost/tx
- Proof time
- Peak memory
- Fairness index
- Failure rate

### Final Output Table

Prepare a table like this:

| Configuration   | Workload    | Goodput TPS | P95 Soft Latency | P95 Hard Finality | Cost/Tx | Proof Time | Fairness | Failure Rate | Best For           |
| --------------- | ----------- | ----------: | ---------------: | ----------------: | ------: | ---------: | -------: | -----------: | ------------------ |
| Baseline        | Normal      |         ... |              ... |               ... |     ... |        ... |      ... |          ... | Reference          |
| Best fixed      | Normal      |         ... |              ... |               ... |     ... |        ... |      ... |          ... | Simple deployment  |
| Best adaptive   | Burst       |         ... |              ... |               ... |     ... |        ... |      ... |          ... | Variable load      |
| BlobPacking     | DA-heavy    |         ... |              ... |               ... |     ... |        ... |      ... |          ... | Lowest DA cost     |
| Real-proof best | Heavy-state |         ... |              ... |               ... |     ... |        ... |      ... |          ... | Realistic security |

---

## 8. Recommended Experiment Matrix

A full factorial matrix would be too large. Use this reduced staged matrix.

### Pilot Runs

Use pilot runs to check correctness.

```text
repeats = 1 or 2
duration = 2–5 minutes
warmup = 30 seconds
```

### Final Runs

Use final runs for report-quality results.

```text
repeats = 5
duration = 10–15 minutes
warmup = 1–2 minutes
seeds = [42, 43, 44, 45, 46]
```

### Core Experiments Summary

| Stage   | Main Variable                |      Number of Candidate Runs |            Final Runs |
| ------- | ---------------------------- | ----------------------------: | --------------------: |
| Stage 1 | Fixed batch size and timeout |                           30+ | 8–12 selected configs |
| Stage 2 | Adaptive batching            |                           20+ |  4–6 selected configs |
| Stage 3 | Policy type                  | 5 policies × selected windows | 5–10 selected configs |
| Stage 4 | DA mode and blob packing     |                           30+ | 8–12 selected configs |
| Stage 5 | Prover mode/backend          |     depends on implementation |  4–8 selected configs |
| Stage 6 | L1 submission/gas            |                         10–20 |  4–6 selected configs |
| Stage 7 | Reliability                  |                         10–20 |  4–6 selected configs |
| Stage 8 | Best final comparison        |                     6 configs |   all final workloads |

---

## 9. Statistical Analysis Plan

Do not present only a single run. Use repeated runs and confidence intervals.

### Required Statistical Outputs

For each metric, report:

- Mean
- Median
- Standard deviation
- 95% confidence interval
- P50, P90, P95, P99 for latency
- Minimum and maximum for failure-related metrics

### Recommended Significance Tests

Use simple and explainable tests.

| Comparison                     | Recommended Method                      |
| ------------------------------ | --------------------------------------- |
| Two configurations             | Mann-Whitney U test or t-test if normal |
| Multiple configurations        | Kruskal-Wallis test or ANOVA if normal  |
| Latency distributions          | ECDF plot and percentile comparison     |
| Cost/latency trade-off         | Pareto frontier                         |
| Relationship between variables | Correlation and regression              |

For the FYP report, the most important analysis is not the p-value. It is the trade-off explanation.

---

## 10. Graphs That Should Appear in the Final Report

These are the graphs most likely to make an impact.

### Core Performance Graphs

1. **Throughput vs P95 latency** for fixed, adaptive, and best configurations.
2. **Batch size vs gas/tx** showing amortization.
3. **Batch size vs proof time** showing proving bottleneck.
4. **Traffic rate vs goodput** showing saturation point.
5. **Mempool backlog over time** under burst load.

### DA and Cost Graphs

6. **DA mode vs cost/tx** for calldata, blob, and offchain.
7. **Batch payload size vs blob fill ratio**.
8. **Blob fill target vs P95 latency and cost/tx**.
9. **Gas price sensitivity** showing calldata vs blob cost under different gas/blob prices.

### Policy and Fairness Graphs

10. **Sequencing policy vs P95 latency per user/fee class**.
11. **Sequencing policy vs Jain fairness index**.
12. **Policy vs starvation count**.
13. **BlobPacking vs FCFS blob utilization**.

### Reliability Graphs

14. **Publish timeout vs failed batch count**.
15. **Retry count vs recovery success rate**.
16. **Mining interval vs hard finality latency**.

---

## 11. Data Schema to Collect

### 11.1 Transaction-Level CSV

Recommended file: `tx_log_<run_id>.csv`

```csv
tx_id,run_id,workload_type,user_id,tx_type,fee_class,payload_bytes,submitted_at,accepted_at,rejected_at,reject_reason,batch_id,batch_included_at,soft_confirmed_at,executed_at,proof_completed_at,l1_submitted_at,l1_included_at,finalized_at,status
```

Derived columns:

```text
admission_latency_ms = accepted_at - submitted_at
queue_latency_ms = batch_included_at - accepted_at
soft_latency_ms = soft_confirmed_at - submitted_at
execution_latency_ms = executed_at - batch_included_at
proof_latency_ms = proof_completed_at - executed_at
l1_latency_ms = l1_included_at - l1_submitted_at
hard_finality_latency_ms = finalized_at - submitted_at
```

### 11.2 Batch-Level CSV

Recommended file: `batch_log_<run_id>.csv`

```csv
batch_id,run_id,policy_type,batch_policy,tx_count,min_batch_size,max_batch_size,timeout_interval_ms,batch_opened_at,batch_sealed_at,batch_reason,execution_started_at,execution_completed_at,proof_started_at,proof_completed_at,submit_started_at,l1_tx_hash,l1_included_at,finalized_at,da_mode,calldata_bytes,blob_bytes,blob_count,blob_fill_ratio,gas_used,da_gas_used,verify_gas_used,cost_eth,cost_usd,status,error
```

### 11.3 Resource Metrics CSV

Recommended file: `resource_log_<run_id>.csv`

```csv
timestamp,run_id,component,cpu_percent,memory_mb,network_rx_bytes,network_tx_bytes,disk_read_bytes,disk_write_bytes,mempool_size,batch_queue_size,proof_queue_size
```

### 11.4 Run Metadata JSON

Recommended file: `run_metadata.json`

```json
{
  "run_id": "adaptive_burst_r01",
  "git_commit": "<commit_hash>",
  "timestamp_start": "...",
  "timestamp_end": "...",
  "environment": "hardhat-local",
  "workload": {
    "arrival_model": "poisson_burst",
    "rate_tps": 100,
    "duration_s": 600,
    "warmup_s": 60,
    "seed": 42,
    "mix": "normal"
  },
  "config": {
    "sequencer": {},
    "executor": {},
    "submitter": {},
    "l1": {}
  }
}
```

---

## 12. How to Interpret Results

### 12.1 Throughput

Use finalized goodput as the main throughput metric. If submitted TPS is high but finalized TPS is low, the system is overloaded.

### 12.2 Latency

Separate soft confirmation latency from hard finality latency. A rollup can give fast soft confirmation while still taking longer for proof generation and L1 settlement.

### 12.3 Cost

Always break cost into:

```text
cost/tx = DA cost/tx + proof verification cost/tx + fixed L1 submission overhead/tx
```

This breakdown shows whether optimization should target DA, proof verification, or batching.

### 12.4 Proving

Report both proof time per batch and proof time per transaction. A larger batch may have higher total proof time but lower proof overhead per transaction.

### 12.5 Fairness

Do not claim a policy is better only because it has higher TPS. A policy that starves low-fee transactions may have good throughput but poor fairness.

---

## 13. Recommended Final Claims to Target

These are realistic result claims that would be meaningful if supported by the data.

### Claim 1 — Fixed Batching Trade-off

> Larger batches reduce cost per transaction because fixed L1 and proof overheads are amortized, but they increase P95 latency under low and medium load because transactions wait longer for batch formation.

### Claim 2 — Adaptive Batching Benefit

> Adaptive batching provides a better trade-off than any single fixed batch size under bursty workloads, because it uses small batches during low load and larger batches during congestion.

### Claim 3 — Blob Packing Crossover

> Blob mode becomes cheaper than calldata only after the batch payload reaches a sufficient size. BlobPacking reduces the crossover point by improving blob utilization.

### Claim 4 — Prover Bottleneck

> When real proofs are required, proof generation becomes one of the dominant contributors to hard finality latency, especially for heavy workloads and larger batches.

### Claim 5 — Sequencer Policy Trade-off

> FeePriority and TimeBoost improve latency for high-priority users but can increase latency variance and starvation risk for low-fee users, while FairBFT improves fairness at some ordering delay cost.

### Claim 6 — Soft vs Hard Finality Gap

> Soft confirmation latency is mainly controlled by the sequencer and batching policy, while hard finality latency is controlled by proving time, DA submission, and L1 inclusion.

---

## 14. Minimum Viable Benchmark Set

If time is limited, run these experiments first.

### Must-Have Experiments

1. Fixed batch size sweep: `max_batch_size = [25, 50, 100, 200, 500]`
2. Timeout sweep: `timeout_interval_ms = [500, 1000, 2000, 5000]`
3. Fixed vs adaptive batching under burst load
4. DA mode comparison: `calldata`, `blob`, `offchain`
5. BlobPacking vs FCFS under DA-heavy workload
6. Mock proof vs real proof
7. Policy comparison: FCFS, FeePriority, TimeBoost, FairBFT, BlobPacking

### Must-Have Metrics

- Goodput TPS
- P95 soft confirmation latency
- P95 hard finality latency
- Gas/cost per transaction
- Proof generation time
- Blob fill ratio
- Mempool backlog
- Fairness index
- Failure rate

### Must-Have Graphs

- Throughput vs P95 latency
- Batch size vs gas/tx
- Batch size vs proof time
- DA mode vs cost/tx
- Fixed vs adaptive under burst workload
- Policy vs fairness/latency

---

## 15. Recommended Report Structure for Benchmark Results

Use this structure in the final report or paper.

```text
1. Experimental Setup
   - Hardware
   - Software versions
   - Network environment
   - Baseline configuration
   - Workloads
   - Metrics

2. Baseline Performance
   - Throughput
   - Latency
   - Cost
   - Proof time

3. Batch Size and Timeout Results
   - Fixed batching trade-off
   - Best fixed configuration

4. Adaptive Batching Results
   - Low/medium/high/burst load comparison
   - Adaptive vs fixed

5. Sequencer Policy Results
   - FCFS vs FeePriority vs TimeBoost vs FairBFT vs BlobPacking
   - Fairness and starvation analysis

6. Data Availability Results
   - Calldata vs blob vs offchain
   - Blob fill target analysis
   - Cost crossover point

7. Prover Results
   - Mock vs real proof
   - Proof backend comparison if available
   - Memory and CPU cost

8. Reliability Results
   - Retry/timeout behavior
   - L1 mining interval impact

9. Optimized Configuration
   - Best configuration table
   - Improvement over baseline

10. Threats to Validity
   - Synthetic workload limitations
   - Testnet differences
   - Prototype limitations
   - Mock proof limitations
```

---

## 16. Final Recommendation

For maximum impact, do not present the benchmark as a simple list of parameter changes. Present it as a **multi-objective optimization study**.

The final message should be:

> RollupX shows that ZK-rollup scalability is not controlled by one parameter. Throughput, latency, cost, fairness, and finality depend on the interaction between batching, sequencing, proving, DA mode, and L1 submission. The benchmark identifies the conditions under which fixed batching, adaptive batching, blob packing, and real-proof execution are beneficial, and provides reproducible configuration guidance for future rollup deployments.

This framing is stronger than saying “we tested batch sizes.” It shows that your implementation is useful as a research platform and that the results can guide real rollup design decisions.
