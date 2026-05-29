## Bottom line

Your current implementation is **good enough for a batch-size feasibility study**, but **not yet aligned with Claude’s stronger research goal**: a controlled interaction study over **batch size × scheduling policy × DA mode**, plus validated improvements.

Right now, the READMEs show a benchmark suite mainly designed around **batch-size sweeps**: transaction count, serialized bytes, gas proxies, execution/proof timing, and L1 submission metrics as batch size changes. It recreates the Docker core stack per run and injects batch size, timeout, policy, DA mode, proof backend, experiment ID, and metrics directory. That is a strong base.

But Claude’s proposed experiment is broader: baseline, diagnostic sweeps, a 5 × 3 × 3 factorial interaction study, then intervention validation for adaptive batching, blob-packing scheduling, and soft finality signaling.

---

# 1. What already matches Claude’s proposal

## Good: Docker-per-run isolation is already mostly right

The benchmark runner recreates the Docker `core` stack for every run, passes config via environment variables, then waits for metric files to stabilize. This is exactly the kind of reproducibility you need for publishable experiments.

Keep this.

## Good: the benchmark suite already records the right _kinds_ of component outputs

Each run produces:

```text
sequencer_batch_metrics.jsonl
executor_batch_metrics.jsonl
submitter_metrics.json
tx_log_<run_id>.csv
run_metadata.json
run_status.json
```

That gives you the basic observability split you need: sequencer for batching/scheduling, executor for execution/proving, submitter for L1/DA/cost.

Keep this structure.

## Good: sequencer already supports multiple policies

The sequencer has configurable policies: `FCFS`, `FeePriority`, `TimeBoost`, and `FairBFT`. That means Claude’s “FIFO vs fee-priority” comparison is already implementable without a major rewrite.

However, Claude’s proposed **blob-packing scheduler** is not currently listed as an available policy, so that is an intervention you still need to implement.

## Good: executor metrics are now much closer to research-grade

The executor README says it records phase-level timing:

```text
signature_verify_ms
nonce_balance_check_ms
state_transition_ms
merkle_update_ms
state_diff_computation_ms
```

It also records `total_prover_wall_ms`, `proof_mode`, and `proof_bytes`.

This directly fixes one of the earlier concerns: Merkle update time is now separated, which is important because it lets you explain _why_ execution time grows with batch size instead of only reporting that it grows.

## Good: submitter has the right lifecycle abstraction

The submitter tracks the batch lifecycle through `Discovered → Proving → Proved → Submitting → Submitted → Confirmed/Failed`, and exposes `batch_e2e_duration_seconds`, `prove_duration_seconds`, `submit_tx_duration_seconds`, and DA-partitioned submission counts.

That is useful for finality and settlement latency analysis.

---

# 2. Main mismatches against Claude’s experiment design

## Mismatch 1: Current suite is still batch-size-first, not factorial-first

Your benchmark README says the suite is for a **batch-size feasibility study**. The named experiments are also batch-size focused:

```text
exp_001_batch_size_bs001_calldata_balanced_10tps
exp_002_batch_size_bs010_calldata_balanced_10tps
...
exp_008_batch_size_bs1000_calldata_balanced_10tps
```

That means your current setup answers:

> “How does batch size affect performance?”

But Claude’s stronger research question is:

> “How do batch size, scheduling policy, and DA mode jointly affect throughput, latency, and cost?”

Claude’s proposed core experiment is explicitly:

```text
5 batch sizes × 3 scheduling policies × 3 DA modes
```

with repetitions and load levels.

### What must change

Add a new matrix generator, not just a batch-size preset.

Minimum matrix:

```text
batch_size = [10, 50, 100, 500, 1000]
timeout_ms = [5000] initially fixed
scheduling_policy = [FCFS, FeePriority, BlobPacking]
da_mode = [calldata, blob, offchain_commitment]
load_level = [low_10tps, medium_50tps, burst_200tps]
repeats = 5
```

This becomes:

```text
5 × 3 × 3 × 3 × 5 = 675 runs
```

That matches Claude’s design.

---

## Mismatch 2: Blob-packing scheduling is proposed but not implemented

The sequencer currently has `FCFS`, `FeePriority`, `TimeBoost`, and `FairBFT`.

Claude’s intervention depends on a new policy:

> sort or group transactions by byte size so blobs fill more efficiently.

That policy is not in the README.

### What must change

Add:

```rust
Policy::BlobPacking
```

or:

```toml
[scheduling]
policy_type = "BlobPacking"
```

Implementation logic should be simple:

