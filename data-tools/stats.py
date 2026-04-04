"""
stats.py — Per-factor statistical summary of RollupX benchmark results.

Reads:  metrics/all_results.csv
Writes: metrics/stats_summary.csv
        metrics/stats_summary.txt  (human-readable table)

For each factor group, reports:
  n_valid, n_failed, mean±std, p50, p95, p99, 95% CI,
  normalized delta vs baseline for every key metric.
"""

import argparse
import math
import os
import warnings

import pandas as pd

warnings.filterwarnings("ignore", category=RuntimeWarning)


# ── which column is the grouping factor for each experiment family ────────────
FACTOR_COL_MAP = {
    "batch_size": "batch_size",
    "timeout":    "timeout_ms",
    "policy":     "policy",
    "da_mode":    "da_mode",
    "prover":     "prover",
    "rate":       "rate_tps",
    "tx_mix":     "tx_mix",
    "baseline":   None,
}

# Metrics to analyse
METRICS = [
    ("tps_offered",          "TPS Offered"),
    ("tps_accepted",         "TPS Accepted"),
    ("tps_committed",        "TPS Committed"),
    ("avg_prove_ms",         "Avg Prove (ms)"),
    ("p95_prove_ms",         "P95 Prove (ms)"),
    ("avg_l2_l1_ms",         "Avg L2→L1 (ms)"),
    ("p50_l2_l1_ms",         "P50 L2→L1 (ms)"),
    ("p95_l2_l1_ms",         "P95 L2→L1 (ms)"),
    ("p99_l2_l1_ms",         "P99 L2→L1 (ms)"),
    ("avg_gas_per_tx",       "Avg Gas/tx"),
    ("avg_gas_saved",        "Avg Gas Saved"),
    ("avg_comp_ratio",       "Avg Comp Ratio"),
    ("avg_calldata_bytes",   "Avg Calldata (B)"),
    ("avg_compressed_bytes", "Avg Compressed (B)"),
    ("total_retries",        "Total Retries"),
    ("failed_batches",       "Failed Batches"),
    ("jains_fairness",       "Jain's Fairness"),
    ("starvation_count",     "Starvation Count"),
    ("p95_latency_typeA_ms", "P95 Lat Type-A (ms)"),
    ("p95_latency_typeB_ms", "P95 Lat Type-B (ms)"),
    ("p95_latency_typeC_ms", "P95 Lat Type-C (ms)"),
]


# ── statistics helpers ────────────────────────────────────────────────────────

_T_CRIT_95 = {
    1: 12.706, 2: 4.303, 3: 3.182, 4: 2.776, 5: 2.571,
    6: 2.447,  7: 2.365, 8: 2.306, 9: 2.262, 10: 2.228,
    15: 2.131, 20: 2.086, 25: 2.060, 29: 2.045,
}

def _ci95(series: pd.Series) -> float:
    """95% confidence interval half-width using t-distribution."""
    n = series.dropna().count()
    if n < 2:
        return float("nan")
    df_val = n - 1
    t = _T_CRIT_95.get(df_val, 1.96 if n >= 30 else _T_CRIT_95.get(10, 2.228))
    return t * series.std(ddof=1) / math.sqrt(n)


def _percentile(series: pd.Series, pct: float) -> float:
    vals = series.dropna()
    if vals.empty:
        return float("nan")
    return float(vals.quantile(pct / 100))


def _delta_pct(value: float, baseline: float) -> str:
    if baseline == 0 or math.isnan(baseline) or math.isnan(value):
        return "n/a"
    d = (value - baseline) / baseline * 100
    sign = "+" if d >= 0 else ""
    return f"{sign}{d:.1f}%"


# ── main summary function ─────────────────────────────────────────────────────

def compute_stats(df: pd.DataFrame, baseline_df: pd.DataFrame) -> pd.DataFrame:
    """
    Compute per-experiment-group statistics for all metrics.
    Returns a tidy DataFrame with one row per (experiment_id, metric).
    """
    rows = []

    # compute baseline values for delta calculation
    baseline_means: dict[str, float] = {}
    if not baseline_df.empty:
        for col, _ in METRICS:
            if col in baseline_df.columns:
                baseline_means[col] = baseline_df[col].mean()

    for exp_id, group in df.groupby("experiment_id"):
        n_valid  = len(group)
        n_failed = group["run_status"].ne("pass").sum() if "run_status" in group.columns else 0

        for col, label in METRICS:
            if col not in group.columns:
                continue
            series = pd.to_numeric(group[col], errors="coerce").dropna()
            if series.empty:
                continue

            mean_val = series.mean()
            std_val  = series.std(ddof=1) if len(series) > 1 else float("nan")
            p50_val  = _percentile(series, 50)
            p95_val  = _percentile(series, 95)
            p99_val  = _percentile(series, 99)
            ci       = _ci95(series)
            delta    = _delta_pct(mean_val, baseline_means.get(col, float("nan")))

            rows.append({
                "experiment_id": exp_id,
                "metric":        label,
                "metric_col":    col,
                "n_valid":       n_valid,
                "n_failed":      int(n_failed),
                "mean":          round(mean_val, 4),
                "std":           round(std_val, 4),
                "p50":           round(p50_val, 4),
                "p95":           round(p95_val, 4),
                "p99":           round(p99_val, 4),
                "ci95_half":     round(ci, 4),
                "delta_vs_baseline": delta,
            })

    return pd.DataFrame(rows)


