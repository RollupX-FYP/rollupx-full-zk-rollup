# RollupX — Benchmark Suite & Data-Tools Research Plan
> **Scope:** `benchmark-suite/` and `data-tools/` only.  
> Sequencer, Executor, Prover, Submitter, and Smart Contracts are complete.

---

## 1. Goal of the Benchmark Suite

Produce a **reproducible, statistically rigorous empirical evaluation** of a modular ZK-Rollup
prototype that generates actionable guidance on how batch size, timeout, scheduling policy,
data-availability mode, proof backend, input rate, and transaction heterogeneity interact to
shape throughput, latency, and cost.

---

## 2. Research Questions (from Proposal)

| RQ | Question |
|----|----------|
| RQ1 | How do batch size & frequency, sequencing policies, and DA modes affect end-to-end performance (TPS, latency, gas/tx, proof time)? |
| RQ2 | To what extent can benchmark-driven tuning improve the scalability of a modular ZK-Rollup? |
| RQ3 | Which batching policy dominates under heterogeneous transaction workloads? |

## 3. Hypotheses

| ID | Hypothesis |
|----|------------|
| H1 | Larger batch sizes reduce per-tx cost but increase latency |
| H2 | Fee-Priority scheduling improves throughput but degrades fairness |
| H3 | Blob DA (EIP-4844) reduces gas cost vs calldata by ≥50% |
| H4 | Agnostic batching becomes unstable as heavy-tx fraction exceeds 30% |
| H5 | Transaction-aware policies strictly dominate naive batching above a heterogeneity threshold |

---

## 4. Experimental Design

### 4.1 Controlled Variables (held constant across all experiments)

| Variable | Fixed Value |
|----------|-------------|
| Sequencer host | localhost |
| L1 endpoint | Sepolia testnet (Infura) |
| Experiment duration | 120 s (after 15 s warm-up) |
| Account funding | Genesis allocation via dev_mode |
| Number of repeats | 5 per configuration |
| Warm-up runs | 1 (discarded) |
| Random seeds | 42, 43, 44, 45, 46 (per repeat) |

### 4.2 Independent Variables (Experimental Factors)

| Factor | ID | Levels |
|--------|----|--------|
| Batch size | F1 | 10, 25, 50 *(baseline)*, 100, 200 |
| Batch timeout | F2 | 500, 1000, 2500, 5000 *(baseline)*, 10000 ms |
| Scheduling policy | F3 | FCFS *(baseline)*, FeePriority, TimeBoost, FairBFT |
| DA mode | F4 | calldata *(baseline)*, blob, offchain |
| Proof backend | F5 | groth16 *(baseline)*, plonk *(where supported)* |
| Input rate | F6 | 5, 10 *(baseline)*, 20, 50 TPS |
| Tx heterogeneity | F7 | balanced *(baseline)*, light, heavy |

**Approach:** One-factor-at-a-time (OFAT). All other factors are at baseline level while one is varied.

### 4.3 Dependent Variables (Metrics)

#### Throughput
| Metric | Description |
|--------|-------------|
| `tps_offered` | Input rate from generator (transactions/s) |
| `tps_accepted` | Rate accepted into tx pool (after validation) |
| `tps_committed` | Rate committed to sealed batches |
| `tps_finalized` | Rate confirmed on L1 |

#### Latency
| Metric | Description |
|--------|-------------|
| `latency_submit_ms` | Tx submit → sequencer acceptance |
| `latency_batch_ms` | Batch sealed → proof ready |
| `latency_proof_ms` | Proof generation time |
| `latency_l2_l1_ms` | Batch sealed → L1 confirmed |
| `latency_e2e_ms` | Tx submit → L1 confirmed (total) |

#### Cost
| Metric | Description |
|--------|-------------|
| `gas_per_batch` | Total L1 gas used per batch |
| `gas_per_tx` | Gas amortized per transaction |
| `calldata_bytes` | Raw DA payload size |
| `compressed_bytes` | Post-compression DA payload |
| `compression_ratio` | calldata_bytes / compressed_bytes |
| `gas_saved` | Gas vs uncompressed baseline |
| `proof_verify_gas` | L1 verification cost |

