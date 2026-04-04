"""
report/generate_md.py — Auto-generate thesis_summary.md from benchmark results.

Reads:
  metrics/all_results.csv      (from aggregate.py)
  metrics/stats_summary.csv    (from stats.py)

Writes:
  thesis_summary.md
"""

import argparse
import math
import os
import sys
from datetime import datetime, timezone

import pandas as pd


# ── helpers ───────────────────────────────────────────────────────────────────

def _fmt(val, decimals: int = 2, suffix: str = "") -> str:
    try:
        f = float(val)
        if math.isnan(f):
            return "n/a"
        return f"{f:.{decimals}f}{suffix}"
    except (TypeError, ValueError):
        return str(val)


def _top_n(df: pd.DataFrame, col: str, n: int = 3, ascending: bool = True) -> pd.DataFrame:
    if col not in df.columns:
        return pd.DataFrame()
    return (
        df.groupby("experiment_id")[col]
        .mean()
        .reset_index()
        .dropna()
        .sort_values(col, ascending=ascending)
        .head(n)
    )


def _delta_str(val: float, base: float, lower_better: bool = True) -> str:
    if base == 0 or math.isnan(base) or math.isnan(val):
        return ""
    delta = (val - base) / abs(base) * 100
    symbol = "▼" if delta < 0 else "▲"
    better = (delta < 0 and lower_better) or (delta > 0 and not lower_better)
    sign = "+" if delta > 0 else ""
    marker = "✓" if better else "✗"
    return f"{marker} {symbol}{sign}{delta:.1f}%"


def _md_table(headers: list[str], rows: list[list[str]]) -> str:
    sep = "|".join(["---"] * len(headers))
    lines = [
        "| " + " | ".join(headers) + " |",
        "| " + sep + " |",
    ]
    for row in rows:
        lines.append("| " + " | ".join(str(c) for c in row) + " |")
    return "\n".join(lines)


# ── section builders ──────────────────────────────────────────────────────────

def section_overview(df: pd.DataFrame, stats_df: pd.DataFrame) -> str:
    n_exp    = df["experiment_id"].nunique()
    n_runs   = len(df)
    n_failed = (df["run_status"] != "pass").sum() if "run_status" in df.columns else 0
    factors  = df["experiment_id"].apply(
        lambda x: x.split("_")[0] if "_" in x else x
    ).nunique()
    gen_ts = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")

    return f"""## 1. Overview

| Item | Value |
|------|-------|
| Generated | {gen_ts} |
| Unique configurations | {n_exp} |
| Total runs | {n_runs} |
| Failed / excluded runs | {n_failed} |
| Metrics columns | {len(df.columns)} |

"""


def section_results_table(df: pd.DataFrame) -> str:
    cols = [
        ("experiment_id",   "Experiment"),
        ("da_mode",         "DA Mode"),
        ("policy",          "Policy"),
        ("batch_size",      "Batch"),
        ("tps_committed",   "TPS Committed"),
        ("avg_l2_l1_ms",    "L2→L1 (ms)"),
        ("p95_l2_l1_ms",    "P95 L2→L1"),
        ("avg_prove_ms",    "Prove (ms)"),
        ("avg_gas_per_tx",  "Gas/tx"),
        ("avg_comp_ratio",  "Comp Ratio"),
        ("jains_fairness",  "Fairness"),
    ]

    available = [(c, h) for c, h in cols if c in df.columns]
    headers   = [h for _, h in available]

    # one row per experiment (mean across repeats)
    numeric_cols = [c for c, _ in available if c != "experiment_id"]
    agg = df.groupby("experiment_id").agg(
        {c: "first" if df[c].dtype == object else "mean" for c, _ in available}
    ).reset_index()

    rows = []
    for _, row in agg.iterrows():
        rows.append([_fmt(row.get(c, ""), decimals=2) for c, _ in available])

    return "## 2. Full Results Table\n\n" + _md_table(headers, rows) + "\n\n"


