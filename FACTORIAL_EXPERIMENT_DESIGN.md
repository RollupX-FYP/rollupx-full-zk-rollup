# Factorial Experiment Design: Throughput × Latency × Cost Analysis

## Objective

After component verification tests pass, run a comprehensive factorial experiment to establish:
- **Throughput**: txs per second achievable at each batch size
- **Latency**: end-to-end latency (sequencer → executor → prover → submitter)
- **Cost per Transaction**: in terms of proof size, DA calldata/blob cost, and overhead

Construct **Pareto frontier** across these three metrics to understand system trade-offs under different configurations.

---

## Experimental Setup

### Independent Variables (3 Levers)

#### 1. Batch Size
- **Levels**: 10, 50, 100, 500, 1000 transactions
- **Rationale**: Measure how batch size impacts proof time, latency, and amortized cost
- **Measurements per level**: 10 runs (for variance estimation)

#### 2. Sequencing Policy
- **FIFO** (First-In-First-Out)
  - Transactions ordered by arrival time
  - Baseline policy
  - Predictable latency per tx

- **Fee-Priority** (MEV-aware)
  - Transactions ordered by: boost_bid (descending) → gas_price (descending) → nonce
  - Enables MEV capture by builder
  - May increase variance in tx latency

#### 3. Data Availability Mode
- **Calldata** (L1 Ethereum calldata)
  - Cost: ~4 gas/byte + 16 gas/byte zeros (assume 10% zeros)
  - Throughput: Limited by L1 block gas (12M gas/block ≈ 120M bytes/year)
  - Latency: ~12 seconds (L1 block time)

- **EIP-4844 Blobs** (Proto-Danksharding)
  - Cost: ~0.08 gas/byte (blob pricing)
  - Throughput: 3 blobs/block × 128 KB/blob × 6 blocks/minute
  - Latency: 12 seconds (L1 block time)

- **Off-Chain** (Celestia, EigenDA, or centralized sequencer signing)
  - Cost: 0 (assume paid separately)
  - Throughput: Unlimited (bounded by sequencer/DA layer only)
  - Latency: 1-3 seconds (DA provider response time)

---

## Experimental Design

### Factorial Combination Matrix

```
5 batch sizes × 2 policies × 3 DA modes = 30 combinations

┌─────────────────────────────────────────────────────────────┐
│ Batch Size × Policy × DA Mode Combinations                  │
├─────────────────────────────────────────────────────────────┤
│  10 txs  { FIFO × [Calldata, Blobs, Off-Chain]  } = 3 tests │
│  50 txs  { FIFO × [Calldata, Blobs, Off-Chain]  } = 3 tests │
│ 100 txs  { FIFO × [Calldata, Blobs, Off-Chain]  } = 3 tests │
│ 500 txs  { FIFO × [Calldata, Blobs, Off-Chain]  } = 3 tests │
│1000 txs  { FIFO × [Calldata, Blobs, Off-Chain]  } = 3 tests │
│           (Fee-Priority × ... ) = 15 additional tests       │
└─────────────────────────────────────────────────────────────┘

Total: 30 configurations × 10 runs = 300 experiments
```

### Measurement Protocol

For **each configuration**:

1. **Setup Phase**
   - Initialize clean executor state
   - Seed N accounts with 10M wei each
   - Generate N×batch_size transactions using poisson workload generator
   - Sort by policy (FIFO or Fee-Priority)

2. **Execution Phase**
   - Record timestamps: 
     - T0: batch start
     - T1: sequencer batch published
     - T2: executor receives batch (gRPC receive)
     - T3: execution complete (all txs processed)
     - T4: trace persisted
     - T5: proof generation started
     - T6: proof artifacts generated
     - T7: batch published to submitter
   
   - Collect metrics:
     - **Throughput**: batch_size / (T7 - T1) [txs/sec]
     - **Executor Latency**: T4 - T2 [ms]
     - **Proof Latency**: T6 - T5 [ms]
     - **E2E Latency**: T7 - T1 [ms]
     - **Proof Size**: bytes written to disk
     - **Journal Size**: bytes in journal
     - **Memory Peak**: max resident set size
     - **State Diffs**: count of state changes
     - **TX Outcomes**: count of included vs rejected txs

3. **DA Cost Calculation**
   - **Calldata Mode**:
     - Serialize trace (JSON)
     - Count bytes
     - Cost = bytes × (4 + 16×fraction_zeros) / 8 [wei]
   
   - **Blobs Mode**:
     - Serialize ExecutionTraceV1
     - Cost = bytes × 0.08 [wei]
   
   - **Off-Chain Mode**:
     - Cost = 0 (assume paid externally)
   
   - **Cost per TX**: DA_cost / batch_size [wei/tx]