#### Reliability
| Metric | Description |
|--------|-------------|
| `failed_batches` | Count of batches that failed submission |
| `retries` | Number of resubmission attempts |
| `dropped_txs` | Transactions lost due to timeout or error |
| `timeout_events` | Batch timeouts triggered |

#### Fairness
| Metric | Description |
|--------|-------------|
| `latency_p95_typeA` | P95 latency for Type-A (Light) txs |
| `latency_p95_typeB` | P95 latency for Type-B (Medium) txs |
| `latency_p95_typeC` | P95 latency for Type-C (Heavy) txs |
| `starvation_count` | Txs waiting > 3× average latency |
| `jains_fairness` | Jain's Fairness Index across all tx classes |

---

## 5. Transaction Types (Workload Design)

| Type | Name | Calldata Size | Gas Profile | DA Footprint | To Address |
|------|------|--------------|-------------|--------------|------------|
| A | Light transfer | ~100 bytes | 21,000 gas | Low | `0x02...02` |
| B | Medium ERC-20 swap | ~300 bytes | 65,000 gas | Moderate | `0x03...03` |
| C | Heavy contract call | ~600 bytes | 200,000 gas | High | `0x04...04` |

Mix presets:

| Preset | Type A | Type B | Type C |
|--------|--------|--------|--------|
| `balanced` | 70% | 20% | 10% |
| `light` | 95% | 4% | 1% |
| `heavy` | 20% | 30% | 50% |

Types differ in calldata size (payload padding), gas_limit, and gas_price tier — not just `to` address — to meaningfully stress the prover and DA layer.

---

## 6. Measurement Protocol

1. **Warm-up:** Run 1 un-timed warm-up run (seed 0) per configuration; discard metrics.
2. **Timed runs:** Run 5 timed runs per configuration (seeds 42–46).
3. **Collection:** Each run emits JSON metrics files (see Section 10).
4. **Aggregation:** `aggregate.py` merges all runs per experiment; `stats.py` computes statistics.
5. **Analysis:** Pareto frontiers, factor sensitivity plots, fairness plots.
6. **Baseline delta:** Every result table includes a column showing % change vs baseline.

---

## 7. Experiment Matrix (`config/experiments.toml`)

### Naming convention
```
{factor_code}_{level_value}_r{repeat:02d}
e.g.  bs_050_r01  bs_050_r02  ...  bs_050_r05
```

One true baseline: **`baseline`** (batch_size=50, timeout=5000, policy=FCFS, da=calldata, prover=groth16, rate=10, mix=balanced).

Factor groups reference this baseline; duplicate aliases are removed.

```toml
[baseline]
batch_size    = 50
timeout_ms    = 5000
policy        = "FCFS"
da_mode       = "calldata"
prover        = "groth16"
rate_tps      = 10
duration_s    = 120
warmup_s      = 15
tx_mix        = "balanced"
repeats       = 5
seeds         = [42, 43, 44, 45, 46]

# ── F1: Batch Size ────────────────────────────────────────────────────────────
[[experiments]]
factor = "batch_size"; id = "bs_010"; batch_size = 10
[[experiments]]
factor = "batch_size"; id = "bs_025"; batch_size = 25
# bs_050 == baseline — no separate row needed
[[experiments]]
factor = "batch_size"; id = "bs_100"; batch_size = 100
[[experiments]]
factor = "batch_size"; id = "bs_200"; batch_size = 200

# ── F2: Timeout ───────────────────────────────────────────────────────────────
[[experiments]]
factor = "timeout"; id = "to_0500"; timeout_ms = 500
[[experiments]]
factor = "timeout"; id = "to_1000"; timeout_ms = 1000
[[experiments]]
factor = "timeout"; id = "to_2500"; timeout_ms = 2500
[[experiments]]
factor = "timeout"; id = "to_10000"; timeout_ms = 10000

# ── F3: Scheduling Policy ─────────────────────────────────────────────────────
[[experiments]]
factor = "policy"; id = "pol_fee";       policy = "FeePriority"
[[experiments]]
factor = "policy"; id = "pol_timeboost"; policy = "TimeBoost"
[[experiments]]
factor = "policy"; id = "pol_fairbft";   policy = "FairBFT"

# ── F4: DA Mode ───────────────────────────────────────────────────────────────
[[experiments]]
factor = "da_mode"; id = "da_blob";     da_mode = "blob"
[[experiments]]
factor = "da_mode"; id = "da_offchain"; da_mode = "offchain"

# ── F5: Proof Backend ─────────────────────────────────────────────────────────
[[experiments]]
factor = "prover"; id = "pv_plonk"; prover = "plonk"
# Note: halo2 deferred — not yet integrated in prover pipeline

# ── F6: Input Rate ────────────────────────────────────────────────────────────
[[experiments]]
factor = "rate"; id = "tps_005"; rate_tps = 5
[[experiments]]
factor = "rate"; id = "tps_020"; rate_tps = 20
[[experiments]]
factor = "rate"; id = "tps_050"; rate_tps = 50

# ── F7: Tx Heterogeneity ──────────────────────────────────────────────────────
[[experiments]]
factor = "tx_mix"; id = "mix_light"; tx_mix = "light"
[[experiments]]
factor = "tx_mix"; id = "mix_heavy"; tx_mix = "heavy"
```

