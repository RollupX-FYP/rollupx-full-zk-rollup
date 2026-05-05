# Complete Test & Experiment Execution Roadmap

## Overview

This document provides a complete roadmap for:
1. **Phase 1**: Verifying executor and prover components work correctly
2. **Phase 2**: Running a comprehensive factorial experiment across three levers
3. **Phase 3**: Analyzing results and plotting Pareto frontier

---

## Phase 1: Component Verification (1-2 days)

### Objective
Ensure executor and prover produce correct outputs before running the factorial experiment.

### Deliverables

| Document | Purpose | Location |
|----------|---------|----------|
| Test Plan | Detailed test specification & requirements | [EXECUTOR_PROVER_TEST_PLAN.md](./EXECUTOR_PROVER_TEST_PLAN.md) |
| Test Execution Guide | How to run tests & interpret results | [TEST_EXECUTION_GUIDE.md](./TEST_EXECUTION_GUIDE.md) |
| Test Code (Executor) | 90 unit/integration tests | `executor/tests/test_*.rs` |
| Test Code (Prover) | 40 unit tests for guest logic | `risc0_prover/tests/test_*.rs` |
| Test Script | Automated test runner | `run_verification_tests.sh` |

### What Gets Tested

#### A. Executor (90 tests)
```
✓ Transaction Validation
  - Signature verification (valid, invalid, unsigned)
  - Nonce validation (acceptance, rejection)
  - Balance validation (acceptance, rejection)
  
✓ State Management
  - In-memory state (read, write, root computation)
  - Persistent state (RocksDB, recovery)
  - Merkle tree updates

✓ Trace Persistence
  - JSON serialization & disk I/O
  - SHA256 hashing & verification
  - Lifecycle tracking (generated → proved → published)

✓ End-to-End Integration
  - Single/multiple tx execution
  - Batch determinism verification
  - Mixed valid/invalid batches
  - Large batch scaling (10-1000 txs)
```

#### B. Prover (40 tests)
```
✓ RISC0 Guest Logic
  - Lightweight SMT state transitions
  - State diff validation
  - Merkle proof verification
  - Root progression determinism
  - Nonce & balance constraint enforcement
  - Edge cases (large values, many accounts)
```

### Test Execution

```bash
# Quick execution
cd /path/to/rollupx-full-zk-rollup
bash run_verification_tests.sh

# Or manual
cd executor
CXXFLAGS='-include cstdint' cargo +nightly-2025-03-19 test \
  --manifest-path executor/src/Cargo.toml \
  -p zksync_state_machine \
  --ignore-rust-version \
  --test-threads=1

cd ../risc0_prover
cargo +nightly test \
  --manifest-path risc0_prover/rollup_core/Cargo.toml \
  -p rollup_core \
  --test-threads=1
```

### Success Criteria (Phase 1)

- [ ] All 90 executor tests pass
- [ ] All 40 prover tests pass
- [ ] No panics or assertion failures
- [ ] Code coverage ≥ 80%
- [ ] All environment variables set correctly
- [ ] Test execution time < 5 minutes

**Expected Result**: ✅ 130/130 tests passing

---

## Phase 2: Factorial Experiment (2-3 weeks)

### Objective
Measure throughput, latency, and cost across all combinations of:
- **Batch Size**: 10, 50, 100, 500, 1000 txs
- **Sequencing Policy**: FIFO vs Fee-Priority
- **DA Mode**: Calldata vs EIP-4844 Blobs vs Off-Chain

### Deliverable

[FACTORIAL_EXPERIMENT_DESIGN.md](./FACTORIAL_EXPERIMENT_DESIGN.md) — Comprehensive experiment specification

### Experiment Matrix

```
5 batch sizes × 2 policies × 3 DA modes = 30 configurations
30 configurations × 10 runs each = 300 total experiments

Combinations:
├─ Batch 10
│  ├─ FIFO + Calldata
│  ├─ FIFO + Blobs
│  ├─ FIFO + Off-Chain
│  ├─ Fee-Priority + Calldata
│  ├─ Fee-Priority + Blobs
│  └─ Fee-Priority + Off-Chain
├─ Batch 50 (same × 6)
├─ Batch 100 (same × 6)
├─ Batch 500 (same × 6)
└─ Batch 1000 (same × 6)
```

### Measurement Points Per Experiment

For each configuration, record:

```
Timing:
  - Batch sequencer publish time
  - Executor receive to complete time
  - Trace persistence time
  - Proof generation time
  - End-to-end latency

Performance:
  - Throughput (txs/sec)
  - Memory peak (MB)
  - CPU usage (%)

Artifacts:
  - Proof size (bytes)
  - Journal size (bytes)
  - Trace size (bytes)

Cost:
  - DA cost in wei (calldata, blobs, or 0 for off-chain)
  - Cost per transaction
```