4. **Repeat** 10 times with different randomized workloads

---

## Data Collection

### Output Structure

```
results/
├── raw_data/
│   ├── experiment_20250505_001.json    # Batch size 10, FIFO, Calldata, run 1
│   ├── experiment_20250505_002.json    # Batch size 10, FIFO, Calldata, run 2
│   ├── ...
│   └── experiment_20250505_300.json    # Batch size 1000, Fee-Priority, Off-Chain, run 10
│
├── aggregated/
│   ├── batch_10_fifo_calldata.csv      # Mean, stddev, quantiles
│   ├── batch_10_fifo_blobs.csv
│   ├── ...
│   └── summary_stats.csv               # All 30 configs
│
├── plots/
│   ├── throughput_vs_batch_size.png
│   ├── latency_vs_batch_size.png
│   ├── cost_vs_batch_size.png
│   ├── pareto_3d.png                   # 3D Pareto frontier
│   ├── pareto_policy_comparison.png    # FIFO vs Fee-Priority
│   └── pareto_da_comparison.png        # Calldata vs Blobs vs Off-Chain
│
└── reports/
    └── factorial_experiment_report.md
```

### JSON Schema (experiment_*.json)

```json
{
  "metadata": {
    "experiment_id": "20250505_001",
    "timestamp": "2025-05-05T10:30:00Z",
    "executor_version": "dev",
    "prover_backend": "risc0"
  },
  "configuration": {
    "batch_size": 10,
    "sequencing_policy": "FIFO",
    "da_mode": "calldata",
    "run_number": 1
  },
  "workload": {
    "transaction_count": 10,
    "included_transactions": 10,
    "rejected_transactions": 0,
    "state_diffs_count": 20
  },
  "timing": {
    "batch_start_ms": 1000000,
    "sequencer_publish_ms": 1000100,
    "executor_receive_ms": 1000150,
    "execution_complete_ms": 1000450,
    "trace_persisted_ms": 1000500,
    "proof_start_ms": 1000500,
    "proof_complete_ms": 1002500,
    "batch_published_ms": 1002600
  },
  "metrics": {
    "throughput_txs_per_sec": 4.54,
    "executor_latency_ms": 300,
    "proof_latency_ms": 2000,
    "e2e_latency_ms": 2500,
    "memory_peak_mb": 85
  },
  "artifacts": {
    "proof_bytes": 4096,
    "journal_bytes": 512,
    "trace_bytes": 8192
  },
  "cost": {
    "da_mode": "calldata",
    "da_cost_wei": 50000000,
    "cost_per_tx_wei": 5000000,
    "gas_equiv": 1250
  }
}
```

---

## Analysis & Pareto Frontier

### Step 1: Data Aggregation

For each configuration (batch_size, policy, da_mode), compute:

```python
# For 10 runs:
throughput_mean = mean(runs[*].metrics.throughput_txs_per_sec)
throughput_std = std(runs[*].metrics.throughput_txs_per_sec)

latency_mean = mean(runs[*].metrics.e2e_latency_ms)
latency_p99 = percentile(runs[*].metrics.e2e_latency_ms, 99)

cost_per_tx_mean = mean(runs[*].cost.cost_per_tx_wei)
cost_per_tx_std = std(runs[*].cost.cost_per_tx_wei)
```

### Step 2: Pareto Frontier Construction

Objective: **Maximize throughput, Minimize latency, Minimize cost**

```
For each point (throughput, latency, cost):
  dominated = ∃ other point where:
    throughput_other ≥ throughput AND
    latency_other ≤ latency AND
    cost_other ≤ cost AND
    at least one strict inequality
  
  if not dominated: add to frontier
```

### Step 3: Pareto Analysis

Extract insights:

1. **Best Throughput**: Which (batch_size, policy, da_mode) achieves max txs/sec?
2. **Best Latency**: Which achieves min p99 latency?
3. **Best Cost Efficiency**: Which achieves min cost/tx?
4. **Trade-off Curves**:
   - Throughput vs Latency
   - Throughput vs Cost
   - Latency vs Cost

5. **Policy Impact**: 
   - How much does Fee-Priority increase/decrease throughput, latency, cost vs FIFO?

6. **DA Mode Impact**:
   - How much does Blobs reduce cost vs Calldata?
   - How much does Off-Chain reduce latency?

### Step 4: Recommendations

Provide decision matrix:

