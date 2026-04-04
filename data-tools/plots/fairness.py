"""
plots/fairness.py — Fairness analysis plots.

Plots:
  1. Jain's Fairness Index by scheduling policy
  2. P95 latency by tx type (A / B / C) per policy
  3. Starvation count per experiment

Reads:  metrics/all_results.csv
Writes: figures/fairness_jains.png
        figures/fairness_per_class.png
        figures/starvation.png
"""

import argparse
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd


def _save(fig, path: str):
    fig.tight_layout()
    fig.savefig(path, dpi=150)
    plt.close(fig)
    print(f"[fairness] saved → {path}")


def plot_jains(df: pd.DataFrame, output_dir: str):
    """Bar chart of mean Jain's Fairness Index per experiment."""
    if "jains_fairness" not in df.columns:
        print("[fairness] jains_fairness column not found — skipping")
        return

    agg = (
        df.groupby("experiment_id")["jains_fairness"]
        .agg(["mean", "std"])
        .reset_index()
        .dropna(subset=["mean"])
        .sort_values("mean", ascending=False)
    )
    if agg.empty:
        return

    # highlight policy experiments
    policy_mask = agg["experiment_id"].str.startswith("pol_") | (agg["experiment_id"] == "baseline")
    colors = ["#2196F3" if m else "#BBDEFB" for m in policy_mask]

    fig, ax = plt.subplots(figsize=(max(8, len(agg) * 1.0), 5))
    bars = ax.bar(
        agg["experiment_id"], agg["mean"],
        yerr=agg["std"].fillna(0), capsize=4,
        color=colors, alpha=0.85, error_kw={"elinewidth": 1.2},
    )
    ax.bar_label(bars, fmt="%.3f", fontsize=7, padding=3)
    ax.axhline(1.0, color="green", linestyle="--", linewidth=1, label="Perfect fairness (1.0)")
    ax.set_xlabel("Experiment")
    ax.set_ylabel("Jain's Fairness Index")
    ax.set_title("Fairness Index per Experiment (higher = fairer)")
    ax.set_ylim(0, 1.15)
    ax.tick_params(axis="x", rotation=35)
    ax.legend()
    ax.grid(axis="y", alpha=0.3)
    _save(fig, os.path.join(output_dir, "fairness_jains.png"))


def plot_per_class_p95(df: pd.DataFrame, output_dir: str):
    """Grouped bar chart of P95 latency per tx class for policy experiments."""
    cols = {
        "A": "p95_latency_typeA_ms",
        "B": "p95_latency_typeB_ms",
        "C": "p95_latency_typeC_ms",
    }
    available = {k: v for k, v in cols.items() if v in df.columns}
    if not available:
        print("[fairness] per-class P95 columns not found — skipping")
        return

    # filter to policy / baseline experiments
    mask = df["experiment_id"].str.startswith("pol_") | (df["experiment_id"] == "baseline")
    sub = df[mask].groupby("experiment_id")[list(available.values())].mean().reset_index()

    if sub.empty:
        return

    x = np.arange(len(sub))
    width = 0.25
    colors = {"A": "#4CAF50", "B": "#FF9800", "C": "#F44336"}

    fig, ax = plt.subplots(figsize=(max(8, len(sub) * 1.5), 5))
    for i, (tx_type, col) in enumerate(available.items()):
        vals = sub[col].fillna(0)
        ax.bar(x + i * width, vals, width, label=f"Type {tx_type}", color=colors[tx_type], alpha=0.85)

    ax.set_xticks(x + width)
    ax.set_xticklabels(sub["experiment_id"], rotation=20, ha="right")
    ax.set_ylabel("P95 Latency (ms)")
    ax.set_title("P95 Latency by Transaction Type and Scheduling Policy")
    ax.legend()
    ax.grid(axis="y", alpha=0.3)
    _save(fig, os.path.join(output_dir, "fairness_per_class.png"))


def plot_starvation(df: pd.DataFrame, output_dir: str):
    """Bar chart of starvation count per experiment."""
    if "starvation_count" not in df.columns:
        return

    agg = df.groupby("experiment_id")["starvation_count"].mean().reset_index().sort_values(
        "starvation_count", ascending=False
    )
    if agg.empty:
        return

    fig, ax = plt.subplots(figsize=(max(8, len(agg) * 1.0), 4))
    ax.bar(agg["experiment_id"], agg["starvation_count"], color="#E53935", alpha=0.8)
    ax.set_xlabel("Experiment")
    ax.set_ylabel("Mean Starvation Count")
    ax.set_title("Starvation Events per Experiment (txs waiting > 3× mean latency)")
    ax.tick_params(axis="x", rotation=35)
    ax.grid(axis="y", alpha=0.3)
    _save(fig, os.path.join(output_dir, "starvation.png"))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--input",      default="metrics/all_results.csv")
    p.add_argument("--output_dir", default="figures")
    args = p.parse_args()

    if not os.path.exists(args.input):
        print(f"Input not found: {args.input}")
        sys.exit(1)

    os.makedirs(args.output_dir, exist_ok=True)
    df = pd.read_csv(args.input)

    plot_jains(df, args.output_dir)
    plot_per_class_p95(df, args.output_dir)
    plot_starvation(df, args.output_dir)


if __name__ == "__main__":
    main()