### Data Output Structure

```
results/
├── raw_data/
│   ├── experiment_001.json   (batch 10, FIFO, Calldata, run 1)
│   ├── experiment_002.json   (batch 10, FIFO, Calldata, run 2)
│   └── ... (298 more)
│
├── aggregated/
│   ├── summary_stats.csv     (mean, stddev for all 30 configs)
│   ├── pareto_frontier.csv   (non-dominated points)
│   └── analysis.md           (insights & observations)
│
└── plots/
    ├── throughput_vs_batch.png
    ├── latency_vs_batch.png
    ├── cost_vs_batch.png
    ├── pareto_3d.png         (3D visualization)
    └── recommendations.png   (decision matrix)
```

### Analysis Output

After experiment completion:

**Key Metrics Generated**:
```json
{
  "best_throughput": {
    "config": "batch_1000, fee_priority, off_chain",
    "value": "5000 txs/sec"
  },
  "best_latency": {
    "config": "batch_10, fifo, off_chain",
    "value": "450 ms p99"
  },
  "best_cost": {
    "config": "batch_500, fifo, blobs",
    "value": "5 wei/tx"
  },
  "pareto_points": 8,
  "policy_impact": {
    "fifo_vs_fee_priority": "-2.3% throughput, +50ms latency"
  },
  "da_mode_impact": {
    "blobs_vs_calldata": "-60% cost",
    "off_chain_vs_blobs": "-65% latency"
  }
}
```

---

## Phase 3: Result Analysis & Pareto Frontier (1 week)

### Deliverable

Final report: `FACTORIAL_EXPERIMENT_REPORT.md` containing:

1. **Executive Summary**
   - Best configurations for each optimization goal
   - Key trade-offs identified
   - Recommendations

2. **Detailed Analysis**
   - Data aggregation & statistics
   - Pareto frontier construction
   - Policy impact analysis
   - DA mode sensitivity analysis

3. **Visualizations**
   - 3D Pareto surface plot
   - Throughput vs latency trade-off curve
   - Latency distributions (box plots)
   - Cost efficiency scatter plot
   - Sensitivity heatmaps

4. **Recommendations**
   - Decision matrix for different use cases
   - Which lever has most impact (batch size? policy? DA mode?)
   - Optimal operating points
   - Deployment guidance

### Example Output

**Pareto Frontier Insight**:
```
PARETO FRONTIER POINTS:

1. (5000 txs/sec, 2500ms latency, 1 wei/tx)
   Config: Batch 1000, Fee-Priority, Off-Chain
   Tradeoff: Max throughput, high latency, free cost
   Best for: Batch processing, cost-sensitive

2. (500 txs/sec, 500ms latency, 10 wei/tx)
   Config: Batch 100, FIFO, Blobs
   Tradeoff: Balanced all metrics
   Best for: General-purpose rollup

3. (100 txs/sec, 250ms latency, 50 wei/tx)
   Config: Batch 10, FIFO, Off-Chain
   Tradeoff: Min latency, high cost per tx
   Best for: Low-latency, high-frequency apps

RECOMMENDATION MATRIX:
┌────────────────────────────────────────────────┐
│ Use Case    │ Best Config        │ Key Metric  │
├────────────────────────────────────────────────┤
│ Exchange   │ Batch 100, FIFO,  │ 500 txs/sec │
│            │ Blobs             │ 800ms p99   │
├────────────────────────────────────────────────┤
│ NFT Market │ Batch 10, FIFO,   │ 250ms p99   │
│            │ Off-Chain         │ 100 txs/sec │
├────────────────────────────────────────────────┤
│ Staking    │ Batch 1000, Fee-  │ 5000 txs/   │
│            │ Priority, Off-    │ sec, 1 wei/ │
│            │ Chain             │ tx          │
└────────────────────────────────────────────────┘
```

---

## Complete Roadmap Timeline

```
Week 1: Component Verification
├─ Day 1-2: Implement & run unit tests
├─ Day 3: Fix any failing tests
├─ Day 4: Achieve 100% test pass rate & coverage ≥ 80%
└─ Day 5: Document baseline metrics

Week 2-4: Factorial Experiment Execution
├─ Week 2: Baseline runs (3 configs, verify setup)
├─ Week 3: Full 30 configurations × 10 runs = 300 experiments
├─ Week 4: Final verification runs, data quality checks
└─ Continuous: Monitor for anomalies, log all failures

Week 5: Analysis & Reporting
├─ Day 1-2: Aggregate data, compute statistics
├─ Day 3: Construct Pareto frontier
├─ Day 4: Generate plots & visualizations
├─ Day 5: Write final report & recommendations
```

---

## Key Documents Summary