def print_table(stats_df: pd.DataFrame, metric_col: str):
    """Print a human-readable comparison table for one metric."""
    sub = stats_df[stats_df["metric_col"] == metric_col].copy()
    if sub.empty:
        return

    label = sub["metric"].iloc[0]
    print(f"\n── {label} ──")
    print(
        f"{'Experiment':<20} {'n':>4} {'mean':>10} {'±std':>10} "
        f"{'p50':>10} {'p95':>10} {'p99':>10} {'CI95±':>10} {'Δbaseline':>12}"
    )
    print("-" * 100)
    for _, row in sub.iterrows():
        std_str = f"±{row['std']:.2f}" if not math.isnan(row["std"]) else "  n/a"
        ci_str  = f"{row['ci95_half']:.2f}" if not math.isnan(row["ci95_half"]) else "n/a"
        print(
            f"{str(row['experiment_id']):<20} "
            f"{int(row['n_valid']):>4} "
            f"{row['mean']:>10.2f} "
            f"{std_str:>10} "
            f"{row['p50']:>10.2f} "
            f"{row['p95']:>10.2f} "
            f"{row['p99']:>10.2f} "
            f"{ci_str:>10} "
            f"{str(row['delta_vs_baseline']):>12}"
        )


# ── ranking helpers ───────────────────────────────────────────────────────────

def _top3(stats_df: pd.DataFrame, metric_col: str, ascending: bool = True, label: str = "") -> str:
    sub = stats_df[stats_df["metric_col"] == metric_col][["experiment_id", "mean"]].dropna()
    if sub.empty:
        return f"No data for {metric_col}"
    top = sub.sort_values("mean", ascending=ascending).head(3)
    lines = [f"  Top 3 by {label or metric_col} ({'lower' if ascending else 'higher'} is better):"]
    for rank, (_, r) in enumerate(top.iterrows(), 1):
        lines.append(f"    {rank}. {r['experiment_id']}  mean={r['mean']:.2f}")
    return "\n".join(lines)


# ── CLI ───────────────────────────────────────────────────────────────────────

def main():
    p = argparse.ArgumentParser(description="RollupX benchmark statistics")
    p.add_argument("--input",  default="metrics/all_results.csv",
                   help="Aggregated CSV from aggregate.py")
    p.add_argument("--output", default="metrics/stats_summary.csv",
                   help="Output stats CSV")
    p.add_argument("--print_metrics", nargs="*",
                   default=["avg_l2_l1_ms", "avg_prove_ms", "avg_gas_per_tx", "jains_fairness"],
                   help="Metric columns to print as tables")
    args = p.parse_args()

    if not os.path.exists(args.input):
        print(f"[stats] Input not found: {args.input}")
        print("        Run aggregate.py first.")
        return

    df = pd.read_csv(args.input)
    print(f"[stats] Loaded {len(df)} rows from {args.input}")

    # ── baseline subset ───────────────────────────────────────────────────────
    baseline_df = df[df["experiment_id"] == "baseline"]
    if baseline_df.empty:
        print("[stats] Warning: no 'baseline' experiment found — delta columns will be n/a")

    # ── compute stats ─────────────────────────────────────────────────────────
    stats_df = compute_stats(df, baseline_df)

    os.makedirs(os.path.dirname(args.output) if os.path.dirname(args.output) else ".", exist_ok=True)
    stats_df.to_csv(args.output, index=False)
    print(f"[stats] written → {args.output}")

    # ── print selected tables ─────────────────────────────────────────────────
    for col in (args.print_metrics or []):
        print_table(stats_df, col)

    # ── rankings ──────────────────────────────────────────────────────────────
    print("\n" + "=" * 60)
    print("Rankings")
    print("=" * 60)
    print(_top3(stats_df, "tps_committed",  ascending=False, label="TPS Committed"))
    print(_top3(stats_df, "avg_l2_l1_ms",   ascending=True,  label="Avg L2→L1 Latency (ms)"))
    print(_top3(stats_df, "avg_gas_per_tx",  ascending=True,  label="Avg Gas/tx"))
    print(_top3(stats_df, "jains_fairness",  ascending=False, label="Jain's Fairness Index"))

    # ── validity report ───────────────────────────────────────────────────────
    validity = stats_df.groupby("experiment_id")[["n_valid", "n_failed"]].first()
    low = validity[validity["n_valid"] < 3]
    if not low.empty:
        print("\n[WARN] Experiments with < 3 valid runs (low statistical confidence):")
        for exp_id in low.index:
            r = validity.loc[exp_id]
            print(f"  {exp_id}: n_valid={int(r['n_valid'])}  n_failed={int(r['n_failed'])}")


if __name__ == "__main__":
    main()