def section_rankings(df: pd.DataFrame) -> str:
    lines = ["## 3. Rankings\n"]

    categories = [
        ("tps_committed",   "Best Throughput (TPS Committed)",        False),
        ("avg_l2_l1_ms",    "Lowest Latency (Avg L2→L1 ms)",          True),
        ("avg_gas_per_tx",  "Lowest Cost (Avg Gas/tx)",               True),
        ("jains_fairness",  "Best Fairness (Jain's Index)",           False),
        ("avg_prove_ms",    "Fastest Proving (Avg Prove ms)",         True),
        ("avg_comp_ratio",  "Best Compression (Avg Comp Ratio)",      False),
    ]

    for col, title, ascending in categories:
        top = _top_n(df, col, n=3, ascending=ascending)
        if top.empty:
            continue
        lines.append(f"### {title}\n")
        rows = []
        for rank, (_, r) in enumerate(top.iterrows(), 1):
            rows.append([str(rank), r["experiment_id"], _fmt(r[col])])
        lines.append(_md_table(["Rank", "Experiment", "Value"], rows))
        lines.append("")

    return "\n".join(lines) + "\n"


def section_baseline_comparison(df: pd.DataFrame) -> str:
    baseline_df = df[df["experiment_id"] == "baseline"]
    if baseline_df.empty:
        return "## 4. Baseline Comparison\n\n*No baseline experiment found.*\n\n"

    metrics = [
        ("tps_committed",   "TPS Committed",        False),
        ("avg_l2_l1_ms",    "Avg L2→L1 (ms)",       True),
        ("p95_l2_l1_ms",    "P95 L2→L1 (ms)",       True),
        ("avg_prove_ms",    "Avg Prove (ms)",         True),
        ("avg_gas_per_tx",  "Avg Gas/tx",            True),
        ("avg_comp_ratio",  "Avg Comp Ratio",        False),
        ("jains_fairness",  "Jain's Fairness",       False),
        ("starvation_count","Starvation Count",       True),
    ]

    baseline_means = {
        col: pd.to_numeric(baseline_df[col], errors="coerce").mean()
        for col, _, _ in metrics
        if col in baseline_df.columns
    }

    non_baseline = df[df["experiment_id"] != "baseline"]
    agg = non_baseline.groupby("experiment_id")[
        [col for col, _, _ in metrics if col in non_baseline.columns]
    ].mean().reset_index()

    headers = ["Experiment"] + [lbl for _, lbl, _ in metrics if metrics[0][0] in agg.columns or True]
    headers = ["Experiment"] + [
        lbl for col, lbl, _ in metrics if col in agg.columns
    ]

    rows = []
    for _, row in agg.iterrows():
        r = [row["experiment_id"]]
        for col, lbl, lower_better in metrics:
            if col not in agg.columns:
                continue
            val  = row[col]
            base = baseline_means.get(col, float("nan"))
            r.append(f"{_fmt(val)} {_delta_str(val, base, lower_better)}")
        rows.append(r)

    return "## 4. Comparison vs Baseline\n\n" + _md_table(headers, rows) + "\n\n"


def section_hypotheses(df: pd.DataFrame) -> str:
    lines = ["## 5. Hypothesis Assessment\n"]

    hypotheses = [
        (
            "H1",
            "Larger batch sizes reduce per-tx cost but increase latency",
            "See batch_size experiments (bs_*) in the results table. "
            "Compare `avg_gas_per_tx` and `avg_l2_l1_ms` columns.",
        ),
        (
            "H2",
            "Fee-Priority scheduling improves throughput but degrades fairness",
            "Compare `pol_fee` vs `baseline` on `tps_committed` and `jains_fairness`.",
        ),
        (
            "H3",
            "Blob DA (EIP-4844) reduces gas cost vs calldata by ≥50%",
            "Compare `da_blob` vs `baseline` on `avg_gas_per_tx` and `avg_gas_saved`.",
        ),
        (
            "H4",
            "Agnostic batching becomes unstable as heavy-tx fraction exceeds 30%",
            "Compare `mix_heavy` vs `baseline` on `tps_committed` and `failed_batches`.",
        ),
        (
            "H5",
            "Transaction-aware policies strictly dominate naive batching above a heterogeneity threshold",
            "Compare all `pol_*` experiments on `mix_heavy` subset (cross-factor analysis).",
        ),
    ]

    rows = []
    for hid, statement, how in hypotheses:
        rows.append([hid, statement, how])

    lines.append(_md_table(["ID", "Hypothesis", "Evidence location"], rows))
    lines.append("\n*Fill in supported / refuted / inconclusive after analysis.*\n")
    return "\n".join(lines) + "\n"