| Document | Purpose | Size | Read Time |
|----------|---------|------|-----------|
| [EXECUTOR_PROVER_TEST_PLAN.md](./EXECUTOR_PROVER_TEST_PLAN.md) | Test specification & scope | 4KB | 10 min |
| [TEST_EXECUTION_GUIDE.md](./TEST_EXECUTION_GUIDE.md) | How to run & interpret tests | 8KB | 15 min |
| [FACTORIAL_EXPERIMENT_DESIGN.md](./FACTORIAL_EXPERIMENT_DESIGN.md) | Experiment protocol & analysis plan | 12KB | 25 min |
| Test Implementation (executor/tests/) | 90 unit/integration tests | 15KB | - |
| Test Implementation (prover/tests/) | 40 unit tests | 8KB | - |

---

## Pre-Requisites Checklist

Before starting Phase 1:

- [ ] Rust nightly-2025-03-19 installed
- [ ] RISC0 environment set up
- [ ] RocksDB dev libraries available
- [ ] Executor builds without errors
- [ ] Prover builds without errors
- [ ] Environment variables configured:
  ```bash
  export PROVER_BACKEND=risc0
  export RISC0_HOST_BIN=<path-to-rollup_host>
  export ALLOW_UNSIGNED_USER_TXS=1
  ```

Before starting Phase 2:

- [ ] Phase 1 tests all passing (130/130)
- [ ] Baseline metrics recorded
- [ ] Monitoring infrastructure set up
- [ ] Sufficient disk space for 300 experiments (≈ 50GB)
- [ ] Network connectivity stable (if using remote sequencer/DA)

---

## Success Definition

### Phase 1: ✅ Component Verification
- **130/130 tests passing**
- Code coverage ≥ 80%
- All edge cases handled
- Latency baseline established

### Phase 2: ✅ Factorial Experiment
- **300/300 experiments completed**
- < 5% failure rate
- Measurements show expected trends
- Statistical significance achieved (stddev < 10%)

### Phase 3: ✅ Analysis & Reporting
- **Pareto frontier identified**
- 3+ clear trade-off regions
- Actionable recommendations provided
- Report ready for publication/presentation

---

## Next Actions

**Immediate** (Today):
1. Review test plan: [EXECUTOR_PROVER_TEST_PLAN.md](./EXECUTOR_PROVER_TEST_PLAN.md)
2. Check pre-requisites (Rust, RocksDB, RISC0)
3. Verify executor/prover build successfully

**This Week** (Phase 1):
1. Run component verification tests
2. Achieve 100% pass rate
3. Record baseline metrics

**Next Week** (Phase 2 Setup):
1. Review experiment design: [FACTORIAL_EXPERIMENT_DESIGN.md](./FACTORIAL_EXPERIMENT_DESIGN.md)
2. Set up monitoring & data collection infrastructure
3. Run baseline experiments (3 configurations)

**Following Weeks** (Phase 2 Execution):
1. Execute full 300-experiment suite
2. Collect raw data & metrics
3. Monitor for anomalies

**Week 5+** (Phase 3):
1. Aggregate & analyze data
2. Generate plots & visualizations
3. Write final report with recommendations

---

## References & Resources

- **Test Framework**: Rust's built-in `#[test]` macro with `tokio::test` for async
- **Component Documentation**:
  - [Executor README](./executor/README.md)
  - [Prover README](./risc0_prover/README.md)
  - [System Design](./executor/SYSTEM_DESIGN.md)
- **Theory**:
  - Factorial Experiments: https://en.wikipedia.org/wiki/Factorial_experiment
  - Pareto Efficiency: https://en.wikipedia.org/wiki/Pareto_efficiency
  - EIP-4844: https://eips.ethereum.org/EIPS/eip-4844

---

## Questions & Troubleshooting

**Q: How long will Phase 1 take?**
A: 1-2 days. Tests are quick (< 5 minutes). Debugging any failures adds time.

**Q: How long will Phase 2 take?**
A: 2-3 weeks depending on hardware. 300 experiments, with proof generation being the bottleneck.

**Q: Can I parallelize Phase 2?**
A: Partially. Executor tests can run in parallel, but RISC0 proof generation may be CPU-bound.

**Q: What if tests fail?**
A: See [TEST_EXECUTION_GUIDE.md](./TEST_EXECUTION_GUIDE.md) troubleshooting section. Most failures are environment-related.

**Q: How do I know results are correct?**
A: Verify determinism: same batch → identical traces multiple runs. Verify state transitions mathematically. Check that metrics follow expected trends.

---

## Contact & Support

For issues with:
- **Tests**: Check test failure output carefully, enable `--nocapture`
- **Build**: Verify RocksDB, nightly toolchain, RISC0 setup
- **Interpretation**: Refer to experiment design doc and analysis guide

