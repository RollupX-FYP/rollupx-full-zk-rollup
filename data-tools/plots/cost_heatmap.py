"""
plots/cost_heatmap.py — Heatmap of avg_gas_per_tx across batch_size × da_mode.

Reads:  metrics/all_results.csv
Writes: figures/cost_heatmap.png
"""

import argparse
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import pandas as pd
import numpy as np


def plot_heatmap(df: pd.DataFrame, x_col: str, y_col: str, value_col: str,
                 output_dir: str, title: str, filename: str):
    os.makedirs(output_dir, exist_ok=True)

    if value_col not in df.columns:
        print(f"[cost_heatmap] Column '{value_col}' not found — skipping")
        return

    pivot = (
        df.groupby([x_col, y_col])[value_col]
        .mean()
        .reset_index()
        .pivot(index=y_col, columns=x_col, values=value_col)
    )
    if pivot.empty:
        return

    fig, ax = plt.subplots(figsize=(max(6, len(pivot.columns) * 1.2), max(4, len(pivot) * 0.8)))
    im = ax.imshow(pivot.values, aspect="auto", cmap="YlOrRd")

    # labels
    ax.set_xticks(np.arange(len(pivot.columns)))
    ax.set_yticks(np.arange(len(pivot.index)))
    ax.set_xticklabels(pivot.columns, rotation=30, ha="right")
    ax.set_yticklabels(pivot.index)
    ax.set_xlabel(x_col)
    ax.set_ylabel(y_col)
    ax.set_title(title)

    # annotate cells
    for i in range(len(pivot.index)):
        for j in range(len(pivot.columns)):
            val = pivot.values[i, j]
            if not np.isnan(val):
                ax.text(j, i, f"{val:.0f}", ha="center", va="center",
                        fontsize=8, color="black" if val < pivot.values.max() * 0.6 else "white")

    fig.colorbar(im, ax=ax, label=value_col)
    fig.tight_layout()
    out = os.path.join(output_dir, filename)
    fig.savefig(out, dpi=150)
    plt.close(fig)
    print(f"[cost_heatmap] saved → {out}")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--input",      default="metrics/all_results.csv")
    p.add_argument("--output_dir", default="figures")
    args = p.parse_args()

    if not os.path.exists(args.input):
        print(f"Input not found: {args.input}")
        sys.exit(1)

    df = pd.read_csv(args.input)

    # Gas/tx: batch_size × da_mode
    plot_heatmap(
        df, x_col="batch_size", y_col="da_mode", value_col="avg_gas_per_tx",
        output_dir=args.output_dir,
        title="Avg Gas per Transaction: Batch Size × DA Mode",
        filename="cost_heatmap_gas_per_tx.png",
    )

    # Compression ratio: batch_size × da_mode
    plot_heatmap(
        df, x_col="batch_size", y_col="da_mode", value_col="avg_comp_ratio",
        output_dir=args.output_dir,
        title="Avg Compression Ratio: Batch Size × DA Mode",
        filename="cost_heatmap_comp_ratio.png",
    )

    # Latency: batch_size × policy
    plot_heatmap(
        df, x_col="batch_size", y_col="policy", value_col="avg_l2_l1_ms",
        output_dir=args.output_dir,
        title="Avg L2→L1 Latency (ms): Batch Size × Policy",
        filename="cost_heatmap_latency.png",
    )


if __name__ == "__main__":
    main()