Total unique configurations: **18 + 1 baseline = 19**.  
With 5 repeats each: **100 timed runs** + 19 warm-ups = **119 total runs**.

---

## 8. Repository Layout

```
benchmark-suite/
├── PLAN.md
├── config/
│   ├── experiments.toml              ← master matrix
│   ├── sequencer.template.toml       ← envsubst template
│   └── workloads/
│       ├── balanced.toml
│       ├── light.toml
│       └── heavy.toml
├── scripts/
│   ├── run_experiment.sh             ← full lifecycle for one experiment
│   ├── run_matrix.sh                 ← sweep all rows
│   ├── wait_for_sequencer.sh         ← health-check with timeout
│   └── collect_env.sh                ← snapshot hw/sw metadata
├── workload/
│   ├── poisson_generator.py          ← extended with --tx_mix, --seed, --warmup
│   └── tx_types.py                   ← Type A / B / C factories
└── README.md

data-tools/
├── pareto.py                         ← extended (load, analyze, Pareto frontier)
├── aggregate.py                      ← merge all metrics/ dirs → all_results.csv
├── stats.py                          ← per-factor stats tables (mean±std, p50/p95/p99, CI)
├── plots/
│   ├── pareto_frontier.py            ← non-dominated scatter + frontier line
│   ├── throughput_bar.py             ← TPS bar (offered vs accepted vs committed)
│   ├── latency_cdf.py                ← per-batch latency CDF per factor level
│   ├── latency_boxplot.py            ← boxplot per factor
│   ├── fairness.py                   ← Jain's index + per-class P95
│   ├── cost_heatmap.py               ← gas/tx × DA mode heatmap
│   └── sensitivity.py                ← normalized delta vs baseline per factor
├── report/
│   └── generate_md.py                ← full thesis_summary.md
├── tests/
│   └── test_aggregate.py             ← unit tests
└── README.md
```

---

## 9. Failure & Retry Handling

- Every experiment run writes a `run_status.json` (pass / fail / partial).
- `run_matrix.sh` logs all failures and continues; failed runs are excluded from analysis with a warning.
- `stats.py` reports `n_valid` and `n_failed` per configuration.
- A `sleep 5` completion guard is replaced with a poll loop checking submitter idle signal and expected batch count.

---

## 10. Metrics JSON Contract (do not break)

```
metrics/<exp_id>/<run_id>/
├── workload_<exp_id>.json       ← poisson_generator writes this
├── executor_<exp_id>.json       ← executor writes this
├── submitter_metrics.json       ← submitter appends one line per batch (JSONL)
├── run_metadata.json            ← collect_env.sh + run_experiment.sh writes this
└── run_status.json              ← pass/fail/partial
```

### `run_metadata.json` schema
```json
{
  "experiment_id": "bs_100",
  "run_id": "bs_100_r03",
  "seed": 44,
  "git_commit": "abc1234",
  "timestamp_start": "2026-03-01T10:00:00Z",
  "timestamp_end":   "2026-03-01T10:02:15Z",
  "machine": { "cpu": "...", "ram_gb": 16, "os": "Ubuntu 24.04" },
  "python_version": "3.12.0",
  "rust_version": "1.78.0",
  "config_snapshot": { ... }
}
```

