"""
plots/throughput_bar.py — Grouped bar chart comparing offered vs accepted vs committed TPS.

Reads:  metrics/all_results.csv  (or --input)
Writes: figures/throughput_by_<factor>.png
"""

import argparse
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

THROUGHPUT_COLS = [
    ("tps_offered",   "TPS Offered",   "#4C72B0"),
    ("tps_accepted",  "TPS Accepted",  "#55A868"),
    ("tps_committed", "TPS Committed", "#C44E52"),
]

FACTOR_GROUPS = {
    "policy":     "policy",
    "batch_size": "batch_size",
    "da_mode":    "da_mode",
    "rate":       "tps_offered",
}


def plot_throughput(df: pd.DataFrame, group_col: str, output_dir: str, label: str):
    os.makedirs(output_dir, exist_ok=True)

    # average across repeats per experiment
    agg = df.groupby("experiment_id")[[c for c, _, _ in THROUGHPUT_COLS]].mean().reset_index()

    # try to sort by group_col value
    if group_col in df.columns:
        order = df.groupby("experiment_id")[group_col].first()
        agg["_sort"] = agg["experiment_id"].map(order)
        try:
            agg["_sort"] = pd.to_numeric(agg["_sort"])
        except Exception:
            pass
        agg = agg.sort_values("_sort")

    x = np.arange(len(agg))
    width = 0.25

    fig, ax = plt.subplots(figsize=(max(8, len(agg) * 1.5), 5))

    for i, (col, lbl, color) in enumerate(THROUGHPUT_COLS):
        if col in agg.columns:
            vals = agg[col].fillna(0)
            bars = ax.bar(x + i * width, vals, width, label=lbl, color=color, alpha=0.85)
            ax.bar_label(bars, fmt="%.1f", fontsize=7, padding=2)

    ax.set_xticks(x + width)
    ax.set_xticklabels(agg["experiment_id"], rotation=30, ha="right", fontsize=8)
    ax.set_ylabel("Transactions per Second")
    ax.set_title(f"Throughput Comparison — grouped by {label}")
    ax.legend()
    ax.grid(axis="y", alpha=0.3)
    ax.set_ylim(bottom=0)
    fig.tight_layout()

    out = os.path.join(output_dir, f"throughput_by_{label.lower().replace(' ','_')}.png")
    fig.savefig(out, dpi=150)
    plt.close(fig)
    print(f"[throughput_bar] saved → {out}")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--input",      default="metrics/all_results.csv")
    p.add_argument("--output_dir", default="figures")
    p.add_argument("--factor",     default=None, help="Specific factor to plot (default: all)")
    args = p.parse_args()

    if not os.path.exists(args.input):
        print(f"Input not found: {args.input}")
        sys.exit(1)

    df = pd.read_csv(args.input)

    factors_to_plot = (
        {args.factor: FACTOR_GROUPS.get(args.factor, args.factor)}
        if args.factor
        else FACTOR_GROUPS
    )

    for factor_name, col in factors_to_plot.items():
        if col in df.columns or col in df.select_dtypes(include="number").columns:
            # filter to relevant experiments
            factor_df = df[
                df["experiment_id"].str.startswith(factor_name[:3]) |
                (df["experiment_id"] == "baseline")
            ]
            if not factor_df.empty:
                plot_throughput(factor_df, col, args.output_dir, factor_name)
        else:
            print(f"[SKIP] Column '{col}' not found for factor '{factor_name}'")


if __name__ == "__main__":
    main()