def section_threats(df: pd.DataFrame) -> str:
    return """## 6. Threats to Validity

| Threat | Type | Mitigation |
|--------|------|------------|
| Synthetic workload bias | Internal | Three tx classes with distinct gas/calldata profiles |
| Dev-mode signatures | Internal | Noted limitation; signature structure valid |
| Single-machine bottleneck | External | CPU/RAM documented in run_metadata.json |
| Sepolia congestion | External | Fixed RPC; submitter retry logic |
| Low repeat count (n=5) | Statistical | CI reported; flag configs with n<3 |
| Blob infra incomplete | Internal | Local archiver stub; noted in each blob experiment |
| Proof backend gap (Halo2 deferred) | Internal | Explicitly excluded from matrix |
| OFAT ignores interactions | Design | Noted; multi-factor cross-analysis deferred |

"""


def section_figures(output_dir: str) -> str:
    figure_map = [
        ("figures/pareto_cost_latency.png",    "Pareto Frontier: Cost vs Latency"),
        ("figures/pareto_throughput_latency.png","Pareto Frontier: Throughput vs Latency"),
        ("figures/pareto_da_comparison.png",   "DA Mode Comparison"),
        ("figures/throughput_by_policy.png",   "Throughput by Scheduling Policy"),
        ("figures/latency_cdf_all.png",        "Latency CDF (all experiments)"),
        ("figures/latency_boxplot_policy.png", "Latency Variance by Policy"),
        ("figures/fairness_jains.png",         "Jain's Fairness Index"),
        ("figures/fairness_per_class.png",     "Per-Class P95 Latency"),
        ("figures/cost_heatmap_gas_per_tx.png","Cost Heatmap: Gas/tx"),
        ("figures/sensitivity_heatmap.png",    "Factor Sensitivity Heatmap"),
    ]

    lines = ["## 7. Figures\n"]
    for path, caption in figure_map:
        if os.path.exists(path):
            lines.append(f"### {caption}\n")
            lines.append(f"![{caption}]({path})\n")
        else:
            lines.append(f"- *{caption}* — `{path}` *(not yet generated)*\n")

    return "\n".join(lines) + "\n"


# ── main ──────────────────────────────────────────────────────────────────────

def generate(
    results_csv: str,
    stats_csv: str | None,
    output: str,
    figures_dir: str = "figures",
):
    if not os.path.exists(results_csv):
        print(f"[generate_md] Input not found: {results_csv}")
        sys.exit(1)

    df = pd.read_csv(results_csv)
    stats_df = pd.read_csv(stats_csv) if stats_csv and os.path.exists(stats_csv) else pd.DataFrame()

    print(f"[generate_md] Building report from {len(df)} rows ...")

    sections = [
        "# RollupX — Thesis Benchmark Summary\n",
        f"> Auto-generated from `{results_csv}`  \n",
        f"> Generated: {datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M UTC')}\n\n",
        "---\n\n",
        section_overview(df, stats_df),
        section_results_table(df),
        section_rankings(df),
        section_baseline_comparison(df),
        section_hypotheses(df),
        section_threats(df),
        section_figures(figures_dir),
        "---\n\n*End of auto-generated summary.*\n",
    ]

    os.makedirs(os.path.dirname(output) if os.path.dirname(output) else ".", exist_ok=True)
    with open(output, "w") as f:
        f.write("\n".join(sections))

    print(f"[generate_md] written → {output}")


def main():
    p = argparse.ArgumentParser(description="Generate RollupX thesis summary markdown")
    p.add_argument("--input",       default="metrics/all_results.csv")
    p.add_argument("--stats",       default="metrics/stats_summary.csv")
    p.add_argument("--output",      default="thesis_summary.md")
    p.add_argument("--figures_dir", default="figures")
    args = p.parse_args()

    generate(args.input, args.stats, args.output, args.figures_dir)


if __name__ == "__main__":
    main()