```text
1. Preserve forced transactions first.
2. For normal transactions, group/sort by serialized transaction byte size.
3. Build batches to maximize DA payload utilization.
4. Record blob fill percentage and wasted bytes.
```

Do **not** replace `FeePriority`. You need all three:

```text
FCFS          = baseline fairness
FeePriority   = economic/revenue policy
BlobPacking   = proposed DA-aware intervention
```

---

## Mismatch 3: DA modes are named differently across proposal and implementation

Claude proposed:

```text
raw calldata
compressed state-diff calldata
off-chain with on-chain pointer
```

Your contracts README says DA providers are:

```text
CalldataDA
BlobDA
OffChainDA
```

with OffChainDA storing data off-chain with an on-chain commitment.

Your submitter README says it supports calldata and EIP-4844 blobs with archiver integration.

### Problem

There is a conceptual mismatch:

| Claude proposal                         | Current implementation wording          | Risk                                                |
| --------------------------------------- | --------------------------------------- | --------------------------------------------------- |
| raw calldata                            | CalldataDA                              | fine                                                |
| compressed calldata/state-diff calldata | Payload compression / zlib in submitter | maybe implemented, but not clearly a DA mode        |
| EIP-4844 blobs                          | BlobDA / blob gas metrics               | fine if actually wired end-to-end                   |
| off-chain pointer                       | OffChainDA                              | only valid if data is really stored and retrievable |

### What must change

Define the DA modes **strictly** in the experiment config:

```yaml
da_modes:
  calldata_raw:
    meaning: full batch/state-diff bytes posted as calldata
  calldata_compressed:
    meaning: compressed payload posted as calldata
  blob:
    meaning: EIP-4844 blob path with blob_gas_used and blob fee captured
  offchain_commitment:
    meaning: payload stored in archiver/IPFS/local DA server; only commitment/pointer on L1
```

Then choose only three for the paper. I recommend:

```text
calldata_raw
blob
offchain_commitment
```

Use `calldata_compressed` as an extra sensitivity experiment, not part of the main 5 × 3 × 3 factorial matrix.

---

## Mismatch 4: Cost breakdown may still be too coarse

The submitter README says the benchmark suite uses `gas_used` and `blob_gas_used` to calculate USD cost.

That is necessary, but not sufficient for Claude’s research claim. Claude wanted a component-level cost breakdown:

```text
proof verification gas
state root update gas
DA posting gas
blob gas
overhead gas
cost per transaction
```

The smart contracts README says Foundry/Hardhat gas reports are available and points to `reports/gas-report.txt` for function-level measurements.

### Problem

`gas_used` and `blob_gas_used` give total cost, but they do not automatically explain where the cost went.

### What must change

For every submitted batch, the final joined row should include:

```text
regular_gas_used
blob_gas_used
effective_gas_price_wei
blob_gas_price_wei
total_l1_cost_wei
total_l1_cost_usd
cost_per_l2_tx_usd

verify_gas_estimate
state_update_gas_estimate
da_calldata_gas_estimate
blob_da_cost_wei
overhead_gas_estimate
```

How to get this:

1. Use transaction receipts for `gas_used`, `effective_gas_price`, `blob_gas_used`, and blob gas price.
2. Use Hardhat/Foundry function-level gas reports for static estimates of verifier and bridge function costs.
3. Compute DA calldata gas from payload bytes.
4. For blobs, keep blob gas separate from regular execution gas.

---

## Mismatch 5: Baseline config is inconsistent

Claude’s baseline was:

```text
batch size = 100
FIFO
raw calldata
Groth16 prover
```

Your benchmark naming currently has:

```text
exp_000_baseline_bs050_calldata_balanced_10tps
```

and the batch-size sweep includes `bs001`, `bs010`, `bs025`, `bs050`, `bs100`, `bs250`, `bs500`, `bs1000`.

### Problem

If the baseline is `bs050` in the suite but `bs100` in the experiment design, your paper and scripts will disagree.

### What must change

Pick one. I recommend Claude’s `bs100` because it sits in the middle of the range and is already the sequencer default. The sequencer config default is `max_batch_size = 100`, `timeout_interval_ms = 5000`, and `min_batch_size = 10`.

Rename baseline:

```text
exp_000_baseline_bs100_fcfs_calldata_raw_10tps
```

---

# 3. Benchmarking thought process

Think of the benchmark as three layers.

## Layer A: Controlled system variables

These are the independent variables you intentionally change:

```text
batch_size
batch_timeout_ms
scheduling_policy
da_mode
load_level
proof_backend
```

For the main paper, keep proof backend fixed:

```text
proof_backend = risc0/groth16 only
```