---

## 11. Threats to Validity

| Threat | Mitigation |
|--------|------------|
| Synthetic workload bias | Three tx classes with realistic gas/calldata profiles |
| Dev-mode mock signature | Noted as limitation; signature content bypassed but structure valid |
| Single-machine bottleneck | Document CPU/RAM; note network I/O is local |
| Sepolia congestion | Fixed RPC endpoint; retry logic in submitter |
| Low repeat count | 5 repeats + CI reporting |
| Blob infra incomplete | Blob mode tested with local archiver stub; noted as limitation |
| Proof backend gap | Halo2 deferred; noted explicitly |

---

## 12. Analysis Outputs

| Output | File | Purpose |
|--------|------|---------|
| Raw merged data | `metrics/all_results.csv` | Input to all tools |
| Stats table | `metrics/stats_summary.csv` | Per-factor mean±std, p50/p95/p99 |
| Pareto frontier | `figures/pareto_cost_latency.png` | RQ2 trade-off |
| Throughput bars | `figures/throughput_by_policy.png` | RQ1 |
| Latency CDF | `figures/latency_cdf.png` | Tail behaviour |
| Latency boxplot | `figures/latency_boxplot.png` | Variance |
| Fairness plot | `figures/fairness_by_policy.png` | RQ3 |
| Cost heatmap | `figures/cost_heatmap.png` | DA × batch_size |
| Sensitivity | `figures/sensitivity.png` | Which factor matters most |
| Thesis summary | `thesis_summary.md` | Auto-generated report |

---

## 13. Implementation Schedule

| Week | Tasks |
|------|-------|
| 1 | `tx_types.py` + extend `poisson_generator.py`; smoke-test 3 manual runs |
| 2 | `run_experiment.sh`, `wait_for_sequencer.sh`, `collect_env.sh`; run F1 manually |
| 3 | `run_matrix.sh` full sweep; `aggregate.py` + `stats.py` |
| 4 | All plots + `generate_md.py`; write threats + analysis narrative |

---

## 14. Environment Variables

| Variable | Used by | Example |
|----------|---------|---------|
| `METRICS_ROOT` | all tools | `metrics/bs_050/bs_050_r01` |
| `SEQ_HOST` | scripts | `localhost` |
| `SEQ_PORT` | scripts | `3000` |
| `RATE_TPS` | run_matrix.sh | `10` |
| `DURATION_S` | scripts | `120` |
| `WARMUP_S` | scripts | `15` |
| `TX_MIX` | generator | `balanced` |
| `PROVER` | sequencer config | `groth16` |
| `MAX_BATCH_SIZE` | sequencer config | `50` |
| `TIMEOUT_MS` | sequencer config | `5000` |
| `POLICY` | sequencer config | `FCFS` |
| `DA_MODE` | submitter config | `calldata` |
| `SEED` | generator | `42` |
| `RUN_ID` | scripts | `bs_050_r01` |

---

## 15. Quick-Start

```bash
# ── Smoke test (workload only) ────────────────────────────────────────────────
cd benchmark-suite
METRICS_ROOT=metrics/smoke python workload/poisson_generator.py \
  --experiment_id smoke --rate 5 --duration 30 --tx_mix balanced --seed 42

# ── Single full experiment ────────────────────────────────────────────────────
bash scripts/run_experiment.sh bs_050 1   # exp_id, repeat_index

# ── Full matrix sweep ─────────────────────────────────────────────────────────
bash scripts/run_matrix.sh

# ── Analyse results ───────────────────────────────────────────────────────────
cd ../data-tools
python aggregate.py --metrics_root ../benchmark-suite/metrics
python stats.py     --input metrics/all_results.csv
python plots/pareto_frontier.py --input metrics/all_results.csv
python plots/fairness.py        --input metrics/all_results.csv
python report/generate_md.py    --input metrics/all_results.csv \
                                 --stats metrics/stats_summary.csv
```