```
┌──────────────────────────────────────────────────────────────┐
│ RECOMMENDATION MATRIX                                        │
├──────────────────────────────────────────────────────────────┤
│ If optimizing for:    │ Recommended Config                   │
├──────────────────────────────────────────────────────────────┤
│ Throughput            │ Batch: 1000, Policy: Fee-Prio,      │
│                       │ DA: Off-Chain (5000 txs/sec)         │
├──────────────────────────────────────────────────────────────┤
│ Low Latency           │ Batch: 10, Policy: FIFO,            │
│                       │ DA: Off-Chain (450ms p99)            │
├──────────────────────────────────────────────────────────────┤
│ Low Cost              │ Batch: 500, Policy: FIFO,           │
│                       │ DA: Blobs (5 wei/tx)                 │
├──────────────────────────────────────────────────────────────┤
│ Balanced              │ Batch: 100, Policy: FIFO,           │
│ (Pareto middle)       │ DA: Blobs (1000 txs/sec, 800ms, 10) │
└──────────────────────────────────────────────────────────────┘
```

---

## Visualization

### 1. 3D Pareto Surface Plot
```
Z-axis: Cost (wei/tx)
Y-axis: Latency (ms)
X-axis: Throughput (txs/sec)

Pareto frontier shown as colored surface
Color gradient: Blue (best) to Red (worst) on dominated regions
```

### 2. Throughput vs Batch Size
```
X: Batch Size (10, 50, 100, 500, 1000)
Y: Throughput (txs/sec)
Lines: FIFO/Fee-Priority, DA modes as separate traces
Legend: Shows improvement from larger batches
```

### 3. Latency Distribution (Box Plots)
```
For each batch size:
  Box plot of p10, p50 (median), p90, p99 latencies
  Separate subplots for each policy/DA mode
```

### 4. Cost Efficiency (Scatter Plot)
```
X: Batch Size
Y: Cost per TX (wei)
Size of bubble: Throughput
Color: Policy (blue=FIFO, orange=Fee-Prio)
Shape: DA mode (circle=Calldata, square=Blobs, triangle=Off-Chain)
```

### 5. Sensitivity Heatmap
```
Rows: Batch sizes
Columns: (Policy, DA Mode) combinations
Cells: Throughput (color-coded)
Shows which configs are most sensitive to batch size
```

---

## Execution Pipeline

### Phase 1: Setup & Validation (1 week)
- [ ] Deploy executor & prover to test harness
- [ ] Verify component tests all pass (100%)
- [ ] Create benchmark fixtures (clean state snapshots)
- [ ] Set up monitoring & profiling tools

### Phase 2: Baseline Measurements (1 week)
- [ ] Run 3 baseline configs (10, 100, 1000 txs × FIFO × Calldata)
- [ ] Establish timing variance baseline
- [ ] Identify potential bottlenecks
- [ ] Adjust experiment design if needed

### Phase 3: Full Factorial Run (2-3 weeks)
- [ ] Execute all 30 configurations
- [ ] 10 runs per configuration = 300 total experiments
- [ ] Monitor for anomalies/failures
- [ ] Collect raw data continuously

### Phase 4: Analysis & Reporting (1 week)
- [ ] Aggregate data into summary statistics
- [ ] Construct Pareto frontier
- [ ] Generate plots and visualizations
- [ ] Write final report with recommendations

---

## Success Criteria

- [ ] All 300 experiments complete without errors
- [ ] No > 5% failure rate in any configuration
- [ ] Measurements show expected trends (latency increases with batch size, cost decreases)
- [ ] Pareto frontier identifies clear trade-offs
- [ ] Statistical significance (stddev < 10% of mean) achieved
- [ ] Recommendations are actionable and well-justified
- [ ] Report generated with full traceability to raw data

---

## Expected Insights

### Hypothesis 1: Batch Size Impact
Larger batches → Higher throughput (amortize proof cost), Higher latency (tx waits longer for batch)

### Hypothesis 2: Policy Impact
Fee-Priority may reorder txs, increasing executor work. Off-chain ordering may be faster.

### Hypothesis 3: DA Mode Impact
Off-Chain vastly cheaper & faster for DA layer, but requires trusted sequencer.
Blobs cheaper than Calldata but subject to EIP-4844 blob price volatility.

### Hypothesis 4: Optimal Operating Point
Pareto frontier will show 3-5 "sweet spots" depending on application constraints.

---

## References

- Factorial Design: https://en.wikipedia.org/wiki/Factorial_experiment
- Pareto Efficiency: https://en.wikipedia.org/wiki/Pareto_efficiency
- EIP-4844 (Proto-Danksharding): https://eips.ethereum.org/EIPS/eip-4844