The executor supports `REQUIRE_REAL_PROOFS=1`, which should be enabled for any production-baseline or final experiment so fake/mock proof timing does not pollute results.

## Layer B: Output metrics

Every run should produce one joined row per batch with:

```text
experiment_id
run_id
repeat
batch_id
configured_batch_size
actual_tx_count
load_level
scheduling_policy
da_mode

sequencer:
  p50_wait_ms
  p99_wait_ms
  raw_tx_bytes
  gas_limit_utilization
  ordering_efficiency
  reordering_events

executor:
  signature_verify_ms
  nonce_balance_check_ms
  state_transition_ms
  merkle_update_ms
  state_diff_computation_ms
  total_prover_wall_ms
  proof_mode
  proof_bytes

submitter:
  batch_e2e_duration_seconds
  prove_duration_seconds
  submit_tx_duration_seconds
  gas_used
  blob_gas_used
  total_l1_cost_wei
  cost_per_tx
```

The sequencer already records wait time, fairness, ordering efficiency, reordering events, cache hit rate, stale nonce rejections, gas limit utilization, and raw transaction bytes.

The executor already records phase-level execution and proof metadata.

The submitter already records lifecycle/finality/proving/submission metrics and gas/blob gas cost inputs.

So the main work is not inventing metrics. It is **joining them reliably and ensuring the experiment matrix produces the right conditions**.

## Layer C: Derived research metrics

From the raw metrics, compute:

```text
throughput_tps = confirmed_l2_tx_count / measured_duration_s

p50_latency_ms = tx arrival → L1 confirmed
p95_latency_ms = tx arrival → L1 confirmed
sequencer_wait_ms = tx arrival → batch sealed
execution_ms = batch sealed → execution done
proof_ms = execution done → proof generated
settlement_ms = submit start → L1 confirmed

cost_per_tx = total_l1_cost / actual_tx_count
blob_utilization = payload_bytes / blob_capacity_bytes
prover_bottleneck_ratio = total_prover_wall_ms / batch_interval_ms
merkle_share = merkle_update_ms / total_execution_ms
da_cost_share = da_cost / total_l1_cost
```

The key research outputs should be:

```text
1. Throughput-latency-cost Pareto frontier
2. Cost breakdown by DA mode and batch size
3. Interaction plots: batch size × scheduling × DA
4. Intervention delta: baseline vs adaptive batching/blob-packing
```

---

# 4. New benchmark setup I recommend

## Phase 0 — Smoke validation

Purpose: verify metrics join and end-to-end correctness.

```text
batch_size = [10]
policy = [FCFS]
da_mode = [calldata_raw]
load = [10 tps]
repeat = 1
duration = 30s
proof = fake allowed only here
```

Pass condition:

```text
sequencer batches == executor batches == submitter confirmed batches
all joined rows have batch_id
no missing metrics files
```

Your current suite already has smoke mode and metric synchronization, so reuse it.

---

## Phase 1 — Baseline

```text
batch_size = 100
timeout_ms = 5000
min_batch_size = 10
policy = FCFS
da_mode = calldata_raw
load = 10 tps
proof = real Groth16/RISC0
repeats = 5 or 10
duration = 120s
warmup = 15s
```

Output:

```text
mean ± stddev:
  throughput
  p50/p95 latency
  cost_per_tx
  total_prover_wall_ms
  merkle_update_ms
  gas_used
```

---

## Phase 2 — Diagnostic sweeps

### 2A. Batch-size sweep

```text
batch_size = [10, 50, 100, 500, 1000]
policy = FCFS
da_mode = calldata_raw
load = 50 tps
repeats = 5
```

Purpose:

```text
find throughput/latency knee point
find prover scaling curve
find cost amortization curve
```

### 2B. Timeout sweep

```text
batch_size = 100
timeout_ms = [1000, 5000, 15000, 30000]
policy = FCFS
da_mode = calldata_raw
load = [10, 50, 200] tps
repeats = 5
```

Purpose:

```text
identify adaptive batching thresholds
```

### 2C. DA sweep

```text
batch_size = 100
policy = FCFS
da_mode = [calldata_raw, blob, offchain_commitment]
load = 50 tps
repeats = 5
```

Purpose:

```text
cost breakdown
DA cost share
blob utilization
```

### 2D. Scheduling sweep

```text
batch_size = 100
policy = [FCFS, FeePriority, BlobPacking]
da_mode = blob
load = [10, 50, 200] tps
repeats = 5
```

Purpose:

```text
show whether scheduling matters only under congestion / blob DA / mixed tx sizes
```

---

## Phase 3 — Factorial interaction study

This is your main contribution.

