# RollupX — Data Tools

Analysis, statistics, and visualisation pipeline for RollupX benchmark results.

## Pipeline

```
benchmark-suite/metrics/       ← raw JSON / JSONL / CSV from each run
        │
        ▼
aggregate.py   →   metrics/all_results.csv
        │
        ├──► stats.py          →   metrics/stats_summary.csv
        │                           metrics/sensitivity_matrix.csv
        │
        ├──► plots/
        │      ├── pareto_frontier.py  →  figures/pareto_*.png
        │      ├── throughput_bar.py   →  figures/throughput_*.png
        │      ├── latency_cdf.py      →  figures/latency_cdf_*.png
        │      ├── latency_boxplot.py  →  figures/latency_boxplot_*.png
        │      ├── fairness.py         →  figures/fairness_*.png, starvation.png
        │      ├── cost_heatmap.py     →  figures/cost_heatmap_*.png
        │      └── sensitivity.py      →  figures/sensitivity_*.png
        │
        └──► report/generate_md.py   →  thesis_summary.md
```

## Quick start

```bash
# 1. Install dependencies
pip install pandas matplotlib

# 2. Aggregate all runs
python aggregate.py --metrics_root ../benchmark-suite/metrics

# 3. Compute statistics
python stats.py --input metrics/all_results.csv

# 4. Generate all plots
python plots/pareto_frontier.py --input metrics/all_results.csv
python plots/throughput_bar.py  --input metrics/all_results.csv
python plots/latency_cdf.py     --metrics_root ../benchmark-suite/metrics
python plots/latency_boxplot.py --input metrics/all_results.csv
python plots/fairness.py        --input metrics/all_results.csv
python plots/cost_heatmap.py    --input metrics/all_results.csv
python plots/sensitivity.py     --input metrics/all_results.csv

# 5. Generate thesis summary
python report/generate_md.py \
  --input  metrics/all_results.csv \
  --stats  metrics/stats_summary.csv \
  --output thesis_summary.md

# 6. Run unit tests
python -m pytest tests/ -v
```

## Key outputs

| File | Description |
|------|-------------|
| `metrics/all_results.csv` | One row per run, all metrics columns |
| `metrics/stats_summary.csv` | Per-experiment: mean±std, p50/p95/p99, CI95, Δbaseline |
| `metrics/sensitivity_matrix.csv` | % change vs baseline per experiment × metric |
| `figures/pareto_cost_latency.png` | Main Pareto frontier plot |
| `figures/sensitivity_heatmap.png` | Factor × metric sensitivity |
| `figures/fairness_jains.png` | Fairness index by experiment |
| `thesis_summary.md` | Auto-generated thesis section |

## Column reference (`all_results.csv`)

### Throughput
- `tps_offered` — offered load from generator
- `tps_accepted` — accepted by sequencer
- `tps_committed` — committed to sealed batches

### Latency
- `avg_l2_l1_ms`, `p50_l2_l1_ms`, `p95_l2_l1_ms`, `p99_l2_l1_ms`
- `avg_prove_ms`, `p50_prove_ms`, `p95_prove_ms`

### Cost
- `avg_gas_per_tx`, `avg_gas_per_batch`, `avg_gas_saved`
- `avg_calldata_bytes`, `avg_compressed_bytes`, `avg_comp_ratio`

### Reliability
- `failed_batches`, `total_retries`

### Fairness
- `jains_fairness` — Jain's Fairness Index (0–1, higher is fairer)
- `starvation_count` — txs waiting > 3× mean latency
- `p95_latency_typeA_ms`, `p95_latency_typeB_ms`, `p95_latency_typeC_ms`