```text
batch_size = [10, 50, 100, 500, 1000]
policy = [FCFS, FeePriority, BlobPacking]
da_mode = [calldata_raw, blob, offchain_commitment]
load = [10, 50, 200] tps
repeats = 5
```

Total:

```text
675 runs
```

Output:

```text
heatmaps:
  cost_per_tx by batch_size × da_mode
  p95_latency by batch_size × policy
  throughput by batch_size × da_mode

interaction plots:
  policy effect under each DA mode
  DA effect under each batch size
  load sensitivity under each policy

Pareto frontier:
  x = p95 latency
  y = cost_per_tx
  marker size = throughput
  color = DA mode
  shape = scheduling policy
```

Claude’s core claim depends on this exact interaction study, not just individual sweeps.

---

## Phase 4 — Validate improvements

## Improvement 1: Adaptive batching

Implement:

```text
if mempool_depth < low_threshold:
    seal by timeout
elif mempool_depth >= high_threshold:
    seal by size
else:
    use normal batch_size/timeout rule
```

Recommended first thresholds:

```text
low_threshold = 0.25 × max_batch_size
high_threshold = 0.80 × max_batch_size
```

Validate against:

```text
fixed bs50
fixed bs100
fixed bs500
adaptive
```

At:

```text
load = [10, 50, 200] tps
da = calldata_raw and blob
policy = FCFS
```

Success metric:

```text
lower p95 latency than large fixed batches
lower cost_per_tx than tiny fixed batches
no throughput collapse under burst load
```

## Improvement 2: Blob-packing scheduler

Implement new scheduling policy:

```text
BlobPacking
```

Validate only under:

```text
da_mode = blob
mixed transaction byte sizes
batch_size = [100, 500, 1000]
load = [50, 200] tps
```

Success metric:

```text
higher blob utilization
lower blob cost per tx
no unacceptable p95 latency increase
```

Claude’s proposal specifically says to compare FIFO, fee-priority, and blob-packing using blob fill percentage and DA cost per transaction.

## Improvement 3: Soft finality signal

I would make this a stretch goal, not core.

Your contracts currently have a bridge and modular DA/verifier architecture. But a “proof started” signal is awkward if proof generation happens off-chain before submission. A Solidity event cannot honestly say “proving began” unless an actor submits that event on-chain, which itself costs gas and changes the benchmark.

Better version:

```text
soft_finality_timestamp = sequencer sealed batch
execution_finality_timestamp = executor accepted batch and produced trace
proof_ready_timestamp = proof generated
hard_finality_timestamp = L1 confirmed
```

Do this as off-chain telemetry first. Only add an on-chain event if you specifically want to study the gas/latency tradeoff of signaling.

---

# 5. Concrete implementation/modification plan

## Priority 1 — Make experiment config factorial

Add a file:

```text
benchmark-suite/config/experiment_matrix.yaml
```

Example:

```yaml
name: factorial_v1
repeats: 5
warmup_seconds: 15
duration_seconds: 120

factors:
  batch_size: [10, 50, 100, 500, 1000]
  timeout_ms: [5000]
  min_batch_size: [10]
  scheduling_policy: ["FCFS", "FeePriority", "BlobPacking"]
  da_mode: ["calldata_raw", "blob", "offchain_commitment"]
  load_level:
    - name: low_10tps
      arrival_process: poisson
      rate_tps: 10
    - name: medium_50tps
      arrival_process: poisson
      rate_tps: 50
    - name: burst_200tps
      arrival_process: poisson
      rate_tps: 200
```

Modify `run_matrix.sh` so it can run:

```bash
bash benchmark-suite/scripts/run_matrix.sh --matrix factorial_v1
```

Keep old batch-size presets, but make them “legacy feasibility presets.”

---

## Priority 2 — Standardize run metadata

Every run must write:

```json
{
  "experiment_id": "exp_123_bs100_blob_feepriority_50tps_r03",
  "phase": "factorial_v1",
  "repeat": 3,
  "batch_size": 100,
  "timeout_ms": 5000,
  "min_batch_size": 10,
  "scheduling_policy": "FeePriority",
  "da_mode": "blob",
  "load_tps": 50,
  "arrival_process": "poisson",
  "proof_backend": "risc0",
  "require_real_proofs": true,
  "network": "hardhat",
  "block_time_ms": 12000,
  "reference_eth_usd": 2500,
  "reference_gas_price_gwei": 2
}
```

This is essential because the current benchmark suite already creates `run_metadata.json`, but the new experiment needs this metadata to be rich enough to reconstruct the full factorial configuration.

---

## Priority 3 — Implement `BlobPacking` in sequencer

Add to `scheduler/policies.rs`:

```rust
pub struct BlobPackingPolicy;

impl SchedulingPolicy for BlobPackingPolicy {
    fn order(&self, txs: Vec<Transaction>) -> Vec<Transaction> {
        // forced txs handled before this layer if existing architecture keeps that invariant
        let mut txs = txs;
        txs.sort_by_key(|tx| serialized_size(tx));
        txs
    }
}
```

Better version:

```text
sort by size descending
pack batches greedily toward blob target capacity
preserve deterministic tie-breaker by arrival timestamp/hash
```

Record extra metrics:

```text
payload_bytes
blob_target_bytes
blob_utilization_pct
blob_wasted_bytes
tx_size_p50
tx_size_p95
```

---

## Priority 4 — Add adaptive batch trigger mode

Current trigger hierarchy is forced → size → timeout.

Add:

```toml
[batch]
trigger_mode = "Fixed" # Fixed | Adaptive
adaptive_low_watermark = 25
adaptive_high_watermark = 80
adaptive_max_delay_ms = 5000
```

Logic:

```text
forced tx exists:
    seal immediately
pool_size >= max_batch_size:
    seal
trigger_mode == Adaptive and pool_size >= high_watermark:
    seal
elapsed >= timeout and pool_size >= min_batch_size:
    seal
elapsed >= adaptive_max_delay and pool_size > 0:
    seal
```

Record:

```text
seal_reason = forced | size | timeout | adaptive_high_watermark | adaptive_max_delay
pool_depth_at_seal
time_since_first_tx_ms
```

Without `seal_reason`, you cannot prove adaptive batching actually changed behavior.

---

## Priority 5 — Build a joined batch-level dataset

Add:

```text
benchmark-suite/data-tools/join_batches.py
```

Join keys:

```text
experiment_id
run_id
batch_id
```

Inputs:

```text
sequencer_batch_metrics.jsonl
executor_batch_metrics.jsonl
submitter_metrics.json
tx_log_<run_id>.csv
run_metadata.json
```

Output:

```text
data-tools/out/joined_batch_results.csv
data-tools/out/joined_tx_results.csv
```

The benchmark README currently says to use `all_batch_results.csv` and plot against actual `tx_count`, not just configured batch size. That is correct. Extend it rather than replacing it.

---

## Priority 6 — Add validity filters

Before analysis, automatically mark runs invalid if:

```text
proof_mode != groth16 when require_real_proofs = true
missing submitter rows
sequencer batches != executor batches != submitter completed batches
actual_tx_count == 0
gas_used missing for submitted batch
blob mode but blob_gas_used missing
offchain mode but no storage receipt / commitment proof
```

The executor already supports `REQUIRE_REAL_PROOFS=1`, so use that for final experiments.

---

# 6. What I would _not_ do

## Do not compare many proof systems

Claude was right to deprioritize Groth16 vs Plonky2 vs Halo2. Your current implementation is already RISC0/Groth16-oriented, and the executor has a strict real-proof path.

Changing proof systems will consume time without strengthening the core contribution.

## Do not make OffChainDA look equivalent unless it really stores data

If `OffChainDA` is just an on-chain commitment without a real retrievable data store, label it honestly:

```text
offchain_commitment_baseline
```

not:

```text
full off-chain DA
```

The contracts README says OffChainDA stores data off-chain with an on-chain commitment, so the benchmark must verify the storage side actually happened.

## Do not mix fake and real proof results

Use fake/mock proofs only for smoke tests. For any claim about throughput, latency, or cost, use:

```text
PROVER_BACKEND=risc0
REQUIRE_REAL_PROOFS=1
```

The executor README explicitly supports this.

---

# 7. Final recommended research framing

Your final paper/project should not be framed as:

> “We built a zk-rollup prototype and benchmarked batch size.”

It should be framed as:

> “We built a reproducible benchmarking harness to map the interaction between batch size, sequencing policy, and data availability mode in a ZK-rollup pipeline, then validated two low-complexity optimizations: adaptive batching and DA-aware blob-packing scheduling.”

That framing matches Claude’s strongest proposed contribution: an empirical map of the cost-throughput-latency frontier across batch size, scheduling, and DA mode.

Your implementation is close. The big changes are:

```text
1. Upgrade from batch-size sweep to factorial matrix.
2. Add BlobPacking scheduler.
3. Add Adaptive batch trigger mode.
4. Make DA modes explicit and honest.
5. Join sequencer/executor/submitter metrics into one batch-level dataset.
6. Add validity filters so bad runs cannot enter the paper.
7. Produce Pareto frontier + interaction plots, not only line charts.
```
